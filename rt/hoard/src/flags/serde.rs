use core::{fmt, num::NonZero, ops};

#[cfg(feature = "serde")]
use {
    core::marker::PhantomData,
    serde::{
        de::{self, DeserializeSeed, Deserializer},
        ser::{self, SerializeSeq},
    },
};

#[cfg(feature = "serde")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitFlagDe<F> {
    pub strict: bool,
    pub _flags: PhantomData<fn() -> F>,
}
#[cfg(feature = "serde")]
impl<F> BitFlagDe<F> {
    pub const fn new() -> Self {
        Self { strict: false, _flags: PhantomData }
    }
}
#[cfg(feature = "serde")]
impl<'de, F> de::Visitor<'de> for &'_ BitFlagDe<F>
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
        deserializer.deserialize_any(self)
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
/// can the others not be used as a hint :<
#[cfg(feature = "serde")]
impl<'de, F> DeserializeSeed<'de> for &'_ BitFlagDe<F>
where
    F: BitFlagContainer,
{
    type Value = F;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}
#[cfg(feature = "serde")]
impl<F> Default for BitFlagDe<F> {
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
pub struct BitFlagDisplay<'a, F> {
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

#[cfg(feature = "serde")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitFlagSer<F> {
    pub string: bool,
    pub flags: F,
}
#[cfg(feature = "serde")]
impl<F> BitFlagSer<F> {
    pub const fn new_bits(flags: F) -> Self {
        Self { string: false, flags }
    }
    pub const fn new_human(flags: F) -> Self {
        Self { string: true, flags }
    }
}
#[cfg(feature = "serde")]
impl<F: BitFlagContainer> ser::Serialize for BitFlagSer<F> {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.string {
            true => {
                let len = match &self.flags {
                    #[cfg(todo)]
                    flags => Some(flags.bits64().count_ones() as _),
                    _ => None,
                };
                let mut seq = serializer.serialize_seq(len)?;
                for flag in self.flags.iter() {
                    match F::bit_name(&flag) {
                        Some(name) => seq.serialize_element(name),
                        None => seq.serialize_element(&flag.bits64()),
                    }?;
                }
                seq.end()
            },
            false => self.flags.bits64().serialize(serializer),
        }
    }
}
