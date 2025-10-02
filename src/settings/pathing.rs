use {
    crate::settings::Settings,
    std::{collections::BTreeMap, fmt, sync::Arc},
    strum::{VariantArray, IntoStaticStr},
    serde::{Serialize, Deserialize},
    taimi_meta::coords::CurrentPerspective as MapContext,
};

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct PathingSettings {
    #[serde(default, skip_serializing_if = "SpaceSettings::is_empty")]
    pub space: SpaceSettings,
}

impl PathingSettings {
    #[cfg(feature = "space")]
    pub async fn pathing_state_update(settings: &mut Settings, path: String, state: bool) {
        if settings.disabled_paths.contains(&path) && state {
            settings.disabled_paths.remove(&path);
        } else if !state {
            settings.disabled_paths.insert(path);
        }
        let _ = settings.save().await;
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct SpaceSettings {
    #[serde(rename = "goggles0", default, skip_serializing_if = "GogglesSettings::is_empty")]
    pub goggles: GogglesSettings,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_source: Option<CameraSource>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_space: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_map_world: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_map_mini: Option<bool>,

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
}

impl SpaceSettings {
    pub const DEFAULT_CAMERA_SOURCE: CameraSource = CameraSource::MumbleLink;
    pub const DEFAULT_VISIBLE: bool = true;
    pub const DEFAULT_VISIBLE_MAP: bool = true;
    pub const DEFAULT_TRAIL_TEXTURED: bool = true;
    pub const DEFAULT_TRAIL_TEXTURED_MAP_MINI: bool = false;
    pub const DEFAULT_TRAIL_TEXTURED_MAP_WORLD: bool = true;
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
    pub const DEFAULT_EDGE_FEATHER_SCALE: Option<f32> = Some(1.0f32);

    pub const NONE_F32: f32 = f32::MIN;

    fn optional_f32(v: f32) -> Option<f32> {
        match v {
            Self::NONE_F32 => None,
            v => Some(v)
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                camera_source: None | Some(Self::DEFAULT_CAMERA_SOURCE),
                visible_space: None | Some(Self::DEFAULT_VISIBLE),
                visible_map_world: None | Some(Self::DEFAULT_VISIBLE_MAP), visible_map_mini: None | Some(Self::DEFAULT_VISIBLE_MAP),
                distance_max: None,
                distance_fade_intensity: None, player_overlap_threshold: None,
                edge_feather_scale: None | Self::DEFAULT_EDGE_FEATHER_SCALE,
                trail_alpha: None,
                poi_alpha: None,
                trail_textured_space: None | Some(Self::DEFAULT_TRAIL_TEXTURED),
                map_trail_textured_mini: None | Some(Self::DEFAULT_TRAIL_TEXTURED_MAP_MINI),
                map_trail_textured_world: None | Some(Self::DEFAULT_TRAIL_TEXTURED_MAP_WORLD),
                map_poi_alpha_mini: None, map_trail_alpha_mini: None,
                map_poi_alpha_world: None, map_trail_alpha_world: None,
                scale_poi_space: None, scale_poi_mini: None, scale_poi_world: None,
                scale_trail_space: None, scale_trail_mini: None, scale_trail_world: None,
                goggles,
            } if goggles.is_empty() =>
                true,
            _ => false,
        }
    }

    pub fn camera_source(&self) -> CameraSource {
        self.camera_source.unwrap_or(Self::DEFAULT_CAMERA_SOURCE)
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
    pub fn visible_map(&self, ctx: MapContext) -> bool {
        match ctx {
            MapContext::Global => self.visible_worldmap(),
            MapContext::Minimap => self.visible_minimap(),
        }
    }

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
        self.map_trail_textured_mini.unwrap_or(Self::DEFAULT_TRAIL_TEXTURED_MAP_MINI)
    }
    pub fn trail_textured_worldmap(&self) -> bool {
        self.map_trail_textured_world.unwrap_or(Self::DEFAULT_TRAIL_TEXTURED_MAP_WORLD)
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
            .map(SpaceSettings::optional_f32)
            .unwrap_or(Self::DEFAULT_EDGE_FEATHER_SCALE)
    }

    pub fn trail_alpha(&self) -> f32 {
        self.trail_alpha.unwrap_or(Self::DEFAULT_TRAIL_ALPHA)
    }
    pub fn poi_alpha(&self) -> f32 {
        self.trail_alpha.unwrap_or(Self::DEFAULT_POI_ALPHA)
    }

    pub fn trail_alpha_worldmap(&self) -> f32 {
        self.map_trail_alpha_world.unwrap_or(Self::DEFAULT_TRAIL_MAP_ALPHA)
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
    pub fn trail_alpha_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.trail_alpha_worldmap(),
            MapContext::Minimap => self.trail_alpha_minimap(),
        }
    }
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
    pub fn trail_scale_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.trail_scale_worldmap(),
            MapContext::Minimap => self.trail_scale_minimap(),
        }
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Deserialize, Serialize, VariantArray, IntoStaticStr)]
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
    pub const DEFAULT_EDGE_SCALE: Option<f32> = Some(0.5f32);

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                goggles_enabled: None | Some(Self::DEFAULT_ENABLED),
                obscured_alpha: None,
                edge_scale: None | Self::DEFAULT_EDGE_SCALE,
                map_depth_calibration,
            } if map_depth_calibration.is_empty() =>
                true,
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
            .map(SpaceSettings::optional_f32)
            .unwrap_or(Self::DEFAULT_EDGE_SCALE)
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
