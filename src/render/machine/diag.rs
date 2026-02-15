use {
    super::RenderMachine,
    crate::exports::runtime::statistics::{MetricsSwitch, StatsCounter, StatsDesc, StatsRef, StatsUnit},
    log::Log,
    std::{
        collections::LinkedList,
        mem,
        ptr,
        sync::{LazyLock, Mutex},
        time::Instant,
    },
    sync_unsafe_cell::SyncUnsafeCell,
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
        ];
        for &(counter, mut desc, detailed) in stats_counters {
            desc.detailed = detailed;
            counter.register(desc);
        }
    }
    pub(super) fn metrics_pre(&mut self) {
        self.metrics_switch = MetricsSwitch::read();
        FrameLog::is_taimi_set(true);
    }
    pub(super) fn metrics_pre_render(&mut self) {
        if self.metrics_switch.contains(MetricsSwitch::COLLECT) {
            self.metrics_checkpoint_render = Some(Instant::now());
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
    pub(super) fn metrics_pre_ui(&mut self) {
        if self.metrics_switch.contains(MetricsSwitch::COLLECT) {
            self.metrics_checkpoint_ui = Some(Instant::now());
        }
    }
    pub(super) fn metrics_post_ui(&mut self) {
        if let Some(checkpoint) = self.metrics_checkpoint_ui.take() {
            let amt = checkpoint.elapsed().as_micros() as u64;
            STATS_FRAME_TIME_UI.reset(amt);
            if let Some(checkpoint) = &self.metrics_checkpoint {
                let total = checkpoint.elapsed().as_micros() as u64 / 0x20;
                let amt = (amt / 0x20) as u32;
                let slice = STATS_FRAME_TIME_SLICE.get() as usize as u64;
                let num = (StatsUnit::frac_num(slice) as u32).saturating_add(amt);
                STATS_FRAME_TIME_SLICE.reset(StatsUnit::frac(num as i32, total as u32));
            }
        }
        FrameLog::is_taimi_set(false);
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
static STATS_FRAME_TIME_SLICE: StatsCounter = StatsCounter::DEFAULT;

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
    pub fn is_taimi() -> bool {
        unsafe { ptr::read_volatile(Self::is_taimi_flag().get()) }
    }
    pub fn is_game() -> bool {
        !Self::is_taimi() && Self::is_enabled()
    }
    pub fn is_taimi_set(is_taimi: bool) {
        unsafe { ptr::write_volatile(Self::is_taimi_flag().get(), is_taimi) }
    }
    pub fn is_taimi_flag() -> &'static SyncUnsafeCell<bool> {
        static FLAG: SyncUnsafeCell<bool> = SyncUnsafeCell::new(false);
        &FLAG
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
            log::debug!("{record}");
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
