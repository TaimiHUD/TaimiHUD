use {
    crate::{
        controller::{Controller, MapId, RtSender},
        exports::runtime::{
            self as rt,
            bindings::{TaimiControls, CONTROLS},
        },
        render::{RenderEvent, TextFont},
        settings::{Settings, SettingsLock, SourceKind},
        timer::{CombatState, Position, TimerFile, TimerMachine},
    },
    anyhow::Context,
    futures::stream::StreamExt,
    std::{collections::HashMap, sync::Arc},
    strum_macros::Display,
    taimi_meta::ui::UiState,
    tokio::{
        fs::{create_dir_all, metadata},
        sync::Mutex,
    },
};

#[derive(Default, Debug)]
pub(crate) struct TimersController {
    pub timers: Vec<Arc<TimerFile>>,
    pub current_timers: Vec<TimerMachine>,
    pub sources_to_timers: HashMap<String, Vec<Arc<TimerFile>>>,
    pub map_id_to_timers: HashMap<u32, Vec<Arc<TimerFile>>>,
}

#[derive(Debug, Clone, Display)]
pub(crate) enum TimersEvent {
    ProgressBarStyle(ProgressBarStyleChange),
    #[strum(to_string = "Id {0}, pressed {1}")]
    TimerKeyTrigger(String, bool),
    ReloadTimers,
    #[allow(dead_code)]
    TimerEnable(String),
    #[allow(dead_code)]
    TimerDisable(String),
    TimerReset,
    #[strum(to_string = "Toggled {0}")]
    TimerToggle(String),
}

#[derive(Debug, Clone, Display)]
pub(crate) enum ProgressBarStyleChange {
    Centre(bool),
    Stock(bool),
    Shadow(bool),
    Height(f32),
    Font(TextFont),
}

impl TimersController {
    pub const TIMERS_NOTABLE_STATE: UiState = UiState::from_bits_retain(UiState::InCombat.bits());

    async fn load(&self, settings: SettingsLock) -> Vec<Arc<TimerFile>> {
        let mut timers = Vec::new();

        let sources = Settings::read_source_dir(settings, SourceKind::Timers).await;
        futures::pin_mut!(sources);
        while let Some(source) = sources.next().await {
            let source = source.context("Error encountered while loading timer sources");

            let (path, source) = match source {
                Ok(s) => s,
                Err(e) => {
                    log::error!("{e:#}");
                    continue
                },
            };

            let res = match metadata(&path).await {
                Ok(m) if m.is_file() => TimerFile::load(&path, source)
                    .await
                    .map(|loaded| timers.push(loaded)),
                _ => TimerFile::load_many(&path, source.as_ref().map(AsRef::as_ref), 100)
                    .await
                    .map(|loaded| timers.extend(loaded)),
            }
            .with_context(|| {
                let path = rt::relative_path(&path);
                format!("Loading timers from {}", path.display())
            });
            match res {
                Err(e) => log::error!("{e:#}"),
                Ok(()) => (),
            }
        }
        log::trace!("Total loaded timers: {}", timers.len());
        timers
    }

    pub(crate) async fn setup(&mut self, settings: SettingsLock, rt_sender: RtSender) {
        log::debug!("Preparing to setup timers");
        let _ = create_dir_all(&SourceKind::Timers.get_user_dir()).await;
        self.timers = self.load(settings).await;
        for timer in &self.timers {
            if let Some(association) = &timer.association {
                self.sources_to_timers.entry(association.clone()).or_default();
                if let Some(val) = self.sources_to_timers.get_mut(association) {
                    val.push(timer.clone());
                };
            }
            // Handle map to timers
            self.map_id_to_timers.entry(timer.map_id).or_default();
            if let Some(val) = self.map_id_to_timers.get_mut(&timer.map_id) {
                val.push(timer.clone());
            };
            let association = match &timer.association {
                Some(s) => format!("{}", s),
                None => "unassociated".to_string(),
            };
            // Handle id to timer file allocation
            log::trace!(
                "Set up {4} {0}: {3} for map {1}, category {2}",
                timer.id,
                timer.name.replace("\n", " "),
                timer.map_id,
                timer.category,
                association,
            );
        }
        log::info!("Set up {} timers.", self.timers.len());
        let _ = rt_sender.send(RenderEvent::TimerData(self.timers.clone())).await;
    }

