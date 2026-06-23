use {
    crate::{attributes::keys::{AttrKey, GetAttr, SetAttr}, category::id::IdCmpRelaxed},
    core::{
        borrow::{Borrow, BorrowMut},
        cmp,
        fmt,
        marker::PhantomData,
        mem,
        num::NonZero,
        ops,
        ptr::{self, NonNull},
        str::FromStr,
    },
    std::{
        any::{Any, TypeId},
        borrow::Cow,
        collections::BTreeMap,
        sync::{Arc, RwLock},
    },
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PackKeyId {
    id: u8,
}
impl PackKeyId {
    pub const unsafe fn new_unchecked(id: u8) -> Self {
        Self { id }
    }

    pub const fn index(&self) -> u8 {
        self.id
    }

    #[inline]
    pub fn for_type<T: AttrKeyValue>() -> Self {
        Self::try_for_type::<T>().expect("pack key registry oom")
    }
    pub fn try_for_type<T: AttrKeyValue>() -> Option<Self> {
        let reg = PackKeyRegistration::for_type::<T>();
        PackKeyRegistry::registry_register(reg)
    }

    pub fn attr_names(self) -> &'static [&'static str] {
        PackKeyRegistry::registry_with(self, |reg| reg.attr_names)
    }
    pub fn value_type(self) -> TypeId {
        PackKeyRegistry::registry_with(self, |reg| reg.value_type)
    }
    pub fn value_size(self) -> usize {
        PackKeyRegistry::registry_with(self, |reg| reg.value_size)
    }
    pub fn vtable_ptr(self) -> *const () {
        PackKeyRegistry::registry_with(self, |reg| reg.vtable) as *const ()
    }
    pub fn lookup_by_attr(a: &str) -> Option<Self> {
        PackKeyRegistry::lookup_by_attr(a)
    }
    pub fn try_from_str(self, s: &str) -> anyhow::Result<PackValueCell> {
        let try_from_str = PackKeyRegistry::registry_with(self, |reg| reg.try_from_str);
        try_from_str(s)
    }
    pub fn all_keys() -> impl Iterator<Item = PackKeyId> + Send + Sync + Clone + 'static {
        let amt = PackKeyRegistry::global()
            .read()
            .ok()
            .map(|reg| reg.registrations.len());
        let amt = match amt {
            Some(amt @ 0..=0xff) => amt as u8,
            _ => 0u8,
        };
        IntoIterator::into_iter(0..amt).map(|idx| unsafe { PackKeyId::new_unchecked(idx) })
    }
}
impl fmt::Display for PackKeyId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(attr) = self.attr_names().get(0) {
            fmt::Display::fmt(attr, f)
        } else {
            fmt::Display::fmt(&self.id, f)
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PackKeyRegistration {
    pub value_type: TypeId,
    pub value_size: usize,
    pub vtable: usize,
    pub try_from_str: fn(&str) -> anyhow::Result<PackValueCell>,
    #[cfg(todo)]
    pub empty: Option<&'static LazyLock<PackValueCell>>,
    pub attr_names: &'static [&'static str],
}
impl PackKeyRegistration {
    pub fn value_type(&self) -> Option<TypeId> {
        match self.value_type == Self::value_type_unknown() {
            true => None,
            false => Some(self.value_type),
        }
    }

    #[inline]
    pub fn value_type_unknown() -> TypeId {
        TypeId::of::<[()]>()
    }

    pub fn for_type<T: AttrKeyValue>() -> Self {
        debug_assert!(
            mem::align_of::<T>() <= mem::align_of::<usize>()
                || mem::size_of::<T>() >= mem::size_of::<usize>()
        );
        Self {
            value_type: TypeId::of::<T>(),
            value_size: mem::size_of::<T>(),
            vtable: T::vtable_ptr() as usize,
            try_from_str: T::try_from_str,
            #[cfg(todo)]
            empty: T::get_default_fn().map(|_| {
                let lock = LazyLock::new(get_default_cell_unchecked::<T> as fn() -> PackValueCell);
                &*Box::leak(Box::new(lock))
            }),
            attr_names: T::attr_names(),
        }
    }
}
/// TODO: ew
#[cfg(todo)]
unsafe impl Send for PackKeyRegistration {}
/// TODO: ew
#[cfg(todo)]
unsafe impl Sync for PackKeyRegistration {}
/// TODO: ew
#[cfg(todo)]
fn get_default_cell_unchecked<T>() -> PackValueCell
where
    T: AttrKeyValue,
{
    unsafe {
        let default = T::get_default_fn().unwrap_unchecked();
        PackValueCell::new_boxed_unchecked(default())
    }
}

#[derive(Debug)]
pub struct PackKeyRegistry {
    registrations: Vec<PackKeyRegistration>,
    id_lookup: BTreeMap<TypeId, PackKeyId>,
    /// TODO: FxHashMap?
    keys_by_attr: BTreeMap<&'static IdCmpRelaxed<str>, PackKeyId>,
}
impl PackKeyRegistry {
    const EMPTY: Self = Self {
        registrations: Vec::new(),
        id_lookup: BTreeMap::new(),
        keys_by_attr: BTreeMap::new(),
    };

    /// TODO: just preallocate 256 registrations then use an atomic growing len? .-.
    fn global() -> &'static RwLock<PackKeyRegistry> {
        static GLOBAL: RwLock<PackKeyRegistry> = RwLock::new(PackKeyRegistry::EMPTY);
        &GLOBAL
    }
    pub fn registry_register(reg: PackKeyRegistration) -> Option<PackKeyId> {
        Self::global().write().ok()?.register(reg)
    }
    pub fn registry_with<R, F: FnOnce(&PackKeyRegistration) -> R>(id: PackKeyId, f: F) -> R {
        f(Self::global().read().expect("pack registry poisoned").get(id))
    }
    pub fn registry_get(id: PackKeyId) -> PackKeyRegistration {
        Self::registry_with(id, Clone::clone)
    }
    pub fn registry_lookup(value_type: &TypeId) -> Option<PackKeyId> {
        Self::global().read().ok()?.lookup(value_type)
    }

    pub fn register(&mut self, reg: PackKeyRegistration) -> Option<PackKeyId> {
        if let Some(id) = self.lookup(&reg.value_type) {
            return Some(id)
        }
        let index = match self.registrations.len() {
            i @ 0..=0xff => unsafe { PackKeyId::new_unchecked(i as u8) },
            // out of space :<
            _ => return None,
        };
        let value_type = reg.value_type;
        self.registrations.push(reg);
        self.id_lookup.insert(value_type, index);
        self.keys_by_attr
            .extend(reg.attr_names.iter().map(|a| (IdCmpRelaxed::with_ref(*a), index)));
        Some(index)
    }
    pub fn get(&self, id: PackKeyId) -> &PackKeyRegistration {
        unsafe { self.registrations.get_unchecked(id.index() as usize) }
    }
    pub fn lookup(&self, value_type: &TypeId) -> Option<PackKeyId> {
        self.id_lookup.get(&value_type).cloned()
    }
    /// case-insensitive
    pub fn lookup_by_attr(a: &str) -> Option<PackKeyId> {
        let query = IdCmpRelaxed::with_ref(a);
        Self::global()
            .read()
            .ok()
            .and_then(|r| r.keys_by_attr.get(query).copied())
    }
}

