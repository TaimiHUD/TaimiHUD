// incomplete WIP, no point in cleaning it up yet
#![cfg_attr(not(taimi_debug = "wip"), allow(nonstandard_style, unused, unexpected_cfgs))]

use {
    self::id::{LoadedScriptNs, ScriptEventPath},
    crate::{exports::runtime as rt, settings::pathing::TriggerKind, Interruption},
    anyhow::Context,
    core::{any::Any, fmt, num::NonZero},
    std::{
        collections::BTreeMap,
        path::Path,
        sync::{
            atomic::{AtomicU32, AtomicUsize, Ordering},
            Arc,
            RwLock,
            RwLockReadGuard,
            RwLockWriteGuard,
        },
    },
    taimi_hoard::lazyfmt,
    taimi_sync::watched::watch,
    tokio::select,
};
#[cfg(feature = "paths")]
use {crate::controller::pathing::registry::PackPath, std::collections::BTreeSet};
#[cfg(feature = "paths-interact")]
use {
    crate::controller::pathing::state::interactive::{InteractionEvent, InteractionEventAction},
    tokio::sync::broadcast,
};
#[cfg(feature = "scripts-lua")]
use {core::cell::LazyCell, std::thread, tokio::sync::mpsc};

#[cfg(feature = "paths")]
use crate::controller::pathing::PathingShared;
#[cfg(not(feature = "paths"))]
type PackIndex = u16;

pub mod debug;
pub mod event;
pub mod id;
#[cfg(feature = "scripts-lua")]
pub mod lua;
pub mod menu;
#[cfg(any(feature = "paths"))]
pub mod mumble;
#[cfg(feature = "extension-nexus")]
pub mod nexus;
#[cfg(not(feature = "extension-nexus"))]
#[path = "nexus/unsupported.rs"]
pub mod nexus;
#[cfg(feature = "paths")]
pub mod pathing;
pub mod persistence;
pub mod ui;

#[cfg(feature = "scripts-lua")]
pub use self::lua::LuaMessage;
#[cfg(feature = "paths")]
pub use self::pathing::PackPlugShared;
pub use self::{
    id::{
        PackScriptPath,
        PlugPath,
        ScriptEventArg,
        ScriptEventId,
        ScriptEventIndex,
        ScriptIndex,
        ScriptPath,
    },
    menu::{PlugMenusById, PlugMenusShared},
};

/// TODO: try out FlatSet?
pub type EventSet = BTreeSet<ScriptEventId>;

