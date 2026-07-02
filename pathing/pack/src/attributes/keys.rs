use {
    super::{AttrList, TacoBehavior, MapType},
    crate::{
        attributes::{AttrString, BounceBehavior, CullDirection},
        category::id,
    },
    anyhow::Context,
    base64::Engine as _,
    glam::{Vec3, Vec4},
    std::{
        borrow::{Borrow, Cow},
        convert::Infallible,
        fmt,
        io,
        mem,
        num::NonZero,
        ops,
        slice,
        str::FromStr,
        sync::Arc,
        time::Duration,
    },
    uuid::Uuid,
};

// TODO: FromStr, Display
pub trait AttrKey: fmt::Debug + Clone {
    type Storage: fmt::Debug + Clone
    where
        Self: Sized;

    const ATTR: &'static str;
    const ATTR_NAMES: &'static [&'static str];

    fn is_plain_attr(attr: &str) -> bool {
        Self::ATTR_NAMES
            .iter()
            .any(|alias| attr.eq_ignore_ascii_case(alias))
    }
    /// hack to allow "specialization" of the blanket impl for [super::cell::AttrKeyValue::pack_key_of]
    ///
    /// implement using a static lazy lock or something to avoid more expensive typeid lookups
    #[doc(hidden)]
    fn __pack_key_of() -> super::cell::PackKeyId
    where
        Self: Sized + super::cell::AttrKeyValue,
    {
        super::cell::PackKeyId::for_type::<Self>()
    }
}

#[cfg(todo)]
#[derive(Debug, Clone, Default)]
pub struct File(pub Arc<Box<str>>);
#[cfg(todo)]
impl File {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[cfg(todo)]
impl FromStr for File {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Arc::new(value.into())))
    }
}
#[cfg(todo)]
impl<S: AsRef<str>> From<S> for File {
    fn from(file: S) -> File {
        Self(Arc::new(file.as_ref().into()))
    }
}
#[cfg(todo)]
impl<S: AsRef<str>> From<S> for IconFile {
    fn from(file: S) -> Self {
        Self(file.into())
    }
}
#[cfg(todo)]
impl<S: AsRef<str>> From<S> for TextureFile {
    fn from(file: S) -> Self {
        Self(file.into())
    }
}
pub type File = AttrString;

#[cfg(todo)]
#[derive(Debug, Clone, Default)]
pub struct Script(pub AttrString);
#[cfg(todo)]
impl From<AttrString> for Script {
    fn from(s: AttrString) -> Self {
        Self(s.into())
    }
}
#[cfg(todo)]
impl FromStr for Script {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(super::string_into(value)))
    }
}
pub type Script = AttrString;

#[derive(Debug, Copy, Clone, Default)]
pub struct Bool(pub bool);

impl Bool {
    pub const TRUE: Self = Self(true);
    pub const FALSE: Self = Self(false);

    #[inline]
    pub const fn from_ref(v: &bool) -> &Self {
        unsafe { mem::transmute(v) }
    }
}

impl FromStr for Bool {
    type Err = io::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            value if value.eq_ignore_ascii_case("true") => Ok(Self::TRUE),
            value if value.eq_ignore_ascii_case("false") => Ok(Self::FALSE),
            value => value.parse::<i32>().map(Self::from).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, format!("unexpected bool {value:?}"))
            }),
        }
    }
}

impl From<bool> for Bool {
    fn from(value: bool) -> Self {
        Self(value)
    }
}
impl From<i32> for Bool {
    fn from(value: i32) -> Self {
        Self(value != 0)
    }
}
impl From<Bool> for bool {
    fn from(value: Bool) -> Self {
        value.0
    }
}
impl Borrow<Bool> for bool {
    #[inline]
    fn borrow(&self) -> &Bool {
        Bool::from_ref(self)
    }
}
impl Borrow<bool> for Bool {
    #[inline]
    fn borrow(&self) -> &bool {
        &self.0
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Colour(pub Vec4);

impl Colour {
    pub const WHITE: Self = Self(Vec4::ONE);

    #[inline]
    pub const fn from_ref(colour: &Vec4) -> &Self {
        unsafe { mem::transmute(colour) }
    }
}

impl FromStr for Colour {
    type Err = <u32 as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // TODO: if prefix missing, is it always hex anyway?
        // TODO: could len==3 mean be 12-bit colour? what if not 6 or 8?
        let value = value.strip_prefix('#').unwrap_or(value);
        let mut itint = u32::from_str_radix(value, 16)?;
        if value.len() == 6 {
            itint |= 0xFF000000;
        }
        Ok(Self(Vec4::new(
            ((itint >> 16) & 0xFF) as f32 / 255.0,
            ((itint >> 8) & 0xFF) as f32 / 255.0,
            ((itint >> 0) & 0xFF) as f32 / 255.0,
            ((itint >> 24) & 0xFF) as f32 / 255.0,
        )))
    }
}
impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !f.alternate() {
            f.write_str("#")?;
        }
        if self.0.w < 1.0 {
            let a = (self.0.w * 255.0) as u8;
            write!(f, "{a:02x}")?;
        }
        let r = (self.0.x * 255.0) as u8;
        let g = (self.0.y * 255.0) as u8;
        let b = (self.0.z * 255.0) as u8;
        write!(f, "{r:02x}{g:02x}{b:02x}")
    }
}

impl From<Colour> for Vec4 {
    fn from(v: Colour) -> Self {
        v.0
    }
}
impl From<Vec4> for Colour {
    fn from(v: Vec4) -> Self {
        Self(v)
    }
}
impl Borrow<Vec4> for Colour {
    #[inline]
    fn borrow(&self) -> &Vec4 {
        &self.0
    }
}
impl Borrow<Colour> for Vec4 {
    #[inline]
    fn borrow(&self) -> &Colour {
        Colour::from_ref(self)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Opt<T>(pub Option<T>);

impl<T: FromStr> FromStr for Opt<T> {
    type Err = <T as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.is_empty() {
            false => value.parse().map(|v| Self(Some(v))),
            true => Ok(Self(None)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct List<T>(pub Box<[T]>);
impl<T> List<T> {
    #[inline]
    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.0.iter()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl<T> Default for List<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: FromStr> FromStr for List<T>
where
    <T as FromStr>::Err: fmt::Display,
{
    type Err = <T as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut err = None;
        let list: Box<[T]> = value
            .split(',')
            .map(|f| f.trim_ascii())
            .filter_map(|f| match f.parse() {
                Ok(v) => Some(v),
                Err(e) => {
                    if let Some(e) = err.replace(e) {
                        log::error!("unrecognized item {f:?} in list {value:?}: {e}");
                    }
                    None
                },
            })
            .collect();

        match err {
            Some(e) if list.is_empty() => Err(e),
            _ => Ok(Self(list)),
        }
    }
}
impl<T> FromIterator<T> for List<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}
#[cfg(todo)]
impl<T> Extend<T> for List<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.0.extend(iter)
    }
}
impl<T> IntoIterator for List<T> {
    type Item = T;
    type IntoIter = <Box<[T]> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self.0)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Array<const N: usize, T>(pub [T; N]);

impl<const N: usize, T: FromStr> FromStr for Array<N, T>
where
    T: Default + Copy,
    <T as FromStr>::Err: fmt::Display,
{
    type Err = io::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut list = [T::default(); N];

        let values = value.split(',').map(|f| f.trim_ascii()).map(FromStr::from_str);
        for (dest, item) in list.iter_mut().zip(values) {
            *dest = item.map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("parsing list {value:?} failed: {e}"),
                )
            })?;
        }

        Ok(Self(list))
    }
}

impl<const N: usize, T> Default for Array<N, T>
where
    T: Default + Copy,
{
    fn default() -> Self {
        Self([T::default(); N])
    }
}