pub unsafe trait AttrKeyValue: Any + Send + Sync {
    fn pack_key_id(&self) -> PackKeyId;
    /// TODO: return PackValueCell instead?
    fn clone_dyn(&self) -> Option<Box<dyn AttrKeyValue>>;
    fn pack_key_of() -> PackKeyId
    where
        Self: Sized;
    // TODO: these should just be moved over to a StaticVtable struct's fields
    fn attr_names() -> &'static [&'static str]
    where
        Self: Sized;
    fn vtable_ptr() -> *const ()
    where
        Self: Sized;
    fn try_from_str(s: &str) -> anyhow::Result<PackValueCell>
    where
        Self: Sized;
    fn get_default_fn() -> Option<DefaultFn<Self>>
    where
        Self: Sized;
}
impl dyn AttrKeyValue {
    #[inline(always)]
    pub fn downcast_ptr_mut<A>(p: *mut dyn AttrKeyValue) -> *mut A
    where
        A: AttrKeyValue,
    {
        p as *mut A
    }
    #[inline(always)]
    pub fn downcast_ptr<A>(p: *const dyn AttrKeyValue) -> *const A
    where
        A: AttrKeyValue,
    {
        p as *const A
    }
    #[inline(always)]
    pub unsafe fn downcast_ref_unchecked<A>(p: &dyn AttrKeyValue) -> &A
    where
        A: AttrKeyValue,
    {
        &*Self::downcast_ptr(p)
    }
    #[inline(always)]
    pub unsafe fn downcast_mut_unchecked<A>(p: &mut dyn AttrKeyValue) -> &mut A
    where
        A: AttrKeyValue,
    {
        &mut *Self::downcast_ptr_mut(p)
    }
    #[inline]
    pub unsafe fn downcast_box_unchecked<A>(p: Box<dyn AttrKeyValue>) -> Box<A>
    where
        A: AttrKeyValue,
    {
        Box::from_raw(Self::downcast_ptr_mut(Box::into_raw(p)))
    }
    #[inline]
    pub unsafe fn downcast_arc_unchecked<A>(p: Arc<dyn AttrKeyValue>) -> Arc<A>
    where
        A: AttrKeyValue,
    {
        Arc::from_raw(Self::downcast_ptr(Arc::into_raw(p)))
    }
}
unsafe impl<A> AttrKeyValue for A
where
    A: AttrKey + Any + Send + Sync + Clone + FromStr + MaybeDefault,
    <A as FromStr>::Err: Into<anyhow::Error>,
{
    #[inline(always)]
    fn attr_names() -> &'static [&'static str] {
        <A as AttrKey>::ATTR_NAMES
    }
    fn vtable_ptr() -> *const () {
        let dummy = mem::MaybeUninit::<A>::uninit();
        let p: *const dyn AttrKeyValue = &raw const *dummy.as_ptr();
        vtable_ptr_of(p)
    }
    fn try_from_str(s: &str) -> anyhow::Result<PackValueCell>
    where
        Self: Sized,
    {
        Self::from_str(s).map(PackValueCell::new).map_err(Into::into)
    }
    #[inline]
    fn get_default_fn() -> Option<DefaultFn<Self>>
    where
        Self: Sized,
    {
        MaybeDefault::get_default_fn()
    }

    fn pack_key_id(&self) -> PackKeyId {
        Self::pack_key_of()
    }
    fn pack_key_of() -> PackKeyId
    where
        Self: Sized,
    {
        // allow "specialization" despite this being a blanket impl
        match () {
            #[cfg(todo = "unnecessary")]
            _ => PackKeyId::for_type::<Self>(),
            _ => Self::__pack_key_of(),
        }
    }
    fn clone_dyn(&self) -> Option<Box<dyn AttrKeyValue>> {
        Some(Box::new(self.clone()) as Box<_>)
    }
}
/// all ptr metadata fns are unstable, so we have fun here :3
fn vtable_ptr_of(p: *const dyn AttrKeyValue) -> *const () {
    let [_, vtbl] = unsafe { mem::transmute::<*const dyn AttrKeyValue, [*const (); 2]>(p) };
    vtbl
}
unsafe fn to_vtable_ptr(p: usize, vtbl: usize) -> *mut dyn AttrKeyValue {
    unsafe { mem::transmute::<[usize; 2], *mut dyn AttrKeyValue>([p, vtbl]) }
}

#[repr(C)]
pub struct PackValueCell {
    id: PackKeyId,
    flag: u8,
    padding: [u8; 6],
    value: usize,
    inner: PhantomData<Arc<dyn AttrKeyValue>>,
}
impl PackValueCell {
    pub const PADDING_SIZE: usize = 6;
    pub const PADDING_EMPTY: [u8; Self::PADDING_SIZE] = [0u8; Self::PADDING_SIZE];
    pub const FLAG_MASK_STORAGE: u8 = 0x07;
    pub const FLAG_INVALID: u8 = 0;
    pub const FLAG_EMPTY: u8 = 1;
    pub const FLAG_INLINE_COPY: u8 = 2;
    pub const FLAG_BOX: u8 = 3;
    pub const FLAG_ARC: u8 = 4;
    pub const INLINE_SIZE: usize = size_of::<usize>();

    pub fn new<A: AttrKeyValue>(value: A) -> Self {
        Self::from_arc(Arc::new(value) as Arc<dyn AttrKeyValue>)
    }
    pub fn copy<A: AttrKeyValue + Copy>(value: A) -> Self {
        let id = value.pack_key_id();
        let ptr = &value as *const dyn AttrKeyValue as *const ();
        unsafe { Self::from_copy_unchecked(NonNull::new_unchecked(ptr as *mut _), id) }
    }
    /// TODO: an actual marker type for Copy?
    #[inline(always)]
    pub fn new_boxed<A: AttrKeyValue + Clone>(value: A) -> Self {
        let id = value.pack_key_id();
        match mem::size_of::<A>() {
            0..=Self::INLINE_SIZE if !mem::needs_drop::<A>() => unsafe {
                let value = mem::ManuallyDrop::new(value);
                let ptr = &*value as *const A as *const dyn AttrKeyValue as *const ();
                Self::from_copy_unchecked(NonNull::new_unchecked(ptr as *mut _), id)
            },
            _ => unsafe { Self::from_box_unchecked(Box::into_raw(Box::new(value)), id) },
        }
    }
    /// `A` safe to copy or move? who knows!
    #[inline(always)]
    pub unsafe fn new_boxed_unchecked<A: AttrKeyValue>(value: A) -> Self {
        let id = value.pack_key_id();
        match mem::size_of::<A>() {
            0..=Self::INLINE_SIZE => unsafe {
                let value = mem::ManuallyDrop::new(value);
                let ptr = &*value as *const A as *const dyn AttrKeyValue as *const ();
                Self::from_copy_unchecked(NonNull::new_unchecked(ptr as *mut _), id)
            },
            _ => unsafe { Self::from_box_unchecked(Box::into_raw(Box::new(value)), id) },
        }
    }

    pub fn from_box(inner: Box<dyn AttrKeyValue>) -> Self {
        let id = inner.pack_key_id();
        let ptr = Box::into_raw(inner);
        unsafe { Self::from_box_unchecked(ptr, id) }
    }

    pub fn from_arc(inner: Arc<dyn AttrKeyValue>) -> Self {
        let id = inner.pack_key_id();
        let ptr = Arc::into_raw(inner);
        unsafe { Self::from_arc_unchecked(ptr, id) }
    }

