use {
    super::{
        CategoryAction,
        CategoryActionSlot,
        CategoryCollectionState,
        CategoryInfo,
        DrawCategoryCollection,
        DrawCategoryCollectionTree,
        DrawCategoryHeader,
        DrawCategoryTooltip,
        DrawPackUnloaded,
        PackAction,
        PackActionSlot,
        PackElement,
        PackElementState,
        UiAction,
    },
    crate::{
        controller::pathing::registry::UnloadedReason,
        exports::runtime::imgui::{self, Condition, TreeNodeToken, Ui},
        with_i18n,
    },
    taimi_meta::packs::{CategoryPath, PackPath, VisibilityFlags},
    taimi_pack::category::CategoryFlags,
};

pub struct DrawPackRoots<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'a PackElementState,
    pub categories: Option<&'a CategoryCollectionState>,
    pub act_cat: CategoryActionSlot,
    pub act_pack: PackActionSlot,
    pub last_menu_open: Option<CategoryPath>,
}
impl<'a, 'u> DrawPackRoots<'a, 'u> {
    pub fn draw(&mut self) {
        let _id = self.ui.push_id(self.state.ui_id());
        let categories = match self.state.unloaded.as_ref() {
            None if self.state.pack.is_some() => self.prepare_categories(),
            _reason => None,
        };
        match categories {
            Some(cats) => self.draw_loaded(cats),
            None => self.draw_unloaded(),
        }
    }

    fn draw_loaded(&mut self, mut categories: DrawCategoryCollectionTree<'a, 'u>) {
        let cats = self.state.info.info.as_ref().map(|i| &i.categories);
        let pseudo_root = self.state.info.unique_root().map(|r| r.path());
        let (mut pack_act, mut pack_toggle) = (None, None);
        self.ui.table_next_column();
        let token = if pseudo_root.is_none() {
            let mut header = self.prepare_header();
            let (act, token) = header.draw();
            pack_act = act;
            pack_toggle = header.draw_toggle_inline();
            token
        } else {
            None
        };
        if let Some(cats) = cats {
            if pseudo_root.is_some() || token.is_some() {
                for root in cats.root_paths() {
                    categories.draw_root(root, pseudo_root.is_some());
                    self.ui.table_next_column();
                    let act_cat = match categories.act.take() {
                        Some((
                            path,
                            action @ (CategoryAction::HoverTooltip | CategoryAction::ContextMenu),
                        )) if Some(path) == pseudo_root => {
                            let pack_act = PackAction::Cat { path: Some(path), action };
                            let clobbered = pack_act.clobber(self.state.pack_path(), &mut self.act_pack);
                            match clobbered {
                                Ok(Some((_, PackAction::Cat { path: Some(p), action })))
                                | Err(PackAction::Cat { path: Some(p), action }) => match action {
                                    CategoryAction::HoverTooltip => None,
                                    action => Some((p, action)),
                                },
                                clobbered => {
                                    PackAction::warn_clobbered(&self.act_pack, clobbered);
                                    None
                                },
                            }
                        },
                        act => act,
                    };
                    if let Some((path, act_cat)) = act_cat {
                        let clobbered = act_cat.clobber(path, &mut self.act_cat);
                        CategoryAction::warn_clobbered(&self.act_cat, clobbered);
                    }
                }
                categories.pop_all();
            }
        }
        drop(token);
        if let Some(act) = pack_toggle {
            self.act_pack = Some((self.state.pack_path(), PackAction::Cat {
                path: pseudo_root,
                action: CategoryAction::Enable(Some(act)),
            }));
        }
        let pack_act = match pack_act {
            #[cfg(todo)]
            Some(UiAction::LEFT_CLICK) => Some(CategoryAction::Enable(None)),
            Some(UiAction::RIGHT_CLICK) => Some(CategoryAction::ContextMenu),
            Some(UiAction::Hovered) => Some(CategoryAction::HoverTooltip),
            Some(act) => {
                log::debug!("DELETEME: pack action {act:?} unexpected");
                None
            },
            None => None,
        };
        if let Some(pack_act) = pack_act {
            let pack_act = PackAction::Cat { path: pseudo_root, action: pack_act };
            let clobbered = pack_act.clobber(self.state.pack_path(), &mut self.act_pack);
            PackAction::warn_clobbered(&self.act_pack, clobbered);
        }
    }
    fn draw_unloaded(&mut self) {
        self.ui.table_next_column();
        let act = DrawPackUnloaded { ui: self.ui, state: self.state }.draw();
        let act_pack = match act {
            Some(UiAction::RIGHT_CLICK) => Some(PackAction::Root(CategoryAction::ContextMenu)),
            Some(UiAction::Hovered) => Some(PackAction::Root(CategoryAction::HoverTooltip)),
            Some(UiAction::Primary) => match &self.state.unloaded {
                Some(reason) if reason.can_reactivate(false) => Some(PackAction::ACTIVATE),
                Some(UnloadedReason::Loading) => None,
                Some(reason) if !reason.can_reload() => Some(PackAction::REFRESH),
                _ => Some(PackAction::RELOAD),
            },
            Some(UiAction::LEFT_CLICK) => Some(PackAction::ACTIVATE),
            Some(act) => {
                log::debug!("DELETEME: unloaded pack action {act:?} unexpected");
                None
            },
            None => None,
        };
        if let Some(act_pack) = act_pack {
            let clobbered = act_pack.clobber(self.state.pack_path(), &mut self.act_pack);
            PackAction::warn_clobbered(&self.act_pack, clobbered);
        }
    }
    pub(super) fn prepare_header(&self) -> DrawCategoryHeader<'a, 'u> {
        DrawCategoryHeader {
            ui: self.ui,
            display_name: &self.state.display_name,
            open: false,
            open_cond: Condition::Once,
            toggle_state: !matches!(self.state.unloaded, Some(UnloadedReason::Disabled)),
            is_leaf: self
                .state
                .info
                .info
                .as_ref()
                .map(|i| i.categories.roots.is_empty()),
            is_decorative: false,
            button_interact: None,
            allow_overlap: true,
        }
    }

