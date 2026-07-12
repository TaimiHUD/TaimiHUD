#[cfg(feature = "windows")]
use windows::{
    core::Error as WinError,
    Win32::{UI::Input::KeyboardAndMouse as vk, UI::WindowsAndMessaging as wm},
};
use {
    bitvec::array::BitArray,
    core::{cell::UnsafeCell, fmt, marker::PhantomData, mem, num::NonZero, ops, ptr},
    std::sync::atomic::{AtomicU32, Ordering},
};

type KeysDownStorageElem = u64;
type KeysDownIndex = u8;
pub type KeysDownStorage = [KeysDownStorageElem; KeysDown::SIZE_ELEMS];
pub type KeysDownBits = BitArray<KeysDownStorage, bitvec::order::Lsb0>;
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeysDown {
    pub down: KeysDownBits,
}
impl KeysDown {
    pub const COUNT: usize = 0x100;
    pub const EMPTY: Self = Self { down: Self::BITS_EMPTY };
    const SIZE_ELEMS: usize = KeysDown::COUNT.div_ceil(mem::size_of::<KeysDownStorageElem>() * 8);
    pub const BITS_EMPTY: KeysDownBits = KeysDownBits {
        data: [0u64; Self::SIZE_ELEMS],
        _ord: PhantomData,
    };
    #[inline]
    pub const fn with_bits(down: KeysDownBits) -> Self {
        Self { down }
    }
    #[inline]
    pub const fn with_storage(data: KeysDownStorage) -> Self {
        Self::with_bits(KeysDownBits { data, _ord: PhantomData })
    }

    #[inline]
    pub fn iter_keys(&self) -> impl Iterator<Item = bool> + Clone + Send + Sync + '_ {
        self.down.iter().map(|i| *i)
    }
    #[inline]
    pub fn iter_down(&self) -> impl Iterator<Item = KeysDownIndex> + Clone + Send + Sync + '_ {
        self.down.iter_ones().map(|i| i as KeysDownIndex)
    }
    #[inline]
    pub fn is_key_down(&self, vk: KeysDownIndex) -> bool {
        unsafe { *self.down.get_unchecked(vk as usize) }
    }

    #[inline]
    pub fn set_key_down(&mut self, vk: KeysDownIndex, down: bool) {
        debug_assert!(Self::COUNT >= KeysDownIndex::MAX as usize);
        unsafe {
            self.down.set_unchecked(vk as usize, down);
        }
    }
}
#[cfg(feature = "windows")]
impl KeysDown {
    /// TODO: WM_KEYFIRST?
    pub const WM_KEY_MIN: u32 = wm::WM_KEYDOWN;
    /// TODO: WM_KEYLAST?
    pub const WM_KEY_MAX: u32 = wm::WM_SYSKEYUP;
    /// XXX: WM_NC* in a different range (currently ignored)
    ///
    /// TODO: `WM_MOUSEFIRST`? we're uninterested in `WM_MOUSEMOVE` though...
    pub const WM_BUTTON_MIN: u32 = wm::WM_LBUTTONDOWN;
    /// XXX: or WM_XBUTTONDBLCLK (currently ignored)
    ///
    /// TODO: WM_MOUSELAST?
    pub const WM_BUTTON_MAX: u32 = wm::WM_XBUTTONUP;

