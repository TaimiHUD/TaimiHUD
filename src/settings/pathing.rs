#[cfg(feature = "space")]
use {
    crate::controller::Controller,
    taimi_meta::ui::MapContext,
    taimi_pack::attributes::{Festival, Festivals},
};
use {
    crate::settings::Settings,
    serde::{Deserialize, Serialize},
    std::{collections::BTreeMap, fmt, sync::Arc},
    strum::{IntoStaticStr, VariantArray},
};

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct PathingSettings {
    #[serde(default, skip_serializing_if = "SpaceSettings::is_empty")]
    pub space: SpaceSettings,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub festival_filter: Arc<BTreeMap<String, FestivalPreference>>,
    #[cfg(feature = "paths-lua")]
    #[serde(default, rename = "deleteme_script_enable")]
    pub scripting_enable: bool,
    #[cfg(feature = "paths-lua")]
    #[serde(default, rename = "deleteme_script_auto")]
    pub scripting_auto: bool,
    #[cfg(feature = "paths-lua")]
    #[serde(default, rename = "deleteme_script_unsecure")]
    pub scripting_unsecured: bool,
    #[cfg(feature = "paths-lua")]
    #[serde(
        default = "taimi_hoard::a_f32::<{1.0f32.to_bits()}>",
        rename = "deleteme_script_tick_rate"
    )]
    pub scripting_tick_rate: f32,
}

impl PathingSettings {
    #[cfg(feature = "space")]
    pub async fn pathing_state_update(settings: &mut Settings, path: String, state: bool) {
        if settings.disabled_paths.contains(&path) && state {
            settings.disabled_paths_mut().remove(&path);
        } else if !state {
            settings.disabled_paths_mut().insert(path);
        }
    }

    #[cfg(feature = "space")]
    pub fn get_festival_preference(&self, festival: Festival) -> Option<FestivalPreference> {
        self.festival_filter.get(festival.as_str()).copied()
    }
    #[cfg(feature = "space")]
    pub fn festival_preferences(&self) -> (Festivals, Festivals) {
        Festival::ALL
            .iter()
            .map(|&f| match self.get_festival_preference(f) {
                None => (Default::default(), Default::default()),
                Some(true) => (Festivals::for_festival(f), Festivals::empty()),
                Some(false) => (Festivals::empty(), Festivals::for_festival(f)),
            })
            .unzip()
    }
    #[cfg(feature = "space")]
    pub fn set_festival_preference(&mut self, festival: Festival, pref: Option<FestivalPreference>) {
        let festival_filter = self.festival_filter_mut();
        match pref {
            None => {
                festival_filter.remove(festival.as_str());
            },
            Some(pref) => {
                festival_filter.insert(festival.into(), pref);
            },
        }
        Controller::with_sender(|s| {
            if let Some(a) = &s.api {
                a.festivals.send_if_modified(|festivals| {
                    let prev = festivals.get();
                    festivals.set_preference(festival, pref);
                    prev != festivals.get()
                });
            }
        });
    }
    #[cfg(feature = "space")]
    pub fn festival_filter_mut(&mut self) -> &mut BTreeMap<String, FestivalPreference> {
        Arc::make_mut(&mut self.festival_filter)
    }
}

/// TODO: enum that can distinguish ignore forever vs dismissed
/// vs enable forever vs listen to api/calendar etc
pub type FestivalPreference = bool;

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct SpaceSettings {
    #[serde(
        rename = "goggles0",
        default,
        skip_serializing_if = "GogglesSettings::is_empty"
    )]
    pub goggles: GogglesSettings,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_source: Option<CameraSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toggle_granularity_space: Option<ToggleGranularity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toggle_granularity_map: Option<ToggleGranularity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toggle_group_orientation: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_space: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_map_world: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_map_mini: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_open: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail_textured_space: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_trail_textured_mini: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_trail_textured_world: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_max: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_fade_intensity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_overlap_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_feather_scale: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail_alpha: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poi_alpha: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_trail_alpha_world: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_poi_alpha_world: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_trail_alpha_mini: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_poi_alpha_mini: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_poi_space: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_poi_mini: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_poi_world: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_trail_space: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_trail_mini: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_trail_world: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail_y_offset: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail_resolution: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail_width: Option<f32>,
}

