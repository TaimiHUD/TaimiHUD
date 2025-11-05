use {
    api::{ApiController, ApiRequest},
    gw2lib::model::{
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
    std::time::Duration,
    tokio::time::sleep,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("Rock and roll!");
    let mut api_controller = ApiController::new()?;
    /*let request = api_controller.request_ids::<Achievement>()
        .await?;
    let mut id_len = request.len();
    while id_len > 0 {
        let id_len_after = id_len.saturating_sub(100);
        let request_small = &request[id_len_after..id_len];
        log::info!("Range: {:?}, {:?}", id_len_after, id_len);
        let request = api_controller.request_bulk::<Achievement>(ApiRequest::Multi(request_small.into()))
            .await?;
        log::info!("{:#?}", request);
        sleep(Duration::from_secs(1)).await;
        id_len = id_len_after
    }*/
    let request = api_controller
        .request_bulk::<WizardsVaultListing>(ApiRequest::All)
        .await?;
    log::info!("{:#?}", request);
    /*for item in request {
        log::info!("{:?}", item);
    }*/
    Ok(())
}
