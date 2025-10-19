use {
    crate::{
        controller::{Controller, ControllerEvent},
        exports::runtime::bindings::{GameControl, GameControls, TaimiControls},
        render::machine::RenderTaskPriority,
        settings::{Settings, SettingsLock},
        space::{
            engine::SpaceEvent, pack::LoaderBox, Engine
        },
    },
    anyhow::{anyhow, Context},
    futures::FutureExt,
    std::{
        fs::exists,
        path::PathBuf,
        sync::Arc,
    }, strum_macros::Display, taimi_meta::ui::MapContext, taimi_pack::Pack, tokio::{
        fs::create_dir_all,
        select,
        time::{sleep, Duration},
    }
};

#[cfg(feature = "space")]
#[derive(Debug, Clone, Display)]
pub(crate) enum PathingEvent {
    VisibleToggle {
        context: Option<MapContext>,
        set: Option<bool>,
    },
    PathingLoadAll,
    PathingUnloadAll,
    RequestDisabledPaths,
    PathingStateUpdate(String, bool),
    ToggleKatRender,
}

#[derive(Default, Debug)]
pub(crate) struct PathingController {
}

impl PathingController {
    async fn pathing_state_update(&mut self, path: String, state: bool) {
        let mut settings_lock = Settings::async_write().await.expect("Settings unitialized, impossible");
        crate::settings::PathingSettings::pathing_state_update(&mut settings_lock, path, state).await;
        drop(settings_lock);

    }

    async fn pathing_load_all(&self) {
        let res = self.pathing_load_all_inner().await
            .context("Loading all paths");
        if let Err(e) = res {
            log::error!("{e}");
        }
    }