    pub unsafe fn from_arc_unchecked(ptr: *const dyn AttrKeyValue, id: PackKeyId) -> Self {
        #[cfg(debug_assertions)]
        if id.vtable_ptr() != vtable_ptr_of(ptr) {
            debug_assert_eq!(id, (&*ptr).pack_key_id());
        }
        unsafe { Self::from_inner_unchecked(ptr as *const () as usize, id, Self::FLAG_ARC) }
    }
    pub unsafe fn from_box_unchecked(ptr: *mut dyn AttrKeyValue, id: PackKeyId) -> Self {
        #[cfg(debug_assertions)]
        if id.vtable_ptr() != vtable_ptr_of(ptr) {
            debug_assert_eq!(id, (&*ptr).pack_key_id());
        }
        unsafe { Self::from_inner_unchecked(ptr as *mut () as usize, id, Self::FLAG_BOX) }
    }
    /// TODO: this should just be a trait method or something...
    pub unsafe fn from_copy_unchecked(value: NonNull<()>, id: PackKeyId) -> Self {
        let size = id.value_size();
        debug_assert!(size <= Self::INLINE_SIZE);
        let mut out = [0u8; Self::INLINE_SIZE];
        ptr::copy_nonoverlapping(value.as_ptr() as *const () as *const u8, out.as_mut_ptr(), size);
        Self::from_inner_unchecked(usize::from_ne_bytes(out), id, Self::FLAG_INLINE_COPY)
    }
    pub unsafe fn from_inner_unchecked(value: usize, id: PackKeyId, flag: u8) -> Self {
        Self {
            id,
            value,
            flag,
            padding: Self::PADDING_EMPTY,
            inner: PhantomData,
        }
    }

    /// TODO: would there be any advantage to defining this undefined
    /// when `self.flag_storage() == Self::FLAG_INVALID` so all-zeroed is valid
    /// (even if no keys are registered)?
    #[inline]
    pub fn id(&self) -> PackKeyId {
        self.id
    }

    pub fn is_valid(&self) -> bool {
        match self.flag_storage() {
            Self::FLAG_ARC | Self::FLAG_BOX => self.value != 0,
            Self::FLAG_INLINE_COPY => true,
            Self::FLAG_EMPTY | _ => false,
        }
    }

    pub fn raw_ptr(&self) -> *const () {
        match self.flag_storage() {
            Self::FLAG_EMPTY | Self::FLAG_ARC | Self::FLAG_BOX => self.value as *const (),
            Self::FLAG_INLINE_COPY => {
                // I'll make this valid someday probably..?
                &raw const self.value as *const ()
            },
            _ => ptr::null(),
        }
    }
    pub fn raw_ptr_mut(&mut self) -> *mut () {
        match self.flag_storage() {
            Self::FLAG_EMPTY | Self::FLAG_ARC | Self::FLAG_BOX => self.value as *mut (),
            Self::FLAG_INLINE_COPY => {
                // I'll make this valid someday probably..?
                &raw mut self.value as *mut ()
            },
            _ => ptr::null_mut(),
        }
    }
    #[inline]
    pub fn vtable_ptr(&self) -> *const () {
        match self.flag_storage() {
            Self::FLAG_INVALID => ptr::null_mut(),
            _ => self.id.vtable_ptr(),
        }
    }
    pub fn ptr(&self) -> *const dyn AttrKeyValue {
        let v = self.vtable_ptr() as usize;
        unsafe { to_vtable_ptr(self.raw_ptr() as usize, v) }
    }
    /// ptr coming from a mutable borrow silliness...
    pub fn ptr_mut(&mut self) -> *mut dyn AttrKeyValue {
        let v = self.vtable_ptr() as usize;
        unsafe { to_vtable_ptr(self.raw_ptr_mut() as usize, v) }
    }
    pub fn get(&self) -> Option<&dyn AttrKeyValue> {
        let p = self.ptr();
        (!p.is_null()).then(move || unsafe { &*p })
    }
    pub fn get_mut(&mut self) -> Option<&mut dyn AttrKeyValue> {
        let p = self.ptr_mut();
        (!p.is_null()).then(move || unsafe { &mut *p })
    }
    #[inline(always)]
    pub fn flag_storage(&self) -> u8 {
        self.flag & Self::FLAG_MASK_STORAGE
    }
    pub fn into_arc(self) -> Option<Arc<dyn AttrKeyValue>> {
        if self.flag_storage() != Self::FLAG_ARC {
            return None
        }
        let this = mem::ManuallyDrop::new(self);
        let ptr = this.ptr();
        Some(unsafe { Arc::from_raw(ptr) })
    }
    pub fn into_box(self) -> Option<Box<dyn AttrKeyValue>> {
        if self.flag_storage() != Self::FLAG_BOX {
            return None
        }
        let mut this = mem::ManuallyDrop::new(self);
        let ptr = this.ptr_mut();
        Some(unsafe { Box::from_raw(ptr) })
    }
    /// beware of leaking allocations...
    pub fn into_inner(self) -> (usize, u8) {
        let this = mem::ManuallyDrop::new(self);
        (this.value, this.flag)
    }

    #[inline]
    pub fn emptied(&self) -> Self {
        Self::new_empty(self.id)
    }
    pub fn empty<A: AttrKeyValue + Sized>() -> Self {
        Self::new_empty(A::pack_key_of())
    }
    #[inline]
    pub fn new_empty(id: PackKeyId) -> Self {
        unsafe { Self::from_inner_unchecked(0, id, Self::FLAG_EMPTY) }
    }

