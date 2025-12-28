use {
    crate::{
        controller::{
            pathing::{
                registry::{PackCategoryFlags, PackInfoSignature, PackVecOf, UnloadedReason},
                shared::{PathingShared, SharedLoaderPacksInfo, SharedPackConfig, SharedPackInfo, SharedPackLoad, SharedPackLoaded},
                PathingEvent,
            },
            Controller,
        },
        render::RenderState,
        exports::runtime::{
            self as rt,
            imgui::{self, Condition, MouseButton, Selectable, TreeNode, TreeNodeFlags, Ui, TreeNodeToken},
        },
        with_i18n,
    },
    std::{collections::BTreeMap, fmt::{self, Write}, mem, sync::{Arc, Weak}}, taimi_hoard::{str_opt, str_opt_ref}, taimi_meta::packs::{CategoryPath, PackPath}, taimi_pack::{attributes::{self, AttrString, InteractionAttributes, MarkerAttributes}, category::{CategoryFlags, CategoryId}, Pack}, taimi_sync::watched::{watch, Watched, Watcher}
};

pub struct DrawPackRoots<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'a PackElementState,
}
impl DrawPackRoots<'_, '_> {
    pub fn draw(&mut self) {
        let _id = self.ui.push_id(self.state.ui_id());
        match self.state.unloaded.as_ref() {
            None if self.state.pack.is_some() => self.draw_loaded(),
            _reason => DrawPackUnloaded {
                ui: self.ui,
                state: self.state,
            }.draw(),
        }
    }

    fn draw_loaded(&self) {
        self.ui.text("TODO");
    }
}

#[derive(Debug)]
pub struct DrawPackRoot<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'a PackElementState,
}
impl DrawPackRoot<'_, '_> {
    pub fn draw(&mut self) {
    }
}

pub struct DrawPackUnloaded<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'a PackElementState,
}
impl DrawPackUnloaded<'_, '_> {
    fn draw(&mut self) {
        if let Some(UnloadedReason::Gravestone) = &self.state.unloaded {
            return
        }
        self.ui.popup("pack-context-unloaded", || {
            self.menu_unloaded();
        });
        let action = self.header_unloaded();
        match action {
            Some(UiAction::RIGHT_CLICK) =>
                self.ui.open_popup("pack-context-unloaded"),
            Some(UiAction::Primary) =>
                PathingEvent::ReloadPack(self.state.pack_path(), false).try_send(),
            _ => (),
        }
    }

    fn header_unloaded(&self) -> Option<UiAction> {
        let reason = self.state.unloaded.as_ref();
        let is_button = match reason {
            | Some(UnloadedReason::Loading | UnloadedReason::Pending)
            | None
            =>
                false,
            Some(..) => true,
        };
        let ui = self.ui;
        let node = TreeNode::new(&self.state.display_name)
            .flags(TreeNodeFlags::SPAN_AVAIL_WIDTH)
            .frame_padding(true)
            .tree_push_on_open(false)
            .opened(false, Condition::Always)
            .leaf(is_button)
            .push(ui);
        let hovered = ui.is_item_hovered();
        let pressed = is_button && ui.is_item_clicked();
        let open_context = ui.is_item_clicked_with_button(MouseButton::Right);

        match reason {
            Some(UnloadedReason::Disabled | UnloadedReason::Gravestone) => {
                ui.same_line();
                with_i18n!("disabled", |msg| ui.text(msg));
            },
            Some(UnloadedReason::Loading) => {
                ui.same_line();
                with_i18n!("loading", |msg| ui.text(msg));
            },
            Some(UnloadedReason::Pending) | None => {
                ui.same_line();
                with_i18n!("unloaded", |msg| ui.text(msg));
                if reason.is_some() && hovered {
                    with_i18n!("render-notice-gameplay", |msg| ui.tooltip_text(msg));
                }
            },
            Some(reason @ (UnloadedReason::LoadingFailed(..) | UnloadedReason::UnknownFormat)) => {
                ui.same_line();
                match reason {
                    UnloadedReason::UnknownFormat =>
                        with_i18n!("unknown", |msg| ui.text(msg)),
                    _ =>
                        with_i18n!("error", |msg| ui.text(msg)),
                }
                if hovered {
                    ui.tooltip_text(reason.to_string());
                }
            },
        }
        ui.table_next_column();
        let res = if let Some(node) = node {
            node.pop();
            !is_button || pressed
        } else {
            pressed
        };
        if open_context {
            Some(UiAction::Clicked(MouseButton::Right))
        } else if res {
            Some(UiAction::Primary)
        } else if hovered {
            Some(UiAction::Hovered)
        } else {
            None
        }
    }

