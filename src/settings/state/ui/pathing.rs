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
    pub const DEFAULT: Self =
        Self::from_bits_retain(Self::Enabled.bits() | Self::Disabled.bits());
    pub const USER: Self = Self::from_bits_retain(Self::all().bits() & !(Self::Unassigned3.bits() | Self::Unassigned4.bits() | Self::Unassigned5.bits()));
    pub const FILTERS_INFO: Self = Self::from_bits_retain(
        Self::ShowHidden.bits()
    );
    pub const FILTERS_CONFIG: Self = Self::from_bits_retain(
        Self::Enabled.bits() | Self::Disabled.bits()
    );
    pub const FILTERS_STATE: Self = Self::from_bits_retain(
        Self::CurrentMap.bits()
    );
    pub const FILTERS_ALL: Self = Self::from_bits_retain(
        Self::FILTERS_INFO.bits() | Self::FILTERS_CONFIG.bits() | Self::FILTERS_STATE.bits()
    );
    pub const FILTERS_INVERTED: Self = Self::from_bits_retain(Self::DEFAULT.bits() & Self::FILTERS_ALL.bits());
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
        const IGNORE_ROOT = 1 << 2;
        const IGNORE_LEAVES = 1 << 3;
        const IGNORE_BRANCHES = 1 << 4;
        const INCLUDE_CHILDREN = 1 << 5;
    }
}
impl PathingSearchFlags {
    pub const DEFAULT: Self = Self::from_bits_retain(Self::IGNORE_CASE.bits() | Self::IGNORE_SPACE.bits() | Self::IGNORE_ROOT.bits());

    pub fn as_str(self) -> Option<&'static str> {
        Some(match self {
            Self::IGNORE_CASE => "case-insensitive",
            Self::IGNORE_SPACE => "ignore-whitespace",
            Self::IGNORE_ROOT => "ignore-root",
            Self::IGNORE_LEAVES => "ignore-leaf",
            Self::IGNORE_BRANCHES => "ignore-branch",
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
            "ignore-root" => Self::IGNORE_ROOT,
            "ignore-leaf" => Self::IGNORE_LEAVES,
            "ignore-branch" => Self::IGNORE_BRANCHES,
            "include-children" => Self::INCLUDE_CHILDREN,
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
        BitFlagSer::<Self>::new_human(*self).serialize(serializer)
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
