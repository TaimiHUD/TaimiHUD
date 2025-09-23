use std::{
    collections::{HashMap, HashSet}, fs::exists, path::Path,
    io::pipe,
};

#[cfg(windows)]
use std::os::windows::io::OwnedHandle;
#[cfg(not(windows))]
use std::os::fd::OwnedFd;

//mod api;
use {
    //crate::api::*,
    futures_util::{SinkExt, StreamExt, TryStreamExt},
    log,
    std::{
        env::{current_dir, current_exe},
        path::PathBuf,
        sync::Arc,
    },
    strum_macros::Display,
    tokio::{
        fs::{create_dir_all, File},
        sync::Semaphore,
        task::JoinSet,
    },
    tokio_into_sink::IntoSinkExt as _,
};

use anyhow::anyhow;
use glam::Vec2;
use glamour::{Point2, Unit};
use glob::Paths;
use reqwest::{StatusCode, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{fs::read_to_string, task::spawn_blocking};

pub struct GW2APIClient {
    semaphore: Arc<Semaphore>,
}

type RequestsPerMinute = usize;

impl GW2APIClient {
    fn new(rate: RequestsPerMinute) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(rate)),
        }
    }

    async fn request<T: DeserializeOwned + Send + GW2APIObject + 'static>(ids: KindQuantity) -> anyhow::Result<Vec<T>, anyhow::Error> {
        let mut results = Vec::new();
        match ids {
            v@KindQuantity::All => {
                let result = download_kind_json::<T>(v).await?;
                results = result;

            },
            KindQuantity::Series(v) => {
                let max_request_id_count = 200_usize;
                for u in v.chunks(max_request_id_count) {
                    let mut result = download_kind_json::<T>(KindQuantity::Series(u.to_owned())).await?;
                    results.append(&mut result);
                }
            },
            v@KindQuantity::Individual(_i) => {
                let mut result = download_kind_json::<T>(v).await?;
                results.append(&mut result)
            },
        }
        Ok(results)
    }
}

pub enum KindQuantity {
    All,
    Series(Vec<usize>),
    Individual(usize),
}

pub trait GW2APIObject {
    fn api_endpoint(ids: KindQuantity) -> Url;
}

pub async fn download_kind_json<T: DeserializeOwned + Send + GW2APIObject + 'static>(ids: KindQuantity) -> anyhow::Result<Vec<T>> {
    let url = T::api_endpoint(ids);

    let download = reqwest::get(url).await?.error_for_status()?;
    let status = download.status();
    if status == StatusCode::from_u16(429)? {
        return Err(anyhow!("429 error, too many requests :("));
    }
    let (pipe_reader, pipe_writer) = pipe()?;
    let bytes_stream = download.bytes_stream().map_err(anyhow::Error::from);
    #[cfg(windows)]
    let pipe_file = tokio::fs::File::from(std::fs::File::from(OwnedHandle::from(pipe_writer)));
    #[cfg(not(windows))]
    let pipe_file = tokio::fs::File::from(std::fs::File::from(OwnedFd::from(pipe_writer)));
    let mut pipe_stream = pipe_file.into_sink().sink_map_err(anyhow::Error::from);

    bytes_stream.forward(&mut pipe_stream).await?;
    let t_deserialized: Vec<T> = spawn_blocking(move || {
        serde_json::from_reader(pipe_reader)
    }).await??;

    Ok(t_deserialized)
}
