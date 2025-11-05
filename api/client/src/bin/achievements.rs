use taimi_api_client::{
    client::ApiClient,
    model::{
        achievements::Achievement,
        authenticated::account::{
            achievements::AccountAchievements,
            wizards_vault::{
                daily::WizardsVaultDailies,
                listings::WizardsVaultListing,
                special::WizardsVaultSpecials,
                weekly::WizardsVaultWeeklies,
            },
        },
    },
};
#[cfg(todo)]
use {std::time::Duration, tokio::time::sleep};

#[tokio_macros::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::new().default_filter_or("debug,reqwest=info")).init();
    log::info!("Rock and roll!");
    let mut client = ApiClient::new()?;
    /*let request = client.request_ids::<Achievement>()
        .await?;
    let mut id_len = request.len();
    while id_len > 0 {
        let id_len_after = id_len.saturating_sub(100);
        let request_small = &request[id_len_after..id_len];
        log::info!("Range: {:?}, {:?}", id_len_after, id_len);
        let request = client.request_bulk::<Achievement>(ApiRequest::Multi(request_small.into()))
            .await?;
        log::info!("{:#?}", request);
        sleep(Duration::from_secs(1)).await;
        id_len = id_len_after
    }*/
    let request = client.request_bulk::<WizardsVaultListing>(..).await?;
    log::info!("{:#?}", request);
    /*for item in request {
        log::info!("{:?}", item);
    }*/
    Ok(())
}
