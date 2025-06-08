use std::{collections::HashMap, path::PathBuf, sync::Arc};
use magic_migrate::{MigrateError, TryMigrate};
use serde::{Deserialize, Serialize};

use relative_path::RelativePathBuf;
use tokio::fs::{create_dir_all, read_to_string, File};
use tokio::io::AsyncWriteExt;

use crate::settings::{GitHubSource, NeedsUpdate, RemoteSource};

use crate::timer::TimerFile;
use crate::ADDON_DIR;

use super::sources::SourceKind;
use super::state::{SourceState, StateFile};
use crate::controller::ProgressBarStyleChange;
use super::{ProgressBarSettings, Settings, SourcesFile};


#[derive(Deserialize, Serialize, Default, Debug)]
pub struct ConfigV2 {
    timers: Vec<TimerSourceConfig>,
    render_2d: Render2DConfig,
    render_3d: Render3DConfig,
}

impl ConfigV2 {
    pub async fn load() -> anyhow::Result<Self> {
        let config_path = ADDON_DIR.join("config.toml");
        log::debug!("Attempting to load the config file at \"{config_path:?}\".");
        let mut file_data = read_to_string(config_path).await?;
        json_strip_comments::strip(&mut file_data)?;
        let data: Self = toml::from_str(&file_data)?;
        Ok(data)
    }
    pub async fn save(&self) -> anyhow::Result<()> {
        let addon_dir = &*ADDON_DIR;
        create_dir_all(&addon_dir).await?;
        let config_path = addon_dir.join("config.toml");
        log::debug!("Saving config path to \"{config_path:?}\".");
        let config = toml::to_string_pretty(&self)?;
        let mut file = File::create(config_path).await?;
        file.write_all(config.as_bytes()).await?;
        Ok(())
    }
    pub async fn toggle_3d_render(&mut self) {
        self.render_3d.toggle().await;
    }
    pub async fn set_progress_bar(&mut self, style: ProgressBarStyleChange) -> ProgressBarSettings {
        self.render_2d.set_progress_bar(style).await
    }
    pub async fn toggle_timer(&mut self, source: &RemoteSource, timer: String) {
        if let Some(timers) = self.timers.iter_mut().find(|t| t.source == *source) {
            timers.toggle_timer(timer);
        }
    }
    pub async fn count_all_disabled_timers(&self) -> usize {
        self.timers.iter().map(|t| t.count_disabled()).sum()
    }
    pub async fn count_source_disabled_timers(&mut self, source: &RemoteSource) -> usize {
        if let Some(source) = self.timers.iter().find(|t| t.source == *source) {
            source.count_disabled()
        } else {
            0
        }
    }

}

#[derive(Deserialize, Serialize, Default, Debug)]
struct Render3DConfig {
    #[serde(default)]
    enable: bool,
}

impl Render3DConfig {
    pub async fn toggle(&mut self) {
        self.enable = !self.enable;
    }
}

#[derive(Deserialize, Serialize, Default, Debug)]
struct Render2DConfig {
    progress_bar: ProgressBarSettings,
}

impl Render2DConfig {
    pub async fn set_progress_bar(&mut self, style: ProgressBarStyleChange) -> ProgressBarSettings {
        use ProgressBarStyleChange::*;
        match style {
            Centre(t) => self.progress_bar.set_centre_after(t),
            Stock(t) => self.progress_bar.set_stock(t),
            Shadow(t) => self.progress_bar.set_shadow(t),
            Height(h) => self.progress_bar.set_height(h),
            Font(f) => self.progress_bar.set_font(f),
        }
        self.progress_bar.clone()
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct TimerSourceConfig {
    source: RemoteSource,
    per_timer_config: HashMap<String, PerTimerConfig>,
}

impl TimerSourceConfig {
    pub fn count_disabled(&self) -> usize {
        self.per_timer_config.values().filter(|x| x.disabled).count()
    }

    pub async fn toggle_timer(&mut self, timer: String) -> bool {
        let entry = self.per_timer_config.entry(timer.clone()).or_default();
        let new_state = entry.toggle();
        let irrelevant = entry == &Default::default();
        if irrelevant {
            self.per_timer_config.remove(&timer);
        }
        new_state
    }
    pub async fn disable_timer(&mut self, timer: String) {
        if let Some(entry_mut) = self.per_timer_config.get_mut(&timer) {
            entry_mut.disable();
        } else {
            self.per_timer_config.insert(timer, PerTimerConfig { disabled: true });
        }
    }
    pub async fn enable_timer(&mut self, timer: String) {
        if let Some(entry_mut) = self.per_timer_config.get_mut(&timer) {
            entry_mut.enable();
        } else {
            self.per_timer_config.insert(timer, PerTimerConfig::default());
        }
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
struct PerTimerConfig {
    #[serde(default)]
    disabled: bool,
}

impl PerTimerConfig {
    pub fn disable(&mut self) {
        self.disabled = true;
    }
    pub fn enable(&mut self) {
        self.disabled = false;
    }
    pub fn toggle(&mut self) -> bool {
        self.disabled = !self.disabled;
        self.disabled
    }
}

