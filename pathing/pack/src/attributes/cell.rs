use {
    crate::attributes::keys::AttrKey,
    std::{any::{Any, TypeId}, collections::BTreeMap, fmt, marker::PhantomData, mem, ops, ptr, str::FromStr, sync::{Arc, RwLock}},
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

    #[inline(never)]
    pub fn for_type<T: AttrKeyValue>() -> Option<Self> {
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
    pub fn try_from_str(self, s: &str) -> anyhow::Result<PackValueCell> {
        let try_from_str = PackKeyRegistry::registry_with(self, |reg| reg.try_from_str);
        try_from_str(s)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PackKeyRegistration {
    pub value_type: TypeId,
    pub value_size: usize,
    pub vtable: usize,
    pub try_from_str: fn(&str) -> anyhow::Result<PackValueCell>,
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
    pub fn value_type_unknown() -> TypeId { TypeId::of::<[()]>() }

    pub fn for_type<T: AttrKeyValue>() -> Self {
        debug_assert!(mem::align_of::<T>() <= mem::align_of::<usize>());
        Self {
            value_type: TypeId::of::<T>(),
            value_size: mem::size_of::<T>(),
            vtable: T::vtable_ptr() as usize,
            try_from_str: T::try_from_str,
            attr_names: T::attr_names(),
        }
    }
}

#[derive(Debug)]
pub struct PackKeyRegistry {
    registrations: Vec<PackKeyRegistration>,
    id_lookup: BTreeMap<TypeId, PackKeyId>
}
impl PackKeyRegistry {
    const EMPTY: Self = Self {
        registrations: Vec::new(),
        id_lookup: BTreeMap::new(),
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
            i @ 0..=0xff => unsafe {
                PackKeyId::new_unchecked(i as u8)
            },
            // out of space :<
            _ => return None,
        };
        let value_type = reg.value_type;
        self.registrations.push(reg);
        self.id_lookup.insert(value_type, index);
        Some(index)
    }
    pub fn get(&self, id: PackKeyId) -> &PackKeyRegistration {
        unsafe {
            self.registrations.get_unchecked(id.index() as usize)
        }
    }
    pub fn lookup(&self, value_type: &TypeId) -> Option<PackKeyId> {
        self.id_lookup.get(&value_type).cloned()
    }
}

pub unsafe trait AttrKeyValue: Any {
    fn pack_key_id(&self) -> PackKeyId;
    fn clone_dyn(&self) -> Option<Box<dyn AttrKeyValue>>;
    fn pack_key_of() -> PackKeyId where Self: Sized;
    // TODO: these should just be moved over to a StaticVtable struct's fields
    fn attr_names() -> &'static [&'static str] where Self: Sized;
    fn vtable_ptr() -> *const () where Self: Sized;
    fn try_from_str(s: &str) -> anyhow::Result<PackValueCell> where Self: Sized;
}
unsafe impl<A: AttrKey + Any> AttrKeyValue for A where
    A: AttrKey + Any + Clone + FromStr,
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
    fn try_from_str(s: &str) -> anyhow::Result<PackValueCell> where Self: Sized {
        Self::from_str(s)
            .map(PackValueCell::new)
            .map_err(Into::into)
    }

    fn pack_key_id(&self) -> PackKeyId {
        Self::pack_key_of()
    }
    fn pack_key_of() -> PackKeyId where Self: Sized {
        PackKeyId::for_type::<Self>().expect("pack key registry oom")
    }
    fn clone_dyn(&self) -> Option<Box<dyn AttrKeyValue>> {
        Some(Box::new(self.clone()) as Box<_>)
    }
}
/// all ptr metadata fns are unstable, so we have fun here :3
fn vtable_ptr_of(p: *const dyn AttrKeyValue) -> *const () {
    let [_, vtbl] = unsafe {
        mem::transmute::<*const dyn AttrKeyValue, [*const (); 2]>(p)
    };
    vtbl
}
unsafe fn to_vtable_ptr(p: usize, vtbl: usize) -> *mut dyn AttrKeyValue {
    unsafe {
        mem::transmute::<[usize; 2], *mut dyn AttrKeyValue>([p, vtbl])
    }
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
    pub const FLAG_INVALID: u8 = 0;
    pub const FLAG_EMPTY: u8 = 1;
    pub const FLAG_INLINE_COPY: u8 = 2;
    pub const FLAG_BOX: u8 = 3;
    pub const FLAG_ARC: u8 = 4;

    pub fn new<A: AttrKeyValue>(value: A) -> Self {
        Self::from_arc(Arc::new(value) as Arc<dyn AttrKeyValue>)
    }
    pub fn copy<A: AttrKeyValue + Copy>(value: A) -> Self {
        let id = value.pack_key_id();
        let ptr = &value as *const dyn AttrKeyValue as *const ();
        unsafe {
            Self::from_copy_unchecked(ptr::NonNull::new_unchecked(ptr as *mut _), id)
        }
    }

    pub fn from_box(inner: Box<dyn AttrKeyValue>) -> Self {
        let id = inner.pack_key_id();
        let ptr = Box::into_raw(inner);
        unsafe {
            Self::from_box_unchecked(ptr, id)
        }
    }

    pub fn from_arc(inner: Arc<dyn AttrKeyValue>) -> Self {
        let id = inner.pack_key_id();
        let ptr = Arc::into_raw(inner);
        unsafe {
            Self::from_arc_unchecked(ptr, id)
        }
    }

    pub unsafe fn from_arc_unchecked(ptr: *const dyn AttrKeyValue, id: PackKeyId) -> Self {
        debug_assert_eq!(id.vtable_ptr(), vtable_ptr_of(ptr));
        unsafe {
            Self::from_inner_unchecked(ptr as *const () as usize, id, Self::FLAG_ARC)
        }
    }
    pub unsafe fn from_box_unchecked(ptr: *mut dyn AttrKeyValue, id: PackKeyId) -> Self {
        debug_assert_eq!(id.vtable_ptr(), vtable_ptr_of(ptr));
        unsafe {
            Self::from_inner_unchecked(ptr as *mut () as usize, id, Self::FLAG_BOX)
        }
    }
    /// TODO: this should just be a trait method or something...
    pub unsafe fn from_copy_unchecked(value: ptr::NonNull<()>, id: PackKeyId) -> Self {
        let size = id.value_size();
        debug_assert!(size <= size_of::<usize>());
        let mut out = [0u8; size_of::<usize>()];
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

    pub fn ptr(&self) -> *mut dyn AttrKeyValue {
        let (p, v) = match self.flag {
            Self::FLAG_EMPTY | Self::FLAG_ARC | Self::FLAG_BOX =>
                (self.value, self.id.vtable_ptr() as usize),
            Self::FLAG_INLINE_COPY => {
                // I'll make this valid someday probably..?
                (&raw const self.value as usize, self.id.vtable_ptr() as usize)
            },
            _ => (0, 0),
        };
        unsafe {
            to_vtable_ptr(p, v)
        }
    }
    /// ptr coming from a mutable borrow silliness...
    pub fn ptr_mut(&mut self) -> *mut dyn AttrKeyValue {
        let (p, v) = match self.flag {
            Self::FLAG_EMPTY | Self::FLAG_ARC | Self::FLAG_BOX =>
                (self.value, self.id.vtable_ptr() as usize),
            Self::FLAG_INLINE_COPY => {
                // I'll make this valid someday probably..?
                (&raw mut self.value as usize, self.id.vtable_ptr() as usize)
            },
            _ => (0, 0),
        };
        unsafe {
            to_vtable_ptr(p, v)
        }
    }
    pub fn get(&self) -> &dyn AttrKeyValue {
        unsafe {
            &*(self.ptr() as *const dyn AttrKeyValue)
        }
    }
    pub fn get_mut(&mut self) -> &mut dyn AttrKeyValue {
        unsafe {
            &mut *(self.ptr_mut() as *mut dyn AttrKeyValue)
        }
    }
    pub fn into_arc(self) -> Option<Arc<dyn AttrKeyValue>> {
        if self.flag != Self::FLAG_ARC {
            return None
        }
        let this = mem::ManuallyDrop::new(self);
        let ptr = this.ptr();
        Some(unsafe {
            Arc::from_raw(ptr)
        })
    }
    pub fn into_box(self) -> Option<Box<dyn AttrKeyValue>> {
        if self.flag != Self::FLAG_BOX {
            return None
        }
        let this = mem::ManuallyDrop::new(self);
        let ptr = this.ptr();
        Some(unsafe {
            Box::from_raw(ptr)
        })
    }
    /// beware of leaking allocations...
    pub fn into_inner(self) -> (usize, u8) {
        let this = mem::ManuallyDrop::new(self);
        (this.value, this.flag)
    }

    pub fn emptied(&self) -> Self {
        unsafe {
            Self::from_inner_unchecked(0, self.id, Self::FLAG_EMPTY)
        }
    }

    pub fn try_get_ptr<A: AttrKeyValue + Sized>(&self) -> Option<ptr::NonNull<A>> {
        #[cfg(todo = "unnecessary")]
        if self.flag == Self::FLAG_INVALID { return None }
        let id = A::pack_key_of();
        if id != self.id {
            return None
        }

        ptr::NonNull::new(
            self.ptr() as *const dyn AttrKeyValue as *const A as *mut A
        )
    }
    pub fn try_get_ptr_mut<A: AttrKeyValue + Sized>(&mut self) -> Option<ptr::NonNull<A>> {
        #[cfg(todo = "unnecessary")]
        if self.flag == Self::FLAG_INVALID { return None }
        let id = A::pack_key_of();
        if id != self.id {
            return None
        }

        ptr::NonNull::new(
            self.ptr_mut() as *mut dyn AttrKeyValue as *mut A
        )
    }
    pub fn try_get<A: AttrKeyValue + Sized>(&self) -> Option<&A> {
        self.try_get_ptr().map(|p| unsafe {
            &*p.as_ptr()
        })
    }
    pub fn try_get_mut<A: AttrKeyValue + Sized>(&mut self) -> Option<&mut A> {
        self.try_get_ptr_mut().map(|p| unsafe {
            &mut *p.as_ptr()
        })
    }
    pub fn try_copy<A: AttrKeyValue + Copy>(&self) -> Option<A> {
        self.try_get_ptr().map(|p| unsafe {
            ptr::read(p.as_ptr())
        })
    }
    pub fn set<A: AttrKeyValue>(&mut self, value: A) -> Option<A> {
        self.try_get_ptr_mut().map(|p| unsafe {
            ptr::replace(p.as_ptr(), value)
        })
    }
}
impl Clone for PackValueCell {
    fn clone(&self) -> Self {
        match self.flag {
            Self::FLAG_ARC => unsafe {
                let ptr = self.ptr();
                Arc::increment_strong_count(ptr);
                Self::from_arc_unchecked(ptr, self.id)
            },
            Self::FLAG_BOX => unsafe {
                let value = match self.get().clone_dyn() {
                    Some(v) => v,
                    None => return self.emptied(),
                };
                let ptr = Box::into_raw(value);
                Self::from_box_unchecked(ptr, self.id)
            },
            Self::FLAG_INLINE_COPY => unsafe {
                Self::from_inner_unchecked(self.value, self.id, self.flag)
            },
            _ =>
                self.emptied(),
        }
    }
}
impl Drop for PackValueCell {
    fn drop(&mut self) {
        unsafe {
            Arc::decrement_strong_count(self.ptr())
        }
    }
}
impl fmt::Debug for PackValueCell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_tuple("PackValueCell");
        if self.flag == Self::FLAG_INVALID {
            return f.finish()
        }
        f.field(&self.id);
        match self.flag {
            _ => {
                f.field(&(self.value as *const ()));
            },
        }
        f.finish()
    }
}
// TODO: require fmt::Display on attrs soon
#[repr(transparent)]
#[derive(Clone)]
pub struct PackValueOf<T: AttrKeyValue + Sized> {
    value: PackValueCell,
    _inner: PhantomData<T>,
}
impl<T: AttrKeyValue + Sized> PackValueOf<T> {
    pub fn new(value: T) -> Self {
        unsafe {
            Self::new_unchecked(PackValueCell::new(value))
        }
    }
    pub unsafe fn new_unchecked(value: PackValueCell) -> Self {
        Self {
            value,
            _inner: PhantomData,
        }
    }
    pub fn into_inner(self) -> PackValueCell {
        self.value
    }
    pub fn get(&self) -> &T {
        unsafe {
            &*(self.value.ptr() as *const dyn AttrKeyValue as *const T)
        }
    }
    pub fn get_mut(&mut self) -> &mut T {
        unsafe {
            &mut *(self.value.ptr_mut() as *mut dyn AttrKeyValue as *mut T)
        }
    }
    pub fn value(&self) -> T where
        T: Clone,
    {
        self.get().clone()
    }
}
impl<T: AttrKeyValue + Sized> ops::Deref for PackValueOf<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}
impl<T: AttrKeyValue + Sized> ops::DerefMut for PackValueOf<T> {
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