    pub(super) fn prepare_categories(&self) -> Option<DrawCategoryCollectionTree<'a, 'u>> {
        self.categories.map(|state| {
            DrawCategoryCollectionTree::new(DrawCategoryCollection::new(self.ui, state, self.state))
        })
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
    /// TODO: return CategoryAction .-.
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
        #[cfg(todo = "unnecessary")]
        if let Some(state) = toggle_act {
            self.act_toggle(Some(state));
        }
        if let Some(state) = toggle_act {
            self.toggle_state.set(VisibilityFlags::TOGGLE, state);
        }

        let act = DecorateCategoryHeader {
            ui: self.ui,
            info: self.info,
            was_hovered: matches!(header_action, Some(UiAction::Hovered)),
        }
        .decorate();
        let act = match act {
            Some(UiAction::Hovered) => Some(UiAction::Hovered),
            _act => {
                if let Some(act) = _act {
                    log::debug!("DELETEME: cat decoration action {act:?} unexpected");
                }
                None
            },
        };
        let act = match header_action {
            None | Some(UiAction::Hovered) => act,
            Some(act) => Some(act),
        };
        (act, header_token)
    }
    #[cfg(todo = "unnecessary")]
    fn act_toggle(&self, state: Option<bool>) {
        PathingEvent::CategoryToggle(self.pack_path, self.category_path, state).try_send();
    }

    pub(super) fn prepare_header(&self) -> DrawCategoryHeader<'a, 'u> {
        DrawCategoryHeader {
            ui: self.ui,
            open: self.open_state,
            toggle_state: self.toggle_state.is_visible(),
            open_cond: Condition::Always,
            display_name: self
                .info
                .display_name()
                .unwrap_or(taimi_hoard::lazyfmt::UNAVAILABLE),
            is_leaf: match self.has_children {
                false if self.is_lonely => None,
                is_parent => Some(!is_parent),
            },
            is_decorative: self.flags.contains(CategoryFlags::SEPARATOR),
            button_interact: Some(self.is_copyable),
            allow_overlap: self.pseudo_root && self.has_toggle(),
        }
    }

    pub(super) fn has_toggle(&self) -> bool {
        !self.flags.contains(CategoryFlags::SEPARATOR) && !self.is_lonely
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
                PackElement::copy_copyable(self.ui, copy_value, copy_message);
            } else if self.ui.is_item_hovered() {
                #[cfg(todo)]
                {
                    // TODO: give downstream enough context to do this properly
                    act = Some(UiAction::Hovered);
                }
                DrawCategoryTooltip {
                    ui: self.ui,
                    info: self.info,
                    include_copyable: true,
                    display_name_visible: true,
                    tooltip: self.info.tooltip.borrowed(),
                }
                .draw();
                show_tip = false;
            }
        }
        let tooltip = show_tip.then_some(self.info.tooltip());
        if let Some(Some(tooltip)) = tooltip {
            act = Some(UiAction::Hovered);
            #[cfg(deleteme)]
            {
                DrawCategoryTooltip {
                    ui: self.ui,
                    info: self.info,
                    include_copyable: false,
                    display_name_visible: true,
                    tooltip,
                }
                .draw();
            }
        }
        act
    }
}

impl super::PackElements {
    /// TODO
    pub fn can_collapse(&self) -> bool {
        self.pack_state
            .values()
            .any(|p| p.categories.open_mask.flags.any())
    }
    pub fn can_expand(&self) -> bool {
        self.pack_state.values().any(|p| {
            let count = p
                .state
                .info
                .category_info()
                .map(|(cats, _)| cats.count())
                .unwrap_or(0);
            p.categories.open_mask.end_len() != count || p.categories.open_mask.flags.not_any()
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
impl PackElement {
    pub fn draw(&mut self, ui: &Ui) {
        let mut roots = self.prepare_draw(ui);
        roots.draw();
        let DrawPackRoots { act_cat, act_pack, .. } = roots;
        self.act_post_draw(ui, act_cat, act_pack, true);
    }

    pub fn prepare_draw<'a, 'u>(&'a self, ui: &'a Ui<'u>) -> DrawPackRoots<'a, 'u> {
        DrawPackRoots {
            ui,
            state: &self.state,
            categories: Some(&self.categories),
            act_cat: Default::default(),
            act_pack: Default::default(),
            last_menu_open: self.categories.open_menu.last().copied(),
        }
    }
}
