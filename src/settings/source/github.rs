use {
    crate::{
        settings::{
            source::{self, MetadataKey},
            RemoteAssetForm,
            Source,
            SourceKind,
        },
        ADDON_DIR,
    },
    anyhow::{anyhow, Context},
    chrono::{DateTime, Utc},
    futures::{FutureExt, TryFutureExt},
    reqwest::{header, Method, Request},
    serde::{Deserialize, Serialize},
    serde_json::Value,
    std::{
        borrow::Cow,
        future::Future,
        ops::Range,
        path::{Path, PathBuf},
        pin::Pin,
    },
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

impl GitHubReleaseAsset {
    pub const STATE_UPLOADED: &'static str = "uploaded";
    pub const REDIR_LIMIT: usize = 3;

    pub async fn download_url(&self) -> anyhow::Result<Url> {
        #[cfg(todo = "unnecessary")]
        let req = self.request_browser().unwrap_or_else(|| self.request_content());
        let req = self.request_content();
        match source::get_location_for(req, Self::REDIR_LIMIT).await {
            Err(e) => match self.browser_download_url.clone() {
                Some(url) => {
                    log::error!("{e:#}");
                    Ok(url)
                },
                None => Err(e),
            },
            Ok(url) => {
                if url == self.url {
                    log::warn!("asset url {url} is unlikely to work correctly...");
                }
                Ok(url)
            },
        }
    }

    pub fn request_content(&self) -> Request {
        #[cfg(todo = "unnecessary")]
        if let Some(req) = self.request_browser() {
            return req
        }
        let mut req = Request::new(Method::GET, self.url.clone());
        source::insert_header(req.headers_mut(), header::ACCEPT, &self.content_type);
        req
    }

    pub fn request_browser(&self) -> Option<Request> {
        self.browser_download_url
            .clone()
            .map(|url| Request::new(Method::GET, url))
    }
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

    pub fn assets(&self) -> impl Iterator<Item = &'_ GitHubReleaseAsset> + Clone + '_ {
        self.assets
            .iter()
            .filter(|asset| asset.state == GitHubReleaseAsset::STATE_UPLOADED)
    }

    pub fn assets_for(
        &self,
        kind: SourceKind,
    ) -> impl Iterator<Item = (&'_ GitHubReleaseAsset, RemoteAssetForm)> + Clone + '_ {
        self.assets().filter_map(move |asset| {
            RemoteAssetForm::with_asset(kind, &asset.name, &asset.content_type).map(|form| (asset, form))
        })
    }

    pub fn request_tarball(&self) -> Option<Request> {
        self.tarball_url.clone().map(|url| Request::new(Method::GET, url))
    }

    pub fn request_zipball(&self) -> Option<Request> {
        self.zipball_url.clone().map(|url| Request::new(Method::GET, url))
    }
}

#[derive(Deserialize, Serialize, Debug, Hash, Eq, Clone, PartialEq)]
pub struct GitHubSource {
    pub owner: String,
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default)]
    #[cfg_attr(todo, serde(skip_serializing_if = "Option::is_none"))]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl GitHubSource {
    pub const CONTENT_TYPE_GH_JSON: &'static str = "application/vnd.github+json";
    pub const ACCEPT_GH_JSON: header::HeaderValue =
        header::HeaderValue::from_static(Self::CONTENT_TYPE_GH_JSON);

    pub fn new_empty(owner: String, repository: String) -> Self {
        Self {
            owner,
            repository,
            author: None,
            description: None,
            display_name: None,
            homepage_url: None,
            name: None,
        }
    }

    /// Get latest or specific tagged release browser download
    pub fn request_release_asset_browser(
        &self,
        tag: Option<&str>,
        filename: &str,
    ) -> Result<Request, url::ParseError> {
        let Self { owner, repository, .. } = self;
        let tag = tag.map(source::url_escape);
        let filename = source::url_escape(filename);
        let url = match tag {
            Some(tag) =>
                format!("https://github.com/{owner}/{repository}/releases/download/{tag}/{filename}"),
            None => format!("https://github.com/{owner}/{repository}/releases/latest/download/{filename}"),
        }
        .parse();

        Ok(Request::new(Method::GET, url?))
    }

    /// Get latest or specific tagged release metadata
    pub fn request_release(&self, tag: Option<&str>) -> Result<Request, url::ParseError> {
        let Self { owner, repository, .. } = self;
        let tag = tag.map(source::url_escape);
        let url = match tag {
            Some(tag) => format!("https://api.github.com/repos/{owner}/{repository}/releases/tags/{tag}"),
            None => format!("https://api.github.com/repos/{owner}/{repository}/releases/latest"),
        }
        .parse();
        let mut req = Request::new(Method::GET, url?);
        req.headers_mut().insert(header::ACCEPT, Self::ACCEPT_GH_JSON);
        Ok(req)
    }

    pub const RELEASES_RANGE_DEFAULT: Range<usize> = 0..30;
    pub fn request_releases(&self, range: Range<usize>) -> Result<Request, url::ParseError> {
        let Self { owner, repository, .. } = self;
        let page = match range {
            range if range == Self::RELEASES_RANGE_DEFAULT => None,
            range if range.start == 0 => Some((1, range.end)),
            range => Some({
                let len = range.len();
                let page = range.start / len;
                (page, len)
            }),
        };
        let url = format!("https://api.github.com/repos/{owner}/{repository}/releases");
        let url = match page {
            None => url.parse(),
            Some((page, per)) =>
                Url::parse_with_params(&url, [("page", page.to_string()), ("per_page", per.to_string())]),
        };
        let mut req = Request::new(Method::GET, url?);
        req.headers_mut().insert(header::ACCEPT, Self::ACCEPT_GH_JSON);
        Ok(req)
    }

    pub async fn get_release(&self, tag: Option<&str>) -> anyhow::Result<GitHubLatestRelease> {
        let req = self.request_release(tag)?;
        source::build_client()?
            .execute(req)
            .map(|res| res.and_then(|res| res.error_for_status()))
            .and_then(|res| res.json())
            .await
            .with_context(|| format!("Deserializing GitHub release for {}", self.repository))
    }

    pub async fn latest_releases(&self, range: Range<usize>) -> anyhow::Result<Vec<GitHubLatestRelease>> {
        let req = self.request_releases(range)?;
        source::build_client()?
            .execute(req)
            .map(|res| res.and_then(|res| res.error_for_status()))
            .and_then(|res| res.json())
            .await
            .with_context(|| format!("Deserializing GitHub release for {}", self.repository))
    }
}

