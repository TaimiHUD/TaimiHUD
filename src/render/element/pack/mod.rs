use {
    crate::{
        controller::{
            pathing::{
                registry::{PackCategory, PackCategoryFlags, PackCategoryInfo, PackInfoSignature, PackVecOf, UnloadedReason}, shared::{PathingShared, SharedLoaderPacksInfo, SharedPackConfig, SharedPackInfo, SharedPackLoad, SharedPackLoaded}, visible::VisibilityFlags, PathingEvent
            },
            Controller,
        }, exports::runtime::imgui::{self, Condition, MouseButton, Selectable, TreeNode, TreeNodeFlags, TreeNodeToken, Ui, StyleVar},
        render::RenderState, with_i18n
    },
    std::{collections::BTreeMap, fmt::{self, Write}, iter, mem, sync::{Arc, Weak}},
    taimi_hoard::{flags::BitSet, str_opt, str_opt_ref, loc::{LocationRef, LocationGet}}, taimi_meta::packs::{CategoryIndex, CategoryPath, PackPath}, taimi_pack::{attributes::{self, AttrString, InteractionAttributes, MarkerAttributes}, category::{Category, CategoryFlags, CategoryId}, Pack}, taimi_sync::watched::{watch, Watched, Watcher},
    taimi_sync::arcs::ArcPtrCmp,
};
pub use self::{
    categories::{DrawCategoryHeader, DrawCategoryTooltip, DrawPackUnloaded, CategoryInfo, DrawCategoryCollection, DrawCategoryCollectionTree, CategoryCollectionState, CategoryAction, CategoryActionSlot},
    menu::{DrawCategoryMenu, DrawCategoryCollectionMenu},
    toggles::{DrawPackRoots, DrawCategoryToggle, DecorateCategoryHeader},
};

mod categories;
mod menu;
mod toggles;

#[derive(Debug, Default)]
pub struct PackElements {
    pub shared: Option<Arc<PathingShared>>,
    pub packs_rx: Watcher<SharedLoaderPacksInfo>,
    pub pack_state: PackVecOf<PackElement>,
}
impl PackElements {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pre_draw(&mut self, visibility: PackVisibility) {
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
            let mut packs_iter = packs.values();
            for (pack_state, pack) in self.pack_state.values_mut().zip(&mut packs_iter) {
                let prev_info_sig = pack_state.state.info.sig;
                if ArcPtrCmp::from_mut(&mut pack_state.state.info).clone_from_arc(&pack.info) {
                    pack_state.state.damage.info = Some(prev_info_sig);
                }
            }
            // any remainding are new packs...
            for pack in packs_iter {
                self.pack_state.data.push(PackElement::new(&pack));
            }
        }
        for pack in self.pack_state.values_mut() {
            pack.pre_draw(visibility);
        }
    }
    pub fn draw(&mut self, ui: &Ui) {
        for pack in self.pack_state.values_mut() {
            pack.draw(ui);
        }
    }

    pub fn any_loaded(&self) -> bool {
        self.pack_state.values().any(|p| p.state.info.info.is_some())
    }
}
#[derive(Debug)]
pub struct PackElement {
    pub state: PackElementState,
    pub categories: CategoryCollectionState,
}
impl PackElement {
    pub fn new(pack: &SharedPackLoad) -> Self {
        Self {
            state: PackElementState::new(pack),
            categories: CategoryCollectionState::default(),
        }
    }

