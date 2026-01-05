use {
    crate::{
        controller::{
            pathing::{
                registry::{PackCategory, PackCategoryFlags, PackCategoryInfo, PackInfoSignature, PackVecOf, UnloadedReason}, shared::{PathingShared, SharedLoaderPacksInfo, SharedPackConfig, SharedPackInfo, SharedPackLoad, SharedPackLoaded}, visible::VisibilityFlags, PathingEvent
            },
            Controller,
        }, exports::runtime::{
            self as rt,
            imgui::{self, Condition, MouseButton, Selectable, TreeNode, TreeNodeFlags, TreeNodeToken, Ui, MenuItem, StyleVar, MenuToken},
        }, render::RenderState, with_i18n
    },
    super::{DrawCategoryHeader, DrawCategoryCollection, CategoryAction, CategoryActionSlot, UiAction, DrawPackRoots},
    glam::Vec2,
    glamour::Rect,
    std::{collections::BTreeMap, fmt::{self, Write}, iter, mem, sync::{Arc, Weak}},
    taimi_hoard::{flags::BitSet, str_opt, str_opt_ref, loc::{LocationRef, LocationGet}}, taimi_meta::packs::{CategoryIndex, CategoryPath, PackPath}, taimi_pack::{attributes::{self, AttrString, InteractionAttributes, MarkerAttributes}, category::{Category, CategoryFlags, CategoryId}, Pack}, taimi_sync::watched::{watch, Watched, Watcher},
};

impl<'a, 'u> super::DrawCategoryToggle<'a, 'u> {
    fn prepare_menu(&self) -> DrawCategoryMenu<'a, 'u> {
        let header = self.prepare_header();
        let mut menu = DrawCategoryMenu::new(header, self.is_lonely);
        menu.has_info = !self.info.tooltip.is_empty();
        menu.is_copyable = self.is_copyable;
        menu.has_toggle = self.has_toggle();
        menu
    }
}
impl<'a, 'u> DrawPackRoots<'a, 'u> {
    pub fn draw_menu(&mut self) {
        let _id = self.ui.push_id(self.state.ui_id());
        let categories = match self.state.unloaded.as_ref() {
            None if self.state.pack.is_some() => self.prepare_menu_categories(),
            _reason => None,
        };
        match categories {
            Some(cats) => self.draw_menu_loaded(cats),
            None => self.draw_menu_unloaded(),
        }
    }
    pub fn draw_menu_loaded(&mut self, mut categories: DrawCategoryCollectionMenu<'a, 'u>) {
        if let Some(cats) = self.state.info.info.as_ref().map(|i| &i.categories) {
            for root in cats.root_paths() {
                categories.draw_root(root, true);
                if let Some((path, act_cat)) = categories.act.take() {
                    let clobbered = act_cat.clobber(path, &mut self.act_cat);
                    CategoryAction::warn_clobbered(&self.act_cat, clobbered);
                }
            }
            let menu_open = categories.act_open.last().copied();
            if menu_open != self.last_menu_open {
                let path = match menu_open {
                    Some(p) => p,
                    None => self.last_menu_open.or(self.state.info.primary_root().map(|r| r.path()))
                        .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX)),
                };
                let clobbered = CategoryAction::Open(menu_open.is_some()).clobber(path, &mut self.act_cat);
                CategoryAction::warn_clobbered(&self.act_cat, clobbered);
            }
        }
    }
    pub fn draw_menu_unloaded(&mut self) {
        let mut menu = DrawCategoryMenu::new(self.prepare_header(), true);
        let (act, token) = menu.draw_start();
        if let Some(token) = &token {
            self.ui.text("uhhhh wasn't I a leaf?");
        }
        menu.draw_end(token);
        let act = match act {
            Some(UiAction::Hovered) => Some(CategoryAction::HoverTooltip),
            Some(UiAction::Primary | UiAction::RIGHT_CLICK | UiAction::LEFT_CLICK) =>
                Some(CategoryAction::Enable(None)),
            Some(act) => {
                log::debug!("DELETEME: category menu action {act:?} unexpected");
                None
            },
            None => None,
        };
        if act.is_some() {
            self.act_pack = act;
        }
        if self.last_menu_open.is_some() {
            let path = self.state.info.primary_root()
                .map(|r| r.path())
                .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX));
            let clobbered = CategoryAction::Open(false).clobber(path, &mut self.act_cat);
            CategoryAction::warn_clobbered(&self.act_cat, clobbered);
        }
    }

    pub fn prepare_menu_categories(&self) -> Option<DrawCategoryCollectionMenu<'a, 'u>> {
        self.categories.map(|state| {
            let mut cats = DrawCategoryCollectionMenu::new(DrawCategoryCollection::new(self.ui, state, self.state));
            if self.last_menu_open.is_some() {
                cats.act_open.reserve(0x10);
            }
            cats
        })
    }
}
impl super::PackElement {
    pub fn draw_menu(&mut self, ui: &Ui) {
        let mut roots = self.prepare_draw(ui);
        roots.draw_menu();
        let DrawPackRoots { mut act_cat, act_pack, .. } = roots;
        if let Some((path, CategoryAction::Open(opened))) = act_cat {
            log::debug!("DELETEME: menu open({opened}) for {path}");
            let cats = self.state.info.category_info();
            let open_menu = &mut self.categories.open_menu;
            if opened {
                if let Some((cats, ..)) = cats {
                    open_menu.clear();
                    open_menu.push(path);
                    open_menu.extend(cats.parents_of(path));
                    open_menu.reverse();
                } else if !open_menu.contains(&path) {
                    open_menu.push(path);
                }
            } else {
                if path.path == CategoryIndex::MAX || cats.map(|(c, ..)| c.is_root(path)).unwrap_or(true) {
                    open_menu.clear();
                } else if let Some(i) = open_menu.iter().rposition(|&p| p == path) {
                    open_menu.truncate(i);
                } else {
                    open_menu.clear();
                }
            }
            act_cat = None;
        }
        self.act_post_draw(ui, act_cat, act_pack, false);
    }
}

