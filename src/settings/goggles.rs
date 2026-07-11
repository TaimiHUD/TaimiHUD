#[cfg(feature = "paths")]
use taimi_meta::map::MapProjection;
use {
    crate::settings::pathing::SpaceSettings,
    bitflags::bitflags,
    serde::{de::DeserializeSeed, Deserialize, Serialize},
    std::{collections::BTreeMap, fmt, num::NonZero, str::FromStr, sync::Arc},
    taimi_hoard::flags::{BitFlagContainer, BitFlagDe, BitFlagSer},
};
#[cfg(feature = "goggles")]
use {
    glamour::Angle,
    std::collections::btree_map,
    taimi_meta::{coords::MapLocalScale, map::MapProjectionDepth},
};

impl SpaceSettings {
    #[cfg(feature = "goggles")]
    pub fn obscured_distance(&self) -> f32 {
        let max = self.distance_max();
        (self.goggles.obscured_distance() * max).max(GogglesSettings::MIN_OBSCURED_DISTANCE.min(max))
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct GogglesSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcrender_enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goggles_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enables: Option<GogglesEnables>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_enabled: Option<bool>,

    /// X-ray opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obscured_alpha: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_scale: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obscured_distance: Option<f32>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map_depth_calibration: Arc<BTreeMap<u32, (f32, f32)>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map_proj_seen: Arc<BTreeMap<u32, GogglesProjection>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_shadowboxing: Option<bool>,
}

impl GogglesSettings {
    pub const DEFAULT_ENABLED: bool = false;
    pub const DEFAULT_ENABLED_PROJECT: bool = false;
    pub const DEFAULT_ARCRENDER: bool = false;
    pub const DEFAULT_OBSCURED_ALPHA: f32 = 0.10;
    pub const DEFAULT_OBSCURED_DISTANCE: f32 = 0.175;
    pub const MIN_OBSCURED_DISTANCE: f32 = 12.0;
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
                enables: None | Some(GogglesEnables::DEFAULT),
                project_enabled: None | Some(Self::DEFAULT_ENABLED_PROJECT),
                project_shadowboxing: None,
                obscured_alpha: None,
                obscured_distance: None,
                edge_scale: None | Some(Self::DEFAULT_EDGE_SCALE),
                map_depth_calibration,
                map_proj_seen,
            } if map_depth_calibration.is_empty() && map_proj_seen.is_empty() => true,
            _ => false,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enables()
            .contains(GogglesEnables::LENS_ENABLE | GogglesEnables::ENABLE)
    }
    #[inline]
    pub fn enables(&self) -> GogglesEnables {
        let mut enables = self.enables.unwrap_or(GogglesEnables::DEFAULT);
        if self.enables.is_none() {
            if let Some(true) = self.arcrender_enabled {
                enables.insert(GogglesEnables::ARCRENDER_ENABLE);
            }
            if let Some(true) = self.goggles_enabled {
                enables.insert(GogglesEnables::ENABLE | GogglesEnables::LENS_ENABLE);
            }
        }
        enables
    }
    pub fn reset_enables(&mut self) {
        #[cfg(todo)]
        {
            self.arcrender_enabled = None;
        }
        let enables = match self.enables {
            Some(enables) => enables & GogglesEnables::STICKY,
            None => return,
        };
        self.enables = match enables.is_empty() {
            true => None,
            false => Some(enables),
        };
    }
    pub fn set_enables(&mut self, enables: GogglesEnables, mask: GogglesEnables) {
        let sticky = self.enables() & !mask;
        let enables = *self.enables.insert(sticky | (enables & mask));
        if mask.contains(GogglesEnables::ARCRENDER_ENABLE) {
            self.arcrender_enabled = Some(enables.contains(GogglesEnables::ARCRENDER_ENABLE));
        }
        if mask.contains(GogglesEnables::LENS_ENABLE) {
            self.goggles_enabled =
                Some(enables.contains(GogglesEnables::LENS_ENABLE | GogglesEnables::ENABLE));
        }
    }
    pub fn arcrender_enabled(&self) -> bool {
        self.enables().contains(GogglesEnables::ARCRENDER_ENABLE)
    }

    pub fn obscured_alpha(&self) -> f32 {
        self.obscured_alpha.unwrap_or(Self::DEFAULT_OBSCURED_ALPHA)
    }
    pub fn obscured_distance(&self) -> f32 {
        self.obscured_distance.unwrap_or(Self::DEFAULT_OBSCURED_DISTANCE)
    }

    pub fn edge_scale(&self) -> Option<f32> {
        self.edge_scale
            .or(Some(Self::DEFAULT_EDGE_SCALE))
            .and_then(SpaceSettings::optional_f32)
    }

    #[cfg(feature = "goggles")]
    pub fn get_map_depth_setting(&self, map_id: u32) -> Option<GogglesMapDepth> {
        self.map_depth_calibration
            .get(&map_id)
            .copied()
            .map(GogglesMapDepth::with_tuple)
    }
    #[cfg(feature = "goggles")]
    pub fn get_map_depth_calibration(&self, map_id: u32) -> Option<GogglesMapDepth> {
        let get_seen = || {
            self.map_proj_seen
                .get(&map_id)
                .map(MapProjectionDepth::from)
                .map(GogglesMapDepth::from)
        };
        self.get_map_depth_setting(map_id).or_else(get_seen)
    }
    #[cfg(feature = "goggles")]
    pub fn set_map_proj_seen_depth(&mut self, map_id: u32, z: MapProjectionDepth) -> bool {
        if self.map_proj_seen.get(&map_id).map(|proj| &proj.depth) == Some(&z) {
            return false
        }
        match self.map_proj_seen_mut().entry(map_id) {
            btree_map::Entry::Vacant(e) => {
                e.insert(z.into());
                true
            },
            btree_map::Entry::Occupied(e) => {
                let e = e.into_mut();
                let delta = (e.depth.farz - z.farz).abs();
                e.depth = z;
                delta > 1e-1f32
            },
        }
    }

    pub fn map_depth_calibration_mut(&mut self) -> &mut BTreeMap<u32, (f32, f32)> {
        Arc::make_mut(&mut self.map_depth_calibration)
    }
    pub fn map_proj_seen_mut(&mut self) -> &mut BTreeMap<u32, GogglesProjection> {
        Arc::make_mut(&mut self.map_proj_seen)
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct GogglesEnables: u32 {
        const ENABLE = 0x0001;
        const LENS_ENABLE = 0x0002;
        const CAMERA_ENABLE = 0x0004;
        const PROJECT_ENABLE = 0x0008;
        const ARCRENDER_ENABLE = 0x0040;

        const CAMERA_PERSPECTIVE = 0x1000;
        const CAMERA_DIR = 0x2000;
        const CAMERA_CUTSCENE = 0x4000;

        const PROJECT_MAP = 0x0002_0000;
        const PROJECT_REFLECTIONS = 0x0004_0000;
        const PROJECT_SHADOWBOXING = 0x0008_0000;
        const PROJECT_COMPAT_FLUSH = 0x0010_0000;
        const PROJECT_COMPAT_METHOD = 0x0020_0000;
    }
}
impl GogglesEnables {
    pub const DEFAULT: Self = Self::from_bits_retain(
        Self::CAMERA_ENABLE.bits()
            | Self::DEFAULTS_CAMERA.bits()
            | Self::DEFAULTS_PROJECT.bits()
            | Self::DEFAULTS_LENS.bits(),
    );
    pub const DEFAULTS_CAMERA: Self = Self::from_bits_retain(
        Self::CAMERA_PERSPECTIVE.bits() | Self::CAMERA_DIR.bits() | Self::CAMERA_CUTSCENE.bits(),
    );
    pub const DEFAULTS_PROJECT: Self = Self::from_bits_retain(Self::PROJECT_MAP.bits());
    pub const DEFAULTS_LENS: Self = Self::empty();
    pub const ENABLES: Self = Self::from_bits_retain(Self::ENABLE.bits() | Self::FEATURE_ENABLES.bits());
    pub const SUPPORTED_FEATURES: Self = Self::from_bits_retain({
        let flags = Self::empty().bits();
        #[cfg(feature = "goggles")]
        let flags = flags | Self::LENS_ENABLE.bits();
        #[cfg(feature = "goggles2-project")]
        let flags = flags | Self::PROJECT_ENABLE.bits();
        #[cfg(feature = "goggles2-camera")]
        let flags = flags | Self::CAMERA_ENABLE.bits();
        flags
    });
    pub const FEATURE_ENABLES: Self = Self::from_bits_retain(
        Self::LENS_ENABLE.bits() | Self::PROJECT_ENABLE.bits() | Self::CAMERA_ENABLE.bits(),
    );
    pub const OPTIONS_CAMERA: Self = Self::from_bits_retain(
        Self::CAMERA_PERSPECTIVE.bits() | Self::CAMERA_DIR.bits() | Self::CAMERA_CUTSCENE.bits(),
    );
    pub const OPTIONS_PROJECT: Self = Self::from_bits_retain(
        Self::PROJECT_MAP.bits()
            | Self::PROJECT_REFLECTIONS.bits()
            | Self::PROJECT_SHADOWBOXING.bits()
            | Self::OPTIONS_PROJECT_COMPAT.bits(),
    );
    pub const OPTIONS_PROJECT_COMPAT: Self =
        Self::from_bits_retain(Self::PROJECT_COMPAT_FLUSH.bits() | Self::PROJECT_COMPAT_METHOD.bits());
    pub const OPTIONS_LENS: Self = Self::empty();
    pub const OPTIONS_ARCRENDER: Self = Self::empty();
    pub const OPTIONS_MASK: Self = Self::from_bits_retain(
        Self::OPTIONS_LENS.bits() | Self::OPTIONS_PROJECT.bits() | Self::OPTIONS_CAMERA.bits(), //| Self::OPTIONS_ARCRENDER.bits()
    );
    #[cfg(feature = "goggles")]
    pub const UI_ENABLES: &[Self] = &[
        Self::ENABLE,
        Self::LENS_ENABLE,
        #[cfg(feature = "goggles2-camera")]
        Self::CAMERA_ENABLE,
        #[cfg(feature = "goggles2-project")]
        Self::PROJECT_ENABLE,
    ];

