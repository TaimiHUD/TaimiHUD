use {
    crate::{
        controller::pathing::{
            registry::{
                PackInfo, PackMapPath, PackPath,
                PackRegistryNs, PackIndex,
                LoadedPoiPath,
                LoadedMarkerPath, LoadedMarkerNs,
                PoiMapPath,
            },
            info::LoadedPoiInfo,
            shared::{interact::{SharedNearbyMarkers, NearbyMarkers, SpaceInteraction, SharedTriggerBvh, SharedInteractEntities, empty_trigger_bvh, TRIGGER_DIMENSION}, SpacePackShared, SharedMapPackLoaded, SharedMapPackState, PathingShared, SharedGameplayMap, LoadedPoiShared},
            state::{
                interactive::{BehaviourConfig, InteractionEvent, InteractionEventAction, InteractivePoi},
                LoadedPoi,
            },
            PathingEvent,
        },
        exports::runtime::{self as rt, imgui::{
            TableToken, TreeNode, TreeNodeFlags, Selectable, MouseButton, Condition,
            Id as UiId, TableColumnFlags, TableColumnSetup, TableFlags, Ui,
            TableSortDirection,
        }},
        with_i18n,
        settings::{state::ui::interact::{InteractFilterFlags, InteractSortFlags}, Settings},
        render::machine::RenderMachine,
        render::PathingWindowState,
        space::engine::Engine,
        space::DrawSpace,
        space::pack::PackRender,
        Controller,
        fl,
    },
        super::{PackVisibility, PackElement,
    CategoryInfo,
    CategoryAction,
    CategoryActionSlot,
        },
    rustc_hash::FxBuildHasher,
    taimi_hoard::{
        str_opt_ref,
        loc::{indexed::IndexedList, Locator, LocationRef, LocationMut},
    },
    taimi_sync::arcs::ArcPtrCmp,
    taimi_meta::packs::{
        collections::PackSet,
        id::{MarkerId, MarkerPath, MarkerIndex},
        CategoryIndex, CategoryPath, PoiIndex, PoiPath,
        MapPath,
        VisibilityFlags,
    },
    taimi_pack::attributes::{
        keys::{self, Guid},
        AttrString,
        InteractionAttributes,
    },
    glamour::{Box2, Point2, Size2, Size3, Point3, Box3},
    bvh::bvh::Bvh,
    std::sync::Arc,
    std::mem,
    std::borrow::Cow,
    std::collections::BTreeSet,
    indexmap::{map::Entry as IndexEntry, IndexMap},
    taimi_hoard::iters::IterExt as _,
    bitflags::bitflags,
    taimi_meta::coords::{LocalSpace, vec_eq},
    taimi_sync::watched::{self, Watched, Watcher},
    taimi_meta::spatial::{box2aabb, box3aabb, MintConv, IRRELEVANT_MID},
    std::cell::{Cell, LazyCell},
    std::cmp,
};
use crate::settings::pathing::TriggerKind;

/// TODO: deleteme
#[derive(Debug, Clone)]
#[cfg(deleteme)]
pub(in super::super::super) struct RenderInteractivePoi {
    pub path: PoiPath,
    pub category_path: CategoryPath,
    pub map_path: PackMapPath,
    pub loaded_index: PoiIndex,
    pub guid: Option<Guid>,
    pub visibility: VisibilityFlags,
    pub category_visibility: VisibilityFlags,
    pub position: Point3<DrawSpace>,
    pub nearby: bool,
    pub hidden: bool,
}
#[cfg(deleteme)]
impl RenderInteractivePoi {
    pub fn new(
        map_path: PackMapPath,
        category_path: CategoryPath,
        path: PoiPath,
        loaded_index: PoiIndex,
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
    pub fn loaded_path(&self) -> PoiMapPath {
        self.map_path.rel(self.loaded_index)
    }
    pub fn category_path(&self) -> CategoryPath<PackPath> {
        self.category_path.pivot(self.pack_path())
    }

    pub fn is_disabled(&self) -> bool {
        !self.category_visibility.is_visible()
            || (!self.visibility.is_visible() && !self.hidden)
    }

    pub fn action_trigger(&self, action: InteractionEventAction) -> InteractionEvent {
        action_trigger(self.path, self.loaded_path(), action)
    }
    pub fn action_untrigger(&self) -> PathingEvent {
        match self.guid.clone() {
            Some(guid) =>
                PathingEvent::ResetMarkerIds(vec![MarkerId::with_uuid(guid.into())]),
            None =>
                PathingEvent::ResetMarkerPath(self.marker_path())
        }
    }

    pub fn draw_table_start<'u>(
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
                    user_id: UiId::Str("name"),
                },
                TableColumnSetup {
                    name: &header_cat,
                    flags: TableColumnFlags::WIDTH_FIXED,
                    init_width_or_weight: 0.0,
                    user_id: UiId::Str("toggle"),
                },
            ],
            table_flags)
        ));
        ui.table_next_column();
        table_token
    }

    pub fn draw(&self, ui: &Ui, interact: &InteractionAttributes, display_name: &str) -> bool {
        let mut draw = DrawInteractivePoi {
            ui,
            rpoi: self,
            interact,
            display_name,
            #[cfg(deleteme)]
            act_selected_poi: None,
            act_selected_poi_open: false
        };
        draw.draw_poi_row();
        draw.act_selected_poi_open
    }
}
/// TODO: deleteme
#[cfg(deleteme)]
pub(in super::super::super) struct DrawInteractivePoi<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub rpoi: &'a RenderInteractivePoi,
    pub interact: &'a InteractionAttributes,
    pub display_name: &'a str,
    pub act_selected_poi_open: bool,
    #[cfg(deleteme)]
    act_selected_poi: Option<PoiMapPath>,
}
#[cfg(deleteme)]
impl<'a, 'u> DrawInteractivePoi<'a, 'u> {
    /// TODO: SpaceInteraction::interest_for_marker if attrs exist
    fn interest(&self) -> TriggerKind {
        SpaceInteraction::interest_for(self.interact).0
    }

