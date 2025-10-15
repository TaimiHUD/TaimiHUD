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
macro_rules! taimihud_addon_name {
    () => {
        "TaimiHUD"
    };
}

pub use taimihud_addon_name;

pub const SIG: i32 = 0x7331BABD;
pub const ADDON_DIR_NAME: &'static str = "Taimi";
