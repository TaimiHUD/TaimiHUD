use {
    super::super::SourceKind,
    crate::{settings::Source, ADDON_DIR},
    anyhow::anyhow,
    reqwest::header::LAST_MODIFIED,
    serde::{Deserialize, Serialize},
    std::{fmt, future::Future, pin::Pin},
    tokio::fs::create_dir_all,
};

#[derive(Deserialize, Serialize, Debug, Hash, Eq, Clone, PartialEq)]
pub struct DirectSource {
    pub name: String,
    pub url: String,
    pub description: Option<String>,
}

impl fmt::Display for DirectSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl DirectSource {}

impl Source for DirectSource {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn description(&self) -> Option<String> {
        self.description.clone()
    }

    fn install_dir(&self) -> String {
        self.name.clone()
    }

    fn view_url(&self) -> String {
        self.url.clone()
    }

    fn download_latest(
        &self,
        kind: SourceKind,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + '_>> {
        Box::pin(async move {
            let install_dir = ADDON_DIR
                .join(kind.get_unpack_dir())
                .join(self.install_dir());
            create_dir_all(&install_dir).await?;
            let last_modified = super::download_file(&install_dir, &self.url).await?;
            Ok(last_modified)
        })
    }

    fn latest_id(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + '_>> {
        Box::pin(async move {
            let response = super::head(&self.url).await?;
            let meep = response
                .headers()
                .get(LAST_MODIFIED)
                .ok_or_else(|| anyhow!("I can't believe you've done this"))?
                .to_str()?;
            Ok(meep.into())
        })
    }
}