pub struct ScriptController {
    pub rx: ScriptReceiver,
    #[cfg(feature = "scripts-lua")]
    pub lua_tx: Option<Option<mpsc::Sender<LuaMessage>>>,
    #[cfg(feature = "scripts-lua")]
    pub lua_thread: Option<thread::JoinHandle<anyhow::Result<()>>>,
    unmasked_events: EventSet,
    /// TODO: track granular interest and maybe dynamically register for the events
    /// via [Self::refresh_event_interest]?
    #[cfg(all(feature = "paths-interact", todo))]
    unmasked_interact: MarkerSet,
}
impl ScriptController {
    pub(super) fn new(rx: ScriptReceiver) -> Self {
        Self {
            rx,
            #[cfg(feature = "scripts-lua")]
            lua_tx: None,
            #[cfg(feature = "scripts-lua")]
            lua_thread: None,
            unmasked_events: Default::default(),
        }
    }
    pub(super) async fn run(mut self) -> anyhow::Result<()> {
        let reason = 'exit: loop {
            let rx_interact = match () {
                #[cfg(feature = "paths-interact")]
                _ => self.rx.rx_interact.recv(),
                #[cfg(not(feature = "paths-interact"))]
                _ => futures::future::pending::<()>,
            };
            select! {
                msg = self.rx.command.recv() => match msg {
                    Some(msg) => {
                        let name = <&str>::from(&msg);
                        let res = self.handle_message(msg).await
                            .context(lazyfmt::fmt_args!(move "processing {name}"));
                        if let Some(Some(reason)) = rt::log::error_ok(res) {
                            break 'exit reason
                        }
                    },
                    None => break 'exit Interruption::Unspecified,
                },
                e = rx_interact => {
                    #[cfg(feature = "paths-interact")]
                    match e {
                        Err(broadcast::error::RecvError::Closed) => (),
                        Err(broadcast::error::RecvError::Lagged(_amt)) => {
                            log::debug!("script interact rx lagged behind {_amt}");
                        },
                        Ok(e) => {
                            let res = self.handle_interact_event(e).await
                                .context("processing interaction");
                            let _ = rt::log::warn_ok(res);
                        },
                    }
                },
            }
        };
        let res = self.teardown_for_exit(reason).await.context("exiting");
        #[cfg(todo = "unnecessary")]
        let _ = rt::log::error_ok(res);
        res
    }
    async fn handle_message(&mut self, msg: ScriptMessage) -> anyhow::Result<Option<Interruption>> {
        match msg {
            ScriptMessage::TearDown => self.teardown().await,
            ScriptMessage::Exit(reason) => return Ok(Some(reason)),
            ScriptMessage::InternalReq(req) => self.process_internal_req(req).await,
            #[cfg(feature = "scripts-lua")]
            ScriptMessage::RefreshPacks => {
                Self::do_refresh_packs().await;
                Ok(())
            },
            #[cfg(feature = "scripts-lua")]
            ScriptMessage::Lua(msg) => {
                if self.lua_tx.is_none() && msg.wants_runtime() {
                    let _ = rt::log::warn_ok(self.start_lua().await);
                }
                if let Some(Some(tx)) = &self.lua_tx {
                    let _ = rt::log::warn_ok(tx.send(msg).await);
                } else {
                    #[cfg(taimi_debug)]
                    #[cfg(deleteme)]
                    log::debug!("couldn't relay {msg:?}")
                }
                Ok(())
            },
        }
        .map(|()| None)
    }
    #[cfg(feature = "paths-interact")]
    async fn handle_interact_event(&mut self, e: InteractionEvent) -> anyhow::Result<()> {
        let (signal, marker, target, action) = match e {
            InteractionEvent::Interact {
                action: InteractionEventAction::Report(..),
                ..
            } => {
                // TODO: what was this used for again?
                return Ok(())
            },
            InteractionEvent::Interact { path, loaded_path, action, .. } => {
                let path: id::EventArgPath = path.pivot_from();
                let is_auto = match action {
                    InteractionEventAction::AutoTrigger => Some(true),
                    InteractionEventAction::Manual(mask) =>
                        mask.contains(TriggerKind::SCRIPT).then_some(false),
                    // TODO: maybe controller should just send Manual or Report or something dedicated to signal this properly?
                    #[cfg(todo)]
                    InteractionEventAction::Manual(..) | InteractionEventAction::Interact => None,
                    _ => Some(false),
                };
                if let Some(is_auto) = is_auto {
                    let arg = event::UntypedArgs::Bool(is_auto);
                    (
                        event::ScriptNotification::PathingTrigger,
                        path,
                        loaded_path.root.root,
                        arg,
                    )
                } else {
                    return Ok(())
                }
            },
            #[cfg(todo)]
            InteractionEvent::Nearby { path, loaded_path, .. } => (
                event::ScriptNotification::PathingFocus,
                path.pivot_from(),
                loaded_path.root.root,
                event::UntypedArgs::Empty,
            ),
            #[cfg(todo)]
            InteractionEvent::Gone { path, .. } => (
                event::ScriptNotification::PathingUnfocus,
                path.pivot_from(),
                event::UntypedArgs::Empty,
            ),
            InteractionEvent::Nearby { path, loaded_path, .. } => (
                event::ScriptNotification::PathingFocus,
                path.pivot_from(),
                loaded_path.root.root,
                event::UntypedArgs::Bool(true),
            ),
            InteractionEvent::Gone { path, loaded_path, .. } => (
                event::ScriptNotification::PathingFocus,
                path.pivot_from(),
                loaded_path.root.root,
                event::UntypedArgs::Bool(false),
            ),
        };
        self.process_script_event(signal, target.pivot_from(), marker.path, action)
            .await
    }
    async fn process_script_event(
        &self,
        signal: event::ScriptNotification,
        target: ScriptPath,
        arg0: ScriptEventArg,
        arg1: event::UntypedArgs,
    ) -> anyhow::Result<()> {
        #[cfg(todo)]
        let targets = self.iter_event_interest(signal.to_repr() as _, arg0);
        let targets = [target];
        #[cfg(feature = "scripts-lua")]
        let lua_args = |arg1: &event::UntypedArgs| match (arg0, arg1) {
            (LoadedScriptNs::ARG_UNK, event::UntypedArgs::Empty) => None,
            (a0, a1) => {
                let a0 = (!LoadedScriptNs::arg_is_empty(a0)).then(|| {
                    Box::new(Some(a0.repr())) as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>
                });
                let a1 = match *a1 {
                    event::UntypedArgs::Empty => None,
                    event::UntypedArgs::Bool(v) =>
                        Some(Box::new(Some(v)) as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>),
                    event::UntypedArgs::Int(i) =>
                        Some(Box::new(Some(i)) as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>),
                    #[cfg(todo)]
                    event::UntypedArgs::Lua(a1) => Some(a1),
                    ref _e => {
                        log::warn!("TODO? event {signal:?}({_e:?})");
                        None
                    },
                };
                Some(a0.into_iter().chain(a1).collect::<Vec<_>>())
            },
        };
        for target in targets {
            #[cfg(feature = "scripts-lua")]
            let is_lua = {
                // TODO?
                true
            };
            #[cfg(feature = "scripts-lua")]
            if is_lua {
                let Some(Some(tx)) = &self.lua_tx else { continue };
                let lua_args = lua_args(&arg1);
                let msg = match lua_args {
                    None => LuaMessage::NotifyScript0 { id: signal, context: target.path },
                    Some(args) => LuaMessage::NotifyScriptWith { id: signal, context: target.path, args },
                };
                #[cfg(taimi_debug)]
                log::debug!("sending: {msg:?}");
                let _ = rt::log::warn_ok(tx.send(msg).await);
            }
        }
        Ok(())
    }
    /// TODO: bleh handle the overlap elsewhere or use a heap instead...
    fn iter_event_interest(
        &self,
        signal: ScriptEventIndex,
        arg: ScriptEventArg,
    ) -> impl Iterator<Item = ScriptPath> + '_ {
        let mut targets = BTreeSet::new();
        let interested = self.unmasked_events.iter().filter_map(|um| {
            let (um_target, um_signal, um_arg) = LoadedScriptNs::id_to_notif(um);
            let arg = LoadedScriptNs::arg_is_empty(um_arg) || um_arg == arg;
            let signal = um_signal == ScriptEventIndex::MAX || um_signal == signal;
            (arg && signal).then_some(um_target)
        });
        for target in interested {
            match target.path {
                ScriptIndex::GLOBAL => {
                    targets.extend(self.rx.plugs_shared.borrow().all_paths());
                    break
                },
                ScriptIndex::WILDCARD_PLUG => {
                    targets.extend(self.rx.plugs_shared.borrow().plug_paths().map(|p| p.pivot_from()));
                    continue
                },
                ScriptIndex::WILDCARD_PACK => {
                    targets.extend(self.rx.plugs_shared.borrow().pack_paths().map(|p| p.pivot_from()));
                    continue
                },
                ScriptIndex::UNK => continue,
                target => {
                    targets.insert(ScriptPath::new_path(target));
                },
            }
        }
        targets.into_iter()
    }
    async fn process_internal_req(&mut self, req: ScriptRequest) -> anyhow::Result<()> {
        match req {
            ScriptRequest::MaskEvent { target, signal, arg } => {
                let arg = (!LoadedScriptNs::arg_is_empty(arg)).then_some(arg);
                let mut dirty = false;
                self.unmasked_events.retain(|um| {
                    let (um_target, um_signal, um_arg) = LoadedScriptNs::id_to_notif(um);
                    let arg = arg.map(|a| {
                        #[cfg(todo = "unnecessary")]
                        let um_arg = LoadedScriptNs::id_to_notif_arg(um);
                        a == um_arg
                    });
                    let signal = signal.map(|a| {
                        #[cfg(todo = "unnecessary")]
                        let um_signal = LoadedScriptNs::id_to_notif_event(um);
                        a == um_signal
                    });
                    if arg == Some(false) || signal == Some(false) {
                        // not a match, irrelevant
                        true
                    } else {
                        let (l, r) = um_target.path.matcher(target.path);
                        let retain = !l.matches(r);
                        dirty |= !retain;
                        retain
                    }
                });
                if dirty {
                    self.refresh_event_interest();
                }
            },
            ScriptRequest::UnmaskEvent { target, signal, arg } => {
                let signal: ScriptEventPath = signal
                    .map(|s| ScriptEventPath::new_path(s as ScriptEventIndex))
                    .unwrap_or(ScriptEventPath::new_path(ScriptEventIndex::MAX));
                self.unmasked_events
                    .insert(LoadedScriptNs::notif_to_id(target, signal, arg));
                self.refresh_event_interest();
            },
        }
        Ok(())
    }
    /// TODO?
    #[inline(always)]
    fn refresh_event_interest(&mut self) {}
    #[cfg(feature = "scripts-lua")]
    async fn start_lua(&mut self) -> anyhow::Result<()> {
        log::debug!("starting lua...");
        let lua_tx = self.lua_tx.insert(None);
        let shared = self.rx.shared.clone();
        let plugs_shared = self.rx.plugs_shared.clone();
        let (tx, rx) = mpsc::channel(48);
        let lua = move || lua::LuaController::run_new(rx, shared, plugs_shared);
        let controller = thread::Builder::new()
            .name(format!("{}/controller/lua", rt::CRATE_NAME))
            .spawn(lua)
            .context("spawning lua controller")?;
        self.lua_thread = Some(controller);
        *lua_tx = Some(tx);
        Ok(())
    }
    async fn teardown(&mut self) -> anyhow::Result<()> {
        #[cfg(feature = "scripts-lua")]
        if let (Some(lua), Some(lua_tx)) = (self.lua_thread.take(), self.lua_tx.as_mut()) {
            if let Some(tx) = lua_tx.take() {
                drop(tx);
                log::debug!("waiting for lua shutdown...");
                let res = tokio::task::spawn_blocking(move || lua.join()).await;
                let res = match res {
                    Err(e) => Ok(crate::log_join_error("lua", e)),
                    Ok(Err(e)) => Ok(crate::log_any_error("lua", &e)),
                    Ok(Ok(res)) => res.context("lua shutdown"),
                };
                rt::log::warn_ok(res);
            }
        }
        // XXX: if lua thread somehow exists without the sender, it gets dropped here

        Ok(())
    }
    pub(super) async fn teardown_for_exit(&mut self, _reason: Interruption) -> anyhow::Result<()> {
        if let Some(Some(tx)) = &self.lua_tx {
            let _ = tx.try_send(LuaMessage::TearDown);
            let res = tx.send(LuaMessage::Exit).await;
            rt::log::info_ok(res);
        }
        self.teardown().await?;
        Ok(())
    }
}
impl fmt::Debug for ScriptController {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ScriptController").finish()
    }
}