#[derive(Debug)]
pub struct DrawCategoryMenu<'a, 'ui> {
    pub draw: DrawCategoryHeader<'a, 'ui>,
    pub has_toggle: bool,
    /// indicator of extra details in tooltip
    pub has_info: bool,
    pub is_copyable: bool,
    pub filtered_inactive: bool,
    pub drawn_bounds: Rect,
}
impl<'a, 'u> DrawCategoryMenu<'a, 'u> {
    pub fn new(draw: DrawCategoryHeader<'a, 'u>, is_lonely: bool) -> Self {
        Self {
            is_copyable: matches!(draw.button_interact, Some(true)),
            has_toggle: !draw.is_decorative && !is_lonely,
            has_info: false,
            filtered_inactive: false,
            drawn_bounds: Rect::ZERO,
            draw,
        }
    }
    #[inline]
    pub fn is_leaf(&self) -> bool {
        matches!(self.draw.is_leaf, Some(true) | None)
    }
    pub fn draw_start(&mut self) -> (Option<UiAction>, Option<MenuToken<'a>>) {
        self.draw_spacing();
        match self.is_leaf() {
            true => (self.draw_leaf(), None),
            _ => self.draw_branch(),
        }
    }
    fn draw_leaf(&mut self) -> Option<UiAction> {
        let ui = self.draw.ui;
        let decorative = self.draw.is_decorative;
        let item = MenuItem::new(self.draw.display_name)
            .selected(self.has_toggle && self.draw.toggle_state)
            .enabled(!decorative || self.is_copyable);
        let toggled = match () {
            _ if decorative && self.draw.display_name.is_empty() => {
                ui.separator();
                self.draw_spacing();
                return None
            },
            _ if self.is_copyable => with_i18n!("copy", |label| item.shortcut(&label).build(ui)),
            _ if self.has_info => item.shortcut("?").build(ui),
            _ if self.filtered_inactive && !decorative =>
                with_i18n!("inactive", |label| item.shortcut(&label).build(ui)),
            _ => item.build(ui),
        };
        let mut act = toggled.then_some(UiAction::Primary);
        if act.is_none() {
            act = self.resolve_action_secondary();
        }
        act
    }
    fn draw_branch(&mut self) -> (Option<UiAction>, Option<MenuToken<'a>>) {
        let ui = self.draw.ui;

        // TODO: manually igSetNextWindowSize when opening a new category
        // because it seems to "inherit" the last menu's size and that's dumb
        let menu_start = Vec2::from_array(ui.cursor_pos());
        let menu_size = Vec2::from_array(ui.calc_text_size(&self.draw.display_name));
        let menu = ui.begin_menu_with_enabled(&self.draw.display_name, true);
        self.drawn_bounds = Rect::new(menu_start.into(), menu_size.into());
        // TODO: track menu.is_some() != open?

        (self.resolve_action(), menu)
    }
    /// explicit enable item at the bottom of the menu
    pub fn draw_trailing_toggle(&mut self) -> Option<UiAction> {
        let ui = self.draw.ui;
        if Self::dead_zone_spacing(ui, false) {
            ui.separator();
        }
        let label = match self.draw.toggle_state {
            true => "disable",
            false => "enable",
        };
        let toggled = with_i18n!(label, |label| {
            let item = MenuItem::new(&label);
            match self.filtered_inactive {
                true => with_i18n!("inactive", |off_map| item.shortcut(&off_map).build(ui)),
                _ => item.build(ui),
            }
        });
        #[cfg(deleteme)]
        if ui.is_item_hovered() {
            ui.tooltip_text("hint: right-click to quickly toggle any category");
        }
        toggled.then_some(UiAction::Primary).or_else(||
            self.resolve_action_secondary()
        )
    }

