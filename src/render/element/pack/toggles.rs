use {
    crate::{
        controller::pathing::{
            registry::UnloadedReason,
            visible::VisibilityFlags,
            PathingEvent,
        },
        exports::runtime::{
            self as rt,
            imgui::{self, Condition, MouseButton, TreeNode, TreeNodeFlags, TreeNodeToken, Ui, StyleVar},
        }, with_i18n,
    },
    super::{DrawCategoryHeader, DrawPackUnloaded, UiAction, PackElementState, CategoryInfo, CategoryCollectionState, DrawCategoryTooltip},
    taimi_hoard::loc::LocationRef,
    taimi_meta::packs::{CategoryIndex, CategoryPath, PackPath},
    taimi_pack::category::CategoryFlags,
};

pub struct DrawPackRoots<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'a PackElementState,
    pub categories: Option<&'a CategoryCollectionState>,
}
impl<'a, 'u> DrawPackRoots<'a, 'u> {
    pub fn draw(&mut self) {
        let _id = self.ui.push_id(self.state.ui_id());
        let categories = match self.state.unloaded.as_ref() {
            None if self.state.pack.is_some() => self.prepare_categories(),
            _reason => None,
        };
        match categories {
            Some(cats) =>
                self.draw_loaded(cats),
            None => self.draw_unloaded(),
        }
    }

    fn draw_loaded(&self, mut categories: DrawCategoryCollection<'a, 'u>) {
        let cats = self.state.info.info.as_ref().map(|i| &i.categories);
        let pseudo_root = cats.and_then(|cats| match &cats.roots[..] {
            &[root] => Some(root),
            _ => None,
        });
        let (mut pack_act, mut pack_toggle) = (None, None);
        self.ui.table_next_column();
        let token = if pseudo_root.is_none() {
            let mut header = self.prepare_header();
            let (act, token) = header.draw();
            pack_act = act;
            pack_toggle = header.draw_toggle_inline();
            token
        } else { None };
        if let Some(cats) = cats {
            if pseudo_root.is_some() || token.is_some() {
                for root in cats.root_paths() {
                    categories.draw_root(root, pseudo_root.is_some());
                    self.ui.table_next_column();
                }
                categories.pop_all();
            }
        }
        drop(token);
        if let Some(act) = pack_act {
            if act != UiAction::Hovered {
                log::error!("TODO: {} {act:?}", self.state.info);
            }
        }
        if let Some(act) = pack_toggle {
            log::error!("TODO: toggle {} {act}", self.state.info);
        }
        if let Some((path, open)) = categories.act_open {
            log::error!("TODO: open {path}");
        }
        if let Some((path, act)) = categories.act {
            if act != UiAction::Hovered {
                log::error!("TODO: {path} {act:?}");
            }
        }
    }
    fn draw_unloaded(&self) {
        self.ui.table_next_column();
        DrawPackUnloaded {
            ui: self.ui,
            state: self.state,
        }.draw();
    }
    fn prepare_header(&self) -> DrawCategoryHeader<'a, 'u> {
        DrawCategoryHeader {
            ui: self.ui,
            display_name: &self.state.display_name,
            open: false,
            open_cond: Condition::Once,
            toggle_state: !matches!(self.state.unloaded, Some(UnloadedReason::Disabled)),
            is_leaf: self.state.info.info.as_ref().map(|i| i.categories.roots.is_empty()),
            is_decorative: false,
            button_interact: None,
            allow_overlap: true,
        }
    }

    fn prepare_categories(&self) -> Option<DrawCategoryCollection<'a, 'u>> {
        self.categories.map(|state|
            DrawCategoryCollection::new(self.ui, state, self.state)
        )
    }
}

