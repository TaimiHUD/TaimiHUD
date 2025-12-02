use std::iter;

use glamour::Point3;
use taimi_pack::attributes::keys::{self, Guid};

use {
    super::PathingWindowState,
    crate::{
        controller::pathing::{registry::PoiPath, visible::interactive::BehaviourConfig, PathingEvent}, exports::runtime::{imgui::{
            TableToken, TreeNode, Selectable, MouseButton, Condition,
            Id, TableColumnFlags, TableColumnSetup, TableFlags, Ui,
        }, Locator}, with_i18n, render::machine::RenderMachine, space::engine::Engine, Controller,
    },
};
use crate::{
    controller::pathing::{
        registry::{CategoryIndex, CategoryPath, MarkerId, MarkerPath, PackInfo, PackMapPath, PackPath, PoiIndex},
        visible::{InteractionEvent, InteractionEventAction, InteractivePoi, VisibilityFlags},
        shared::{SharedMapPackLoaded, SharedMapPackState},
    },
    space::{render_list::RenderId, DrawSpace},
};
use crate::controller::pathing::registry::MarkerIndex;
use crate::settings::pathing::TriggerKind;

impl PathingWindowState {
    pub fn draw_pois(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: Option<&mut anyhow::Result<Engine>>,
    ) {
        self.act_selected_poi_open = false;
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
        let map_packs = {
            let Some(pack_info) = self.pack_info.as_ref().map(|i| i.borrow()) else { return };
            pack_info.pack_info()
                .filter_map(|(path, info)| {
                    let map_path = path.rel(map_id);
                    let map_info = pack_info.map_info.get(&map_path)?;
                    let map = pack_info.map_state.get(&map_path)?;
                    Some((map_path, info.clone(), map_info.clone(), map.clone()))
                }).collect::<Vec<_>>()
        };
        let ipois = map_packs.iter()
            .map(|(map_path, _pack_info, map_info, map)|
                map_info.interactive_pois.iter().enumerate()
                .zip(map.interactive_pois_nearby.iter().map(|n| *n).chain(iter::repeat(false)))
                .zip(map.interactive_poi_pois.iter().map(Some).chain(iter::repeat(None)))
                .map(move |(((ipoii, ipoi), nearby), lpoi)| {
                    let loaded_index = ipoi.loaded_index().path;
                    let (path, guid) = map_info.poi_guids().nth(loaded_index as usize).unzip();
                    let mut rpoi = RenderInteractivePoi::new(
                        *map_path,
                        CategoryPath::with_path(lpoi.map(|l| l.category).unwrap_or(CategoryIndex::MAX)),
                        path.unwrap_or(PoiPath::with_path(PoiIndex::MAX)),
                        loaded_index,
                        ipoii as PoiIndex,
                        guid.flatten().cloned(),
                        lpoi.map(|l| l.visibility).unwrap_or(VisibilityFlags::TOGGLE),
                        lpoi.map(|l| l.position).unwrap_or(Point3::INFINITY),
                        (
                            nearby,
                            false
                        ),
                    );
                    rpoi.hidden = Self::lpoi_get_hidden(map, rpoi.marker_path(), rpoi.guid.as_ref());
                    if let Some(cat) = map.categories.get(rpoi.category_path.path as usize) {
                        rpoi.category_visibility = cat.visibility;
                    }
                    rpoi
                }).collect::<Vec<_>>()
            ).collect::<Vec<_>>();
        {
            ui.same_line();
            if ui.small_button("DEBUG") {
                let mut hidden_count = 0;
                log::debug!("HIDDEN POIs:");
                for (_path, _pack_info, _map_info, map) in &map_packs {
                    for hidden in map.hidden_markers.iter() {
                        log::debug!("- {:?} {:?}", hidden.variant(), hidden);
                        hidden_count += 1;
                    }
                }
                log::debug!("({} total)", hidden_count);
            }
        }
        {
            let title_id = "pois-nearby";
            let _id = ui.push_id(title_id);
            let mut table_token = None;
            'nearby_packs: for (ipois, (_path, pack_info, map_info, map)) in ipois.iter().zip(&map_packs) {
                let ipois = ipois.iter()
                    .zip(map_info.interactive_pois.iter())
                    .filter(|(rpoi, _)| rpoi.nearby);
                for (rpoi, ipoi) in ipois {
                    let table_token = table_token.get_or_insert_with(|| self.poi_table_start(ui, title_id));
                    if table_token.is_none() {
                        break 'nearby_packs;
                    }
                    self.draw_poi_row(ui, pack_info, map_info, map, rpoi, ipoi);
                    ui.table_next_column();
                }
            }
        }
        {
            let title_id = "pois-map";
            let _id = ui.push_id(title_id);
            let table_token = self.poi_table_start(ui, title_id);
            if let Some(..) = &table_token {
                let mut ipois = ipois.into_iter()
                    .zip(&map_packs)
                    .flat_map(|(ipois, map_packs @ (_, _, map_info, ..))| ipois.into_iter()
                        .zip(map_info.interactive_pois.iter())
                        .filter(|(rpoi, ..)| !rpoi.nearby)
                        .map(move |(rpoi, ipoi)| (rpoi, ipoi, map_packs))
                    ).collect::<Vec<_>>();
                if let Some(engine) = &engine {
                    ipois.sort_unstable_by_key(|(rpoi, ..)| {
                        /// 1000000/0x80000 = ~2m^3 buckets
                        const GROUP_FACTOR: i32 = 0x80000i32;
                        const POS_FACTOR: f32 = 256.0;
                        let prio = (
                            #[cfg(todo = "unnecessary")]
                            rpoi.nearby,
                            rpoi.is_disabled(),
                            !rpoi.visibility.is_visible(),
                        );
                        let dist = match prio {
                            #[cfg(todo = "unnecessary")]
                            (false, false) => {
                                let id = RenderId::Poi { pack_idx: rpoi.pack_path().path, poi_idx: rpoi.loaded_index };
                                engine.packs.render_list.find_entity(&id)
                                    .and_then(|i| engine.packs.render_list.find_dist(i))
                                    .map(|(_i, dist)| dist)
                            },
                            // we've already rendered these, and they're all close anyway
                            #[cfg(todo = "unnecessary")]
                            (true, ..) => None,
                            _ => None,
                        };
                        // TODO: offset by ppos if dist is none
                        let mut pos = rpoi.position / POS_FACTOR;
                        if dist.is_none() {
                            if let Some((playerpos, _)) = machine.get_player_pos() {
                                pos = Point3::splat(pos.distance_squared(playerpos / POS_FACTOR));
                            }
                        }
                        let dist = dist.unwrap_or(0x38000000) / GROUP_FACTOR;
                        let cat = rpoi.category_path();
                        (prio.0, (dist, pos.x as i8, pos.y as i8, pos.z as i8), cat)
                    })
                }
                let mut disabled_token = None;
                for (rpoi, ipoi, (_map_path, pack_info, map_info, map)) in ipois.iter() {
                    if disabled_token.is_none() && rpoi.is_disabled() {
                        //ui.separator();
                        let disabled_token = disabled_token.get_or_insert_with(|| with_i18n!("disabled", |label| TreeNode::new("pois-disabled")
                            .opened(false, Condition::Once)
                            .framed(true)
                            .tree_push_on_open(false)
                            .frame_padding(true)
                            .label::<&str, _>(&label)
                            .push(ui)
                        ));
                        ui.table_next_column(); ui.table_next_column();
                        if disabled_token.is_none() {
                            break
                        }
                    }
                    self.draw_poi_row(ui, pack_info, map_info, map, rpoi, ipoi);
                    ui.table_next_column();
                }
                drop(disabled_token);
            }
        }
        if self.act_selected_poi.is_some() {
            ui.popup("poi-context", || {
                self.menu_ipoi(ui, machine, engine)
            })
        }
        if self.act_selected_poi_open {
            self.act_selected_poi_delay = None;
            ui.open_popup("poi-context");
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
    pub fn menu_ipoi(
        &mut self,
        ui: &Ui,
        _machine: &mut RenderMachine,
        _engine: Option<&Engine>,
    ) {
        let Some(rpoi) = &self.act_selected_poi else { return };
        let mut action_untrigger = false;
        let mut action_trigger = None;
        match rpoi.hidden {
            false => if with_i18n!("trigger-trigger", |label| Selectable::new(label).build(ui)) {
                let _ = action_trigger.get_or_insert(InteractionEventAction::Trigger);
            },
            true =>
                action_untrigger = with_i18n!("trigger-untrigger", |label| Selectable::new(label).build(ui)),
        }
        #[cfg(todo)]
        if with_i18n!("trigger-behaviour", |label| Selectable::new(label).build(ui)) {
            action_trigger.get_or_insert(InteractionEventAction::Manual(TriggerKind::ALL));
        }

        ui.separator();
        let action_dismiss_open = with_i18n!("trigger-behaviour", |label| Selectable::new(label)
            .close_popups(false)
            .build(ui));
        match action_dismiss_open.then(|| self.act_selected_poi_delay.take()) {
            Some(Some(..)) => {
                let _ = action_trigger.get_or_insert(InteractionEventAction::Manual(TriggerKind::DISMISS));
                ui.close_current_popup();
            }
            Some(None) =>
                self.act_selected_poi_delay = Some(1.0),
            None => (),
        }
        if let Some(delay) = &mut self.act_selected_poi_delay {
            let mut action_dismiss = None;
            let behaviours = keys::Behaviour::ALL.iter().skip(1);
            ui.indent();
            for &behaviour in behaviours {
                let label = format!("dismiss-behaviour-{}", behaviour.value());
                let act = with_i18n!(&label, |label| Selectable::new(label).build(ui));
                match behaviour {
                    keys::Behaviour::Taco(keys::TacoBehaviour::ResetDelay) => {
                        ui.indent();
                        let _ = ui.input_float("hours", delay)
                            .build();
                        ui.unindent();
                    },
                    _ => (),
                }
                if act {
                    action_dismiss = Some(behaviour);
                }
            }
            ui.unindent();
            if let Some(mode) = action_dismiss {
                let mut config = BehaviourConfig::new(mode);
                config.reset_delay = self.act_selected_poi_delay.map(|delay|
                    (delay * 3600.0).into()
                ).unwrap_or_default();
                let _ = action_trigger.get_or_insert(InteractionEventAction::Dismiss(config));
            }
        }

        if let Some(action) = action_trigger {
            let action = rpoi.action_trigger(action);
            if let Some(pi) = &self.pack_info {
                let _ = pi.borrow().interactions.send(action);
            }
        } else if action_untrigger {
            rpoi.action_untrigger().try_send();
        }
    }

    pub fn poi_table_start<'u>(
        &mut self,
        ui: &Ui<'u>,
        title_id: &str,
    ) -> Option<TableToken<'u>> {
        let table_flags =
            TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
        let table_token = with_i18n!("toggle", |header_cat| with_i18n!(title_id, |header_title|
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
                    user_id: Id::Str("toggle"),
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
        rpoi: &RenderInteractivePoi,
        ipoi: &InteractivePoi,
        display_name: &str,
    ) -> Option<InteractionEvent> {
        let path = rpoi.path;
        let marker_path = rpoi.marker_path();
        let display_name_storage;
        let display_name = match display_name.is_empty() {
            false => Some(display_name),
            true => Self::marker_display_name(&self.pack_loader_data, &mut self.category_names, pack_info, marker_path),
        };
        let display_name = match display_name {
            Some(name) => name,
            None => {
                if let Some(guid) = &rpoi.guid {
                    display_name_storage = guid.to_string();
                } else {
                    display_name_storage = format!("#{}", path.path);
                }
                &display_name_storage[..]
            },
        };

        let mut action = None;
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
        if ui.is_item_clicked_with_button(MouseButton::Right) {
            self.act_selected_poi = None;
            self.act_selected_poi_open = true;
        }
        let mut same_line = wrapped;
        let mut same_line = || {
            let thresh = match same_line {
                true => 0.90,
                false => 0.935,
            };
            let width = ui.content_region_max()[0];
            let used = ui.item_rect_max()[0] - ui.window_pos()[0];
            let cramped = used / width > thresh;
            if cramped {
                ui.text(" ");
                same_line = false;
            }
            ui.same_line();
        };

        if rpoi.is_disabled() {
            same_line();
            with_i18n!("disabled", |msg| ui.text_disabled(&msg));
        }

        if let Some(r) = &ipoi.reset {
            same_line();
            if ui.small_button("reset") {
                PathingEvent::GuidReset(r.guid.iter().cloned().collect()).try_send();
            }
        }
        for showhide in ipoi.show_hide() {
            same_line();
            if ui.small_button(showhide.action.to_string()) {
                let cat_path = showhide.category().pivot(rpoi.pack_path());
                PathingEvent::CategorySetToggle(cat_path, showhide.action.tristate()).try_send();
            }
        }
        if let Some(dismiss) = &ipoi.behaviour {
            same_line();
            if with_i18n!("trigger-behaviour", |label| ui.small_button(&label)) {
                //PathingEvent::DismissMarker(poi_path, std::time::Duration::from_secs(5)).try_send();
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::BEHAVIOUR)));
            }
            if ui.is_item_hovered() {
                ui.tooltip(|| {
                    // TODO: idk how to do a select case is our fluent too old?
                    with_i18n!(&format!("dismiss-behaviour-{}", dismiss.mode.value()), |label|
                        ui.text(label)
                    );
                });
            }
        }
        if let Some(copy) = &ipoi.copy {
            same_line();
            if with_i18n!("trigger-copy", |label| ui.small_button(&label)) {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::COPY)));
            }
            if ui.is_item_hovered() {
                Self::draw_tooltip(ui, display_name, || {
                    let copy_value = &copy.value.0[..];
                    let copy_message = copy.message.as_ref()
                        .map(|m| &m.0[..])
                        .unwrap_or("");
                    Self::draw_tooltip_copyable(ui, visible_title, copy_value, copy_message);
                });
            }
        }
        if let Some(info) = &ipoi.info {
            same_line();
            if ui.small_button("read") {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::INFO)));
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
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::BOUNCE)));
            }
        }
        if let Some(..) = &ipoi.script {
            same_line();
            if ui.small_button("script") {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::SCRIPT)));
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
            let display_name = display_name.to_owned();
            let visible_title = visible_title.to_owned();
            let display_name = &display_name[..];
            let category_names = &self.category_names;
            Self::draw_tooltip(ui, &display_name, || {
                let mut visible_title = &visible_title[..];
                if wrapped {
                    ui.text_wrapped(display_name);
                    visible_title = &display_name;
                }

                let mut cat_name = category_names.get(&rpoi.marker_path());

                if let Some((title, desc)) = tip {
                    let mut visible_title = visible_title;

                    let mut cat_redundant = false;
                    if let Some(Some(cat_name)) = &cat_name {
                        if cat_name.starts_with(&title[..]) {
                            visible_title = &cat_name[..];
                            cat_redundant = true;
                        } else if title.starts_with(&cat_name[..]) {
                            cat_redundant = true;
                        }
                    }

                    Self::draw_tooltip_category(ui, visible_title, title, desc);

                    if cat_redundant {
                        let _ = cat_name.take();
                    }
                }

                if let Some(Some(cat_name)) = &cat_name {
                    if !display_name.starts_with(&cat_name[..]) {
                        ui.text_wrapped(cat_name);
                    }
                }

                #[cfg(todo)]
                let info_text = Self::marker_info(pack_loader_data, &mut self.cache_info, pack_info, marker_path);
                let info_text = ipoi.info.as_ref().map(|i| &i.message.0[..]);
                if let Some(info_text) = info_text {
                    ui.text_wrapped(info_text);
                }

                if let Some(copy) = ipoi.copy.as_ref() {
                    let copy_value = &copy.value.0[..];
                    let copy_message = copy.message.as_ref()
                        .map(|m| &m.0[..])
                        .unwrap_or("");
                    Self::draw_tooltip_copyable(ui, display_name, copy_value, copy_message);
                }

                if let Some(guid) = &rpoi.guid {
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
        rpoi: &RenderInteractivePoi,
        ipoi: &InteractivePoi,
    ) {
        let _id = ui.push_id(Id::Int(rpoi.path.path as i32 ^ (rpoi.pack_path().path as i32) << 28));
        let action = self.draw_poi_name(ui, pack_info, rpoi, ipoi, Default::default())
            .map(Err);

        ui.table_next_column();

        let action = action.or(match rpoi.hidden {
            true => with_i18n!("trigger-untrigger", |label| ui.small_button(&label))
                .then(|| Ok(rpoi.action_untrigger())),
            false => with_i18n!("trigger-trigger", |label| ui.small_button(&label))
                .then(|| Err(rpoi.action_trigger(InteractionEventAction::Trigger))),
        });
        if ui.is_item_clicked_with_button(MouseButton::Right) {
            self.act_selected_poi = None;
            self.act_selected_poi_open = true;
        }
        match action {
            Some(Ok(action)) =>
                action.try_send(),
            Some(Err(action)) =>
                if let Some(pi) = &self.pack_info {
                    let _ = pi.borrow().interactions.send(action);
                },
            None => (),
        }

        if self.act_selected_poi_open {
            let _ = self.act_selected_poi.get_or_insert_with(|| rpoi.clone());
        }
    }

    #[cfg(todo)]
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
    #[cfg(todo)]
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

#[derive(Debug, Clone)]
pub(super) struct RenderInteractivePoi {
    pub path: PoiPath,
    pub category_path: CategoryPath,
    pub map_path: PackMapPath,
    pub loaded_index: PoiIndex,
    pub ipoi_index: PoiIndex,
    pub guid: Option<Guid>,
    pub visibility: VisibilityFlags,
    pub category_visibility: VisibilityFlags,
    pub position: Point3<DrawSpace>,
    pub nearby: bool,
    pub hidden: bool,
}
impl RenderInteractivePoi {
    pub fn new(
        map_path: PackMapPath,
        category_path: CategoryPath,
        path: PoiPath,
        loaded_index: PoiIndex,
        ipoi_index: PoiIndex,
        guid: Option<Guid>,
        visibility: VisibilityFlags,
        position: Point3<DrawSpace>,
        (nearby, hidden): (bool, bool),
    ) -> Self {
        Self {
            map_path,
            category_path,
            path,
            loaded_index,
            ipoi_index,
            guid,
            category_visibility: visibility.default_toggles(),
            visibility,
            position,
            nearby,
            hidden,
        }
    }

    pub fn path(&self) -> PoiPath<PackPath> {
        self.path.pivot(self.pack_path())
    }
    pub fn pack_path(&self) -> PackPath {
        self.map_path.root
    }
    pub fn marker_path(&self) -> MarkerPath<PackPath> {
        self.path().map_path(MarkerIndex::with_poi)
    }
    pub fn loaded_path(&self) -> Locator<PackMapPath, PoiIndex> {
        self.map_path.rel(self.loaded_index)
    }
    pub fn category_path(&self) -> CategoryPath<PackPath> {
        self.category_path.pivot(self.pack_path())
    }
    pub fn ipoi_path(&self) -> Locator<PackMapPath, usize> {
        self.map_path.rel(self.ipoi_index as usize)
    }

    pub fn is_disabled(&self) -> bool {
        !self.category_visibility.is_visible()
            || (!self.visibility.is_visible() && !self.hidden)
    }

    pub fn action_trigger(&self, action: InteractionEventAction) -> InteractionEvent {
        InteractionEvent::Interact {
            action,
            path: self.path,
            loaded_path: self.loaded_path(),
            interactive_path: Locator::with_path(self.ipoi_index),
        }
    }
    pub fn action_untrigger(&self) -> PathingEvent {
        match self.guid.clone() {
            Some(guid) =>
                PathingEvent::GuidReset(vec![guid]),
            None =>
                PathingEvent::ResetMarker(self.marker_path())
        }
    }
}