    pub fn try_get_ptr<A: AttrKeyValue + Sized>(&self) -> Option<ptr::NonNull<A>> {
        #[cfg(todo = "unnecessary")]
        if self.flag_storage() == Self::FLAG_INVALID {
            return None
        }
        let id = A::pack_key_of();
        if id != self.id {
            return None
        }

        ptr::NonNull::new(self.ptr() as *const dyn AttrKeyValue as *const A as *mut A)
    }
    pub fn try_get_ptr_mut<A: AttrKeyValue + Sized>(&mut self) -> Option<ptr::NonNull<A>> {
        #[cfg(todo = "unnecessary")]
        if self.flag_storage() == Self::FLAG_INVALID {
            return None
        }
        let id = A::pack_key_of();
        if id != self.id {
            return None
        }

        ptr::NonNull::new(self.ptr_mut() as *mut dyn AttrKeyValue as *mut A)
    }
    pub fn try_get<A: AttrKeyValue + Sized>(&self) -> Option<&A> {
        self.try_get_ptr().map(|p| unsafe { &*p.as_ptr() })
    }
    pub fn try_get_mut<A: AttrKeyValue + Sized>(&mut self) -> Option<&mut A> {
        self.try_get_ptr_mut().map(|p| unsafe { &mut *p.as_ptr() })
    }
    pub fn try_copy<A: AttrKeyValue + Copy>(&self) -> Option<A> {
        self.try_get_ptr().map(|p| unsafe { ptr::read(p.as_ptr()) })
    }
    pub fn set<A: AttrKeyValue>(&mut self, value: A) -> Option<A> {
        self.try_get_ptr_mut()
            .map(|p| unsafe { ptr::replace(p.as_ptr(), value) })
    }
}
impl Clone for PackValueCell {
    fn clone(&self) -> Self {
        match self.flag_storage() {
            Self::FLAG_ARC => unsafe {
                let ptr = self.ptr();
                Arc::increment_strong_count(ptr);
                Self::from_arc_unchecked(ptr, self.id)
            },
            Self::FLAG_BOX => unsafe {
                let value = match self.get().and_then(|v| v.clone_dyn()) {
                    Some(v) => v,
                    None => return self.emptied(),
                };
                let ptr = Box::into_raw(value);
                Self::from_box_unchecked(ptr, self.id)
            },
            Self::FLAG_INLINE_COPY => unsafe { Self::from_inner_unchecked(self.value, self.id, self.flag) },
            _ => self.emptied(),
        }
    }
}
impl Drop for PackValueCell {
    fn drop(&mut self) {
        match self.flag_storage() {
            Self::FLAG_ARC => unsafe { Arc::decrement_strong_count(self.ptr()) },
            Self::FLAG_BOX => unsafe {
                drop(Box::from_raw(self.ptr_mut()));
            },
            _ => (),
        }
    }
}
impl Borrow<PackKeyId> for PackValueCell {
    #[inline]
    fn borrow(&self) -> &PackKeyId {
        &self.id
    }
}
impl fmt::Debug for PackValueCell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_tuple("PackValueCell");
        if self.flag_storage() == Self::FLAG_INVALID {
            return f.finish()
        }
        f.field(&format_args!("{}", self.id));
        match self.flag_storage() {
            Self::FLAG_EMPTY => {
                f.field(&None::<()>);
            },
            Self::FLAG_BOX => {
                f.field(&taimi_hoard::lazyfmt::fmt_fn(|f| {
                    f.debug_tuple("Box").field(&self.ptr()).finish()
                }));
            },
            Self::FLAG_ARC => {
                f.field(&taimi_hoard::lazyfmt::fmt_fn(|f| {
                    f.debug_tuple("Arc").field(&self.ptr()).finish()
                }));
            },
            _ => {
                f.field(&(self.value as *const ()));
            },
        }
        f.finish()
    }
}
/// TODO: require fmt::Display on attrs soon
#[repr(transparent)]
#[derive(Clone)]
pub struct PackValueOf<T: ?Sized + AttrKeyValue> {
    value: PackValueCell,
    _inner: PhantomData<T>,
}
pub type PackValueDyn = PackValueOf<dyn AttrKeyValue>;
impl<T> PackValueOf<T>
where
    T: ?Sized + AttrKeyValue,
{
    #[inline]
    pub unsafe fn new_unchecked(value: PackValueCell) -> Self {
        Self { value, _inner: PhantomData }
    }
    #[inline(always)]
    pub unsafe fn from_ref_unchecked(value: &PackValueCell) -> &Self {
        mem::transmute(value)
    }
    #[inline(always)]
    pub unsafe fn from_mut_unchecked(value: &mut PackValueCell) -> &mut Self {
        mem::transmute(value)
    }
    #[inline(always)]
    pub fn inner(&self) -> &PackValueCell {
        &self.value
    }
    #[inline(always)]
    pub unsafe fn inner_mut(&mut self) -> &mut PackValueCell {
        &mut self.value
    }
    #[inline(always)]
    pub fn into_inner(self) -> PackValueCell {
        self.value
    }
    #[inline]
    pub fn get_dyn(&self) -> &dyn AttrKeyValue {
        unsafe { &*(self.value.ptr()) }
    }
    #[inline]
    pub fn get_dyn_mut(&mut self) -> &mut dyn AttrKeyValue {
        unsafe { &mut *(self.value.ptr_mut()) }
    }

    #[inline]
    pub fn as_dyn(&self) -> &PackValueDyn {
        unsafe { PackValueDyn::from_ref_unchecked(self.inner()) }
    }
    #[inline]
    pub fn as_dyn_mut(&mut self) -> &mut PackValueDyn {
        unsafe { PackValueDyn::from_mut_unchecked(self.inner_mut()) }
    }
    #[inline]
    pub fn into_dyn(self) -> PackValueDyn {
        unsafe { PackValueDyn::new_unchecked(self.into_inner()) }
    }
}
impl PackValueDyn {
    #[inline(always)]
    pub fn new_arc_dyn<T: AttrKeyValue>(value: T) -> Self {
        unsafe { Self::new_unchecked(PackValueCell::new(value)) }
    }
    #[inline(always)]
    pub fn new_boxed_dyn<T: AttrKeyValue + Clone>(value: T) -> Self {
        unsafe { Self::new_unchecked(PackValueCell::new_boxed(value)) }
    }
    #[inline(always)]
    pub fn from_box_dyn(value: Box<dyn AttrKeyValue>) -> Self {
        unsafe { Self::new_unchecked(PackValueCell::from_box(value)) }
    }
    #[inline(always)]
    pub fn from_arc_dyn(value: Arc<dyn AttrKeyValue>) -> Self {
        unsafe { Self::new_unchecked(PackValueCell::from_arc(value)) }
    }
    #[inline]
    pub fn from_cell_dyn(value: PackValueCell) -> Option<Self> {
        value.is_valid().then(|| unsafe { Self::new_unchecked(value) })
    }
    #[inline]
    pub fn from_cell_dyn_ref(value: &PackValueCell) -> Option<&Self> {
        value
            .is_valid()
            .then(|| unsafe { Self::from_ref_unchecked(value) })
    }
    #[inline]
    pub fn from_cell_dyn_mut(value: &mut PackValueCell) -> Option<&mut Self> {
        value
            .is_valid()
            .then(|| unsafe { Self::from_mut_unchecked(value) })
    }

