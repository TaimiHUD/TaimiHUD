use {
    crate::{controller::Controller, settings::Settings},
    bitflags::bitflags,
    rustc_hash::FxHashSet,
    serde::{de::DeserializeSeed, Deserialize, Serialize},
    std::{collections::BTreeMap, fmt, num::NonZero, str::FromStr, sync::Arc},
    strum::{IntoStaticStr, VariantArray},
    taimi_hoard::flags::{BitFlagContainer, BitFlagDe, BitFlagSer},
};
#[cfg(feature = "paths")]
use {
    std::borrow::Cow,
    taimi_hoard::time::Timestamp,
    taimi_meta::{packs::VisibilityFlags, ui::MapContext},
    taimi_pack::attributes::{
        keys::{Guid, ShowHideAction},
        Festival,
        Festivals,
    },
    taimi_pack::category::id::{AsFullId, CategoryId, FullIdRef, IdCmpRelaxed},
};
#[cfg(not(feature = "paths"))]
type Timestamp = u64;
#[cfg(not(feature = "paths"))]
type Guid = String;
#[cfg(not(feature = "paths"))]
type CategoryId = String;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PathingSettings {
    #[serde(default, skip_serializing_if = "SpaceSettings::is_empty")]
    pub space: SpaceSettings,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub festival_filter: Arc<BTreeMap<String, FestivalPreference>>,
    #[serde(
        default = "TriggerKind::settings_default_auto",
        skip_serializing_if = "TriggerKind::settings_default_is_auto"
    )]
    pub trigger_allow_auto: TriggerKind,
    #[serde(
        default = "TriggerKind::settings_default_interact",
        skip_serializing_if = "TriggerKind::settings_default_is_interact"
    )]
    pub trigger_allow_interact: TriggerKind,
    #[serde(
        default = "TriggerKind::settings_default_enable",
        skip_serializing_if = "TriggerKind::settings_default_is_enable"
    )]
    pub trigger_enable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_simultaneous: Option<usize>,
}

impl PathingSettings {
    #[cfg(feature = "paths")]
    pub const DEFAULT_LOAD_SIMULTANEOUS: usize = 4;

    #[cfg(feature = "paths")]
    pub fn get_festival_preference(&self, festival: Festival) -> Option<FestivalPreference> {
        self.festival_filter.get(festival.as_str()).copied()
    }
    #[cfg(feature = "paths")]
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
    #[cfg(feature = "paths")]
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
    #[cfg(feature = "paths")]
    pub fn festival_filter_mut(&mut self) -> &mut BTreeMap<String, FestivalPreference> {
        Arc::make_mut(&mut self.festival_filter)
    }

    #[cfg(feature = "paths")]
    pub fn load_simultaneous(&self) -> usize {
        self.load_simultaneous.unwrap_or(Self::DEFAULT_LOAD_SIMULTANEOUS)
    }
    #[cfg(feature = "paths")]
    pub fn set_load_simultaneous(&mut self, v: usize) {
        self.load_simultaneous = Some(v);
    }
}
#[cfg(feature = "paths")]
impl Settings {
    pub fn pathing_state_update(&mut self, path: String, state: bool) {
        if self.disabled_paths.contains(&path) && state {
            self.disabled_paths_mut().remove(&path);
        } else if !state {
            self.disabled_paths_mut().insert(path);
        }
    }
}