impl SpaceSettings {
    pub const DEFAULT_CAMERA_SOURCE: CameraSource = CameraSource::MumbleLink;
    pub const DEFAULT_TOGGLE_GRANULARITY_MAP: ToggleGranularity = ToggleGranularity::Individual;
    pub const DEFAULT_TOGGLE_GROUP_ORIENTATION: bool = false;
    pub const DEFAULT_TOGGLE_GRANULARITY_SPACE: ToggleGranularity = ToggleGranularity::Individual;
    pub const DEFAULT_VISIBLE: bool = true;
    pub const DEFAULT_VISIBLE_MAP: bool = true;
    pub const DEFAULT_TRAIL_TEXTURED: bool = true;
    pub const DEFAULT_TRAIL_TEXTURED_MAP_MINI: bool = false;
    pub const DEFAULT_TRAIL_TEXTURED_MAP_WORLD: bool = true;
    pub const DEFAULT_TRAIL_Y_OFFSET: f32 = 0.001;
    pub const DEFAULT_TRAIL_RESOLUTION: f32 = 1.0 / 20.0;
    pub const DEFAULT_TRAIL_WIDTH: f32 = 1.016;
    pub const DEFAULT_DISTANCE_MAX: f32 = 700.0;
    pub const DEFAULT_POI_ALPHA: f32 = 1.0;
    pub const DEFAULT_POI_SCALE: f32 = 1.0;
    pub const DEFAULT_POI_SCALE_MAP: f32 = 1.0;
    pub const DEFAULT_TRAIL_ALPHA: f32 = Self::DEFAULT_POI_ALPHA;
    pub const DEFAULT_TRAIL_SCALE: f32 = 1.0;
    pub const DEFAULT_TRAIL_SCALE_MAP: f32 = 5.0;
    pub const DEFAULT_POI_MAP_ALPHA: f32 = Self::DEFAULT_POI_ALPHA;
    pub const DEFAULT_TRAIL_MAP_ALPHA: f32 = Self::DEFAULT_TRAIL_ALPHA;
    pub const DEFAULT_DISTANCE_FADE_INTENSITY: f32 = 84.0;
    pub const DEFAULT_PLAYER_OVERLAP_THRESHOLD: f32 = 38.0;
    pub const DEFAULT_EDGE_FEATHER_SCALE: f32 = 0.8f32;
    pub const DEFAULT_MAP_OPEN: bool = false;

    pub const NONE_F32: f32 = f32::MIN;