    async fn pathing_load_all_inner(&self) -> anyhow::Result<()> {
        use tokio::fs::read_dir;

        let pathing_dir = crate::ADDON_DIR.join("pathing");
        if !exists(&pathing_dir).unwrap_or(false) {
            create_dir_all(&pathing_dir).await?;
        }

        let mut path_loads = tokio::task::JoinSet::new();

        log::info!("Pre-loading all paths...");
        let mut dir = read_dir(pathing_dir).await?;
        loop {
            let entry = match dir.next_entry().await {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(e) => {
                    log::error!("Failed to list pathing files: {e}");
                    continue
                },
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let context = format!("Loading pathing pack {name}");
            log::debug!("{context}...");
            let path = entry.path();
            let is_taco = path.extension().map(|e| e.eq_ignore_ascii_case("taco") || e.eq_ignore_ascii_case("zip"));
            let is_taco = path.is_file() || is_taco.unwrap_or(false);
            let loader = move || match is_taco {
                true => Self::pathing_load_taco(path),
                false => Self::pathing_load_dir(path),
            }.context(context);
            let loader = async move {
                let res = tokio::task::spawn_blocking(loader).await
                    .context("Path load panicked");
                match res {
                    Ok(Ok((pack, loader))) => {
                        Self::pathing_load_pack(pack, loader, name).await;
                        Ok(())
                    },
                    Err(e) | Ok(Err(e)) => {
                        Err(e)
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
                }.map(|r| r.context("Path load panicked"));
                match res {
                    None => break,
                    Some(Err(e) | Ok(Err(e))) =>
                        log::error!("{e:#}"),
                    Some(Ok(Ok(()))) =>
                        disabled_paths_dirty = true,
                }
            }

            // TODO: sender+await, or ideally just make this unnecessary

            if disabled_paths_dirty {
                Self::try_send(PathingEvent::RequestDisabledPaths);
            }
        });

        Ok(())
    }

    async fn toggle_katrender(&mut self) {
        let mut settings_lock = Settings::async_write().await.expect("Settings unitialized, impossible");
        settings_lock.toggle_katrender();
        drop(settings_lock);
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
                Some(res) => res.as_mut()
                    .map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            engine.packs.fixup_pack(&mut pack);
            let pack = Arc::new(pack);
            let pack_idx = engine.packs.add_pack(pack, loader);
            engine.packs.load_pack(&engine.render_backend.device, pack_idx)
        }).await;
        let res = res.map(|res| res.context(context))
            .context("Submitting pack to engine");
        if let Err(e) | Ok(Err(e)) = res {
            log::error!("{e:#}");
        }
    }

    async fn pathing_unload_all(&self) {
        log::info!("Unloading all paths...");
        let context = "Unloading packs from engine";
        let res = Controller::run_render(RenderTaskPriority::High, move |state| -> anyhow::Result<()> {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut()
                    .map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            engine.packs.clear();
            Ok(())
        }).await;
        let res = res.map(|res| res.context(context))
            .context(context);
        if let Err(e) | Ok(Err(e)) = res {
            log::error!("{e:#}");
        }
    }

    async fn provide_disabled_paths(&self, settings: SettingsLock) {
        let settings_lock = settings.read().await;
        let disabled_paths = settings_lock.disabled_paths.clone();
        drop(settings_lock);

        let context = "Providing disabled paths to engine";
        let res = Controller::run_render(RenderTaskPriority::Normal, move |state| -> anyhow::Result<()> {
            let engine = match &mut state.engine {
                Some(res) => res.as_mut()
                    .map_err(|e| anyhow!("{e:#}")),
                None => return Ok(()),
            }?;
            engine.disable_paths(&state.machine, disabled_paths);
            Ok(())
        }).await;
        let res = res.map(|res| res.context(context))
            .context(context);
        if let Err(e) | Ok(Err(e)) = res {
            log::error!("{e:#}");
        }
    }

    pub(crate) async fn handle_event(&mut self, event: PathingEvent, settings: &SettingsLock) {
        use PathingEvent::*;
        match event {
            PathingLoadAll => self.pathing_load_all().await,
            PathingUnloadAll => self.pathing_unload_all().await,
            RequestDisabledPaths => self.provide_disabled_paths(settings.clone()).await,
            PathingStateUpdate(p, s) => self.pathing_state_update(p, s).await,
            ToggleKatRender => self.toggle_katrender().await,
            VisibleToggle { context, set } => self.set_visible(context, set).await,
        }
    }

    pub(crate) async fn set_visible(&mut self, context: Option<MapContext>, set: Option<bool>) {
        let Ok(mut settings) = Settings::async_write().await else {
            return
        };

        let pathing = settings.pathing_mut();
        let (is_visible, out) = match context {
            Some(MapContext::Global) => (pathing.space.visible_worldmap(), &mut pathing.space.visible_map_world),
            Some(MapContext::Minimap) =>(pathing.space.visible_minimap(),  &mut pathing.space.visible_map_mini),
            None => (pathing.space.visible_space(), &mut pathing.space.visible_space),
        };
        let set = set.unwrap_or(!is_visible);
        *out = Some(set);

        #[cfg(feature = "goggles")]
        match (context, set) {
            (None, true) => Engine::try_send(SpaceEvent::GogglesRefreshLens { force: false, delay_override: Some(2) }),
            (None, false) => Engine::try_send(SpaceEvent::GogglesClearLens),
            _ => (),
        }
        Engine::try_send(SpaceEvent::SettingsDirty);
    }

    pub(crate) async fn handle_keybinds(&mut self, state: TaimiControls, changed: TaimiControls) {
        let pressed = state & changed;
        if pressed.intersects(TaimiControls::PATHING_SPACE) {
            self.set_visible(None, None).await;
        }
        if pressed.intersects(TaimiControls::PATHING_MAP) {
            self.set_visible(Some(MapContext::Global), None).await;
        }
        if pressed.intersects(TaimiControls::PATHING_MINIMAP) {
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
        log::debug!("TODO: player interaction");
    }
    
    #[inline]
    pub fn try_send(e: PathingEvent) {
        Controller::try_send(e.into())
    }
}

impl From<PathingEvent> for ControllerEvent {
    fn from(e: PathingEvent) -> Self {
        ControllerEvent::Pathing(e)
    }
}

impl PathingEvent {
    #[inline]
    pub fn try_send(self) {
        PathingController::try_send(self);
    }

    pub const VISIBLE_TOGGLE_SPACE: Self = Self::VisibleToggle {
        context: None,
        set: None,
    };
    pub const fn visible_toggle(context: MapContext) -> Self {
        Self::VisibleToggle { context: Some(context), set: None }
    }
}