    pub fn decode_key_event(msg: u32, w: usize, l: isize) -> Option<(NonZero<KeysDownIndex>, bool)> {
        #[cfg(todo)]
        let prev_down = l & (1 << 30) != 0;
        let repeat = l & 0xff;
        let is_up = matches!(msg, wm::WM_KEYUP | wm::WM_SYSKEYUP);
        #[cfg(todo)]
        let is_trigger = !is_up && repeat == 1;
        #[cfg(todo)]
        let is_release = is_up && prev_down;

        if !is_up && repeat != 1 {
            return None
        }
        let down = !is_up;

        match w {
            w @ 1..=0xfe => Some(unsafe { (NonZero::new_unchecked(w as KeysDownIndex), down) }),
            _ => {
                #[cfg(debug_assertions)]
                log::trace!("ignoring vk {w}");
                None
            },
        }
    }
    pub fn decode_button_event(msg: u32, w: usize, _l: isize) -> Option<(NonZero<KeysDownIndex>, bool)> {
        let (button, down) = match msg {
            wm::WM_LBUTTONUP => (vk::VK_LBUTTON, false),
            wm::WM_LBUTTONDOWN => (vk::VK_LBUTTON, true),
            #[cfg(todo)]
            | wm::WM_LBUTTONDBLCLK
            | wm::WM_RBUTTONDBLCLK
            | wm::WM_MBUTTONDBLCLK
            | wm::WM_XBUTTONDBLCLK
            | wm::WM_NCLBUTTONDBLCLK
            | wm::WM_NCRBUTTONDBLCLK
            | wm::WM_NCMBUTTONDBLCLK
            | wm::WM_NCXBUTTONDBLCLK => todo,
            wm::WM_RBUTTONUP => (vk::VK_RBUTTON, false),
            wm::WM_RBUTTONDOWN => (vk::VK_RBUTTON, true),
            wm::WM_MBUTTONUP => (vk::VK_MBUTTON, false),
            wm::WM_MBUTTONDOWN => (vk::VK_MBUTTON, true),
            wm::WM_XBUTTONUP | wm::WM_XBUTTONDOWN => (
                match (w >> 16) as u16 {
                    wm::XBUTTON1 => vk::VK_XBUTTON1,
                    wm::XBUTTON2 => vk::VK_XBUTTON2,
                    _ => return None,
                },
                msg == wm::WM_XBUTTONDOWN,
            ),
            _ => return None,
        };
        let vk = unsafe { NonZero::new_unchecked(button.0 as KeysDownIndex) };
        Some((vk, down))
    }
    #[inline]
    pub fn process_key_event(
        &mut self,
        msg: u32,
        w: usize,
        l: isize,
    ) -> Option<(NonZero<KeysDownIndex>, bool)> {
        let vk = Self::decode_key_event(msg, w, l);
        if let Some((vk, down)) = vk {
            self.set_key_down(vk.get(), down);
        }
        vk
    }
    /// XXX: may clobber state if button VKs overlap...
    #[inline]
    pub fn process_button_event(
        &mut self,
        msg: u32,
        w: usize,
        l: isize,
    ) -> Option<(NonZero<KeysDownIndex>, bool)> {
        let vk = Self::decode_button_event(msg, w, l);
        if let Some((vk, down)) = vk {
            self.set_key_down(vk.get(), down);
        }
        vk
    }

    /// overwrite and resynchronize using [GetKeyboardState](vk::GetKeyboardState)
    pub fn win32_refresh(&mut self) -> Result<(), WinError> {
        let mut out = [0u8; 0x100];
        let () = unsafe { vk::GetKeyboardState(&mut out) }?;
        let keys = self.down.iter_mut().zip(out);
        for (mut b, state) in keys {
            let is_down = state & 0x80 != 0;
            *b = is_down;
        }
        Ok(())
    }
    /// TODO: of dubious use?
    pub fn normalize_mod_hand(vk: vk::VIRTUAL_KEY) -> vk::VIRTUAL_KEY {
        match vk {
            vk::VK_LMENU | vk::VK_RMENU => vk::VK_MENU,
            vk::VK_LSHIFT | vk::VK_RSHIFT => vk::VK_SHIFT,
            vk::VK_LCONTROL | vk::VK_RCONTROL => vk::VK_CONTROL,
            k => k,
        }
    }
}
impl ops::BitAnd for KeysDown {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self::with_bits(self.down & rhs.down)
    }
}
#[cfg(todo)]
impl ops::BitAnd<&'_ Self> for KeysDown {}
#[cfg(todo)]
impl ops::BitAnd<KeysDown> for &'_ KeysDown {}
impl ops::BitOr for KeysDown {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self::with_bits(self.down | rhs.down)
    }
}
impl ops::Not for KeysDown {
    type Output = Self;
    #[inline]
    fn not(self) -> Self::Output {
        Self::with_bits(!self.down)
    }
}
impl ops::BitXor for KeysDown {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self::with_bits(self.down ^ rhs.down)
    }
}
impl ops::BitAndAssign for KeysDown {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.down &= rhs.down
    }
}
impl ops::BitAndAssign<&'_ Self> for KeysDown {
    #[inline]
    fn bitand_assign(&mut self, rhs: &Self) {
        self.down &= &rhs.down
    }
}
impl ops::BitOrAssign for KeysDown {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.down |= rhs.down
    }
}
impl ops::BitOrAssign<&'_ Self> for KeysDown {
    #[inline]
    fn bitor_assign(&mut self, rhs: &Self) {
        self.down |= &rhs.down
    }
}
impl ops::BitXorAssign for KeysDown {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.down ^= rhs.down
    }
}
impl ops::BitXorAssign<&'_ Self> for KeysDown {
    #[inline]
    fn bitxor_assign(&mut self, rhs: &Self) {
        self.down ^= &rhs.down
    }
}
impl Default for KeysDown {
    #[inline]
    fn default() -> Self {
        Self::EMPTY
    }
}
impl fmt::Debug for KeysDown {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter_down()).finish_non_exhaustive()
    }
}

