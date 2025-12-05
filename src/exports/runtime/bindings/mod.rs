use {
    crate::settings::state::SaveState,
    arcdps::extras::keybinds::KeybindChange,
    bitflags::bitflags,
    bitvec::array::BitArray,
    std::{
        collections::BTreeMap,
        marker::PhantomData,
        sync::{
            atomic::{AtomicU64, Ordering},
            LazyLock,
        },
    },
    taimi_input::win::keyboard::{KeyInput, KeyState},
    windows::Win32::UI::{
        Input::KeyboardAndMouse::{self, VIRTUAL_KEY},
        WindowsAndMessaging,
    },
};

pub use self::{
    controls::{Control, ControlSlot, GameBinds, GameControl, GameControls},
    held::{ControlsReceiver, HeldControls, TaimiReceiver},
    intercept::KeyIntercept,
    keys::KeyPresses,
};

pub mod controls;
pub mod held;
pub mod intercept;
pub mod keys;

pub static CONTROLS: LazyLock<HeldControls> = LazyLock::new(|| HeldControls::new(interesting_controls()));
pub(crate) fn read_game_binds<R, F: FnOnce(&GameBinds) -> R>(f: F) -> R {
    SaveState::read_with(|s| f(&s.game_binds))
}
pub(crate) fn write_game_binds<F: FnOnce(&mut GameBinds)>(f: F) {
    SaveState::write_with(|s| f(&mut s.game_binds_mut()))
}
#[cfg(todo)]
pub static GAME_BINDS: RwLock<GameBinds> = RwLock::new(GameBinds::new());
/// TODO: any need to bring this back?
fn try_read_game_binds<R, F: FnOnce(&GameBinds) -> R>(f: F) -> Option<R> {
    Some(read_game_binds(f))
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
    pub struct TaimiControls: u32 {
        const WINDOW_PRIMARY = 0x0001;
        #[cfg(feature = "markers")]
        const WINDOW_MARKERS = 0x0002;
        #[cfg(feature = "timers")]
        const WINDOW_TIMERS = 0x0004;
        #[cfg(feature = "space")]
        const WINDOW_PATHING = 0x0008;

        const UNASSIGNED_4 = 0x0010;
        const UNASSIGNED_5 = 0x0020;
        const UNASSIGNED_6 = 0x0040;
        const UNASSIGNED_7 = 0x0080;

        #[cfg(feature = "timers")]
        const TIMER_TRIGGER_0 = 0x0100;
        #[cfg(feature = "timers")]
        const TIMER_TRIGGER_1 = 0x0200;
        #[cfg(feature = "timers")]
        const TIMER_TRIGGER_2 = 0x0400;
        #[cfg(feature = "timers")]
        const TIMER_TRIGGER_3 = 0x0800;
        #[cfg(feature = "timers")]
        const TIMER_TRIGGER_4 = 0x1000;
        #[cfg(feature = "timers")]
        const TIMER_RESET = 0x2000;

        const UNASSIGNED_14 = 0x4000;
        const UNASSIGNED_15 = 0x8000;

        #[cfg(feature = "space")]
        const PATHING_SPACE = 0x0001_0000;
        #[cfg(feature = "space")]
        const PATHING_MINIMAP = 0x0002_0000;
        #[cfg(feature = "space")]
        const PATHING_MAP = 0x0004_0000;
    }
}

impl TaimiControls {
    #[allow(unreachable_patterns)]
    pub const WINDOW_TOGGLES: Self = Self::from_bits_retain({
        #[rustfmt::skip]
        let bits = Self::WINDOW_PRIMARY.bits()
            | match () {
                #[cfg(feature = "timers")]
                _ => Self::WINDOW_TIMERS.bits(),
                _ => 0,
            }
            | match () {
                #[cfg(feature = "markers")]
                _ => Self::WINDOW_MARKERS.bits(),
                _ => 0,
            }
            | match () {
                #[cfg(feature = "space")]
                _ => Self::WINDOW_PATHING.bits(),
                _ => 0,
            };
        bits
    });

    #[cfg(feature = "timers")]
    pub const TIMER_TRIGGERS: Self = Self::from_bits_retain(
        Self::TIMER_TRIGGER_0.bits()
            | Self::TIMER_TRIGGER_1.bits()
            | Self::TIMER_TRIGGER_2.bits()
            | Self::TIMER_TRIGGER_3.bits()
            | Self::TIMER_TRIGGER_4.bits(),
    );

    #[cfg(feature = "space")]
    pub const PATHING_TOGGLES: Self = Self::from_bits_retain(
        Self::PATHING_SPACE.bits() | Self::PATHING_MAP.bits() | Self::PATHING_MINIMAP.bits(),
    );

