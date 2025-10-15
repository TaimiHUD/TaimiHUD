use {
    crate::{
        settings::RemoteSource,
        timer::{TimerPhase, TimerTrigger},
    },
    anyhow::anyhow,
    glob::Paths,
    relative_path::RelativePathBuf,
    serde::{Deserialize, Serialize},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
        future::Future,
    },
    tokio::{fs::read_to_string, sync::Semaphore, task::JoinSet},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimerFile {
    #[serde(default, skip)]
    pub association: Option<String>,
    pub path: Option<PathBuf>,
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub author: String,
    pub icon: RelativePathBuf,
    // I probably don't need to do this, but it's convenient :o
    #[serde(rename = "map")]
    pub map_id: u32,
    pub reset: TimerTrigger,
    pub phases: Vec<TimerPhase>,
}

impl TimerFile {
    pub fn glob() -> String {
        "**/*.bhtimer".to_string()
    }

    pub fn path_glob(path: &Path) -> PathBuf {
        path.join(&Self::glob())
    }

    pub fn get_paths(path: &Path) -> anyhow::Result<Paths> {
        let pathbuf_glob = Self::path_glob(path);

        let path_glob_str = pathbuf_glob
            .to_str()
            .ok_or_else(|| anyhow!("Timer file loading path glob unparseable for {path:?}"))?;
        Ok(glob::glob(path_glob_str)?)
    }

    pub async fn load(
        path: &PathBuf,
        source: Option<RemoteSource>,
    ) -> anyhow::Result<Arc<Self>> {
        log::trace!("Attempting to load the timer file at \"{path:?}\".");
        let mut file_data = read_to_string(path).await?;
        json_strip_comments::strip(&mut file_data)?;
        let mut data: Self = serde_json::from_str(&file_data)?;
        data.path = Some(path.to_path_buf());
        data.association = source.map(|x| x.name());
        log::trace!("Successfully loaded the timer file at \"{path:?}\".");
        Ok(Arc::new(data))
    }

    pub async fn load_many_sourceless(
        load_dir: &Path,
        simultaneous_limit: usize,
    ) -> anyhow::Result<Vec<Arc<Self>>> {
        log::debug!("Beginning load_many for {load_dir:?} with a simultaneous open limit of {simultaneous_limit}.");
        let mut set = JoinSet::new();
        let semaphore = Arc::new(Semaphore::new(simultaneous_limit));
        let mut paths = Self::get_paths(load_dir)?;
        while let Some(path) = paths.next() {
            let permit = semaphore.clone().acquire_owned().await?;
            let path = path?.clone();
            set.spawn(async move {
                let timer_file = Self::load(&path, None).await?;
                drop(permit);
                Ok::<Arc<TimerFile>, anyhow::Error>(timer_file)
            });
        }
        let mut timer_files = Vec::new();
        let (mut join_errors, mut load_errors): (usize, usize) = (0, 0);
        while let Some(timer_file) = set.join_next().await {
            match timer_file {
                Ok(res) => match res {
                    Ok(timer_file) => {
                        timer_files.push(timer_file);
                    }
                    Err(err) => {
                        load_errors += 1;
                        log::error!("Timer load_many error for {load_dir:?}: {err}");
                    }
                },
                Err(err) => {
                    join_errors += 1;
                    log::error!("Timer load_many join error for {load_dir:?}: {err}");
                }
            }
        }
        log::debug!(
            "Finished load_many for sourceless load, {load_dir:?}: {} succeeded, {join_errors} join errors, {load_errors} other errors.",
            timer_files.len()
        );
        Ok(timer_files)
    }

    pub async fn load_many(
        load_dir: &Path,
        source: RemoteSource,
        simultaneous_limit: usize,
    ) -> anyhow::Result<Vec<Arc<Self>>> {
        log::debug!("Beginning load_many for {load_dir:?} with a simultaneous open limit of {simultaneous_limit}.");
        let mut set = JoinSet::new();
        let semaphore = Arc::new(Semaphore::new(simultaneous_limit));
        let mut paths = Self::get_paths(load_dir)?;
        while let Some(path) = paths.next() {
            let permit = semaphore.clone().acquire_owned().await?;
            let path = path?.clone();
            let source = source.clone();
            set.spawn(async move {
                let timer_file = Self::load(&path, Some(source)).await?;
                drop(permit);
                Ok::<Arc<TimerFile>, anyhow::Error>(timer_file)
            });
        }
        let mut timer_files = Vec::new();
        let (mut join_errors, mut load_errors): (usize, usize) = (0, 0);
        while let Some(timer_file) = set.join_next().await {
            match timer_file {
                Ok(res) => match res {
                    Ok(timer_file) => {
                        timer_files.push(timer_file);
                    }
                    Err(err) => {
                        load_errors += 1;
                        log::error!("Timer load_many error for {load_dir:?}: {err}");
                    }
                },
                Err(err) => {
                    join_errors += 1;
                    log::error!("Timer load_many join error for {load_dir:?}: {err}");
                }
            }
        }
        log::debug!(
            "Finished load_many for {source}, {load_dir:?}: {} succeeded, {join_errors} join errors, {load_errors} other errors.",
            timer_files.len()
        );
        Ok(timer_files)
    }

    pub fn name(&self) -> String {
        self.name.replace("\n", " ")
    }
    pub fn title(&self) -> &str {
        self.name.split('\n').next().unwrap()
    }
    pub fn subtitle(&self) -> Option<String> {
        Some(self.name.split_once('\n')?.1.replace("\n", " - "))
    }
    pub fn source(&self) -> String {
        self.association.clone().unwrap_or_default()
    }
    pub fn combined(&self) -> String {
        match self.subtitle() {
            Some(sbubby) => format!("{}\n{}", self.title(), sbubby),
            None => self.name.clone(),
        }
    }
    pub fn hypheny_name(&self) -> String {
        self.name.replace("\n", " - ")
    }
    pub fn author(&self) -> String {
        self.author.replace("\n", "")
    }
}
