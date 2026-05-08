use {
    crate::{
        exports::{
            arcdps as exports,
            runtime::{
                self as rt,
                imgui::{self, sys as imgui_sys, Ui},
            },
        },
        settings::state::AddonHostName,
    },
    arcffi::cstr::{cstr, CStrPtr},
    core::{
        mem,
        num::NonZero,
        ptr::{self, NonNull},
    },
    dpsapi::api::header::{
        arc_export,
        c_bool32,
        CombatArgs,
        ExtensionExports,
        ExtensionFnCombat,
        ExtensionHeader,
        ExtensionLoadResult,
        ID3d,
        ImCtx,
        InitArgs,
        InitFn,
        ReleaseFn,
        HWND,
        LPARAM,
        WPARAM,
    },
    std::panic,
    sync_unsafe_cell::SyncUnsafeCell,
};

pub const ARC_SIG: NonZero<u32> = unsafe { NonZero::new_unchecked(exports::SIG) };
static ARC_ARGS: SyncUnsafeCell<InitArgs> = SyncUnsafeCell::new(InitArgs::EMPTY);
static ARC_EXPORT: SyncUnsafeCell<ExtensionExports<'static>> = SyncUnsafeCell::new(ExtensionExports::EMPTY);
static ARC_IMGUI_CONTEXT: SyncUnsafeCell<Option<NonZero<usize>>> = SyncUnsafeCell::new(None);
static ARC_IMGUI_UI: SyncUnsafeCell<Option<NonZero<usize>>> = SyncUnsafeCell::new(None);
//pub const ARC_CB_COMBAT: ExtensionFnCombat = ExtensionExports::wrap_combat_fn_item(&arc_cb_combat);
pub const ARC_CB_COMBAT_LOCAL: ExtensionFnCombat =
    ExtensionExports::wrap_combat_fn_item(&arc_cb_combat_local);
pub const ARC_BUILD: CStrPtr<'static> = cstr!(&env!("CARGO_PKG_VERSION"));
pub const ARC_NAME: CStrPtr<'static> = CStrPtr::with_cstr(crate::exports::ADDON_TITLE_C);
fn arc_imgui_version() -> u32 {
    let fallback = match () {
        #[cfg(feature = "extension-nexus")]
        _ => nexus::gui::IMGUI_VERSION,
        #[cfg(not(feature = "extension-nexus"))]
        _ => ImCtx::VERSION_20210202.get(),
    };
    arc_args()
        .and_then(|arc| arc.imgui.version())
        .map(|v| v.get())
        .unwrap_or(fallback)
}
fn exported_imgui_version() -> &'static u32 {
    let header = unsafe { &(&*ARC_EXPORT.get()).header };
    header.imgui_version()
}

pub fn arc_args() -> Option<&'static InitArgs> {
    let args = unsafe { &*ARC_ARGS.get() };
    match args.module.module().is_invalid() {
        true => None,
        false => Some(args),
    }
}

unsafe fn arc_imgui_context() -> Option<&'static imgui::Context> {
    let context_global = *ARC_IMGUI_CONTEXT.get();
    let context = match context_global {
        Some(context_global) => {
            let ptr = context_global.get() as *mut imgui::Context;
            return Some(&*ptr)
        },
        None => {
            let _context_sys = imgui_bind_context()?;
            Box::new(imgui::Context::current())
        },
    };
    let context = Box::into_raw(context);
    ptr::write(ARC_IMGUI_CONTEXT.get() as *mut usize, context as usize);
    Some(&*context)
}
pub(crate) fn arc_imgui_context_ptr() -> Option<NonNull<imgui_sys::ImGuiContext>> {
    let arc = arc_args()?;
    let imgui_version = arc.imgui.version().or(NonZero::new(*exported_imgui_version()));
    match imgui_version {
        Some(ImCtx::VERSION_20210202) => arc.imgui.ptr().map(NonNull::cast),
        _ => None,
    }
}