impl Default for PathingSettings {
    fn default() -> Self {
        Self {
            space: Default::default(),
            festival_filter: Default::default(),
            trigger_enable: TriggerKind::settings_default_enable(),
            trigger_allow_auto: TriggerKind::settings_default_auto(),
            trigger_allow_interact: TriggerKind::settings_default_interact(),
            load_simultaneous: None,
        }
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
    pub distance_fade_range: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_overlap_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_overlap_poi: Option<bool>,
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
    pub poi_limit_size: Option<bool>,
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
    pub anim_trail_space: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_trail_mini: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_trail_world: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail_y_offset: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail_resolution: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail_width: Option<f32>,
}

impl SpaceSettings {
    pub const DEFAULT_CAMERA_SOURCE: CameraSource = CameraSource::MumbleLink;
    pub const DEFAULT_VISIBLE: bool = true;
    pub const DEFAULT_VISIBLE_MAP: bool = true;
    pub const DEFAULT_TRAIL_TEXTURED: bool = true;
    pub const DEFAULT_TRAIL_TEXTURED_MAP_MINI: bool = false;
    pub const DEFAULT_TRAIL_TEXTURED_MAP_WORLD: bool = true;
    pub const DEFAULT_TRAIL_Y_OFFSET: f32 = 0.001;
    pub const DEFAULT_TRAIL_RESOLUTION: f32 = 1.0 / 20.0;
    pub const DEFAULT_TRAIL_WIDTH: f32 = 1.016;
    pub const DEFAULT_DISTANCE_MAX: f32 = 700.0;
    pub const DEFAULT_POI_LIMIT_SIZE: bool = true;
    pub const DEFAULT_POI_ALPHA: f32 = 1.0;
    pub const DEFAULT_POI_SCALE: f32 = 1.0;
    pub const DEFAULT_POI_SCALE_MAP: f32 = 1.0;
    pub const DEFAULT_TRAIL_ALPHA: f32 = Self::DEFAULT_POI_ALPHA;
    pub const DEFAULT_TRAIL_SCALE: f32 = 1.0;
    pub const DEFAULT_TRAIL_ANIM: f32 = 1.0;
    pub const DEFAULT_TRAIL_SCALE_MAP: f32 = 5.0;
    pub const DEFAULT_POI_MAP_ALPHA: f32 = Self::DEFAULT_POI_ALPHA;
    pub const DEFAULT_TRAIL_MAP_ALPHA: f32 = Self::DEFAULT_TRAIL_ALPHA;
    pub const DEFAULT_DISTANCE_FADE_INTENSITY: f32 = 84.0;
    pub const DEFAULT_DISTANCE_FADE_RANGE: bool = true;
    pub const DEFAULT_PLAYER_OVERLAP_THRESHOLD: f32 = 38.0;
    pub const DEFAULT_PLAYER_OVERLAP_POI: bool = false;
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
                visible_space: None | Some(Self::DEFAULT_VISIBLE),
                visible_map_world: None | Some(Self::DEFAULT_VISIBLE_MAP),
                visible_map_mini: None | Some(Self::DEFAULT_VISIBLE_MAP),
                map_open: None | Some(Self::DEFAULT_MAP_OPEN),
                distance_max: None,
                distance_fade_intensity: None,
                distance_fade_range: None,
                player_overlap_threshold: None,
                player_overlap_poi: None,
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
                poi_limit_size: None,
                scale_poi_space: None,
                scale_poi_mini: None,
                scale_poi_world: None,
                scale_trail_space: None,
                scale_trail_mini: None,
                scale_trail_world: None,
                anim_trail_space: None,
                anim_trail_mini: None,
                anim_trail_world: None,
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

    pub fn visible_space(&self) -> bool {
        self.visible_space.unwrap_or(Self::DEFAULT_VISIBLE)
    }
    pub fn visible_minimap(&self) -> bool {
        self.visible_map_mini.unwrap_or(Self::DEFAULT_VISIBLE_MAP)
    }
    pub fn visible_worldmap(&self) -> bool {
        self.visible_map_world.unwrap_or(Self::DEFAULT_VISIBLE_MAP)
    }
    #[cfg(feature = "paths")]
    pub fn visible_map(&self, ctx: MapContext) -> bool {
        match ctx {
            MapContext::Global => self.visible_worldmap(),
            MapContext::Minimap => self.visible_minimap(),
        }
    }
    pub fn map_open(&self) -> bool {
        self.map_open.unwrap_or(Self::DEFAULT_MAP_OPEN)
    }

