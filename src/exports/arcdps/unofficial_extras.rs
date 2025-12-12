use {
    crate::exports::{arcdps as exports, runtime as rt},
    arcdps::extras::{self, ExtrasSubscriberInfo},
    std::{ffi::CStr, panic, str},
    taimi_input::win::keyboard::keybind_change_from_raw,
};

pub(crate) unsafe fn extras_init_raw(
    info: *const extras::RawExtrasAddonInfo,
    subscriber: *mut ExtrasSubscriberInfo,
) {
    if !exports::loaded() {
        crate::crate_init();
    }

    let res = panic::catch_unwind(|| {
        if info.is_null() || subscriber.is_null() {
            log::warn!("arcdps_unofficial_extras init missing required argument");
            return
        }
        #[cfg(feature = "extension-nexus")]
        if rt::nexus_available() || exports::check_for_nexus() {
            log::info!("ignoring arcdps_unofficial_extras, nexus is available");
            return
        }

        let info = &*info;
        let version = info.version();
        if !version.is_compatible() {
            log::info!(
                "unsupported arcdps_unofficial_extras api version {}/{}",
                version.api_version,
                version.max_info_version
            );
            return
        }

        let (squad_update, language_changed, keybind_changed) =
            extras_callbacks().unwrap_or(EXTRAS_CALLBACKS_DEFAULT);
        let squad_chat_message = None;
        let chat_message = None;

        // there's a type for this you know...
        let name = str::from_utf8_unchecked(rt::NAME_C.to_bytes_with_nul());
        ExtrasSubscriberInfo::subscribe(
            subscriber,
            info,
            name,
            squad_update,
            language_changed,
            keybind_changed,
            squad_chat_message,
            chat_message,
        );

        if (*subscriber).header.info_version == 0 {
            log::warn!(
                "arcdps-rs refused to subscribe to extras (api {})",
                version.api_version
            );
            return
        }
        let account_name = match info.self_account_name {
            name if name.is_null() => None,
            name => Some(CStr::from_ptr(name as *const _)),
        };
        if let Some(name) = account_name {
            crate::receive_account_name(name.to_string_lossy());
        }

        exports::extras_init(version);
    });
    if let Err(e) = res {
        crate::log_any_error("arcdps_unofficial_extras_subscriber_init", &e);
    }
}

pub(crate) unsafe fn extras_release() {
    log::info!("extras unload");

    match () {
        #[cfg(feature = "closure-ffi")]
        _ => hotload::extras_release(),
        #[cfg(not(feature = "closure-ffi"))]
        _ => {
            // TODO: if game exiting, don't bother saying this
            log::error!("cannot unsubscribe, expect to crash soon");
        },
    }
}

/// Once reloaded, try to restore our callbacks
pub(crate) fn extras_resubscribe() -> bool {
    #[cfg(feature = "closure-ffi")]
    const UNOFFICIAL_EXTRAS_SIG: u32 = 0xb39746bf;

    match () {
        #[cfg(feature = "closure-ffi")]
        _ if exports::has_extension(UNOFFICIAL_EXTRAS_SIG) == Some(true) && hotload::can_reclaim() =>
            hotload::extras_resubscribe() != (None, None, None),
        _ => false,
    }
}

const EXTRAS_CALLBACKS_DEFAULT: ExtrasCallbackFns = (
    Some(cb_squad_update_raw),
    Some(cb_language_changed_raw),
    Some(cb_keybind_changed_raw),
);
fn extras_callbacks() -> Option<ExtrasCallbackFns> {
    match () {
        #[cfg(feature = "closure-ffi")]
        _ => {
            let cbs = hotload::extras_resubscribe();
            if cbs != (None, None, None) {
                Some(cbs)
            } else {
                None
            }
        },
        #[cfg(not(feature = "closure-ffi"))]
        _ => None,
    }
}

pub(crate) type SquadUpdateFn = unsafe extern "C-unwind" fn(*const extras::user::UserInfo, u64);
pub(crate) type LanguageChangedFn = unsafe extern "C-unwind" fn(arcdps::Language);
pub(crate) type KeybindChangedFn = unsafe extern "C-unwind" fn(extras::keybinds::RawKeybindChange);
pub(crate) type ExtrasCallbackFns = (
    Option<SquadUpdateFn>,
    Option<LanguageChangedFn>,
    Option<KeybindChangedFn>,
);

