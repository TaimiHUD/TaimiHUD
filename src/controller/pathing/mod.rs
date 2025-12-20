use {
    crate::{
        controller::{
            api::{AchievementState, RaidState},
            Controller,
        },
        exports::runtime::{
            self as rt,
            bindings::{
                ControlsReceiver,
                GameControl,
                GameControls,
                TaimiControls,
                TaimiReceiver,
                CONTROLS,
            },
        },
        render::machine::RenderTaskPriority,
        settings::{Settings, SettingsLock, SourceKind},
        space::{
            engine::SpaceEvent,
            Engine,
        },
        Interruption,
        InterruptionSignal,
    },
    anyhow::{anyhow, Context},
    futures::{FutureExt, StreamExt},
    std::{path::PathBuf, sync::Arc},
    strum_macros::Display,
    taimi_meta::ui::MapContext,
    taimi_pack::{attributes::Festivals, category::CategoryId, Pack},
    tokio::{
        fs::create_dir_all,
        select,
        time::{sleep, Duration},
    },
};

pub use self::{
    registry::{LoaderBox, UnloadedReason},
    festivals::FestivalFixup,
    shared::{PathingEnables, PathingReceiver, PathingSender, PathingShared},
    state::visible,
};
pub use taimi_meta::coords::LocalSpace as PackSpace;

mod festivals;
pub mod registry;
mod shared;
mod state;
pub mod space;

pub type ExternalFilterState = (Festivals, Arc<RaidState>, Arc<AchievementState>);

#[derive(Debug, Clone, Display)]
pub(crate) enum PathingEvent {
    VisibleToggle { context: Option<MapContext>, set: Option<bool> },
    ReloadAll(bool),
    LoadAll,
    UnloadAll,
    RequestDisabledPaths,
    PathingStateUpdate(CategoryId, bool),
    ToggleKatRender,
    ApiBypass(Option<bool>),
    Exit(Interruption),
}

pub(crate) struct PathingController {
    rx: PathingReceiver,
    controls: ControlsReceiver,
    keybinds: TaimiReceiver,
    settings: SettingsLock,
}

impl PathingController {
    pub fn new(rx: PathingReceiver, settings: SettingsLock) -> Self {
        Self {
            rx,
            controls: CONTROLS.subscribe_controls(),
            keybinds: CONTROLS.subscribe_taimi(),
            settings,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut enables = self.rx.enables.borrow().clone();
        {
            let settings = self.settings.read().await;
            enables.set(PathingEnables::KATRENDER, settings.enable_katrender);
        }
        self.rx.enables.send_replace(enables);
        let _interruption = loop {
            select! {
                e = self.rx.command.recv() => {
                    let res = match e {
                        None => break Interruption::Unspecified,
                        Some(e) =>
                            self.handle_event(e).await,
                    };
                    match res {
                        Some(int) => break int,
                        None => (),
                    }
                },
                _ = self.rx.festivals.changed() => {
                    self.provide_disabled_paths().await;
                },
                controls = self.controls.wait() => match controls {
                    Err(e) => log::error!("Control bindings error! {e:#}"),
                    Ok((&controls_state, controls_changed)) => {
                        self.handle_presses(controls_state, controls_changed).await;
                    },
                },
                keybinds = self.keybinds.wait() => match keybinds {
                    Err(e) => log::error!("Keybind receive error! {e:#}"),
                    Ok((binds_state, binds_changed)) => {
                        self.handle_keybinds(binds_state, binds_changed).await;
                    },
                },
            }
        };
        Ok(())
    }

    async fn pathing_state_update(&mut self, path: CategoryId, state: bool) {
        let mut settings_lock = Settings::async_write()
            .await
            .expect("Settings unitialized, impossible");
        crate::settings::PathingSettings::pathing_state_update(&mut settings_lock, path.into(), state)
            .await;
        drop(settings_lock);
    }

    async fn toggle_katrender(&self) {
        let mut settings_lock = Settings::async_write()
            .await
            .expect("Settings unitialized, impossible");
        settings_lock.toggle_katrender();
        let katrender = settings_lock.enable_katrender;
        drop(settings_lock);
        self.rx
            .enables
            .send_modify(|en| en.set(PathingEnables::KATRENDER, katrender));
    }

    fn toggle_api_bypass(&self, set: Option<bool>) {
        self.rx.enables.send_modify(|en| match set {
            Some(set) => en.set(PathingEnables::API_BYPASS, set),
            None => en.toggle(PathingEnables::API_BYPASS),
        });
    }

    async fn provide_disabled_paths(&self) {
        log::debug!("TODO: provide_disabled_paths (filter/config dirty bs)");
    }
}

/// moving back to registry loader Soon
#[cfg(todo = "deleteme")]
impl PathingController {
    pub(crate) async fn reload_all(&self, remove: bool) {
        if !remove {
            log::debug!("TODO: pack refresh rather than reload");
        }
        self.unload_all().await;
        let res = Self::load_all_inner(self.settings.clone())
            .await
            .context("Reloading all paths");
        if let Err(e) = res {
            log::error!("{e:#}");
        }
    }

