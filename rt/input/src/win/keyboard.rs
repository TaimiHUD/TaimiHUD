#[cfg(feature = "arcdps-extras")]
use arcdps::extras::{self, keybinds::RawKeybindChange, KeyCode, KeybindChange, MouseCode};
use {
    crate::win::{window_message, window_send_inputs},
    anyhow::{anyhow, Context},
    core::{convert::identity, ffi::CStr, fmt, iter, mem::size_of, num::NonZeroU16},
    windows::Win32::{
        Foundation::{HWND, LPARAM},
        System::SystemServices::{self, MODIFIERKEYS_FLAGS},
        UI::{
            Input::KeyboardAndMouse::{self, MOUSE_EVENT_FLAGS, VIRTUAL_KEY},
            WindowsAndMessaging,
        },
    },
};

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq /*, PartialOrd, Ord, Hash*/)]
pub struct KeyInput {
    pub vk: VIRTUAL_KEY,
    pub down: bool,
    pub mods: KeyState,
}

impl KeyInput {
    pub const EMPTY: Self = Self::empty_with_mods(KeyState::EMPTY, false);
    pub const VK_EMPTY: VIRTUAL_KEY = VIRTUAL_KEY(0);

    pub const fn new(vk: VIRTUAL_KEY, mods: KeyState, down: bool) -> Self {
        Self { vk, mods, down }
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

    pub const fn any(self) -> Option<Self> {
        match self.is_empty() {
            true => None,
            false => Some(self),
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
            match as_mod.intersects(KeyState::BUTTON) {
                true => as_mod,
                false => self.mods & KeyState::MODS,
            }
            .event_msg(self.down)
        };
        let repeat = 1u16;
        let prev_state = ((!self.down) as isize) << 30;
        let trans_state = ((!self.down) as isize) << 31;
        let sc = match scan_code(self.vk).map(|sc| sc.get()).unwrap_or(0) {
            sc @ 0..=0xff => sc as isize,
            sc => sc as u8 as isize | (sc as isize & 0x8000) >> 7,
        } << 16;
        let w = self.vk.0 as _;
        let l = prev_state | trans_state | sc | repeat as isize;
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
                },
            },
        }
    }

    #[cfg(feature = "arcdps-extras")]
    pub fn vk_from_extras(key: KeyCode) -> VIRTUAL_KEY {
        match key {
            KeyCode::LeftAlt => KeyboardAndMouse::VK_LMENU,
            KeyCode::LeftCtrl => KeyboardAndMouse::VK_LCONTROL,
            KeyCode::LeftShift => KeyboardAndMouse::VK_LSHIFT,
            KeyCode::LeftWin => KeyboardAndMouse::VK_LWIN,
            KeyCode::RightAlt => KeyboardAndMouse::VK_RMENU,
            KeyCode::RightCtrl => KeyboardAndMouse::VK_RCONTROL,
            KeyCode::RightShift => KeyboardAndMouse::VK_RSHIFT,
            KeyCode::RightWin => KeyboardAndMouse::VK_RWIN,
            KeyCode::Menu => KeyboardAndMouse::VK_APPS,
            KeyCode::Minus => KeyboardAndMouse::VK_OEM_MINUS,
            KeyCode::Equals => KeyboardAndMouse::VK_OEM_PLUS,
            KeyCode::CapsLock => KeyboardAndMouse::VK_CAPITAL,
            KeyCode::NumLock => KeyboardAndMouse::VK_NUMLOCK,
            KeyCode::Enter => KeyboardAndMouse::VK_RETURN,
            KeyCode::ArrowDown => KeyboardAndMouse::VK_DOWN,
            KeyCode::ArrowLeft => KeyboardAndMouse::VK_LEFT,
            KeyCode::ArrowRight => KeyboardAndMouse::VK_RIGHT,
            KeyCode::ArrowUp => KeyboardAndMouse::VK_UP,
            KeyCode::Tab => KeyboardAndMouse::VK_TAB,
            KeyCode::Home => KeyboardAndMouse::VK_HOME,
            KeyCode::End => KeyboardAndMouse::VK_END,
            KeyCode::Insert => KeyboardAndMouse::VK_INSERT,
            KeyCode::Next => KeyboardAndMouse::VK_NEXT,
            KeyCode::Prior => KeyboardAndMouse::VK_PRIOR,
            KeyCode::Delete => KeyboardAndMouse::VK_DELETE,
            KeyCode::Backspace => KeyboardAndMouse::VK_BACK,
            // TODO: colon vs semicolon?
            KeyCode::Colon => KeyboardAndMouse::VK_OEM_1,
            KeyCode::Semicolon => KeyboardAndMouse::VK_OEM_1,
            KeyCode::Slash => KeyboardAndMouse::VK_OEM_2,
            KeyCode::Tilde => KeyboardAndMouse::VK_OEM_3,
            KeyCode::OpenBracket => KeyboardAndMouse::VK_OEM_4,
            KeyCode::Backslash => KeyboardAndMouse::VK_OEM_5,
            KeyCode::CloseBracket => KeyboardAndMouse::VK_OEM_6,
            KeyCode::Quote => KeyboardAndMouse::VK_OEM_7,
            // TODO: dunno where this one goes .-.
            KeyCode::Hash => KeyboardAndMouse::VK_OEM_102,
            KeyCode::Period => KeyboardAndMouse::VK_OEM_PERIOD,
            KeyCode::Print => KeyboardAndMouse::VK_PRINT,
            KeyCode::Escape => KeyboardAndMouse::VK_ESCAPE,
            KeyCode::PlusNum => KeyboardAndMouse::VK_ADD,
            KeyCode::DivideNum => KeyboardAndMouse::VK_DIVIDE,
            KeyCode::MinusNum => KeyboardAndMouse::VK_SUBTRACT,
            KeyCode::MultiplyNum => KeyboardAndMouse::VK_MULTIPLY,
            KeyCode::DecimalNum => KeyboardAndMouse::VK_DECIMAL,
            KeyCode::Space => KeyboardAndMouse::VK_SPACE,
            KeyCode::ImeKey1 => KeyboardAndMouse::VK_IME_ON,
            KeyCode::ImeKey2 => KeyboardAndMouse::VK_IME_OFF,
            // TODO: KeyCode::EnterNum => VIRTUAL_KEY(0xe01c)?
            KeyCode::EnterNum => KeyboardAndMouse::VK_RETURN,
            k if k >= KeyCode::Number0Num && k <= KeyCode::Number9Num =>
                VIRTUAL_KEY(KeyboardAndMouse::VK_NUMPAD0.0 + (k as i32 - KeyCode::Number0Num as i32) as u16),
            k if k >= KeyCode::F1 && k <= KeyCode::F12 =>
                VIRTUAL_KEY(KeyboardAndMouse::VK_F1.0 + (k as i32 - KeyCode::F1 as i32) as u16),
            k => VIRTUAL_KEY(k as i32 as u16),
        }
    }

    #[cfg(feature = "arcdps-extras")]
    pub fn vk_from_extras_button(button: MouseCode) -> VIRTUAL_KEY {
        match button {
            MouseCode::Mouse1 => KeyboardAndMouse::VK_LBUTTON,
            // yes 2 and 3 are inverted
            MouseCode::Mouse2 => KeyboardAndMouse::VK_MBUTTON,
            MouseCode::Mouse3 => KeyboardAndMouse::VK_RBUTTON,
            // unsure of any of this mapping tbh...
            MouseCode::Mouse4 => KeyboardAndMouse::VK_XBUTTON1,
            MouseCode::Mouse5 => KeyboardAndMouse::VK_XBUTTON2,
            MouseCode::Mouse6 => KeyboardAndMouse::VK_GAMEPAD_A,
            MouseCode::Mouse7 => KeyboardAndMouse::VK_GAMEPAD_B,
            MouseCode::Mouse8 => KeyboardAndMouse::VK_GAMEPAD_X,
            MouseCode::Mouse9 => KeyboardAndMouse::VK_GAMEPAD_Y,
            MouseCode::Mouse10 => KeyboardAndMouse::VK_GAMEPAD_RIGHT_SHOULDER,
            MouseCode::Mouse11 => KeyboardAndMouse::VK_GAMEPAD_LEFT_SHOULDER,
            MouseCode::Mouse12 => KeyboardAndMouse::VK_GAMEPAD_LEFT_TRIGGER,
            MouseCode::Mouse13 => KeyboardAndMouse::VK_GAMEPAD_RIGHT_TRIGGER,
            MouseCode::Mouse14 => KeyboardAndMouse::VK_GAMEPAD_DPAD_UP,
            MouseCode::Mouse15 => KeyboardAndMouse::VK_GAMEPAD_DPAD_DOWN,
            MouseCode::Mouse16 => KeyboardAndMouse::VK_GAMEPAD_DPAD_LEFT,
            MouseCode::Mouse17 => KeyboardAndMouse::VK_GAMEPAD_DPAD_RIGHT,
            MouseCode::Mouse18 => KeyboardAndMouse::VK_GAMEPAD_MENU,
            MouseCode::Mouse19 => KeyboardAndMouse::VK_GAMEPAD_VIEW,
            MouseCode::Mouse20 => KeyboardAndMouse::VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON,
            #[cfg(todo)]
            MouseCode::Mouse18 => KeyboardAndMouse::VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON,
            #[cfg(todo)]
            MouseCode::Mouse19 => KeyboardAndMouse::VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON,
        }
    }

    pub fn vk_is_button(vk: VIRTUAL_KEY) -> bool {
        match vk {
            KeyboardAndMouse::VK_LBUTTON
            | KeyboardAndMouse::VK_MBUTTON
            | KeyboardAndMouse::VK_RBUTTON
            | KeyboardAndMouse::VK_XBUTTON1
            | KeyboardAndMouse::VK_XBUTTON2 => true,
            vk =>
                vk.0 >= KeyboardAndMouse::VK_GAMEPAD_A.0
                //&& vk.0 <= KeyboardAndMouse::VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON.0
                && vk.0 <= KeyboardAndMouse::VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT.0,
        }
    }

    pub fn parse_ascii(s: &[u8]) -> anyhow::Result<Self> {
        use KeyboardAndMouse as vk;

        let (mods, key) = KeyState::parse_bind_ascii(s)?;
        let mut bind = Self::empty_with_mods(mods, false);

        if let Some(button) = KeyState::from_ascii(s) {
            return if button.is_empty() && !bind.mods.is_empty() {
                Err(anyhow!("nonsensical keybind"))
            } else if !button.is_empty() && bind.mods.intersects(button) {
                // would use button_from_ascii but why not parse mods as keys anyway...
                Err(anyhow!("redundant keybind"))
            } else {
                bind.vk = button.vkeycode();
                Ok(bind)
            }
        }

        let key_str = match key.is_ascii() {
            true => Some(unsafe { str::from_utf8_unchecked(key) }),
            false => None,
        };
        let vk = match key_str.unwrap_or("").as_bytes() {
            &[] => None,
            &[c] if c.is_ascii_alphanumeric() => Some(VIRTUAL_KEY(c as u16)),
            &[b'N' | b'n', _, _, _, _, _, c @ b'1'..=b'9'] if key[..6].eq_ignore_ascii_case(b"numpad") =>
                Some(VIRTUAL_KEY(vk::VK_NUMPAD0.0 + (c - b'0') as u16)),
            &[b'F' | b'f', c @ b'1'..=b'9'] => Some(VIRTUAL_KEY(vk::VK_F1.0 + (c - b'1') as u16)),
            &[b'F' | b'f', b'1', c @ b'0'..=b'9'] => Some(VIRTUAL_KEY(vk::VK_F10.0 + (c - b'0') as u16)),
            &[b'F' | b'f', b'2', c @ b'0'..=b'4'] => Some(VIRTUAL_KEY(vk::VK_F20.0 + (c - b'0') as u16)),
            k if k.eq_ignore_ascii_case(b"return") || k.eq_ignore_ascii_case(b"enter") =>
                Some(vk::VK_RETURN),
            k if k.eq_ignore_ascii_case(b"escape") || k.eq_ignore_ascii_case(b"esc") => Some(vk::VK_ESCAPE),
            k if k.eq_ignore_ascii_case(b"up") => Some(vk::VK_UP),
            k if k.eq_ignore_ascii_case(b"down") => Some(vk::VK_DOWN),
            k if k.eq_ignore_ascii_case(b"left") => Some(vk::VK_LEFT),
            k if k.eq_ignore_ascii_case(b"right") => Some(vk::VK_RIGHT),
            k if k.eq_ignore_ascii_case(b"tab") => Some(vk::VK_TAB),
            #[cfg(todo = "unnecessary")]
            k if k.eq_ignore_ascii_case(b"alt") => Some(vk::VK_MENU),
            #[cfg(todo = "unnecessary")]
            k if k.eq_ignore_ascii_case(b"ctrl") => Some(vk::VK_CONTROL),
            k if k.eq_ignore_ascii_case(b"control") => Some(vk::VK_CONTROL),
            #[cfg(todo = "unnecessary")]
            k if k.eq_ignore_ascii_case(b"shift") => Some(vk::VK_SHIFT),
            k if k.eq_ignore_ascii_case(b"lwin")
                || k.eq_ignore_ascii_case(b"win")
                || k.eq_ignore_ascii_case(b"windows")
                || k.eq_ignore_ascii_case(b"super") =>
                Some(vk::VK_LWIN),
            k if k.eq_ignore_ascii_case(b"rwin") => Some(vk::VK_RWIN),
            _ => None,
        };

        bind.vk = vk
            .or_else(|| key_str.and_then(Self::parse_vk_numeric))
            .ok_or_else(|| anyhow!("unknown key \"{}\"", key.escape_ascii()))?;

        Ok(bind)
    }

    pub fn parse_vk_numeric(input: &str) -> Option<VIRTUAL_KEY> {
        match input.strip_prefix("0x") {
            Some(hex) => u16::from_str_radix(hex, 16).ok(),
            None => input.parse().ok(),
        }
        .map(VIRTUAL_KEY)
    }

    pub fn display_addonapi(self) -> DisplayAddonApi<Self> {
        DisplayAddonApi(self)
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

#[cfg(feature = "arcdps-extras")]
impl From<KeyCode> for KeyInput {
    fn from(k: KeyCode) -> Self {
        Self::vk_down(Self::vk_from_extras(k))
    }
}
#[cfg(feature = "arcdps-extras")]
impl From<MouseCode> for KeyInput {
    fn from(k: MouseCode) -> Self {
        Self::vk_down(Self::vk_from_extras_button(k))
    }
}

#[cfg(feature = "arcdps-extras")]
impl TryFrom<KeybindChange> for KeyInput {
    type Error = anyhow::Error;

    fn try_from(key: KeybindChange) -> Result<Self, Self::Error> {
        use extras::Key;

        let mut mods = KeyState::from(&key);
        let code = match key.key {
            Key::Key(KeyCode::Colon) => {
                // this must refer to some other keycode but idk .-.
                mods.insert(KeyState::SHIFT);
                KeyCode::Semicolon
            },
            #[cfg(todo)]
            Key::Key(KeyCode::Hash) => {
                mods.insert(KeyState::SHIFT);
                KeyCode::Number3
            },
            Key::Key(code) => code,
            k => anyhow::bail!("not a keyboard binding: {k:?}"),
        };
        let mut input = Self::from(code);
        input.mods = mods;
        Ok(input)
    }
}
#[cfg(feature = "arcdps-extras")]
impl TryFrom<RawKeybindChange> for KeyInput {
    type Error = anyhow::Error;

    fn try_from(key: RawKeybindChange) -> Result<Self, Self::Error> {
        Self::try_from(keybind_change_from_raw(&key))
    }
}

/// Work around incorrect conversion in arcdps-rs (as of 2025-10)
#[cfg(feature = "arcdps-extras")]
pub fn keybind_change_from_raw(key: &RawKeybindChange) -> KeybindChange {
    use arcdps::extras::keybinds::Modifier;
    KeybindChange {
        control: key.control,
        index: key.index,
        key: key.key.clone().into(),
        mod_alt: key.key.modifier & Modifier::Alt as i32 != 0,
        mod_ctrl: key.key.modifier & Modifier::Ctrl as i32 != 0,
        mod_shift: key.key.modifier & Modifier::Shift as i32 != 0,
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
    pub const MODS: Self =
        Self::from_bits_retain(Self::SHIFT.bits() | Self::CTRL.bits() | Self::ALT.bits());
    pub const MODIFIERKEYS: Self =
        Self::from_bits_retain(Self::SHIFT.bits() | Self::CTRL.bits() | Self::ALT.bits());
    pub const BUTTON_LRM: Self =
        Self::from_bits_retain(Self::BUTTON_L.bits() | Self::BUTTON_R.bits() | Self::BUTTON_M.bits());
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

    pub const fn from_vk(vk: VIRTUAL_KEY) -> Self {
        Self::from_index(vk.0 as _)
    }
    pub const fn try_from_vk(vk: VIRTUAL_KEY) -> Option<Self> {
        match Self::from_vk(vk) {
            b if b.is_empty() => None,
            b => Some(b),
        }
    }

    pub const fn is_unique(self) -> bool {
        self.bits().count_ones() == 1
    }

    pub const fn vkeycode(self) -> VIRTUAL_KEY {
        match self.index() {
            index @ 0..=31 => VIRTUAL_KEY(index as u16),
            _ => KeyInput::VK_EMPTY,
        }
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

    pub const ALL_BUTTONS: [Self; 5] = [
        Self::BUTTON_L,
        Self::BUTTON_R,
        Self::BUTTON_M,
        Self::BUTTON_X1,
        Self::BUTTON_X2,
    ];
    pub const ALL_MODS: [Self; 3] = [Self::SHIFT, Self::CTRL, Self::ALT];

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
            (button, _) if !button.intersects(Self::BUTTON) => return None,
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
                Self::BUTTON_R => WindowsAndMessaging::WM_RBUTTONDOWN,
                Self::BUTTON_M => WindowsAndMessaging::WM_MBUTTONDOWN,
                _ => WindowsAndMessaging::WM_XBUTTONDOWN,
            },
            (b, false) if b.intersects(Self::BUTTON) => match b {
                Self::BUTTON_L => WindowsAndMessaging::WM_LBUTTONUP,
                Self::BUTTON_R => WindowsAndMessaging::WM_RBUTTONUP,
                Self::BUTTON_M => WindowsAndMessaging::WM_MBUTTONUP,
                _ => WindowsAndMessaging::WM_XBUTTONUP,
            },
            (k, down) if k.contains(Self::ALT) => match down {
                true => WindowsAndMessaging::WM_SYSKEYDOWN,
                false => WindowsAndMessaging::WM_SYSKEYUP,
            },
            (_, true) => WindowsAndMessaging::WM_KEYDOWN,
            (_, false) => WindowsAndMessaging::WM_KEYUP,
        }
    }

    pub const fn event_w(self, msg: u32) -> usize {
        match msg {
            WindowsAndMessaging::WM_XBUTTONUP => (self.button_x() << 16) as usize,
            WindowsAndMessaging::WM_SYSKEYDOWN
            | WindowsAndMessaging::WM_SYSKEYUP
            | WindowsAndMessaging::WM_KEYDOWN
            | WindowsAndMessaging::WM_KEYUP => self.vkeycode().0 as _,
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
            shift => Self::from_index_retain(shift),
        };
        Some(bit)
    }

    pub fn take_key(&mut self) -> Option<Self> {
        let key = self.next_key();
        if let Some(key) = key {
            self.remove(key);
        }
        key
    }

    pub fn iter_keys(mut self) -> impl Iterator<Item = Self> + Clone + Send + Sync + 'static {
        iter::from_fn(move || self.take_key())
    }

    pub fn vkeycodes(self) -> impl Iterator<Item = VIRTUAL_KEY> + Clone + Send + Sync + 'static {
        self.iter_keys().map(|flag| flag.vkeycode())
    }

    pub fn modifierkeys(self) -> impl Iterator<Item = MODIFIERKEYS_FLAGS> + Clone + Send + Sync + 'static {
        self.iter_keys().filter_map(|flag| flag.modifierkey())
    }

    pub fn to_modifierkeys(self) -> MODIFIERKEYS_FLAGS {
        MODIFIERKEYS_FLAGS(self.modifierkeys().map(|m| m.0).sum())
    }

    pub fn mouse_flags(
        self,
        down: bool,
    ) -> impl Iterator<Item = MOUSE_EVENT_FLAGS> + Clone + Send + Sync + 'static {
        self.iter_keys().filter_map(move |flag| flag.mouse_flag(down))
    }

    pub fn to_mouse_flags(self, down: bool) -> MOUSE_EVENT_FLAGS {
        MOUSE_EVENT_FLAGS(self.mouse_flags(down).map(|f| f.0).sum())
    }

    pub const fn as_name(self) -> &'static str {
        match self {
            Self::BUTTON_L => "left",
            Self::BUTTON_R => "right",
            Self::BUTTON_M => "middle",
            Self::BUTTON_X1 => "x1",
            Self::BUTTON_X2 => "x2",
            Self::SHIFT => "shift",
            Self::CTRL => "ctrl",
            Self::ALT => "alt",
            _ => "",
        }
    }
    pub const fn as_name_addonapi(self) -> &'static str {
        match self {
            Self::BUTTON_L => Self::ADDONAPI_LMB_ASCII,
            Self::BUTTON_R => Self::ADDONAPI_RMB_ASCII,
            Self::BUTTON_M => Self::ADDONAPI_MMB_ASCII,
            Self::BUTTON_X1 => Self::ADDONAPI_M4_ASCII,
            Self::BUTTON_X2 => Self::ADDONAPI_M5_ASCII,
            Self::SHIFT => Self::ADDONAPI_SHIFT_ASCII,
            Self::CTRL => Self::ADDONAPI_CTRL_ASCII,
            Self::ALT => Self::ADDONAPI_ALT_ASCII,
            Self::EMPTY => Self::ADDONAPI_EMPTY_ASCII,
            _ => "",
        }
    }
    pub fn display_addonapi(self) -> DisplayAddonApi<Self> {
        DisplayAddonApi(self)
    }

    pub const ADDONAPI_EMPTY_C: &'static CStr = c"(null)";
    pub const ADDONAPI_CTRL_C: &'static CStr = c"CTRL";
    pub const ADDONAPI_SHIFT_C: &'static CStr = c"SHIFT";
    pub const ADDONAPI_ALT_C: &'static CStr = c"ALT";
    pub const ADDONAPI_LMB_C: &'static CStr = c"LMB";
    pub const ADDONAPI_RMB_C: &'static CStr = c"LMB";
    pub const ADDONAPI_MMB_C: &'static CStr = c"MMB";
    pub const ADDONAPI_M4_C: &'static CStr = c"M4";
    pub const ADDONAPI_M5_C: &'static CStr = c"M5";
    pub const ADDONAPI_EMPTY_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_EMPTY_C.to_str() { s } else { unreachable!() };
    pub const ADDONAPI_CTRL_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_CTRL_C.to_str() { s } else { unreachable!() };
    pub const ADDONAPI_SHIFT_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_SHIFT_C.to_str() { s } else { unreachable!() };
    pub const ADDONAPI_ALT_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_ALT_C.to_str() { s } else { unreachable!() };
    pub const ADDONAPI_LMB_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_LMB_C.to_str() { s } else { unreachable!() };
    pub const ADDONAPI_RMB_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_RMB_C.to_str() { s } else { unreachable!() };
    pub const ADDONAPI_MMB_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_MMB_C.to_str() { s } else { unreachable!() };
    pub const ADDONAPI_M4_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_M4_C.to_str() { s } else { unreachable!() };
    pub const ADDONAPI_M5_ASCII: &'static str =
        if let Ok(s) = Self::ADDONAPI_M5_C.to_str() { s } else { unreachable!() };
    pub const MOD_SEP_ASCII: u8 = b'+';

    pub fn parse_bind_ascii(s: &[u8]) -> anyhow::Result<(Self, &[u8])> {
        let mut mods = Self::EMPTY;
        let mut split = s.splitn(4, |&b| b == Self::MOD_SEP_ASCII);
        let key = loop {
            let seg = match split.next() {
                Some(seg) if split.size_hint().0 == 0 => break seg,
                Some(seg) => seg,
                // should be unreachable...
                #[cfg(debug_assertions)]
                None => anyhow::bail!("expected segment"),
                #[cfg(not(debug_assertions))]
                None => break b"",
            };
            let modifier = Self::try_mod_from_ascii(seg)
                .with_context(|| format!("interpreting mod \"{}\"", seg.escape_ascii()))?;
            if mods.intersects(modifier) {
                log::warn!("redundant keybind \"{}\"", s.escape_ascii());
            }
            mods.insert(modifier);
        };

        Ok((mods, key))
    }

    pub fn mod_from_ascii(s: &[u8]) -> Option<Self> {
        match s {
            s if s.eq_ignore_ascii_case(Self::ADDONAPI_ALT_ASCII.as_bytes()) => Some(Self::ALT),
            s if s.eq_ignore_ascii_case(Self::ADDONAPI_CTRL_ASCII.as_bytes()) => Some(Self::CTRL),
            s if s.eq_ignore_ascii_case(Self::ADDONAPI_SHIFT_ASCII.as_bytes()) => Some(Self::SHIFT),
            _ => None,
        }
    }
    pub fn try_mod_from_ascii(s: &[u8]) -> anyhow::Result<Self> {
        Self::mod_from_ascii(s).ok_or_else(|| anyhow!("unknown modifier"))
    }

    pub fn button_from_ascii(s: &[u8]) -> Option<Self> {
        match s {
            s if s.is_empty() || s.eq_ignore_ascii_case(Self::ADDONAPI_EMPTY_ASCII.as_bytes()) =>
                Some(Self::EMPTY),
            s if s.eq_ignore_ascii_case(Self::ADDONAPI_LMB_ASCII.as_bytes()) => Some(Self::BUTTON_L),
            s if s.eq_ignore_ascii_case(Self::ADDONAPI_RMB_ASCII.as_bytes()) => Some(Self::BUTTON_R),
            s if s.eq_ignore_ascii_case(Self::ADDONAPI_MMB_ASCII.as_bytes()) => Some(Self::BUTTON_M),
            [b'M' | b'm', button @ b'1'..=b'5'] => Self::from_button_index((button - b'1') as usize),
            _ => None,
        }
    }

    pub fn from_ascii(s: &[u8]) -> Option<Self> {
        Self::button_from_ascii(s).or_else(|| Self::mod_from_ascii(s))
    }
}

