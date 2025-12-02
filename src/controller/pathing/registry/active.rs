use {
    crate::{
        controller::{
            Controller,
            pathing::{
                festivals::FestivalFixup,
                registry::{
                    PackPath, PackIndex,
                    PackInfo,
                    PackConfig,
                    TrailIndex,
                    LoadedPack, UnloadedReason,
                },
                state::shared::SharedPacks,
            },
        },
        exports::runtime as rt,
        settings::SettingsLock,
    },
    anyhow::{anyhow, Context},
    std::{collections::BTreeMap, error::Error as StdError, fmt, future::Future, path::{Path, PathBuf}, sync::{Arc, Weak}},
    taimi_pack::{
        attributes::Festival, category::id::AsFullId, loader::{DirectoryLoader, PackLoaderContext, ZipLoader}, trail::TrailData, Pack
    }, tokio::sync::{watch, Mutex},
};

impl LoadedPack {
    pub fn activate_start(&mut self) -> anyhow::Result<Option<(PackFormat, String, Option<bool>)>> {
        if self.active.is_some() {
            return Ok(None)
        }

        let (format, context) = match &self.info.info {
            Ok(info) => (
                Some(info.format),
                format!("Reloading pack {info}"),
            ),
            Err(UnloadedReason::Pending) => {
                let name = self.info.path.file_name()
                    .map(Path::new)
                    .unwrap_or_else(|| rt::relative_path(&self.info.path));
                (
                    PackFormat::guess_for_path(&self.info.path),
                    format!("Loading pack {}", name.display()),
                )
            },
            Err(_reason) => return Ok(None),
        };

        match format {
            Some(format) => {
                let wants_config = match &self.config {
                    None => Some(true),
                    #[cfg(todo)]
                    Some(config) if !config.borrow().is_empty() => Some(false),
                    _ => None,
                };
                Ok(Some((format, context, wants_config)))
            },
            None => {
                self.info.info = Err(UnloadedReason::UnknownFormat);
                Err(anyhow!("unknown pack format").context(context))
            },
        }
    }

    pub async fn activate_load((format, context, wants_config): (PackFormat, String, Option<bool>), path: PathBuf, manager: &PackLoader) -> anyhow::Result<(Arc<ActivePack>, Arc<PackInfo>, Option<PackConfig>)> {
        let loader = format.loader_for_path(path).await;
        let pack = match loader {
            Ok(loader) => manager.load_pack(loader).await,
            Err(e) => Err(e),
        }.context(context);
        let res = pack.map(|pack| {
            let info = PackInfo::from_pack(&pack.pack, format);
            (pack, Arc::new(info))
        });

        match res {
            Ok((pack, info)) => {
                let settings = match wants_config {
                    Some(true) => Some(manager.settings.read().await),
                    None => manager.settings.try_read().ok(),
                    Some(false) => None,
                };
                let config = settings.map(|settings| {
                    let mut config = PackConfig::default();
                    config.fill_settings(&pack.pack, &settings.pathing(), &settings.disabled_paths);
                    config
                });
                Ok((Arc::new(pack), info, config))
            },
            Err(e) => Err(e),
        }
    }

    pub fn activate_finish(&mut self, pack: anyhow::Result<(Arc<ActivePack>, Arc<PackInfo>, Option<PackConfig>)>, manager: &PackLoader) -> anyhow::Result<()> {
        match pack {
            Err(e) => {
                #[cfg(todo)]
                let e = e.into_boxed_dyn_error().into();
                let e: Arc<dyn StdError + Send + Sync> = Box::<dyn StdError + Send + Sync>::from(e).into();
                self.info.info = Err(UnloadedReason::LoadingFailed(e.clone()));
                manager.shared.update_pack_info(self.info.index, &self.info);
                Err(e.into())
            },
            Ok((pack, info, config)) => {
                self.info.info = Ok(info);
                let active = self.active.insert(pack);
                if let Some(config) = config {
                    let update_config = self.config.is_none();
                    Self::try_update_config_inner(&mut self.config, config);
                    if update_config {
                        manager.shared.update_pack_config(self.info.index, self.config.as_ref());
                    }
                }
                manager.shared.update_pack_active(self.info.index, Some(active));
                Ok(())
            },
        }
    }

