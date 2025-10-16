use {
    arcdps::extras::keybinds::{Key, KeybindChange, KeyCode, MouseCode},
    bitflags::bitflags,
    bitvec::array::BitArray,
    std::{
        collections::BTreeMap,
        fmt,
        marker::PhantomData,
        mem,
        ops,
        sync::{
            atomic::{AtomicU64, Ordering},
            LazyLock, RwLock,
        },
    },
    taimi_input::win::keyboard::{KeyState, KeyInput},
    tokio::sync::watch,
    windows::Win32::UI::{
        Input::KeyboardAndMouse::{self, VIRTUAL_KEY},
        WindowsAndMessaging,
    },
};
pub use arcdps::extras::keybinds::Control as GameControl;

const GAME_CONTROL_BITS: usize = 256;
pub type GameControlsBits = [u64; GAME_CONTROL_BITS / 64];

#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameControls {
    pub bits: BitArray<GameControlsBits>,
}
impl GameControls {
    pub const fn new(bits: BitArray<GameControlsBits>) -> Self {
        Self {
            bits,
        }
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = GameControl> + Clone + 'a {
        self.bits.iter_ones()
            .filter_map(|i| GameControl::try_from(i as i32).ok())
    }

    pub fn into_iter(self) -> impl Iterator<Item = GameControl> + Clone + 'static {
        self.bits.into_iter().enumerate()
            .filter_map(|(i, set)| match set {
                true => GameControl::try_from(i as i32).ok(),
                false => None,
            })
    }

    pub fn contains(&self, control: GameControl) -> bool {
        let i = i32::from(control) as usize;
        unsafe {
            *self.bits.get_unchecked(i)
        }
    }

    pub fn set(&mut self, control: GameControl, set: bool) {
        let i = i32::from(control) as usize;
        unsafe {
            self.bits.set_unchecked(i, set);
        }
    }

    #[cfg(not(test))]
    pub fn all_controls() -> impl Iterator<Item = GameControl> + Clone {
        (0i32..0x100).filter_map(|i| GameControl::try_from(i).ok())
    }
    #[cfg(test)]
    pub fn all_controls() -> impl Iterator<Item = GameControl> + Clone {
        (0i32..0x10000).filter_map(|i| GameControl::try_from(i).ok())
    }
}
impl fmt::Debug for GameControls {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("GameControls")
            .field(&&self.iter().collect::<Vec<_>>()[..])
            .finish()
    }
}
impl ops::Deref for GameControls {
    type Target = BitArray<GameControlsBits>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.bits
    }
}
impl ops::DerefMut for GameControls {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bits
    }
}
impl ops::Not for GameControls {
    type Output = Self;
    fn not(mut self) -> Self::Output {
        self.bits = !self.bits;
        self
    }
}
impl ops::BitXor for GameControls {
    type Output = Self;
    fn bitxor(mut self, rhs: Self) -> Self::Output {
        self.bits ^= rhs.bits;
        self
    }
}
impl ops::BitXorAssign for GameControls {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.bits ^= rhs.bits;
    }
}
impl ops::BitXorAssign<&Self> for GameControls {
    fn bitxor_assign(&mut self, rhs: &Self) {
        self.bits ^= rhs.bits;
    }
}
impl ops::BitAnd for GameControls {
    type Output = Self;
    fn bitand(mut self, rhs: Self) -> Self::Output {
        self.bits &= rhs.bits;
        self
    }
}
impl ops::BitAnd<&Self> for GameControls {
    type Output = Self;
    fn bitand(mut self, rhs: &Self) -> Self::Output {
        self.bits &= rhs.bits;
        self
    }
}
impl ops::BitOr for GameControls {
    type Output = Self;
    fn bitor(mut self, rhs: Self) -> Self::Output {
        self.bits |= rhs.bits;
        self
    }
}
impl ops::BitOr<&Self> for GameControls {
    type Output = Self;
    fn bitor(mut self, rhs: &Self) -> Self::Output {
        self.bits |= rhs.bits;
        self
    }
}
// TODO: BitAndAssign, BitOrAssign

pub type ControlSlot = (GameControl, u8);

