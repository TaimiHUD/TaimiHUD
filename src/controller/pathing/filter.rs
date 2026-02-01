use {
    crate::{
        controller::pathing::{
            registry::{PackMapPath, PackPath, LoadedPoiPath, LoadedMarkerPath},
            state::{
                hidden::{AutoReset, HideContext},
                filter::FilterState,
                LoadedMaps, LoadedMapInfo, LoadedMapPack,
            },
            info::MapPackInfo,
            shared::HiddenGuids,
            PathingController, PathingEvent,
        },
        controller::runtime::WallInstant,
        exports::runtime as rt,
        settings::state::SaveState,
    },
    taimi_meta::packs::{
        collections::PackSet,
        id::{MarkerId, MarkerPath, IdVariant},
    },
    taimi_hoard::time::Timestamp,
    taimi_hoard::loc::{Locator, LocationRef},
    futures::future::Either,
    std::iter,
    std::sync::Arc,
    std::collections::BTreeMap,
    std::time::Duration,
    taimi_pack::attributes::keys::Guid,
    taimi_meta::packs::MapIndex,
    tokio::task::AbortHandle,
};

impl PathingController {
    pub(super) fn filter_dismiss_poi(&mut self, lpath: LoadedPoiPath<PackMapPath>, expiry: Option<Either<Timestamp, Duration>>, hide_contexts: Vec<HideContext>, reset: Option<AutoReset>) {
        let map_path = lpath.root;
        let lpoi_path: LoadedPoiPath = lpath.unscope();
        let (poi_path, guid) = match self.maps.lookup_with_info(&self.map_info, &map_path) {
            Some((map, map_info)) => {
                let guid = map.poi_guid_by_index(map_info, lpoi_path).cloned();
                let poi_path = map_info.poi_path(lpoi_path);
                (poi_path, guid)
            },
            None => (None, None),
        };
        let pack_path = map_path.root;
        let id = match guid {
            Some(guid) if !guid.is_empty() => MarkerId::from(guid.0.clone()),
            _ => if let Some(poi_path) = poi_path {
                let marker_path: MarkerPath = poi_path.pivot_from();
                match &hide_contexts {
                    #[cfg(todo)]
                    c if c.iter().any(|c| matches!(c, HideContext::Local(..))) =>
                        MarkerId::for_marker(path),
                    _ => MarkerId::for_marker(marker_path.pivot(pack_path)),
                }
            } else {
                let lmarker_path: LoadedMarkerPath = lpoi_path.pivot_to();
                MarkerId::for_marker(lmarker_path.pivot(map_path))
            },
        };
        let expiry = expiry.map(WallInstant::from_moment);
        let hidden = if let Some(expiry) = expiry.clone() {
            let ts = expiry.timestamp;
            self.expire_at(id.clone(), expiry);
            self.filter_state.hidden.expire_at(id.clone(), ts).0
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
        let expiry = match (expiry, reset) {
            // TODO: this is a mess
            (_, Some(AutoReset::Distance | AutoReset::MapChange)) =>
                None,
            (None, _) if has_context =>
                None,
            (Some(e), ..) => Some(e),
            (None, _) =>
                Some(WallInstant::far_future()),
        };
        if let (Some(expiry), Some(guid)) = (expiry, guid) {
            SaveState::write_with(|save| {
                save.pathing_mut().hidden_guid_expire_at(guid.into(), expiry.timestamp)
            });
        }
        self.filter_state_signal = Some(true);
        self.update_shared_hidden(Some(&mut iter::once(map_path)));
    }

    /// TODO: maintain interest list of states that are actually relevant to loaded marker filters
    pub fn update_filter_state(&mut self, now: Option<Timestamp>) -> bool {
        let mut dirty = false;
        if let Ok(ml) = rt::mumble_link_ptr() {
            dirty |= self.filter_state.map.update_from_mumblelink_context(&ml);
            dirty |= self.filter_state.avatar.update_from_mumblelink_context(&ml);
            // TODO: self.filter_state.character.update_from_mumblelink(ml);
        }
        if let Some(now) = now {
            dirty |= self.filter_state.hidden.reset_expired(&now);
        }
        #[cfg(todo)]
        {
            dirty |= self.update_filter_state_schedule(now);
        }
        dirty
    }
    pub fn filter_state_next_schedule(&mut self, _now: Option<Timestamp>) -> Option<Timestamp> {
        #[cfg(feature = "paths-schedule")]
        let next_scheduled = {
            self.filter_state.schedule.update_time();
            let mut next_scheduled = None;
            if let Some(now) = &_now {
                if let Some(map_id) = self.gameplay_map() {
                    let next_update = self.map_packs.iter_mut()
                        .filter(|(path, _)| path.path == map_id)
                        .filter_map(|(_, map)| {
                            map.filters.next_schedule_event(&now)
                        })
                        .min();
                    next_scheduled = match next_update {
                        #[cfg(todo = "unnecessary")]
                        Some(n) => n.signed_duration_since(now).to_std().ok(),
                        Some(n) => Some(n),
                        None => None,
                    };
                }
            }
        };
        let next_expire = self.filter_state.hidden.next_expiry();
        #[cfg(todo = "unnecessary")]
        let next_expire = next_expire
            .and_then(|expiry| expiry.checked_duration_since(std::time::Instant::now()));
        [
            #[cfg(feature = "paths-schedule")]
            next_schedule,
            next_expire,
        ].into_iter().flatten().min()
    }
    pub fn schedule_filter_state(&mut self, when: Option<Timestamp>) {
        let next = match when {
            Some(next) => {
                self.filter_next_schedule_dirty = true;
                WallInstant::from(next)
            },
            None => WallInstant::soon(Self::SCHEDULE_TIMEOUT),
            #[cfg(todo)]
            _ => return,
        };
        self.filter_next_schedule.set(next.to_future());
    }
    pub fn reschedule_filter_state(&mut self, now: Option<Timestamp>) {
        let when = self.filter_state_next_schedule(now);
        self.schedule_filter_state(when);
    }

    #[doc(alias = "mark_hidden_dirty")]
    pub fn update_shared_hidden(&self, dirty_packs: Option<&mut dyn Iterator<Item = PackMapPath>>) {
        let mut all_packs;
        let dirty_packs = match dirty_packs {
            Some(p) => p,
            None => {
                let Some(map_id) = self.gameplay_map() else { return };
                all_packs = self.packs.packs.paths().map(move |p| p.rel(map_id));
                &mut all_packs
            },
        };
        let maps = &self.maps;
        let state = &self.filter_state.hidden;
        if let (_, Some(0)) = dirty_packs.size_hint() { return }
        self.loader.shared.gameplay.send_if_modified(|shared_map| {
            let mut updated = false;
            for path in dirty_packs {
                let Some(shared_state) = shared_map.get_state_mut(path) else { continue };
                if let Some(map) = maps.lookup_ref(&path) {
                    updated |= shared_state.update_with_hidden(path, state, map).unwrap_or(true);
                }
            }
            updated
        });
    }

    #[inline]
    #[cfg(deleteme)]
    pub(super) fn unexpire_at(filter_expiry: &mut BTreeMap<MarkerId, AbortHandle>, item: &MarkerId) -> bool {
        if let Some(handle) = filter_expiry.remove(item) {
            handle.abort();
            true
        } else {
            false
        }
    }
    pub(super) fn unexpire(&mut self, item: impl AsRef<MarkerId>) -> bool {
        Self::unexpire_at(&mut self.scheduled_events, &mut self.filter_expiry, item.as_ref())
    }
    #[cfg(deleteme)]
    pub(super) fn expire_at(&mut self, item: MarkerId, expiry: WallInstant) {
        let handle = self.tasks.spawn(async move {
            let _ = expiry.to_future().await;
            Ok(PathingEvent::ResetMarkerIds(vec![item]))
        });
        // TODO: there's probably an entry replace api for this...
        self.unexpire(&item);
        self.filter_expiry.insert(item, handle);
    }
    pub(super) const SCHEDULE_TIMEOUT: Duration = Duration::from_secs(Timestamp::HOUR.as_secs() * 12);
}

/// [PathingEvent::ResetMarkerIds] and [PathingEvent::ResetMarkerPath]
impl PathingController {
    pub(super) fn process_filter_clear_ids(&mut self, ids: Vec<MarkerId>) {
        self.filter_clear_save_ids(&mut ids.iter());
        self.filter_clear_update_ids(&mut ids.into_iter());
    }
    pub(super) fn process_filter_clear_path(&mut self, path: MarkerPath<PackPath>) {
        let id = MarkerId::for_marker(path);
        self.filter_clear_update_ids(&mut iter::once(id));
    }
    fn filter_clear_save_ids(&mut self, ids: &mut dyn Iterator<Item = &'_ MarkerId>) -> bool {
        let guids = ids.filter_map(|id| match id.variant() {
            IdVariant::MarkerRegistered(..) | IdVariant::MarkerLoaded(..) => None,
            #[cfg(todo = "unnecessary")]
            IdVaraint::MarkerUnscoped(..) => None,
            _ => Some(Guid::from_uuid_ref(&id.uuid)),
        });
        self.filter_clear_save_guids(&mut {guids})
    }
    pub(super) fn filter_clear_update_ids(&mut self, ids: &mut dyn Iterator<Item = MarkerId>) {
        let map_info = &self.map_info;
        let ids = ids.flat_map(|id| {
            let variant = id.variant();
            let path = match variant {
                IdVariant::MarkerRegistered(path) => Some(path.root.rel(None)),
                IdVariant::MarkerLoaded(path) => Some(path.root.map_path(Some)),
                #[cfg(todo)]
                IdVariant::Group => {
                    // we could try to find it out but idk what index to check or when,
                    // plus it's possible for a GUID to span multiple packs!
                    lookup_guid()
                },
                _ => None,
            };
            let loaded = if let IdVariant::MarkerRegistered(path) = variant {
                Some(map_info.find_loaded_markers(path).map(MarkerId::for_marker))
            } else { None };
            let unloaded = if let IdVariant::MarkerLoaded(lpath) = variant {
                map_info.find_marker_path(lpath).map(MarkerId::for_marker)
            } else { None };
            iter::once(id)
                .chain(loaded.into_iter().flatten())
                .chain(unloaded)
                .map(move |id| (path, id))
        });
        let map_id = self.gameplay_map();
        let (mut hidden_dirty_packs, mut hidden_dirty_unk) = (PackSet::default(), false);
        for (path, id) in ids {
            let hidden_dirty = self.filter_state.hidden.reset(id);
            match path {
                _ if !hidden_dirty => (),
                Some(Locator { path: Some(pack_map), .. }) if Some(pack_map) != map_id => (),
                Some(path) => {
                    hidden_dirty_packs.insert(path.root);
                },
                None => hidden_dirty_unk = true,
            }
            let id_dirty = hidden_dirty | Self::unexpire_at(&mut self.scheduled_events, &mut self.filter_expiry, &id);
            if id_dirty {
                self.filter_state_signal = Some(true);
            }
        }
        if hidden_dirty_unk || !hidden_dirty_packs.is_empty() {
            // unknown means we cleared a GUID without checking which packs were affected,
            // so refresh all just to be safe
            let mut dirty_packs = match (hidden_dirty_unk, map_id) {
                (false, Some(map_id)) => Some(hidden_dirty_packs.iter().map(move |p| p.rel(map_id))),
                _ => None,
            };
            self.update_shared_hidden(dirty_packs.as_mut().map(|p| &mut *p as &mut dyn Iterator<Item = PackMapPath>));
        }
    }
    fn filter_clear_save_guids(&mut self, guids: &mut dyn Iterator<Item = &'_ Guid>) -> bool {
        if let (_, Some(0)) = guids.size_hint() { return false }
        SaveState::try_write_with(|save| {
            let mut dirty = false;
            for guid in guids {
                if dirty || save.pathing().hidden_guid_expiry_get(&guid).is_some() {
                    save.pathing_mut().hidden_guid_expire(&guid);
                    dirty = true;
                }
            }
            dirty
        })
    }

