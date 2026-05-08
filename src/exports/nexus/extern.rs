use {
    super::is_provider_nexus,
    crate::{
        built_info,
        exports::{
            nexus as exports,
            runtime::{
                self as rt,
                imgui::{self, sys as imgui_sys},
                log::DeferredLogger,
            },
            update_url_of,
            ADDON_TITLE_C,
        },
        settings::state::{AddonHostName, BootstrapState},
    },
    core::{
        ffi::c_char,
        mem,
        ptr::{self, NonNull},
    },
    nexus::{
        addon::{AddonDefinition, AddonFlags, AddonVersion, UpdateProvider},
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
    let prev_status = update_status(NexusLoadStatus::Enumerated, None);
    match prev_status {
        Ok(()) | Err(None) => (),
        Err(Some(NexusLoadStatus::Unloaded)) => {
            // be careful...
        },
        Err(Some(prev_status)) => {
            match prev_status {
                NexusLoadStatus::Enumerated => (),
                NexusLoadStatus::Loaded | NexusLoadStatus::Loading => curious("GetAddonDef"),
                NexusLoadStatus::Failed | NexusLoadStatus::Unloading | NexusLoadStatus::Unloaded => {
                    // Unloaded *might* be fine as long as nexus isn't responsible for updates
                    if !is_disabled() {
                        report_error_ptr(ERROR_INCOMPLETE_UNLOAD.as_ptr());
                    }
                },
            }
            return Some(&*ADDON_DEF.get())
        },
    }

    let mut provider = UPDATE_NONE;
    let mut update_link = ptr::null();
    let author;
    {
        let def = &mut *ADDON_DEF.get();
        author = def.author;
        def.provider = provider.clone();
        def.update_link = update_link;
        def.flags = exports::FLAGS;
        def.api_version = nexus::AddonApi::VERSION;
        def.description = ADDON_DESC_C.as_ptr();
        def.version = rt::update::CRATE_ADDONAPI_VERSION.clone();
    }

    // TODO: panic::catch_unwind for any of this?

    let def = ADDON_DEF.get();
    exports::pre_init();

    if author.is_null() || author == EMPTY_C.as_ptr() {
        ptr::write_volatile(&raw mut (*def).author, into_cstring_leak(rt::crate_authors()));
    }

    match AddonHostName::Nexus.is_preferred_host() {
        Err(preferred) => {
            let disabled_msg = format!("disabled, configured for {preferred} via boot.json");
            log::info!(logger: DeferredLogger::BEST_EFFORT, "GetAddonDef {disabled_msg}");
            ptr::write_volatile(&raw mut (*def).description, into_cstring_leak(disabled_msg));
            #[cfg(todo = "unnecessary")]
            {
                def.flags |= AddonFlags::OnlyLoadDuringGameLaunchSequence;
                def.api_version = 0;
            }
        },
        Ok(()) => (),
    }

    let update_allowed = match AddonHostName::Nexus.is_preferred_update_host() {
        Ok(()) => true,
        Err(_host) => false,
    };
    #[allow(unreachable_patterns)]
    BootstrapState::read_with(|s| {
        let channel = match str_opt_ref(&s.update_override_channel) {
            Some(rt::update::CHANNEL_RELEASE_NAME) => Some(None),
            c => c.map(Some),
        };
        provider = match channel {
            _ if !update_allowed => UpdateProvider::Manual,
            None => exports::UPDATE_PROVIDER,
            Some(None) | Some(Some(rt::update::CHANNEL_PRERELEASE)) => UpdateProvider::GitHub,
            Some(Some(url)) if url.starts_with("https") => {
                // x.x
                update_link = into_cstring_leak(url);
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
        if update_link.is_null() {
            let version = str_opt_ref(&s.update_override_version);
            let channel = channel.map(|c| c.strip_prefix(rt::update::CHANNEL_DL_PREFIX).unwrap_or(c));
            let direct_url = match provider {
                UpdateProvider::Direct => match channel {
                    #[cfg(taimi_has = "url-update-direct")]
                    None if version.is_none() => None,
                    c => Some(rt::update::format_direct_url(c.as_ref().map(|c| &c[..]), version)),
                },
                _ => None,
            };
            if let Some(direct_url) = direct_url {
                update_link = into_cstring_leak(direct_url.into());
            }
        }
    });
    if update_link.is_null() {
        match provider {
            #[cfg(taimi_has = "url-update-direct")]
            UpdateProvider::Direct => update_link = update_url_of!(Direct:&CStr).as_ptr(),
            UpdateProvider::GitHub => update_link = update_url_of!(GitHub:&CStr).as_ptr(),
            _ => (),
        }
    }
    if matches!(prev_status, Err(Some(NexusLoadStatus::Unloaded))) {
        if is_provider_nexus(&provider) {
            provider = UPDATE_NONE;
            update_link = ptr::null();
            if !is_disabled() {
                report_error_ptr(ERROR_INCOMPLETE_UNLOAD.as_ptr());
            }
        }
    }
    ptr::write_volatile(&raw mut (*def).provider, provider);
    ptr::write_volatile(&raw mut (*def).update_link, update_link);

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
const ADDON_DESC_C: &'static CStr = match built_info::IS_TAGGED_RELEASE {
    false if built_info::IS_TAGGED_RELEASE_OR_RC => ADDON_DESC_RC,
    _ => ADDON_DESC_EN,
};
const ADDON_DESC_EN: &'static CStr = c"Pathing, encounter timers and markers";
const ADDON_DESC_RC: &'static CStr = c"Give us your feedback via GitHub or Discord! Disable pre-releases if you need to opt out of beta-testing the newest features";
const ERROR_INCOMPLETE_UNLOAD: &'static CStr = c"unload incomplete, game restart may be required";
const NEXUS_VERSION_ZERO: AddonVersion = AddonVersion {
    major: 0,
    minor: 0,
    build: 0,
    revision: 0,
};
/// [UpdateProvider::None] actually means "auto-detect", which is usually not
/// what we want
const UPDATE_NONE: UpdateProvider = UpdateProvider::Manual;

pub fn is_enumerated() -> bool {
    let version = unsafe { &(&*ADDON_DEF.get()).version };
    !matches!(version, AddonVersion { major: 0, minor: 0, .. })
}
pub fn is_disabled() -> bool {
    // requested_api() == 0
    let (desc, api) = unsafe {
        let def = &*ADDON_DEF.get();
        (def.description, def.api_version)
    };
    desc != ADDON_DESC_C.as_ptr() || api == 0
}
pub fn requested_api() -> i32 {
    unsafe { (&*ADDON_DEF.get()).api_version }
}

pub fn requested_provider() -> UpdateProvider {
    unsafe { (&*ADDON_DEF.get()).provider }
}

unsafe extern "C-unwind" fn nexus_load(api: *const AddonApi) {
    let api: Option<&'static AddonApi> = mem::transmute(api);
    let disabled = is_disabled();
    if api.is_none() && !disabled {
        log::error!(logger: DeferredLogger::BEST_EFFORT, "load requested without AddonApi");
        report_error_ptr(c"unexpected APIv0".as_ptr());
        set_status(NexusLoadStatus::Failed);
        return
    }
    let status = match update_status(NexusLoadStatus::Loading, Some(NexusLoadStatus::Enumerated)) {
        Ok(()) | Err(Some(NexusLoadStatus::Unloaded)) => {
            let nexus_api = NEXUS_API.get();
            let prev_api = ptr::read_volatile(nexus_api);
            #[cfg(feature = "extension-nexus-extern-todo")]
            let prev_init = *NEXUS_API_INIT.get();
            let api = match (api, prev_api) {
                (Some(api), _) => {
                    ptr::write_volatile(nexus_api, Some(api));
                    Some(api)
                },
                (None, Some(prev)) => Some(prev),
                (api, _) => api,
            };
            let log_filter: Option<&'static str> = None;
            let res = panic::catch_unwind(|| {
                match api {
                    #[cfg(feature = "extension-nexus-extern-todo")]
                    Some(api) if /* !disabled &&*/ !prev_init => {
                        ptr::write_volatile(NEXUS_API_INIT.get(), true);
                        nexus::__macro::init(api as *const AddonApi, "TaimiHUD", log_filter);
                        // expect this to have clobbered our hook, meanies :<
                        crate::setup_panic_hook();
                    },
                    _ => (),
                }
                if !disabled && prev_init {
                    // TODO: if we implement auto-updates, disable them since something is subtly wrong with dll refcount
                    let incomplete = str::from_utf8_unchecked(ERROR_INCOMPLETE_UNLOAD.to_bytes());
                    log::warn!("nexus {incomplete}");
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
#[cfg(feature = "extension-nexus-extern-todo")]
static NEXUS_API_INIT: SyncUnsafeCell<bool> = SyncUnsafeCell::new(false);
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

#[inline]
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
    ptr::write_volatile(
        &raw mut (*def).flags,
        exports::FLAGS | AddonFlags::OnlyLoadDuringGameLaunchSequence,
    );
}
fn curious(op: &str) {
    log::debug!(logger: DeferredLogger::BEST_EFFORT, "redundant {op}, curious");
}

unsafe fn imgui_bind_context() -> Option<NonNull<imgui_sys::ImGuiContext>> {
    let aapi = addon_api()?;
    let ctx = ptr::NonNull::new(aapi.imgui_context)?.cast::<imgui_sys::ImGuiContext>();
    if imgui_sys::igGetCurrentContext() != ctx.as_ptr() as *mut _ {
        imgui_sys::igSetCurrentContext(ctx.as_ptr());
        imgui_sys::igSetAllocatorFunctions(aapi.imgui_malloc, aapi.imgui_free, ptr::null_mut());
    }

    Some(ctx)
}

#[cfg(feature = "extension-nexus-extern-todo")]
pub unsafe fn with_ui<'u, R, F: FnOnce(&imgui::Ui<'u>) -> R>(f: F) -> Option<R> {
    imgui_bind_context()?;
    Some(nexus::__macro::with_ui(|ui| {
        let ui = mem::transmute::<&imgui::Ui<'static>, &imgui::Ui<'u>>(ui);
        f(ui)
    }))
}
