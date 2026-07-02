use {
    super::PathingWindowState,
    crate::{
        render::{element::prelude::*, machine::RenderMachine},
        with_i18n,
    },
    taimi_pack::category::CategoryFlags,
};
#[cfg(feature = "paths-lua")]
use {
    crate::controller::script::{PlugMenusById, PlugMenusShared, ScriptMessage},
    core::mem,
    taimi_pack::category::{id::AsFullId, CategoryId},
};

impl PathingWindowState {
    pub fn draw_context_menu<'ui, U>(&mut self, ui: &mut U, machine: &mut RenderMachine)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.draw_context_menu_packs(ui, machine, false);
        if machine.pack_ui_state.any_loaded() {
            Self::dead_zone_spacing(ui, false);
            ui.separator();
            if let Some(_menu) = with_i18n!("show-all", |label| ui.begin_menu(&label)) {
                self.draw_context_menu_packs(ui, machine, true);
            }
        }
    }
    pub fn draw_context_menu_packs<'ui, U>(
        &mut self,
        ui: &mut U,
        machine: &mut RenderMachine,
        unfiltered: bool,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _id = ui.push_id(match unfiltered {
            false => c"packmenu-all",
            true => c"packmenu-active",
        });
        let mut was_multi_root = None;
        for (pack_idx, pack) in machine.pack_ui_state.pack_state.iter_mut() {
            let _id_pack = ui.push_id(pack.state.ui_id());
            let pack_info = pack.state.info.info.as_ref();
            let cats = pack_info.map(|i| i.categories.as_ref());
            let roots = cats.into_iter().flat_map(|cats| {
                cats.root_paths()
                    .filter_map(|path| cats.info_of(path).map(|_| (path, cats.lookup_flags(path))))
                    .filter(|(_p, flags)| !flags.contains(CategoryFlags::HIDDEN))
                    .filter_map(|(path, flags)| {
                        let filtered = match pack.categories.category_is_loaded(&pack.state, path) {
                            Some(false) if !unfiltered => return None,
                            None if !unfiltered => return None,
                            #[cfg(todo = "unnecessary")]
                            _ if filtered => None,
                            Some(f) => Some(!f),
                            None => Some(false),
                        };
                        Some((path, flags, filtered))
                    })
            });
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
            drop(roots);
            pack.draw_menu(ui);
            #[cfg(feature = "paths-lua")]
            {
                let script_menus = pack.state.plug.as_ref().map(|m| {
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
                            ScriptMessage::menu_clicked_with(clicked, pack_idx.pivot_from())
                                .try_send();
                        }
                    }
                }
            }
        }
    }
    #[cfg(feature = "scripts")]
    pub fn draw_context_menu_plugs<'ui, U>(
        &mut self,
        ui: &mut U,
        _machine: &mut RenderMachine,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
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
                    ScriptMessage::menu_clicked_with(clicked, path.pivot_from()).try_send();
                }
            }
        }
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
                if let Some(_token) = ui.begin_tooltip() {
                    if let Some(title) = &menu.tooltip_title {
                        ui.text(&title[..]);
                    }
                    if let Some(tip) = &menu.tooltip {
                        // TODO: text_wrapped
                        ui.text(&tip[..]);
                    }
                }
            }
            if let (true, true) = (clicked, commit.is_none()) {
                commit = Some(id.clone())
            }
        }
        commit
    }
}
