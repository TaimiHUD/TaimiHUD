use {
    super::super::SourceKind,
    crate::{
        settings::{RemoteAssetForm, Source},
        ADDON_DIR,
    },
    anyhow::{anyhow, Context},
    reqwest::header,
    serde::{Deserialize, Serialize},
    std::{
        fmt,
        future::Future,
        path::{Path, PathBuf},
        pin::Pin,
    },
    url::Url,
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

impl DirectSource {
    pub fn id_from_headers(&self, headers: &reqwest::header::HeaderMap) -> anyhow::Result<String> {
        // TODO: prio etag (if not weak/temporary)
        let h = headers
            .get(header::LAST_MODIFIED)
            .ok_or_else(|| anyhow!("identifying header for {self} missing"))?
            .to_str()?;

        Ok(h.into())
    }
}

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
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<(String, PathBuf)>> + '_>> {
        Box::pin(async move {
            let context = || format!("downloading {self} from {}", &self.url);
            let url = self.url.parse::<Url>().with_context(context)?;
            let client = super::build_client()?;

            let mut install_dest = ADDON_DIR.join(kind.get_unpack_dir()).join(self.install_dir());

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
            let id = self.id_from_headers(res.headers()).with_context(context)?;
            match form {
                RemoteAssetForm::File { .. } => super::install_remote_file(&install_dest, res).await,
                RemoteAssetForm::Tarball { .. } => super::install_remote_tarball(&install_dest, res).await,
            }
            .with_context(context)?;
            Ok((id, install_dest))
        })
    }

    fn latest_id(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + '_>> {
        Box::pin(async move {
            let response = super::head(&self.url).await?;
            self.id_from_headers(response.headers())
        })
    }
}
