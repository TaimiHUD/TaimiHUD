use {
    crate::{
        exports::{
            nexus as exports,
            runtime::{self as rt, log::DeferredLogger},
            update_url_of,
            ADDON_TITLE_C,
        },
        settings::state::{AddonHostName, BootstrapState},
    },
    core::{ffi::c_char, mem, ptr},
    nexus::{
        addon::{AddonDefinition, AddonVersion, UpdateProvider},
        AddonApi,
    },
    std::{
        ffi::{CStr, CString},
        panic,
    },
    sync_unsafe_cell::SyncUnsafeCell,
    taimi_hoard::str_opt_ref,
};

/// nexus copies the def, so lifetime isn't actually 'static, it just outlives
/// the fn call
///
/// TODO: remove nexus-rs dependency entirely and switch to arcloader's types...
/// won't be easy given how often it's used throughout the project for imgui
/// .-.
#[export_name = "GetAddonDef"]
pub unsafe extern "system" fn nexus_get_init() -> Option<&'static AddonDefinition> {
    match update_status(NexusLoadStatus::Enumerated, None) {
        Ok(()) | Err(None) => (),
        Err(Some(prev_status)) => {
            match prev_status {
                NexusLoadStatus::Enumerated => (),
                NexusLoadStatus::Loaded | NexusLoadStatus::Loading => curious("GetAddonDef"),
                NexusLoadStatus::Failed | NexusLoadStatus::Unloaded | NexusLoadStatus::Unloading =>
                    if (*ADDON_DEF.get()).api_version != 0 {
                        report_error_ptr(c"unload incomplete, game restart may be required".as_ptr());
                    },
            }
            return Some(&*ADDON_DEF.get())
        },
    }

    let def = &mut *ADDON_DEF.get();
    def.description = ADDON_DESC_C.as_ptr();
    def.provider = UpdateProvider::None;
    def.update_link = ptr::null();
    def.version = {
        let (major, minor, build, ..) = *rt::update::CRATE_SEMVER_PARTS;
        let revision = 0;
        AddonVersion {
            major: major as i16,
            minor: minor as i16,
            build: build as i16,
            revision,
        }
    };

    // TODO: panic::catch_unwind for any of this?

    exports::pre_init();

    if def.author.is_null() || def.author == EMPTY_C.as_ptr() {
        def.author = into_cstring_leak(rt::crate_authors());
    }

    let preferred_host = BootstrapState::read_with(|s| s.addon_host_preference);
    let preferred_host = match preferred_host {
        #[cfg(feature = "extension-arcdps")]
        Some(h @ AddonHostName::ArcDPS) => Err(h),
        None | Some(AddonHostName::Nexus) => Ok(()),
    };
    def.api_version = match preferred_host {
        Err(preferred) => {
            let disabled = format!("disabled, configured for {preferred}");
            log::info!(logger: DeferredLogger::BEST_EFFORT, "GetAddonDef {disabled}");
            def.description = into_cstring_leak(disabled);
            0
        },
        Ok(()) => nexus::AddonApi::VERSION,
    };

    #[allow(unreachable_patterns)]
    BootstrapState::read_with(|s| {
        let channel = match str_opt_ref(&s.update_override_channel) {
            Some(rt::update::CHANNEL_RELEASE_NAME) => Some(None),
            c => c.map(Some),
        };
        def.provider = match channel {
            None => exports::UPDATE_PROVIDER,
            Some(None) | Some(Some(rt::update::CHANNEL_PRERELEASE)) => UpdateProvider::GitHub,
            Some(Some(url)) if url.starts_with("https") => {
                // x.x
                def.update_link = into_cstring_leak(url);
                let is_gh = url
                    .starts_with("https://github.com/")
                    .then(|| url.as_bytes().iter().filter(|&&c| c == b'/').count() == 4)
                    .unwrap_or(false);
                if is_gh {
                    UpdateProvider::GitHub
                } else {
                    UpdateProvider::Direct
                }
            },
            #[cfg(feature = "updates")]
            Some(Some(..)) => UpdateProvider::Manual,
            #[cfg(not(feature = "updates"))]
            Some(Some(..)) => UpdateProvider::Direct,
            _ => UpdateProvider::None,
        };
        #[cfg(all(
            taimi_has = "url-update-base",
            any(not(feature = "updates"), taimi_update = "direct")
        ))]
        if def.update_link.is_null() {
            let version = str_opt_ref(&s.update_override_version);
            let channel = channel.map(|c| c.strip_prefix(rt::update::CHANNEL_DL_PREFIX).unwrap_or(c));
            let direct_url = match def.provider {
                UpdateProvider::Direct => match channel {
                    #[cfg(taimi_has = "url-update-direct")]
                    None if version.is_none() => None,
                    c => Some(rt::update::format_direct_url(c.as_ref().map(|c| &c[..]), version)),
                },
                _ => None,
            };
            if let Some(direct_url) = direct_url {
                def.update_link = into_cstring_leak(direct_url.into());
            }
        }
    });
    if def.update_link.is_null() {
        match def.provider {
            #[cfg(taimi_has = "url-update-direct")]
            UpdateProvider::Direct => def.update_link = update_url_of!(Direct:&CStr).as_ptr(),
            UpdateProvider::GitHub => def.update_link = update_url_of!(GitHub:&CStr).as_ptr(),
            _ => (),
        }
    }

    Some(&*def)
}
static ADDON_DEF: SyncUnsafeCell<AddonDefinition> = SyncUnsafeCell::new(AddonDefinition {
    signature: exports::SIG,
    api_version: nexus::AddonApi::VERSION,
    name: ADDON_TITLE_C.as_ptr(),
    version: NEXUS_VERSION_ZERO,
    description: ADDON_DESC_C.as_ptr(),
    author: EMPTY_C.as_ptr(),
    load: nexus_load,
    unload: Some(nexus_unload),
    flags: exports::FLAGS,
    provider: UpdateProvider::None,
    update_link: ptr::null(),
});
const EMPTY_C: &'static CStr = c"";
const ADDON_DESC_C: &'static CStr = arcffi::cstr!(env!("CARGO_PKG_DESCRIPTION"));
const NEXUS_VERSION_ZERO: AddonVersion = AddonVersion {
    major: 0,
    minor: 0,
    build: 0,
    revision: 0,
};