// common
pack_key! {
    #[pack(attr = "alpha")]
    #[derive(Copy)]
    pub struct Alpha(pub f32);
    #[pack(attr = "canfade")]
    #[derive(Copy)]
    /// unrelated to [FadeNear] and [FadeFar] btw
    pub struct CanFade(pub Bool);
    #[pack(attr = "cull")]
    #[derive(Copy)]
    pub struct Cull(pub CullDirection);
    #[pack(attr = "edittag")]
    #[derive(Copy)]
    pub struct EditTag(pub i32);
    #[pack(attr = "fadenear")]
    #[derive(Copy)]
    pub struct FadeNear(pub f32);
    #[pack(attr = "fadefar")]
    #[derive(Copy)]
    pub struct FadeFar(pub f32);
    #[pack(attr = "minimapvisibility")]
    #[derive(Copy)]
    pub struct MinimapVisibility(pub Bool);
    #[pack(attr = "mapvisibility")]
    #[derive(Copy)]
    pub struct MapVisibility(pub Bool);
    #[pack(attr = "ingamevisibility")]
    #[derive(Copy)]
    pub struct InGameVisibility(pub Bool);
}
impl CanFade {
    pub const DEFAULT: Self = Self(Bool::TRUE);
}
impl Default for CanFade {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl InGameVisibility {
    pub const DEFAULT: Self = Self(Bool::TRUE);
}
impl Default for InGameVisibility {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl MinimapVisibility {
    pub const DEFAULT: Self = Self(Bool::TRUE);
}
impl Default for MinimapVisibility {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl MapVisibility {
    pub const DEFAULT: Self = Self(Bool::TRUE);
}
impl Default for MapVisibility {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl FadeNear {
    pub const DEFAULT: Self = Self(-1.0);
    #[inline(always)]
    pub const fn inches(&self) -> f32 { self.0 }
}
impl Default for FadeNear {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl FadeFar {
    pub const DEFAULT: Self = Self(-1.0);
    #[inline(always)]
    pub fn inches(&self) -> f32 { self.0 }
}
impl Default for FadeFar {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl Cull {
    pub const DEFAULT: Self = Self(CullDirection::None);
}
impl Default for Cull {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Tint(pub Colour);
impl Tint {
    pub const DEFAULT: Self = Self(Colour::WHITE);