    #[allow(unreachable_patterns)]
    pub const QUICK_ACCESS_ICONS: Self = Self::from_bits_retain(
        Self::WINDOW_TOGGLES.bits()
            | match () {
                #[cfg(feature = "space")]
                _ => Self::PATHING_TOGGLES.bits(),
                _ => 0,
            },
    );

    pub fn index(self) -> u8 {
        self.bits().trailing_zeros() as u8
    }

    /// Either a single bit, or [Self::empty()]
    pub const fn from_index(index: u8) -> Self {
        Self::from_bits_retain(1u32.unbounded_shl(index as _))
    }

    pub fn to_vk_dummy(self) -> VIRTUAL_KEY {
        // TODO: use reserved and/or OEM ranges properly?
        // VIRTUAL_KEY(KeyboardAndMouse::VK_F17.0 + self.index() as u16)
        // .. or just go beyond 8 bits
        VIRTUAL_KEY(0x100 + self.index() as u16)
    }

    pub(crate) fn default_quick_access() -> Self {
        Self::WINDOW_TOGGLES | Self::PATHING_SPACE
    }
    pub(crate) fn is_default_quick_access(&self) -> bool {
        *self == Self::default_quick_access()
    }
}

impl serde::Serialize for TaimiControls {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
    }
}
impl<'de> serde::Deserialize<'de> for TaimiControls {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64::deserialize(deserializer).map(|v| {
            if v > u32::MAX as u64 {
                log::info!("unexpected taimi controls {v:#018x}, maybe version reverted?");
            }
            Self::from_bits_truncate(v as u32)
        })
    }
}

pub static DEFAULT_GAMEBINDS: LazyLock<BTreeMap<Control, KeyInput>> =
    LazyLock::new(|| self::controls::default_gamebinds());

/// we don't care about detecting these, only simulating them...
fn is_interesting_bind(control: Control) -> bool {
    let Some(control) = control.as_control() else {
        // if we don't know about it then we don't actually care
        return false
    };
    match control {
        GameControl::Squad_Location_X
        | GameControl::Squad_Location_Star
        | GameControl::Squad_Location_Arrow
        | GameControl::Squad_Location_Heart
        | GameControl::Squad_Location_Circle
        | GameControl::Squad_Location_Square
        | GameControl::Squad_Location_Spiral
        | GameControl::Squad_Location_Triangle => true,
        #[cfg(todo)]
        GameControl::Squad_Object_X
        | GameControl::Squad_Object_Star
        | GameControl::Squad_Object_Arrow
        | GameControl::Squad_Object_Heart
        | GameControl::Squad_Object_Circle
        | GameControl::Squad_Object_Square
        | GameControl::Squad_Object_Spiral
        | GameControl::Squad_Object_Triangle => true,
        _ => false,
    }
}
fn is_interesting(control: Control) -> bool {
    let Some(control) = control.as_control() else { return false };
    match control {
        GameControl::Miscellaneous_Interact
        | GameControl::Map_OpenClose
        | GameControl::UI_ShowHideUI
        | GameControl::Squad_ClearAllObjectMarkers
        | GameControl::Squad_ClearAllLocationMarkers => true,
        #[cfg(todo)]
        GameControl::Map_FloorUp
        | GameControl::Map_FloorDown
        | GameControl::Map_ZoomIn
        | GameControl::Map_ZoomOut
        | GameControl::Map_Recenter => true,
        _ => false,
    }
}
pub fn interesting_controls() -> GameControls {
    GameControls::all_controls()
        .map(Control::from)
        .filter(|&c| is_interesting(c))
        .collect()
}
pub fn interesting_keybinds() -> GameControls {
    GameControls::all_controls()
        .map(Control::from)
        .filter(|&c| is_interesting_bind(c))
        .collect()
}

fn notify_interesting(keyboard: bool, vk: VIRTUAL_KEY, down: bool) {
    let interested = CONTROLS.is_interested_in_key(vk);
    #[cfg(todo = "unnecessary")]
    let interested = interested || !down;
    if !interested {
        return
    }
    match down {
        false => CONTROLS.notify_release(vk),
        true => {
            let bind = try_read_game_binds(|binds| {
                let mods = held_mods();
                match keyboard {
                    true => binds.key_binds.get(&(vk.0, mods)),
                    false => binds.mouse_binds.get(&(vk.0, mods)),
                }
                .copied()
            })
            .flatten();
            if let Some(bind) = bind {
                CONTROLS.notify_press(vk, bind);
            }
        },
    }
}

pub fn process_key_bound(change: KeybindChange) {
    let interesting = CONTROLS.is_interested_in_control(change.control.into());
    #[cfg(todo)]
    if !interesting {
        return
    }

    let mut interesting_keys = None;
    write_game_binds(|binds| {
        binds.process_update(&change);
        if !interesting {
            return
        }

        // TODO: these should be delayed since we expect to receive a lot of these events at startup...
        interesting_keys = Some(collect_bind_controls(binds));
    });
    let Some(interesting_keys) = interesting_keys else { return };
    CONTROLS.set_interesting_keys(interesting_keys);
}