impl Source for GitHubSource {
    fn name(&self) -> Cow<'_, str> {
        format!("{}/{}", self.owner, self.repository).into()
    }

    fn get_metadata_str(&self, key: MetadataKey) -> Option<Cow<'_, str>> {
        match key {
            MetadataKey::DisplayName => self
                .display_name
                .as_ref()
                .or(self.name.as_ref())
                .map(|v| Cow::Borrowed(&v[..])),
            MetadataKey::Author => Some(
                self.author
                    .as_ref()
                    .map(|v| Cow::Borrowed(&v[..]))
                    .unwrap_or(Cow::Borrowed(&self.owner)),
            ),
            MetadataKey::Description => self.description.as_ref().map(|v| Cow::Borrowed(&v[..])),
            MetadataKey::HomepageUrl => Some(
                self.homepage_url
                    .as_ref()
                    .map(|v| Cow::Borrowed(&v[..]))
                    .unwrap_or_else(|| {
                        format!("https://github.com/{}/{}", self.owner, self.repository).into()
                    }),
            ),
            _ => None,
        }
    }
    fn has_metadata(&self, key: MetadataKey) -> bool {
        match key {
            MetadataKey::Author | MetadataKey::HomepageUrl => true,
            MetadataKey::DisplayName => self.display_name.is_some() || self.name.is_some(),
            MetadataKey::Description => self.description.is_some(),
            _ => false,
        }
    }

    fn download_latest(
        &self,
        kind: SourceKind,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<(String, PathBuf)>> + Send + '_>> {
        Box::pin(async move {
            let install_dir = ADDON_DIR
                .join(kind.get_unpack_dir())
                .join(&self.install_dir()[..]);
            let latest = self.get_release(None).await?;
            let tarball = latest.request_tarball();
            let tag_name = &latest.tag_name;
            let mut action = latest
                .assets_for(kind)
                .map(|(asset, form)| (asset.request_content(), form, Some(&asset.name)))
                .chain(tarball.and_then(|tarball| {
                    RemoteAssetForm::TAR_GZ
                        .for_source_archive(kind)
                        .map(|form| (tarball, form, None))
                }));

            let client = source::build_client()?;

            let mut err = None;
            while let Some((req, form, fname)) = action.next() {
                let fname = fname.as_ref().map(Path::new);
                let mut install_dest = install_dir.clone();
                // TODO: consider appending tag name to dest?
                let res = match client.execute(req).await {
                    Ok(res) => match form {
                        RemoteAssetForm::Tarball { .. } =>
                            source::install_remote_tarball(&install_dest, res).await,
                        RemoteAssetForm::File { .. } => {
                            if let Some(ext) = fname.and_then(|n| n.extension()) {
                                let append = install_dest.as_mut_os_string();
                                append.push(".");
                                append.push(ext);
                            }
                            source::install_remote_file(&install_dest, res).await
                        },
                        #[cfg(todo)]
                        RemoteAssetForm::ZipArchive { .. } => compile_error!("TODO"),
                    }
                    .with_context(|| format!("installing into {}", install_dest.display())),
                    Err(e) => Err(e.into()),
                };
                let res = res.with_context(|| {
                    match (
                        fname,
                        format_args!("downloading {}/{}/{tag_name}", self.owner, self.repository),
                    ) {
                        (Some(fname), msg) => format!("{msg}/{}", fname.display()),
                        (None, msg) => msg.to_string(),
                    }
                });
                match res {
                    Ok(()) => return Ok((tag_name.clone(), install_dest)),
                    Err(e) if err.is_some() => log::error!("{e:#}"),
                    Err(e) => err = Some(e),
                }
            }
            Err(err.unwrap_or_else(|| anyhow!("no release download found for {tag_name}")))
        })
    }

    fn latest_id(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
        Box::pin(async move {
            let release = self.get_release(None).await?;
            Ok(release.tag_name)
        })
    }
}