    #[cfg(feature = "paths")]
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
    pub fn distance_fade_range(&self) -> bool {
        self.distance_fade_range
            .unwrap_or(Self::DEFAULT_DISTANCE_FADE_RANGE)
    }
    pub fn player_overlap_threshold(&self) -> Option<f32> {
        self.player_overlap_threshold
            .map(Self::optional_f32)
            .unwrap_or(Some(Self::DEFAULT_PLAYER_OVERLAP_THRESHOLD))
    }
    pub fn player_overlap_poi(&self) -> bool {
        self.player_overlap_poi
            .unwrap_or(Self::DEFAULT_PLAYER_OVERLAP_POI)
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
    #[cfg(feature = "paths")]
    pub fn trail_alpha_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.trail_alpha_worldmap(),
            MapContext::Minimap => self.trail_alpha_minimap(),
        }
    }
    #[cfg(feature = "paths")]
    pub fn poi_alpha_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.poi_alpha_worldmap(),
            MapContext::Minimap => self.poi_alpha_minimap(),
        }
    }

    pub fn poi_limit_size(&self) -> bool {
        self.poi_limit_size.unwrap_or(Self::DEFAULT_POI_LIMIT_SIZE)
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
    #[cfg(feature = "paths")]
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
    #[cfg(feature = "paths")]
    pub fn trail_scale_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.trail_scale_worldmap(),
            MapContext::Minimap => self.trail_scale_minimap(),
        }
    }

    pub fn trail_anim_space(&self) -> f32 {
        self.anim_trail_space.unwrap_or(Self::DEFAULT_TRAIL_ANIM)
    }
    pub fn trail_anim_worldmap(&self) -> f32 {
        self.anim_trail_world.unwrap_or(self.trail_anim_space())
    }
    pub fn trail_anim_minimap(&self) -> f32 {
        self.anim_trail_mini.unwrap_or(self.trail_anim_worldmap())
    }
    #[cfg(feature = "paths")]
    pub fn trail_anim_map(&self, ctx: MapContext) -> f32 {
        match ctx {
            MapContext::Global => self.trail_anim_worldmap(),
            MapContext::Minimap => self.trail_anim_minimap(),
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
    pub arcrender_enabled: Option<bool>,

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
    pub const DEFAULT_ARCRENDER: bool = false;
    pub const DEFAULT_OBSCURED_ALPHA: f32 = 0.15;
    pub const DEFAULT_DEPTH_CALIBRATION: (f32, f32) = (1.0, 1.0);
    #[cfg(todo)]
    pub const DEFAULT_EDGE_SCALE: f32 = 0.5f32;
    pub const DEFAULT_EDGE_SCALE: f32 = SpaceSettings::NONE_F32;
    pub const DEFAULT_TRAIL_Y_OFFSET: f32 = 0.025;

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                arcrender_enabled: None | Some(Self::DEFAULT_ARCRENDER),
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
    pub fn arcrender_enabled(&self) -> bool {
        self.arcrender_enabled.unwrap_or(Self::DEFAULT_ARCRENDER)
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

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TriggerKind: u16 {
        const BEHAVIOUR = 0x0001;
        const COPY = 0x0002;
        const INFO = 0x0004;
        const RESET = 0x0008;
        const TOGGLE = 0x0010;
        const SHOW = 0x0020;
        const HIDE = 0x0040;
        const SCRIPT = 0x0080;
        const BOUNCE = 0x0100;
    }
}
impl TriggerKind {
    pub const fn flag_str(self) -> Option<&'static str> {
        Some(match self {
            Self::BEHAVIOUR => "trigger-behaviour",
            Self::COPY => "trigger-copy",
            Self::INFO => "trigger-info",
            Self::RESET => "trigger-reset",
            Self::TOGGLE => "trigger-toggle",
            Self::SHOW => "trigger-show",
            Self::HIDE => "trigger-hide",
            Self::SCRIPT => "trigger-script",
            Self::BOUNCE => "trigger-bounce",
            _ => return None,
        })
    }
}
impl fmt::Display for TriggerKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.flag_str() {
            Some(name) => f.write_str(name),
            None => write!(f, "{}", self.bits()),
        }
    }
}
impl FromStr for TriggerKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "trigger-behaviour" => Self::BEHAVIOUR,
            "trigger-copy" => Self::COPY,
            "trigger-info" => Self::INFO,
            "trigger-reset" => Self::RESET,
            "trigger-toggle" => Self::TOGGLE,
            "trigger-show" => Self::SHOW,
            "trigger-hide" => Self::HIDE,
            "trigger-script" => Self::SCRIPT,
            "trigger-bounce" => Self::BOUNCE,
            _ => anyhow::bail!("unsupported interaction trigger `{s}`"),
        })
    }
}
impl TriggerKind {
    pub const AUTO_TRIGGER_MASK: Self = Self::SETTINGS_DEFAULT_AUTO;
    /// [Self::SHOW] | [Self::HIDE] | [Self::TOGGLE]
    pub const CATEGORY_MASK: Self =
        Self::from_bits_retain(Self::SHOW.bits() | Self::HIDE.bits() | Self::TOGGLE.bits());
    pub const SETTINGS_GUI: Self =
        Self::from_bits_retain(Self::all().bits() & !(Self::SHOW.bits() | Self::HIDE.bits()));
    pub const SETTINGS_TOGGLE_SHOWHIDE: Self =
        Self::from_bits_retain(Self::SHOW.bits() | Self::HIDE.bits());
    pub const SETTINGS_DEFAULT_AUTO: Self = Self::from_bits_retain(
        Self::BEHAVIOUR.bits()
            | Self::INFO.bits()
            | Self::RESET.bits()
            | Self::TOGGLE.bits()
            | Self::SHOW.bits()
            | Self::HIDE.bits()
            | Self::BOUNCE.bits(),
    );
    pub const DISMISS: Self = Self::from_bits_retain(Self::BEHAVIOUR.bits() | Self::BOUNCE.bits());
    pub const fn settings_default_auto() -> Self {
        Self::SETTINGS_DEFAULT_AUTO
    }
    pub const SETTINGS_DEFAULT_INTERACT: Self = Self::from_bits_retain(
        Self::BEHAVIOUR.bits()
            | Self::COPY.bits()
            | Self::INFO.bits()
            | Self::RESET.bits()
            | Self::TOGGLE.bits()
            | Self::SHOW.bits()
            | Self::HIDE.bits()
            | Self::BOUNCE.bits(),
    );
    pub const fn settings_default_interact() -> Self {
        Self::SETTINGS_DEFAULT_INTERACT
    }
    pub const fn settings_default_is_auto(&self) -> bool {
        self.bits() == Self::settings_default_auto().bits()
    }
    pub const fn settings_default_is_interact(&self) -> bool {
        self.bits() == Self::settings_default_interact().bits()
    }
    pub const fn settings_default_enable() -> bool {
        true
    }
    pub const fn settings_default_is_enable(enable: &bool) -> bool {
        *enable == Self::settings_default_enable()
    }

