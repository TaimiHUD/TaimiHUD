use {
    super::Source, crate::{
        settings::{GitHubSource, RemoteSource, DirectSource},
        ADDON_DIR,
    }, serde::{Deserialize, Serialize}, std::collections::HashMap, tokio::{
        fs::{create_dir_all, read_to_string, File},
        io::AsyncWriteExt,
    },
    reqwest::Response,
    std::{
        fs::read_to_string as sync_read_to_string,
        path::PathBuf,
    },
    strum_macros::Display,
};

#[derive(Clone, Copy, Deserialize, Display, Serialize, Hash, Debug, Default, PartialEq, Eq)]
pub enum SourceKind {
    #[default]
    Timers,
    Pathing,
    Markers,
    Addon,
    Unspecified,
}

impl SourceKind {
    pub fn get_unpack_dir(&self) -> PathBuf {
        let addon_dir = &*ADDON_DIR;
        let type_dir = match self {
            &SourceKind::Timers => "timers",
            &SourceKind::Markers => "markers",
            &SourceKind::Pathing => "pathing",
            &SourceKind::Addon | &SourceKind::Unspecified => unreachable!("No, bad girl."),
        };
        addon_dir.join(type_dir)
    }
}

#[derive(Deserialize, Serialize, Hash, Eq, PartialEq, Debug, Clone)]
#[serde(tag = "type")]
pub enum DeserializedSource {
    GitHub(GitHubSource),
    Direct(DirectSource),
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct SourcesFile(pub HashMap<SourceKind, Vec<DeserializedSource>>);

impl SourcesFile {
    pub async fn download_sources() -> anyhow::Result<Self> {
        let response: Response = super::source::get("https://raw.githubusercontent.com/TaimiHUD/DataSources/refs/heads/main/sources.toml").await?;
        let text = response.text().await?;
        let sources: HashMap<SourceKind, Vec<DeserializedSource>> = toml::from_str(&text)?;
        Ok(Self(sources))
    }
    pub fn generate_stock() -> Self {
        log::info!("Asked to generate stock");
        let mut inner: HashMap<SourceKind, Vec<DeserializedSource>> = HashMap::new();
        /*inner.insert(
            SourceKind::Timers,
            vec![
                    DeserializedSource::GitHub(GitHubSource {
                        owner: "QuitarHero".to_string(),
                        repository: "Hero-Timers".to_string(),
                        description: Some("The OG timer pack for BlishHUD!".to_string()),
                    }),
                ]
        );*/
        Self(inner)
    }
    pub async fn get_sources() -> anyhow::Result<()> {
        let addon_dir = &*ADDON_DIR;
        let sources_path = addon_dir.join("sources.toml");
        let sources = match Self::download_sources().await {
            Ok(sources) => sources,
            Err(e) => {
                log::error!("{:?}", e);
                Self::generate_stock()
            }
        };
        let sources_str = toml::to_string_pretty(&sources)?;
        let mut file = File::create(sources_path).await?;
        file.write_all(sources_str.as_bytes()).await?;
        Ok(())
    }
    pub async fn create_stock() -> anyhow::Result<()> {
        let addon_dir = &*ADDON_DIR;
        create_dir_all(addon_dir).await?;
        let sources_path = addon_dir.join("sources.toml");
        let sources = Self::generate_stock();
        let sources_str = toml::to_string_pretty(&sources)?;
        let mut file = File::create(sources_path).await?;
        file.write_all(sources_str.as_bytes()).await?;
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
            log::info!("Sources file doesn't exist! Creating sources file at {sources_path:?}.");
            Self::create_stock().await?;
        }
        log::trace!("Attempting to load the sources file at \"{sources_path:?}\".");
        let file_data = read_to_string(&sources_path).await?;
        let data: Self = toml::from_str(&file_data)?;
        log::trace!("Loaded the sources file at \"{sources_path:?}\".");
        Ok(data)
    }

    pub fn downloadless_load() -> anyhow::Result<Self> {
        let sources_path = ADDON_DIR.join("sources.toml");
        log::trace!("Attempting to load the sources file at \"{sources_path:?}\".");
        let file_data = sync_read_to_string(&sources_path)?;
        let data: Self = toml::from_str(&file_data)?;
        log::trace!("Loaded the sources file at \"{sources_path:?}\".");
        Ok(data)
    }

    #[allow(dead_code)]
    pub fn get_by_kind(&self, kind: SourceKind) -> Option<&Vec<DeserializedSource>> {
        self.0.get(&kind)
    }
}