    pub(crate) async fn handle_event(
        &mut self,
        event: TimersEvent,
        settings: &SettingsLock,
        alert_sem: &Arc<Mutex<()>>,
        map_id: MapId,
        rt_sender: &RtSender,
    ) {
        use TimersEvent::*;
        match event {
            ReloadTimers => self.reload(settings.clone(), rt_sender.clone()).await,
            TimerEnable(id) =>
                self.enable(&id, map_id, alert_sem.clone(), rt_sender.clone())
                    .await,
            TimerDisable(id) => self.disable(&id).await,
            TimerToggle(id) =>
                self.toggle(&id, map_id, alert_sem.clone(), rt_sender.clone())
                    .await,
            TimerReset => self.reset().await,
            TimerKeyTrigger(id, is_release) => self.timer_key_trigger(id, is_release).await,
            ProgressBarStyle(style) => self.progress_bar_style(style, rt_sender.clone()).await,
        }
    }

    pub(crate) async fn handle_keybinds(&mut self, state: TaimiControls, changed: TaimiControls) -> bool {
        let pressed = state & changed;
        if pressed.intersects(TaimiControls::TIMER_RESET) {
            CONTROLS.notify_handled(TaimiControls::TIMER_RESET);
            self.reset().await;
        }
        let triggers = changed & TaimiControls::TIMER_TRIGGERS;
        for trigger in triggers {
            //CONTROLS.notify_handled(trigger);
            let idx = trigger.index() - TaimiControls::TIMER_TRIGGER_0.index();
            self.timer_key_trigger_idx(idx.into(), !state.intersects(trigger))
        }
        !triggers.is_empty()
    }

    pub(crate) async fn reload(&mut self, settings: SettingsLock, rt_sender: RtSender) {
        self.timers.clear();
        self.sources_to_timers.clear();
        self.map_id_to_timers.clear();
        self.setup(settings.clone(), rt_sender).await;
        self.reset().await;
    }

    async fn toggle(&mut self, id: &str, map_id: MapId, alert_sem: Arc<Mutex<()>>, rt_sender: RtSender) {
        let mut settings_lock = Settings::async_write()
            .await
            .expect("Settings unitialized, impossible");
        let disabled = settings_lock.toggle_timer(id.to_string());
        drop(settings_lock);
        match disabled {
            false =>
                if let Some(map_id) = map_id {
                    if let Some(timers_for_map) = &self.map_id_to_timers.get(&map_id) {
                        let timers = timers_for_map.iter().filter(|t| t.id == id);
                        for timer in timers {
                            log::debug!("Creating timer machine for {} as it has been enabled.", timer.id);
                            self.current_timers.push(TimerMachine::new(
                                timer.clone(),
                                alert_sem.clone(),
                                rt_sender.clone(),
                            ));
                        }
                    }
                },
            true => {
                let timers_to_remove = self.current_timers.iter_mut().filter(|t| t.timer.id == id);
                for timer in timers_to_remove {
                    log::debug!(
                        "Starting cleanup for timer {} as it has been disabled.",
                        timer.timer.id
                    );
                    timer.cleanup().await;
                }
            },
        }
    }