    #[inline]
    pub const fn from_ref(colour: &Colour) -> &Self {
        unsafe { mem::transmute(colour) }
    }
}

impl From<Tint> for Vec4 {
    fn from(tint: Tint) -> Self {
        tint.0.into()
    }
}
impl From<Vec4> for Tint {
    fn from(tint: Vec4) -> Self {
        Self(tint.into())
    }
}

impl AttrKey for Tint {
    type Storage = Colour;
    const ATTR: &'static str = "tint";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR, "color"];
}
impl FromStr for Tint {
    type Err = <Colour as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Colour::from_str(value).map(Self)
    }
}

impl Default for Tint {
    fn default() -> Self {
        Self(Colour::WHITE)
    }
}
impl Borrow<Colour> for Tint {
    #[inline]
    fn borrow(&self) -> &Colour {
        &self.0
    }
}
impl Borrow<Tint> for Colour {
    #[inline]
    fn borrow(&self) -> &Tint {
        Tint::from_ref(self)
    }
}
impl Borrow<Vec4> for Tint {
    #[inline]
    fn borrow(&self) -> &Vec4 {
        &self.0.borrow()
    }
}
impl Borrow<Tint> for Vec4 {
    #[inline]
    fn borrow(&self) -> &Tint {
        Tint::from_ref(Colour::from_ref(self))
    }
}

/// sampled average colour of texture for use on minimap
/// (trails drawn as solid lines)
#[derive(Debug, Copy, Clone)]
pub struct MapTint(pub Colour);
impl MapTint {
    #[inline]
    pub fn from_ref(v: &Colour) -> &Self {
        unsafe { mem::transmute(v) }
    }
}

/// not real (yet?)
impl AttrKey for MapTint {
    type Storage = Colour;
    const ATTR: &'static str = "map-tint";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR, "trailsamplecolor"];
}
impl FromStr for MapTint {
    type Err = <Colour as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Colour::from_str(value).map(Self)
    }
}
impl From<MapTint> for Vec4 {
    fn from(tint: MapTint) -> Self {
        tint.0.into()
    }
}
impl From<Vec4> for MapTint {
    fn from(tint: Vec4) -> Self {
        Self(tint.into())
    }
}
impl Borrow<Colour> for MapTint {
    #[inline]
    fn borrow(&self) -> &Colour {
        &self.0
    }
}
impl Borrow<MapTint> for Colour {
    #[inline]
    fn borrow(&self) -> &MapTint {
        MapTint::from_ref(self)
    }
}
impl Borrow<Vec4> for MapTint {
    #[inline]
    fn borrow(&self) -> &Vec4 {
        &self.0.borrow()
    }
}
impl Borrow<MapTint> for Vec4 {
    #[inline]
    fn borrow(&self) -> &MapTint {
        MapTint::from_ref(Colour::from_ref(self))
    }
}
impl Default for MapTint {
    fn default() -> Self {
        Self(Colour::WHITE)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Point3(pub Vec3);
impl FromStr for Point3 {
    type Err = <Array<3, f32> as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        #[cfg(todo = "unnecessary")]
        let Some(value) = str_opt(value) else {
            return Ok(Self::DEFAULT)
        };
        Array::<3, f32>::from_str(value)
            .map(|Array(v)| Vec3::from_array(v))
            .map(Self)
    }
}

pack_key! {
    #[pack(attr = "specialization")]
    pub struct Specializations(pub List<Specialization>);
    #[pack(attr = "maptype")]
    pub struct MapTypes(pub List<MapType>);
    #[pack(attr = "raid")]
    pub struct Raids(pub List<Raid>);
}
#[derive(Debug, Copy, Clone)]
pub struct Specialization(pub u32);
impl Specialization {
    pub const fn slice_from(spec: &[u32]) -> &[Self] {
        unsafe { mem::transmute(spec) }
    }
    pub const fn slice_from_i32(spec: &[i32]) -> &[Self] {
        unsafe { mem::transmute(spec) }
    }
}
impl FromStr for Specialization {
    type Err = <u32 as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}
impl Specializations {
    /// TODO: u32 though...
    #[inline]
    pub fn from_attrlist(v: &AttrList<i32>) -> &Self {
        let list: &Box<[i32]> = &**v;
        unsafe { mem::transmute(list) }
    }
}
impl MapTypes {
    #[inline]
    pub fn from_attrlist(v: &AttrList<MapType>) -> &Self {
        let list: &Box<[MapType]> = &**v;
        unsafe { mem::transmute(list) }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Raid(pub String);
impl Raid {
    #[inline]
    pub fn from_ref(v: &String) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
impl FromStr for Raid {
    type Err = Infallible;

    #[inline]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.into()))
    }
}
impl From<String> for Raid {
    #[inline]
    fn from(raid: String) -> Self {
        Self(raid)
    }
}
impl From<&'_ String> for Raid {
    #[inline]
    fn from(raid: &String) -> Self {
        Self(raid.clone())
    }
}
impl From<&'_ str> for Raid {
    #[inline]
    fn from(raid: &str) -> Self {
        Self(raid.into())
    }
}
impl From<Raid> for String {
    #[inline]
    fn from(raid: Raid) -> Self {
        raid.0
    }
}
impl Borrow<Raid> for String {
    #[inline]
    fn borrow(&self) -> &Raid {
        Raid::from_ref(self)
    }
}
impl Borrow<String> for Raid {
    #[inline]
    fn borrow(&self) -> &String {
        &self.0
    }
}
impl Borrow<str> for Raid {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0[..]
    }
}
impl Raids {
    #[inline]
    pub fn from_attrlist(v: &AttrList<String>) -> &Self {
        let list: &Box<[String]> = &**v;
        unsafe { mem::transmute(list) }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Festivals(pub super::Festivals);
impl AttrKey for Festivals {
    type Storage = super::Festivals;
    const ATTR: &'static str = "festival";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR];
}
impl FromStr for Festivals {
    type Err = <List<super::Festival> as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        List::<super::Festival>::from_str(value).map(|festivals| Self(festivals.into_iter().collect()))
    }
}
impl Festivals {
    #[inline]
    pub const fn from_ref(v: &super::Festivals) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
impl From<super::Festivals> for Festivals {
    fn from(v: super::Festivals) -> Self { Self(v) }
}
impl From<Festivals> for super::Festivals {
    fn from(v: Festivals) -> Self { v.0 }
}
impl Borrow<Festivals> for super::Festivals {
    fn borrow(&self) -> &Festivals { Festivals::from_ref(self) }
}
impl Borrow<super::Festivals> for Festivals {
    fn borrow(&self) -> &super::Festivals { &self.0 }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mounts(pub super::Mounts);
impl AttrKey for Mounts {
    type Storage = super::Mounts;
    const ATTR: &'static str = "mount";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR];
}
impl FromStr for Mounts {
    type Err = <List<super::Mount> as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        List::<super::Mount>::from_str(value).map(|mounts| Self(mounts.into_iter().collect()))
    }
}
impl Mounts {
    #[inline]
    pub const fn from_ref(v: &super::Mounts) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
impl From<super::Mounts> for Mounts {
    fn from(v: super::Mounts) -> Self { Self(v) }
}
impl From<Mounts> for super::Mounts {
    fn from(v: Mounts) -> Self { v.0 }
}
impl Borrow<Mounts> for super::Mounts {
    fn borrow(&self) -> &Mounts { Mounts::from_ref(self) }
}
impl Borrow<super::Mounts> for Mounts {
    fn borrow(&self) -> &super::Mounts { &self.0 }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Professions(pub super::Professions);
impl AttrKey for Professions {
    type Storage = super::Professions;
    const ATTR: &'static str = "profession";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR];
}
impl FromStr for Professions {
    type Err = <List<super::Profession> as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        List::<super::Profession>::from_str(value).map(|mounts| Self(mounts.into_iter().collect()))
    }
}
impl Professions {
    #[inline]
    pub const fn from_ref(v: &super::Professions) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
impl From<super::Professions> for Professions {
    fn from(v: super::Professions) -> Self { Self(v) }
}
impl From<Professions> for super::Professions {
    fn from(v: Professions) -> Self { v.0 }
}
impl Borrow<Professions> for super::Professions {
    fn borrow(&self) -> &Professions { Professions::from_ref(self) }
}
impl Borrow<super::Professions> for Professions {
    fn borrow(&self) -> &super::Professions { &self.0 }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Races(pub super::Races);
impl AttrKey for Races {
    type Storage = super::Races;
    const ATTR: &'static str = "race";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR];
}
impl FromStr for Races {
    type Err = <List<super::Race> as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        List::<super::Race>::from_str(value).map(|mounts| Self(mounts.into_iter().collect()))
    }
}
impl Races {
    #[inline]
    pub const fn from_ref(v: &super::Races) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
impl From<super::Races> for Races {
    fn from(v: super::Races) -> Self { Self(v) }
}
impl From<Races> for super::Races {
    fn from(v: Races) -> Self { v.0 }
}
impl Borrow<Races> for super::Races {
    fn borrow(&self) -> &Races { Races::from_ref(self) }
}
impl Borrow<super::Races> for Races {
    fn borrow(&self) -> &super::Races { &self.0 }
}

// POI
pack_key! {
    #[pack(attr = "heightoffset")]
    #[derive(Copy)]
    pub struct HeightOffset(pub f32);
    #[pack(attr = "iconfile")]
    pub struct IconFile(pub File);
    #[pack(attr = "iconsize")]
    #[derive(Copy)]
    pub struct IconSize(pub f32);
    #[pack(attr = "invertbehavior")]
    #[derive(Copy, Default)]
    pub struct InvertBehaviour(pub Bool);
    #[pack(attr = "resetlength")]
    #[derive(Copy, Default, PartialEq, PartialOrd)]
    pub struct ResetLength(pub f32);
    #[pack(attr = "mapdisplaysize")]
    #[derive(Copy)]
    pub struct MapDisplaySize(pub f32);
    #[pack(attr = "scaleonmapwithzoom")]
    #[derive(Copy)]
    pub struct ScaleOnMapWithZoom(pub Bool);
    #[pack(attr = "minsize")]
    #[derive(Copy)]
    /// in pixels
    pub struct MinSize(pub f32);
    #[pack(attr = "maxsize")]
    #[derive(Copy)]
    /// in pixels
    pub struct MaxSize(pub f32);
    #[pack(attr = "occlude")]
    #[derive(Copy, Default)]
    pub struct Occlude(pub Bool);
    #[pack(attr = "rotate-x")]
    #[derive(Copy, Default)]
    pub struct RotateX(pub f32);
    #[pack(attr = "rotate-y")]
    #[derive(Copy, Default)]
    pub struct RotateY(pub f32);
    #[pack(attr = "rotate-z")]
    #[derive(Copy, Default)]
    pub struct RotateZ(pub f32);
    #[pack(attr = "xpos")]
    #[derive(Copy, Default)]
    pub struct PositionX(pub f32);
    #[pack(attr = "ypos")]
    #[derive(Copy, Default)]
    pub struct PositionY(pub f32);
    #[pack(attr = "zpos")]
    #[derive(Copy, Default)]
    pub struct PositionZ(pub f32);
    #[pack(attr = "text")]
    /// Billboard text
    ///
    /// alias for [Title]?
    pub struct Text(pub AttrString);
    #[pack(attr = "title")]
    pub struct Title(pub AttrString);
    #[pack(attr = "title-color")]
    #[derive(Copy)]
    pub struct TitleColour(pub Colour);
    #[pack(attr = "tip-name")]
    pub struct TipName(pub AttrString);
    #[pack(attr = "tip-description")]
    pub struct TipDescription(pub AttrString);
}

impl IconSize {
    pub const DEFAULT: Self = Self(1.0);
}
impl Default for IconSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl MapDisplaySize {
    pub const DEFAULT: Self = Self(20.0);
}
impl Default for MapDisplaySize {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl ScaleOnMapWithZoom {
    pub const DEFAULT: Self = Self(Bool::TRUE);
}
impl Default for ScaleOnMapWithZoom {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl MinSize {
    pub const DEFAULT: Self = Self(5.0);
}
impl Default for MinSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl MaxSize {
    pub const DEFAULT: Self = Self(2048.0);
}
impl Default for MaxSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl Occlude {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}

impl HeightOffset {
    pub const DEFAULT: Self = Self(1.5);
}
impl Default for HeightOffset {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Alpha {
    pub const DEFAULT: Self = Self(1.0);
}
impl Default for Alpha {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rotate(pub Vec3);
impl Rotate {
    pub const DEFAULT: Self = Self(Vec3::ZERO);

    #[inline]
    pub const fn from_ref(v: &Vec3) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
impl AttrKey for Rotate {
    type Storage = Vec3;
    const ATTR: &'static str = "rotate";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR];
}
impl FromStr for Rotate {
    type Err = <Array<3, f32> as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        #[cfg(todo = "unnecessary")]
        let Some(value) = str_opt(value) else {
            return Ok(Self::DEFAULT)
        };
        Array::<3, f32>::from_str(value)
            .map(|Array(v)| Vec3::from_array(v))
            .map(Self)
    }
}
impl Default for Rotate {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl From<Vec3> for Rotate {
    #[inline]
    fn from(v: Vec3) -> Self {
        Self(v)
    }
}
impl From<Rotate> for Vec3 {
    #[inline]
    fn from(v: Rotate) -> Self {
        v.0
    }
}
impl Borrow<Vec3> for Rotate {
    #[inline]
    fn borrow(&self) -> &Vec3 {
        &self.0
    }
}
impl Borrow<Rotate> for Vec3 {
    #[inline]
    fn borrow(&self) -> &Rotate {
        Rotate::from_ref(self)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShowHideAction {
    Show,
    Hide,
    Toggle,
}
impl ShowHideAction {
    pub fn tristate(self) -> Option<bool> {
        match self {
            Self::Show => Some(true),
            Self::Hide => Some(false),
            Self::Toggle => None,
        }
    }
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Hide => "hide",
            Self::Toggle => "toggle",
        }
    }