    #[cfg(feature = "paths")]
    pub const fn show_hide_action(&self) -> Option<ShowHideAction> {
        match *self {
            Self::SHOW => Some(ShowHideAction::Show),
            Self::HIDE => Some(ShowHideAction::Hide),
            Self::TOGGLE => Some(ShowHideAction::Toggle),
            _ => None,
        }
    }
    pub fn show_hide_actions(self) -> impl Iterator<Item = (Self, ShowHideAction)> {
        (self & Self::CATEGORY_MASK)
            .into_iter()
            .filter_map(|t| t.show_hide_action().map(|a| (t, a)))
    }
}
impl<'de> Deserialize<'de> for TriggerKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BitFlagDe::new().deserialize(deserializer)
    }
}
impl Serialize for TriggerKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        BitFlagSer::<Self>::new_human(*self).serialize(serializer)
    }
}
impl BitFlagContainer for TriggerKind {
    type ClonedIter = <Self as IntoIterator>::IntoIter;
    type FromStrErr = <Self as FromStr>::Err;
    fn all() -> Self {
        Self::all()
    }
    fn empty() -> Self {
        Self::empty()
    }
    fn bit_name(&self) -> Option<&'static str> {
        self.flag_str()
    }
    fn iter(&self) -> Self::ClonedIter {
        self.clone().into_iter()
    }
    fn bits64(&self) -> u64 {
        self.bits() as u64
    }
    fn from_bits64(bits: u64) -> Result<Self, (Self, NonZero<u64>)> {
        let flags = Self::from_bits_truncate(bits as _);
        let rest = bits ^ flags.bits() as u64;
        match NonZero::new(rest) {
            Some(rest) => Err((flags, rest)),
            None => Ok(flags),
        }
    }
    fn try_from_str(s: &str) -> Result<Self, Self::FromStrErr> {
        Self::from_str(s)
    }
}

pub type HiddenGuids = BTreeMap<Guid, Timestamp>;
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PathingSave {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hidden_guid_expiry: Arc<HiddenGuids>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_account: BTreeMap<String, PathingAccountSave>,
    #[serde(default, skip_serializing_if = "PathingCategories::is_empty")]
    pub categories: PathingCategories,
}
impl PathingSave {
    pub fn is_empty(&self) -> bool {
        match self {
            Self { hidden_guid_expiry, .. } if !hidden_guid_expiry.is_empty() => false,
            Self { per_account, .. } if !Self::is_per_account_empty(per_account) => false,
            Self { categories, .. } if !categories.is_empty() => false,
            Self {
                categories: _,
                hidden_guid_expiry: _,
                per_account: _,
            } => true,
        }
    }

