use {crate::attributes::keys::{self, GetAttr, SetAttr}, anyhow::anyhow, bitflags::bitflags, std::str::FromStr};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Festival {
    Halloween,
    Wintersday,
    #[cfg_attr(feature = "serde", serde(rename = "superadventurefestival"))]
    SuperAdventureBox,
    LunarNewYear,
    #[cfg_attr(feature = "serde", serde(rename = "festivalofthefourwinds"))]
    FourWinds,
    DragonBash,
}

impl Festival {
    pub const ALL: &'static [Self] = &[
        Self::LunarNewYear,
        Self::SuperAdventureBox,
        Self::DragonBash,
        Self::FourWinds,
        Self::Halloween,
        Self::Wintersday,
    ];

    pub fn all() -> impl Iterator<Item = Self> + Clone {
        Self::ALL.iter().copied()
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Halloween => "halloween",
            Self::Wintersday => "wintersday",
            Self::SuperAdventureBox => "superadventurefestival",
            Self::LunarNewYear => "lunarnewyear",
            Self::FourWinds => "festivalofthefourwinds",
            Self::DragonBash => "dragonbash",
        }
    }
}

impl FromStr for Festival {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("halloween") {
            Ok(Self::Halloween)
        } else if s.eq_ignore_ascii_case("wintersday") {
            Ok(Self::Wintersday)
        } else if s.eq_ignore_ascii_case("superadventurefestival") {
            Ok(Self::SuperAdventureBox)
        } else if s.eq_ignore_ascii_case("lunarnewyear") {
            Ok(Self::LunarNewYear)
        } else if s.eq_ignore_ascii_case("festivalofthefourwinds") {
            Ok(Self::FourWinds)
        } else if s.eq_ignore_ascii_case("dragonbash") {
            Ok(Self::DragonBash)
        } else {
            Err(anyhow!("unexpected festival `{s}`"))
        }
    }
}

impl AsRef<str> for Festival {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl Into<String> for Festival {
    fn into(self) -> String {
        self.as_str().into()
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Festivals: u16 {
        const LUNAR_NEW_YEAR = 0x01;
        const SUPER_ADVENTURE_BOX = 0x02;
        const DRAGON_BASH = 0x04;
        const FOUR_WINDS = 0x08;
        const HALLOWEEN = 0x10;
        const WINTERSDAY = 0x20;
    }
}

impl Festivals {
    pub const fn for_festival(festival: Festival) -> Self {
        match festival {
            Festival::LunarNewYear => Self::LUNAR_NEW_YEAR,
            Festival::SuperAdventureBox => Self::SUPER_ADVENTURE_BOX,
            Festival::DragonBash => Self::DRAGON_BASH,
            Festival::FourWinds => Self::FOUR_WINDS,
            Festival::Halloween => Self::HALLOWEEN,
            Festival::Wintersday => Self::WINTERSDAY,
        }
    }

    pub const fn to_festival(self) -> Option<Festival> {
        Some(match self {
            Self::LUNAR_NEW_YEAR => Festival::LunarNewYear,
            Self::SUPER_ADVENTURE_BOX => Festival::SuperAdventureBox,
            Self::DRAGON_BASH => Festival::DragonBash,
            Self::FOUR_WINDS => Festival::FourWinds,
            Self::HALLOWEEN => Festival::Halloween,
            Self::WINTERSDAY => Festival::Wintersday,
            _ => return None,
        })
    }

    pub const fn get(&self, festival: Festival) -> bool {
        self.contains(Self::for_festival(festival))
    }

    pub fn iter_festivals(self) -> impl Iterator<Item = Festival> {
        self.into_iter().filter_map(Self::to_festival)
    }
}

impl From<Festival> for Festivals {
    fn from(festival: Festival) -> Self {
        Self::for_festival(festival)
    }
}
impl From<Option<Festival>> for Festivals {
    fn from(festival: Option<Festival>) -> Self {
        festival.map(Into::into).unwrap_or(Self::empty())
    }
}
impl FromIterator<Festival> for Festivals {
    fn from_iter<T: IntoIterator<Item = Festival>>(iter: T) -> Self {
        iter.into_iter().map(Self::from).collect()
    }
}
impl<'a> FromIterator<&'a Festival> for Festivals {
    fn from_iter<T: IntoIterator<Item = &'a Festival>>(iter: T) -> Self {
        iter.into_iter().map(|&f| Self::from(f)).collect()
    }
}

impl<T> GetAttr<Festival> for T where
    T: ?Sized + GetAttr<keys::Festivals>,
    // TODO: dumb hack to avoid blanket impl havoc
    T: core::borrow::Borrow<crate::attributes::FilterAttributes>,
{
    fn has_attr(&self) -> bool {
        GetAttr::<keys::Festivals>::get_attr(self).map(|f|
            !f.0.is_empty()
        ).unwrap_or(false)
    }
    fn get_attr(&self) -> Option<std::borrow::Cow<'_, Festival>> {
        GetAttr::<keys::Festivals>::get_attr(self).and_then(|f|
            f.0.iter_festivals().next()
        ).map(std::borrow::Cow::Owned)
    }
}
impl<T> SetAttr<Festival> for T where
    T: ?Sized + SetAttr<keys::Festivals> + GetAttr<keys::Festivals>,
    T: core::borrow::Borrow<crate::attributes::FilterAttributes>,
{
    fn set_attr(&mut self, value: Festival) {
        let mut f = GetAttr::<keys::Festivals>::get_attr_or_default(self).into_owned();
        f.0.insert(value.into());
        SetAttr::<keys::Festivals>::set_attr(self, f)
    }
    fn unset_attr(&mut self) {
        SetAttr::<keys::Festivals>::unset_attr(self)
    }
}
