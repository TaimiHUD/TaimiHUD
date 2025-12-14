use {
    crate::exports::runtime as rt,
    anyhow::Context,
    std::{ffi::CString, fmt, process},
    taimi_ffi::win32::mmap::{FileMapping, FILE_MAP},
};

pub fn check_for_data(name: &str) -> bool {
    match open_data_link(&name, FileMapping::ACCESS_READ) {
        Ok(mut handle) => {
            let res = handle
                .close()
                .with_context(|| format!("cleanup after checking for {name} failed"));
            rt::log::debug_ok(res);
            true
        },
        Err(_e) => {
            // TODO: does it matter what error code we expect, ERROR_OBJECT_NOT_FOUND?
            #[cfg(debug_assertions)]
            {
                log::debug!("NexusLink({name}) unavailable: {_e}");
            }
            false
        },
    }
}
pub fn check_for_nexus_link() -> bool {
    check_for_data("DL_NEXUS_LINK")
}

pub fn open_data_link(name: &dyn fmt::Display, access: FILE_MAP) -> anyhow::Result<FileMapping> {
    let object_name = unsafe {
        //let process_id = windows::Win32::System::Threading::GetCurrentProcessId();
        let process_id = process::id();
        let name = format!("{name}_{process_id}");
        CString::from_vec_unchecked(name.into())
    };
    FileMapping::open_named(&object_name, access)
        .with_context(move || unsafe { String::from_utf8_unchecked(object_name.into_bytes()) })
}