#[derive(Debug, strum::Display, strum::IntoStaticStr)]
pub enum ScriptMessage {
    TearDown,
    Exit(Interruption),
    #[cfg(feature = "scripts-lua")]
    RefreshPacks,
    #[cfg(feature = "scripts-lua")]
    #[strum(to_string = "Lua::{0}")]
    Lua(LuaMessage),
    /// flows in reverse *from* a script
    #[strum(to_string = "InternalReq::{0}")]
    InternalReq(ScriptRequest),
}
impl ScriptMessage {
    pub fn try_send(self) {
        super::Controller::with_sender(move |s| {
            if let Some(tx) = &s.scripting {
                let _ = tx.command.try_send(self);
            }
        });
    }

    /// TODO: check event mask interest for early filtering
    pub fn gameplay_keybind(
        state: rt::bindings::GameControls,
        changed: rt::bindings::GameControls,
    ) -> Option<Self> {
        match () {
            #[cfg(feature = "scripts-lua")]
            _ => {
                #[cfg(deleteme)]
                if changed.contains(rt::bindings::GameControl::Miscellaneous_Interact)
                    && state.contains(rt::bindings::GameControl::Miscellaneous_Interact)
                {
                    // prepare ahead of time...
                    let and_interact = true;
                    LuaMessage::RefreshMarkerFocus { and_interact }.try_send();
                    if and_interact {
                        // TODO: if state ^ changed & !Interact == 0?
                        return None
                    }
                }
                let args = vec![
                    Box::new(Some(state)) as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>,
                    Box::new(Some(changed)),
                ];
                Some(
                    LuaMessage::NotifyScriptWith {
                        id: event::ScriptNotification::GameplayKeybind,
                        context: ScriptIndex::GLOBAL,
                        args,
                    }
                    .into(),
                )
            },
            _ => None,
        }
    }
}
/// [ScriptMessage::InternalReq]
#[derive(Debug, strum::Display, strum::IntoStaticStr)]
pub enum ScriptRequest {
    MaskEvent {
        target: ScriptPath,
        signal: Option<ScriptEventIndex>,
        arg: ScriptEventArg,
    },
    UnmaskEvent {
        target: ScriptPath,
        signal: Option<ScriptEventIndex>,
        arg: ScriptEventArg,
    },
}
impl ScriptRequest {
    #[inline]
    fn try_send(self) {
        ScriptMessage::InternalReq(self).try_send()
    }
}