    async fn enable(&mut self, id: &str, map_id: MapId, alert_sem: Arc<Mutex<()>>, rt_sender: RtSender) {
        let mut settings_lock = Settings::async_write()
            .await
            .expect("Settings unitialized, impossible");
        settings_lock.enable_timer(id.to_string());
        drop(settings_lock);
        if let Some(map_id) = map_id {
            if let Some(timers_for_map) = &self.map_id_to_timers.get(&map_id) {
                let timers = timers_for_map.iter().filter(|t| t.id == id);
                for timer in timers {
                    log::debug!("Creating timer machine for {}", timer.id);
                    self.current_timers.push(TimerMachine::new(
                        timer.clone(),
                        alert_sem.clone(),
                        rt_sender.clone(),
                    ));
                }
            }
        }
    }

    async fn disable(&mut self, id: &str) {
        let mut settings_lock = Settings::async_write()
            .await
            .expect("Settings unitialized, impossible");
        settings_lock.disable_timer(id.to_string());
        drop(settings_lock);
        let timers_to_remove = self.current_timers.iter_mut().filter(|t| t.timer.id == id);
        for timer in timers_to_remove {
            log::debug!("Starting cleanup for timer {}", timer.timer.id);
            timer.cleanup().await;
        }
        self.current_timers.retain(|t| t.timer.id != id);
    }

    async fn reset(&mut self) {
        for timer in &mut self.current_timers {
            timer.do_reset().await;
        }
    }

    async fn timer_key_trigger(&mut self, id: String, is_release: bool) {
        let idx = id.chars().last().unwrap().to_digit(10).unwrap();
        self.timer_key_trigger_idx(idx, is_release);
    }

    fn timer_key_trigger_idx(&mut self, idx: u32, is_release: bool) {
        for timer in &mut self.current_timers {
            timer.key_event(idx, is_release);
        }
    }

    pub(crate) async fn handle_map_event(
        &mut self,
        settings: SettingsLock,
        alert_sem: Arc<Mutex<()>>,
        new_map_id: u32,
        rt_sender: RtSender,
    ) {
        for timer in &mut self.current_timers {
            timer.cleanup().await;
        }
        self.current_timers.clear();
        if self.map_id_to_timers.contains_key(&new_map_id) {
            let map_timers = &self.map_id_to_timers[&new_map_id];
            for timer in map_timers {
                let settings_lock = settings.read().await;
                let settings_for_timer = settings_lock.timers.get(&timer.id);
                let timer_enabled = match settings_for_timer {
                    Some(setting) => !setting.disabled,
                    None => true,
                };
                if timer_enabled {
                    self.current_timers.push(TimerMachine::new(
                        timer.clone(),
                        alert_sem.clone(),
                        rt_sender.clone(),
                    ));
                }
                drop(settings_lock);
            }
            for machine in &mut self.current_timers {
                machine.update_on_map(new_map_id)
            }
        }
    }

    pub(crate) async fn handle_loading_screen(&mut self) {
        // TODO: when RTAPI isn't present, this will count "cinematic" sequences
        // (vistas or whenever arcdps auto-hides for example) as a loading screen
        // could cause problems with multi-phase fights involving cutscenes?
        self.reset().await;
        /*
        for timer in &mut self.current_timers {
            timer.cleanup().await;
        }
        self.current_timers.clear();
        */
    }

    pub(crate) async fn handle_combat_event(&mut self, cbt: CombatState) {
        for machine in &mut self.current_timers {
            machine.set_combat_state(cbt);
        }
    }

    pub(crate) async fn handle_position(&mut self, position: Position) {
        for machine in &mut self.current_timers {
            machine.tick(position).await
        }
    }

    async fn progress_bar_style(&mut self, style: ProgressBarStyleChange, rt_sender: RtSender) {
        let mut settings_lock = Settings::async_write()
            .await
            .expect("Settings unitialized, impossible");
        let progress_bar_settings = settings_lock.set_progress_bar(style);
        let _ = rt_sender
            .send(RenderEvent::ProgressBarUpdate(progress_bar_settings))
            .await;

        drop(settings_lock);
    }

    pub fn try_send(e: TimersEvent) {
        Controller::with_sender(|sender| sender.timers_try_send(e));
    }
}
