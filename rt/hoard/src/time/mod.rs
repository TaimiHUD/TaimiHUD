use core::{cmp, fmt, mem, time::Duration};
use std::time::{Instant, SystemTime, SystemTimeError};
#[cfg(feature = "serde")]
use serde::{ser, de};

/// Unix timestamp
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Timestamp {
    /// since epoch
    pub offset: Duration,
}
impl Timestamp {
    pub const MAX: Self = Self::after_epoch(Duration::MAX);
    /// minimum and default of 0
    pub const EPOCH: Self = Self::with_timestamp(0);
    pub const MAX_U64: Self = Self::with_timestamp(u64::MAX);
    pub const MAX_U32: Self = Self::with_timestamp(u32::MAX as u64);
    /// aka javascript's `MAX_SAFE_INTEGER`
    pub const MAX_F64_INT: Self = Self::with_timestamp(match f64::MANTISSA_DIGITS {
        m => 2u64.pow(m) - 1,
        #[cfg(todo = "unnecessary")]
        53 => 0x1f_ffff_ffff_ffff,
    });
    pub const MAX_F32_INT: Self = Self::with_timestamp(
        2u64.pow(f32::MANTISSA_DIGITS) - 1
    );

    pub const SECOND: Duration = Duration::from_secs(1);
    pub const MINUTE: Duration = Duration::from_secs(60);
    pub const HOUR: Duration = Duration::from_secs(Self::MINUTE.as_secs() * 60);
    pub const DAY: Duration = Duration::from_secs(Self::HOUR.as_secs() * 24);
    pub const WEEK: Duration = Duration::from_secs(Self::DAY.as_secs() * 7);

    pub const SECOND_AS_NANOS: u32 = Self::SECOND.as_nanos() as u32;
    pub const SECOND_AS_MICROS: u32 = Self::SECOND.as_micros() as u32;
    pub const SECOND_AS_MILLIS: u16 = Self::SECOND.as_millis() as u16;

    #[inline]
    pub const fn after_epoch(offset: Duration) -> Self {
        Self { offset }
    }
    #[inline]
    pub const fn with_timestamp(ts: u64) -> Self {
        Self::after_epoch(Duration::from_secs(ts))
    }
    #[inline]
    pub fn with_timestamp_f64(ts: f64) -> Self {
        Self::after_epoch(Duration::from_secs_f64(ts))
    }
    /// TODO: const in 1.90 x.x
    #[inline]
    pub fn with_timestamp_f64_const(ts: f64) -> Self {
        let nanosecond = Self::SECOND_AS_NANOS as f64;
        Self::after_epoch(Duration::new(ts.floor() as u64, (ts.fract() * nanosecond) as u32))
    }
    /// TODO: const in 1.90 x.x
    #[inline]
    pub fn with_timestamp_f32_const(ts: f32) -> Self {
        Self::with_timestamp_f64_const(ts as f64)
    }
    #[inline]
    pub fn with_timestamp_f32(ts: f32) -> Self {
        Self::after_epoch(Duration::from_secs_f32(ts))
    }
    #[inline]
    pub fn with_timestamp_f64_saturating(ts: f64) -> Self {
        let offset = match Duration::try_from_secs_f64(ts) {
            #[cfg(debug_assertions)]
            Err(_e) => panic!("{_e}"),
            #[cfg(not(debug_assertions))]
            Err(_e) => {
                log::warn!("f64 timestamp invalid: {_e}");
                None
            },
            Ok(o) => Some(o),
        };
        match offset {
            #[cfg(todo = "unnecessary")]
            None if ts <= 0.0 => Self::EPOCH,
            None => Self::MAX,
            Some(o) => Self::after_epoch(o),
        }
    }
    #[inline]
    pub fn with_timestamp_f32_saturating(ts: f32) -> Self {
        let offset = match Duration::try_from_secs_f32(ts) {
            #[cfg(debug_assertions)]
            Err(_e) => panic!("{_e}"),
            #[cfg(not(debug_assertions))]
            Err(_e) => {
                log::warn!("f32 timestamp invalid: {_e}");
                None
            },
            Ok(o) => Some(o),
        };
        match offset {
            #[cfg(todo = "unnecessary")]
            None if ts <= 0.0 => Self::EPOCH,
            None => Self::MAX,
            Some(o) => Self::after_epoch(o),
        }
    }
    #[inline]
    pub const fn from_ref(offset: &Duration) -> &Self {
        unsafe { mem::transmute(offset) }
    }
    #[inline]
    pub fn from_mut(offset: &mut Duration) -> &mut Self {
        unsafe { mem::transmute(offset) }
    }

    #[inline]
    pub const fn timestamp(&self) -> u64 {
        self.offset.as_secs()
    }
    #[inline]
    pub const fn timestamp_f64(&self) -> f64 {
        self.offset.as_secs_f64()
    }
    #[inline]
    pub const fn timestamp_f32(&self) -> f32 {
        self.offset.as_secs_f64() as f32
    }
}

