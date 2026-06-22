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
        render::element::prelude::*,
        settings::state::ui::pathing::PathingFilterFlags,
    },
    taimi_hoard::flags::BitSet,
    taimi_meta::packs::{CategoryPath, PackPath, VisibilityFlags},
    taimi_pack::category::CategoryFlags,
};

pub struct DrawPackRoots<'a, 'u, U: ?Sized + 'u> {
    pub ui: &'u mut U,
    pub state: &'a PackElementState,
    pub categories: Option<&'a CategoryCollectionState>,
    pub act_cat: CategoryActionSlot,
    pub act_pack: PackActionSlot,
    pub unfilter_interest: Option<CategoryPath>,
    pub last_menu_open: Option<CategoryPath>,
}
impl<'a, 'u, 'ui, U> DrawPackRoots<'a, 'u, U>
where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn draw(&mut self) {
        let _id = self.ui.push_id(self.state.ui_id());
        let categories = match self.state.unloaded.as_ref() {
            None if self.state.pack.is_some() => self.categories.is_some(),
            _reason => false,
        };
        match categories {
            true => self.draw_loaded(),
            _ => self.draw_unloaded(),
        }
    }

    /// lifetime woes
    #[cfg(todo)]
    fn draw_loaded(&mut self, mut categories: DrawCategoryCollectionTree<'a, '_, 'ui, U>) {}
    fn draw_loaded(&mut self) {
        let cats = self.state.info.info.as_ref().map(|i| &i.categories);
        let pseudo_root = self.state.info.unique_root().map(|r| r.path());
        let (mut pack_act, mut pack_toggle) = (None, None);
        let token = if pseudo_root.is_none() {
            let mut header = self.prepare_header();
            let (act, token) = header.draw();
            pack_act = act;
            pack_toggle = header.draw_toggle_inline();
            if token.is_some() {
                self.ui.indent();
            }
            token
        } else {
            None
        };
        if let Some(cats) = cats {
            if pseudo_root.is_some() || token.is_some() {
                let mut categories = match () {
                    #[cfg(todo)]
                    _ => self.prepare_categories(),
                    _ => {
                        let categories = self.categories.map(|state| {
                            DrawCategoryCollectionTree::new(DrawCategoryCollection::new(
                                self.ui, state, self.state,
                            ))
                        });
                        let Some(categories) = categories else { return };
                        categories
                    },
                };
                for root in cats.root_paths() {
                    categories.draw_root(root, pseudo_root.is_some());
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
                if let Some(unfilter) = categories.unfilter_interest {
                    self.unfilter_interest = Some(unfilter);
                }
            }
        }
        if token.is_some() {
            self.ui.unindent();
        } else if pseudo_root.is_none() {
            self.ui.table_next_column();
        }
        drop(token);
        if let Some(act) = pack_toggle {
            self.act_pack = Some((self.state.pack_path(), PackAction::Cat {
                path: pseudo_root,
                action: match (pseudo_root, act) {
                    (None, false) => {
                        #[cfg(taimi_debug)]
                        log::debug!("MULTIROOT DISABLE HACK");
                        CategoryAction::EnableChildren(Some(act))
                    },
                    _ => CategoryAction::Enable(Some(act)),
                },
            }));
        }
        let pack_act = match pack_act {
            #[cfg(todo)]
            Some(UiAction::LEFT_CLICK) => Some(CategoryAction::Enable(None)),
            Some(UiAction::RIGHT_CLICK) => Some(CategoryAction::ContextMenu),
            Some(UiAction::Hovered) => Some(CategoryAction::HoverTooltip),
            Some(act) => {
                #[cfg(taimi_debug)]
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
                #[cfg(taimi_debug)]
                log::debug!("DELETEME: unloaded pack action {act:?} unexpected");
                None
            },
            None => None,
        };
        if let Some(act_pack) = act_pack {
            let clobbered = act_pack.clobber(self.state.pack_path(), &mut self.act_pack);
            PackAction::warn_clobbered(&self.act_pack, clobbered);
        }
        self.ui.table_next_column();
    }
    pub(super) fn prepare_header(&mut self) -> DrawCategoryHeader<'a, '_, U> {
        DrawCategoryHeader {
            ui: self.ui,
            display_name: &self.state.display_name,
            open: false,
            open_cond: ImCondition::Startup,
            toggle_state: !matches!(self.state.unloaded, Some(UnloadedReason::Disabled)),
            is_leaf: self
                .state
                .info
                .info
                .as_ref()
                .map(|i| i.categories.roots.is_empty()),
            is_decorative: false,
            is_header: true,
            button_interact: None,
            allow_overlap: true,
            filter_selected: self
                .categories
                .as_ref()
                .and_then(|c| match c.filter_state.is_active() {
                    false => None,
                    true => match c.filter_state.all_filtered() {
                        true => Some(false),
                        false => None,
                    },
                }),
        }
    }

    pub(super) fn prepare_categories(&mut self) -> Option<DrawCategoryCollectionTree<'a, '_, 'ui, U>> {
        self.categories.map(|state| {
            DrawCategoryCollectionTree::new(DrawCategoryCollection::new(self.ui, state, self.state))
        })
    }
}

