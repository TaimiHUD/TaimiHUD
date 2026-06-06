use std::{
    borrow::{Borrow, Cow, ToOwned},
    cmp,
    convert::TryFrom,
    fmt,
    hash,
    iter,
    mem,
    num::NonZero,
    ops,
    slice,
    str,
    sync::Arc,
};

#[cfg(feature = "serde")]
use ::serde::{Deserialize, Serialize};

pub const SEP_CHAR: char = '.';
pub const SEP_STR: &'static str = ".";
pub const SEP_LEN: usize = SEP_STR.len();

pub trait AsFullId {
    type SegmentRef<'s>: AsRef<IdNameSeg> + 's
    where
        Self: 's;
    type SegmentIter<'s>: IntoIterator<Item = Self::SegmentRef<'s>>
    where
        Self: 's;
    fn segments(&self) -> Self::SegmentIter<'_>;

    fn id_len(&self) -> usize {
        self.segments()
            .into_iter()
            .enumerate()
            .map(|(i, seg)| seg.as_ref().as_str().len() + (i > 0).then_some(1).unwrap_or(0))
            .sum()
    }
    fn id_to_str(&self) -> Cow<'_, str> {
        Cow::Owned(FullIdOf::new(self).to_string())
    }
    fn id_starts_with(&self, prefix: impl AsRef<FullIdRef>) -> bool {
        full_id_starts_with(self, prefix.as_ref())
    }
    fn id_is_root(&self) -> bool {
        let mut segs = self.segments().into_iter();
        if segs.next().is_some() {
            segs.next().is_none()
        } else {
            // XXX: unclear if empty is or isn't..?
            // but we expect at least one segment atm so
            false
        }
    }
}
fn full_id_starts_with<I: ?Sized + AsFullId>(id: &I, prefix: &FullIdRef) -> bool {
    let mut segs = id.segments().into_iter();
    let mut prefix = prefix.segments().into_iter();
    while let Some(prefix) = prefix.next() {
        let prefix = prefix.as_ref();
        let seg = segs.next();
        if let Some(eq) = FullIdOf::name_iter_eq(seg.as_ref().map(AsRef::as_ref), Some(prefix)) {
            return eq
        }
    }
    true
}
impl<'a, T: ?Sized + AsFullId> AsFullId for &'a T {
    type SegmentRef<'s>
        = <T as AsFullId>::SegmentRef<'s>
    where
        T: 's,
        'a: 's;
    type SegmentIter<'s>
        = <T as AsFullId>::SegmentIter<'s>
    where
        T: 's,
        'a: 's;
    fn segments(&self) -> Self::SegmentIter<'_> {
        AsFullId::segments(*self)
    }
    fn id_len(&self) -> usize {
        AsFullId::id_len(*self)
    }
    fn id_to_str(&self) -> Cow<'_, str> {
        AsFullId::id_to_str(*self)
    }
}

/// TODO: newtype for multiple segments
pub type FullIdRef = IdNameSeg;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(transparent)]
pub struct IdNameSeg {
    pub segment: str,
}
impl IdNameSeg {
    pub fn from_str(segment: &str) -> &Self {
        unsafe { mem::transmute(segment) }
    }
    pub fn from_arcbox(segment: Arc<Box<str>>) -> Arc<Box<Self>> {
        unsafe { mem::transmute(segment) }
    }
    pub fn into_arcbox_str(segment: Arc<Box<Self>>) -> Arc<Box<str>> {
        unsafe { mem::transmute(segment) }
    }
    pub fn from_arc(segment: Arc<str>) -> Arc<Self> {
        unsafe { mem::transmute(segment) }
    }
    pub fn into_arc_str(self: Arc<Self>) -> Arc<str> {
        unsafe { mem::transmute(self) }
    }
    pub fn from_box(segment: Box<str>) -> Box<Self> {
        unsafe { mem::transmute(segment) }
    }
    pub fn into_box_str(self: Box<Self>) -> Box<str> {
        unsafe { mem::transmute(self) }
    }

    pub fn as_str(&self) -> &str {
        &self.segment
    }

