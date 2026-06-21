#![doc(html_logo_url = "https://taimihud.com/logotype-holo.png")]
//! Input handling and keybind processing for TaimiHUD.
#[cfg(feature = "windows")]
pub mod win;

pub type RuntimeError = &'static str;
pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;
