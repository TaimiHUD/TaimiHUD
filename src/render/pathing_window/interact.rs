use std::iter;

use taimi_pack::attributes::keys::Guid;

use {
    super::PathingWindowState,
    crate::{
        controller::pathing::{registry::PoiPath, PathingEvent}, exports::runtime::{imgui::{
            TableToken, Selectable,
            Id, TableColumnFlags, TableColumnSetup, TableFlags, Ui,
        }, Locator}, with_i18n, render::machine::RenderMachine, space::engine::Engine, Controller,
    },
};
use crate::controller::pathing::{registry::{MarkerId, MarkerPath, PackInfo, PackMapPath, PackPath, PoiIndex}, visible::{InteractionEvent, InteractionEventAction, InteractivePoi}, SharedMapPackLoaded, SharedMapPackState};
use crate::controller::pathing::registry::MarkerIndex;
use crate::settings::pathing::TriggerKind;

impl PathingWindowState {
    pub fn draw_pois(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        let mut engine = match engine {
            Some(Ok(e)) => Some(e),
            _ => None,
        };
        let Some(Some(map_id)) = Controller::with_sender(|s| s.gameplay.as_ref().and_then(|g|
            g.borrow().gameplay_map()
        )) else { return };

        self.draw_pois_header(ui, machine, &mut engine);

        let engine = match engine {
            Some(e) => Some(&*e),
            _ => None,
        };
        let map_packs =
            machine.map_pack_state.iter()
                .map(|(path, map)| {
                    let map_path = path.rel(map_id);
                    let map_info = self.pack_info.as_ref()
                        .and_then(|pi| pi.borrow().map_info.get(&map_path).cloned());
                    let pack_info = self.pack_loader_info.as_ref().map(|i| i.borrow())
                        .and_then(|i| i.get(path.path as usize)
                            .and_then(|i| i.info.as_ref().ok().cloned())
                        );
                    #[cfg(todo)]
                    let info = self.pack_info.as_ref()
                        .and_then(|pi| pi.borrow().pack_info.get(&path))
                        .and_then(|i| i.ok().cloned());
                    (map_path, pack_info, map_info, map.clone())
                }).collect::<Vec<_>>();
        let ipois = map_packs.iter().flat_map(|(map_path, pack_info, map_info, map)|
            map_info.as_ref().into_iter().flat_map(|i| i.interactive_pois.iter()).enumerate()
                .zip(map.interactive_pois_nearby.iter().map(|n| *n).chain(iter::repeat(false)))
                .map(move |poi| (pack_info, map, poi))
                .filter_map(|(pack_info, map, ((ipoii, ipoi), nearby))| {
                    let pack_info = pack_info.as_ref()?;
                    let ipoi_path = map_path.rel(ipoii);
                    let lpath = ipoi.loaded_index();
                    let lidx = lpath.path as usize;
                    let map_info = map_info.as_ref()?;
                    let poi_path = map_info.info.pois().nth(lidx)?;
                    let guid = Self::lpoi_get_guid(map_info, lpath.pivot(ipoi_path.root));
                    let marker_path = poi_path.map_path(MarkerIndex::with_poi)
                        .pivot(map_path.root);
                    let hidden = Self::lpoi_get_hidden(map, marker_path, guid.as_ref());
                    let filtered = Self::lpoi_get_filtered(map, ipoii, map_path.root.rel(lidx), engine);
                    Some((
                        ipoi_path,
                        ipoi,
                        (nearby, hidden, filtered),
                        guid,
                        poi_path,
                        (pack_info, map_info, map),
                    ))
                })
        );
        {
            let title_id = "pois-nearby";
            let _id = ui.push_id(title_id);
            let table_token = self.poi_table_start(ui, title_id);
            if let Some(..) = &table_token {
                let ipois_nearby = ipois.clone()
                        .filter(|(_path, _ipoi, (nearby, ..), ..)| *nearby);
                for (ipoi_path, ipoi, nearby, guid, poi_path, (pack_info, map_info, map)) in ipois_nearby {
                    self.draw_poi_row(ui, pack_info, map_info, map, poi_path, ipoi_path, ipoi, guid, nearby);
                    ui.table_next_column();
                }
            }
        }
        {
            let title_id = "pois-map";
            let _id = ui.push_id(title_id);
            let table_token = self.poi_table_start(ui, title_id);
            if let Some(..) = &table_token {
                let ipois_map = ipois.clone().filter(|&(_path, _ipoi, (nearby, _h, _f), ..)| !nearby);

                let ipois_map_filters = [
                    (false, false),
                    (true, false),
                    (false, true)
                ];
                for (i, hf) in ipois_map_filters.into_iter().enumerate() {
                    if i > 0 {
                        ui.separator();
                        ui.table_next_column();
                        ui.table_next_column();
                    }
                    let ipois_map = ipois_map.clone().filter(|&(_, _, (_, hidden, filtered), ..)| (hidden, filtered) == hf);
                    for (ipoi_path, ipoi, nearby, guid, poi_path, (pack_info, map_info, map)) in ipois_map {
                        self.draw_poi_row(ui, pack_info, map_info, map, poi_path, ipoi_path, ipoi, guid, nearby);
                        ui.table_next_column();
                    }
                }
            }
        }
    }
    pub fn draw_pois_header(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: &mut Option<&mut Engine>,
    ) {
        use std::collections::BTreeMap;
        use crate::controller::pathing::visible::VisibilityFlags;
        use glam::Vec4;
        use glamour::{Box3, Vector3};
        use crate::space::pack::poi::ActivePoi;
        use std::path::Path;

        let Some(Some(map_id)) = Controller::with_sender(|s| s.gameplay.as_ref().and_then(|g|
            g.borrow().gameplay_map()
        )) else { return };
        let Some(engine) = engine else { return };

        let mut selected = None;
        ui.popup("poi-create", || {
            let active_packs = self.pack_loader_data.as_ref()
                .map(|packs| packs.borrow().iter().enumerate()
                    .filter_map(|(i, active)| active.upgrade().map(|a| (i, a)))
                    .collect::<Vec<_>>()
                ).unwrap_or(Vec::new());
            for (i, active) in active_packs {
                let pack_path: PackPath = PackPath::with_path(i as u16);
                let map_path = pack_path.rel(map_id);
                let Some(map_info) = self.pack_info.as_ref().and_then(|i| i.borrow().map_info.get(&map_path).cloned()) else { continue };
                let textures = map_info.info.pois().enumerate()
                    .filter_map(|(i, path)| active.pack.pois.get(path.path as usize)
                        .and_then(|poi| poi.attributes.icon_file.as_ref())
                        .map(|texture| (texture, (pack_path.rel(path), i)))
                    )
                    //.chain(active.pack.trails.iter().filter_map(|trail| trail.attributes.texture.as_ref()))
                    .collect::<BTreeMap<_, _>>();
                for (texture, (path, lidx)) in textures {
                    let name = Path::new(&texture[..]);
                    let name = name.file_stem().unwrap_or(name.as_os_str())
                        .to_str()
                        .unwrap_or(&texture[..]);
                    if Selectable::new(name).build(ui) {
                        selected = Some((path, lidx, texture.clone()));
                    }
                }
            }
        });
        if ui.small_button("create") {
            ui.open_popup("poi-create");
        }
        if let Some((path, lidx, _texture)) = selected {
            let Some(pack) = engine.packs.loaded_packs.get_mut(path.root.path as usize) else { return };
            let Some((playerpos, ..)) = machine.get_player_pos() else { return };
            let mut poi = ActivePoi::empty();
            poi.icon = pack.active_pois.get(lidx).and_then(|poi| poi.icon.clone());
            poi.position = playerpos;
            poi.bounds = Box3::new(playerpos - Vector3::splat(1.0), playerpos + Vector3::splat(1.0));
            poi.visibility = VisibilityFlags::all();
            poi.tint = Vec4::ONE;
            poi.opacity = 1.0;
            poi.scale = 1.0;
            poi.scale_map = 20.0;
            pack.active_pois.push(poi);
            engine.packs.rebuild_active(&engine.render_backend.device);
        }
    }

