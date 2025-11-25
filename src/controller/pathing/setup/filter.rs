use {
    crate::{
        controller::pathing::{
            PathingController,
            PathingEventContext,
            registry::{PackMapPath, PoiPath},
            filter::{AutoReset, HideContext},
        },
        exports::runtime as rt, settings::state::SaveState,
    },
    std::time::SystemTime,
    taimi_pack::attributes::keys::Guid,
    tokio::time::{Duration, Instant},
};

impl PathingController {
    pub(super) fn handle_guid_reset(&mut self, ctx: &mut PathingEventContext, guids: Vec<Guid>) {
        SaveState::try_write_with(|save| {
            let mut dirty = false;
            for guid in guids {
                if save.pathing().hidden_guid_expiry_get(&guid).is_some() {
                    save.pathing_mut().hidden_guid_expire(&guid);
                    dirty = true;
                }
                if self.filter_state.hidden.reset(&guid) {
                    ctx.filter_state_signal = true;
                }
                if ctx.unexpire(&Ok(guid)) {
                    ctx.filter_state_signal = true;
                }
            }
            dirty
        });
    }
    pub(super) async fn handle_dismiss(&mut self, ctx: &mut PathingEventContext, path: PoiPath<PackMapPath>, delay: Option<Duration>, expiry: Option<SystemTime>, hide_contexts: Vec<HideContext>) {
        let Some(guid) = ({
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
        }) else {
            // TODO: ctx.expire_at(Err(path.into()), expiry);
            log::warn!("no GUID on {path} to dismiss");
            return
        };
        let hidden = if let Some(expiry) = expiry {
            ctx.expire_at(Ok(guid.clone().into()), expiry, delay);
            let expiry_now = std::time::Instant::now();
            let expiry_std = expiry_now + if let Some(delay) = delay {
                delay
            } else {
                log::warn!("TODO: expiry to instant");
                Duration::from_secs(2)
            };
            self.filter_state.hidden.expire_at(guid.clone(), expiry_std)
        } else {
            self.filter_state.hidden.marker_mut(guid.clone())
        };
        if !hide_contexts.iter().all(|hide| match hide {
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
        let expiry = match (expiry, delay) {
            (Some(e), ..) => Some(Some(e)),
            (None, Some(delay)) =>
                SystemTime::now().checked_add(delay).map(Some),
            (None, None) if has_context =>
                Some(None),
            (None, None) =>
                SystemTime::now().checked_add(Duration::MAX).map(Some),
        }.unwrap_or_else(|| {
            log::error!("when is the future?");
            Some(SystemTime::now() + Duration::from_secs(3600 * 24 * 365 * 2))
        });
        if let Some(expiry) = expiry {
            SaveState::write_with(|save| {
                save.pathing_mut().hidden_guid_expire_at(guid.into(), expiry)
            });
        }
        ctx.filter_state_signal = true;
    }

    pub fn update_filter_state(&mut self, ctx: &mut PathingEventContext) {
        #[cfg(todo = "unnecessary")]
        {
            self.filter_state.festival = ctx.festivals.read().clone();
        }
        self.filter_state.achievements.update_from_save();
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
}
