use {
    crate::{settings::Source, ADDON_DIR},
    chrono::{DateTime, Utc},
    serde::{Deserialize, Serialize},
    serde_json::Value,
    std::fmt,
    tokio::fs::create_dir_all,
    url::Url,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct GitHubReleaseAsset {
    pub url: Url,
    pub id: usize,
    pub node_id: String,
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub uploader: Value,
    pub content_type: String,
    pub state: String,
    pub size: usize,
    #[serde(default)]
    pub digest: Option<String>,
    pub download_count: usize,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub browser_download_url: Option<Url>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GitHubLatestRelease {
    pub url: Url,
    pub html_url: Url,
    pub assets_url: Url,
    pub upload_url: Url,
    pub tarball_url: Option<Url>,
    pub zipball_url: Option<Url>,
    pub id: usize,
    pub node_id: String,
    pub tag_name: String,
    pub target_commitish: String,
    pub name: Option<String>,
    pub body: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    pub created_at: DateTime<Utc>,
    pub published_at: DateTime<Utc>,
    // i don't really care about these ><
    pub author: Value,
    #[serde(default)]
    pub assets: Vec<GitHubReleaseAsset>,
}

#[derive(Deserialize, Serialize, Debug, Hash, Eq, Clone, PartialEq)]
pub struct GitHubSource {
    pub owner: String,
    pub repository: String,
    pub description: Option<String>,
}

impl fmt::Display for GitHubSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.repository)
    }
}

impl GitHubSource {
    pub fn repo_string(&self) -> String {
        format!("{}", self)
    }

    pub async fn latest_release(&self) -> anyhow::Result<GitHubLatestRelease> {
        let url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            self.repo_string()
        );
        let response = Self::get(url).await?;
        let json_data = response.text().await?;
        let data = serde_json::from_str::<GitHubLatestRelease>(&json_data)?;
        Ok(data)
    }
}

impl Source for GitHubSource {
    fn install_dir(&self) -> String {
        format!("{}_{}", self.owner, self.repository)
    }
    fn view_url(&self) -> String {
        format!("https://github.com/{}", self.repo_string())
    }
    async fn download_latest(&self) -> anyhow::Result<String> {
        let install_dir = ADDON_DIR.join(self.install_dir());
        create_dir_all(&install_dir).await?;
        let latest = self.latest_release().await?;
        if let Some(tarball_url) = latest.tarball_url {
            Self::get_and_extract_tar(&install_dir, tarball_url).await?;
        }
        Ok(latest.tag_name)
    }

    async fn latest_id(&self) -> anyhow::Result<String> {
        let release = self.latest_release().await?;
        Ok(release.tag_name)
    }
}
