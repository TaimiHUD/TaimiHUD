use {
    crate::attributes::keys::{self, GetAttr, SetAttr},
    anyhow::anyhow,
    bitflags::bitflags,
    std::{mem, str::FromStr},
};

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Mount {
    #[default]
    None = 0,
    Jackal = 1,
    Griffon = 2,
    Springer = 3,
    Skimmer = 4,
    Raptor = 5,
    RollerBeetle = 6,
    Warclaw = 7,
    Skyscale = 8,
    Skiff = 9,
    SiegeTurtle = 10,
}

impl Mount {
    pub const REPR_MIN: u8 = Self::None as u8;
    pub const REPR_MAX: u8 = Self::SiegeTurtle as u8;
    pub const INDEX_MAX: u8 = Self::REPR_MAX;

    pub const fn repr(self) -> u8 {
        self as u8
    }
    pub const fn index(self) -> u8 {
        self.repr()
    }
    pub const fn from_repr(repr: u8) -> Option<Self> {
        Self::from_index(repr)
    }
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0..=Self::INDEX_MAX => Some(unsafe { Self::from_index_unchecked(index) }),
            _ => None,
        }
    }
    pub const unsafe fn from_index_unchecked(index: u8) -> Self {
        mem::transmute(index + 1)
    }
}

impl TryFrom<i32> for Mount {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use Mount::*;
        Ok(match value {
            0 => None,
            1 => Jackal,
            2 => Griffon,
            3 => Springer,
            4 => Skimmer,
            5 => Raptor,
            6 => RollerBeetle,
            7 => Warclaw,
            8 => Skyscale,
            9 => Skiff,
            10 => SiegeTurtle,
            _ => {
                anyhow::bail!("unknown mount `{value}`")
            },
        })
    }
}

impl FromStr for Mount {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Mount::*;
        if let Ok(i) = s.parse::<i32>() {
            i.try_into()
        } else if s.eq_ignore_ascii_case("none") {
            Ok(None)
        } else if s.eq_ignore_ascii_case("jackal") {
            Ok(Jackal)
        } else if s.eq_ignore_ascii_case("griffon") {
            Ok(Griffon)
        } else if s.eq_ignore_ascii_case("springer") {
            Ok(Springer)
        } else if s.eq_ignore_ascii_case("skimmer") {
            Ok(Skimmer)
        } else if s.eq_ignore_ascii_case("raptor") {
            Ok(Raptor)
        } else if s.eq_ignore_ascii_case("rollerbeetle") {
            Ok(RollerBeetle)
        } else if s.eq_ignore_ascii_case("warclaw") {
            Ok(Warclaw)
        } else if s.eq_ignore_ascii_case("skyscale") {
            Ok(Skyscale)
        } else if s.eq_ignore_ascii_case("skiff") {
            Ok(Skiff)
        } else if s.eq_ignore_ascii_case("siegeturtle") {
            Ok(SiegeTurtle)
        } else {
            Err(anyhow!("unknown mount `{s}`"))
        }
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Mounts: u16 {
        const NONE = 1 << Mount::None.index();
        const JACKAL = 1 << Mount::Jackal.index();
        const GRIFFON = 1 << Mount::Griffon.index();
        const SPRINGER = 1 << Mount::Springer.index();
        const SKIMMER = 1 << Mount::Skimmer.index();
        const RAPTOR = 1 << Mount::Raptor.index();
        const ROLLER_BEETLE = 1 << Mount::RollerBeetle.index();
        const WARCLAW = 1 << Mount::Warclaw.index();
        const SKYSCALE = 1 << Mount::Skyscale.index();
        const SKIFF = 1 << Mount::Skiff.index();
        const SIEGE_TURTLE = 1 << Mount::SiegeTurtle.index();
    }
}

impl Mounts {
    pub const fn for_mount(mount: Mount) -> Self {
        match mount {
            mount => Self::from_bits_retain(1u16 << mount.index()),
        }
    }

    pub const fn to_mount(self) -> Option<Mount> {
        Mount::from_index(self.bits().trailing_zeros() as u8)
    }

    pub const fn get(&self, mount: Mount) -> bool {
        self.contains(Self::for_mount(mount))
    }

    pub fn iter_mounts(self) -> impl Iterator<Item = Mount> {
        self.into_iter().filter_map(Self::to_mount)
    }
}

impl From<Mount> for Mounts {
    fn from(mount: Mount) -> Self {
        Self::for_mount(mount)
    }
}
impl From<Option<Mount>> for Mounts {
    fn from(mount: Option<Mount>) -> Self {
        mount.map(Into::into).unwrap_or(Self::empty())
    }
}
impl FromIterator<Mount> for Mounts {
    fn from_iter<T: IntoIterator<Item = Mount>>(iter: T) -> Self {
        iter.into_iter().map(Self::from).collect()
    }
}
impl<'a> FromIterator<&'a Mount> for Mounts {
    fn from_iter<T: IntoIterator<Item = &'a Mount>>(iter: T) -> Self {
        iter.into_iter().map(|&f| Self::from(f)).collect()
    }
}

impl<T> GetAttr<Mount> for T where
    T: ?Sized + GetAttr<keys::Mounts>,
{
    fn has_attr(&self) -> bool {
        GetAttr::<keys::Mounts>::get_attr(self).map(|f|
            !f.0.is_empty()
        ).unwrap_or(false)
    }
    fn get_attr(&self) -> Option<std::borrow::Cow<'_, Mount>> {
        GetAttr::<keys::Mounts>::get_attr(self).and_then(|f|
            f.0.iter_mounts().next()
        ).map(std::borrow::Cow::Owned)
    }
}
impl<T> SetAttr<Mount> for T where
    T: ?Sized + SetAttr<keys::Mounts> + GetAttr<keys::Mounts>,
{
    fn set_attr(&mut self, value: Mount) {
        let mut f = GetAttr::<keys::Mounts>::get_attr_or_default(self).into_owned();
        f.0.insert(value.into());
        SetAttr::<keys::Mounts>::set_attr(self, f)
    }
    fn unset_attr(&mut self) {
        SetAttr::<keys::Mounts>::unset_attr(self)
    }
}