    pub fn name(&self) -> &IdNameSeg {
        match self.as_str().rsplit_once(SEP_STR) {
            None => self.as_ref(),
            Some((_parent, name)) => IdNameSeg::from_str(name),
        }
    }
    pub fn parent(&self) -> Option<&FullIdRef> {
        let (parent, _) = self.as_str().rsplit_once(SEP_STR)?;
        Some(FullIdRef::from_str(parent))
    }
    pub fn ancestors(&self) -> impl Iterator<Item = &FullIdRef> + Clone {
        let mut parent = self.parent();
        iter::from_fn(move || {
            let next = parent.take()?;
            parent = next.parent();
            Some(next)
        })
    }
    #[inline]
    pub fn cow_from_str<'a>(id: &'a str) -> Cow<'a, IdNameSeg> {
        Cow::Borrowed(IdNameSeg::from_str(id))
    }
    pub fn from_str_cow<'a>(id: &Cow<'a, str>) -> Cow<'a, IdNameSeg> {
        match id {
            Cow::Borrowed(id) => Cow::Borrowed(IdNameSeg::from_str(id)),
            Cow::Owned(id) => Cow::Owned(IdNameBox::from(id)),
        }
    }
    pub fn cow_to_str<'a>(id: &Cow<'a, IdNameSeg>) -> Cow<'a, str> {
        match id {
            Cow::Borrowed(id) => Cow::Borrowed(id.as_str()),
            Cow::Owned(id) => Cow::Owned(id.as_str().into()),
        }
    }
}
impl fmt::Display for IdNameSeg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl<R: ?Sized + AsRef<IdNameSeg>> PartialEq<R> for IdNameSeg {
    fn eq(&self, rhs: &R) -> bool {
        self.as_str() == rhs.as_ref().as_str()
    }
}
impl Eq for IdNameSeg {}
impl<R: ?Sized + AsRef<IdNameSeg>> PartialOrd<R> for IdNameSeg {
    fn partial_cmp(&self, rhs: &R) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(rhs.as_ref().as_str())
    }
}
impl Ord for IdNameSeg {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        self.as_str().cmp(rhs.as_str())
    }
}
/// TODO: trailing prefix once [FullIdRef] is a real type and can be distinguished
/// (update other ID Hash impls to match)
impl hash::Hash for IdNameSeg {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write(self.as_str().as_bytes());
        state.write_u8(0xff);
        //state.write_u8(SEP_CHAR as u8);
    }
}
impl AsFullId for IdNameSeg {
    type SegmentRef<'s> = &'s IdNameSeg;
    type SegmentIter<'s> = iter::Map<str::Split<'s, &'static str>, fn(&'s str) -> &'s IdNameSeg>;
    fn segments(&self) -> Self::SegmentIter<'_> {
        self.as_str().split(SEP_STR).map(Self::from_str)
    }
    fn id_len(&self) -> usize {
        self.as_str().len()
    }
    fn id_to_str(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }
}
impl ToOwned for IdNameSeg {
    type Owned = IdNameBox;
    fn to_owned(&self) -> Self::Owned {
        IdNameBox::new_cloned(self)
    }
}
impl AsRef<str> for IdNameSeg {
    fn as_ref(&self) -> &str {
        &self.segment
    }
}
impl AsRef<IdNameSeg> for IdNameSeg {
    fn as_ref(&self) -> &IdNameSeg {
        self
    }
}
impl AsRef<IdNameSeg> for Arc<str> {
    fn as_ref(&self) -> &IdNameSeg {
        IdNameSeg::from_str(self)
    }
}
impl AsRef<IdNameSeg> for Box<str> {
    fn as_ref(&self) -> &IdNameSeg {
        IdNameSeg::from_str(self)
    }
}
impl AsRef<IdNameSeg> for str {
    fn as_ref(&self) -> &IdNameSeg {
        IdNameSeg::from_str(self)
    }
}
#[cfg(todo)]
impl From<String> for Box<IdNameSeg> {
    fn from(value: String) -> Self {
        IdNameSeg::new_box(value)
    }
}
#[cfg(todo)]
impl<'a> From<&'a str> for Box<IdNameSeg> {
    fn from(value: &'a str) -> Self {
        IdNameSeg::new_box(value)
    }
}
impl<'a> From<&'a str> for &'a IdNameSeg {
    fn from(value: &'a str) -> Self {
        IdNameSeg::from_str(value)
    }
}
impl From<Box<str>> for Box<IdNameSeg> {
    fn from(value: Box<str>) -> Self {
        IdNameSeg::from_box(value)
    }
}
impl<'a> From<&'a IdNameSeg> for &'a str {
    fn from(value: &'a IdNameSeg) -> Self {
        &value.segment
    }
}
impl<'a> From<&'a IdNameSeg> for Cow<'a, IdNameSeg> {
    fn from(value: &'a IdNameSeg) -> Self {
        Cow::Borrowed(value)
    }
}
impl<'a> From<&'a IdNameBox> for Cow<'a, IdNameSeg> {
    fn from(value: &'a IdNameBox) -> Self {
        Cow::Borrowed(value.as_id())
    }
}
impl<'a> From<IdNameBox> for Cow<'a, IdNameSeg> {
    fn from(value: IdNameBox) -> Self {
        Cow::Owned(value)
    }
}
impl<'a> From<&'a IdNameSeg> for String {
    fn from(value: &'a IdNameSeg) -> Self {
        value.segment.into()
    }
}

/// TODO: newtype that validates ascii+alphanum?
pub type IdStr = str;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct IdNameBox {
    /// TODO: `Arc<IdStr>>` may be a better measure...
    /// (alongside `Arc<CategoryId>` or `Box<CategoryId>`)
    pub name: Arc<Box<IdStr>>,
}
impl IdNameBox {
    pub fn new_cloned<N: AsRef<IdStr>>(name: N) -> Self {
        let name = name.as_ref();
        Self::with_arcbox(Arc::new(Box::from(name)))
    }
    pub fn with_arcbox(name: Arc<Box<IdStr>>) -> Self {
        Self { name }
    }
    pub fn with_arcbox_ref(name: &Arc<Box<IdStr>>) -> &Self {
        unsafe { mem::transmute(name) }
    }
    pub fn as_str(&self) -> &IdStr {
        &**self.name
    }
    pub fn as_id(&self) -> &FullIdRef {
        FullIdRef::from_str(self.as_str())
    }
    pub fn as_name(&self) -> &IdNameSeg {
        self.as_id().as_ref()
    }
}
impl AsFullId for IdNameBox {
    type SegmentRef<'s> = <FullIdRef as AsFullId>::SegmentRef<'s>;
    type SegmentIter<'s> = <FullIdRef as AsFullId>::SegmentIter<'s>;
    fn segments(&self) -> Self::SegmentIter<'_> {
        self.as_id().segments()
    }
    fn id_len(&self) -> usize {
        self.as_id().id_len()
    }
    fn id_to_str(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }
}
impl fmt::Display for IdNameBox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_id(), f)
    }
}
impl hash::Hash for IdNameBox {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_id().hash(state);
        //state.write_u8(0xff);
    }
}
impl ops::Index<ops::RangeFull> for IdNameBox {
    type Output = IdStr;
    fn index(&self, _: ops::RangeFull) -> &Self::Output {
        self.as_ref()
    }
}
impl AsRef<IdNameBox> for IdNameBox {
    fn as_ref(&self) -> &IdNameBox {
        self
    }
}
impl AsRef<IdNameSeg> for IdNameBox {
    fn as_ref(&self) -> &IdNameSeg {
        self.as_name()
    }
}
impl AsRef<str> for IdNameBox {
    fn as_ref(&self) -> &str {
        self.as_name().as_ref()
    }
}
impl Borrow<IdNameSeg> for IdNameBox {
    fn borrow(&self) -> &IdNameSeg {
        self.as_ref()
    }
}
impl Borrow<str> for IdNameBox {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}
impl From<String> for IdNameBox {
    fn from(name: String) -> IdNameBox {
        Self::new_cloned(name)
    }
}
impl From<&'_ String> for IdNameBox {
    fn from(name: &String) -> IdNameBox {
        name[..].into()
    }
}
impl From<&'_ str> for IdNameBox {
    fn from(name: &str) -> IdNameBox {
        Self::new_cloned(name)
    }
}
impl From<Box<IdStr>> for IdNameBox {
    fn from(name: Box<IdStr>) -> IdNameBox {
        Self::with_arcbox(Arc::new(name))
    }
}
impl From<Arc<Box<IdStr>>> for IdNameBox {
    fn from(name: Arc<Box<IdStr>>) -> IdNameBox {
        Self::with_arcbox(name)
    }
}

