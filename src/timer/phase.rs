use {
    super::{direction::BlishDirection, BlishMarker, TimerMarker},
    crate::timer::{BlishAlert, TimerAction, TimerAlert, TimerTrigger},
    serde::{
        de::{self, Error as _, MapAccess, Visitor},
        Deserialize,
        Serialize,
    },
    serde_json::Value,
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
     */
    #[serde(skip, default)]
    #[allow(dead_code)]
    directions: Vec<BlishDirection>,
    #[serde(flatten, default)]
    pub markers: BlishMarkers,
    #[serde(skip, default)]
    #[allow(dead_code)]
    sounds: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct BlishMarkersHolder {
    pub markers: BlishMarkers,
}

impl TimerPhase {
    pub fn get_alerts(&self) -> Vec<TimerAlert> {
        self.alerts.iter().flat_map(BlishAlert::get_alerts).collect()
    }
    pub fn get_markers(&self) -> Vec<TimerMarker> {
        self.markers.0.iter().flat_map(BlishMarker::get_markers).collect()
    }
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct BlishMarkers(pub Vec<BlishMarker>);

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
