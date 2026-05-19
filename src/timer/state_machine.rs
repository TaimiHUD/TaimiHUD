use {
    crate::{
        controller::RtSender,
        render::{RenderEvent, RenderState},
        settings::Settings,
        timer::{CombatState, Position, TimerAction, TimerFile, TimerFileAction, TimerFilePhase},
    },
    bitflags::bitflags,
    std::{
        collections::HashMap,
        fmt::Display,
        sync::{
            atomic::{AtomicIsize, Ordering},
            Arc,
            RwLock,
        },
    },
    tokio::{
        sync::Mutex,
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
        match Settings::try_read() {
            Some(settings) if settings.enable_katrender => (),
            _ => return,
        }
        let space_sender = match Engine::sender() {
            Some(sender) => sender,
            None => return,
        };

        match self {
            Self::Feed(ps) => {
                let _ = space_sender.send(SpaceEvent::MarkerFeed(ps.clone())).await;
            },
            Self::Reset(tf) => {
                let _ = space_sender.send(SpaceEvent::MarkerReset(tf.clone())).await;
            },
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
            },
            Self::Reset(tf) => {
                let _ = render_sender.send(RenderEvent::AlertReset(tf.clone())).await;
            },
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

#[derive(Debug)]
pub struct TimerMachine {
    state: TimerMachineState,
    pub timer: Arc<TimerFile>,
    alert_sem: Arc<Mutex<()>>,
    sender: RtSender,
    combat_state: CombatState,
    tasks: Vec<JoinHandle<()>>,
    key_pressed: TimerKeybinds,
    /// skipTime adjustments
    offsets: Arc<SharedTimeOffsets>,
}

#[derive(Clone)]
pub struct PhaseState {
    pub start: Instant,
    pub phase: TimerFilePhase,
    pub offsets: Arc<SharedTimeOffsets>,
}
impl PhaseState {
    #[inline]
    pub fn timer(&self) -> &Arc<TimerFile> {
        &self.phase.timer
    }
    #[inline]
    pub fn start_for_set(&self, set: &str) -> Instant {
        self.offsets.adjust_start_for(set, self.start)
    }
}

#[derive(Clone)]
pub struct TextAlert {
    pub timer: Arc<TimerFile>,
    pub message: String,
}

impl TimerMachine {
    pub fn new(timer: Arc<TimerFile>, alert_sem: Arc<Mutex<()>>, sender: RtSender) -> Self {
        TimerMachine {
            state: TimerMachineState::AwakeUnaware,
            timer,
            alert_sem,
            sender,
            combat_state: CombatState::Outside,
            tasks: Default::default(),
            key_pressed: Default::default(),
            offsets: Default::default(),
        }
    }

    async fn send_alerender(
        sender: RtSender,
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
            },
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
        let _ = self.sender.send(RenderEvent::AlertEnd(self.timer.clone())).await;
    }

    async fn abort_tasks(&self, reason: String) {
        log::info!("Aborting {} tasks for reason: \"{}\".", self.tasks.len(), reason);
        let reset_event = EventMapper::reset(self.timer.clone());
        reset_event.send().await;
    }

    async fn start_tasks(&mut self, phase: &TimerFilePhase) {
        let phase_state = PhaseState {
            start: Instant::now(),
            phase: phase.clone(),
            offsets: self.offsets.clone(),
        };
        let alert_sounds = phase
            .sounds
            .iter()
            .flat_map(|sound| {
                sound.timestamps.iter().map(|&ts| {
                    // TODO: actually tie to a clock that obeys the machine!
                    let when = phase_state
                        .offsets
                        .adjust_reference_for(sound.offset_set(), phase_state.start)
                        + Duration::from_secs_f32(ts);
                    self.text_alert(
                        sound.text.clone(),
                        when.saturating_duration_since(phase_state.start),
                        Duration::from_secs_f32(super::marker::default_duration()),
                    )
                })
            })
            .collect::<Vec<_>>();
        self.tasks.extend(alert_sounds);
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

    /// TODO: show feedback when pressed (`action.name`)?
    async fn apply_action(&mut self, action: TimerFileAction) {
        if let Some((duration, sets)) = action.as_skip_time() {
            let fallback = match sets.is_empty() {
                false => None,
                true => Some(TimerAction::DEFAULT_SET),
            };
            let sets = fallback.into_iter().chain(sets.iter().map(|set| &set[..]));
            for set in sets {
                self.offsets.adjust_forward(set, duration);
            }
        } else {
            // unreachable, no other action types are defined!
            debug_assert_eq!(action.kind, crate::timer::TimerActionType::SkipTime);
        }
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
            },
            // within a phase (nth)
            OnPhase(phase) => {
                // handle the finish check
                let finished = match &phase.finish {
                    Some(trigger) => trigger.check(pos, self.combat_state, &mut self.key_pressed),
                    _ => false,
                };
                if finished {
                    self.state_change(FinishedPhase(phase.clone())).await
                } else {
                    // check for action triggers otherwise...
                    let triggered = {
                        let combat_state = self.combat_state.clone();
                        let key_pressed = &mut self.key_pressed;
                        phase
                            .actions
                            .iter()
                            .enumerate()
                            .filter(move |(_, action)| action.trigger.check(pos, combat_state, key_pressed))
                            .map(|(idx, _)| TimerFileAction::new(phase.clone(), idx))
                            .flatten()
                    }
                    .collect::<Vec<_>>();
                    for action in triggered {
                        self.apply_action(action).await;
                    }
                }
            },
            FinishedPhase(phase) => {
                // check the next phase's start trigger
                if let Some(next_phase) = &phase.clone().next() {
                    let trigger = &next_phase.start;
                    if trigger.check(pos, self.combat_state, &mut self.key_pressed) {
                        self.state_change(OnPhase(next_phase.clone())).await;
                    }
                }
            },
            Finished => (),
        }
    }

    pub fn key_event(&mut self, idx: u32, is_release: bool) {
        let flag = 1u8 << idx;
        match is_release {
            false => self.key_pressed.insert(TimerKeybinds::from_bits_retain(flag)),
            true => self.key_pressed.remove(TimerKeybinds::from_bits_retain(flag)),
        }
    }

    pub fn set_combat_state(&mut self, combat_state: CombatState) {
        self.combat_state = combat_state;
    }

    pub fn update_on_map(&mut self, map_id: u32) {
        let machine_map_id = &self.timer.map_id;
        if *machine_map_id == map_id {
            log::info!("On map with ID \"{}\" for \"{}\"", map_id, self.timer.name());
            self.state = TimerMachineState::OnMap;
        } else {
            log::info!("Off map with ID \"{}\" for \"{}\"", map_id, self.timer.name());
            self.state = TimerMachineState::OffMap;
        }
    }
}