    pub(crate) fn is_empty_opt(save: &Option<Self>) -> bool {
        match save {
            None => true,
            Some(pathing) => pathing.is_empty(),
        }
    }
    pub(crate) fn is_per_account_empty(per_account: &BTreeMap<String, PathingAccountSave>) -> bool {
        per_account.values().all(|a| a.is_empty())
    }
}

#[cfg(feature = "paths-filter")]
impl PathingSave {
    pub fn hidden_guid_expiry_mut(&mut self) -> &mut BTreeMap<Guid, Timestamp> {
        Arc::make_mut(&mut self.hidden_guid_expiry)
    }
    pub fn hidden_guid_expire_at(&mut self, guid: Guid, expiry: Timestamp) {
        self.hidden_guid_expiry_mut().insert(guid, expiry);
    }
    pub fn hidden_guid_expire(&mut self, guid: &Guid) -> Option<Timestamp> {
        if self.hidden_guid_expiry.contains_key(guid) {
            self.hidden_guid_expiry_mut().remove(guid)
        } else {
            None
        }
    }
    pub fn hidden_guid_expiry_get(&self, guid: &Guid) -> Option<&Timestamp> {
        self.hidden_guid_expiry.get(guid)
    }
    /// TODO: initial search can get an index to continue retain from afterward
    pub fn hidden_guid_prune_older_than(&mut self, now: &Timestamp) -> bool {
        let dirty = self.hidden_guid_expiry.values().any(|expiry| expiry <= now);
        if dirty {
            self.hidden_guid_expiry_mut().retain(|_, expiry| &*expiry > now)
        }
        dirty
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PathingAccountSave {
    /// if api controller just keeps a cached response around, why bother?
    #[cfg(todo)]
    #[serde(default, skip_serializing_if = "PathingAchievementSave::is_empty")]
    pub achievements: Arc<PathingAchievementSave>,
}
impl PathingAccountSave {
    pub fn is_empty(&self) -> bool {
        match self {
            #[cfg(todo)]
            Self { achievements, .. } if !achievements.is_empty() => false,
            Self {
                #[cfg(todo)]
                    achievements: _,
            } => true,
        }
    }
}

/// TODO: per-mode toggles
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PathingCategories {
    pub toggles: FxHashSet<IdCmpRelaxed<CategoryId>>,
    #[cfg(todo)]
    pub deviations: FxHashMap<CategoryId, VisibilityFlags>,
}
impl PathingCategories {
    pub fn is_empty(&self) -> bool {
        match self {
            Self { toggles, .. } if !toggles.is_empty() => false,
            Self { toggles: _ } => true,
        }
    }
}
#[cfg(feature = "paths")]
impl PathingCategories {
    pub fn visibility_deviations_for<'a, 'r>(
        &'a self,
        root: &'r FullIdRef,
    ) -> impl Iterator<Item = (&'a CategoryId, VisibilityFlags)> + 'r
    where
        'a: 'r,
    {
        let root = IdCmpRelaxed::with_ref(root);
        #[cfg(todo)]
        let deviations = self.deviations.iter();
        self.toggles
            .iter()
            .filter(move |id| id.id_starts_with(root))
            .map(|id| (&id.id, VisibilityFlags::TOGGLE))
    }
    pub fn visibility_deviation(&self, id: &FullIdRef) -> VisibilityFlags {
        let id = IdCmpRelaxed::with_ref(id);
        #[cfg(todo)]
        if let Some(deviations) = self.deviations.get(id) {
            return *deviations
        }
        VisibilityFlags::visible(self.toggles.contains(id))
    }
    pub fn set_visibility_deviation<'i, I>(&mut self, id: I, deviation: VisibilityFlags)
    where
        I: Into<Cow<'i, FullIdRef>>,
    {
        let id = id.into();
        #[cfg(todo)]
        if !dev.intersects(!VisibilityFlags::TOGGLE) {
            self.deviations.remove(&id);
        }
        match deviation {
            VisibilityFlags::TOGGLE => {
                self.toggles.insert(id.into_owned().into());
            },
            _ => {
                self.toggles.remove(IdCmpRelaxed::with_ref(&*id));
                #[cfg(todo)]
                if !deviation.is_empty() {
                    self.deviations.insert(id.into_owned().into(), deviation);
                }
            },
        }
    }
}
