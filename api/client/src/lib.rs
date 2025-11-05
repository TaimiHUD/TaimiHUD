pub use gw2lib_model::{self as model, BulkEndpoint, Endpoint, EndpointWithId, FixedEndpoint, Language};
use {
    core::{fmt, mem, ops},
    serde::{de::DeserializeOwned, Serialize},
};

#[cfg(feature = "reqwest")]
pub mod client;
pub mod festivals;

pub type IdValue = u32;
pub type Gw2ApiKey = String;
pub trait Gw2Endpoint: FixedEndpoint + DeserializeOwned + Serialize {}
impl<T: FixedEndpoint + DeserializeOwned + Serialize> Gw2Endpoint for T {}

pub trait Gw2BulkEndpoint: BulkEndpoint + FixedEndpoint + DeserializeOwned + Serialize {}
impl<T: BulkEndpoint + FixedEndpoint + DeserializeOwned + Serialize> Gw2BulkEndpoint for T {}

pub trait IdRange {
    fn id_is_multiple(&self) -> bool;

    fn id_key(&self) -> Option<&'static str> {
        Some(match self.id_is_multiple() {
            false => "id",
            true => "ids",
        })
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result;

    fn id_string_value(&self) -> String
    where
        Self: Sized,
    {
        self.id_display_value().as_dyn().to_string()
    }
    fn id_display_value(&self) -> &IdRangeDisplay<Self>
    where
        Self: Sized,
    {
        IdRangeDisplay::from_ref(self)
    }
}
impl<I: IdRange> IdRange for &'_ I {
    fn id_is_multiple(&self) -> bool {
        IdRange::id_is_multiple(*self)
    }
    fn id_key(&self) -> Option<&'static str> {
        IdRange::id_key(*self)
    }
    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        IdRange::id_fmt_value(*self, f)
    }
}

impl dyn IdRange {
    pub fn fmt_value<I: Into<IdValue>>(id: I, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&id.into(), f)
    }

    pub fn fmt_values<I: IntoIterator>(ids: I, f: &mut fmt::Formatter) -> fmt::Result
    where
        I::Item: Into<IdValue>,
    {
        let mut is_empty = true;
        for (i, id) in ids.into_iter().enumerate() {
            is_empty = false;
            if i > 0 {
                f.write_str(",")?;
            }
            Self::fmt_value(id, f)?;
        }
        if is_empty {
            log::warn!("API request expected IDs");
            //return f.write_str(",")
            f.write_str("-1")
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct IdRangeDisplay<I: ?Sized>(pub I);
impl<I: ?Sized> IdRangeDisplay<I> {
    pub fn from_ref(id: &I) -> &Self {
        unsafe { mem::transmute(id) }
    }

    pub fn as_dyn<'a>(&'a self) -> &'a IdRangeDisplay<dyn IdRange + 'a>
    where
        I: IdRange + Sized,
    {
        IdRangeDisplay::from_ref(&self.0 as &dyn IdRange)
    }
}
impl<I: ?Sized + IdRange> fmt::Display for IdRangeDisplay<I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.id_fmt_value(f)
    }
}

impl IdRange for ops::RangeFull {
    fn id_is_multiple(&self) -> bool {
        true
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("all")
    }
}
impl IdRange for IdValue {
    fn id_is_multiple(&self) -> bool {
        false
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_value(*self, f)
    }
}
impl<I: Into<IdValue> + PartialOrd> IdRange for ops::Range<I>
where
    Self: Iterator<Item = I> + ExactSizeIterator + Clone,
{
    fn id_is_multiple(&self) -> bool {
        self.len() != 1
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_values(self.clone(), f)
    }
}
impl<I: Into<IdValue> + PartialOrd> IdRange for ops::RangeTo<I>
where
    Self: Iterator<Item = I> + ExactSizeIterator + Clone,
{
    fn id_is_multiple(&self) -> bool {
        self.len() != 1
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_values(self.clone(), f)
    }
}
impl<I: Into<IdValue> + PartialOrd> IdRange for ops::RangeFrom<I>
where
    Self: Iterator<Item = I> + ExactSizeIterator + Clone,
{
    fn id_is_multiple(&self) -> bool {
        self.len() != 1
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_values(self.clone(), f)
    }
}
impl<I: Into<IdValue> + PartialOrd> IdRange for ops::RangeInclusive<I>
where
    Self: Iterator<Item = I> + ExactSizeIterator + Clone,
{
    fn id_is_multiple(&self) -> bool {
        self.len() != 1
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_values(self.clone(), f)
    }
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct IdRangeIter<I: ?Sized>(pub I);
impl<I, ID> IdRange for IdRangeIter<I>
where
    for<'a> &'a I: IntoIterator<Item = &'a ID>,
    for<'a> ID: Clone + Into<IdValue>,
    for<'a> <&'a I as IntoIterator>::IntoIter: ExactSizeIterator,
{
    fn id_is_multiple(&self) -> bool {
        IntoIterator::into_iter(&self.0).len() != 1
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let iter = IntoIterator::into_iter(&self.0).cloned();
        <dyn IdRange>::fmt_values(iter, f)
    }
}

pub enum IdQueryContainer {
    Ids,
    Single(IdValue),
    Multi(Vec<IdValue>),
    All,
}

impl IdQueryContainer {
    pub fn key(&self) -> Option<&'static str> {
        Some(match self {
            Self::Ids => return None,
            Self::Single(..) => "id",
            Self::Multi(..) | Self::All => "ids",
        })
    }
    #[cfg(todo = "unused")]
    fn pair(&self) -> Option<(&'static str, String)> {
        self.key().map(|key| (key, self.to_string()))
    }
}

impl fmt::Display for IdQueryContainer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.id_fmt_value(f)
    }
}

impl From<IdValue> for IdQueryContainer {
    fn from(id: IdValue) -> Self {
        Self::Single(id)
    }
}
impl From<ops::RangeFull> for IdQueryContainer {
    fn from(_: ops::RangeFull) -> Self {
        Self::All
    }
}
impl IdRange for IdQueryContainer {
    fn id_is_multiple(&self) -> bool {
        match self {
            Self::Single(..) => false,
            _ => true,
        }
    }

    fn id_key(&self) -> Option<&'static str> {
        self.key()
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Ids => Ok(()),
            Self::Single(id) => fmt::Display::fmt(id, f),
            Self::Multi(ids) => {
                if ids.is_empty() {
                    log::warn!("API request expected IDs");
                    //return f.write_str(",")
                    return f.write_str("-1")
                }
                for (i, id) in ids.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    fmt::Display::fmt(id, f)?;
                }
                Ok(())
            },
            Self::All => f.write_str("all"),
        }
    }
}
