use {anyhow::anyhow, core::mem};

pub mod gameplay;
mod minimap;
#[cfg(feature = "taimi_mumblelink")]
pub mod mumblelink;
mod worldmap;

pub use self::{
    gameplay::GameplayState,
    minimap::{CompassTransform, MinimapPlacement, MinimapState},
    worldmap::{MapCalibration, MapOpen, MapState, MapUnit, UiMap},
};

#[derive(Debug, Default, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "u32", try_from = "u32"))]
#[repr(u32)]
pub enum UiSize {
    #[default]
    /// 80%
    Small = 0,
    /// 90%
    Normal = 1,
    /// 100%
    Large = 2,
    /// 110%
    Larger = 3,
}

impl UiSize {
    pub const MIN: Self = Self::Small;
    pub const MAX: Self = Self::Larger;
    pub const ZERO: Self = Self::Small;

    pub const SCALE_FACTORS: [f32; 4] = [0.9 * 0.9, 0.9, 1.0, 1.1];
    #[cfg(todo)]
    pub const SCALE_FACTORS: [f32; 4] = {
        let large = 1.0 / 1.1;
        let normal = large * 0.9;
        let small = normal * 0.9; //let small = 0.81 / 1.1;
        [small, normal, large, 1.0]
    };

    pub const fn from_repr(value: u32) -> Option<Self> {
        match value {
            0..=3 => Some(unsafe { Self::from_repr_unchecked(value) }),
            _ => None,
        }
    }

    pub const unsafe fn from_repr_unchecked(value: u32) -> Self {
        mem::transmute(value)
    }

    pub const fn repr(self) -> u32 {
        self as _
    }

    /// Relative to Normal as the 1.0 reference point
    ///
    /// Ranges from about 0.9 to 1.2
    pub const fn normal_scale(self) -> f32 {
        const SMALL: f32 = UiSize::Small.scale_amount() / UiSize::Normal.scale_amount();
        const LARGE: f32 = UiSize::Large.scale_amount() / UiSize::Normal.scale_amount();
        const LARGER: f32 = UiSize::Larger.scale_amount() / UiSize::Normal.scale_amount();
        match self {
            Self::Larger => LARGER,
            Self::Large => LARGE,
            Self::Normal => 1.0,
            Self::Small => SMALL,
        }
    }

    /// Relative to Large as the 1.0 reference point
    ///
    /// Index into [Self::SCALE_FACTORS]
    /// Ranges from 0.81 to 1.1
    pub const fn scale_amount(self) -> f32 {
        Self::SCALE_FACTORS[self.repr() as usize]
    }

    /// Relative to Small as the 1.0 reference point
    ///
    /// Ranges from 1.0 to about 1.35
    pub const fn scale_growth(self) -> f32 {
        Self::Small.scale_amount() / self.scale_amount()
    }

    /// Relative to Large as the 1.0 reference point
    ///
    /// See also [self.scale_amount()]
    pub const fn blish_scale_ratio(self) -> f32 {
        match self {
            Self::Small => 0.81,
            Self::Normal => 0.897,
            Self::Large => 1.0,
            Self::Larger => 1.103,
        }
    }
}

impl From<UiSize> for u32 {
    fn from(size: UiSize) -> Self {
        size.repr()
    }
}

impl TryFrom<u32> for UiSize {
    type Error = anyhow::Error;

    fn try_from(size: u32) -> Result<Self, Self::Error> {
        Self::from_repr(size).ok_or_else(|| {
            anyhow!(
                "known UI sizes range from {} to {}",
                Self::MIN.repr(),
                Self::MAX.repr()
            )
        })
    }
}

/// rounds to intended degree value (nearest ~0.1° - please see [realign_fov])
pub fn realign_fov_to_degree(fov_y: f32) -> f32 {
    const PREC: f32 = 10.0;
    const OFFSET: f32 = 50.0;
    const UNOFFSET: f32 = OFFSET * PREC;
    let deg = ((fov_y.to_degrees() - OFFSET) * PREC).round_ties_even();
    (deg + UNOFFSET) / PREC
}

