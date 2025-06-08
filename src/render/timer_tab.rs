use {
    super::Alignment,
    crate::{
        controller::ControllerEvent,
        fl,
        render::RenderState,
        settings::{RemoteSource, TimerSettings},
        timer::TimerFile,
        CONTROLLER_SENDER, SETTINGS, TIMERS_DIR,
    },
    glam::Vec2,
    indexmap::IndexMap,
    nexus::imgui::{ChildWindow, Condition, Selectable, TreeNode, TreeNodeFlags, Ui, WindowFlags},
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
};

pub struct TimerTabState {
    timers: Vec<Arc<TimerFile>>,
    categories: IndexMap<String, Vec<Arc<TimerFile>>>,
    pub timer_selection: Option<Arc<TimerFile>>,
    category_status: HashSet<String>,
    sources_to_timers: IndexMap<Arc<RemoteSource>, Vec<Arc<TimerFile>>>,
    //search_string: String,
}

impl TimerTabState {
    pub fn new() -> Self {
        Self {
            timers: Default::default(),
            categories: Default::default(),
            timer_selection: Default::default(),
            category_status: Default::default(),
            sources_to_timers: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &Ui, state_errors: &mut HashMap<String, anyhow::Error>) {
        ui.columns(2, "timers_tab_start", true);
        self.draw_sidebar(ui, state_errors);
        ui.next_column();
        self.draw_main(ui);
        ui.columns(1, "timers_tab_end", false)
    }

    fn draw_sidebar(&mut self, ui: &Ui, state_errors: &mut HashMap<String, anyhow::Error>) {
        self.draw_sidebar_header(ui, state_errors);
        self.draw_sidebar_child(ui);
    }

    fn draw_sidebar_header(&mut self, ui: &Ui, state_errors: &mut HashMap<String, anyhow::Error>) {
        RenderState::draw_open_button(
            state_errors,
            ui,
            fl!("open-button", kind = "ad-hoc folder"),
            TIMERS_DIR.to_string_lossy(),
        );
        ui.same_line();
        if ui.button(fl!("reload-timers")) {
            let sender = CONTROLLER_SENDER.get().unwrap();
            let event_send = sender.try_send(ControllerEvent::ReloadTimers);
            drop(event_send);
        }
        /*let button_text = match timer_window_state.open {
            true => "Close Timers",
            false => "Open Timers",
        };
        if ui.button(button_text) {
            timer_window_state.open = !timer_window_state.open;
            let sender = CONTROLLER_SENDER.get().unwrap();
            let event_send = sender.try_send(ControllerEvent::WindowState(
                "timers".to_string(),
                Some(timer_window_state.open),
            ));
            drop(event_send);
        }
        ui.same_line();
        if ui.button("Reset Timers") {
            let sender = CONTROLLER_SENDER.get().unwrap();
            let event_send = sender.try_send(ControllerEvent::TimerReset);
            drop(event_send);
            timer_window_state.reset_phases();
        }*/
        if self.category_status.len() != self.categories.keys().len() {
            if ui.button(&fl!("expand-all")) {
                self.category_status.extend(self.categories.keys().cloned());
            }
        }
        if self.category_status.len() != self.categories.keys().len()
            && !self.category_status.is_empty()
        {
            ui.same_line();
        }
        if !self.category_status.is_empty() {
            if ui.button(&fl!("collapse-all")) {
                self.category_status.clear();
            }
        }
        //InputText::new(ui, "Search", &mut self.search_string);
    }

    fn draw_sidebar_child(&mut self, ui: &Ui) {
        let child_window_flags = WindowFlags::HORIZONTAL_SCROLLBAR;
        ChildWindow::new("timer_sidebar")
            .flags(child_window_flags)
            .size([0.0, 0.0])
            .build(ui, || {
                let header_flags = TreeNodeFlags::FRAMED;
                // interface design is my passion
                let height = Vec2::from_array(ui.calc_text_size("U\nI"));
                let height = height.y;
                for idx in 0..self.categories.len() {
                    self.draw_category(ui, header_flags, height, idx);
                }
            });
    }

    fn draw_category(&mut self, ui: &Ui, header_flags: TreeNodeFlags, height: f32, idx: usize) {
        let (category_name, category) = self
            .categories
            .get_index(idx)
            .expect("given an incorrect index for the category");
        let category_closure = || {
            ui.dummy([0.0, 4.0]);
            for timer in category {
                let mut selected = false;
                if let Some(selected_timer) = &self.timer_selection {
                    selected = Arc::ptr_eq(selected_timer, timer);
                }
                let element_selected = Self::draw_timer(ui, height, timer, selected);
                if element_selected && element_selected != selected {
                    self.timer_selection = Some(timer.clone());
                }
            }
        };
        let tree_node = TreeNode::new(category_name)
            .flags(header_flags)
            .opened(
                self.category_status.contains(category_name),
                Condition::Always,
            )
            .tree_push_on_open(false)
            .build(ui, category_closure);
        match tree_node {
            Some(_) => {
                self.category_status.insert(category_name.to_string());
            }
            None => {
                self.category_status.remove(category_name);
            }
        }
    }

    fn draw_timer(ui: &Ui, height: f32, timer: &Arc<TimerFile>, selected_in: bool) -> bool {
        let mut selected = selected_in;
        let group_token = ui.begin_group();
        let widget_pos = Vec2::from(ui.cursor_pos());
        let window_size = Vec2::from(ui.window_content_region_max());
        let widget_size = window_size.with_y(height);
        RenderState::icon(ui, Some(height), Some(&timer.icon), timer.path.as_ref());
        if Selectable::new(&timer.combined())
            .selected(selected)
            .build(ui)
        {
            selected = true;
        }
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            let settings_for_timer = settings.timers.get(&timer.id);
            ui.same_line();
            let (color, text) = match settings_for_timer {
                Some(TimerSettings { disabled: true, .. }) => {
                    ([1.0, 0.0, 0.0, 1.0], &fl!("disabled"))
                }
                _ => ([0.0, 1.0, 0.0, 1.0], &fl!("enabled")),
            };
            let text_size = Vec2::from(ui.calc_text_size(text));
            Alignment::set_cursor(
                ui,
                Alignment::RIGHT_MIDDLE,
                widget_pos,
                widget_size,
                text_size,
            );
            ui.text_colored(color, text);
        }
        ui.dummy([0.0, 4.0]);
        group_token.end();
        selected
    }

