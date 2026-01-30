use {
    super::TimerWindowState,
    crate::{
        built_info,
        exports::runtime::{self as rt, statistics},
        fl,
        render::{machine::RenderMachine, element::im::prelude::*, RenderState},
        Controller,
        ControllerEvent,
        TEXTURES,
    },
    glam::Vec2,
    nexus::imgui::{Image, StyleColor, TableColumnSetup, Ui},
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

    pub fn draw(&self, ui: &Ui, timer_window_state: &TimerWindowState) {
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
        let mut project_heading = format!("{heading}, {version}");
        let subheading = match format_args!("by {}", self.authors) {
            subh if heading.contains("(") || !version.pre.is_empty() => Some(subh.to_string()),
            subh => {
                use core::fmt::Write;
                let _ = write!(&mut project_heading, " {subh}");
                None
            },
        };
        RenderState::font_text("big", ui, &project_heading);

        let wrap_limit = if let Some(Some(logo)) = TEXTURES.lookup_imgui(RenderMachine::TEXTURE_LOGO_KEY) {
            const MIN_LOGO_WIDTH: f32 = 128.0;
            const LOGO_UV1: Vec2 = Vec2::new(1.0, 0.5);
            let heading_right = ui.item_rect_max()[0];
            let cursor_restore = Vec2::from_array(ui.cursor_screen_pos());
            let avail = Vec2::from_array(ui.content_region_avail());
            let logo_size = Vec2::from_array(logo.size) * LOGO_UV1;
            let size = match avail.x * 0.4 {
                max_logo_width if max_logo_width < MIN_LOGO_WIDTH => None,
                max_logo_width if max_logo_width < logo_size.x => Some(Vec2::new(
                    max_logo_width,
                    logo_size.y * max_logo_width / logo_size.x,
                )),
                _ => Some(logo_size),
            };
            if let Some(size) = size {
                let right_align = ui.window_pos()[0] + avail.x - size.x;
                let logo_pos = match avail.x - (heading_right - cursor_restore.x) {
                    heading_avail if heading_avail > logo_size.x => Vec2::from_array(cursor_top)
                        //.with_x(heading_right)
                        .with_x(right_align),
                    _ => cursor_restore.with_x(right_align),
                };
                ui.set_cursor_screen_pos(logo_pos.to_array());
                Image::new(logo.id, size.to_array())
                    .uv1(LOGO_UV1.to_array())
                    .build(ui);
                let wrap_y = ui.item_rect_max()[1];
                ui.set_cursor_screen_pos(cursor_restore.to_array());
                let logo_pos_x = logo_pos.x - ui.window_pos()[0];
                let wrap_limit = ui.push_text_wrap_pos_with_pos(logo_pos_x);
                Some((wrap_limit, wrap_y))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(subheading) = subheading {
            RenderState::font_text("big", ui, &subheading);
        }

        let in_ci = match built_info::CI_PLATFORM {
            Some(platform) => format!(" via {platform}"),
            None => "".to_string(),
        };
        if let (Some(git_head_ref), Some(git_hash)) =
            (built_info::GIT_HEAD_REF, built_info::GIT_COMMIT_HASH_SHORT)
        {
            let mut build = format!("Built from {}@{}", git_head_ref, git_hash);
            build.push_str(&in_ci);
            build.push('.');
            ui.text_wrapped(build);
        }
        ui.dummy([4.0, 4.0]);
        ui.text_wrapped(fl!("having-issues"));
        ui.dummy([4.0, 4.0]);
        let path = fl!("discord-link");
        let color_token = ui.push_style_color(StyleColor::Button, [0.0, 0.5, 0.8, 1.0]);
        if ui.button(fl!("join-discord")) {
            log::debug!("Triggered open Discord join");
            Controller::try_send(ControllerEvent::OpenOpenable(
                fl!("join-discord"),
                fl!("discord-link").into(),
            ));
        }
        color_token.pop();
        if ui.is_item_hovered() {
            ui.tooltip_text(fl!("location", path = path));
        }
        ui.dummy([4.0, 4.0]);
        let description = env!("CARGO_PKG_DESCRIPTION");
        ui.text_wrapped(description);
        ui.dummy([4.0, 4.0]);

        ui.text_wrapped(&fl!("keybind-triggers"));

        if let Some((wrap_limit, wrap_y)) = wrap_limit {
            wrap_limit.pop(ui);
            let pos = ui.cursor_screen_pos();
            if wrap_y > pos[1] {
                ui.set_cursor_screen_pos([pos[0], wrap_y])
            }
        }

        ui.separator();
        RenderState::font_text("ui", ui, &fl!("active-timer-phases"));
        let table_token = ui.begin_table_header("phase_states", [
            TableColumnSetup::new(&fl!("timer")),
            TableColumnSetup::new(&fl!("phase")),
        ]);
        ui.table_next_column();
        for phase_state in &timer_window_state.phase_states {
            let phase = phase_state.phase.phase();
            ui.text_wrapped(phase_state.timer.hypheny_name());
            ui.table_next_column();
            ui.text_wrapped(&phase.name);
            ui.table_next_column();
        }
        drop(table_token);
        self.stats_table(ui);
        #[cfg(feature = "space")]
        self.space_info(ui);
        if let Ok(tex_count) = TEXTURES.textures.try_read().map(|t| t.len()) {
            ui.text(&fl!("textures", count = tex_count));
        }
        #[cfg(deleteme)]
        #[cfg(feature = "texture-loader")]
        if let Some(tex_count) = crate::resources::texture::STATS_TEXTURE_COUNT.get_any() {
            use crate::resources::texture;
            ui.text(&fl!("d3d-textures", count = tex_count));
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
            ui.text(&fl!("alloc-size", size = size));
        }
    }


    #[cfg(deleteme)]
    #[cfg(any(feature = "allocator", feature = "texture-loader", feature = "space"))]
    fn size_frag(size: isize) -> String {
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
        match size as f64 {
            size if size >= MIN_MB => {
                #[cfg(todo)]
                let value = fluent::types::FluentNumber::new(size / SIZE_MB, opts);
                let value = (size / SIZE_KB).round() / 1000.0;
                fl!("size-frag-mb", size = value).into()
            },
            size => {
                #[cfg(todo)]
                let value = fluent::types::FluentNumber::new(size, opts);
                let value = size.round() / 1000.0;
                fl!("size-frag-kb", size = value)
            },
        }
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
    pub fn space_info(&self, ui: &Ui) {
        use {crate::space::pack, std::sync::atomic::Ordering};

        #[cfg(todo)]
        engine_ref(|engine| {
            RenderState::font_text("big", ui, &fl!("engine"));
            RenderState::font_text("ui", ui, &fl!("ecs-data"));
            let entities = engine.world.entities();
            let used_entities = entities.used_count();
            let total_entities = entities.total_count();
            ui.text(format!("Used: {}", used_entities));
            ui.text(format!("Total: {}", total_entities));
            #[cfg(feature = "space-ecs")]
            {
                RenderState::font_text("ui", ui, &fl!("object-data"));
                let table_token =
                    ui.begin_table_header("object_types", [TableColumnSetup::new(&fl!("object-kind"))]);
                ui.table_next_column();
                for object in engine.object_kinds.keys() {
                    ui.text(object);
                    ui.table_next_column();
                }
                drop(table_token);
                RenderState::font_text("ui", ui, &fl!("model-files"));
                #[cfg(feature = "space-ecs")]
                let table_token = ui.begin_table_header("model_files", [
                    TableColumnSetup::new(&fl!("name")),
                    TableColumnSetup::new(&fl!("path")),
                    TableColumnSetup::new(&fl!("vertices")),
                ]);
                ui.table_next_column();
                for (path, file) in &engine.model_files {
                    for model in &file.models {
                        ui.text(format!("{:?}", path));
                        ui.table_next_column();
                        ui.text(&model.0.name);
                        ui.table_next_column();
                        ui.text(format!("{}", model.0.mesh.positions.len() / 3));
                        ui.table_next_column();
                    }
                }
                drop(table_token);
            }
        });
    }
}