pub type SignedDuration = Result<Duration, Duration>;
impl Timestamp {
    pub fn seconds_since_f32(&self, earlier: &Self) -> f32 {
        self.timestamp_f32() - earlier.timestamp_f32()
    }
    pub fn seconds_since_f64(&self, earlier: &Self) -> f64 {
        self.timestamp_f64() - earlier.timestamp_f64()
    }
    /// TODO: properly with wrapping 2compliment ops or w/e
    pub fn saturating_seconds_since_i64(&self, rhs: &Self) -> i64 {
        let sign = self.offset.cmp(&rhs.offset);
        let diff = match sign {
            cmp::Ordering::Less => rhs.timestamp() - self.timestamp(),
            _ => self.timestamp() - rhs.timestamp(),
        };
        match (i64::try_from(diff), sign) {
            (Ok(diff), cmp::Ordering::Less) => -diff,
            (Ok(diff), _) => diff,
            (Err(..), cmp::Ordering::Less) => i64::MIN,
            (Err(..), _) => i64::MAX,
        }
    }
    /// saturates to [Duration::MAX] on overflow
    pub fn checked_duration_since_f64(&self, earlier: &Self) -> SignedDuration {
        let diff = self.timestamp_f64() - earlier.timestamp_f64();
        let (neg, amt) = match diff {
            diff if diff < 0.0 => (true, -diff),
            diff => (false, diff),
        };
        let duration = match Duration::try_from_secs_f64(amt) {
            Ok(amt) => amt,
            Err(_e) => {
                log::warn!("timestamp difference overflowed: {_e}");
                Duration::MAX
            },
        };
        Self::signed_duration_new(duration, neg)
    }
    pub fn checked_duration_since(&self, earlier: &Self) -> SignedDuration {
        #[cfg(todo = "unnecessary")]
        let (neg, amt) = match self.offset.cmp(&earlier.offset) {
            cmp::Ordering::Less => (true, Duration::from_secs(earlier.timestamp() - self.timestamp())),
            _ => (false, Duration::from_secs(self.timestamp() - earlier.timestamp())),
        };
        let amt = self.offset.abs_diff(earlier.offset);
        let neg = self.offset.cmp(&earlier.offset).is_lt();
        Self::signed_duration_new(amt, neg)
    }
    pub fn instant_checked_duration_since(lhs: &Instant, earlier: Instant) -> SignedDuration {
        let (neg, amt) = match lhs.checked_duration_since(earlier) {
            Some(amt) => (false, amt),
            None => (true, earlier - *lhs),
        };
        Self::signed_duration_new(amt, neg)
    }
    pub fn system_time_checked_duration_since(lhs: &SystemTime, earlier: SystemTime) -> SignedDuration {
        let (neg, amt) = match lhs.duration_since(earlier) {
            Ok(amt) => (false, amt),
            Err(e) => (true, e.duration()),
        };
        Self::signed_duration_new(amt, neg)
    }

    pub fn saturating_add_mut(&mut self, amt: Duration) {
        self.offset = self.offset.checked_add(amt).unwrap_or(Duration::MAX)
    }
    pub fn saturating_sub_mut(&mut self, amt: Duration) {
        self.offset = self.offset.checked_sub(amt).unwrap_or(Self::EPOCH.offset)
    }

    pub fn signed_duration_new(amt: Duration, neg: bool) -> SignedDuration {
        match neg {
            true => Err(amt),
            false => Ok(amt),
        }
    }
    pub fn signed_duration_magnitude(amt: &SignedDuration) -> &Duration {
        match amt {
            Ok(amt) => amt,
            Err(amt) => amt,
        }
    }
    #[inline]
    pub fn signed_duration_neg(amt: &SignedDuration) -> bool {
        match amt {
            Ok(..) => false,
            Err(..) => true,
        }
    }
    pub fn signed_duration_seconds_f64(amt: &SignedDuration) -> f64 {
        match amt {
            Ok(amt) => amt.as_secs_f64(),
            Err(amt) => -amt.as_secs_f64(),
        }
    }
    pub fn signed_duration_seconds_f32(amt: &SignedDuration) -> f32 {
        match amt {
            Ok(amt) => amt.as_secs_f32(),
            Err(amt) => -amt.as_secs_f32(),
        }
    }
    pub fn signed_duration_seconds_i64(amt: &SignedDuration) -> i64 {
        let neg = Self::signed_duration_neg(amt);
        match i64::try_from(Self::signed_duration_magnitude(amt).as_secs()) {
            Ok(amt) if neg => -amt,
            Ok(amt) => -amt,
            Err(..) if neg => i64::MIN,
            Err(..) => i64::MAX,
        }
    }
    pub fn signed_duration_system_time(amt: SignedDuration, to: &SystemTime) -> Option<SystemTime> {
        match amt {
            Ok(amt) =>
                to.checked_add(amt),
            Err(amt) =>
                to.checked_sub(amt),
        }
    }
    pub fn signed_duration_instant(amt: SignedDuration, to: &Instant) -> Option<Instant> {
        match amt {
            Ok(amt) =>
                to.checked_add(amt),
            Err(amt) =>
                to.checked_sub(amt),
        }
    }
    pub fn signed_duration_saturating_sub(lhs: SignedDuration, rhs: SignedDuration) -> SignedDuration {
        let neg = match (lhs, rhs) {
            (Ok(..), Err(..)) => false,
            (Err(..), Ok(..)) => true,
            (Ok(l), Ok(r)) => r > l,
            (Err(l), Err(r)) => l > r,
        };
        let amt = match (lhs, rhs, neg) {
            | (Ok(p), Err(n), _)
            | (Err(n), Ok(p), _)
            => p.saturating_add(n),
            | (Ok(s), Ok(b), true)
            | (Err(s), Err(b), false)
            | (Ok(b), Ok(s), false)
            | (Err(b), Err(s), true)
                => b.saturating_sub(s),
        };
        Self::signed_duration_new(amt, neg)
    }
    pub fn saturating_signed_add(mut self, amt: SignedDuration) -> Self {
        self.saturating_signed_add_mut(amt);
        self
    }
    pub fn saturating_signed_add_mut(&mut self, amt: SignedDuration) {
        match amt {
            Ok(amt) => self.saturating_add_mut(amt),
            Err(amt) => self.saturating_sub_mut(amt),
        }
    }
}

