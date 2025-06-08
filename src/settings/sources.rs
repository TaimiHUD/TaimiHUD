use {
    crate::{
        settings::{GitHubSource, RemoteSource},
        ADDON_DIR,
    },
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    tokio::{
        fs::{create_dir_all, read_to_string, File},
        io::AsyncWriteExt,
    },
};

#[derive(Deserialize, Serialize, Hash, Debug, Default, PartialEq, Eq)]
pub enum SourceKind {
    #[default]
    Timers,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct SourcesFile(pub HashMap<SourceKind, Vec<RemoteSource>>);

impl SourcesFile {
    pub fn generate_stock() -> Self {
        let mut inner: HashMap<SourceKind, Vec<RemoteSource>> = HashMap::new();
        inner.insert(
            SourceKind::Timers,
            vec![
                    RemoteSource::GitHub(GitHubSource {
                        owner: "kittywitch".to_string(),
                        repository: "Hero-Timers".to_string(),
                        description: Some("The author of this mod's fork of the below; changes such as Sabetha markers and others planned, specific to this addon.".to_string()),
                    }),
                    RemoteSource::GitHub(GitHubSource {
                        owner: "QuitarHero".to_string(),
                        repository: "Hero-Timers".to_string(),
                        description: Some("The OG timer pack for BlishHUD!".to_string()),
                    }),
                ]
        );
        Self(inner)
    }
    pub async fn create_stock() -> anyhow::Result<()> {
        let addon_dir = &*ADDON_DIR;
        create_dir_all(addon_dir).await?;
        let sources_path = addon_dir.join("sources.toml");
        let stock_sources = Self::generate_stock();
        let sources = toml::to_string_pretty(&stock_sources)?;
        let mut file = File::create(sources_path).await?;
        file.write_all(sources.as_bytes()).await?;
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
        log::info!("Attempting to load the sources file at \"{sources_path:?}\".");
        let file_data = read_to_string(&sources_path).await?;
        let data: Self = toml::from_str(&file_data)?;
        log::info!("Loaded the sources file at \"{sources_path:?}\".");
        Ok(data)
    }

    #[allow(dead_code)]
    pub fn get_by_kind(&self, kind: SourceKind) -> Option<&Vec<RemoteSource>> {
        self.0.get(&kind)
    }
}
