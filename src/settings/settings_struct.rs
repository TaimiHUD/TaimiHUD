use {
    super::{ArcSettings, PathingSettings, ProgressBarSettings, RemoteSource, RemoteState, Source, SourceKind, TimerSettings},
    crate::{
        controller::timers::ProgressBarStyleChange,
        exports::runtime::bindings::TaimiControls,
        SETTINGS, SOURCES,
    },
    anyhow::{anyhow, Context},
    chrono::{DateTime, Utc},
    futures::stream::StreamExt,
    magic_migrate::TryMigrate,
    nexus::imgui::Ui,
    serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        collections::{HashMap, HashSet},
        fmt::{self},
        path::{Path, PathBuf},
        sync::{Arc, atomic::{AtomicBool, Ordering}},
    },
    strum_macros::EnumIter,
    tokio::{
        fs::{create_dir_all, read_to_string, try_exists, File},
        io::AsyncWriteExt,
        sync::RwLock,
    },
};

pub type SettingsLock = Arc<RwLock<Settings>>;
pub type SettingsSave = (PathBuf, String, Arc<AtomicBool>);
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
    #[serde(skip)]
    dirty: Arc<AtomicBool>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi_scaling: Option<f32>,
    #[serde(default = "TaimiControls::default_quick_access", skip_serializing_if = "TaimiControls::is_default_quick_access")]
    pub quick_access_visible: TaimiControls,
    #[serde(default)]
    pub marker_autoplace: MarkerAutoPlaceSettings,
    #[serde(default)]
    pub disabled_paths: HashSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc: Option<ArcSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pathing: Option<PathingSettings>,
}

impl Settings {
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

    pub fn set_window_state(&mut self, window: &str, state: Option<bool>) {
        let Some(window_open) = self.get_window_state_mut(window) else {
            log::error!("unsupported window: {window}");
            return
        };

        match state {
            Some(s) => {
                *window_open = s;
            }
            None => {
                *window_open = !*window_open;
            }
        }
    }

    /// consider an enum...
    pub fn get_window_state_mut(&mut self, window: &str) -> Option<&mut bool> {
        Some(match window {
            crate::WINDOW_PRIMARY => &mut self.primary_window_open,
            crate::WINDOW_TIMERS => &mut self.timers_window_open,
            crate::WINDOW_MARKERS => &mut self.markers_window_open,
            crate::WINDOW_PATHING => &mut self.pathing_window_open,
            _ => return None,
        })
    }

    pub fn toggle_timer(&mut self, timer: String) -> bool {
        let entry = self.timers.entry(timer.clone()).or_default();
        let new_state = entry.toggle();
        let irrelevant = entry == &Default::default();
        if irrelevant {
            self.timers.remove(&timer);
        }
        new_state
    }
    pub fn disable_timer(&mut self, timer: String) {
        if let Some(entry_mut) = self.timers.get_mut(&timer) {
            entry_mut.disable();
        } else {
            self.timers.insert(timer, TimerSettings { disabled: true });
        }
    }
    pub fn enable_timer(&mut self, timer: String) {
        if let Some(entry_mut) = self.timers.get_mut(&timer) {
            entry_mut.enable();
        } else {
            self.timers.insert(timer, TimerSettings::default());
        }
    }
    pub fn toggle_marker(&mut self, marker: String) -> bool {
        let entry = self.markers.entry(marker.clone()).or_default();
        let new_state = entry.toggle();
        new_state
    }
    pub fn disable_marker(&mut self, marker: String) {
        if let Some(entry_mut) = self.markers.get_mut(&marker) {
            entry_mut.disable();
        } else {
            self.markers
                .insert(marker, MarkerSettings { disabled: true });
        }
    }
    pub fn enable_marker(&mut self, marker: String) {
        if let Some(entry_mut) = self.markers.get_mut(&marker) {
            entry_mut.enable();
        } else {
            self.markers.insert(marker, MarkerSettings::default());
        }
    }

    #[allow(dead_code)]
    pub fn get_status_for(&self, source: &RemoteSource) -> Option<&RemoteState> {
        self.remotes.iter().find(|dd| dd.source().name() == source.name())
    }

    pub fn get_status_for_mut(&mut self, source: &RemoteSource) -> Option<&mut RemoteState> {
        self.remotes.iter_mut().find(|dd| dd.source().name() == source.name())
    }

