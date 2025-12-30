use {
    super::PathingWindowState,
    crate::{
        controller::pathing::{
            PathingController, PathingEvent,
        },
        space::{engine::Engine},
        render::{
            element::pack::{PackElement, CategoryInfo},
            machine::RenderMachine,
        },
        with_i18n,
    },
    glam::Vec2,
    nexus::imgui::{Id, MenuItem, MouseButton, StyleVar, Ui},
    taimi_pack::category::{Category, CategoryFlags},
    taimi_meta::packs::CategoryPath,
};

#[derive(Default)]
struct CategoryMenuContext {
    recompute: Vec<u32>,
    filtered: bool,
}
impl PathingWindowState {
    pub fn draw_context_menu(&mut self, ui: &Ui, machine: &mut RenderMachine) {
        self.draw_context_menu_packs(ui, machine, false);
        if machine.pack_ui_state.any_loaded() {
            Self::dead_zone_spacing(ui, false);
            ui.separator();
            if let Some(_menu) = with_i18n!("show-all", |label| ui.begin_menu(&label)) {
                self.draw_context_menu_packs(ui, machine, true);
            }
        }
    }
    pub fn draw_context_menu_packs(&mut self, ui: &Ui, machine: &mut RenderMachine, unfiltered: bool) {
        let _id = ui.push_id(match unfiltered {
            false => "packmenu-all",
            true => "packmenu-active",
        });
        let mut was_multi_root = None;
        for (pack_idx, pack) in machine.pack_ui_state.pack_state.iter() {
            let _id_pack = ui.push_id(pack.state.ui_id());
            #[cfg(deleteme)]
            if !unfiltered && pack.available_categories.is_empty() {
                pack.update_available_categories();
            }
            let pack_info = pack.state.info.info.as_ref();
            let cats = pack_info.map(|i| i.categories.as_ref());
            let roots = cats.into_iter().flat_map(|cats| cats.root_paths()
                .filter_map(|path| cats.info_of(path).map(|_| (path, cats.lookup_flags(path))))
                .filter(|(_p, flags)| !flags.contains(CategoryFlags::HIDDEN))
                .filter_map(|(path, flags)| {
                    let filtered = match pack.categories.category_is_on_map(path) {
                        true if !unfiltered => return None,
                        #[cfg(todo = "unnecessary")]
                        _ if filtered => None,
                        f => Some(f),
                    };
                    Some((path, flags, filtered))
                })
            );
            let (cats, pack_data) = match (cats, pack.state.pack_data()) {
                (Some(cats), Some(pack_data)) if roots.clone().next().is_some() => (cats, pack_data),
                _ => {
                    if unfiltered {
                        MenuItem::new(&pack.state.display_name).enabled(false).build(ui);
                    }
                    continue
                },
            };
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
            ctx.filtered = unfiltered;
            for (path, flags, filtered) in roots {
                let cat = pack.categories.categories.get(&path).unwrap_or(&CategoryInfo::EMPTY);
                let has_toggle = !flags.contains(CategoryFlags::SEPARATOR) && !cats.lonely.contains(path);
                let state = has_toggle.then(|| pack.state.category_get_visibility(path).is_visible());
                ui.text(format!("TODO: {}", cat.display_name().unwrap_or("")));
                #[cfg(todo)]
                let act =
                    Self::draw_context_menu_cat(ui, false, &pack, path, cat, flags, filtered, state, &mut ctx);
                #[cfg(todo)]
                let _ = Self::act_context_menu_cat(
                    ui,
                    pack,
                    path,
                    cat, flags,
                    filtered,
                    state,
                    act,
                    &mut ctx,
                    Some(&mut |part| {
                        if part == 0 {
                            ui.text(&pack.state.display_name);
                        }
                        #[cfg(todo = "unnecessary")]
                        if part == 1 {
                            ui.text_disabled(&pack.pack.name);
                        }
                    }),
                );
            }
            if !ctx.recompute.is_empty() {
                log::warn!("TODO: recompute cats");
                #[cfg(deleteme)] {
                for cat_idx in recompute {
                    if let Some(mut b) = pack.user_category_state.get_mut(cat_idx as usize) {
                        *b ^= true;
                    }
                }
                let external = PathingController::external_filter_state();
                pack.recompute_enabled(external.as_ref());
                }
            }
        }
    }
}
#[cfg(todo)]
impl PathingWindowState {
    pub fn draw_context_menu_cat_leaf(
        ui: &Ui,
        cat_index: CategoryPath,
        cat: &CategoryInfo,
        cat_flags: CategoryFlags,
        filtered: Option<bool>,
        state: Option<bool>,
        _ctx: &mut CategoryMenuContext,
    ) -> (bool, bool) {
        let _id = ui.push_id(Id::Int(cat_index.path as _));
        Self::dead_zone_spacing(ui, false);
        let decorative = cat_flags.contains(CategoryFlags::SEPARATOR);
        let is_copyable = cat.copyable().is_some();
        let item = MenuItem::new(cat.display_name().unwrap_or(""))
            .selected(state.unwrap_or(false))
            .enabled(!decorative || is_copyable);
        let mut toggled = match () {
            _ if decorative && cat.display_name().is_none() => {
                ui.separator();
                Self::dead_zone_spacing(ui, false);
                return (false, false)
            },
            _ if is_copyable => with_i18n!("copy", |label| item.shortcut(&label).build(ui)),
            _ if PackElement::category_has_tooltip(cat) => item.shortcut("?").build(ui),
            _ if filtered == Some(true) && !decorative =>
                with_i18n!("inactive", |label| item.shortcut(&label).build(ui)),
            _ => item.build(ui),
        };
        if ui.is_item_clicked_with_button(MouseButton::Right) {
            toggled |= true;
        }
        let hovered = ui.is_item_hovered();
        if !decorative {
            Self::dead_zone_spacing(ui, false);
        }
        (toggled, hovered)
    }
    pub fn draw_context_menu_cat_branch(
        ui: &Ui,
        pack: &PackElement,
        cat_index: CategoryPath,
        cat: &CategoryInfo,
        cat_flags: CategoryFlags,
        filtered: Option<bool>,
        state: Option<bool>,
        ctx: &mut CategoryMenuContext,
    ) -> (bool, bool) {
        let _id = ui.push_id(Id::Int(cat_index.path as _));
        Self::dead_zone_spacing(ui, true);

        let tooltip_hint = match PackElement::category_has_tooltip(cat) {
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
        let menu_start = Vec2::from_array(ui.cursor_pos());
        let display_name = cat.display_name().unwrap_or("");
        let menu_size = Vec2::from_array(ui.calc_text_size(display_name));
        let menu = ui.begin_menu_with_enabled(display_name, true);
        let mut toggled = false;
        if let Some(_menu) = &menu {
            toggled |= Self::draw_context_menu_cat_children(ui, pack, cat_index, cat, cat_flags, filtered, state, ctx);
        }
        drop(menu);
        toggled |= ui.is_item_clicked();
        let hovered = ui.is_item_hovered();
        if ui.is_item_clicked_with_button(MouseButton::Right) {
            toggled |= true;
        }
        if !tooltip_hint.is_empty() || !state_postfix.is_empty() {
            let checkpoint = ui.cursor_pos();
            ui.set_cursor_pos((menu_start + menu_size.with_y(0.0)).into());
            ui.text(format!(" {tooltip_hint}{state_postfix}"));
            ui.set_cursor_pos(checkpoint);
        }

        Self::dead_zone_spacing(ui, true);
        (toggled, hovered)
    }
    pub fn draw_context_menu_cat_children(
        ui: &Ui,
        pack: &PackElement,
        _cat_index: CategoryPath,
        cat: &CategoryInfo,
        cat_flags: CategoryFlags,
        filtered: Option<bool>,
        state: Option<bool>,
        ctx: &mut CategoryMenuContext,
    ) -> bool {
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
            .filter(|(_, _, _, f, _state)| f == &Some(true) && ctx.filtered);
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
                    let item = MenuItem::new(&label);
                    match filtered {
                        Some(true) => with_i18n!("inactive", |off_map| item.shortcut(&off_map).build(ui)),
                        _ => item.build(ui),
                    }
                });
                if ui.is_item_clicked_with_button(MouseButton::Right) {
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
    pub fn draw_context_menu_cat(
        ui: &Ui,
        inline: bool,
        pack: &PackElement,
        cat_index: CategoryPath,
        cat: &CategoryInfo,
        cat_flags: CategoryFlags,
        filtered: Option<bool>,
        state: Option<bool>,
        ctx: &mut CategoryMenuContext,
    ) -> (bool, bool) {
        if filtered == Some(true) && !ctx.filtered {
            return (false, false)
        }
        let (toggled, hovered) = match (inline, cat.sub_categories.len()) {
            //#[cfg(todo = "unnecessary")]
            (_, 0) => Self::draw_context_menu_cat_leaf(ui, cat_index, cat, cat_flags, filtered, state, ctx),
            #[cfg(todo)]
            (true, amt) | (_, amt @ 0..=2) => {
                let (mut toggled, hovered) =
                    Self::draw_context_menu_cat_leaf(ui, cat_index, cat, cat_flags, filtered, state, ctx);
                if amt > 0 {
                    let _id = ui.push_id(Id::Int(cat_index as _));
                    ui.indent();
                    toggled |= Self::draw_context_menu_cat_children(
                        ui, pack, cat_index, cat, cat_flags, filtered, state, ctx,
                    );
                    ui.unindent();
                }
                (toggled, hovered)
            },
            _ => Self::draw_context_menu_cat_branch(ui, pack, cat_index, cat, cat_flags, filtered, state, ctx),
        };

        (toggled, hovered)
    }
    pub fn act_context_menu_cat(
        ui: &Ui,
        _pack: &PackElement,
        cat_index: CategoryPath,
        cat: &CategoryInfo,
        cat_flags: CategoryFlags,
        filtered: Option<bool>,
        state: Option<bool>,
        (toggled, hovered): (bool, bool),
        ctx: &mut CategoryMenuContext,
        mut draw_tooltip: Option<&mut dyn FnMut(usize)>,
    ) -> bool {
        if toggled {
            let is_copyable = cat
                .marker_attributes
                .interaction
                .as_ref()
                .and_then(|i| i.copy_value.as_ref())
                .is_some();
            if let Some(state) = state {
                PathingEvent::PathingStateUpdate(cat.full_id.clone(), !state).try_send();
                ctx.recompute.push(cat_index as u32);
            } else if is_copyable {
                PackElement::copy_copyable(ui, &cat.marker_attributes);
            }
        }
        if hovered && (PackElement::category_has_tooltip(cat) || draw_tooltip.is_some()) {
            PackElement::draw_tooltip(ui, &cat.display_name, || {
                if let Some(draw) = &mut draw_tooltip {
                    draw(0);
                }
                PackElement::draw_tooltip_category(ui, cat);
                if let Some(draw) = &mut draw_tooltip {
                    draw(1);
                }
                if filtered.is_none() && state.is_some() && cat.sub_categories.is_empty() {
                    ui.spacing();
                    ui.text("shift-click to enable");
                }
                if let Some(draw) = &mut draw_tooltip {
                    draw(2);
                }
            });
        }
        toggled && ui.io().key_shift
    }
}
impl PathingWindowState {
    /// create a dead zone for the mouse to rest without triggering a menu change
    const MENU_DEAD_ZONE: Vec2 = Vec2::new(2.0, 2.0);
    fn dead_zone_spacing(ui: &Ui, branch: bool) {
        let _vspace = ui.push_style_var(StyleVar::ItemSpacing([0.2, 0.2]));
        let sz = match branch {
            true => Self::MENU_DEAD_ZONE,
            false => Self::MENU_DEAD_ZONE / 2.0,
        };
        if Vec2::from(ui.cursor_pos()).y > Vec2::from(ui.cursor_start_pos()).y + Self::MENU_DEAD_ZONE.y {
            // create a dead zone for the mouse to rest without triggering a menu change
            ui.dummy(sz.to_array());
        }
    }
}
