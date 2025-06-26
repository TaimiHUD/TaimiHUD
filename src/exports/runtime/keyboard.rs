use {
    std::{iter, num::NonZeroU16, ops},
    crate::exports::runtime::{self as rt, RuntimeResult},
    windows::Win32::UI::{
        WindowsAndMessaging,
        Input::KeyboardAndMouse::{self, VIRTUAL_KEY},
    },
};
#[cfg(feature = "extension-arcdps")]
use arcdps::extras::KeybindChange;

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq/*, PartialOrd, Ord, Hash*/)]
pub struct KeyInput {
    pub vk: VIRTUAL_KEY,
    pub down: bool,
}

impl KeyInput {
    pub const fn new(vk: VIRTUAL_KEY, down: bool) -> Self {
        Self {
            vk,
            down,
        }
    }

    const VK_ALT: VIRTUAL_KEY = KeyboardAndMouse::VK_MENU;
    pub fn to_event(self) -> (u32, usize, isize) {
        let msg = match (self.down, self.vk) {
            (true, Self::VK_ALT) => WindowsAndMessaging::WM_SYSKEYDOWN,
            (true, _) => WindowsAndMessaging::WM_KEYDOWN,
            (false, Self::VK_ALT) => WindowsAndMessaging::WM_SYSKEYUP,
            (false, _) => WindowsAndMessaging::WM_KEYUP,
        };
        let prev_state = ((!self.down) as isize) << 30;
        let trans_state = (self.down as isize) << 31;
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

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyMods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[cfg(feature = "extension-arcdps")]
impl From<KeybindChange> for KeyMods {
    fn from(k: KeybindChange) -> Self {
        Self::from(&k)
    }
}

#[cfg(feature = "extension-arcdps")]
impl<'a> From<&'a KeybindChange> for KeyMods {
    fn from(k: &'a KeybindChange) -> Self {
        Self {
            ctrl: k.mod_ctrl,
            shift: k.mod_shift,
            alt: k.mod_alt,
        }
    }
}

impl KeyMods {
    pub fn is_empty(&self) -> bool {
        !(self.ctrl | self.shift | self.alt)
    }

    pub fn vkeycodes(self) -> impl Iterator<Item = VIRTUAL_KEY> + Clone + Send + Sync + 'static {
        self.ctrl.then_some(KeyboardAndMouse::VK_CONTROL).into_iter()
            .chain(self.shift.then_some(KeyboardAndMouse::VK_SHIFT))
            .chain(self.alt.then_some(KeyboardAndMouse::VK_MENU))
    }
}

impl ops::Not for KeyMods {
    type Output = Self;

    fn not(mut self) -> Self {
        self.ctrl ^= true;
        self.alt ^= true;
        self.shift ^= true;
        self
    }
}

pub fn scan_code(vk: VIRTUAL_KEY) -> Option<NonZeroU16> {
    let vsc = unsafe {
        KeyboardAndMouse::MapVirtualKeyA(vk.0.into(), KeyboardAndMouse::MAPVK_VK_TO_VSC)
    };
    NonZeroU16::new(vsc as u16)
}

pub fn send_key_combo(input: KeyInput, mods: KeyMods) -> RuntimeResult<()> {
    do_key_combo(move || send_key(input), input.down, mods)
}

pub fn do_key_combo<R, F: FnOnce() -> Result<R, rt::RuntimeError>>(f: F, down: bool, mods: KeyMods) -> RuntimeResult<R> {
    let mod_inputs = mods.vkeycodes().map(|vk| KeyInput::new(vk, down));
    if down {
        // start by "holding" down any relevant modifiers
        let mod_inputs =
            // (and releasing the inverse)
            (!mods).vkeycodes().map(|vk| KeyInput::new(vk, false))
            .chain(mod_inputs.clone());
        for mod_input in mod_inputs {
            send_key(mod_input)?;
        }
    }

    let mut res = f();

    if !down {
        // release modifiers afterward
        for mod_input in mod_inputs {
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
    rt::window_send_inputs(iter::once(input.into()))
}