#[derive(Debug, Clone)]
pub struct CategoryId<T: ?Sized = IdNameBox> {
    len: NonZero<u16>,
    full_id: T,
}
impl<T: Sized + AsRef<IdStr>> CategoryId<T> {
    pub fn new<S: Into<T>>(full_id: S, len: usize) -> Option<Self> {
        let full_id = full_id.into();
        match len <= full_id.as_ref().len() {
            true if len > 0 => Some(unsafe { Self::new_unchecked(full_id, len) }),
            _ => None,
        }
    }
    pub unsafe fn new_unchecked<S: Into<T>>(full_id: S, len: usize) -> Self {
        Self {
            len: NonZero::new_unchecked(len as u16),
            full_id: full_id.into(),
        }
    }
    /// panic!
    pub fn with_full_id<S: Into<T>>(full_id: S) -> Self {
        Self::try_with_full_id(full_id).expect(Self::WITH_FULL_ID_ERR)
    }
    pub fn try_with_full_id<S: Into<T>>(full_id: S) -> Option<Self> {
        let full_id = full_id.into();
        let len = full_id.as_ref().len();
        debug_assert!(len <= u16::MAX as usize);
        if len == 0 {
            return None
        }
        Some(unsafe { Self::new_unchecked(full_id, len) })
    }

    pub fn get_parent(&self) -> Option<CategoryId<T>>
    where
        T: Clone,
    {
        match self.as_id().parent() {
            None => None,
            Some(id) => Some({
                let len = id.as_str().len();
                debug_assert!(len < self.len());
                unsafe { Self::new_unchecked(self.full_id.clone(), len) }
            }),
        }
    }
}
impl<T: ?Sized> CategoryId<T> {
    pub const WITH_FULL_ID_ERR: &'static str = "category ID empty or long";

    pub fn len(&self) -> usize {
        self.len.get() as usize
    }