#[cfg(feature = "arcdps-extras")]
impl TryFrom<MouseCode> for KeyState {
    type Error = anyhow::Error;

    fn try_from(m: MouseCode) -> Result<Self, Self::Error> {
        Self::from_button_index(m as _).ok_or_else(|| anyhow!("mouse button {m:?} beyond L/R/M/X1/X2"))
    }
}

#[cfg(feature = "arcdps-extras")]
impl From<KeybindChange> for KeyState {
    fn from(k: KeybindChange) -> Self {
        Self::from(&k)
    }
}

#[cfg(feature = "arcdps-extras")]
impl<'a> From<&'a KeybindChange> for KeyState {
    fn from(k: &'a KeybindChange) -> Self {
        [
            k.mod_ctrl.then_some(Self::CTRL),
            k.mod_shift.then_some(Self::SHIFT),
            k.mod_alt.then_some(Self::ALT),
        ]
        .into_iter()
        .collect()
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
impl fmt::Display for KeyState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, key) in self.iter().enumerate() {
            let prefix = (i > 0).then_some("+").unwrap_or("");
            let name = key.as_name();
            write!(f, "{prefix}{name}")?;
        }
        Ok(())
    }
}
pub struct DisplayAddonApi<K>(pub K);
impl fmt::Display for DisplayAddonApi<KeyState> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, key) in self.0.iter().enumerate() {
            let prefix = (i > 0).then_some("+").unwrap_or("");
            let name = key.as_name_addonapi();
            write!(f, "{prefix}{name}")?;
        }
        Ok(())
    }
}
impl fmt::Display for DisplayAddonApi<&'_ KeyState> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&DisplayAddonApi(*self.0), f)
    }
}
impl fmt::Display for DisplayAddonApi<KeyInput> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&DisplayAddonApi(&self.0), f)
    }
}
impl fmt::Display for DisplayAddonApi<&'_ KeyInput> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let has_mods = !self.0.mods.is_empty();
        if has_mods {
            fmt::Display::fmt(&self.0.mods.display_addonapi(), f)?;
        }
        if has_mods {
            f.write_str("+")?;
        }
        let vk = self.0.vk;
        if vk == KeyInput::VK_EMPTY {
            return f.write_str(KeyState::ADDONAPI_EMPTY_ASCII)
        }
        #[cfg(windows)]
        if let Ok(name) = vk_name(vk) {
            return fmt::Display::fmt(&name, f)
        }

        fmt::Display::fmt(&vk.0, f)
    }
}

