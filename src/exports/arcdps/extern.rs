use core::{
    num::{NonZeroU32, NonZeroUsize},
    mem::{self, ManuallyDrop, transmute},
    ptr::{self, NonNull},
    slice,
};
use std::panic;
use arcffi::cstr::{cstr, CStrPtr, CStrPtr16};
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
    runtime::{
        self as rt,
        imgui::{self, sys::{self as imgui_sys, ImGuiContext}, Ui},
    },
};
use sync_unsafe_cell::SyncUnsafeCell;

pub const ARC_SIG: NonZeroU32 = unsafe {
    NonZeroU32::new_unchecked(exports::SIG)
};
static ARC_ARGS: SyncUnsafeCell<InitArgs> = SyncUnsafeCell::new(InitArgs::EMPTY);
static ARC_EXPORT: SyncUnsafeCell<ExtensionExports<'static>> = SyncUnsafeCell::new(ExtensionExports::EMPTY);
static ARC_IMGUI_CONTEXT_RAW: SyncUnsafeCell<Option<NonZeroUsize>> = SyncUnsafeCell::new(None);
static ARC_IMGUI_CONTEXT: SyncUnsafeCell<Option<NonZeroUsize>> = SyncUnsafeCell::new(None);
static ARC_IMGUI_UI: SyncUnsafeCell<Option<NonZeroUsize>> = SyncUnsafeCell::new(None);
//pub const ARC_CB_COMBAT: ExtensionFnCombat = ExtensionExports::wrap_combat_fn_item(&arc_cb_combat);
pub const ARC_CB_COMBAT_LOCAL: ExtensionFnCombat = ExtensionExports::wrap_combat_fn_item(&arc_cb_combat_local);
pub const ARC_BUILD: CStrPtr<'static> = cstr!(&env!("CARGO_PKG_VERSION"));
pub const ARC_NAME: CStrPtr<'static> = CStrPtr::with_cstr(rt::NAME_C);
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

pub unsafe fn arc_imgui_context_raw(arc: &InitArgs) -> Option<NonNull<ImGuiContext>> {
    unsafe {
        
        let c = *ARC_IMGUI_CONTEXT_RAW.get();
        transmute(c)
    }
}

pub unsafe fn arc_imgui_context() -> Option<&'static imgui::Context> {
    let context_global = unsafe {
        *ARC_IMGUI_CONTEXT.get()
    };
    let context = match context_global {
        Some(context_global) => {
            let ptr = context_global.get() as *mut imgui::Context;
            return Some(&*ptr)
        },
        None => {
            let arc = arc_args()?;
            let context_sys = arc.imgui_ctx
                .map(NonNull::cast::<imgui_sys::ImGuiContext>)?;
            unsafe {
                imgui_sys::igSetCurrentContext(context_sys.as_ptr());
                imgui_sys::igSetAllocatorFunctions(arc.malloc, arc.free, InitArgs::ALLOC_USER_DATA);
                Box::new(imgui::Context::current())
            }
        },
    };
    let context = Box::into_raw(context);
    Some(unsafe {
        ptr::write(ARC_IMGUI_CONTEXT.get() as *mut usize, context as usize);
        &*context
    })
}

#[cfg(feature = "extension-arcdps-extern-cleanup")]
const DEFAULT_BUFFER_CAP: usize = 1024;
#[cfg(feature = "extension-arcdps-extern-cleanup")]
fn find_buffer_offset<T>(ui: *const T, ignore_context: usize) -> Result<usize, &'static str> {
    let res = unsafe {
        let szs = mem::size_of::<T>() / mem::size_of::<usize>();
        let ptrs = slice::from_raw_parts(ui as *const usize, szs);
        let mut p = ptrs.iter().copied().enumerate()
            .filter(|&(_, p)| p != 0 && p != DEFAULT_BUFFER_CAP && p != ignore_context);
        let found = p.next();
        let res = found.ok_or("no ptr found");
        if res.is_ok() && p.next().is_some() {
            return Err("multiple ptrs found")
        }
        res.map(|(i, _p)| i)
    };
    #[cfg(debug_assertions)]
    if let Ok(off) = res {
        let buffer = unsafe {
            &*((&*ui as *const T as *const usize).add(off) as *const Vec<u8>)
        };
        assert_eq!(buffer.capacity(), DEFAULT_BUFFER_CAP);
    }

    res
}