    fn menu_unloaded(&self) {
        let action_remove = with_i18n!("remove", |label| Selectable::new(&label).build(self.ui));
        let action_reload = with_i18n!("reload-pack", |label| Selectable::new(&label).build(self.ui));
        if action_reload {
            PathingEvent::ReloadPack(self.state.pack_path(), true).try_send();
        } else if action_remove {
            PathingEvent::UnloadPack(self.state.pack_path(), true).try_send();
        }
    }
}

#[derive(Debug)]
pub struct DrawCategoryToggle<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub info: &'a CategoryInfo,
    pub state: &'a PackCategoryState,
    pub pack_path: PackPath,
    pub category_path: CategoryPath,
    pub flags: CategoryFlags,
    pub is_lonely: bool,
    pub is_copyable: bool,
    pub has_children: bool,
}
impl<'u> DrawCategoryToggle<'_, 'u> {
    pub fn draw(&mut self) -> (Option<UiAction>, Option<TreeNodeToken<'u>>) {
        let (header_action, header_token) = self.header().draw();
        let act = DecorateCategoryHeader {
            ui: self.ui,
            info: self.info,
            was_hovered: matches!(header_action, Some(UiAction::Hovered)),
        }.decorate();
        let mut act = match (act, header_action) {
            (Some(UiAction::Hovered), Some(UiAction::Hovered)) => None,
            _ => header_action,
        };
        if self.has_toggle() {
            draw_toggle();
        }
        (act, header_token)
    }

    fn header(&self) -> DrawCategoryHeader<'_, 'u> {
        DrawCategoryHeader {
            ui: self.ui,
            open: self.state.open,
            open_cond: Condition::Always,
            display_name: self.state.display_name(),
            is_leaf: match self.has_children {
                false if self.is_lonely => None,
                is_parent => Some(!is_parent),
            },
            is_decorative: self.flags.contains(CategoryFlags::SEPARATOR),
            button_interact: Some(self.is_copyable),
        }
    }

    fn has_toggle(&self) -> bool { !self.flags.contains(CategoryFlags::SEPARATOR) && !self.is_lonely }
}

