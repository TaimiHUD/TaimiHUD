use {
    crate::{
        exports::runtime as rt,
        settings::{source, DirectSource, GitHubSource, Source},
        with_i18n,
        ADDON_DIR,
    },
    anyhow::Context,
    serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        collections::BTreeMap,
        fmt,
        fs::read_to_string as sync_read_to_string,
        path::{Path, PathBuf},
        sync::LazyLock,
    },
    strum_macros::Display,
    taimi_hoard::{loc::Locator, paths::new_path_const},
    tokio::{
        fs::{create_dir_all, read_to_string, File},
        io::AsyncWriteExt,
    },
};

#[derive(
    Clone,
    Copy,
    Deserialize,
    Serialize,
    Hash,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    strum::IntoStaticStr,
)]
#[strum(serialize_all = "kebab-case")]
pub enum SourceKind {
    #[default]
    Timers,
    Pathing,
    Markers,
    Addon,
    /// Datasource repository
    ///
    /// such as <https://github.com/TaimiHUD/DataSources>
    DataSources,
    Unspecified,
}

impl SourceKind {
    #[inline]
    pub fn label_ident(&self) -> &'static str {
        self.into()
    }
}
impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        with_i18n!(self.label_ident(), |label| f.write_str(&label))
    }
}

impl SourceKind {
    pub fn get_unpack_dir(&self) -> PathBuf {
        let addon_dir = &*ADDON_DIR;
        let type_dir = match self {
            &SourceKind::Timers => "timers",
            &SourceKind::Markers => "markers",
            &SourceKind::Pathing => "pathing",
            &SourceKind::DataSources => "sources",
            &SourceKind::Addon | &SourceKind::Unspecified => unreachable!("No, bad girl."),
        };
        addon_dir.join(type_dir)
    }
    /// Same as [self.get_unpack_dir()] for now
    pub fn get_user_dir(&self) -> PathBuf {
        self.get_unpack_dir()
    }

    pub fn is(&self, kind: Self) -> Option<bool> {
        match *self {
            k if k == kind => Some(true),
            Self::Unspecified => None,
            _ => Some(false),
        }
    }
}

#[derive(Deserialize, Serialize, Hash, Eq, PartialEq, Debug, Clone)]
#[serde(tag = "type")]
pub enum DeserializedSource {
    GitHub(GitHubSource),
    Direct(DirectSource),
}

