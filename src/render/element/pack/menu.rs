use {
    super::{
        CategoryAction,
        CategoryActionSlot,
        DrawCategoryCollection,
        DrawCategoryHeader,
        DrawPackRoots,
        PackAction,
        PackActionSlot,
        PackElementState,
        UiAction,
    },
    crate::{
        controller::pathing::PathingEvent,
        render::element::prelude::*,
    },
    std::borrow::Cow,
    glamour::Rect,
    taimi_meta::packs::{CategoryIndex, CategoryPath, PackPath},
};
#[cfg(feature = "paths-interact")]
use crate::controller::pathing::InteractMessage;

impl<'a, 'u, 'ui, U> super::DrawCategoryToggle<'a, 'u, U> where
    U: ?Sized + ImDrawWindow<'ui>,
{
    fn prepare_menu(&mut self) -> DrawCategoryMenu<'a, '_, U> {
        let is_lonely = self.is_lonely;
        let has_info = !self.info.tooltip.is_empty();
        let is_copyable = self.is_copyable;
        let has_toggle = self.has_toggle();
        let header = self.prepare_header();
        let mut menu = DrawCategoryMenu::new(header, is_lonely);
        menu.has_info = has_info;
        menu.is_copyable = is_copyable;
        menu.has_toggle = has_toggle;
        menu
    }
}
impl<'a, 'u, 'ui, U> DrawPackRoots<'a, 'u, U> where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    /// TODO: split this up (lifetime woes)
    pub fn draw_menu(&mut self) {
        let _id = self.ui.push_id(self.state.ui_id());
        let categories = match self.state.unloaded.as_ref() {
            None if self.state.pack.is_some() =>
                self.categories.map(|state| {
                    let mut cats =
                        DrawCategoryCollectionMenu::new(DrawCategoryCollection::new(self.ui, state, self.state));
                    if self.last_menu_open.is_some() {
                        cats.act_open.reserve(0x10);
                    }
                    cats
                }),
            _reason => None,
        };
        let Some(mut categories) = categories else {
            self.draw_menu_unloaded();
            return
        };
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
                    None => self
                        .last_menu_open
                        .or(self.state.info.primary_root().map(|r| r.path()))
                        .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX)),
                };
                let clobbered =
                    CategoryAction::Open(Some(menu_open.is_some())).clobber(path, &mut self.act_cat);
                CategoryAction::warn_clobbered(&self.act_cat, clobbered);
            }
        }
    }
    pub fn draw_menu_unloaded(&mut self) {
        let mut menu = DrawCategoryMenu::new(self.prepare_header(), true);
        let (act, token) = menu.draw_start();
        if let Some(..) = &token {
            menu.draw.ui.text("uhhhh wasn't I a leaf?");
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
        if let Some(action) = act {
            let act = PackAction::Cat {
                path: self.state.info.unique_root().map(|r| r.path()),
                action,
            };
            let clobbered = act.clobber(self.state.pack_path(), &mut self.act_pack);
            PackAction::warn_clobbered(&self.act_pack, clobbered);
        }
        if self.last_menu_open.is_some() {
            let path = self
                .state
                .info
                .primary_root()
                .map(|r| r.path())
                .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX));
            let clobbered = CategoryAction::Open(Some(false)).clobber(path, &mut self.act_cat);
            CategoryAction::warn_clobbered(&self.act_cat, clobbered);
        }
    }

    /// lifetime woes
    #[cfg(todo)]
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
    #[cfg(todo)]
    pub fn prepare_menu_categories(&mut self) -> Option<DrawCategoryCollectionMenu<'a, '_, 'ui, U>> {
        self.categories.map(|state| {
            let mut cats =
                DrawCategoryCollectionMenu::new(DrawCategoryCollection::new(self.ui, state, self.state));
            if self.last_menu_open.is_some() {
                cats.act_open.reserve(0x10);
            }
            cats
        })
    }
    #[cfg(todo)]
    pub fn draw_menu_loaded(&mut self, mut categories: DrawCategoryCollectionMenu<'a, 'u, 'ui, U>) {
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
                    None => self
                        .last_menu_open
                        .or(self.state.info.primary_root().map(|r| r.path()))
                        .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX)),
                };
                let clobbered =
                    CategoryAction::Open(Some(menu_open.is_some())).clobber(path, &mut self.act_cat);
                CategoryAction::warn_clobbered(&self.act_cat, clobbered);
            }
        }
    }
}
impl super::PackElement {
    pub fn draw_menu<'ui, U>(&mut self, ui: &mut U) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut roots = self.prepare_draw(ui);
        roots.draw_menu();
        let DrawPackRoots { mut act_cat, act_pack, .. } = roots;
        if let Some((path, CategoryAction::Open(Some(opened)))) = act_cat {
            log::debug!("DELETEME: menu open({opened}) for {path}");
            let cats = self.state.info.category_info();
            let open_menu = &mut self.categories.open_menu;
            if opened {
                if let Some((cats, ..)) = cats {
                    open_menu.clear();
                    open_menu.push(path);
                    open_menu.extend(cats.ancestors_of(path));
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
    pub fn draw_menu_advanced(&self, ui: &Ui) {
        let display_name = self.state.display_name().map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(self.state.info.to_string()));
        let menu = ui.begin_menu(&display_name);
        if let Some(_menu) = menu {
            let mut draw = DrawPackAdvancedMenu {
                ui,
                path: self.state.pack_path(),
                act_pathing: None,
            };
            draw.draw();
            if let Some(pmsg) = draw.act_pathing {
                pmsg.try_send();
            }
        }
    }

    pub(super) fn draw_pack_context<'ui, U>(&mut self, ui: &mut U) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut draw_cat = if let Some(root) = self.state.info.unique_root().map(|r| r.path()) {
            with_i18n!("pack-root", |label| ui.text_disabled(label));
            let mut draw_cat = self.prepare_category_context_contents(ui, root, Some((true, true)));
            draw_cat.draw_contents_category();
            draw_cat.ui.separator();
            with_i18n!("pack-root-submenu", |label| draw_cat.ui.text_disabled(label));
            Ok(draw_cat)
        } else {
            Err(&mut *ui)
        };
        let mut draw = DrawPackContextMenu {
            ui: match &mut draw_cat {
                Ok(draw_cat) => draw_cat.ui,
                Err(ui) => ui,
            },
            state: &self.state,
            act: None,
        };
        draw.draw_contents();
        let act = draw.act;
        let act_cat = match draw_cat.ok() {
            Some(mut draw_cat) => {
                draw_cat.ui.separator();
                draw_cat.draw_contents_adjacent();
                draw_cat.act
            },
            None => None,
        };
        self.act_post_draw_context(ui, act_cat, act);
    }
    pub(super) fn draw_category_context<'ui, U>(&mut self, ui: &mut U, category_path: CategoryPath) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let act = {
            let mut draw = self.prepare_category_context_contents(ui, category_path, None);
            draw.draw_contents();
            draw.act
        };
        self.act_post_draw_context(ui, act, None);
    }
    pub(super) fn prepare_category_context_contents<'u, 'ui, U>(
        &self,
        ui: &'u mut U,
        category_path: CategoryPath,
        root_cat: Option<(bool, bool)>,
    ) -> DrawCategoryContextMenu<'u, U> where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let (mut is_root, pack_visible) = match root_cat {
            None => (None, false),
            Some((is_root, pack_visible)) => (Some(is_root), pack_visible),
        };
        let (mut any_open, mut any_closed) = (false, false);
        if let Some((cats, ..)) = self.state.info.category_info() {
            let _ = is_root.get_or_insert_with(|| cats.is_root(category_path));
            let children = cats.descendents_of(category_path);
            for cat in children {
                if self.categories.open_mask.contains(cat) {
                    any_open = true;
                } else {
                    any_closed = true;
                }
            }
        }
        DrawCategoryContextMenu {
            ui,
            act: None,
            category_path,
            is_root: is_root.unwrap_or(false),
            pack_visible,
            any_open,
            any_closed,
        }
    }
}
impl super::PackElements {
    pub fn draw_menu_advanced(&mut self, ui: &Ui) {
        ui.text_disabled("debug menu");
        let mut act_pathing = None;
        if MenuItem::new("scan for packs").build(ui) {
            act_pathing = Some(PathingEvent::Refresh { include_datasources: true });
        }
        if MenuItem::new("scan for packs (sans datasources)").build(ui) {
            act_pathing = Some(PathingEvent::Refresh { include_datasources: false });
        }
        ui.separator();
        if MenuItem::new("refresh vis").build(ui) {
            act_pathing = Some(PathingEvent::RequestRebuildVis { pack_path: None, partial: false, notify: None });
        }
        if MenuItem::new("rebuild vis (force)").build(ui) {
            act_pathing = Some(PathingEvent::RequestRebuildVis { pack_path: None, partial: false, notify: Some(true) });
        }
        if MenuItem::new("rebuild vis (partial)").build(ui) {
            act_pathing = Some(PathingEvent::RequestRebuildVis { pack_path: None, partial: true, notify: Some(true) });
        }
        if MenuItem::new("rebuild space").build(ui) {
            act_pathing = Some(PathingEvent::RequestRebuildSpace { entities: None, bvh: None });
        }
        if MenuItem::new("rebuild space (force)").build(ui) {
            act_pathing = Some(PathingEvent::RequestRebuildSpace { entities: Some(true), bvh: Some(true) });
        }
        if MenuItem::new("rebuild space (bvh only)").build(ui) {
            act_pathing = Some(PathingEvent::RequestRebuildSpace { entities: Some(false), bvh: Some(true) });
        }
        if MenuItem::new("nuke space bvh").build(ui) {
            act_pathing = Some(PathingEvent::RequestRebuildSpace { entities: Some(true), bvh: Some(false) });
        }
        ui.separator();
        #[cfg(feature = "paths-interact")]
        if MenuItem::new("rebuild interact").build(ui) {
            act_pathing = Some(PathingEvent::InteractControl(InteractMessage::RequestRebuild));
        }
        #[cfg(feature = "paths-interact")]
        if MenuItem::new("rebuild interact (bvh only)").build(ui) {
            act_pathing = Some(PathingEvent::InteractControl(InteractMessage::BvhRebuild));
        }
        ui.separator();
        if MenuItem::new("collect garbage").build(ui) {
            act_pathing = Some(PathingEvent::CollectGarbage { tick: 1, aggressive: false });
        }
        if MenuItem::new("collect garbage timidly").build(ui) {
            act_pathing = Some(PathingEvent::COLLECT_GARBAGE_PRUNE_ONLY);
        }
        if MenuItem::new("collect garbage aggressively").build(ui) {
            act_pathing = Some(PathingEvent::COLLECT_GARBAGE_NOW);
        }
        if MenuItem::new("report resources").build(ui) {
            act_pathing = Some(PathingEvent::RequestResourceReport { pack_path: None });
        }
        if MenuItem::new("release resources").build(ui) {
            act_pathing = Some(PathingEvent::RequestResourceRelease { pack_path: None });
        }
        #[cfg(todo = "unnecessary")]
        #[cfg(feature = "paths-interact")]
        if MenuItem::new("reload interact settings").build(ui) {
            act_pathing = Some(PathingEvent::InteractControl(InteractMessage::RefreshSettings));
        }
        if let Some(pmsg) = act_pathing {
            pmsg.try_send();
        }
        ui.separator();
        for (_pack_path, pack) in self.pack_state.iter() {
            pack.draw_menu_advanced(ui);
        }
    }
}