#[derive(Debug)]
pub struct DrawCategoryHeader<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub display_name: &'a str,
    pub open: bool,
    pub open_cond: Condition,
    pub is_leaf: Option<bool>,
    pub is_decorative: bool,
    pub button_interact: Option<bool>,
}
impl<'u> DrawCategoryHeader<'_, 'u> {
    pub fn draw(
        &mut self,
    ) -> (Option<UiAction>, Option<TreeNodeToken<'u>>) {
        let mut unbuilt = TreeNode::new(self.display_name);
        match self.button_interact {
            Some(false) if self.is_decorative || self.is_leaf.unwrap_or(true) =>
                unbuilt = unbuilt.flags(TreeNodeFlags::SPAN_AVAIL_WIDTH),
            None =>
                unbuilt = unbuilt.allow_item_overlap(true),
            _ => (),
        }
        unbuilt = unbuilt.frame_padding(true)
            .tree_push_on_open(false)
            .leaf(self.is_leaf.unwrap_or(true));
        let mut framed = false;
        match self.is_leaf {
            Some(false) => if !self.is_decorative {
                framed = true;
            },
            Some(true) =>
                unbuilt = unbuilt.bullet(true),
            None => (),
        }
        if self.is_decorative {
            match self.is_leaf {
                Some(true) =>
                    unbuilt = unbuilt.selected(true),
                Some(false) => {
                    framed = true;
                    unbuilt = unbuilt.bullet(true);
                },
                None => {
                    // needs to stand out more among branches too..?
                    // TODO: less necessary once checkboxes become left-aligned
                    unbuilt = unbuilt.selected(true);
                    // would use this but leaf|framed results in strange text alignment...
                    // framed = true
                },
            }
        }
        if framed {
            unbuilt = unbuilt
                .framed(true)
                .opened(self.open, self.open_cond);
        }
        let tree_token = unbuilt.push(self.ui);
        let action = match (self.open_cond, self.open, &tree_token) {
            (Condition::Always, open, token) if framed && open != token.is_some() =>
                Some(UiAction::Primary),
            _ if self.ui.is_item_clicked_with_button(MouseButton::Right) =>
                Some(UiAction::RIGHT_CLICK),
            #[cfg(todo)]
            _ if !framed && self.ui.is_item_clicked() => Some(UiAction::Primary),
            _ if !framed && self.ui.is_item_clicked() => Some(UiAction::LEFT_CLICK),
            _ if self.ui.is_item_hovered() =>
                Some(UiAction::Hovered),
            _ => None,
        };
        (action, tree_token)
    }
}
/// Draw buttons and tooltips and stuff on top
#[derive(Debug)]
pub struct DecorateCategoryHeader<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub info: &'a CategoryInfo,
    pub was_hovered: bool,
}
impl<'u> DecorateCategoryHeader<'_, 'u> {
    pub fn decorate(&mut self) -> Option<UiAction> {
        let mut show_tip = self.was_hovered;
        let mut act = None;
        if let Some((copy_value, copy_message)) = self.info.copyable() {
            self.ui.same_line();
            if with_i18n!("copy", |label| self.ui.small_button(&label)) {
                Self::copy_copyable(self.ui, copy_value, copy_message);
            } else if self.ui.is_item_hovered() {
                act = Some(UiAction::Hovered);
                DrawCategoryTooltip {
                    ui: self.ui,
                    info: self.info,
                    include_copyable: true,
                    display_name_visible: true,
                    tooltip: self.info.tooltip.borrowed(),
                }.draw();
                show_tip = false;
            }
        }
        let tooltip = show_tip.then_some(self.info.tooltip());
        if let Some(Some(tooltip)) = tooltip {
            act = Some(UiAction::Hovered);
            DrawCategoryTooltip {
                ui: self.ui,
                info: self.info,
                include_copyable: false,
                display_name_visible: true,
                tooltip,
            }.draw();
        }
        act
    }

    fn copy_copyable(ui: &Ui, copy_value: &str, copy_message: Option<&str>) {
        ui.set_clipboard_text(copy_value);
        if let Some(copy_message) = copy_message {
            let _ = rt::send_alert(ui, copy_message);
        }
    }
}

pub struct DrawCategoryTooltip<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub info: &'a CategoryInfo,
    pub tooltip: PackTooltipRef<'a>,
    pub display_name_visible: bool,
    pub include_copyable: bool,
}
impl DrawCategoryTooltip<'_, '_> {
    const NAME_TEMPLATE: &'static str = "Generic Copyable Marker Name";
    pub fn draw(&mut self) {
        let title_template = self.info.display_name()
            .and_then(str_opt)
            .unwrap_or(Self::NAME_TEMPLATE);
        Self::draw_tooltip(self.ui, title_template, move || self.draw_contents());
    }

    pub fn draw_contents(&mut self) {
        let desc = self.tooltip.description();
        let title = match self.tooltip.title() {
            Some(title) if self.display_name_visible && self.info.display_name().unwrap_or("").starts_with(title) =>
                None,
            title => title,
        };

        if let Some(title) = title {
            let _title_font = desc.map(|_| RenderState::push_font("big", self.ui));
            self.ui.text(title);
        }

        if let Some(tip) = desc {
            self.ui.text_wrapped(tip);
        }

        let copyable = self.include_copyable.then(|| self.info.copyable()).flatten();
        if let Some((copy_value, copy_message)) = copyable {
            Self::draw_tooltip_copyable(self.ui, copy_value, copy_message);
        }
    }

