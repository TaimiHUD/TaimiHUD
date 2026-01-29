use {
    bitflags::bitflags,
    core::{num::NonZero, str::FromStr},
    serde::{
        de::{self, DeserializeSeed, Deserializer},
        ser,
    },
    taimi_hoard::flags::{BitFlagContainer, BitFlagDe, BitFlagSer},
    crate::settings::pathing::TriggerKind,
};

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialOrd, Ord, PartialEq, Eq, Hash)]
    pub struct InteractSortFlags: u8 {
        const TITLE = 0x01;
        const DISTANCE = 0x02;
        const VISIBLE = 0x04;
        const FILTERED = 0x08;
        const NEARBY = 0x10;
        const INTERACTIVE = 0x20;
        const ENABLED = 0x40;
    }
}
impl InteractSortFlags {
    pub const EMPTY: Self = Self::empty();
    /// bools that are typically more interesting when true
    /// (thus sort descending by default)
    pub const SORT_INVERTED: Self = Self::from_bits_retain(
        Self::NEARBY.bits()
        | Self::VISIBLE.bits()
        | Self::INTERACTIVE.bits()
        | Self::ENABLED.bits()
    );

    pub const DEFAULT_UI: Self = Self::from_bits_retain(
        Self::DISTANCE.bits()
        | Self::NEARBY.bits()
        //| Self::TITLE.bits()
    );
    pub const DEFAULT_UI_DESC: Self = match Self::SORT_INVERTED {
        #[cfg(todo)]
        inv => inv,
        inv => Self::from_bits_retain(Self::DEFAULT_UI.bits() & inv.bits()),
    };
    pub const INTERACTIVE_MASK: TriggerKind = TriggerKind::from_bits_retain(TriggerKind::SETTINGS_GUI.bits() & !TriggerKind::BOUNCE.bits());

    pub const fn interactive(flags: TriggerKind) -> Self {
        match flags.intersects(Self::INTERACTIVE_MASK) {
            true => Self::INTERACTIVE,
            false => Self::empty(),
        }
    }
    pub const fn get(self) -> Option<Self> {
        match self.is_empty() {
            true => None,
            false => Some(self),
        }
    }
    pub fn set_replace(&mut self, flag: Self, set: bool) -> bool {
        let prev = self.contains(flag);
        self.set(flag, set);
        prev
    }

    pub fn as_str(self) -> Option<&'static str> {
        Some(match self {
            Self::TITLE => "name",
            Self::DISTANCE => "distance",
            Self::VISIBLE => "visible",
            Self::FILTERED => "filtered",
            Self::NEARBY => "nearby",
            Self::INTERACTIVE => "interactive",
            Self::ENABLED => "enabled",
            _ => return None,
        })
    }

    pub(super) fn settings_default() -> Self {
        Self::DEFAULT_UI
    }
    pub(super) fn settings_default_descending() -> Self {
        Self::DEFAULT_UI_DESC
    }
    pub(super) fn is_settings_default(&self) -> bool {
        matches!(*self, Self::DEFAULT_UI)
    }
    pub(super) fn is_settings_default_descending(&self) -> bool {
        matches!(*self, Self::DEFAULT_UI_DESC)
    }
}

impl FromStr for InteractSortFlags {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "name" => Self::TITLE,
            "distance" => Self::DISTANCE,
            "visible" => Self::VISIBLE,
            "filtered" => Self::FILTERED,
            "nearby" => Self::NEARBY,
            "interactive" => Self::INTERACTIVE,
            "enabled" => Self::ENABLED,
            _ => anyhow::bail!("unsupported interact sort `{s}`"),
        })
    }
}
impl<'de> de::Deserialize<'de> for InteractSortFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BitFlagDe::new().deserialize(deserializer)
    }
}
impl ser::Serialize for InteractSortFlags {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        BitFlagSer::<Self>::new_human(*self).serialize(serializer)
    }
}
impl BitFlagContainer for InteractSortFlags {
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
    #[derive(Debug, Copy, Clone, Default, PartialOrd, Ord, PartialEq, Eq, Hash)]
    pub struct InteractFilterFlags: u8 {
        const FILTERED = InteractSortFlags::FILTERED.bits();
        const FAR = InteractSortFlags::DISTANCE.bits();
        const STATIC = InteractSortFlags::INTERACTIVE.bits();
        const DISABLED = InteractSortFlags::VISIBLE.bits();
    }
}
impl InteractFilterFlags {
    pub const EMPTY: Self = Self::empty();
    pub const SORT_FILTER: InteractSortFlags = InteractSortFlags::from_bits_truncate(Self::all().bits());
    pub const SORT_MASK: Self = Self::from_bits_truncate(Self::SORT_FILTER.bits());
    /// filters that are a direct negation ([Self::DISABLED] vs [InteractSortFlags::VISIBLE] for example)
    ///
    /// [Self::FILTERED] is the exception
    pub const SORT_INVERTED: Self = Self::from_bits_truncate(
        Self::DISABLED.bits()
        | Self::STATIC.bits()
        | Self::FAR.bits()
    );
    pub const SORT_INVERT_MASK: Self = Self::from_bits_truncate(
        !Self::SORT_INVERTED.bits()
    );

    pub const DEFAULT_UI: Self = Self::FILTERED;

    pub fn for_sort(flags: InteractSortFlags) -> Self {
        Self::sort_as_bits(flags) ^ Self::SORT_INVERT_MASK
    }
    pub const fn sort_as_bits(flags: InteractSortFlags) -> Self {
        Self::from_bits_retain(flags.bits() & Self::SORT_MASK.bits())
    }
    /// be careful using this directly
    pub const fn as_sort_bits(self) -> InteractSortFlags {
        InteractSortFlags::from_bits_retain(self.bits() & Self::SORT_FILTER.bits())
    }
    /// blacklist anything that doesn't contain all of these flags
    pub fn to_sort_exclude_not(self) -> InteractSortFlags {
        (self ^ Self::SORT_INVERT_MASK).as_sort_bits()
    }
    pub fn interactive(flags: TriggerKind) -> Self {
        Self::for_sort(InteractSortFlags::interactive(flags))
    }
    pub const fn get(self) -> Option<Self> {
        match self.is_empty() {
            true => None,
            false => Some(self),
        }
    }
    pub fn set_replace(&mut self, flag: Self, set: bool) -> bool {
        let prev = self.contains(flag);
        self.set(flag, set);
        prev
    }

    pub fn as_str(self) -> Option<&'static str> {
        Some(match self {
            Self::FILTERED => "filtered",
            Self::FAR => "far",
            Self::STATIC => "static",
            Self::DISABLED => "disabled",
            _ => return None,
        })
    }

    pub(crate) fn settings_default() -> Self {
        Self::DEFAULT_UI
    }
    pub(crate) fn is_settings_default(&self) -> bool {
        matches!(*self, Self::DEFAULT_UI)
    }
}

impl FromStr for InteractFilterFlags {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "filtered" => Self::FILTERED,
            "far" => Self::FAR,
            "static" => Self::STATIC,
            "disabled" => Self::DISABLED,
            _ => anyhow::bail!("unsupported interact sort `{s}`"),
        })
    }
}
impl<'de> de::Deserialize<'de> for InteractFilterFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BitFlagDe::new().deserialize(deserializer)
    }
}
impl ser::Serialize for InteractFilterFlags {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        BitFlagSer::<Self>::new_human(*self).serialize(serializer)
    }
}
impl BitFlagContainer for InteractFilterFlags {
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