pub fn scan_code(vk: VIRTUAL_KEY) -> Option<NonZeroU16> {
    let vsc = unsafe { KeyboardAndMouse::MapVirtualKeyA(vk.0.into(), KeyboardAndMouse::MAPVK_VK_TO_VSC) };
    NonZeroU16::new(vsc as u16)
}

pub fn key_char(vk: VIRTUAL_KEY) -> Option<NonZeroU16> {
    let vsc = unsafe { KeyboardAndMouse::MapVirtualKeyA(vk.0.into(), KeyboardAndMouse::MAPVK_VK_TO_CHAR) };
    NonZeroU16::new(vsc as u16)
}

pub fn key_name(code: LPARAM) -> windows::core::Result<windows::core::HSTRING> {
    let mut buf = [0u16; 128];
    let res = unsafe {
        match KeyboardAndMouse::GetKeyNameTextW(code.0 as i32, &mut buf) {
            0 => Err(windows::core::Error::from_win32()),
            sz => Ok(sz as usize),
        }
    };
    match res {
        Err(e) => Err(e),
        Ok(len @ 0..=128) => Ok(windows::core::HSTRING::from_wide(&buf[..len])),
        Ok(_res) => {
            log::debug!("weird, I didn't ask for {_res}");
            Err(windows::core::Error::new(
                windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER.to_hresult(),
                "key name too long",
            ))
        },
    }
}

