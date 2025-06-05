use {
    super::{ProgressBarSettings, RemoteSource, RemoteState, Source, SourceKind, TimerSettings},
    crate::{controller::ProgressBarStyleChange, SETTINGS, SOURCES},
    anyhow::anyhow,
    chrono::{DateTime, Utc},
    futures::stream::StreamExt,
    magic_migrate::TryMigrate,
    nexus::imgui::Ui,
    serde::{Deserialize, Serialize},
    std::{
        collections::{HashMap, HashSet},
        fmt::{self},
        path::{Path, PathBuf},
        sync::Arc,
    },
    strum_macros::EnumIter,
    tokio::{
        fs::{create_dir_all, read_to_string, try_exists, File},
        io::AsyncWriteExt,
        sync::RwLock,
    },
};

pub type SettingsLock = Arc<RwLock<Settings>>;
#[derive(PartialEq, Clone, Debug, Default)]
pub enum NeedsUpdate {
    #[default]
    Unknown,
    Error(String),
    Known(bool, String),
}

impl fmt::Display for NeedsUpdate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use NeedsUpdate::*;
        match &self {
            Unknown => write!(f, "Unknown"),
            Error(e) => write!(f, "Error: {e}!"),
            Known(true, id) => write!(f, "Available: {}", id),
            Known(false, _id) => write!(f, "Up to date!"),
        }
    }
}

impl NeedsUpdate {
    pub fn draw(&self, ui: &Ui) {
        let text = self.to_string();
        use NeedsUpdate::*;
        match &self {
            Unknown => ui.text_colored([1.0, 1.0, 0.0, 1.0], text),
            Error(_e) => ui.text_colored([1.0, 0.0, 0.0, 1.0], text),
            Known(true, _id) => ui.text_colored([1.0, 0.6, 0.0, 1.0], text),
            Known(false, _id) => ui.text_colored([0.0, 1.0, 0.0, 1.0], text),
        }
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct MarkerSettings {
    #[serde(default)]
    pub disabled: bool,
}

impl MarkerSettings {
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

#[derive(PartialEq, Deserialize, Serialize, Default, Debug, Clone, EnumIter)]
pub enum SquadCondition {
    #[default]
    Always,
    IfCommander,
    IfLieutenantOrAbove,
    Never,
}

#[derive(PartialEq, Deserialize, Serialize, Default, Debug, Clone, EnumIter)]
pub enum MarkerAutoPlaceSettings {
    OpenWindow(SquadCondition),
    Place(SquadCondition),
    #[default]
    DoNothing,
}

impl fmt::Display for SquadCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SquadCondition::*;
        match &self {
            Always => write!(f, "Always do action"),
            IfCommander => write!(f, "Do action if squad commander"),
            IfLieutenantOrAbove => write!(f, "Do action if lieutenant or commander"),
            Never => write!(f, "Never do action"),
        }
    }
}

impl fmt::Display for MarkerAutoPlaceSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use MarkerAutoPlaceSettings::*;
        match &self {
            OpenWindow(_t) => write!(f, "Open the markers window"),
            Place(_t) => write!(f, "Place markers automatically"),
            DoNothing => write!(f, "Do nothing"),
        }
    }
}

#[derive(Deserialize, Serialize, TryMigrate, Default, Debug, Clone)]
#[try_migrate(from = None)]
pub struct Settings {
    #[serde(default)]
    pub last_checked: Option<DateTime<Utc>>,
    #[serde(skip)]
    addon_dir: PathBuf,
    #[serde(default)]
    pub timers: HashMap<String, TimerSettings>,
    #[serde(default)]
    pub markers: HashMap<String, MarkerSettings>,
    #[serde(default)]
    pub remotes: Vec<RemoteState>,
    #[serde(default)]
    pub primary_window_open: bool,
    #[serde(default)]
    pub timers_window_open: bool,
    #[serde(default)]
    pub pathing_window_open: bool,
    #[serde(default)]
    pub markers_window_open: bool,
    #[serde(default)]
    pub progress_bar: ProgressBarSettings,
    #[serde(default)]
    pub enable_katrender: bool,
    #[serde(default)]
    pub marker_autoplace: MarkerAutoPlaceSettings,
}

