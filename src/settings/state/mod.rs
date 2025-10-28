use {
    anyhow::Context,
    std::{fs, path::Path, time::UNIX_EPOCH},
};

mod bootstrap;
mod save;

pub use self::{
    bootstrap::{AddonHostName, BootstrapState, UpdatePreference},
    save::SaveState,
};

pub fn save_state_backup(path: &Path) {
    if !path.exists() {
        let path = path.file_name().unwrap_or(path.as_ref());
        log::debug!("Skipping backup of {}", path.display());
        return
    }
    let ts = UNIX_EPOCH.elapsed().map(|d| d.as_secs()).unwrap_or(0);

    let mut backup_path = path.to_owned();
    {
        use std::fmt::Write;
        // append to end of filename...
        let backup_os = backup_path.as_mut_os_string();
        let _ = write!(backup_os, ".bak.{ts}");
    }

    log::warn!(
        "Something went wrong! Saving backup to {} in case we lose your settings...",
        backup_path.display()
    );
    let res = fs::copy(path, &backup_path).context("Copying to backup");
    if let Err(e) = res {
        log::error!("{e:#}");
    }
}