/// Mumble link identity payload contains vertical FoV
/// rounded to fewer significant digits than we'd like
///
/// Clamp it to degrees and recompute radians to match the real underlying value.
///
/// Examples: 0.855(49°), 0.873(50°), 0.890(51°), 0.908(52°)
pub fn realign_fov(fov_y: f32) -> f32 {
    realign_fov_to_degree(fov_y).to_radians()
}
#[test]
fn mumble_identity_fov() {
    fn assert_mumble_fov(fov_y: f32, deg: f32) {
        let fixed_deg = realign_fov_to_degree(fov_y);
        let fixed = fixed_deg.to_radians();
        let fixed_deg = fixed.to_degrees();
        let exp_rad = deg.to_radians();
        let prev = (fov_y - exp_rad).abs();
        let inacc = (fixed - exp_rad).abs();
        let improvement = (prev - inacc).to_degrees();
        let fov_y_deg = fov_y.to_degrees();
        println!("fov.y={fov_y:?}({fov_y_deg:?}°) -> {fixed:?}({fixed_deg}°), expect={deg}° improvement={improvement:?}°");
        if improvement <= 0.0 {
            assert_eq!(fixed_deg, deg);
            unreachable!();
        }
    }
    assert_mumble_fov(0.855, 49.0);
    assert_mumble_fov(0.873, 50.0);
    assert_mumble_fov(0.890, 51.0);
    assert_mumble_fov(0.908, 52.0);
    assert_mumble_fov(0.909, 52.1);
    assert_mumble_fov(1.222, 70.0);
    assert_mumble_fov(0.436, 25.0);
}

bitflags::bitflags! {
    #[derive(Debug, Default, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
    pub struct UiState: u32 {
        const MAP_OPEN = 0x01;
        const COMPASS_TOP_RIGHT = 0x02;
        const COMPASS_ROTATION = 0x04;
        const WINDOW_FOCUS = 0x08;
        const COMPETITIVE_MODE = 0x10;
        const TEXT_INPUT = 0x20;
        const COMBAT = 0x40;
    }
}

#[allow(non_upper_case_globals)]
impl UiState {
    pub const MapOpen: Self = Self::MAP_OPEN;
    pub const CompassTopRight: Self = Self::COMPASS_TOP_RIGHT;
    pub const CompassRotation: Self = Self::COMPASS_ROTATION;
    pub const Focused: Self = Self::WINDOW_FOCUS;
    pub const Competitive: Self = Self::COMPETITIVE_MODE;
    pub const TextInput: Self = Self::TEXT_INPUT;
    pub const InCombat: Self = Self::COMBAT;
}

impl From<u32> for UiState {
    fn from(state: u32) -> Self {
        Self::from_bits_retain(state)
    }
}

#[doc(alias = "CurrentPerspective")]
#[derive(Debug, Default, PartialOrd, Ord, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MapContext {
    #[default]
    Minimap,
    /// [UiState::MapOpen]
    Global,
}

impl MapContext {
    pub const DEFAULT: Self = Self::Minimap;

    pub fn ui_flag(self) -> UiState {
        match self {
            MapContext::Global => UiState::MapOpen,
            MapContext::Minimap => UiState::empty(),
        }
    }
}

impl From<UiState> for MapContext {
    fn from(ui_state: UiState) -> Self {
        match ui_state.contains(UiState::MapOpen) {
            true => Self::Global,
            false => Self::Minimap,
        }
    }
}

impl From<MapContext> for UiState {
    fn from(ctx: MapContext) -> Self {
        ctx.ui_flag()
    }
}

#[derive(Copy, Clone)]
pub enum LocalContext {
    World,
    Map(MapContext),
}

impl LocalContext {
    pub const MAP: Self = Self::GLOBAL;
    pub const GLOBAL: Self = Self::Map(MapContext::Global);
    pub const MINIMAP: Self = Self::Map(MapContext::Minimap);

    pub const fn as_map(self) -> Option<MapContext> {
        match self {
            LocalContext::World => None,
            LocalContext::Map(map) => Some(map),
        }
    }

    #[inline]
    pub const fn is_map(self) -> bool {
        matches!(self, LocalContext::Map(..))
    }

    pub fn ui_flag(self) -> UiState {
        self.as_map().map(MapContext::ui_flag).unwrap_or_default()
    }
}

impl From<MapContext> for LocalContext {
    fn from(map: MapContext) -> Self {
        Self::Map(map)
    }
}

impl From<Option<MapContext>> for LocalContext {
    fn from(value: Option<MapContext>) -> Self {
        match value {
            None => Self::World,
            Some(map) => Self::Map(map),
        }
    }
}

impl From<LocalContext> for Option<MapContext> {
    fn from(value: LocalContext) -> Self {
        value.as_map()
    }
}