#[test]
fn game_control_bitarray() {
    for control in GameControls::all_controls() {
        let index: i32 = control.into();
        assert!(index < GAME_CONTROL_BITS as i32);
    }
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
    pub const WINDOW_TOGGLES: Self = Self::from_bits_retain(
        Self::WINDOW_PRIMARY.bits()
        | match () {
            #[cfg(feature = "timers")]
            _ => Self::WINDOW_TIMERS.bits(),
            _ => 0,
        } | match () {
            #[cfg(feature = "markers")]
            _ => Self::WINDOW_MARKERS.bits(),
            _ => 0,
        } | match () {
            #[cfg(feature = "space")]
            _ => Self::WINDOW_PATHING.bits(),
            _ => 0,
        }
    );

    #[cfg(feature = "timers")]
    pub const TIMER_TRIGGERS: Self = Self::from_bits_retain(
        Self::TIMER_TRIGGER_0.bits()
        | Self::TIMER_TRIGGER_1.bits()
        | Self::TIMER_TRIGGER_2.bits()
        | Self::TIMER_TRIGGER_3.bits()
        | Self::TIMER_TRIGGER_4.bits()
    );

    #[cfg(feature = "space")]
    pub const PATHING_TOGGLES: Self = Self::from_bits_retain(
        Self::PATHING_SPACE.bits()
        | Self::PATHING_MAP.bits()
        | Self::PATHING_MINIMAP.bits()
    );

    #[allow(unreachable_patterns)]
    pub const QUICK_ACCESS_ICONS: Self = Self::from_bits_retain(
        Self::WINDOW_TOGGLES.bits()
        | match () {
            #[cfg(feature = "space")]
            _ => Self::PATHING_TOGGLES.bits(),
            _ => 0,
        }
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
        u32::deserialize(deserializer).map(Self::from_bits_retain)
    }
}

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

pub static GAME_BINDS: RwLock<GameBinds> = RwLock::new(GameBinds::new());

