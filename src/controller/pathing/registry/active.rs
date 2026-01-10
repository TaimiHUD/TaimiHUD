use {
    crate::{
        controller::{
            pathing::{
                festivals::FestivalFixup,
                registry::{PackConfig, PackInfo, PackInfoSignature},
                shared::{SharedPackConfig, SharedPackInfo, SharedPackLoaded},
                space::TrailParams,
                PathingShared,
            },
            Controller,
        },
        exports::runtime as rt,
        settings::{PathingSettings, SettingsLock},
    },
    anyhow::{anyhow, Context},
    std::{
        collections::BTreeMap,
        fmt,
        path::{Path, PathBuf},
        sync::Arc,
    },
    taimi_hoard::loc::LocationRef,
    taimi_meta::packs::PackPath,
    taimi_pack::{
        attributes::Festival,
        category::id::AsFullId,
        loader::{DirectoryLoader, PackLoaderContext, ZipLoader},
        Pack,
    },
    taimi_sync::watched::watch,
    tokio::sync::Mutex,
};

#[derive(Debug, Clone)]
pub struct PackActivateContext {
    pub format: PackFormat,
    pub path: PathBuf,
    pub context: String,
    #[allow(dead_code)]
    pub sig_prev: PackInfoSignature,
    pub config_sig_prev: PackInfoSignature,
}
impl PackActivateContext {
    /// fails if format cannot be guessed
    pub fn new<P>(path: P, format: Option<PackFormat>, prev_info: Option<&PackInfo>) -> anyhow::Result<Self>
    where
        P: AsRef<Path> + Into<PathBuf>,
    {
        let (format, sig_prev, context) = match prev_info {
            Some(info) => (
                Some(format.unwrap_or(info.format)),
                PackInfoSignature::from_info(info),
                format!("Reloading pack {info}"),
            ),
            None => {
                let path = path.as_ref();
                let name = path
                    .file_name()
                    .map(Path::new)
                    .unwrap_or_else(|| rt::relative_path(path));
                (
                    format.or_else(|| PackFormat::guess_for_path(path)),
                    PackInfoSignature::EMPTY,
                    format!("Loading pack {}", name.display()),
                )
            },
        };

        match format {
            Some(format) => Ok(Self {
                path: path.into(),
                format,
                context,
                config_sig_prev: sig_prev.clone(),
                sig_prev,
            }),
            None => Err(anyhow!("unknown pack format").context(context)),
        }
    }

    pub async fn load(self, manager: &PackLoader) -> anyhow::Result<PackActivateLoaded> {
        let loader = self
            .format
            .loader_for_path(self.path)
            .await
            .map(|loader| Arc::new(Mutex::new(loader)));
        let pack = match loader {
            Ok(loader) => manager.load_pack(loader.clone()).await.map(|p| (loader, p)),
            Err(e) => Err(e),
        }
        .context(self.context);
        let res = pack.map(|(l, pack)| {
            let info = PackInfo::from_pack(&pack, self.format);
            (l, Arc::new(pack), Arc::new(info))
        });

        match res {
            Ok((loader, pack, info)) => {
                let sig = PackInfoSignature::from_info(&info);
                let settings = match self.config_sig_prev {
                    prev if prev == sig => None,
                    PackInfoSignature::EMPTY => Some(manager.settings.read().await),
                    _prev => manager.settings.try_read().ok(),
                };
                let config = settings.map(|settings| {
                    let mut config = PackConfig::default();
                    config.fill_settings(&pack, &settings.pathing(), &settings.disabled_paths);
                    config
                });
                Ok(PackActivateLoaded { pack, loader, info, config })
            },
            Err(e) => Err(e),
        }
    }
}
#[derive(Clone)]
pub struct PackActivateLoaded {
    pub pack: Arc<Pack>,
    pub loader: SharedLoaderBox,
    pub info: Arc<PackInfo>,
    pub config: Option<PackConfig>,
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
                let loader = move || {
                    ZipLoader::new(&path).with_context(|| {
                        let path = path
                            .file_name()
                            .map(Path::new)
                            .unwrap_or_else(|| rt::relative_path(&path));
                        format!("{context} {}", path.display())
                    })
                };
                Controller::try_run_blocking(context, loader)
                    .await
                    .map(|loader| Box::new(loader) as LoaderBox)
            },
            Self::TacoDir => Ok(Box::new(DirectoryLoader::new(path))),
        }
    }
}

