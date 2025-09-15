use {
    arcdps::extras::{self, ExtrasSubscriberInfo},
    crate::exports::{arcdps as exports, runtime as rt},
    std::{ffi::CStr, panic, str},
};

pub(crate) unsafe fn extras_init_raw(info: *const extras::RawExtrasAddonInfo, subscriber: *mut ExtrasSubscriberInfo) {
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
            log::info!("unsupported arcdps_unofficial_extras api version {}/{}", version.api_version, version.max_info_version);
            return
        }

        #[cfg(feature = "closure-ffi")]
        let (squad_update, language_changed, keybind_changed) = match hotload::CALLBACKS.lock() {
            Ok(mut callbacks) => {
                if callbacks.is_empty() {
                    match hotload::ExtrasCallbacks::new_cb() {
                        Ok(cb) => {
                            *callbacks = cb;
                        },
                        Err(e) => {
                            log::warn!("failed to allocate extras callbacks: {e:?}");
                        },
                    }
                }
                (
                    callbacks.squad_update_bare(),
                    callbacks.language_changed_bare(),
                    callbacks.keybind_changed_bare(),
                )
            },
            Err(..) => {
                log::warn!("extras callbacks poisoned");
                (None, None, None)
            },
        };
        #[cfg(not(feature = "closure-ffi"))]
        let (squad_update, language_changed, keybind_changed) = (
            Some(cb_squad_update_raw),
            Some(cb_language_changed_raw),
            Some(cb_keybind_changed_raw),
        );
        let squad_chat_message = None;
        let chat_message = None;

        // there's a type for this you know...
        let name = str::from_utf8_unchecked(rt::NAME_C.to_bytes_with_nul());
        ExtrasSubscriberInfo::subscribe(subscriber, info, name,
            squad_update, language_changed, keybind_changed,
            squad_chat_message, chat_message,
        );
        
        let account_name = match info.self_account_name {
            name if name.is_null() =>
                None,
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

#[inline(never)]
pub(crate) unsafe extern "C-unwind" fn cb_squad_update_raw(users: *const extras::user::UserInfo, len: u64) {
    exports::extras_squad_update(extras::user::to_user_info_iter(users, len))
}

#[inline(never)]
pub(crate) unsafe extern "C-unwind" fn cb_language_changed_raw(language: arcdps::Language) {
    exports::extras_language(language)
}

#[inline(never)]
pub(crate) unsafe extern "C-unwind" fn cb_keybind_changed_raw(keybind: extras::keybinds::RawKeybindChange) {
    exports::extras_keybind(keybind.into())
}

#[cfg(feature = "closure-ffi")]
mod hotload {
    use {
        arcdps::{extras::{keybinds::RawKeybindChange, user::UserInfo}, Language},
        closure_ffi::{jit_alloc::JitAllocError, BareFnAny, cc::CUnwind as C},
        jit_allocator2::{JitAllocator, JitAllocatorOptions},
        std::{mem, ptr, sync::{LazyLock, Mutex}},
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

        // TODO: stash away pointers somewhere so we can reclaim them later on hot reload?

        if let Ok(mut jit) = JIT.lock() {
            let jit = mem::replace(&mut *jit, new_jit());
            mem::forget(jit);
            log::debug!("leaking JIT allocations");
        }
    }

    pub(crate) static CALLBACKS: Mutex<ExtrasCallbacks> = Mutex::new(ExtrasCallbacks::EMPTY);
    pub(crate) static JIT: LazyLock<Mutex<JitAllocator>> = LazyLock::new(|| {
        // TODO: impl JitAlloc for box etc
        Mutex::new(new_jit())
    });
    type StaticJitAlloc = &'static LazyLock<Mutex<JitAllocator>>;
    type ErasedBareFn = closure_ffi::UntypedBareFn<dyn Send, StaticJitAlloc>;

    fn new_jit() -> JitAllocator {
        let opts = JitAllocatorOptions {
            use_dual_mapping: false,
            use_multiple_pools: false,
            immediate_release: true,
            .. Default::default()
        };
        let jit = mem::ManuallyDrop::new(JitAllocator::new(opts));
        let jit = unsafe {
            std::ptr::read(&**jit)
        };
        jit
    }

    #[derive(Default)]
    pub(crate) struct ExtrasCallbacks {
        squad_update: Option<ErasedBareFn>,
        language_changed: Option<ErasedBareFn>,
        keybind_changed: Option<ErasedBareFn>,
        //squad_chat_message: Option<ErasedBareFn>,
        //chat_message: Option<ErasedBareFn>,
    }

    impl ExtrasCallbacks {
        pub const EMPTY: Self = Self {
            squad_update: None,
            language_changed: None,
            keybind_changed: None,
        };

        pub fn new_cb() -> Result<Self, JitAllocError> {
            let squad_update = |users, len| unsafe {
                super::cb_squad_update_raw(users, len)
            };
            let language_changed = |language| unsafe {
                super::cb_language_changed_raw(language)
            };
            let keybind_changed = |keybind| unsafe {
                super::cb_keybind_changed_raw(keybind)
            };

            let squad_update = BareFnAny::try_with_cc_in(C, squad_update, &JIT)?;
            let language_changed = BareFnAny::try_with_cc_in(C, language_changed, &JIT)?;
            let keybind_changed = BareFnAny::try_with_cc_in(C, keybind_changed, &JIT)?;

            Ok(Self {
                squad_update: Some(squad_update.into_untyped()),
                language_changed: Some(language_changed.into_untyped()),
                keybind_changed: Some(keybind_changed.into_untyped()),
            })
        }

        /// the dragons have become mush
        unsafe fn stub_fn(f: *const ()) {
            use closure_ffi::jit_alloc::{JitAlloc, ProtectJitAccess};

            let stub_template: &[u8] = &__EXTRAS_STUB_TEMPLATE;

            let f = f as *const u8;
            let f_w = JIT.lock()
                .ok().and_then(|mut jit| {
                    jit.query(f).ok()
                });
            let (base, f_w, len) = match f_w {
                Some((base, f_w, len)) => {
                    (base, f_w, len)
                },
                None => {
                    log::warn!("query JIT memory {f:p} failed");
                    (f, f as *mut u8, stub_template.len())
                },
            };
            let jit = &JIT;
            let offset = f.offset_from_unsigned(base);
            jit.protect_jit_memory(base, len, ProtectJitAccess::ReadWrite);
            ptr::copy_nonoverlapping(stub_template.as_ptr(), f_w.add(offset), stub_template.len());

            jit.protect_jit_memory(base, len, ProtectJitAccess::ReadExecute);
            jit.flush_instruction_cache(base, len);
        }

        /// Replace allocated trampoline with a no-op stub
        pub fn leak_unload(&mut self) {
            let squad_update = self.squad_update.take().map(|cb| cb.leak());
            let language_changed = self.language_changed.take().map(|cb| cb.leak());
            let keybind_changed = self.keybind_changed.take().map(|cb| cb.leak());

            unsafe {
                if let Some(cb) = squad_update {
                    Self::stub_fn(cb);
                }
                if let Some(cb) = language_changed {
                    Self::stub_fn(cb);
                }
                if let Some(cb) = keybind_changed {
                    Self::stub_fn(cb);
                }
            }
        }

        pub fn is_empty(&self) -> bool {
            matches!(self, Self { squad_update: None, language_changed: None, keybind_changed: None })
        }

        pub fn squad_update_bare(&self) -> Option<unsafe extern "C-unwind" fn(*const UserInfo, u64)> {
            self.squad_update.as_ref()
                .map(|cb| unsafe {
                    mem::transmute(cb.bare())
                })
        }

        pub fn language_changed_bare(&self) -> Option<unsafe extern "C-unwind" fn(Language)> {
            self.language_changed.as_ref()
                .map(|cb| unsafe {
                    mem::transmute(cb.bare())
                })
        }

        pub fn keybind_changed_bare(&self) -> Option<unsafe extern "C-unwind" fn(RawKeybindChange)> {
            self.keybind_changed.as_ref()
                .map(|cb| unsafe {
                    mem::transmute(cb.bare())
                })
        }
    }

    #[cfg(target_arch = "x86_64")]
    extern "C" {
        static __EXTRAS_STUB_TEMPLATE: [u8; 8];
    }
    // TODO: this is silly just hard-code it
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

    unsafe impl Sync for ExtrasCallbacks {}
}