    pub fn poi_table_start<'u>(
        &mut self,
        ui: &Ui<'u>,
        title_id: &str,
    ) -> Option<TableToken<'u>> {
        let table_flags =
            TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
        let table_token = with_i18n!("category", |header_cat| with_i18n!(title_id, |header_title|
            ui.begin_table_header_with_flags("ipois", [
                TableColumnSetup {
                    name: &header_title,
                    flags: TableColumnFlags::WIDTH_STRETCH,
                    init_width_or_weight: 0.0,
                    user_id: Id::Str("name"),
                },
                TableColumnSetup {
                    name: &header_cat,
                    flags: TableColumnFlags::WIDTH_FIXED,
                    init_width_or_weight: 0.0,
                    user_id: Id::Str("cat"),
                },
            ],
            table_flags)
        ));
        ui.table_next_column();
        table_token
    }

    pub(super) fn draw_poi_name(
        &mut self,
        ui: &Ui,
        pack_info: &PackInfo,
        path: PoiPath,
        ipoi_path: Locator<PackMapPath, usize>,
        ipoi: &InteractivePoi,
        guid: Option<Guid>,
        nearby: (bool, bool, bool),
        display_name: &str,
    ) -> Option<InteractionEvent> {
        let (_nearby, _hidden, filtered) = nearby;
        let marker_path = path.map_path(MarkerIndex::with_poi)
            .pivot(ipoi_path.root.root);
        let display_name_storage;
        let display_name = match display_name.is_empty() {
            false => Some(display_name),
            true => Self::marker_display_name(&self.pack_loader_data, &mut self.category_names, pack_info, marker_path),
        };
        let display_name = match display_name {
            Some(name) => name,
            None => {
                if let Some(guid) = guid {
                    display_name_storage = guid.to_string();
                } else {
                    display_name_storage = format!("#{}", path.path);
                }
                &display_name_storage[..]
            },
        };

        let mut action = None;
        let loaded_path = ipoi.loaded_index()
            .pivot(ipoi_path.root);
        let interactive_path = ipoi_path.unscope().map_path(|i| i as PoiIndex);
        let mut visible_title = display_name;
        let wrapped = match display_name {
            n if n.len() <= 48 =>
                false,
            n if ui.calc_text_size(n)[0] > ui.content_region_avail()[0] * 0.8 =>
                true,
            _ => false,
        };
        if wrapped {
            //ui.text_wrapped(display_name);

            Self::draw_title_text_truncate(ui, display_name);
            visible_title = Self::NAME_TEMPLATE;
        } else {
            ui.text(display_name);
        }
        let hover = ui.is_item_hovered();
        let mut same_line = wrapped;
        let mut same_line = || {
            let thresh = match same_line {
                true => 0.92,
                false => 0.97,
            };
            let cramped = ui.content_region_avail()[0] < ui.content_region_max()[0] * thresh;
            if cramped {
                ui.text(" ");
                same_line = false;
            } else {
                ui.same_line();
            }
        };

        if filtered {
            same_line();
            with_i18n!("disabled", |msg| ui.text_disabled(&msg));
        }

        if let Some(r) = &ipoi.reset {
            same_line();
            if ui.small_button("reset") {
                PathingEvent::GuidReset(r.guid.iter().cloned().collect()).try_send();
            }
        }
        for (i, showhide) in ipoi.show_hide().enumerate() {
            same_line();
            if ui.small_button(showhide.action.to_string()) {
                let cat_path = showhide.category().pivot(ipoi_path.root.root);
                PathingEvent::CategorySetToggle(cat_path, showhide.action.tristate()).try_send();
            }
        }
        if let Some(..) = &ipoi.behaviour {
            same_line();
            if ui.small_button("dismiss") {
                log::debug!("TODO: dismiss");
                //PathingEvent::DismissMarker(poi_path, std::time::Duration::from_secs(5)).try_send();
                action = Some(InteractionEvent::Interact {
                    action: InteractionEventAction::Manual(TriggerKind::BEHAVIOUR),
                    path,
                    loaded_path,
                    interactive_path,
                });
            }
        }
        if let Some(copy) = &ipoi.copy {
            same_line();
            if ui.small_button("copy") {
                action = Some(InteractionEvent::Interact {
                    action: InteractionEventAction::Manual(TriggerKind::COPY),
                    path,
                    loaded_path,
                    interactive_path,
                });
            }
            if ui.is_item_hovered() {
                Self::draw_tooltip(ui, display_name, || {
                    let copy_value = &copy.value.0[..];
                    let copy_message = copy.message.as_ref()
                        .map(|m| &m.0[..])
                        .unwrap_or("");
                    Self::draw_tooltip_copyable(ui, display_name, copy_value, copy_message);
                });
            }
        }
        if let Some(info) = &ipoi.info {
            same_line();
            if ui.small_button("read") {
                action = Some(InteractionEvent::Interact {
                    action: InteractionEventAction::Manual(TriggerKind::INFO),
                    path,
                    loaded_path,
                    interactive_path,
                });
            }
            if ui.is_item_hovered() {
                Self::draw_tooltip(ui, display_name, || {
                    ui.text_wrapped(&info.message[..]);
                });
            }
        }
        if let Some(..) = &ipoi.bounce {
            same_line();
            if ui.small_button("anim") {
                action = Some(InteractionEvent::Interact {
                    action: InteractionEventAction::Manual(TriggerKind::BOUNCE),
                    path,
                    loaded_path,
                    interactive_path,
                });
            }
        }
        if let Some(..) = &ipoi.script {
            same_line();
            if ui.small_button("script") {
                action = Some(InteractionEvent::Interact {
                    action: InteractionEventAction::Manual(TriggerKind::SCRIPT),
                    path,
                    loaded_path,
                    interactive_path,
                });
            }
        }

        let pack_loader_data = &self.pack_loader_data;
        let tip = &*self.category_tips.entry(marker_path)
            .or_insert_with(|| {
                let packs = pack_loader_data.as_ref().map(|d| d.borrow());
                let packs = packs.as_ref().map(|d| &**d);
                Self::get_marker_tip(packs, pack_info, marker_path)
            });
        match tip.as_ref().map(|(title, _)| &title[..]) {
            Some(title) if !title.is_empty() && !display_name.starts_with(title) => {
                ui.text_wrapped(title);
                visible_title = title;
            },
            _ => {
                #[cfg(todo)]
                let info_text = Self::marker_info(pack_loader_data, &mut self.cache_info, pack_info, marker_path);
                let info_text = ipoi.info.as_ref().map(|i| &i.message.0[..]);
                if let Some(info_text) = info_text {
                    Self::draw_title_text_truncate(ui, info_text);
                }
            },
        }
        if hover {
            Self::draw_tooltip(ui, display_name, || {
                if wrapped {
                    ui.text_wrapped(display_name);
                }

                if let Some((title, desc)) = tip {
                    Self::draw_tooltip_category(ui, visible_title, title, desc);
                }

                #[cfg(todo)]
                let info_text = Self::marker_info(pack_loader_data, &mut self.cache_info, pack_info, marker_path);
                let info_text = ipoi.info.as_ref().map(|i| &i.message.0[..]);
                if let Some(info_text) = info_text {
                    ui.text_wrapped(info_text);
                }

                if let Some(guid) = guid {
                    ui.text(guid.to_string());
                }
            });
        }

        action
    }
    pub(super) fn draw_poi_row(
        &mut self,
        ui: &Ui,
        pack_info: &PackInfo,
        map_info: &SharedMapPackLoaded,
        map: &SharedMapPackState,
        poi_path: PoiPath,
        ipoi_path: Locator<PackMapPath, usize>,
        ipoi: &InteractivePoi,
        guid: Option<Guid>,
        nearby: (bool, bool, bool),
    ) {
        let (_nearby, hidden, _filtered) = nearby;
        let pack_path = ipoi_path.root.root;
        let marker_path = pack_path.rel(MarkerIndex::with_poi(poi_path.path));
        let _id = ui.push_id(Id::Int(poi_path.path as i32 ^ (pack_path.path as i32) << 28));
        let action = self.draw_poi_name(ui, pack_info, poi_path, ipoi_path, ipoi, guid, nearby, Default::default());

        ui.table_next_column();

        let action = match hidden {
            true => ui.small_button("unhide")
                .then(|| Ok(match guid {
                    Some(guid) =>
                        PathingEvent::GuidReset(vec![guid]),
                    None =>
                        PathingEvent::ResetMarker(marker_path)
                })),
            false => ui.small_button("trigger")
                .then(|| Err(InteractionEvent::Interact {
                    action: InteractionEventAction::Trigger,
                    path: poi_path,
                    loaded_path: ipoi.loaded_index().pivot(ipoi_path.root),
                    interactive_path: ipoi_path.unscope().map_path(|i| i as PoiIndex),
                })).or(action.map(Err)),
        };
        match action {
            Some(Ok(action)) =>
                action.try_send(),
            Some(Err(action)) =>
                if let Some(pi) = &self.pack_info {
                    let _ = pi.borrow().interactions.send(action);
                },
            None => (),
        }
    }

    pub(super) fn lpoi_get_guid(map_info: &SharedMapPackLoaded, loaded_path: PoiPath<PackMapPath>) -> Option<Guid> {
        let guid_idx = map_info.info.poi_guid_mask()
            .take(loaded_path.path as usize)
            .filter(|&has| has)
            .count();
        map_info.poi_guids.get(guid_idx).cloned()
    }
    pub(super) fn lpoi_get_hidden(map: &SharedMapPackState, path: MarkerPath<PackPath>, guid: Option<&Guid>) -> bool {
        let hidden_guid = guid
            .map(MarkerId::from_guid_ref)
            .map(|id| map.hidden_markers.contains(id))
            .unwrap_or(false);
        hidden_guid || map.hidden_markers.contains(&MarkerId::from(path))
    }
    pub(super) fn lpoi_get_filtered(map: &SharedMapPackState, ipoi_idx: usize, lpoi_path: Locator<PackPath, usize>, engine: Option<&Engine>) -> bool {
        #[cfg(deleteme)]
        {
            let pack_path = lpoi_path.root;
            let spacepoi = engine
                .and_then(|e| e.packs.loaded_packs.get(pack_path.path as usize))
                .and_then(|p| p.active_pois.get(lpoi_path.path as usize));
            let Some(spacepoi) = spacepoi else {
                // idunno
                return false
            };

            !spacepoi.visibility.is_visible()
        }
        map.interactive_poi_pois.get(ipoi_idx)
            .map(|poi| !poi.visibility.is_visible())
            .unwrap_or(false)
    }
}
