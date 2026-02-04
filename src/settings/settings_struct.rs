use {
    super::{ArcSettings, PathingSettings, ProgressBarSettings, RemoteState, SourceKind, TimerSettings},
    crate::{
        controller::timers::ProgressBarStyleChange,
        exports::runtime::{self as rt, bindings::TaimiControls},
        settings::{
            state::{save_state_backup, UiState},
            IconStyle,
        },
        SETTINGS,
    },
    anyhow::Context,
    chrono::{DateTime, Utc},
    futures::stream::{self, StreamExt},
    magic_migrate::TryMigrate,
    nexus::imgui::Ui,
    serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        collections::{HashMap, HashSet},
        fmt::{self},
        io,
        mem::take,
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::Poll,
    },
    strum_macros::EnumIter,
    tokio::{
        fs::{create_dir_all, read_dir, read_to_string, try_exists, File},
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
    /// TODO: delete this field soon (migrate post-de or don't bother?)
    #[serde(default)]
    #[deprecated]
    pub pathing_window_open: bool,
    #[serde(default)]
    pub markers_window_open: bool,
    #[serde(default)]
    pub progress_bar: ProgressBarSettings,
    #[serde(default)]
    pub enable_katrender: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi_scaling: Option<f32>,
    #[serde(
        default = "TaimiControls::default_quick_access",
        skip_serializing_if = "TaimiControls::is_default_quick_access"
    )]
    pub quick_access_visible: TaimiControls,
    #[serde(default, skip_serializing_if = "IconStyle::is_default")]
    pub quick_access_style: IconStyle,
    #[serde(default)]
    pub marker_autoplace: MarkerAutoPlaceSettings,
    #[serde(default)]
    pub disabled_paths: Arc<HashSet<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc: Option<ArcSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pathing: Option<PathingSettings>,
    #[serde(default, skip_serializing_if = "UiState::is_empty")]
    pub ui_state: UiState,
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
        let state = state.unwrap_or_else(|| !self.get_window_state(window).unwrap_or_default());
        if self.update_window_state(window, state).is_err() {
            log::error!("unsupported window: {window}");
        }
    }

    /// consider an enum...
    pub fn get_window_state(&self, window: &str) -> Option<bool> {
        Some(match window {
            crate::WINDOW_PRIMARY => self.primary_window_open,
            crate::WINDOW_TIMERS => self.timers_window_open,
            crate::WINDOW_MARKERS => self.markers_window_open,
            crate::WINDOW_PATHING => self.ui_state.pathing_window.read().window.open.is_active(),
            _ => return None,
        })
    }
    /// TODO: remove &mut from this once all windows move to a sync lock like pathing
    pub fn update_window_state(&mut self, window: &str, open: bool) -> Result<(), ()> {
        let dest = match window {
            crate::WINDOW_PRIMARY => &mut self.primary_window_open,
            crate::WINDOW_TIMERS => &mut self.timers_window_open,
            crate::WINDOW_MARKERS => &mut self.markers_window_open,
            crate::WINDOW_PATHING => {
                let pathing_window = &self.ui_state.pathing_window;
                pathing_window.write_with(|pathing| pathing.window.open = open.into());
                return Ok(())
            },
            _ => return Err(()),
        };
        *dest = open;
        Ok(())
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
            self.markers.insert(marker, MarkerSettings { disabled: true });
        }
    }
    pub fn enable_marker(&mut self, marker: String) {
        if let Some(entry_mut) = self.markers.get_mut(&marker) {
            entry_mut.enable();
        } else {
            self.markers.insert(marker, MarkerSettings::default());
        }
    }

    pub fn set_marker_autoplace_settings(&mut self, maps: &MarkerAutoPlaceSettings) -> anyhow::Result<()> {
        self.marker_autoplace = maps.clone();
        Ok(())
    }

    pub async fn download_latest(settings: SettingsLock, mut state: RemoteState) -> anyhow::Result<()> {
        let res = state.source().download_latest(state.kind).await.with_context(|| {
            format!(
                "{} datasource {} failed to install",
                state.kind,
                state.source().display_name()
            )
        });
        let mut err = None;
        if let (Ok((_, install_dest)), Some(old_install)) = (&res, &state.installed_path) {
            if old_install != install_dest
                && rt::relative_path(old_install) != rt::relative_path(&install_dest)
            {
                let res = state.remove().await.with_context(|| {
                    format!(
                        "Manual clean-up of prior {} install may be required",
                        state.source().name()
                    )
                });
                if let Err(e) = res {
                    log::warn!("{e:#}");
                    err = Some(format!("{e}"));
                }
            }
        }
        {
            let mut settings = settings.write().await;
            let state = match RemoteState::lookup_datasource_mut(
                &mut settings.remotes,
                state.kind,
                &state.datasource_name(),
            ) {
                Some(remote) => {
                    // XXX: be careful of clobbering fields here, but it should be a perfect clone...
                    *remote = state;
                    remote
                },
                None => {
                    settings.remotes.push(state);
                    settings.remotes.last_mut().unwrap()
                },
            };
            let err = err.map(Err).unwrap_or(Ok(()));
            match res {
                Ok((tag_name, install_dest)) => {
                    state.commit_downloaded(Some(tag_name), Some(install_dest), err);
                    Ok(())
                },
                Err(e) => {
                    state.commit_downloaded(None, None, Err(format!("{e:#}")));
                    Err(e)
                },
            }
        }
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

    pub async fn check_for_updates<F>(settings: SettingsLock, mut filter: F) -> anyhow::Result<()>
    where
        F: FnMut(&RemoteState) -> bool,
    {
        // XXX: this should just stay as a method on the controller...
        use crate::controller::Controller;

        let sources: Vec<_> = settings
            .read()
            .await
            .remotes
            .iter()
            .filter(move |r| filter(r))
            .map(|remote| {
                let name = remote.datasource_name().clone().into_owned();
                let source = Controller::with_datasource(remote.kind, &name, |source| Some(source.clone()));
                (
                    (remote.kind, name),
                    source.unwrap_or_else(|| remote.source.clone()),
                )
            })
            .collect();
        let updates: Vec<_> = tokio_stream::iter(sources)
            .then(|((kind, name), source)| async move {
                let latest = source.as_source().latest_id().await;
                ((kind, name), latest)
            })
            .collect()
            .await;
        let save = {
            let mut settings_write_lock = settings.write().await;
            for ((kind, name), latest) in updates {
                let Some(state) =
                    RemoteState::lookup_datasource_mut(&mut settings_write_lock.remotes, kind, &name)
                else {
                    continue
                };
                let nu = state.get_needs_update(latest);
                log::info!("{} update state: {}", name, nu);
                state.needs_update = nu;
            }
            settings_write_lock.last_checked = Some(Utc::now());
            settings_write_lock.start_save().await?
        };
        Self::save_to(&save).await
    }

    pub async fn read_source_dir(
        settings: SettingsLock,
        kind: SourceKind,
    ) -> impl stream::Stream<Item = io::Result<(PathBuf, Option<String>)>> {
        let path = kind.get_unpack_dir();
        // TODO: just poll this inline, why bother prior to poll_next...
        let mut dir = read_dir(&path).await.map_err(Some);

        let root_relative = rt::relative_path(&path);
        let mut source_paths: HashMap<PathBuf, String> = settings
            .read_owned()
            .await
            .remotes
            .iter()
            .filter(|remote| remote.kind == kind)
            .filter_map(|remote| {
                remote.installed_path.as_ref().and_then(|installed| {
                    let relative = if installed.is_relative() {
                        installed.strip_prefix(root_relative)
                    } else {
                        installed.strip_prefix(&path)
                    };
                    #[cfg(todo)]
                    if installed.parent().is_some() {
                        // what, are we supposed to care about recursion?
                        return None
                    }
                    relative
                        .ok()
                        .map(|rel| (rel.to_owned(), remote.datasource_name().into_owned()))
                })
            })
            .collect();
        // after iterating over all contents, we'll spit out any remaining sources
        // (they may be installed outside of this dir, and that's okay!)
        let mut external_sources = None;

        stream::poll_fn(move |cx| {
            let dir = match &mut dir {
                Ok(dir) => dir,
                Err(e) =>
                    return match e.take() {
                        Some(e) if e.kind() == io::ErrorKind::NotFound => Poll::Ready(None),
                        Some(e) => Poll::Ready(Some(Err(e))),
                        None => Poll::Ready(None),
                    },
            };
            let entry = match dir.poll_next_entry(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(None)) =>
                    return Poll::Ready({
                        let sources =
                            external_sources.get_or_insert_with(|| take(&mut source_paths).into_iter());
                        sources.next().map(|(path, source)| Ok((path, Some(source))))
                    }),
                Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(Some(entry))) => entry,
            };
            let path = entry.path();
            let source = path
                .file_name()
                .and_then(|file_name| source_paths.remove(Path::new(&file_name)));
            Poll::Ready(Some(Ok((path, source))))
        })
    }

    pub fn new(addon_dir: &Path) -> Self {
        Self {
            last_checked: None,
            addon_dir: addon_dir.to_path_buf(),
            dirty: Arc::new(AtomicBool::new(false)),
            timers: Default::default(),
            markers: Default::default(),
            remotes: Default::default(),
            progress_bar: Default::default(),
            timers_window_open: false,
            pathing_window_open: false,
            markers_window_open: false,
            primary_window_open: false,
            enable_katrender: false,
            dpi_scaling: None,
            quick_access_visible: TaimiControls::default_quick_access(),
            quick_access_style: Default::default(),
            marker_autoplace: Default::default(),
            disabled_paths: Default::default(),
            pathing: Default::default(),
            arc: Default::default(),
            ui_state: Default::default(),
        }
    }
    pub fn file_path(addon_dir: &Path) -> PathBuf {
        addon_dir.join("settings.json")
    }
    pub async fn load(addon_dir: &Path) -> anyhow::Result<Self> {
        let settings_path = Self::file_path(addon_dir);
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
        let settings_path = Self::file_path(addon_dir);
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
        let res = Settings::load(addon_dir).await.context("SettingsLock load error");
        match res {
            Ok(settings) => settings,
            Err(err) => {
                log::error!("{err:#}");
                save_state_backup(&Self::file_path(addon_dir));
                Self::new(addon_dir)
            },
        }
    }

    pub async fn load_access(addon_dir: &Path) -> SettingsLock {
        Arc::new(RwLock::new(Self::load_default(addon_dir).await))
    }

    pub async fn settings_path(&self) -> anyhow::Result<PathBuf> {
        let addon_dir = &self.addon_dir;
        create_dir_all(addon_dir).await?;
        Ok(Self::file_path(addon_dir))
    }

    pub fn settings_str(&self) -> anyhow::Result<String> {
        serde_json::to_string(self).context("settings serialization error")
    }

    pub async fn start_save(&self) -> anyhow::Result<SettingsSave> {
        #[cfg(todo)]
        {
            // just don't have enough control over external types .-.
            self.ui_state.mark_clean();
        }
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
            Ok(mut file) => file.write_all(settings_str.as_bytes()).await,
            Err(e) => Err(e),
        };

        if res.is_err() {
            dirty.store(true, Ordering::Relaxed);
        }

        Ok(())
    }

    pub fn mark_dirty(&mut self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.ui_state.mark_clean();
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed) || self.is_state_dirty()
    }
    fn is_state_dirty(&self) -> bool {
        self.ui_state.is_dirty()
    }

    pub async fn try_commit() -> anyhow::Result<Option<SettingsSave>> {
        let settings = match Self::try_read() {
            None => return Ok(None),
            Some(s) => s,
        };
        let dirty = settings.dirty.swap(false, Ordering::SeqCst);
        if !dirty && !settings.is_state_dirty() {
            return Ok(None)
        }

        settings.start_save().await.map(Some)
    }

    pub fn try_read() -> Option<tokio::sync::RwLockReadGuard<'static, Self>> {
        SETTINGS.get().and_then(|settings| settings.try_read().ok())
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
        let mut res = SETTINGS.get().and_then(|settings| settings.try_write().ok());

        if let Some(settings) = &mut res {
            settings.mark_dirty();
        }

        res
    }

    pub async fn async_read() -> anyhow::Result<tokio::sync::RwLockReadGuard<'static, Self>> {
        let settings = match SETTINGS.get() {
            Some(settings) => settings.read().await,
            None => anyhow::bail!("SETTINGS not loaded"),
        };

        Ok(settings)
    }

    pub async fn async_write() -> anyhow::Result<tokio::sync::RwLockWriteGuard<'static, Self>> {
        let mut settings = match SETTINGS.get() {
            Some(settings) => settings.write().await,
            None => anyhow::bail!("SETTINGS not loaded"),
        };

        settings.mark_dirty();

        Ok(settings)
    }

    pub fn write_with_blocking<R, F: FnOnce(&mut Self) -> R>(f: F) -> anyhow::Result<R> {
        let mut settings = match SETTINGS.get() {
            Some(settings) => settings.blocking_write(),
            None => anyhow::bail!("SETTINGS not loaded"),
        };

        settings.mark_dirty();

        Ok(f(&mut *settings))
    }

    pub fn disabled_paths_mut(&mut self) -> &mut HashSet<String> {
        self.mark_dirty();
        Arc::make_mut(&mut self.disabled_paths)
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
