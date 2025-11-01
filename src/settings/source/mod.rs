use {
    crate::{exports::runtime as rt, settings::SourceKind},
    anyhow::Context,
    async_compression::tokio::bufread::GzipDecoder,
    futures::stream::{StreamExt, TryStreamExt},
    reqwest::{header, Client, ClientBuilder, IntoUrl, Request, Response},
    std::{
        fmt::{Debug, Display},
        future::Future,
        io,
        path::{Path, PathBuf},
        pin::Pin,
        sync::Arc,
    },
    tokio::{
        fs::{create_dir_all, remove_dir_all, File},
        io::AsyncWriteExt,
    },
    tokio_tar::Archive,
    tokio_util::io::StreamReader,
    url::Url,
};

pub mod direct;
pub mod github;

pub use {direct::DirectSource, github::GitHubSource};

pub type RemoteSource = Arc<dyn Source + Send + Sync>;

pub fn new_client() -> ClientBuilder {
    let name = rt::CRATE_NAME;
    let version = rt::CRATE_VERSION;
    let user_agent = format!("{name}/{version}");
    Client::builder().user_agent(user_agent)
}

pub fn build_client() -> anyhow::Result<Client> {
    new_client().build().map_err(Into::into)
}

pub fn url_escape<'a, F: ?Sized + AsRef<str>>(fragment: &'a F) -> impl Display + Clone + 'a {
    let fragment = fragment.as_ref();
    // TODO: this
    fragment
}

pub fn insert_header<H: header::IntoHeaderName + Display, V: AsRef<str>>(
    headers: &mut header::HeaderMap,
    header: H,
    value: V,
) {
    let value = value.as_ref();
    let value = value
        .parse()
        .with_context(|| format!("invalid {header} value {value:?}"));
    match value {
        Ok(v) => {
            headers.insert(header, v);
        },
        Err(e) => log::debug!("{e:#}"),
    }
}

pub async fn get<U: IntoUrl>(url: U) -> anyhow::Result<Response> {
    let client = build_client()?;
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp)
}

pub async fn head<U: IntoUrl>(url: U) -> anyhow::Result<Response> {
    let client = build_client()?;
    let resp = client.head(url).send().await?.error_for_status()?;
    Ok(resp)
}

pub async fn head_location_for(
    mut req: Request,
    redir_limit: usize,
) -> anyhow::Result<(Response, Option<Url>)> {
    use std::sync::Mutex as StdMutex;

    let latest_redir = Arc::new(StdMutex::new(None));
    let client = {
        let latest_redir = latest_redir.clone();
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            if let Ok(mut redir) = latest_redir.lock() {
                *redir = Some(attempt.url().clone());
            }
            match attempt.previous().len() < redir_limit {
                true => attempt.follow(),
                false => attempt.stop(),
            }
        });
        new_client().redirect(policy).build()
    }?;
    *req.method_mut() = reqwest::Method::HEAD;
    let res = client.execute(req).await?;
    drop(client);

    let latest_redir = match Arc::try_unwrap(latest_redir) {
        Ok(redir) => redir.into_inner().ok(),
        // not so important, just nice to avoid if possible?
        Err(redir) => redir.lock().ok().map(|redir| redir.clone()),
    }
    .flatten();

    Ok((res, latest_redir))
}

pub async fn get_location_for(req: Request, redir_limit: usize) -> anyhow::Result<Url> {
    let url = req.url().clone();
    let context = || format!("querying {url} for location");
    let (res, latest) = head_location_for(req, redir_limit).await.with_context(context)?;
    let status = res.error_for_status_ref().with_context(context);
    match latest {
        Some(latest) => {
            if let Err(e) = status {
                log::warn!("{e:#}");
            } else if res.status().is_redirection() {
                log::warn!("redirect limit hit while querying {url} for location");
            }
            if !res.status().is_success() {
                log::debug!("(stopped at {latest})");
            }
            Ok(latest)
        },
        None => status.map(|_| url),
    }
}

pub async fn install_remote_tarball(dir: &Path, res: Response) -> anyhow::Result<()> {
    let response = res.error_for_status()?;
    let bytes_stream = response.bytes_stream().map_err(io::Error::other);
    let stream_reader = StreamReader::new(bytes_stream);
    let gzip_decoder = GzipDecoder::new(stream_reader);
    let mut tar_file = Archive::new(gzip_decoder);
    let entries = tar_file.entries()?;
    let mut containing_directory: Option<PathBuf> = None;
    let mut iterator = entries;
    iterator.next().await; // skip pax_global_header
    if dir.exists() {
        log::info!("Directory {dir:?} exists already; removing prior to extraction.");
        // TODO: rename in case of failure, or unpack into temp dir then rename that after
        remove_dir_all(dir).await?;
    }
    while let Some(file) = iterator.next().await {
        let mut f = file?;
        let path = f.path()?;
        if let Some(prefix) = &containing_directory {
            let destination_suffix = path.strip_prefix(prefix)?;
            let destination_path = dir.join(destination_suffix);
            if let Some(destination_parent) = destination_path.parent() {
                create_dir_all(destination_parent).await?;
                f.unpack(destination_path).await?;
                //f.unpack_in(destination).await?;
            }
        } else {
            containing_directory = Some(path.into_owned());
        }
    }
    log::debug!("Completed fetching and extracting into {}", dir.display());
    Ok(())
}

pub async fn install_remote_file(dest: &Path, res: Response) -> anyhow::Result<()> {
    let response = res.error_for_status()?;

    // TODO: if dest is dir, remove... unlikely but could happen maybe?
    if let Some(dir) = dest.parent() {
        create_dir_all(dir).await?;
    }
    let mut f = File::create(dest).await?;

    let mut content = response.bytes_stream().map_err(io::Error::other);
    while let Some(bytes) = content.next().await {
        let mut bytes = bytes?;
        f.write_all_buf(&mut bytes).await?;
    }

    f.sync_all().await?;

    Ok(())
}

pub trait Source: Display + Debug {
    fn description(&self) -> Option<String>;
    fn name(&self) -> String;
    fn install_dir(&self) -> String;
    fn view_url(&self) -> String;
    fn download_latest(
        &self,
        kind: SourceKind,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<(String, PathBuf)>> + '_>>;
    fn latest_id(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + '_>>;
}
