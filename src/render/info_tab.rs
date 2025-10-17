use {
    super::TimerWindowState,
    crate::{built_info, fl, render::RenderState, TEXTURES, ControllerEvent, Controller},
    nexus::imgui::{TableColumnSetup, Ui, StyleColor},
};

#[cfg(feature = "space")]
use crate::engine_ref;

pub struct InfoTabState {}

impl InfoTabState {
    pub fn new() -> Self {
        Self {}
    }

    pub fn draw(&self, ui: &Ui, timer_window_state: &TimerWindowState) {
        let name = env!("CARGO_PKG_NAME");
        let authors = env!("CARGO_PKG_AUTHORS");
        let version = env!("CARGO_PKG_VERSION");
        let profile = match () {
            #[cfg(debug_assertions)]
            _ => "debug",
            #[cfg(not(debug_assertions))]
            _ => "release",
        };

        let project_heading = format!("{}, {} by {}", name, version, authors);
        RenderState::font_text("big", ui, &project_heading);

        let in_ci = match built_info::CI_PLATFORM {
            Some(platform) => format!(" using {platform}"),
            None => "".to_string(),
        };
        if let (Some(git_head_ref), Some(git_hash)) =
            (built_info::GIT_HEAD_REF, built_info::GIT_COMMIT_HASH_SHORT)
        {
            let mut build = format!("Built from {}@{}", git_head_ref, git_hash);
            build.push_str(&in_ci);
            build.push_str(&format!(", in profile \"{profile}\""));
            build.push('.');
            ui.text_wrapped(build);
        }
        ui.dummy([4.0, 4.0]);
        ui.text_wrapped(fl!("having-issues"));
        ui.dummy([4.0, 4.0]);
        let path = fl!("discord-link");
        let color_token = ui.push_style_color(
            StyleColor::Button,
            [0.0, 0.5, 0.8, 1.0]
        );
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
        ui.separator();
        RenderState::font_text("ui", ui, &fl!("active-timer-phases"));
        let table_token = ui.begin_table_header(
            "phase_states",
            [
                TableColumnSetup::new(&fl!("timer")),
                TableColumnSetup::new(&fl!("phase")),
            ],
        );
        ui.table_next_column();
        for phase_state in &timer_window_state.phase_states {
            let phase = phase_state.phase.phase();
            ui.text_wrapped(phase_state.timer.hypheny_name());
            ui.table_next_column();
            ui.text_wrapped(&phase.name);
            ui.table_next_column();
        }
        drop(table_token);
        #[cfg(feature = "space")]
        self.space_info(ui);
        if let Ok(tex_count) = TEXTURES.textures.try_read().map(|t| t.len()) {
            ui.text(&fl!("textures", count = tex_count));
        }
        #[cfg(feature = "texture-loader")]
        if let Some(tex_count) = crate::resources::texture::STATS_TEXTURE_COUNT.get_any() {
            use crate::resources::texture;
            ui.text(&fl!("d3d-textures", count = tex_count));
            if let Some(tex_size) = texture::STATS_TEXTURE_SIZE.get_any() {
                ui.same_line(); ui.text(", ");
                ui.same_line(); ui.text(Self::size_frag(tex_size));
            }
            if let Some(tex_size_cloned) = texture::STATS_TEXTURE_SIZE_CLONED.get_any() {
                ui.same_line(); ui.text(" - <=");
                ui.same_line(); ui.text(Self::size_frag(tex_size_cloned));
            }
        }
        #[cfg(feature = "allocator")]
        if let Some(alloc_size) = crate::exports::runtime::allocator::STATS_ALLOC_SIZE.get_any() {
            let size = Self::size_frag(alloc_size);
            ui.text(&fl!("alloc-size", size = size));
        }
    }

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
            .. Default::default()
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

    #[cfg(feature = "space")]
    pub fn space_info(&self, ui: &Ui) {
        use {
            crate::space::pack::{pack, poi, trail},
            std::sync::atomic::Ordering,
        };

        RenderState::font_text("big", ui, &fl!("engine"));
        engine_ref(|engine| {
            RenderState::font_text("ui", ui, &fl!("ecs-data"));
            let entities = engine.world.entities();
            let used_entities = entities.used_count();
            let total_entities = entities.total_count();
            ui.text(format!("Used: {}", used_entities));
            ui.text(format!("Total: {}", total_entities));
            #[cfg(feature = "space-ecs")]
            {
                RenderState::font_text("ui", ui, &fl!("object-data"));
                let table_token = ui.begin_table_header(
                    "object_types",
                    [TableColumnSetup::new(&fl!("object-kind"))],
                );
                ui.table_next_column();
                for object in engine.object_kinds.keys() {
                    ui.text(object);
                    ui.table_next_column();
                }
                drop(table_token);
                RenderState::font_text("ui", ui, &fl!("model-files"));
                #[cfg(feature = "space-ecs")]
                let table_token = ui.begin_table_header(
                    "model_files",
                    [
                        TableColumnSetup::new(&fl!("name")),
                        TableColumnSetup::new(&fl!("path")),
                        TableColumnSetup::new(&fl!("vertices")),
                    ],
                );
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

            RenderState::font_text("ui", ui, "Pathing Stats");
            let pack_entity_total = pack::STATS_ENTITY_COUNT.load(Ordering::Relaxed);
            let pack_entity_draw = pack::STATS_ENTITY_DRAW.load(Ordering::Relaxed);
            let pack_entity_draw_map = pack::STATS_ENTITY_DRAW_MAP.load(Ordering::Relaxed);
            ui.text(format!("Drawn: {}", pack_entity_draw));
            ui.text(format!("Mapped: {}", pack_entity_draw_map));
            ui.text(format!("Total: {}", pack_entity_total));
            if let Some(size) = trail::STATS_TRAIL_VERTEX_SIZE.get_any() {
                let trail = fl!("trail");
                let vertices = fl!("vertices");
                let size = Self::size_frag(size);
                let size = fl!("alloc-size", size = size);
                ui.text(&format!("{trail} {vertices} {size}"));
            }
            if let Some(size) = poi::STATS_POI_INSTANCE_SIZE.get_any() {
                let size = Self::size_frag(size);
                let size = fl!("alloc-size", size = size);
                ui.text(&format!("POI Instance Buffer {size}"));
            }
        });
    }
}