    pub async fn uninstall_remote(&mut self, source: &RemoteSource) -> anyhow::Result<()> {
        if let Some(remote) = self.remotes.iter_mut().find(|dd| dd.source().name() == source.name()) {
            remote.uninstall().await?;
        }
        Ok(())
    }

    pub fn set_marker_autoplace_settings(
        &mut self,
        maps: &MarkerAutoPlaceSettings,
    ) -> anyhow::Result<()> {
        self.marker_autoplace = maps.clone();
        Ok(())
    }

    pub async fn download_latest(state: RemoteState) -> anyhow::Result<()> {
        let source = state.remote_source();
        let settings_arc = SETTINGS
            .get()
            .expect("SettingsLock should've been initialized by now!");
        let install_dir = {
            let settings_read_lock = settings_arc.read().await;
            settings_read_lock
                .addon_dir
                .join(source.install_dir())
        };
        let tag_name = source.download_latest(state.kind).await?;
        {
            let mut settings_write_lock = settings_arc.write().await;
            if let Some(dd_mut) = settings_write_lock.get_status_for_mut(&source) {
                let res = dd_mut.commit_downloaded(tag_name, install_dir).await;
                let _ = settings_write_lock
                    .save()
                    .await;
                res
            } else {
                Err(anyhow!("GitHub repository \"{}\" not found.", source))
            }
        }?;
        Ok(())
    }

    pub fn set_progress_bar(&mut self, style: ProgressBarStyleChange) -> ProgressBarSettings {
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

    pub fn toggle_katrender(&mut self) {
        self.enable_katrender = !self.enable_katrender;
    }

    pub async fn check_for_updates() -> anyhow::Result<()> {
        let settings_arc = SETTINGS
            .get()
            .expect("SettingsLock should've been initialized by now!");
        let sources: Vec<(RemoteSource, NeedsUpdate)> = {
            let settings_read_lock = settings_arc.read().await;
            tokio_stream::iter(settings_read_lock.remotes.iter())
                .then(|r| async move { (r.remote_source(), r.needs_update().await) })
                .collect()
                .await
        };
        {
            let mut settings_write_lock = settings_arc.write().await;
            for (source, nu) in sources {
                log::debug!("{} update state: {:?}", source, nu);
                if let Some(dd) = settings_write_lock.get_status_for_mut(&source) {
                    log::debug!("Found dd {} update state: {:?}", dd.source(), nu);
                    dd.needs_update = nu;
                }
            }
            settings_write_lock.last_checked = Some(Utc::now());
            settings_write_lock
                .save()
                .await?;
        }
        Ok(())
    }

    pub fn new(addon_dir: &Path) -> Self {
        Self {
            last_checked: None,
            addon_dir: addon_dir.to_path_buf(),
            dirty: Arc::new(AtomicBool::new(false)),
            timers: Default::default(),
            markers: Default::default(),
            remotes: RemoteState::suggested_sources().unwrap_or_default(),
            progress_bar: Default::default(),
            timers_window_open: false,
            pathing_window_open: false,
            markers_window_open: false,
            primary_window_open: false,
            enable_katrender: false,
            dpi_scaling: None,
            quick_access_visible: TaimiControls::default_quick_access(),
            marker_autoplace: Default::default(),
            disabled_paths: Default::default(),
            pathing: Default::default(),
            arc: Default::default(),
        }
    }
    pub async fn load(addon_dir: &Path) -> anyhow::Result<Self> {
        let settings_path = addon_dir.join("settings.json");
        if try_exists(&settings_path).await? {
            let file_data = read_to_string(settings_path).await?;
            let mut settings = serde_json::from_str::<Self>(&file_data)?;
            settings.addon_dir = addon_dir.to_path_buf();
            return Ok(settings);
        }
        Ok(Self::new(addon_dir))
    }
    pub fn open_blocking(addon_dir: &Path) -> anyhow::Result<Self> {
        use std::fs;
        let settings_path = addon_dir.join("settings.json");
        Ok(if fs::exists(&settings_path)? {
            let file_data = fs::read_to_string(settings_path)?;
            let mut settings = serde_json::from_str::<Self>(&file_data)?;
            settings.addon_dir = addon_dir.to_path_buf();
            settings
        } else {
            Self::new(addon_dir)
        })
    }

    pub async fn load_default(addon_dir: &Path) -> Self {
        match Settings::load(addon_dir).await {
            Ok(settings) => settings,
            Err(err) => {
                log::error!("SettingsLock load error: {}", err);
                Self::new(addon_dir)
            }
        }
    }

    pub async fn load_access(addon_dir: &Path) -> SettingsLock {
        Arc::new(RwLock::new(Self::load_default(addon_dir).await))
    }

    pub async fn settings_path(&self) -> anyhow::Result<PathBuf> {
        let addon_dir = &self.addon_dir;
        create_dir_all(addon_dir).await?;
        Ok(addon_dir.join("settings.json"))
    }

    pub fn settings_str(&self) -> anyhow::Result<String> {
        serde_json::to_string(self)
            .context("settings serialization error")
    }

    pub async fn start_save(&self) -> anyhow::Result<SettingsSave> {
        Ok((
            self.settings_path().await?,
            self.settings_str()?,
            self.dirty.clone(),
        ))
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        let save = self.start_save().await?;
        Self::save_to(&save).await
    }

    pub async fn save_to((settings_path, settings_str, dirty): &SettingsSave) -> anyhow::Result<()> {
        log::trace!("Settings: Saving to \"{:?}\".", settings_path);
        let res = match File::create(settings_path).await {
            Ok(mut file) =>
                file.write_all(settings_str.as_bytes()).await,
            Err(e) => Err(e),
        };

        if res.is_err() {
            dirty.store(true, Ordering::Relaxed);
        }

        Ok(())
    }

    pub fn mark_dirty(&mut self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    pub async fn try_commit() -> anyhow::Result<Option<SettingsSave>> {
        let settings = match Self::try_read() {
            None => return Ok(None),
            Some(s) => s,
        };
        let dirty = settings.dirty.swap(false, Ordering::SeqCst);
        if !dirty {
            return Ok(None)
        }

        settings.start_save().await.map(Some)
    }

    pub fn try_read() -> Option<tokio::sync::RwLockReadGuard<'static, Self>> {
        SETTINGS.get()
            .and_then(|settings| settings.try_read().ok())
    }

    pub fn read_with_blocking<R, F: FnOnce(&Self) -> R>(f: F) -> anyhow::Result<R> {
        let settings_lock;
        let settings_temporary;
        let settings = match SETTINGS.get() {
            Some(settings) => {
                settings_lock = settings.blocking_read();
                &*settings_lock
            },
            None => {
                settings_temporary = Self::open_blocking(&*crate::ADDON_DIR)?;
                &settings_temporary
            },
        };

        Ok(f(settings))
    }

    pub fn try_write() -> Option<tokio::sync::RwLockWriteGuard<'static, Self>> {
        let mut res = SETTINGS.get()
            .and_then(|settings| settings.try_write().ok());

        if let Some(settings) = &mut res {
            settings.mark_dirty();
        }

        res
    }

