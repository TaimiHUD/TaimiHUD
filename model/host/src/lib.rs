use core::num::NonZero;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(all(todo, feature = "serde"), derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct AddonHostType {
    pub id: NonZero<u8>,
}
#[allow(non_upper_case_globals)]
impl AddonHostType {
    pub const REPR_ARCDPS: u8 = 1;
    pub const ArcDPS: Self = Self::expect_id(Self::REPR_ARCDPS);

    pub const REPR_NEXUS: u8 = 2;
    pub const Nexus: Self = Self::expect_id(Self::REPR_NEXUS);

    pub const REPR_GW2LOAD: u8 = 3;
    pub const Gw2Load: Self = Self::expect_id(Self::REPR_GW2LOAD);

    pub const REPR_ADDONLOADER: u8 = 4;
    pub const AddonLoader: Self = Self::expect_id(Self::REPR_ADDONLOADER);

    pub const REPR_ARCLOADER: u8 = 5;
    pub const Arcloader: Self = Self::expect_id(Self::REPR_ARCLOADER);

    pub const REPR_INJECTED: u8 = 6;
    pub const Injected: Self = Self::expect_id(Self::REPR_INJECTED);

    pub const REPR_STANDALONE: u8 = 7;
    pub const Standalone: Self = Self::expect_id(Self::REPR_STANDALONE);

    pub const REPR_SIM: u8 = 8;
    pub const Sim: Self = Self::expect_id(Self::REPR_SIM);

    pub const REPR_EXTERNAL_OVERLAY: u8 = 9;
    pub const External: Self = Self::expect_id(Self::REPR_EXTERNAL_OVERLAY);

    pub const REPR_BLISH_HUD: u8 = 10;
    pub const BlishHUD: Self = Self::expect_id(Self::REPR_BLISH_HUD);

    pub const REPR_UNKNOWN: u8 = 0xfd;
    pub const Unknown: Self = Self::expect_id(Self::REPR_UNKNOWN);

    pub const REPR_ANY: u8 = 0xfe;
    pub const Any: Self = Self::expect_id(Self::REPR_ANY);
    #[deprecated]
    pub const All: Self = Self::Any;

    pub const None: Option<Self> = None;
    /// kinda arbitrary...
    pub const KNOWN: &'static [Self] = &[
        Self::ArcDPS,
        Self::Nexus,
        Self::Gw2Load,
        #[cfg(todo)]
        Self::Arcloader,
        #[cfg(todo)]
        Self::Injected,
        #[cfg(debug_assertions)]
        Self::Sim,
        #[cfg(todo)]
        Self::BlishHUD,
    ];

    #[inline(always)]
    pub const fn new(id: NonZero<u8>) -> Self {
        Self { id }
    }
    #[inline]
    pub const fn from_repr(id: u8) -> Option<Self> {
        match NonZero::new(id) {
            Some(id) => Some(Self::new(id)),
            None => None,
        }
    }
    #[inline(always)]
    pub const fn expect_id(id: u8) -> Self {
        match Self::from_repr(id) {
            Some(id) => id,
            None => panic!("AddonHostType(0)"),
        }
    }

    #[inline(always)]
    pub const fn to_repr(self) -> u8 {
        self.id.get()
    }
}

impl Default for AddonHostType {
    #[inline]
    fn default() -> Self {
        Self::Unknown
    }
}
impl From<AddonHostType> for NonZero<u8> {
    #[inline]
    fn from(ty: AddonHostType) -> Self {
        ty.id
    }
}
impl From<NonZero<u8>> for AddonHostType {
    #[inline]
    fn from(id: NonZero<u8>) -> Self {
        Self::new(id)
    }
}
impl From<AddonHostType> for u8 {
    #[inline]
    fn from(ty: AddonHostType) -> Self {
        ty.to_repr()
    }
}