pub fn vk_name(vk: VIRTUAL_KEY) -> windows::core::Result<windows::core::HSTRING> {
    let char_fastpath = key_char(vk).map(|c| {
        let b = [c.get()];
        windows::core::HSTRING::from_wide(&b)
    });
    if let Some(name) = char_fastpath {
        return Ok(name)
    }
    // TODO: ToUnicodeEx exists for this too?

    scan_code(vk)
        .ok_or_else(|| {
            windows::core::Error::new(
                windows::Win32::Foundation::ERROR_KEY_DOES_NOT_EXIST.to_hresult(),
                "scan code unknown",
            )
        })
        .and_then(|sc| key_name(scan_code_param(sc)))
}

pub fn scan_code_param(sc: NonZeroU16) -> LPARAM {
    LPARAM((sc.get() as usize as isize) << 16)
}

pub fn scan_code_key(vsc: NonZeroU16) -> Option<VIRTUAL_KEY> {
    let vk =
        unsafe { KeyboardAndMouse::MapVirtualKeyA(vsc.get().into(), KeyboardAndMouse::MAPVK_VSC_TO_VK) };
    NonZeroU16::new(vk as u16).map(|vk| vk.get()).map(VIRTUAL_KEY)
}

pub fn send_key_combo<I: Into<KeyInput>>(hwnd: HWND, input: I) -> anyhow::Result<()> {
    let input = input.into();
    do_key_combo(hwnd, move || send_key(hwnd, input), input)
}