fn is_interesting(control: GameControl) -> bool {
    match control {
        // we don't care about detecting these, only simulating them...
        #[cfg(todo = "unnecessary")]
        GameControl::Squad_Object_X | GameControl::Squad_Location_X
        | GameControl::Squad_Object_Star | GameControl::Squad_Location_Star
        | GameControl::Squad_Object_Arrow | GameControl::Squad_Location_Arrow
        | GameControl::Squad_Object_Heart | GameControl::Squad_Location_Heart
        | GameControl::Squad_Object_Circle | GameControl::Squad_Location_Circle
        | GameControl::Squad_Object_Square | GameControl::Squad_Location_Square
        | GameControl::Squad_Object_Spiral | GameControl::Squad_Location_Spiral
        | GameControl::Squad_Object_Triangle | GameControl::Squad_Location_Triangle
        => true,
        GameControl::Miscellaneous_Interact | GameControl::Map_OpenClose
        | GameControl::UI_ShowHideUI
        | GameControl::Map_ZoomIn | GameControl::Map_ZoomOut | GameControl::Map_Recenter
        | GameControl::Map_FloorUp | GameControl::Map_FloorDown
        | GameControl::Squad_ClearAllObjectMarkers | GameControl::Squad_ClearAllLocationMarkers
        => true,
        _ => false,
    }
}
fn interesting_controls() -> GameControls {
    HeldControls::collect_controls(GameControls::all_controls().filter(|&c| is_interesting(c)))
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WatcherSlot {
    Control {
        control: GameControl,
        index: u8,
    },
    Taimi {
        index: u8,
    },
}

impl WatcherSlot {
    pub fn control(&self) -> Option<GameControl> {
        match self {
            &Self::Control { control, .. } => Some(control),
            _ => None,
        }
    }

    /// Either a single bit, or [TaimiControls::empty()]
    pub fn taimi(&self) -> TaimiControls {
        match self {
            &Self::Taimi { index } => TaimiControls::from_index(index),
            _ => TaimiControls::empty(),
        }
    }
}

impl From<ControlSlot> for WatcherSlot {
    fn from((control, index): ControlSlot) -> Self {
        Self::Control {
            control,
            index,
        }
    }
}
impl From<TaimiControls> for WatcherSlot {
    fn from(controls: TaimiControls) -> Self {
        Self::Taimi {
            index: controls.index(),
        }
    }
}
pub const KEY_PRESS_BITS: usize = 256;
pub type KeyPresses = BitArray<[u64; KEY_PRESS_BITS / 64]>;

/// TODO: BTreeSet would be fineish
pub type HeldControlsState = BTreeMap<WatcherSlot, u16>;
pub struct HeldControls {
    pub controls: watch::Sender<HeldControlsState>,
    pub interesting_keys: RwLock<KeyPresses>,
    pub interesting_controls: GameControls,
}

impl HeldControls {
    pub fn new(interesting_controls: GameControls) -> Self {
        Self {
            controls: watch::Sender::new(BTreeMap::new()),
            interesting_keys: RwLock::new(Default::default()),
            interesting_controls,
        }
    }

    pub fn is_interested_in_control(&self, control: GameControl) -> bool {
        self.interesting_controls.contains(control)
    }

    pub fn is_interested_in_key(&self, vk: VIRTUAL_KEY) -> bool {
        self.interesting_keys.read().ok()
            .and_then(|interesting| interesting.get(vk.0 as usize)
                .map(|b| *b)
            ).unwrap_or(false)
    }

    pub fn notify_release(&self, vk: VIRTUAL_KEY) {
        self.controls.send_if_modified(|controls| {
            let prev_len = controls.len();
            controls.retain(|_, &mut heldvk| heldvk != vk.0);
            prev_len != controls.len()
        });
    }

    pub fn notify_press<C: Into<WatcherSlot>>(&self, vk: VIRTUAL_KEY, control: C) {
        self.controls.send_modify(|controls| {
            // consider storing controls twice with slot index 0xff to track/ignore overlap better..?
            let control = control.into();
            let _prev = controls.insert(control, vk.0);
            if _prev.is_some() {
                log::debug!("held control {control:?} double-pressed with {vk:?}");
            }
        });
    }

    pub fn held_controls(controls: &HeldControlsState) -> GameControls {
        Self::collect_controls(controls.keys().filter_map(|&slot| slot.control()))
    }

    pub fn taimi_controls(controls: &HeldControlsState) -> TaimiControls {
        controls.keys().map(|&slot| slot.taimi()).collect()
    }

    pub fn collect_interesting_keys<B>(&self, binds: B) -> KeyPresses where
        B: IntoIterator<Item = (ControlSlot, VIRTUAL_KEY)>,
    {
        let mut interesting_binds = KeyPresses::default();
        for ((control, _index), vk) in binds {
            if !self.is_interested_in_control(control) { continue }
            if vk.0 == 0 || vk.0 >= 0xff { continue }
            unsafe {
                interesting_binds.set_unchecked(vk.0 as usize, true);
            }
        }
        interesting_binds
    }

    pub fn collect_controls<C>(controls: C) -> GameControls where
        C: IntoIterator<Item = GameControl>,
    {
        let mut interesting_controls = GameControls::default();
        for control in controls {
            interesting_controls.set(control, true);
        }
        interesting_controls
    }

    pub fn set_interesting_keys(&self, interesting_keys: KeyPresses) {
        if let Ok(mut out) = self.interesting_keys.write() {
            *out = interesting_keys;
        }
    }

    pub fn subscribe_controls(&self) -> ControlsReceiver {
        ControlsReceiver::new(self.controls.subscribe())
    }

    pub fn subscribe_taimi(&self) -> TaimiReceiver {
        TaimiReceiver::new(self.controls.subscribe())
    }
}

pub static CONTROLS: LazyLock<HeldControls> = LazyLock::new(|| HeldControls::new(interesting_controls()));

#[derive(Clone)]
pub struct ControlsReceiver {
    pub prev: GameControls,
    pub receiver: watch::Receiver<HeldControlsState>,
}

impl ControlsReceiver {
    pub fn new(receiver: watch::Receiver<HeldControlsState>) -> Self {
        Self {
            prev: Default::default(),
            receiver,
        }
    }

    pub fn mark_unchanged(&mut self) {
    }

    pub fn current(&self) -> &GameControls {
        &self.prev
    }

    #[cfg(todo = "unused")]
    pub fn latest(&self) -> GameControls {
        HeldControls::held_controls(&self.receiver.borrow())
    }

    pub async fn wait<'a>(&'a mut self) -> Result<(&'a GameControls, GameControls), watch::error::RecvError> {
        let mut latest = Default::default();
        let prev = &mut self.prev;
        self.receiver.wait_for(|held| {
            latest = HeldControls::held_controls(held);
            let prev = mem::replace(prev, latest);
            latest ^= prev;
            !latest.is_empty()
        }).await?;
        Ok((&*prev, latest))
    }

    pub fn update<'a>(&'a mut self) -> Option<(&'a GameControls, GameControls)> {
        let mut latest = HeldControls::held_controls(&*self.receiver.borrow_and_update());
        let prev = mem::replace(&mut self.prev, latest);
        latest ^= prev;
        (!latest.is_empty()).then_some((&self.prev, latest))
    }
}

