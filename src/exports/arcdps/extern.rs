use core::{
    num::{NonZeroU32, NonZeroUsize},
    mem::{ManuallyDrop, transmute},
    ptr::{self, NonNull},
};
use arcffi::cstr::{cstr, CStrPtr};
use dpsapi::api::header::{
    c_bool32, wrap_init_addr,
    HWND, LPARAM, WPARAM,
    GetInitFn,
    ExtensionExports, ExtensionHeader,
    ExtensionFnCombat,
    InitArgs, CombatArgs,
    InitFn,
    ReleaseFn,
};
use crate::exports::{
    arcdps as exports,
    runtime::imgui::{self, sys::{self as imgui_sys, ImGuiContext}, Ui},
};
use sync_unsafe_cell::SyncUnsafeCell;

pub const ARC_SIG: NonZeroU32 = unsafe {
    NonZeroU32::new_unchecked(exports::SIG)
};
static ARC_ARGS: SyncUnsafeCell<InitArgs> = SyncUnsafeCell::new(InitArgs::EMPTY);
static ARC_EXPORT: SyncUnsafeCell<ExtensionExports<'static>> = SyncUnsafeCell::new(ExtensionExports::EMPTY);
static ARC_IMGUI_CONTEXT_RAW: SyncUnsafeCell<Option<NonZeroUsize>> = SyncUnsafeCell::new(None);
#[cfg(todo)]
static ARC_IMGUI_CONTEXT: SyncUnsafeCell<Option<imgui::Context>> = SyncUnsafeCell::new(None);
//pub const ARC_CB_COMBAT: ExtensionFnCombat = ExtensionExports::wrap_combat_fn_item(&arc_cb_combat);
pub const ARC_CB_COMBAT_LOCAL: ExtensionFnCombat = ExtensionExports::wrap_combat_fn_item(&arc_cb_combat_local);
pub const ARC_BUILD: CStrPtr<'static> = cstr!(&env!("CARGO_PKG_VERSION"));
pub const ARC_NAME: CStrPtr<'static> = cstr!(&concat!("arcloader-", env!("CARGO_CRATE_NAME")));
#[cfg(feature = "extension-nexus")]
pub const ARC_IMGUI_VERSION: u32 = nexus::gui::IMGUI_VERSION;
#[cfg(not(feature = "extension-nexus"))]
pub const ARC_IMGUI_VERSION: u32 = ExtensionHeader::IMGUI_VERSION_20210202;

pub fn arc_args() -> Option<&'static InitArgs> {
    let args = unsafe {
        &*ARC_ARGS.get()
    };
    match args.module.module().is_invalid() {
        true => None,
        false => Some(args),
    }
}

pub fn arc_imgui_context_raw() -> Option<NonNull<ImGuiContext>> {
    unsafe {
        let c = *ARC_IMGUI_CONTEXT_RAW.get();
        transmute(c)
    }
}

// TODO: stash away somewhere so this can return &'static?
pub fn arc_imgui_context() -> Option<imgui::Context> {
    let context = arc_imgui_context_raw();
    match (context, arc_args()) {
        (Some(context), Some(arc)) => Some(unsafe {
            imgui_sys::igSetCurrentContext(context.as_ptr());
            imgui_sys::igSetAllocatorFunctions(arc.malloc, arc.free, InitArgs::ALLOC_USER_DATA);
            imgui::Context::current()
        }),
        _ => None,
    }
}
#[cfg(todo)]
pub unsafe fn arc_imgui_ui<'u>() -> Option<ManuallyDrop<Ui<'u>>> {
    match arc_imgui_context() {
        Some(context) => Some(unsafe {
            let ui = Ui::from_ctx(context);
            ManuallyDrop::new(ui)
        }),
        None => None,
    }
}