    pub fn inner(&self) -> &T {
        &self.full_id
    }
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.full_id
    }
}
impl<T: ?Sized + AsFullId> CategoryId<T> {
    pub fn is_full_id(&self) -> bool {
        self.len() == self.full_id.id_len()
    }
    pub fn as_full_id(&self) -> Option<&T> {
        self.is_full_id().then_some(&self.full_id)
    }
}
impl<T: ?Sized + AsRef<IdStr>> CategoryId<T> {
    pub fn as_str(&self) -> &str {
        let id = self.full_id.as_ref();
        unsafe { id.get_unchecked(..self.len()) }
    }
    pub fn as_id(&self) -> &FullIdRef {
        FullIdRef::from_str(self.as_str())
    }
    #[cfg(todo)]
    pub fn as_ref(&self) -> IdRef<'_> {
        IdRef::with_full_id_ref(&self.full_id)
    }
}
impl<T: ?Sized + AsFullId + AsRef<IdNameBox> + AsRef<IdStr>> CategoryId<T> {
    pub fn to_id_box(&self) -> Cow<'_, IdNameSeg> {
        match self.as_full_id() {
            Some(id) => {
                let id: &IdNameBox = id.as_ref();
                Cow::Owned(id.clone())
            },
            None => Cow::Borrowed(self.as_id()),
        }
    }
}
impl<T: ?Sized> fmt::Display for CategoryId<T>
where
    Self: AsFullId,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&FullIdOf::new(self), f)
    }
}
impl<T: ?Sized + AsFullId> AsFullId for CategoryId<T>
where
    for<'s> CategoryIdIterator<<T as AsFullId>::SegmentIter<'s>>: Iterator,
    for<'s> <CategoryIdIterator<<T as AsFullId>::SegmentIter<'s>> as Iterator>::Item: AsRef<IdNameSeg>,
{
    //type SegmentRef<'s> = <T as AsFullId>::SegmentRef<'s> where T: 's;
    type SegmentRef<'s>
        = <Self::SegmentIter<'s> as Iterator>::Item
    where
        T: 's;
    type SegmentIter<'s>
        = CategoryIdIterator<<T as AsFullId>::SegmentIter<'s>>
    where
        T: 's;
    fn segments(&self) -> Self::SegmentIter<'_> {
        let len = self.len();
        CategoryIdIterator { inner: self.full_id.segments(), len }
    }

    fn id_len(&self) -> usize {
        self.len()
    }
    fn id_to_str(&self) -> Cow<'_, str> {
        let len = self.len();
        match self.full_id.id_to_str() {
            Cow::Borrowed(s) => Cow::Borrowed(s.get(..len).unwrap_or(s)),
            Cow::Owned(mut s) => {
                if s.len() > len {
                    s.truncate(len);
                }
                Cow::Owned(s)
            },
        }
    }
}
impl<T: AsRef<IdStr>> TryFrom<Cow<'_, str>> for CategoryId<T>
where
    for<'a> T: From<&'a str>,
{
    type Error = anyhow::Error;

    fn try_from(id: Cow<str>) -> Result<Self, Self::Error> {
        Self::try_with_full_id(&id[..]).ok_or_else(|| anyhow::Error::msg(Self::WITH_FULL_ID_ERR))
    }
}
impl<'a, T: ?Sized> From<&'a CategoryId<T>> for Cow<'a, FullIdRef>
where
    T: AsFullId + AsRef<IdNameBox> + AsRef<IdStr>,
{
    #[inline]
    fn from(id: &'a CategoryId<T>) -> Self {
        match id {
            #[cfg(todo)]
            id => id.to_id_box(),
            id => Cow::Borrowed(id.as_id()),
        }
    }
}
impl<'a, T> From<CategoryId<T>> for Cow<'a, FullIdRef>
where
    T: AsFullId + AsRef<IdNameBox> + AsRef<IdStr>,
{
    #[inline]
    fn from(id: CategoryId<T>) -> Self {
        Cow::Owned(id.to_id_box().into_owned())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CategoryIdIterator<T> {
    inner: T,
    len: usize,
}
impl<'a, T, I> Iterator for CategoryIdIterator<T>
where
    T: Iterator<Item = &'a I>,
    I: ?Sized + AsRef<IdNameSeg> + 'a,
    // TODO: <T as Iterator>::Item: AsRef<IdNameSeg>,
{
    #[cfg(todo)]
    type Item = <T as Iterator>::Item;
    type Item = &'a IdNameSeg;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None
        }
        let seg = match self.inner.next() {
            Some(seg) => seg,
            None => {
                self.len = 0;
                return None
            },
        };
        let id_seg = AsRef::<IdNameSeg>::as_ref(seg);
        let len = id_seg.as_str().len();
        let prev_len = self.len;
        self.len = prev_len.saturating_sub(len + SEP_LEN);
        let seg_len = len.min(prev_len);
        Some(unsafe { IdNameSeg::from_str(id_seg.segment.get_unchecked(..seg_len)) })
    }
}
impl<T: iter::FusedIterator> iter::FusedIterator for CategoryIdIterator<T> where Self: Iterator {}
impl<T: ?Sized + AsRef<IdStr>, R: ?Sized + AsRef<FullIdRef>> PartialOrd<R> for CategoryId<T> {
    fn partial_cmp(&self, rhs: &R) -> Option<cmp::Ordering> {
        self.as_id().partial_cmp(rhs.as_ref())
    }
}
impl<T: ?Sized + AsRef<IdStr>> Ord for CategoryId<T> {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        self.as_id().cmp(rhs.as_id())
    }
}
impl<T: ?Sized + AsRef<IdStr>, R: ?Sized + AsRef<FullIdRef>> PartialEq<R> for CategoryId<T> {
    fn eq(&self, rhs: &R) -> bool {
        self.as_id() == rhs.as_ref()
    }
}
impl<T: ?Sized + AsRef<IdStr>> hash::Hash for CategoryId<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_id().hash(state);
        //state.write_u8(0xff);
    }
}
impl<T: ?Sized + AsRef<IdStr>> Eq for CategoryId<T> {}
impl CategoryId {
    /// if you really need a default...
    pub fn invalid_singleton() -> &'static Self {
        use std::sync::LazyLock;
        static INVALID_CATEGORY_ID: LazyLock<CategoryId> =
            LazyLock::new(|| CategoryId::with_full_id(SEP_STR));
        &*INVALID_CATEGORY_ID
    }
}
impl<T: ?Sized + AsRef<IdStr>> ops::Deref for CategoryId<T> {
    type Target = FullIdRef;
    fn deref(&self) -> &Self::Target {
        self.as_id()
    }
}
#[cfg(todo)]
impl From<String> for CategoryId {
    fn from(value: String) -> Self {
        assert!(!value.is_empty());
        Self::with_full_id(value)
    }
}
#[cfg(todo)]
impl From<&'_ String> for CategoryId {
    fn from(value: &String) -> Self {
        value[..].into()
    }
}
#[cfg(todo)]
impl<'a> From<&'a str> for CategoryId {
    fn from(value: &'a str) -> Self {
        assert!(!value.is_empty());
        Self::with_full_id(value)
    }
}
#[cfg(todo)]
impl From<Arc<str>> for CategoryId {
    fn from(value: Arc<str>) -> Self {
        Self::with_full_id(value)
    }
}
#[cfg(todo)]
impl<'a> From<&'a Arc<str>> for CategoryId {
    fn from(value: &'a Arc<str>) -> Self {
        Self::with_full_id(value.clone())
    }
}
impl From<Box<str>> for CategoryId {
    fn from(value: Box<str>) -> Self {
        Self::with_full_id(value)
    }
}
impl<T> From<CategoryId<T>> for String
where
    CategoryId<T>: fmt::Display,
{
    fn from(value: CategoryId<T>) -> Self {
        Into::into(&value)
    }
}
impl<T: ?Sized> From<&'_ CategoryId<T>> for String
where
    CategoryId<T>: fmt::Display,
{
    fn from(value: &CategoryId<T>) -> Self {
        value.to_string()
    }
}
impl<'a, T: ?Sized> From<&'a CategoryId<T>> for &'a str
where
    CategoryId<T>: AsRef<str>,
{
    fn from(value: &'a CategoryId<T>) -> Self {
        value.as_ref()
    }
}
impl<T: ?Sized + AsRef<IdStr>> AsRef<IdStr> for CategoryId<T> {
    fn as_ref(&self) -> &IdStr {
        self.as_str()
    }
}
impl<T: ?Sized + AsRef<IdStr>> AsRef<FullIdRef> for CategoryId<T> {
    fn as_ref(&self) -> &FullIdRef {
        self.as_id()
    }
}
#[cfg(todo)]
impl<T: ?Sized + AsRef<IdStr>> AsRef<IdNameSeg> for CategoryId<T> {
    fn as_ref(&self) -> &IdNameSeg {
        IdNameSeg::from_str(self.as_ref())
    }
}
impl<T: ?Sized + AsRef<IdStr>> Borrow<IdStr> for CategoryId<T> {
    fn borrow(&self) -> &IdStr {
        self.as_ref()
    }
}
impl<T: ?Sized + AsRef<IdStr>> Borrow<FullIdRef> for CategoryId<T> {
    fn borrow(&self) -> &FullIdRef {
        self.as_ref()
    }
}
impl<T: ?Sized + AsRef<IdStr>> Borrow<IdStr> for &'_ CategoryId<T> {
    fn borrow(&self) -> &IdStr {
        Borrow::borrow(*self)
    }
}
impl<T: ?Sized + AsRef<IdStr>> Borrow<FullIdRef> for &'_ CategoryId<T> {
    fn borrow(&self) -> &FullIdRef {
        self.as_ref()
    }
}
impl ops::Index<ops::RangeFull> for CategoryId {
    type Output = IdStr;
    fn index(&self, _: ops::RangeFull) -> &Self::Output {
        self.as_ref()
    }
}
impl AsFullId for IdStr {
    /// TODO: &IdNameSeg?
    type SegmentRef<'s> = &'s str;
    type SegmentIter<'s> = str::Split<'s, &'static str>;
    fn segments(&self) -> Self::SegmentIter<'_> {
        self.split(SEP_STR)
    }
    fn id_len(&self) -> usize {
        self.len()
    }
    fn id_to_str(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }
}

