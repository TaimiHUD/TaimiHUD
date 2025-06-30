use {
    async_compression::tokio::bufread::GzipDecoder,
    futures::stream::{StreamExt, TryStreamExt},
    reqwest::{Client, IntoUrl, Response},
    serde::{Deserialize, Serialize},
    std::{
        fmt::{self, Display},
        io,
        path::{Path, PathBuf},
    },
    tokio::fs::{create_dir_all, remove_dir_all},
    tokio_tar::Archive,
    tokio_util::io::StreamReader,
};

mod github;

pub use github::{GitHubSource, GitHubLatestRelease};

#[derive(Deserialize, Serialize, Hash, Eq, PartialEq, Debug, Clone)]
#[serde(tag = "type")]
pub enum RemoteSource {
    GitHub(GitHubSource),
}

impl RemoteSource {
    pub fn source(&self) -> GitHubSource {
        match self {
            RemoteSource::GitHub(gs) => gs.clone(),
        }
    }

    #[allow(dead_code)]
    pub async fn download_latest(&self) -> anyhow::Result<String> {
        match self {
            RemoteSource::GitHub(gs) => Ok(gs.download_latest().await?),
        }
    }
}

impl fmt::Display for RemoteSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let RemoteSource::GitHub(inner) = self;
        /*let inner = match self {
            RemoteSource::GitHub(gs) => gs,
        };*/

        write!(f, "{}", inner)
    }
}

pub trait Source: Display {
    fn install_dir(&self) -> String;
    fn view_url(&self) -> String;
    async fn download_latest(&self) -> anyhow::Result<String>;
    async fn latest_id(&self) -> anyhow::Result<String>;

    async fn get<U: IntoUrl>(url: U) -> anyhow::Result<Response> {
        let name = env!("CARGO_PKG_NAME");
        let authors = env!("CARGO_PKG_AUTHORS");
        let user_agent = format!("{} by {}", name, authors);
        let client = Client::builder().user_agent(user_agent).build()?;
        let resp = client.get(url).send().await?.error_for_status()?;
        Ok(resp)
    }

    async fn get_and_extract_tar<U: IntoUrl>(dir: &Path, url: U) -> anyhow::Result<()> {
        let url = url.into_url()?;
        log::debug!("Beginning to fetch and extract into {dir:?} from {:?}", url);
        let response = Self::get(url.clone()).await?;
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
        log::debug!(
            "Completed fetching and extracting into {dir:?} from {:?}",
            url
        );
        Ok(())
    }
}