    pub(super) fn clone_hidden_guids() -> Option<Arc<HiddenGuids>> {
        SaveState::read_with(|s| s.pathing_state.as_ref().map(|p| p.hidden_guid_expiry.clone()))
    }
    pub(super) fn prune_hidden_guids_settings(now: &Timestamp) -> bool {
        SaveState::try_write_with(|save| {
            save.pathing_state.as_mut().map(|pathing| pathing.hidden_guid_prune_older_than(now))
                .unwrap_or(false)
        })
    }
    pub(super) fn populate_hidden_guids_for_map(
        filter_state: &mut FilterState,
        hidden_guids: &HiddenGuids,
        map: &LoadedMapPack,
        map_info: &MapPackInfo,
        now: Option<Timestamp>
    ) -> bool {
        let all_guids = map.marker_guids(map_info)
            .map(|(_, _, guid)| MarkerId::from_uuid_ref(guid.as_ref()));
        filter_state.hidden.populate_from_settings(hidden_guids, &mut {all_guids}, now)
    }
    #[cfg(todo = "unused")]
    pub(super) fn populate_hidden_guids(filter_state: &mut FilterState, maps: &LoadedMaps, map_info: &LoadedMapInfo, for_map_id: Option<MapIndex>, now: Option<Timestamp>) -> bool {
        let all_guids = maps.marker_guids(map_info, for_map_id).map(|(_, _, guid)| MarkerId::from_uuid_ref(guid.as_ref()));
        Self::clone_hidden_guids().map(move |hidden_guids| filter_state.hidden.populate_from_settings(&hidden_guids, &mut {all_guids}, now))
            .unwrap_or(false)
    }
    #[cfg(todo = "unused")]
    pub(super) fn refresh_hidden_guids_inner(filter_state: &mut FilterState, maps: &LoadedMaps, map_info: &LoadedMapInfo, for_map_id: Option<MapIndex>, now: Option<Timestamp>) -> bool {
        let mut dirty = Self::populate_hidden_guids(&mut *filter_state, maps, map_info, for_map_id, now);
        if let Some(now) = now {
            dirty |= filter_state.hidden.reset_expired(&now);
        }
        dirty
    }
    #[cfg(todo = "unused")]
    pub(super) fn refresh_hidden_guids(&mut self, for_map_id: Option<MapIndex>, now: Option<Timestamp>) -> bool {
        Self::refresh_hidden_guids_inner(&mut self.filter_state, &self.maps, &self.map_info, for_map_id, now)
    }
}
