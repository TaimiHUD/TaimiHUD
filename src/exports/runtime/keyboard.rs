use {
    std::{convert::identity, iter, mem::size_of, num::NonZeroU16},
    crate::exports::runtime::{self as rt, RuntimeResult},
    windows::Win32::{
        System::SystemServices::{self, MODIFIERKEYS_FLAGS},
        UI::{
            WindowsAndMessaging,
            Input::KeyboardAndMouse::{self, MOUSE_EVENT_FLAGS, VIRTUAL_KEY},
        },
    },
};
#[cfg(feature = "extension-arcdps")]
use arcdps::extras::{self, KeybindChange, KeyCode, MouseCode};

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq/*, PartialOrd, Ord, Hash*/)]
pub struct KeyInput {
    pub vk: VIRTUAL_KEY,
    pub down: bool,
    pub mods: KeyState,
}

impl KeyInput {
    pub const EMPTY: Self = Self::empty_with_mods(KeyState::EMPTY, false);
    pub const VK_EMPTY: VIRTUAL_KEY = VIRTUAL_KEY(0);

    pub const fn new(vk: VIRTUAL_KEY, mods: KeyState, down: bool) -> Self {
        Self {
            vk,
            mods,
            down,
        }
    }

    pub const fn empty_with_mods(mods: KeyState, down: bool) -> Self {
        Self::new(Self::VK_EMPTY, mods, down)
    }

    pub const fn vk_down(vk: VIRTUAL_KEY) -> Self {
        Self::new(vk, KeyState::EMPTY, true)
    }

    pub const fn vk_up(vk: VIRTUAL_KEY) -> Self {
        Self::new(vk, KeyState::EMPTY, false)
    }

    pub const fn is_empty(&self) -> bool {
        match self.vk {
            Self::VK_EMPTY => true,
            _ => false,
        }
    }

    pub const fn mods_unused(&self) -> KeyState {
        KeyState::from_bits_retain(!self.mods.bits() & KeyState::MODS.bits())
    }

    pub fn vk_as_mod(&self) -> KeyState {
        KeyState::from_index(self.vk.0.into())
    }

    pub fn to_event(self) -> (u32, usize, isize) {
        let msg = {
            let as_mod = self.vk_as_mod();
            // a dummy so it isn't unrecognized isn't necessary, but is more correct...
            #[cfg(debug_assertions)]
            let as_mod = as_mod.any().unwrap_or(KeyState::SHIFT);
            as_mod.event_msg(self.down)
        };
        let prev_state = ((!self.down) as isize) << 30;
        let trans_state = ((!self.down) as isize) << 31;
        let w = self.vk.0 as _;
        let l = prev_state | trans_state;
        (msg, w, l)
    }

    pub fn to_input(self) -> KeyboardAndMouse::INPUT {
        let flag_down = match self.down {
            false => KeyboardAndMouse::KEYEVENTF_KEYUP,
            true => Default::default(),
        };
        // TODO: KEYEVENTF_EXTENDED?
        KeyboardAndMouse::INPUT {
            r#type: KeyboardAndMouse::INPUT_KEYBOARD,
            Anonymous: KeyboardAndMouse::INPUT_0 {
                ki: KeyboardAndMouse::KEYBDINPUT {
                    wVk: self.vk,
                    wScan: scan_code(self.vk).map(|sc| sc.get()).unwrap_or(0),
                    dwFlags: flag_down,
                    time: 0,
                    dwExtraInfo: 0,
                }
            },
        }
    }
}

impl From<KeyInput> for KeyboardAndMouse::INPUT {
    fn from(input: KeyInput) -> Self {
        input.to_input()
    }
}

impl From<VIRTUAL_KEY> for KeyInput {
    fn from(vk: VIRTUAL_KEY) -> Self {
        Self::vk_down(vk)
    }
}

impl From<u16> for KeyInput {
    fn from(vk: u16) -> Self {
        Self::from(VIRTUAL_KEY(vk))
    }
}

