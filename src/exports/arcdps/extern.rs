use {
    crate::{
        exports::{arcdps as exports, runtime as rt},
        render::element::im::{
            img::io::{UiAllocatorFns, UiAllocatorRaw, WinHeapAllocator},
            ImDrawWindow,
            UiContextCell,
        },
        settings::state::AddonHostName,
    },
    anyhow::Context,
    arcffi::{
        alloc::borrow::Cow,
        cstr::{cstr, CStrPtr, Str0},
        UnsaferCell,
    },
    core::{
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
        UserMalloc,
        HWND,
        LPARAM,
        WPARAM,
    },
    std::panic,
};

pub const ARC_SIG: NonZero<u32> = unsafe { NonZero::new_unchecked(exports::SIG) };
static ARC_ARGS: UnsaferCell<InitArgs> = unsafe { UnsaferCell::new(InitArgs::EMPTY) };
static ARC_EXPORT: UnsaferCell<ExtensionExports<'static>> =
    unsafe { UnsaferCell::new(ExtensionExports::EMPTY) };
fn exported_imgui_version() -> &'static u32 {
    let header = unsafe { &(&*ARC_EXPORT.get()).header };
    header.imgui_version()
}
static ARC_IMGUI_CONTEXT: UnsaferCell<Option<Option<UiContextCell<'static>>>> =
    unsafe { UnsaferCell::new(None) };
//pub const ARC_CB_COMBAT: ExtensionFnCombat = ExtensionExports::wrap_combat_fn_item(&arc_cb_combat);
pub const ARC_CB_COMBAT_LOCAL: ExtensionFnCombat =
    ExtensionExports::wrap_combat_fn_item(&arc_cb_combat_local);
pub const ARC_BUILD: CStrPtr<'static> = cstr!(&env!("CARGO_PKG_VERSION"));
pub const ARC_NAME: CStrPtr<'static> = CStrPtr::with_cstr(crate::exports::ADDON_TITLE_C);
fn arc_imgui_version() -> u32 {
    match arc_args().map(|arc| arc.imgui.version()) {
        #[cfg(all(todo = "unnecessary", not(taimi_imgui = "180")))]
        Some(Some(ImCtx::VERSION_20210202)) => 0,
        #[cfg(all(todo = "unnecessary", not(taimi_imgui = "192")))]
        Some(Some(ImCtx::VERSION_20260507)) => 0,
        #[cfg(all(todo = "unnecessary", not(taimi_imgui = "192")))]
        Some(None) | None => ImCtx::VERSION_20210202.get(),
        //#[cfg(taimi_imgui = "192")]
        Some(None) | None => ImCtx::VERSION_20260507.get(),
        Some(Some(v)) => v.get(),
    }
}

pub fn arc_args() -> Option<&'static InitArgs> {
    let args = unsafe { &*ARC_ARGS.get() };
    match args.module.module().is_invalid() {
        true => None,
        false => Some(args),
    }
}

#[inline(always)]
unsafe fn with_imgui<'ui, R, F: FnOnce(Option<&'ui mut UiContextCell>) -> R>(f: F) -> R {
    f(ARC_IMGUI_CONTEXT
        .as_mut_unchecked()
        .as_mut()
        .and_then(|ctx| ctx.as_mut()))
}
#[inline(always)]
unsafe fn try_with_imgui<'ui, R, F: FnOnce(&mut dyn ImDrawWindow<'ui>) -> R>(f: F) -> Option<R> {
    with_imgui(move |ctx| ctx.map(move |ctx| f(ctx.bound_ui_unchecked())))
}
/// marks it dead to avoid re-init if callback happens to be invoked
#[inline]
unsafe fn imgui_context_release() {
    if let Some(ctx) = ARC_IMGUI_CONTEXT.as_mut_unchecked() {
        drop(ctx.take());
    }
}
/// unlike [imgui_context_release] this doesn't clean up after prior context
/// and allows it to be re-initialized again
#[inline]
unsafe fn imgui_context_reset() {
    debug_assert!(matches!(ARC_IMGUI_CONTEXT.as_ref_unchecked(), None | Some(None)));
    ptr::write(ARC_IMGUI_CONTEXT.get(), None);
}
unsafe fn imgui_context_adopt<'a>(ctx: &'_ ImCtx<'a>) -> Result<UiContextCell<'a>, Cow<'static, Str0>> {
    let ptr = ctx.ptr().ok_or(cstr!(0"null context"))?;
    let version = ctx
        .version()
        .or(NonZero::new(*exported_imgui_version()))
        .ok_or(cstr!(0"version negotiation failed"))?;
    let alloc = imgui_alloc_fns(ctx.user_malloc()).ok_or(cstr!(0"malloc missing"))?;

    UiContextCell::try_new_borrowed(version, ptr.cast(), alloc)
        .ok_or(cstr!(0fmt:"unsupported version {version}").into())
}
fn imgui_alloc_fns(user_malloc: &UserMalloc) -> Option<UiAllocatorFns> {
    match (user_malloc.malloc, user_malloc.free) {
        (Some(malloc), Some(free)) => Some((Some(malloc), Some(free), user_malloc.userdata_ptr())),
        (Some(malloc), None) => Some({
            log::info!("was imgui free() meant to be missing?");
            (
                Some(malloc),
                Some(<dyn UiAllocatorRaw>::nop_free),
                user_malloc.userdata_ptr(),
            )
        }),
        (None, _) => rt::log::warn_ok(WinHeapAllocator::process_heap().context("imgui needs a heap"))
            .map(|heap| heap.get_allocator_raw()),
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
    match ARC_IMGUI_CONTEXT.as_mut_unchecked() {
        dest @ &mut None =>
            if let Some(arc) = arc_args() {
                let ctx = match imgui_context_adopt(&arc.imgui) {
                    Ok(ctx) => Some(ctx),
                    Err(e) => {
                        log::warn!("arcdps imgui setup failed: {e}");
                        // Some(None) to avoid retrying setup every frame
                        None
                    },
                };
                *dest = Some(ctx);
            },
        #[cfg(todo)]
        Some(Some(ctx)) => todo!("frame setup?"),
        Some(..) => (),
    }

    let frame = crate::render::machine::RenderMachine::ui_read_context().to_frame_storage();
    with_imgui(|ui| {
        let ui = ui.map(|ui| (ui, frame.as_ref()));
        exports::imgui_present(ui, not_charsel_or_loading.into(), hide_if_combat_or_ooc.value)
    });
}

unsafe extern "C" fn arc_cb_imgui_options_tab() {
    let frame = crate::render::machine::RenderMachine::ui_read_context().to_frame_storage();
    let _ = try_with_imgui(|ui| exports::imgui_draw_options_tab(ui, frame.as_ref()));
}

unsafe extern "C" fn arc_cb_imgui_options_windows(window_name: Option<CStrPtr>) -> c_bool32 {
    let frame = crate::render::machine::RenderMachine::ui_read_context().to_frame_storage();
    try_with_imgui(|ui| {
        let window = window_name.as_ref().map(|w| w.to_string_lossy());
        let window = window.as_ref().map(|w| &w[..]);
        exports::imgui_draw_options_windows(ui, frame.as_ref(), window)
    })
    .map(Into::into)
    .unwrap_or(c_bool32::FALSE)
}

extern "C" fn arc_init() -> Option<NonNull<ExtensionExports<'static>>> {
    exports::pre_init();

    unsafe {
        imgui_context_reset();
    }
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
                log::error!("Failed initialization: {e}");
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
    imgui_context_release();
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