    async fn load_all(&self) {
        let res = Self::load_all_inner(self.settings.clone())
            .await
            .context("Loading all paths");
        if let Err(e) = res {
            log::error!("{e}");
        }
    }

    async fn load_all_inner(settings: SettingsLock) -> anyhow::Result<()> {
        let _ = create_dir_all(SourceKind::Pathing.get_user_dir());

        let mut path_loads = tokio::task::JoinSet::new();

        log::info!("Pre-loading all paths...");
        let dir = Settings::read_source_dir(settings, SourceKind::Pathing).await;
        futures::pin_mut!(dir);
        while let Some(entry) = dir.next().await {
            let (path, _source) = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::error!("Failed to list pathing files: {e}");
                    continue
                },
            };
            // TODO: name could be source? what do we actually use that for, and is it meant to be user-facing or a unique id?
            let name = path
                .file_name()
                .unwrap_or(path.as_ref())
                .to_string_lossy()
                .into_owned();
            let context = format!("Loading pathing pack {name}");
            log::debug!("{context}...");
            let is_taco = path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("taco") || e.eq_ignore_ascii_case("zip"));
            let is_taco = path.is_file() || is_taco.unwrap_or(false);
            let loader = move || {
                match is_taco {
                    true => Self::pathing_load_taco(path),
                    false => Self::pathing_load_dir(path),
                }
                .context(context)
            };
            let loader = async move {
                let res = tokio::task::spawn_blocking(loader)
                    .await
                    .context("Path load panicked");
                match res {
                    Ok(Ok((pack, loader))) => {
                        Self::pathing_load_pack(pack, loader, name).await;
                        Ok(())
                    },
                    Err(e) | Ok(Err(e)) => {
                        let e = rt::log::error_into_arc(e);
                        Self::pathing_notify_pack_error(
                            name,
                            UnloadedReason::LoadingFailed(e.clone()),
                        )
                        .await;
                        Err(e.into())
                    },
                }
            };
            path_loads.spawn(loader);
        }

        tokio::spawn(async move {
            let mut disabled_paths_dirty = false;
            loop {
                let pack_load = path_loads.join_next();
                let res = if disabled_paths_dirty {
                    // throttle repeated state event if packs load quickly enough...
                    let timeout = sleep(Duration::from_millis(174)).fuse();
                    tokio::pin!(timeout);
                    tokio::pin!(pack_load);
                    loop {
                        select! {
                            res = &mut pack_load => break res,
                            _ = &mut timeout => {
                                // this will take a while, so emit the pending update
                                Self::try_send(PathingEvent::RequestDisabledPaths);
                                disabled_paths_dirty = false;
                            },
                        }
                    }
                } else {
                    pack_load.await
                }
                .map(|r| r.context("Path load panicked"));
                match res {
                    None => break,
                    Some(Err(e) | Ok(Err(e))) => log::error!("{e:#}"),
                    Some(Ok(Ok(()))) => disabled_paths_dirty = true,
                }
            }

            // TODO: sender+await, or ideally just make this unnecessary

            if disabled_paths_dirty {
                Self::try_send(PathingEvent::RequestDisabledPaths);
            }
        });

        Ok(())
    }

    fn pathing_load_taco(path: PathBuf) -> anyhow::Result<(Pack, LoaderBox)> {
        use taimi_pack::loader::ZipLoader;
        let mut loader = ZipLoader::new(&path)?;
        let pack = Pack::load(&mut loader)?;
        Ok((pack, Box::new(loader)))
    }

    fn pathing_load_dir(path: PathBuf) -> anyhow::Result<(Pack, LoaderBox)> {
        use taimi_pack::loader::DirectoryLoader;
        let mut loader = DirectoryLoader::new(path);
        let pack = Pack::load(&mut loader)?;
        Ok((pack, Box::new(loader)))
    }

    async fn pathing_load_pack(mut pack: Pack, loader: LoaderBox, name: String) {
        let context = format!("Loading pack {name} onto engine");
        if pack.name.is_empty() {
            pack.name = name;
        }
        let res = Controller::run_render(RenderTaskPriority::High, move |state| {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut().map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            engine.packs.fixup_pack(&mut pack);
            let pack = Arc::new(pack);
            let pack_idx = engine.packs.add_pack(pack, loader);
            engine.packs.load_pack(&engine.render_backend.device, pack_idx)
        })
        .await;
        let res = res
            .map(|res| res.context(context))
            .context("Submitting pack to engine");
        if let Err(e) | Ok(Err(e)) = res {
            log::error!("{e:#}");
        }
    }
    async fn pathing_notify_pack_error(name: String, reason: UnloadedReason) {
        let _ = Controller::run_render(RenderTaskPriority::Normal, move |state| {
            let engine = match &mut state.engine {
                Some(Ok(e)) => e,
                _ => return,
            };
            engine.packs.load_failed(name, reason);
        })
        .await;
    }

    async fn unload_all(&self) {
        log::info!("Unloading all paths...");
        if let Err(e) = Self::unload_all_inner().await {
            log::error!("{e:#}");
        }
    }

    async fn unload_all_inner() -> anyhow::Result<()> {
        let context = "Unloading packs from engine";
        let res = Controller::run_render(RenderTaskPriority::High, move |state| -> anyhow::Result<()> {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut().map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            engine.packs.clear();
            Ok(())
        })
        .await;
        match res.map(|res| res.context(context)).context(context) {
            Err(e) => Err(e),
            Ok(res) => res,
        }
    }

    async fn provide_disabled_paths(&self) {
        let disabled_paths = {
            let settings_lock = self.settings.read().await;
            settings_lock.disabled_paths.clone()
        };

        let context = "Providing disabled paths to engine";
        let res = Controller::run_render(RenderTaskPriority::Normal, move |state| -> anyhow::Result<()> {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut().map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            engine.disable_paths(&disabled_paths);
            Ok(())
        })
        .await;
        let res = res.map(|res| res.context(context)).context(context);
        if let Err(e) | Ok(Err(e)) = res {
            log::error!("{e:#}");
        }
    }
}

