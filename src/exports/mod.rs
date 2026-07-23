#[cfg(feature = "extension-arcdps")]
pub mod arcdps;
#[cfg(feature = "extension-nexus")]
pub mod nexus;

pub mod hosted;
pub mod runtime;

/// Update URL provided as a literal to appease `nexus::export!`
#[macro_export]
macro_rules! update_url_of {
    (GitHub:&str) => {
        env!("ADDON_URL_GITHUB")
    };
    (Direct:&str) => {
        env!("ADDON_URL_UPDATE_DIRECT")
    };
    ($spec:tt:&CStr) => {
        arcffi::cstr! { $crate::exports::update_url_of! { $spec:&str } }
    };
}
pub use update_url_of;

#[macro_export]
#[cfg(taimi_has = "title")]
macro_rules! addon_title {
    () => { env!("ADDON_TITLE") };
}
#[macro_export]
#[cfg(not(taimi_has = "title"))]
macro_rules! addon_title {
    () => { "TaimiHUD" };
}
pub use addon_title;
#[cfg(any(feature = "extension-arcdps-extern", feature = "extension-nexus-extern"))]
pub const ADDON_TITLE_C: &'static std::ffi::CStr = arcffi::cstr!(addon_title!());

pub const SIG: i32 = 0x7331BABD;
pub const ADDON_DIR_NAME: &'static str = "Taimi";
