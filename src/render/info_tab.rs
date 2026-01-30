use {
    super::TimerWindowState,
    crate::{
        built_info,
        exports::runtime::{self as rt, statistics},
        render::{
            element::prelude::*,
            machine::{RenderMachine, RenderSlot},
        },
        Controller,
        ControllerEvent,
        TEXTURES,
    },
};

pub struct InfoTabState {
    authors: String,
}

impl InfoTabState {
    pub fn new() -> Self {
        Self { authors: rt::crate_authors() }
    }

    pub fn regen_authors(&mut self) {
        self.authors = rt::crate_authors();
    }

    pub fn draw<'ui, U>(&self, ui: &mut U, timer_window_state: &TimerWindowState, slot: RenderSlot)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let version = match () {
            #[cfg(feature = "updates")]
            _ => {
                let mut version = rt::update::CRATE_SEMVER.clone();
                version.build = Default::default();
                version
            },
            #[cfg(not(feature = "updates"))]
            _ => rt::CRATE_VERSION,
        };

        let cursor_top = ui.cursor_screen_pos();
        let heading = crate::exports::addon_title!();
        let project_heading = fmt_args!(&"{heading}, {version}");
        let subheading = {
            let authors = &self.authors;
            fmt_args!("by {authors}")
        };
        let project_heading_;
        let (project_heading, subheading) = match subheading {
            subh if heading.contains("(") || !version.pre.is_empty() =>
                (project_heading.as_dyn(), Some(subh.display_imstr())),
            subh => {
                project_heading_ = fmt_args!("{project_heading} {subh}");
                (project_heading_.as_dyn(), None)
            },
        };
        ui.text_with_font(NexusLinkFont::Big, project_heading.display_imstr());

        let wrap_limit = if let Some(Some(logo)) = TEXTURES.lookup_imgui(RenderMachine::TEXTURE_LOGO_KEY) {
            const MIN_LOGO_WIDTH: f32 = 128.0;
            const LOGO_UV1: ImVec2<f32> = {
                let keytype = RenderMachine::TEXTURE_LOGO_KEY.as_bytes();
                let typeoff = keytype.len() - 7;
                let key = [keytype[typeoff], keytype[typeoff + 1], keytype[typeoff + 2]];
                match key {
                    [b'g', b'l', b'o'] => ImVec2::new(1.0, 0.65),
                    _ => ImVec2::new(1.0, 0.5),
                }
            };
            let heading_right = ui.item_rect_max().x;
            let cursor_restore = ui.cursor_screen_pos();
            let avail = ui.content_region_avail();
            let logo_size = logo.im_size() * LOGO_UV1.to_size().cast();
            let size = match avail.width * 0.4 {
                max_logo_width if max_logo_width < MIN_LOGO_WIDTH => None,
                max_logo_width if max_logo_width < logo_size.width => Some(ImSize2::new(
                    max_logo_width,
                    logo_size.height * max_logo_width / logo_size.width,
                )),
                _ => Some(logo_size),
            };
            if let Some(size) = size {
                let right_align = ui.window_pos().x + avail.width - size.width;
                let logo_pos = match avail.width - (heading_right - cursor_restore.x) {
                    heading_avail if heading_avail > logo_size.width => cursor_top
                        //.with_x(heading_right)
                        .with_x(right_align),
                    _ => cursor_restore.with_x(right_align),
                };
                ui.set_cursor_screen_pos(logo_pos);
                let logo_tint = match () {
                    _ => ImColourIndex::ButtonHovered,
                    #[cfg(todo)]
                    _ => ImColourIndex::TextDisabled,
                };
                let mut texture = logo.tinted(ui.lookup_style_colour(logo_tint));
                texture.uv.max = LOGO_UV1.into();
                ui.image(texture, size);
                let wrap_y = ui.item_rect_max().y;
                ui.set_cursor_screen_pos(cursor_restore);
                let logo_pos_x = logo_pos.x - ui.window_pos().x;
                let wrap_limit = ui.push_text_wrap_pos_with_pos(logo_pos_x);
                Some((wrap_limit, wrap_y))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(subheading) = subheading {
            ui.text_with_font(NexusLinkFont::Big, subheading);
        }

        let in_ci = built_info::CI_PLATFORM.map(|platform| fmt_args!(" via {platform}"));
        if let (Some(git_head_ref), Some(git_hash)) =
            (built_info::GIT_HEAD_REF, built_info::GIT_COMMIT_HASH_SHORT)
        {
            ui.text_wrapped(im_fmt!("Built from {git_head_ref}@{git_hash}{}.", fmt_opt(in_ci)));
        }
        ui.dummy([4.0, 4.0]);
        ui.text_wrapped(im_fmt!(i18n: "having-issues"));
        ui.dummy([4.0, 4.0]);
        let path = fl!("discord-link");
        let color_token = ui.push_colour(ImColourIndex::Button, [0.0, 0.5, 0.8, 1.0]);
        if ui.button(im_fmt!(i18n: "join-discord")) {
            log::debug!("Triggered open Discord join");
            Controller::try_send(ControllerEvent::OpenOpenable(
                fl!("join-discord").into(),
                fl!("discord-link").into(),
            ));
        }
        color_token.end();
        if ui.is_item_hovered() {
            ui.tooltip_text(fl!("location", [path = path]));
        }
        ui.dummy([4.0, 4.0]);
        let description = env!("CARGO_PKG_DESCRIPTION");
        ui.text_wrapped(description);
        ui.dummy([4.0, 4.0]);

        ui.text_wrapped(fl!("keybind-triggers"));

        if let Some((wrap_limit, wrap_y)) = wrap_limit {
            wrap_limit.end();
            let pos = ui.cursor_screen_pos();
            if wrap_y > pos.y {
                ui.set_cursor_screen_pos(pos.with_y(wrap_y))
            }
        }

        ui.separator();
        ui.text_with_font(NexusLinkFont::Ui, fl!("active-timer-phases"));
        let cols = ["timer", "phase"];
        let table_timers = ui.begin_table_with_flags(c"phase_states", cols.len(), Default::default());
        if let Some(_table) = table_timers {
            for id in cols {
                let user_id = 0;
                with_i18n!(id, |label| ui.table_column_setup_untyped(
                    Some(label),
                    None,
                    None,
                    user_id
                ));
            }
            ui.table_header_row();
            ui.table_next_column();
            for phase_state in &timer_window_state.phase_states {
                let phase = &phase_state.phase;
                ui.text_wrapped(phase_state.timer().hypheny_name());
                ui.table_next_column();
                ui.text_wrapped(&phase.name);
                ui.table_next_column();
            }
        }
        self.stats_table(ui);
        #[cfg(feature = "space")]
        self.space_info(ui, slot);
        if let Ok(tex_count) = TEXTURES.textures.try_read().map(|t| t.len()) {
            ui.text(fl!("textures", count = tex_count));
        }
        #[cfg(deleteme)]
        #[cfg(feature = "texture-loader")]
        if let Some(tex_count) = crate::resources::texture::STATS_TEXTURE_COUNT.get_any() {
            use crate::resources::texture;
            ui.text(fl!("d3d-textures", count = tex_count));
            if let Some(tex_size) = texture::STATS_TEXTURE_SIZE.get_any() {
                ui.same_line();
                ui.text(", ");
                ui.same_line();
                ui.text(Self::size_frag(tex_size));
            }
            if let Some(tex_size_cloned) = texture::STATS_TEXTURE_SIZE_CLONED.get_any() {
                ui.same_line();
                ui.text(" - <=");
                ui.same_line();
                ui.text(Self::size_frag(tex_size_cloned));
            }
        }
        #[cfg(deleteme)]
        #[cfg(feature = "allocator")]
        if let Some(alloc_size) = crate::exports::runtime::allocator::STATS_ALLOC_SIZE.get_any() {
            let size = Self::size_frag(alloc_size);
            ui.text(fl!("alloc-size", size = size));
        }
    }


    #[cfg(deleteme)]
    #[cfg(any(feature = "allocator", feature = "texture-loader", feature = "space"))]
    fn size_frag(size: isize) -> impl ImStrExt + fmt::Display + Copy + Into<fluent::FluentValue<'static>> {
        // once we have a working formatter...
        #[cfg(todo)]
        const SIZE_MB: f64 = 0x10_0000u32 as f64;
        #[cfg(todo)]
        const SIZE_KB: f64 = 0x400u32 as f64;
        const SIZE_KB: f64 = 1000.0;
        const SIZE_MB: f64 = SIZE_KB * 1000.0;
        const MIN_MB: f64 = SIZE_MB * 0.9;

        #[cfg(todo)]
        let opts = fluent::types::FluentNumberOptions {
            minimum_significant_digits: Some(3),
            maximum_significant_digits: Some(5),
            maximum_fraction_digits: Some(4),
            ..Default::default()
        };
        let (id, size) = match size as f64 {
            size if size >= MIN_MB => {
                #[cfg(todo)]
                let value = fluent::types::FluentNumber::new(size / SIZE_MB, opts);
                let value = (size / SIZE_KB).round() / 1000.0;
                ("size-frag-mb", value)
            },
            size => {
                #[cfg(todo)]
                let value = fluent::types::FluentNumber::new(size, opts);
                let value = size.round() / 1000.0;
                ("size-frag-kb", value)
            },
        };
        im_fmt!(i18n: id => move size = size)
    }

    /// TODO: TreeNode sections
    pub fn stats_table(&self, ui: &Ui) {
        if let Ok(stats) = statistics::StatsRef::registry().try_read() {
            if stats.is_empty() { return }
            with_i18n!("stats", |label| ui.text_with_font(NexusLinkFont::Big, label));
            let _table = ui.begin_table("stats", 2);
            let mut section_prev = 0usize;
            for (desc, counter) in stats.iter() {
                let value = match counter.read() {
                    0 => continue,
                    v => v,
                };
                let section = desc.section.as_ptr() as usize;
                if section_prev != section {
                    section_prev = section;
                    ui.table_next_column();
                    with_i18n!(desc.section, |label| ui.table_header(label));
                    ui.table_next_column();
                }
                ui.indent();
                ui.table_next_column();
                let display = counter.unit.display_value(value);
                with_i18n!(desc.name, |label| ui.display_with_font(&NexusLinkFont::Ui, &format_args!("{label}:")));
                ui.table_next_column();
                ui.display_with_font(&NexusLinkFont::Ui, &display);
                ui.unindent();
            }
        }
    }
    #[cfg(feature = "space")]
    pub fn space_info<'ui, U>(&self, ui: &mut U, (engine,): RenderSlot)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        use {crate::space::pack, std::sync::atomic::Ordering};

        if let Some(Ok(engine)) = engine {
            ui.text_with_font(NexusLinkFont::Big, fl!("engine"));
            ui.text_with_font(NexusLinkFont::Ui, fl!("ecs-data"));
            let entities = engine.world.entities();
            let used_entities = entities.used_count();
            let total_entities = entities.total_count();
            ui.text(im_fmt!("Used: {}", used_entities));
            ui.text(im_fmt!("Total: {}", total_entities));
            #[cfg(feature = "space-ecs")]
            {
                let table_flags = match ui.imgui_version_num() {
                    #[cfg(taimi_imgui = "180")]
                    Some(im180::VERSION_NUM) => imw::Table::IM180_ARGS_PRESET,
                    #[cfg(taimi_imgui = "192")]
                    Some(im192::VERSION_NUM) => imw::Table::IM192_ARGS_PRESET,
                    _ => Default::default(),
                };

                let table_token = (!engine.object_kinds.is_empty()).then(|| {
                    ui.text_with_font(NexusLinkFont::Ui, fl!("object-data"));
                    let cols = [fl!("object-kind")];
                    ui.begin_table_with_flags(c"object_types", cols.len(), table_flags)
                        .map(|tok| (tok, cols))
                });
                if let Some((_table, cols)) = table_token.flatten() {
                    for label in cols {
                        let user_id = 0;
                        ui.table_column_setup_untyped(Some(label), Default::default(), None, user_id);
                    }
                    ui.table_header_row();
                    ui.table_next_column();
                    for object in engine.object_kinds.keys() {
                        ui.text(&object[..]);
                        ui.table_next_column();
                    }
                }
                let table_token = (!engine.model_files.is_empty()).then(|| {
                    ui.text_with_font(NexusLinkFont::Ui, fl!("model-files"));
                    let cols = [fl!("name"), fl!("path"), fl!("vertices")];
                    ui.begin_table_with_flags(c"model_files", cols.len(), table_flags)
                        .map(|tok| (tok, cols))
                });
                if let Some((_table, cols)) = table_token.flatten() {
                    for label in cols {
                        let user_id = 0;
                        ui.table_column_setup_untyped(Some(label), Default::default(), None, user_id);
                    }
                    ui.table_header_row();
                    ui.table_next_column();

                    for (path, file) in &engine.model_files {
                        for model in &file.models {
                            ui.text(im_fmt!("{:?}", path));
                            ui.table_next_column();
                            ui.text(&model.0.name);
                            ui.table_next_column();
                            ui.text(im_to_s!(model.0.mesh.positions.len() / 3));
                            ui.table_next_column();
                        }
                    }
                }
            }
        }
    }
}
