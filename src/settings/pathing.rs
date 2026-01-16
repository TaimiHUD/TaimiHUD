use {
    crate::{controller::Controller, settings::Settings},
    bitflags::bitflags,
    serde::{Deserialize, Serialize},
    std::{collections::BTreeMap, fmt, sync::Arc, time},
    strum::{IntoStaticStr, VariantArray},
};
#[cfg(feature = "space")]
use {
    taimi_meta::ui::MapContext,
    taimi_pack::attributes::{keys::Guid, Festival, Festivals},
    taimi_hoard::time::Timestamp,
};
#[cfg(not(feature = "space"))]
type Timestamp = u64;
#[cfg(not(feature = "space"))]
type Guid = String;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_simultaneous: Option<usize>,

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
    #[cfg(feature = "paths")]
    pub const DEFAULT_LOAD_SIMULTANEOUS: usize = 4;

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

    #[cfg(feature = "paths")]
    pub fn load_simultaneous(&self) -> usize {
        self.load_simultaneous.unwrap_or(Self::DEFAULT_LOAD_SIMULTANEOUS)
    }
    #[cfg(feature = "paths")]
    pub fn set_load_simultaneous(&mut self, v: usize) {
        self.load_simultaneous = Some(v);
    }
}
#[cfg(feature = "space")]
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
            trigger_allow_auto: TriggerKind::SETTINGS_DEFAULT_AUTO,
            trigger_allow_interact: TriggerKind::SETTINGS_DEFAULT_INTERACT,
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
impl serde::Serialize for TriggerKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
    }
}
impl<'de> serde::Deserialize<'de> for TriggerKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u16::deserialize(deserializer).map(Self::from_bits_retain)
    }
}
impl TriggerKind {
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
}

pub type HiddenGuids = BTreeMap<Guid, Timestamp>;
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PathingSave {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hidden_guid_expiry: Arc<HiddenGuids>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_account: BTreeMap<String, PathingAccountSave>,
}
impl PathingSave {
    pub fn is_empty(&self) -> bool {
        match self {
            Self { hidden_guid_expiry, .. } if !hidden_guid_expiry.is_empty() => false,
            Self { per_account, .. } if !Self::is_per_account_empty(per_account) => false,
            Self { hidden_guid_expiry: _, per_account: _ } => true,
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
#[cfg(feature = "space")]
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
    #[cfg(deleteme)]
    pub fn hidden_guid_expiry(&self, guid: &Guid) -> Option<time::SystemTime> {
        self.hidden_guid_expiry
            .get(guid)
            .and_then(|&expiry| time::UNIX_EPOCH.checked_add(time::Duration::from_secs(expiry)))
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