#[derive(Debug)]
pub struct DrawCategoryMenu<'a, 'u, U: ?Sized + 'u> {
    pub draw: DrawCategoryHeader<'a, 'u, U>,
    pub has_toggle: bool,
    /// indicator of extra details in tooltip
    pub has_info: bool,
    pub is_copyable: bool,
    pub filtered_inactive: bool,
    pub drawn_bounds: Rect<WindowSpace>,
}
impl<'a, 'u, 'ui, U> DrawCategoryMenu<'a, 'u, U> where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn new(draw: DrawCategoryHeader<'a, 'u, U>, is_lonely: bool) -> Self {
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
    pub fn draw_start(&mut self) -> (Option<UiAction>, Option<UiTokenDyn<'ui>>) {
        self.draw_spacing();
        match self.is_leaf() {
            true => (self.draw_leaf(), None),
            _ => self.draw_branch(),
        }
    }
    fn draw_leaf(&mut self) -> Option<UiAction> {
        let decorative = self.draw.is_decorative;
        let mut shortcut_i18n;
        let shortcut = match () {
            _ if decorative && self.draw.display_name.is_empty() => {
                self.draw.ui.separator();
                self.draw_spacing();
                return None
            },
            _ if self.is_copyable => Some({
                shortcut_i18n = fl!("copy");
                &mut shortcut_i18n as &mut dyn ImStr
            }),
            _ if self.has_info => Some(&mut c"?" as &mut _),
            _ if self.filtered_inactive && !decorative => Some({
                shortcut_i18n = fl!("inactive");
                &mut shortcut_i18n as &mut _
            }),
            _ => None,
        };
        let toggled = self.draw.ui.menu_item_with(self.draw.display_name,
            self.has_toggle && self.draw.toggle_state,
            shortcut,
            !decorative || self.is_copyable,
        );
        let mut act = toggled.then_some(UiAction::Primary);
        if act.is_none() {
            act = self.resolve_action_secondary();
        }
        act
    }
    fn draw_branch(&mut self) -> (Option<UiAction>, Option<UiTokenDyn<'ui>>) {
        // TODO: manually igSetNextWindowSize when opening a new category
        // because it seems to "inherit" the last menu's size and that's dumb
        let menu_start = self.draw.ui.cursor_pos();
        let menu_size = self.draw.ui.calc_text_size(&self.draw.display_name);
        let menu = self.draw.ui.begin_menu_with_enabled(&self.draw.display_name, true);
        self.drawn_bounds = Rect::new(menu_start.into(), menu_size.into());
        // TODO: track menu.is_some() != open?

        (self.resolve_action(), menu)
    }
    /// explicit enable item at the bottom of the menu
    pub fn draw_trailing_toggle(&mut self) -> Option<UiAction> {
        if Self::dead_zone_spacing(&mut *self.draw.ui, false) {
            self.draw.ui.separator();
        }
        let label = match self.draw.toggle_state {
            true => "disable",
            false => "enable",
        };
        let off_map = self.filtered_inactive.then_some(fl!("inactive"));
        let toggled = self.draw.ui.menu_item_with(fl!(label), false, off_map, true);
        if self.draw.ui.is_item_hovered() {
            self.draw.ui.tooltip_text("hint: right-click to quickly toggle any category");
        }
        toggled
            .then_some(UiAction::Primary)
            .or_else(|| self.resolve_action_secondary())
    }

    fn draw_end(&mut self, token: Option<UiTokenDyn<'ui>>) -> Option<UiAction> {
        drop(token);
        let act = self.resolve_action();
        if !self.draw.is_decorative || !self.is_leaf() {
            self.draw_spacing();
        }
        act
    }
    pub fn draw_decoration_with<R, F: FnOnce(&Self) -> R>(&mut self, f: F) -> Option<R> {
        if self.drawn_bounds.is_empty() {
            return None
        }
        let checkpoint = self.draw.ui.cursor_pos();
        let mut top_right = self.drawn_bounds.origin;
        top_right.x += self.drawn_bounds.size.width;
        self.draw.ui.set_cursor_pos(top_right);
        let res = f(&*self);
        self.draw.ui.set_cursor_pos(checkpoint);
        Some(res)
    }
    /// for use within [self.draw_decoration_with()]
    pub fn draw_decoration_info(&mut self) {
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
        if self.draw.ui.is_item_right_clicked() {
            Some(UiAction::RIGHT_CLICK)
        } else if self.draw.ui.is_item_hovered() {
            Some(UiAction::Hovered)
        } else {
            None
        }
    }

    /// create a dead zone for the mouse to rest without triggering a menu change
    const MENU_DEAD_ZONE: ImSize2 = ImSize2::new(2.0, 2.0);
    fn dead_zone_spacing(ui: &mut U, branch: bool) -> bool {
        let _vspace = ui.push_style_item_spacing(ImVec2::splat(0.2));
        let sz = match branch {
            true => Self::MENU_DEAD_ZONE,
            false => Self::MENU_DEAD_ZONE / 2.0,
        };
        if ui.cursor_pos().y > ui.cursor_start_pos().y + Self::MENU_DEAD_ZONE.height {
            // create a dead zone for the mouse to rest without triggering a menu change
            ui.dummy(sz);
            true
        } else {
            false
        }
    }
    fn draw_spacing(&mut self) -> bool {
        Self::dead_zone_spacing(self.draw.ui, !self.is_leaf())
    }
}
pub struct DrawCategoryCollectionMenu<'a, 'u, 'ui, U: ?Sized + 'u> {
    pub draw: DrawCategoryCollection<'a, 'u, 'ui, U>,
    pub menu_stack: Vec<(Option<UiTokenDyn<'ui>>, DrawCategoryMenu<'a, 'u, U>)>,
    pub act: CategoryActionSlot,
    pub act_open: Vec<CategoryPath>,
}
impl<'a, 'u, 'ui, U> DrawCategoryCollectionMenu<'a, 'u, 'ui, U> where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn new(draw: DrawCategoryCollection<'a, 'u, 'ui, U>) -> Self {
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
        self.draw.ui.text(c"TODO: menus");
        None
    }
    #[cfg(todo)]
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
            } else if self.draw.path_stack.len() == self.act_open.len()
                && self.draw.path_stack.last() != self.act_open.last()
            {
                log::debug!(
                    "DELETEME: category menu open already set? {:?} vs {:?}",
                    self.act_open,
                    self.draw.path_stack
                );
            }
        }
        res
    }
    fn act_to_action(menu: &DrawCategoryMenu<'_, '_, U>, act: Option<UiAction>) -> Option<CategoryAction> {
        let is_leaf = menu.is_leaf();
        match act {
            Some(UiAction::Primary) if is_leaf && menu.is_copyable => Some(CategoryAction::Copy),
            Some(UiAction::LEFT_CLICK) if menu.is_copyable => Some(CategoryAction::Copy),
            Some(UiAction::Primary) if is_leaf => Some(CategoryAction::Enable(None)),
            Some(UiAction::RIGHT_CLICK) => Some(CategoryAction::Enable(None)),
            Some(UiAction::LEFT_CLICK) if !is_leaf => Some(CategoryAction::Enable(None)),
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
            (Some(path), Some((menu, act))) => Self::act_to_action(&menu, act).map(|act| (act, menu, path)),
            _ => None,
        };
        if let Some((act, _menu, path)) = act {
            let clobbered = act.clobber(path, &mut self.act);
            CategoryAction::warn_clobbered(&self.act, clobbered);
        }
        path
    }
}