#[derive(Clone)]
pub struct ScriptSender {
    pub command: mpsc::Sender<ScriptMessage>,
    pub plugs_shared: watch::Sender<PlugsShared>,
    pub shared: Arc<ScriptShared>,
}
impl fmt::Debug for ScriptSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ScriptSender").finish()
    }
}
pub struct ScriptReceiver {
    pub command: mpsc::Receiver<ScriptMessage>,
    pub plugs_shared: watch::Sender<PlugsShared>,
    pub shared: Arc<ScriptShared>,
    #[cfg(feature = "paths")]
    pub pathing: Arc<PathingShared>,
    #[cfg(feature = "paths-interact")]
    pub rx_interact: broadcast::Receiver<InteractionEvent>,
}
impl fmt::Debug for ScriptReceiver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ScriptReceiver").finish()
    }
}
impl ScriptSender {
    pub fn new(#[cfg(feature = "paths")] pathing: &Arc<PathingShared>) -> (Self, ScriptReceiver) {
        let (tx, rx) = mpsc::channel(Self::QUEUE_LEN);
        let sender = Self {
            command: tx,
            plugs_shared: Default::default(),
            shared: Default::default(),
        };
        let receiver = ScriptReceiver {
            command: rx,
            plugs_shared: sender.plugs_shared.clone(),
            shared: sender.shared.clone(),
            #[cfg(feature = "paths-interact")]
            rx_interact: pathing.interact.events.subscribe(),
            #[cfg(feature = "paths")]
            pathing: pathing.clone(),
        };
        (sender, receiver)
    }
    const QUEUE_LEN: usize = 64;
}