#[inline(never)]
pub(crate) unsafe extern "C-unwind" fn cb_squad_update_raw(users: *const extras::user::UserInfo, len: u64) {
    exports::extras_squad_update(extras::user::to_user_info_iter(users, len))
}

#[inline(never)]
pub(crate) unsafe extern "C-unwind" fn cb_language_changed_raw(language: arcdps::Language) {
    rt::notify_game_language(language)
}

#[inline(never)]
pub(crate) unsafe extern "C-unwind" fn cb_keybind_changed_raw(keybind: extras::keybinds::RawKeybindChange) {
    rt::bindings::process_key_bound(keybind_change_from_raw(&keybind));
}

#[cfg(feature = "closure-ffi")]
mod hotload {
    use {
        super::ExtrasCallbackFns,
        crate::exports::runtime as rt,
        anyhow::{anyhow, Context},
        closure_ffi::{
            cc::CUnwind as C,
            jit_alloc::{JitAlloc, JitAllocError, ProtectJitAccess},
            BareFnAny,
        },
        jit_allocator2::{JitAllocator, JitAllocatorOptions},
        std::{
            collections::BTreeMap,
            ffi::CString,
            mem,
            ops,
            process,
            ptr,
            slice,
            sync::{LazyLock, Mutex},
        },
    };
    pub unsafe fn extras_release() {
        let mut callbacks = match CALLBACKS.lock() {
            Ok(callbacks) if callbacks.is_empty() => return,
            Ok(callbacks) => callbacks,
            Err(..) => {
                log::warn!("extras callbacks poisoned");
                return
            },
        };

        callbacks.leak_unload();

        if !rt::is_shutdown() {
            log::debug!("leaking JIT allocations");
            let res = JIT.abandon().context("Failed to leave extras");
            if let Err(e) = &res {
                log::warn!("{e:#}");
            }
        } else {
            // game quitting, don't bother
            JIT.leak();
        };
    }

    pub fn can_reclaim() -> bool {
        !JIT.is_registry_empty()
    }

    pub fn extras_resubscribe() -> ExtrasCallbackFns {
        let mut callbacks = CALLBACKS.lock();
        let callbacks = match &mut callbacks {
            Ok(callbacks) if callbacks.is_empty() => {
                let res = ExtrasCallbacks::new_cb().context("failed to allocate extras callbacks");
                match res {
                    Ok(cb) => **callbacks = cb,
                    Err(e) => log::warn!("{e:#}"),
                }
                callbacks
            },
            Ok(callbacks) => callbacks,
            Err(..) => {
                log::warn!("extras callbacks poisoned");
                return (None, None, None)
            },
        };
        (
            callbacks.squad_update_bare(),
            callbacks.language_changed_bare(),
            callbacks.keybind_changed_bare(),
        )
    }

    #[derive(Default)]
    pub struct SharedJit {
        pub jit: Mutex<Option<Box<Jit>>>,
    }

    impl SharedJit {
        pub fn new() -> Self {
            Self::with_jit(Jit::new())
        }

        pub fn with_jit<J: Into<Box<Jit>>>(jit: J) -> Self {
            Self { jit: Mutex::new(Some(jit.into())) }
        }

        pub fn is_registry_empty(&self) -> bool {
            self.map_jit(|jit| Ok(jit.registry.is_empty())).unwrap_or(true)
        }

        pub fn reclaim_or_new() -> Self {
            let reclaimed = Self::reclaim().context("Failed to reclaim JIT allocator");
            match reclaimed {
                Ok(jit) => jit,
                Err(e) => {
                    log::warn!("{e:#}");
                    None
                },
            }
            .unwrap_or_else(|| Self::new())
        }

        pub fn reclaim() -> anyhow::Result<Option<Self>> {
            Self::map_reclaim_handle(false, |w| unsafe {
                let w = match ptr::NonNull::new(w) {
                    Some(ptr) => ptr.as_ptr(),
                    None => return Ok(None),
                };
                let data = ptr::read_volatile(w);
                ptr::write_volatile(w, Default::default());
                let p: *mut Jit = mem::transmute(data);
                let jit = ptr::NonNull::new(p)
                    .map(|jit| Box::from_raw(jit.as_ptr()))
                    .map(Self::with_jit);
                Ok(jit)
            })
        }

        const RECLAIM_SIZE: usize = mem::size_of::<*mut Jit>();