pub fn do_key_combo<R, E, F: FnOnce() -> Result<R, E>>(
    hwnd: HWND,
    f: F,
    input: KeyInput,
) -> anyhow::Result<R>
where
    E: Into<anyhow::Error>,
{
    if input.down {
        // start by "holding" down any relevant modifiers
        let mod_inputs = input.mods.vkeycodes().map(KeyInput::vk_down);
        // (and releasing the inverse)
        let mod_unused = match input.mods.is_empty() {
            true => KeyState::EMPTY,
            false => input.mods_unused(),
        }
        .vkeycodes()
        .map(KeyInput::vk_up);
        for mod_input in mod_unused.chain(mod_inputs) {
            send_key(hwnd, mod_input)?;
        }
    }

    let mut res = f().map_err(Into::into);

    if !input.down {
        // release modifiers afterward
        for mod_input in input.mods.vkeycodes().map(KeyInput::vk_up) {
            if let Err(e) = send_key(hwnd, mod_input) {
                if res.is_ok() {
                    res = Err(e);
                }
            }
        }
    }

    res
}

pub fn send_key(hwnd: HWND, input: KeyInput) -> anyhow::Result<()> {
    let (msg, w, l) = input.to_event();
    unsafe { window_message(hwnd, msg, w, l) }
}

pub fn send_key_input<I: Into<KeyInput>>(hwnd: HWND, input: I) -> anyhow::Result<()> {
    let input = input.into();
    let mods = input.mods;
    let mod_inputs = input
        .down
        .then({
            let mods_unused = match mods.is_empty() {
                true => KeyState::EMPTY,
                false => input.mods_unused(),
            };
            move || {
                mods.vkeycodes()
                    .map(KeyInput::vk_down)
                    .chain(mods_unused.vkeycodes().map(KeyInput::vk_up))
            }
        })
        .into_iter()
        .flatten();

    let mod_release = (!input.down)
        .then(move || mods.vkeycodes().map(KeyInput::vk_up))
        .into_iter()
        .flatten();

    let inputs = mod_inputs.chain(iter::once(input.into())).chain(mod_release);
    window_send_inputs(hwnd, inputs)
}
