use {
    anyhow::anyhow,
    bitflags::bitflags,
    std::{mem, str::FromStr},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Profession {
    Guardian = 1,
    Warrior = 2,
    Engineer = 3,
    Ranger = 4,
    Thief = 5,
    Elementalist = 6,
    Mesmer = 7,
    Necromancer = 8,
    Revenant = 9,
}

impl Profession {
    pub const REPR_MIN: u8 = Self::Guardian as u8;
    pub const REPR_MAX: u8 = Self::Revenant as u8;
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

impl TryFrom<i32> for Profession {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use Profession::*;
        Ok(match value {
            1 => Guardian,
            2 => Warrior,
            3 => Engineer,
            4 => Ranger,
            5 => Thief,
            6 => Elementalist,
            7 => Mesmer,
            8 => Necromancer,
            9 => Revenant,
            _ => {
                anyhow::bail!("unknown profession `{value}`")
            },
        })
    }
}

impl FromStr for Profession {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Profession::*;
        if let Ok(i) = s.parse::<i32>() {
            i.try_into()
        } else if s.eq_ignore_ascii_case("guardian") {
            Ok(Guardian)
        } else if s.eq_ignore_ascii_case("warrior") {
            Ok(Warrior)
        } else if s.eq_ignore_ascii_case("Engineer") {
            Ok(Engineer)
        } else if s.eq_ignore_ascii_case("ranger") {
            Ok(Ranger)
        } else if s.eq_ignore_ascii_case("thief") {
            Ok(Thief)
        } else if s.eq_ignore_ascii_case("elementalist") {
            Ok(Elementalist)
        } else if s.eq_ignore_ascii_case("mesmer") {
            Ok(Mesmer)
        } else if s.eq_ignore_ascii_case("necromancer") {
            Ok(Necromancer)
        } else if s.eq_ignore_ascii_case("revenant") {
            Ok(Revenant)
        } else {
            Err(anyhow!("unknown profession `{s}`"))
        }
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Professions: u16 {
        const GUARDIAN = 1 << Profession::Guardian.index();
        const WARRIOR = 1 << Profession::Warrior.index();
        const ENGINEER = 1 << Profession::Engineer.index();
        const RANGER = 1 << Profession::Ranger.index();
        const THIEF = 1 << Profession::Thief.index();
        const ELEMENTALIST = 1 << Profession::Elementalist.index();
        const MESMER = 1 << Profession::Mesmer.index();
        const NECROMANCER = 1 << Profession::Necromancer.index();
        const REVENANT = 1 << Profession::Revenant.index();
    }
}

impl Professions {
    pub const fn for_profession(profession: Profession) -> Self {
        Self::from_bits_retain(1u16 << profession.index())
    }

    pub const fn to_profession(self) -> Option<Profession> {
        Profession::from_index(self.bits().trailing_zeros() as u8)
    }

    pub const fn get(&self, profession: Profession) -> bool {
        self.contains(Self::for_profession(profession))
    }

    pub fn iter_professions(self) -> impl Iterator<Item = Profession> {
        self.into_iter().filter_map(Self::to_profession)
    }
}

impl From<Profession> for Professions {
    fn from(profession: Profession) -> Self {
        Self::for_profession(profession)
    }
}
impl From<Option<Profession>> for Professions {
    fn from(profession: Option<Profession>) -> Self {
        profession.map(Into::into).unwrap_or(Self::empty())
    }
}
impl FromIterator<Profession> for Professions {
    fn from_iter<T: IntoIterator<Item = Profession>>(iter: T) -> Self {
        iter.into_iter().map(Self::from).collect()
    }
}
impl<'a> FromIterator<&'a Profession> for Professions {
    fn from_iter<T: IntoIterator<Item = &'a Profession>>(iter: T) -> Self {
        iter.into_iter().map(|&f| Self::from(f)).collect()
    }
}