    pub fn pre_draw(&mut self, visibility: PackVisibility) {
        let damage = self.state.pre_draw(visibility);
        let category_visibility = match visibility {
            PackVisibility::Visible if !self.any_roots_open() =>
                PackVisibility::Pending,
            v => v,
        };
        self.categories.pre_draw(&self.state, &damage, category_visibility);
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

    /// TODO: deleteme (info.categories is relied on too heavily atm)
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
                info: Some(PackInfoSignature::EMPTY),
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
        self.damage.visibility = Some(visibility);
        if damage.visibility == Some(visibility) {
            damage.visibility = None;
        }
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
        if self.id_name.is_empty() && self.info.datasource.is_none() {
            if let Some(fname) = self.info.path.file_name() {
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
        let id_name =
            str_opt_ref(&self.id_name)
            .or_else(|| self.info.datasource.as_ref().map(|ds| &ds.path[..]));
        //let id_name = id_name.or(str_opt_ref(&self.display_name));
        id_name.map(imgui::Id::Str)
            .unwrap_or(imgui::Id::Int(self.info.index.path as _))
    }
    pub fn pack_path(&self) -> PackPath {
        self.info.index
    }

    pub fn pack_data(&self) -> Option<Arc<Pack>> {
        self.pack.as_ref().and_then(Weak::upgrade)
    }
}

/// redo generic-y
#[cfg(todo)]
pub struct DrawCategoryCollection<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'a CategoryCollectionState,
    pub pack: &'a PackElementState,
    pub path_stack: Vec<CategoryPath>,
    /// TODO: these are ZSTs, just make a collection type for this?
    pub id_stack: Vec<imgui::IdStackToken<'ui>>,
    /// XXX: same as [self.id_stack]
    pub node_stack: Vec<Option<TreeNodeToken<'ui>>>,
    pub act_open: Option<(CategoryPath, bool)>,
    pub act: Option<(CategoryPath, UiAction)>,
}
#[cfg(todo)]
impl<'a, 'u> DrawCategoryCollection<'a, 'u> {
    pub fn new(ui: &'a Ui<'u>, state: &'a CategoryCollectionState, pack: &'a PackElementState) -> Self {
        Self {
            ui,
            state,
            pack,
            path_stack: Vec::new(),
            id_stack: Vec::new(),
            node_stack: Vec::new(),
            act_open: None,
            act: None,
        }
    }

    const DEPTH_LIMIT: usize = 78;
    pub fn draw_root(&mut self, path: CategoryPath, pseudo_root: bool) {
        self.push_and_draw(path, Some(pseudo_root));
        let cats = self.pack.info.info.as_ref().map(|i| &i.categories);

        while self.node_contents_visible() {
            if self.path_stack.len() >= Self::DEPTH_LIMIT {
                log::warn!("category nesting limit reached");
                break
            }
            let Some(cats) = cats else { continue };
            let cat_info = cats.all().lookup_ref(&self.category_path());
            let mut next = None;
            let popping = if let Some(child) = cat_info.and_then(|c| c.child()) {
                next = Some(child);
                false
            } else if let Some(sibling) = cat_info.and_then(|c| c.sibling()) {
                next = Some(sibling);
                self.ui.table_next_column();
                true
            } else { true };
            if popping && self.pop_to(path).is_none() {
                break
            }
            if let Some(next) = next.map(CategoryPath::with_path) {
                self.draw_one(next);
            }
        }
        while let Some(..) = self.pop_to(path) {}
        #[cfg(todo)]
        if draw_footer_idk {
            self.pop_draw(token);
            footer_stuff();
        }
        self.pop();
    }
    pub fn draw_one(&mut self, path: CategoryPath) -> Option<()> {
        let token = self.push_and_draw(path, None);
        token
    }

    fn push_and_draw(&mut self, path: CategoryPath, pseudo_root: Option<bool>) -> Option<()> {
        self.push(path);
        let mut toggle = self.prepare_toggle(path, pseudo_root);
        let (act, token) = toggle.draw();
        let res = token.as_ref().map(drop);
        self.set_draw_node(token);
        match act {
            Some(UiAction::Primary) => {
                if self.act_open.is_none() {
                    self.act_open = Some((path, toggle.toggle_state.is_visible()));
                } else {
                    log::debug!("DELETEME: category action already set? {:?}", self.act);
                }
            },
            Some(act) => {
                if self.act.is_none() {
                    self.act = Some((path, act));
                } else {
                    log::debug!("DELETEME: category action already set? {:?}", self.act);
                }
            },
            None => (),
        }
        res
    }

    fn prepare_toggle(&self, path: CategoryPath, pseudo_root: Option<bool>) -> DrawCategoryToggle<'a, 'u> {
        let cats = self.pack.info.info.as_ref().map(|i| &i.categories);
        let cat = self.pack.category_info(path);
        let mut vis = VisibilityFlags::TOGGLES;
        let info = match self.state.categories.get(&path) {
            Some(info) => {
                vis = info.visibility;
                info
            },
            None =>&CategoryInfo::EMPTY,
        };
        vis ^= self.pack.category_visibility_deviation(path);
        
        DrawCategoryToggle {
            ui: self.ui,
            info,
            pack_path: self.pack.pack_path(),
            category_path: path,
            flags: self.pack.category_flags(path),
            toggle_state: vis,
            open_state: self.state.open_mask.contains(path),
            is_lonely: match pseudo_root {
                Some(..) => false,
                None => cats.map(|cats| cats.lonely.contains(path))
                    .unwrap_or(false),
            },
            is_copyable: info.copyable().is_some(),
            has_children: cat.map(|cat| cat.child().is_some()).unwrap_or(true),
            // caller should decide this...
            #[cfg(todo)]
            pseudo_root: cats.map(|cats| cats.root_paths().all(|p| p == path))
                .unwrap_or(false),
            pseudo_root: pseudo_root.unwrap_or(false),
        }
    }