        pub fn abandon(&self) -> anyhow::Result<()> {
            let mut jit = match self.jit.lock() {
                Ok(jit) => jit,
                Err(e) => {
                    mem::forget(e.into_inner().take());
                    anyhow::bail!("JIT poisoned, leaking...")
                },
            };
            let jit = match jit.take() {
                None => return Ok(()),
                Some(jit) => jit,
            };
            let ptr = Box::into_raw(jit);
            Self::map_reclaim_handle(true, |w| unsafe {
                let w = ptr::NonNull::new(w).ok_or_else(|| anyhow!("JIT stash unavailable"))?;
                let data = &ptr as *const *mut Jit as *const [u8; Self::RECLAIM_SIZE];
                ptr::write_volatile(w.as_ptr(), *data);
                Ok(())
            })
        }

        #[cfg(todo = "unused")]
        pub fn cleanup(&self) {
            let res = Self::map_reclaim_handle(false, |w| unsafe {
                if !w.is_null() {
                    ptr::write_volatile(w, Default::default());
                }
                Ok(())
            })
            .context("cleaning up extras JIT");
            if let Err(e) = res {
                log::warn!("{e:#}");
            }
            let mut jit = self.jit.lock().unwrap_or_else(|e| e.into_inner());
            drop(jit.take());
        }

        pub fn leak(&self) {
            let mut jit = self.jit.lock().unwrap_or_else(|e| e.into_inner());
            mem::forget(jit.take());
        }

        fn reclaim_name() -> arcffi::cstr::CStrBox {
            let process_id = process::id();
            let name = format!("Local\\ARC_UE_RECLAIM_JIT_{process_id}");
            arcffi::cstr::CStrBox::with_cstring(unsafe { CString::from_vec_unchecked(name.into()) })
        }

        fn map_reclaim_handle<R, F: FnOnce(*mut [u8; Self::RECLAIM_SIZE]) -> anyhow::Result<R>>(
            create: bool,
            f: F,
        ) -> anyhow::Result<R> {
            use windows::Win32::{
                Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, INVALID_HANDLE_VALUE},
                System::Memory::{
                    CreateFileMappingA,
                    MapViewOfFile,
                    OpenFileMappingA,
                    UnmapViewOfFile,
                    FILE_MAP_WRITE,
                    PAGE_READWRITE,
                },
            };

            let name = Self::reclaim_name();
            let access = FILE_MAP_WRITE;
            unsafe {
                let handle = OpenFileMappingA(access.0, false, &name);
                let handle = match (handle, create) {
                    (Err(..), true) => CreateFileMappingA(
                        INVALID_HANDLE_VALUE,
                        None,
                        PAGE_READWRITE,
                        0,
                        Self::RECLAIM_SIZE as _,
                        &name,
                    )
                    .context("CreateFileMappingA"),
                    (Err(e), false) => {
                        if e.code() != ERROR_FILE_NOT_FOUND.to_hresult() {
                            log::info!("Failed to open JIT mapping: {e}");
                        }
                        return f(ptr::null_mut())
                    },
                    (res, _) => res.context("OpenFileMappingA"),
                }?;
                let map = {
                    let p = MapViewOfFile(handle, access, 0, 0, Self::RECLAIM_SIZE as _);
                    match p {
                        p if p.Value.is_null() => Err(windows::core::Error::from_win32()),
                        p => Ok(p),
                    }
                }
                .context("map file view")?;
                let w = map.Value.cast::<[u8; Self::RECLAIM_SIZE]>();
                let res = f(w);
                let res_unmap = UnmapViewOfFile(map).context("UnmapViewOfFile");
                let res_close = match create {
                    true => Ok(()),
                    false => CloseHandle(handle).context("CloseHandle"),
                };
                if let Err(e) = res_unmap {
                    log::warn!("{e:#}");
                }
                if let Err(e) = res_close {
                    log::warn!("{e:#}");
                }

                res
            }
        }

        pub fn map_jit<R, F: FnOnce(&mut Jit) -> anyhow::Result<R>>(&self, f: F) -> anyhow::Result<R> {
            let mut jit = self.jit.lock();
            let jit = match jit.as_mut().map(|j| &mut **j) {
                Ok(Some(jit)) => jit,
                Ok(None) => anyhow::bail!("JIT inactive"),
                Err(_e) => anyhow::bail!("JIT poisoned"),
            };
            f(jit)
        }

