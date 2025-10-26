use {
    bitvec::array::BitArray,
    taimi_input::win::keyboard::KeyState,
};

pub const KEY_PRESS_BITS: usize = 256;
pub type KeyPresses = BitArray<[u64; KEY_PRESS_BITS / 64]>;

pub type KeyBind = (u16, KeyState);
#[cfg(todo)]
pub type MouseBind = (MouseCode, KeyState);
pub type MouseBind = KeyBind;