#[cfg(todo)]
#[derive(Debug, Clone, Hash)]
pub enum IdRef<'i> {
    FullIdRef(&'i Arc<str>),
    FullId(CategoryId),
}
#[cfg(todo)]
impl<'i> IdRef<'i> {
    pub fn with_full_id_ref(id: &'i Arc<str>) -> Self {
        Self::FullIdRef(id)
    }
    pub fn with_full_id(id: CategoryId) -> Self {
        Self::FullId(id)
    }
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        match self {
            Self::FullIdRef(full_id) => full_id[..].split('.'),
            Self::FullId(full_id) => full_id.full_id[..].split('.'),
        }
    }
    pub fn into_owned(self) -> FullIdOf<'static> {
        match self {
            Self::FullIdRef(full_id) => IdRef::with_full_id(full_id.into()),
            Self::FullId(full_id) => IdRef::with_full_id(full_id),
        }
    }
}
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(transparent)]
pub struct FullIdOf<T: ?Sized + AsFullId> {
    pub id: T,
}
impl<T: AsFullId> FullIdOf<T> {
    pub const fn new(id: T) -> Self {
        Self { id }
    }
}

impl<T: ?Sized + AsFullId> FullIdOf<T> {
    pub const fn with_ref(id: &T) -> &Self {
        unsafe { mem::transmute(id) }
    }

    pub fn with_mut(id: &mut T) -> &mut Self {
        unsafe { mem::transmute(id) }
    }