/// prints [Timestamp::timestamp], truncates to the second
impl fmt::Display for Timestamp {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.timestamp(), f)
    }
}

impl Timestamp {
    /// time in far future expected to fit within [SystemTime]
    #[allow(unreachable_patterns)]
    pub const MAX_SYS: Self = match () {
        #[cfg(all(target_os = "linux", not(target_pointer_width = "32")))]
        _ => Self::with_timestamp(0x8000_0000_0000_0000),
        #[cfg(not(target_pointer_width = "32"))]
        _ => Self::with_timestamp(0x0000_0800_0000_0000),
        _ => Self::with_timestamp(0xf000_0000),
    };
    /// time in far future expected to fit within [Instant](std::time::Instant)
    #[allow(unreachable_patterns)]
    pub const MAX_INSTANT: Self = match () {
        #[cfg(any(target_os = "linux", target_pointer_width = "32"))]
        _ => Self::MAX_SYS,
        _ => Self::with_timestamp(0x1_8000_0000),
    };
    /// [SystemTime::UNIX_EPOCH] + self
    #[inline]
    pub fn to_system_time(self) -> Option<SystemTime> {
        SystemTime::UNIX_EPOCH.checked_add(self.offset)
    }
    pub fn into_system_time(self) -> SystemTime {
        match self.to_system_time() {
            Some(time) => time,
            #[cfg(debug_assertions)]
            None => panic!("{self} too large for SystemTime"),
            #[cfg(not(debug_assertions))]
            None => unsafe {
                log::warn!("{self} too large for SystemTime");
                Self::MAX_SYS.to_system_time().unwrap_unchecked()
            },
        }
    }
    /// [SystemTime::duration_since]
    #[inline]
    pub fn try_from_system_time(time: &SystemTime) -> Result<Self, SystemTimeError> {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .map(Self::after_epoch)
    }
    /// [Self::try_from_system_time] or clamp to [Self::EPOCH]
    pub fn from_system_time(time: &SystemTime) -> Self {
        match Self::try_from_system_time(time) {
            Ok(ts) => ts,
            Err(_e) => {
                log::warn!("{time:?} precedes unix epoch: {_e}");
                #[cfg(debug_assertions)]
                debug_assert!(*time < SystemTime::UNIX_EPOCH);
                Self::EPOCH
            },
        }
    }
}
impl From<Timestamp> for SystemTime {
    fn from(ts: Timestamp) -> Self {
        ts.into_system_time()
    }
}
impl From<&'_ Timestamp> for SystemTime {
    fn from(ts: &Timestamp) -> Self {
        Self::from(*ts)
    }
}
impl From<&'_ SystemTime> for Timestamp {
    fn from(time: &SystemTime) -> Self {
        Self::from_system_time(time)
    }
}
impl From<SystemTime> for Timestamp {
    #[inline]
    fn from(time: SystemTime) -> Self {
        Self::from(&time)
    }
}
#[test]
fn timestamp_max() {
    assert!(Timestamp::MAX_SYS.to_system_time().is_some())
}

#[cfg(feature = "serde")]
impl ser::Serialize for Timestamp {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.offset.subsec_nanos() {
            0 => self.timestamp().serialize(serializer),
            _ => self.timestamp_f64().serialize(serializer),
        }
    }
}
/// TODO: properly accept u64 directly and maybe even Duration's serde impl itself if it has one?
#[cfg(feature = "serde")]
impl<'de> de::Deserialize<'de> for Timestamp {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        f64::deserialize(deserializer).map(Self::with_timestamp_f64)
    }
}