    fn draw_tooltip<F: FnOnce()>(ui: &Ui, title_template: &str, f: F) {
        let _id = ui.push_id("category_tooltip");
        let [minwidth, lineheight] = ui.calc_text_size(title_template);
        unsafe {
            imgui::sys::igSetNextWindowSize([0.0, lineheight * 1.5].into(), Condition::Appearing as _);
        };
        let _size = ui.push_style_var(imgui::StyleVar::WindowMinSize([minwidth, lineheight]));
        ui.tooltip(|| {
            {
                let _padding = ui.push_style_var(imgui::StyleVar::ItemSpacing([f32::EPSILON, f32::EPSILON]));
                ui.dummy([minwidth, f32::EPSILON]);
            }
            f()
        })
    }

    /// since these aren't intended to be displayed, there's no canon name to use...
    /// if it looks like more than just a location link, we'll try to preview it
    fn copyable_value_has_message(copy_value: &str) -> bool {
        if !copy_value[..].starts_with('[') || !copy_value.ends_with(']') {
            return true
        }
        false
    }

    fn draw_tooltip_copyable(ui: &Ui, copy_value: &str, copy_message: Option<&str>) {
        let copy_message = match copy_message {
            // TODO: consider stubbing out generic success messages
            // like "x has been copied to your clipboard"
            m => m,
        };
        if let Some(copy_message) = copy_message {
            ui.text_wrapped(copy_message);
        } else if Self::copyable_value_has_message(copy_value) {
            ui.text_wrapped(&format!("\"{copy_value}\""));
        }
    }
}

#[cfg(todo)]
impl PackElement {
    fn draw_category(&mut self, ui: &Ui, info: &PackInfo, (cat_path, map_path): (CategoryPath<PackPath>, Option<PackMapPath>), display_name: &str) {
        if !self.category_visible(cat_path, info) {
            return
        }
        self.draw_category_item(ui, info, (cat_path, map_path), &display_name)
    }

    fn draw_category_item(&mut self, ui: &Ui, info: &PackInfo, (cat_path, map_path): (CategoryPath<PackPath>, Option<PackMapPath>), display_name: &str) {
        let open = self.is_open(cat_path.root, Some(Locator::with_path(cat_path.path)));
        let unscoped = Locator::with_path(cat_path.path);
        let cat_info = info.categories.info_of(unscoped);
        let cat_lonely = info.categories.lonely.contains(unscoped);
        let is_leaf = cat_info.as_ref().map(|cat| cat.child().is_none());
        let is_leaf = match is_leaf {
            Some(true) if cat_lonely => None,
            Some(l) => Some(l),
            None => {
                log::error!("invalid category {cat_path}???");
                Some(true)
            },
        };
        let is_decorative = info.categories.separators.contains(cat_path);
        let is_copyable = info.categories.copyable.contains(cat_path);
        let state = match (is_decorative, cat_lonely) {
            (false, false) => Some(self.item_config_toggle(cat_path, info)),
            _ => None,
        };
        let _id = self.category_header_prestart(ui, Some(cat_path), &display_name);
        if let Some(state) = state {
            ui.unindent();
            if let Some(toggled) = Self::category_toggle(ui, state) {
                self.commit_state(cat_path, toggled);
            }
            ui.same_line();
        }
        let tree = self.category_header_start(ui, Some(cat_path), &display_name, open, is_leaf, is_decorative, Some(is_copyable));
        if !self.act_selected_category_open && ui.is_item_clicked_with_button(MouseButton::Right) {
            self.act_selected_category = Some((cat_path, state, open, is_copyable));
            self.act_selected_category_open = true;
        }
        self.category_header_decorate(ui, info, cat_path);

        let now_open = tree.is_some();
        if !is_leaf.unwrap_or(false) && open != now_open {
            let open = self.open_items.entry(cat_path.root).or_default();
            Self::set_bit(open, None, cat_path.path as usize, now_open);
        }

        Self::category_name_finish(ui);

        if state.is_some() {
            ui.indent();
        }

        if !is_leaf.unwrap_or(true) && tree.is_some() {
            ui.indent();
            self.draw_children(ui, info, (cat_path, map_path));
            ui.unindent();
        }
        Self::category_finish(ui, tree);
    }