pub struct DrawPackContextMenu<'a, 'u, U: ?Sized + 'u> {
    pub ui: &'u mut U,
    pub state: &'a PackElementState,
    pub act: PackActionSlot,
}
impl<'a, 'u, 'ui, U> DrawPackContextMenu<'a, 'u, U> where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn draw_contents(&mut self) {
        let s = match self.state.unloaded.as_ref() {
            None if self.state.pack.is_some() => Some(()),
            _reason => None,
        };
        let act = match s {
            Some(()) => self.draw_contents_loaded(),
            None => self.draw_contents_unloaded(),
        };
        if let Some(act) = act {
            let clobbered = act.clobber(self.state.pack_path(), &mut self.act);
            PackAction::warn_clobbered(&self.act, clobbered);
        }
    }
    pub fn draw_contents_unloaded(&mut self) -> Option<PackAction> {
        let action_remove = self.ui.selectable(fl!("remove-pack"), false);
        let action_reload = self.ui.selectable(fl!("activate-pack"), false);
        if action_reload {
            Some(match &self.state.unloaded {
                Some(reason) if !reason.can_reactivate(false) => PackAction::RELOAD,
                _ => PackAction::ACTIVATE,
            })
        } else if action_remove {
            Some(PackAction::REMOVE)
        } else {
            None
        }
    }
    pub fn draw_contents_loaded(&mut self) -> Option<PackAction> {
        let ui = &mut *self.ui;
        //with_i18n!("pack", |header| ui.text(&header));
        let is_loaded = self.state.pack.is_some();
        let action_later = match is_loaded {
            true => ui.selectable(fl!("offload-pack"), false),
            false => false,
        };
        let action_load = match is_loaded {
            true => ui.selectable(fl!("deactivate-pack"), false),
            false => ui.selectable(fl!("activate-pack"), false),
        };
        let action_unload = match is_loaded {
            true => ui.selectable(fl!("unload-pack"), false),
            false => false,
        };
        let action_reload = match is_loaded {
            true => ui.selectable(fl!("reload-pack"), false),
            false => false,
        };
        let action_refresh = ui.selectable(fl!("refresh-pack"), false);
        if action_unload {
            Some(PackAction::REMOVE)
        } else if action_later {
            Some(PackAction::OFFLOAD)
        } else if action_load {
            Some(match is_loaded {
                true => PackAction::UNLOAD,
                false => PackAction::ACTIVATE,
            })
        } else if action_reload {
            Some(PackAction::RELOAD)
        } else if action_refresh {
            Some(PackAction::REFRESH)
        } else {
            None
        }
    }
    pub fn id() -> &'static str {
        "pack-context"
    }
}

