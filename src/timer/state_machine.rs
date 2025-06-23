use {
    super::TimerMarker,
    crate::{
        render::{RenderEvent, RenderState},
        timer::{CombatState, Position, TimerAlert, TimerFile, TimerPhase},
    },
    bitflags::bitflags,
    std::{fmt::Display, ops::Deref, sync::Arc},
    tokio::{
        sync::{mpsc::Sender, Mutex},
        task::JoinHandle,
        time::{sleep, Duration, Instant},
    },
};

#[cfg(feature = "space")]
use crate::space::engine::{Engine, SpaceEvent};

bitflags! {
    #[derive(Debug, Clone, Default)]
    pub struct TimerKeybinds: u8 {
        const A = 1;
        const B = 1 << 1;
        const C = 1 << 2;
        const D = 1 << 3;
        const E = 1 << 4;
    }
}

enum EventMapper {
    Feed(PhaseState),
    Reset(Arc<TimerFile>),
}

impl EventMapper {
    fn feed(ps: PhaseState) -> Self {
        Self::Feed(ps)
    }

    fn reset(tf: Arc<TimerFile>) -> Self {
        Self::Reset(tf.clone())
    }

    #[cfg(feature = "space")]
    async fn send_space(&self) {
        match crate::SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            Some(settings) if settings.enable_katrender =>
                (),
            _ => return,
        }
        let space_sender = match Engine::sender() {
            Some(sender) => sender,
            None => return,
        };

        match self {
            Self::Feed(ps) => {
                let _ = space_sender.send(SpaceEvent::MarkerFeed(ps.clone())).await;
            }
            Self::Reset(tf) => {
                let _ = space_sender.send(SpaceEvent::MarkerReset(tf.clone())).await;
            }
        }
    }
    async fn send_render(&self) {
        let render_sender = match RenderState::sender() {
            Some(sender) => sender,
            None => return,
        };
        match self {
            Self::Feed(ps) => {
                let _ = render_sender.send(RenderEvent::AlertFeed(ps.clone())).await;
            }
            Self::Reset(tf) => {
                let _ = render_sender
                    .send(RenderEvent::AlertReset(tf.clone()))
                    .await;
            }
        }
    }

    async fn send(&self) {
        #[cfg(feature = "space")]
        self.send_space().await;
        self.send_render().await;
    }
}

/*
* A timer can be:
* - existent without knowledge of current map
* - off the map
* - on the map, first phase untriggered
* - phase triggered, cycling through alerts
* - nth phase done, next phase
* - finished, denoted by a different area, departure, out of combat, ...
* - failed, with reset condition
*/
#[derive(Debug, Clone)]
enum TimerMachineState {
    /*
     * Ensolyss: I am awake.
     * Ensolyss: I am aware.
     * Ensolyss: Suffer, mortal things.
     */
    AwakeUnaware,
    OffMap,
    OnMap,
    OnPhase(TimerFilePhase),
    FinishedPhase(TimerFilePhase),
    Finished,
}