impl PathingController {
    pub(crate) async fn handle_event(&mut self, event: PathingEvent) -> Option<Interruption> {
        use PathingEvent::*;
        match event {
            Exit(interruption) => return Some(interruption),
            #[cfg(deleteme)]
            ReloadAll(remove) => self.reload_all(remove).await,
            #[cfg(deleteme)]
            LoadAll => self.load_all().await,
            #[cfg(deleteme)]
            UnloadAll => self.unload_all().await,
            ReloadAll(..) | UnloadAll | LoadAll =>
                log::debug!("TODO: pathing load"),
            RequestDisabledPaths => self.provide_disabled_paths().await,
            PathingStateUpdate(p, s) => self.pathing_state_update(p, s).await,
            ToggleKatRender => self.toggle_katrender().await,
            ApiBypass(set) => self.toggle_api_bypass(set),
            VisibleToggle { context, set } => self.set_visible(context, set).await,
        }
        None
    }

    pub(crate) async fn set_visible(&mut self, context: Option<MapContext>, set: Option<bool>) {
        let Ok(mut settings) = Settings::async_write().await else { return };

        let pathing = settings.pathing_mut();
        let (_control, is_visible, out) = match context {
            Some(MapContext::Global) => (
                TaimiControls::PATHING_MAP,
                pathing.space.visible_worldmap(),
                &mut pathing.space.visible_map_world,
            ),
            Some(MapContext::Minimap) => (
                TaimiControls::PATHING_MINIMAP,
                pathing.space.visible_minimap(),
                &mut pathing.space.visible_map_mini,
            ),
            None => (
                TaimiControls::PATHING_SPACE,
                pathing.space.visible_space(),
                &mut pathing.space.visible_space,
            ),
        };
        let set = set.unwrap_or(!is_visible);
        *out = Some(set);
        drop(settings);

        #[cfg(feature = "extension-nexus")]
        crate::QUICK_ACCESS_STATE.send_if_modified(|state| {
            if state.contains(_control) != set {
                state.toggle(_control);
                true
            } else {
                false
            }
        });

        #[cfg(feature = "goggles")]
        match (context, set) {
            (None, true) =>
                Engine::try_send(SpaceEvent::GogglesRefreshLens { force: false, delay_override: Some(2) }),
            (None, false) => Engine::try_send(SpaceEvent::GogglesClearLens),
            _ => (),
        }
        Engine::try_send(SpaceEvent::SettingsDirty);
    }

