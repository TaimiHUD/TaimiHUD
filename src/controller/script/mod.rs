// incomplete WIP, no point in cleaning it up yet
#![cfg_attr(not(taimi_debug = "wip"), allow(nonstandard_style, unused, unexpected_cfgs))]

use {
    crate::{exports::runtime as rt, Interruption},
    anyhow::Context,
    core::{any::Any, fmt},
    std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
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
#[cfg(feature = "scripts-lua")]
use {std::thread, tokio::sync::mpsc};

pub mod debug;
pub mod event;
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
pub use self::menu::{PlugMenusById, PlugMenusShared};
#[cfg(feature = "paths")]
pub use self::pathing::{PackLoc, PackPlugShared};

pub struct ScriptController {
    pub rx: ScriptReceiver,
    #[cfg(feature = "scripts-lua")]
    pub lua_tx: Option<Option<mpsc::Sender<LuaMessage>>>,
    #[cfg(feature = "scripts-lua")]
    pub lua_thread: Option<thread::JoinHandle<anyhow::Result<()>>>,
}
impl ScriptController {
    pub(super) fn new(rx: ScriptReceiver) -> Self {
        Self {
            rx,
            #[cfg(feature = "scripts-lua")]
            lua_tx: None,
            #[cfg(feature = "scripts-lua")]
            lua_thread: None,
        }
    }
    pub(super) async fn run(mut self) -> anyhow::Result<()> {
        let reason = 'exit: loop {
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
            #[cfg(feature = "scripts-lua")]
            ScriptMessage::Lua(msg) => {
                if self.lua_tx.is_none() && msg.wants_runtime() {
                    let _ = rt::log::warn_ok(self.start_lua().await);
                }
                if let Some(Some(tx)) = &self.lua_tx {
                    let _ = rt::log::warn_ok(tx.send(msg).await);
                } else {
                    #[cfg(taimi_debug)]
                    log::debug!("couldn't relay {msg:?}")
                }
                Ok(())
            },
        }
        .map(|()| None)
    }
    #[cfg(feature = "scripts-lua")]
    async fn start_lua(&mut self) -> anyhow::Result<()> {
        log::debug!("starting lua...");
        let lua_tx = self.lua_tx.insert(None);
        let plugs_shared = self.rx.plugs_shared.clone();
        let (tx, rx) = mpsc::channel(48);
        let lua = move || lua::LuaController::run_new(rx, plugs_shared);
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

#[derive(Debug, strum::Display, strum::IntoStaticStr)]
pub enum ScriptMessage {
    TearDown,
    Exit(Interruption),
    #[cfg(feature = "scripts-lua")]
    #[strum(to_string = "Lua::{0}")]
    Lua(LuaMessage),
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
                        context: lua::LuaExecContext::Global,
                        args,
                    }
                    .into(),
                )
            },
            _ => None,
        }
    }
}
impl fmt::Debug for ScriptController {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ScriptController").finish()
    }
}

#[derive(Clone)]
pub struct ScriptSender {
    pub command: mpsc::Sender<ScriptMessage>,
    pub plugs_shared: watch::Sender<PlugsShared>,
}
impl fmt::Debug for ScriptSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ScriptSender").finish()
    }
}
pub struct ScriptReceiver {
    pub command: mpsc::Receiver<ScriptMessage>,
    pub plugs_shared: watch::Sender<PlugsShared>,
}
impl fmt::Debug for ScriptReceiver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ScriptReceiver").finish()
    }
}
impl ScriptSender {
    pub fn new() -> (Self, ScriptReceiver) {
        let (tx, rx) = mpsc::channel(Self::QUEUE_LEN);
        let sender = Self {
            command: tx,
            plugs_shared: Default::default(),
        };
        let receiver = ScriptReceiver {
            command: rx,
            plugs_shared: sender.plugs_shared.clone(),
        };
        (sender, receiver)
    }
    const QUEUE_LEN: usize = 64;
}

#[derive(Debug, Clone, Default)]
pub struct PlugsShared {
    pub plugs: BTreeMap<usize, Arc<PlugShared>>,
    #[cfg(feature = "paths-lua")]
    pub packs: BTreeMap<PackLoc, Arc<PackPlugShared>>,
}

/// TODO: newtype? there must be other fields...
pub type PlugShared = PlugSharedData;
#[derive(Debug)]
pub struct PlugSharedData {
    pub name: Arc<str>,
    pub menus: menu::PlugMenusShared,
    pub state: PlugStateBeacon,
}
impl PlugSharedData {
    #[inline]
    pub fn with_name<N>(name: N) -> Self
    where
        N: Into<Arc<str>>,
    {
        Self {
            name: name.into(),
            menus: Default::default(),
            state: Default::default(),
        }
    }

    /// TODO
    pub fn is_active(&self) -> bool {
        true
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

#[derive(Debug, Default)]
pub struct PlugStateBeacon {
    pub shared: RwLock<PlugStateData>,
    pub state_gen: AtomicUsize,
}
impl PlugStateBeacon {
    #[inline]
    pub fn latest_gen(&self) -> usize {
        self.state_gen.load(Ordering::Relaxed)
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
#[derive(Debug, Clone, Default)]
pub struct PlugStateData {
    pub debug_watches: debug::DebugWatches,
}
