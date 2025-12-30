use {
    crate::{
        controller::{
            pathing::{
                registry::{PackCategory, PackCategoryFlags, PackCategoryInfo, PackInfoSignature, PackVecOf, UnloadedReason}, shared::{PathingShared, SharedLoaderPacksInfo, SharedPackConfig, SharedPackInfo, SharedPackLoad, SharedPackLoaded}, visible::VisibilityFlags, PathingEvent
            },
            Controller,
        }, exports::runtime::{
            self as rt,
            imgui::{self, Condition, MouseButton, Selectable, TreeNode, TreeNodeFlags, TreeNodeToken, Ui, MenuItem, StyleVar},
        }, render::RenderState, with_i18n
    },
    glam::Vec2,
    glamour::Rect,
    std::{collections::BTreeMap, fmt::{self, Write}, iter, mem, sync::{Arc, Weak}},
    taimi_hoard::{flags::BitSet, str_opt, str_opt_ref, loc::{LocationRef, LocationGet}}, taimi_meta::packs::{CategoryIndex, CategoryPath, PackPath}, taimi_pack::{attributes::{self, AttrString, InteractionAttributes, MarkerAttributes}, category::{Category, CategoryFlags, CategoryId}, Pack}, taimi_sync::watched::{watch, Watched, Watcher},
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

pub struct DrawPackUnloaded<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub state: &'a PackElementState,
}
impl DrawPackUnloaded<'_, '_> {
    pub fn draw(&mut self) {
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
        //ui.table_next_column();
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
    #[cfg(deleteme)]
    pub state: &'a PackCategoryState,
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
impl<'u> DrawCategoryToggle<'_, 'u> {
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

    fn prepare_header(&self) -> DrawCategoryHeader<'_, 'u> {
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

    fn has_toggle(&self) -> bool { !self.flags.contains(CategoryFlags::SEPARATOR) && !self.is_lonely }
}

#[derive(Debug)]
pub struct DrawCategoryHeader<'a, 'ui> {
    pub ui: &'a Ui<'ui>,
    pub display_name: &'a str,
    pub open: bool,
    pub open_cond: Condition,
    pub toggle_state: bool,
    pub is_leaf: Option<bool>,
    pub is_decorative: bool,
    pub button_interact: Option<bool>,
    pub allow_overlap: bool,
}
impl<'a, 'u> DrawCategoryHeader<'a, 'u> {
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
            .allow_item_overlap(self.allow_overlap)
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
    pub fn draw_toggle_inline(&mut self) -> Option<bool> {
        self.ui.same_line();
        self.ui.dummy([4.0, 0.0]);
        self.ui.same_line();
        self.draw_toggle_checkbox()
    }
    pub fn draw_toggle_prefix(&mut self) -> (Option<bool>, imgui::StyleStackToken<'a>) {
        self.ui.unindent();
        let checkbox_gap = self.ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
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
        self.ui.checkbox("", &mut self.toggle_state).then(move || {
            self.toggle_state
        })
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
    pub fn draw_start(&mut self) -> (Option<UiAction>, Option<imgui::MenuToken<'a>>) {
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
    fn draw_branch(&mut self) -> (Option<UiAction>, Option<imgui::MenuToken<'a>>) {
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

    fn draw_end(&mut self, token: Option<imgui::MenuToken<'a>>) {
        drop(token);
        #[cfg(todo)]
        let act = self.resolve_action();
        if !self.draw.is_decorative || !self.is_leaf() {
            self.draw_spacing();
        }
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
        let _size = ui.push_style_var(StyleVar::WindowMinSize([minwidth, lineheight]));
        ui.tooltip(|| {
            {
                let _padding = ui.push_style_var(StyleVar::ItemSpacing([f32::EPSILON, f32::EPSILON]));
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
            log::debug!("TODO: packs damage report and setup etc");
            let new_packs = packs.iter().skip(self.pack_state.len());
            for (_path, pack) in new_packs {
                self.pack_state.data.push(PackElement::new(&pack));
            }
        }
        for pack in self.pack_state.values_mut() {
            pack.pre_draw(visibility);
        }
    }
    pub fn draw(&self, ui: &Ui) {
        for pack in self.pack_state.values() {
            pack.draw(ui);
        }
    }

    pub fn any_loaded(&self) -> bool {
        self.pack_state.values().any(|p| p.state.info.info.is_some())
    }

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

    pub fn draw(&self, ui: &Ui) {
        self.prepare_draw(ui).draw();
    }
    fn any_roots_open(&self) -> bool {
        self.state.info.info.as_ref().map(|i|
            i.categories.root_paths().any(|r| self.categories.open_mask.contains(r))
        ).unwrap_or(false)
    }

    pub fn prepare_draw<'a, 'u>(&'a self, ui: &'a Ui<'u>) -> DrawPackRoots<'a, 'u> {
        DrawPackRoots {
            ui,
            state: &self.state,
            categories: Some(&self.categories),
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
    pub fn category_flags(&self, path: CategoryPath) -> CategoryFlags {
        let category_flags = self.category_flags.as_ref()
            .and_then(|f| f.lookup_get(&path));
        if let Some(category_flags) = category_flags {
            category_flags
        } else if let Some(info) = &self.info.info {
            info.categories.lookup_flags(path)
        } else {
            CategoryFlags::empty()
        }
    }
    pub fn category_info(&self, path: CategoryPath) -> Option<&PackCategory> {
        self.info.info.as_ref().and_then(|info|
            info.categories.all().lookup_ref(&path)
        )
    }
    pub fn category_visibility_deviation(&self, path: CategoryPath) -> VisibilityFlags {
        self.config.cached.as_ref().map(|config|
            config.config.visibility_deviation_for(path)
        ).unwrap_or(VisibilityFlags::empty())
    }
    pub fn category_get_visibility(&self, path: CategoryPath) -> VisibilityFlags {
        let dev = self.category_visibility_deviation(path);
        VisibilityFlags::from_category_flags(self.category_flags(path)) ^ dev
    }

    pub fn pack_data(&self) -> Option<Arc<Pack>> {
        self.pack.as_ref().and_then(Weak::upgrade)
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

#[derive(Debug, Default)]
pub struct CategoryCollectionState {
    pub info_sig: PackInfoSignature,
    pub categories: BTreeMap<CategoryPath, CategoryInfo>,
    pub open_mask: BitSet,
}
impl CategoryCollectionState {
    pub fn pre_draw(&mut self, pack: &PackElementState, pack_damage: &PackDamageReport, visibility: PackVisibility) {
        if pack_damage.info.is_some() {
            if pack.info.sig.is_empty() {
                self.cleanup_cache(true);
            } else if self.info_sig != pack.info.sig {
                self.categories.clear();
                //self.open_mask.clear();
            }
            self.info_sig = pack.info.sig;
        } else if !pack_damage.loaded {
            return
        }
        if let PackVisibility::Closed = visibility {
            self.cleanup_cache(false);
            return
        }
        let Some(info) = &pack.info.info else {
            return
        };
        let cats = &info.categories;
        match visibility {
            PackVisibility::Visible => {
                let Some(pack_data) = pack.pack_data() else { return };
                let visible_cats = info.categories.root_paths().flat_map(|root| {
                    self.all_visible_children(cats, root).chain(iter::once(root))
                });
                let missing_info: BitSet = visible_cats
                    .filter(|path| !self.categories.contains_key(path))
                    .collect();
                for path in missing_info.iter_of::<CategoryPath>() {
                    let Some((_id, category)) = pack_data.categories.all_categories.get_index(path.path as usize) else {
                        log::error!("missing {path} from {}", pack.info);
                        continue
                    };
                    self.categories.insert(path, CategoryInfo::from_pack_category(category));
                }
            },
            PackVisibility::Pending => {
                self.categories.retain(|&path, _| Self::is_path_visible(&cats, &self.open_mask, path));
            },
            _ => (),
        }
    }
    fn cleanup_cache(&mut self, purge: bool) {
        self.categories = Default::default();
        if purge {
            self.open_mask = Default::default();
        }
        self.info_sig = PackInfoSignature::EMPTY;
    }

    /// don't assume order (DFS atm)
    ///
    /// excludes the root, and produces nothing if root isn't open
    pub fn all_visible_children<'a, 'c>(&'a self, cats: &'c PackCategoryInfo, root: CategoryPath) -> impl Iterator<Item = CategoryPath> + 'a + 'c where
        'a: 'c,
        'c: 'a,
    {
        let open_mask = &self.open_mask;
        cats.descendents_of(root).filter(|&path|
            Self::is_path_visible(cats, open_mask, path)
        )
    }

    /// TODO: also apply filters like current-map
    /// (beware multiple UI elements may have differing filter states?)
    pub fn is_path_visible(cats: &PackCategoryInfo, open_mask: &BitSet, path: CategoryPath) -> bool {
        if open_mask.contains(path) { return true }
        let direct_parent_open = cats.parent_of(path).map(|p| {
            open_mask.contains(p)
        });
        direct_parent_open.unwrap_or(true)
    }

    /// TODO
    pub fn category_is_on_map(&self, path: CategoryPath) -> bool {
        true
    }
}

#[derive(Debug, Default)]
pub struct CategoryInfo {
    /// TODO: probably not yet used?
    pub id: Option<CategoryId>,
    pub display_name: Option<Arc<str>>,
    pub tooltip: PackTooltip,
    pub interaction: Option<Arc<InteractionAttributes>>,
    pub visibility: VisibilityFlags,
}
impl CategoryInfo {
    pub const EMPTY: Self = Self {
        id: None,
        display_name: None,
        tooltip: PackTooltip::EMPTY,
        interaction: None,
        visibility: VisibilityFlags::empty(),
    };

    pub fn from_pack_category(category: &Category) -> Self {
        Self {
            id: Some(category.full_id.clone()),
            display_name: Some(category.display_name.clone()),
            tooltip: PackTooltip::from_attrs(&category.marker_attributes),
            interaction: category.marker_attributes.interaction.clone(),
            visibility: VisibilityFlags::from_pack_category(category),
        }
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_ref().map(|n| &n[..])
    }
    pub fn tooltip(&self) -> Option<PackTooltipRef<'_>> {
        self.tooltip.get().map(PackTooltip::borrowed)
    }
    pub fn ui_id(&self, path: CategoryPath) -> imgui::Id<'_> {
        self.id.as_ref().map(|id| imgui::Id::Str(id.as_str()))
            .unwrap_or(imgui::Id::Int(path.path as _))
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
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
