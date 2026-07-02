use {
    super::RenderMachine,
    crate::{
        exports::runtime::statistics::{MetricsSwitch, StatsCounter, StatsDesc, StatsRef, StatsUnit},
        render::element::prelude::*,
    },
    log::Log,
    std::{
        collections::LinkedList,
        mem,
        sync::{
            atomic::{AtomicU32, Ordering},
            LazyLock,
            Mutex,
        },
        time::Instant,
    },
};

impl RenderMachine {
    pub(super) fn metrics_init(&mut self) {
        let sec = "stats-render";
        let stats_counters = &[
            (
                StatsRef::with_counter(&STATS_FRAME_TIME_SLICE, StatsUnit::Fraction),
                StatsDesc::new(sec, "stats-render-time-slice"),
                true,
            ),
            (
                StatsRef::with_counter(&STATS_FRAME_TIME_RENDER, StatsUnit::Time),
                StatsDesc::new(sec, "stats-render-time"),
                true,
            ),
            (
                StatsRef::with_counter(&STATS_FRAME_TIME_UI, StatsUnit::Time),
                StatsDesc::new(sec, "stats-render-time-ui"),
                true,
            ),
            (
                StatsRef::with_counter(&STATS_FRAME_TIME_INTERVAL, StatsUnit::Time),
                StatsDesc::new(sec, "stats-render-time-interval"),
                true,
            ),
            (
                StatsRef::with_counter(&STATS_FRAME_TIME_LATENCY, StatsUnit::Time),
                StatsDesc::new(sec, "stats-render-time-latency"),
                true,
            ),
            #[cfg(todo)]
            (
                StatsRef::with_counter(&STATS_FRAME_IMGUI_ALLOC, StatsUnit::Size),
                StatsDesc::new("stats-runtime-allocator", "stats-runtime-alloc-imgui"),
                true,
            ),
        ];
        for &(counter, mut desc, detailed) in stats_counters {
            desc.detailed = detailed;
            counter.register(desc);
        }
    }
    pub(super) fn metrics_pre(&mut self) {
        self.metrics_switch = MetricsSwitch::read();
        FrameState::TAIMI.publish_set();
        FrameState::GAME.publish_clear();
    }
    pub(super) fn metrics_pre_render(&mut self, now: &Instant) {
        if self.metrics_switch.contains(MetricsSwitch::COLLECT) {
            self.metrics_checkpoint_render = Some(*now);
            if self.metrics_checkpoint.is_none() {
                self.metrics_checkpoint = self.metrics_checkpoint_render;
            }
        } else {
            self.metrics_checkpoint = None;
            STATS_FRAME_TIME_SLICE.reset(0);
        }
    }
    pub(super) fn metrics_post_render(&mut self) {
        if let Some(checkpoint) = self.metrics_checkpoint_render.take() {
            let amt = checkpoint.elapsed().as_micros() as u64;
            STATS_FRAME_TIME_RENDER.reset(amt);
            let amt = (amt / 0x20) as u32;
            STATS_FRAME_TIME_SLICE.increment(amt);
        }
    }
    pub(super) fn metrics_pre_ui<'ui, U>(&mut self, _ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        #[cfg(todo)]
        {
            self.metrics_alloc_ui = ui.with_io_dyn(|io| io.metrics_active_allocations());
        }
        if self.metrics_switch.contains(MetricsSwitch::COLLECT) {
            self.metrics_checkpoint_ui = Some(Instant::now());
        }
    }
    pub(super) fn metrics_post_ui<'ui, U>(&mut self, _ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        #[cfg(todo)]
        {
            STATS_FRAME_IMGUI_ALLOC.increment_by(|| {
                let ui_alloc_after = ui.with_io_dyn(|io| io.metrics_active_allocations());
                let ui_alloc_pre = mem::replace(&mut self.metrics_alloc_ui, ui_alloc_after);
                ui_alloc_after - ui_alloc_pre
            });
        }
        self.metrics_post_ui_checkpoint();
    }
    fn metrics_post_ui_checkpoint(&mut self) {
        if let Some(checkpoint) = self.metrics_checkpoint_ui.take() {
            let amt = checkpoint.elapsed().as_micros() as u64;
            STATS_FRAME_TIME_UI.reset(amt);
            let frame_start = self.mumblelink_frames.latest_render_timestamp();
            let interval = if let Some(frametime) = self.frame_duration {
                Some(frametime)
            } else {
                let prior_frame = self
                    .mumblelink_frames
                    .render_to_uitick(self.mumblelink_frames.latest_render_tick().wrapping_sub(1));
                self.mumblelink_frames
                    .timestamp_at(prior_frame)
                    .map(|prev| frame_start.saturating_duration_since(*prev))
            };
            if let Some(interval) = interval {
                STATS_FRAME_TIME_INTERVAL.reset(interval.as_micros() as u64);
            }
            STATS_FRAME_TIME_LATENCY
                .reset_with(|| checkpoint.saturating_duration_since(*frame_start).as_micros() as u64);
            if let Some(checkpoint) = &self.metrics_checkpoint {
                let total = checkpoint.elapsed().as_micros() as u64 / 0x20;
                let amt = (amt / 0x20) as u32;
                let slice = STATS_FRAME_TIME_SLICE.get() as usize as u64;
                let num = (StatsUnit::frac_num(slice) as u32).saturating_add(amt);
                STATS_FRAME_TIME_SLICE.reset(StatsUnit::frac(num as i32, total as u32));
            }
        }
        FrameState::TAIMI.publish_clear();
        let game_frame_subsequent = FrameState::GAME_FRAME_SUBSEQUENT;
        #[cfg(feature = "extension-nexus")]
        let game_frame_subsequent = match crate::exports::nexus::available() {
            true => false,
            _ => game_frame_subsequent,
        };
        if game_frame_subsequent {
            FrameState::GAME.publish_set();
        }
    }
    pub(super) fn act_frame_log(&mut self) {
        if self.metrics_switch.contains(MetricsSwitch::FRAME_LOG) {
            if self.metrics_switch.contains(MetricsSwitch::FRAME_LOG_TRIGGER) {
                MetricsSwitch::FRAME_LOG_TRIGGER.publish_clear();
                log::debug!("FRAME LOG TRIGGERED");
                Self::frame_log().commit();
            } else {
                Self::frame_log().clear();
            }
        }
    }
    pub fn frame_log() -> &'static FrameLog {
        static FRAME_LOG: LazyLock<FrameLog> = LazyLock::new(FrameLog::new);
        &FRAME_LOG
    }
}

