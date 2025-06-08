#[cfg(feature = "markers")]
use {crate::marker::atomic::MarkerInputData, crate::marker::format::MarkerSet};
use {
    crate::{
        controller::ControllerEvent,
        fl,
        load_texture_bytes, load_texture_path,
        marker::format::MarkerType,
        marker_icon_data,
        render::{MarkerWindowState, PrimaryWindowState, TimerWindowState},
        exports::runtime as rt,
        settings::ProgressBarSettings,
        timer::{PhaseState, TextAlert, TimerFile},
        CONTROLLER_SENDER, IMGUI_TEXTURES, RENDER_STATE,
    },
    glam::Vec2,
    nexus::imgui::{
        internal::RawCast, Condition, Font, Image, Io, PopupModal, StyleColor, Ui,
        Window, WindowFlags,
    },
    relative_path::RelativePathBuf,
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, MutexGuard},
    },
    strum_macros::{Display, EnumIter},
    tokio::sync::mpsc::Receiver,
};

#[cfg(feature = "markers-edit")]
use super::edit_marker_window::EditMarkerWindowState;

#[cfg(feature = "space")]
use super::PathingWindowState;

pub enum RenderEvent {
    TimerData(Vec<Arc<TimerFile>>),
    #[cfg(feature = "markers")]
    MarkerData(HashMap<String, Vec<Arc<MarkerSet>>>),
    MarkerMap(Vec<Arc<MarkerSet>>),
    AlertFeed(PhaseState),
    OpenableError(String, anyhow::Error),
    AlertReset(Arc<TimerFile>),
    AlertStart(TextAlert),
    AlertEnd(Arc<TimerFile>),
    CheckingForUpdates(bool),
    #[allow(dead_code)]
    RenderKeybindUpdate,
    #[cfg(feature = "markers-edit")]
    OpenEditMarkers(Option<MarkerSet>),
    #[cfg(feature = "markers-edit")]
    GiveMarkerPaths(Vec<PathBuf>),
    ProgressBarUpdate(ProgressBarSettings),
}

#[derive(Display, Default, Clone, Debug, Deserialize, Serialize, EnumIter, PartialEq)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TextFont {
    #[default]
    Fontless,
    Font,
    Ui,
    Big,
}

pub struct RenderState {
    pub primary_window: PrimaryWindowState,
    #[cfg(feature = "markers-edit")]
    pub edit_marker_window: EditMarkerWindowState,
    #[cfg(feature = "markers")]
    pub marker_window: MarkerWindowState,
    #[cfg(feature = "space")]
    pub pathing_window: PathingWindowState,
    timer_window: TimerWindowState,
    receiver: Receiver<RenderEvent>,
    alert: Option<TextAlert>,
    last_display_size: Option<[f32; 2]>,
    pub state_errors: HashMap<String, anyhow::Error>,
}

impl RenderState {
    pub fn new(receiver: Receiver<RenderEvent>) -> Self {
        Self {
            receiver,
            alert: Default::default(),
            primary_window: PrimaryWindowState::new(),
            timer_window: TimerWindowState::new(),
            #[cfg(feature = "markers-edit")]
            edit_marker_window: EditMarkerWindowState::new(),
            #[cfg(feature = "markers")]
            marker_window: MarkerWindowState::new(),
            #[cfg(feature = "space")]
            pathing_window: PathingWindowState::new(),
            last_display_size: Default::default(),
            state_errors: Default::default(),
        }
    }

