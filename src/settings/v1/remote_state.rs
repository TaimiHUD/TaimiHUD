use {
    crate::{
        settings::{
            source::Source,
            DeserializedSource,
            NeedsUpdate,
            RemoteSource,
            SourceKind,
            SourcesFile,
        },
        timer::TimerFile,
    },
    serde::{Deserialize, Serialize},
    std::{path::PathBuf, sync::Arc},
    tokio::fs::remove_dir_all,
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RemoteState {
    pub source: DeserializedSource,
    #[serde(default)]
    pub kind: SourceKind,
    pub installed_tag: Option<String>,
    pub installed_path: Option<PathBuf>,
    #[serde(skip)]
    pub needs_update: NeedsUpdate,
}

impl RemoteState {
    pub fn new_from_source(kind: SourceKind, source: DeserializedSource) -> Self {
        Self {
            source,
            kind,
            installed_tag: Default::default(),
            installed_path: Default::default(),
            needs_update: Default::default(),
        }
    }

    pub fn source(&self) -> &dyn Source {
        match &self.source {
            DeserializedSource::GitHub(s) => s,
            DeserializedSource::Direct(s) => s,
        }
    }

    pub fn remote_source(&self) -> RemoteSource {
        match &self.source {
            DeserializedSource::GitHub(s) => Arc::new(s.clone()),
            DeserializedSource::Direct(s) => Arc::new(s.clone()),
        }
    }

    pub fn name(&self) -> String {
        self.source().name()
    }

    pub async fn load(&self) -> Vec<Arc<TimerFile>> {
        let association = self.remote_source();
        if let Some(path) = &self.installed_path {
            TimerFile::load_many(path, association, 100)
                .await
                .expect("Could not load timer file for source {self.source}")
        } else {
            Default::default()
        }
    }

    pub fn update(&mut self, source: DeserializedSource) {
        self.source = source;
    }

    pub async fn uninstall(&mut self) -> anyhow::Result<()> {
        // fuck man, be careful o:
        if let Some(path) = &self.installed_path {
            if path.exists() {
                log::warn!("Uninstalling: removing {path:?}!");
                remove_dir_all(path).await?;
            } else {
                log::warn!("Uninstalling: {path:?} no longer exists.");
            }
        }
        self.installed_tag = None;
        self.installed_path = None;
        self.needs_update = NeedsUpdate::Unknown;
        Ok(())
    }

    pub fn hardcoded_sources() -> Vec<(&'static str, &'static str, &'static str)> {
        let hardcoded_sources = [(
            "QuitarHero",
            "Hero-Timers",
            "The OG timer pack for BlishHUD!",
        )];
        hardcoded_sources.into()
    }
    pub fn suggested_sources() -> Result<Vec<Self>, anyhow::Error> {
        let sources = SourcesFile::downloadless_load()?;
        Ok(sources
            .0
            .into_iter()
            .flat_map(|(kind, ssources)| {
                ssources
                    .into_iter()
                    .map(move |ssource| Self::new_from_source(kind, ssource))
            })
            .collect())
    }

    pub async fn needs_update(&self) -> NeedsUpdate {
        use NeedsUpdate::*;
        let source = self.source();
        let remote_id = source.latest_id().await;
        log::debug!("{:?}", remote_id);
        match remote_id {
            Ok(rid) => {
                if let Some(lid) = &self.installed_tag {
                    Known(*lid != rid, rid)
                } else {
                    Known(true, rid)
                }
            },
            Err(err) => {
                log::error!("Update check failed: {}", err);
                NeedsUpdate::Error(err.to_string())
            },
        }
    }
    pub async fn commit_downloaded(
        &mut self,
        tag_name: String,
        install_dir: PathBuf,
    ) -> anyhow::Result<()> {
        self.installed_tag = Some(tag_name);
        self.needs_update = self.needs_update().await;
        self.installed_path = Some(install_dir);
        Ok(())
    }
}
