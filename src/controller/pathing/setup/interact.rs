use {
    crate::{
        controller::pathing::{
            registry::{MapIndex, MarkerId, MarkerIndex, PoiIndex}, state::hidden::{AutoReset, HideContext}, visible::{InteractionEvent, InteractionEventAction, InteractivePoi}, PathingController, PathingEvent, PathingEventContext
        },
        exports::runtime::{self as rt, Locator}, render::{RenderEvent, RenderState}, settings::pathing::TriggerKind,
    },
    std::{cmp, collections::BinaryHeap, num::NonZero, sync::Arc, time::{Duration, UNIX_EPOCH}},
};

impl PathingController {
    const INTERACT_COOLDOWN: Duration = Duration::from_secs(120);

    pub(super) async fn handle_interaction(&mut self, ctx: &mut PathingEventContext, event: InteractionEvent) {
        let (path, loaded_path, ipoi, lpoi, action) = match event {
            InteractionEvent::Nearby { path, loaded_path, interactive_path } => {
                let Some(map) = self.map_packs.get(&loaded_path.root) else { return };
                let Some(ipoi) = map.interactive_pois.get(interactive_path.path as usize) else { return };
                let lpoi = map.pois.get(loaded_path.path as usize);
                let auto_trigger_configured = || {
                    log::debug!("TODO: auto-trigger setting");
                    true
                };
                let action = if ipoi.trigger.auto && auto_trigger_configured() {
                    InteractionEventAction::AutoTrigger
                } else {
                    return
                };
                (path, loaded_path, ipoi, lpoi, action)
            },
            InteractionEvent::Gone { path, loaded_path, interactive_path: _ } => {
                let Some(map_info) = self.map_pack_info.get(&loaded_path.root) else { return };
                let Some(map) = self.map_packs.get(&loaded_path.root) else { return };
                let marker_path = loaded_path.root.root.rel(MarkerIndex::with_poi(path.path));
                // TODO: nth with option variant
                let guid = map.poi_guids(map_info)
                    .find(|(p, _)| p.path == path.path)
                    .map(|(_, guid)| guid.clone());
                let mut removed = self.handle_interaction_end(ctx, &MarkerId::for_marker(marker_path));
                if let Some(guid) = guid {
                    removed |= self.handle_interaction_end(ctx, guid.as_ref());
                }
                if removed {
                    ctx.filter_state_signal = true;
                    self.mark_hidden_dirty(ctx, Some(loaded_path.root));
                }

                // remove on-screen info maybe?
                return
            },
            InteractionEvent::Interact { action, path, loaded_path, interactive_path } => {
                let Some(map) = self.map_packs.get(&loaded_path.root) else { return };
                let Some(ipoi) = map.interactive_pois.get(interactive_path.path as usize) else { return };
                let lpoi = map.pois.get(loaded_path.path as usize);
                (path, loaded_path, ipoi, lpoi, action)
            },
        };

        let mut behaviour = ipoi.behaviour.as_ref();
        let allowed = {
            let settings = self.loader.settings.read().await;
            let pathing = settings.pathing();
            let is_filtered = || {
                if lpoi.as_ref().map(|lpoi| !lpoi.visibility.is_visible()).unwrap_or(false) {
                    return true
                }
                log::debug!("TODO: POI autoreset filter");
                false
            };
            match action {
                InteractionEventAction::Trigger => TriggerKind::all(),
                InteractionEventAction::Dismiss(ref config) => {
                    behaviour = Some(config);
                    TriggerKind::DISMISS
                },
                InteractionEventAction::Manual(mask) => mask,
                action if action.is_natural() && is_filtered() => {
                    log::debug!("ignoring filtered POI interaction for {loaded_path}");
                    return
                },
                InteractionEventAction::Interact => pathing.trigger_allow_interact,
                InteractionEventAction::AutoTrigger => pathing.trigger_allow_auto,
            }
        };

        let blocked = "trigger settings blocked";
        if let InteractivePoi { info: Some(info), .. } = ipoi {
            if allowed.contains(TriggerKind::INFO) {
                ctx.spawn_alert(info.message.clone()[..].into(), Duration::from_secs(10));
            } else {
                log::info!("{blocked} info popup");
            }
        }
        if let Some(behaviour) = behaviour {
            if allowed.contains(TriggerKind::BEHAVIOUR) {
                const HOUR: Duration = Duration::from_secs(3600);
                const DAY: Duration = Duration::from_secs(HOUR.as_secs() * 24);
                const WEEK: Duration = Duration::from_secs(DAY.as_secs() * 7);
                const MANY_WEEKS: Duration = Duration::from_secs(WEEK.as_secs() * 52);

                use taimi_pack::attributes::keys::{Behaviour, TacoBehaviour, BlishBehaviour};
                let timestamp = rt::log::error_ok(UNIX_EPOCH.elapsed()).unwrap_or_default();
                let mut contexts = None;
                let mut reset = None;
                let delay = match behaviour.mode {
                    Behaviour::Taco(TacoBehaviour::ResetDaily) | Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) => Some(Duration::from_secs({
                        if let Behaviour::Taco(TacoBehaviour::ResetDailyPerCharacter) = behaviour.mode {
                            contexts = Some(HideContext::for_character(self.filter_state.character.name.clone()));
                        }
                        const SOME_DAY: Duration = Duration::from_secs(1754265600 - MANY_WEEKS.as_secs() * 13);
                        (SOME_DAY.as_secs() as i64).wrapping_sub(timestamp.as_secs() as i64).wrapping_rem_euclid(DAY.as_secs() as i64)
                    } as u64)),
                    Behaviour::Blish(BlishBehaviour::ResetWeekly) => Some(Duration::from_secs({
                        const SOME_WEEK: Duration = Duration::from_secs(1754265600 - MANY_WEEKS.as_secs() * 13);
                        (SOME_WEEK.as_secs() as i64).wrapping_sub(timestamp.as_secs() as i64).wrapping_rem_euclid(WEEK.as_secs() as i64)
                    } as u64)),
                    Behaviour::Taco(TacoBehaviour::ResetDelay) => Some(behaviour.reset_delay.duration()),
                    Behaviour::Taco(TacoBehaviour::AlwaysVisible) => Some(Duration::from_secs(0)),
                    Behaviour::Taco(TacoBehaviour::ResetPermanent) => {
                        reset = Some(AutoReset::Never);
                        None
                    },
                    Behaviour::Taco(TacoBehaviour::ResetMap) => {
                        contexts = Some(HideContext::for_map(loaded_path.root.path, None));
                        reset = Some(AutoReset::MapChange);
                        None
                    },
                    Behaviour::Taco(TacoBehaviour::ResetInstance) => {
                        contexts = Some(HideContext::for_map(loaded_path.root.path, NonZero::new(self.filter_state.map.shard_id)));
                        None
                    },
                    Behaviour::Taco(behaviour) => {
                        log::debug!("TODO: {behaviour:?}");
                        Some(HOUR)
                    },
                };
                log::info!("hiding marker for {delay:?}({contexts:?})");
                let contexts = contexts.into_iter().collect();
                PathingEvent::DismissMarker(loaded_path.root.rel(path.path), delay, contexts, reset).try_send();
            } else {
                log::info!("{blocked} dismiss behaviour");
            }
        }
        if let InteractivePoi { copy: Some(copy), .. } = ipoi {
            if allowed.contains(TriggerKind::COPY) {
                RenderState::try_send(RenderEvent::SendClipboard(copy.value[..].into()));
                let msg = copy.message.clone().map(|m| String::from(&m[..]))
                    .unwrap_or_else(|| crate::fl!("copied").into());
                let message = format!("{msg}\n\n{:?}", &copy.value.0[..]);
                ctx.spawn_alert(message, Duration::from_secs(6));
            } else {
                log::info!("{blocked} copy");
            }
        }
        for show_hide in ipoi.show_hide() {
            if allowed.contains(TriggerKind::TOGGLE) {
                let cat_path = show_hide.category().pivot(loaded_path.root.root);
                // TODO: spawn instead to ensure it arrives?
                PathingEvent::CategorySetToggle(cat_path, show_hide.action.tristate()).try_send();
            } else {
                log::info!("{blocked} {}", show_hide.action);
            }
        }
        if let InteractivePoi { reset: Some(reset), .. } = ipoi {
            if allowed.contains(TriggerKind::RESET) {
                PathingEvent::GuidReset(reset.guid.iter().cloned().collect()).try_send();
            } else {
                log::info!("{blocked} reset");
            }
        }
        if let InteractivePoi { script: Some(..), .. } = ipoi {
            if allowed.contains(TriggerKind::SCRIPT) {
                log::debug!("TODO: interact script");
            } else {
                log::info!("{blocked} script");
            }
        }
        if let InteractivePoi { bounce: Some(..), .. } = ipoi {
            if allowed.contains(TriggerKind::BOUNCE) {
                log::debug!("TODO: interact bounce anim");
            } else {
                log::info!("{blocked} animation");
            }
        }