#[cfg(feature = "extension-arcdps")]
impl From<KeyCode> for KeyInput {
    fn from(vk: KeyCode) -> Self {
        Self::vk_down(VIRTUAL_KEY(vk as _))
    }
}

#[cfg(feature = "extension-arcdps")]
impl TryFrom<KeybindChange> for KeyInput {
    type Error = rt::RuntimeError;

    fn try_from(key: KeybindChange) -> Result<Self, Self::Error> {
        let mods = KeyState::from(&key);
        let mut input = match key.key {
            extras::Key::Key(code) => Ok(Self::from(code)),
            _ => Err("not a keyboard binding"),
        }?;
        input.mods = mods;
        Ok(input)
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct KeyState: u32 {
        const BUTTON_L = 1 << KeyboardAndMouse::VK_LBUTTON.0;
        const BUTTON_R = 1 << KeyboardAndMouse::VK_RBUTTON.0;
        const BUTTON_M = 1 << KeyboardAndMouse::VK_MBUTTON.0;
        const BUTTON_X1 = 1 << KeyboardAndMouse::VK_XBUTTON1.0;
        const BUTTON_X2 = 1 << KeyboardAndMouse::VK_XBUTTON2.0;
        const SHIFT = 1 << KeyboardAndMouse::VK_SHIFT.0;
        const CTRL = 1 << KeyboardAndMouse::VK_CONTROL.0;
        const ALT = 1 << KeyboardAndMouse::VK_MENU.0;
    }
}

impl KeyState {
    pub const EMPTY: Self = Self::empty();
    pub const MODS: Self = Self::from_bits_retain(Self::SHIFT.bits() | Self::CTRL.bits() | Self::ALT.bits());
    pub const MODIFIERKEYS: Self = Self::from_bits_retain(Self::SHIFT.bits() | Self::CTRL.bits() | Self::ALT.bits());
    pub const BUTTON_LRM: Self = Self::from_bits_retain(Self::BUTTON_L.bits() | Self::BUTTON_R.bits() | Self::BUTTON_M.bits());
    pub const BUTTON_X: Self = Self::from_bits_retain(Self::BUTTON_X1.bits() | Self::BUTTON_X2.bits());
    pub const BUTTON: Self = Self::from_bits_retain(Self::BUTTON_LRM.bits() | Self::BUTTON_X.bits());

    pub const fn any(self) -> Option<Self> {
        match self.is_empty() {
            true => None,
            false => Some(self),
        }
    }

    pub const fn key(self) -> Option<Self> {
        match self.is_unique() {
            true => Some(self),
            _ => None,
        }
    }

    pub const fn index(self) -> u32 {
        #[cfg(debug_assertions)]
        if !self.is_unique() {
            panic!("KeyState index expects unique bit")
        }
        self.bits().trailing_zeros()
    }

    pub const fn from_index_retain(index: u32) -> Self {
        Self::from_bits_retain(1 << index)
    }

    const INDEX_BOUND: u32 = (size_of::<KeyState>() * 8) as u32;
    pub const fn from_index(index: u32) -> Self {
        match index {
            Self::INDEX_BOUND..=u32::MAX => Self::EMPTY,
            shift => Self::from_bits_truncate(1 << shift),
        }
    }

    pub const fn is_unique(self) -> bool {
        self.bits().count_ones() == 1
    }

    pub const fn vkeycode(self) -> VIRTUAL_KEY {
        VIRTUAL_KEY(self.index() as u16)
    }

    pub const fn modifierkey(self) -> Option<MODIFIERKEYS_FLAGS> {
        #[cfg(debug_assertions)]
        if !self.is_unique() {
            panic!("KeyState::mousekeycode expects unique bit")
        }
        Some(match self {
            Self::CTRL => SystemServices::MK_CONTROL,
            Self::SHIFT => SystemServices::MK_SHIFT,
            Self::BUTTON_L => SystemServices::MK_LBUTTON,
            Self::BUTTON_R => SystemServices::MK_RBUTTON,
            Self::BUTTON_M => SystemServices::MK_MBUTTON,
            Self::BUTTON_X1 => SystemServices::MK_XBUTTON1,
            Self::BUTTON_X2 => SystemServices::MK_XBUTTON2,
            _ => return None,
        })
    }

