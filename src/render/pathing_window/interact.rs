use {
    super::PathingWindowState,
    crate::{
        controller::pathing::PathingEvent, exports::runtime::{imgui::{
            Id, TableColumnFlags, TableColumnSetup, TableFlags, Ui,
        }, Locator}, fl, render::machine::RenderMachine, space::engine::Engine, Controller,
    },
};

impl PathingWindowState {
    pub fn draw_pois(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        _engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        use crate::controller::pathing::visible::{InteractionEvent, InteractionEventAction};
        use crate::controller::pathing::registry::MarkerIndex;
        use crate::settings::pathing::TriggerKind;
        let Some(Some(map_id)) = Controller::with_sender(|s| s.gameplay.as_ref().and_then(|g|
                g.borrow().gameplay_map()
        )) else { return };
        let Some(pack_info) = &machine.pack_info else { return };
        let table_flags =
            TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
        let table_token = ui.begin_table_header_with_flags(
            "interactive_pois",
            [
                TableColumnSetup {
                    name: &fl!("name"),
                    flags: TableColumnFlags::WIDTH_STRETCH,
                    init_width_or_weight: 0.0,
                    user_id: Id::Str("name"),
                },
                TableColumnSetup {
                    name: &fl!("category"),
                    flags: TableColumnFlags::WIDTH_FIXED,
                    init_width_or_weight: 0.0,
                    user_id: Id::Str("cat"),
                },
            ],
            table_flags,
        );
        ui.table_next_column();
        for (path, map) in &machine.map_pack_state {
            let path = path.rel(map_id);
            let pack_info = pack_info.borrow();
            let Some(info) = pack_info.map_info.get(&path) else { continue };
            let ipois = map.interactive_pois.iter().enumerate()
                .zip(map.interactive_pois_nearby.iter())
                .filter_map(|((i, ipoi), nearby)| match *nearby {
                    true => Some((i, ipoi)),
                    false => None,
                });
            for (ipoii, ipoi) in ipois {
                let loaded_path = ipoi.loaded_index();
                // TODO: cache this in the refresh .-.
                let Some(poi_path) = info.pois().nth(loaded_path.path as usize) else { continue };
                let poi_path = path.rel(poi_path.path);
                let guid_idx = info.poi_guid_mask()
                    .take(loaded_path.path as usize)
                    .filter(|&has| has)
                    .count();
                let guid = map.poi_guids.get(guid_idx).cloned();

                ui.text(guid.unwrap_or_default().to_string());

                ui.same_line();
                if ui.small_button("trigger") {
                    let _ = pack_info.interactions.send(InteractionEvent::Interact {
                        action: InteractionEventAction::Trigger,
                        path: poi_path.unscope(),
                        loaded_path: path.rel(loaded_path.path),
                        interactive_path: Locator::with_path(ipoii as u32),
                    });
                }
                if let Some(r) = &ipoi.reset {
                    ui.same_line();
                    if ui.small_button("reset") {
                        PathingEvent::GuidReset(r.guid.iter().cloned().collect()).try_send();
                    }
                }
                for (i, showhide) in ipoi.show_hide().enumerate() {
                    if i > 0 {
                        ui.same_line();
                    }
                    if ui.small_button(showhide.action.to_string()) {
                        let cat_path = showhide.category().pivot(path.root);
                        PathingEvent::CategorySetToggle(cat_path, showhide.action.tristate()).try_send();
                    }
                }
                if let Some(b) = &ipoi.behaviour {
                    if ui.small_button("dismiss") {
                        log::debug!("TODO: dismiss");
                        //PathingEvent::DismissMarker(poi_path, std::time::Duration::from_secs(5)).try_send();
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::BEHAVIOUR),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }
                if let Some(b) = &ipoi.copy {
                    if ui.small_button("copy") {
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::COPY),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }
                if let Some(b) = &ipoi.info {
                    if ui.small_button("info") {
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::INFO),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }
                if let Some(b) = &ipoi.bounce {
                    if ui.small_button("anim") {
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::BOUNCE),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }
                if let Some(b) = &ipoi.script {
                    if ui.small_button("script") {
                        let _ = pack_info.interactions.send(InteractionEvent::Interact {
                            action: InteractionEventAction::Manual(TriggerKind::SCRIPT),
                            path: poi_path.unscope(),
                            loaded_path: path.rel(loaded_path.path),
                            interactive_path: Locator::with_path(ipoii as u32),
                        });
                    }
                }

                ui.table_next_column();
                // TODO
                let hidden = true;
                if hidden && ui.small_button("unhide") {
                    if let Some(guid) = guid {
                        PathingEvent::GuidReset(vec![guid])
                    } else {
                        PathingEvent::ResetMarker(path.root.rel(MarkerIndex::with_poi(poi_path.path)))
                    }.try_send();
                }
                ui.table_next_column();
            }
        }
        drop(table_token);
    }
}
