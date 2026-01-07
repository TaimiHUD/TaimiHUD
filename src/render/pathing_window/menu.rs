use {
    super::PathingWindowState,
    crate::{render::machine::RenderMachine, with_i18n},
    glam::Vec2,
    nexus::imgui::{StyleVar, Ui},
    taimi_pack::category::CategoryFlags,
};

#[derive(Default)]
struct CategoryMenuContext {
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
        for (pack_idx, pack) in machine.pack_ui_state.pack_state.iter_mut() {
            let _id_pack = ui.push_id(pack.state.ui_id());
            let pack_info = pack.state.info.info.as_ref();
            let cats = pack_info.map(|i| i.categories.as_ref());
            let roots = cats.into_iter().flat_map(|cats| {
                cats.root_paths()
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
            let mut ctx: CategoryMenuContext = Default::default();
            ctx.filtered = unfiltered;
            drop(roots);
            pack.draw_menu(ui);
        }
    }

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
