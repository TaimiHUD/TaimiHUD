pub use arcdps::extras::keybinds::Control as GameControl;
use {
    crate::exports::runtime::bindings::keys::{KeyBind, MouseBind},
    arcdps::extras::keybinds::{Key, KeyCode, KeybindChange, MouseCode},
    bitvec::array::BitArray,
    serde::{Deserialize, Serialize},
    std::{collections::BTreeMap, fmt, iter, num::TryFromIntError, ops},
    taimi_input::win::keyboard::{KeyInput, KeyState},
    windows::Win32::UI::Input::KeyboardAndMouse::{self, VIRTUAL_KEY},
};

#[derive(
    Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Control {
    pub index: u8,
}

impl Control {
    pub const fn with_index(index: u8) -> Self {
        Self { index }
    }

    pub fn from_int(index: i32) -> Self {
        match Self::try_from_int(index) {
            Some(c) => c,
            None => {
                log::warn!("GameControl out of bounds: {index}");
                Self::with_index(0xff)
            },
        }
    }

    pub const fn try_from_int(index: i32) -> Option<Self> {
        match index {
            index @ 0..=0xff => Some(Self::with_index(index as u8)),
            _ => None,
        }
    }

    pub fn as_control(self) -> Option<GameControl> {
        GameControl::try_from(self.index as i32).ok()
    }

    pub fn label_ident(&self) -> &'static str {
        match self.as_control() {
            Some(control) => control.into(),
            None => {
                log::warn!("unrecognized control {self}");
                "unknown"
            },
        }
    }
}

impl From<GameControl> for Control {
    fn from(index: GameControl) -> Self {
        Self::from_int(index as i32)
    }
}
impl From<Control> for u8 {
    fn from(c: Control) -> Self {
        c.index
    }
}
impl From<Control> for i32 {
    fn from(c: Control) -> Self {
        c.index.into()
    }
}
impl TryFrom<i32> for Control {
    type Error = TryFromIntError;

    fn try_from(index: i32) -> Result<Self, Self::Error> {
        let res = Self::from_int(index);
        u8::try_from(index).map(|_| res)
    }
}
impl TryFrom<Control> for GameControl {
    type Error = anyhow::Error;

    fn try_from(index: Control) -> Result<Self, Self::Error> {
        GameControl::try_from(index.index as i32).map_err(Into::into)
    }
}

impl fmt::Debug for Control {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.as_control() {
            Some(c) => f.write_str(c.into()),
            None => f.debug_tuple("GameControl").field(&self.index).finish(),
        }
    }
}
impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_control() {
            Some(c) => f.write_str(c.into()),
            None => write!(f, "#{}", self.index),
        }
    }
}

pub const GAME_CONTROL_BITS: usize = 256;
pub type GameControlsBits = [u64; GAME_CONTROL_BITS / 64];

#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameControls {
    pub bits: BitArray<GameControlsBits>,
}
impl GameControls {
    pub const fn new(bits: BitArray<GameControlsBits>) -> Self {
        Self { bits }
    }