unsafe extern "C-unwind" fn nexus_load(api: *const AddonApi) {
    let api: Option<&'static AddonApi> = mem::transmute(api);
    let Some(api) = api else {
        log::error!(logger: DeferredLogger::BLOCKING, "load requested without AddonApi");
        return
    };
    let status = match update_status(NexusLoadStatus::Loading, Some(NexusLoadStatus::Enumerated)) {
        Ok(()) => {
            ptr::write_volatile(NEXUS_API.get(), Some(api));
            let log_filter: Option<&'static str> = None;
            let res = panic::catch_unwind(|| {
                #[cfg(feature = "extension-nexus-extern-todo")]
                {
                    nexus::__macro::init(api as *const AddonApi, "TaimiHUD", log_filter);
                    // expect this to have clobbered our hook, meanies :<
                    crate::setup_panic_hook();
                }
                exports::init()
            });
            match res {
                Err(e) => {
                    crate::with_any_error(&e, move |e| {
                        report_error_ptr(into_cstring_leak(e));
                        log::error!(logger: DeferredLogger::BLOCKING, "nexus::init panicked: {e}");
                    });
                    NexusLoadStatus::Failed
                },
                Ok(()) => NexusLoadStatus::Loaded,
            }
        },
        Err(_) => {
            curious("nexus::init");
            return
        },
    };
    let _ = update_status(status, Some(NexusLoadStatus::Loading));
}
unsafe extern "C-unwind" fn nexus_unload() {
    match update_status(NexusLoadStatus::Unloading, Some(NexusLoadStatus::Loaded)) {
        Ok(()) => {
            let res = panic::catch_unwind(|| {
                exports::uninit();
                #[cfg(feature = "extension-nexus-extern-todo")]
                nexus::__macro::deinit();
            });
            let status = match res {
                Err(e) => {
                    crate::with_any_error(&e, move |e| {
                        report_error_ptr(into_cstring_leak(e));
                        log::error!(logger: DeferredLogger::BLOCKING, "nexus::unload panicked: {e}");
                    });
                    NexusLoadStatus::Failed
                },
                Ok(()) => NexusLoadStatus::Unloaded,
            };
            let _ = update_status(status, Some(NexusLoadStatus::Unloading));
        },
        Err(Some(NexusLoadStatus::Failed | NexusLoadStatus::Unloaded)) => {
            let _ = set_status(NexusLoadStatus::Failed);
        },
        Err(..) => (),
    }
    ptr::write_volatile(NEXUS_API.get(), None);
}
static NEXUS_API: SyncUnsafeCell<Option<&'static AddonApi>> = SyncUnsafeCell::new(None);
static NEXUS_LOAD_STATUS: SyncUnsafeCell<Option<NexusLoadStatus>> = SyncUnsafeCell::new(None);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NexusLoadStatus {
    /// seen via GetAddonDef call
    Enumerated,
    Loading,
    Loaded,
    Failed,
    Unloading,
    Unloaded,
}

#[cfg(todo = "unused")]
pub fn addon_api() -> Option<&'static AddonApi> {
    unsafe { *NEXUS_API.get() }
}

/// LEAKS MEMORY!
unsafe fn into_cstring_leak<S: Into<String>>(s: S) -> *const c_char {
    let string = s.into();
    let cstring = CString::from_vec_unchecked(string.into());
    cstring.into_raw() as *const _
}
fn update_status(
    set: NexusLoadStatus,
    expected: Option<NexusLoadStatus>,
) -> Result<(), Option<NexusLoadStatus>> {
    let prev = unsafe { ptr::read_volatile(NEXUS_LOAD_STATUS.get()) };
    match prev {
        s if s == expected => Ok(set_status(set)),
        prev => Err(prev),
    }
}
fn set_status(set: NexusLoadStatus) {
    unsafe { ptr::write_volatile(NEXUS_LOAD_STATUS.get(), Some(set)) }
}
unsafe fn report_error_ptr(e: *const c_char) {
    let def = ADDON_DEF.get();
    ptr::write_volatile(&raw mut (*def).description, e);
    ptr::write_volatile(&raw mut (*def).api_version, 0);
}
fn curious(op: &str) {
    log::debug!(logger: DeferredLogger::BEST_EFFORT, "redundant {op}, curious");
}
