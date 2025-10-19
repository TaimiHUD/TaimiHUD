use {
    bitvec::array::BitArray,
    std::{
        fmt,
        ops,
    },
};
pub use arcdps::extras::keybinds::Control as GameControl;

pub const GAME_CONTROL_BITS: usize = 256;
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