pub struct KeysDownBroadcast {
    pub down: UnsafeCell<KeysDown>,
    pub version: AtomicU32,
}
impl KeysDownBroadcast {
    pub const EMPTY: Self = Self::with_keys(KeysDown::EMPTY);
    #[inline]
    pub const fn with_keys(down: KeysDown) -> Self {
        Self {
            down: UnsafeCell::new(down),
            version: AtomicU32::new(0),
        }
    }
    pub fn into_keys(self) -> KeysDown {
        self.down.into_inner()
    }
    #[inline(always)]
    pub fn keys_racy(&self) -> &KeysDown {
        unsafe { &*self.down.get() }
    }
    #[inline(always)]
    pub fn keys_mut_unchecked(&self) -> &mut KeysDown {
        unsafe { &mut *self.down.get() }
    }
    /// [KeysDown::is_key_down]
    #[inline(always)]
    pub fn is_key_down(&self, vk: KeysDownIndex) -> bool {
        self.keys_racy().is_key_down(vk)
    }
    /// TODO: cex loop if anyone cares but this is meant to be spmc so...
    #[cfg(todo)]
    pub fn with_mut(&self, mut f: dyn FnMut(&mut KeysDown)) {}
    /// [KeysDown::set_key_down]
    #[inline(always)]
    pub fn set_key_down_unchecked(&self, vk: KeysDownIndex, down: bool) {
        let ver = self.version_bump_changing_unchecked();
        self.keys_mut_unchecked().set_key_down(vk, down);
        self.version_bump_changed_unchecked(ver);
    }
    #[inline(always)]
    pub fn read_keys_unchecked(&self) -> KeysDown {
        unsafe { ptr::read(self.down.get()) }
    }
    /// if version is missing, consider it unreliable
    pub fn read_keys(&self) -> (KeysDown, Option<KeysDownVersion>) {
        let mut i = 0u32;
        let mut keys = None;
        let mut ver_seen = None;
        while i < Self::READ_TIMEOUT {
            i += 1;
            let Some(ver_pre) = self.version_read_start() else {
                // dumbest spinlock ever
                #[cfg(todo)]
                nop();
                continue
            };
            // XXX: ordering on the loads already handles need for memory barriers here right guys?
            keys = Some(self.read_keys_unchecked());
            if self.version_read_end(ver_pre) {
                ver_seen = Some(ver_pre);
                break
            }
            // someone's busy, consider trying again later...
            i += 4;
        }
        match keys {
            Some(keys) => (keys, ver_seen),
            None => (self.read_keys_unchecked(), None),
        }
    }
    const READ_TIMEOUT: u32 = 0x400;
    /// [KeysDown::process_key_event]
    #[inline(always)]
    #[cfg(feature = "windows")]
    pub fn process_key_event_unchecked(
        &self,
        msg: u32,
        w: usize,
        l: isize,
    ) -> Option<(NonZero<KeysDownIndex>, bool)> {
        let vk = KeysDown::decode_key_event(msg, w, l);
        if let Some((vk, down)) = vk {
            self.set_key_down_unchecked(vk.get(), down);
        }
        vk
    }
    /// [KeysDown::process_button_event]
    #[inline(always)]
    #[cfg(feature = "windows")]
    pub fn process_button_event_unchecked(
        &self,
        msg: u32,
        w: usize,
        l: isize,
    ) -> Option<(NonZero<KeysDownIndex>, bool)> {
        let vk = KeysDown::decode_button_event(msg, w, l);
        if let Some((vk, down)) = vk {
            self.set_key_down_unchecked(vk.get(), down);
        }
        vk
    }
}
pub type KeysDownVersion = u32;
impl KeysDownBroadcast {
    pub fn version_read_start(&self) -> Option<KeysDownVersion> {
        let version = self.version.load(Ordering::Acquire);
        let changing = version & 1 != 0;
        (!changing).then_some(version)
    }
    pub fn version_read_end(&self, pre: KeysDownVersion) -> bool {
        let version = self.version.load(Ordering::Acquire);
        version == pre
    }
    pub fn version_bump_changing_unchecked(&self) -> KeysDownVersion {
        self.version.fetch_add(1, Ordering::Release)
    }
    pub fn version_bump_changed_unchecked(&self, pre_bump: KeysDownVersion) {
        let committed = pre_bump.wrapping_add(2);
        self.version.store(committed, Ordering::Release);
    }
}
impl Default for KeysDownBroadcast {
    #[inline]
    fn default() -> Self {
        Self::EMPTY
    }
}
impl fmt::Debug for KeysDownBroadcast {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(core::any::type_name::<Self>())
            .field(&self.read_keys_unchecked())
            .finish()
    }
}
unsafe impl Sync for KeysDownBroadcast {}
unsafe impl Send for KeysDownBroadcast {}