    pub fn iter_in_attrs<A>(attrs: &A) -> impl Iterator<Item = (Cow<'_, AttrString>, Self)> where
        A: ?Sized
            + GetAttr<ShowCategory>
            + GetAttr<HideCategory>
            + GetAttr<ToggleCategory>,
    {
        fn borrow_str<'a, C>(cat: Cow<'a, C>) -> Cow<'a, AttrString> where
            C: Borrow<AttrString> + ToOwned,
            C::Owned: Into<AttrString>,
        {
            match cat {
                Cow::Borrowed(b) => Cow::Borrowed(b.borrow()),
                Cow::Owned(b) => Cow::Owned(b.into()),
            }
        }
        IntoIterator::into_iter([
            (GetAttr::<ShowCategory>::get_attr(attrs).map(borrow_str), Self::Show),
            (GetAttr::<HideCategory>::get_attr(attrs).map(borrow_str), Self::Hide),
            (GetAttr::<ToggleCategory>::get_attr(attrs).map(borrow_str), Self::Toggle),
        ]).filter_map(|(v, t)| v.map(|v| (v, t)))
    }
}
impl fmt::Display for ShowHideAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum TacoBehaviour {
    AlwaysVisible = 0,
    /// On map change
    ResetVisit = 1,
    ResetDaily = 2,
    ResetPermanent = 3,
    /// See [ResetLength]
    ResetDelay = 4,
    /// Unimplemented on TacO and BlishHUD
    ResetMap = 5,
    /// When map shard changes
    ResetInstance = 6,
    ResetDailyPerCharacter = 7,
}
impl TacoBehaviour {
    pub const ALLOC_TACO_MIN: u32 = 0;
    pub const ALLOC_TACO_MAX: u32 = 99;
    pub const TACO_MIN: u8 = Self::AlwaysVisible as _;
    pub const TACO_MAX: u8 = Self::ResetDailyPerCharacter as _;
    pub const NONE: Self = Self::AlwaysVisible;

