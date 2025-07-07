use {
    arcdps::extras::{self, ExtrasSubscriberInfo},
    crate::exports::{arcdps as exports, runtime as rt},
    std::{ffi::CStr, panic, str},
};

pub(crate) unsafe extern "C-unwind" fn extras_init_raw(info: *const extras::RawExtrasAddonInfo, subscriber: *mut ExtrasSubscriberInfo) {
    if !exports::loaded() {
        crate::crate_init();
    }

    let res = panic::catch_unwind(|| {
        if info.is_null() || subscriber.is_null() {
            log::warn!("arcdps_unofficial_extras init missing required argument");
            return
        }
        #[cfg(feature = "extension-nexus")]
        if rt::nexus_available() || exports::check_for_nexus() {
            log::info!("ignoring arcdps_unofficial_extras, nexus is available");
            return
        }

        let info = &*info;
        let version = info.version();
        if !version.is_compatible() {
            log::info!("unsupported arcdps_unofficial_extras api version {}/{}", version.api_version, version.max_info_version);
            return
        }

        // there's a type for this you know...
        let name = str::from_utf8_unchecked(rt::NAME_C.to_bytes_with_nul());
        ExtrasSubscriberInfo::subscribe(subscriber, info, name,
            Some(cb_squad_update_raw),
            Some(cb_language_changed_raw),
            Some(cb_keybind_changed_raw),
            None, None,
        );
        
        let account_name = match info.self_account_name {
            name if name.is_null() =>
                None,
            name => Some(CStr::from_ptr(name as *const _)),
        };
        if let Some(name) = account_name {
            crate::receive_account_name(name.to_string_lossy());
        }

        exports::extras_init(version);
    });
    if let Err(e) = res {
        crate::log_any_error("arcdps_unofficial_extras_subscriber_init", &e);
    }
}

pub(crate) unsafe extern "C-unwind" fn cb_squad_update_raw(users: *const extras::user::UserInfo, len: u64) {
    exports::extras_squad_update(extras::user::to_user_info_iter(users, len))
}

pub(crate) unsafe extern "C-unwind" fn cb_language_changed_raw(language: arcdps::Language) {
    exports::extras_language(language)
}

pub(crate) unsafe extern "C-unwind" fn cb_keybind_changed_raw(keybind: extras::keybinds::RawKeybindChange) {
    exports::extras_keybind(keybind.into())
}