#[derive(Debug)]
pub struct DrawCategoryToggle<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub info: &'a CategoryInfo,
    pub pack_path: PackPath,
    pub category_path: CategoryPath,
    pub flags: CategoryFlags,
    pub toggle_state: VisibilityFlags,
    pub open_state: bool,
    pub is_lonely: bool,
    pub is_copyable: bool,
    pub has_children: bool,
    pub pseudo_root: bool,
}
impl<'a, 'u> DrawCategoryToggle<'a, 'u> {
    pub fn draw(&mut self) -> (Option<UiAction>, Option<TreeNodeToken<'u>>) {
        let mut header = self.prepare_header();
        let has_toggle = self.has_toggle();
        let mut toggle_checkbox = has_toggle.then_some(());
        let mut toggle_act = None;
        let mut checkbox_gap = None;
        if let Some(()) = (!self.pseudo_root).then(|| toggle_checkbox.take()).flatten() {
            let (act, gap) = header.draw_toggle_prefix();
            checkbox_gap = Some(gap);
            toggle_act = act;
        }

        let (header_action, header_token) = header.draw();

        drop(checkbox_gap);
        if let Some(()) = toggle_checkbox {
            toggle_act = header.draw_toggle_inline();
        } else if has_toggle {
            header.end_toggle_prefix();
        }
        if let Some(state) = toggle_act {
            self.act_toggle(Some(state));
        }
        if toggle_act.is_some() {
            self.toggle_state.toggle(VisibilityFlags::TOGGLE);
        }

        let act = DecorateCategoryHeader {
            ui: self.ui,
            info: self.info,
            was_hovered: matches!(header_action, Some(UiAction::Hovered)),
        }.decorate();
        let act = match (act, header_action) {
            (Some(UiAction::Hovered), Some(UiAction::Hovered)) => None,
            _ => header_action,
        };
        (act, header_token)
    }
    fn act_toggle(&self, state: Option<bool>) {
        PathingEvent::CategoryToggle(
            self.pack_path, self.category_path,
            state
        ).try_send();
    }

    pub(super) fn prepare_header(&self) -> DrawCategoryHeader<'a, 'u> {
        DrawCategoryHeader {
            ui: self.ui,
            open: self.open_state,
            toggle_state: self.toggle_state.is_visible(),
            open_cond: Condition::Always,
            display_name: self.info.display_name().unwrap_or(taimi_hoard::lazyfmt::UNAVAILABLE),
            is_leaf: match self.has_children {
                false if self.is_lonely => None,
                is_parent => Some(!is_parent),
            },
            is_decorative: self.flags.contains(CategoryFlags::SEPARATOR),
            button_interact: Some(self.is_copyable),
            allow_overlap: self.pseudo_root && self.has_toggle(),
        }
    }

    pub(super) fn has_toggle(&self) -> bool { !self.flags.contains(CategoryFlags::SEPARATOR) && !self.is_lonely }
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

impl super::PackElements {
    /// TODO
    pub fn can_collapse(&self) -> bool {
        self.pack_state.values().any(|p| p.categories.open_mask.flags.any())
    }
    pub fn can_expand(&self) -> bool {
        self.pack_state.values().any(|p| {
            let count = p.state.info.category_info()
                .map(|(cats, _)| cats.count())
                .unwrap_or(0);
            p.categories.open_mask.len() != count || p.categories.open_mask.flags.not_any()
        })
    }
    pub fn act_expand_all(&mut self) {
        for pack in self.pack_state.values_mut() {
            if let Some((cats, _)) = pack.state.info.category_info() {
                log::debug!("TODO: avoid opening filtered/hidden cats");
                pack.categories.open_mask.flags.fill(true);
                pack.categories.open_mask.extend_for(cats.count(), true);
            }
        }
    }
    pub fn act_collapse_all(&mut self) {
        for pack in self.pack_state.values_mut() {
            // TODO: avoid closing filtered ones too?
            pack.categories.open_mask.clear();
        }
    }
}
impl super::PackElement {
    pub fn draw(&self, ui: &Ui) {
        self.prepare_draw(ui).draw();
    }

    pub fn prepare_draw<'a, 'u>(&'a self, ui: &'a Ui<'u>) -> DrawPackRoots<'a, 'u> {
        DrawPackRoots {
            ui,
            state: &self.state,
            categories: Some(&self.categories),
        }
    }
}

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
