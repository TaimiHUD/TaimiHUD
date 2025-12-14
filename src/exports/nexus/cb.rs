use {
    crate::{
        built_info,
        exports::{nexus as exports, runtime as rt, update_url_of},
    },
    nexus::{AddonFlags, UpdateProvider},
};

pub use crate::exports::nexus::{FLAGS, SIG, UPDATE_PROVIDER};

pub(crate) fn load() {
    exports::pre_init();
    exports::init()
}

pub(crate) fn unload() {
    exports::uninit()
}

#[cfg(taimi_has = "update-github")]
macro_rules! update_url {
    () => { update_url_of! { GitHub: &str } };
}
#[cfg(taimi_has = "update-direct")]
macro_rules! update_url {
    () => { update_url_of! { Direct: &str } };
}
#[cfg(not(any(taimi_has = "update-github", taimi_has = "update-direct")))]
macro_rules! update_url {
    () => { "" };
}
pub(crate) use update_url;

pub fn addon_api() -> Option<&'static AddonApi> {
    if !exports::loaded() {
        return None
    }
    nexus::addon_api()
}

/// TODO: same logic as extern...
pub(super) unsafe fn new_imgui_frame() {
    #[cfg(feature = "extension-arcdps")]
    match (addon_api(), crate::exports::arcdps::imgui_context_ptr()) {
        (Some(aapi), Some(ctx)) if aapi.imgui_context == ctx.as_ptr() => {
            // they're linked, no need to switch
        },
        (_, None) => {
            // TODO: unloading may make this an outdated comparison?
        },
        _ if !rt::arcdps_available() => (),
        (None, _) => (),
        (Some(aapi), ..) if aapi.imgui_context.is_null() => (),
        (Some(aapi), ..) => {
            rt::imgui::sys::igSetCurrentContext(aapi.imgui_context);
            rt::imgui::sys::igSetAllocatorFunctions(aapi.imgui_malloc, aapi.imgui_free, ptr::null_mut());
        },
    }
}

pub unsafe fn imgui_ui<'a, 'u>() -> Option<&'a rt::imgui::Ui<'u>> {
    match exports::loaded() {
        true => Some(nexus::ui()),
        false => None,
    }
}
