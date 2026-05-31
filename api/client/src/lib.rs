pub use gw2lib_model::{self as model, BulkEndpoint, Endpoint, EndpointWithId, FixedEndpoint, Language};
use {
    core::{fmt, marker::PhantomData, mem, ops},
    serde::de::DeserializeOwned,
};

#[cfg(feature = "reqwest")]
pub mod client;
pub mod festivals;

pub type IdValue = u32;
pub type Gw2ApiKey = String;
pub trait Gw2Endpoint: FixedEndpoint + DeserializeOwned {}
impl<T: FixedEndpoint + DeserializeOwned> Gw2Endpoint for T {}

pub trait Gw2BulkEndpoint: BulkEndpoint + FixedEndpoint + DeserializeOwned {}
impl<T: BulkEndpoint + FixedEndpoint + DeserializeOwned> Gw2BulkEndpoint for T {}

pub trait IdRange {
    fn id_is_multiple(&self) -> bool;
    fn id_is_all(&self) -> bool;

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
    fn id_is_all(&self) -> bool {
        IdRange::id_is_all(*self)
    }
    fn id_key(&self) -> Option<&'static str> {
        IdRange::id_key(*self)
    }
    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        IdRange::id_fmt_value(*self, f)
    }
}

impl dyn IdRange {
    pub fn fmt_value<E: EndpointWithId>(id: &E::IdType, f: &mut fmt::Formatter) -> fmt::Result {
        match id {
            #[cfg(todo = "unnecessary")]
            id => f.write_str(&E::format_id(id)),
            id => fmt::Display::fmt(id, f),
        }
    }

