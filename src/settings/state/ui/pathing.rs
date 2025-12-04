use {
    bitflags::bitflags,
    core::{fmt, marker::PhantomData, num::NonZero, ops, str::FromStr},
    serde::{
        de::{self, DeserializeSeed, Deserializer},
        ser,
    },
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
        BitFlagSerde::new().deserialize(deserializer)
    }
}
/// TODO: string/list encoding?
impl ser::Serialize for PathingFilterFlags {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
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
        BitFlagSerde::new().deserialize(deserializer)
    }
}
/// TODO: string/list encoding?
impl ser::Serialize for PathingSearchFlags {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitFlagSerde<F> {
    pub strict: bool,
    pub _flags: PhantomData<fn() -> F>,
}
impl<F> BitFlagSerde<F> {
    pub const fn new() -> Self {
        Self { strict: false, _flags: PhantomData }
    }
}
impl<'de, F> de::Visitor<'de> for &'_ BitFlagSerde<F>
where
    F: BitFlagContainer,
{
    type Value = F;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let all = F::all();
        let flags_display = BitFlagDisplay::human(&all);
        write!(f, "one or many flags of: {flags_display}")
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        self.visit_u64(value as u64)
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        match F::from_bits64(value as u64) {
            Ok(flags) => Ok(flags),
            Err((flags, rest)) => {
                let flags_display = BitFlagDisplay::human(&flags);
                match self.strict {
                    true => Err(de::Error::custom(format_args!(
                        "extra unrecognized bits with {flags_display}: {rest:#010x}"
                    ))),
                    false => {
                        log::warn!("extra unrecognized bits with {flags_display}: {rest:#010x}");
                        Ok(flags)
                    },
                }
            },
        }
    }
    /// TODO: comma-sep
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        match F::try_from_str(v) {
            Ok(f) => Ok(f),
            Err(e) => Err(de::Error::custom(e)),
        }
    }
    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        match self.strict {
            true => Err(de::Error::invalid_type(de::Unexpected::Option, &self)),
            false => Ok(F::empty()),
        }
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        match self.strict {
            true => Err(de::Error::invalid_type(de::Unexpected::Unit, &self)),
            false => Ok(F::empty()),
        }
    }
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_u64(self)
    }
    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut flags = F::empty();
        while let Some(next) = seq.next_element_seed(self)? {
            flags |= next;
        }
        Ok(flags)
    }
    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut flags = F::empty();
        while let Some(next) = map.next_key_seed(self)? {
            let is_set = map.next_value::<bool>()?;
            if is_set {
                flags |= next;
            }
        }
        Ok(flags)
    }
}
impl<'de, F> DeserializeSeed<'de> for &'_ BitFlagSerde<F>
where
    F: BitFlagContainer,
{
    type Value = F;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_u64(self)
    }
}
impl<F> Default for BitFlagSerde<F> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait BitFlagContainer: Sized + Clone + ops::BitOr + ops::BitOrAssign + fmt::Debug {
    type ClonedIter: Iterator<Item = Self>;
    type FromStrErr: fmt::Display;
    fn all() -> Self;
    fn empty() -> Self;
    fn bit_name(&self) -> Option<&'static str>;
    fn iter(&self) -> Self::ClonedIter;
    fn bits64(&self) -> u64;
    fn from_bits64(bits: u64) -> Result<Self, (Self, NonZero<u64>)>;
    fn try_from_str(s: &str) -> Result<Self, Self::FromStrErr>;
    fn display_fmt(display: &BitFlagDisplay<'_, Self>, f: &mut fmt::Formatter) -> fmt::Result {
        let flags = display.flags;
        let prefix = match display.human_spacing {
            true => ", ",
            false => ",",
        };
        for (i, flag) in flags.iter().enumerate() {
            let prefix = (i > 0).then_some(prefix).unwrap_or("");
            let bits;
            let name = Self::bit_name(&flag);
            let display = match name {
                Some(ref name) => name as &dyn fmt::Display,
                None => {
                    bits = flags.bits64();
                    &bits as &dyn fmt::Display
                },
            };
            write!(f, "{prefix}{display}")?;
        }
        Ok(())
    }
}
struct BitFlagDisplay<'a, F> {
    pub flags: &'a F,
    pub human_spacing: bool,
}
impl<'a, F> BitFlagDisplay<'a, F> {
    pub const fn serialize(flags: &'a F) -> Self {
        Self { flags, human_spacing: false }
    }
    pub const fn human(flags: &'a F) -> Self {
        Self { flags, human_spacing: true }
    }
}
impl<F: BitFlagContainer> fmt::Display for BitFlagDisplay<'_, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        F::display_fmt(self, f)
    }
}