pub struct DrawCategoryContextMenu<'u, U: ?Sized + 'u> {
    pub ui: &'u mut U,
    pub act: CategoryActionSlot,
    pub category_path: CategoryPath,
    pub is_root: bool,
    pub pack_visible: bool,
    pub any_open: bool,
    pub any_closed: bool,
}
impl<'u, 'ui, U> DrawCategoryContextMenu<'u, U> where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn draw_contents(&mut self) {
        self.draw_contents_category();
        self.ui.separator();
        self.draw_contents_adjacent();
    }
    pub fn draw_contents_category(&mut self) {
        let act = self.draw_menu_category();
        self.set_act(act);
    }
    pub fn draw_contents_adjacent(&mut self) {
        let act = self.draw_menu_adjacent();
        self.set_act(act);
    }
    fn set_act(&mut self, act: Option<CategoryAction>) {
        if let Some(act) = act {
            let clobbered = act.clobber(self.category_path, &mut self.act);
            CategoryAction::warn_clobbered(&self.act, clobbered);
        }
    }
    fn draw_menu_category(&mut self) -> Option<CategoryAction> {
        let ui = &mut *self.ui;
        let action_toggle = ui.selectable(fl!("toggle"), false);
        let action_enable_all = ui.selectable(fl!("enable-all"), false);
        let action_disable_all = ui.selectable(fl!("disable-all"), false);
        let action_reset_all = ui.selectable(fl!("reset-all"), false);
        let (action_enable_to, action_disable_to) = if !self.is_root {
            ui.separator();
            let enable_to = ui.selectable(fl!("enable-to"), false);
            let disable_to = ui.selectable(fl!("disable-to"), false);
            (enable_to, disable_to)
        } else {
            (false, false)
        };
        let action_all = if action_enable_all {
            Some(Some(true))
        } else if action_disable_all {
            Some(Some(false))
        } else if action_reset_all {
            Some(None)
        } else {
            None
        };
        let action_parents = if action_enable_to {
            Some(true)
        } else if action_disable_to {
            Some(false)
        } else {
            None
        };
        let act = if action_toggle {
            Some(CategoryAction::TOGGLE)
        } else if let Some(action_all) = action_all {
            Some(match action_all {
                Some(enable) => CategoryAction::EnableChildren(Some(enable)),
                None => CategoryAction::ResetChildren,
            })
        } else if let Some(parents_enable) = action_parents {
            Some(CategoryAction::EnableParents(parents_enable))
        } else {
            None
        };

        if self.any_closed || self.any_open {
            ui.separator();
        }
        let action_expand_all = if self.any_closed {
            ui.selectable(fl!("expand-all"), false)
        } else {
            false
        };
        let action_collapse_all = if self.any_open {
            ui.selectable(fl!("collapse-all"), false)
        } else {
            false
        };
        #[cfg(todo)]
        let action_hide = ui.selectable(fl!(if self.hidden { "unhide" } else { "hide" }), false);
        let action_hide = false;

        if let Some(act) = act {
            Some(act)
        } else if action_hide {
            Some(CategoryAction::Open(None))
        } else if action_expand_all {
            Some(CategoryAction::OpenChildren(Some(true)))
        } else if action_collapse_all {
            Some(CategoryAction::OpenChildren(Some(false)))
        } else {
            None
        }
    }
    fn draw_menu_adjacent(&mut self) -> Option<CategoryAction> {
        let action_isolate = self.ui.selectable(fl!("isolate"), false);
        let action_unisolate = self.ui.selectable(fl!("unisolate"), false);
        let action_isolate = if action_isolate {
            Some(Some(None))
        } else if action_unisolate {
            Some(None)
        } else {
            None
        };
        if let Some(isolate) = action_isolate {
            Some(match isolate {
                Some(state) => CategoryAction::Isolate(state),
                None => CategoryAction::ResetSiblings,
            })
        } else {
            None
        }
    }

    pub fn id() -> &'static str {
        "cat-context"
    }
}
pub struct DrawPackAdvancedMenu<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub path: PackPath,
    pub act_pathing: Option<PathingEvent>,
}
impl<'a, 'u> DrawPackAdvancedMenu<'a, 'u> {
    pub fn draw(&mut self) {
        let ui = self.ui;
        if MenuItem::new("rebuild vis").build(ui) {
            self.act_pathing = Some(PathingEvent::RequestRebuildVis { pack_path: Some(self.path), partial: false, notify: Some(true) });
        }
        if MenuItem::new("rebuild vis (partial)").build(ui) {
            self.act_pathing = Some(PathingEvent::RequestRebuildVis { pack_path: Some(self.path), partial: true, notify: None });
        }
        ui.separator();
        if MenuItem::new("report resources").build(ui) {
            self.act_pathing = Some(PathingEvent::RequestResourceReport { pack_path: Some(self.path) });
        }
        if MenuItem::new("release resources").build(ui) {
            self.act_pathing = Some(PathingEvent::RequestResourceRelease { pack_path: Some(self.path) });
        }
    }
}