    pub const fn value(self) -> u8 {
        self as u8
    }
    pub const fn value_ref(&self) -> &u8 {
        unsafe { mem::transmute(self) }
    }
    pub const unsafe fn from_value_unchecked(value: u8) -> Self {
        mem::transmute(value)
    }
    pub const unsafe fn from_ref_unchecked(value: &u8) -> &Self {
        mem::transmute(value)
    }
}
impl Default for TacoBehaviour {
    fn default() -> Self {
        Self::NONE
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BlishBehaviour {
    ResetWeekly = 101,
    /// "dismiss" an achievement-filtered marker
    TaimiAchievement = TaimiBehaviour::ALLOC_TAIMI_MIN as u8,
}
impl BlishBehaviour {
    pub const ALLOC_BLISH_MIN: u32 = 100;
    #[cfg(todo)]
    pub const ALLOC_BLISH_MAX: u32 = 199;
    pub const BLISH_MIN: u8 = Self::ResetWeekly as _;
    pub const BLISH_MAX: u8 = Self::ResetWeekly as _;
    pub const fn value(self) -> u8 {
        self as u8
    }
    pub const fn value_ref(&self) -> &u8 {
        unsafe { mem::transmute(self) }
    }
    pub const unsafe fn from_value_unchecked(value: u8) -> Self {
        mem::transmute(value)
    }
    pub const unsafe fn from_ref_unchecked(value: &u8) -> &Self {
        mem::transmute(value)
    }
}
/// TODO
pub use self::BlishBehaviour as TaimiBehaviour;
impl TaimiBehaviour {
    pub const ALLOC_TAIMI_MIN: u32 = 33;
    pub const ALLOC_TAIMI_MAX: u32 = 45;
    pub const TAIMI_MIN: u8 = Self::TaimiAchievement as _;
    pub const TAIMI_MAX: u8 = Self::TaimiAchievement as _;
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Behaviour {
    Taco(TacoBehaviour),
    Blish(BlishBehaviour),
}
impl Behaviour {
    pub const NONE: Self = Self::Taco(TacoBehaviour::NONE);
    pub const ALL: &'static [Self] = &[
        Self::Taco(TacoBehaviour::AlwaysVisible),
        Self::Taco(TacoBehaviour::ResetVisit),
        Self::Taco(TacoBehaviour::ResetDaily),
        Self::Taco(TacoBehaviour::ResetPermanent),
        Self::Taco(TacoBehaviour::ResetDelay),
        Self::Taco(TacoBehaviour::ResetMap),
        Self::Taco(TacoBehaviour::ResetInstance),
        Self::Taco(TacoBehaviour::ResetDailyPerCharacter),
        Self::Blish(BlishBehaviour::ResetWeekly),
        Self::Blish(BlishBehaviour::TaimiAchievement),
    ];

    pub const fn value(self) -> u8 {
        match self {
            Self::Taco(behaviour) => behaviour.value(),
            Self::Blish(behaviour) => behaviour.value(),
        }
    }
    pub const fn value_ref(&self) -> &u8 {
        match self {
            Self::Taco(behaviour) => behaviour.value_ref(),
            Self::Blish(behaviour) => behaviour.value_ref(),
        }
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Taco(TacoBehaviour::AlwaysVisible))
    }

    pub fn from_value(value: u8) -> Option<Self> {
        match value {
            | TaimiBehaviour::TAIMI_MIN..=TaimiBehaviour::TAIMI_MAX
            | TacoBehaviour::TACO_MIN..=TacoBehaviour::TACO_MAX
            | BlishBehaviour::BLISH_MIN..=BlishBehaviour::BLISH_MAX =>
                Some(unsafe { Self::from_value_unchecked(value) }),
            _ => None,
        }
    }
    #[inline]
    pub unsafe fn from_value_unchecked(value: u8) -> Self {
        match value as u32 {
            TaimiBehaviour::ALLOC_TAIMI_MIN..=TaimiBehaviour::ALLOC_TAIMI_MAX =>
                Self::Taimi(TaimiBehaviour::from_value_unchecked(value as u8)),
            TacoBehaviour::ALLOC_TACO_MIN..=TacoBehaviour::ALLOC_TACO_MAX =>
                Self::Taco(TacoBehaviour::from_value_unchecked(value as u8)),
            _ => Self::Blish(BlishBehaviour::from_value_unchecked(value as u8)),
        }
    }

    #[inline]
    #[allow(non_snake_case)]
    pub fn Taimi(b: TaimiBehaviour) -> Self {
        Self::Blish(b)
    }
}
impl AttrKey for Behaviour {
    type Storage = u8;
    const ATTR: &'static str = "behavior";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR];
}
impl Default for Behaviour {
    fn default() -> Self {
        Self::NONE
    }
}
impl From<TacoBehaviour> for u8 {
    #[inline]
    fn from(behaviour: TacoBehaviour) -> Self {
        behaviour as _
    }
}
impl From<BlishBehaviour> for u8 {
    #[inline]
    fn from(behaviour: BlishBehaviour) -> Self {
        behaviour as _
    }
}
impl From<Behaviour> for u8 {
    fn from(behaviour: Behaviour) -> Self {
        behaviour.value()
    }
}
impl Borrow<u8> for Behaviour {
    fn borrow(&self) -> &u8 {
        self.value_ref()
    }
}
impl Borrow<TacoBehavior> for Behaviour {
    fn borrow(&self) -> &TacoBehavior {
        unsafe { mem::transmute(self.value_ref()) }
    }
}
impl From<TacoBehavior> for Behaviour {
    fn from(value: TacoBehavior) -> Self {
        unsafe { Self::from_value_unchecked(value as _) }
    }
}
impl From<Behaviour> for TacoBehavior {
    fn from(value: Behaviour) -> TacoBehavior {
        unsafe { mem::transmute(value.value()) }
    }
}
impl FromStr for Behaviour {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u8>()
            .with_context(|| format!("unknown taco behaviour `{s}`"))
            .and_then(|v| v.try_into())
    }
}
impl TryFrom<u8> for Behaviour {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_value(value).with_context(|| format!("unknown taco behaviour `{value}`"))
    }
}
impl ResetLength {
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f32(self.0)
    }
}
impl InvertBehaviour {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}

// Trails
pack_key! {
    #[pack(attr = "animspeed")]
    #[derive(Copy)]
    pub struct AnimSpeed(pub f32);
    #[pack(attr = "traildata")]
    pub struct TrailDataFile(pub AttrString);
    #[pack(attr = "texture")]
    pub struct TextureFile(pub File);
    #[pack(attr = "trailscale")]
    #[derive(Copy)]
    pub struct TrailScale(pub f32);
    #[pack(attr = "iswall")]
    #[derive(Copy, Default)]
    pub struct IsWall(pub Bool);
}

impl AnimSpeed {
    pub const DEFAULT: Self = Self(1.0);
}
impl Default for AnimSpeed {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl TrailScale {
    pub const DEFAULT: Self = Self(1.0);
}
impl Default for TrailScale {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl IsWall {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}

// Categories
pack_key! {
    #[pack(attr = "defaulttoggle")]
    #[derive(Copy)]
    pub struct DefaultToggle(pub Bool);
    #[pack(attr = "ishidden")]
    #[derive(Copy, Default)]
    pub struct IsHidden(pub Bool);
    #[pack(attr = "isseparator")]
    #[derive(Copy, Default)]
    pub struct IsSeparator(pub Bool);
    #[pack(attr = "displayname")]
    pub struct DisplayName(pub AttrString);
}
impl DefaultToggle {
    pub const DEFAULT: Self = Self(Bool::TRUE);
}
impl Default for DefaultToggle {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl IsHidden {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl IsSeparator {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl Default for DisplayName {
    fn default() -> Self {
        Self(Default::default())
    }
}

// Modifiers
pack_key! {
    #[pack(attr = "type")]
    pub struct CategoryRef(pub AttrString);
    #[pack(attr = "name")]
    pub struct NameId(pub AttrString);
    #[pack(attr = "mapid")]
    #[derive(Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct GameMap(pub u32);
    #[pack(attr = "info")]
    pub struct Info(pub AttrString);
    #[pack(attr = "inforange")]
    #[derive(Copy)]
    /// Similar to [TriggerRange] but treat with higher priority
    pub struct InfoRange(pub f32);
    #[pack(attr = "triggerrange")]
    #[derive(Copy)]
    pub struct TriggerRange(pub f32);
    #[pack(attr = "autotrigger")]
    #[derive(Copy, Default)]
    pub struct AutoTrigger(pub Bool);
    #[pack(attr = "copy")]
    pub struct CopyValue(pub AttrString);
    #[pack(attr = "copy-message")]
    pub struct CopyMessage(pub AttrString);
    #[pack(attr = "resetguid")]
    #[derive(Default)]
    pub struct ResetGuid(pub List<Guid>);
    #[pack(attr = "toggle", aliases("togglecategory"))]
    pub struct ToggleCategory(pub AttrString);
    #[pack(attr = "show")]
    pub struct ShowCategory(pub AttrString);
    #[pack(attr = "hide")]
    pub struct HideCategory(pub AttrString);
    #[pack(attr = "achievementid")]
    #[derive(Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct AchievementId(pub u32);
    #[pack(attr = "achievementbit")]
    #[derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct AchievementBit(pub u8);
    #[pack(attr = "schedule")]
    #[derive(Default)]
    pub struct ScheduleStart(pub AttrString);
    #[pack(attr = "schedule-duration")]
    #[derive(Copy, Default)]
    pub struct ScheduleDuration(pub f32);
    #[pack(attr = "bounce")]
    #[derive(Copy)]
    pub struct Bounce(pub BounceBehavior);
    #[pack(attr = "bounce-height")]
    #[derive(Copy)]
    pub struct BounceHeight(pub f32);
    #[pack(attr = "bounce-duration")]
    #[derive(Copy)]
    pub struct BounceDuration(pub f32);
    #[pack(attr = "bounce-delay")]
    #[derive(Copy, Default)]
    pub struct BounceDelay(pub f32);
    #[pack(attr = "script-tick")]
    pub struct ScriptTick(pub Script);
    #[pack(attr = "script-focus")]
    pub struct ScriptFocus(pub Script);
    #[pack(attr = "script-trigger")]
    pub struct ScriptTrigger(pub Script);
    #[pack(attr = "script-filter")]
    pub struct ScriptFilter(pub Script);
    #[pack(attr = "script-once")]
    pub struct ScriptOnce(pub Script);
}

impl From<id::CategoryId> for CategoryRef {
    fn from(id: id::CategoryId) -> Self {
        if id.is_full_id() {
            id.into_inner().into()
        } else {
            id.as_str().into()
        }
    }
}
impl From<&'_ id::CategoryId> for CategoryRef {
    fn from(id: &id::CategoryId) -> Self {
        id.to_id_box().into_owned().into()
    }
}
#[cfg(todo)]
impl<T> From<&'_ id::CategoryId<T>> for CategoryRef
where
    T: ?Sized,
    id::CategoryId<T>: AsRef<str>,
{
    #[inline]
    fn from(s: &id::CategoryId<T>) -> Self {
        AsRef::<str>::as_ref(s).into()
    }
}
impl AutoTrigger {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl TriggerRange {
    pub const DEFAULT: Self = Self(2.0);
}
impl Default for TriggerRange {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl InfoRange {
    pub const DEFAULT: Self = Self(TriggerRange::DEFAULT.0);
}
impl Default for InfoRange {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl BounceDelay {
    pub const DEFAULT: Self = Self(0.0);
}
impl BounceDuration {
    pub const DEFAULT: Self = Self(1.0);
}
impl Default for BounceDuration {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl BounceHeight {
    pub const DEFAULT: Self = Self(2.0);
}
impl Default for BounceHeight {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl ScheduleDuration {
    pub const DEFAULT: Self = Self(0.0);
}

impl From<i32> for GameMap {
    #[inline]
    fn from(id: i32) -> Self {
        Self(id as _)
    }
}
impl From<NonZero<u32>> for GameMap {
    #[inline]
    fn from(id: NonZero<u32>) -> Self {
        Self(id.get() as _)
    }
}
impl From<GameMap> for i32 {
    #[inline]
    fn from(id: GameMap) -> Self {
        id.0 as _
    }
}
impl Borrow<GameMap> for i32 {
    #[inline]
    fn borrow(&self) -> &GameMap {
        GameMap::from_ref(unsafe { &*(self as *const i32 as *const u32) })
    }
}

impl From<i32> for AchievementId {
    #[inline]
    fn from(id: i32) -> Self {
        Self(id as _)
    }
}
impl From<AchievementId> for i32 {
    #[inline]
    fn from(id: AchievementId) -> Self {
        id.0 as _
    }
}
impl From<NonZero<u32>> for AchievementId {
    #[inline]
    fn from(id: NonZero<u32>) -> Self {
        Self(id.get())
    }
}
impl From<i32> for AchievementBit {
    #[inline]
    fn from(id: i32) -> Self {
        Self(id as _)
    }
}
impl From<AchievementBit> for i32 {
    #[inline]
    fn from(id: AchievementBit) -> Self {
        id.0 as _
    }
}
impl From<u16> for AchievementBit {
    #[inline]
    fn from(id: u16) -> Self {
        Self(id as _)
    }
}

impl Borrow<AchievementId> for i32 {
    #[inline]
    fn borrow(&self) -> &AchievementId {
        AchievementId::from_ref(unsafe { &*(self as *const i32 as *const u32) })
    }
}
impl Borrow<AchievementBit> for i32 {
    #[inline]
    fn borrow(&self) -> &AchievementBit {
        let bytes = unsafe { &*(self as *const i32 as *const [u8; mem::size_of::<i32>()]) };
        let byte = match bytes {
            #[cfg(target_endian = "little")]
            &[ref b, ..] => b,
            #[cfg(target_endian = "big")]
            &[_, _, _, ref b] => b,
            #[cfg(not(any(target_endian = "little", target_endian = "big")))]
            &[unexpected_endianness, _, _, ref b] => b,
        };
        AchievementBit::from_ref(byte)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Guid(pub Uuid);
impl Guid {
    pub const EMPTY: Self = Self(Uuid::nil());

    #[inline]
    pub const fn from_ref(uuid: &Uuid) -> &Self {
        unsafe { mem::transmute(uuid) }
    }
    #[inline]
    pub const fn from_slice(uuids: &[Uuid]) -> &[Self] {
        unsafe { mem::transmute(uuids) }
    }
    #[inline]
    pub fn from_uuid_ref<U: AsRef<Uuid>>(uuid: &U) -> &Self {
        Self::from_ref(uuid.as_ref())
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_nil()
    }
    pub fn or_empty(&self) -> Option<&Self> {
        (!self.is_empty()).then_some(self)
    }
}
impl AttrKey for Guid {
    type Storage = Uuid;
    const ATTR: &'static str = "guid";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR];
}
impl FromStr for Guid {
    type Err = base64::DecodeSliceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut raw_guid = [0u8; 16];
        match base64::engine::general_purpose::STANDARD.decode_slice(s, &mut raw_guid) {
            Ok(16) => Ok(Self(Uuid::from_bytes_le(raw_guid))),
            Ok(l) => Err(base64::DecodeError::InvalidLength(l).into()),
            Err(e) => Err(e),
        }
    }
}
impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = [0u8; 24];
        let len = base64::engine::general_purpose::STANDARD
            .encode_slice(&self.0.to_bytes_le(), &mut out)
            .map_err(|_| fmt::Error)?;
        f.write_str(unsafe { str::from_utf8_unchecked(out.get_unchecked(..len)) })
    }
}
impl Default for Guid {
    #[inline]
    fn default() -> Self {
        Self(Uuid::nil())
    }
}
impl From<Uuid> for Guid {
    #[inline]
    fn from(v: Uuid) -> Self {
        Self(v)
    }
}
impl From<Option<Uuid>> for Guid {
    #[inline]
    fn from(v: Option<Uuid>) -> Self {
        Self(v.unwrap_or_default())
    }
}
impl From<Guid> for Uuid {
    #[inline]
    fn from(v: Guid) -> Self {
        v.0
    }
}
impl From<Option<Guid>> for Guid {
    #[inline]
    fn from(v: Option<Guid>) -> Self {
        v.unwrap_or_default()
    }
}
impl<'a> From<&'a Uuid> for &'a Guid {
    #[inline]
    fn from(v: &'a Uuid) -> Self {
        Guid::from_ref(v)
    }
}
impl<'a> From<&'a Guid> for &'a Uuid {
    #[inline]
    fn from(v: &'a Guid) -> Self {
        &v.0
    }
}
impl AsRef<Uuid> for Guid {
    #[inline]
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}
impl AsRef<Guid> for Uuid {
    #[inline]
    fn as_ref(&self) -> &Guid {
        Guid::from_ref(self)
    }
}
impl Borrow<Uuid> for Guid {
    #[inline]
    fn borrow(&self) -> &Uuid {
        &self.0
    }
}
impl Borrow<Guid> for Uuid {
    #[inline]
    fn borrow(&self) -> &Guid {
        Guid::from_ref(self)
    }
}
impl PartialEq<Uuid> for Guid {
    #[inline]
    fn eq(&self, rhs: &Uuid) -> bool {
        &self.0 == rhs
    }
}
impl PartialEq<Guid> for Uuid {
    #[inline]
    fn eq(&self, rhs: &Guid) -> bool {
        self == &rhs.0
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Guid {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)
            .and_then(|guid| Self::from_str(&guid).map_err(|e| serde::de::Error::custom(e)))
    }
}
#[cfg(feature = "serde")]
impl serde::Serialize for Guid {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_string().serialize(s)
    }
}

pub trait GetAttr<A: ?Sized> {
    #[inline]
    fn has_attr(&self) -> bool {
        self.get_attr_ref().is_some()
    }
    #[inline]
    fn get_attr_ref(&self) -> Option<&A> {
        None
    }
    #[cfg(todo)]
    fn get_attr_storage(&self) -> Option<&A::Storage>;
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, A>> where
        A: ToOwned,
    {
        self.get_attr_ref().map(Cow::Borrowed)
    }
    #[inline]
    fn get_attr_or_default(&self) -> Cow<'_, A>
    where
        A: ToOwned,
        A::Owned: Default,
    {
        self.get_attr().unwrap_or_default()
    }
}
pub trait SetAttr<A: ?Sized> {
    #[cfg(todo)]
    fn get_attr_mut(&mut self) -> Option<&mut A>;
    fn set_attr(&mut self, value: A);
    #[inline]
    fn unset_attr(&mut self) {}
}
impl<A: ?Sized + AttrKey> GetAttr<A> for A {
    fn has_attr(&self) -> bool {
        true
    }
    fn get_attr_ref(&self) -> Option<&A> {
        Some(self)
    }
}
#[cfg(todo)]
impl<A: AttrKey, T: ?Sized + GetAttr<A>> GetAttr<A> for &'_ T {
    fn has_attr(&self) -> bool {
        GetAttr::has_attr(*self)
    }
    fn get_attr_ref(&self) -> Option<&A> {
        GetAttr::get_attr_ref(*self)
    }
    fn get_attr(&self) -> Option<Cow<'_, A>> {
        GetAttr::get_attr(*self)
    }
}
impl<T, A: ?Sized + AttrKey> GetAttr<A> for Option<T>
where
    T: GetAttr<A>,
{
    fn has_attr(&self) -> bool {
        self.as_ref().map(GetAttr::<A>::has_attr).unwrap_or(false)
    }
    fn get_attr_ref(&self) -> Option<&A> {
        self.as_ref().and_then(GetAttr::<A>::get_attr_ref)
    }
    fn get_attr(&self) -> Option<Cow<'_, A>> {
        self.as_ref().and_then(GetAttr::<A>::get_attr)
    }
}

macro_rules! pack_key {
    () => {};
    (
        $(#[$($meta:tt)*])*
        $vis:vis struct $ident:ident(
            $(#[$($meta_field:tt)*])*
            $($field:tt)*
        );
        $($rest:tt)*
    ) => {
        pack_key! {
            @pack
            $(#[$($meta)*])*
            $vis struct $ident(
                $(#[$($meta_field)*])*
                $($field)*
            );
            $($rest)*
        }
        pack_key! {
            @fromstr
            struct $ident(
                $($field)*
            );
        }
        #[cfg(feature = "script-lua")]
        pack_key! {
            @luaconv
            struct $ident(
                $($field)*
            );
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis Script
        );
    ) => {
        pack_key! {
            @fromstr
            struct $ident(
                $vis_field AttrString
            );
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis File
        );
    ) => {
        pack_key! {
            @fromstr
            struct $ident(
                $vis_field AttrString
            );
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis AttrString
        );
    ) => {
        impl fmt::Display for $ident {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(&self[..], f)
            }
        }
        impl AsRef<str> for $ident {
            #[inline]
            fn as_ref(&self) -> &str {
                &self.0[..]
            }
        }
        impl AsRef<Box<str>> for $ident {
            #[inline]
            fn as_ref(&self) -> &Box<str> {
                &*self.0
            }
        }
        impl Borrow<str> for $ident {
            #[inline]
            fn borrow(&self) -> &str {
                &self.0[..]
            }
        }
        impl FromStr for $ident {
            type Err = Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self::from(s))
            }
        }
        impl From<&'_ str> for $ident {
            #[inline]
            fn from(s: &str) -> Self {
                Self(super::string_into(s))
            }
        }
        impl From<&'_ Arc<str>> for $ident {
            #[inline]
            fn from(s: &Arc<str>) -> Self {
                s[..].into()
            }
        }
        impl From<String> for $ident {
            #[inline]
            fn from(s: String) -> Self {
                Self(super::string_into(s))
            }
        }
        impl From<&'_ String> for $ident {
            #[inline]
            fn from(s: &String) -> Self {
                s[..].into()
            }
        }
        impl From<Box<str>> for $ident {
            #[inline]
            fn from(s: Box<str>) -> Self {
                Self(super::string_into(s))
            }
        }
        impl From<&'_ Box<str>> for $ident {
            #[inline]
            fn from(s: &Box<str>) -> Self {
                Self(super::string_into(s.clone()))
            }
        }
        impl From<&'_ crate::category::id::IdNameSeg> for $ident {
            #[inline]
            fn from(s: &crate::category::id::IdNameSeg) -> Self {
                s.as_str().into()
            }
        }
        impl From<&'_ crate::category::id::IdNameBox> for $ident {
            #[inline]
            fn from(s: &crate::category::id::IdNameBox) -> Self {
                Self(s.name.clone().into())
            }
        }
        impl From<crate::category::id::IdNameBox> for $ident {
            #[inline]
            fn from(s: crate::category::id::IdNameBox) -> Self {
                Self(s.name.into())
            }
        }
        impl From<$ident> for crate::category::id::IdNameBox {
            #[inline]
            fn from(s: $ident) -> Self {
                s[..].into()
            }
        }
        impl Borrow<$ident> for crate::category::id::IdNameBox {
            #[inline]
            fn borrow(&self) -> &$ident {
                $ident::from_ref(&self.name)
            }
        }
        impl Borrow<$ident> for AttrString {
            #[inline]
            fn borrow(&self) -> &$ident {
                $ident::from_ref(core::borrow::Borrow::borrow(self))
            }
        }
        impl Borrow<AttrString> for $ident {
            #[inline]
            fn borrow(&self) -> &AttrString {
                core::borrow::Borrow::borrow(&self.0)
            }
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis List<$elem:ty>
        );
    ) => {
        pack_key! {
            @fromstr
            struct $ident(
                $vis_field, {List<$elem>}
            );
        }
        impl<E: Into<$elem>> FromIterator<E> for $ident {
            #[inline]
            fn from_iter<I: IntoIterator<Item = E>>(iter: I) -> Self {
                Self(FromIterator::from_iter(iter.into_iter().map(Into::into)))
            }
        }
        impl IntoIterator for $ident {
            type Item = $elem;
            type IntoIter = <List<$elem> as IntoIterator>::IntoIter;
            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                self.0.into_iter()
            }
        }
        impl From<$elem> for $ident {
            #[inline]
            fn from(e: $elem) -> Self { FromIterator::from_iter([e]) }
        }
        impl From<Box<[$elem]>> for $ident {
            #[inline]
            fn from(e: Box<[$elem]>) -> Self { Self(List(e)) }
        }
        impl From<Vec<$elem>> for $ident {
            #[inline]
            fn from(e: Vec<$elem>) -> Self { Self(List(e.into_boxed_slice())) }
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis Colour
        );
    ) => {
        pack_key! {
            @fromstr
            struct $ident(
                $vis_field, {Colour}
            );
        }
        impl core::borrow::Borrow<$ident> for Vec4 {
            #[inline]
            fn borrow(&self) -> &$ident {
                $ident::from_ref(core::borrow::Borrow::borrow(self))
            }
        }
        impl core::borrow::Borrow<Vec4> for $ident {
            #[inline]
            fn borrow(&self) -> &Vec4 {
                core::borrow::Borrow::borrow(&self.0)
            }
        }
        impl From<Vec4> for $ident {
            #[inline]
            fn from(v: Vec4) -> Self {
                Self(Colour(v))
            }
        }
        impl From<$ident> for Vec4 {
            #[inline]
            fn from(v: $ident) -> Self {
                v.0.0
            }
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis Bool
        );
    ) => {
        pack_key! {
            @fromstr
            struct $ident(
                $vis_field, {Bool}
            );
        }
        impl core::borrow::Borrow<$ident> for bool {
            #[inline]
            fn borrow(&self) -> &$ident {
                $ident::from_ref(core::borrow::Borrow::borrow(self))
            }
        }
        impl core::borrow::Borrow<bool> for $ident {
            #[inline]
            fn borrow(&self) -> &bool {
                core::borrow::Borrow::borrow(&self.0)
            }
        }
        impl From<bool> for $ident {
            #[inline]
            fn from(v: bool) -> Self {
                Self(Bool(v))
            }
        }
        impl From<$ident> for bool {
            #[inline]
            fn from(v: $ident) -> Self {
                v.0.0
            }
        }
        impl PartialEq<bool> for $ident {
            #[inline]
            fn eq(&self, rhs: &bool) -> bool {
                self.0.0 == *rhs
            }
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis $ty:ty
        );
    ) => {
        pack_key! {
            @fromstr
            struct $ident(
                $vis_field, {$ty}
            );
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis, {$ty:ty}
        );
    ) => {
        impl FromStr for $ident {
            type Err = <$ty as FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <$ty as FromStr>::from_str(s)
                    .map(Self)
            }
        }
        impl core::borrow::Borrow<$ident> for $ty {
            #[inline]
            fn borrow(&self) -> &$ident {
                $ident::from_ref(core::borrow::Borrow::borrow(self))
            }
        }
        impl core::borrow::Borrow<$ty> for $ident {
            #[inline]
            fn borrow(&self) -> &$ty {
                core::borrow::Borrow::borrow(&self.0)
            }
        }
    };
    (@luaconv
        $vis:vis struct $ident:ident(
            $vis_field:vis Bool
        );
    ) => {
        impl mlua::IntoLua for $ident {
            #[inline]
            fn into_lua(self, _lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                Ok(mlua::Value::Boolean(self.0.into()))
            }
        }
        impl mlua::FromLua for $ident {
            #[inline]
            fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
                <bool as mlua::FromLua>::from_lua(value, lua).map(|v| Self(v.into()))
            }
        }
    };
    (@luaconv
        $vis:vis struct $ident:ident(
            $vis_field:vis f32
        );
    ) => {
        impl mlua::IntoLua for $ident {
            #[inline]
            fn into_lua(self, _lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                Ok(mlua::Value::Number(self.0 as _))
            }
        }
        impl mlua::FromLua for $ident {
            #[inline]
            fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
                <f32 as mlua::FromLua>::from_lua(value, lua).map(|v| Self(v))
            }
        }
    };
    (@luaconv
        $vis:vis struct $ident:ident(
            $vis_field:vis File
        );
    ) => {
        pack_key! {
            @luaconv
            struct $ident(
                $vis_field AttrString
            );
        }
    };
    (@luaconv
        $vis:vis struct $ident:ident(
            $vis_field:vis Script
        );
    ) => {
        pack_key! {
            @luaconv
            struct $ident(
                $vis_field AttrString
            );
        }
    };
    (@luaconv
        $vis:vis struct $ident:ident(
            $vis_field:vis AttrString
        );
    ) => {
        impl mlua::IntoLua for $ident {
            #[inline]
            fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                mlua::IntoLua::into_lua(&self.0[..], lua)
            }
        }
        impl mlua::FromLua for $ident {
            #[inline]
            fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
                let value = crate::script::lua::RuntimeLua::lua_tostring(lua, value, false)
                    .map(mlua::Value::String)?;
                <Box<str> as mlua::FromLua>::from_lua(value, lua).map(|v| Self(v.into()))
            }
        }
    };
    (@luaconv
        $vis:vis struct $ident:ident(
            $vis_field:vis File
        );
    ) => {
        impl mlua::IntoLua for $ident {
            #[inline]
            fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                mlua::IntoLua::into_lua(&self.0[..], lua)
            }
        }
        impl mlua::FromLua for $ident {
            #[inline]
            fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
                let value = crate::script::lua::RuntimeLua::lua_tostring(lua, value)
                    .map(mlua::Value::String)?;
                <Box<str> as mlua::FromLua>::from_lua(value, lua).map(|v| Self(v.into()))
            }
        }
    };
    (@luaconv
        $vis:vis struct $ident:ident(
            $vis_field:vis $ty:ty
        );
    ) => {
    };
    (@pack
        #[pack(attr = $attr:literal $(, aliases($($attr_alias:literal),*))?)]
        $(#[$meta:meta])*
        $vis:vis struct $ident:ident(
            $(#[$meta_field:meta])*
            $vis_field:vis $ty:ty
        );
        $($($rest:tt)+)?
    ) => {
        #[derive(Debug, Clone)]
        #[repr(transparent)]
        $(#[$meta])*
        $vis struct $ident(
            $(#[$meta_field])*
            $vis_field $ty,
        );

        impl $ident {
            #[inline]
            pub fn from_ref<'a>(v: &'a $ty) -> &'a Self {
                unsafe {
                    mem::transmute(v)
                }
            }
            #[inline]
            pub fn from_mut<'a>(v: &'a mut $ty) -> &'a mut Self {
                unsafe {
                    mem::transmute(v)
                }
            }
        }

        impl<'a> From<&'a $ident> for &'a $ty {
            #[inline]
            fn from(v: &'a $ident) -> Self {
                &v.0
            }
        }
        impl<'a> From<&'a $ty> for &'a $ident {
            #[inline]
            fn from(v: &'a $ty) -> Self {
                $ident::from_ref(v)
            }
        }
        impl<'a> From<&'a mut $ident> for &'a mut $ty {
            #[inline]
            fn from(v: &'a mut $ident) -> Self {
                &mut v.0
            }
        }
        impl<'a> From<&'a mut $ty> for &'a mut $ident {
            #[inline]
            fn from(v: &'a mut $ty) -> Self {
                $ident::from_mut(v)
            }
        }
        impl From<$ident> for $ty {
            #[inline]
            fn from(v: $ident) -> Self {
                v.0
            }
        }
        impl From<$ty> for $ident {
            #[inline]
            fn from(v: $ty) -> Self {
                Self(v)
            }
        }
        impl AsRef<$ty> for $ident {
            #[inline]
            fn as_ref(&self) -> &$ty {
                &self.0
            }
        }
        impl AsMut<$ty> for $ident {
            #[inline]
            fn as_mut(&mut self) -> &mut $ty {
                &mut self.0
            }
        }
        impl ops::Deref for $ident {
            type Target = $ty;
            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl ops::DerefMut for $ident {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl AttrKey for $ident {
            type Storage = $ty;
            const ATTR: &'static str = $attr;
            const ATTR_NAMES: &'static [&'static str] = &[$attr $(, $($attr_alias),*)?];

            fn __pack_key_of() -> $crate::attributes::cell::PackKeyId
            where
                $ident: $crate::attributes::cell::AttrKeyValue,
            {
                static KEY: ::std::sync::LazyLock<$crate::attributes::cell::PackKeyId> = ::std::sync::LazyLock::new(
                    $crate::attributes::cell::PackKeyId::for_type::<$ident>
                );
                *KEY
            }
        }

        $(crate::attributes::keys::pack_key! {
            $($rest)*
        })?
    };
}
pub(crate) use pack_key;