#[derive(Debug)]
pub struct DrawCategoryToggle<'a, 'u, U: ?Sized> {
    pub ui: &'u mut U,
    pub info: &'a CategoryInfo,
    #[cfg(todo)]
    pub category_path: CategoryPath<PackPath>,
    pub flags: CategoryFlags,
    pub toggle_state: VisibilityFlags,
    pub open_state: bool,
    pub is_lonely: bool,
    pub is_copyable: bool,
    pub has_children: bool,
    pub pseudo_root: bool,
    pub filter_selected: Option<bool>,
}
impl<'a, 'u, 'ui, U> DrawCategoryToggle<'a, 'u, U>
where
    U: ?Sized + ImDrawWindow<'ui>,
{
    /// TODO: return CategoryAction .-.
    pub fn draw(&mut self) -> (Option<UiAction>, Option<UiTokenDyn<'ui>>) {
        let has_toggle = self.has_toggle();
        let mut toggle_checkbox = has_toggle.then_some(());
        let mut toggle_act = None;
        let mut checkbox_gap = None;
        let not_root = !self.pseudo_root;
        let (header_action, header_token) = {
            let mut header = self.prepare_header();
            if let Some(()) = not_root.then(|| toggle_checkbox.take()).flatten() {
                let (act, gap) = header.draw_toggle_prefix();
                checkbox_gap = Some(gap);
                toggle_act = act;
            }

            let token = header.draw();

            drop(checkbox_gap);
            if let Some(()) = toggle_checkbox {
                toggle_act = header.draw_toggle_inline();
            } else if has_toggle {
                header.end_toggle_prefix();
            }
            token
        };
        if let Some(state) = toggle_act {
            self.toggle_state.set(VisibilityFlags::TOGGLE, state);
        }

        let act = DecorateCategoryHeader {
            ui: &mut *self.ui,
            info: self.info,
            was_hovered: matches!(header_action, Some(UiAction::Hovered)),
        }
        .decorate();
        let act = match act {
            Some(UiAction::Hovered) => Some(UiAction::Hovered),
            _act => {
                if let Some(act) = _act {
                    #[cfg(taimi_debug)]
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

    pub(super) fn prepare_header<'u0, 'a0>(&'a0 mut self) -> DrawCategoryHeader<'a, 'u0, U>
    where
        'u: 'u0,
        'a0: 'u0,
    {
        let allow_overlap = self.pseudo_root && self.has_toggle();
        let is_decorative = self.flags.contains(CategoryFlags::SEPARATOR);
        DrawCategoryHeader {
            ui: &mut *self.ui,
            open: self.open_state,
            toggle_state: self.toggle_state.is_visible(),
            open_cond: ImCondition::Always,
            display_name: self
                .info
                .display_name()
                .unwrap_or(taimi_hoard::lazyfmt::UNAVAILABLE),
            is_leaf: match self.has_children {
                false if self.is_lonely => None,
                is_parent => Some(!is_parent),
            },
            is_header: (self.has_children && !is_decorative) || self.pseudo_root,
            is_decorative,
            button_interact: Some(self.is_copyable),
            allow_overlap,
            filter_selected: self.filter_selected,
        }
    }

    pub(super) fn has_toggle(&self) -> bool {
        !self.flags.contains(CategoryFlags::SEPARATOR) && !self.is_lonely
    }
}
impl<'a, 'u, 'ui, U> DrawCategoryHeader<'a, 'u, U>
where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn draw_toggle_inline(&mut self) -> Option<bool> {
        self.ui.same_line();
        self.ui.dummy([4.0, 0.0]);
        self.ui.same_line();
        self.draw_toggle_checkbox()
    }
    pub fn draw_toggle_prefix(&mut self) -> (Option<bool>, UiTokenDyn<'ui>) {
        self.ui.unindent();
        let checkbox_gap = self.ui.push_style_item_spacing(ImVec2::ZERO);
        #[cfg(todo = "unnecessary")]
        let _inner_gap = ui.push_style_var(StyleVar::ItemInnerSpacing([0.0, 0.0]));
        let act = self.draw_toggle_checkbox();
        self.ui.same_line();
        (act, checkbox_gap)
    }
    pub fn end_toggle_prefix(&mut self) {
        self.ui.indent();
    }
    pub fn draw_toggle_checkbox(&mut self) -> Option<bool> {
        self.ui
            .checkbox("", &mut self.toggle_state)
            .then(move || self.toggle_state)
    }
}

/// Draw buttons and tooltips and stuff on top
#[derive(Debug)]
pub struct DecorateCategoryHeader<'a, 'u, U: ?Sized + 'u> {
    pub ui: &'u mut U,
    pub info: &'a CategoryInfo,
    pub was_hovered: bool,
}
impl<'a, 'u, 'ui, U> DecorateCategoryHeader<'_, 'u, U>
where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
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
    pub fn act_expand_all(&mut self, skip_filtered: bool) {
        for pack in self.pack_state.values_mut() {
            if let Some((cats, _)) = pack.state.info.category_info() {
                let apply_filters = match skip_filtered {
                    false => pack.categories.filter_state.is_active(),
                    true => false,
                };
                if apply_filters {
                    // TODO: go one level up? only open parents with at least one whitelisted child!
                    let cats: BitSet = pack.categories.iter_whitelisted(&pack.state).collect();
                    pack.categories.open_mask.extend(cats.iter_of::<CategoryPath>());
                } else {
                    pack.categories.open_mask.flags.fill(true);
                    pack.categories.open_mask.extend_for(cats.count(), true);
                }
            }
        }
    }
    pub fn act_collapse_all(&mut self, skip_filtered: bool) {
        for pack in self.pack_state.values_mut() {
            let apply_filters = match skip_filtered {
                false => pack.categories.filter_state.is_active() && pack.categories.open_mask.any(),
                true => false,
            };
            let cats = apply_filters.then_some(pack.state.info.category_info()).flatten();
            if let Some(..) = cats {
                let cats: BitSet = pack.categories.iter_whitelisted(&pack.state).collect();
                for cat in cats.iter_of::<CategoryPath>() {
                    pack.categories.open_mask.remove_at(cat);

                    let new_len = pack.categories.open_mask.last_one().map(|i| i + 1).unwrap_or(0);
                    pack.categories.open_mask.truncate(new_len);
                }
            } else {
                pack.categories.open_mask.clear();
            }
        }
    }
}
impl PackElement {
    pub fn draw<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut roots = self.prepare_draw(ui);
        roots.draw();
        let DrawPackRoots {
            act_cat, act_pack, unfilter_interest, ..
        } = roots;
        self.act_post_draw(ui, act_cat, act_pack, true);
        if let Some(interest) = unfilter_interest {
            if let Some(cats) = self.state.info.info.as_ref().map(|i| &i.categories) {
                let hide = self
                    .categories
                    .filter_state
                    .flags
                    .contains(PathingFilterFlags::ShowHidden);
                let filter = move |path: &CategoryPath| match hide {
                    false => !cats.hidden.contains(*path),
                    true => true,
                };
                self.categories
                    .filter_state
                    .extend_interest(cats.children_of(interest).filter(filter));
            }
        }
    }

    pub fn prepare_draw<'a, 'u, 'ui, U>(&'a self, ui: &'u mut U) -> DrawPackRoots<'a, 'u, U>
    where
        U: ?Sized + ImDrawWindow<'ui> + 'u,
    {
        DrawPackRoots {
            ui,
            state: &self.state,
            categories: Some(&self.categories),
            act_cat: Default::default(),
            act_pack: Default::default(),
            last_menu_open: self.categories.open_menu.last().copied(),
            unfilter_interest: None,
        }
    }
}
