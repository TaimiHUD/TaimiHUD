use {
    futures::future, serde::{Deserialize, Serialize}, std::{borrow::Cow, hint::unreachable_unchecked, mem, ops, pin::Pin, ptr, sync::OnceLock}, tokio::{sync::watch, time}
};

#[derive(Debug)]
pub struct Watcher<T> {
    watch: OnceLock<watch::Receiver<T>>,
}

impl<T> Watcher<T> {
    pub const EMPTY: Self = Self { watch: OnceLock::new() };

    pub fn new(value: T) -> Self {
        Self::with_sender(watch::Sender::new(value))
    }
    pub fn with_opt(value: Option<T>) -> Self {
        value.map(Self::new).unwrap_or(Self::EMPTY)
    }
    /// TODO: is this guaranteed to work correctly if sender_count() is 0?
    pub fn with_receiver(watch: watch::Receiver<T>) -> Self {
        let mut this = Self::EMPTY;
        this.set_receiver(watch);
        this
    }
    pub fn with_sender(watch: watch::Sender<T>) -> Self {
        let mut this = Self::EMPTY;
        this.set_sender(watch);
        this
    }
    pub fn subscribe_to(sender: &watch::Sender<T>) -> Self {
        Self::with_sender(sender.clone())
    }

    pub fn watched(&self) -> Watched<T>
    where
        T: Clone,
    {
        Watched::with_watcher(self.clone())
    }

    /// Leaks sender so one tx reference count and one rx ref is "owned"
    pub fn sender_to_receiver(sender: watch::Sender<T>) -> watch::Receiver<T> {
        let receiver = sender.subscribe();
        mem::forget(sender);
        receiver
    }
    #[cfg(todo)]
    /// the reference semantics involved are too awkward to recommend using this
    pub unsafe fn receiver_to_sender(receiver: watch::Receiver<T>) -> watch::Sender<T> {
        // TODO: need to determine offset of version vs ptr, but rustc shouldn't shuffle two usize fields right? right?
        let sender = unsafe { mem::transmute_copy(&receiver) };
        mem::forget(receiver);
        sender
    }
    fn receiver_as_sender(receiver: &watch::Receiver<T>) -> &watch::Sender<T> {
        unsafe { &*(receiver as *const watch::Receiver<T> as *const watch::Sender<T>) }
    }
    /// unnecessary since sender has no methods that take &mut self, but why not...
    fn receiver_as_sender_mut(receiver: &mut watch::Receiver<T>) -> &mut watch::Sender<T> {
        unsafe { &mut *(receiver as *mut watch::Receiver<T> as *mut watch::Sender<T>) }
    }

    pub fn init(&self, value: T) -> &watch::Receiver<T> {
        let sender = watch::Sender::new(value);
        let watch = Self::sender_to_receiver(sender);
        unsafe {
            self.init_receiver(watch)
        }
    }
    pub fn init_sender(&self, sender: watch::Sender<T>) -> &watch::Sender<T> {
        let watch = Self::sender_to_receiver(sender);
        let receiver = unsafe { self.init_receiver(watch) };
        Self::receiver_as_sender(receiver)
    }

    /// Must have had reference counts adjusted by [Self::sender_to_receiver]
    pub unsafe fn init_receiver(&self, watch: watch::Receiver<T>) -> &watch::Receiver<T> {
        self.watch.get_or_init(|| watch)
    }

    pub fn set_sender(&mut self, sender: watch::Sender<T>) {
        let watch = Self::sender_to_receiver(sender);
        unsafe {
            self.set_watch(watch)
        }
    }
    fn set_receiver(&mut self, receiver: watch::Receiver<T>) {
        let sender = Self::receiver_as_sender(&receiver).clone();
        mem::forget(sender);
        unsafe {
            self.set_watch(receiver)
        }
    }
    /// Must have had reference counts adjusted by [Self::sender_to_receiver]
    pub unsafe fn set_watch(&mut self, w: watch::Receiver<T>) {
        match self.watch.get_mut() {
            Some(watch) => {
                let prev = mem::replace(watch, w);
                Self::receiver_to_parts(prev);
            },
            None => {
                self.init_receiver(w);
            },
        }
    }

