use {
    crate::timer::{
        BlishAlert,
        BlishDirection,
        BlishMarker,
        BlishSound,
        TimerAction,
        TimerAlert,
        TimerDirection,
        TimerFile,
        TimerFileAlert,
        TimerFileDirection,
        TimerFileMarker,
        TimerFileSound,
        TimerMarker,
        TimerSound,
        TimerTrigger,
    },
    serde::{
        de::{self, Error as _, MapAccess, Visitor},
        Deserialize,
        Serialize,
    },
    std::{
        ops::{Deref, DerefMut},
        sync::Arc,
    },
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimerPhase {
    pub name: String,
    pub start: TimerTrigger,
    #[serde(default)]
    pub finish: Option<TimerTrigger>,
    #[serde(default)]
    pub alerts: Vec<BlishAlert>,
    #[serde(default)]
    pub actions: Vec<TimerAction>,
    /*
     * Not yet implemented:
     * - directions
     * - markers
     * - sounds
     * - actions (SkipTime)
     */
    #[serde(default)]
    #[allow(dead_code)]
    pub directions: Vec<BlishDirection>,
    #[serde(flatten, default)]
    pub markers: BlishMarkers,
    #[serde(default)]
    #[allow(dead_code)]
    pub sounds: Vec<BlishSound>,
}

#[cfg(todo)]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct BlishMarkersHolder {
    pub markers: BlishMarkers,
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct BlishMarkers(pub Vec<BlishMarker>);
impl Deref for BlishMarkers {
    type Target = Vec<BlishMarker>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for BlishMarkers {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'de> Deserialize<'de> for BlishMarkers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct MyVisitor;

        impl<'d> Visitor<'d> for MyVisitor {
            type Value = Vec<BlishMarker>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                f.write_str("a map of markers")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'d>,
            {
                let mut markers = Vec::new();
                while let Some((key, value)) = access.next_entry::<&str, Vec<BlishMarker>>()? {
                    if key == "markers" {
                        markers.extend(value);
                    } else {
                        return Err(M::Error::unknown_field(key, &["markers"]));
                    }
                }
                Ok(markers)
            }
        }
        Ok(BlishMarkers(deserializer.deserialize_struct(
            "BlishMarkers",
            &["markers"],
            MyVisitor,
        )?))
    }
}

#[derive(Debug, Clone)]
pub struct TimerFilePhase {
    pub(super) timer: Arc<TimerFile>,
    pub(super) phase_idx: usize,
}

impl TimerFilePhase {
    #[inline]
    pub fn from_index(timer: Arc<TimerFile>, phase_idx: usize) -> Option<Self> {
        (timer.phases.len() > phase_idx).then(|| Self { timer, phase_idx })
    }

    #[inline]
    pub fn new(timer: Arc<TimerFile>) -> Option<Self> {
        Self::from_index(timer, 0)
    }

    /// rewind to first phase
    ///
    /// (this couldn't have been constructed if at least one phase hadn't existed)
    pub fn reset(&mut self) {
        self.phase_idx = 0;
    }

    pub fn next(self) -> Option<Self> {
        Self::from_index(self.timer, self.phase_idx + 1)
    }

    #[inline]
    pub fn as_phase(&self) -> &TimerPhase {
        unsafe { self.timer.phases.get_unchecked(self.phase_idx) }
    }

    #[inline]
    pub fn timer(&self) -> &Arc<TimerFile> {
        &self.timer
    }

    pub fn iter_alerts(&self) -> impl Iterator<Item = TimerFileAlert> + '_ {
        (0..self.alerts.len()).map(|alert_idx| TimerFileAlert { phase: self.clone(), alert_idx })
    }
    pub fn get_alerts(&self) -> impl Iterator<Item = TimerAlert> + '_ {
        self.iter_alerts().flat_map(|a| a.fan_out())
    }
    pub fn iter_sounds(&self) -> impl Iterator<Item = TimerFileSound> + '_ {
        (0..self.sounds.len()).map(|sound_idx| TimerFileSound { phase: self.clone(), sound_idx })
    }
    pub fn get_sounds(&self) -> impl Iterator<Item = TimerSound> + '_ {
        self.iter_sounds().flat_map(|a| a.fan_out())
    }
    pub fn iter_markers(&self) -> impl Iterator<Item = TimerFileMarker> + '_ {
        (0..self.markers.len()).map(|marker_idx| TimerFileMarker { phase: self.clone(), marker_idx })
    }
    pub fn get_markers(&self) -> impl Iterator<Item = TimerMarker> + '_ {
        self.iter_markers().flat_map(|a| a.fan_out())
    }
    pub fn iter_directions(&self) -> impl Iterator<Item = TimerFileDirection> + '_ {
        (0..self.directions.len()).map(|dir_idx| TimerFileDirection { phase: self.clone(), dir_idx })
    }
    pub fn get_directions(&self) -> impl Iterator<Item = TimerDirection> + '_ {
        self.iter_directions().flat_map(|a| a.fan_out())
    }
}

impl Deref for TimerFilePhase {
    type Target = TimerPhase;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_phase()
    }
}
