use {
    crate::{
        controller::ControllerEvent,
        exports::runtime::{self as rt, bindings::TaimiControls},
        marker::format::MarkerType,
        marker_icon_data,
        render::{
            element::prelude::*,
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
    anyhow::Context,
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
    strum::{Display, EnumIter, IntoStaticStr},
    tokio::sync::mpsc::{Receiver, Sender},
};

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
    AlertNotify(String, Option<core::time::Duration>),
    ClipboardSend(String, Option<String>),
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

#[derive(
    Display, IntoStaticStr, Default, Copy, Clone, Debug, Deserialize, Serialize, EnumIter, PartialEq,
)]
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
    frame_count: u32,
    pub state_errors: HashMap<String, anyhow::Error>,
    pub task_queue: RenderTaskQueue,
    pub machine: RenderMachine,
    pub runtime: Option<crate::controller::runtime::RemoteContext>,
    #[cfg(feature = "space")]
    pub engine: Option<anyhow::Result<Engine>>,
    pub container_state: elem::frame::ContainerContextState,
    pub frame_state: elem::frame::RenderFrameUi,
}

impl RenderState {
    pub fn new(receiver: Receiver<RenderEvent>) -> Self {
        Self {
            receiver,
            machine: RenderMachine::new(),
            frame_count: 0u32,
            runtime: None,
            #[cfg(feature = "space")]
            engine: None,
            container_state: Default::default(),
            frame_state: Default::default(),
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

    fn draw<'ui, U>(&mut self, ui: &mut U, context: DrawContextInput<'ui>) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
                    AlertNotify(message, _dur) => {
                        let res = rt::send_alert(ui, &message)
                            .map_err(anyhow::Error::msg)
                            .with_context(|| format!("notifying you about {message}"));
                        let _ = rt::log::warn_ok(res);
                    },
                    ClipboardSend(value, message) => {
                        let msg = message.map(|m| match m {
                            s if s.is_empty() => format!("copied {value:?} to clipboard"),
                            m => m,
                        });
                        ui.set_clipboard_text(value);
                        if let Some(msg) = msg {
                            let _ = rt::send_alert(ui, &msg);
                        }
                    },
                    ContextMenuOpen { menus } => self.open_context(ui, menus),
                    AlertFeed(phase_state) => {
                        self.timer_window.new_phase(phase_state);
                    },
                    AlertReset(timer) => {
                        self.timer_window.remove_phase(&timer);
                    },
                    #[cfg(any(feature = "markers", feature = "space"))]
                    UiMapOpen(open) =>
                        if self.machine.set_map_open(open) {
                            self.machine.act_map_open();
                        },
                    #[cfg(feature = "goggles")]
                    UiDepthReleased() => {
                        self.machine.turn_depth_event(false);
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

        self.frame_count = self.frame_count.saturating_add(1);
        let fps_settled = || {
            // while initializing graphics/game/etc, it can "run" at wrong res for a bit
            // though imgui seems to init fps at FLT_MAX or something,
            // uncapped frame rates (600+) seem like a workable indicator for this -
            // the moment charsel is working it will drop down to a reasonable value
            let fps = ui.with_io_dyn(|io| io.frame_rate());
            fps <= 340.0f32 && fps.to_bits() != 0.0f32.to_bits()
        };
        let startup_delay = match self.frame_count {
            0..=16 => true,
            0..=32 if self.machine.gameplay.latest_map().is_none() => true,
            0..=160 if self.machine.gameplay.is_initial() => {
                if fps_settled() {
                    // one more frame for good luck (unnecessary but why not)
                    self.frame_count = 160;
                }
                true
            },
            _ => false,
        };
        if startup_delay {
            // imgui does not like living early in the morning
            // maybe just stop opening windows on startup and it won't matter anymore
            return true
        }

        let mut container_state = self.container_state.clone();
        let mut context = context.new_root_scope(&mut container_state);

        self.handle_alert(ui);
        self.timer_window.draw(ui);
        let slot = match () {
            #[cfg(feature = "space")]
            _ => (&mut self.engine,),
            #[cfg(not(feature = "space"))]
            _ => (),
        };
        self.primary_window.draw(
            ui,
            &mut context,
            &mut self.machine,
            slot,
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
            if let Some(_token) = ui.begin_popup_modal(entry_name, Default::default(), None) {
                ui.text(im_fmt!("{errory:#}"));
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
    pub fn marker_icon<'ui, U>(ui: &mut U, height: Option<f32>, marker: &MarkerType)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
            Some(height) => ImSize2::splat(height),
            None => icon.im_size(),
        };
        ui.image(icon, size);
        ui.same_line();
    }

    pub fn icon<'ui, U>(
        ui: &mut U,
        height: Option<f32>,
        alert_icon: Option<&RelativePathBuf>,
        path: Option<&Path>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
            Some(height) => ImSize2::splat(height),
            None => icon.im_size(),
        };
        ui.image(icon, size);
        ui.same_line();
    }
    pub fn draw_open_path_button<'ui, U, S>(ui: &mut U, text: S, path: &Path)
    where
        U: ?Sized + ImDrawWindow<'ui>,
        S: ImStrExt,
    {
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
    pub fn draw_open_button<'ui, U, S, O, TT>(
        ui: &mut U,
        text: S,
        openable: impl FnOnce() -> O,
        tooltip: impl FnOnce() -> TT,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
        S: ImStrExt,
        O: Display + Into<String>,
        TT: Display,
    {
        let text = text.im_into_imstr();
        let imstr = &text;
        if ui.button(imstr) {
            let openable = openable();
            let display = ImStrExt::im_as_display(&imstr);
            log::debug!("Triggered open {openable} for {display}");
            let openable_display = openable.to_string();
            let text_display = text.im_into_string();
            let entry_name = fl!("open-error", kind = &text_display, path = &openable_display);
            Controller::try_send(ControllerEvent::OpenOpenable(entry_name.into(), openable.into()));
        } else if ui.is_item_hovered() {
            let tooltip = tooltip().to_string();
            ui.tooltip_text(fl!("location", path = &tooltip));
        }
    }

    pub fn offset_font_text<'ui, U>(
        font: Option<NexusLinkFont>,
        ui: &mut U,
        position: ImPos2,
        bounding_size: ImSize2,
        shadow: bool,
        text: &str,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _font_token = ui.push_font_opt(font);
        let text_size = ui.calc_text_size(text);
        let cursor_pos =
            Alignment::get_position(Alignment::CENTRE_MIDDLE, position, bounding_size, text_size);
        if shadow {
            let cursor_pos_shadow = cursor_pos + ImVec2 { x: 2.0, y: text_size.height / 8.0 };
            ui.set_cursor_pos(cursor_pos_shadow);
            ui.text_unformatted_coloured(text, ImColourIndex::V4_BLACK);
        }
        ui.set_cursor_pos(cursor_pos);
        ui.text(text);
    }