static STATS_FRAME_TIME_RENDER: StatsCounter = StatsCounter::DEFAULT;
static STATS_FRAME_TIME_UI: StatsCounter = StatsCounter::DEFAULT;
static STATS_FRAME_TIME_INTERVAL: StatsCounter = StatsCounter::DEFAULT;
static STATS_FRAME_TIME_LATENCY: StatsCounter = StatsCounter::DEFAULT;
static STATS_FRAME_TIME_SLICE: StatsCounter = StatsCounter::DEFAULT;
#[cfg(todo)]
static STATS_FRAME_IMGUI_ALLOC: StatsCounter = StatsCounter::DEFAULT;

pub struct FrameLog {
    records: Mutex<LinkedList<String>>,
}
impl FrameLog {
    pub fn new() -> Self {
        Self { records: Default::default() }
    }
    pub fn is_enabled() -> bool {
        MetricsSwitch::read().contains(MetricsSwitch::FRAME_LOG)
    }
    pub fn is_game() -> bool {
        FrameState::is_game() && Self::is_enabled()
    }
    pub fn take_records(&self) -> Option<LinkedList<String>> {
        self.records
            .lock()
            .ok()
            .map(|mut records| mem::take(&mut *records))
    }
    pub fn clear(&self) {
        let records = self.take_records();
        drop(records);
    }
    pub fn commit(&self) {
        let records = self.take_records();
        for record in records.into_iter().flatten() {
            log::debug!(target: "framelog", "{record}");
        }
    }
}
impl Log for FrameLog {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        Self::is_enabled()
    }

    fn log(&self, record: &log::Record) {
        #[cfg(todo = "unnecessary")]
        if !Self::is_enabled() {
            return
        }

        let record = record.args().to_string();
        if let Ok(mut records) = self.records.lock() {
            records.push_back(record);
        }
    }
    fn flush(&self) {}
}
#[macro_export]
macro_rules! frame_log {
    (::$f:ident($($args:tt)*)) => {
        $crate::render::machine::FrameLog::$f($($args)*)
    };
    (; $($log:tt)*) => {
        ::log::debug! { logger: $crate::render::machine::RenderMachine::frame_log(), $($log)* }
    };
    ($($log:tt)*) => {
        if $crate::render::machine::FrameLog::is_enabled() {
            $crate::render::machine::frame_log! {; $($log)* }
        }
    };
}
pub use frame_log;

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FrameState: u32 {
        const TAIMI = 0x01;
        #[cfg(feature = "extension-nexus")]
        const NEXUS = 0x02;
        const GAME = 0x04;
    }
}
impl FrameState {
    pub(crate) const STATE_ORDERING: Ordering = Ordering::Relaxed;
    pub const DEFAULT: Self = Self::empty();
    pub const RENDER_THREAD_MASK: Self = Self::from_bits_retain({
        let mask = Self::TAIMI.bits() | Self::GAME.bits();
        #[cfg(feature = "extension-nexus")]
        let mask = mask | Self::NEXUS.bits();
        mask
    });

    pub(crate) const fn state() -> &'static AtomicU32 {
        static STATE: AtomicU32 = AtomicU32::new(FrameState::DEFAULT.bits());
        &STATE
    }

    /// assume game frame starts once we're done rendering
    ///
    /// TODO: use dx buffer clear to mark start of frame or something -
    /// this isn't guaranteed and will be a problem with multi-threaded or bg rendering
    pub const GAME_FRAME_SUBSEQUENT: bool = true;
    pub fn is_game() -> bool {
        matches!(Self::render_thread_state(), Self::DEFAULT | Self::GAME)
    }
    #[cfg(todo = "unused")]
    pub fn is_taimi() -> bool {
        Self::read().contains(Self::TAIMI)
    }
    pub fn read() -> Self {
        Self::from_bits_retain(Self::state().load(Self::STATE_ORDERING))
    }
    pub fn render_thread_state() -> Self {
        Self::read() & Self::RENDER_THREAD_MASK
    }
    #[cfg(todo = "unused")]
    pub fn publish_toggle(self) {
        Self::state().fetch_xor(self.bits(), Self::STATE_ORDERING);
    }
    pub fn publish_set(self) {
        Self::state().fetch_or(self.bits(), Self::STATE_ORDERING);
    }
    pub fn publish_clear(self) {
        Self::state().fetch_and((!self).bits(), Self::STATE_ORDERING);
    }
}