    pub fn iter<'a>(&'a self) -> GameControlsIter<'a> {
        fn map_control(index: usize) -> Control {
            Control::with_index(index as u8)
        }
        self.bits.iter_ones().map(map_control)
    }

    pub fn into_iter(self) -> GameControlsIntoIter {
        fn map_control_i((index, set): (usize, bool)) -> Option<Control> {
            match set {
                true => Some(Control::with_index(index as u8)),
                false => None,
            }
        }
        self.bits.into_iter().enumerate().filter_map(map_control_i)
    }

    pub fn contains<C: Into<Control>>(&self, control: C) -> bool {
        let i = control.into().index as usize;
        unsafe { *self.bits.get_unchecked(i) }
    }

    pub fn set<C: Into<Control>>(&mut self, control: C, set: bool) {
        let i = control.into().index as usize;
        unsafe {
            self.bits.set_unchecked(i, set);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bits.not_any()
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
impl<I: Into<Control>> Extend<I> for GameControls {
    fn extend<T: IntoIterator<Item = I>>(&mut self, iter: T) {
        for item in iter {
            self.set(item.into(), true);
        }
    }
}
impl<I: Into<Control>> FromIterator<I> for GameControls {
    fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
        let mut controls = Self::default();
        controls.extend(iter);
        controls
    }
}
pub type GameControlsIntoIter = iter::FilterMap<
    iter::Enumerate<bitvec::array::IntoIter<GameControlsBits, bitvec::order::Lsb0>>,
    fn((usize, bool)) -> Option<Control>,
>;
pub type GameControlsIter<'a> =
    iter::Map<bitvec::slice::IterOnes<'a, u64, bitvec::order::Lsb0>, fn(usize) -> Control>;
impl IntoIterator for GameControls {
    type Item = Control;
    type IntoIter = GameControlsIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}
impl<'a> IntoIterator for &'a GameControls {
    type Item = Control;
    type IntoIter = GameControlsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
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

pub type ControlIndex = i8;
pub type ControlSlot = (Control, ControlIndex);

#[test]
fn game_control_bitarray() {
    for control in GameControls::all_controls() {
        let index: i32 = control.into();
        assert!(index < GAME_CONTROL_BITS as i32);
    }
}

#[derive(Debug, Clone, Default)]
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

    pub fn is_empty(&self) -> bool {
        self.key_binds.is_empty() && self.mouse_binds.is_empty()
    }

    pub fn process_update(&mut self, change: &KeybindChange) {
        let mods = KeyState::from(change);
        let slot = (Control::from(change.control), change.index as _);

        // empty the prior binding on this slot
        self.remove(slot);

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
                log::warn!(
                    "unrecognized keybind code {code} for control {}",
                    i32::from(change.control)
                );
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
            // TODO: delete this?
            KeyCode::LeftAlt | KeyCode::RightAlt => KeyboardAndMouse::VK_MENU,
            KeyCode::LeftCtrl | KeyCode::RightCtrl => KeyboardAndMouse::VK_CONTROL,
            KeyCode::LeftShift | KeyCode::RightShift => KeyboardAndMouse::VK_SHIFT,
            key => KeyInput::vk_from_extras(key),
        }
    }

    pub fn vk_for_button(button: MouseCode) -> VIRTUAL_KEY {
        KeyInput::vk_from_extras_button(button)
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

    pub fn get<C: Into<Control>>(
        &self,
        control: C,
        index: Option<ControlIndex>,
    ) -> Option<(VIRTUAL_KEY, KeyState)> {
        self.get_slot(control, index).map(|(c, _)| c)
    }
    pub fn get_slot<C: Into<Control>>(
        &self,
        control: C,
        index: Option<ControlIndex>,
    ) -> Option<((VIRTUAL_KEY, KeyState), ControlIndex)> {
        let control = control.into();
        let bind = self
            .key_binds
            .iter()
            .chain(self.mouse_binds.iter())
            .find(|(_k, &(c, i))| match index {
                Some(index) if index != i => false,
                _ => c == control,
            });
        bind.map(|(&(vk, mods), &(_, sloti))| ((VIRTUAL_KEY(vk), mods), sloti))
    }

    pub fn set<C: Into<Control>>(&mut self, control: C, index: ControlIndex, bind: KeyBind) {
        let (vk, _) = bind;
        let slot = (control.into(), index);
        self.remove(slot);
        match Self::vk_is_button(VIRTUAL_KEY(vk)) {
            true => {
                self.mouse_binds.insert(bind, slot);
            },
            false => {
                self.key_binds.insert(bind, slot);
            },
        }
    }

    pub fn remove(&mut self, slot: ControlSlot) {
        self.key_binds.retain(|_, &mut s| s != slot);
        self.mouse_binds.retain(|_, &mut s| s != slot);
    }
}

impl<'de> Deserialize<'de> for GameBinds {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let binds = <Vec<((u16, u32), (i32, ControlIndex))> as Deserialize>::deserialize(deserializer)?;

        let mut seen = BTreeMap::new();
        let mut out = Self::default();
        for ((vk, mods), (control, index)) in binds {
            let control =
                Control::try_from(control).map_err(|e| <D::Error as serde::de::Error>::custom(e))?;
            let mods = KeyState::from_bits_retain(mods);
            let slot = (control, index);
            let bind = (vk, mods);
            let slot_seen = *seen.entry(slot).or_insert(bind);
            if slot_seen != bind {
                log::warn!("discarding overlapping keybind for {control:?}({index}): {bind:?}");
                continue
            }
            match Self::vk_is_button(VIRTUAL_KEY(vk)) {
                false => {
                    out.key_binds.insert(bind, slot);
                },
                true => {
                    out.mouse_binds.insert(bind, slot);
                },
            }
        }
        Ok(out)
    }
}
impl Serialize for GameBinds {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let binds = self
            .key_binds
            .iter()
            .chain(self.mouse_binds.iter())
            .map(|(&(vk, mods), &(control, index))| ((vk, mods.bits()), (control, index)))
            .collect::<Vec<_>>();
        binds.serialize(serializer)
    }
}