    const STICKY: Self =
        Self::from_bits_retain(!Self::all().bits() | !Self::ENABLES.bits() | Self::ARCRENDER_ENABLE.bits());

    pub const fn flag_str(self) -> Option<&'static str> {
        Some(match self {
            Self::ENABLE => "enable",
            Self::LENS_ENABLE => "lens-enable",
            Self::CAMERA_ENABLE => "camera-enable",
            Self::CAMERA_PERSPECTIVE => "camera-perspective",
            Self::CAMERA_CUTSCENE => "camera-cutscene",
            Self::CAMERA_DIR => "camera-dir",
            Self::PROJECT_ENABLE => "project-enable",
            Self::PROJECT_REFLECTIONS => "project-reflection",
            Self::PROJECT_SHADOWBOXING => "project-shadowboxing",
            Self::PROJECT_MAP => "project-map",
            Self::PROJECT_COMPAT_FLUSH => "project-compat-flush",
            Self::PROJECT_COMPAT_METHOD => "project-compat-method",
            Self::ARCRENDER_ENABLE => "arcrender-enable",
            _ => return None,
        })
    }

    #[inline(always)]
    pub const fn r#if(self, cond: bool) -> Self {
        match cond {
            true => self,
            false => Self::empty(),
        }
    }
    #[inline(always)]
    pub fn feature_mask(self) -> Self {
        Self::FEATURE_ENABLES.r#if(self.contains(Self::ENABLE))
    }
    pub fn options_mask(self) -> Self {
        Self::OPTIONS_LENS.r#if(self.contains(Self::LENS_ENABLE))
            | Self::OPTIONS_CAMERA.r#if(self.contains(Self::CAMERA_ENABLE))
            | Self::OPTIONS_PROJECT.r#if(self.contains(Self::PROJECT_ENABLE))
    }

    pub fn omit_unavailable(self) -> Self {
        let mask = Self::ENABLE | self.feature_mask() | self.options_mask();
        self & mask
    }
}
impl fmt::Display for GogglesEnables {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.flag_str() {
            Some(name) => f.write_str(name),
            None => write!(f, "{}", self.bits()),
        }
    }
}
impl FromStr for GogglesEnables {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "enable" => Self::ENABLE,
            "lens-enable" => Self::LENS_ENABLE,
            "camera-enable" => Self::CAMERA_ENABLE,
            "camera-perspective" => Self::CAMERA_PERSPECTIVE,
            "camera-cutscene" => Self::CAMERA_CUTSCENE,
            "camera-dir" => Self::CAMERA_DIR,
            "project-enable" => Self::PROJECT_ENABLE,
            "project-reflection" => Self::PROJECT_REFLECTIONS,
            "project-shadowboxing" => Self::PROJECT_SHADOWBOXING,
            "project-map" => Self::PROJECT_MAP,
            "project-compat-flush" => Self::PROJECT_COMPAT_FLUSH,
            "project-compat-method" => Self::PROJECT_COMPAT_METHOD,
            "arcrender-enable" => Self::ARCRENDER_ENABLE,
            _ => anyhow::bail!("unsupported goggles enable `{s}`"),
        })
    }
}
impl<'de> Deserialize<'de> for GogglesEnables {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BitFlagDe::new().deserialize(deserializer)
    }
}
/// TODO: human-readable once this is more stable or partly released
impl Serialize for GogglesEnables {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match () {
            _ => BitFlagSer::<Self>::new_bits(*self).serialize(serializer),
            #[cfg(todo)]
            _ => BitFlagSer::<Self>::new_human(*self).serialize(serializer),
        }
    }
}
impl BitFlagContainer for GogglesEnables {
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

#[cfg(feature = "goggles")]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GogglesMapDepth {
    pub value: (f32, f32),
}
#[cfg(feature = "goggles")]
impl GogglesMapDepth {
    #[inline]
    pub const fn with_tuple(value: (f32, f32)) -> Self {
        Self { value }
    }