impl fmt::Debug for ControlsReceiver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ControlsReceiver")
            .field("prev", &self.prev)
            .finish()
    }
}

#[derive(Clone)]
pub struct TaimiReceiver {
    pub prev: TaimiControls,
    pub receiver: watch::Receiver<HeldControlsState>,
}

impl TaimiReceiver {
    pub fn new(receiver: watch::Receiver<HeldControlsState>) -> Self {
        Self {
            prev: Default::default(),
            receiver,
        }
    }

    pub fn mark_unchanged(&mut self) {
    }

    pub fn current(&self) -> &TaimiControls {
        &self.prev
    }

    #[cfg(todo = "unused")]
    pub fn latest(&self) -> TaimiControls {
        HeldControls::taimi_controls(&self.receiver.borrow())
    }

    pub async fn wait(&mut self) -> Result<(TaimiControls, TaimiControls), watch::error::RecvError> {
        let mut latest = Default::default();
        let prev = &mut self.prev;
        self.receiver.wait_for(|held| {
            latest = HeldControls::taimi_controls(held);
            let prev = mem::replace(prev, latest);
            latest ^= prev;
            !latest.is_empty()
        }).await?;
        Ok((*prev, latest))
    }

    pub fn update(&mut self) -> Option<(TaimiControls, TaimiControls)> {
        let mut latest = HeldControls::taimi_controls(&*self.receiver.borrow_and_update());
        let prev = mem::replace(&mut self.prev, latest);
        latest ^= prev;
        (!latest.is_empty()).then_some((self.prev, latest))
    }
}

impl fmt::Debug for TaimiReceiver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaimiReceiver")
            .field("prev", &self.prev)
            .finish()
    }
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
            let bind = if let Ok(binds) = GAME_BINDS.try_read() {
                let mods = held_mods();
                match keyboard {
                    true => binds.key_binds.get(&(vk.0, mods)),
                    false => binds.mouse_binds.get(&(vk.0, mods)),
                }.copied()
            } else { None };
            if let Some(bind) = bind {
                CONTROLS.notify_press(vk, bind);
            }
        },
    }
}

pub fn process_key_bound(change: KeybindChange) {
    let interesting = CONTROLS.is_interested_in_control(change.control);
    #[cfg(todo)]
    if !interesting { return }

    let interesting_keys = {
        let Ok(mut binds) = GAME_BINDS.write() else { return };
        binds.process_update(&change);
        if !interesting { return }

        CONTROLS.collect_interesting_keys(
            // TODO: these should be delayed since we expect to receive a lot of these events at startup...
            binds.key_binds.iter()
                .chain(binds.mouse_binds.iter())
                .map(|(&(vk, _), &slot)| (slot, VIRTUAL_KEY(vk)))
            )
        };
    CONTROLS.set_interesting_keys(interesting_keys);
}

pub static HELD_KEYS: BitArray<[AtomicU64; KEY_PRESS_BITS / 64]> = BitArray {
    data: [const { AtomicU64::new(0) }; KEY_PRESS_BITS / 64],
    _ord: PhantomData,
};