    fn draw_end(&mut self, token: Option<MenuToken<'a>>) -> Option<UiAction> {
        drop(token);
        let act = self.resolve_action();
        if !self.draw.is_decorative || !self.is_leaf() {
            self.draw_spacing();
        }
        act
    }
    pub fn draw_decoration_with<R, F: FnOnce(&Self) -> R>(&mut self, f: F) -> Option<R> {
        if self.drawn_bounds.is_empty() { return None }
        let checkpoint = self.draw.ui.cursor_pos();
        let mut top_right = self.drawn_bounds.origin;
        top_right.x += self.drawn_bounds.size.width;
        self.draw.ui.set_cursor_pos(top_right.into());
        let res = f(&*self);
        self.draw.ui.set_cursor_pos(checkpoint);
        Some(res)
    }
    /// for use within [self.draw_decoration_with()]
    pub fn draw_decoration_info(&self) {
        let tooltip_hint = match self.has_info {
            true => "❓",
            #[cfg(todo)]
            true => "(?)",
            _ => "",
        };
        let state_postfix = match self.has_toggle {
            true if !self.draw.toggle_state => " ×",
            _ => "",
        };
        if !tooltip_hint.is_empty() || !state_postfix.is_empty() {
            self.draw.ui.text(format!(" {tooltip_hint}{state_postfix}"));
        }
    }
    /// TODO: double-check if these checks must follow branch menu token drop or not
    fn resolve_action(&self) -> Option<UiAction> {
        let act = self.draw.ui.is_item_clicked().then_some(UiAction::LEFT_CLICK);
        act.or_else(|| self.resolve_action_secondary())
    }
    fn resolve_action_secondary(&self) -> Option<UiAction> {
        if self.draw.ui.is_item_clicked_with_button(MouseButton::Right) {
            Some(UiAction::RIGHT_CLICK)
        } else if self.draw.ui.is_item_hovered() {
            Some(UiAction::Hovered)
        } else {
            None
        }
    }