        pub fn reclaim_or_register_cb<F>(
            &self,
            id: usize,
            _f: *const (),
            create: F,
        ) -> anyhow::Result<StoredFn>
        where
            F: FnOnce() -> anyhow::Result<ErasedBareFn>,
        {
            let cb = {
                let cb = self
                    .map_jit(|jit| Ok(jit.registry.get(&id).map(|&cb: &usize| cb as *const u8)))
                    .ok()
                    .flatten();
                cb.map(|cb| {
                    (
                        cb,
                        self.map_jit(|jit| jit.query(cb).map_err(|e| anyhow!("{e:?}"))),
                    )
                })
            };
            let cb = match cb {
                Some((cb, Ok(cb_w))) => {
                    log::debug!("attempting to reclaim cb#{id} at {:p}: {:?}", cb, cb_w);
                    Some((cb, cb_w))
                },
                Some((cb, Err(_))) => {
                    log::warn!("reclaiming cb#{id} but could not find allocation for {cb:p}");
                    // TODO: try anyway via retour/mh or no..?
                    None
                },
                None => None,
            };

            let res = create()?;

            if let Some((cb, (cb_r, cb_w, cb_size))) = cb {
                let len = cb_size - (cb as usize - cb_r as usize);
                unsafe {
                    // TODO: replace closure-ffi with another stub (or retour/mh?)
                    let pat = res.bare();
                    let template = slice::from_raw_parts(pat as *const u8, len);
                    self.copy_stub(template, cb, cb_r, cb_w, cb_size);
                }
                return Ok(Err(cb as *const _))
            }

            let _ = self.map_jit(|jit| {
                jit.registry.insert(id, res.bare() as usize);
                Ok(())
            });

            Ok(Ok(res))
        }

