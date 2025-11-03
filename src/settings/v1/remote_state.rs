use {
    crate::{
        exports::runtime as rt,
        settings::{source::Source, DeserializedSource, NeedsUpdate, SourceKind},
        timer::TimerFile,
    },
    anyhow::Context,
    serde::{Deserialize, Serialize},
    std::{borrow::Cow, path::PathBuf, sync::Arc},
    tokio::fs::{remove_dir_all, remove_file, symlink_metadata},
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RemoteState {
    pub source: DeserializedSource,
    #[serde(default)]
    pub kind: SourceKind,
    pub installed_tag: Option<String>,
    pub installed_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datasource_repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datasource_name: Option<String>,
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
            datasource_repo: None,
            datasource_name: None,
        }
    }

    pub fn source(&self) -> &dyn Source {
        self.source.as_source()
    }

    pub fn datasource_name(&self) -> Cow<'_, str> {
        self.datasource_name
            .as_ref()
            .map(|n| Cow::Borrowed(&n[..]))
            .unwrap_or_else(|| self.source().name())
    }

    pub fn datasource_name_matches(&self, name: &str) -> bool {
        let datasource_name = self
            .datasource_name
            .as_ref()
            .map(|datasource_name| datasource_name == name)
            .unwrap_or(false);

        datasource_name || self.source().name() == name
    }

    pub fn lookup_datasource<'a>(sources: &'a [Self], kind: SourceKind, name: &str) -> Option<&'a Self> {
        sources
            .iter()
            .find(|s| s.kind == kind && s.datasource_name() == name)
    }
    pub fn lookup_datasource_mut<'a>(
        sources: &'a mut [Self],
        kind: SourceKind,
        name: &str,
    ) -> Option<&'a mut Self> {
        sources
            .iter_mut()
            .find(|s| s.kind == kind && s.datasource_name() == name)
    }

    pub async fn load_timers(&self) -> Vec<Arc<TimerFile>> {
        let association = self.datasource_name();
        if let Some(path) = &self.installed_path {
            TimerFile::load_many(path, &association, 100)
                .await
                .expect("Could not load timer file for source {self.source}")
        } else {
            Default::default()
        }
    }

    pub fn update(&mut self, source: DeserializedSource) {
        self.source = source;
    }

    /// fuck man, be careful o:
    pub async fn remove(&mut self) -> anyhow::Result<(PathBuf, bool)> {
        if let Some(path) = self.installed_path.take() {
            match symlink_metadata(&path).await {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(e),
                Ok(m) if m.is_dir() => remove_dir_all(&path).await.map(|()| true),
                Ok(_m) => remove_file(&path).await.map(|()| true),
            }
            .with_context(|| format!("cleaning up {}", path.display()))
            .map(|removed| (path, removed))
        } else {
            Ok((Default::default(), true))
        }
    }

    pub async fn uninstall(&mut self) -> anyhow::Result<()> {
        let removed = self
            .remove()
            .await
            .with_context(|| format!("uninstalling {}", self.source().display_name()));
        if let (path, false) = removed? {
            log::warn!("Uninstalling: {} no longer exists", path.display());
        }
        self.installed_tag = None;
        self.needs_update = NeedsUpdate::Unknown;
        Ok(())
    }

    pub fn hardcoded_sources() -> Vec<(&'static str, &'static str, &'static str)> {
        let hardcoded_sources = [("QuitarHero", "Hero-Timers", "The OG timer pack for BlishHUD!")];
        hardcoded_sources.into()
    }

    pub fn from_sources<S>(sources: S) -> Vec<Self>
    where
        S: IntoIterator<Item = (SourceKind, DeserializedSource)>,
    {
        sources
            .into_iter()
            .map(|(kind, source)| Self::new_from_source(kind, source))
            .collect()
    }

    pub fn get_needs_update(&self, remote_id: anyhow::Result<String>) -> NeedsUpdate {
        use NeedsUpdate::*;
        match remote_id {
            Ok(rid) =>
                if let Some(lid) = &self.installed_tag {
                    Known(*lid != rid, rid)
                } else {
                    Known(true, rid)
                },
            Err(err) => {
                log::error!("Update check failed: {err:#}");
                NeedsUpdate::Error(err.to_string())
            },
        }
    }

    pub fn commit_downloaded(
        &mut self,
        tag_name: Option<String>,
        installed_path: Option<PathBuf>,
        status: Result<(), String>,
    ) {
        let tag_name = match tag_name {
            Some(tag_name) => Some(self.installed_tag.insert(tag_name)),
            None if installed_path.is_some() => {
                log::info!("install status uncertain?");
                self.installed_tag = None;
                None
            },
            None => None,
        };
        self.needs_update = match (status, tag_name) {
            (Err(message), _) => NeedsUpdate::Error(message),
            (Ok(()), Some(tag_name)) => NeedsUpdate::Known(false, tag_name.clone()),
            (Ok(()), None) => NeedsUpdate::Unknown,
        };
        if let Some(installed_path) = installed_path {
            self.installed_path = Some(match rt::relative_path(&installed_path) {
                rel if rel != installed_path => rel.to_owned(),
                _ => installed_path,
            });
        }
    }
}
