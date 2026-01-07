pub use {
    self::{
        serde::{BitFlagContainer, BitFlagDisplay},
        set::{BitSet, FlagSet},
    },
    bitvec::{
        self,
        array::{self as bitarray, BitArray},
        boxed::{self as bitbox, BitBox},
        index as bitidx,
        order::{self as bitorder, BitOrder, LocalBits as BitsNative, Lsb0 as BitsLsb, Msb0 as BitsMsb},
        ptr::{self as bitptr, BitPtr, BitRef, Mutability as BitMutability},
        slice::{self as bitslice, BitSlice},
        store::{self as bitstore, BitStore},
        vec::BitVec,
        view::{self as bitview, BitView},
    },
    wyz::comu::{self, Address as BitAddr},
};

#[cfg(feature = "serde")]
pub use self::serde::{BitFlagDe, BitFlagSer};

mod serde;
pub mod set;