#[derive(Debug, Default)]
pub struct SharedTimeOffset {
    offset: AtomicIsize,
}
impl SharedTimeOffset {
    /// milliseconds
    pub const SCALE: f32 = 1000.0f32;
    pub fn read_seconds(&self) -> f32 {
        self.offset.load(Ordering::Relaxed) as f32 / Self::SCALE
    }
    pub fn adjust_forward(&self, amt: Duration) {
        self.adjust_seconds(-amt.as_secs_f32())
    }
    pub fn adjust_seconds(&self, amt: f32) {
        let ms = (amt * Self::SCALE) as isize;
        self.offset.fetch_add(ms, Ordering::Release);
    }
}
#[derive(Debug, Default)]
pub struct SharedTimeOffsets {
    /// global/default skipTime
    pub global: SharedTimeOffset,
    /// split-mechanics offsets
    pub for_set: RwLock<HashMap<String, Arc<SharedTimeOffset>>>,
}
impl SharedTimeOffsets {
    pub fn offset_seconds_for(&self, set: &str) -> f32 {
        match set {
            "" | TimerAction::DEFAULT_SET => self.global.read_seconds(),
            set => self
                .for_set
                .read()
                .ok()
                .and_then(|offsets| offsets.get(set).map(|o| o.read_seconds()))
                .unwrap_or(0.0f32),
        }
    }
    pub fn offset_for(&self, set: &str) -> Duration {
        Duration::from_secs_f32(self.offset_seconds_for(set))
    }
    pub fn adjust_start_for(&self, set: &str, start: Instant) -> Instant {
        start + self.offset_for(set)
    }
    pub fn adjust_reference_for(&self, set: &str, now: Instant) -> Instant {
        now - self.offset_for(set)
    }
    pub fn adjust_forward(&self, set: &str, amt: Duration) {
        match set {
            TimerAction::DEFAULT_SET => self.global.adjust_forward(amt),
            set =>
                if let Ok(mut offsets) = self.for_set.write() {
                    // TODO: could've tried a read() lock here first but unlikely to be applied often or even used at all so...
                    let offset = if let Some(set) = offsets.get(set) {
                        set
                    } else {
                        &*offsets.entry(set.into()).or_default()
                    };
                    offset.adjust_forward(amt);
                },
        }
    }
}