    pub const ALL_BUTTONS: [Self; 5] = [Self::BUTTON_L, Self::BUTTON_R, Self::BUTTON_M, Self::BUTTON_X1, Self::BUTTON_X2];

    /// [0..=4](Self::ALL_BUTTONS) for L/R/M/X1/X2
    pub const fn button_index(self) -> Option<usize> {
        const INDEX_BUTTON_L: u32 = KeyboardAndMouse::VK_LBUTTON.0 as _;
        const INDEX_BUTTON_R: u32 = KeyboardAndMouse::VK_RBUTTON.0 as _;
        const INDEX_BUTTON_M: u32 = KeyboardAndMouse::VK_MBUTTON.0 as _;
        const INDEX_BUTTON_X2: u32 = KeyboardAndMouse::VK_XBUTTON2.0 as _;
        const OFFSET_BUTTON_L: u32 = INDEX_BUTTON_L;
        const OFFSET_BUTTON_M: u32 = INDEX_BUTTON_M - 2;
        let button = match self.index() {
            idx @ INDEX_BUTTON_L..=INDEX_BUTTON_R => idx - OFFSET_BUTTON_L,
            idx @ INDEX_BUTTON_M..=INDEX_BUTTON_X2 => idx - OFFSET_BUTTON_M,
            _ => return None,
        };
        Some(button as usize)
    }

    const BUTTON_X_INDEX: u32 = Self::BUTTON_X1.index();
    pub const fn button_x(self) -> u32 {
        (self.bits() & Self::BUTTON_X.bits()) >> Self::BUTTON_X_INDEX
    }

    pub const fn from_button_index(index: usize) -> Option<Self> {
        match index {
            index @ 0..=4 => Some(Self::ALL_BUTTONS[index]),
            _ => None,
        }
    }

    pub const fn mouse_flag(self, down: bool) -> Option<KeyboardAndMouse::MOUSE_EVENT_FLAGS> {
        let flag = match (self, down) {
            (button, _) if !button.intersects(Self::BUTTON) =>
                return None,
            (Self::BUTTON_L, true) => KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN,
            (Self::BUTTON_L, false) => KeyboardAndMouse::MOUSEEVENTF_LEFTUP,
            (Self::BUTTON_R, true) => KeyboardAndMouse::MOUSEEVENTF_RIGHTDOWN,
            (Self::BUTTON_R, false) => KeyboardAndMouse::MOUSEEVENTF_RIGHTUP,
            (Self::BUTTON_M, true) => KeyboardAndMouse::MOUSEEVENTF_MIDDLEDOWN,
            (Self::BUTTON_M, false) => KeyboardAndMouse::MOUSEEVENTF_MIDDLEUP,
            (_, false) => KeyboardAndMouse::MOUSEEVENTF_XDOWN,
            (_, true) => KeyboardAndMouse::MOUSEEVENTF_XUP,
        };
        Some(flag)
    }

    pub const fn event_msg(self, down: bool) -> u32 {
        match (self, down) {
            (b, true) if b.intersects(Self::BUTTON) => match b {
                Self::BUTTON_L => WindowsAndMessaging::WM_LBUTTONDOWN,
                Self::BUTTON_R =>  WindowsAndMessaging::WM_RBUTTONDOWN,
                Self::BUTTON_M =>  WindowsAndMessaging::WM_MBUTTONDOWN,
                _ =>  WindowsAndMessaging::WM_XBUTTONDOWN,
            },
            (b, false) if b.intersects(Self::BUTTON) => match b {
                Self::BUTTON_L => WindowsAndMessaging::WM_LBUTTONUP,
                Self::BUTTON_R =>  WindowsAndMessaging::WM_RBUTTONUP,
                Self::BUTTON_M =>  WindowsAndMessaging::WM_MBUTTONUP,
                _ =>  WindowsAndMessaging::WM_XBUTTONUP,
            },
            (Self::ALT, true) => WindowsAndMessaging::WM_SYSKEYDOWN,
            (Self::ALT, false) => WindowsAndMessaging::WM_SYSKEYUP,
            (_, true) => WindowsAndMessaging::WM_KEYDOWN,
            (_, false) => WindowsAndMessaging::WM_KEYUP,
        }
    }