    fn draw_main(&mut self, ui: &Ui) {
        let child_window_flags = WindowFlags::HORIZONTAL_SCROLLBAR;
        ChildWindow::new("timer_main")
            .flags(child_window_flags)
            .size([0.0, 0.0])
            .build(ui, || {
                if let Some(selected_timer) = &self.timer_selection {
                    RenderState::icon(
                        ui,
                        None,
                        Some(&selected_timer.icon),
                        selected_timer.path.as_ref(),
                    );
                    ui.same_line();
                    let split_name = selected_timer.name.split("\n");
                    let layout_group = ui.begin_group();
                    for (i, text) in split_name.into_iter().enumerate() {
                        if i == 0 {
                            RenderState::font_text("big", ui, text);
                        } else {
                            RenderState::font_text("ui", ui, text);
                        }
                    }
                    layout_group.end();
                    RenderState::font_text(
                        "font",
                        ui,
                        &fl!("author-arg", author = selected_timer.author()),
                    );
                    if !selected_timer.source().is_empty() {
                        RenderState::font_text(
                            "font",
                            ui,
                            &fl!("source-arg", source = selected_timer.source()),
                        );
                    } else {
                        RenderState::font_text("font", ui, &fl!("source-adhoc"));
                    }
                    if let Some(path) = &selected_timer.path {
                        let path_display = format!("{}", path.display());
                        RenderState::font_text("font", ui, &fl!("location", path = path_display));
                    }
                    RenderState::font_text(
                        "font",
                        ui,
                        &fl!("id-arg", id = selected_timer.id.clone()),
                    );
                    RenderState::font_text(
                        "font",
                        ui,
                        &fl!("map-id-arg", id = selected_timer.map_id),
                    );
                    ui.dummy([4.0; 2]);
                    ui.separator();
                    ui.dummy([4.0; 2]);
                    RenderState::font_text("font", ui, &selected_timer.description);
                    ui.dummy([4.0; 2]);
                    ui.separator();
                    ui.dummy([4.0; 2]);
                    if let Some(settings) =
                        SETTINGS.get().and_then(|settings| settings.try_read().ok())
                    {
                        let settings_for_timer = settings.timers.get(&selected_timer.id);
                        let button_text = match settings_for_timer {
                            Some(TimerSettings { disabled: true, .. }) => &fl!("enable"),
                            _ => &fl!("disable"),
                        };
                        if ui.button(button_text) {
                            let sender = CONTROLLER_SENDER.get().unwrap();
                            let event_send = sender
                                .try_send(ControllerEvent::TimerToggle(selected_timer.id.clone()));
                            drop(event_send);
                        }
                    }
                } else {
                    ui.text(&fl!("select-a-timer"));
                }
            });
    }
    pub fn timers_update(&mut self, timers: Vec<Arc<TimerFile>>) {
        self.timers = timers;
        self.sources_to_timers.clear();
        self.categories.clear();
        for timer in &self.timers {
            if let Some(association) = &timer.association {
                self.sources_to_timers
                    .entry(association.clone())
                    .or_default();
                if let Some(val) = self.sources_to_timers.get_mut(association) {
                    val.push(timer.clone());
                };
            }
            self.categories.entry(timer.category.clone()).or_default();
            if let Some(val) = self.categories.get_mut(&timer.category) {
                val.push(timer.clone());
            };
        }
        self.categories.sort_keys();
    }
}