        pub unsafe fn copy_stub(
            &self,
            stub_template: &[u8],
            dst: *const u8,
            alloc_r: *const u8,
            alloc_w: *mut u8,
            size: usize,
        ) {
            let offset = dst.offset_from_unsigned(alloc_r);
            self.protect_jit_memory(alloc_r, size, ProtectJitAccess::ReadWrite);
            // TODO: volatile?
            ptr::copy_nonoverlapping(stub_template.as_ptr(), alloc_w.add(offset), stub_template.len());

            self.protect_jit_memory(alloc_r, size, ProtectJitAccess::ReadExecute);
            self.flush_instruction_cache(alloc_r, size);
        }
    }

    impl JitAlloc for SharedJit {
        fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
            let res = self
                .map_jit(move |jit| jit.alloc(size).map_err(|e| anyhow!("{e:?}")))
                .context("JIT alloc failed");

            match res {
                Ok((rx, rw)) => Ok((rx, rw)),
                Err(e) => {
                    log::warn!("{e:#}");
                    Err(JitAllocError)
                },
            }
        }

        unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
            let res = self
                .map_jit(move |jit| jit.release(rx_ptr).map_err(|e| anyhow!("{e:?}")))
                .context("JIT release failed");

            match res {
                Ok(()) => Ok(()),
                Err(e) => {
                    log::warn!("{e:#}");
                    Err(JitAllocError)
                },
            }
        }

        unsafe fn protect_jit_memory(&self, _ptr: *const u8, _size: usize, access: ProtectJitAccess) {
            let access = match access {
                ProtectJitAccess::ReadExecute => jit_allocator2::ProtectJitAccess::ReadExecute,
                ProtectJitAccess::ReadWrite => jit_allocator2::ProtectJitAccess::ReadWrite,
            };
            // TODO: manually via jit.alloc.query() for page granularity?
            jit_allocator2::protect_jit_memory(access)
        }

        unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
            jit_allocator2::flush_instruction_cache(rx_ptr, size)
        }
    }

    impl Drop for SharedJit {
        fn drop(&mut self) {
            let empty = self
                .jit
                .lock()
                .ok()
                .and_then(|j| j.as_ref().map(|j| j.registry.is_empty()))
                .unwrap_or(false);
            if !empty {
                self.leak();
            }
        }
    }

    pub struct Jit {
        pub alloc: JitAllocator,
        pub(crate) registry: BTreeMap<usize, usize>,
    }

    impl Jit {
        pub fn new() -> Self {
            let opts = JitAllocatorOptions {
                use_dual_mapping: false,
                use_multiple_pools: false,
                immediate_release: true,
                ..Default::default()
            };
            Self::with_allocator(*JitAllocator::new(opts))
        }

        pub fn with_allocator<J: Into<JitAllocator>>(alloc: J) -> Self {
            let alloc = alloc.into();
            Self { alloc, registry: BTreeMap::new() }
        }
    }
    impl ops::Deref for Jit {
        type Target = JitAllocator;

        fn deref(&self) -> &Self::Target {
            &self.alloc
        }
    }
    impl ops::DerefMut for Jit {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.alloc
        }
    }
    pub(crate) static CALLBACKS: Mutex<ExtrasCallbacks> = Mutex::new(ExtrasCallbacks::EMPTY);
    pub(crate) static JIT: LazyLock<SharedJit> = LazyLock::new(|| SharedJit::reclaim_or_new());
    type StaticJitAlloc = &'static SharedJit;
    type ErasedBareFn = closure_ffi::UntypedBareFn<dyn Send, StaticJitAlloc>;
    type StoredFn = Result<ErasedBareFn, *const ()>;

    #[derive(Default)]
    pub(crate) struct ExtrasCallbacks {
        squad_update: Option<StoredFn>,
        language_changed: Option<StoredFn>,
        keybind_changed: Option<StoredFn>,
        //squad_chat_message: Option<StoredFn>,
        //chat_message: Option<StoredFn>,
    }

    impl ExtrasCallbacks {
        pub const EMPTY: Self = Self {
            squad_update: None,
            language_changed: None,
            keybind_changed: None,
        };

        pub const ID_SQUAD_UPDATE: usize = 0;
        pub const ID_LANGUAGE_CHANGED: usize = 1;
        pub const ID_KEYBIND_CHANGED: usize = 2;

        pub fn new_cb() -> anyhow::Result<Self> {
            let squad_update = |users, len| unsafe { super::cb_squad_update_raw(users, len) };
            let language_changed = |language| unsafe { super::cb_language_changed_raw(language) };
            let keybind_changed = |keybind| unsafe { super::cb_keybind_changed_raw(keybind) };

            // TODO: reclaim and redirect callbacks if found

            let jit = &*JIT;
            let squad_update = jit.reclaim_or_register_cb(
                Self::ID_SQUAD_UPDATE,
                super::cb_squad_update_raw as super::SquadUpdateFn as _,
                || {
                    BareFnAny::try_with_cc_in(C, squad_update, jit)
                        .map_err(|e| anyhow!("{e:?}"))
                        .map(|cb| cb.into_untyped())
                },
            )?;
            let language_changed = jit.reclaim_or_register_cb(
                Self::ID_LANGUAGE_CHANGED,
                super::cb_language_changed_raw as super::LanguageChangedFn as _,
                || {
                    BareFnAny::try_with_cc_in(C, language_changed, jit)
                        .map_err(|e| anyhow!("{e:?}"))
                        .map(|cb| cb.into_untyped())
                },
            )?;
            let keybind_changed = jit.reclaim_or_register_cb(
                Self::ID_KEYBIND_CHANGED,
                super::cb_keybind_changed_raw as super::KeybindChangedFn as _,
                || {
                    BareFnAny::try_with_cc_in(C, keybind_changed, jit)
                        .map_err(|e| anyhow!("{e:?}"))
                        .map(|cb| cb.into_untyped())
                },
            )?;

            Ok(Self {
                squad_update: Some(squad_update),
                language_changed: Some(language_changed),
                keybind_changed: Some(keybind_changed),
            })
        }

        /// the dragons have become mush
        unsafe fn stub_fn(id: usize, f: *const (), textf: *const ()) {
            if f.is_null() || f == textf {
                return
            }

            let stub_template: &[u8] = stub_template_bytes();

            let f = f as *const u8;
            let jit = &*JIT;
            let f_w = jit
                .map_jit(|jit| {
                    jit.registry.insert(id, f as usize);
                    jit.query(f).map_err(|e| anyhow!("{e:?}"))
                })
                .ok();
            let (base, f_w, len) = match f_w {
                Some((base, f_w, len)) => (base, f_w, len),
                None => {
                    log::warn!("query JIT memory {f:p} failed");
                    // TODO? (f, f as *mut u8, stub_template.len())
                    return
                },
            };
            jit.copy_stub(stub_template, f, base, f_w, len);
        }

        /// Replace allocated trampoline with a no-op stub
        pub fn leak_unload(&mut self) {
            let squad_update = self
                .squad_update
                .take()
                .map(|cb| cb.map(|cb| cb.leak()).unwrap_or_else(|cb| cb));
            let language_changed = self
                .language_changed
                .take()
                .map(|cb| cb.map(|cb| cb.leak()).unwrap_or_else(|cb| cb));
            let keybind_changed = self
                .keybind_changed
                .take()
                .map(|cb| cb.map(|cb| cb.leak()).unwrap_or_else(|cb| cb));

            unsafe {
                if let Some(cb) = squad_update {
                    Self::stub_fn(Self::ID_SQUAD_UPDATE, cb, super::cb_squad_update_raw as *const ());
                }
                if let Some(cb) = language_changed {
                    Self::stub_fn(
                        Self::ID_LANGUAGE_CHANGED,
                        cb,
                        super::cb_language_changed_raw as *const (),
                    );
                }
                if let Some(cb) = keybind_changed {
                    Self::stub_fn(
                        Self::ID_KEYBIND_CHANGED,
                        cb,
                        super::cb_keybind_changed_raw as *const (),
                    );
                }
            }
        }

        pub fn is_empty(&self) -> bool {
            matches!(self, Self {
                squad_update: None,
                language_changed: None,
                keybind_changed: None
            })
        }

        pub fn squad_update_bare(&self) -> Option<super::SquadUpdateFn> {
            self.squad_update
                .as_ref()
                .map(|cb| unsafe { mem::transmute(Self::cb_bare(cb)) })
        }

        pub fn language_changed_bare(&self) -> Option<super::LanguageChangedFn> {
            self.language_changed
                .as_ref()
                .map(|cb| unsafe { mem::transmute(Self::cb_bare(cb)) })
        }

        pub fn keybind_changed_bare(&self) -> Option<super::KeybindChangedFn> {
            self.keybind_changed
                .as_ref()
                .map(|cb| unsafe { mem::transmute(Self::cb_bare(cb)) })
        }

        fn cb_bare(cb: &StoredFn) -> *const () {
            cb.as_ref().map(|cb| cb.bare()).unwrap_or_else(|&cb| cb)
        }

        fn cb_drop(cb: &mut Option<StoredFn>) {
            if let Some(Ok(cb)) = cb.take() {
                let cb = cb.leak();
                log::warn!("leaking extra cb: {cb:p}");
            }
        }
    }

    impl Drop for ExtrasCallbacks {
        fn drop(&mut self) {
            if !self.is_empty() {
                Self::cb_drop(&mut self.squad_update);
                Self::cb_drop(&mut self.language_changed);
                Self::cb_drop(&mut self.keybind_changed);
            }
        }
    }

    unsafe impl Send for ExtrasCallbacks {}

    // TODO: macro to construct jmp/call template wrappers then remove closure-ffi

    /// XXX: this all feels silly when hard-coding the encoded bytes
    /// wouldn't be that unreasonable .-.
    fn stub_template_bytes() -> &'static [u8] {
        unsafe {
            match () {
                #[cfg(any(target_arch = "x86_64"))]
                _ => &__EXTRAS_STUB_TEMPLATE,
                #[cfg(not(any(target_arch = "x86_64")))]
                _ => &*(__extras_stub_template as unsafe extern "C" fn() as usize as *const [u8; 8]),
            }
        }
    }

    #[cfg(any(target_arch = "x86_64"))]
    extern "C" {
        #[link_name = "__extras_stub_template"]
        static __EXTRAS_STUB_TEMPLATE: [u8; 8];
    }
    #[cfg(target_arch = "x86_64")]
    core::arch::global_asm! {
        ".global {stub_template_return}",
        ".balign 8",
        "{stub_template_return}:",
        "ret",
        "nop",
        ".balign 8",
        stub_template_return = sym __EXTRAS_STUB_TEMPLATE,
    }

    /* XXX: once rustc is updated to 1.88.0, naked functions may be a viable alternative:
    #[cfg(any(target_arch = "x86_64"))]
    #[unsafe(naked)]
    #[link_section = ".data"] // I wonder...
    //#[no_mangle]
    unsafe extern "C" fn __extras_stub_template() {
        core::arch::naked_asm!(
            "ret"
            "nop"
        );
    }*/

    #[cfg(not(any(target_arch = "x86_64")))]
    #[inline(never)]
    #[deprecated = "naive fallback, build architecture not supported"]
    unsafe extern "C" fn __extras_stub_template() {}
}