    pub fn cmp_with<R: ?Sized + AsFullId>(&self, rhs: &R) -> cmp::Ordering {
        let mut segs = self.segments().into_iter();
        let mut rhs = rhs.segments().into_iter();
        loop {
            let seg = segs.next();
            let rhs = rhs.next();
            let ord =
                FullIdOf::name_iter_cmp(seg.as_ref().map(AsRef::as_ref), rhs.as_ref().map(AsRef::as_ref));
            if let Some(ord) = ord {
                break ord
            }
        }
    }
    pub fn eq_with<R: ?Sized + AsFullId>(&self, rhs: &R) -> bool {
        let mut segs = self.segments().into_iter();
        let mut rhs = rhs.segments().into_iter();
        loop {
            let seg = segs.next();
            let rhs = rhs.next();
            let eq =
                FullIdOf::name_iter_eq(seg.as_ref().map(AsRef::as_ref), rhs.as_ref().map(AsRef::as_ref));
            if let Some(eq) = eq {
                break eq
            }
        }
    }
}
impl FullIdOf<FullIdRef> {
    fn name_iter_eq(seg: Option<&IdNameSeg>, rhs: Option<&IdNameSeg>) -> Option<bool> {
        match (seg, rhs) {
            (Some(seg), Some(rhs)) if seg == rhs => None,
            (None, None) => Some(true),
            _ => Some(false),
        }
    }
    fn name_iter_eq_relaxed(seg: Option<&IdNameSeg>, rhs: Option<&IdNameSeg>) -> Option<bool> {
        match (seg, rhs) {
            (Some(seg), Some(rhs)) if seg.as_str().eq_ignore_ascii_case(rhs.as_str()) => None,
            (None, None) => Some(true),
            _ => Some(false),
        }
    }
    fn name_iter_cmp(seg: Option<&IdNameSeg>, rhs: Option<&IdNameSeg>) -> Option<cmp::Ordering> {
        match (seg, rhs) {
            #[cfg(todo)]
            (Some(seg), Some(rhs)) if seg == rhs => None,
            (Some(seg), Some(rhs)) => match seg.cmp(rhs) {
                // TODO: skip this is we can intern and compare pointers someday...
                cmp::Ordering::Equal => None,
                ord => Some(ord),
            },
            (seg, rhs) => Some(seg.is_some().cmp(&rhs.is_some())),
        }
    }
    fn name_iter_cmp_relaxed(seg: Option<&IdNameSeg>, rhs: Option<&IdNameSeg>) -> Option<cmp::Ordering> {
        match (seg, rhs) {
            #[cfg(todo)]
            (Some(seg), Some(rhs)) if seg == rhs => None,
            // TODO: skip this is we can intern and compare pointers someday...
            (Some(seg), Some(rhs)) => {
                // zip truncates, so add a trailing 0
                let seg = seg.as_str().bytes().chain(iter::once(0));
                let rhs = rhs.as_str().bytes().chain(iter::once(0));
                let cmp = seg.zip(rhs).find_map(|(l, r)| {
                    match l.to_ascii_lowercase().cmp(&r.to_ascii_lowercase()) {
                        cmp::Ordering::Equal => None,
                        ord => Some(ord),
                    }
                });
                cmp
            },
            (seg, rhs) => Some(seg.is_some().cmp(&rhs.is_some())),
        }
    }
}
impl<T: ?Sized + AsFullId> AsFullId for FullIdOf<T> {
    type SegmentRef<'s>
        = <T as AsFullId>::SegmentRef<'s>
    where
        T: 's;
    type SegmentIter<'s>
        = <T as AsFullId>::SegmentIter<'s>
    where
        T: 's;
    fn segments(&self) -> Self::SegmentIter<'_> {
        self.id.segments()
    }
    fn id_len(&self) -> usize {
        self.id.id_len()
    }
    fn id_to_str(&self) -> Cow<'_, str> {
        self.id.id_to_str()
    }
}
impl<T: ?Sized + AsFullId> fmt::Display for FullIdOf<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, seg) in self.segments().into_iter().enumerate() {
            let seg = seg.as_ref();
            if i > 0 {
                f.write_str(SEP_STR)?;
            }
            f.write_str(seg.as_str())?;
        }
        Ok(())
    }
}
impl<T: ?Sized + AsFullId, R: ?Sized + AsFullId> PartialEq<R> for FullIdOf<T> {
    fn eq(&self, rhs: &R) -> bool {
        self.eq_with(rhs)
    }
}
impl<T: ?Sized + AsFullId> Eq for FullIdOf<T> {}
impl<T: ?Sized + AsFullId, R: ?Sized + AsFullId> PartialOrd<R> for FullIdOf<T> {
    fn partial_cmp(&self, rhs: &R) -> Option<cmp::Ordering> {
        Some(self.cmp_with(rhs))
    }
}
impl<T: ?Sized + AsFullId> Ord for FullIdOf<T> {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        self.cmp_with(rhs)
    }
}
/// TODO: combine this with the Display impl
impl<T: ?Sized + AsFullId> hash::Hash for FullIdOf<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        for (i, seg) in self.segments().into_iter().enumerate() {
            let seg = seg.as_ref();
            if i > 0 {
                state.write(SEP_STR.as_bytes());
            }
            state.write(seg.as_str().as_bytes());
        }
        // as write_str typically would...
        state.write_u8(0xff);
    }
}

/// case-insensitive comparisons
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(transparent)]
pub struct IdCmpRelaxed<T: ?Sized> {
    pub id: T,
}
impl<T> IdCmpRelaxed<T> {
    pub const fn new(id: T) -> Self {
        Self { id }
    }
}
impl<T: ?Sized> IdCmpRelaxed<T> {
    #[inline]
    pub const fn with_ref(id: &T) -> &Self {
        unsafe { mem::transmute(id) }
    }

    #[inline]
    pub fn with_mut(id: &mut T) -> &mut Self {
        unsafe { mem::transmute(id) }
    }

    #[inline]
    pub fn id_ref<U: ?Sized>(&self) -> &IdCmpRelaxed<U>
    where
        T: AsRef<U>,
    {
        IdCmpRelaxed::with_ref(self.id.as_ref())
    }
    #[inline]
    pub fn id_mut<U: ?Sized>(&mut self) -> &mut IdCmpRelaxed<U>
    where
        T: AsMut<U>,
    {
        IdCmpRelaxed::with_mut(self.id.as_mut())
    }
    pub fn as_canon_id(&self) -> Option<&FullIdRef>
    where
        T: AsRef<FullIdRef> + AsFullId,
    {
        self.is_canon().then_some(self.id.as_ref())
    }
    pub fn to_id_box(&self) -> Cow<'_, FullIdRef>
    where
        T: AsRef<FullIdRef> + AsFullId,
    {
        let id = self.id_ref();
        match self.is_canon() {
            true => Cow::Borrowed(id.as_ref()),
            false => Cow::Owned(IdNameBox::new_cloned(id)),
        }
    }
    pub fn cow_from<'a>(id: Cow<'a, T>) -> Cow<'a, Self>
    where
        T: ToOwned,
        Self: ToOwned,
        <T as ToOwned>::Owned: Into<<Self as ToOwned>::Owned>,
    {
        match id {
            Cow::Borrowed(id) => Cow::Borrowed(IdCmpRelaxed::with_ref(id)),
            Cow::Owned(id) => Cow::Owned(id.into()),
        }
    }
    pub fn cow_into<'a>(id: Cow<'a, Self>) -> Cow<'a, T>
    where
        T: ToOwned,
        Self: ToOwned,
        <Self as ToOwned>::Owned: Into<<T as ToOwned>::Owned>,
    {
        match id {
            #[cfg(todo)]
            id => Self::cow_of_into::<T>(id),
            Cow::Borrowed(id) => Cow::Borrowed(&id.id),
            Cow::Owned(id) => Cow::Owned(id.into()),
        }
    }
    pub fn cow_of_into<'a, U: ?Sized>(id: Cow<'a, Self>) -> Cow<'a, U>
    where
        T: AsRef<U>,
        U: ToOwned,
        Self: ToOwned,
        <Self as ToOwned>::Owned: Into<<U as ToOwned>::Owned>,
    {
        match id {
            Cow::Borrowed(id) => Cow::Borrowed(id.id.as_ref()),
            Cow::Owned(id) => Cow::Owned(id.into()),
        }
    }
}