impl DeserializedSource {
    pub fn as_source(&self) -> &dyn Source {
        match self {
            Self::GitHub(s) => s,
            Self::Direct(s) => s,
        }
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
#[serde(transparent)]
pub struct SourcesFile(pub BTreeMap<SourceKind, Vec<DeserializedSource>>);
pub static SOURCES_SRC: LazyLock<GitHubSource> =
    LazyLock::new(|| GitHubSource::new_empty("TaimiHUD".into(), "DataSources".into()));

impl SourcesFile {
    pub const EMPTY: Self = Self(BTreeMap::new());
    pub const STOCK_SOURCES_TOML: &'static str = include_str!("../../../data/sources.toml");
    pub const FILENAME: &'static str = "sources.toml";

    pub fn file_path() -> PathBuf {
        rt::addon_dir().join(Path::new(Self::FILENAME))
    }

    pub async fn download_sources() -> anyhow::Result<Self> {
        let req = SOURCES_SRC.request_release_asset_browser(None, Self::FILENAME)?;
        let context = "downloading sources.toml";
        let response = source::build_client()?
            .execute(req)
            .await
            .and_then(|res| res.error_for_status())
            .context(context)?;
        let text = response.text().await.context(context)?;
        toml::from_str(&text).context(context)
    }
    pub fn stock() -> Self {
        let sources = toml::from_str(Self::STOCK_SOURCES_TOML)
            .context("Stock sources should not fail, please report this bug!");
        match sources {
            Ok(s) => s,
            Err(e) => {
                log::error!("{e:#}");
                Self::default()
            },
        }
    }

    pub async fn get_sources() -> anyhow::Result<Self> {
        let addon_dir = &*ADDON_DIR;
        let sources_path = addon_dir.join("sources.toml");
        let sources = match Self::download_sources().await {
            Ok(sources) => sources,
            Err(e) if sources_path.exists() => return Err(e),
            Err(e) => {
                log::error!("{e:#}, falling back to stock sources");
                Self::stock()
            },
        };
        let sources_str = toml::to_string_pretty(&sources)?;
        let mut file = File::create(sources_path).await?;
        file.write_all(sources_str.as_bytes()).await?;
        Ok(sources)
    }
    pub async fn create_stock(dest: &Path) -> anyhow::Result<()> {
        if let Some(parent) = dest.parent() {
            create_dir_all(parent).await?;
        }
        let mut file = File::create_new(dest).await?;
        file.write_all(Self::STOCK_SOURCES_TOML.as_bytes()).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn reload(&mut self) -> anyhow::Result<()> {
        *self = Self::load().await?;
        Ok(())
    }

    pub async fn load() -> anyhow::Result<Self> {
        let sources_path = ADDON_DIR.join("sources.toml");
        if !sources_path.exists() {
            Self::create_stock(&sources_path)
                .await
                .context("Creating stock sources.toml")?;
        }
        read_to_string(&sources_path)
            .await
            .context("reading sources.toml")
            .and_then(|data| toml::from_str(&data).context("loading sources.toml"))
    }

    pub fn downloadless_load() -> anyhow::Result<Self> {
        let sources_path = ADDON_DIR.join("sources.toml");
        sync_read_to_string(&sources_path)
            .context("reading sources.toml")
            .and_then(|data| toml::from_str(&data).context("loading sources.toml"))
    }

    pub fn lookup(&self, kind: SourceKind, name: &str) -> Option<&DeserializedSource> {
        self.0
            .get(&kind)
            .and_then(|sources| sources.iter().find(|s| name == &s.as_source().name()))
    }

    pub fn into_iter(self) -> impl Iterator<Item = (SourceKind, DeserializedSource)> {
        self.0
            .into_iter()
            .flat_map(|(kind, sources)| sources.into_iter().map(move |source| (kind, source)))
    }
    pub fn iter(&self) -> impl Iterator<Item = (SourceKind, &'_ DeserializedSource)> {
        self.0
            .iter()
            .flat_map(|(&kind, sources)| sources.iter().map(move |source| (kind, source)))
    }
    #[cfg(todo = "unused")]
    pub fn get_by_kind(&self, kind: SourceKind) -> Option<&Vec<DeserializedSource>> {
        self.0.get(&kind)
    }
}

#[derive(Clone, Copy, Deserialize, Display, Serialize, Hash, Debug, PartialEq, Eq)]
pub enum RemoteAssetForm {
    /// Retain download locally
    File {
        #[cfg(todo = "unnecessary")]
        extension: &'static OsStr,
    },
    /// Extract download locally
    Tarball {
        #[cfg(todo = "unnecessary")]
        compression: (),
    },
    /// Extract download locally
    #[cfg(todo)]
    ZipArchive,
}

impl RemoteAssetForm {
    pub const FILE: Self = Self::File {};
    pub const TAR_GZ: Self = Self::Tarball {};
    pub const TAR: Self = Self::Tarball {};
    #[cfg(todo)]
    pub const ZIP: Self = Self::ZipArchive {};

    pub const CONTENT_TYPE_BINARY: &'static str = "application/octet-stream";

    pub fn with_asset<P: AsRef<Path>>(kind: SourceKind, filename: P, _content_type: &str) -> Option<Self> {
        let filename = filename.as_ref();
        let ext = filename.extension();
        let is_ext =
            |matches: &'static str| ext.map(|ext| ext.eq_ignore_ascii_case(matches)).unwrap_or(false);
        let is_ext2 = |matches: &'static str| {
            filename
                .file_stem()
                .and_then(|n| Path::new(n).extension())
                .map(|ext| ext.eq_ignore_ascii_case(matches))
                .unwrap_or(false)
        };
        let is_tgz = || is_ext("tgz") || (is_ext("gz") && is_ext2("tar"));
        match kind {
            SourceKind::Addon if is_ext("dll") => Some(Self::FILE),
            SourceKind::DataSources if is_ext("toml") || is_ext("json") => Some(Self::FILE),
            SourceKind::Pathing if is_ext("zip") || is_ext("taco") => Some(Self::FILE),
            #[cfg(todo)]
            SourceKind::Timers if is_ext("zip") => Some(Self::ZIP),
            SourceKind::Timers if is_tgz() => Some(Self::TAR_GZ),
            SourceKind::Timers if is_ext("tar") => Some(Self::TAR),
            // TODO: would anyone do this, also what if there are more than one?
            SourceKind::Timers if is_ext("bhtimer") => Some(Self::FILE),
            SourceKind::Markers if is_ext("json") || is_ext("markers") => Some(Self::FILE),
            // TODO: filter out manifest files or hashes etc?
            SourceKind::Unspecified => Some(Self::FILE),
            _ => None,
        }
    }

    pub fn for_source_archive(self, kind: SourceKind) -> Option<Self> {
        match kind {
            SourceKind::Pathing | SourceKind::Timers | SourceKind::Markers => Some(self),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct SourcesNs<'a> {
    pub name: Cow<'a, Path>,
}

impl SourcesNs<'static> {
    pub const SOURCES_TOML: Self =
        Self::with_name(Cow::Borrowed(new_path_const("addons/Taimi/sources.toml")));
}

impl<'a> SourcesNs<'a> {
    #[inline]
    pub const fn with_name(name: Cow<'a, Path>) -> Self {
        Self { name }
    }

    pub fn new<N: Into<Cow<'a, Path>>>(name: N) -> Self {
        Self::with_name(name.into())
    }
}

#[derive(Debug, Copy, Clone, Default, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct SourcesToml;

impl SourcesToml {
    pub const DEFAULT: Self = Self;

    #[inline]
    pub const fn locator_for(path: String) -> DataSourcePath<Self> {
        Locator::with_parts(Self::DEFAULT, path)
    }
}

impl AsRef<SourcesNs<'static>> for SourcesToml {
    fn as_ref(&self) -> &SourcesNs<'static> {
        static SOURCES_TOML: LazyLock<SourcesNs<'static>> =
            LazyLock::new(|| SourcesNs::with_name(Cow::Owned(SourcesFile::file_path())));
        &SOURCES_TOML
    }
}

pub type DataSourcePath<S = SourcesToml> = Locator<S, String>;