impl Settings {
    pub fn handle_sources_changes(&mut self) {
        log::debug!("Preparing to handle sources changes for settings");
        let sources = SOURCES.get().unwrap();
        let sources_lock = sources.read().unwrap();
        if let Some(timer_sources) = sources_lock.0.get(&SourceKind::Timers) {
            let sources_hashset: HashSet<&RemoteSource> = HashSet::from_iter(timer_sources.iter());
            let mut found_sources = HashSet::new();
            for source in timer_sources {
                let inner_source_local = source.source();
                if let Some(matching_remote) = self.remotes.iter_mut().find(|r| {
                    found_sources.insert(source);
                    let inner_source_remote = r.source();
                    inner_source_local.owner == inner_source_remote.owner
                        && inner_source_local.repository == inner_source_remote.repository
                }) {
                    if inner_source_local != matching_remote.source() {
                        matching_remote.update(Arc::new(source.clone()));
                    }
                }
            }
            let remaining = sources_hashset.symmetric_difference(&found_sources);
            let remaining_vec: Vec<_> = remaining
                .into_iter()
                .map(|s| RemoteState::new_from_source(s))
                .collect();
            self.remotes.extend(remaining_vec);
        }
        drop(sources_lock);
    }

    #[allow(dead_code)]
    pub fn update_sources_data(&mut self) {
        let all_sources = RemoteState::hardcoded_sources();
        let mut all_sources_data = RemoteState::hardcoded_sources();
        for (owner, repository, description) in all_sources {
            for remote in &mut self.remotes {
                let source = remote.source.source();
                if owner == source.owner && repository == source.repository {
                    //*remote = remote.clone().update(description);
                    all_sources_data.retain(|x| *x != (owner, repository, description));
                    //description));
                }
            }
        }
        for (owner, repository, description) in all_sources_data {
            self.remotes
                .push(RemoteState::new(owner, repository, description))
        }
    }

    #[allow(dead_code)]
    pub fn count_disabled_timers(&self) -> usize {
        self.timers.values().filter(|x| x.disabled).count()
    }

    #[allow(dead_code)]
    pub fn get_paths(&self) -> Vec<&PathBuf> {
        self.remotes
            .iter()
            .filter_map(|dd| dd.installed_path.as_ref())
            .collect()
    }

    pub async fn set_window_state(&mut self, window: &str, state: Option<bool>) {
        let window_open = match window {
            crate::WINDOW_PRIMARY => &mut self.primary_window_open,
            crate::WINDOW_TIMERS => &mut self.timers_window_open,
            crate::WINDOW_MARKERS => &mut self.markers_window_open,
            crate::WINDOW_PATHING => &mut self.pathing_window_open,
            _ => unreachable!("unsupported window"),
        };

        match state {
            Some(s) => {
                *window_open = s;
            }
            None => {
                *window_open = !*window_open;
            }
        }
        let _ = self.save(&self.addon_dir).await;
    }

    pub async fn toggle_timer(&mut self, timer: String) -> bool {
        let entry = self.timers.entry(timer.clone()).or_default();
        let new_state = entry.toggle();
        let irrelevant = entry == &Default::default();
        if irrelevant {
            self.timers.remove(&timer);
        }
        let _ = self.save(&self.addon_dir).await;
        new_state
    }
    pub async fn disable_timer(&mut self, timer: String) {
        if let Some(entry_mut) = self.timers.get_mut(&timer) {
            entry_mut.disable();
        } else {
            self.timers.insert(timer, TimerSettings { disabled: true });
        }
        let _ = self.save(&self.addon_dir).await;
    }
    pub async fn enable_timer(&mut self, timer: String) {
        if let Some(entry_mut) = self.timers.get_mut(&timer) {
            entry_mut.enable();
        } else {
            self.timers.insert(timer, TimerSettings::default());
        }
        let _ = self.save(&self.addon_dir).await;
    }
    pub async fn toggle_marker(&mut self, marker: String) -> bool {
        let entry = self.markers.entry(marker.clone()).or_default();
        let new_state = entry.toggle();
        let _ = self.save(&self.addon_dir).await;
        new_state
    }
    pub async fn disable_marker(&mut self, marker: String) {
        if let Some(entry_mut) = self.markers.get_mut(&marker) {
            entry_mut.disable();
        } else {
            self.markers
                .insert(marker, MarkerSettings { disabled: true });
        }
        let _ = self.save(&self.addon_dir).await;
    }
    pub async fn enable_marker(&mut self, marker: String) {
        if let Some(entry_mut) = self.markers.get_mut(&marker) {
            entry_mut.enable();
        } else {
            self.markers.insert(marker, MarkerSettings::default());
        }
        let _ = self.save(&self.addon_dir).await;
    }

    #[allow(dead_code)]
    pub async fn get_status_for(&self, source: &RemoteSource) -> Option<&RemoteState> {
        self.remotes.iter().find(|dd| *dd.source == *source)
    }

    pub async fn get_status_for_mut(&mut self, source: &RemoteSource) -> Option<&mut RemoteState> {
        self.remotes.iter_mut().find(|dd| *dd.source == *source)
    }

    pub async fn uninstall_remote(&mut self, source: &RemoteSource) -> anyhow::Result<()> {
        if let Some(remote) = self.remotes.iter_mut().find(|dd| *dd.source == *source) {
            remote.uninstall().await?;
        }
        let _ = self.save(&self.addon_dir).await;
        Ok(())
    }