    fn handle_alert<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if let Some(alert) = &self.alert {
            let message = &alert.message;
            let _token = NexusLinkFont::Big.push_font(ui);
            let font_scale = _token.as_ref().map(|_| {
                let TODO = /*big_scale*/ 0u32;
                1.0
            });
            Self::render_alert(ui, message, font_scale);
        }
    }
    /// TODO: imw::Window::PIVOT_CENTRE probably makes some calculations here unnecessary
    pub fn render_alert<'ui, U>(ui: &mut U, text: &String, font_scale: Option<f32>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let font_scale = font_scale.unwrap_or(1.0);
        let (game, fb_scale) = ui.im_io_display_size();
        let mut tsize = ui.calc_text_size(text);
        tsize.width *= font_scale;
        let text_pos = {
            let centre = game.to_vector() * 0.5;
            let offset_x = tsize.width / 2.0;
            let above_y = game.height * 0.2;
            let pos = centre - ImVec2::new(offset_x, above_y);
            (pos * fb_scale).to_point().cast::<ImSpace>()
        };
        ui.window_prepare_pos(text_pos, ImCondition::Always, imw::Window::PIVOT_TOPLEFT);
        ui.window_prepare_size(
            tsize.cast::<ImSpace>() * ImSize2::new(1.25, 2.0),
            ImCondition::Always,
        );
        let window_flags = match ui.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => {
                let flags = im180::sys::ImGuiWindowFlags_AlwaysAutoResize
                    | im180::sys::ImGuiWindowFlags_NoTitleBar
                    | im180::sys::ImGuiWindowFlags_NoResize
                    | im180::sys::ImGuiWindowFlags_NoBackground
                    | im180::sys::ImGuiWindowFlags_NoMove
                    | im180::sys::ImGuiWindowFlags_NoScrollbar
                    | im180::sys::ImGuiWindowFlags_NoInputs
                    | im180::sys::ImGuiWindowFlags_NoFocusOnAppearing
                    | im180::sys::ImGuiWindowFlags_NoBringToFrontOnFocus
                    | im180::sys::ImGuiWindowFlags_NoSavedSettings;
                imw::DynArgsWindow::new(Some(flags))
            },
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => {
                let flags = im192::sys::ImGuiWindowFlags_AlwaysAutoResize
                    | im192::sys::ImGuiWindowFlags_NoTitleBar
                    | im192::sys::ImGuiWindowFlags_NoResize
                    | im192::sys::ImGuiWindowFlags_NoBackground
                    | im192::sys::ImGuiWindowFlags_NoMove
                    | im192::sys::ImGuiWindowFlags_NoScrollbar
                    | im192::sys::ImGuiWindowFlags_NoInputs
                    | im192::sys::ImGuiWindowFlags_NoFocusOnAppearing
                    | im192::sys::ImGuiWindowFlags_NoBringToFrontOnFocus
                    | im192::sys::ImGuiWindowFlags_NoSavedSettings;
                imw::DynArgsWindow::new(Some(flags))
            },
            _ => Default::default(),
        };
        let window = ui.begin_window_with(c"TAIMIHUD_ALERT_AREA", None, window_flags);
        if let Some(_window) = imw::BeginVisible::pop_open(window) {
            let checkpoint = ui.cursor_pos();
            ui.set_cursor_pos((checkpoint - Vec2::splat(1.0)));
            ui.text_colored([1.0; 4], &text);
            ui.set_cursor_pos((checkpoint + Vec2::splat(1.0)));
            ui.text_colored([0.0, 0.0, 0.0, 1.0], &text);
            ui.set_cursor_pos(checkpoint);
            ui.text(text);
        }
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
            let visibility = self.pathing_window.visibility();
            let interact_visibility = self.machine.pack_ui_state.interact.visibility().min(visibility);
            let pack_visibility =
                PackVisibility::visible(self.pathing_menu_open).max(match interact_visibility {
                    // if poi tab open, packs aren't!
                    PackVisibility::Visible => visibility.min(PackVisibility::Pending),
                    _ => visibility,
                });
            self.machine.pack_ui_state.pre_draw(pack_visibility);
            self.machine.pack_ui_state.interact.pre_draw(visibility);
            if interact_visibility.is_visible() {
                let player_pos = self.machine.get_player_pos().map(|(pos, _)| pos);
                let interact = &mut self.machine.pack_ui_state.interact;
                if interact.wants_static {
                    let wants_all = interact.wants_static_all();
                    if let Some(Ok(engine)) = &self.engine {
                        interact.update_static_render(&engine.packs);
                    }
                    if wants_all && !self.machine.pack_ui_state.pack_state.is_empty() {
                        interact
                            .update_static_ui(&self.machine.pack_ui_state.pack_state.map_ref_as_slice());
                    }
                }
                let player_pos = match self.machine.gameplay.gameplay_map() {
                    Some(..) => player_pos,
                    None => None,
                };
                if interact.wants_pos(player_pos) {
                    interact.update_dist(player_pos);
                }
            }
            self.pathing_window.pre_draw(&mut self.machine);
            self.pathing_menu_open = false;
        }
    }

    pub fn pre_render(host: AddonHostName) -> Option<bool> {
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

    pub fn render_setup() {
        if !Self::is_running() {
            return
        }
        crate::texture_schedule_bytes(RenderMachine::TEXTURE_LOGO_KEY, RenderMachine::TEXTURE_LOGO_BIN);

        let mut state = Self::lock();
        if let Some(state) = state.as_mut() {
            state.machine.act_setup();
        }
    }

    pub fn render_ui<'ui, U>(ui: &mut U, context: DrawContextInput<'ui>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
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
            true => state.draw(ui, context),
            false => false,
        };

        if !is_running {
            state.shutdown();
            lock.take();
        }
    }

    pub fn render_options<'ui, U>(ui: &mut U, context: DrawContextInput<'ui>, host: AddonHostName) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut lock = Self::lock();
        let state = match &mut *lock {
            None => return false,
            Some(state) => state,
        };

        let mut state_errors = Default::default();
        let mut container_state = state.container_state.clone();
        let mut context = context.new_root_scope(&mut container_state);

        let slot = match () {
            #[cfg(feature = "space")]
            _ => (&mut state.engine,),
            #[cfg(not(feature = "space"))]
            _ => (),
        };
        state.primary_window.draw_tabs(
            ui,
            &mut context,
            Some(host),
            &mut state.machine,
            slot,
            &mut state.timer_window,
            &mut state_errors,
            false,
        );
        true
    }
    #[cfg(feature = "extension-arcdps")]
    pub fn render_options_arc<'ui, U>(
        ui: &mut U,
        context: DrawContextInput<'ui>,
        host: AddonHostName,
    ) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut lock = Self::lock();
        let state = match &mut *lock {
            None => return false,
            Some(state) => state,
        };
        let mut container_state = state.container_state.clone();
        let mut context = context.new_root_scope(&mut container_state);

        state.primary_window.arc_tab.ui_options(ui, &mut context, host);
        true
    }
    #[cfg(feature = "extension-arcdps")]
    pub fn render_options_fallback<'ui, U>(ui: &mut U, context: DrawContextInput<'ui>, host: AddonHostName)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        use crate::{render::arc::ArcRenderState, settings::state::BootstrapState};

        let mut container_state = Default::default();
        let mut context = context.new_root_scope(&mut container_state);
        if ArcRenderState::ui_options_disabled(ui, &mut context, host) {
            let res = BootstrapState::read_with(|s| s.start_save())
                .and_then(|save| BootstrapState::write_file(&save));
            rt::log::error_ok(res);
        }
    }
    #[cfg(not(feature = "extension-arcdps"))]
    pub fn render_options_fallback<'ui, U>(ui: &mut U, host: AddonHostName)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        ui.text("TODO: offline");
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
    pub const LEFT_TOP: ImVec2 = ImVec2::new(0.0, 0.0);
    pub const LEFT_MIDDLE: ImVec2 = ImVec2::new(0.0, 0.5);
    pub const LEFT_BOTTOM: ImVec2 = ImVec2::new(0.0, 1.0);
    pub const CENTRE_TOP: ImVec2 = ImVec2::new(0.5, 0.0);
    pub const CENTRE_MIDDLE: ImVec2 = ImVec2::new(0.5, 0.5);
    pub const CENTRE_BOTTOM: ImVec2 = ImVec2::new(0.5, 1.0);
    pub const RIGHT_TOP: ImVec2 = ImVec2::new(1.0, 0.0);
    pub const RIGHT_MIDDLE: ImVec2 = ImVec2::new(1.0, 0.5);
    pub const RIGHT_BOTTOM: ImVec2 = ImVec2::new(1.0, 1.0);

    pub fn get_position(
        scaler: ImVec2,
        position: ImPos2,
        bounding_size: ImSize2,
        element_size: ImSize2,
    ) -> ImPos2 {
        let scaled_size = (bounding_size - element_size).to_vector() * scaler;
        position + scaled_size
    }

    pub fn set_cursor<'ui, U>(
        ui: &mut U,
        scaler: ImVec2,
        position: ImPos2,
        bounding_size: ImSize2,
        element_size: ImSize2,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        ui.set_cursor_pos(Self::get_position(scaler, position, bounding_size, element_size));
    }

    pub fn set_cursor_with_offset<'ui, U>(
        ui: &mut U,
        scaler: ImVec2,
        position: ImPos2,
        bounding_size: ImSize2,
        element_size: ImSize2,
        offset: ImVec2,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let position = position + offset;
        Self::set_cursor(ui, scaler, position, bounding_size, element_size);
    }
}
