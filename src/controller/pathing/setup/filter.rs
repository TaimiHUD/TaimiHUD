use {
    anyhow::Context,
    super::PathingEvent, crate::{
        controller::{api::{ApiAccountInfo, ApiController, ApiMessage}, pathing::{
            filter::{AchievementState, RaidState}, registry::{MarkerId, MarkerIndex, PackMapPath, PoiPath}, setup::PathingTaskBox, state::hidden::{AutoReset, HideContext}, PathingController, PathingEventContext
        }},
        exports::runtime as rt, settings::{pathing::{PathingAchievementApi, PathingAchievementSave}, state::SaveState},
    }, bitvec::array::BitArray, futures::future::Either, std::{future::Future, path::{Path, PathBuf}, time::SystemTime}, taimi_pack::attributes::keys::{self, Guid}, tokio::{fs, io::AsyncReadExt, time::{Duration, Instant}},
};

impl PathingController {
    pub(super) fn handle_guid_reset<'g, G>(&mut self, ctx: &mut PathingEventContext, guids: G) where
        G: IntoIterator<Item = &'g Guid> + Clone,
    {
        SaveState::try_write_with(|save| {
            let mut dirty = false;
            for guid in guids.clone() {
                if save.pathing().hidden_guid_expiry_get(&guid).is_some() {
                    save.pathing_mut().hidden_guid_expire(&guid);
                    dirty = true;
                }
            }
            dirty
        });
        for guid in guids {
            if self.filter_state.hidden.reset(&guid) {
                self.mark_hidden_dirty(ctx, None);
                ctx.filter_state_signal = true;
            }
            if ctx.unexpire(&guid) {
                ctx.filter_state_signal = true;
            }
        }
    }
    pub(super) async fn handle_dismiss(&mut self, ctx: &mut PathingEventContext, path: PoiPath<PackMapPath>, delay: Option<Duration>, expiry: Option<SystemTime>, hide_contexts: Vec<HideContext>, reset: Option<AutoReset>) {
        let guid = {
            self.map_pack_info.get(&path.root)
                .and_then(|info| self.map_packs.get(&path.root)
                    .map(|map| (map, info))
                ).and_then(|(map, info)| map.poi_guids(info)
                    .find(|(p, ..)| p.path == path.path)
                    .map(|(_, guid)| guid.clone())
                )
            /*Self::packs().read().await.lookup_ref(&path.root)
                .and_then(|pack| pack.active.as_ref())
                .and_then(|active| active.pack.pois.get(path.path as usize))
                .and_then(|poi| match poi.guid {
                    guid if guid == Uuid::nil() => None,
                    guid => Some(guid),
                })*/
        };
        let id = match guid {
            Some(guid) => MarkerId::from(guid.clone()),
            None => {
                let path = path.map_path(MarkerIndex::with_poi);
                match &hide_contexts {
                    #[cfg(todo)]
                    c if c.iter().any(|c| matches!(c, HideContext::Local(..))) =>
                        MarkerId::for_marker(path),
                    _ => MarkerId::for_marker(path.map_root(|map_path| map_path.root)),
                }
            },
        };
        let hidden = if let Some(expiry) = expiry {
            ctx.expire_at(id.clone(), expiry, delay);
            let expiry_now = std::time::Instant::now();
            let expiry_std = expiry_now + if let Some(delay) = delay {
                delay
            } else {
                log::warn!("TODO: expiry to instant");
                Duration::from_secs(2)
            };
            self.filter_state.hidden.expire_at(id.clone(), expiry_std)
        } else {
            self.filter_state.hidden.marker_mut(id.clone())
        };
        if let Some(reset) = &reset {
            hidden.reset = reset.clone();
        } else if !hide_contexts.iter().all(|hide| match hide {
            HideContext::Local(map) if map.shard.is_none() =>
                false,
            _ => true,
        }) && matches!(&hidden.reset, AutoReset::Never) {
            hidden.reset = AutoReset::MapChange;
        }
        let has_context = !hide_contexts.is_empty();
        if has_context {
            hidden.contexts.extend(hide_contexts);
        }
        let expiry = match (expiry, delay, reset) {
            // TODO: this is a mess
            (_, _, Some(AutoReset::Distance | AutoReset::MapChange)) =>
                Some(None),
            (None, None, _) if has_context =>
                Some(None),
            (Some(e), ..) => Some(Some(e)),
            (None, Some(delay), _) =>
                SystemTime::now().checked_add(delay).map(Some),
            (None, None, _) =>
                SystemTime::now().checked_add(Duration::MAX).map(Some),
        }.unwrap_or_else(|| {
            log::error!("when is the future?");
            Some(SystemTime::now() + Duration::from_secs(3600 * 24 * 365 * 2))
        });
        if let (Some(expiry), Some(guid)) = (expiry, guid) {
            SaveState::write_with(|save| {
                save.pathing_mut().hidden_guid_expire_at(guid.into(), expiry)
            });
        }
        ctx.filter_state_signal = true;
        self.mark_hidden_dirty(ctx, Some(path.root));
    }