    /// `&None::<Self>`
    #[inline(always)]
    pub fn none_ref() -> &'static Option<Self> {
        struct SyncUnsafeCell<T>(T);
        unsafe impl<T> Send for SyncUnsafeCell<T> {}
        unsafe impl<T> Sync for SyncUnsafeCell<T> {}
        static NONE: SyncUnsafeCell<Option<PackValueDyn>> = SyncUnsafeCell(None);
        &NONE.0
    }
}
impl<T: AttrKeyValue + Sized> PackValueOf<T> {
    pub fn from_cell(value: PackValueCell) -> Option<Self> {
        (value.is_valid() && value.id() == T::pack_key_of()).then(|| unsafe { Self::new_unchecked(value) })
    }
    pub fn from_cell_ref(value: &PackValueCell) -> Option<&Self> {
        (value.is_valid() && value.id() == T::pack_key_of())
            .then(|| unsafe { Self::from_ref_unchecked(value) })
    }
    #[inline]
    pub fn new(value: T) -> Self {
        unsafe { Self::new_unchecked(PackValueCell::new(value)) }
    }
    #[inline]
    pub fn new_boxed(value: T) -> Self
    where
        T: Clone,
    {
        unsafe { Self::new_unchecked(PackValueCell::new_boxed(value)) }
    }
    #[inline(always)]
    pub fn get(&self) -> &T {
        unsafe { &*(self.value.ptr() as *const dyn AttrKeyValue as *const T) }
    }
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *(self.value.ptr_mut() as *mut dyn AttrKeyValue as *mut T) }
    }
    #[inline]
    pub fn value(&self) -> T
    where
        T: Clone,
    {
        self.get().clone()
    }
    pub fn into_value_arc(self) -> Option<Arc<T>> {
        self.value
            .into_arc()
            .map(|v| unsafe { <dyn AttrKeyValue>::downcast_arc_unchecked(v) })
    }
    pub fn into_value_box(self) -> Option<Box<T>> {
        self.value
            .into_box()
            .map(|v| unsafe { <dyn AttrKeyValue>::downcast_box_unchecked(v) })
    }
    pub fn to_value(self) -> Option<T>
    where
        T: Clone,
    {
        match self.value.flag_storage() {
            PackValueCell::FLAG_ARC => Some(Arc::unwrap_or_clone(unsafe {
                self.into_value_arc().unwrap_unchecked()
            })),
            _ => self.into_value(),
        }
    }
    pub fn into_value(self) -> Option<T> {
        match self.value.flag_storage() {
            PackValueCell::FLAG_ARC => Arc::into_inner(unsafe { self.into_value_arc().unwrap_unchecked() }),
            PackValueCell::FLAG_BOX => Some(unsafe { *self.into_value_box().unwrap_unchecked() }),
            PackValueCell::FLAG_INLINE_COPY => Some(unsafe {
                let this = mem::ManuallyDrop::new(self);
                let p = <dyn AttrKeyValue>::downcast_ptr::<T>(this.get());
                ptr::read(p)
            }),
            #[cfg(debug_assertions)]
            PackValueCell::FLAG_INVALID => panic!("invalid cell"),
            PackValueCell::FLAG_EMPTY | _ => None,
        }
    }
}
impl<T> From<PackValueOf<T>> for PackValueDyn
where
    T: Sized + AttrKeyValue,
{
    #[inline]
    fn from(v: PackValueOf<T>) -> Self {
        v.into_dyn()
    }
}
impl<'a, T> From<&'a PackValueOf<T>> for &'a PackValueDyn
where
    T: Sized + AttrKeyValue,
{
    #[inline]
    fn from(value: &'a PackValueOf<T>) -> Self {
        value.as_dyn()
    }
}
impl<T: ?Sized + AttrKeyValue> From<PackValueOf<T>> for PackValueCell {
    #[inline]
    fn from(v: PackValueOf<T>) -> Self {
        v.into_inner()
    }
}
impl From<Box<dyn AttrKeyValue>> for PackValueDyn {
    #[inline]
    fn from(v: Box<dyn AttrKeyValue>) -> Self {
        Self::from_box_dyn(v)
    }
}
impl From<Arc<dyn AttrKeyValue>> for PackValueDyn {
    #[inline]
    fn from(v: Arc<dyn AttrKeyValue>) -> Self {
        Self::from_arc_dyn(v)
    }
}
impl<T: AttrKeyValue + Sized + Clone> From<T> for PackValueDyn {
    #[inline]
    fn from(v: T) -> Self {
        Self::new_boxed_dyn(v)
    }
}
impl ops::Deref for PackValueDyn {
    type Target = dyn AttrKeyValue;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.get_dyn()
    }
}
impl ops::DerefMut for PackValueDyn {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_dyn_mut()
    }
}
impl<T: AttrKeyValue + Sized> ops::Deref for PackValueOf<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}
impl<T: AttrKeyValue + Sized> ops::DerefMut for PackValueOf<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}
impl<T: AttrKeyValue + Sized + Default> Default for PackValueOf<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
impl<T: AttrKeyValue + Sized + fmt::Debug> fmt::Debug for PackValueOf<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("PackValueCell")
            .field(&self.value.id)
            .field(self.get())
            .finish()
    }
}
impl<T: AttrKeyValue + Sized + fmt::Display> fmt::Display for PackValueOf<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.get(), f)
    }
}

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackValueRef<T: ?Sized + AttrKeyValue> {
    pub value: T,
}
impl<T: ?Sized + AttrKeyValue> PackValueRef<T> {
    #[inline(always)]
    pub const fn from_ref(value: &T) -> &Self {
        unsafe { mem::transmute(value) }
    }
    #[inline(always)]
    pub fn from_mut(value: &mut T) -> &mut Self {
        unsafe { mem::transmute(value) }
    }
}
impl<T: AttrKeyValue> PackValueRef<T> {
    #[inline(always)]
    pub const fn new(value: T) -> Self {
        Self { value }
    }
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.value
    }
}
impl<T: ?Sized + AttrKeyValue> ops::Deref for PackValueRef<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<T: ?Sized + AttrKeyValue> ops::DerefMut for PackValueRef<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
impl<'a, T: ?Sized + AttrKeyValue> From<&'a T> for &'a PackValueRef<T> {
    #[inline(always)]
    fn from(v: &'a T) -> Self {
        PackValueRef::from_ref(v)
    }
}
impl<'a, T: ?Sized + AttrKeyValue> From<&'a mut T> for &'a mut PackValueRef<T> {
    #[inline(always)]
    fn from(v: &'a mut T) -> Self {
        PackValueRef::from_mut(v)
    }
}
impl<T: ?Sized + AttrKeyValue> Borrow<T> for PackValueRef<T> {
    #[inline(always)]
    fn borrow(&self) -> &T {
        &self.value
    }
}
impl<T: ?Sized + AttrKeyValue> BorrowMut<T> for PackValueRef<T> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl<T: ?Sized + AttrKeyValue> AsRef<T> for PackValueRef<T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        &self.value
    }
}
impl<T: ?Sized + AttrKeyValue> AsMut<T> for PackValueRef<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl<T: AttrKeyValue> ToOwned for PackValueRef<T>
where
    //T: ToOwned, T::Owned: AttrKeyValue,
    T: Clone,
{
    type Owned = PackValueOf<T>;
    #[inline]
    fn to_owned(&self) -> Self::Owned {
        PackValueOf::new(self.value.clone())
    }
}
#[cfg(todo)]
impl<T: AttrKeyValue + Copy> Copy for PackValueRef<T> {}
impl<T: AttrKeyValue> Borrow<PackValueRef<T>> for PackValueOf<T> {
    #[inline(always)]
    fn borrow(&self) -> &PackValueRef<T> {
        PackValueRef::from_ref(self.get())
    }
}
#[cfg(todo)]
impl ToOwned for dyn AttrKeyValue {
    type Owned = PackValueCell;
    #[inline]
    fn to_owned(&self) -> Self::Owned {
        self.clone_dyn()
            .map(PackValueCell::from_box)
            .unwrap_or_else(|| PackValueCell::new_empty(self.pack_key_id()))
    }
}
/// panic if not clone :<
impl ToOwned for dyn AttrKeyValue {
    type Owned = PackValueDyn;
    #[inline]
    fn to_owned(&self) -> Self::Owned {
        self.clone_dyn()
            .and_then(|v| PackValueDyn::from_cell_dyn(PackValueCell::from_box(v)))
            .expect("unique cell cloned")
    }
}
/// bleh, use [PackValueDyn] instead where possible
impl Borrow<dyn AttrKeyValue> for PackValueCell {
    #[inline(always)]
    fn borrow(&self) -> &dyn AttrKeyValue {
        self.get().expect("empty cell")
    }
}
impl Borrow<Option<PackValueDyn>> for PackValueCell {
    #[inline(always)]
    fn borrow(&self) -> &Option<PackValueDyn> {
        match PackValueDyn::from_cell_dyn_ref(self) {
            Some(v) => unsafe { mem::transmute::<&PackValueDyn, &Option<PackValueDyn>>(v) },
            None => PackValueDyn::none_ref(),
        }
    }
}
impl Borrow<dyn AttrKeyValue> for PackValueDyn {
    #[inline(always)]
    fn borrow(&self) -> &dyn AttrKeyValue {
        self.get_dyn()
    }
}
impl From<Cow<'_, dyn AttrKeyValue>> for PackValueCell {
    #[inline]
    fn from(value: Cow<'_, dyn AttrKeyValue>) -> Self {
        value.into_owned().into_inner()
    }
}
impl From<Cow<'_, dyn AttrKeyValue>> for PackValueDyn {
    #[inline]
    fn from(value: Cow<'_, dyn AttrKeyValue>) -> Self {
        value.into_owned()
    }
}
impl<T> From<PackValueOf<T>> for Cow<'_, dyn AttrKeyValue>
where
    T: ?Sized + AttrKeyValue,
{
    #[inline]
    fn from(value: PackValueOf<T>) -> Self {
        Cow::Owned(value.into_dyn())
    }
}
impl<'a, T> From<&'a PackValueOf<T>> for Cow<'a, dyn AttrKeyValue>
where
    T: ?Sized + AttrKeyValue,
{
    #[inline]
    fn from(value: &'a PackValueOf<T>) -> Self {
        Cow::Borrowed(value.get_dyn())
    }
}