pub fn with_imgui<R, F: FnOnce(&'_ Ui<'_>) -> R>(f: F) -> Option<R> {
    match arc_imgui_context() {
        Some(context) => Some({
            // TODO: what do either of these even do on drop anyway?
            let ui = Ui::from_ctx(&context);
            let ui = ManuallyDrop::new(ui);
            f(&ui)
        }),
        None => None,
    }
}

fn arc_get_init(args: InitArgs) -> Option<InitFn> {
    unsafe {
        ptr::write(ARC_ARGS.get(), args);
    }

    Some(arc_init)
}

//fn arc_cb_combat(args: CombatArgs) {}

fn arc_cb_combat_local(args: CombatArgs) {
}

//unsafe extern "C" fn arc_cb_wnd(wnd: HWND, msg: u32, w: WPARAM, l: LPARAM) {}

unsafe extern "C" fn arc_cb_wnd_filter(wnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> u32 {
    exports::wnd_filter(wnd.into(), msg, w.into(), l.into())
}

unsafe extern "C" fn arc_cb_imgui(not_charsel_or_loading: c_bool32, hide_if_combat_or_ooc: c_bool32) {
    with_imgui(|ui|
        exports::imgui(ui, not_charsel_or_loading.into(), hide_if_combat_or_ooc.value)
    );
}

unsafe extern "C" fn arc_cb_imgui_options_tab() {
    with_imgui(|ui|
        exports::imgui_options_tab(ui)
    );
}

unsafe extern "C" fn arc_cb_imgui_options_windows(window_name: Option<CStrPtr>) -> c_bool32 {
    with_imgui(|ui| {
        let window = window_name.as_ref().map(|w| w.to_string_lossy());
        let window = window.as_ref().map(|w| &w[..]);
        exports::imgui_options_windows(ui, window)
    }).map(Into::into).unwrap_or(c_bool32::FALSE)
}

extern "C" fn arc_init() -> Option<NonNull<ExtensionExports<'static>>> {
    let res = Some(exports::init());
    let exports = ExtensionExports {
        name: Some(ARC_NAME),
        build: Some(ARC_BUILD),
        cb_wnd: None,
        cb_wnd_filter: Some(arc_cb_wnd_filter),
        cb_combat: None,
        cb_combat_local: Some(ARC_CB_COMBAT_LOCAL),
        cb_ui_imgui: Some(arc_cb_imgui),
        cb_ui_options_tab: Some(arc_cb_imgui_options_tab),
        cb_ui_options_windows: Some(arc_cb_imgui_options_windows),
        header: match res {
            Some(Ok(())) => ExtensionHeader::new_loaded(ARC_SIG, ExtensionExports::SIZE, ARC_IMGUI_VERSION),
            Some(Err(e)) => {
                // TODO
                let message = cstr!(&"init failed");
                ExtensionHeader::new_failed(Some(message))
            },
            None => {
                ExtensionHeader::new_failed(None)
            },
        },
    };
    let export = unsafe {
        let export = ARC_EXPORT.get();
        ptr::write(export, exports);
        NonNull::new_unchecked(export)
    };
    Some(export)
}

unsafe extern "C" fn arc_release() {
    exports::release();

    ptr::write(ARC_ARGS.get(), InitArgs::EMPTY);
    ptr::write(ARC_EXPORT.get(), ExtensionExports::EMPTY);
    ptr::write(ARC_IMGUI_CONTEXT_RAW.get(), None);
}

wrap_init_addr! {
    unsafe extern fn get_init_addr() => arc_get_init;
}

#[no_mangle]
pub unsafe extern "system" fn get_release_addr() -> Option<ReleaseFn> {
    Some(arc_release)
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn dxgi_swap_chain() -> Option<windows::core::InterfaceRef<'static, windows::Win32::Graphics::Dxgi::IDXGISwapChain>> {
    let sc = arc_args().and_then(|arc| match arc.d3d_version {
        0..=9 => None,
        _ => arc.id3d,
    });
    sc.map(|sc| unsafe {
        windows::core::InterfaceRef::from_raw(sc)
    })
}