pub unsafe fn arc_imgui_ui<'u>() -> Option<&'u Ui<'static>> {
    if !super::loaded() {
        return None
    }
    let ui_global = unsafe { *ARC_IMGUI_UI.get() };
    if let Some(ui_global) = ui_global {
        imgui_check_context()?;
        return Some(&*(ui_global.get() as *const Ui<'static>))
    }
    match arc_imgui_context() {
        Some(context) => Some(unsafe {
            let ui = Box::new(Ui::from_ctx(context));
            let ui = Box::into_raw(ui);
            ptr::write(ARC_IMGUI_UI.get() as *mut usize, ui as usize);
            &*ui
        }),
        None => None,
    }
}

pub unsafe fn imgui_context_cleanup() {
    let context = unsafe { mem::replace(&mut *ARC_IMGUI_CONTEXT.get(), None) };
    let Some(context) = context else { return };

    imgui_frame_cleanup();

    let context = context.get() as *mut mem::ManuallyDrop<imgui::Context>;
    // we do not want this to clean up the context shared by other addons!
    let context = Box::from_raw(context);
    drop(context);
}
pub unsafe fn imgui_frame_cleanup() {
    let ui_global = unsafe { mem::replace(&mut *ARC_IMGUI_UI.get(), None) };
    let ui = match ui_global {
        Some(ui) => ui,
        None => return,
    };
    ptr::write(ARC_IMGUI_UI.get(), None);
    let ptr = ui.get() as *mut mem::ManuallyDrop<Ui<'static>>;
    drop(Box::from_raw(ptr));
}
pub unsafe fn imgui_bind_context() -> Option<NonNull<imgui_sys::ImGuiContext>> {
    let arc = arc_args()?;
    let imgui_version = arc.imgui.version().or(NonZero::new(*exported_imgui_version()));
    let context_sys = match arc.imgui.version() {
        Some(ImCtx::VERSION_20210202) => arc.imgui.ptr().map(NonNull::cast::<imgui_sys::ImGuiContext>)?,
        _ => return None,
    };
    imgui_sys::igSetCurrentContext(context_sys.as_ptr());
    imgui_sys::igSetAllocatorFunctions(
        arc.imgui.user_malloc().malloc,
        arc.imgui.user_malloc().free,
        arc.imgui.user_malloc().userdata_ptr(),
    );
    Some(context_sys)
}
pub unsafe fn new_imgui_frame() {
    #[cfg(todo = "unnecessary")]
    imgui_frame_cleanup();
}
pub unsafe fn imgui_check_context() -> Option<()> {
    let Some(target) = arc_imgui_context_ptr() else { return None };
    let current = imgui_sys::igGetCurrentContext();
    if target.as_ptr() != current {
        imgui_bind_context();
    }
    Some(())
}

pub unsafe fn with_imgui<R, F: FnOnce(&'_ Ui<'_>) -> R>(f: F) -> Option<R> {
    match arc_imgui_ui() {
        Some(ui) => Some(f(ui)),
        None => None,
    }
}

fn arc_get_init(args: InitArgs) -> Option<InitFn> {
    unsafe {
        ptr::write(ARC_ARGS.get(), args);
    }
    exports::pre_init();

    match AddonHostName::ArcDPS.is_preferred_host() {
        Ok(()) => (),
        Err(pref) => {
            log::info!("ignoring arcdps, {pref} is preferred");
            if pref == AddonHostName::Nexus && pref.is_detected() != Some(false) {
                // arcdps doesn't report this as an error when it's a potential addonapi module?
                exports::disable_load();
                return None
            }
            // ... otherwise continue on so we can produce a less confusing error message
        },
    }

    Some(arc_init)
}

//fn arc_cb_combat(args: CombatArgs) {}

fn arc_cb_combat_local(args: CombatArgs) {
    exports::combat_local(args)
}

unsafe extern "C" fn arc_cb_wnd(wnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> u32 {
    exports::wnd(wnd.into(), msg, w.into(), l.into())
}

unsafe extern "C" fn arc_cb_wnd_filter(wnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> u32 {
    exports::wnd_filter(wnd.into(), msg, w.into(), l.into())
}

unsafe extern "C" fn arc_cb_imgui(not_charsel_or_loading: c_bool32, hide_if_combat_or_ooc: c_bool32) {
    new_imgui_frame();

    with_imgui(|ui| exports::imgui(ui, not_charsel_or_loading.into(), hide_if_combat_or_ooc.value));
}

unsafe extern "C" fn arc_cb_imgui_options_tab() {
    with_imgui(|ui| exports::imgui_options_tab(ui));
}

unsafe extern "C" fn arc_cb_imgui_options_windows(window_name: Option<CStrPtr>) -> c_bool32 {
    with_imgui(|ui| {
        let window = window_name.as_ref().map(|w| w.to_string_lossy());
        let window = window.as_ref().map(|w| &w[..]);
        exports::imgui_options_windows(ui, window)
    })
    .map(Into::into)
    .unwrap_or(c_bool32::FALSE)
}

extern "C" fn arc_init() -> Option<NonNull<ExtensionExports<'static>>> {
    exports::pre_init();

    let res = panic::catch_unwind(|| Some(exports::init()));
    let exports = ExtensionExports {
        name: Some(ARC_NAME),
        build: Some(ARC_BUILD),
        cb_wnd: Some(arc_cb_wnd),
        cb_wnd_filter: Some(arc_cb_wnd_filter),
        cb_combat: None,
        cb_combat_local: Some(ARC_CB_COMBAT_LOCAL),
        cb_ui_imgui: Some(arc_cb_imgui),
        cb_ui_options_tab: Some(arc_cb_imgui_options_tab),
        cb_ui_options_windows: Some(arc_cb_imgui_options_windows),
        header: match res {
            Ok(Some(Ok(()))) =>
                ExtensionHeader::new_loaded(ARC_SIG, ExtensionExports::SIZE, arc_imgui_version()),
            Ok(Some(Err(e))) => {
                // TODO
                ::log::error!("Failed initialization: {e}");
                exports::disable_load();
                let message = c"init failed";
                ExtensionHeader::new_failed(Some(message.into()))
            },
            Ok(None) => ExtensionHeader::new_failed(None),
            Err(e) => {
                crate::log_any_error("arcdps init", &e);
                exports::disable_load();
                ExtensionHeader::new_failed(Some(c"init panic".into()))
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

    //ptr::write(ARC_ARGS.get(), InitArgs::EMPTY);
    ptr::write(ARC_EXPORT.get(), ExtensionExports::EMPTY);
    // XXX: leaking these buffers because the destructors call imgui APIs :<
    ptr::write(ARC_IMGUI_CONTEXT.get(), None);
    ptr::write(ARC_IMGUI_UI.get(), None);
}

arc_export! {
    #[naked]
    unsafe extern fn get_init_addr() => arc_get_init;
    unsafe extern fn get_release_addr(reason) => arc_get_release_addr;
    extern fn get_update_url() => exports::get_update_url;
}

#[inline(always)]
fn arc_get_release_addr(_reason: ExtensionLoadResult) -> Option<ReleaseFn> {
    Some(arc_release)
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn dxgi_swap_chain() -> Option<&'static rt::SwapChain> {
    let sc = arc_args().and_then(|arc| match arc.id3d.version() {
        Some(ID3d::VERSION_DX9) => None,
        Some(ID3d::VERSION_DX11) | Some(..) => arc.id3d.ptr_ref().as_ref(),
        _ => None,
    });
    sc.map(|sc| unsafe { rt::SwapChain::from_d3d_raw_ref(sc) })
}

#[no_mangle]
#[cfg(feature = "extension-arcdps-extras")]
pub unsafe extern "C" fn arcdps_unofficial_extras_subscriber_init(info: usize, subscriber: usize) {
    exports::unofficial_extras::extras_init_raw(info as *const _, subscriber as *mut _)
}