/// TODO: move to newtype wrapper for use in set collections
/// (plus maybe a flag for exceptions that can be set multiple times instead of `Vec<T>` attr containers?)
pub type PackValueSet = std::collections::BTreeSet<PackValueCell>;
impl GetAttrDyn for PackValueSet {
    #[inline]
    fn holds_attr_dyn(_key: PackKeyId) -> bool {
        true
    }
    #[inline]
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        self.contains(&key)
    }
    #[inline]
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        self.get(&key).and_then(|v| v.get())
    }
    #[inline]
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        self.get(&key)
            .cloned()
            .and_then(|v| PackValueDyn::from_cell_dyn(v))
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {
        self.iter().filter_map(|v| v.get().map(Cow::Borrowed))
    }
}
impl SetAttrDyn for PackValueSet {
    #[inline]
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        self.replace(value);
        true
    }
}
impl<A: AttrKey + AttrKeyValue> GetAttr<A> for PackValueSet {
    fn has_attr(&self) -> bool {
        self.contains(&A::pack_key_of())
    }
    fn get_attr_ref(&self) -> Option<&A> {
        self.get(&A::pack_key_of())
            .and_then(PackValueOf::<A>::from_cell_ref)
            .map(PackValueOf::get)
    }
}
impl<A: AttrKey + AttrKeyValue> SetAttr<A> for PackValueSet {
    fn set_attr(&mut self, v: A) {
        self.replace(PackValueOf::new_boxed(v).into_inner());
    }
    fn unset_attr(&mut self) {
        self.remove(&A::pack_key_of());
    }
}
impl PartialEq for PackValueCell {
    #[inline]
    fn eq(&self, rhs: &Self) -> bool {
        self.id == rhs.id
    }
}
impl PartialOrd for PackValueCell {
    #[inline]
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        self.id.partial_cmp(&rhs.id)
    }
}
impl Eq for PackValueCell {}
impl Ord for PackValueCell {
    #[inline]
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        self.id.cmp(&rhs.id)
    }
}

#[allow(unused_variables)]
pub trait GetAttrDyn {
    fn holds_attr_dyn(key: PackKeyId) -> bool
    where
        Self: Sized;
    #[inline]
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        self.get_attr_dyn_ref(key).is_some()
    }
    #[inline]
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        None
    }
    #[inline]
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        self.get_attr_dyn_ref(key).map(Cow::Borrowed)
    }
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        self.get_attr_dyn(key).and_then(|v| match v {
            Cow::Owned(v) => Some(v),
            Cow::Borrowed(v) => v.clone_dyn().map(Into::into),
        })
    }

    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_
    where
        Self: Sized,
    {
        PackKeyId::all_keys().filter_map(|key| self.get_attr_dyn(key))
    }
}
impl dyn GetAttrDyn {
    pub const EMPTY: &'static Self = &() as &dyn GetAttrDyn;

    pub fn imp_get_attr_dyn<A, T>(container: &T) -> Option<Cow<'_, dyn AttrKeyValue>>
    where
        A: AttrKey + AttrKeyValue,
        T: GetAttr<A>,
    {
        container.get_attr().map(|v| match v {
            Cow::Owned(v) => Cow::Owned(PackValueDyn::new_boxed_dyn(v)),
            Cow::Borrowed(v) => Cow::Borrowed(v as &dyn AttrKeyValue),
        })
    }
}
pub trait GetAttrDynExt {
    fn get_attr_dyn_ref_of<A: AttrKeyValue>(&self) -> Option<&A>;
    fn clone_attr_dyn_of<A: AttrKeyValue>(&self) -> Option<PackValueOf<A>>;
    fn has_attr_dyn_of<A: AttrKeyValue>(&self) -> bool;
    #[inline]
    fn attr_dyn_or_default<A: AttrKeyValue>(&self) -> A where
        A: Default,
    {
        self.clone_attr_dyn_of::<A>().and_then(|v| v.into_value())
            .unwrap_or_default()
    }
    #[inline]
    fn attr_dyn_or_default_into<A: AttrKeyValue, T>(&self) -> T where
        A: Default + Into<T>,
    {
        self.attr_dyn_or_default::<A>().into()
    }

    // these could be GetAttrExt but idk if there's a good marker trait for that...
    #[inline]
    fn has_attr_of<A>(&self) -> bool where
        Self: GetAttr<A>,
    {
        GetAttr::<A>::has_attr(self)
    }
    #[inline]
    fn get_attr_ref_of<A>(&self) -> Option<&A> where
        Self: GetAttr<A>,
    {
        GetAttr::<A>::get_attr_ref(self)
    }
    #[inline]
    fn get_attr_of<A>(&self) -> Option<Cow<'_, A>> where
        Self: GetAttr<A>,
        A: ToOwned,
    {
        GetAttr::<A>::get_attr(self)
    }
    #[inline]
    fn clone_attr_of<A>(&self) -> Option<A::Owned> where
        Self: GetAttr<A>,
        A: ToOwned,
    {
        GetAttr::<A>::get_attr(self).map(Cow::into_owned)
    }
    #[inline]
    fn attr_or_default<A>(&self) -> A::Owned where
        Self: GetAttr<A>,
        A: ToOwned,
        A::Owned: Default,
    {
        GetAttr::<A>::get_attr_or_default(self).into_owned()
    }
    #[inline]
    fn attr_or_default_into<A, T>(&self) -> T where
        Self: GetAttr<A>,
        A: ToOwned,
        A::Owned: Default + Into<T>,
    {
        self.attr_or_default::<A>().into()
    }
}
impl<T> GetAttrDynExt for T
where
    T: ?Sized + GetAttrDyn,
{
    #[inline]
    fn get_attr_dyn_ref_of<A: AttrKeyValue>(&self) -> Option<&A> {
        self.get_attr_dyn_ref(A::pack_key_of())
            .map(|v| unsafe { <dyn AttrKeyValue>::downcast_ref_unchecked(v) })
    }
    #[inline]
    fn clone_attr_dyn_of<A: AttrKeyValue>(&self) -> Option<PackValueOf<A>> {
        self.clone_attr_dyn(A::pack_key_of())
            .map(|v| unsafe { PackValueOf::new_unchecked(v.into()) })
    }
    #[inline]
    fn has_attr_dyn_of<A: AttrKeyValue>(&self) -> bool {
        self.has_attr_dyn(A::pack_key_of())
    }
}
impl GetAttrDyn for () {
    #[inline]
    fn holds_attr_dyn(_: PackKeyId) -> bool {
        false
    }
    #[inline]
    fn has_attr_dyn(&self, _: PackKeyId) -> bool {
        false
    }
    #[inline]
    fn get_attr_dyn_ref(&self, _: PackKeyId) -> Option<&dyn AttrKeyValue> {
        None
    }
    #[inline]
    fn get_attr_dyn(&self, _: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        None
    }
    #[inline]
    fn clone_attr_dyn(&self, _: PackKeyId) -> Option<PackValueDyn> {
        None
    }
    #[inline]
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {
        core::iter::empty()
    }
}

#[allow(unused_variables)]
pub trait SetAttrDyn {
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        false
    }
}

/// TODO: bitvec?
pub type PackKeySet = ::std::collections::BTreeSet<PackKeyId>;

pub trait MaybeDefault {
    fn get_default_fn() -> Option<DefaultFn<Self>>
    where
        Self: Sized;
    fn get_default_fn_ptr() -> Option<NonZero<usize>>;
}
impl<T> MaybeDefault for T
where
    T: Default,
{
    #[inline]
    fn get_default_fn() -> Option<DefaultFn<Self>>
    where
        Self: Sized,
    {
        Some(T::default)
    }
    #[inline]
    fn get_default_fn_ptr() -> Option<NonZero<usize>> {
        Self::get_default_fn().and_then(|default| NonZero::new(default as usize))
    }
}
pub type DefaultFn<T> = fn() -> T;