    pub fn draw_children(&mut self, ui: &Ui, info: &PackInfo, (cat_path, map_path): (CategoryPath<PackPath>, Option<PackMapPath>)) {
        for child in info.categories.children_of(Locator::with_path(cat_path.path)) {
            let child_path = child.pivot(cat_path.root);
            let display_name = {
                Self::category_display_name(&self.pack_loader_data, &mut self.category_names, info, child_path)
                .map(|name| name.to_owned())
            }.unwrap_or_else(|| format!("#{}", child.path));
            self.draw_category(ui, info, (child_path, map_path), &display_name);
        }
    }

    pub fn category_header_prestart<'u>(
        &mut self,
        ui: &'u Ui,
        path: Option<CategoryPath<PackPath>>,
        display_name: &str,
    ) -> IdStackToken<'u> {
        let push_token = match path {
            Some(path) => ui.push_id(path.path as i32 ^ ((path.root.path as i32) << 20)),
            _ => ui.push_id(display_name),
        };

        push_token
    }

    #[cfg(todo)]
    fn set_open_item(&mut self, info: &PackInfo, path: CategoryPath<PackPath>) {
        let open = self.open_items.entry(path.root).or_default();
        let mut next = Some(CategoryPath::with_path(path.path));
        loop {
            let Some(cat) = next else { break };
            Self::set_bit(open, None, cat.path as usize, true);
            next = info.categories.parent_of(cat);
        }
    }

    pub fn category_name_finish<'u>(
        ui: &'u Ui,
    ) {
        ui.table_next_column();
    }
    pub fn category_toggle<'u>(
        ui: &'u Ui,
        mut state: bool,
    ) -> Option<bool> {
        let mut toggled = None;
        if ui.checkbox("", &mut state) {
            toggled = Some(state);
        }
        toggled
    }
    pub fn category_finish<'u>(
        _ui: &'u Ui,
        tree: Option<TreeNodeToken<'u>>,
    ) {
        drop(tree);
    }

    pub(super) fn draw_title_text_truncate(ui: &Ui, text: &str) {
        let header = text.split_once(['\n', '.'])
            .map(|(header, _rest)| header)
            .unwrap_or(text);
        let header = match header.len() {
            0 => text,
            _ => header,
        };
        #[cfg(todo)]
        let sz = ui.calc_text_size(header);
        let _wrap = ui.push_text_wrap_pos_with_pos(-1.0);
        ui.text_wrapped(header);
    }
}

#[derive(Debug, Default)]
pub struct PackElements {
    pub shared: Option<Arc<PathingShared>>,
    pub packs_rx: Watcher<SharedLoaderPacksInfo>,
    pub pack_state: PackVecOf<PackElementState>,
}
impl PackElements {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pre_draw(&mut self) {
        if self.shared.is_none() {
            Controller::with_sender(|s| if let Some(pathing) = &s.pathing {
                self.shared = Some(pathing.shared.clone());
            });
            if let Some(shared) = &self.shared {
                self.packs_rx.restart_watching(&shared.packs.packs);
            }
        }
        let Some(_shared) = &self.shared else { return };
        if let Some(packs) = self.packs_rx.try_read_if_changed() {
            log::debug!("TODO: packs damage report and setup etc");
            let new_packs = packs.iter().skip(self.pack_state.len());
            for (_path, pack) in new_packs {
                self.pack_state.data.push(PackElementState::new(&pack));
            }
        }
    }
}