    /// create a dead zone for the mouse to rest without triggering a menu change
    const MENU_DEAD_ZONE: Vec2 = Vec2::new(2.0, 2.0);
    fn dead_zone_spacing(ui: &Ui, branch: bool) -> bool {
        let _vspace = ui.push_style_var(StyleVar::ItemSpacing([0.2, 0.2]));
        let sz = match branch {
            true => Self::MENU_DEAD_ZONE,
            false => Self::MENU_DEAD_ZONE / 2.0,
        };
        if Vec2::from(ui.cursor_pos()).y > Vec2::from(ui.cursor_start_pos()).y + Self::MENU_DEAD_ZONE.y {
            // create a dead zone for the mouse to rest without triggering a menu change
            ui.dummy(sz.to_array());
            true
        } else {
            false
        }
    }
    fn draw_spacing(&self) -> bool {
        Self::dead_zone_spacing(self.draw.ui, !self.is_leaf())
    }
}
pub struct DrawCategoryCollectionMenu<'a, 'ui> {
    pub draw: DrawCategoryCollection<'a, 'ui>,
    pub menu_stack: Vec<(Option<MenuToken<'a>>, DrawCategoryMenu<'a, 'ui>)>,
    pub act: CategoryActionSlot,
    pub act_open: Vec<CategoryPath>,
}
impl<'a, 'u> DrawCategoryCollectionMenu<'a, 'u> {
    pub fn new(draw: DrawCategoryCollection<'a, 'u>) -> Self {
        Self {
            draw,
            menu_stack: Vec::new(),
            act: Default::default(),
            act_open: Vec::new(),
        }
    }
    pub fn draw_root(&mut self, path: CategoryPath, pseudo_root: bool) {
        self.menu_stack.clear();
        let root = self.push_and_draw(path, Some(pseudo_root));
        let cats = root.and_then(|()| self.draw.pack.info.info.as_ref().map(|i| &i.categories));
        if let Some(cats) = cats {
            let mut cat_iter = cats.nested_descendents_of(path);
            let mut prev_depth = cat_iter.depth();
            'cats: while let Some(cat_path) = cat_iter.next() {
                let depth = cat_iter.depth();
                if let Some(popping) = prev_depth.checked_sub(depth) {
                    for _ in 0..=popping {
                        if self.pop().is_none() {
                            break 'cats
                        }
                    }
                }
                prev_depth = depth;
                if self.push_and_draw(cat_path, None).is_none() {
                    cat_iter.skip_to_sibling();
                }
            }
        }
        while let Some(..) = self.pop() {}
    }

    pub fn push_and_draw(&mut self, path: CategoryPath, pseudo_root: Option<bool>) -> Option<()> {
        self.draw.push(path)?;
        let toggle = self.draw.prepare_toggle(path, pseudo_root);
        let mut menu = toggle.prepare_menu();
        let (act, token) = menu.draw_start();
        let res = token.as_ref().map(drop);
        let act = Self::act_to_action(&menu, act);
        self.menu_stack.push((token, menu));
        if let Some(act) = act {
            let clobbered = act.clobber(path, &mut self.act);
            CategoryAction::warn_clobbered(&self.act, clobbered);
        }

        if res.is_some() {
            if self.draw.path_stack.len() > self.act_open.len() {
                self.act_open.clone_from(&self.draw.path_stack);
            } else if self.draw.path_stack.len() == self.act_open.len() && self.draw.path_stack.last() != self.act_open.last() {
                log::debug!("DELETEME: category menu open already set? {:?} vs {:?}", self.act_open, self.draw.path_stack);
            }
        }
        res
    }
    fn act_to_action(menu: &DrawCategoryMenu, act: Option<UiAction>) -> Option<CategoryAction> {
        let is_leaf = menu.is_leaf();
        match act {
            Some(UiAction::Primary) if is_leaf && menu.is_copyable =>
                Some(CategoryAction::Copy),
            Some(UiAction::LEFT_CLICK) if menu.is_copyable =>
                Some(CategoryAction::Copy),
            Some(UiAction::Primary) if is_leaf =>
                Some(CategoryAction::Enable(None)),
            Some(UiAction::RIGHT_CLICK) =>
                Some(CategoryAction::Enable(None)),
            Some(UiAction::LEFT_CLICK) if !is_leaf =>
                Some(CategoryAction::Enable(None)),
            Some(UiAction::Hovered) => Some(CategoryAction::HoverTooltip),
            Some(act) => {
                log::debug!("DELETEME: category menu action {act:?} unexpected");
                None
            },
            None => None,
        }
    }
    pub fn pop(&mut self) -> Option<CategoryPath> {
        let act = if let Some((token, mut menu)) = self.menu_stack.pop() {
            let act = menu.draw_end(token);
            Some((menu, act))
        } else {
            None
        };
        let path = self.draw.pop();
        let act = match (path, act) {
            (Some(path), Some((menu, act))) =>
             Self::act_to_action(&menu, act).map(|act| (act, menu, path)),
            _ => None,
        };
        if let Some((act, menu, path)) = act {
            let clobbered = act.clobber(path, &mut self.act);
            CategoryAction::warn_clobbered(&self.act, clobbered);
        }
        path
    }
}
