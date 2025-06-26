use std::{borrow::Cow, num::NonZeroU64, ptr, time::Duration};
use arcdps::{extras::{ExtrasAddonInfo, KeybindChange, UserInfoIter}, imgui, Language};
use dpsapi::combat::CombatArgs;
use crate::exports::{arcdps as exports, runtime as rt};
use windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY;

#[cfg(feature = "extension-nexus")]
pub fn has_extension<const SIG: u32>() -> bool {
    use core::cell::Cell;

    thread_local! {
        static HAS_EXT: Cell<bool> = Cell::new(false);
    }

    extern "C" fn list_cb<const SIG: u32>(exp: &arcdps::callbacks::ArcDpsExport) {
        if exp.sig == SIG {
            HAS_EXT.set(true);
        }
    }

    HAS_EXT.set(false);
    unsafe {
        arcdps::exports::raw::list_extension(list_cb::<SIG> as usize as *mut _)
    }

    HAS_EXT.get()
}

pub fn init() -> Result<(), String> {
    exports::init()
        .map_err(Into::into)
}

pub fn release() {
    exports::release()
}

pub fn imgui(ui: &imgui::Ui, not_charsel_loading: bool) {
    exports::imgui(ui, not_charsel_loading, 0)
}

pub fn options_end(ui: &imgui::Ui) {
    exports::imgui_options_tab(ui)
}

pub fn options_windows(ui: &imgui::Ui, window_name: Option<&str>) -> bool {
    exports::imgui_options_windows(ui, window_name)
}

pub fn wnd_filter(keycode: usize, key_down: bool, prev_key_down: bool) -> bool {
    let vk = VIRTUAL_KEY(keycode as _);
    let (msg, w, mut l) = rt::keyboard::KeyInput::new(vk, key_down).to_event();
    if key_down == prev_key_down {
        l ^= 1 << 30;
    }

    match exports::wnd_filter(ptr::null_mut(), msg, w.into(), l.into()) {
        0 => false,
        _ => true,
    }
}

pub fn update_url() -> Option<String> {
    exports::update_url()
        .map(Into::into)
}

pub fn combat_local(
    ev: Option<&arcdps::Event>,
    src: Option<&arcdps::Agent>,
    dst: Option<&arcdps::Agent>,
    skill_name: Option<&'static str>,
    id: u64,
    revision: u64,
) {
    let skill_name = match skill_name {
        // if one strongly suspects the str wasn't reallocated
        // then you could do an out-of-bounds check, but also...
        // just don't, it's unused anyway
        _ => Default::default(),
    };
    let event = CombatArgs {
        ev: ev
            .map(|e| Cow::Borrowed(e.as_ref())),
        src: src
            .map(|a| Cow::Borrowed(a.as_ref())),
        dst: dst
            .map(|a| Cow::Borrowed(a.as_ref())),
        skill_name,
        id: NonZeroU64::new(id),
        revision,
    };
    exports::combat_local(event)
}

pub fn extras_init(info: ExtrasAddonInfo, account_name: Option<&str>) {
    if let Some(name) = account_name {
        crate::receive_account_name(name);
    }
    exports::extras_init(info)
}

pub fn extras_language(language: Language) {
    exports::extras_language(language)
}

pub fn extras_keybind(changed: KeybindChange) {
    exports::extras_keybind(changed)
}

pub fn extras_squad_update(members: UserInfoIter) {
    exports::extras_squad_update(members)
}

pub fn available() -> bool {
    arcdps::d3d_version() != 0
}

#[cfg(any(feature = "space", feature = "texture-loader"))]
pub fn dxgi_swap_chain() -> Option<windows::Win32::Graphics::Dxgi::IDXGISwapChain> {
    let swap_chain: Option<_> = arcdps::dxgi_swap_chain().map(|sc| sc.to_owned());

    unsafe {
        core::mem::transmute(swap_chain)
    }
}