#[derive(Debug, Default)]
pub struct ScriptShared {
    #[cfg(feature = "scripts-lua")]
    pub lua_processed_tick: Arc<AtomicU32>,
}
impl ScriptShared {
    #[inline]
    pub fn read_last_processed_tick(&self) -> Option<NonZero<u32>> {
        #[cfg(feature = "scripts-lua")]
        if let Some(tick) = NonZero::new(self.lua_processed_tick.load(Ordering::Relaxed)) {
            return Some(tick)
        }
        None
    }
    #[cfg(feature = "scripts-lua")]
    fn record_lua_processed_tick(&self, tick: u32) {
        self.record_lua_processed_tick_of(Some(tick));
    }
    #[cfg(feature = "scripts-lua")]
    fn record_lua_tick_interest(&self, want_tick: bool) {
        let recent = want_tick
            .then(|| {
                rt::mumble_link_ptr()
                    .ok()
                    .map(|ml| ml.read_ui_tick().wrapping_sub(ScriptTicker::TICK_TIMEOUT >> 3))
            })
            .flatten();
        self.record_lua_processed_tick_of(recent);
    }
    #[cfg(feature = "scripts-lua")]
    fn record_lua_processed_tick_of(&self, tick: Option<u32>) {
        let tick = tick.map(|t| t.max(1u32)).unwrap_or(0u32);
        self.lua_processed_tick.store(tick, Ordering::Relaxed);
    }
}
/// TODO: placeholder until Controller gets some sort of sane time tracking...
#[derive(Debug, Clone, Default)]
pub struct ScriptTicker {
    pub last_sent: u32,
    pub next_scheduled: u32,
    pub last_period: u32,
    pub shared: Option<Arc<ScriptShared>>,
    pub tx: Option<mpsc::Sender<ScriptMessage>>,
}
impl ScriptTicker {
    #[inline]
    pub fn wants_subscribe(&self) -> bool {
        self.shared.is_none()
    }
    pub fn subscribe(&mut self, sender: &ScriptSender) {
        self.shared = Some(sender.shared.clone());
        self.tx = Some(sender.command.clone());
    }
    pub fn unsubscribe(&mut self) {
        self.next_scheduled = 0;
        self.shared = None;
        self.tx = None;
    }
    pub fn process_new_tick(&mut self, ui_frame: u32) -> bool {
        if self.next_scheduled != 0 && Self::tick_is_earlier_than(ui_frame, self.next_scheduled) {
            // not yet time
            return false
        }
        let last_processed = self
            .shared
            .as_ref()
            .and_then(|s| s.read_last_processed_tick())
            .map(|l| l.get());
        let Some(last) = last_processed else {
            self.next_scheduled = 0;
            return false
        };
        if Self::tick_is_earlier_than(self.last_sent, last) && !Self::tick_is_expired(last, ui_frame) {
            return false
        }

        self.process_tick(ui_frame);
        true
    }
    #[inline]
    pub fn process_player_tick(&mut self, ui_frame: u32) {
        self.process_tick(ui_frame)
    }
    fn process_tick(&mut self, ui_frame: u32) {
        if self.next_scheduled == 0 {
            let interest = self.shared.as_ref().and_then(|s| s.read_last_processed_tick());
            if interest.is_none() {
                return
            }
        }
        let sent = if let Some(tick) = ScriptMessage::tick(Some(ui_frame)) {
            self.tx
                .as_ref()
                .and_then(|tx| tx.try_send(tick).is_ok().then_some(self.last_period.max(2) - 1))
        } else {
            // back off and pretend it was sent for now
            Some(Self::TICK_TIMEOUT >> 1)
        };
        if let Some(delay) = sent {
            let prev = core::mem::replace(&mut self.last_sent, ui_frame);
            self.last_period = ui_frame.wrapping_sub(prev).clamp(1, 0x2000);
            self.next_scheduled = ui_frame.wrapping_add(delay);
        }
    }
    #[inline]
    fn tick_is_earlier_than(tick: u32, future: u32) -> bool {
        tick.wrapping_sub(future) > 0x20000000
    }
    const TICK_TIMEOUT: u32 = 0x180;
    #[inline]
    fn tick_is_expired(tick: u32, now: u32) -> bool {
        tick.abs_diff(now) > Self::TICK_TIMEOUT
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlugsShared {
    pub plugs: BTreeMap<PlugPath, Arc<PlugShared>>,
    #[cfg(feature = "paths-lua")]
    pub packs: BTreeMap<PackScriptPath, Arc<PackPlugShared>>,
    /// not loaded but could be maybe!
    #[cfg(feature = "paths-lua")]
    pub available_packs: BTreeSet<PackPath>,
    #[cfg(feature = "paths-lua")]
    pub available_plugs: BTreeSet<Arc<Path>>,
}
impl PlugsShared {
    #[inline]
    pub fn plug_paths(&self) -> impl Iterator<Item = PlugPath> + Clone + '_ {
        self.plugs.keys().copied()
    }
    #[inline]
    #[cfg(feature = "paths-lua")]
    pub fn pack_paths(&self) -> impl Iterator<Item = PackScriptPath> + Clone + '_ {
        self.packs.keys().copied()
    }
    #[inline(always)]
    #[cfg(not(feature = "paths"))]
    pub fn pack_paths(&self) -> impl Iterator<Item = PackScriptPath> + Clone + '_ {
        core::iter::empty()
    }
    #[inline(always)]
    pub fn all_paths(&self) -> impl Iterator<Item = ScriptPath> + Clone + '_ {
        let packs = self.pack_paths().map(|p| p.pivot_from());
        self.plug_paths().map(|p| p.pivot_from()).chain(packs)
    }
}