#[cfg(feature = "extension-arcdps-extern-cleanup")]
static ARC_IMGUI_BUFFER_OFFSET: SyncUnsafeCell<Option<usize>> = SyncUnsafeCell::new(None);
pub unsafe fn arc_imgui_ui<'u>() -> Option<&'u Ui<'static>> {
    let ui_global = unsafe {
        *ARC_IMGUI_UI.get()
    };
    if let Some(ui_global) = ui_global {
        return Some(&*(ui_global.get() as *const Ui<'static>))
    }
    match arc_imgui_context() {
        Some(context) => Some(unsafe {
            let ui = Box::new(Ui::from_ctx(context));
            #[cfg(feature = "extension-arcdps-extern-cleanup")]
            if (*ARC_IMGUI_BUFFER_OFFSET.get()).is_none() {
                match find_buffer_offset::<Ui>(&*ui, context as *const imgui::Context as usize) {
                    Ok(off) => {
                        ptr::write(ARC_IMGUI_BUFFER_OFFSET.get(), Some(off));
                    },
                    Err(e) => {
                        log::error!("could not find imgui buffer offset, this should not happen! {e}");
                    },
                }
            }
            let ui = Box::into_raw(ui);
            ptr::write(ARC_IMGUI_UI.get() as *mut usize, ui as usize);
            &*ui
        }),
        None => None,
    }
}

pub unsafe fn new_imgui_frame() {
    let ui_global = unsafe {
        mem::replace(&mut *ARC_IMGUI_UI.get(), None)
    };
    let ui = match ui_global {
        Some(ui) => ui,
        None => return,
    };
    ptr::write(ARC_IMGUI_UI.get(), None);
    let ptr = ui.get() as *mut ManuallyDrop<Ui<'static>>;
    #[cfg(feature = "extension-arcdps-extern-cleanup")]
    if let Some(offset) = unsafe { *ARC_IMGUI_BUFFER_OFFSET.get() } {
        let buffer = (ptr as *mut usize).add(offset) as *mut Vec<u8>;
        ptr::drop_in_place(buffer);
    }
    drop(Box::from_raw(ptr));
}

pub unsafe fn with_imgui<R, F: FnOnce(&'_ Ui<'_>) -> R>(f: F) -> Option<R> {
    match unsafe { arc_imgui_ui() } {
        Some(ui) => Some({
            f(ui)
        }),
        None => None,
    }
}

fn arc_get_init(args: InitArgs) -> Option<InitFn> {
    unsafe {
        ptr::write(ARC_ARGS.get(), args);
    }
    exports::pre_init();

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
    new_imgui_frame();

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
    panic::set_hook(Box::new(crate::panic_hook));

    let res = panic::catch_unwind(|| {
        exports::pre_init();
        Some(exports::init())
    });
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
            Ok(Some(Ok(()))) => ExtensionHeader::new_loaded(ARC_SIG, ExtensionExports::SIZE, ARC_IMGUI_VERSION),
            Ok(Some(Err(e))) => {
                // TODO
                ::log::error!("Failed initialization: {e}");
                let message = cstr!(&"init failed");
                ExtensionHeader::new_failed(Some(message))
            },
            Ok(None) => {
                ExtensionHeader::new_failed(None)
            },
            Err(e) => {
                crate::log_join_error("init", e);
                ExtensionHeader::new_failed(Some(cstr!(&"init panic")))
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
    // XXX: leaking these buffers because the destructors call imgui APIs :<
    ptr::write(ARC_IMGUI_CONTEXT.get(), None);
    ptr::write(ARC_IMGUI_UI.get(), None);
    drop(panic::take_hook());
}

wrap_init_addr! {
    unsafe extern fn get_init_addr() => arc_get_init;
}

#[no_mangle]
pub unsafe extern "system" fn get_release_addr() -> Option<ReleaseFn> {
    Some(arc_release)
}

#[no_mangle]
pub unsafe extern "system" fn get_update_url() -> Option<CStrPtr16<'static>> {
    use windows::core::HSTRING;

    let url = exports::update_url()?;
    let url = HSTRING::from(url);
    let ptr = url.as_ptr();
    // memory leak, goodbye
    mem::forget(url);
    NonNull::new(ptr as *mut _)
        .map(|p| CStrPtr16::new(p))
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

// TODO: arcdps_unofficial_extras_subscriber_init