    /// in case we want to keep id token active for footer/menus/etc
    pub fn pop_draw(&mut self) {
        if self.pop_node().is_some() {
            self.node_stack.push(None);
        }
    }
    fn pop_node(&mut self) -> Option<Option<TreeNodeToken<'u>>> {
        let token = self.node_stack.pop();
        if let Some(Some(..)) = &token {
            self.ui.unindent();
        }
        token
    }

    pub fn pop(&mut self) -> Option<CategoryPath> {
        let path = self.path_stack.pop();
        drop(self.pop_node());
        drop(self.id_stack.pop());
        path
    }
    pub fn pop_all(&mut self) {
        while self.pop().is_some() {}
    }
    pub fn pop_to(&mut self, root: CategoryPath) -> Option<CategoryPath> {
        let path = self.path_stack.pop_if(|p| *p != root);
        if path.is_some() {
            drop(self.pop_node());
            drop(self.id_stack.pop());
        }
        if path.is_none() && self.path_stack.is_empty() {
            log::error!("cat stack missing {root}?");
        }
        path
    }
    pub fn push(&mut self, path: CategoryPath) {
        self.path_stack.push(path);
        let id = self.ui.push_id(self.category_info().ui_id(path));
        self.id_stack.push(id);
        self.node_stack.push(None);
    }
    pub fn set_draw_node(&mut self, token: Option<TreeNodeToken<'u>>) {
        match self.node_stack.pop_if(|n| n.is_none()) {
            Some(..) => {
                if token.is_some() {
                    self.ui.indent();
                }
                self.node_stack.push(token);
            },
            None =>
                log::error!("cat node tree({}) inconsistent", self.node_stack.len()),
        }
    }

    pub fn root_path(&self) -> CategoryPath {
        self.path_stack.first().copied().unwrap_or(CategoryPath::with_path(CategoryIndex::MAX))
    }
    pub fn category_path(&self) -> CategoryPath {
        self.path_stack.last().copied().unwrap_or(CategoryPath::with_path(CategoryIndex::MAX))
    }
    #[cfg(todo)]
    pub fn category_visibility_defaults(&self) -> VisibilityFlags {
        self.state.categories.get(&self.category_path())
            .map(|info| info.visibility)
            .unwrap_or_else(|| VisibilityFlags::from_category_flags(self.pack.category_flags()))
    }
    pub fn category_info(&self) -> &'a CategoryInfo {
        self.state.categories.get(&self.category_path())
            .unwrap_or(&CategoryInfo::EMPTY)
    }

    pub fn node_contents_visible(&self) -> bool {
        self.node_stack.last().map(Option::is_some).unwrap_or(false)
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
    visibility: Option<PackVisibility>,
    info: Option<PackInfoSignature>,
    config: bool,
    loaded: bool,
}
impl PackDamageReport {
    pub const CLEAN: Self = Self {
        visibility: None,
        info: None,
        config: false,
        loaded: false,
    };
    pub const ALL: Self = Self {
        visibility: Some(PackVisibility::Pending),
        info: Some(PackInfoSignature::EMPTY),
        config: true,
        loaded: true,
    };
}
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackVisibility {
    Visible = 4,
    /// would be visible, but scrolled off-screen
    Offset = 3,
    /// relevant and available (can be navigated to),
    /// usually inside a collapsed tree node or menu
    #[default]
    Pending = 2,
    /// window closed
    Closed = 1,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[must_use]
pub enum UiAction {
    Hovered,
    Dismissed,
    /// toggled/selected/committed/etc
    Primary,
    Clicked(MouseButton),
}
impl UiAction {
    pub const LEFT_CLICK: Self = Self::Clicked(MouseButton::Left);
    pub const RIGHT_CLICK: Self = Self::Clicked(MouseButton::Right);
    #[cfg(todo = "unused")]
    pub const MIDDLE_CLICK: Self = Self::Clicked(MouseButton::Middle);
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