impl<T: ?Sized + AsFullId> IdCmpRelaxed<T> {
    pub fn as_canon(&self) -> Option<&T> {
        self.is_canon().then_some(&self.id)
    }
    pub fn is_canon(&self) -> bool {
        self.id.segments().into_iter().all(|seg| {
            let seg = seg.as_ref();
            seg.as_str()
                .bytes()
                .all(|c| !c.is_ascii_uppercase() /*&& c.is_ascii()*/)
        })
    }

    pub fn cmp_with<R: ?Sized + AsFullId>(&self, rhs: &R) -> cmp::Ordering {
        let mut segs = self.id.segments().into_iter();
        let mut rhs = rhs.segments().into_iter();
        loop {
            let seg = segs.next();
            let rhs = rhs.next();
            let ord = FullIdOf::name_iter_cmp_relaxed(
                seg.as_ref().map(AsRef::as_ref),
                rhs.as_ref().map(AsRef::as_ref),
            );
            if let Some(ord) = ord {
                break ord
            }
        }
    }
    pub fn eq_with_or_prefix<R: ?Sized + AsFullId>(&self, rhs: &R) -> Option<bool> {
        let mut segs = self.id.segments().into_iter();
        let mut rhs = rhs.segments().into_iter();
        loop {
            let seg = segs.next();
            let rhs = rhs.next();
            let eq = FullIdOf::name_iter_eq_relaxed(
                seg.as_ref().map(AsRef::as_ref),
                rhs.as_ref().map(AsRef::as_ref),
            );
            match eq {
                Some(false) if seg.is_none() => break None,
                Some(eq) => break Some(eq),
                None => (),
            }
        }
    }
    pub fn eq_with<R: ?Sized + AsFullId>(&self, rhs: &R) -> bool {
        self.eq_with_or_prefix(rhs).unwrap_or(false)
    }
    pub fn starts_with<I: ?Sized + AsFullId>(&self, prefix: &I) -> bool {
        self.eq_with_or_prefix(prefix).unwrap_or(true)
    }
}
impl<T: ?Sized + AsFullId> AsFullId for IdCmpRelaxed<T> {
    type SegmentRef<'s>
        = <T as AsFullId>::SegmentRef<'s>
    where
        T: 's;
    type SegmentIter<'s>
        = <T as AsFullId>::SegmentIter<'s>
    where
        T: 's;
    fn segments(&self) -> Self::SegmentIter<'_> {
        self.id.segments()
    }
    fn id_len(&self) -> usize {
        self.id.id_len()
    }
    fn id_to_str(&self) -> Cow<'_, str> {
        self.id.id_to_str()
    }
    fn id_starts_with(&self, prefix: impl AsRef<FullIdRef>) -> bool {
        #[cfg(todo)]
        if is_id::<T>() {
            self.id_ref().starts_with(prefix)
        }
        self.starts_with(prefix.as_ref())
    }
}
/// TODO: `[u8]::split_inclusive` if ascii already guaranteed
impl<T: ?Sized + AsFullId> fmt::Display for IdCmpRelaxed<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, seg) in self.id.segments().into_iter().enumerate() {
            if i > 0 {
                f.write_str(SEP_STR)?;
            }
            let seg = seg.as_ref();
            for s in seg.as_str().split_inclusive(|c: char| c.is_ascii_uppercase()) {
                let (upper, s) = match s.as_bytes().split_last() {
                    Some((upper, s)) => (upper, unsafe { str::from_utf8_unchecked(s) }),
                    None => continue,
                };
                f.write_str(s)?;
                let lower = upper.to_ascii_lowercase();
                let lower = unsafe { str::from_utf8_unchecked(slice::from_ref(&lower)) };
                f.write_str(lower)?;
            }
        }
        Ok(())
    }
}
impl<T: ?Sized + AsFullId, R: ?Sized + AsFullId> PartialEq<R> for IdCmpRelaxed<T> {
    fn eq(&self, rhs: &R) -> bool {
        self.eq_with(rhs)
    }
}
impl<T: ?Sized + AsFullId> Eq for IdCmpRelaxed<T> {}
impl<T: ?Sized + AsFullId, R: ?Sized + AsFullId> PartialOrd<R> for IdCmpRelaxed<T> {
    fn partial_cmp(&self, rhs: &R) -> Option<cmp::Ordering> {
        Some(self.cmp_with(rhs))
    }
}
impl<T: ?Sized + AsFullId> Ord for IdCmpRelaxed<T> {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        self.cmp_with(&rhs.id)
    }
}
/// TODO: combine this with the Display impl
impl<T: ?Sized + AsFullId> hash::Hash for IdCmpRelaxed<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        for (i, seg) in self.id.segments().into_iter().enumerate() {
            if i > 0 {
                state.write(SEP_STR.as_bytes());
            }
            let seg = seg.as_ref();
            for s in seg
                .as_str()
                .as_bytes()
                .split_inclusive(|c| c.is_ascii_uppercase())
            {
                if let Some((upper, s)) = s.split_last() {
                    state.write(s);
                    state.write_u8(upper.to_ascii_lowercase());
                }
            }
            state.write_u8(0xff);
        }
    }
}
impl<T: ?Sized> Borrow<IdCmpRelaxed<FullIdRef>> for IdCmpRelaxed<CategoryId<T>>
where
    CategoryId<T>: Borrow<FullIdRef>,
{
    fn borrow(&self) -> &IdCmpRelaxed<FullIdRef> {
        IdCmpRelaxed::with_ref(self.id.borrow())
    }
}
impl Borrow<IdCmpRelaxed<FullIdRef>> for IdCmpRelaxed<IdNameBox> {
    fn borrow(&self) -> &IdCmpRelaxed<FullIdRef> {
        IdCmpRelaxed::with_ref(self.id.borrow())
    }
}
impl<U: ?Sized, T: ?Sized> AsRef<U> for IdCmpRelaxed<T>
where
    T: AsRef<U>,
{
    fn as_ref(&self) -> &U {
        self.id.as_ref()
    }
}
impl<'a> From<&'a FullIdRef> for &'a IdCmpRelaxed<FullIdRef> {
    fn from(id: &'a FullIdRef) -> Self {
        IdCmpRelaxed::with_ref(id)
    }
}
impl<'a> From<&'a IdStr> for &'a IdCmpRelaxed<IdStr> {
    fn from(id: &'a IdStr) -> Self {
        IdCmpRelaxed::with_ref(id)
    }
}
impl From<IdNameBox> for IdCmpRelaxed<IdNameBox> {
    fn from(id: IdNameBox) -> Self {
        IdCmpRelaxed::new(id)
    }
}
impl From<IdCmpRelaxed<IdNameBox>> for IdNameBox {
    fn from(id: IdCmpRelaxed<IdNameBox>) -> Self {
        id.id
    }
}
impl<T> From<CategoryId<T>> for IdCmpRelaxed<CategoryId<T>> {
    fn from(id: CategoryId<T>) -> Self {
        IdCmpRelaxed::new(id)
    }
}
impl From<IdNameBox> for IdCmpRelaxed<CategoryId<IdNameBox>> {
    fn from(id: IdNameBox) -> Self {
        IdCmpRelaxed::new(CategoryId::with_full_id(id))
    }
}
impl<T> From<IdCmpRelaxed<CategoryId<T>>> for CategoryId<T> {
    fn from(id: IdCmpRelaxed<CategoryId<T>>) -> Self {
        id.id
    }
}
impl ToOwned for IdCmpRelaxed<IdNameSeg> {
    type Owned = IdCmpRelaxed<<IdNameSeg as ToOwned>::Owned>;
    fn to_owned(&self) -> Self::Owned {
        IdCmpRelaxed::new(self.id.to_owned())
    }
}