#[derive(Debug)]
pub struct PackElementState {
    damage: PackDamageReport,
    pub info: Arc<SharedPackInfo>,
    pub config: Watched<SharedPackConfig>,
    pub loaded: watch::Receiver<SharedPackLoaded>,
    pub unloaded: Option<UnloadedReason>,
    pub pack: Option<Weak<Pack>>,

    pub category_flags: Option<PackCategoryFlags>,
    pub display_name: String,
    pub id_name: String,
}
impl PackElementState {
    pub fn new(pack: &SharedPackLoad) -> Self {
        let mut loaded = pack.loaded.subscribe();
        loaded.mark_changed();
        Self {
            info: pack.info.clone(),
            config: Watched::start_watching(&pack.config),
            loaded,
            damage: PackDamageReport {
                info: Some(pack.info.sig),
                .. PackDamageReport::ALL
            },
            unloaded: None,
            pack: None,
            category_flags: None,
            display_name: String::new(),
            id_name: String::new(),
        }
    }

    pub fn pre_draw(&mut self, visibility: PackVisibility) -> PackDamageReport {
        let mut damage = mem::take(&mut self.damage);
        if let Some(_config) = self.config.try_read_if_changed() {
            damage.config = true;
        }
        if self.loaded.has_changed().unwrap_or(false) {
            let loaded = self.loaded.borrow_and_update();
            damage.loaded = true;
            self.unloaded = loaded.unloaded.clone();
            self.pack = loaded.pack.as_ref().map(Arc::downgrade);
        }
        if let PackVisibility::Closed = visibility {
            self.cleanup_cache();
            return damage
        }
        if damage.info.is_some() {
            self.category_flags = None;
            self.display_name.clear();
            self.id_name.clear();
        }
        if let PackVisibility::Visible = visibility {
            if self.display_name.is_empty() {
                let _ = write!(&mut self.display_name, "{}", self.info);
            }
        }
        if self.id_name.is_empty() {
            let root = || self.info.info.as_ref().and_then(|i| match i.roots.iter().next() {
                Some(root) if i.roots.len() == 1 =>
                    Some(root),
                _ => None,
            });
            if let Some(datasource) = self.info.datasource.as_ref() {
                // TODO: just ref this inline self.ui_id() instead
                self.id_name.push_str(&datasource.path[..]);
            } else if let Some(root) = root() {
                self.id_name.push_str(root.id.as_str());
            } else if let Some(fname) = self.info.path.file_name() {
                let _ = write!(&mut self.id_name, "{}", fname.display());
            }
        }

        damage
    }
    /// deallocate cached data commonly useful for displaying UI
    fn cleanup_cache(&mut self) {
        self.display_name = String::new();
        self.id_name = String::new();
        self.category_flags = None;
    }

    pub fn ui_id(&self) -> imgui::Id<'_> {
        str_opt_ref(&self.id_name)
            .or(str_opt_ref(&self.display_name))
            .map(imgui::Id::Str)
            .unwrap_or(imgui::Id::Int(self.info.index.path as _))
    }
    pub fn pack_path(&self) -> PackPath {
        self.info.index
    }
}

pub struct CategoryCollectionState {
    pub categories: BTreeMap<CategoryPath, PackCategoryState>
}

#[derive(Debug, Default)]
pub struct CategoryInfo {
    pub id: Option<CategoryId>,
    pub display_name: Option<Arc<str>>,
    pub tooltip: PackTooltip,
    pub interaction: Option<Arc<InteractionAttributes>>,
}
impl CategoryInfo {
    pub const EMPTY: Self = Self {
        id: None,
        display_name: None,
        tooltip: PackTooltip::EMPTY,
        interaction: None,
    };

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_ref().map(|n| &n[..])
    }
    pub fn tooltip(&self) -> Option<PackTooltipRef<'_>> {
        self.tooltip.get().map(PackTooltip::borrowed)
    }

    pub fn copyable(&self) -> Option<(&str, Option<&str>)> {
        let interaction = self.interaction.as_ref()?;
        let copy_value = interaction.copy_value.as_ref().map(|c| &c[..]).and_then(str_opt_ref)?;
        let copy_message = interaction.copy_message.as_ref().map(|c| &c[..]).and_then(str_opt_ref);
        Some((copy_value, copy_message))
    }
}