    pub const fn event_w(self, msg: u32) -> usize {
        match msg {
            WindowsAndMessaging::WM_XBUTTONUP => (self.button_x() << 16) as usize,
            WindowsAndMessaging::WM_SYSKEYDOWN | WindowsAndMessaging::WM_SYSKEYUP | WindowsAndMessaging::WM_KEYDOWN | WindowsAndMessaging::WM_KEYUP =>
                self.vkeycode().0 as _,
            _ => 0,
        }
    }

    pub fn take(&mut self, mask: Self) -> Self {
        let taken = *self & mask;
        self.remove(mask);
        taken
    }

    pub fn next_key(self) -> Option<Self> {
        let bit = match self.bits().trailing_zeros() {
            Self::INDEX_BOUND => return None,
            shift => Self::from_bits_retain(shift),
        };
        Some(bit)
    }

    pub fn take_key(&mut self) -> Option<Self> {
        let key = self.next_key();
        if let Some(key) = key {
            self.remove(key);
        }
        return key
    }

    pub fn iter_keys(mut self) -> impl Iterator<Item = Self> + Clone + Send + Sync + 'static {
        iter::from_fn(move || self.take_key())
    }

    pub fn vkeycodes(self) -> impl Iterator<Item = VIRTUAL_KEY> + Clone + Send + Sync + 'static {
        self.iter_keys()
            .map(|flag| flag.vkeycode())
    }

    pub fn modifierkeys(self) -> impl Iterator<Item = MODIFIERKEYS_FLAGS> + Clone + Send + Sync + 'static {
        self.iter_keys()
            .filter_map(|flag| flag.modifierkey())
    }

    pub fn to_modifierkeys(self) -> MODIFIERKEYS_FLAGS {
        MODIFIERKEYS_FLAGS(self.modifierkeys().map(|m| m.0).sum())
    }

    pub fn mouse_flags(self, down: bool) -> impl Iterator<Item = MOUSE_EVENT_FLAGS> + Clone + Send + Sync + 'static {
        self.iter_keys()
            .filter_map(move |flag| flag.mouse_flag(down))
    }

    pub fn to_mouse_flags(self, down: bool) -> MOUSE_EVENT_FLAGS {
        MOUSE_EVENT_FLAGS(self.mouse_flags(down).map(|f| f.0).sum())
    }
}

#[cfg(feature = "extension-arcdps")]
impl TryFrom<MouseCode> for KeyState {
    type Error = rt::RuntimeError;

    fn try_from(m: MouseCode) -> Result<Self, Self::Error> {
        Self::from_button_index(m as _)
            .ok_or("mouse button beyond L/R/M/X1/X2")
    }
}

#[cfg(feature = "extension-arcdps")]
impl From<KeybindChange> for KeyState {
    fn from(k: KeybindChange) -> Self {
        Self::from(&k)
    }
}

#[cfg(feature = "extension-arcdps")]
impl<'a> From<&'a KeybindChange> for KeyState {
    fn from(k: &'a KeybindChange) -> Self {
        [
            k.mod_ctrl.then_some(Self::CTRL),
            k.mod_shift.then_some(Self::SHIFT),
            k.mod_alt.then_some(Self::ALT),
        ].into_iter().collect()
    }
}