pub fn collect_bind_controls(binds: &GameBinds) -> KeyPresses {
    CONTROLS.collect_interesting_keys(
        binds
            .key_binds
            .iter()
            .chain(binds.mouse_binds.iter())
            .map(|(&(vk, _), &slot)| (slot, VIRTUAL_KEY(vk))),
    )
}

/// Initialize bind interest
pub fn populate_bind_controls() {
    let mut interesting_keys = None;
    read_game_binds(|binds| {
        interesting_keys = Some(collect_bind_controls(binds));
    });
    if let Some(interesting_keys) = interesting_keys {
        CONTROLS.set_interesting_keys(interesting_keys);
    }
}

pub static HELD_KEYS: BitArray<[AtomicU64; keys::KEY_PRESS_BITS / 64], bitvec::order::Lsb0> = BitArray {
    data: [const { AtomicU64::new(0) }; keys::KEY_PRESS_BITS / 64],
    _ord: PhantomData,
};

pub fn process_key_event(msg: u32, w: usize, l: isize) -> u32 {
    #[cfg(todo)]
    let prev_down = l & (1 << 30) != 0;
    let repeat = l & 0xff;
    let is_up = matches!(
        msg,
        WindowsAndMessaging::WM_KEYUP | WindowsAndMessaging::WM_SYSKEYUP
    );
    #[cfg(todo)]
    let is_trigger = !is_up && repeat == 1;
    #[cfg(todo)]
    let is_release = is_up && prev_down;

    if !is_up && repeat != 1 {
        return msg
    }
    let down = !is_up;

    let vk = match w {
        w @ 1..=0xfe => match VIRTUAL_KEY(w as u16) {
            // TODO: normalize mods elsewhere...
            KeyboardAndMouse::VK_LMENU | KeyboardAndMouse::VK_RMENU => KeyboardAndMouse::VK_MENU,
            KeyboardAndMouse::VK_LSHIFT | KeyboardAndMouse::VK_RSHIFT => KeyboardAndMouse::VK_SHIFT,
            KeyboardAndMouse::VK_LCONTROL | KeyboardAndMouse::VK_RCONTROL => KeyboardAndMouse::VK_CONTROL,
            w => w,
        },
        _ => {
            log::trace!("ignoring vk {w}");
            return msg
        },
    };
    HELD_KEYS.set_aliased(vk.0 as usize, down);

    let mods = KeyState::from_index(vk.0.into());
    if down && mods.is_empty() {
        let key = KeyInput { down, vk, mods: held_mods() };
        if KeyIntercept::intercept_try_report(key) {
            return 0
        }
    }
    notify_interesting(true, vk, down);

    msg
}

pub fn process_button_event(msg: u32, w: usize, _l: isize) -> u32 {
    let (button, down) = match msg {
        WindowsAndMessaging::WM_LBUTTONUP => (KeyboardAndMouse::VK_LBUTTON, false),
        WindowsAndMessaging::WM_LBUTTONDOWN => (KeyboardAndMouse::VK_LBUTTON, true),
        WindowsAndMessaging::WM_RBUTTONUP => (KeyboardAndMouse::VK_RBUTTON, false),
        WindowsAndMessaging::WM_RBUTTONDOWN => (KeyboardAndMouse::VK_RBUTTON, true),
        WindowsAndMessaging::WM_MBUTTONUP => (KeyboardAndMouse::VK_MBUTTON, false),
        WindowsAndMessaging::WM_MBUTTONDOWN => (KeyboardAndMouse::VK_MBUTTON, true),
        WindowsAndMessaging::WM_XBUTTONUP | WindowsAndMessaging::WM_XBUTTONDOWN => (
            match (w >> 16) as u16 {
                WindowsAndMessaging::XBUTTON1 => KeyboardAndMouse::VK_XBUTTON1,
                WindowsAndMessaging::XBUTTON2 => KeyboardAndMouse::VK_XBUTTON2,
                _ => return msg,
            },
            msg == WindowsAndMessaging::WM_XBUTTONDOWN,
        ),
        _ => return msg,
    };
    HELD_KEYS.set_aliased(button.0 as usize, down);
    if down {
        let key = KeyInput { down, vk: button, mods: held_mods() };
        if KeyIntercept::intercept_try_report(key) {
            return 0
        }
    }
    notify_interesting(false, button, down);

    msg
}

pub fn held_mods() -> KeyState {
    // Lsb0 is the default/recommended bitvec ordering
    let bits0 = HELD_KEYS.data[0].load(Ordering::Relaxed);
    // TODO: handle or normalize LCTRL vs RCTRL etc
    KeyState::from_bits_retain(bits0 as u32 & KeyState::MODS.bits())
}