#[derive(Debug)]
pub struct PackCategoryState {
    pub open: bool,
    pub display_name: String,
}
impl PackCategoryState {
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackTooltip {
    pub title: Option<AttrString>,
    pub description: Option<AttrString>,
}
impl PackTooltip {
    pub const EMPTY: Self = Self {
        title: None,
        description: None,
    };

    pub fn new<S: Into<Box<str>>>(title: Option<S>, description: Option<S>) -> Self {
        Self {
            title: title.map(attributes::string_into),
            description: description.map(attributes::string_into),
        }
    }
    pub fn from_attrs(attrs: &MarkerAttributes) -> Self {
        Self {
            title: attrs.tip_name.clone(),
            description: attrs.tip_description.clone(),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self { title: None, description: None })
    }
    pub fn get(&self) -> Option<&Self> {
        (!self.is_empty()).then_some(self)
    }

    pub fn borrowed(&self) -> PackTooltipRef<'_> {
        PackTooltipRef::from_tip(self)
    }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackTooltipRef<'a> {
    pub title: &'a str,
    pub description: &'a str,
}
impl<'a> PackTooltipRef<'a> {
    pub const EMPTY: Self = Self {
        title: "",
        description: "",
    };

    #[inline]
    pub const fn new(title: &'a str, description: &'a str) -> Self {
        Self { title, description }
    }
    #[inline]
    pub const fn with_title(title: &'a str) -> Self {
        Self::new(title, "")
    }

    #[inline]
    pub fn from_tip(tip: &'a PackTooltip) -> Self {
        Self {
            title: tip.title.as_ref().map(|n| &n[..]).unwrap_or(""),
            description: tip.description.as_ref().map(|n| &n[..]).unwrap_or(""),
        }
    }
    #[inline]
    pub fn from_attrs(attrs: &'a MarkerAttributes) -> Self {
        Self {
            title: attrs.tip_name.as_ref().map(|n| &n[..]).unwrap_or(""),
            description: attrs.tip_description.as_ref().map(|n| &n[..]).unwrap_or(""),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self { title: "", description: "" })
    }

    pub fn title(&self) -> Option<&'a str> {
        str_opt(self.title)
    }
    pub fn description(&self) -> Option<&'a str> {
        str_opt(self.description)
    }
    fn to_tip(&self) -> PackTooltip {
        PackTooltip::new(self.title(), self.description())
    }
}
#[cfg(todo)]
impl ToOwned for PackTooltipRef<'_> {
    type Owned = PackTooltip;

    fn to_owned(&self) -> Self::Owned {
        PackTooltip::new(self.title().map(attributes::string_into), self.description().map(attributes::string_into))
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackDamageReport {
    info: Option<PackInfoSignature>,
    config: bool,
    loaded: bool,
}
impl PackDamageReport {
    pub const CLEAN: Self = Self {
        info: None,
        config: false,
        loaded: false,
    };
    pub const ALL: Self = Self {
        info: Some(PackInfoSignature::EMPTY),
        config: true,
        loaded: true,
    };
}
#[derive(Debug, Clone, Default)]
pub enum PackVisibility {
    Visible,
    /// would be visible, but scrolled off-screen
    Offset,
    /// relevant and available (can be navigated to),
    /// usually inside a collapsed tree node or menu
    #[default]
    Pending,
    /// window closed
    Closed,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[must_use]
pub enum UiAction {
    Clicked(MouseButton),
    /// toggled/selected/committed/etc
    Primary,
    Hovered,
    Dismissed,
}
impl UiAction {
    pub const LEFT_CLICK: Self = Self::Clicked(MouseButton::Left);
    pub const RIGHT_CLICK: Self = Self::Clicked(MouseButton::Right);
    #[cfg(todo = "unused")]
    pub const MIDDLE_CLICK: Self = Self::Clicked(MouseButton::Middle);
}