impl fmt::Display for PackFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::TacoZip => f.write_str("TacO zip archive"),
            Self::TacoDir => f.write_str("TacO folder"),
        }
    }
}

#[derive(Debug)]
pub struct PackLoader {
    pub settings: SettingsLock,
    pub festival_categories: BTreeMap<&'static str, Festival>,

    pub shared: Arc<PathingShared>,
}

impl PackLoader {
    pub fn new(shared: Arc<PathingShared>, settings: SettingsLock) -> Self {
        Self {
            settings,
            shared,
            festival_categories: FestivalFixup::festival_categories(),
        }
    }

    pub fn pack_info(&self, path: PackPath) -> Option<Arc<SharedPackInfo>> {
        self.shared
            .packs
            .packs
            .borrow()
            .lookup_ref(&path)
            .map(|i| i.info.clone())
    }
    pub fn pack_loaded(&self, path: PackPath) -> Option<watch::Sender<SharedPackLoaded>> {
        self.shared
            .packs
            .packs
            .borrow()
            .lookup_ref(&path)
            .map(|i| i.loaded.clone())
    }
    pub fn pack_config(&self, path: PackPath) -> Option<watch::Sender<SharedPackConfig>> {
        self.shared
            .packs
            .packs
            .borrow()
            .lookup_ref(&path)
            .map(|i| i.config.clone())
    }
    pub fn get_pack_loaded_data(&self, path: PackPath) -> Option<Arc<Pack>> {
        self.pack_loaded(path).and_then(|l| l.borrow().pack.clone())
    }

    pub async fn load_pack(&self, loader: SharedLoaderBox) -> anyhow::Result<Pack> {
        let mut pack = Self::load_pack_data(loader).await?;

        self.fixup_pack(&mut pack);
        Ok(pack)
    }
    pub async fn load_pack_data(loader: SharedLoaderBox) -> anyhow::Result<Pack> {
        let context = "loading TacO pack";
        let mut loader = loader.lock_owned().await;
        Controller::try_run_blocking(context, move || Pack::load(&mut *loader).context(context)).await
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
                let festivals = category
                    .attributes_mut()
                    .filters_mut()
                    .festivals
                    .insert(festival.into())
                    .clone();
                fixed_festival_categories.insert(&category.full_id, festivals);
            } else {
                log::info!("unrecognized festival category: `{}`", category.full_id);
            }
        }
        if !fixed_festival_categories.is_empty() {
            // TODO: this should be less necessary once a tree of attribute inherits exist...
            let pois = pack.pois.iter_mut().filter_map(|poi| {
                match fixed_festival_categories.get(poi.category.as_id()) {
                    Some(f) => Some((poi, f)),
                    None => None,
                }
            });
            for (poi, f) in pois {
                let filters = poi.attributes.filters_mut();
                if filters.festivals.is_none() {
                    filters.festivals = Some(f.clone());
                }
            }
            let trails = pack.trails.iter_mut().filter_map(|trail| {
                match fixed_festival_categories.get(trail.category.as_id()) {
                    Some(f) => Some((trail, f)),
                    None => None,
                }
            });
            for (trail, f) in trails {
                let filters = trail.attributes.filters_mut();
                if filters.festivals.is_none() {
                    filters.festivals = Some(f.clone());
                }
            }
        }
        pack.categories.trim_attributes();
    }

    #[cfg(todo = "unused")]
    pub fn get_trail_params(&self) -> impl Future<Output = TrailParams> + Send + 'static {
        let settings = self.settings.clone();
        settings
            .read_owned()
            .map(|settings| Self::trail_params_for(&settings.pathing()))
    }
    pub async fn trail_params(&self) -> TrailParams {
        let settings = self.settings.read().await;
        Self::trail_params_for(&settings.pathing())
    }
    pub fn trail_params_for(pathing: &PathingSettings) -> TrailParams {
        let space = &pathing.space;
        TrailParams {
            resolution: Some(space.trail_resolution()),
            y_offset: space.trail_y_offset().unwrap_or(0.0),
            width: space.trail_width(),
            smoothing: TrailParams::DEFAULT.smoothing,
        }
    }
}

pub type LoaderBox = Box<dyn PackLoaderContext + Send + 'static>;
pub type SharedLoaderBox = Arc<Mutex<LoaderBox>>;
