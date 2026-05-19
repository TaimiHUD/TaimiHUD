#![allow(dead_code)]
//! TODO

use {
    serde::{Deserialize, Serialize},
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
    fn to_sound(&self, timestamp: f32) -> TimerSound {
        TimerSound {
            text: self.text.clone(),
            set: self.set.clone(),
            timestamp,
        }
    }
    fn sounds(&self) -> impl Iterator<Item = TimerSound> + '_ {
        self.timestamps.iter().map(|&ts| self.to_sound(ts))
    }
}

#[derive(Debug, Clone)]
pub struct TimerSound {
    pub text: String,
    pub set: Option<String>,
    pub timestamp: f32,
}

impl TimerSound {
    pub fn timestamp(&self) -> Duration {
        Duration::from_secs_f32(self.timestamp)
    }
    pub fn start(&self, start: Instant) -> Instant {
        start + self.timestamp()
    }
}