#[macro_export]
macro_rules! pack_attr {
    (impl$({$($arg:tt)*})?
        !Default
        for $ty:ty
        $(where{$($where_:tt)*})?
        {}
        $($($rest:tt)+)?
    ) => {
        impl$(<$($arg)*>)? $crate::attributes::cell::MaybeDefault for $ty where
            $($($where_)*)?
        {
            #[inline]
            fn get_default_fn() -> Option<$crate::attributes::cell::DefaultFn<Self>> where Self: Sized {
                None
            }
            #[inline]
            fn get_default_fn_ptr() -> Option<::std::num::NonZero<usize>> { None }
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl$({$($arg:tt)*})?
        Attr{$attr:ty}
        for &struct{$ty:ty}.$field:ident ?
        $(where{$($where_:tt)*})?
        {}
        $($($rest:tt)+)?
    ) => {
        impl$(<$($arg)*>)? $crate::attributes::keys::GetAttr<$attr> for $ty where
            $($($where_)*)?
        {
            #[inline]
            fn has_attr(&self) -> bool {
                self.$field.is_some()
            }
            #[inline]
            fn get_attr_ref(&self) -> Option<&$attr> {
                self.$field.as_ref().map(|v| ::core::borrow::Borrow::borrow(v))
            }
        }
        impl$(<$($arg)*>)? $crate::attributes::keys::SetAttr<$attr> for $ty where
            $($($where_)*)?
        {
            #[inline]
            fn set_attr(&mut self, value: $attr) {
                self.$field = Some(value.into());
            }
            #[inline]
            fn unset_attr(&mut self) {
                self.$field = None;
            }
            /*#[inline] fn get_attr_mut(&mut self) -> Option<&mut $attr> {
                self.$field.as_mut().map(|v| ::core::borrow::BorrowMut::borrow_mut(v))
            }*/
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl$({$($arg:tt)*})?
        Attr{$attr:ty}
        for &struct{$ty:ty}.$field:ident
        $(where{$($where_:tt)*})?
        {}
        $($($rest:tt)+)?
    ) => {
        impl$(<$($arg)*>)? $crate::attributes::keys::GetAttr<$attr> for $ty where
            $($($where_)*)?
        {
            #[inline(always)]
            fn has_attr(&self) -> bool {
                true
            }
            #[inline(always)]
            fn get_attr_ref(&self) -> Option<&$attr> {
                Some(::core::borrow::Borrow::borrow(&self.$field))
            }
            #[inline(always)]
            fn get_attr(&self) -> Option<::std::borrow::Cow<'_, $attr>> {
                Some(::std::borrow::Cow::Borrowed(::core::borrow::Borrow::borrow(&self.$field)))
            }
        }
        impl$(<$($arg)*>)? $crate::attributes::keys::SetAttr<$attr> for $ty where
            $($($where_)*)?
        {
            #[inline]
            fn set_attr(&mut self, value: $attr) {
                self.$field = value.into()
            }
            #[inline]
            fn unset_attr(&mut self) {
                if let Some(d) = <$attr as $crate::attributes::cell::MaybeDefault>::get_default_fn() {
                    self.$field = d().into();
                }
            }
            /*#[inline] fn get_attr_mut(&mut self) -> Option<&mut $attr> {
                Some(::core::borrow::BorrowMut::borrow_mut(&mut self.$field))
            }*/
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl$({$($arg:tt)*})?
        Attr{$attr:ty}
        for as &struct{$ty:ty}.$field:ident
        $(where{$($where_:tt)*})?
        {}
        $($($rest:tt)+)?
    ) => {
        impl$(<$($arg)*>)? $crate::attributes::keys::GetAttr<$attr> for $ty where
            $($($where_)*)?
        {
            #[inline]
            fn has_attr(&self) -> bool {
                $crate::attributes::keys::GetAttr::<$attr>::has_attr(&self.$field)
            }
            #[inline]
            fn get_attr_ref(&self) -> Option<&$attr> {
                $crate::attributes::keys::GetAttr::<$attr>::get_attr_ref(&self.$field)
            }
            #[inline]
            fn get_attr(&self) -> Option<::std::borrow::Cow<'_, $attr>> {
                $crate::attributes::keys::GetAttr::<$attr>::get_attr(&self.$field)
            }
        }
        impl$(<$($arg)*>)? $crate::attributes::keys::SetAttr<$attr> for $ty where
            $($($where_)*)?
        {
            #[inline]
            fn set_attr(&mut self, value: $attr) {
                $crate::attributes::keys::SetAttr::<$attr>::set_attr(&mut self.$field, value)
            }
            //#[inline] fn get_attr_mut(&mut self) -> Option<&mut $attr> {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl$({$($arg:tt)*})?
        Attr{$attr:ty}
        for as &struct{$ty:ty}.$field:ident?: $inner_ty:ty
        $(where{$($where_:tt)*})?
        {}
        $($($rest:tt)+)?
    ) => {
        impl$(<$($arg)*>)? $crate::attributes::keys::GetAttr<$attr> for $ty where
            $($($where_)*)?
        {
            #[inline]
            fn has_attr(&self) -> bool {
                self.$field.as_ref().map(|f|
                    $crate::attributes::keys::GetAttr::<$attr>::has_attr(
                        core::borrow::Borrow::<$inner_ty>::borrow(f)
                    )
                ).unwrap_or(false)
            }
            #[inline]
            fn get_attr_ref(&self) -> Option<&$attr> {
                self.$field.as_ref().and_then(|f|
                    $crate::attributes::keys::GetAttr::<$attr>::get_attr_ref(
                        core::borrow::Borrow::<$inner_ty>::borrow(f)
                    )
                )
            }
            #[inline]
            fn get_attr(&self) -> Option<::std::borrow::Cow<'_, $attr>> {
                self.$field.as_ref().and_then(|f|
                    $crate::attributes::keys::GetAttr::<$attr>::get_attr(
                        core::borrow::Borrow::<$inner_ty>::borrow(f)
                    )
                )
            }
        }
        impl$(<$($arg)*>)? $crate::attributes::keys::SetAttr<$attr> for $ty where
            $($($where_)*)?
        {
            #[inline]
            fn set_attr(&mut self, value: $attr) {
                let f = core::convert::AsMut::<$inner_ty>::as_mut(self);
                $crate::attributes::keys::SetAttr::<$attr>::set_attr(&mut *f, value)
            }
            //#[inline] fn get_attr_mut(&mut self) -> Option<&mut $attr> {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl
        Attr{$attr:ty}
        in Internal{MarkerAttributes}
        {}
        $($($rest:tt)+)?
    ) => {
        $crate::attributes::cell::pack_attr! {
            impl Attr{$attr} for as &struct{$crate::poi::Poi}.attributes {}
            impl Attr{$attr} for as &struct{$crate::trail::Trail}.attributes {}
            impl Attr{$attr} for as &struct{$crate::category::Category}.marker_attributes {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl
        Attr{$attr:ty}
        in Internal{ScriptAttributes}
        {}
        $($($rest:tt)+)?
    ) => {
        $crate::attributes::cell::pack_attr! {
            impl Attr{$attr} for as &struct{$crate::attributes::MarkerAttributes}.script?: $crate::attributes::ScriptAttributes {}
            impl Attr{$attr} for as &struct{$crate::poi::Poi}.attributes {}
            impl Attr{$attr} for as &struct{$crate::trail::Trail}.attributes {}
            impl Attr{$attr} for as &struct{$crate::category::Category}.marker_attributes {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl
        Attr{$attr:ty}
        in Internal{InteractionAttributes}
        {}
        $($($rest:tt)+)?
    ) => {
        $crate::attributes::cell::pack_attr! {
            impl Attr{$attr} for as &struct{$crate::attributes::MarkerAttributes}.interaction?: $crate::attributes::InteractionAttributes {}
            impl Attr{$attr} for as &struct{$crate::poi::Poi}.attributes {}
            impl Attr{$attr} for as &struct{$crate::trail::Trail}.attributes {}
            impl Attr{$attr} for as &struct{$crate::category::Category}.marker_attributes {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl
        Attr{$attr:ty}
        in Internal{FilterAttributes}
        {}
        $($($rest:tt)+)?
    ) => {
        $crate::attributes::cell::pack_attr! {
            impl Attr{$attr} for as &struct{$crate::attributes::MarkerAttributes}.filters?: $crate::attributes::FilterAttributes {}
            impl Attr{$attr} for as &struct{$crate::poi::Poi}.attributes {}
            impl Attr{$attr} for as &struct{$crate::trail::Trail}.attributes {}
            impl Attr{$attr} for as &struct{$crate::category::Category}.marker_attributes {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl
        Attr{$attr:ty}
        in Internal{RenderAttributes}
        {}
        $($($rest:tt)+)?
    ) => {
        $crate::attributes::cell::pack_attr! {
            impl Attr{$attr} for as &struct{$crate::attributes::MarkerAttributes}.render?: $crate::attributes::RenderAttributes {}
            impl Attr{$attr} for as &struct{$crate::poi::Poi}.attributes {}
            impl Attr{$attr} for as &struct{$crate::trail::Trail}.attributes {}
            impl Attr{$attr} for as &struct{$crate::category::Category}.marker_attributes {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl
        Attr{$attr:ty}
        in Internal{TrailAttributes}
        {}
        $($($rest:tt)+)?
    ) => {
        $crate::attributes::cell::pack_attr! {
            impl Attr{$attr} for as &struct{$crate::attributes::MarkerAttributes}.render?: $crate::attributes::RenderAttributes {}
            impl Attr{$attr} for as &struct{$crate::attributes::RenderAttributes}.trail?: $crate::attributes::TrailAttributes {}
            impl Attr{$attr} for as &struct{$crate::trail::Trail}.attributes {}
            impl Attr{$attr} for as &struct{$crate::category::Category}.marker_attributes {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (impl
        Attr{$attr:ty}
        in Internal{PoiAttributes}
        {}
        $($($rest:tt)+)?
    ) => {
        $crate::attributes::cell::pack_attr! {
            impl Attr{$attr} for as &struct{$crate::attributes::MarkerAttributes}.render?: $crate::attributes::RenderAttributes {}
            impl Attr{$attr} for as &struct{$crate::attributes::RenderAttributes}.poi?: $crate::attributes::PoiAttributes {}
            impl Attr{$attr} for as &struct{$crate::poi::Poi}.attributes {}
            impl Attr{$attr} for as &struct{$crate::category::Category}.marker_attributes {}
        }
        $($crate::attributes::cell::pack_attr! { $($rest)* })?
    };
    (=id_is_in($id:expr, [$($attr:ty),*$(,)?])) => {
        {
            /*static PACK_IDS: ::std::sync::LazyLock<Box<[$crate::attributes::keys::PackKeyId]>> = ::std::sync::LazyLock::new(|| {
                vec![
                    $(<$attr as $crate::attributes::keys::AttrKeyValue>::pack_key_of()),*
                ].into_boxed_slice()
            });*/
            static PACK_IDS: ::std::sync::LazyLock<$crate::attributes::cell::PackKeySet> = ::std::sync::LazyLock::new(|| {
                ::core::iter::Iterator::collect(::core::iter::IntoIterator::into_iter([
                    $(<$attr as $crate::attributes::cell::AttrKeyValue>::pack_key_of()),*
                ]))
            });
            (&*PACK_IDS).contains(&$id)
        }
    };
    (match =id_is($id:expr) {
        $(
            = $attr:ty => $v:expr,
        )*
        $(_ => $fallback:expr,)?
    }) => {
        'packkeymatch: loop {
            let pack_id = $id;
            $(
                // TODO: switch to single array here and index into it?
                // if captures aren't needed could use dyn dispatch for the branches too...
                let exp_id = <$attr as $crate::attributes::cell::AttrKeyValue>::pack_key_of();
                if pack_id == exp_id {
                    break 'packkeymatch ($v)
                }
            )*
            break 'packkeymatch $(($fallback))?;
        }
    };
    (match $bind:ident @ id_of_cell($cell:expr) {
        $(
            @ $attr:ty => $v:expr,
        )*
        $(_ => $fallback:expr,)?
    }) => {
        {
            let $bind = $cell;
            let pack_id = $bind.id();
            if false { unsafe { ::core::hint::unreachable_unchecked() } }
            $(
                else if ({
                    let exp_id = <$attr as $crate::attributes::cell::AttrKeyValue>::pack_key_of();
                    pack_id == exp_id
                }) {
                    let $bind = unsafe { $crate::attributes::cell::PackValueOf::<$attr>::new_unchecked($bind) };
                    $v
                }
            )*
            else { $($fallback)? }
        }
    };
    (imp GetAttrDyn::clone_attr_dyn($this:ident, $id:ident) in [
     $($attr:ty),*$(,)?
    ]) => {
        $crate::attributes::cell::pack_attr! {
            match =id_is($id) {
                $(
                    = $attr => Some($crate::attributes::keys::GetAttr::<$attr>::get_attr($this).map(|v|
                        $crate::attributes::cell::PackValueDyn::new_boxed_dyn(v.into_owned())
                    )),
                )*
                _ => None::<Option<$crate::attributes::cell::PackValueDyn>>,
            }
        }
    };
    (imp GetAttrDyn::get_attr_dyn($this:ident, $id:ident) in [
     $($attr:ty),*$(,)?
    ]) => {
        $crate::attributes::cell::pack_attr! {
            match =id_is($id) {
                $(
                    = $attr => Some(<dyn $crate::attributes::cell::GetAttrDyn>::imp_get_attr_dyn::<$attr, _>($this)),
                )*
                _ => None::<Option<::std::borrow::Cow<dyn $crate::attributes::cell::AttrKeyValue>>>,
            }
        }
    };
    (imp GetAttrDyn::has_attr_dyn($this:ident, $id:ident) in [
     $($attr:ty),*$(,)?
    ]) => {
        $crate::attributes::cell::pack_attr! {
            match =id_is($id) {
                $(
                    = $attr => Some($crate::attributes::keys::GetAttr::<$attr>::has_attr($this)),
                )*
                _ => None::<bool>,
            }
        }
    };
    (imp GetAttrDyn::get_attr_dyn_ref($this:ident, $id:ident) in [
     $($attr:ty),*$(,)?
    ]) => {
        $crate::attributes::cell::pack_attr! {
            match =id_is($id) {
                $(
                    = $attr => Some($crate::attributes::keys::GetAttr::<$attr>::get_attr_ref($this).map(|v|
                        v as &dyn $crate::attributes::cell::AttrKeyValue
                    )),
                )*
                _ => None::<Option<&dyn $crate::attributes::cell::AttrKeyValue>>,
            }
        }
    };
    (imp GetAttrDyn::iter_attrs_dyn($this:ident) in [
     $($attr:ty),*$(,)?
    ]) => {
        ::core::iter::IntoIterator::into_iter([
            $(
                <dyn $crate::attributes::cell::GetAttrDyn>::imp_get_attr_dyn::<$attr, _> as fn(_) -> _,
            )*
        ]).filter_map(move |f| f($this))
    };
    (imp SetAttrDyn::set_attr_dyn($this:ident, $cell:ident) in
        [ $($attr:ty),*$(,)? ]
        $(, _ => $fallback:expr,)?
    ) => {
        $crate::attributes::cell::pack_attr! {
            match $cell @ id_of_cell($cell) {
                $(
                    @ $attr => if let Some(value) = $cell.to_value() {
                        $crate::attributes::keys::SetAttr::<$attr>::set_attr($this, value);
                        true
                    } else {
                        $crate::attributes::keys::SetAttr::<$attr>::unset_attr($this);
                        true
                    },
                )*
                _ => match () {
                    $(() => $fallback,)?
                    #[allow(unreachable_patterns)]
                    () => false
                },
            }
        }
    };
}
pub use pack_attr;
