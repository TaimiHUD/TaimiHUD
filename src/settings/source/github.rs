use {
    anyhow::Context,
    super::super::SourceKind, crate::{settings::Source, ADDON_DIR}, chrono::{DateTime, Utc}, serde::{Deserialize, Serialize}, serde_json::Value, std::{fmt,
    ops::Range,
    pin::Pin,
    future::Future,
}, tokio::fs::create_dir_all, url::Url
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

impl GitHubLatestRelease {
    pub fn empty_with_url(url: Url) -> Self {
        Self {
            html_url: url.clone(),
            assets_url: url.clone(),
            upload_url: url.clone(),
            url,
            tarball_url: None,
            zipball_url: None,
            id: 0,
            node_id: String::new(),
            tag_name: String::new(),
            target_commitish: String::new(),
            name: None,
            body: None,
            draft: false,
            prerelease: false,
            created_at: DateTime::default(),
            published_at: DateTime::default(),
            author: Value::Null,
            assets: Vec::new(),
        }
    }
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
    pub async fn latest_release(&self) -> anyhow::Result<GitHubLatestRelease> {
        let url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            self.name()
        );
        let response = super::get(url).await?;
        let json_data = response.text().await?;
        serde_json::from_str(&json_data)
            .context("Deserializing GitHub release")
    }

    pub const RELEASES_RANGE_DEFAULT: Range<usize> = 0..30;
    pub async fn latest_releases(&self, range: Range<usize>) -> anyhow::Result<Vec<GitHubLatestRelease>> {
        let page = match range {
            range if range == Self::RELEASES_RANGE_DEFAULT =>
                None,
            range if range.start == 0 => Some((1, range.end)),
            range => Some({
                let len = range.len();
                let page = range.start / len;
                (page, len)
            }),
        };
        let url = format!(
            "https://api.github.com/repos/{}/releases",
            self.name()
        );
        let url = match page {
            None => url,
            Some((page, per)) => format!("{url}?page={page}&per_page={per}")
        };
        let response = super::get(url).await?;
        let json_data = response.text().await?;
        serde_json::from_str(&json_data)
            .context("Deserializing GitHub releases")
    }
}

impl Source for GitHubSource {
    fn name(&self) -> String {
        format!("{}/{}", self.owner, self.repository)
    }

    fn description(&self) -> Option<String> {
        self.description.clone()
    }

    fn install_dir(&self) -> String {
        format!("{}_{}", self.owner, self.repository)
    }

    fn view_url(&self) -> String {
        format!("https://github.com/{}", self.name())
    }

    fn download_latest(&self, kind: SourceKind) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + '_>> {
        Box::pin(async move {
            let install_dir = ADDON_DIR.join(kind.get_unpack_dir()).join(self.install_dir());
            create_dir_all(&install_dir).await?;
            let latest = self.latest_release().await?;
            if let Some(tarball_url) = latest.tarball_url {
                super::get_and_extract_tar(&install_dir, tarball_url).await?;
            }
            Ok(latest.tag_name)
        })
    }

    fn latest_id(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + '_>> {
        Box::pin(async move {
            let release = self.latest_release().await?;
            Ok(release.tag_name)
        })
    }
}
