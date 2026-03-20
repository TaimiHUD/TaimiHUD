use {
    super::{AttrList, TacoBehavior},
    crate::attributes::{AttrString, BounceBehavior, CullDirection},
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
}

#[derive(Debug, Clone, Default)]
pub struct File(pub Arc<Box<str>>);

impl File {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl FromStr for File {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Arc::new(value.into())))
    }
}

impl<S: AsRef<str>> From<S> for File {
    fn from(file: S) -> File {
        Self(Arc::new(file.as_ref().into()))
    }
}
impl<S: AsRef<str>> From<S> for IconFile {
    fn from(file: S) -> Self {
        Self(file.into())
    }
}
impl<S: AsRef<str>> From<S> for TextureFile {
    fn from(file: S) -> Self {
        Self(file.into())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Script(pub AttrString);
impl From<AttrString> for Script {
    fn from(s: AttrString) -> Self {
        Self(s.into())
    }
}
impl FromStr for Script {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(super::string_into(value)))
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Bool(pub bool);

impl Bool {
    pub const TRUE: Self = Self(true);
    pub const FALSE: Self = Self(false);
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

#[derive(Debug, Copy, Clone, Default)]
pub struct Colour(pub Vec4);

impl Colour {
    pub const WHITE: Self = Self(Vec4::ONE);
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

#[derive(Debug, Clone, Default)]
pub struct List<T>(pub Box<[T]>);
impl<T> List<T> {
    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.0.iter()
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
impl From<bool> for CanFade {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<CanFade> for bool {
    #[inline]
    fn from(v: CanFade) -> Self {
        v.0.into()
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
impl From<bool> for InGameVisibility {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<InGameVisibility> for bool {
    #[inline]
    fn from(v: InGameVisibility) -> Self {
        v.0.into()
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
impl From<bool> for MinimapVisibility {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<MinimapVisibility> for bool {
    #[inline]
    fn from(v: MinimapVisibility) -> Self {
        v.0.into()
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
impl From<bool> for MapVisibility {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<MapVisibility> for bool {
    #[inline]
    fn from(v: MapVisibility) -> Self {
        v.0.into()
    }
}
impl FadeNear {
    pub const DEFAULT: Self = Self(-1.0);
}
impl Default for FadeNear {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl FadeFar {
    pub const DEFAULT: Self = Self(-1.0);
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

impl Default for Tint {
    fn default() -> Self {
        Self(Colour::WHITE)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Point3(pub Vec3);

pack_key! {
    #[pack(attr = "specialization")]
    pub struct Specializations(pub List<Specialization>);
    #[pack(attr = "maptype")]
    pub struct MapTypes(pub List<super::MapType>);
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
    pub fn from_attrlist(v: &AttrList<super::MapType>) -> &Self {
        let list: &Box<[super::MapType]> = &**v;
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
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

// TODO: more attrs are on the category/trail/etc structs, some optional and some required

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
    #[derive(Copy)]
    pub struct RotateX(pub f32);
    #[pack(attr = "rotate-y")]
    #[derive(Copy)]
    pub struct RotateY(pub f32);
    #[pack(attr = "rotate-z")]
    #[derive(Copy)]
    pub struct RotateZ(pub f32);
    #[pack(attr = "xpos")]
    #[derive(Copy)]
    pub struct PositionX(pub f32);
    #[pack(attr = "ypos")]
    #[derive(Copy)]
    pub struct PositionY(pub f32);
    #[pack(attr = "zpos")]
    #[derive(Copy)]
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
impl From<bool> for ScaleOnMapWithZoom {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<ScaleOnMapWithZoom> for bool {
    #[inline]
    fn from(v: ScaleOnMapWithZoom) -> Self {
        v.0.into()
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
impl From<bool> for Occlude {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<Occlude> for bool {
    #[inline]
    fn from(v: Occlude) -> Self {
        v.0.into()
    }
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
        let Some(value) = str_opt(value) else { return Ok(Self::DEFAULT) };
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
    pub const TACO_MIN: u32 = 0;
    pub const TACO_MAX: u32 = 99;

    pub const fn value(self) -> u8 {
        self as u8
    }
    pub const unsafe fn from_value_unchecked(value: u8) -> Self {
        mem::transmute(value)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BlishBehaviour {
    ResetWeekly = 101,
    /// "dismiss" an achievement-filtered marker
    TaimiAchievement = 33,
}
impl BlishBehaviour {
    pub const fn value(self) -> u8 {
        self as u8
    }
    pub const unsafe fn from_value_unchecked(value: u8) -> Self {
        mem::transmute(value)
    }
}
/// TODO
pub use self::BlishBehaviour as TaimiBehaviour;
impl TaimiBehaviour {
    pub const TAIMI_MIN: u32 = 33;
    pub const TAIMI_MAX: u32 = 45;
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Behaviour {
    Taco(TacoBehaviour),
    Blish(BlishBehaviour),
}
impl Behaviour {
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
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Taco(TacoBehaviour::AlwaysVisible))
    }

    #[inline]
    pub fn Taimi(b: TaimiBehaviour) -> Self {
        Self::Blish(b)
    }
}
impl AttrKey for Behaviour {
    type Storage = u8;
    const ATTR: &'static str = "behavior";
    const ATTR_NAMES: &'static [&'static str] = &[Self::ATTR];
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
        match behaviour {
            Behaviour::Taco(b) => b.into(),
            Behaviour::Blish(b) => b.into(),
        }
    }
}
impl From<TacoBehavior> for Behaviour {
    fn from(value: TacoBehavior) -> Self {
        unsafe {
            match value as u32 {
                TaimiBehaviour::TAIMI_MIN..=TaimiBehaviour::TAIMI_MAX =>
                    Self::Taimi(TaimiBehaviour::from_value_unchecked(value as u8)),
                TacoBehaviour::TACO_MIN..=TacoBehaviour::TACO_MAX =>
                    Self::Taco(TacoBehaviour::from_value_unchecked(value as u8)),
                _ => Self::Blish(BlishBehaviour::from_value_unchecked(value as u8)),
            }
        }
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
impl From<bool> for InvertBehaviour {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<InvertBehaviour> for bool {
    #[inline]
    fn from(v: InvertBehaviour) -> Self {
        v.0.into()
    }
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
impl From<bool> for IsWall {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<IsWall> for bool {
    #[inline]
    fn from(v: IsWall) -> Self {
        v.0.into()
    }
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
impl From<bool> for DefaultToggle {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<DefaultToggle> for bool {
    #[inline]
    fn from(v: DefaultToggle) -> Self {
        v.0.into()
    }
}
impl IsHidden {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl From<bool> for IsHidden {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<IsHidden> for bool {
    #[inline]
    fn from(v: IsHidden) -> Self {
        v.0.into()
    }
}
impl IsSeparator {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl From<bool> for IsSeparator {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<IsSeparator> for bool {
    #[inline]
    fn from(v: IsSeparator) -> Self {
        v.0.into()
    }
}

// Modifiers
pack_key! {
    #[pack(attr = "type")]
    pub struct CategoryRef(pub AttrString);
    #[pack(attr = "name")]
    pub struct NameId(pub AttrString);
    #[pack(attr = "mapid")]
    #[derive(Copy)]
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
    pub struct ResetGuid(pub List<Guid>);
    #[pack(attr = "category", aliases("togglecategory"))]
    #[derive(Copy, Default)]
    pub struct ToggleCategory(pub Bool);
    #[pack(attr = "show")]
    #[derive(Copy, Default)]
    pub struct ShowCategory(pub Bool);
    #[pack(attr = "hide")]
    #[derive(Copy, Default)]
    pub struct HideCategory(pub Bool);
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

impl AutoTrigger {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl From<bool> for AutoTrigger {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<AutoTrigger> for bool {
    #[inline]
    fn from(v: AutoTrigger) -> Self {
        v.0.into()
    }
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

impl ToggleCategory {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl From<bool> for ToggleCategory {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<ToggleCategory> for bool {
    #[inline]
    fn from(v: ToggleCategory) -> Self {
        v.0.into()
    }
}
impl ShowCategory {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl From<bool> for ShowCategory {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<ShowCategory> for bool {
    #[inline]
    fn from(v: ShowCategory) -> Self {
        v.0.into()
    }
}
impl HideCategory {
    pub const DEFAULT: Self = Self(Bool::FALSE);
}
impl From<bool> for HideCategory {
    #[inline]
    fn from(v: bool) -> Self {
        Self(v.into())
    }
}
impl From<HideCategory> for bool {
    #[inline]
    fn from(v: HideCategory) -> Self {
        v.0.into()
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

impl From<i32> for AchievementId {
    #[inline]
    fn from(id: i32) -> Self {
        Self(id as _)
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
impl From<u16> for AchievementBit {
    #[inline]
    fn from(id: u16) -> Self {
        Self(id as _)
    }
}

impl IntoIterator for ResetGuid {
    type Item = Guid;
    type IntoIter = <List<Guid> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<G: Into<Guid>> FromIterator<G> for ResetGuid {
    fn from_iter<I: IntoIterator<Item = G>>(iter: I) -> Self {
        Self(iter.into_iter().map(Into::into).collect())
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

    pub fn is_empty(&self) -> bool {
        self.0.is_nil()
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
impl From<Guid> for Uuid {
    #[inline]
    fn from(v: Guid) -> Self {
        v.0
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

pub trait GetAttr<A: ?Sized + AttrKey> {
    fn has_attr(&self) -> bool {
        self.get_attr_ref().is_some()
    }
    fn get_attr_ref(&self) -> Option<&A>;
    #[cfg(todo)]
    fn get_attr_storage(&self) -> Option<&A::Storage>;
    fn get_attr(&self) -> Option<Cow<'_, A>> {
        self.get_attr_ref().map(Cow::Borrowed)
    }
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
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis AttrString
        );
    ) => {
        impl FromStr for $ident {
            type Err = Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(super::string_into(s)))
            }
        }
    };
    (@fromstr
        $vis:vis struct $ident:ident(
            $vis_field:vis $ty:ty
        );
    ) => {
        impl FromStr for $ident {
            type Err = <$ty as FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <$ty as FromStr>::from_str(s)
                    .map(Self)
            }
        }
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
        }

        $(crate::attributes::keys::pack_key! {
            $($rest)*
        })?
    };
}
pub(crate) use pack_key;