impl Extend<Option<Self>> for KeyState {
    fn extend<I: IntoIterator<Item = Option<Self>>>(&mut self, iter: I) {
        self.extend(iter.into_iter().filter_map(identity));
    }
}
impl FromIterator<Option<Self>> for KeyState {
    fn from_iter<T: IntoIterator<Item = Option<Self>>>(iter: T) -> Self {
        iter.into_iter().filter_map(identity).collect()
    }
}

pub fn scan_code(vk: VIRTUAL_KEY) -> Option<NonZeroU16> {
    let vsc = unsafe {
        KeyboardAndMouse::MapVirtualKeyA(vk.0.into(), KeyboardAndMouse::MAPVK_VK_TO_VSC)
    };
    NonZeroU16::new(vsc as u16)
}

pub fn key_name(code: NonZeroU16) -> windows::core::Result<windows::core::HSTRING> {
    let mut buf = [0u16; 128];
    let res = unsafe {
        match KeyboardAndMouse::GetKeyNameTextW(code.get() as i32, &mut buf) {
            0 => Err(windows::core::Error::from_win32()),
            sz => Ok(sz as usize),
        }
    };
    match res {
        Err(e) => Err(e),
        Ok(len @ 0..=128) => Ok(windows::core::HSTRING::from_wide(&buf[..len])),
        Ok(_res) => {
            log::debug!("weird, I didn't ask for {_res}");
            Err(windows::core::Error::new(windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER.to_hresult(), "key name too long"))
        },
    }
}

pub fn scan_code_key(vsc: NonZeroU16) -> Option<VIRTUAL_KEY> {
    let vk = unsafe {
        KeyboardAndMouse::MapVirtualKeyA(vsc.get().into(), KeyboardAndMouse::MAPVK_VSC_TO_VK)
    };
    NonZeroU16::new(vk as u16)
        .map(|vk| vk.get())
        .map(VIRTUAL_KEY)
}

pub fn send_key_combo<I: Into<KeyInput>>(input: I) -> RuntimeResult<()> {
    let input = input.into();
    do_key_combo(move || send_key(input), input)
}

pub fn do_key_combo<R, F: FnOnce() -> Result<R, rt::RuntimeError>>(f: F, input: KeyInput) -> RuntimeResult<R> {
    if input.down {
        // start by "holding" down any relevant modifiers
        let mod_inputs = input.mods.vkeycodes().map(KeyInput::vk_down);
        // (and releasing the inverse)
        let mod_unused = match input.mods.is_empty() {
            true => KeyState::EMPTY,
            false => input.mods_unused(),
        }.vkeycodes().map(KeyInput::vk_up);
        for mod_input in mod_unused.chain(mod_inputs) {
            send_key(mod_input)?;
        }
    }

    let mut res = f();

    if !input.down {
        // release modifiers afterward
        for mod_input in input.mods.vkeycodes().map(KeyInput::vk_up) {
            if let Err(e) = send_key(mod_input) {
                if res.is_ok() {
                    res = Err(e);
                }
            }
        }
    }

    res
}

pub fn send_key(input: KeyInput) -> RuntimeResult<()> {
    let (msg, w, l) = input.to_event();
    unsafe {
        rt::window_message(msg, w, l)
    }
}

pub fn send_key_input<I: Into<KeyInput>>(input: I) -> RuntimeResult<()> {
    let input = input.into();
    let mods = input.mods;
    let mod_inputs = input.down.then({
        let mods_unused = match mods.is_empty() {
            true => KeyState::EMPTY,
            false => input.mods_unused(),
        };
        move || mods.vkeycodes().map(KeyInput::vk_down)
            .chain(mods_unused.vkeycodes().map(KeyInput::vk_up))
    }).into_iter().flatten();

    let mod_release = (!input.down).then(move ||
        mods.vkeycodes().map(KeyInput::vk_up)
    ).into_iter().flatten();

    let inputs = mod_inputs
        .chain(iter::once(input.into()))
        .chain(mod_release);
    rt::window_send_inputs(inputs)
}
