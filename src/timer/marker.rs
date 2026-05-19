use {
    crate::timer::BlishVec3,
    glam::{Mat4, Vec3},
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
    tokio::time::{Duration, Instant},
};

fn default_size() -> f32 {
    return 1.0;
}

fn default_opacity() -> f32 {
    return 0.8;
}

fn default_duration() -> f32 {
    return 10.0;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlishMarker {
    #[serde(default)]
    pub position: BlishVec3,
    #[serde(default)]
    pub rotation: BlishVec3,
    #[serde(default = "default_size")]
    pub size: f32,
    #[serde(default)]
    pub fade_center: bool,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    pub texture: PathBuf,
    #[serde(default = "default_duration")]
    pub duration: f32,
    #[serde(default)]
    pub timestamps: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<String>,
}

impl BlishMarker {
    fn marker(&self, timestamp: f32) -> TimerMarker {
        let position = self.position.to_vec3();
        let rotation = self.rotation.to_vec3();
        let kind = if rotation == Vec3::ZERO {
            RotationType::Billboard
        } else {
            let rotation_rads = rotation.map(|x| x.to_radians());
            RotationType::Rotation(rotation_rads)
        };
        TimerMarker {
            position,
            size: self.size,
            duration: self.duration,
            opacity: self.opacity,
            texture: self.texture.clone(),
            timestamp,
            kind,
        }
    }

    pub fn get_markers(&self) -> Vec<TimerMarker> {
        self.timestamps.iter().map(|&ts| self.marker(ts)).collect()
    }
}

#[derive(Clone)]
pub enum RotationType {
    Rotation(Vec3),
    Billboard,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct TimerMarker {
    pub kind: RotationType,
    pub position: Vec3,
    pub size: f32,
    pub opacity: f32,
    pub texture: PathBuf,
    pub timestamp: f32,
    pub duration: f32,
}

impl TimerMarker {
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

    #[allow(dead_code)]
    pub fn remaining(&self, start: Instant) -> Duration {
        self.end(start).saturating_duration_since(Instant::now())
    }
    pub fn model_matrix(&self) -> Mat4 {
        // scale first
        let scaler = self.size;
        let mtx_scale = Mat4::from_scale(Vec3::new(scaler, scaler, scaler));
        // then rotate the points
        let mtx_rotation = match self.kind {
            // billboards should have their rotation component handled elsewhere ideally
            // perhaps *prior* to the application of this, thus NOOP :p
            RotationType::Billboard => Mat4::IDENTITY, //Mat4::from_rotation_y(180.0f32.to_radians()), //Mat4::from_rotation_x(90.0f32.to_radians()), //* Mat4::from_rotation_z(90.0f32.to_radians()),
            RotationType::Rotation(rot) =>
                Mat4::from_rotation_x(rot.x) * Mat4::from_rotation_y(rot.y) * Mat4::from_rotation_z(rot.z),
        };
        // then move them
        //let mtx_position = Mat4::from_translation(self.position);
        //mtx_rotation = mtx_rotation * Mat4::from_rotation_y(180.0f32.to_radians());
        mtx_scale * mtx_rotation // * mtx_position
    }
}