    fn optional_f32(v: f32) -> Option<f32> {
        match v {
            Self::NONE_F32 => None,
            v => Some(v),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                camera_source: None | Some(Self::DEFAULT_CAMERA_SOURCE),
                toggle_granularity_space: None | Some(Self::DEFAULT_TOGGLE_GRANULARITY_SPACE),
                toggle_granularity_map: None | Some(Self::DEFAULT_TOGGLE_GRANULARITY_MAP),
                toggle_group_orientation: None | Some(Self::DEFAULT_TOGGLE_GROUP_ORIENTATION),
                visible_space: None | Some(Self::DEFAULT_VISIBLE),
                visible_map_world: None | Some(Self::DEFAULT_VISIBLE_MAP),
                visible_map_mini: None | Some(Self::DEFAULT_VISIBLE_MAP),
                map_open: None | Some(Self::DEFAULT_MAP_OPEN),
                distance_max: None,
                distance_fade_intensity: None,
                player_overlap_threshold: None,
                edge_feather_scale: None | Some(Self::DEFAULT_EDGE_FEATHER_SCALE),
                trail_alpha: None,
                poi_alpha: None,
                trail_textured_space: None | Some(Self::DEFAULT_TRAIL_TEXTURED),
                map_trail_textured_mini: None | Some(Self::DEFAULT_TRAIL_TEXTURED_MAP_MINI),
                map_trail_textured_world: None | Some(Self::DEFAULT_TRAIL_TEXTURED_MAP_WORLD),
                map_poi_alpha_mini: None,
                map_trail_alpha_mini: None,
                map_poi_alpha_world: None,
                map_trail_alpha_world: None,
                scale_poi_space: None,
                scale_poi_mini: None,
                scale_poi_world: None,
                scale_trail_space: None,
                scale_trail_mini: None,
                scale_trail_world: None,
                trail_y_offset: None,
                trail_resolution: None,
                trail_width: None,
                goggles,
            } if goggles.is_empty() => true,
            _ => false,
        }
    }

    pub fn camera_source(&self) -> CameraSource {
        self.camera_source.unwrap_or(Self::DEFAULT_CAMERA_SOURCE)
    }
    pub fn toggle_granularity_space(&self) -> ToggleGranularity {
        self.toggle_granularity_space
            .unwrap_or(Self::DEFAULT_TOGGLE_GRANULARITY_SPACE)
    }
    pub fn toggle_granularity_map(&self) -> ToggleGranularity {
        self.toggle_granularity_map
            .unwrap_or(Self::DEFAULT_TOGGLE_GRANULARITY_MAP)
    }
    pub fn toggle_group_orientation(&self) -> bool {
        self.toggle_group_orientation
            .unwrap_or(Self::DEFAULT_TOGGLE_GROUP_ORIENTATION)
    }

    pub fn visible_space(&self) -> bool {
        self.visible_space.unwrap_or(Self::DEFAULT_VISIBLE)
    }
    pub fn visible_minimap(&self) -> bool {
        self.visible_map_mini.unwrap_or(Self::DEFAULT_VISIBLE_MAP)
    }
    pub fn visible_worldmap(&self) -> bool {
        self.visible_map_world.unwrap_or(Self::DEFAULT_VISIBLE_MAP)
    }
    #[cfg(feature = "space")]
    pub fn visible_map(&self, ctx: MapContext) -> bool {
        match ctx {
            MapContext::Global => self.visible_worldmap(),
            MapContext::Minimap => self.visible_minimap(),
        }
    }
    pub fn map_open(&self) -> bool {
        self.map_open.unwrap_or(Self::DEFAULT_MAP_OPEN)
    }

    #[cfg(feature = "space")]
    pub fn trail_textured_map(&self, ctx: MapContext) -> bool {
        match ctx {
            MapContext::Global => self.trail_textured_worldmap(),
            MapContext::Minimap => self.trail_textured_minimap(),
        }
    }
    pub fn trail_textured_space(&self) -> bool {
        self.trail_textured_space.unwrap_or(Self::DEFAULT_TRAIL_TEXTURED)
    }
    pub fn trail_textured_minimap(&self) -> bool {
        self.map_trail_textured_mini
            .unwrap_or(Self::DEFAULT_TRAIL_TEXTURED_MAP_MINI)
    }
    pub fn trail_textured_worldmap(&self) -> bool {
        self.map_trail_textured_world
            .unwrap_or(Self::DEFAULT_TRAIL_TEXTURED_MAP_WORLD)
    }

    pub fn distance_max(&self) -> f32 {
        self.distance_max.unwrap_or(Self::DEFAULT_DISTANCE_MAX)
    }

    pub fn distance_fade_intensity(&self) -> Option<f32> {
        self.distance_fade_intensity
            .map(Self::optional_f32)
            .unwrap_or(Some(Self::DEFAULT_DISTANCE_FADE_INTENSITY))
    }
    pub fn player_overlap_threshold(&self) -> Option<f32> {
        self.player_overlap_threshold
            .map(Self::optional_f32)
            .unwrap_or(Some(Self::DEFAULT_PLAYER_OVERLAP_THRESHOLD))
    }
    pub fn edge_feather_scale(&self) -> Option<f32> {
        self.edge_feather_scale
            .or(Some(Self::DEFAULT_EDGE_FEATHER_SCALE))
            .and_then(Self::optional_f32)
    }

    pub fn trail_alpha(&self) -> f32 {
        self.trail_alpha.unwrap_or(Self::DEFAULT_TRAIL_ALPHA)
    }
    pub fn poi_alpha(&self) -> f32 {
        self.trail_alpha.unwrap_or(Self::DEFAULT_POI_ALPHA)
    }

    pub fn trail_alpha_worldmap(&self) -> f32 {
        self.map_trail_alpha_world
            .unwrap_or(Self::DEFAULT_TRAIL_MAP_ALPHA)
    }
    pub fn poi_alpha_worldmap(&self) -> f32 {
        self.map_poi_alpha_world.unwrap_or(Self::DEFAULT_POI_MAP_ALPHA)
    }
    pub fn trail_alpha_minimap(&self) -> f32 {
        self.map_trail_alpha_mini.unwrap_or(Self::DEFAULT_TRAIL_MAP_ALPHA)
    }
    pub fn poi_alpha_minimap(&self) -> f32 {
        self.map_poi_alpha_mini.unwrap_or(Self::DEFAULT_POI_MAP_ALPHA)
    }
    #[cfg(feature = "space")]
    pub fn trail_alpha_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.trail_alpha_worldmap(),
            MapContext::Minimap => self.trail_alpha_minimap(),
        }
    }
    #[cfg(feature = "space")]
    pub fn poi_alpha_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.poi_alpha_worldmap(),
            MapContext::Minimap => self.poi_alpha_minimap(),
        }
    }

    pub fn poi_scale_space(&self) -> f32 {
        self.scale_poi_space.unwrap_or(Self::DEFAULT_POI_SCALE)
    }
    pub fn poi_scale_worldmap(&self) -> f32 {
        self.scale_poi_world.unwrap_or(Self::DEFAULT_POI_SCALE_MAP)
    }
    pub fn poi_scale_minimap(&self) -> f32 {
        self.scale_poi_mini.unwrap_or(Self::DEFAULT_POI_SCALE_MAP)
    }
    #[cfg(feature = "space")]
    pub fn poi_scale_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.poi_scale_worldmap(),
            MapContext::Minimap => self.poi_scale_minimap(),
        }
    }

    pub fn trail_scale_space(&self) -> f32 {
        self.scale_trail_space.unwrap_or(Self::DEFAULT_TRAIL_SCALE)
    }
    pub fn trail_scale_worldmap(&self) -> f32 {
        self.scale_trail_world.unwrap_or(Self::DEFAULT_TRAIL_SCALE_MAP)
    }
    pub fn trail_scale_minimap(&self) -> f32 {
        self.scale_trail_mini.unwrap_or(Self::DEFAULT_TRAIL_SCALE_MAP)
    }
    #[cfg(feature = "space")]
    pub fn trail_scale_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.trail_scale_worldmap(),
            MapContext::Minimap => self.trail_scale_minimap(),
        }
    }

    pub fn trail_y_offset(&self) -> Option<f32> {
        self.trail_y_offset
            .map(Self::optional_f32)
            .unwrap_or(match self.goggles.enabled() {
                #[cfg(feature = "goggles")]
                true => Some(GogglesSettings::DEFAULT_TRAIL_Y_OFFSET),
                _ => Some(Self::DEFAULT_TRAIL_Y_OFFSET),
            })
    }
    pub fn trail_resolution(&self) -> f32 {
        self.trail_resolution.unwrap_or(Self::DEFAULT_TRAIL_RESOLUTION)
    }
    pub fn trail_width(&self) -> f32 {
        self.trail_width.unwrap_or(Self::DEFAULT_TRAIL_WIDTH)
    }
}