/// TODO: newtype? there must be other fields...
pub type PlugShared = PlugSharedData;
#[derive(Debug)]
pub struct PlugSharedData {
    pub name: Arc<str>,
    pub path: ScriptPath,
    pub menus: menu::PlugMenusShared,
    pub state: PlugStateBeacon,
}
impl PlugSharedData {
    #[inline]
    pub fn with_name<N>(path: ScriptPath, name: N) -> Self
    where
        N: Into<Arc<str>>,
    {
        Self {
            path,
            name: name.into(),
            menus: Default::default(),
            state: Default::default(),
        }
    }

    /// TODO
    pub fn is_active(&self) -> bool {
        let status = self.state.status.load(Ordering::Relaxed);
        status != event::ScriptSignal::Ended as usize
    }
}
impl AsRef<PlugSharedData> for PlugSharedData {
    #[inline]
    fn as_ref(&self) -> &PlugSharedData {
        self
    }
}
pub trait PlugSharedRef: AsRef<PlugSharedData> + Any {}
impl<T> PlugSharedRef for T where T: AsRef<PlugSharedData> + Any {}
impl From<PlugSharedData> for Arc<dyn PlugSharedRef> {
    fn from(v: PlugSharedData) -> Self {
        Arc::new(v) as Arc<_>
    }
}
impl From<PackPlugShared> for Arc<dyn PlugSharedRef> {
    fn from(v: PackPlugShared) -> Self {
        Arc::new(v) as Arc<_>
    }
}
impl dyn PlugSharedRef {
    #[inline]
    pub unsafe fn as_plug_unchecked(&self) -> &PlugShared {
        &*(self as &dyn Any as *const _ as *const _)
    }
    #[cfg(feature = "paths-lua")]
    #[inline]
    pub unsafe fn as_pack_unchecked(&self) -> &pathing::PackPlugShared {
        &*(self as &dyn Any as *const _ as *const _)
    }
}
impl AsRef<PlugSharedData> for Arc<dyn PlugSharedRef> {
    #[inline]
    fn as_ref(&self) -> &PlugSharedData {
        (&**self).as_ref()
    }
}

