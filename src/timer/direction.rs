use {
    super::{
        marker::{default_duration, default_opacity},
        BlishVec3,
        TimerFilePhase,
    },
    glam::Vec3,
    serde::{Deserialize, Serialize},
    std::{ops::Deref, path::PathBuf},
    tokio::time::{Duration, Instant},
};

fn default_anim_speed() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlishDirection {
    pub name: String,
    pub destination: BlishVec3,
    pub texture: PathBuf,
    #[serde(default = "default_anim_speed")]
    pub anim_speed: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_duration")]
    pub duration: f32,
    pub timestamps: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<String>,
}

impl BlishDirection {
    #[inline]
    pub fn destination(&self) -> Vec3 {
        self.destination.to_vec3()
    }

    #[inline]
    pub fn offset_set(&self) -> &str {
        self.set
            .as_ref()
            .map(|s| &s[..])
            .unwrap_or(super::TimerAction::DEFAULT_SET)
    }
}

#[derive(Debug, Clone)]
pub struct TimerDirection {
    pub file_direction: TimerFileDirection,
    pub timestamp: f32,
}
#[allow(dead_code)]
impl TimerDirection {
    pub fn raw_timestamp(&self) -> Duration {
        Duration::from_secs_f32(self.timestamp)
    }
    pub fn timestamp(&self) -> Duration {
        self.raw_timestamp()
            .checked_sub(self.duration())
            .unwrap_or_default()
    }
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f32(self.duration)
    }
    pub fn end(&self, start: Instant) -> Instant {
        self.start(start) + self.duration()
    }
    pub fn start(&self, start: Instant) -> Instant {
        start + self.timestamp()
    }
    pub fn remaining(&self, start: Instant) -> Duration {
        self.end(start).saturating_duration_since(Instant::now())
    }
}
impl Deref for TimerDirection {
    type Target = TimerFileDirection;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.file_direction
    }
}

#[derive(Debug, Clone)]
pub struct TimerFileDirection {
    pub(super) phase: TimerFilePhase,
    pub(super) dir_idx: usize,
}
impl TimerFileDirection {
    #[inline]
    fn new(phase: TimerFilePhase, dir_idx: usize) -> Option<Self> {
        (phase.directions.len() > dir_idx).then(|| Self { phase, dir_idx })
    }
    #[inline]
    pub fn as_dir(&self) -> &BlishDirection {
        unsafe { self.phase.directions.get_unchecked(self.dir_idx) }
    }

    pub fn get_directions(&self) -> impl Iterator<Item = TimerDirection> + '_ {
        self.as_dir().timestamps.iter().map(|&timestamp| TimerDirection {
            file_direction: self.clone(),
            timestamp,
        })
    }
    pub fn fan_out(self) -> impl Iterator<Item = TimerDirection> + 'static {
        (0..self.timestamps.len()).map(move |ts_idx| TimerDirection {
            timestamp: unsafe { *self.timestamps.get_unchecked(ts_idx) },
            file_direction: self.clone(),
        })
    }
}
impl Deref for TimerFileDirection {
    type Target = BlishDirection;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_dir()
    }
}