    pub fn draw(&mut self, ui: &Ui) {
        let io = ui.io();
        if let Some(last_display_size) = self.last_display_size {
            if io.display_size != last_display_size {
                #[cfg(feature = "markers")]
                MarkerInputData::from_render(io.display_size.into());
                self.last_display_size = Some(io.display_size);
            }
        } else {
            #[cfg(feature = "markers")]
            MarkerInputData::from_render(io.display_size.into());
            self.last_display_size = Some(io.display_size);
        }
        match self.receiver.try_recv() {
            Ok(event) => {
                use RenderEvent::*;
                match event {
                    #[cfg(feature = "markers-edit")]
                    OpenEditMarkers(e) => match e {
                        None => self.edit_marker_window.open(),
                        Some(e) => self.edit_marker_window.open_edit(e),
                    },
                    #[cfg(feature = "markers")]
                    MarkerMap(markers) => {
                        self.marker_window.new_map_markers(markers);
                    }
                    #[cfg(feature = "markers-edit")]
                    GiveMarkerPaths(paths) => {
                        self.edit_marker_window.set_filenames(paths);
                    }
                    OpenableError(key, err) => {
                        self.state_errors.insert(key, err);
                    }
                    RenderKeybindUpdate => {
                        self.primary_window.keybind_handler();
                    }
                    ProgressBarUpdate(settings) => {
                        self.timer_window.progress_bar = settings;
                    }
                    CheckingForUpdates(checking_for_updates) => {
                        self.primary_window.data_sources_tab.checking_for_updates =
                            checking_for_updates;
                    }
                    TimerData(timers) => {
                        self.primary_window.timer_tab.timer_selection = None;
                        self.primary_window.timer_tab.timers_update(timers);
                    }
                    #[cfg(feature = "markers")]
                    MarkerData(markers) => {
                        self.primary_window.marker_tab.marker_selection = None;
                        let categories: Vec<_> = markers.keys().cloned().collect();
                        #[cfg(feature = "markers-edit")]
                        self.edit_marker_window.category_update(categories);
                        self.primary_window.marker_tab.marker_update(markers);
                    }
                    AlertStart(alert) => {
                        self.alert = Some(alert);
                    }
                    AlertEnd(timer_file) => {
                        if let Some(alert) = &self.alert {
                            if Arc::ptr_eq(&alert.timer, &timer_file) {
                                self.alert = None;
                            }
                        }
                    }
                    AlertFeed(phase_state) => {
                        self.timer_window.new_phase(phase_state);
                    }
                    AlertReset(timer) => {
                        self.timer_window.remove_phase(timer);
                    }
                }
            }
            Err(_error) => (),
        }
        self.handle_alert(ui, io);
        self.timer_window.draw(ui);
        self.primary_window
            .draw(ui, &mut self.timer_window, &mut self.state_errors);
        #[cfg(feature = "markers")]
        self.marker_window.draw(ui);
        #[cfg(feature = "markers-edit")]
        self.edit_marker_window.draw(ui);
        #[cfg(feature = "space")]
        self.pathing_window.draw(ui);
        let mut items_to_delete = Vec::new();
        for (entry_name, errory) in &self.state_errors {
            ui.open_popup(entry_name);
            if let Some(_token) = PopupModal::new(&entry_name)
                .always_auto_resize(true)
                .begin_popup(ui)
            {
                ui.text(format!("{:?}", errory));
                ui.dummy([4.0; 2]);
                if ui.button(fl!("okay")) {
                    items_to_delete.push(entry_name.clone());
                    ui.close_current_popup();
                }
            } else {
                ui.close_current_popup();
            }
        }
        for item in items_to_delete {
            self.state_errors.remove(&item);
        }
    }
    pub fn marker_icon(ui: &Ui, height: Option<f32>, marker: &MarkerType) {
        let gooey = IMGUI_TEXTURES.get().unwrap();
        if let Some(icon) = gooey.read().unwrap().get(&marker.to_string()) {
            let size = match height {
                Some(height) => [height, height],
                None => icon.size(),
            };
            Image::new(icon.id(), size).build(ui);
            ui.same_line();
            return
        }
        if let Some(data) = marker_icon_data(marker.clone()) {
            load_texture_bytes(marker.to_string(), data);
        }
    }

    pub fn icon(
        ui: &Ui,
        height: Option<f32>,
        alert_icon: Option<&RelativePathBuf>,
        path: Option<&PathBuf>,
    ) {
        if let Some(icon) = alert_icon {
            if let Some(path) = path {
                let gooey = IMGUI_TEXTURES.get().unwrap();
                let path_str = icon.as_str();
                if let Some(icon) = gooey.read().unwrap().get(path_str) {
                    //if let Some(icon) = get_texture(icon.as_str()) {
                    let size = match height {
                        Some(height) => [height, height],
                        None => icon.size(),
                    };
                    Image::new(icon.id(), size).build(ui);
                    ui.same_line();
                    return
                }
                load_texture_path(icon.clone(), path.clone());
            }
        };
    }
    pub fn draw_open_button<S: AsRef<str> + std::fmt::Display, O: Into<String> + std::fmt::Debug>(
        state_errors: &mut HashMap<String, anyhow::Error>,
        ui: &Ui,
        text: S,
        openable: O,
    ) {
        let openable_display = format!("{:?}", openable);
        let text_display = text.to_string();
        let entry_name = fl!(
            "open-error",
            kind = text_display.clone(),
            path = openable_display.clone()
        );
        if ui.button(&text) {
            log::info!("Triggered open {openable:?} for {text}");
            let sender = CONTROLLER_SENDER.get().unwrap();
            let event_send = sender.try_send(ControllerEvent::OpenOpenable(
                entry_name.clone(),
                openable.into(),
            ));
            drop(event_send);
        }
        if ui.is_item_hovered() {
            ui.tooltip_text(fl!("location", path = openable_display));
        }
    }