    async fn handle_keybinds(&mut self, state: TaimiControls, changed: TaimiControls) {
        let pressed = state & changed;
        if pressed.intersects(TaimiControls::PATHING_SPACE) {
            CONTROLS.notify_handled(TaimiControls::PATHING_SPACE);
            self.set_visible(None, None).await;
        }
        if pressed.intersects(TaimiControls::PATHING_MAP) {
            CONTROLS.notify_handled(TaimiControls::PATHING_MAP);
            self.set_visible(Some(MapContext::Global), None).await;
        }
        if pressed.intersects(TaimiControls::PATHING_MINIMAP) {
            CONTROLS.notify_handled(TaimiControls::PATHING_MINIMAP);
            self.set_visible(Some(MapContext::Minimap), None).await;
        }
    }

    pub(crate) async fn handle_presses(&mut self, state: GameControls, changed: GameControls) {
        let pressed = state & changed;
        if pressed.contains(GameControl::Miscellaneous_Interact) {
            self.handle_press_interact().await;
        }
    }

    pub(crate) async fn handle_press_interact(&mut self) {
        log::trace!("TODO: player interaction");
    }

    pub fn external_filter_state() -> Option<ExternalFilterState> {
        Controller::with_sender(|s| {
            let bypass = s
                .pathing
                .as_ref()
                .map(|p| p.enables.borrow().clone())
                .unwrap_or_default()
                .contains(PathingEnables::API_BYPASS);
            s.api.as_ref().map(|a| {
                let festivals = a.festivals.borrow().get();
                let (clears, achievements) = match bypass {
                    true => Default::default(),
                    false => (a.raids.borrow().clone(), a.achievements.borrow().clone()),
                };
                (festivals, clears, achievements)
            })
        })
        .flatten()
    }

    #[inline]
    pub fn try_send(e: PathingEvent) -> bool {
        Controller::with_sender(|s| s.pathing_try_send(e)).unwrap_or(false)
    }
}

impl PathingEvent {
    #[inline]
    pub fn try_send(self) {
        let _ = PathingController::try_send(self);
    }

    pub const VISIBLE_TOGGLE_SPACE: Self = Self::VisibleToggle { context: None, set: None };
    pub const fn visible_toggle(context: MapContext) -> Self {
        Self::VisibleToggle { context: Some(context), set: None }
    }
}
impl InterruptionSignal for PathingEvent {
    fn interrupted(&self) -> Option<Interruption> {
        match self {
            &Self::Exit(reason) => Some(reason),
            _ => None,
        }
    }
}