#[derive(Debug)]
pub struct PlugStateBeacon {
    pub shared: RwLock<PlugStateData>,
    pub state_gen: AtomicUsize,
    pub status: AtomicUsize,
}
impl PlugStateBeacon {
    #[inline]
    pub fn latest_gen(&self) -> usize {
        self.state_gen.load(Ordering::Relaxed)
    }

    pub(crate) fn update_status(&self, status: event::ScriptSignal) {
        self.status.store(status as usize, Ordering::Relaxed);
    }

    #[inline]
    pub fn read(&self) -> (RwLockReadGuard<'_, PlugStateData>, usize) {
        let r = self.shared.read().unwrap_or_else(|e| e.into_inner());
        let gen = self.state_gen.load(Ordering::SeqCst);
        (r, gen)
    }
    #[inline]
    pub fn write(&self) -> RwLockWriteGuard<'_, PlugStateData> {
        let w = self.shared.write().unwrap_or_else(|e| e.into_inner());
        let _ = self.state_gen.fetch_add(1, Ordering::SeqCst);
        w
    }
}
impl Default for PlugStateBeacon {
    fn default() -> Self {
        Self {
            shared: Default::default(),
            state_gen: Default::default(),
            status: AtomicUsize::new(event::ScriptSignal::Started as usize),
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct PlugStateData {
    pub debug_watches: debug::DebugWatches,
}
