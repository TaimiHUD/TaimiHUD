use {
    super::super::SourceKind,
    crate::{
        settings::{source::MetadataKey, RemoteAssetForm, Source},
        ADDON_DIR,
    },
    anyhow::{anyhow, Context},
    reqwest::header,
    serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        future::Future,
        path::{Path, PathBuf},
        pin::Pin,
    },
    taimi_hoard::is_false_ref,
    url::Url,
};

#[derive(Deserialize, Serialize, Debug, Hash, Eq, Clone, PartialEq)]
pub struct DirectSource {
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default)]
    #[cfg_attr(todo, serde(skip_serializing_if = "Option::is_none"))]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
    #[serde(rename = "deprecated", default, skip_serializing_if = "is_false_ref")]
    pub is_deprecated: bool,
}

impl DirectSource {
    pub fn id_from_headers(headers: &reqwest::header::HeaderMap) -> anyhow::Result<String> {
        // TODO: prio etag (if not weak/temporary)
        let h = headers
            .get(header::LAST_MODIFIED)
            .ok_or_else(|| anyhow!("identifying header missing"))?
            .to_str()?;

        Ok(h.into())
    }
}

impl Source for DirectSource {
    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.name)
    }

    fn get_metadata_str(&self, key: MetadataKey) -> Option<Cow<'_, str>> {
        match key {
            MetadataKey::Author => self.author.as_ref().map(|v| Cow::Borrowed(&v[..])),
            MetadataKey::Description => self.description.as_ref().map(|v| Cow::Borrowed(&v[..])),
            MetadataKey::DisplayName => self.display_name.as_ref().map(|v| Cow::Borrowed(&v[..])),
            MetadataKey::HomepageUrl => self.homepage_url.as_ref().map(|v| Cow::Borrowed(&v[..])),
            MetadataKey::IsDeprecated => Some(Cow::Borrowed(MetadataKey::bool_value(self.is_deprecated))),
            _ => None,
        }
    }
    fn has_metadata(&self, key: MetadataKey) -> bool {
        match key {
            MetadataKey::Author => self.author.is_some(),
            MetadataKey::Description => self.description.is_some(),
            MetadataKey::DisplayName => self.display_name.is_some(),
            MetadataKey::HomepageUrl => self.homepage_url.is_some(),
            MetadataKey::IsDeprecated => self.is_deprecated,
            _ => false,
        }
    }
    fn is_deprecated(&self) -> bool {
        self.is_deprecated
    }

    fn download_latest(
        &self,
        kind: SourceKind,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<(String, PathBuf)>> + Send + '_>> {
        Box::pin(async move {
            let context = || format!("downloading {} from {}", &self.name, &self.url);
            let url = self.url.parse::<Url>().with_context(context)?;
            let client = super::build_client()?;

            let mut install_dest = ADDON_DIR
                .join(kind.get_unpack_dir())
                .join(&self.install_dir()[..]);

            let fname = url.path_segments().and_then(|segs| segs.last()).map(Path::new);
            let form = if let Some(fname) = fname {
                let form = RemoteAssetForm::with_asset(kind, fname, RemoteAssetForm::CONTENT_TYPE_BINARY);
                if let Some(RemoteAssetForm::File { .. }) = &form {
                    if let Some(ext) = fname.extension() {
                        let append = install_dest.as_mut_os_string();
                        append.push(".");
                        append.push(ext);
                    }
                }
                form
            } else {
                None
            }
            .unwrap_or(RemoteAssetForm::FILE);
            let res = {
                let req = client.get(url);
                req.send().await.and_then(|res| res.error_for_status())
            }
            .with_context(context)?;
            let id = Self::id_from_headers(res.headers()).with_context(context)?;
            match form {
                RemoteAssetForm::File { .. } => super::install_remote_file(&install_dest, res).await,
                RemoteAssetForm::Tarball { .. } => super::install_remote_tarball(&install_dest, res).await,
            }
            .with_context(context)?;
            Ok((id, install_dest))
        })
    }

    fn latest_id(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
        Box::pin(async move {
            let response = super::head(&self.url).await?;
            Self::id_from_headers(response.headers())
        })
    }
}
