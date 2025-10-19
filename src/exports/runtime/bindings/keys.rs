use {
    arcdps::extras::keybinds::{Key, KeybindChange, KeyCode, MouseCode},
    crate::exports::runtime::bindings::ControlSlot,
    bitvec::array::BitArray,
    std::{
        collections::BTreeMap,
    },
    taimi_input::win::keyboard::KeyState,
    windows::Win32::UI::Input::KeyboardAndMouse::{self, VIRTUAL_KEY},
};

pub const KEY_PRESS_BITS: usize = 256;
pub type KeyPresses = BitArray<[u64; KEY_PRESS_BITS / 64]>;

pub type KeyBind = (u16, KeyState);
#[cfg(todo)]
pub type MouseBind = (MouseCode, KeyState);
pub type MouseBind = KeyBind;

pub struct GameBinds {
    pub key_binds: BTreeMap<KeyBind, ControlSlot>,
    pub mouse_binds: BTreeMap<MouseBind, ControlSlot>,
}

impl GameBinds {
    pub const fn new() -> Self {
        Self {
            key_binds: BTreeMap::new(),
            mouse_binds: BTreeMap::new(),
        }
    }

    pub fn process_update(&mut self, change: &KeybindChange) {
        let mods = KeyState::from(change);
        let slot = (change.control, change.index as _);

        // empty the prior binding on this slot
        self.key_binds.retain(|_, &mut b| b != slot);
        self.mouse_binds.retain(|_, &mut b| b != slot);

        match change.key {
            Key::Mouse(mouse) => {
                let vk = Self::vk_for_button(mouse);
                self.mouse_binds.insert((vk.0, mods), slot);
            },
            Key::Key(key) => {
                let vk = Self::vk_for_key(key);
                self.key_binds.insert((vk.0, mods), slot);
            },
            // 0 indicates unbound
            Key::Unknown(0) => (),
            Key::Unknown(code) => {
                log::warn!("unrecognized keybind code {code} for control {}", i32::from(change.control));
            },
        }
    }

    pub fn key_event(&self, mods: KeyState, vk: VIRTUAL_KEY) -> Option<&ControlSlot> {
        self.key_binds.get(&(vk.0, mods))
    }

    pub fn mouse_event(&self, mods: KeyState, button: VIRTUAL_KEY) -> Option<&ControlSlot> {
        self.mouse_binds.get(&(button.0, mods))
    }

    pub fn vk_for_key(key: KeyCode) -> VIRTUAL_KEY {
        match key {
            #[cfg(todo)]
            KeyCode::LeftAlt => KeyboardAndMouse::VK_LMENU,
            #[cfg(todo)]
            KeyCode::LeftCtrl => KeyboardAndMouse::VK_LCONTROL,
            #[cfg(todo)]
            KeyCode::LeftShift => KeyboardAndMouse::VK_LSHIFT,
            KeyCode::LeftAlt | KeyCode::RightAlt => KeyboardAndMouse::VK_MENU,
            KeyCode::LeftCtrl | KeyCode::RightCtrl => KeyboardAndMouse::VK_CONTROL,
            KeyCode::LeftShift | KeyCode::RightShift => KeyboardAndMouse::VK_SHIFT,
            KeyCode::Quote => KeyboardAndMouse::VK_OEM_7,
            // KeyCode::Hash => KeyboardAndMouse::VK_POUN, ?
            KeyCode::CapsLock => KeyboardAndMouse::VK_CAPITAL,
            // TODO: lots .-.
            KeyCode::ArrowDown => KeyboardAndMouse::VK_DOWN,
            KeyCode::ArrowLeft => KeyboardAndMouse::VK_LEFT,
            KeyCode::ArrowRight => KeyboardAndMouse::VK_RIGHT,
            KeyCode::ArrowUp => KeyboardAndMouse::VK_UP,
            k if k >= KeyCode::F1 && k <= KeyCode::F12 =>
                VIRTUAL_KEY(KeyboardAndMouse::VK_F1.0 + (
                    k as i32 - KeyCode::F1 as i32
                ) as u16),
            k => VIRTUAL_KEY(k as i32 as u16),
        }
    }

    pub fn vk_for_button(button: MouseCode) -> VIRTUAL_KEY {
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
}
