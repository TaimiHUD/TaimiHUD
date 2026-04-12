use {
    crate::{
        controller::ControllerEvent,
        exports::runtime::{self as rt, bindings::TaimiControls},
        fl,
        marker::format::MarkerType,
        marker_icon_data,
        render::{
            machine::{RenderMachine, RenderTaskQueue},
            MarkerWindowState,
            PrimaryWindowState,
            TimerWindowState,
        },
        settings::{state::AddonHostName, ProgressBarSettings},
        timer::{PhaseState, TextAlert, TimerFile},
        Controller,
        Interruption,
        InterruptionSignal,
        RENDER_SENDER,
        TEXTURES,
    },
    glam::Vec2,
    nexus::imgui::{
        internal::RawCast,
        Condition,
        Font,
        Image,
        Io,
        PopupModal,
        StyleColor,
        Ui,
        Window,
        WindowFlags,
    },
    relative_path::RelativePathBuf,
    serde::{Deserialize, Serialize},
    std::{
        cell::Cell,
        collections::HashMap,
        fmt::Display,
        path::{Path, PathBuf},
        ptr,
        sync::{
            atomic::{AtomicPtr, Ordering},
            Arc,
            MutexGuard,
        },
    },
    strum_macros::{Display, EnumIter},
    tokio::sync::mpsc::{Receiver, Sender},
};

#[cfg(feature = "extension-nexus")]
use crate::render::machine::FrameState;
#[cfg(feature = "markers-edit")]
use super::edit_marker_window::EditMarkerWindowState;
#[cfg(feature = "markers")]
use crate::marker::format::MarkerSet;
#[cfg(feature = "space")]
use crate::{render::PathingWindowState, space::Engine};

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
    ContextMenuOpen {
        menus: TaimiControls,
    },
    CheckingForUpdates {
        checking: bool,
        downloading: bool,
    },
    #[allow(dead_code)]
    RenderKeybindUpdate,
    #[cfg(feature = "markers-edit")]
    OpenEditMarkers(Option<MarkerSet>),
    #[cfg(feature = "markers-edit")]
    GiveMarkerPaths(Vec<PathBuf>),
    ProgressBarUpdate(ProgressBarSettings),
    SendToClipboard(String),
    Reload,
    ReloadAll,
    /// user pressed "quit" button, which should initiate shutdown as much as
    /// possible
    ///
    /// this will request an unload from arcdps
    InitiateQuit,
    /// Determine primary render host (due to recent load or unload)
    RefreshHost,
    Quit(Interruption),
    #[cfg(any(feature = "markers", feature = "space"))]
    UiMapOpen(taimi_meta::ui::MapOpen),
    /// The buffer we were using has disappeared
    #[cfg(feature = "goggles")]
    UiDepthReleased(),
    #[cfg(feature = "goggles")]
    UiDepthAcquired(),
}
impl RenderEvent {
    pub const INITIATE_QUIT_REASON: Interruption = match () {
        #[cfg(todo)]
        () => Interruption::Shutdown,
        () => Interruption::Unspecified,
    };
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
    #[cfg(feature = "paths")]
    pub pathing_menu_open: bool,
    pub(super) timer_window: TimerWindowState,
    receiver: Receiver<RenderEvent>,
    alert: Option<TextAlert>,
    pub state_errors: HashMap<String, anyhow::Error>,
    pub task_queue: RenderTaskQueue,
    pub machine: RenderMachine,
    pub runtime: Option<crate::controller::runtime::RemoteContext>,
    #[cfg(feature = "space")]
    pub engine: Option<anyhow::Result<Engine>>,
}

