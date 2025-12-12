use {
    crate::{
        built_info,
        exports::{nexus as exports, update_url_of},
    },
    nexus::{AddonFlags, UpdateProvider},
};

pub use crate::exports::nexus::{FLAGS, SIG, UPDATE_PROVIDER};

pub(crate) fn load() {
    exports::pre_init();
    exports::init()
}

pub(crate) fn unload() {
    exports::uninit()
}

#[cfg(taimi_has = "update-github")]
macro_rules! update_url {
    () => { update_url_of! { GitHub: &str } };
}
#[cfg(taimi_has = "update-direct")]
macro_rules! update_url {
    () => { update_url_of! { Direct: &str } };
}
#[cfg(not(any(taimi_has = "update-github", taimi_has = "update-direct")))]
macro_rules! update_url {
    () => { "" };
}
pub(crate) use update_url;