    pub(super) fn draw_poi_row(
        &mut self,
    ) {
        let Self { ui, rpoi, .. } = *self;
        let _id = ui.push_id(UiId::Int(rpoi.path.path as i32 ^ (rpoi.pack_path().path as i32) << 28));
        let action = self.draw_poi_name()
            .map(Err);

        ui.table_next_column();

        let action = action.or(match rpoi.hidden {
            true => with_i18n!("trigger-untrigger", |label| ui.small_button(&label))
                .then(|| Ok(rpoi.action_untrigger())),
            false => with_i18n!("trigger-trigger", |label| ui.small_button(&label))
                .then(|| Err(rpoi.action_trigger(InteractionEventAction::Trigger))),
        });
        if ui.is_item_clicked_with_button(MouseButton::Right) {
            #[cfg(deleteme)]
            {
            self.act_selected_poi = None;
            }
            self.act_selected_poi_open = true;
        }
        match action {
            Some(Ok(action)) =>
                action.try_send(),
            Some(Err(action)) => {
                Controller::with_sender(|s| if let Some(s) = &s.pathing {
                    let _ = s.shared.interact.events.send(action);
                });
            },
            None => (),
        }

        #[cfg(deleteme)]
        if self.act_selected_poi_open {
            let _ = self.act_selected_poi.get_or_insert_with(|| rpoi.loaded_path());
        }
    }
    pub(super) fn draw_poi_name(
        &mut self,
    ) -> Option<InteractionEvent> {
        let Self { ui, rpoi, display_name, .. } = *self;
        let interest = self.interest();
        let path = rpoi.path;
        let marker_path = rpoi.marker_path();
        let display_name_storage;
        let display_name = str_opt_ref(display_name);
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
            ui.text_wrapped(display_name);

            let TODO = ();
            #[cfg(todo)]
            {
                Self::draw_title_text_truncate(ui, display_name);
                visible_title = Self::NAME_TEMPLATE;
            }
        } else {
            ui.text(display_name);
        }
        let hover = ui.is_item_hovered();
        if ui.is_item_clicked_with_button(MouseButton::Right) {
            #[cfg(deleteme)]
            {
            self.act_selected_poi = None;
            }
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

        if interest.contains(TriggerKind::RESET) {
            same_line();
            if ui.small_button("reset") {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::RESET)));
                #[cfg(todo)]
                if let Some(r) = &ipoi.reset {
                    PathingEvent::GuidReset(r.guid.iter().cloned().collect()).try_send();
                }
            }
        }
        for (trigger, showhide_action) in interest.show_hide_actions() {
            same_line();
            if ui.small_button(showhide_action.to_string()) {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(trigger)));
                #[cfg(todo)] {
                let cat_path = showhide.category().pivot(rpoi.pack_path());
                PathingEvent::CategorySetToggle(cat_path, showhide.action.tristate()).try_send();
                }
            }
        }
        if interest.contains(TriggerKind::BEHAVIOUR) {
            same_line();
            if with_i18n!("trigger-behaviour", |label| ui.small_button(&label)) {
                //PathingEvent::DismissMarker(poi_path, std::time::Duration::from_secs(5)).try_send();
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::BEHAVIOUR)));
            }
            if ui.is_item_hovered() {
                ui.tooltip(|| {
                    let mode = self.interact.behaviour().map(|b| b.value())
                        .unwrap_or(keys::TacoBehaviour::ResetInstance.value());
                    // TODO: idk how to do a select case is our fluent too old?
                    with_i18n!(&format!("dismiss-behaviour-{}", mode), |label|
                        ui.text(label)
                    );
                });
            }
        }
        if interest.contains(TriggerKind::COPY) {
            same_line();
            if with_i18n!("trigger-copy", |label| ui.small_button(&label)) {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::COPY)));
            }
            if ui.is_item_hovered() {
                let TODO = ();
                #[cfg(todo)] {
                    if let Some(copy_value) = self.interact.copy_value() {
                        Self::draw_tooltip(ui, display_name, || {
                            let copy_message = self.interact.copy_message()
                                .unwrap_or("");
                            Self::draw_tooltip_copyable(ui, visible_title, copy_value, copy_message);
                        });
                    }
                }
            }
        }
        if interest.contains(TriggerKind::INFO) {
            same_line();
            if ui.small_button("read") {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::INFO)));
            }
            if ui.is_item_hovered() {
                if let Some(info) = self.interact.info() {
                    ui.tooltip_text(info);
                    #[cfg(todo)]
                    {
                        Self::draw_tooltip(ui, display_name, || {
                            ui.text_wrapped(&info.message[..]);
                        });
                    }
                }
            }
        }
        if interest.contains(TriggerKind::BOUNCE) {
            same_line();
            if ui.small_button("anim") {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::BOUNCE)));
            }
        }
        if interest.contains(TriggerKind::SCRIPT) {
            same_line();
            if ui.small_button("script") {
                action = Some(rpoi.action_trigger(InteractionEventAction::Manual(TriggerKind::SCRIPT)));
            }
        }

        #[cfg(todo)] {
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
        }
        #[cfg(todo)]
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
}
pub struct DrawMenuPoi<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub hidden: bool,
    pub guid: bool,
    pub act_trigger: Option<InteractionEventAction>,
    pub act_untrigger: bool,
    pub act_guid_copy: bool,
    pub act_cat_open: bool,
    pub act_selected_poi_delay: Option<f32>,
}
impl<'a, 'u> DrawMenuPoi<'a, 'u> {
    pub fn draw(
        &mut self,
    ) {
        let ui = self.ui;
        match self.hidden {
            false => if with_i18n!("trigger-trigger", |label| Selectable::new(label).build(ui)) {
                let _ = self.act_trigger.get_or_insert(InteractionEventAction::Trigger);
            },
            true =>
                self.act_untrigger = with_i18n!("trigger-untrigger", |label| Selectable::new(label).build(ui)),
        }
        if self.guid {
            self.act_guid_copy = with_i18n!("poi-copy-guid", |label| Selectable::new(label).build(ui));
        }
        #[cfg(todo)]
        if with_i18n!("trigger-behaviour", |label| Selectable::new(label).build(ui)) {
            self.act_trigger.get_or_insert(InteractionEventAction::Manual(TriggerKind::ALL));
        }

        ui.separator();
        let action_dismiss_open = with_i18n!("trigger-behaviour", |label| Selectable::new(label)
            .close_popups(false)
            .build(ui));
        match action_dismiss_open.then(|| self.act_selected_poi_delay.take()) {
            Some(Some(..)) => {
                let _ = self.act_trigger.get_or_insert(InteractionEventAction::Manual(TriggerKind::DISMISS));
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
                let _ = self.act_trigger.get_or_insert(InteractionEventAction::Dismiss(config));
            }
        }
        ui.separator();
        self.act_cat_open = with_i18n!("poi-category-navigate", |label| Selectable::new(label).build(ui));
    }
    pub fn action_trigger(&self, path: PoiPath, loaded_path: PoiMapPath, guid: Option<&Guid>) {
        if let Some(action) = self.act_trigger {
            let action = InteractionEvent::Interact {
                action,
                path,
                loaded_path,
            };
            Controller::with_sender(|s| if let Some(s) = &s.pathing {
                let _ = s.shared.interact.events.send(action);
            });
        } else if self.act_untrigger {
            let msg = match guid.cloned() {
                Some(guid) =>
                    PathingEvent::ResetMarkerIds(vec![MarkerId::with_uuid(guid.into())]),
                None => {
                    let marker_path = loaded_path.root.root.rel(MarkerIndex::with_poi(path.path));
                    PathingEvent::ResetMarkerPath(marker_path)
                }
            };
            msg.try_send();
        } else if self.act_guid_copy {
            if let Some(guid) = guid {
                let guid = guid.to_string();
                with_i18n!("copied", |copied|
                    rt::send_alert(self.ui, &format!("{copied}: {guid}"))
                );
                self.ui.set_clipboard_text(guid);
            }
        }
    }
}

#[deprecated]
type PoiInfoSorts = InteractSortFlags;
#[derive(Debug)]
pub struct PoiInfoContext {
    pub path: PoiPath,
    pub loaded_path: PoiMapPath,
    pub category_path: CategoryPath,
    pub guid: Guid,
    pub hidden: bool,
    pub selected_delay: Option<f32>,
    pub act_cat: CategoryActionSlot,
}
impl PoiInfoContext {
    pub fn new(
        path: PoiPath,
        loaded_path: PoiMapPath,
        category_path: Option<CategoryPath>,
        guid: Option<Guid>,
        hidden: bool,
    ) -> Self {
        Self {
            path,
            loaded_path,
            category_path: category_path.unwrap_or(CategoryPath::with_path(CategoryIndex::MAX)),
            guid: guid.unwrap_or_default(),
            hidden,
            selected_delay: None,
            act_cat: None,
        }
    }

