use {
    anyhow::anyhow,
    bitflags::bitflags,
    std::{mem, str::FromStr},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Race {
    Asura = 1,
    Charr = 2,
    Human = 3,
    Norn = 4,
    Sylvari = 5,
}

impl Race {
    pub const REPR_MIN: u8 = Self::Asura as u8;
    pub const REPR_MAX: u8 = Self::Sylvari as u8;
    pub const INDEX_MAX: u8 = Self::REPR_MAX - 1;

    pub const fn repr(self) -> u8 {
        self as u8
    }
    pub const fn index(self) -> u8 {
        self.repr() - 1
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

impl TryFrom<i32> for Race {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        value
            .try_into()
            .ok()
            .and_then(Self::from_index)
            .ok_or_else(|| anyhow!("unknown race `{value}`"))
    }
}

impl FromStr for Race {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(i) = s.parse::<i32>() {
            i.try_into()
        } else if s.eq_ignore_ascii_case("asura") {
            Ok(Self::Asura)
        } else if s.eq_ignore_ascii_case("charr") {
            Ok(Self::Charr)
        } else if s.eq_ignore_ascii_case("human") {
            Ok(Self::Human)
        } else if s.eq_ignore_ascii_case("norn") {
            Ok(Self::Norn)
        } else if s.eq_ignore_ascii_case("sylvari") {
            Ok(Self::Sylvari)
        } else {
            Err(anyhow!("unknown race `{s}`"))
        }
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Races: u8 {
        const ASURA = 1 << Race::Asura.index();
        const CHARR = 1 << Race::Charr.index();
        const HUMAN = 1 << Race::Human.index();
        const NORN = 1 << Race::Norn.index();
        const SYLVARI = 1 << Race::Sylvari.index();
    }
}

impl Races {
    pub const fn for_race(race: Race) -> Self {
        Self::from_bits_retain(1u8 << race.index())
    }

    pub const fn to_race(self) -> Option<Race> {
        Race::from_index(self.bits().trailing_zeros() as u8)
    }

    pub const fn get(&self, race: Race) -> bool {
        self.contains(Self::for_race(race))
    }

    pub fn iter_races(self) -> impl Iterator<Item = Race> {
        self.into_iter().filter_map(Self::to_race)
    }
}

impl From<Race> for Races {
    fn from(race: Race) -> Self {
        Self::for_race(race)
    }
}
impl From<Option<Race>> for Races {
    fn from(race: Option<Race>) -> Self {
        race.map(Into::into).unwrap_or(Self::empty())
    }
}
impl FromIterator<Race> for Races {
    fn from_iter<T: IntoIterator<Item = Race>>(iter: T) -> Self {
        iter.into_iter().map(Self::from).collect()
    }
}
impl<'a> FromIterator<&'a Race> for Races {
    fn from_iter<T: IntoIterator<Item = &'a Race>>(iter: T) -> Self {
        iter.into_iter().map(|&f| Self::from(f)).collect()
    }
}