    pub fn fmt_values<E: EndpointWithId, I>(
        ids: impl IntoIterator<Item = I>,
        f: &mut fmt::Formatter,
    ) -> fmt::Result
    where
        I: ops::Deref<Target = E::IdType>,
    {
        let mut is_empty = true;
        for (i, id) in ids.into_iter().enumerate() {
            is_empty = false;
            if i > 0 {
                f.write_str(",")?;
            }
            Self::fmt_value::<E>(&id, f)?;
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
    pub const fn from_ref(id: &I) -> &Self {
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
    fn id_is_all(&self) -> bool {
        true
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("all")
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct RequestOne<E: EndpointWithId>(pub E::IdType);
impl<E: EndpointWithId> RequestOne<E> {
    pub const fn from_ref(id: &E::IdType) -> &Self {
        unsafe { mem::transmute(id) }
    }
    pub fn from_mut(id: &mut E::IdType) -> &mut Self {
        unsafe { mem::transmute(id) }
    }
}
impl<E: EndpointWithId> IdRange for RequestOne<E> {
    fn id_is_multiple(&self) -> bool {
        false
    }
    fn id_is_all(&self) -> bool {
        false
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_value::<E>(&self.0, f)
    }
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct IdsIn<E: EndpointWithId, R> {
    pub _endpoint: PhantomData<E>,
    pub range: R,
}
impl<E: EndpointWithId, R> IdsIn<E, R> {
    pub const fn new(range: R) -> Self {
        Self { range, _endpoint: PhantomData }
    }
}
impl<E: EndpointWithId, I: ops::Deref<Target = E::IdType> + PartialOrd> IdRange for IdsIn<E, ops::Range<I>>
where
    ops::Range<I>: Iterator<Item = I> + ExactSizeIterator + Clone,
{
    fn id_is_multiple(&self) -> bool {
        //self.range.len() != 1
        self.range.clone().count() != 1
    }
    fn id_is_all(&self) -> bool {
        false
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_values::<E, _>(self.range.clone(), f)
    }
}
impl<E: EndpointWithId, I: ops::Deref<Target = E::IdType> + PartialOrd> IdRange
    for IdsIn<E, ops::RangeTo<I>>
where
    ops::RangeTo<I>: Iterator<Item = I> + ExactSizeIterator + Clone,
{
    fn id_is_multiple(&self) -> bool {
        self.range.clone().count() != 1
    }
    fn id_is_all(&self) -> bool {
        false
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_values::<E, _>(self.range.clone(), f)
    }
}
impl<E: EndpointWithId, I: ops::Deref<Target = E::IdType> + PartialOrd> IdRange
    for IdsIn<E, ops::RangeFrom<I>>
where
    ops::RangeFrom<I>: Iterator<Item = I> + ExactSizeIterator + Clone,
{
    fn id_is_multiple(&self) -> bool {
        self.range.clone().count() != 1
    }
    fn id_is_all(&self) -> bool {
        false
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_values::<E, _>(self.range.clone(), f)
    }
}
impl<E: EndpointWithId, I: ops::Deref<Target = E::IdType> + PartialOrd> IdRange
    for IdsIn<E, ops::RangeInclusive<I>>
where
    ops::RangeInclusive<I>: Iterator<Item = I> + ExactSizeIterator + Clone,
{
    fn id_is_multiple(&self) -> bool {
        self.range.clone().count() != 1
    }
    fn id_is_all(&self) -> bool {
        false
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <dyn IdRange>::fmt_values::<E, _>(self.range.clone(), f)
    }
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct IterIds<E: EndpointWithId, I: ?Sized> {
    pub _endpoint: PhantomData<E>,
    pub iter: I,
}
impl<E: EndpointWithId, I> IterIds<E, I> {
    pub const fn new(iter: I) -> Self {
        Self { iter, _endpoint: PhantomData }
    }
}
impl<E: EndpointWithId, I: ?Sized> IterIds<E, I> {
    pub fn from_ref(iter: &I) -> &Self {
        unsafe { mem::transmute(iter) }
    }
    pub fn from_mut(iter: &mut I) -> &mut Self {
        unsafe { mem::transmute(iter) }
    }
}
impl<E: EndpointWithId, I: ?Sized, ID> IdRange for IterIds<E, I>
where
    for<'a> &'a I: IntoIterator<Item = &'a ID>,
    for<'a> ID: Clone + ops::Deref<Target = E::IdType>,
    for<'a> <&'a I as IntoIterator>::IntoIter: ExactSizeIterator,
{
    fn id_is_multiple(&self) -> bool {
        IntoIterator::into_iter(&self.iter).len() != 1
    }
    fn id_is_all(&self) -> bool {
        false
    }

    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let iter = IntoIterator::into_iter(&self.iter).cloned();
        <dyn IdRange>::fmt_values::<E, ID>(iter, f)
    }
}

pub enum IdQueryContainer<E: EndpointWithId> {
    Ids,
    Single(E::IdType),
    Multi(Vec<E::IdType>),
    All,
}

impl<E: EndpointWithId> IdQueryContainer<E> {
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

impl<E: EndpointWithId> fmt::Display for IdQueryContainer<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.id_fmt_value(f)
    }
}

impl<E: EndpointWithId> IdRange for IdQueryContainer<E> {
    fn id_is_multiple(&self) -> bool {
        match self {
            Self::Single(..) => false,
            _ => true,
        }
    }
    fn id_is_all(&self) -> bool {
        matches!(self, Self::All)
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct RequestAll<E>(pub PhantomData<E>);
impl<E> RequestAll<E> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<E: BulkEndpoint> IdRange for RequestAll<E> {
    fn id_is_multiple(&self) -> bool {
        true
    }
    fn id_is_all(&self) -> bool {
        E::ALL
    }
    fn id_key(&self) -> Option<&'static str> {
        E::ALL.then_some("ids")
    }
    fn id_fmt_value(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("all")
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct RequestIds<E>(pub PhantomData<E>);
impl<E> RequestIds<E> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<E: EndpointWithId> IdRange for RequestIds<E> {
    fn id_is_multiple(&self) -> bool {
        true
    }
    fn id_is_all(&self) -> bool {
        false
    }
    fn id_key(&self) -> Option<&'static str> {
        None
    }
    fn id_fmt_value(&self, _: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

impl IdRange for () {
    /// if it could be, use [RequestAll] instead
    fn id_is_multiple(&self) -> bool {
        false
    }
    fn id_is_all(&self) -> bool {
        false
    }
    fn id_key(&self) -> Option<&'static str> {
        None
    }
    fn id_fmt_value(&self, _: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}