    pub fn update_filter_state(&mut self, ctx: &mut PathingEventContext) {
        #[cfg(todo = "unnecessary")]
        {
            self.filter_state.festival = ctx.festivals.read().clone();
            self.filter_state.achievements.update_from_save();
        }
        if let Ok(ml) = rt::mumble_link_ptr() {
            self.filter_state.map.update_from_mumblelink_context(&ml);
            self.filter_state.avatar.update_from_mumblelink_context(&ml);
            // TODO: self.filter_state.character.update_from_mumblelink(ml);
        }
        self.update_filter_state_schedule(ctx);
    }
    pub fn update_filter_state_schedule(&mut self, ctx: &mut PathingEventContext) {
        #[cfg(feature = "paths-schedule")]
        let next_scheduled = {
            self.filter_state.schedule.update_time();
            let mut next_scheduled = None;
            if let Some(now) = &self.filter_state.schedule.now {
                if let Some(map_id) = ctx.gameplay_map() {
                    let next_update = self.map_packs.iter_mut()
                        .filter(|(path, _)| path.path == map_id)
                        .filter_map(|(_, map)| {
                            map.filters.next_schedule_event(&now)
                        })
                        .min();
                    next_scheduled = next_update.and_then(|next|
                        next.signed_duration_since(now).to_std().ok()
                    );
                }
            }
        };
        let next_expire = self.filter_state.hidden.next_expiry()
            .and_then(|expiry| expiry.checked_duration_since(std::time::Instant::now()));
        let next = [
            #[cfg(feature = "paths-schedule")]
            next_schedule,
            next_expire,
        ].into_iter().flatten().min();
        let next = next.or(if ctx.next_schedule.is_elapsed() {
            Some(PathingEventContext::SCHEDULE_TIMEOUT)
        } else {
            None
        });
        if let Some(next) = next {
            ctx.next_schedule.as_mut().reset(Instant::now() + next);
        }
    }

    pub fn mark_hidden_dirty(&self, ctx: &mut PathingEventContext, path: Option<PackMapPath>) {
        let state = &self.filter_state.hidden;
        let map_packs = &self.map_packs;
        ctx.pack_info.send_if_modified(|shared| {
            let mut updated = false;
            let shared_map = path
                .and_then(|path| shared.map_state.get_mut(&path)
                    .map(|shared_map| (path, shared_map))
                );
            if let Some((path, shared_map)) = shared_map {
                if let Some(map_pack) = map_packs.get(&path) {
                    updated = shared_map.update_with_hidden(path, state, map_pack);
                }
            } else {
                for (&path, shared_map) in &mut shared.map_state {
                    if let Some(map_pack) = map_packs.get(&path) {
                        updated |= shared_map.update_with_hidden(path, state, map_pack);
                    }
                }
            }
            updated
        });
    }

