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
    name: String,
    destination: BlishVec3,
    texture: RelativePathBuf,
    anim_speed: f32,
    duration: f32,
    timestamps: Vec<f32>,
}

#[allow(dead_code)]
impl BlishDirection {
    fn direction(&self, timestamp: f32) -> TimerDirection {
        let destination = self.destination.to_vec3();
        TimerDirection {
            name: self.name.clone(),
            texture: self.texture.clone(),
            anim_speed: self.anim_speed,
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
    name: String,
    destination: Vec3,
    texture: RelativePathBuf,
    anim_speed: f32,
    duration: f32,
    timestamp: f32,
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