#[derive(
    Default,
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deserialize,
    Serialize,
    VariantArray,
    IntoStaticStr,
)]
#[serde(rename_all = "lowercase")]
pub enum ToggleGranularity {
    #[default]
    #[strum(serialize = "individual")]
    Individual,
    #[strum(serialize = "group")]
    Group,
    #[strum(serialize = "adaptive")]
    Adaptive,
}
impl ToggleGranularity {
    pub fn name_id_space(self) -> &'static str {
        match self {
            Self::Group => "config-paths-toggle-mode-space-group",
            Self::Adaptive => "config-paths-toggle-mode-space-on",
            Self::Individual => "config-paths-toggle-mode-space-off",
        }
    }
    pub fn name_id_map(self) -> &'static str {
        match self {
            Self::Group => "config-paths-toggle-mode-map-group",
            Self::Adaptive => "config-paths-toggle-mode-map-on",
            Self::Individual => "config-paths-toggle-mode-map-off",
        }
    }
    pub fn index(self) -> u8 {
        Self::VARIANTS.iter().position(|&v| v == self).unwrap_or_default() as _
    }
    pub fn from_index(idx: u8) -> Option<Self> {
        Self::VARIANTS.get(idx as usize).copied()
    }
}

#[derive(
    Default,
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deserialize,
    Serialize,
    VariantArray,
    IntoStaticStr,
)]
#[serde(rename_all = "lowercase")]
pub enum CameraSource {
    #[default]
    #[strum(serialize = "mumblelink")]
    MumbleLink,
    #[serde(rename = "rtapi")]
    #[strum(serialize = "rtapi")]
    RealTimeAPI,
}