super::cell::pack_attr! {
    impl !Default for GameMap {}
    impl !Default for TextureFile {}
    impl !Default for IconFile {}
    impl !Default for TrailDataFile {}
    impl !Default for CategoryRef {}
    impl !Default for NameId {}
    impl !Default for TipName {}
    impl !Default for TipDescription {}
    impl !Default for EditTag {}
    impl !Default for ScriptTick {}
    impl !Default for ScriptFocus {}
    impl !Default for ScriptTrigger {}
    impl !Default for ScriptFilter {}
    impl !Default for ScriptOnce {}
    impl !Default for Title {}
    impl !Default for Text {}
    impl !Default for TitleColour {}
    impl !Default for Info {}
    impl !Default for CopyValue {}
    impl !Default for CopyMessage {}
    impl !Default for Bounce {}
    impl !Default for AchievementId {}
    impl !Default for ShowCategory {}
    impl !Default for HideCategory {}
    impl !Default for ToggleCategory {}
    impl !Default for Specializations {}
    impl !Default for Raids {}
    impl !Default for MapTypes {}
    // TODO: do these impl AttrKey or no?
    impl !Default for Specialization {}
    impl !Default for Raid {}
    // TODO: unclear exactly how this interacts? there is a default bounce mode, but I don't think markers always bounce by default?
    // impl !Default for Bounce {}
}