        if behaviour.is_none() && action.is_natural() {
            let context = vec![HideContext::for_map(loaded_path.root.path, None)];
            PathingEvent::DismissMarker(loaded_path.root.rel(path.path), Some(Self::INTERACT_COOLDOWN), context, Some(AutoReset::Distance)).try_send();
        }
    }

    fn handle_interaction_end(&mut self, _ctx: &mut PathingEventContext, marker_id: &MarkerId) -> bool {
        let Some(hidden) = self.filter_state.hidden.hidden.get(marker_id) else {
            return false
        };
        match hidden.reset {
            AutoReset::Distance => (),
            AutoReset::Never | AutoReset::Expiry { .. } | AutoReset::MapChange =>
                return false,
        }

        self.filter_state.hidden.hidden.remove(marker_id).is_some()
    }

    pub(super) fn handle_press_interact(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex) {
        self.trigger_interact_action(ctx, map_id, InteractionEventAction::Interact)
    }

    fn trigger_interact_action(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex, action: InteractionEventAction) {
        let maps = self.map_packs.iter_mut()
            .filter(|(path, map)| path.path == map_id && !map.interactive_pois.is_empty());
        let mut playerpos = None;
        let mut nearby_pois = BinaryHeap::new();
        for (path, map) in maps {
            let Some(info) = self.map_pack_info.get(path) else { continue };
            let playerpos = playerpos.get_or_insert_with(|| PathingEventContext::read_player_pos().map(|pos| {
                ctx.player_pos = pos;
                pos
            })).clone();
            let Some(playerpos) = playerpos else { break };
            if map.interactive_pois_nearby.is_empty() {
                map.interactive_pois_nearby.resize(map.interactive_pois.len(), false);
            }

            let ipois = map.interactive_pois.iter()
                .zip(map.interactive_pois_nearby.iter_mut())
                .enumerate();
            for (i, (ipoi, nearby_bit)) in ipois {
                let Some(lpoi) = ipoi.loaded_poi(&map.pois) else { continue };
                let Some(nearby) = ipoi.is_nearby(lpoi.position, playerpos) else { continue };
                // TODO: *nearby_bit = true?
                let nearby_discrete = (nearby * 1_000_000.0)
                    .min(0x40000000u32 as f32) as u32;
                let prev_nearby = *nearby_bit;
                let auto_triggered = ipoi.is_passive() && prev_nearby;
                let interactive_path = Locator::with_path(i as PoiIndex);
                let loaded_path = path.rel(ipoi.loaded_index().path);
                let path =
                    info.pois().nth(loaded_path.path as usize)
                    .unwrap_or(Locator::with_path(PoiIndex::MAX));
                nearby_pois.push(cmp::Reverse((nearby_discrete, !ipoi.trigger.auto, !auto_triggered, (path, loaded_path, interactive_path))));
            }
        }
        if nearby_pois.is_empty() {
            // TODO: fall back to non-interactive pois in case user is trying to dismiss or get info about a marker?
            // (maybe on a different keybind though?)
            return
        }
        ctx.pack_info.send_if_modified(|shared_info| {
            for cmp::Reverse((_distdist, _, _, (path, loaded_path, interactive_path))) in nearby_pois {
                let _ = shared_info.interactions.send(InteractionEvent::Interact {
                    action,
                    path,
                    loaded_path,
                    interactive_path,
                });
            }
            false
        });
    }

    pub const UPDATE_INTERVAL_SLOW: Duration = Duration::from_secs(10);
    pub const UPDATE_INTERVAL_RESPONSIVE: Duration = Duration::from_millis(350);
    /// Don't bother re-scanning if player hasn't moved at least `sqrt(distance)` [metres](PackSpace}
    pub const UPDATE_DISTANCE_DISTANCE: f32 = 0.005;
    pub(super) async fn handle_update_tick(&mut self, ctx: &mut PathingEventContext, map_id: MapIndex) {
        // TODO: skip all processing if feature is disabled in settings
        let maps = self.map_packs.iter_mut()
            .filter(|(path, map)| path.path == map_id && !map.interactive_pois.is_empty());
        let mut playerpos = None;
        let mut nearby_changes = Vec::new();
        for (path, map) in maps {
            let Some(info) = self.map_pack_info.get(path) else { continue };
            let playerpos = playerpos.get_or_insert_with(|| {
                let prev = ctx.player_pos();
                match (PathingEventContext::read_player_pos(), prev) {
                    (Some(pos), Some(prev)) if pos.distance_squared(prev) < Self::UPDATE_DISTANCE_DISTANCE =>
                        None,
                    (Some(pos), _) => Some({
                        ctx.player_pos = pos;
                        pos
                    }),
                    _ => None,
                }
            }).clone();
            let Some(playerpos) = playerpos else { break };
            if map.interactive_pois_nearby.is_empty() {
                map.interactive_pois_nearby.resize(map.interactive_pois.len(), false);
            }

            let mut updated = Vec::new();
            let ipois = map.interactive_pois.iter()
                .zip(map.interactive_pois_nearby.iter_mut())
                .enumerate();
            for (i, (ipoi, mut nearby_bit)) in ipois {
                let prev_nearby = *nearby_bit;
                #[cfg(todo)]
                if !ipoi.is_passive() && !prev_nearby {
                    continue
                }
                let Some(lpoi) = ipoi.loaded_poi(&map.pois) else { continue };
                let nearby = ipoi.is_nearby(lpoi.position, playerpos).is_some();
                if nearby != prev_nearby {
                    *nearby_bit = nearby;
                    let interactive_path = Locator::with_path(i as PoiIndex);
                    let loaded_path = path.rel(ipoi.loaded_index().path);
                    let path =
                        info.pois().nth(loaded_path.path as usize)
                        .unwrap_or(Locator::with_path(PoiIndex::MAX));
                    updated.push(match nearby {
                        true => InteractionEvent::Nearby { path, loaded_path, interactive_path, },
                        false => InteractionEvent::Gone { path, loaded_path, interactive_path },
                    });
                }
            }
            if !updated.is_empty() {
                nearby_changes.push((path, Arc::new(map.interactive_pois_nearby.clone()), updated));
            }
        }
        if !nearby_changes.is_empty() {
            ctx.pack_info.send_if_modified(|shared_info| {
                for (path, nearby, events) in nearby_changes {
                    let Some(shared_map) = shared_info.map_state.get_mut(&path) else { continue };
                    shared_map.interactive_pois_nearby = nearby;
                    for e in events {
                        let _ = shared_info.interactions.send(e);
                    }
                }
                true
            });
        }
    }

}
