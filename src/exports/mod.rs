#[cfg(feature = "extension-arcdps")]
pub mod arcdps;
#[cfg(feature = "extension-nexus")]
pub mod nexus;

pub mod runtime;

/// Update URL provided as a literal to appease `nexus::export!`
#[macro_export]
macro_rules! gh_repo_url {
    () => {
        "https://github.com/TaimiHUD/TaimiHUD"
    };
}
pub use gh_repo_url;

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
#[cfg(feature = "extension-arcdps-extern")]
pub const ADDON_TITLE_C: &'static std::ffi::CStr = arcffi::cstr!(addon_title!());

pub const SIG: i32 = 0x7331BABD;
pub const ADDON_DIR_NAME: &'static str = "Taimi";