impl RenderState {
    pub fn new(receiver: Receiver<RenderEvent>) -> Self {
        Self {
            receiver,
            machine: RenderMachine::new(),
            runtime: None,
            #[cfg(feature = "space")]
            engine: None,
            task_queue: Default::default(),
            alert: Default::default(),
            primary_window: PrimaryWindowState::new(),
            timer_window: TimerWindowState::new(),
            #[cfg(feature = "markers-edit")]
            edit_marker_window: EditMarkerWindowState::new(),
            #[cfg(feature = "markers")]
            marker_window: MarkerWindowState::new(),
            #[cfg(feature = "space")]
            pathing_window: PathingWindowState::new(),
            #[cfg(feature = "paths")]
            pathing_menu_open: false,
            state_errors: Default::default(),
        }
    }

    fn draw(&mut self, ui: &Ui) -> bool {
        let io = ui.io();
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
                    },
                    #[cfg(feature = "markers-edit")]
                    GiveMarkerPaths(paths) => {
                        self.edit_marker_window.set_filenames(paths);
                    },
                    OpenableError(key, err) => {
                        self.state_errors.insert(key, err);
                    },
                    RenderKeybindUpdate => {
                        self.primary_window.keybind_handler();
                    },
                    ProgressBarUpdate(settings) => {
                        self.timer_window.progress_bar = settings;
                    },
                    SendToClipboard(data) => {
                        ui.set_clipboard_text(data);
                    },
                    CheckingForUpdates { checking, downloading } => {
                        let sources = &mut self.primary_window.data_sources_tab;
                        sources.checking_for_updates = checking;
                        sources.downloading_update = downloading;
                    },
                    TimerData(timers) => {
                        self.primary_window.timer_tab.timer_selection = None;
                        self.primary_window.timer_tab.timers_update(timers);
                    },
                    #[cfg(feature = "markers")]
                    MarkerData(markers) => {
                        self.primary_window.marker_tab.marker_selection = None;
                        let categories: Vec<_> = markers.keys().cloned().collect();
                        #[cfg(feature = "markers-edit")]
                        self.edit_marker_window.category_update(categories);
                        self.primary_window.marker_tab.marker_update(markers);
                    },
                    AlertStart(alert) => {
                        self.alert = Some(alert);
                    },
                    AlertEnd(timer_file) =>
                        if let Some(alert) = &self.alert {
                            if Arc::ptr_eq(&alert.timer, &timer_file) {
                                self.alert = None;
                            }
                        },
                    ContextMenuOpen { menus } => self.open_context(ui, menus),
                    AlertFeed(phase_state) => {
                        self.timer_window.new_phase(phase_state);
                    },
                    AlertReset(timer) => {
                        self.timer_window.remove_phase(timer);
                    },
                    #[cfg(any(feature = "markers", feature = "space"))]
                    UiMapOpen(open) =>
                        if self.machine.set_map_open(open) {
                            self.machine.act_map_open();
                        },
                    #[cfg(feature = "goggles")]
                    UiDepthReleased() => {
                        self.machine.turn_depth_event(false);
                        #[cfg(deleteme)]
                        if self.machine.gameplay.gameplay_map().is_some() && crate::space::goggles::lens::read_lens() == core::ptr::dangling_mut() {
                            if let Some(Ok(engine)) = &mut self.engine {
                                engine.goggles_lens_reset(0, false);
                            }
                        }
                    },
                    #[cfg(feature = "goggles")]
                    UiDepthAcquired() => {
                        self.machine.turn_depth_event(true);
                    },
                    event @ (Reload | ReloadAll) => {
                        self.reload(matches!(event, Reload));
                        return true
                    },
                    RefreshHost => {
                        Self::select_host();
                    },
                    InitiateQuit => {
                        #[cfg(feature = "extension-arcdps")]
                        Controller::arc_spawn_early_exit();

                        crate::TEXTURES.quit();
                        Controller::send_exit(RenderEvent::INITIATE_QUIT_REASON);
                        let _ = crate::SPACE_SENDER.write().map(|mut s| s.take());
                        if let Ok(mut sender) = crate::RENDER_SENDER.try_write() {
                            sender.take();
                        }
                        self.quit();
                        crate::TEXTURES.cleanup(true);
                        return false
                    },
                    Quit(Interruption::Abort) => {
                        log::debug!("render skipping shutdown due to abort");
                        self.shutdown_background();
                        return false
                    },
                    Quit(_reason) => {
                        self.quit();
                        return false;
                    },
                }
            },
            Err(_error) => (),
        }
        self.handle_alert(ui, io);
        self.timer_window.draw(ui);
        self.primary_window.draw(
            ui,
            &mut self.machine,
            &mut self.timer_window,
            &mut self.state_errors,
        );
        #[cfg(feature = "markers")]
        self.marker_window.draw(ui);
        #[cfg(feature = "markers-edit")]
        self.edit_marker_window.draw(ui);
        #[cfg(feature = "space")]
        self.pathing_window
            .draw(ui, &mut self.machine, self.engine.as_mut());
        self.draw_context_menu(ui);
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

        true
    }
    pub fn marker_icon(ui: &Ui, height: Option<f32>, marker: &MarkerType) {
        let key = marker.to_string();
        let icon = match TEXTURES.lookup_imgui(&key) {
            Some(t) => t,
            None => {
                if let Some(data) = marker_icon_data(*marker) {
                    crate::texture_schedule_bytes(key, data);
                }
                None
            },
        }
        .unwrap_or_default();
        let size = match height {
            Some(height) => [height, height],
            None => icon.size,
        };
        Image::new(icon.id, size).build(ui);
        ui.same_line();
    }

    pub fn icon(ui: &Ui, height: Option<f32>, alert_icon: Option<&RelativePathBuf>, path: Option<&Path>) {
        let icon = match alert_icon {
            Some(icon) => icon,
            None => return,
        };
        let key = icon.as_str();
        let icon = match TEXTURES.lookup_imgui(&key) {
            Some(t) => t,
            None => {
                if let Some(path) = path {
                    crate::texture_schedule_path(icon, icon.to_path(path));
                }
                None
            },
        }
        .unwrap_or_default();
        let size = match height {
            Some(height) => [height, height],
            None => icon.size,
        };
        Image::new(icon.id, size).build(ui);
        ui.same_line();
    }
    pub fn draw_open_path_button<S: AsRef<str> + Display>(ui: &Ui, text: S, path: &Path) {
        Self::draw_open_button(
            ui,
            text,
            || {
                match path.metadata() {
                    Ok(m) if !m.is_dir() => path.parent().unwrap_or(path),
                    _ => path,
                }
                .to_string_lossy()
            },
            || rt::relative_path(path).display(),
        )
    }
    pub fn draw_open_button<S, O, TT>(
        ui: &Ui,
        text: S,
        openable: impl FnOnce() -> O,
        tooltip: impl FnOnce() -> TT,
    ) where
        S: AsRef<str> + Display,
        O: Into<String> + Display,
        TT: Display,
    {
        if ui.button(&text) {
            let openable = openable();
            log::debug!("Triggered open {openable} for {text}");
            let openable_display = openable.to_string();
            let text_display = text.to_string();
            let entry_name = fl!("open-error", kind = text_display, path = openable_display);
            Controller::try_send(ControllerEvent::OpenOpenable(entry_name.clone(), openable.into()));
        } else if ui.is_item_hovered() {
            let tooltip = tooltip().to_string();
            ui.tooltip_text(fl!("location", path = tooltip));
        }
    }

    pub fn push_font<'a>(font: &str, ui: &'a Ui) -> Option<nexus::imgui::FontStackToken<'a>> {
        let imfont_pointer = rt::read_nexus_link()
            .ok()
            .and_then(|nexus_link| match font {
                #[cfg(feature = "extension-nexus")]
                "big" => Some(nexus_link.font_big),
                #[cfg(feature = "extension-nexus")]
                "ui" => Some(nexus_link.font_ui),
                #[cfg(feature = "extension-nexus")]
                "font" => Some(nexus_link.font),
                _ => None,
            })
            .and_then(|font| unsafe { Self::font_from_raw(font) });
        imfont_pointer.map(|font| ui.push_font(font.id()))
    }
    pub fn font_text(font: &str, ui: &Ui, text: &str) {
        let font_handle = Self::push_font(font, ui);
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
        let font_handle = Self::push_font(font, ui);
        let text_size = Vec2::from(ui.calc_text_size(text));
        let cursor_pos =
            Alignment::get_position(Alignment::CENTRE_MIDDLE, position, bounding_size, text_size);
        if shadow {
            let cursor_pos_shadow = cursor_pos + Vec2 { x: 2.0, y: text_size.y / 8.0 };
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
            let imfont = match rt::read_nexus_link() {
                #[cfg(feature = "extension-nexus")]
                Ok(nexus_link) => unsafe { Self::font_from_raw(nexus_link.font_big) },
                _ => None,
            };
            Self::render_alert(ui, io, message, imfont);
        }
    }
    pub fn render_alert(ui: &Ui, io: &nexus::imgui::Io, text: &String, font: Option<&Font>) {
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
                let checkpoint = Vec2::from_array(ui.cursor_pos());
                ui.set_cursor_pos((checkpoint - Vec2::splat(1.0)).into());
                ui.text_colored([1.0; 4], &text);
                ui.set_cursor_pos((checkpoint + Vec2::splat(1.0)).into());
                ui.text_colored([0.0, 0.0, 0.0, 1.0], &text);
                ui.set_cursor_pos(checkpoint.into());
                ui.text(text);
            });
        drop(font_handle);
    }

    fn quit(&mut self) {
        self.cleanup(true);
        crate::unload_render();
    }
    pub fn cleanup(&mut self, unload: bool) {
        #[cfg(feature = "space")]
        if let Some(Ok(mut engine)) = self.engine.take() {
            log::debug!("unloading space engine");
            engine.cleanup(unload);
        }
    }
    pub fn cleanup_background(mut self) {
        self.shutdown_background();
    }
    fn shutdown_background(&mut self) {
        self.receiver.close();
        #[cfg(feature = "space")]
        if let Some(Ok(engine)) = self.engine.take() {
            engine.cleanup_background();
        }
    }
    pub fn reload(&mut self, superficial: bool) {
        log::info!("{} renderer...", if superficial { "reloading" } else { "reinit" });

        #[cfg(feature = "goggles")]
        let _ = crate::space::goggles::shutdown();

        #[cfg(feature = "space")]
        if let Some(Ok(mut engine)) = self.engine.take() {
            log::debug!("reloading space engine");
            if Self::is_render_thread() {
                engine.cleanup(false);
            } else {
                log::warn!("TODO: reloading outside of render thread");
                engine.cleanup_background();
            }
            // ... and let it reinit on its own next render frame
        }

        if !superficial {
            // probably no need to reload textures/etc unless we've lost the entire d3d device or something?
            TEXTURES.cleanup(RenderState::is_render_thread());
        }

        unsafe {
            rt::notify_render_reinit();
        }
    }

    fn shutdown(&mut self) {
        match Interruption::try_drain_signals(&mut self.receiver) {
            Some(Interruption::Abort) => {
                log::debug!("render skipping shutdown due to abort");
                self.shutdown_background();
            },
            #[cfg(todo)]
            Some(Interruption::GameQuit) => self.shutdown_background(),
            Some(..) => {
                self.quit();
            },
            None => (),
        }
    }

    pub fn unload(mut self) {
        self.cleanup(true);
        drop(self);
        crate::unload_render();
    }

    pub fn lock() -> MutexGuard<'static, Option<RenderState>> {
        crate::RENDER_STATE.lock().unwrap()
    }
    fn host() -> &'static AtomicPtr<u8> {
        static RENDER_HOST: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
        &RENDER_HOST
    }
    pub fn select_host() {
        for host in AddonHostName::HOST_PRIORITY {
            if !host.is_loaded() {
                continue
            }
            if !host.is_active() {
                log::info!("reactivating {host}");
                let res = match host {
                    #[cfg(feature = "extension-arcdps")]
                    AddonHostName::ArcDPS => crate::exports::arcdps::enter(),
                    #[cfg(feature = "extension-nexus")]
                    AddonHostName::Nexus => crate::exports::nexus::enter(),
                    _ => continue,
                };
                if let Err(e) = res {
                    log::warn!("failed to reenter {host}: {e}");
                    continue
                }
            }

            Self::set_host(*host);
            return
        }
        let prev = Self::host().swap(ptr::null_mut(), Ordering::Relaxed);
        if !prev.is_null() {
            log::debug!("primary renderer cleared");
        }
    }
    pub fn set_host(host: AddonHostName) {
        let id = host.id().as_ptr() as *mut _;
        let prev = Self::host().swap(id, Ordering::Relaxed);
        if prev != id {
            log::debug!("selected primary renderer {host}");
            // TODO: clear imgui context because uhhh yeah
        }
    }
    pub fn is_host(host: AddonHostName) -> Option<bool> {
        let id = Self::host().load(Ordering::Relaxed);
        (!id.is_null()).then_some(id as *const u8 == host.id().as_ptr())
    }

    pub fn sender() -> Option<Sender<RenderEvent>> {
        RENDER_SENDER.try_read().as_ref().ok().and_then(|s| (*s).clone())
    }

    pub fn try_send(e: RenderEvent) {
        let sender = RENDER_SENDER.try_read();
        let sender = sender.as_ref().map(|s| &**s);
        if let Ok(Some(sender)) = sender {
            let _ = sender.try_send(e);
        }
    }

    pub fn is_running() -> bool {
        RENDER_SENDER
            .read()
            .map(|sender| sender.is_some())
            .unwrap_or(false)
    }

    /// per-frame state setup
    pub fn pre_render_ui(&mut self) {
        #[cfg(feature = "paths")]
        {
            use crate::render::element::pack::PackVisibility;
            self.pathing_window.pre_render();
            let visibility = self.pathing_window.window_visibility();
            let pack_visibility = self
                .pathing_window
                .packs_visibility()
                .within(visibility)
                .max(PackVisibility::visible(self.pathing_menu_open));
            self.machine.pack_ui_state.pre_draw(pack_visibility);
            #[cfg(any(feature = "paths-edit", feature = "paths-interact"))]
            let gameplay_map = self.machine.gameplay.gameplay_map();
            #[cfg(feature = "paths-edit")]
            {
                let edit_vis = self.pathing_window.edit_visibility();
                self.machine
                    .pack_ui_state
                    .pack_edit
                    .pre_draw(edit_vis.within(visibility), gameplay_map);
            }
            #[cfg(feature = "paths-interact")]
            {
                #[cfg(deleteme)]
                let interact_visibility = interact_visibility.min(self.machine.pack_ui_state.interact.visibility());
                let interact_visibility = self.pathing_window.pois_visibility().within(visibility);
                self.machine
                    .pack_ui_state
                    .interact
                    .pre_draw(visibility.within(visibility));
                if interact_visibility.is_visible() {
                    let player_pos = self.machine.get_player_pos().map(|(pos, _)| pos);
                    self.machine.pack_ui_state.pack_edit.env.latest_pos = player_pos;
                    let interact = &mut self.machine.pack_ui_state.interact;
                    if interact.wants_static {
                        let wants_all = interact.wants_static_all();
                        if let Some(Ok(engine)) = &self.engine {
                            interact.update_static_render(&engine.packs);
                        }
                        if wants_all && !self.machine.pack_ui_state.pack_state.is_empty() {
                            interact.update_static_ui(
                                &self.machine.pack_ui_state.pack_state.map_ref_as_slice(),
                            );
                        }
                    }
                    let player_pos = match gameplay_map {
                        Some(..) => player_pos,
                        None => None,
                    };
                    if interact.wants_pos(player_pos) {
                        interact.update_dist(player_pos);
                    }
                }
            }
            self.pathing_window.pre_draw(&mut self.machine);
            self.pathing_menu_open = false;
        }
    }

    pub fn pre_render(host: AddonHostName) -> Option<bool> {
        #[cfg(feature = "extension-nexus")]
        if let AddonHostName::Nexus = host {
            FrameState::NEXUS.publish_set();
        }
        let host = match Self::is_host(host) {
            None => {
                Self::select_host();
                Self::is_host(host)
            },
            h => h,
        };
        match host {
            None => {
                // *shrug*
                None
            },
            Some(false) => None,
            Some(true) => Some({
                let ready = IS_RENDER_THREAD.replace(true);
                ready || !Self::is_running()
            }),
        }
    }
    pub fn post_render(host: AddonHostName) {
        #[cfg(feature = "extension-nexus")]
        if let AddonHostName::Nexus = host {
            FrameState::NEXUS.publish_clear();
            if FrameState::GAME_FRAME_SUBSEQUENT && crate::exports::nexus::available() {
                FrameState::GAME.publish_set();
            }
        }
    }

    pub fn render_setup() {
        if !Self::is_running() {
            return
        }
        crate::texture_schedule_bytes(RenderMachine::TEXTURE_LOGO_KEY, RenderMachine::TEXTURE_LOGO_BIN);
        crate::texture_schedule_bytes(RenderMachine::TEXTURE_LOGO_LINES_KEY, RenderMachine::TEXTURE_LOGO_LINES_BIN);
        rt::setup_stats();
        #[cfg(feature = "space")]
        crate::space::Engine::setup_stats();

        let mut state = Self::lock();
        if let Some(state) = state.as_mut() {
            state.machine.act_setup();
        }
    }

    pub fn render_ui(ui: &Ui) {
        let is_running = Self::is_running();

        if is_running {
            crate::process_textures();
        }

        let mut lock = Self::lock();
        let state = match &mut *lock {
            None => return,
            Some(state) => state,
        };

        let is_running = match is_running {
            true => state.draw(ui),
            false => false,
        };

        if !is_running {
            state.shutdown();
            lock.take();
        } else {
            let render_slot = (match () {
                #[cfg(feature = "space")]
                () => &mut state.engine,
                #[cfg(not(feature = "space"))]
                () => (),
            },);
            state.machine.post_ui();
            state.machine.post_render_late(render_slot);
        }
    }

    pub fn render_options(ui: &Ui, host: AddonHostName) -> bool {
        let mut lock = Self::lock();
        let state = match &mut *lock {
            None => return false,
            Some(state) => state,
        };
        let mut state_errors = Default::default();
        state.primary_window.draw_tabs(
            ui,
            Some(host),
            &mut state.machine,
            &mut state.timer_window,
            &mut state_errors,
            false,
        );
        true
    }
    pub fn render_options_fallback(ui: &Ui, host: AddonHostName) {
        use crate::{render::arc::ArcRenderState, settings::state::BootstrapState};

        if ArcRenderState::ui_options_disabled(ui, host) {
            let res = BootstrapState::read_with(|s| s.start_save())
                .and_then(|save| BootstrapState::write_file(&save));
            rt::log::error_ok(res);
        }
    }

    pub fn is_render_thread() -> bool {
        IS_RENDER_THREAD.get()
    }
}

thread_local! {
    static IS_RENDER_THREAD: Cell<bool> = Cell::new(false);
}

impl InterruptionSignal for RenderEvent {
    fn interrupted(&self) -> Option<Interruption> {
        match self {
            &Self::Quit(reason) => Some(reason),
            Self::InitiateQuit => Some(RenderEvent::INITIATE_QUIT_REASON),
            _ => None,
        }
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

    pub fn get_position(scaler: Vec2, position: Vec2, bounding_size: Vec2, element_size: Vec2) -> Vec2 {
        let scaled_size = (bounding_size - element_size) * scaler;
        position + scaled_size
    }

    pub fn set_cursor(ui: &Ui, scaler: Vec2, position: Vec2, bounding_size: Vec2, element_size: Vec2) {
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
