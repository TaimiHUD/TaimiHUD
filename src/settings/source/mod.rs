use {
    crate::{exports::runtime as rt, settings::SourceKind},
    anyhow::anyhow,
    async_compression::tokio::bufread::GzipDecoder,
    futures::stream::{StreamExt, TryStreamExt},
    reqwest::{Client, IntoUrl, Response},
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
};

mod direct;
mod github;

use reqwest::header::LAST_MODIFIED;
pub use {
    direct::DirectSource,
    github::{GitHubLatestRelease, GitHubSource},
};

pub type RemoteSource = Arc<dyn Source + Send + Sync>;

pub async fn build_client() -> anyhow::Result<Client> {
    let name = rt::CRATE_NAME;
    let version = rt::CRATE_VERSION;
    let user_agent = format!("{name}/{version}");
    let client = Client::builder().user_agent(user_agent).build()?;
    Ok(client)
}

pub async fn get<U: IntoUrl>(url: U) -> anyhow::Result<Response> {
    let client = build_client().await?;
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp)
}

pub async fn head<U: IntoUrl>(url: U) -> anyhow::Result<Response> {
    let client = build_client().await?;
    let resp = client.head(url).send().await?.error_for_status()?;
    Ok(resp)
}

pub async fn download_file<U: IntoUrl>(dir: &Path, url: U) -> anyhow::Result<String> {
    let url = url.into_url()?;
    log::debug!("Fetching file into {dir:?} from {:?}", url);
    let response = get(url.clone()).await?;
    let filename = url
        .path()
        .split("/")
        .last()
        .ok_or_else(|| anyhow!("Should've had a filename, blegh!"))?;
    let meep = response
        .headers()
        .get(LAST_MODIFIED)
        .ok_or_else(|| anyhow!("I can't believe you've done this"))?
        .to_str()?
        .to_string();
    let bitey = response.bytes().await?;
    let final_path = dir.join(filename);
    let mut file = File::create(final_path).await?;
    file.write(&bitey).await?;
    file.flush().await?;
    Ok(meep)
}

pub async fn get_and_extract_tar<U: IntoUrl>(dir: &Path, url: U) -> anyhow::Result<()> {
    let url = url.into_url()?;
    log::debug!("Beginning to fetch and extract into {dir:?} from {:?}", url);
    let response = get(url.clone()).await?;
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
    log::debug!("Completed fetching and extracting into {dir:?} from {:?}", url);
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
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + '_>>;
    fn latest_id(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + '_>>;
}
