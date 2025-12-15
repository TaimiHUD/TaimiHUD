use {
    crate::{
        built_info,
        exports::{
            nexus as exports,
            runtime::{
                self as rt,
                imgui::{self, sys as imgui_sys},
            },
            update_url_of,
        },
    },
    nexus::{AddonFlags, UpdateProvider},
    std::ptr::NonNull,
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

unsafe fn imgui_bind_context() -> Option<NonNull<imgui_sys::ImGuiContext>> {
    let aapi = addon_api()?;
    let ctx = ptr::NonNull::new(aapi.imgui_context)?;
    if imgui_sys::igGetCurrentContext() != ctx.as_ptr() as *mut _ {
        imgui_sys::igSetCurrentContext(ctx.as_ptr());
        imgui_sys::igSetAllocatorFunctions(aapi.imgui_malloc, aapi.imgui_free, ptr::null_mut());
    }

    Some(ctx)
}

pub(super) unsafe fn new_imgui_frame() {}
unsafe fn imgui_ui<'a, 'u>() -> Option<&'a imgui::Ui<'u>> {
    imgui_bind_context()?;
    Some(nexus::ui())
}
pub unsafe fn with_ui<'u, R, F: FnOnce(&imgui::Ui<'u>) -> R>(f: F) -> Option<R> {
    imgui_ui().map(f)
}
