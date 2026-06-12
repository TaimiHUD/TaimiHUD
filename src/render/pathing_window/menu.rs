use {
    super::PathingWindowState,
    crate::{
        controller::pathing::{PathingController, PathingEvent},
        render::element::prelude::*,
        space::{engine::Engine, pack::ActivePack},
        with_i18n,
    },
    taimi_pack::Category,
};
#[cfg(feature = "paths-lua")]
use {
    crate::controller::script::{PlugMenusById, PlugMenusShared, ScriptMessage},
    core::mem,
    taimi_pack::category::{id::AsFullId, CategoryId},
};

type CategoryMenuContext = (Vec<u32>, bool);
impl PathingWindowState {
    pub fn draw_context_menu<'ui, U>(&mut self, ui: &mut U, engine: &mut Engine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.draw_context_menu_packs(ui, engine, false);
        if !engine.packs.loaded_packs.is_empty() {
            Self::dead_zone_spacing(ui, false);
            ui.separator();
            if let Some(_menu) = with_i18n!("show-all", |label| ui.begin_menu(&label)) {
                self.draw_context_menu_packs(ui, engine, true);
            }
        }
    }
    pub fn draw_context_menu_packs<'ui, U>(&mut self, ui: &mut U, engine: &mut Engine, filtered: bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _id = ui.push_id(match filtered {
            false => c"packmenu-active",
            true => c"packmenu-all",
        });
        let mut was_multi_root = None;
        for (pack_idx, pack) in engine.packs.loaded_packs.values_mut().enumerate() {
            let _id_pack = ui.push_id(pack_idx);
            if !filtered && pack.available_categories.is_empty() {
                pack.update_available_categories();
            }
            let roots = pack
                .pack
                .categories
                .root_categories
                .iter()
                .filter_map(|id| pack.pack.categories.all_categories.get_full(id))
                .filter(|(_, _, cat)| !cat.is_hidden())
                .filter_map(|(idx, id, cat)| {
                    let filtered = match pack.available_categories.get(idx).map(|b| !*b) {
                        Some(true) | None if !filtered => return None,
                        #[cfg(todo = "unnecessary")]
                        _ if filtered => None,
                        f => f,
                    };
                    Some((idx, id, cat, filtered))
                });
            if pack.pack.categories.root_categories.is_empty() {
                if filtered {
                    ui.menu_item_enabled(&pack.pack.name, true, false);
                }
                continue
            }
            let multi_root = roots.clone().count() > 1;
            match was_multi_root {
                Some(was) if multi_root || was => {
                    Self::dead_zone_spacing(ui, false);
                    ui.separator();
                    Self::dead_zone_spacing(ui, false);
                },
                _ => (),
            }
            was_multi_root = Some(multi_root);
            let mut ctx: CategoryMenuContext = Default::default();
            ctx.1 = filtered;
            for (idx, _id, cat, filtered) in roots {
                let state = pack.user_category_state.get(idx).map(|b| *b);
                let act =
                    Self::draw_context_menu_cat(ui, false, &pack, idx, cat, filtered, state, &mut ctx);
                let _ = Self::act_context_menu_cat(
                    ui,
                    pack,
                    idx,
                    cat,
                    filtered,
                    state,
                    act,
                    &mut ctx,
                    Some(&mut |ui, part| {
                        if part == 0 {
                            ui.text(&pack.pack.name);
                        }
                        #[cfg(todo = "unnecessary")]
                        if part == 1 {
                            ui.text_disabled(&pack.pack.name);
                        }
                    }),
                );
            }
            let (recompute, ..) = ctx;
            if !recompute.is_empty() {
                for cat_idx in recompute {
                    if let Some(mut b) = pack.user_category_state.get_mut(cat_idx as usize) {
                        *b ^= true;
                    }
                }
                let external = PathingController::external_filter_state();
                pack.recompute_enabled(external.as_ref());
            }
            #[cfg(feature = "paths-lua")]
            {
                let script_menus = pack.script_data.as_ref().map(|m| {
                    (
                        &m.plug.menus,
                        m.plug.menus.shared.read().unwrap_or_else(|e| e.into_inner()),
                    )
                });
                #[cfg(feature = "paths-lua")]
                if let Some((shared, script_menus)) = script_menus {
                    let clicked = (!script_menus.is_empty())
                        .then(|| ui.begin_menu(c"\tScripts"))
                        .flatten();
                    let clicked = clicked
                        .and_then(|_token| self.draw_context_menu_scripts(ui, &*script_menus, shared));
                    drop(script_menus);
                    if let Some(clicked) = clicked {
                        if shared.menu_write(&clicked, |s| s.click_state()).is_some() {
                            ScriptMessage::menu_clicked_pack(clicked, engine.packs.generation, pack_idx)
                                .try_send();
                        }
                    }
                }
            }
        }
        #[cfg(feature = "scripts")]
        {
            let plugs = crate::controller::Controller::with_sender(|s| {
                s.scripting
                    .as_ref()
                    .map(|s| s.plugs_shared.borrow().plugs.clone())
            })
            .flatten()
            .into_iter()
            .flat_map(|p| p);
            for (path, plug) in plugs {
                let script_menus = plug.menus.shared.read().unwrap_or_else(|e| e.into_inner());

                let _id = ui.push_id(std::sync::Arc::as_ptr(&plug));
                let Some(_token) = ui.begin_menu(&plug.name[..]) else { continue };
                let clicked = self.draw_context_menu_scripts(ui, &*script_menus, &plug.menus);
                drop(script_menus);
                if let Some(clicked) = clicked {
                    if plug.menus.menu_write(&clicked, |s| s.click_state()).is_some() {
                        ScriptMessage::menu_clicked_plug(clicked, path).try_send();
                    }
                }
            }
        }
    }
    pub fn draw_context_menu_cat_leaf<'ui, U>(
        ui: &mut U,
        cat_index: usize,
        cat: &Category,
        filtered: Option<bool>,
        state: Option<bool>,
        _ctx: &mut CategoryMenuContext,
    ) -> (bool, bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _id = ui.push_id(cat_index);
        Self::dead_zone_spacing(ui, false);
        let decorative = cat.is_separator();
        let is_copyable = cat
            .marker_attributes
            .interaction
            .as_ref()
            .and_then(|i| i.copy_value.as_ref())
            .is_some();
        let mk_menu = |ui: &mut U, shortcut: Option<&mut dyn ImStr>| {
            ui.menu_item_with(
                cat.display_name(),
                state.unwrap_or(false),
                shortcut,
                !decorative || is_copyable,
            )
        };
        let mut toggled = match () {
            _ if cat.is_separator() && cat.display_name().is_empty() => {
                ui.separator();
                Self::dead_zone_spacing(ui, false);
                return (false, false)
            },
            _ if is_copyable => with_i18n!("copy", |label| mk_menu(ui, Some(&mut { label }))),
            _ if ActivePack::category_has_tooltip(cat) => mk_menu(ui, Some(&mut c"?")),
            _ if filtered == Some(true) && !decorative =>
                with_i18n!("inactive", |label| mk_menu(ui, Some(&mut { label }))),
            _ => mk_menu(ui, None),
        };
        if ui.is_item_right_clicked() {
            toggled |= true;
        }
        let hovered = ui.is_item_hovered();
        if !decorative {
            Self::dead_zone_spacing(ui, false);
        }
        (toggled, hovered)
    }
    pub fn draw_context_menu_cat_branch<'ui, U>(
        ui: &mut U,
        pack: &ActivePack,
        cat_index: usize,
        cat: &Category,
        filtered: Option<bool>,
        state: Option<bool>,
        ctx: &mut CategoryMenuContext,
    ) -> (bool, bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _id = ui.push_id(cat_index);
        Self::dead_zone_spacing(ui, true);

        let tooltip_hint = match ActivePack::category_has_tooltip(cat) {
            true => "❓",
            #[cfg(todo)]
            true => "(?)",
            _ => "",
        };
        let state_postfix = match state {
            Some(false) => " ×",
            _ => "",
        };
        // TODO: manually igSetNextWindowSize when opening a new category
        // because it seems to "inherit" the last menu's size and that's dumb
        let menu_start = ui.cursor_pos();
        let menu_size = ui.calc_text_size(cat.display_name());
        let menu = ui.begin_menu_with_enabled(cat.display_name(), true);
        let mut toggled = false;
        if let Some(_menu) = &menu {
            toggled |= Self::draw_context_menu_cat_children(ui, pack, cat_index, cat, filtered, state, ctx);
        }
        drop(menu);
        toggled |= ui.is_item_clicked();
        let hovered = ui.is_item_hovered();
        if ui.is_item_right_clicked() {
            toggled |= true;
        }
        if !tooltip_hint.is_empty() || !state_postfix.is_empty() {
            let checkpoint = ui.cursor_pos();
            ui.set_cursor_pos(menu_start + menu_size.with_y(0.0).to_vector());
            ui.text(im_fmt!(" {tooltip_hint}{state_postfix}"));
            ui.set_cursor_pos(checkpoint);
        }

        Self::dead_zone_spacing(ui, true);
        (toggled, hovered)
    }
    pub fn draw_context_menu_cat_children<'ui, U>(
        ui: &mut U,
        pack: &ActivePack,
        _cat_index: usize,
        cat: &Category,
        filtered: Option<bool>,
        state: Option<bool>,
        ctx: &mut CategoryMenuContext,
    ) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let &mut (_, ctx_filtered, ..) = ctx;
        let mut toggled = false;

        let children = cat
            .sub_categories
            .iter()
            .filter_map(|id| pack.pack.categories.all_categories.get_full(id))
            .filter(|(_, _, cat)| !cat.is_hidden())
            .map(|(idx, id, cat)| {
                let filtered = match filtered {
                    Some(false) => pack.available_categories.get(idx).map(|b| !*b),
                    _ => None,
                };
                let state = match cat.is_separator() {
                    true => None,
                    false => pack.user_category_state.get(idx).map(|b| *b),
                };
                (idx, id, cat, filtered, state)
            });
        let children_visible = children
            .clone()
            .filter(|(_, _, _, filtered, _)| filtered != &Some(true));
        let inline = cat.sub_categories.len() <= 1 || children_visible.clone().count() <= 1;
        let mut any_visible = false;
        let mut toggle_parent = false;
        for (child_idx, _child_id, child, child_filtered, child_state) in children_visible {
            any_visible = true;
            let act = Self::draw_context_menu_cat(
                ui,
                inline,
                pack,
                child_idx,
                child,
                child_filtered,
                child_state,
                ctx,
            );
            toggle_parent |= Self::act_context_menu_cat(
                ui,
                pack,
                child_idx,
                child,
                child_filtered,
                child_state,
                act,
                ctx,
                None,
            );
        }
        let children_filtered = children
            .clone()
            .filter(|(_, _, _, f, _state)| f == &Some(true) && ctx_filtered);
        for (i, (child_idx, _child_id, child, child_filtered, child_state)) in children_filtered.enumerate()
        {
            if any_visible && i == 0 {
                Self::dead_zone_spacing(ui, false);
                ui.separator();
                Self::dead_zone_spacing(ui, false);
            }
            any_visible = true;
            let act = Self::draw_context_menu_cat(
                ui,
                inline,
                pack,
                child_idx,
                child,
                child_filtered,
                child_state,
                ctx,
            );
            toggle_parent |= Self::act_context_menu_cat(
                ui,
                pack,
                child_idx,
                child,
                child_filtered,
                child_state,
                act,
                ctx,
                None,
            );
        }
        if toggle_parent && state == Some(false) {
            toggled |= true;
        }

        match state {
            Some(state) if !inline => {
                if any_visible {
                    Self::dead_zone_spacing(ui, false);
                    ui.separator();
                }
                let label = match state {
                    true => "disable",
                    false => "enable",
                };
                toggled |= with_i18n!(label, |label| {
                    match filtered {
                        Some(true) => with_i18n!("inactive", |off_map| ui.menu_item_with(
                            label,
                            false,
                            Some(off_map),
                            true
                        )),
                        _ => ui.menu_item(label, false),
                    }
                });
                if ui.is_item_right_clicked() {
                    toggled |= true;
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text("hint: right-click to quickly toggle any category");
                }
            },
            _ => (),
        }

        toggled
    }
    pub fn draw_context_menu_cat<'ui, U>(
        ui: &mut U,
        inline: bool,
        pack: &ActivePack,
        cat_index: usize,
        cat: &Category,
        filtered: Option<bool>,
        state: Option<bool>,
        ctx: &mut CategoryMenuContext,
    ) -> (bool, bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let &mut (_, ctx_filtered, ..) = ctx;
        if filtered == Some(true) && !ctx_filtered {
            return (false, false)
        }
        let (toggled, hovered) = match (inline, cat.sub_categories.len()) {
            //#[cfg(todo = "unnecessary")]
            (_, 0) => Self::draw_context_menu_cat_leaf(ui, cat_index, cat, filtered, state, ctx),
            #[cfg(todo)]
            (true, amt) | (_, amt @ 0..=2) => {
                let (mut toggled, hovered) =
                    Self::draw_context_menu_cat_leaf(ui, cat_index, cat, filtered, state, ctx);
                if amt > 0 {
                    let _id = ui.push_id(Id::Int(cat_index as _));
                    ui.indent();
                    toggled |= Self::draw_context_menu_cat_children(
                        ui, pack, cat_index, cat, filtered, state, ctx,
                    );
                    ui.unindent();
                }
                (toggled, hovered)
            },
            _ => Self::draw_context_menu_cat_branch(ui, pack, cat_index, cat, filtered, state, ctx),
        };

        (toggled, hovered)
    }
    pub fn act_context_menu_cat<'ui, U>(
        ui: &mut U,
        _pack: &ActivePack,
        cat_index: usize,
        cat: &Category,
        filtered: Option<bool>,
        state: Option<bool>,
        (toggled, hovered): (bool, bool),
        ctx: &mut CategoryMenuContext,
        mut draw_tooltip: Option<&mut dyn FnMut(&mut U, usize)>,
    ) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let (recompute, ..) = ctx;
        if toggled {
            let is_copyable = cat
                .marker_attributes
                .interaction
                .as_ref()
                .and_then(|i| i.copy_value.as_ref())
                .is_some();
            if let Some(state) = state {
                PathingEvent::PathingStateUpdate(cat.full_id.clone(), !state).try_send();
                recompute.push(cat_index as u32);
            } else if is_copyable {
                ActivePack::copy_copyable(ui, &cat.marker_attributes);
            }
        }
        if hovered && (ActivePack::category_has_tooltip(cat) || draw_tooltip.is_some()) {
            ActivePack::draw_tooltip(ui, cat.display_name(), |ui| {
                if let Some(draw) = &mut draw_tooltip {
                    draw(ui, 0);
                }
                ActivePack::draw_tooltip_category(ui, cat);
                if let Some(draw) = &mut draw_tooltip {
                    draw(ui, 1);
                }
                if filtered.is_none() && state.is_some() && cat.sub_categories.is_empty() {
                    ui.spacing();
                    ui.text("shift-click to enable");
                }
                if let Some(draw) = &mut draw_tooltip {
                    draw(ui, 2);
                }
            });
        }
        toggled && ui.im_io_mod_keys().contains(KeyState::SHIFT)
    }
    /// create a dead zone for the mouse to rest without triggering a menu change
    const MENU_DEAD_ZONE: ImVec2 = ImVec2::new(2.0, 2.0);
    fn dead_zone_spacing<'ui, U>(ui: &mut U, branch: bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _vspace = ui.push_style_item_spacing(ImVec2::splat(0.2));
        let sz = match branch {
            true => Self::MENU_DEAD_ZONE,
            false => Self::MENU_DEAD_ZONE / 2.0,
        };
        if ui.cursor_pos().y > ui.cursor_start_pos().y + Self::MENU_DEAD_ZONE.y {
            // create a dead zone for the mouse to rest without triggering a menu change
            ui.dummy(sz.to_array());
        }
    }

    #[cfg(feature = "paths-lua")]
    fn draw_context_menu_scripts<'ui, U>(
        &mut self,
        ui: &mut U,
        menus: &PlugMenusById,
        _shared: &PlugMenusShared,
    ) -> Option<CategoryId>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut commit = None;
        let mut segs = 0usize;
        let mut menus = menus.iter().peekable();
        // XXX: drain/drop is fine because the order they drop in doesn't matter (don't use for mixed purposes later!)
        let mut menu_stack = Vec::new();
        #[cfg(taimi_debug)]
        let mut prev_id = None::<&CategoryId>;
        while let Some((id, menu)) = menus.next() {
            let prev_segs = mem::replace(&mut segs, id.segments().count());
            if segs > prev_segs && matches!(menu_stack.last(), Some(None)) {
                segs = prev_segs;
                continue
            }
            menu_stack.drain({
                let popping = (prev_segs + 1).saturating_sub(segs);
                #[cfg(taimi_debug)]
                match (prev_id.and_then(|prev| prev.parent()), id.parent()) {
                    (Some(prev), Some(p)) if segs <= prev_segs && !prev.id_starts_with(p) =>
                        log::error!("inconsistent menu tree at {id}"),
                    _ => (),
                }
                (menu_stack.len().saturating_sub(popping))..
            });
            #[cfg(taimi_debug)]
            {
                prev_id = Some(id);
            }
            let has_tooltip = menu.tooltip.is_some() | menu.tooltip_title.is_some();
            let has_children = menus
                .peek()
                .map(|(nextid, _)| nextid.segments().count() > segs && nextid.id_starts_with(id))
                .unwrap_or(false);
            let (token, clicked) = if has_children {
                let t = ui.begin_menu(&menu.display_name[..]);
                (t, ui.is_item_clicked() | ui.is_item_right_clicked())
            } else {
                let shortcut = match has_tooltip {
                    true => Some(c"❓"),
                    false => None,
                };
                let label = ImStrId::new(id, &menu.display_name[..]);
                let clicked = ui.menu_item_with(label, menu.checked.unwrap_or(false), shortcut, true);
                (Some(UiTokenDyn::empty()), clicked | ui.is_item_right_clicked())
            };
            // TODO: tooltip might need to be after token pop for nested menus?
            menu_stack.push(token);
            if has_tooltip && ui.item_is_hovered() {
                let title_template = menu
                    .tooltip_title
                    .as_ref()
                    .map(|t| &t[..])
                    .unwrap_or("LLLLLLLLLLLLLLLLLL");
                ActivePack::draw_tooltip(ui, title_template, |ui| {
                    if let Some(title) = &menu.tooltip_title {
                        ui.text(&title[..]);
                    }
                    if let Some(tip) = &menu.tooltip {
                        // TODO: text_wrapped
                        ui.text(&tip[..]);
                    }
                });
            }
            if let (true, true) = (clicked, commit.is_none()) {
                commit = Some(id.clone())
            }
        }
        commit
    }
}
