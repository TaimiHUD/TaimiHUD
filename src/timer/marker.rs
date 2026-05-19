use {
    crate::timer::{BlishVec3, TimerFilePhase},
    glam::{EulerRot, Mat4, Quat, Vec3},
    serde::{Deserialize, Serialize},
    std::{ops::Deref, path::PathBuf},
    taimi_meta::coords::vec_eq as vec32_eq,
    tokio::time::{Duration, Instant},
};

fn default_size() -> f32 {
    return 1.0;
}

pub(super) fn default_opacity() -> f32 {
    return 0.8;
}

pub(super) fn default_duration() -> f32 {
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
    #[inline]
    pub fn position(&self) -> Vec3 {
        self.position.to_vec3()
    }
    #[inline]
    pub fn rotation(&self) -> RotationType {
        // TODO: move this to taimi_hoard dep once that's finally merged...
        if vec32_eq(self.rotation.child, Vec3::ZERO) {
            RotationType::Billboard
        } else {
            let rotation_rads = self.rotation.to_vec3().map(|deg| deg.to_radians());
            RotationType::Rotation(rotation_rads)
        }
    }

    #[inline]
    pub fn offset_set(&self) -> &str {
        self.set
            .as_ref()
            .map(|s| &s[..])
            .unwrap_or(super::TimerAction::DEFAULT_SET)
    }
}

#[derive(Copy, Clone)]
pub enum RotationType {
    Rotation(Vec3),
    Billboard,
}

#[derive(Debug, Clone)]
pub struct TimerMarker {
    pub file_marker: TimerFileMarker,
    pub timestamp: f32,
}
impl TimerMarker {
    pub fn end_timestamp(&self) -> Duration {
        Duration::from_secs_f32(self.timestamp)
    }
    pub fn start_timestamp(&self) -> Duration {
        self.end_timestamp().saturating_sub(self.duration())
    }
    pub fn end(&self, start: Instant) -> Instant {
        start + self.end_timestamp()
    }
    pub fn start(&self, start: Instant) -> Instant {
        start + self.start_timestamp()
    }

    #[allow(dead_code)]
    pub fn remaining(&self, start: Instant) -> Duration {
        self.end(start).saturating_duration_since(Instant::now())
    }
}
impl Deref for TimerMarker {
    type Target = TimerFileMarker;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.file_marker
    }
}

#[derive(Debug, Clone)]
pub struct TimerFileMarker {
    pub(super) phase: TimerFilePhase,
    pub(super) marker_idx: usize,
}
impl TimerFileMarker {
    #[inline]
    pub fn new(phase: TimerFilePhase, marker_idx: usize) -> Option<Self> {
        (phase.markers.len() > marker_idx).then(|| Self { phase, marker_idx })
    }

    #[inline]
    pub fn as_marker(&self) -> &BlishMarker {
        unsafe { self.phase.markers.get_unchecked(self.marker_idx) }
    }

    pub fn get_markers(&self) -> impl Iterator<Item = TimerMarker> + '_ {
        self.timestamps
            .iter()
            .map(|&timestamp| TimerMarker { file_marker: self.clone(), timestamp })
    }
    pub fn fan_out(self) -> impl Iterator<Item = TimerMarker> + 'static {
        (0..self.timestamps.len()).map(move |ts_idx| TimerMarker {
            timestamp: unsafe { *self.timestamps.get_unchecked(ts_idx) },
            file_marker: self.clone(),
        })
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f32(self.duration)
    }
    /// TODO: cache as field or is this already stored elsewhere?
    pub fn model_matrix(&self) -> Mat4 {
        // scale first
        // then rotate the points, then move them
        let scaler = Vec3::splat(self.size);
        let mtx_position = self.position();
        let rotation = match self.rotation() {
            // billboards have their rotation component handled elsewhere, thus NOOP :p
            RotationType::Billboard => Quat::IDENTITY,
            RotationType::Rotation(rot) =>
                Quat::from_euler(EulerRot::XZY, rot.x - core::f32::consts::FRAC_PI_2, rot.y, -rot.z),
        };
        Mat4::from_scale_rotation_translation(scaler, rotation, mtx_position)
    }
}
impl Deref for TimerFileMarker {
    type Target = BlishMarker;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_marker()
    }
}