impl Display for TimerMachineState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TimerMachineState::*;
        match self {
            AwakeUnaware => write!(f, "AwakeUnaware"),
            OffMap => write!(f, "OffMap"),
            OnMap => write!(f, "OnMap"),
            OnPhase(tfp) => write!(f, "OnPhase {}", tfp.name),
            FinishedPhase(tfp) => write!(f, "FinishedPhase {}", tfp.name),
            Finished => write!(f, "Finished"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimerFilePhase {
    timer: Arc<TimerFile>,
    phase: usize,
}

impl TimerFilePhase {
    fn new(timer: Arc<TimerFile>) -> Option<Self> {
        match timer.phases.is_empty() {
            true => None,
            false => Some(Self { timer, phase: 0 }),
        }
    }

    #[allow(dead_code)]
    fn reset(mut self) -> Self {
        self.phase = 0;
        self
    }

    fn next(self) -> Option<Self> {
        let phase_len = self.timer.phases.len();
        let phase = (self.phase + 1..phase_len).next()?;
        Some(Self {
            timer: self.timer,
            phase,
        })
    }

    pub fn phase(&self) -> &TimerPhase {
        &self.timer.phases[self.phase]
    }
}

impl Deref for TimerFilePhase {
    type Target = TimerPhase;

    fn deref(&self) -> &Self::Target {
        self.phase()
    }
}

#[derive(Debug, Clone)]
pub struct TimerMachine {
    state: TimerMachineState,
    pub timer: Arc<TimerFile>,
    alert_sem: Arc<Mutex<()>>,
    sender: Sender<RenderEvent>,
    combat_state: CombatState,
    tasks: Vec<Arc<JoinHandle<()>>>,
    key_pressed: TimerKeybinds,
}

#[derive(Clone)]
pub struct PhaseState {
    pub timer: Arc<TimerFile>,
    pub start: Instant,
    pub phase: TimerFilePhase,
    pub alerts: Vec<TimerAlert>,
    pub markers: Vec<TimerMarker>,
}

#[derive(Clone)]
pub struct TextAlert {
    pub timer: Arc<TimerFile>,
    pub message: String,
}

impl TimerMachine {
    pub fn new(
        timer: Arc<TimerFile>,
        alert_sem: Arc<Mutex<()>>,
        sender: Sender<RenderEvent>,
    ) -> Self {
        TimerMachine {
            state: TimerMachineState::AwakeUnaware,
            timer,
            alert_sem,
            sender,
            combat_state: CombatState::Outside,
            tasks: Default::default(),
            key_pressed: Default::default(),
        }
    }

    async fn send_alerender(
        sender: Sender<RenderEvent>,
        lock: Arc<Mutex<()>>,
        timer: Arc<TimerFile>,
        message: String,
        wait_duration: Duration,
        display_duration: Duration,
    ) {
        log::info!(
            "Sleeping {:?} for {}: a message with {:?} duration",
            wait_duration,
            message,
            display_duration
        );
        sleep(wait_duration).await;
        let alert_handle = lock.lock().await;
        log::info!(
            "Slept {:?}, displaying {}: a message with {:?} duration",
            wait_duration,
            message,
            display_duration
        );
        let _ = sender
            .send(RenderEvent::AlertStart(TextAlert {
                timer: timer.clone(),
                message: message.clone(),
            }))
            .await;
        sleep(display_duration).await;
        let _ = sender.send(RenderEvent::AlertEnd(timer.clone())).await;
        log::info!(
            "Stopping displaying {}: we slept for {:?} a message with {:?} duration",
            message,
            wait_duration,
            display_duration
        );
        // this is my EMOTIONAL SUPPORT drop
        drop(alert_handle);
    }

    fn text_alert(
        &self,
        message: String,
        wait_duration: Duration,
        display_duration: Duration,
    ) -> JoinHandle<()> {
        tokio::spawn(Self::send_alerender(
            self.sender.clone(),
            self.alert_sem.clone(),
            self.timer.clone(),
            message,
            wait_duration,
            display_duration,
        ))
    }

    async fn reset_check(&mut self, pos: Position) {
        let trigger = &self.timer.reset;
        use TimerMachineState::*;
        match &self.state {
            OnPhase(_) | FinishedPhase(_) | Finished => {
                if trigger.check(pos, self.combat_state, &mut self.key_pressed) {
                    self.do_reset().await;
                }
            }
            _ => (),
        }
    }

    pub async fn do_reset(&mut self) {
        let reason = format!("Reset triggered for \"{}\"", self.timer.name);
        log::info!("Reset triggered!");
        self.combat_state = CombatState::Outside;
        self.state_change(TimerMachineState::OnMap).await;
        self.abort_tasks(reason.clone()).await;
        let zero_s = Duration::from_secs(0);
        let one_s = Duration::from_secs(1);
        self.text_alert(reason, zero_s, one_s);
    }

    pub async fn cleanup(&mut self) {
        let reason = format!(
            "\"{}\" is being told to cleanup, about to be deleted!",
            self.timer.name
        );
        self.abort_tasks(reason).await;
        let _ = self
            .sender
            .send(RenderEvent::AlertEnd(self.timer.clone()))
            .await;
    }

    async fn abort_tasks(&self, reason: String) {
        log::info!(
            "Aborting {} tasks for reason: \"{}\".",
            self.tasks.len(),
            reason
        );
        let reset_event = EventMapper::reset(self.timer.clone());
        reset_event.send().await;
    }

    async fn start_tasks(&self, phase: &TimerFilePhase) {
        let alerts = phase.get_alerts();
        let markers = phase.get_markers();
        let phase_state = PhaseState {
            timer: self.timer.clone(),
            start: Instant::now(),
            phase: phase.clone(),
            alerts,
            markers,
        };
        let feed_event = EventMapper::feed(phase_state.clone());
        feed_event.send().await;
    }

    /**
        state_change is about code that should run once, upon a stage or phase change.
    */
    async fn state_change(&mut self, state: TimerMachineState) {
        use TimerMachineState::*;
        let final_state = match state {
            FinishedPhase(ref phase) if phase.clone().next().is_none() => Finished,
            _ => state,
        };
        let reason = format!("Switching from state {} to {}", self.state, final_state);
        self.abort_tasks(reason).await;
        if let OnPhase(phase) = &final_state {
            self.start_tasks(phase).await;
        }
        self.state = final_state;
    }

    /**
     * tick, in comparison to state_change, runs perpetually and is used for
     * checking to see if conditions for a next phase are met
     */
    pub async fn tick(&mut self, pos: Position) {
        // It is always important to check if we have met the conditions for resetting the timer
        self.reset_check(pos).await;

        use TimerMachineState::*;
        match &self.state {
            // We exist, but is there anything to do about that?
            // Nothing, without the current map. Lost adrift in the void.
            AwakeUnaware => (),
            // We're off map, this means the timer conditions cannot be met.
            OffMap => (),
            // OnMap means time to start looking for our conditions, with location and
            // (unimplemented) key first.
            OnMap => {
                // All timers have a start trigger and a zeroth (first) phase
                let trigger = &self.timer.phases.first().unwrap().start;
                if trigger.check(pos, self.combat_state, &mut self.key_pressed) {
                    if let Some(phase) = TimerFilePhase::new(self.timer.clone()) {
                        self.state_change(OnPhase(phase)).await;
                    }
                }
            }
            // within a phase (nth)
            OnPhase(phase) => {
                // handle the finish check
                if let Some(trigger) = &phase.finish {
                    if trigger.check(pos, self.combat_state, &mut self.key_pressed) {
                        self.state_change(FinishedPhase(phase.clone())).await;
                    }
                }
            }
            FinishedPhase(phase) => {
                // check the next phase's start trigger
                if let Some(next_phase) = &phase.clone().next() {
                    let trigger = &next_phase.start;
                    if trigger.check(pos, self.combat_state, &mut self.key_pressed) {
                        self.state_change(OnPhase(next_phase.clone())).await;
                    }
                }
            }
            Finished => (),
        }
    }

    pub fn key_event(&mut self, idx: u32, is_release: bool) {
        let flag = 1u8 << idx;
        match is_release {
            false => self
                .key_pressed
                .insert(TimerKeybinds::from_bits_retain(flag)),
            true => self
                .key_pressed
                .remove(TimerKeybinds::from_bits_retain(flag)),
        }
    }

    pub fn set_combat_state(&mut self, combat_state: CombatState) {
        self.combat_state = combat_state;
    }

    pub fn update_on_map(&mut self, map_id: u32) {
        let machine_map_id = &self.timer.map_id;
        if *machine_map_id == map_id {
            log::info!(
                "On map with ID \"{}\" for \"{}\"",
                map_id,
                self.timer.name()
            );
            self.state = TimerMachineState::OnMap;
        } else {
            log::info!(
                "Off map with ID \"{}\" for \"{}\"",
                map_id,
                self.timer.name()
            );
            self.state = TimerMachineState::OffMap;
        }
    }
}