impl CameraSource {
    pub fn name(self) -> &'static str {
        match self {
            Self::MumbleLink => "mumblelink",
            Self::RealTimeAPI => "rtapi",
        }
    }
}

impl AsRef<str> for CameraSource {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl fmt::Display for CameraSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct GogglesSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goggles_enabled: Option<bool>,

    /// X-ray opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obscured_alpha: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_scale: Option<f32>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map_depth_calibration: Arc<BTreeMap<u32, (f32, f32)>>,
}

impl GogglesSettings {
    pub const DEFAULT_ENABLED: bool = false;
    pub const DEFAULT_OBSCURED_ALPHA: f32 = 0.15;
    pub const DEFAULT_DEPTH_CALIBRATION: (f32, f32) = (1.0, 1.0);
    #[cfg(todo)]
    pub const DEFAULT_EDGE_SCALE: f32 = 0.5f32;
    pub const DEFAULT_EDGE_SCALE: f32 = SpaceSettings::NONE_F32;
    pub const DEFAULT_TRAIL_Y_OFFSET: f32 = 0.025;

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                goggles_enabled: None | Some(Self::DEFAULT_ENABLED),
                obscured_alpha: None,
                edge_scale: None | Some(Self::DEFAULT_EDGE_SCALE),
                map_depth_calibration,
            } if map_depth_calibration.is_empty() => true,
            _ => false,
        }
    }

    pub fn enabled(&self) -> bool {
        self.goggles_enabled.unwrap_or(Self::DEFAULT_ENABLED)
    }

    pub fn obscured_alpha(&self) -> f32 {
        self.obscured_alpha.unwrap_or(Self::DEFAULT_OBSCURED_ALPHA)
    }

    pub fn edge_scale(&self) -> Option<f32> {
        self.edge_scale
            .or(Some(Self::DEFAULT_EDGE_SCALE))
            .and_then(SpaceSettings::optional_f32)
    }

    pub fn map_depth_calibration(&self, map_id: u32) -> (f32, f32) {
        self.map_depth_calibration
            .get(&map_id)
            .copied()
            .unwrap_or(Self::DEFAULT_DEPTH_CALIBRATION)
    }

    pub fn map_depth_calibration_mut(&mut self) -> &mut BTreeMap<u32, (f32, f32)> {
        Arc::make_mut(&mut self.map_depth_calibration)
    }
}
