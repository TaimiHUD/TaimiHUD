pub use self::{
    serde::{BitFlagContainer, BitFlagDisplay},
    set::FlagSet,
};
#[cfg(feature = "serde")]
pub use self::serde::{BitFlagDe, BitFlagSer};

mod serde;
pub mod set;