pub(crate) fn default_gamebinds() -> BTreeMap<Control, KeyInput> {
    let mut keybinds = BTreeMap::new();
    macro_rules! default_keybind {
        () => {};
        ($control:ident => KeyCode::$key:ident, $mods:expr; $($rest:tt)*) => {
            //if !keybinds.contains_key(&GameControl::$control)
            keybinds.insert(GameControl::$control.into(), KeyInput::new(GameBinds::vk_for_key(arcdps::extras::KeyCode::$key), $mods, false));
            default_keybind! {
                $($rest)*
            }
        };
    }

    #[cfg(feature = "markers")]
    default_keybind! {
        Squad_Location_Arrow => KeyCode::Number1, KeyState::ALT;
        Squad_Location_Circle => KeyCode::Number2, KeyState::ALT;
        Squad_Location_Square => KeyCode::Number3, KeyState::ALT;
        Squad_Location_Heart => KeyCode::Number4, KeyState::ALT;
        Squad_Location_Star => KeyCode::Number5, KeyState::ALT;
        Squad_Location_Spiral => KeyCode::Number6, KeyState::ALT;
        Squad_Location_Triangle => KeyCode::Number7, KeyState::ALT;
        Squad_Location_X => KeyCode::Number8, KeyState::ALT;
        Squad_ClearAllLocationMarkers => KeyCode::Number9, KeyState::ALT;
        Squad_Object_Arrow => KeyCode::Number1, KeyState::ALT | KeyState::SHIFT;
        Squad_Object_Circle => KeyCode::Number2, KeyState::ALT | KeyState::SHIFT;
        Squad_Object_Square => KeyCode::Number3, KeyState::ALT | KeyState::SHIFT;
        Squad_Object_Heart => KeyCode::Number4, KeyState::ALT | KeyState::SHIFT;
        Squad_Object_Star => KeyCode::Number5, KeyState::ALT | KeyState::SHIFT;
        Squad_Object_Spiral => KeyCode::Number6, KeyState::ALT | KeyState::SHIFT;
        Squad_Object_Triangle => KeyCode::Number7, KeyState::ALT | KeyState::SHIFT;
        Squad_Object_X => KeyCode::Number8, KeyState::ALT | KeyState::SHIFT;
        Squad_ClearAllObjectMarkers => KeyCode::Number9, KeyState::ALT | KeyState::SHIFT;
    }
    #[cfg(feature = "space")]
    default_keybind! {
        Miscellaneous_Interact => KeyCode::F, KeyState::empty();
    }
    #[cfg(any(feature = "markers", feature = "space"))]
    default_keybind! {
        Map_OpenClose => KeyCode::M, KeyState::empty();
        Map_ZoomIn => KeyCode::PlusNum, KeyState::empty();
        Map_ZoomOut => KeyCode::MinusNum, KeyState::empty();
        Map_FloorUp => KeyCode::Next, KeyState::empty();
        Map_FloorDown => KeyCode::Prior, KeyState::empty();
        Map_Recenter => KeyCode::Space, KeyState::empty();
        UI_ShowHideUI => KeyCode::H, KeyState::CTRL | KeyState::SHIFT;
        // TODO: settarget, setpersonaltarget, setjadebotwaypoint
        // TODO: setpersonalwaypoint, draw-on-map
    }
    keybinds
}
