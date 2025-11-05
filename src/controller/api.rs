use {
    crate::settings::source::build_client,
    gw2lib::model::achievements::categories::AchievementCategories,
    reqwest::Client as HttpClient,
    serde::{de::DeserializeOwned, Serialize},
    std::collections::HashMap,
    taimi_pack::attributes::Festival,
};

pub(crate) type Gw2ApiKey = String;
pub(crate) enum ApiEvent {}

#[derive(Default)]
pub(crate) struct ApiController {
    client: Client,
}
// TODO: wire up settings api_key
impl ApiController {
    pub fn new() -> anyhow::Result<Self> {
        let client = build_client()?;
        Ok(Self { client })
    }

    // TODO: implement language selection for the api client
    pub fn setup(&self, api_key: Option<Gw2ApiKey>) {
        if let Some(api_key) = api_key {
            //self.client.api_key(api_key);
            log::info!("API key was provided, authenticated endpoints will be available.");
        } else {
            log::warn!("No API key provided, only unauthenticated endpoints will be available.");
        }
    }

    pub fn current_festival(&self) -> Option<Festival> {
        let mut festival_categories = HashMap::new();
        festival_categories.insert(79, Festival::Halloween);
        festival_categories.insert(98, Festival::Wintersday);
        festival_categories.insert(162, Festival::SuperAdventureBox);
        festival_categories.insert(201, Festival::LunarNewYear);
        festival_categories.insert(213, Festival::FourWinds);
        festival_categories.insert(233, Festival::DragonBash);

        //let all_categories: AchievementCategories = self.client.many(festival_categories.keys()).unwrap();

        None
    }
}