    pub async fn set_marker_autoplace_settings(
        &mut self,
        maps: &MarkerAutoPlaceSettings,
    ) -> anyhow::Result<()> {
        self.marker_autoplace = maps.clone();
        let _ = self.save(&self.addon_dir).await;
        Ok(())
    }

    pub async fn download_latest(source: &RemoteSource) -> anyhow::Result<()> {
        let underlying_source = source.source();
        let settings_arc = SETTINGS
            .get()
            .expect("SettingsLock should've been initialized by now!");
        let install_dir = {
            let settings_read_lock = settings_arc.read().await;
            settings_read_lock
                .addon_dir
                .join(underlying_source.install_dir())
        };
        let tag_name = underlying_source.download_latest().await?;
        {
            let mut settings_write_lock = settings_arc.write().await;
            if let Some(dd_mut) = settings_write_lock.get_status_for_mut(source).await {
                let res = dd_mut.commit_downloaded(tag_name, install_dir).await;
                let _ = settings_write_lock
                    .save(&settings_write_lock.addon_dir)
                    .await;
                res
            } else {
                Err(anyhow!("GitHub repository \"{}\" not found.", source))
            }
        }?;
        Ok(())
    }

    pub async fn set_progress_bar(&mut self, style: ProgressBarStyleChange) -> ProgressBarSettings {
        use ProgressBarStyleChange::*;
        match style {
            Centre(t) => self.progress_bar.set_centre_after(t),
            Stock(t) => self.progress_bar.set_stock(t),
            Shadow(t) => self.progress_bar.set_shadow(t),
            Height(h) => self.progress_bar.set_height(h),
            Font(f) => self.progress_bar.set_font(f),
        }
        let _ = self.save(&self.addon_dir).await;
        self.progress_bar.clone()
    }

    pub async fn toggle_katrender(&mut self) {
        self.enable_katrender = !self.enable_katrender;
    }

    pub async fn check_for_updates() -> anyhow::Result<()> {
        let settings_arc = SETTINGS
            .get()
            .expect("SettingsLock should've been initialized by now!");
        let sources: Vec<(Arc<RemoteSource>, NeedsUpdate)> = {
            let settings_read_lock = settings_arc.read().await;
            tokio_stream::iter(settings_read_lock.remotes.iter())
                .then(|r| async move { (r.source.clone(), r.needs_update().await) })
                .collect()
                .await
        };
        {
            let mut settings_write_lock = settings_arc.write().await;
            for (source, nu) in sources {
                log::debug!("{} update state: {:?}", source, nu);
                if let Some(dd) = settings_write_lock.get_status_for_mut(&source).await {
                    log::debug!("Found dd {} update state: {:?}", dd.source, nu);
                    dd.needs_update = nu;
                }
            }
            settings_write_lock.last_checked = Some(Utc::now());
            settings_write_lock
                .save(&settings_write_lock.addon_dir)
                .await?;
        }
        Ok(())
    }

    pub async fn new(addon_dir: &Path) -> Self {
        Self {
            last_checked: None,
            addon_dir: addon_dir.to_path_buf(),
            timers: Default::default(),
            markers: Default::default(),
            remotes: RemoteState::suggested_sources().collect(),
            progress_bar: Default::default(),
            timers_window_open: false,
            pathing_window_open: false,
            markers_window_open: false,
            primary_window_open: false,
            enable_katrender: false,
            marker_autoplace: Default::default(),
        }
    }
    pub async fn load(addon_dir: &Path) -> anyhow::Result<Self> {
        let settings_path = addon_dir.join("settings.json");
        if try_exists(&settings_path).await? {
            let file_data = read_to_string(settings_path).await?;
            let mut settings = serde_json::from_str::<Self>(&file_data)?;
            settings.addon_dir = addon_dir.to_path_buf();
            settings.handle_sources_changes();
            return Ok(settings);
        }
        Ok(Self::new(addon_dir).await)
    }

    pub async fn load_default(addon_dir: &Path) -> Self {
        match Settings::load(addon_dir).await {
            Ok(settings) => settings,
            Err(err) => {
                log::error!("SettingsLock load error: {}", err);
                Self::new(addon_dir).await
            }
        }
    }

    pub async fn load_access(addon_dir: &Path) -> SettingsLock {
        Arc::new(RwLock::new(Self::load_default(addon_dir).await))
    }

    pub async fn save(&self, addon_dir: &Path) -> anyhow::Result<()> {
        create_dir_all(addon_dir).await?;
        let settings_path = addon_dir.join("settings.json");
        log::debug!("Settings: Saving to \"{:?}\".", settings_path);
        let settings_str = serde_json::to_string(self)?;
        let mut file = File::create(settings_path).await?;
        file.write_all(settings_str.as_bytes()).await?;
        Ok(())
    }
}
