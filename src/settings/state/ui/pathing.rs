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
        const IgnoreRoot = 1 << 3;
        const IgnoreLeaves = 1 << 4;
        const IgnoreBranches = 1 << 5;
        const ShowHidden = 1 << 6;
    }
}

impl PathingFilterFlags {
    pub const DEFAULT: Self =
        Self::from_bits_retain(Self::Enabled.bits() | Self::Disabled.bits() | Self::IgnoreRoot.bits());
    /// TODO: implement [Self::ShowHidden]
    pub const USER: Self = Self::from_bits_retain(Self::all().bits() & !Self::ShowHidden.bits());

    pub fn as_str(self) -> Option<&'static str> {
        Some(match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::CurrentMap => "current-map",
            Self::IgnoreRoot => "ignore-root",
            Self::IgnoreLeaves => "ignore-leaf",
            Self::IgnoreBranches => "ignore-branch",
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
            "ignore-root" => Self::IgnoreRoot,
            "ignore-leaf" => Self::IgnoreLeaves,
            "ignore-branch" => Self::IgnoreBranches,
            "show-hidden" => Self::ShowHidden,
            "current-map" => Self::CurrentMap,
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
    }
}
impl PathingSearchFlags {
    pub const DEFAULT: Self = Self::from_bits_retain(Self::IGNORE_CASE.bits() | Self::IGNORE_SPACE.bits());

    pub fn as_str(self) -> Option<&'static str> {
        Some(match self {
            Self::IGNORE_CASE => "case-insensitive",
            Self::IGNORE_SPACE => "ignore-whitespace",
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
