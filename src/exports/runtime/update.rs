use anyhow::{anyhow, Context};
use crate::settings::{GitHubSource, GitHubLatestRelease};
use tokio::{runtime, time::timeout};
use std::time::Duration;
use url::Url;

pub const GIT_REF_BRANCH_PREFIX: &'static str = "refs/heads/";
pub const GIT_REF_TAG_PREFIX: &'static str = "refs/tags/";
pub const GIT_REF_RELEASE_PREFIX: &'static str = "refs/tags/v";

pub fn latest_release_blocking(src: &GitHubSource, patience: Duration) -> anyhow::Result<GitHubLatestRelease> {
    log::info!("Checking for updates at {}...", src);

    let runner = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to start update check")?;

    runner.block_on(async move {
        let check = src.latest_release();
        timeout(patience, check).await
    }).context("Timed out while checking for updates").and_then(|res| res)
}

pub fn release_version(release: &GitHubLatestRelease) -> anyhow::Result<&str> {
    release.tag_name.strip_prefix("v")
        .ok_or_else(|| anyhow!("Latest version {} unrecognized", release.tag_name))
}

pub fn release_dll_url(release: &GitHubLatestRelease) -> anyhow::Result<&Url> {
    let dll_asset = release.assets.iter()
        .find(|a| a.name.ends_with(".dll") /*&& a.state == "uploaded"*/);

    dll_asset.and_then(|dll_asset|
        // asset.url can also work as long as Content-Type is set correctly...
        dll_asset.browser_download_url.as_ref()
    ).ok_or_else(|| anyhow!("Expected associated dll with release"))
}