    pub async fn activate(&mut self, manager: &PackLoader) -> anyhow::Result<Option<()>> {
        let activate = match self.activate_start()? {
            None => return Ok(None),
            Some(activate) => activate,
        };
        let res = Self::activate_load(activate, self.info.path.to_path_buf(), manager).await;
        self.activate_finish(res, manager).map(Some)
    }

    #[cfg(todo = "unused")]
    pub fn try_update_config(&mut self, config: PackConfig) {
        Self::try_update_config_inner(&mut self.config, config)
    }
    fn try_update_config_inner(out: &mut Option<watch::Sender<Arc<PackConfig>>>, config: PackConfig) {
        match out {
            out @ None => {
                let _ = out.insert(watch::Sender::new(Arc::new(config)));
            },
            Some(out) => {
                out.send_if_modified(|out| {
                    if **out == config {
                        return false
                    }
                    *Arc::make_mut(out) = config;
                    true
                });
            },
        }
    }
    #[cfg(todo)]
    pub async fn try_populate_config(&mut self, settings: &SettingsLock) -> bool {
        #[cfg(todo = "unnecessary")]
        let Ok(info) = &self.info else { return false };
        let Some(active) = &self.active else { return false };
        Self::populate_config(&mut self.config, active, settings);
        true
    }
    #[cfg(todo)]
    pub async fn populate_config(out: &mut Option<watch::Sender<Arc<PackConfig>>>, active: &ActivePack, settings: &SettingsLock) {
        let mut config = PackConfig::default();
        {
            let settings = settings.write().await;
            config.fill_settings(&active.pack, &settings.pathing(), &settings.disabled_paths);
        }
        Self::try_update_config_inner(out, config)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackFormat {
    TacoZip,
    TacoDir,
}

impl PackFormat {
    pub fn guess_for_path(path: &Path) -> Option<Self> {
        let is_taco = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("taco") || e.eq_ignore_ascii_case("zip"));
        match is_taco {
            _ if path.is_dir() => Some(Self::TacoDir),
            Some(true) => Some(Self::TacoZip),
            _ => None,
        }
    }

    pub async fn loader_for_path(&self, path: PathBuf) -> anyhow::Result<LoaderBox> {
        match self {
            Self::TacoZip => {
                let context = "Loading TacO zip";
                let loader = move || ZipLoader::new(&path)
                    .with_context(|| {
                        let path = path.file_name()
                            .map(Path::new)
                            .unwrap_or_else(|| rt::relative_path(&path));
                        format!("{context} {}", path.display())
                    });
                Controller::try_run_blocking(context, loader).await
                    .map(|loader| Box::new(loader) as LoaderBox)
            },
            Self::TacoDir => Ok(Box::new(DirectoryLoader::new(path))),
        }
    }
}

impl fmt::Display for PackFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::TacoZip =>
                f.write_str("TacO zip archive"),
            Self::TacoDir =>
                f.write_str("TacO folder"),
        }
    }
}

#[derive(Debug)]
pub struct PackLoader {
    pub settings: SettingsLock,
    pub festival_categories: BTreeMap<&'static str, Festival>,

    pub shared: SharedPacks,
}

impl PackLoader {
    pub fn new(settings: SettingsLock) -> Self {
        Self {
            settings,
            festival_categories: FestivalFixup::festival_categories(),
            shared: SharedPacks::new(),
        }
    }

    pub async fn load_pack(&self, mut loader: LoaderBox) -> anyhow::Result<ActivePack> {
        let context = "loading TacO pack";
        let (mut pack, loader) = Controller::try_run_blocking(context, move || Pack::load(&mut loader)
            .context(context)
            .map(move |pack| (pack, loader))
        ).await?;

        self.fixup_pack(&mut pack);
        Ok(ActivePack::with_pack(Arc::new(pack), loader))
    }