    pub async fn async_read() -> anyhow::Result<tokio::sync::RwLockReadGuard<'static, Self>> {
        let settings = match SETTINGS.get() {
            Some(settings) =>
                settings.read().await,
            None =>
                anyhow::bail!("SETTINGS not loaded"),
        };

        Ok(settings)
    }

    pub async fn async_write() -> anyhow::Result<tokio::sync::RwLockWriteGuard<'static, Self>> {
        let mut settings = match SETTINGS.get() {
            Some(settings) =>
                settings.write().await,
            None =>
                anyhow::bail!("SETTINGS not loaded"),
        };

        settings.mark_dirty();

        Ok(settings)
    }

    pub fn write_with_blocking<R, F: FnOnce(&mut Self) -> R>(f: F) -> anyhow::Result<R> {
        let mut settings = match SETTINGS.get() {
            Some(settings) =>
                settings.blocking_write(),
            None =>
                anyhow::bail!("SETTINGS not loaded"),
        };

        settings.mark_dirty();

        Ok(f(&mut *settings))
    }

    pub fn arc(&self) -> Cow<ArcSettings> {
        match self.arc.as_ref() {
            Some(arc) => Cow::Borrowed(arc),
            None => Cow::Owned(Default::default()),
        }
    }

    pub fn arc_mut(&mut self) -> &mut ArcSettings {
        self.mark_dirty();
        self.arc.get_or_insert_default()
    }

    pub fn pathing(&self) -> Cow<PathingSettings> {
        match self.pathing.as_ref() {
            Some(pathing) => Cow::Borrowed(pathing),
            None => Cow::Owned(Default::default()),
        }
    }

    pub fn pathing_mut(&mut self) -> &mut PathingSettings {
        self.mark_dirty();
        self.pathing.get_or_insert_default()
    }
}
