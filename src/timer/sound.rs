#![allow(dead_code)]
//! TODO

use {
    crate::timer::TimerFilePhase,
    serde::{Deserialize, Serialize},
    std::ops::Deref,
    tokio::time::{Duration, Instant},
};

pub use self::BlishSoundText as BlishSound;

/// text-to-speech
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlishSoundText {
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<String>,
    #[serde(default)]
    pub timestamps: Vec<f32>,
}

impl BlishSound {
    #[inline]
    pub fn offset_set(&self) -> &str {
        self.set
            .as_ref()
            .map(|s| &s[..])
            .unwrap_or(super::TimerAction::DEFAULT_SET)
    }
}

#[derive(Debug, Clone)]
pub struct TimerSound {
    pub file_sound: TimerFileSound,
    pub timestamp: f32,
}

impl TimerSound {
    #[inline]
    pub fn timestamp(&self) -> Duration {
        Duration::from_secs_f32(self.timestamp)
    }
    #[inline]
    pub fn start(&self, start: Instant) -> Instant {
        start + self.timestamp()
    }
}
impl Deref for TimerSound {
    type Target = TimerFileSound;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.file_sound
    }
}

#[derive(Debug, Clone)]
pub struct TimerFileSound {
    pub(super) phase: TimerFilePhase,
    pub(super) sound_idx: usize,
}
impl TimerFileSound {
    #[inline]
    pub fn new(phase: TimerFilePhase, sound_idx: usize) -> Option<Self> {
        (phase.sounds.len() > sound_idx).then(|| Self { phase, sound_idx })
    }
    #[inline]
    pub fn as_sound(&self) -> &BlishSound {
        unsafe { self.phase.sounds.get_unchecked(self.sound_idx) }
    }
    fn reset(&mut self) {
        self.sound_idx = 0;
    }

    pub fn get_sounds(&self) -> impl Iterator<Item = TimerSound> + '_ {
        self.timestamps
            .iter()
            .map(move |&timestamp| TimerSound { file_sound: self.clone(), timestamp })
    }
    pub fn fan_out(self) -> impl Iterator<Item = TimerSound> + 'static {
        (0..self.timestamps.len()).map(move |ts_idx| TimerSound {
            timestamp: unsafe { *self.timestamps.get_unchecked(ts_idx) },
            file_sound: self.clone(),
        })
    }
}
impl Deref for TimerFileSound {
    type Target = BlishSound;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_sound()
    }
}
