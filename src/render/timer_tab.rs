use {
    super::Alignment,
    crate::{
        controller::timers::{TimersController, TimersEvent},
        render::{element::prelude::*, RenderState},
        settings::{Settings, SourceKind, TimerSettings},
        timer::TimerFile,
    },
    indexmap::IndexMap,
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
    sources_to_timers: IndexMap<String, Vec<Arc<TimerFile>>>,
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

    pub fn draw<'ui, U>(&mut self, ui: &mut U, state_errors: &mut HashMap<String, anyhow::Error>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        ui.columns(2, "timers_tab_start", true);
        self.draw_sidebar(ui, state_errors);
        ui.next_column();
        self.draw_main(ui);
        ui.columns(1, "timers_tab_end", false)
    }

    fn draw_sidebar<'ui, U>(&mut self, ui: &mut U, state_errors: &mut HashMap<String, anyhow::Error>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.draw_sidebar_header(ui, state_errors);
        self.draw_sidebar_child(ui);
    }

    fn draw_sidebar_header<'ui, U>(
        &mut self,
        ui: &mut U,
        _state_errors: &mut HashMap<String, anyhow::Error>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let timers_dir = SourceKind::Timers.get_user_dir();
        RenderState::draw_open_path_button(ui, fl!("open-button", kind = "ad-hoc folder"), &timers_dir);
        ui.same_line();
        if ui.button(fl!("reload-timers")) {
            TimersController::try_send(TimersEvent::ReloadTimers);
        }
        /*let button_text = match timer_window_state.open {
            true => "Close Timers",
            false => "Open Timers",
        };
        if ui.button(button_text) {
            timer_window_state.open = !timer_window_state.open;
            Controller::try_send(ControllerEvent::WindowState(
                "timers".to_string(),
                Some(timer_window_state.open),
            ));
        }
        ui.same_line();
        if ui.button("Reset Timers") {
            Controller::try_send(ControllerEvent::TimerReset);
            timer_window_state.reset_phases();
        }*/
        if self.category_status.len() != self.categories.keys().len() {
            if ui.button(fl!("expand-all")) {
                self.category_status.extend(self.categories.keys().cloned());
            }
        }
        if self.category_status.len() != self.categories.keys().len() && !self.category_status.is_empty() {
            ui.same_line();
        }
        if !self.category_status.is_empty() {
            if ui.button(fl!("collapse-all")) {
                self.category_status.clear();
            }
        }
        //InputText::new(ui, "Search", &mut self.search_string);
    }

    fn draw_sidebar_child<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let Some(_container) = ui.begin_sidebar(c"timer_sidebar") else { return };
        // interface design is my passion
        let ImSize2 { height, .. } = ui.calc_text_size("U\nI");
        for idx in 0..self.categories.len() {
            self.draw_category(ui, height, idx);
        }
    }

    fn draw_category<'ui, U>(&mut self, ui: &mut U, height: f32, idx: usize)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let (category_name, category) = self
            .categories
            .get_index(idx)
            .expect("given an incorrect index for the category");
        let tree_node = ui.begin_sidebar_tree_node(
            ImCondition::always(self.category_status.contains(category_name)),
            idx,
            category_name,
        );
        if let Some(_tree) = tree_node {
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
            self.category_status.insert(category_name.to_string());
        } else {
            self.category_status.remove(category_name);
        }
    }

    fn draw_timer<'ui, U>(ui: &mut U, height: f32, timer: &Arc<TimerFile>, selected_in: bool) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut selected = selected_in;
        let group_token = ui.begin_group();
        let widget_pos = ui.cursor_pos();
        let window_size = ui.window_region_size();
        let widget_size = window_size.with_y(height);
        RenderState::icon(
            ui,
            Some(height),
            Some(&timer.icon),
            timer.path.as_ref().and_then(|p| p.parent()),
        );
        selected |= ui.selectable(timer.combined(), selected);
        if let Some(settings) = Settings::try_read() {
            let settings_for_timer = settings.timers.get(&timer.id);
            ui.same_line();
            let (color, text) = match settings_for_timer {
                Some(TimerSettings { disabled: true, .. }) => ([1.0, 0.0, 0.0, 1.0], fl!("disabled")),
                _ => ([0.0, 1.0, 0.0, 1.0], fl!("enabled")),
            };
            let text_size = ui.calc_text_size(text);
            Alignment::set_cursor(ui, Alignment::RIGHT_MIDDLE, widget_pos, widget_size, text_size);
            ui.text_colored(color, text);
        }
        ui.dummy([0.0, 4.0]);
        group_token.end();
        selected
    }

    fn draw_main<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let Some(_container) = ui.begin_mainbar(c"timer_main") else { return };
        if let Some(selected_timer) = &self.timer_selection {
            RenderState::icon(
                ui,
                None,
                Some(&selected_timer.icon),
                selected_timer.path.as_ref().and_then(|p| p.parent()),
            );
            ui.same_line();
            let split_name = selected_timer.name.split("\n");
            let layout_group = ui.begin_group();
            for (i, text) in split_name.into_iter().enumerate() {
                if i == 0 {
                    ui.text_with_font(NexusLinkFont::Big, text);
                } else {
                    ui.text_with_font(NexusLinkFont::Ui, text);
                }
            }
            layout_group.end();
            if let _font_token = ui.push_font(NexusLinkFont::Font) {
                ui.text(fl!("author-arg", author = selected_timer.author()));
                if !selected_timer.source().is_empty() {
                    ui.text(fl!("source-arg", source = selected_timer.source()));
                } else {
                    ui.text(fl!("source-adhoc"));
                }
                if let Some(path) = &selected_timer.path {
                    let path_display = format!("{}", path.display());
                    ui.text(fl!("location", path = &path_display));
                }
                ui.text(fl!("id-arg", id = selected_timer.id.clone()));
                ui.text(fl!("map-id-arg", id = selected_timer.map_id));
                ui.dummy([4.0; 2]);
                ui.separator();
                ui.dummy([4.0; 2]);
                ui.text(&selected_timer.description);
                ui.dummy([4.0; 2]);
                ui.separator();
                ui.dummy([4.0; 2]);
            }
            if let Some(settings) = Settings::try_read() {
                let settings_for_timer = settings.timers.get(&selected_timer.id);
                let button_text = match settings_for_timer {
                    Some(TimerSettings { disabled: true, .. }) => fl!("enable"),
                    _ => fl!("disable"),
                };
                if ui.button(button_text) {
                    TimersController::try_send(TimersEvent::TimerToggle(selected_timer.id.clone()));
                }
            }
        } else {
            ui.text(fl!("select-a-timer"));
        }
    }
    pub fn timers_update(&mut self, timers: Vec<Arc<TimerFile>>) {
        self.timers = timers;
        self.sources_to_timers.clear();
        self.categories.clear();
        for timer in &self.timers {
            if let Some(association) = &timer.association {
                self.sources_to_timers.entry(association.clone()).or_default();
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