    pub fn set(&self, value: T) {
        let mut value = Some(value);
        let receiver = self.watch.get_or_init(|| {
            let value = match value.take() {
                Some(v) => v,
                None => unsafe { unreachable_unchecked() },
            };
            Self::sender_to_receiver(watch::Sender::new(value))
        });
        if let Some(value) = value {
            Self::receiver_as_sender(receiver).send_modify(move |out| *out = value)
        }
    }

    /// the inverse of [Self::sender_to_receiver]
    pub unsafe fn receiver_to_parts(watch: watch::Receiver<T>) -> (watch::Sender<T>, watch::Receiver<T>) {
        let sender = ptr::read(Self::receiver_as_sender(&watch));
        (sender, watch)
    }

    pub fn take_parts(&mut self) -> Option<(watch::Sender<T>, watch::Receiver<T>)> {
        let watch = self.watch.take()?;

        let parts = unsafe { Self::receiver_to_parts(watch) };
        Some(parts)
    }

    pub fn into_parts(self) -> Option<(watch::Sender<T>, watch::Receiver<T>)> {
        let mut this = mem::ManuallyDrop::new(self);
        this.take_parts()
    }

    pub fn get_receiver(&self) -> Option<&watch::Receiver<T>> {
        self.watch.get()
    }
    pub fn get_receiver_mut(&mut self) -> Option<&mut watch::Receiver<T>> {
        self.watch.get_mut()
    }

    pub fn get_sender(&self) -> Option<&watch::Sender<T>> {
        self.watch.get().map(Self::receiver_as_sender)
    }
    pub fn get_sender_mut(&mut self) -> Option<&mut watch::Sender<T>> {
        self.watch.get_mut().map(Self::receiver_as_sender_mut)
    }

    pub fn has_changed(&self) -> bool {
        match self.get_receiver().map(|r| r.has_changed()) {
            Some(Ok(changed)) => changed,
            #[cfg(todo = "unnecessary")]
            Some(Err(..)) => unsafe { unreachable_unchecked() },
            _ => false,
        }
    }
    pub fn mark_unchanged(&mut self) {
        if let Some(r) = self.get_receiver_mut() {
            r.mark_unchanged();
        }
    }
    pub fn try_mark_changed(&mut self) -> Result<(), ()> {
        if let Some(r) = self.get_receiver_mut() {
            r.mark_changed();
            Ok(())
        } else {
            Err(())
        }
    }
    pub fn try_read(&self) -> Option<watch::Ref<'_, T>> {
        self.get_receiver().map(|r| r.borrow())
    }
    pub fn try_read_update(&mut self) -> Option<watch::Ref<'_, T>> {
        self.get_receiver_mut().map(|r| r.borrow_and_update())
    }

    pub async fn when_changed(&mut self) {
        if let Some(receiver) = self.get_receiver_mut() {
            match receiver.changed().await {
                Ok(()) => return,
                Err(..) => (),
            }
        }
        future::pending().await
    }
    pub async fn watch<F: FnMut(&T) -> bool>(&mut self, cond: F) -> watch::Ref<'_, T> {
        match self.get_receiver_mut() {
            Some(watch) => match watch.wait_for(cond).await {
                Ok(w) => return w,
                #[cfg(todo = "unnecessary")]
                Err(..) => unsafe { unreachable_unchecked() },
                Err(..) => (),
            },
            None => (),
        }
        future::pending().await
    }
}

impl<T: Default> Watcher<T> {
    pub fn new_default() -> Self {
        Self::new(T::default())
    }

    fn initial() -> watch::Receiver<T> {
        Self::sender_to_receiver(watch::Sender::new(T::default()))
    }

    pub fn sender(&self) -> &watch::Sender<T> {
        Self::receiver_as_sender(self.receiver())
    }
    pub fn receiver(&self) -> &watch::Receiver<T> {
        self.init(T::default())
    }

