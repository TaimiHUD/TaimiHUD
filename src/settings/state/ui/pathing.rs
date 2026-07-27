use {
    bitflags::bitflags,
    core::{num::NonZero, str::FromStr},
    serde::{
        de::{self, DeserializeSeed, Deserializer},
        ser,
    },
    taimi_hoard::flags::{BitFlagContainer, BitFlagDe, BitFlagSer},
};

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PathingFilterFlags: u8 {
        const Enabled = 1;
        const Disabled = 1 << 1;
        const CurrentMap = 1 << 2;
        /// previously IgnoreRoot
        const Unassigned3 = 1 << 3;
        /// previously IgnoreLeaves
        const Unassigned4 = 1 << 4;
        /// previously IgnoreBranches
        const Unassigned5 = 1 << 5;
        const ShowHidden = 1 << 6;
    }
}

impl PathingFilterFlags {
    pub const DEFAULT: Self = Self::from_bits_retain(Self::Enabled.bits() | Self::Disabled.bits());
    pub const USER: Self = Self::from_bits_retain(
        Self::all().bits()
            & !(Self::Unassigned3.bits() | Self::Unassigned4.bits() | Self::Unassigned5.bits()),
    );
    pub const FILTERS_INFO: Self = Self::from_bits_retain(Self::ShowHidden.bits());
    pub const FILTERS_ENABLE: Self = Self::from_bits_retain(Self::Enabled.bits() | Self::Disabled.bits());
    pub const FILTERS_CONFIG: Self = Self::FILTERS_ENABLE;
    pub const FILTERS_STATE: Self = Self::from_bits_retain(Self::CurrentMap.bits());
    pub const FILTERS_ALL: Self = Self::from_bits_retain(
        Self::FILTERS_INFO.bits() | Self::FILTERS_CONFIG.bits() | Self::FILTERS_STATE.bits(),
    );
    pub const FILTERS_INVERTED: Self =
        Self::from_bits_retain(Self::DEFAULT.bits() & Self::FILTERS_ALL.bits());
    pub const EMPTY: Self = Self::empty();

    pub fn as_str(self) -> Option<&'static str> {
        Some(match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::CurrentMap => "current-map",
            Self::ShowHidden => "show-hidden",
            _ => return None,
        })
    }
    #[cfg(todo = "unnecessary")]
    pub fn bit_as_str(self) -> Option<&'static str> {
        self.into_iter().next()?.as_str()
    }

    pub fn enable_filter(self) -> Option<bool> {
        match self & Self::FILTERS_ENABLE {
            Self::Enabled | Self::Disabled => Some(self.contains(Self::Enabled)),
            _ => None,
        }
    }
    pub fn filter_for_enable(enable: Option<bool>) -> Self {
        match enable {
            Some(true) => Self::Enabled,
            Some(false) => Self::Disabled,
            None => Self::empty(),
        }
    }
    pub fn set_enable_filter(&mut self, enable: Option<bool>) {
        self.remove(Self::FILTERS_ENABLE);
        self.insert(Self::filter_for_enable(enable));
    }
    /// clear out the invalid [Self::Enabled] | [Self::Disabled] combination
    pub fn canonicalize_enable_filter(&mut self) {
        if self.enable_filter().is_none() {
            self.remove(Self::FILTERS_ENABLE);
        }
    }
}

impl Default for PathingFilterFlags {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl FromStr for PathingFilterFlags {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "show-hidden" => Self::ShowHidden,
            "current-map" => Self::CurrentMap,
            "ignore-root" | "ignore-leaf" | "ignore-branch" => {
                // moved to search flags
                Self::empty()
            },
            _ => anyhow::bail!("unsupported filter option `{s}`"),
        })
    }
}
impl<'de> de::Deserialize<'de> for PathingFilterFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BitFlagDe::new().deserialize(deserializer)
    }
}
impl ser::Serialize for PathingFilterFlags {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        BitFlagSer::<Self>::new_human(*self).serialize(serializer)
    }
}
impl BitFlagContainer for PathingFilterFlags {
    type ClonedIter = <Self as IntoIterator>::IntoIter;
    type FromStrErr = <Self as FromStr>::Err;
    fn all() -> Self {
        Self::all()
    }
    fn empty() -> Self {
        Self::empty()
    }
    fn bit_name(&self) -> Option<&'static str> {
        self.as_str()
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

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PathingSearchFlags: u8 {
        const IGNORE_CASE = 1 << 0;
        const IGNORE_SPACE = 1 << 1;
        const INCLUDE_ID = 1 << 2;
        const PATTERN_REGEX = 1 << 3;
        const NEGATIVE = 1 << 7;
    }
}
impl PathingSearchFlags {
    pub const DEFAULT: Self = Self::from_bits_retain(Self::IGNORE_CASE.bits() | Self::IGNORE_SPACE.bits());
    pub const USER: Self = Self::from_bits_retain(
        Self::IGNORE_CASE.bits() | Self::IGNORE_SPACE.bits(), /*| Self::INCLUDE_ID.bits()*/
    );
    pub const ADVANCED: Self = Self::from_bits_retain(Self::all().bits() & !(Self::USER.bits()));
    pub const PERSIST: Self =
        Self::from_bits_retain((Self::USER.bits() | Self::ADVANCED.bits()) & !(Self::NEGATIVE.bits()));

    pub fn as_str(self) -> Option<&'static str> {
        Some(match self {
            Self::IGNORE_CASE => "case-insensitive",
            Self::IGNORE_SPACE => "ignore-whitespace",
            Self::INCLUDE_ID => "include-id",
            Self::PATTERN_REGEX => "pattern-regex",
            Self::NEGATIVE => "negative",
            #[cfg(todo = "unused")]
            Self::IGNORE_ROOT => "ignore-root",
            #[cfg(todo = "unused")]
            Self::IGNORE_LEAVES => "ignore-leaf",
            #[cfg(todo = "unused")]
            Self::IGNORE_BRANCHES => "ignore-branch",
            #[cfg(todo = "unused")]
            Self::INCLUDE_CHILDREN => "include-children",
            _ => return None,
        })
    }
    #[cfg(todo = "unnecessary")]
    pub fn bit_as_str(self) -> Option<&'static str> {
        self.into_iter().next()?.as_str()
    }
}
impl Default for PathingSearchFlags {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl FromStr for PathingSearchFlags {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "case-insensitive" => Self::IGNORE_CASE,
            "ignore-whitespace" => Self::IGNORE_SPACE,
            "include-id" => Self::INCLUDE_ID,
            "pattern-regex" => Self::PATTERN_REGEX,
            "negative" => Self::NEGATIVE,
            _ => anyhow::bail!("unsupported search option `{s}`"),
        })
    }
}
impl<'de> de::Deserialize<'de> for PathingSearchFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BitFlagDe::new().deserialize(deserializer)
    }
}
impl ser::Serialize for PathingSearchFlags {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let persist = *self & Self::PERSIST;
        BitFlagSer::<Self>::new_human(persist).serialize(serializer)
    }
}
impl BitFlagContainer for PathingSearchFlags {
    type ClonedIter = <Self as IntoIterator>::IntoIter;
    type FromStrErr = <Self as FromStr>::Err;
    fn all() -> Self {
        Self::all()
    }
    fn empty() -> Self {
        Self::empty()
    }
    fn bit_name(&self) -> Option<&'static str> {
        self.as_str()
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