#[cfg(feature = "serde")]
pub mod serde {
    use {
        super::{CategoryId, FullIdRef, IdNameBox, IdStr},
        serde::{de, ser},
        std::{borrow::Cow, sync::Arc},
    };

    impl<'de> de::Deserialize<'de> for IdNameBox {
        #[inline]
        fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            de::Deserialize::deserialize(d).map(|name| IdNameBox::with_arcbox(Arc::new(name)))
        }
    }
    impl ser::Serialize for IdNameBox {
        #[inline]
        fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            ser::Serialize::serialize(&*self.name, s)
        }
    }

    impl<T: ?Sized + AsRef<IdStr>> ser::Serialize for CategoryId<T> {
        fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            self.as_str().serialize(s)
        }
    }
    impl<'de, T: AsRef<IdStr>> de::Deserialize<'de> for CategoryId<T>
    where
        Self: TryFrom<Cow<'de, str>>,
    {
        fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            <Cow<str> as de::Deserialize>::deserialize(d)
                .and_then(|id| Self::try_from(id).map_err(|_| de::Error::custom(Self::WITH_FULL_ID_ERR)))
        }
    }
    pub fn deserialize_cow<'de, D: de::Deserializer<'de>>(d: D) -> Result<Cow<'de, FullIdRef>, D::Error> where
    {
        <Cow<str> as de::Deserialize>::deserialize(d).map(|id| FullIdRef::from_str_cow(&id))
    }

    pub mod relaxed {
        use {
            super::super::{FullIdRef, IdCmpRelaxed, IdStr},
            serde::{de, ser},
            std::borrow::Cow,
        };

        pub fn serialize<T, S>(id: &T, s: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
            T: ?Sized + AsRef<IdStr>,
        {
            let id = IdCmpRelaxed::new(id.as_ref());
            match id.as_canon_id() {
                Some(id) => ser::Serialize::serialize(id, s),
                None => ser::Serialize::serialize(&format_args!("{id}"), s),
            }
        }
        pub fn deserialize<'de, T, D>(d: D) -> Result<T, D::Error>
        where
            T: AsRef<IdStr> + From<Cow<'de, FullIdRef>>,
            D: de::Deserializer<'de>,
        {
            <Cow<str> as de::Deserialize>::deserialize(d)
                .map(|id| FullIdRef::from_str_cow(&id))
                .map(IdCmpRelaxed::cow_from)
                .map(IdCmpRelaxed::cow_into)
                .map(T::from)
        }
    }
}