    pub fn font_text(font: &str, ui: &Ui, text: &str) {
        let imfont_pointer = rt::read_nexus_link().ok().and_then(|nexus_link| match font {
            "big" => Some(nexus_link.font_big),
            "ui" => Some(nexus_link.font_ui),
            "font" => Some(nexus_link.font),
            _ => None,
        }).and_then(|font| unsafe { Self::font_from_raw(font) });
        let font_handle = imfont_pointer.map(|font| ui.push_font(font.id()));
        ui.text_wrapped(text);
        drop(font_handle);
    }
    pub fn offset_font_text(
        font: &str,
        ui: &Ui,
        position: Vec2,
        bounding_size: Vec2,
        shadow: bool,
        text: &str,
    ) {
        let imfont_pointer = rt::read_nexus_link().ok().and_then(|nexus_link| match font {
            "big" => Some(nexus_link.font_big),
            "ui" => Some(nexus_link.font_ui),
            "font" => Some(nexus_link.font),
            _ => None,
        }).and_then(|font| unsafe { Self::font_from_raw(font) });
        let font_handle = imfont_pointer.map(|font| ui.push_font(font.id()));
        let text_size = Vec2::from(ui.calc_text_size(text));
        let cursor_pos =
            Alignment::get_position(Alignment::CENTRE_MIDDLE, position, bounding_size, text_size);
        if shadow {
            let cursor_pos_shadow = cursor_pos
                + Vec2 {
                    x: 2.0,
                    y: text_size.y / 8.0,
                };
            ui.set_cursor_pos(cursor_pos_shadow.into());
            let token = ui.push_style_color(StyleColor::Text, [0.0, 0.0, 0.0, 1.0]);
            ui.text(text);
            token.pop();
        }
        ui.set_cursor_pos(cursor_pos.into());
        ui.text(text);
        drop(font_handle);
    }

    unsafe fn font_from_raw<'a>(font: *const nexus::imgui::sys::ImFont) -> Option<&'a Font> {
        match font {
            p if p.is_null() => None,
            imfont_pointer => Some(Font::from_raw(&*imfont_pointer)),
        }
    }

    fn handle_alert(&mut self, ui: &Ui, io: &Io) {
        if let Some(alert) = &self.alert {
            let message = &alert.message;
            let imfont = rt::read_nexus_link().ok().and_then(|nexus_link| unsafe {
                Self::font_from_raw(nexus_link.font_big)
            });
            Self::render_alert(ui, io, message, imfont);
        }
    }
    pub fn render_alert(
        ui: &Ui,
        io: &nexus::imgui::Io,
        text: &String,
        font: Option<&Font>,
    ) {
        use WindowFlags;
        let font_handle = font.map(|font| ui.push_font(font.id()));
        let font_scale = font.map(|f| f.scale).unwrap_or(1.0);
        let fb_scale = io.display_framebuffer_scale;
        let [text_width, text_height] = ui.calc_text_size(text);
        let text_width = text_width * font_scale;
        let offset_x = text_width / 2.0;
        let [game_width, game_height] = io.display_size;
        let centre_x = game_width / 2.0;
        let centre_y = game_height / 2.0;
        let above_y = game_height * 0.2;
        let text_x = (centre_x - offset_x) * fb_scale[0];
        let text_y = (centre_y - above_y) * fb_scale[1];
        Window::new("TAIMIHUD_ALERT_AREA")
            .flags(
                WindowFlags::ALWAYS_AUTO_RESIZE
                    | WindowFlags::NO_TITLE_BAR
                    | WindowFlags::NO_RESIZE
                    | WindowFlags::NO_BACKGROUND
                    | WindowFlags::NO_MOVE
                    | WindowFlags::NO_SCROLLBAR
                    | WindowFlags::NO_INPUTS
                    | WindowFlags::NO_FOCUS_ON_APPEARING
                    | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS,
            )
            .position([text_x, text_y], Condition::Always)
            .size([text_width * 1.25, text_height * 2.0], Condition::Always)
            .build(ui, || {
                ui.text(text);
            });
        drop(font_handle);
    }

    pub fn lock() -> MutexGuard<'static, RenderState> {
        RENDER_STATE.get().unwrap().lock().unwrap()
    }
}

pub struct Alignment {}

#[allow(dead_code)]
impl Alignment {
    pub const LEFT_TOP: Vec2 = Vec2::new(0.0, 0.0);
    pub const LEFT_MIDDLE: Vec2 = Vec2::new(0.0, 0.5);
    pub const LEFT_BOTTOM: Vec2 = Vec2::new(0.0, 1.0);
    pub const CENTRE_TOP: Vec2 = Vec2::new(0.5, 0.0);
    pub const CENTRE_MIDDLE: Vec2 = Vec2::new(0.5, 0.5);
    pub const CENTRE_BOTTOM: Vec2 = Vec2::new(0.5, 1.0);
    pub const RIGHT_TOP: Vec2 = Vec2::new(1.0, 0.0);
    pub const RIGHT_MIDDLE: Vec2 = Vec2::new(1.0, 0.5);
    pub const RIGHT_BOTTOM: Vec2 = Vec2::new(1.0, 1.0);

    pub fn get_position(
        scaler: Vec2,
        position: Vec2,
        bounding_size: Vec2,
        element_size: Vec2,
    ) -> Vec2 {
        let scaled_size = (bounding_size - element_size) * scaler;
        position + scaled_size
    }

    pub fn set_cursor(
        ui: &Ui,
        scaler: Vec2,
        position: Vec2,
        bounding_size: Vec2,
        element_size: Vec2,
    ) {
        ui.set_cursor_pos(Self::get_position(scaler, position, bounding_size, element_size).into());
    }

    pub fn set_cursor_with_offset(
        ui: &Ui,
        scaler: Vec2,
        position: Vec2,
        bounding_size: Vec2,
        element_size: Vec2,
        offset: Vec2,
    ) {
        let position = position + offset;
        Self::set_cursor(ui, scaler, position, bounding_size, element_size);
    }
}
