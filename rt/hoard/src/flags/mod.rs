pub use self::serde::{BitFlagContainer, BitFlagDisplay};
#[cfg(feature = "serde")]
pub use self::serde::{BitFlagDe, BitFlagSer};

mod serde;