    pub fn prepare_draw_menu<'a, 'u>(&self, ui: &'a Ui<'u>) -> DrawMenuPoi<'a, 'u> {
        DrawMenuPoi {
            ui,
            hidden: self.hidden,
            guid: true,
            act_trigger: None,
            act_untrigger: false,
            act_guid_copy: false,
            act_selected_poi_delay: self.selected_delay,
            act_cat_open: false,
        }
    }
    pub fn finish_draw_menu(&mut self, draw: DrawMenuPoi) {
        self.selected_delay = draw.act_selected_poi_delay;
        let guid = match self.guid.is_empty() {
            true => None,
            false => Some(&self.guid),
        };
        draw.action_trigger(self.path, self.loaded_path, guid);
        if draw.act_cat_open {
            if self.category_path.path == CategoryIndex::MAX {
                log::warn!("category unknown :<");
            } else {
                self.act_cat = Some((self.category_path, CategoryAction::Open(Some(true))));
            }
        }
    }
}
#[derive(Debug)]
pub struct PoiInfo {
    pub context: Option<PoiInfoContext>,
    pub wants_static: bool,
    pub wants_maps: bool,
    pub wants_entities: bool,
    pub filters: InteractFilterFlags,
    since_rendered: usize,
    #[cfg(todo)]
    pub open_interest: BTreeSet<MarkerId>,
    pub markers: IndexMap<MarkerId, PoiInfoMarker, FxBuildHasher>,
    pub dirty: PoiInfoSorts,
    pub dirty_sort: bool,
    pub dirty_pos: bool,
    pub last_pos: Point3<LocalSpace>,
    pub nearby: Watched<SharedNearbyMarkers>,
    pub entities: Watcher<SharedInteractEntities>,
    pub entities_bvh: SharedTriggerBvh,
}
impl Default for PoiInfo {
    fn default() -> Self {
        Self {
            context: Default::default(),
            #[cfg(todo)]
            open_interest: Default::default(),
            markers: Default::default(),
            dirty: PoiInfoSorts::empty(),
            dirty_sort: Default::default(),
            dirty_pos: Default::default(),
            wants_static: false,
            wants_maps: false,
            wants_entities: false,
            filters: InteractFilterFlags::DEFAULT_UI,
            since_rendered: 1,
            last_pos: Point3::INFINITY,
            nearby: Watched::EMPTY,
            entities: Watcher::EMPTY,
            entities_bvh: empty_trigger_bvh().clone(),
        }
    }
}
impl PoiInfo {
    pub fn is_dirty(&self) -> bool {
        self.dirty_sort | !self.dirty.is_empty()
    }
    pub fn prepare_sort(&mut self, sorts: PoiInfoSorts) -> bool {
        for flag in PoiInfoSorts::all() {
            self.dirty_sort |= self.dirty.set_replace(flag, false) & sorts.contains(flag);
        }
        self.dirty_sort
    }
    pub fn apply_sort(&mut self, sorts: PoiInfoSorts, sort_desc: PoiInfoSorts) {
        self.markers.sort_by(|aid, a, bid, b| {
            let mut a = a.sort_info(aid, sorts);
            a.flags = (a.flags ^ sort_desc) & sorts;
            let mut b = b.sort_info(bid, sorts);
            b.flags = (b.flags ^ sort_desc) & sorts;
            if sort_desc.contains(InteractSortFlags::DISTANCE) {
                mem::swap(&mut a.dist, &mut b.dist);
            }
            a.cmp(&b)
        });
        self.dirty_sort = false;
    }
    pub fn mark_gone<'a, I: IntoIterator>(&mut self, gone: I) where
        I::Item: AsRef<MarkerId>,
    {
        for mid in gone {
            let mid = mid.as_ref();
            let Some(marker) = self.markers.get_mut(mid) else { continue };
            marker.nearish = true;
            if mem::replace(&mut marker.nearby, false) {
                self.dirty.insert(PoiInfoSorts::NEARBY);
            }
        }
    }
    pub fn extend_nearby(&mut self, nearby: Option<&NearbyMarkers>) {
        let Some(nearby) = nearby.or(self.nearby.cached.as_ref()) else {
            return
        };
        for (lpath, path) in nearby.iter_pois() {
            let lpoi_path: Locator<PackMapPath, LoadedPoiPath> = lpath.map_path(LoadedPoiPath::with_path);
            let mpath: LoadedMarkerPath<PackMapPath> = lpoi_path.map_path(|p| p.pivot_to::<LoadedMarkerNs>().path);
            let mid = MarkerId::for_marker(mpath);
            match self.markers.entry(mid) {
                IndexEntry::Occupied(mut e) => {
                    let e = e.get_mut();
                    match e.path {
                        #[cfg(todo = "unnecessary")]
                        Locator { path: PoiIndex::MAX, .. } => (),
                        ref mut epath => *epath = path,
                    }
                    e.nearish = true;
                    if !mem::replace(&mut e.nearby, true) {
                        self.dirty.insert(PoiInfoSorts::NEARBY);
                    }
                },
                IndexEntry::Vacant(e) => {
                    let info = PoiInfoMarker {
                        path, nearby: true,
                        .. Default::default()
                    };
                    e.insert(info);
                    self.dirty_sort = true;
                },
            }
        }
    }
    pub fn clear_nearby(&mut self) {
        for marker in self.markers.values_mut() {
            if mem::replace(&mut marker.nearby, false) {
                self.dirty.insert(PoiInfoSorts::NEARBY);
            }
        }
    }
    pub fn iter_markers<'a>(&'a mut self) -> impl Iterator<Item = (MarkerId, Option<&'a mut PoiInfoMarker>)> + 'a {
        let exclude = !self.filters;
        self.markers.iter_mut()
            .filter(move |(_id, marker)| {
                let too_far = match marker.has_dist() {
                    _ if !exclude.contains(InteractFilterFlags::FAR) => false,
                    true => marker.dist_dist >= PoiInfoMarker::DIST_DIST_FAR_FILTER,
                    false => marker.dist_dist > PoiInfoMarker::DIST_DIST_MODERATE,
                };
                let flagged = marker.filter_flagged()
                    .or_if(InteractFilterFlags::FAR, too_far);
                !flagged.intersects(exclude)
            }).map(|(&id, marker)| (id, Some(marker)))
    }
    const DIST_DIST_THRESH: f32 = 8.0;
    pub fn wants_pos(&self, player_pos: Option<Point3<LocalSpace>>) -> bool {
        match player_pos {
            pos if self.last_pos.x.is_infinite() != pos.is_none() => true,
            _ if self.dirty_pos => true,
            Some(pos) => self.last_pos.distance_squared(pos) >= Self::DIST_DIST_THRESH,
            None => false,
        }
    }
    pub fn update_dist(&mut self, player_pos: Option<Point3<LocalSpace>>) {
        self.last_pos = player_pos.unwrap_or(Point3::INFINITY);

        let markers = {
            let exclude = !self.filters;
            self.markers.values_mut()
                .filter(move |marker| {
                    if !marker.has_pos() { return false }
                    let flagged = marker.filter_flagged()
                        .or_if(InteractFilterFlags::FAR, marker.dist_is_far());
                    !flagged.intersects(exclude)
                })
        };
        for e in markers {
            e.update_dist(player_pos);
        }
        self.dirty_pos = false;
        self.dirty.insert(PoiInfoSorts::DISTANCE);
    }

    pub fn init(&mut self, pathing: &Arc<PathingShared>) {
        match &mut self.nearby {
            #[cfg(todo = "unnecessary")]
            nearby if self.nearby.is_watching() => (),
            nearby => nearby.resubscribe_to(&pathing.interact.nearby),
        }
        match &mut self.entities {
            #[cfg(todo = "unnecessary")]
            entities if self.entities.is_watching() => (),
            entities => entities.resubscribe_to(&pathing.interact.entities),
        }
    }
    pub fn rx_nearby(&mut self) {
        if let Some(entities) = self.entities.try_read_if_changed() {
            let entities_bvh = ArcPtrCmp::from_mut(&mut self.entities_bvh);
            let entities_dirty = entities_bvh.clone_from_arc(&entities.trigger_bvh);
            #[cfg(todo)]
            let entities_dirty = true;
            self.wants_entities |= entities_dirty;
        }
        let prev = match self.nearby.watch.has_changed() {
            false => None,
            true => Some(self.nearby.cached.clone()),
        };
        let Some(nearby) = prev.as_ref().and_then(|_| self.nearby.try_read_update()) else { return };
        let mut prev = prev.flatten();
        let mut nearby = Cow::Borrowed(&*nearby);
        match &mut prev {
            Some(prev) if prev.map_id != nearby.map_id => {
                prev.clear();
            },
            Some(prev) if prev.is_empty() | nearby.is_empty() => (),
            Some(prev) => {
                let nearby = nearby.to_mut();
                prev.pois.retain(|lpath, _| {
                    match nearby.pois.remove(lpath) {
                        Some(..) => false,
                        None => true,
                    }
                });
            },
            None => (),
        }
        let mut alone = false;
        let nearby = match nearby {
            n if n.is_empty() => {
                if matches!(n, Cow::Borrowed(..)) {
                    alone = true;
                }
                None
            },
            Cow::Borrowed(_) => Some(None),
            Cow::Owned(n) => Some(Some(n)),
        };
        if alone {
            self.clear_nearby();
            prev = None;
        }
        if let Some(prev) = &prev {
            self.wants_maps = true;
            self.mark_gone(prev.iter_pois().map(|(lpath, _)| {
                let lpoi_path: Locator<PackMapPath, LoadedPoiPath> = lpath.map_path(LoadedPoiPath::with_path);
                let mpath: LoadedMarkerPath<PackMapPath> = lpoi_path.map_path(|p| p.pivot_to::<LoadedMarkerNs>().path);
                MarkerId::for_marker(mpath)
            }));
        }
        if let Some(nearby) = nearby {
            self.wants_maps = true;
            self.extend_nearby(nearby.as_ref());
        }
    }
    fn update_static_of_inner<'a>(
        &mut self,
        updates: impl IntoIterator<Item = (MarkerId, PoiPath, &'a LoadedPoiInfo, Option<&'a LoadedPoiShared>, bool, InteractSortFlags)>,
    ) {
        let updates = updates.into_iter().map(|(mid, poi_path, pinfo, spoi, hidden, mut filterable)| {
            let interactive = pinfo.get_marker_attrs().map(|a| SpaceInteraction::interest_for_marker(a));
            let mut flags = InteractSortFlags::empty();
            let mut flags_populated = InteractSortFlags::empty();
            let mut pos = Point3::INFINITY;
            if let Some(spoi) = spoi {
                flags.set(InteractSortFlags::VISIBLE | InteractSortFlags::ENABLED, spoi.visibility.is_visible());
                flags_populated.insert(InteractSortFlags::DISTANCE | InteractSortFlags::VISIBLE | InteractSortFlags::ENABLED | InteractSortFlags::FILTERED);
                pos = spoi.position;
                if !flags.contains(InteractSortFlags::VISIBLE) {
                    if !hidden | !spoi.visibility.intersects(VisibilityFlags::DEFAULT_TOGGLE) {
                        // can't really tell these two apart bleh
                        filterable.insert(InteractFilterFlags::DISABLED.as_sort_bits());
                    } else {
                        flags_populated.remove(InteractSortFlags::ENABLED);
                    }
                }
            } else {
                filterable.insert((InteractFilterFlags::FAR | InteractFilterFlags::STATIC).as_sort_bits());
            }
            if hidden {
                filterable.insert(InteractFilterFlags::FILTERED.as_sort_bits());
            }
            flags.set(InteractSortFlags::FILTERED, hidden);
            let interactive = match interactive {
                Some(i) => {
                    flags_populated.insert(InteractSortFlags::INTERACTIVE);
                    #[cfg(todo = "unnecessary")]
                    {
                        flags.set(PoiInfoSorts::INTERACTIVE, i.0.intersects(PoiInfoSorts::INTERACTIVE_MASK));
                    }
                    i
                },
                None => {
                    filterable.insert(InteractFilterFlags::STATIC.as_sort_bits());
                    (TriggerKind::empty(), false)
                },
            };
            (mid, poi_path, pinfo.category_path, pos, interactive, filterable, (flags, flags_populated))
        });
        self.update_entities_of(&mut {updates})
    }
    pub fn update_entities_of<'a>(
        &mut self,
        updates: &mut dyn Iterator<Item = (MarkerId, PoiPath, CategoryPath, Point3<LocalSpace>, (TriggerKind, bool), InteractSortFlags, (InteractSortFlags, InteractSortFlags))>,
    ) {
        let auto_config = LazyCell::new(|| Settings::try_read().map(|s| s.pathing.as_ref().map(|p| p.trigger_allow_auto).unwrap_or_else(TriggerKind::settings_default_interact)));
        for (mid, poi_path, category_path, pos, (interactive, auto), filterable, (flags, flags_populated)) in updates {
            #[cfg(todo)]
            let map_path = mid.get_marker_pack_map_path();
            #[cfg(todo)]
            let lidx = mid.get_marker_index();
            let entry = self.markers.entry(mid);
            let is_interactive = match flags_populated.contains(PoiInfoSorts::INTERACTIVE) {
                false if match &entry { IndexEntry::Occupied(e) => !InteractSortFlags::interactive(e.get().interactive).is_empty(), _ => false } => Ok(true),
                false if interactive.intersects(PoiInfoSorts::INTERACTIVE_MASK) => Err(Some(true)),
                false if filterable.contains(InteractFilterFlags::STATIC.as_sort_bits()) => Ok(false),
                false => Err(None),
                #[cfg(todo = "unnecessary")]
                true if flags.contains(PoiInfoSorts::INTERACTIVE) => Some(true),
                true => Ok(interactive.intersects(PoiInfoSorts::INTERACTIVE_MASK)),
            };
            let is_blacklisted = {
                let mut filterable = InteractFilterFlags::sort_as_bits(filterable);
                if let Ok(interactive) = is_interactive {
                    filterable.set(InteractFilterFlags::STATIC, !interactive);
                }
                let whitelist = self.filters | InteractFilterFlags::FILTERED | InteractFilterFlags::FAR;
                (!whitelist).intersects(filterable)
            };
            let marker = match entry {
                IndexEntry::Occupied(e) if is_blacklisted => {
                    e.shift_remove();
                    continue
                },
                IndexEntry::Occupied(e) => e.into_mut(),
                IndexEntry::Vacant(_) if is_blacklisted =>
                    continue,
                IndexEntry::Vacant(_) if !self.filters.contains(InteractFilterFlags::FAR) && !flags_populated.contains(PoiInfoSorts::DISTANCE) & !pos.x.is_infinite() & !(pos.x < PoiInfoMarker::DIST_DIST_FAR_FILTER) =>
                    continue,
                IndexEntry::Vacant(e) => {
                    self.dirty_sort = true;
                    let marker = e.insert(Default::default());

                    marker.nearish = !filterable.contains(InteractFilterFlags::FAR.as_sort_bits());
                    if filterable.contains(InteractFilterFlags::FAR.as_sort_bits()) {
                        marker.dist_dist = PoiInfoMarker::DIST_DIST_FAR;
                    } else if filterable.contains(InteractSortFlags::NEARBY) {
                        marker.nearish = true;
                    }
                    if filterable.contains(InteractFilterFlags::FILTERED.as_sort_bits()) {
                        marker.filtered = true;
                        marker.visible = false;
                    }
                    marker.enabled = !filterable.contains(InteractFilterFlags::DISABLED.as_sort_bits());
                    marker.filtered = filterable.contains(InteractFilterFlags::FILTERED.as_sort_bits());
                    marker.visible = !marker.filtered & marker.enabled;
                    marker.auto = auto;
                    if flags_populated.contains(PoiInfoSorts::INTERACTIVE) {
                        marker.interactive = interactive;
                        marker.auto = auto;
                    } else if filterable.contains(InteractFilterFlags::STATIC.as_sort_bits()) {
                        marker.interactive = TriggerKind::empty();
                    } else {
                        #[cfg(todo)]
                        if flags.contains(PoiInfoSorts::INTERACTIVE) {
                            marker.auto = auto;
                        }
                        if interactive.is_empty() {
                            marker.interactive = interactive;
                        }
                    }
                    marker
                },
            };
            let mut dirty_int = false;
            if flags_populated.contains(PoiInfoSorts::INTERACTIVE) {
                let prev = mem::replace(&mut marker.interactive, interactive);
                dirty_int |= prev != marker.interactive;
            }
            if (flags | flags_populated).contains(PoiInfoSorts::INTERACTIVE) {
                let assume_auto = false;
                let auto = match auto {
                    true if auto_config.map(|allowed| marker.interactive.intersects(!allowed)).unwrap_or(!assume_auto) =>
                        false,
                    auto => auto,
                };
                let prev = mem::replace(&mut marker.auto, auto);
                dirty_int |= prev != marker.auto;
            }
            if dirty_int {
                self.dirty.insert(PoiInfoSorts::INTERACTIVE);
            }
            let nearish = match filterable.contains(InteractSortFlags::NEARBY) {
                true => Some(true),
                _ if filterable.contains(InteractFilterFlags::FAR.as_sort_bits()) => Some(false),
                false => None,
            };
            if let Some(nearish) = nearish {
                let _prev = mem::replace(&mut marker.nearish, nearish);
                if !_prev {
                    self.dirty.insert(PoiInfoSorts::DISTANCE);
                }
            }
            if poi_path.path != PoiIndex::MAX {
                marker.path = poi_path;
            }
            if category_path.path != CategoryIndex::MAX {
                marker.category_path = category_path;
            }
            if flags_populated.contains(PoiInfoSorts::VISIBLE) {
                let prev = mem::replace(&mut marker.visible, flags.contains(PoiInfoSorts::VISIBLE));
                if prev != marker.visible {
                    self.dirty.insert(PoiInfoSorts::VISIBLE);
                }
            }
            if flags_populated.contains(PoiInfoSorts::ENABLED) {
                let prev = mem::replace(&mut marker.enabled, flags.contains(PoiInfoSorts::ENABLED));
                if prev != marker.enabled {
                    self.dirty.insert(PoiInfoSorts::ENABLED);
                }
            }
            if flags_populated.contains(PoiInfoSorts::DISTANCE) {
                let prev = mem::replace(&mut marker.pos, pos);
                if !vec_eq(prev, marker.pos) {
                    self.dirty_pos = true;
                }
            } else if !marker.has_dist() | !marker.has_pos() {
                let dist_dist = if !pos.x.is_infinite() {
                    Some(pos.x)
                } else if filterable.contains(InteractFilterFlags::FAR.as_sort_bits()) {
                    Some(PoiInfoMarker::DIST_DIST_FAR)
                } else if filterable.contains(InteractSortFlags::NEARBY) {
                    Some(PoiInfoMarker::DIST_DIST_MODERATE)
                } else { None };
                if let Some(dist_dist) = dist_dist {
                    let prev = mem::replace(&mut marker.dist_dist, dist_dist);
                    if prev != marker.dist_dist {
                        self.dirty.insert(PoiInfoSorts::DISTANCE);
                        if !filterable.contains(InteractSortFlags::NEARBY) && marker.dist_dist.to_bits() == PoiInfoMarker::DIST_DIST_FAR.to_bits() {
                            marker.nearish = false;
                        }
                    }
                }
            }
            if flags_populated.contains(PoiInfoSorts::FILTERED) {
                let prev = mem::replace(&mut marker.filtered, flags.contains(PoiInfoSorts::FILTERED));
                if prev != marker.filtered {
                    self.dirty.insert(PoiInfoSorts::FILTERED);
                }
            }
        }
    }
    pub fn update_static_render(&mut self, render: &PackRender) {
        let spacepacks = if self.last_pos.x.is_infinite() {
            // unlikely in-game
            return
        } else {
            match render.spacepacks.cached.as_ref() {
                Some(sp) if sp.map_id.is_none() => None,
                sp => sp,
            }
        };
        self.wants_static = false;
        match spacepacks {
            Some(spacepacks) => {
                let range = self.range_for(self.last_pos);
                let query = box3aabb(range);
                let filterable = InteractSortFlags::NEARBY;
                let mut nearish = BTreeSet::new();
                let updates = spacepacks.bvh_traverse_shapes(&query).map(|e| {
                    let mid = e.id;
                    let pos: Point3<LocalSpace> = MintConv::from_nalg(e.bounds.center());
                    let poi_path: PoiPath = PoiPath::with_path(PoiIndex::MAX);
                    let category_path: CategoryPath = CategoryPath::with_path(CategoryIndex::MAX);
                    let flags = InteractSortFlags::empty();
                    let flags_populated = InteractSortFlags::DISTANCE;
                    nearish.insert(mid);
                    (mid, poi_path, category_path, pos, (TriggerKind::empty(), false), filterable, (flags, flags_populated))
                });
                self.update_entities_of(&mut {updates});
                if !nearish.is_empty() {
                    let updates = render.pack_data.iter().filter_map(|(_pack_path, pack)|
                        match &pack.map_info {
                            Some(map_info) => Some((map_info, Some(&pack.map_state))),
                            None => None,
                        }
                    );
                    let nearish = &nearish;
                    let updates = updates.flat_map(move |(map_info, map)| {
                        map_info.loaded_pois(map).filter_map(move |poi| {
                            let mid = poi.as_marker().loaded_id();
                            if !nearish.contains(&mid) { return None }
                            let lpoi = poi.as_loaded();
                            let is_hidden = lpoi.map(|lpoi| lpoi.is_hidden());
                            let lpoi = lpoi.map(|lpoi| lpoi.lpoi());
                            let poi_path = poi.poi_path();
                            Some((mid, poi_path, poi.lpoi_info(), lpoi, is_hidden.unwrap_or(false), filterable))
                        })
                    });
                    self.update_static_of_inner(updates);
                }
            },
            _ => {
                let updates = render.pack_data.iter().filter_map(|(_pack_path, pack)|
                    match &pack.map_info {
                        Some(map_info) => Some((map_info, Some(&pack.map_state))),
                        None => None,
                    }
                );
                self.update_static_of(&mut {updates})
            },
        }
    }
    pub fn update_static_ui(&mut self, packs: &IndexedList<PackRegistryNs, PackIndex, [PackElement]>) {
        self.wants_static = false;
        let updates = packs.values().filter_map(|pack_state|
            pack_state.state.map_info.as_ref().map(|map_info| (map_info, None))
        );
        self.update_static_of(&mut {updates})
    }
    pub fn update_static_of<'a>(&mut self, updates: &mut dyn Iterator<Item = (&'a SharedMapPackLoaded, Option<&'a SharedMapPackState>)>) {
        let updates = updates.flat_map(move |(map_info, map)| {
            map_info.loaded_pois(map).map(move |poi| {
                let mid = poi.as_marker().loaded_id();
                #[cfg(todo)]
                let interactive = pinfo.get_marker_attrs().map(|i| SpaceInteraction::interaction_is(i, PoiInfoSorts::INTERACTIVE_MASK)).unwrap_or(false);
                #[cfg(todo)]
                let marker = match self.markers.entry(mid) {
                    #[cfg(todo)]
                    IndexEntry::Occupied(e) if !self.filters.contains(InteractFilterFlags::STATIC) && !interactive => {
                        e.remove();
                        continue
                    },
                    IndexEntry::Occupied(e) => e.into_mut(),
                    IndexEntry::Vacant(e) if !self.filters.contains(InteractFilterFlags::STATIC) && !interactive =>
                        continue,
                    IndexEntry::Vacant(e) => e.insert(Default::default()),
                };
                let lpoi = poi.as_loaded();
                let is_hidden = lpoi.map(|lpoi| lpoi.is_hidden());
                let lpoi = lpoi.map(|lpoi| lpoi.lpoi());
                let poi_path = poi.poi_path();
                let filterable = InteractSortFlags::empty();
                (mid, poi_path, poi.lpoi_info(), lpoi, is_hidden.unwrap_or(false), filterable)
            })
        });
        self.update_static_of_inner(updates);
    }
    /// XXX: 2 locks is scary :<
    pub fn update_entities(&mut self, range: Box3<LocalSpace>) {
        let entities_bvh = self.entities_bvh.clone();
        let Ok(trigger_bvh) = entities_bvh.try_read_owned() else {
            // next time...
            return
        };
        self.wants_entities = false;
        let entities = self.entities.get_sender().cloned();
        let entities = match entities.as_ref().map(|e| e.borrow()) {
            Some(e) => e,
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => return,
        };
        let range2 = Box2 {
            min: LocalSpace::to2(range.min),
            max: LocalSpace::to2(range.max),
        };
        let mut farish = match self.filters.contains(InteractFilterFlags::FAR) {
            true => Some(entities.entities.iter()
                .map(|e| e.poi_path())
                .collect::<BTreeSet<_>>()
            ),
            false => None,
        };
        let query = box2aabb(range2);
        let map_entity = |lpath: PoiMapPath, pos: Option<Point3<LocalSpace>>, auto: bool, mut filterable: InteractSortFlags| {
            let lpoi_path: Locator<PackMapPath, LoadedPoiPath> = lpath.map_path(LoadedPoiPath::with_path);
            let mpath: LoadedMarkerPath<PackMapPath> = lpoi_path.map_path(|p| p.pivot_to::<LoadedMarkerNs>().path);
            let mid = MarkerId::for_marker(mpath);
            // we know it's interactive, just not how much... so fill this with a falsehood as long as we don't claim it's populated
            let interactive = (TriggerKind::BEHAVIOUR, auto);
            // TODO: ENABLED?
            let mut populated = PoiInfoSorts::empty();
            let flags = PoiInfoSorts::empty();
            let poi_path: PoiPath = PoiPath::with_path(PoiIndex::MAX);
            let cat_path: CategoryPath = CategoryPath::with_path(CategoryIndex::MAX);
            let pos = match pos {
                Some(pos) => {
                    populated.insert(PoiInfoSorts::DISTANCE);
                    pos
                },
                None => {
                    filterable.insert(InteractFilterFlags::FAR.as_sort_bits());
                    filterable.remove(InteractSortFlags::NEARBY);
                    Point3::INFINITY
                },
            };
            (mid, poi_path, cat_path, pos, interactive, filterable, (flags, populated))
        };
        let nearish_entities = trigger_bvh.traverse_iterator(&query, &entities.entities)
            .filter(|e| match TRIGGER_DIMENSION {
                2 => {
                    let pos = e.value.bounds.position;
                    !((pos.y > range.max.y) | (pos.y < range.min.y))
                },
                _ => true,
            }).map(|e| {
                if let Some(farish) = &mut farish {
                    farish.remove(&e.value.poi_path());
                }
                let filterable = InteractSortFlags::NEARBY;
                map_entity(e.value.poi_path(), Some(e.value.bounds.position), e.value.bounds.is_auto(), filterable)
            });
        self.update_entities_of(&mut {nearish_entities});
        if let Some(farish) = farish {
            let farish_entities = farish.into_iter()
                .map(|path| {
                    let filterable = InteractFilterFlags::FAR.as_sort_bits();
                    map_entity(path, None, false, filterable)
                });
            self.update_entities_of(&mut {farish_entities});
        }
    }
    pub fn update_entities_relaxed(&mut self) {
        if self.wants_entities & !self.wants_static {
            if !self.last_pos.x.is_infinite() {
                let range = self.range_for(self.last_pos);
                self.update_entities(range)
            }
        }
    }
    pub fn wants_maps(&self) -> bool {
        self.wants_maps && !(self.wants_static | self.wants_entities | self.nearby.cached.is_none())
    }
    pub fn rx_maps(&mut self, maps: &SharedGameplayMap) {
        self.wants_maps = false;
        log::debug!("rx maps");
        if let Some(map_id) = maps.map_id {
            let dirty = &mut self.dirty;
            match self.filters.contains(InteractFilterFlags::DISABLED) {
                false => {
                    let updates = maps.iter_state()
                        .map(|(_, map, map_info)| (map_info, Some(map)));
                    self.update_static_of(&mut {updates})
                },
                true => {
                    let updates = maps.iter_loaded()
                        .map(|(_, map_info, map)| (map_info, map));
                    self.update_static_of(&mut {updates})
                },
            }
            #[cfg(todo)]
            self.markers.retain(|k, marker| {
                let map_path = k.get_marker_pack_map_path();
                if map_path.path != map_id { return false }
                let lpoi = maps.ref_loaded_marker(*k)
                    .and_then(|l| l.to_loaded_poi());
                if let Some(poi) = lpoi {
                    if let Some((interest, _auto)) = poi.lpoi_info().get_interaction_attrs().map(|i| SpaceInteraction::interest_for(i)) {
                        let prev = mem::replace(&mut marker.interactive, interest);
                        if prev != marker.interactive {
                            dirty.insert(PoiInfoSorts::INTERACTIVE);
                        }
                    }

                    let lpoi = poi.lpoi();

                    let prev = mem::replace(&mut marker.visible, lpoi.visibility.is_visible());
                    if prev != marker.visible {
                        dirty.insert(PoiInfoSorts::VISIBLE);
                    }
                    let prev = mem::replace(&mut marker.filtered, poi.is_hidden());
                    if prev != marker.filtered {
                        dirty.insert(PoiInfoSorts::FILTERED);
                    }
                    true
                } else if let Some(_map_info) = maps.get_info_for(map_path.root) {
                    true
                } else { false }
            });
        } else {
            if !self.markers.is_empty() {
                log::debug!("TODO: premature clear or no?");
            }
            self.markers.clear();
        }
    }
    pub fn wants_static_all(&self) -> bool {
        self.wants_static & self.filters.contains(InteractFilterFlags::DISABLED)
    }
    /// TODO: populate+sync filter+sort flags with settings
    pub fn pre_draw(&mut self, visibility: PackVisibility) {
        let rendered = match (visibility, self.since_rendered) {
            (PackVisibility::Closed, _) =>
                usize::MAX,
            (PackVisibility::Visible, usize::MAX) => {
                self.wants_static |= self.filters.intersects(InteractFilterFlags::STATIC | InteractFilterFlags::DISABLED);
                self.wants_entities |= self.filters.contains(InteractFilterFlags::FAR);
                self.wants_maps |= true;
                self.nearby.mark_changed();
                1
            },
            (_, usize::MAX) =>
                Self::GIVE_UP_FRAMES + 1,
            _ => self.since_rendered.saturating_add(1),
        };
        let prev = mem::replace(&mut self.since_rendered, rendered);
        if visibility.is_closed() {
            if prev == usize::MAX { return }
            self.cleanup_cache();
            self.nearby.mark_changed();
            self.wants_maps = true;
        }
    }
    pub fn cleanup_cache(&mut self) {
        self.markers.clear();
        self.entities_bvh = empty_trigger_bvh().clone();
        self.nearby.cached = None;
        self.last_pos = Point3::INFINITY;
    }
    pub(crate) fn range_for(&self, player_pos: Point3<LocalSpace>) -> Box3<LocalSpace> {
        let half = PoiInfoMarker::DIST_FAR_FILTER_2;
        Box3::new(
            player_pos - half.to_vector(),
            player_pos + half.to_vector(),
        )
    }
    const GIVE_UP_FRAMES: usize = 0x200;
    pub fn visibility(&self) -> PackVisibility {
        match self.since_rendered {
            0 => PackVisibility::Visible,
            usize::MAX => PackVisibility::Closed,
            1..=Self::GIVE_UP_FRAMES => PackVisibility::Offset,
            _ => PackVisibility::Pending,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PoiInfoSort {
    pub dist: u32,
    pub flags: PoiInfoSorts,
    pub pack_path: PackMapPath,
    pub index: MarkerIndex,
}
impl cmp::PartialOrd for PoiInfoSort {
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
impl cmp::Ord for PoiInfoSort {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        self.dist.cmp(&rhs.dist)
            .then((self.flags ^ InteractSortFlags::SORT_INVERTED).cmp(&(rhs.flags ^ InteractSortFlags::SORT_INVERTED)))
            .then(self.pack_path.path.cmp(&rhs.pack_path.path))
            .then(self.index.cmp(&rhs.index))
    }
}
#[derive(Debug, Clone)]
pub struct PoiInfoMarker {
    pub path: PoiPath,
    pub nearby: bool,
    pub nearish: bool,
    pub visible: bool,
    pub enabled: bool,
    pub auto: bool,
    pub category_path: CategoryPath,
    pub dist_dist: f32,
    pub pos: Point3<LocalSpace>,
    pub display_name: Option<Result<AttrString, Arc<str>>>,
    pub filtered: bool,
    #[cfg(todo = "unnecessary")]
    pub interactive: Option<InteractionAttributes>,
    pub interactive: TriggerKind,
}
impl PoiInfoMarker {
    const DIST_DIST_MODERATE: f32 = 100_00.0007;
    const DIST_DIST_FAR: f32 = 100_00_00.0007;
    const DIST_DIST_IRRELEVANT: f32 = IRRELEVANT_MID * IRRELEVANT_MID;
    const DIST_FAR_FILTER_2: Size3<LocalSpace> = {
        let (far, far_v) = (325.0f32, 250.0);
        Size3::new(far / 2.0, far / 2.0, far_v / 2.0)
    };
    const DIST_DIST_FAR_FILTER: f32 = ((Self::DIST_FAR_FILTER_2.width * 2.0) as u64 - 1).pow(2) as f32;

    pub fn update_dist(&mut self, player_pos: Option<Point3<LocalSpace>>) {
        self.dist_dist = match player_pos {
            Some(player_pos) if !self.pos.x.is_infinite() =>
                self.pos.distance_squared(player_pos),
            _ => Self::DIST_DIST_MODERATE,
        };
    }

    pub fn has_pos(&self) -> bool {
        !self.pos.x.is_infinite()
    }
    const BITS_MODERATE: u32 = Self::DIST_DIST_MODERATE.to_bits();
    const BITS_FAR: u32 = Self::DIST_DIST_FAR.to_bits();
    const BITS_IRRELEVANT: u32 = Self::DIST_DIST_IRRELEVANT.to_bits();
    pub fn has_dist(&self) -> bool {
        !matches!(self.dist_dist.to_bits(), Self::BITS_MODERATE | Self::BITS_FAR | Self::BITS_IRRELEVANT)
    }
    pub fn dist_is_far(&self) -> bool {
        let bits = self.dist_dist.to_bits();
        (bits == Self::DIST_DIST_FAR.to_bits()) | (bits == Self::DIST_DIST_IRRELEVANT.to_bits())
    }

    pub fn sort_flags(&self) -> PoiInfoSorts {
        [
            self.nearby.then_some(PoiInfoSorts::NEARBY),
            (self.visible & !self.filtered).then_some(PoiInfoSorts::VISIBLE),
            self.filtered.then_some(PoiInfoSorts::FILTERED),
            self.enabled.then_some(PoiInfoSorts::ENABLED),
            #[cfg(todo = "unnecessary")]
            self.has_dist().then_some(PoiInfoSorts::DISTANCE),
            #[cfg(todo = "unnecessary")]
            self.has_pos().then_some(PoiInfoSorts::DISTANCE),
            PoiInfoSorts::interactive(self.interactive).get(),
        ].into_iter().map(|v| v.unwrap_or(PoiInfoSorts::empty())).collect()
    }
    pub fn filter_flagged(&self) -> InteractFilterFlags {
        let mask = !InteractFilterFlags::FAR;
        InteractFilterFlags::for_sort(self.sort_flags()) & mask
    }
    pub fn sort_info(&self, mid: &MarkerId, mask: PoiInfoSorts) -> PoiInfoSort {
        PoiInfoSort {
            flags: self.sort_flags(),
            pack_path: mid.get_marker_pack_map_path(),
            index: mid.get_marker_index(),
            dist: match mask.contains(PoiInfoSorts::DISTANCE) {
                false => 0,
                true => (self.dist_dist as u32).saturating_mul(32),
                #[cfg(todo = "unnecessary")]
                true => (self.dist_dist * 100.0)
                    .min(0x40000000i32 as f32) as u32,
            },
        }
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_ref().map(|n| match n {
            Ok(n) => &n[..],
            Err(n) => &n[..],
        })
    }
    pub fn ui_id(&self, mid: &MarkerId) -> (MapPath, UiId) {
        let map_path = mid.get_marker_pack_map_path();
        let idx = match self.path.path {
            #[cfg(todo = "unnecessary")]
            i if i != PoiIndex::MAX => MarkerIndex::with_poi(idx),
            _ => mid.get_marker_index().index_poi_unchecked(),
        };
        let pidx = (map_path.root.path as u32).rotate_left(28);
        (map_path.unscope(), UiId::Int((pidx ^ idx) as i32))
    }
}
impl Default for PoiInfoMarker {
    fn default() -> Self {
        Self {
            path: Default::default(),
            nearby: false,
            nearish: true,
            visible: true,
            enabled: true,
            auto: false,
            filtered: false,
            interactive: TriggerKind::DISMISS,
            dist_dist: Self::DIST_DIST_MODERATE,
            pos: Point3::INFINITY,
            display_name: None,
            category_path: CategoryPath::with_path(CategoryIndex::MAX),
        }
    }
}
pub struct DrawPoiInfo<'s, 'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'s mut PoiInfo,
    pub pack_state: &'a IndexedList<PackRegistryNs, PackIndex, [PackElement]>,
    pub bounds: Box2<f32>,
    pub bounds_height: f32,
}
impl<'s, 'a, 'u> DrawPoiInfo<'s, 'a, 'u> {
    pub fn new(
        ui: &'a Ui<'u>,
        state: &'s mut PoiInfo,
        pack_state: &'a IndexedList<PackRegistryNs, PackIndex, [PackElement]>,
    ) -> Self {
        Self {
            ui,
            state,
            pack_state,
            bounds: Box2::ZERO,
            bounds_height: 0.0,
        }
    }
    pub fn draw(&mut self) {
        let ui = self.ui;
        self.bounds = Box2::new(
            Point2::from_array(ui.window_content_region_min()),
            Point2::from_array(ui.window_content_region_max()),
        );
        #[cfg(todo = "unused")]
        let start_pos: Point2<f32> = Point2::from_array(ui.cursor_start_pos());
        let window_size: Size2<f32> = Size2::from_array(ui.window_size());
        self.bounds_height = (self.bounds.max.y - self.bounds.min.y).max(window_size.height) + ui.text_line_height_with_spacing() * 2.0;

        {
            let _flags = ui.push_id("filter-flags");
            if with_i18n!("pois-map", |label| ui.checkbox_flags(&label, &mut self.state.filters, InteractFilterFlags::FAR)) {
                if self.state.filters.contains(InteractFilterFlags::FAR) {
                    self.state.wants_entities = true;
                }
                if self.state.filters.intersects(InteractFilterFlags::STATIC | InteractFilterFlags::DISABLED) {
                    self.state.wants_static = true;
                }
            }
            ui.same_line();
            if with_i18n!("pois-other", |label| ui.checkbox_flags(&label, &mut self.state.filters, InteractFilterFlags::STATIC)) {
                if self.state.filters.contains(InteractFilterFlags::STATIC) {
                    self.state.wants_static = true;
                }
            }
            ui.same_line();
            if with_i18n!("disabled", |label| ui.checkbox_flags(&label, &mut self.state.filters, InteractFilterFlags::DISABLED)) {
                if self.state.filters.contains(InteractFilterFlags::DISABLED) {
                    self.state.wants_static = true;
                }
            }
            ui.same_line();
            if with_i18n!("pois-hidden", |label| ui.checkbox_flags(&label, &mut self.state.filters, InteractFilterFlags::FILTERED)) {
                #[cfg(todo = "unnecessary")]
                if self.filters.contains(InteractFilterFlags::FILTERED) {
                    self.state.wants_static = true;
                }
            }
        }
        let table = Self::open_table(ui, "ipois");
        if let Some(_table) = table {
            self.state.since_rendered = 0;
            self.update_sort();
            self.draw_nearby();
            #[cfg(deleteme)]
            {
            let id = "pois-other";
            let other = with_i18n!(id, |label| TreeNode::new(id)
                .flags(TreeNodeFlags::SPAN_FULL_WIDTH)
                .label::<&str, _>(&label)
                .tree_push_on_open(false)
                .opened(false, Condition::Appearing)
                .leaf(false).push(ui));
            if let Some(_node) = other {
                self.draw_nearby_other();
            }
            }
        }
        #[cfg(deleteme)]
        let table = RenderInteractivePoi::draw_table_start(ui, "pois-map");
        #[cfg(deleteme)]
        if let Some(_table) = table {
        }
    }
    pub fn draw_nearby(
        &mut self,
    ) {
        let mut map_id_token = (None, None);
        let ui = self.ui;
        let label_static = fl!("poi-static");
        let label_filtered = fl!("poi-filtered");
        let label_disabled = fl!("disabled");
        let label_hidden = label_disabled;
        let label_auto = fl!("poi-auto");
        let label_activate = fl!("poi-activate");
        let label_bounce = fl!("poi-activate-bounce");
        let label_copy = fl!("poi-activate-copy");
        let label_info = fl!("poi-activate-info");
        let label_behaviour = fl!("poi-activate-behaviour");
        let label_far = "×";
        let mut open_context = None;
        for (mid, marker) in self.state.iter_markers() {
            let Some(marker) = marker else { continue };
            let (map_id, ui_id) = marker.ui_id(&mid);
            if map_id_token.0 != Some(map_id) {
                // drop prior token first!
                map_id_token.1 = None;
                map_id_token.1 = Some(ui.push_id(UiId::Int(map_id.path.get() as i32)));
                map_id_token.0 = Some(map_id);
            }
            let _id = ui.push_id(ui_id);
            let pos: Point2<f32> = Point2::from_array(ui.cursor_pos());
            let offset = pos.y + self.bounds.min.y;
            let render = offset >= 0.0 && offset <= self.bounds_height;

            let marker_path = mid.get_marker_path_pack_map();
            let map_path = marker_path.root;
            let lpoi_path: Option<LoadedPoiPath> = match marker_path.path.namespace() {
                MarkerIndex::NS_POI => Some(LoadedPoiPath::with_path(marker_path.path.index_poi_unchecked())),
                _ => None,
            };
            let pack_state = self.pack_state;
            let pd = LazyCell::new(|| pack_state.lookup_ref(&map_path.root));
            let map_info = LazyCell::new(|| pd.and_then(|pd| pd.state.map_info.as_ref()));
            let info = LazyCell::new(|| map_info
                .and_then(|map_info| lpoi_path.and_then(|loc| map_info.pois().lookup_ref(&loc))));
            let guid = || lpoi_path.and_then(|lpoi_path|
                map_info.and_then(|map_info| map_info.poi_guid_by_index(lpoi_path))
            );
            let poi_attrs = || info.and_then(|i| i.get_marker_attrs());

            for i in 0..4 {
                #[cfg(todo)]
                let coli = {
                    if !ui.table_next_column() { continue }
                    ui.table_column_index()
                };
                let coli = {
                    if i == 0 {
                        ui.table_next_row();
                        continue
                    }
                    ui.table_set_column_index(i);
                    i
                };
                match coli {
                    0 => (),
                    1 => {
                        let label = match (marker.nearby, marker.nearish, marker.dist_dist) {
                            _ if !render => "",
                            (true, _, _) => "<",
                            (false, true, _) => "❓",
                            (false, _, PoiInfoMarker::DIST_DIST_MODERATE) => "❓",
                            (false, _, PoiInfoMarker::DIST_DIST_FAR) => ">",
                            (false, _, PoiInfoMarker::DIST_DIST_IRRELEVANT) => "",
                            (false, _, _) => "×",
                        };
                        if !label.is_empty() {
                            ui.text(label)
                        }
                    },
                    2 => {
                        let label = match (marker.filtered, marker.visible) {
                            _ if !render => "",
                            (true, _) =>
                                &label_filtered[..],
                            (_, false) =>
                                &label_hidden[..],
                            _ => "",
                        };
                        ui.text(label);
                    },
                    3 => {
                        let mut trigger = false;
                        let interactive = marker.interactive & PoiInfoSorts::INTERACTIVE_MASK;
                        let interest = marker.interactive;
                        let label = match interactive.is_empty() {
                            _ if !render => continue,
                            true if marker.interactive.contains(TriggerKind::BOUNCE) => {
                                if ui.small_button(&label_bounce) {
                                    trigger = true;
                                }
                                ""
                            },
                            true => {
                                &label_static[..]
                            },
                            false if marker.auto => &label_auto[..],
                            false => {
                                let label = if interest.contains(TriggerKind::COPY) {
                                    &label_copy[..]
                                } else if interest.contains(TriggerKind::INFO) {
                                    &label_info[..]
                                } else if interactive == TriggerKind::BEHAVIOUR {
                                    &label_behaviour[..]
                                } else {
                                    &label_activate[..]
                                };
                                if ui.small_button(&label) {
                                    trigger = true;
                                }
                                ""
                            },
                        };
                        if !label.is_empty() {
                            ui.text(label)
                        }
                        let has_tooltip = interest.intersects(TriggerKind::INFO | TriggerKind::COPY | TriggerKind::BEHAVIOUR);
                        if has_tooltip && ui.is_item_hovered() {
                            #[cfg(deleteme)]
                            let pack_path = mid.get_marker_path_pack_map();
                            #[cfg(deleteme)]
                            let lidx = mid.get_marker_index();
                            #[cfg(deleteme)]
                            let lpoi_path: Option<LoadedPoiPath> = match lidx.namespace() {
                                MarkerIndex::NS_POI => Some(LoadedPoiPath::with_path(lidx.index_poi_unchecked())),
                                _ => None,
                            };
                            #[cfg(deleteme)]
                            let pd = self.pack_state.lookup_ref(&pack_path.root.root);
                            #[cfg(deleteme)]
                            let map_info = pd.and_then(|pd| pd.state.map_info.as_ref());
                            #[cfg(deleteme)]
                            let info = map_info
                                .and_then(|map_info| lpoi_path.and_then(|loc| map_info.pois().lookup_ref(&loc)));
                            if let Some(attrs) = poi_attrs() {
                                ui.tooltip(|| {
                                    let copyable = attrs.interaction.as_ref().and_then(|i| i.copy_value().map(|v| (v, i.copy_message())));
                                    let info = attrs.interaction.as_ref().and_then(|i| i.info());
                                    if let Some(msg) = info {
                                        ui.text(msg);
                                    }
                                    if let Some((value, msg)) = copyable {
                                        if info.is_some() {
                                            ui.separator();
                                        }
                                        ui.text("\"");
                                        ui.same_line();
                                        ui.text(value);
                                        ui.same_line();
                                        ui.text("\"");
                                        if let Some(msg) = msg {
                                            ui.separator();
                                            ui.text(msg);
                                        }
                                    }
                                })
                            }
                        }
                    },
                    _ => (),
                }
            }
            ui.table_set_column_index(0);

            let pack_data = LazyCell::new(|| pd.and_then(|pd| pd.state.pack_data()));
            let cat_path = match marker.category_path {
                p if p.path != CategoryIndex::MAX => Some(p),
                _ => None,
            };
            let cat = LazyCell::new(||{
                let cat_path = cat_path.or_else(|| info.map(|pinfo| pinfo.category_path));
                pd.and_then(|state| cat_path.and_then(|cat_path| {
                    state.categories.categories.get(&cat_path).map(Ok)
                    .or_else(|| {
                        pack_data.as_ref().and_then(|pack| pack.categories.all_categories.get_index(cat_path.path as usize).map(Err))
                    })
                }))
            });
            let cat_info = LazyCell::new(|| cat.map(|cat| match cat {
                Ok(cat_info) => {
                    Cow::Borrowed(cat_info)
                },
                Err((_cat_id, cat)) => {
                    Cow::Owned(CategoryInfo::from_pack_category(cat))
                },
            }));
            #[cfg(deleteme)]
            let poi_attrs = LazyCell::new(|| {
                info.and_then(|i| i.get_marker_attrs())
                .map(Ok)
                .or_else(|| cat_info.as_ref().map(Err))
            });

            let mut name_is_cat = false;
            let named = if !render {
                Some("POI")
            } else if marker.display_name.is_none() {
                let poi_name = poi_attrs().and_then(|a| a.tip_name.clone());
                name_is_cat = poi_name.is_none();
                marker.display_name = poi_name.map(Ok).or_else(||
                    cat_info.as_ref().and_then(|info| info.display_name.clone())
                        .map(Err)
                );
                None
            } else {
                None
            };
            match named.or(marker.display_name()) {
                Some(name) => ui.text(name),
                None => {
                    ui.text(mid.get_marker_path_pack_map().root.rel(marker.path).to_string())
                },
            }
            if ui.is_item_clicked_with_button(MouseButton::Right) {
                open_context = lpoi_path.map(|loaded_path| PoiInfoContext::new(
                    marker.path,
                    map_path.rel(loaded_path.path),
                    cat_path,
                    guid().cloned(),
                    !marker.visible | marker.filtered,
                ));
            } else if ui.is_item_hovered() {
                let name_is_cat = matches!(marker.display_name, Some(Err(..)));
                let (poi_tip, poi_desc) = match poi_attrs() {
                    Some(attrs) =>
                        (attrs.tip_name(), attrs.tip_description()),
                    None => (None, None),
                };
                if poi_tip.is_some() | poi_desc.is_some() {
                    ui.tooltip(|| {
                        if let Some(name) = poi_tip {
                            ui.text(name);
                        }
                        if let Some(desc) = poi_desc {
                            ui.text(desc);
                        }
                    });
                } else if let Some(info) = cat_info.as_ref() {
                    if let Some(tooltip) = info.tooltip() {
                        super::DrawCategoryTooltip {
                            ui,
                            info,
                            tooltip,
                            display_name_visible: name_is_cat,
                            include_copyable: false,
                        }.draw();
                    }
                }
            }
        }

        if let Some(context) = open_context {
            self.state.context = Some(context);
        }
        #[cfg(deleteme)]
        if let Some(context) = open_context {
            let open = self.state.context.is_none();
            if open {
                ui.open_popup("poi-context");
            }
        }
    }

        #[cfg(deleteme)]
    pub fn draw_nearby_other(
        &mut self,
    ) {
        if let Some(spacepacks) = self.spacepacks {
            //box3 of map space, iter, filter out anything from other sections
            todo
        } else {
            // iter all lpois, filter here too
            todo
        }
    }
    const HEADER_TITLE: &'static str = "pois-map";
    const HEADER_NEARBY: &'static str = "pois-nearby";
    const HEADER_HIDDEN: &'static str = "pois-hidden";
    const HEADER_INTERACT: &'static str = "pois-interactive";
    fn open_table(
        ui: &Ui<'u>,
        title_id: &str,
    ) -> Option<TableToken<'u>> {
        let table_flags =
            TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS | TableFlags::SORTABLE | TableFlags::SORT_MULTI | TableFlags::SORT_TRISTATE;
        let table_token = ui.begin_table_with_flags(title_id, 4, table_flags);
        if let Some(..) = &table_token {
            let cols = [
                (Self::HEADER_TITLE, TableColumnFlags::WIDTH_STRETCH | TableColumnFlags::NO_REORDER | TableColumnFlags::NO_HIDE | TableColumnFlags::DEFAULT_SORT, 2.0),
                (Self::HEADER_NEARBY, TableColumnFlags::WIDTH_STRETCH /*| TableColumnFlags::DEFAULT_SORT*/, 0.5),
                (Self::HEADER_HIDDEN, TableColumnFlags::DEFAULT_SORT, 1.5),
                (Self::HEADER_INTERACT, TableColumnFlags::DEFAULT_SORT /*| TableColumnFlags::PREFER_SORT_DESCENDING*/, 1.0),
            ];
            for (i, (id, flags, weight)) in cols.into_iter().enumerate() {
                with_i18n!(id, |header| ui.table_setup_column_with(TableColumnSetup {
                    name: &header,
                    flags,
                    init_width_or_weight: 0.0,
                    user_id: UiId::Ptr(id.as_ptr() as *const _),
                }));
                #[cfg(todo = "unnecessary")]
                if ui.table_next_column() {
                    // table_headers_row() does this for us anyway
                    ui.table_header(header);
                }
            }
            ui.table_headers_row();
        }
        table_token
    }

    /// TODO? 3d trigger bounds could point ray downward for distance-sorted iter but we have other factors...
    fn update_sort(&mut self) {
        let sorting = self.ui.table_sort_specs_mut();
        let should_sort = sorting.as_ref().map(|s| s.should_sort());
        if should_sort.unwrap_or(false) || self.state.is_dirty() {
            let mut sort_desc = PoiInfoSorts::empty();
            let mut sorts = PoiInfoSorts::empty();
            if let Some(sorting) = &sorting {
                let specs = sorting.specs();
                let cols = [
                    (Self::HEADER_TITLE, PoiInfoSorts::DISTANCE),
                    (Self::HEADER_NEARBY, PoiInfoSorts::NEARBY),
                    (Self::HEADER_HIDDEN, PoiInfoSorts::VISIBLE),
                    (Self::HEADER_INTERACT, PoiInfoSorts::INTERACTIVE),
                ];
                for spec in specs.iter() {
                    let Some(&(_id, flag)) = cols.get(spec.column_idx() as usize) else {
                        log::debug!("BUG: spec idx");
                        continue
                    };
                    #[cfg(todo = "unnecessary")]
                    if flag.is_empty() {
                        continue
                    }
                    sorts.insert(flag);
                    #[cfg(todo)]
                    let order = spec.sort_order();
                    match spec.sort_direction() {
                        #[cfg(todo)]
                        None => sorts.remove(flag),
                        Some(TableSortDirection::Descending) => sort_desc.insert(flag),
                        _ => (),
                    }
                }
            } else {
                sorts = PoiInfoSorts::DEFAULT_UI
            };
            if self.state.prepare_sort(sorts) | should_sort.unwrap_or(false) {
                self.state.apply_sort(sorts, sort_desc)
            }
            if let (Some(mut sorting), Some(true)) = (sorting, should_sort) {
                sorting.set_sorted();
            }
        }
    }

    #[cfg(todo)]
    pub fn draw_poi_create(
        &mut self,
        ui: &Ui,
        machine: &mut RenderMachine,
        engine: &mut Option<&mut Engine>,
    ) {
        use std::collections::BTreeMap;
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
                let Some(map_info) = self.pack_maps.as_ref().and_then(|i| i.borrow().map_info.get(&map_path).cloned()) else { continue };
                let textures = map_info.info.pois().enumerate()
                    .filter_map(|(i, path)| active.pack.pois.get(path.path as usize)
                        .and_then(|poi| poi.attributes.render.as_ref())
                        .and_then(|render| render.poi.as_ref())
                        .and_then(|poi| poi.icon_file.as_ref())
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
}
impl super::PackElements {
    pub fn draw_interact(&mut self, ui: &Ui) {
        let mut draw = DrawPoiInfo::new(
            ui,
            &mut self.interact,
            self.pack_state.map_ref_as_slice(),
        );
        let was_context = draw.state.context.is_some();
        draw.draw();
        let context_id = "poi-context";
        if draw.state.context.is_some() && !was_context {
            ui.open_popup(context_id);
        }
        let popup = ui.begin_popup(context_id);
        let popup_drawn = popup.is_some();
        let mut act_cat = None;
        if let Some(_token) = popup {
            if let Some(context) = &mut draw.state.context {
                let mut menu = context.prepare_draw_menu(ui);
                menu.draw();
                context.finish_draw_menu(menu);
                act_cat = context.act_cat.take().map(|act| (context.loaded_path, act));
            } else {
                ui.close_current_popup();
            }
        }
        if draw.state.context.is_some() == was_context && popup_drawn != was_context {
            draw.state.context = None;
        }
        if let Some((lpath, (cat_path, act))) = act_cat {
            let category_path = lpath.root.root.rel(cat_path.path);
            let pack = self.pack_state.lookup_mut(&category_path.root);
            match (pack, act) {
                (Some(pack), CategoryAction::Open(open)) => {
                    pack.categories.update_open(cat_path, open);
                    if let Some(cats) = pack.state.info.info.as_ref().map(|i| &i.categories) {
                        for parent in cats.ancestors_of(cat_path) {
                            pack.categories.update_open(parent, open);
                        }
                    }
                    let _ = rt::send_alert(ui, "navigate over yourself, im sleepy");
                },
                (Some(..), act) => log::warn!("unexpected action {act:?}"),
                (None, _) => (),
            }
        }
    }
}