// collection elements that aren't keys themselves...
// (see also: Festival, Mount, Race, Profession)
impl<T> GetAttr<Raid> for T where
    T: ?Sized + GetAttr<Raids>,
    // TODO: dumb hack to avoid blanket impl havoc
    T: Borrow<crate::attributes::FilterAttributes>,
{
    fn has_attr(&self) -> bool {
        GetAttr::<Raids>::get_attr(self).map(|f|
            !f.0.is_empty()
        ).unwrap_or(false)
    }
    fn get_attr(&self) -> Option<Cow<'_, Raid>> {
        GetAttr::<Raids>::get_attr(self).and_then(|f| match f {
            Cow::Borrowed(r) => r.iter().next().map(Cow::Borrowed),
            Cow::Owned(r) => r.iter().next().cloned().map(Cow::Owned),
        })
    }
}
impl<T> SetAttr<Raid> for T where
    T: ?Sized + SetAttr<Raids> + GetAttr<Raids>,
    T: Borrow<crate::attributes::FilterAttributes>,
{
    fn set_attr(&mut self, value: Raid) {
        let r = match GetAttr::<Raids>::get_attr(self) {
            Some(s) => s.iter().cloned().chain([value]).collect::<List<_>>(),
            None => List(vec![value].into()),
        };
        SetAttr::<Raids>::set_attr(self, r.into())
    }
    fn unset_attr(&mut self) {
        SetAttr::<Raids>::unset_attr(self)
    }
}
impl<T> GetAttr<Specialization> for T where
    T: ?Sized + GetAttr<Specializations>,
    // TODO: dumb hack to avoid blanket impl havoc
    T: Borrow<crate::attributes::FilterAttributes>,
{
    fn has_attr(&self) -> bool {
        GetAttr::<Specializations>::get_attr(self).map(|f|
            !f.0.is_empty()
        ).unwrap_or(false)
    }
    fn get_attr(&self) -> Option<Cow<'_, Specialization>> {
        GetAttr::<Specializations>::get_attr(self).and_then(|f| match f {
            Cow::Borrowed(r) => r.iter().next().map(Cow::Borrowed),
            Cow::Owned(r) => r.iter().next().cloned().map(Cow::Owned),
        })
    }
}
impl<T> SetAttr<Specialization> for T where
    T: ?Sized + SetAttr<Specializations> + GetAttr<Specializations>,
    T: Borrow<crate::attributes::FilterAttributes>,
{
    fn set_attr(&mut self, value: Specialization) {
        let s = match GetAttr::<Specializations>::get_attr(self) {
            Some(s) => s.iter().cloned().chain([value]).collect::<List<_>>(),
            None => List(slice::from_ref(&value).into()),
        };
        SetAttr::<Specializations>::set_attr(self, s.into())
    }
    fn unset_attr(&mut self) {
        SetAttr::<Specializations>::unset_attr(self)
    }
}
impl<T> GetAttr<MapType> for T where
    T: ?Sized + GetAttr<MapTypes>,
    // TODO: dumb hack to avoid blanket impl havoc
    T: Borrow<crate::attributes::FilterAttributes>,
{
    fn has_attr(&self) -> bool {
        GetAttr::<MapTypes>::get_attr(self).map(|f|
            !f.0.is_empty()
        ).unwrap_or(false)
    }
    fn get_attr(&self) -> Option<Cow<'_, MapType>> {
        GetAttr::<MapTypes>::get_attr(self).and_then(|f| match f {
            Cow::Borrowed(r) => r.iter().next().map(Cow::Borrowed),
            Cow::Owned(r) => r.iter().next().cloned().map(Cow::Owned),
        })
    }
}
impl<T> SetAttr<MapType> for T where
    T: ?Sized + SetAttr<MapTypes> + GetAttr<MapTypes>,
    T: Borrow<crate::attributes::FilterAttributes>,
{
    fn set_attr(&mut self, value: MapType) {
        let s = match GetAttr::<MapTypes>::get_attr(self) {
            Some(s) => s.iter().cloned().chain([value]).collect::<List<_>>(),
            None => List(slice::from_ref(&value).into()),
        };
        SetAttr::<MapTypes>::set_attr(self, s.into())
    }
    fn unset_attr(&mut self) {
        SetAttr::<MapTypes>::unset_attr(self)
    }
}