    const VALUE_ZERO: u32 = 0.0f32.to_bits();
    const VALUE_ONE: u32 = 1.0f32.to_bits();
    pub fn near_value(&self) -> Option<f32> {
        match self.value.0.to_bits() {
            Self::VALUE_ZERO | Self::VALUE_ONE => None,
            _ => Some(self.value.0),
        }
    }
    pub fn far_value(&self) -> Option<f32> {
        match self.value.1.to_bits() {
            Self::VALUE_ZERO | Self::VALUE_ONE => None,
            _ => Some(self.value.1),
        }
    }

    pub fn as_v2_preset(&self) -> Option<MapProjectionDepth> {
        match (self.far_value(), self.near_value()) {
            (Some(far), None) if far < Self::V2_FARZ_MAX => Some(MapProjectionDepth::with_farz(far)),
            _ => None,
        }
    }
    pub const V2_FARZ_MAX: f32 = 130.0f32;
    pub const V2_FARZ_SLIDER_START: f32 = 6.0f32;
    pub const V2_FARZ_SLIDER_END: f32 = 20.0f32;

    const V1_DEPTH_NEAR_M: f32 = 0.65616665;
    const V1_DEPTH_FAR_M: f32 = 1524.0030867f32;
    const V1_DEPTH_MULT: f32 = Self::V1_DEPTH_FAR_M / Self::V1_DEPTH_NEAR_M;
    /// this is *heavily* skewed by FoV, and will make little sense if camera settings change
    #[cfg(todo)]
    pub fn reinterpret_v1_as_v2(&self, fovy: Angle, scale: &MapLocalScale) -> Option<MapProjectionDepth> {
        let near_m = self.near_value()? * Self::V1_DEPTH_NEAR_M;
        let far = match self.far_value() {
            None => None,
            Some(f) => {
                let far_m = f * Self::V1_DEPTH_FAR_M;
                return None
            },
        };
        if self.far_value().is_some() {
            return None
        }
        let near_m = match near_m {
            n if n / fovy > MapProjectionDepth::NEAR_MAX_M => {},
            n => n,
        };
        #[cfg(todo)]
        let m_per_in = scale.scale_length();
        let m_per_in = MapLocalScale::METRES_PER_INCH;
        let far = near_m * MapProjectionDepth::NEAR_FACTOR * fovy;
        let far_bucket_scale = MapProjectionDepth::FAR_FACTOR_IN.recip() * 2.0f32 * m_per_in;
        let snapped = (far * far_bucket_scale).round();
        if (snapped % 2.0) == 1.0 {
            // closer to the center than a real "notch", don't bother with the guess
            return None
        }
        Some(MapProjectionDepth::with_farz(snapped * 0.5f32))
    }
    pub fn reinterpret_v1_as_v2(&self, _fovy: Angle, _scale: &MapLocalScale) -> Option<MapProjectionDepth> {
        None
    }
}
#[cfg(feature = "goggles")]
impl From<MapProjectionDepth> for GogglesMapDepth {
    fn from(v: MapProjectionDepth) -> Self {
        Self::with_tuple((0.0, v.farz))
    }
}
#[cfg(feature = "goggles")]
impl From<GogglesMapDepth> for (f32, f32) {
    fn from(v: GogglesMapDepth) -> Self {
        v.value
    }
}

#[cfg(feature = "paths")]
pub type GogglesProjection = MapProjection;
#[cfg(not(feature = "paths"))]
pub type GogglesProjection = serde_json::Value;
