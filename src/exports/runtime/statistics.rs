pub use taimi_hoard::statistics::Counter as StatsCounter;
#[cfg(feature = "statistics")]
pub use taimi_hoard::statistics::Counter;
#[cfg(not(feature = "statistics"))]
pub use taimi_hoard::statistics::Dummy as Counter;
use {
    core::{fmt, time::Duration},
    std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicUsize, Ordering as AtomicOrdering},
            RwLock,
        },
    },
    taimi_hoard::lazyfmt,
};

#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct StatsDesc {
    pub section: &'static str,
    pub name: &'static str,
    pub detailed: bool,
}
impl StatsDesc {
    pub const fn new(section: &'static str, name: &'static str) -> Self {
        Self { section, name, detailed: false }
    }
}
pub type StatsRegistry = RwLock<BTreeMap<StatsDesc, StatsRef>>;
#[derive(Debug, Copy, Clone)]
pub struct StatsRef {
    pub counter: Option<&'static StatsCounter>,
    pub unit: StatsUnit,
}
impl StatsRef {
    pub const fn new(counter: &'static Counter, unit: StatsUnit) -> Self {
        match counter {
            #[cfg(feature = "statistics")]
            counter => Self::with_counter(counter, unit),
            #[cfg(not(feature = "statistics"))]
            _ => Self::empty(unit),
        }
    }
    pub const fn with_counter(counter: &'static StatsCounter, unit: StatsUnit) -> Self {
        Self { counter: Some(counter), unit }
    }

    pub const fn registry() -> &'static StatsRegistry {
        static REGISTRY: StatsRegistry = StatsRegistry::new(BTreeMap::new());
        &REGISTRY
    }

    pub fn register(self, desc: StatsDesc) {
        if self.is_empty() {
            return
        }
        if let Ok(mut reg) = Self::registry().write() {
            reg.insert(desc, self);
        }
    }
    pub fn deregister(desc: &StatsDesc) {
        if let Ok(mut reg) = Self::registry().write() {
            reg.remove(desc);
        }
    }

    pub const fn empty(unit: StatsUnit) -> Self {
        Self { counter: None, unit }
    }
    pub fn is_empty(&self) -> bool {
        self.counter.is_none()
    }
    pub fn read(&self) -> u64 {
        self.counter.map(|c| c.get() as usize as u64).unwrap_or(0)
    }
}
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum StatsUnit {
    Count,
    /// bytes
    Size,
    /// microseconds
    Time,
    /// u32 parts
    Fraction,
}
impl StatsUnit {
    const TIME_M: u64 = Self::TIME_S * 60;
    const TIME_S: u64 = 1_000_000;
    const TIME_MS: u64 = Self::TIME_S / 1000;
    const SIZE_KB: u64 = 0x400;
    const SIZE_MB: u64 = Self::SIZE_KB * 0x400;

    pub fn percent(progress: f32) -> u64 {
        Self::frac((progress * 100.0) as i32, 100)
    }
    pub fn frequency(fps: f64) -> u64 {
        (Self::TIME_S as f64 * fps) as u64
    }
    pub fn frac(num: i32, denom: u32) -> u64 {
        (denom as u64) << 32 | (num as u32 as u64)
    }
    pub fn frac_parts(value: u64) -> (i32, u32) {
        let num = value as u32;
        let denom = (value >> 32) as u32;
        (num as i32, denom)
    }
    pub fn frac_num(value: u64) -> i32 {
        value as u32 as i32
    }
    pub fn frac_inc_denom(value: u64, amt: u32) -> u64 {
        value.saturating_add((amt as u64) << 32)
    }
    pub fn frac_inc(value: u64, amt: u32) -> u64 {
        value.saturating_add(amt as u64)
    }
    pub fn frac_inc_num(value: u64, amt: i32) -> u64 {
        let num = value as u32 as i32;
        Self::frac_set_num(value, num.saturating_add(amt) as u32)
    }
    pub fn frac_set_num(value: u64, num: u32) -> u64 {
        let denom = value & !(u32::MAX as u64);
        denom | num as u64
    }
    pub fn frac_set_denom(value: u64, denom: u32) -> u64 {
        let num = value as u32;
        ((denom as u64) << 32) | num as u64
    }
    #[inline]
    pub fn time(span: Duration) -> u64 {
        span.as_micros() as u64
    }
    #[inline]
    pub fn time_ms(ms: u64) -> u64 {
        ms.saturating_mul(Self::TIME_MS)
    }
    #[inline]
    pub fn bytes(amt: u64) -> u64 {
        amt
    }

    pub fn display_value(self, value: u64) -> impl fmt::Display {
        lazyfmt::MaybeFmt::new(move |f| match self {
            Self::Count => fmt::Display::fmt(&value, f),
            Self::Size =>
                if value <= Self::SIZE_MB {
                    write!(f, "{:.03}KB", value as f64 / Self::SIZE_KB as f64)
                } else {
                    write!(f, "{:.03}MB", value as i64 as f64 / Self::SIZE_MB as f64)
                },
            Self::Time => {
                if value < Self::TIME_MS {
                    return write!(f, "0.{value:03}ms")
                }
                let time = (value / Self::TIME_MS) as f64;
                let (time, unit) = if value <= Self::TIME_S * 8 {
                    (value as f64 / Self::TIME_MS as f64, "ms")
                } else if value <= Self::TIME_M * 4 {
                    (time / 1_000.0, "s")
                } else {
                    (time / 60_000.0, "m")
                };
                write!(f, "{time:.02}{unit}")
            },
            Self::Fraction => {
                let num = value as u32;
                let denom = (value >> 32) as u32;
                let num = num as i32;
                match denom {
                    denom if denom <= 2 => {
                        let num = num as i32;
                        write!(f, "{num}/{denom}")
                    },
                    denom if denom % 100 == 0 => {
                        return write!(f, "{:.04}%", num as f64 * 100.0 / denom as f64)
                    },
                    denom => {
                        return write!(f, "{:.04}", num as f64 / denom as f64)
                    },
                }
            },
        })
    }
}

#[derive(Debug)]
pub struct MetricsCollectionState {
    pub switch: AtomicUsize,
}
impl MetricsCollectionState {
    pub const SWITCH_ORDERING: AtomicOrdering = AtomicOrdering::Relaxed;

    pub const fn new() -> Self {
        Self {
            switch: AtomicUsize::new(MetricsSwitch::DEFAULT.bits()),
        }
    }
    pub const fn config() -> &'static Self {
        static CONFIG: MetricsCollectionState = MetricsCollectionState::new();
        &CONFIG
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct MetricsSwitch: usize {
        const COLLECT = 0x01;
        const FRAME_LOG = 0x02;
        const FRAME_LOG_TRIGGER = 0x04;
    }
}
impl MetricsSwitch {
    pub const DEFAULT: Self = Self::empty();

    pub const fn switch() -> &'static AtomicUsize {
        &MetricsCollectionState::config().switch
    }

    pub fn read() -> Self {
        Self::from_bits_retain(Self::switch().load(MetricsCollectionState::SWITCH_ORDERING))
    }
    pub fn publish_toggle(self) {
        Self::switch().fetch_xor(self.bits(), MetricsCollectionState::SWITCH_ORDERING);
    }
    pub fn publish_set(self) {
        Self::switch().fetch_or(self.bits(), MetricsCollectionState::SWITCH_ORDERING);
    }
    pub fn publish_clear(self) {
        Self::switch().fetch_and((!self).bits(), MetricsCollectionState::SWITCH_ORDERING);
    }
}