pub fn process_key_event(msg: u32, w: usize, l: isize) -> u32 {
    #[cfg(todo)]
    let prev_down = l & (1 << 30) != 0;
    let repeat = l & 0xff;
    let is_up = matches!(msg, WindowsAndMessaging::WM_KEYUP | WindowsAndMessaging::WM_SYSKEYUP);
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
        let key = KeyInput {
            down,
            vk,
            mods: held_mods(),
        };
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
        let key = KeyInput {
            down,
            vk: button,
            mods: held_mods(),
        };
        if KeyIntercept::intercept_try_report(key) {
            return 0
        }
    }
    notify_interesting(false, button, down);

    msg
}

pub fn held_mods() -> KeyState {
    let bits0 = HELD_KEYS.data[0].load(Ordering::Relaxed);
    let binds: BitArray<_> = BitArray {
        data: [bits0],
        _ord: PhantomData,
    };
    let (shift, ctrl, alt) = unsafe {
        (
            *binds.get_unchecked(KeyboardAndMouse::VK_SHIFT.0 as usize),
            *binds.get_unchecked(KeyboardAndMouse::VK_CONTROL.0 as usize),
            *binds.get_unchecked(KeyboardAndMouse::VK_MENU.0 as usize),
        )
    };
    let mut mods = KeyState::EMPTY;
    mods.set(KeyState::SHIFT, shift);
    mods.set(KeyState::CTRL, ctrl);
    mods.set(KeyState::ALT, alt);
    mods
}

pub enum KeyIntercept {
    Pending,
    Intercepted {
        key: KeyInput,
    },
}

static KEY_INTERCEPT: AtomicU64 = AtomicU64::new(KeyIntercept::NONE);
impl KeyIntercept {
    const NONE: u64 = 0;
    const PENDING: u64 = u64::MAX;
    const DOWN: u64 = 0x1_00000000_0000;

    pub fn raw(&self) -> u64 {
        match self {
            Self::Pending => Self::PENDING,
            Self::Intercepted { key } => {
                let vk = key.vk.0 as u64;
                let mods = (key.mods.bits() as u64) << 16;
                let down = match key.down {
                    true => Self::DOWN,
                    false => 0,
                };
                vk as u64 | mods | down
            },
        }
    }

    pub fn from_raw(raw: u64) -> Option<Self> {
        Some(match raw {
            0 => return None,
            Self::PENDING => Self::Pending,
            raw => Self::Intercepted {
                key: KeyInput {
                    vk: KeyboardAndMouse::VIRTUAL_KEY(raw as u16),
                    mods: KeyState::from_bits_retain((raw >> 16) as u32),
                    down: raw & Self::DOWN != 0,
                },
            },
        })
    }

    pub fn intercept_restart() {
        KEY_INTERCEPT.store(Self::PENDING, Ordering::SeqCst);
    }

    pub fn intercept_take() -> Option<Self> {
        let mut raw = KEY_INTERCEPT.load(Ordering::SeqCst);
        loop {
            let int = match Self::from_raw(raw) {
                res @ (None | Some(Self::Pending)) => return res,
                int => int,
            };
            match KEY_INTERCEPT.compare_exchange_weak(raw, Self::NONE, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(..) => break int,
                Err(current) => {
                    raw = current;
                },
            }
        }
    }

    pub fn intercept_ready() -> bool {
        KEY_INTERCEPT.load(Ordering::Relaxed) == Self::PENDING
    }

    #[cfg(todo)]
    pub fn intercept_read() -> Option<Self> {
        Self::from_raw(KEY_INTERCEPT.load(Ordering::Relaxed))
    }

    pub fn intercept_report(key: KeyInput) {
        let int = Self::Intercepted {
            key,
        };
        KEY_INTERCEPT.store(int.raw(), Ordering::SeqCst);
    }

    pub fn intercept_try_report(key: KeyInput) -> bool {
        let int = Self::Intercepted {
            key,
        };
        KEY_INTERCEPT.compare_exchange(Self::PENDING, int.raw(), Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }
}