    pub fn read(&self) -> watch::Ref<'_, T> {
        self.receiver().borrow()
    }
    pub fn read_update(&mut self) -> watch::Ref<'_, T> {
        self.receiver_mut().borrow_and_update()
    }
    pub fn write_with<F: FnOnce(&mut T)>(&self, f: F) {
        self.sender().send_modify(f)
    }
    pub fn write_if<F: FnOnce(&mut T) -> bool>(&self, f: F) -> bool {
        self.sender().send_if_modified(f)
    }

    pub fn receiver_mut(&mut self) -> &mut watch::Receiver<T> {
        if self.watch.get().is_none() {
            self.watch.get_or_init(Self::initial);
        }
        match self.watch.get_mut() {
            Some(watch) => watch,
            None => unsafe { unreachable_unchecked() },
        }
    }
    pub fn sender_mut(&mut self) -> &mut watch::Sender<T> {
        Self::receiver_as_sender_mut(self.receiver_mut())
    }
}

impl<T> Drop for Watcher<T> {
    fn drop(&mut self) {
        let _ = self.take_parts();
    }
}

impl<T> From<T> for Watcher<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
impl<T> Default for Watcher<T> {
    fn default() -> Self {
        Self::EMPTY
    }
}
impl<T> Clone for Watcher<T> {
    fn clone(&self) -> Self {
        match self.get_receiver() {
            Some(watch) => Self::with_receiver(watch.clone()),
            None => Self::EMPTY,
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Watcher<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <Option<T> as Deserialize<'de>>::deserialize(deserializer).map(Self::with_opt)
    }
}

impl<T: Serialize> Serialize for Watcher<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let watch = self.try_read();
        let watch: Option<&T> = watch.as_ref().map(|s| &**s);
        watch.serialize(serializer)
    }
}

#[derive(Debug, Clone)]
pub struct Watched<T: Clone> {
    pub watch: Watcher<T>,
    pub cached: Option<T>,
}

impl<T: Clone> Watched<T> {
    pub const EMPTY: Self = Self { watch: Watcher::EMPTY, cached: None };

    pub fn new(value: T) -> Self {
        let cached = Some(value.clone());
        Self { watch: Watcher::new(value), cached }
    }

    pub const fn empty_with(value: T) -> Self {
        Self {
            watch: Watcher::EMPTY,
            cached: Some(value),
        }
    }
    pub fn subscribe_to(sender: &watch::Sender<T>) -> Self {
        Self::new_with_watcher(Watcher::subscribe_to(sender))
    }
    pub fn start_watching(sender: &watch::Sender<T>) -> Self {
        let mut watched = Self::new_with_watcher(Watcher::subscribe_to(sender));
        let _ = watched.watch.try_mark_changed();
        watched
    }

    pub const fn new_with_watcher(watch: Watcher<T>) -> Self {
        Self { watch, cached: None }
    }

    pub fn with_watcher(watch: Watcher<T>) -> Self {
        let cached = watch.try_read().map(|w| (*w).clone());
        Self { watch, cached }
    }
    pub fn with_sender(sender: watch::Sender<T>) -> Self {
        Self::new_with_watcher(Watcher::with_sender(sender))
    }
    pub fn with_receiver(receiver: watch::Receiver<T>) -> Self {
        Self::new_with_watcher(Watcher::with_receiver(receiver))
    }
    pub fn start_receiving(mut receiver: watch::Receiver<T>) -> Self {
        receiver.mark_changed();
        Self::with_receiver(receiver)
    }

    pub fn try_read_update(&mut self) -> Option<&mut T> {
        match self.watch.try_read_update().map(|w| w.clone()) {
            Some(v) => Some(self.cached.insert(v)),
            None => None,
        }
    }

