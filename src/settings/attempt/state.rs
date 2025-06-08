use tokio::fs::{create_dir_all, read_to_string, File};
use tokio::io::AsyncWriteExt;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use relative_path::RelativePathBuf;

use crate::settings::{GitHubSource, Source, NeedsUpdate, RemoteSource};

use crate::timer::TimerFile;
use crate::ADDON_DIR;

use super::sources::SourceKind;
use super::RemoteState;

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct StateFile {
    pub sources: HashMap<SourceKind, UpdateState>,
    pub render_2d: Render2DState,
}

impl StateFile {
    pub fn generate_stock() -> Self {
        Default::default()
    }

    pub async fn create_stock() -> anyhow::Result<()> {
        let addon_dir = &*ADDON_DIR;
        create_dir_all(addon_dir).await?;
        let state_path = addon_dir.join("state.toml");
        let stock_state = Self::generate_stock();
        let state = toml::to_string_pretty(&stock_state)?;
        let mut file = File::create(state_path).await?;
        file.write_all(state.as_bytes()).await?;
        Ok(())
    }


    pub async fn load() -> anyhow::Result<Self> {
        let state_path = ADDON_DIR.join("state.toml");
        log::debug!("Attempting to load the state file at \"{state_path:?}\".");
        let file_data = read_to_string(&state_path).await?;
        let data: Self = toml::from_str(&file_data)?;
        log::debug!("Loaded the state file at \"{state_path:?}\".");
        Ok(data)
    }
    pub async fn save(&self) -> anyhow::Result<()> {
        let addon_dir = &*ADDON_DIR;
        create_dir_all(addon_dir).await?;
        let state_path = addon_dir.join("state.toml");
        log::debug!("Saving state path to \"{state_path:?}\".");
        let state = toml::to_string_pretty(&self)?;
        let mut file = File::create(state_path).await?;
        file.write_all(state.as_bytes()).await?;
        Ok(())
    }
    pub fn get_by_kind(&self, kind: SourceKind) -> Option<&Vec<SourceState>> {
        match self.sources.get(&kind) {
            Some(s) => Some(&s.sources),
            None => None,
        }
    }
    pub fn get_paths(&self, kind: SourceKind) -> Vec<PathBuf> {
        self.get_by_kind(kind)
            .unwrap_or(&Vec::new())
            .iter()
            .map(
                |s|
                s.install.dir.to_path_buf()
            )
            .collect()
    }

    pub fn get_timers(&self) {
        if let Some(update_state) = self.sources.get(&SourceKind::Timers) {
        }
    }
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct UpdateState {
    pub last_checked: Option<DateTime<Utc>>,
    pub sources: Vec<SourceState>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SourceState {
    pub source: RemoteSource,
    // This isn't an Option<T> because if it's not installed,
    // we shouldn't keep track of it as state but instead consider it from the
    // sources file, not the state file.
    pub install: SourceInstallState,
}

impl SourceState {

}

#[derive(Deserialize, Serialize, Debug)]
pub struct SourceInstallState {
    // This doesn't necessarily need to be a semver, you can
    // realistically assign any arbitrary string that changes between releases
    // for this.
    pub version: String,
    pub dir: PathBuf,
}


#[derive(Deserialize, Serialize, Default, Debug)]
pub struct Render2DState {
    primary_window: PrimaryWindowState,
    timers_window: TimersWindowState,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct PrimaryWindowState {
    open: bool,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct TimersWindowState {
    open: bool,
}

#[derive(Debug)]
struct TimerSourceRuntimeState {
    association: Arc<SourceState>,
    needs_update: NeedsUpdate,
}

impl TimerSourceRuntimeState {
    fn associate(source: Arc<SourceState>) -> Self {
        Self {
            association: source.clone(),
            needs_update: Default::default(),
        }
    }

    pub async fn check_for_update(&self) -> NeedsUpdate {
        use NeedsUpdate::*;
        let source = self.association.source.source();
        let remote_id = source.latest_id().await;
        log::debug!("{:?}", remote_id);
        match remote_id {
            Ok(rid) => {
                let lid = &self.association.install.version;
                Known(*lid != rid, rid)
            }, Err(err) => {
                log::error!("Update check failed: {}", err);
                NeedsUpdate::Error(err.to_string())
            },
        }
    }
}
