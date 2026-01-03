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
    super::{DrawCategoryHeader, UiAction},
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

    fn draw_end(&mut self, token: Option<MenuToken<'a>>) {
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