    pub fn try_get_mut(&mut self) -> Option<&mut T> {
        if self.cached.is_none() {
            match self.watch.try_read().map(|w| w.clone()) {
                Some(v) => {
                    let _ = self.cached.insert(v);
                },
                None => (),
            }
        }
        self.cached.as_mut()
    }
    pub fn try_read_mut(&mut self) -> Option<&mut T> {
        if self.cached.is_none() || self.watch.has_changed() {
            self.try_read_update();
        }
        self.cached.as_mut()
    }
    pub async fn when_changed(&mut self) -> &mut T {
        self.watch.when_changed().await;
        match self.try_read_update() {
            Some(v) => v,
            None => future::pending().await,
        }
    }
    pub async fn watch<F: FnMut(&T, &mut Option<T>) -> bool>(&mut self, mut cond: F) -> &mut T {
        let value =  {
            let cached = &mut self.cached;
            self.watch.watch(move |v|
                cond(v, cached)
            ).await
        };
        self.cached.insert(value.clone())
    }
}

impl<T: Clone + Default> Watched<T> {
    pub fn new_default() -> Self {
        Self::new(T::default())
    }

    pub fn get_ref(&mut self) -> &T {
        &*self.get_mut()
    }
    pub fn borrow_mut(&mut self) -> &mut T {
        match self.watch.has_changed() {
            false if self.cached.is_some() => match &mut self.cached {
                Some(c) => c,
                None => unsafe { unreachable_unchecked() },
            },
            _ => {
                let latest = self.watch.read().clone();
                self.cached.insert(latest)
            },
        }
    }
    pub fn get_mut(&mut self) -> &mut T {
        match self.watch.has_changed() {
            false if self.cached.is_some() => match &mut self.cached {
                Some(c) => c,
                None => unsafe { unreachable_unchecked() },
            },
            _ => {
                let latest = self.watch.read_update().clone();
                self.cached.insert(latest)
            },
        }
    }
    pub fn get(&self) -> Cow<'_, T> {
        if let Some(cached) = &self.cached {
            return Cow::Borrowed(cached)
        } else if self.watch.has_changed() {
            Cow::Owned(self.watch.read().clone())
        } else {
            Cow::default()
        }
    }
}

impl<T: Clone> ops::Deref for Watched<T>
where
    // the alternative is to panic when empty, so...
    T: Default + Send + Sync + 'static,
{
    type Target = T;

    fn deref(&self) -> &T {
        match &self.cached {
            Some(c) => c,
            None => {
                log::debug!("Watched default_ref fallback hit, likely a bug!");
                default_ref::<T>()
            },
        }
    }
}
impl<T: Clone + Default> ops::DerefMut for Watched<T>
where
    // :<
    Self: ops::Deref<Target = T>,
{
    fn deref_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}

impl<T: Clone> From<T> for Watched<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
impl<T: Clone> Default for Watched<T> {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl<'de, T: Clone + Deserialize<'de>> Deserialize<'de> for Watched<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Deserialize::<'de>::deserialize(deserializer).map(Self::with_watcher)
    }
}

impl<T: Clone + Serialize> Serialize for Watched<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.watch.serialize(serializer)
    }
}

#[test]
fn watcher_sender_receiver_abi() {
    let sender = watch::Sender::new(5usize);
    let receiver = sender.subscribe();
    let (read_shared, read_version) = unsafe {
        (
            ptr::read_volatile(&receiver as *const _ as *const usize),
            ptr::read_volatile((&receiver as *const _ as *const usize).add(1)),
        )
    };
    assert_eq!(read_version, 0);
    assert_ne!(read_shared, 0);
}

static DEFAULT_REFS: std::sync::Mutex<std::collections::BTreeMap<std::any::TypeId, usize>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());
pub fn default_ref<T: Default + Send + Sync>() -> &'static T {
    let id = std::any::TypeId::of::<T>();
    let default_ref: usize = *DEFAULT_REFS.lock().unwrap().entry(id).or_insert_with(|| {
        let default = Box::<T>::default();
        Box::into_raw(default) as usize
    });
    unsafe { &*(default_ref as *const T) }
}

/// TODO: Option because unclear if sleep is fused or what...
pub type WatchThrottleDelay = Option<Pin<Box<time::Sleep>>>;
