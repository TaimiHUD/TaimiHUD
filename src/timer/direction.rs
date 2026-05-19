use {
    super::BlishVec3,
    glam::Vec3,
    relative_path::RelativePathBuf,
    serde::{Deserialize, Serialize},
    tokio::time::{Duration, Instant},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlishDirection {
    pub name: String,
    pub destination: BlishVec3,
    pub texture: RelativePathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_speed: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    pub duration: f32,
    pub timestamps: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
}

#[allow(dead_code)]
impl BlishDirection {
    fn direction(&self, timestamp: f32) -> TimerDirection {
        let destination = self.destination.to_vec3();
        TimerDirection {
            name: self.name.clone(),
            uid: self.uid.clone(),
            texture: self.texture.clone(),
            anim_speed: self.anim_speed.unwrap_or(1.0),
            opacity: self.opacity.unwrap_or(0.8),
            duration: self.duration,
            destination,
            timestamp,
        }
    }

    pub fn get_directions(&self) -> Vec<TimerDirection> {
        self.timestamps.iter().map(|&ts| self.direction(ts)).collect()
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TimerDirection {
    pub name: String,
    pub uid: Option<String>,
    pub destination: Vec3,
    pub texture: RelativePathBuf,
    pub anim_speed: f32,
    pub opacity: f32,
    pub duration: f32,
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