    /// TODO: this
    /// (assuming false as long as any data exists for now)
    pub fn api_info_outdated(&mut self, endpoint: ApiAccountInfo) -> bool {
        match endpoint {
            #[cfg(todo)]
            ApiAccountInfo::RaidClears => last_updated < start_of_week || recently_left_raid_map,
            #[cfg(todo)]
            ApiAccountInfo::Achievements => last_updated < settings.achievement_update_period_id || recently_left_story_map,
            _ => false,
        }
    }
    /// list of paths to load or populate with given api token
    pub fn api_setup_get(&mut self, _ctx: &mut PathingEventContext) -> impl Future<Output = Vec<(ApiAccountInfo, Either<PathBuf, Option<String>>)>> + 'static {
        let (account_name, token_id) = match ApiController::current_account_token() {
            Ok(token) if token.id().is_none() => (
                Some(token.account_name),
                None,
            ),
            Ok(token) => (
                Some(token.account_name),
                Some(token.id),
            ),
            Err(account_name) => (
                account_name,
                None,
            ),
        };

        let mut outdated: BitArray<[u32; 1]> = Default::default();
        for (&endpoint, mut outdated) in Self::API_ENDPOINTS.iter().zip(outdated.iter_mut()) {
            *outdated = self.api_info_outdated(endpoint);
        }
        async move {
            let mut res = Vec::with_capacity(Self::API_ENDPOINTS.len());
            if let Some(acc) = &account_name {
                let mut account_path = ApiController::account_path(&acc);
                for (&endpoint, outdated) in Self::API_ENDPOINTS.into_iter().zip(outdated) {
                    rt::path_join_append(&mut account_path, endpoint.filename());
                    let missing = fs::try_exists(&account_path).await.ok() == Some(false);
                    if !missing {
                        res.push((endpoint, Either::Right(token_id.clone())));
                    }
                    if missing || (token_id.is_some() && outdated) {
                        res.push((endpoint, Either::Left(account_path.clone())));
                    };
                    account_path.pop();
                }
            }
            res
        }
    }
    pub(super) fn api_setup_get_each((endpoint, missing): (ApiAccountInfo, Either<PathBuf, Option<String>>)) -> Option<PathingTaskBox> {
        match missing {
            Either::Left(load_path) =>
                return Some(Box::pin(Self::api_load_info_from(endpoint, load_path))),
            Either::Right(Some(token_id)) => {
                ApiMessage::RefreshAccount {
                    endpoint,
                    token_id: Some(token_id),
                }.try_send();
            },
            Either::Right(None) => (),
        }
        None
    }
    pub(super) async fn api_load_info_from(endpoint: ApiAccountInfo, path: PathBuf) -> Option<PathingEvent> {
        let res = match endpoint {
            ApiAccountInfo::RaidClears => {
                Self::api_load_info_from_raids(path).await
                    .map(PathingEvent::AccountInfoRaidClears)
            },
            ApiAccountInfo::Achievements => {
                Self::api_load_info_from_achievements(path).await
                    .map(PathingEvent::AccountInfoAchievements)
            },
        }.with_context(|| format!("parsing account {endpoint}"));

        rt::log::error_ok(res)
    }
    async fn api_load_info_from_achievements(path: PathBuf) -> anyhow::Result<AchievementState> {
        let achievements = Self::deserialize_path::<PathingAchievementApi>(&path).await
            .map(PathingAchievementSave::from)?;
        Ok(AchievementState::new(achievements))
    }
    async fn api_load_info_from_raids(path: PathBuf) -> anyhow::Result<RaidState> {
        let clears = Self::deserialize_path::<Vec<keys::Raid>>(&path).await?;
        Ok(RaidState::new(clears))
    }
    async fn deserialize_path<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
        let mut f = fs::File::open(path).await?;
        let mut data = Vec::with_capacity(match () {
            #[cfg(windows)]
            () => {
                use std::os::windows::fs::MetadataExt;
                f.metadata().await.ok().and_then(|meta|
                    meta.file_size().try_into().ok()
                )
            },
            #[cfg(not(windows))]
                _ => None::<usize>,
        }.unwrap_or(0x1000));
        f.read_to_end(&mut data).await?;
        serde_json::from_slice::<T>(&data)
            .map_err(anyhow::Error::from)
    }
}