    fn fixup_pack(&self, pack: &mut Pack) {
        let mut fixed_festival_categories = BTreeMap::new();
        for (_name, category) in &mut pack.categories.all_categories {
            let is_festival = FestivalFixup::FESTIVAL_PREFIXES
                .iter()
                .copied()
                .find(|&prefix| category.full_id.id_starts_with(prefix));
            match is_festival {
                Some(prefix) if category.full_id.as_id() != prefix => (),
                _ => continue,
            }
            let festival = self
                .festival_categories
                .iter()
                .find_map(|(&prefix, &fest)| category.full_id.id_starts_with(prefix).then_some(fest));
            if let Some(festival) = festival {
                let festivals = Arc::make_mut(&mut category.marker_attributes).festivals.insert(festival.into());
                fixed_festival_categories.insert(&category.full_id, festivals.clone());
            } else {
                log::info!("unrecognized festival category: `{}`", category.full_id);
            }
        }
        if !fixed_festival_categories.is_empty() {
            // TODO: this should be less necessary once a tree of attribute inherits exist...
            let pois = pack.pois.iter_mut().filter_map(|poi| match fixed_festival_categories.get(poi.category.as_id()) {
                Some(f) => Some((poi, f)),
                None => None,
            });
            for (poi, f) in pois {
                if poi.attributes.festivals.is_none() {
                    poi.attributes.festivals = Some(f.clone());
                }
            }
            let trails = pack.trails.iter_mut().filter_map(|trail| match fixed_festival_categories.get(trail.category.as_id()) {
                Some(f) => Some((trail, f)),
                None => None,
            });
            for (trail, f) in trails {
                if trail.attributes.festivals.is_none() {
                    trail.attributes.festivals = Some(f.clone());
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct ActivePack {
    pub pack: Arc<Pack>,
    pub loader: Arc<Mutex<LoaderBox>>,
}

impl ActivePack {
    pub fn with_pack(pack: Arc<Pack>, loader: LoaderBox) -> Self {
        Self {
            pack,
            loader: Arc::new(Mutex::new(loader)),
        }
    }

    pub fn read_trail_data(&self, index: TrailIndex) -> anyhow::Result<TrailData> {
        let Some(trail) = self.pack.trails.get(index as usize) else {
            anyhow::bail!("Trail #{index} not found in {self}")
        };

        let mut loader = self.loader.blocking_lock();
        trail.read_trl_data(&mut *loader)
            .with_context(|| format!("Reading trail {trail} vertices from {self}"))
    }

    pub fn load_trail_data(&self, index: TrailIndex) -> impl Future<Output = anyhow::Result<TrailData>> + Send + 'static {
        let pack = self.clone();
        Controller::try_run_blocking("reading trl", move || pack.read_trail_data(index))
    }

    pub fn iter_weak<'a>(active: &'a [Weak<Self>]) -> impl Iterator<Item = (PackPath, &'a Weak<Self>)> {
        active.into_iter().enumerate().map(|(i, a)| (
            PackPath::with_path(i as PackIndex),
            a,
        ))
    }
    pub fn iter_strong<'a>(active: &'a [Weak<Self>]) -> impl Iterator<Item = (PackPath, Arc<Self>)> + 'a {
        active.into_iter().enumerate().filter_map(|(i, a)| a.upgrade().map(|a| (
            PackPath::with_path(i as PackIndex),
            a,
        )))
    }
    pub fn enum_strong<'a>(active: &'a [Weak<Self>]) -> impl Iterator<Item = (PackPath, bool)> + 'a {
        active.into_iter().enumerate().map(|(i, a)| (
            PackPath::with_path(i as PackIndex),
            a.strong_count() > 0,
        ))
    }
}

impl fmt::Display for ActivePack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.pack.name, f)
    }
}
impl fmt::Debug for ActivePack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ActivePack")
            .field(&self.pack)
            .finish()
    }
}
impl AsRef<ActivePack> for ActivePack {
    fn as_ref(&self) -> &ActivePack {
        self
    }
}
impl AsRef<Pack> for ActivePack {
    fn as_ref(&self) -> &Pack {
        &self.pack
    }
}

pub type LoaderBox = Box<dyn PackLoaderContext + Send + 'static>;
