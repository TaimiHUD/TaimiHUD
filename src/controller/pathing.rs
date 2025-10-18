use {
    crate::{
        controller::ControllerEvent,
        exports::runtime::bindings::{GameControl, GameControls, TaimiControls},
        settings::{Settings, SettingsLock},
        space::{
            engine::SpaceEvent, pack::LoaderBox, Engine
        },
    },
    anyhow::Context,
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
    PathingLoadAll,
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

    async fn provide_disabled_paths(&self, settings: SettingsLock) {
        let settings_lock = settings.read().await;
        let disabled_paths = settings_lock.disabled_paths.clone();
        drop(settings_lock);
        if let Some(sender) = Engine::sender() {
            let _event_send = sender.send(SpaceEvent::DisabledPaths(disabled_paths)).await;
        }
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
            let loader = move || {
                let res = if path.is_file() || is_taco.unwrap_or(false) {
                    Self::pathing_load_taco(name, path)
                } else {
                    Self::pathing_load_dir(name, path)
                }.context(context);

                if let Err(e) = &res {
                    log::error!("Path load failed: {e:#}");
                }
                res.is_ok()
            };
            path_loads.spawn_blocking(loader);
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
                };
                match res {
                    None => break,
                    Some(Err(e)) =>
                        log::error!("Path load panicked: {e}"),
                    Some(Ok(true)) =>
                        disabled_paths_dirty = true,
                    Some(Ok(..)) => (),
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

    fn pathing_load_taco(name: String, path: PathBuf) -> anyhow::Result<()> {
        use taimi_pack::loader::ZipLoader;
        let mut loader = ZipLoader::new(&path)?;
        let pack = Pack::load(&mut loader)?;
        Self::pathing_load_pack(pack, Box::new(loader), name);
        Ok(())
    }

    fn pathing_load_dir(name: String, path: PathBuf) -> anyhow::Result<()> {
        use taimi_pack::loader::DirectoryLoader;
        let mut loader = DirectoryLoader::new(path);
        let pack = Pack::load(&mut loader)?;
        Self::pathing_load_pack(pack, Box::new(loader), name);
        Ok(())
    }

    fn pathing_load_pack(mut pack: Pack, loader: LoaderBox, name: String) {
        if pack.name.is_empty() {
            pack.name = name;
        }
        let event = SpaceEvent::PackLoad {
            pack: Arc::new(pack),
            loader,
        };
        // TODO: await!
        if let Some(sender) = Engine::sender() {
            let _ = sender.blocking_send(event);
        }
    }

    pub(crate) async fn handle_event(&mut self, event: PathingEvent, settings: &SettingsLock) {
        use PathingEvent::*;
        match event {
            PathingLoadAll => self.pathing_load_all().await,
            RequestDisabledPaths => self.provide_disabled_paths(settings.clone()).await,
            PathingStateUpdate(p, s) => self.pathing_state_update(p, s).await,
            ToggleKatRender => self.toggle_katrender().await,
        }
        
    }

    pub(crate) async fn handle_keybinds(&mut self, state: TaimiControls, changed: TaimiControls) {
        let pressed = state & changed;
        if pressed.intersects(TaimiControls::PATHING_SPACE) {
            Engine::try_send(SpaceEvent::PathingToggle);
        }
        if pressed.intersects(TaimiControls::PATHING_MAP) {
            Engine::try_send(SpaceEvent::MapToggle(MapContext::Global));
        }
        if pressed.intersects(TaimiControls::PATHING_MINIMAP) {
            Engine::try_send(SpaceEvent::MapToggle(MapContext::Minimap));
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
    
    pub fn try_send(e: PathingEvent) {
        let sender = crate::CONTROLLER_SENDER.try_read();
        let sender = sender.as_ref().map(|s| &**s);
        let full_e = ControllerEvent::Pathing(e);
        if let Ok(Some(sender)) = sender {
            let _ = sender.try_send(full_e);
        }
    }
}
