#[cfg(feature = "paths-lua")]
use {
    super::PackLoc,
    crate::{controller::script::pathing::LuaPackDesc, space::pack::SharedLoader},
    core::num::NonZero,
    taimi_hoard::lazyfmt,
    taimi_pack::{
        pack::Pack,
        script::pathing::{
            imp::{MarkerLoc, MarkerOverrides, MarkerOverridesAttrs, PackMarkerRef, PackOverrides},
            ScriptApiUser,
        },
    },
    tokio::sync::mpsc,
};
use {
    super::{
        event::{ScriptNotification, ScriptSignal},
        menu::{PlugMenu, PlugMenuInstance},
        persistence::ScriptHostPersistence,
        PackPlugShared,
        PlugMenusShared,
        PlugSharedData,
        PlugSharedRef,
        PlugStateBeacon,
        PlugsShared,
        ScriptMessage,
    },
    crate::{controller::Controller, exports::runtime as rt, space::engine::SpaceEvent},
    anyhow::Context,
    core::{fmt, ops},
    mlua::{
        FromLua,
        IntoLuaMulti,
        Lua,
        MetaMethod,
        UserData,
        UserDataFields,
        UserDataMethods,
        UserDataRegistry,
    },
    rust_embed::RustEmbed,
    std::{
        borrow::Cow,
        fs,
        io,
        path::{Path, PathBuf},
        sync::Arc,
        time::Instant,
    },
    taimi_meta::map::MapID,
    taimi_pack::{
        attributes::{
            cell::{pack_attr, AttrKeyValue, GetAttrDyn, GetAttrDynExt, PackKeyId, PackValueCell},
            keys,
        },
        category::{id, CategoryId},
        loader::{DirectoryLoader, LoaderAssetReader, PackLoaderContext},
        script::{
            self,
            format_err,
            lua::{
                self,
                to_lua_error,
                DiscardValues,
                IntoLuaMultiMut,
                IntoLuaMut,
                LuaCallable,
                RuntimeLua,
            },
            pathing::event::NotifyScript,
        },
    },
    taimi_sync::watched::watch,
};

pub struct LuaController {
    pub runtime: Option<RuntimeLua>,
    pub rx: mpsc::Receiver<LuaMessage>,
    pub plugs_shared: watch::Sender<PlugsShared>,
    pub packs: Vec<LuaPackDesc>,
    pub plugs: Vec<LuaPlugDesc>,
    pub plug_count: usize,
    pub exiting: bool,
    pub timestamp_start: Instant,
    /// latest/previous tick event time
    pub timestamp_tick: Instant,
    pub prev_tick: u32,
    /// hack
    pub signal_markers_started: bool,
}
impl LuaController {
    pub fn new(rx: mpsc::Receiver<LuaMessage>, plugs_shared: watch::Sender<PlugsShared>) -> Self {
        Self {
            rx,
            plugs_shared,
            runtime: None,
            packs: Default::default(),
            plugs: Default::default(),
            plug_count: 0,
            exiting: false,
            timestamp_start: Instant::now(),
            timestamp_tick: Instant::now(),
            prev_tick: 0,
            signal_markers_started: false,
        }
    }
    pub fn run_new(
        rx: mpsc::Receiver<LuaMessage>,
        plugs_shared: watch::Sender<PlugsShared>,
    ) -> anyhow::Result<()> {
        Self::new(rx, plugs_shared).run()
    }
    pub fn run(&mut self) -> anyhow::Result<()> {
        while !self.exiting {
            let mut notif = Some(ScriptNotification::Nop);
            let incoming = if self.signal_markers_started {
                Ok(LuaMessage::InternalMarkersStarted)
            } else {
                self.rx.try_recv()
            };
            let mut msg = match incoming {
                #[cfg(feature = "paths-lua")]
                Ok(
                    msg @ LuaMessage::NotifyScript0 {
                        id: ScriptNotification::PathingTick, ..
                    },
                ) => self.preprocess_message(msg),
                Ok(LuaMessage::NotifyScript0 { id, context: LuaExecContext::Global }) => {
                    notif = Some(id);
                    None
                },
                Ok(m) => {
                    match &m {
                        LuaMessage::TearDown => notif = Some(ScriptNotification::Exit),
                        LuaMessage::Exit => notif = None,
                        _ => (),
                    }
                    self.preprocess_message(m)
                },
                Err(mpsc::error::TryRecvError::Empty) => None,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            };
            let mut still_waiting = false;
            if let (Some(lua), Some(next)) = (self.runtime.as_ref(), notif) {
                'pack: for desc in &mut self.packs {
                    match (desc.received, next) {
                        (ScriptSignal::Ended, _) => continue 'pack,
                        (ScriptSignal::Pending, ScriptNotification::Nop) => continue 'pack,
                        (ScriptSignal::Started | ScriptSignal::Resume | _, _) => (),
                    }
                    let Some(thread) = rt::log::warn_ok(desc.running()) else {
                        desc.received = ScriptSignal::Ended;
                        continue 'pack
                    };
                    let next = thread.call(ScriptNotification0(next));
                    let _ = rt::log::warn_ok(desc.spun(lua, next));
                    if desc.wants_poll() {
                        still_waiting |= true;
                    }
                }
                'plug: for desc in &mut self.plugs {
                    match (desc.received, next) {
                        (ScriptSignal::Ended, _) => continue 'plug,
                        (ScriptSignal::Pending, ScriptNotification::Nop) => continue 'plug,
                        (ScriptSignal::Started | ScriptSignal::Resume | _, _) => (),
                    }
                    let Some(thread) = rt::log::warn_ok(desc.running()) else {
                        desc.received = ScriptSignal::Ended;
                        continue 'plug
                    };
                    let next = thread.call(ScriptNotification0(next));
                    let _ = rt::log::warn_ok(desc.spun(lua, next));
                    if desc.wants_poll() {
                        still_waiting |= true;
                    }
                }
            }
            #[cfg(todo = "unnecessary")]
            if self.signal_markers_started {
                still_waiting = true;
            }
            if msg.is_none() && !still_waiting {
                msg = self
                    .rx
                    .blocking_recv()
                    .and_then(|msg| self.preprocess_message(msg));
            }
            if let Some(msg) = msg {
                let name: &'static str = (&msg).into();
                let res = self
                    .process_message(msg)
                    .with_context(|| format!("processing {name}"));
                let cont = rt::log::warn_ok(res);
                if cont == Some(false) {
                    break
                }
            }
        }
        Ok(())
    }
    fn preprocess_message(&mut self, msg: LuaMessage) -> Option<LuaMessage> {
        match msg {
            #[cfg(feature = "paths-lua")]
            LuaMessage::NotifyScript0 {
                id: id @ ScriptNotification::PathingTick,
                context: context @ LuaExecContext::Global,
            } => {
                let tick = crate::exports::runtime::mumble_link_ptr()
                    .ok()
                    .map(|ml| ml.read_ui_tick())
                    .unwrap_or(0);
                let now = Instant::now();
                let args = taimi_pack::script::value::GameTime {
                    elapsed: now.duration_since(self.timestamp_tick),
                    elapsed_ticks: if self.prev_tick == 0 { 0 } else { tick.wrapping_sub(self.prev_tick) },
                    total: now.duration_since(self.timestamp_start),
                    total_ticks: tick,
                };
                self.timestamp_tick = now;
                self.prev_tick = tick;
                let args = vec![Box::new(Some(args)) as Box<dyn IntoLuaMut + Send>];
                Some(LuaMessage::NotifyScriptWith { id, context, args })
            },
            #[cfg(feature = "paths-lua")]
            LuaMessage::InternalMarkersStarted => {
                let mut processed = None;
                self.signal_markers_started = false;
                for pack in &self.packs {
                    let Ok(mut pending) = pack.shared().pending_start.lock() else { continue };
                    if pending.is_empty() {
                        continue
                    }
                    if processed.is_some() {
                        // more than one pack wants attention
                        self.signal_markers_started = true;
                        break
                    }
                    let markers = core::mem::take(&mut *pending);
                    processed = Some(LuaMessage::NotifyMapEnter {
                        target: pack.path(),
                        map_id: 0,
                        active_markers: Box::new(IntoIterator::into_iter(markers.into_boxed_slice()))
                            as Box<_>,
                        append: true,
                    });
                }
                processed
            },
            m => Some(m),
        }
    }
    fn process_message(&mut self, msg: LuaMessage) -> anyhow::Result<bool> {
        match msg {
            LuaMessage::TearDown => {
                let res = self.teardown().context("lua teardown");
                let _ = rt::log::warn_ok(res);
                return Ok(false)
            },
            LuaMessage::SpinUp => {
                self.init_lua()?;
            },
            LuaMessage::Exit => {
                self.exiting = true;
                return Ok(false)
            },
            LuaMessage::Exec { source, context, interactive } => {
                let lua = self.runtime.as_ref().context("lualess")?;
                let env = match context {
                    LuaExecContext::Global => None,
                    LuaExecContext::Plugin(target) => {
                        log::warn!("TODO: exec plug");
                        None
                    },
                    #[cfg(feature = "paths-lua")]
                    LuaExecContext::Pack(target) => {
                        let pack =
                            Self::locate_pack_mut(&mut self.packs, target).context("unknown pack")?;
                        Some(pack.globals.clone())
                    },
                };
                let chunk = lua
                    .lua()
                    .load(&source[..])
                    .set_mode(mlua::ChunkMode::Text)
                    .set_name("=exec.lua");
                let chunk = match env {
                    None => chunk,
                    Some(e) => chunk.set_environment(e),
                };
                let chunk = chunk.into_function()?;
                let res = chunk.call::<mlua::MultiValue>(())?;
                if interactive {
                    for v in res {
                        log::debug!("{}", lua.debug_display(v));
                    }
                }
            },
            LuaMessage::DebugWatchRefresh { context, interactive } => {
                let Some(lua) = self.runtime.as_ref() else { return Ok(true) };
                match Self::context_globals(context, lua, &self.packs, &self.plugs)? {
                    Some(g) => {
                        // TODO: this is cheating...
                        let watch = g
                            .get::<Option<mlua::Table>>("Debug")
                            .and_then(|w| w.map(|w| w.get::<Option<mlua::Table>>("watches")).transpose())
                            .and_then(|w| {
                                w.flatten()
                                    .map(|w| {
                                        mlua::ObjectLike::call_method::<mlua::Table>(&w, "GetWatches", ())
                                    })
                                    .transpose()
                            });
                        let watch = rt::log::warn_ok(watch).flatten();
                        if let Some(watches) = &watch {
                            if interactive {
                                log::info!("{:#}", lua.debug_display(watches));
                            }
                            let plug = match context {
                                LuaExecContext::Pack(target) =>
                                    Self::locate_pack(&self.packs, target).ok().map(|p| &p.plug),
                                LuaExecContext::Plugin(target) =>
                                    Self::locate_plug(&self.plugs, target).ok().map(|p| &p.base),
                                LuaExecContext::Global => None,
                            };
                            if let Some(plug) = plug {
                                let mut state = plug.state().write();
                                state.set_debug_watches(watches.pairs().filter_map(|p| {
                                    p.ok().map(|(k, v): (mlua::Value, mlua::Value)| {
                                        (
                                            lua.debug_display(k).to_string(),
                                            lazyfmt::display2debug(lua.debug_display(v)),
                                        )
                                    })
                                }));
                            }
                        }
                    },
                    None => log::debug!("TODO: refresh all globals"),
                }
            },
            LuaMessage::NotifyScript0 { id, context: LuaExecContext::Global } => {
                let Some(lua) = self.runtime.as_ref() else { return Ok(true) };
                let context = || format!("notifying script {id:?}");
                for pack in &mut self.packs {
                    let res = pack.notify0(lua, id).with_context(context);
                    let _ = rt::log::warn_ok(res);
                }
            },
            LuaMessage::NotifyScriptWith {
                id,
                args,
                context: LuaExecContext::Global,
            } => {
                let Some(lua) = self.runtime.as_ref() else { return Ok(true) };
                let context = || format!("notifying script {id:?}");
                let args = IntoIterator::into_iter(args)
                    .map(|mut v| v.into_lua_mut(lua.lua()))
                    .collect::<mlua::Result<mlua::MultiValue>>()?;
                let args = IntoLuaMulti::into_lua_multi(args, lua.lua())?;
                #[cfg(feature = "paths-lua")]
                for pack in &mut self.packs {
                    let mut args = Some(args.clone());
                    let args = &mut args as &mut dyn IntoLuaMultiMut;
                    let res = pack.notify_with(lua, id, args).with_context(context);
                    let _ = rt::log::warn_ok(res);
                }
                for plug in &mut self.plugs {
                    let mut args = Some(args.clone());
                    let args = &mut args as &mut dyn IntoLuaMultiMut;
                    let res = plug.notify_with(lua, id, args).with_context(context);
                    let _ = rt::log::warn_ok(res);
                }
            },
            LuaMessage::NotifyScriptWith {
                id,
                args,
                context: LuaExecContext::Plugin(target),
            } => {
                let Some(lua) = self.runtime.as_ref() else { return Ok(true) };
                let context = || format!("notifying pack {id:?}");
                let args = IntoIterator::into_iter(args)
                    .map(|mut v| v.into_lua_mut(lua.lua()))
                    .collect::<mlua::Result<mlua::MultiValue>>()?;
                let res = Self::locate_plug_mut(&mut self.plugs, target)
                    .and_then(|plug| plug.notify_with(lua, id, args))
                    .with_context(context);
                res?;
            },
            LuaMessage::NotifyScript0 {
                id,
                context: LuaExecContext::Plugin(target),
            } => {
                let Some(lua) = self.runtime.as_ref() else { return Ok(true) };
                let context = || format!("notifying pack {id:?}");
                let res = Self::locate_plug_mut(&mut self.plugs, target)
                    .and_then(|plug| plug.notify0(lua, id))
                    .with_context(context);
                res?;
            },
            #[cfg(feature = "paths-lua")]
            LuaMessage::NotifyScriptWith {
                id,
                args,
                context: LuaExecContext::Pack(target),
            } => {
                let Some(lua) = self.runtime.as_ref() else { return Ok(true) };
                let context = || format!("notifying pack {id:?}");
                let args = IntoIterator::into_iter(args)
                    .map(|mut v| v.into_lua_mut(lua.lua()))
                    .collect::<mlua::Result<mlua::MultiValue>>()?;
                let res = Self::locate_pack_mut(&mut self.packs, target)
                    .and_then(|pack| pack.notify_with(lua, id, args))
                    .with_context(context);
                res?;
            },
            #[cfg(feature = "paths-lua")]
            LuaMessage::NotifyScript0 {
                id,
                context: LuaExecContext::Pack(target),
            } => {
                let Some(lua) = self.runtime.as_ref() else { return Ok(true) };
                let context = || format!("notifying pack {id:?}");
                let res = Self::locate_pack_mut(&mut self.packs, target)
                    .and_then(|pack| pack.notify0(lua, id))
                    .with_context(context);
                res?;
            },
            #[cfg(feature = "paths-lua")]
            LuaMessage::InternalMarkersStarted => {
                self.signal_markers_started = true;
            },
            #[cfg(feature = "paths-lua")]
            LuaMessage::RefreshMarkerFocus { and_interact } => {
                use taimi_pack::script::pathing::imp::MarkerType;

                let playerpos = rt::mumble_link_ptr().ok().map(|ml| {
                    glam::Vec3A::from_array(unsafe { *&raw const (*ml.as_ptr()).avatar.position })
                });
                let Some(playerpos) = playerpos else { return Ok(true) };
                let mut good_enough = false;
                let mut msg_queue = Vec::new();
                for desc in &self.packs {
                    let Ok(mut active) = desc.shared().active_markers.try_lock() else { continue };
                    let mut focused = active.iter_mut().filter(|(_, s)| s.focused).peekable();
                    if focused.peek().is_none() {
                        continue
                    }
                    let Some(pack) = desc.shared().get_pack().ok() else { continue };
                    let Some(overrides) = PackOverrides::shared_try_read(&desc.shared().overrides) else {
                        continue
                    };
                    for (tag, status) in focused {
                        let Some(path @ (MarkerType::Poi, idx)) = LuaPackDesc::pathable_tag_from(*tag)
                        else {
                            continue
                        };
                        let o = overrides.overrides.get(&path).map(MarkerOverrides::shared_read);
                        let mo;
                        let attrs = match (pack.pois.get(idx), o.as_ref()) {
                            (Some(poi), Some(o)) => {
                                mo = MarkerOverridesAttrs::wrap_with_overrides(poi, &*o);
                                &mo as &dyn GetAttrDyn
                            },
                            (None, Some(o)) => &o.attrs as &_,
                            (Some(poi), None) => poi as &_,
                            (None, None) => continue,
                        };
                        let pos = glam::Vec3A::new(
                            attrs
                                .clone_attr_dyn_of::<keys::PositionX>()
                                .and_then(|v| v.into_value())
                                .unwrap_or_default()
                                .into(),
                            attrs
                                .clone_attr_dyn_of::<keys::PositionY>()
                                .and_then(|v| v.into_value())
                                .unwrap_or_default()
                                .into(),
                            attrs
                                .clone_attr_dyn_of::<keys::PositionZ>()
                                .and_then(|v| v.into_value())
                                .unwrap_or_default()
                                .into(),
                        );
                        let range = attrs
                            .clone_attr_dyn_of::<keys::TriggerRange>()
                            .and_then(|v| v.into_value())
                            .map(f32::from)
                            .or_else(|| {
                                attrs
                                    .clone_attr_dyn_of::<keys::InfoRange>()
                                    .and_then(|v| v.into_value())
                                    .map(f32::from)
                            })
                            .unwrap_or(keys::TriggerRange::DEFAULT.into());
                        if playerpos.distance_squared(pos) > range.powi(2) {
                            status.focused = false;
                            if attrs.has_attr_dyn_of::<keys::ScriptFocus>() {
                                let msg = ScriptMessage::marker_event_bool(
                                    ScriptNotification::PathingFocus,
                                    false,
                                    path,
                                    desc.path.generation,
                                    desc.path.index,
                                );
                                match msg {
                                    ScriptMessage::Lua(m) => {
                                        msg_queue.push(m);
                                    },
                                    #[cfg(todo = "unnecessary")]
                                    m => m.try_send(),
                                    _ => (),
                                }
                            }
                            continue
                        }
                        let seems_interactive = || {
                            attrs.has_attr_dyn_of::<keys::ScriptTrigger>()
                                || attrs.has_attr_dyn_of::<keys::CopyValue>()
                                || attrs.has_attr_dyn_of::<keys::Info>()
                        };
                        if and_interact && seems_interactive() {
                            let msg = ScriptMessage::marker_event_bool(
                                ScriptNotification::PathingTrigger,
                                false,
                                path,
                                desc.path.generation,
                                desc.path.index,
                            );
                            match msg {
                                ScriptMessage::Lua(m) => {
                                    msg_queue.push(m);
                                },
                                #[cfg(todo = "unnecessary")]
                                m => m.try_send(),
                                _ => (),
                            }
                            if let Some(copy) = attrs
                                .clone_attr_dyn_of::<keys::CopyValue>()
                                .and_then(|v| v.into_value())
                            {
                                if !copy[..].is_empty() {
                                    let msg = attrs
                                        .clone_attr_dyn_of::<keys::CopyMessage>()
                                        .and_then(|v| v.into_value());
                                    super::ui::ScriptHostUiX::new()
                                        .set_clipboard(&copy.0[..], msg.as_ref().map(|m| &m.0[..]));
                                }
                            }
                            #[cfg(todo)]
                            if let Some(info) = attrs
                                .clone_attr_dyn_of::<keys::Info>()
                                .and_then(|v| v.into_value())
                            {
                                super::ui::ScriptHostUiX::new().info_notify(&info.0[..], None);
                            }
                        }
                        good_enough |= range < 10.0;
                    }
                }
                #[cfg(todo)]
                let focus = (!good_enough).then_some(self.packs.iter()).into_iter().flatten();
                let focus = self.packs.iter();
                for desc in focus {
                    // otherwise, try to find something...
                    let Ok(mut active) = desc.shared().active_markers.try_lock() else { continue };
                    if active.is_empty() {
                        continue
                    }
                    let Some(pack) = desc.shared().get_pack().ok() else { continue };
                    let Some(overrides) = PackOverrides::shared_try_read(&desc.shared().overrides) else {
                        continue
                    };
                    for (tag, status) in active.iter_mut() {
                        if status.focused {
                            continue
                        }
                        let Some(path @ (MarkerType::Poi, idx)) = LuaPackDesc::pathable_tag_from(*tag)
                        else {
                            continue
                        };
                        let o = overrides.overrides.get(&path).map(MarkerOverrides::shared_read);
                        let mo;
                        let attrs = match (pack.pois.get(idx), o.as_ref()) {
                            (Some(poi), Some(o)) => {
                                mo = MarkerOverridesAttrs::wrap_with_overrides(poi, &*o);
                                &mo as &dyn GetAttrDyn
                            },
                            (None, Some(o)) => &o.attrs as &_,
                            (Some(poi), None) => poi as &_,
                            (None, None) => continue,
                        };
                        let wants_focus_event = attrs.has_attr_dyn_of::<keys::ScriptFocus>();
                        let auto_trigger = || -> bool {
                            attrs
                                .clone_attr_dyn_of::<keys::AutoTrigger>()
                                .and_then(|v| v.into_value())
                                .unwrap_or_default()
                                .into()
                        };
                        let seems_interactive = attrs.has_attr_dyn_of::<keys::CopyValue>()
                            || attrs.has_attr_dyn_of::<keys::Info>()
                            || attrs.has_attr_dyn_of::<keys::ScriptTrigger>()
                            || wants_focus_event;
                        if !seems_interactive && !auto_trigger() {
                            continue
                        }
                        let pos = glam::Vec3A::new(
                            attrs
                                .clone_attr_dyn_of::<keys::PositionX>()
                                .and_then(|v| v.into_value())
                                .unwrap_or_default()
                                .into(),
                            attrs
                                .clone_attr_dyn_of::<keys::PositionY>()
                                .and_then(|v| v.into_value())
                                .unwrap_or_default()
                                .into(),
                            attrs
                                .clone_attr_dyn_of::<keys::PositionZ>()
                                .and_then(|v| v.into_value())
                                .unwrap_or_default()
                                .into(),
                        );
                        let range = attrs
                            .clone_attr_dyn_of::<keys::TriggerRange>()
                            .and_then(|v| v.into_value())
                            .map(f32::from)
                            .or_else(|| {
                                attrs
                                    .clone_attr_dyn_of::<keys::InfoRange>()
                                    .and_then(|v| v.into_value())
                                    .map(f32::from)
                            })
                            .unwrap_or(keys::TriggerRange::DEFAULT.into());
                        if playerpos.distance_squared(pos) <= range.powi(2) {
                            status.focused = true;
                            if wants_focus_event {
                                let msg = ScriptMessage::marker_event_bool(
                                    ScriptNotification::PathingFocus,
                                    true,
                                    path,
                                    desc.path.generation,
                                    desc.path.index,
                                );
                                match msg {
                                    ScriptMessage::Lua(m) => {
                                        msg_queue.push(m);
                                    },
                                    #[cfg(todo = "unnecessary")]
                                    m => m.try_send(),
                                    _ => (),
                                }
                            }
                            if let Some(info) = attrs
                                .clone_attr_dyn_of::<keys::Info>()
                                .and_then(|v| v.into_value())
                            {
                                super::ui::ScriptHostUiX::new().info_notify(&info.0[..], None);
                            }
                        }
                    }
                }
                for m in msg_queue {
                    let _ = rt::log::warn_ok(self.process_message(m));
                }
            },
            #[cfg(feature = "paths-lua")]
            LuaMessage::NotifyMapEnter { target, map_id, active_markers, append } => {
                let map_id = match NonZero::new(map_id) {
                    None if append => Some(None),
                    None => None,
                    Some(m) => Some(Some(m)),
                };
                if let Some(map_id) = map_id {
                    self.start_map_markers(target, map_id, active_markers)?;
                }
            },
            #[cfg(feature = "paths-lua")]
            LuaMessage::SpawnPack(pack, loader, entrypoint, generation, idx) => {
                let res = self
                    .spawn_pack(&pack, &loader, entrypoint, generation, idx)
                    .and_then(|desc| self.start_pack(desc))
                    .with_context(|| format!("spawning pack.lua for {}", pack.name));
                res?;
            },
            LuaMessage::SpawnPlug(path) => {
                let pathname = rt::relative_path(&path);
                let entrypoint =
                    fs::File::open(&path).with_context(|| format!("opening {}", pathname.display()))?;
                let res = self
                    .spawn_plug(&path, &mut { entrypoint })
                    .and_then(|desc| self.start_plug(desc))
                    .with_context(|| format!("spawning plug.lua for {}", pathname.display()));
                res?;
            },
            LuaMessage::Stop { context: LuaExecContext::Global } => {
                for i in 0..self.plugs.len() {
                    let res = self
                        .process_message(LuaMessage::Stop { context: LuaExecContext::Plugin(i) })
                        .context(lazyfmt::fmt_args!(move "stopping plug#{i}"));
                    rt::log::warn_ok(res);
                }
                #[cfg(feature = "paths-lua")]
                let packs = self.packs.iter().map(|p| p.path()).collect::<Vec<_>>();
                #[cfg(feature = "paths-lua")]
                for path in packs {
                    let res = self
                        .process_message(LuaMessage::Stop { context: LuaExecContext::Pack(path) })
                        .context(lazyfmt::fmt_args!(move "stopping {path}"));
                    rt::log::warn_ok(res);
                }
            },
            LuaMessage::Stop {
                context: LuaExecContext::Plugin(target),
            } => {
                let base = self.plugs.as_ptr();
                let desc = Self::locate_plug_mut(&mut self.plugs, target)?;
                let idx = unsafe { (&*desc as *const LuaPlugDesc).offset_from_unsigned(base) };
                let lua = self.runtime.as_ref().context("lualess")?;
                let res = desc.exit(lua).with_context(|| format!("stopping {desc}"));
                self.plugs.swap_remove(idx);
                self.plugs_shared.send_modify(|shared| {
                    shared.plugs.remove(&target);
                });
                let () = res?;
            },
            #[cfg(feature = "paths-lua")]
            LuaMessage::Stop { context: LuaExecContext::Pack(target) } => {
                let base = self.packs.as_ptr();
                let desc = Self::locate_pack_mut(&mut self.packs, target)?;
                let idx = unsafe { (&*desc as *const LuaPackDesc).offset_from_unsigned(base) };
                let lua = self.runtime.as_ref().context("lualess")?;
                let res = desc.exit(lua).with_context(|| format!("stopping {desc}"));
                self.packs.swap_remove(idx);
                self.plugs_shared.send_modify(|shared| {
                    shared.packs.remove(&target);
                });
                let () = res?;
            },
        }
        Ok(true)
    }
    fn init_lua(&mut self) -> anyhow::Result<()> {
        if self.runtime.is_some() {
            return Ok(())
        } else if self.exiting {
            anyhow::bail!("shutting down")
        }

        #[cfg(feature = "paths")]
        let enables = Controller::with_sender(|s| s.pathing.as_ref().map(|p| *p.enables.borrow()))
            .flatten()
            .unwrap_or_default();
        let unsecured = match () {
            #[cfg(feature = "paths")]
            _ if enables.contains(crate::controller::pathing::PathingEnables::SCRIPTING_UNSECURED) =>
                Some(lua::UnsafeRuntime),
            _ => None,
        };
        let opts = Default::default();
        let lua = RuntimeLua::new_script_runtime(opts, unsecured).context("lua runtime init")?;
        let lua = &*self.runtime.insert(lua);
        lua.setup_package_builtin()?;
        lua.setup_api_rt()?;
        lua.setup_api_version(super::debug::ScriptHostVersion::new())?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/util/init.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/util/ud.lua")?;
        lua.setup_api_log(super::debug::ScriptHostDebug::new())?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/debug.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/bitop.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/id.lua")?;
        lua.setup_api_vectors()?;
        lua.setup_api_mumble(super::mumble::ScriptHostMumbleLink::new())?;
        lua.setup_api_event(super::event::ScriptHostEvent::new())?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/event.lua")?;
        lua.setup_api_bindings::<rt::bindings::GameControls, _, _>(rt::bindings::interesting_controls())?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/bindings.lua")?;
        lua.setup_api_ui_exchange(super::ui::ScriptHostUiX::new())?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/ui/exchange.lua")?;
        lua.setup_api_attrs()?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/pack/attrs.lua")?;
        //lua.setup_api_interact()?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/todo/interact.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/pack/interact.lua")?;
        lua.setup_api_ui_menu::<LuaPlugBase, PlugMenu<Arc<dyn PlugSharedRef>>>()?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/ui/menu.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/compat/category.lua")?;
        #[cfg(feature = "paths")]
        {
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/compat/trail.lua")?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/compat/poi.lua")?;
        }
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/compat/menu.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/compat/env.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/compat/init.lua")?;

        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/event/init.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/mumblelink.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/plug/log.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/plug/persist.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/plug/loader.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/plug/init.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/menu/init.lua")?;
        #[cfg(feature = "extension-nexus")]
        {
            use crate::controller::script::nexus::ScriptHostNexus;
            lua.preload_builtin("@taimi/core/nexus", ScriptHostNexus::default())?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/nexus/datalink/init.lua")?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/nexus/datalink/rtapi.lua")?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/nexus/event/init.lua")?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/nexus/input/init.lua")?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/nexus/paths.lua")?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/nexus/quickaccess.lua")?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/nexus/texture.lua")?;
            EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/nexus/init.lua")?;
        }
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/v0/init.lua")?;

        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/main/pack.lua")?;
        EmbeddedLuaTaimi::preload_lib(lua, "@taimi/main/plug.lua")?;
        Ok(())
    }
    #[cfg(feature = "paths")]
    #[inline]
    fn locate_pack_mut(packs: &mut [LuaPackDesc], loc: PackLoc) -> anyhow::Result<&mut LuaPackDesc> {
        packs
            .iter_mut()
            .find(|p| p.path() == loc)
            .with_context(|| format!("lost pack {loc}"))
    }
    #[inline]
    fn locate_plug_mut(plugs: &mut [LuaPlugDesc], loc: usize) -> anyhow::Result<&mut LuaPlugDesc> {
        let idx = match plugs.get(loc) {
            Some(p) if p.index == loc => Some(loc),
            _ => plugs.iter().position(|p| p.index == loc),
        };
        idx.map(|idx| unsafe { plugs.get_unchecked_mut(idx) })
            .with_context(|| format!("lost plug#{loc}"))
    }
    #[cfg(feature = "paths")]
    #[inline]
    fn locate_pack(packs: &[LuaPackDesc], loc: PackLoc) -> anyhow::Result<&LuaPackDesc> {
        packs
            .iter()
            .find(|p| p.path() == loc)
            .with_context(|| format!("lost pack {loc}"))
    }
    #[inline]
    fn locate_plug(plugs: &[LuaPlugDesc], loc: usize) -> anyhow::Result<&LuaPlugDesc> {
        match plugs.get(loc) {
            Some(p) if p.index == loc => Some(p),
            _ => None,
        }
        .or_else(|| plugs.iter().find(|p| p.index == loc))
        .with_context(|| format!("lost plug#{loc}"))
    }
    fn context_plug<'a>(
        context: LuaExecContext,
        lua: &RuntimeLua,
        packs: &'a [LuaPackDesc],
        plugs: &'a [LuaPlugDesc],
    ) -> anyhow::Result<Option<&'a LuaPlugBase>> {
        match context {
            LuaExecContext::Global => Ok(None),
            #[cfg(feature = "paths")]
            LuaExecContext::Pack(path) => Self::locate_pack(packs, path).map(|desc| Some(&desc.plug)),
            LuaExecContext::Plugin(path) => Self::locate_plug(plugs, path).map(|desc| Some(&desc.base)),
        }
    }
    fn context_plug_mut<'a>(
        context: LuaExecContext,
        lua: &RuntimeLua,
        packs: &'a mut [LuaPackDesc],
        plugs: &'a mut [LuaPlugDesc],
    ) -> anyhow::Result<Option<&'a mut LuaPlugBase>> {
        match context {
            LuaExecContext::Global => Ok(None),
            #[cfg(feature = "paths")]
            LuaExecContext::Pack(path) => {
                let pack = Self::locate_pack_mut(packs, path);
                pack.map(|desc| Some(&mut desc.plug))
            },
            LuaExecContext::Plugin(path) => {
                let plug = Self::locate_plug_mut(plugs, path);
                plug.map(|desc| Some(&mut desc.base))
            },
        }
    }
    #[inline]
    fn context_globals<'a>(
        context: LuaExecContext,
        lua: &RuntimeLua,
        packs: &'a [LuaPackDesc],
        plugs: &'a [LuaPlugDesc],
    ) -> anyhow::Result<Option<&'a mlua::Table>> {
        Self::context_plug(context, lua, packs, plugs).map(|res| res.map(|plug| &plug.globals))
    }
    fn teardown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    #[cfg(feature = "paths-lua")]
    fn spawn_pack(
        &mut self,
        pack: &Arc<Pack>,
        loader: &SharedLoader,
        entrypoint: Box<dyn LoaderAssetReader>,
        generation: usize,
        pack_idx: usize,
    ) -> anyhow::Result<LuaPackDesc> {
        let mut pack_lua = Vec::new();
        { entrypoint }
            .read_to_end(&mut pack_lua)
            .context("reading pack.lua")?;

        let lua = {
            let _ = self.init_lua()?;
            self.runtime.as_ref().context("lua expected")?
        };
        let common_globals = lua.pack_globals_shared();
        let pack_globals = {
            let mt = lua.lua().create_table()?;
            mt.set(MetaMethod::Index.name(), common_globals)?;
            // TODO: mt.set(MetaMethod::NewIndex.name(), pack_globals_mut);
            let genv = lua.lua().create_table()?;
            genv.set_metatable(Some(mt))?;
            genv
        };
        let chunk = lua
            .lua()
            .load(&pack_lua)
            .set_name(format!("={}/pack.lua", pack.name))
            .set_environment(pack_globals.clone());
        let chunk = match lua.is_unsecured() {
            None => chunk.set_mode(mlua::ChunkMode::Text),
            Some(lua::UnsafeRuntime) => chunk,
        }
        .into_function()?;
        let path = PackLoc::new(generation, pack_idx);
        let mut pack_info = LuaPackDesc {
            plug: LuaPlugBase::with_globals(
                pack_globals,
                PackPlugShared::new(path, &pack, Arc::downgrade(loader)),
            ),
            path,
        };

        let runner = {
            let require = lua
                .lua()
                .globals()
                .get::<mlua::Function>(RuntimeLua::STD_PACKAGE_REQUIRE);
            let main = require
                .and_then(|req| req.call::<mlua::Table>("@taimi/main/pack"))
                .and_then(|main| main.get::<mlua::Function>("pathing_pack_start"));
            main
        }
        .context("preparing pack.lua loader")?;

        SpaceEvent::ScriptStart {
            generation,
            pack_idx,
            shared: pack_info.shared_arc(),
        }
        .try_send();

        runner
            .call::<LuaCallable>((
                RuntimeLua::new_api_pack_info(pack_info.clone(), pack_info.globals.clone()),
                chunk,
                &pack_info.globals,
            ))
            .map(move |co| {
                pack_info.co = Some(co);
                pack_info
            })
            .context("pack_start")
    }
    fn spawn_plug(&mut self, path: &Path, entrypoint: &mut dyn io::Read) -> anyhow::Result<LuaPlugDesc> {
        let mut main_lua = Vec::new();
        let name = Path::new(
            match path.file_name() {
                Some(n) if n.eq("init.lua") => path.parent().and_then(|p| p.file_name()),
                n => n,
            }
            .unwrap_or("plug.lua".as_ref()),
        );
        let name = match name.extension() {
            Some(ext) if ext.eq_ignore_ascii_case("lua") => name.file_stem(),
            _ => None,
        }
        .unwrap_or(name.as_ref());
        { entrypoint }.read_to_end(&mut main_lua).context("reading src")?;
        let dir = path.parent().unwrap_or(path);

        let lua = {
            let _ = self.init_lua()?;
            self.runtime.as_ref().context("lua expected")?
        };
        let common_globals = lua.pack_globals_shared();
        let pack_globals = {
            let mt = lua.lua().create_table()?;
            mt.set(MetaMethod::Index.name(), common_globals)?;
            // TODO: mt.set(MetaMethod::NewIndex.name(), pack_globals_mut);
            let genv = lua.lua().create_table()?;
            genv.set_metatable(Some(mt))?;
            genv
        };
        let chunk = lua
            .lua()
            .load(&main_lua)
            .set_name(format!("={}/plug.lua", name.display()))
            .set_environment(pack_globals.clone());
        let chunk = match lua.is_unsecured() {
            None => chunk.set_mode(mlua::ChunkMode::Text),
            Some(lua::UnsafeRuntime) => chunk,
        }
        .into_function()?;
        let mut plug_info = LuaPlugDesc {
            index: self.plug_count,
            dir: dir.into(),
            base: LuaPlugBase::with_globals(
                pack_globals,
                PlugSharedData::with_name(&name.to_string_lossy()[..]),
            ),
        };
        self.plug_count += 1;

        let runner = {
            let require = lua
                .lua()
                .globals()
                .get::<mlua::Function>(RuntimeLua::STD_PACKAGE_REQUIRE);
            let main = require
                .and_then(|req| req.call::<mlua::Table>("@taimi/main/plug"))
                .and_then(|main| main.get::<mlua::Function>("plugin_start"));
            main
        }
        .context("preparing loader")?;

        #[cfg(todo)]
        RenderEvent::ScriptStart {
            index: plug_info.index,
            menus: plug_info.menus.clone(),
            state: plug_info.state.clone(),
        }
        .try_send();

        runner
            .call::<LuaCallable>((
                RuntimeLua::new_api_pack_info(plug_info.clone(), plug_info.globals.clone()),
                chunk,
                &plug_info.globals,
            ))
            .map(move |co| {
                plug_info.co = Some(co);
                plug_info
            })
            .context("pack_start")
    }
    #[cfg(feature = "paths-lua")]
    fn start_pack(&mut self, mut desc: LuaPackDesc) -> anyhow::Result<()> {
        let mut timeout = LuaPlugBase::START_TIMEOUT;
        debug_assert!(desc.co.is_some());
        let lua = self.runtime.as_ref().context("lualess")?;
        loop {
            let next = desc.plug.turn_start::<(), _>(lua, &mut timeout, None);
            match next {
                Ok(None) => break,
                next => desc.spun(lua, next)?,
            }
        }
        if desc.received != ScriptSignal::Ended {
            self.plugs_shared.send_modify(|shared| {
                shared.packs.insert(desc.path(), desc.shared_arc());
            });
            self.packs.push(desc);
        }
        Ok(())
    }
    fn start_plug(&mut self, mut desc: LuaPlugDesc) -> anyhow::Result<()> {
        let mut timeout = LuaPlugBase::START_TIMEOUT;
        debug_assert!(desc.co.is_some());
        let lua = self.runtime.as_ref().context("lualess")?;
        loop {
            let next = desc.base.turn_start::<(), _>(lua, &mut timeout, None);
            match next {
                Ok(None) => break,
                next => desc.spun(lua, next)?,
            }
        }
        if desc.received != ScriptSignal::Ended {
            self.plugs_shared.send_modify(|shared| {
                shared.plugs.insert(desc.index, desc.shared_arc());
            });
            self.plugs.push(desc);
        }
        Ok(())
    }
    #[cfg(feature = "paths-lua")]
    fn start_map_markers<I>(
        &mut self,
        target: PackLoc,
        map_id: Option<NonZero<MapID>>,
        markers: I,
    ) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = MarkerLoc>,
    {
        use mlua::ObjectLike;

        let mut markers = markers.into_iter().peekable();
        if markers.peek().is_none() {
            return Ok(())
        }

        let lua = self.runtime.as_ref().context("lualess")?;

        let desc = Self::locate_pack_mut(&mut self.packs, target)?;
        let pack = desc.shared().get_pack()?;
        let _ = desc.running()?;
        let overrides = desc.shared().overrides.clone();

        // TODO: lol
        let eventloop = desc
            .globals
            .get::<mlua::Table>("Taimi")?
            .get::<mlua::Table>("ctx")?
            .get::<mlua::Table>("events")?;

        let shared = desc.shared_arc();
        let mut marker_focus = shared.active_markers.try_lock().ok();
        if let Some(map_id) = map_id {
            if let Some(focus) = &mut marker_focus {
                focus.clear();
            }
            // event loop may want to clean up prior to receiving new set of handlers...
            let res = desc.notify_with(lua, ScriptNotification::PathingMapExit, (map_id.get(),));
            let _ = rt::log::error_ok(res);
        }

        let key_once = keys::ScriptOnce::pack_key_of();
        for loc in markers {
            if let Some(focus) = &mut marker_focus {
                focus.insert(LuaPackDesc::pathable_tag_for(loc), Default::default());
            }
            let marker = unsafe { PackMarkerRef::new_unchecked(pack.clone(), loc) };
            let overrides = PackOverrides::shared_read(&overrides);
            let attrs_o;
            let o = overrides.overrides.get(&loc).map(MarkerOverrides::shared_read);
            let attrs = match o.as_ref() {
                None => marker.get_attrs_dyn(),
                Some(o) => {
                    attrs_o = MarkerOverridesAttrs::wrap_with_overrides(&marker, o);
                    &attrs_o as &_
                },
            };
            let script_attrs = [
                (keys::ScriptFocus::pack_key_of(), ScriptNotification::PathingFocus),
                (
                    keys::ScriptFilter::pack_key_of(),
                    ScriptNotification::PathingFilterMarker,
                ),
                (
                    keys::ScriptTrigger::pack_key_of(),
                    ScriptNotification::PathingTrigger,
                ),
                (
                    keys::ScriptTick::pack_key_of(),
                    ScriptNotification::PathingTickMarker,
                ),
                (key_once, ScriptNotification::PathingLoadMarker),
            ];
            /// ew but they're all repr(transparent) to the same type so let's skip some pain...
            unsafe fn script_cell_to_str(cell: &dyn AttrKeyValue) -> &keys::Script {
                let inconspicuous_whisling = cell as &dyn core::any::Any;
                &*(inconspicuous_whisling as *const _ as *const keys::Script)
            }
            let mut has_once = false;
            let markertag = LuaPackDesc::pathable_tag_for(loc);
            let guid = attrs.clone_attr_dyn_of::<keys::Guid>();
            let name = lazyfmt::fmt_fn(|f| {
                if let Some(guid) = &guid {
                    write!(f, "{guid}")
                } else {
                    write!(f, "{}#{}", loc.0, loc.1)
                }
            });
            for (key, id) in script_attrs {
                let Some(attr) = attrs.get_attr_dyn(key) else { continue };
                if key == key_once {
                    has_once = true;
                }
                let attr = unsafe { script_cell_to_str(&*attr) };
                let name = format!("{name}/{key}");
                let args = lua.prepare_script_attr_args(&name, attr[..].as_bytes(), desc.globals.clone());
                let Some((fname, lazyargs)) = rt::log::warn_ok(args) else { continue };
                let globals = desc.globals.clone();
                let callback = desc.globals.get::<mlua::Function>(&fname).or_else(|_| {
                    lua.lua().create_function(move |_lua, a: mlua::MultiValue| {
                        mlua::ErrorContext::with_context(globals.get::<mlua::Function>(&fname), |_| {
                            format!("{key} handler {}() missing", fname.display())
                        })
                        .and_then(move |f| f.call::<()>(a))
                    })
                })?;
                eventloop.call_method::<()>("RegisterMarkerAttr", (id, markertag, callback, lazyargs))?;
            }
            if has_once {
                // TODO: maybe schedule for next tick instead?
                let res = desc
                    .notify_with(lua, ScriptNotification::PathingLoadMarker, (markertag,))
                    .with_context(|| format!("{name}/{key_once}"));
                let _ = rt::log::warn_ok(res);
            }
        }

        Ok(())
    }
}

#[derive(strum::Display, strum::IntoStaticStr)]
pub enum LuaMessage {
    TearDown,
    SpinUp,
    Exit,
    Stop {
        context: LuaExecContext,
    },
    NotifyScript0 {
        id: ScriptNotification,
        context: LuaExecContext,
    },
    NotifyScriptWith {
        id: ScriptNotification,
        context: LuaExecContext,
        /// TODO: Iterator<Item = &dyn mut IntoLua> would be nice wouldn't it?
        args: Vec<Box<dyn IntoLuaMut + Send>>,
    },
    Exec {
        source: Cow<'static, [u8]>,
        context: LuaExecContext,
        /// initiated by debug console or repl etc
        interactive: bool,
    },
    DebugWatchRefresh {
        context: LuaExecContext,
        /// otherwise passive
        interactive: bool,
    },
    SpawnPlug(PathBuf),
    #[cfg(todo)]
    #[cfg(feature = "paths")]
    SpawnPlugLoader(SharedLoader, Box<dyn LoaderAssetReader>),
    #[cfg(feature = "paths-lua")]
    SpawnPack(Arc<Pack>, SharedLoader, Box<dyn LoaderAssetReader>, usize, usize),
    #[cfg(feature = "paths-lua")]
    NotifyMapEnter {
        target: PackLoc,
        map_id: MapID,
        active_markers: Box<dyn Iterator<Item = MarkerLoc> + Send>,
        append: bool,
    },
    #[cfg(todo)]
    SpawnPlug(String, Box<dyn std::io::BufRead>),
    /// signals that [PackPlugShared.pending_start] has some entries to process
    #[cfg(feature = "paths-lua")]
    InternalMarkersStarted,
    /// typically to prepare to process interaction
    #[cfg(feature = "paths-lua")]
    RefreshMarkerFocus {
        and_interact: bool,
    },
}
impl LuaMessage {
    pub fn try_send(self) {
        ScriptMessage::Lua(self).try_send();
    }

    pub fn wants_runtime(&self) -> bool {
        match self {
            Self::SpawnPlug(..) => true,
            #[cfg(feature = "paths-lua")]
            Self::SpawnPack(..) => true,
            #[cfg(todo)]
            Self::SpawnPlugLoader(..) => true,
            #[cfg(todo)]
            Self::SpawnPlug(..) => true,
            Self::SpinUp => true,
            _ => false,
        }
    }

    /// TODO: compare based on next scheduled interest, skip if idle, etc
    pub fn tick(ui_tick: Option<u32>) -> Option<Self> {
        match ui_tick {
            #[cfg(feature = "paths-lua")]
            _ => Some(Self::NotifyScript0 {
                id: ScriptNotification::PathingTick,
                context: LuaExecContext::Global,
            }),
            #[cfg(not(feature = "paths-lua"))]
            _ => None,
        }
    }
}
impl ScriptMessage {
    pub fn menu_clicked_plug(id: CategoryId, index: usize) -> Self {
        Self::menu_clicked_with(id, LuaExecContext::Plugin(index))
    }
    pub fn menu_clicked_with(id: CategoryId, context: LuaExecContext) -> Self {
        let args = vec![Box::new(Some(id)) as Box<dyn taimi_pack::script::lua::IntoLuaMut + Send>];
        LuaMessage::NotifyScriptWith {
            id: ScriptNotification::MenuClick,
            context,
            args,
        }
        .into()
    }
}
impl From<LuaMessage> for ScriptMessage {
    #[inline]
    fn from(m: LuaMessage) -> Self {
        Self::Lua(m)
    }
}
impl fmt::Debug for LuaMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = <&str>::from(self);
        f.debug_tuple("LuaMessage").field(&name).finish()
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, strum::IntoStaticStr)]
pub enum LuaExecContext {
    #[default]
    Global,
    Plugin(usize),
    #[cfg(feature = "paths-lua")]
    Pack(PackLoc),
}
#[derive(Clone)]
pub struct LuaPlugBase {
    pub globals: mlua::Table,
    pub co: Option<LuaCallable>,
    pub received: ScriptSignal,
    pub shared: Arc<dyn PlugSharedRef>,
}
impl LuaPlugBase {
    pub fn with_globals<S>(globals: mlua::Table, shared: S) -> Self
    where
        S: Into<Arc<dyn PlugSharedRef>>,
    {
        Self {
            globals,
            co: None,
            received: ScriptSignal::Resume,
            shared: shared.into(),
        }
    }

    pub fn wants_poll(&self) -> bool {
        match self.received {
            ScriptSignal::Ended | ScriptSignal::Pending => false,
            _ => true,
        }
    }
    pub fn running(&self) -> anyhow::Result<&LuaCallable> {
        match self.received {
            ScriptSignal::Ended => None,
            _ => self.co.as_ref(),
        }
        .context("not running")
    }

    pub const START_TIMEOUT: u32 = 0x800;
    const START_TIMEOUT_RESUME: u32 = 8;
    pub fn turn_start<V, R>(
        &mut self,
        _lua: &RuntimeLua,
        timeout: &mut u32,
        signal: Option<V>,
    ) -> mlua::Result<Option<R>>
    where
        R: mlua::FromLuaMulti,
        V: mlua::IntoLuaMulti,
    {
        let thread = match self.co.as_ref() {
            // this is dumb, called wrong if this ever happens...
            e => e.context("threadless").map_err(to_lua_error)?,
        };
        let weight = match self.received {
            ScriptSignal::Started => {
                log::debug!("{self} started up");
                return Ok(None)
            },
            ScriptSignal::Ended => {
                log::info!("{self} early exit");
                #[cfg(todo = "unnecessary")]
                return thread
                    .call::<()>((ScriptNotification0(ScriptNotification::Exit),))
                    .map(|()| None);
                return Ok(None)
            },
            ScriptSignal::Pending => {
                log::info!("{self} might start later");
                return Ok(None)
            },
            ScriptSignal::Resume => Self::START_TIMEOUT_RESUME,
            _ => 1,
        };
        *timeout = match timeout.checked_sub(weight) {
            Some(left) => left,
            None => {
                log::info!("{self} took too long to start...");
                return Ok(None)
            },
        };
        if let Some(signal) = signal {
            thread.call(signal)
        } else {
            thread.call((ScriptNotification0(ScriptNotification::Nop),))
        }
        .map(Some)
    }
    pub(super) fn signal_with(
        id: ScriptNotification,
        args: impl mlua::IntoLuaMulti + 'static,
    ) -> impl mlua::IntoLua {
        let get_args = taimi_pack::script::lua::IntoLuaFn::new(move |lua| {
            let args = args.into_lua_multi(lua)?;
            lua.create_function(move |_lua, _: DiscardValues| {
                //mlua::IntoLuaMulti::into_lua_multi(&args, lua)
                Ok(args.clone())
            })
            .map(mlua::Value::Function)
        });
        taimi_pack::script::lua::IntoLuaTable([
            (
                "id",
                Box::new(Some(id.to_repr())) as Box<dyn taimi_pack::script::lua::IntoLuaMut>,
            ),
            ("GetArgsPositional", Box::new(Some(get_args)) as Box<_>),
        ])
    }
    #[inline]
    pub fn name(&self) -> &Arc<str> {
        &self.shared().name
    }
    #[inline]
    pub fn menus(&self) -> &PlugMenusShared {
        &self.shared().menus
    }
    #[inline]
    pub fn state(&self) -> &PlugStateBeacon {
        &self.shared().state
    }
    #[inline]
    pub fn shared(&self) -> &PlugSharedData {
        AsRef::<PlugSharedData>::as_ref(&*self.shared)
    }
}
impl script::pathing::MenuInstance for LuaPlugBase {
    fn gen_id(
        &self,
        parent: Option<&id::FullIdRef>,
        name: Option<&id::IdNameSeg>,
    ) -> script::Result<String> {
        PlugMenuInstance::<&PlugSharedData, _>::new(self.shared(), Some(self)).gen_id(parent, name)
    }
    fn lookup_id(&self, id: &id::FullIdRef) -> script::Result<Option<Self::Menu>> {
        PlugMenuInstance::<Arc<dyn PlugSharedRef>, Self>::new(self.shared.clone(), Some(self.clone()))
            .lookup_id(id)
    }
    fn remove_id(&self, id: &id::FullIdRef, recursive: bool) -> script::Result<()> {
        PlugMenuInstance::<&PlugSharedData, _>::new(self.shared(), Some(self)).imp_remove_id(id, recursive)
    }
    fn register_id(&self, id: CategoryId) -> script::Result<Self::RegisteredMenu> {
        PlugMenuInstance::<Arc<dyn PlugSharedRef>, Self>::new(self.shared.clone(), Some(self.clone()))
            .register_id(id)
    }
    type Menu = <PlugMenuInstance<Arc<dyn PlugSharedRef>, Self> as script::pathing::MenuInstance>::Menu;
    type RegisteredMenu =
        <PlugMenuInstance<Arc<dyn PlugSharedRef>, Self> as script::pathing::MenuInstance>::RegisteredMenu;
}
impl script::pathing::MenuDesc for LuaPlugBase {
    fn get_id(&self) -> script::Result<CategoryId> {
        CategoryId::try_with_full_id(&self.name()[..])
            .ok_or_else(|| script::format_err!("{}", CategoryId::<id::IdNameBox>::WITH_FULL_ID_ERR))
    }
    fn get_menu_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        let v = self
            .get_id()
            .map(|root| {
                self.menus()
                    .menu_read(&root, |_, s| {
                        s.get_attr_dyn(id).map(|v| v.into_owned().into_inner())
                    })
                    .flatten()
            })
            .ok()
            .flatten();
        pack_attr! { match =id_is(id) {
            = keys::DisplayName => Ok(v
                .or_else(|| taimi_hoard::str_opt_ref(&self.name())
                .map(keys::DisplayName::from).map(PackValueCell::new_boxed))),
            = keys::NameId => v.map(Ok).unwrap_or_else(|| self.get_id().map(|id| keys::NameId::from(id.as_str())).map(PackValueCell::new_boxed)).map(Some),
            _ => Ok(v),
        } }
    }
}
impl script::pathing::MenuDesc for &'_ LuaPlugBase {
    #[inline]
    fn get_id(&self) -> script::Result<CategoryId> {
        script::pathing::MenuDesc::get_id(*self)
    }
    #[inline]
    fn get_menu_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        script::pathing::MenuDesc::get_menu_attr_dyn(*self, id)
    }
}
impl fmt::Display for LuaPlugBase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.name()[..], f)
    }
}
impl fmt::Debug for LuaPlugBase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LuaPlug")
            .field("name", &&self.name()[..])
            .field("co", &self.co)
            .field("_ENV", &self.globals)
            .field("state", &self.received)
            .finish()
    }
}
#[derive(Debug, Clone)]
pub struct LuaPlugDesc {
    pub base: LuaPlugBase,
    pub dir: Arc<Path>,
    /// think AUTOINCREMENT
    pub index: usize,
}
impl LuaPlugDesc {
    pub fn spun(
        &mut self,
        _lua: &RuntimeLua,
        yielded: mlua::Result<Option<NotifyScript<mlua::MultiValue>>>,
    ) -> anyhow::Result<()> {
        let yielded = match yielded {
            Ok(v) => v,
            Err(e) => {
                self.received = ScriptSignal::Resume;
                return Err(mlua::ErrorContext::with_context(e, |_| format!("polling {self}")).into())
            },
        };
        let (id, yielded) = match yielded {
            None => {
                log::warn!("{self} ended suddenly");
                self.received = ScriptSignal::Ended;
                return Ok(())
            },
            Some(ev) => {
                let id = ScriptSignal::from_repr(ev.id as _);
                let Some(id) = id else {
                    log::warn!("ignoring unsupported message {}", ev.id);
                    self.received = ScriptSignal::Resume;
                    return Ok(())
                };
                self.received = id;
                (id, ev)
            },
        };
        match id {
            ScriptSignal::Started | ScriptSignal::Pending | ScriptSignal::Resume => (),
            ScriptSignal::Ended => {
                log::info!("{self} quit");
                self.co = None;
                return Ok(())
            },
            #[cfg(todo)]
            id => {
                log::debug!("TODO: handle {id:?}: {yielded:?}");
            },
        }
        Ok(())
    }
    pub fn notify0(&mut self, lua: &RuntimeLua, id: ScriptNotification) -> anyhow::Result<()> {
        let co = self.running()?;
        let yielded = co.call((ScriptNotification0(id),));
        self.spun(lua, yielded)
    }
    pub fn exit(&mut self, lua: &RuntimeLua) -> anyhow::Result<()> {
        let mut signalled = false;
        while !matches!(self.received, ScriptSignal::Ended | ScriptSignal::Pending) {
            if !signalled {
                let Ok(..) = self.running() else { return Ok(()) };
                let () = self
                    .notify0(lua, ScriptNotification::Exit)
                    .context("requesting exit")?;
                signalled = true;
            } else {
                self.notify0(lua, ScriptNotification::Nop)?;
            }
        }
        Ok(())
    }
    pub fn notify_with(
        &mut self,
        lua: &RuntimeLua,
        id: ScriptNotification,
        args: impl mlua::IntoLuaMulti,
    ) -> anyhow::Result<()> {
        let co = self.running()?;
        let args = args.into_lua_multi(lua.lua())?;
        let yielded = co.call((LuaPlugBase::signal_with(id, args),));
        self.spun(lua, yielded)
    }

    pub fn get_loader(&self) -> script::Result<impl taimi_pack::PackLoaderContext + '_> {
        Ok(DirectoryLoader::new(&*self.dir))
    }
    pub fn shared_arc(&self) -> Arc<PlugSharedData> {
        unsafe {
            Arc::from_raw(
                Arc::into_raw(self.base.shared.clone()) as *const dyn core::any::Any
                    as *const PlugSharedData,
            )
        }
    }
    pub fn shared(&self) -> &PlugSharedData {
        unsafe { <dyn PlugSharedRef>::as_plug_unchecked(&*self.base.shared) }
    }
}
impl script::pathing::ScriptApiPack for LuaPlugDesc {
    fn current_pack_assets(&self) -> script::Result<Self::PackAssets<'_>> {
        Ok(self.clone())
    }
    type PackAssets<'a> = Self;

    fn current_pack_store(&self) -> script::Result<Self::PackStore<'_>> {
        let id = ScriptHostPersistence::id_for_pack("@plug", &self.name()[..]);
        crate::SETTINGS
            .get()
            .cloned()
            .context("settings missing")
            .map(|settings| ScriptHostPersistence::with_owner_id(id, settings))
    }
    type PackStore<'a> = ScriptHostPersistence;

    fn current_pack_menu(&self) -> script::Result<Self::PackMenu<'_>> {
        Ok(PlugMenuInstance::new(self.shared_arc(), Some(self.clone())))
    }
    type PackMenu<'a> = PlugMenuInstance<Arc<PlugSharedData>, Self>;

    fn current_pack(&self) -> script::Result<Self::Pack> {
        script::script_unimpl!()
    }
    type Pack = script::Unimplemented;

    fn current_pack_world(&self) -> script::Result<Self::PackWorld<'_>> {
        script::script_unimpl!()
    }
    type PackWorld<'a> = script::Unimplemented;

    fn current_pack_space(&self) -> script::Result<Self::PackSpace<'_>> {
        script::script_unimpl!()
    }
    type PackSpace<'a> = script::Unimplemented;
}
impl script::pathing::MenuDesc for LuaPlugDesc {
    fn get_id(&self) -> script::Result<CategoryId> {
        script::pathing::MenuDesc::get_id(&self.base)
    }
    fn get_menu_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        script::pathing::MenuDesc::get_menu_attr_dyn(&self.base, id)
    }
}
impl script::pathing::ScriptApiPackAssets for LuaPlugDesc {
    fn require_src<S>(&self, path: S) -> script::Result<Option<Self::RequireSrc>>
    where
        S: script::user::ScriptUserStr,
    {
        let mut loader = self.get_loader()?;
        path.with_str(|path| {
            let mut res = loader
                .load_asset_dyn(path)
                .with_context(|| format!("{path} not found in pack"));
            // the ".lua" extension is optional...
            let has_ext = || {
                Path::new(path)
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("lua"))
                    .unwrap_or(false)
            };
            match res {
                Err(..) if !has_ext() => {
                    let fallback = loader.load_asset_dyn(&format!("{path}.lua"));
                    if let Ok(fallback) = fallback {
                        res = Ok(fallback);
                    }
                },
                _ => (),
            }
            res
        })
        .map(Some)
    }
    type RequireSrc = Box<dyn LoaderAssetReader>;

    fn open_texture<P>(&self, path: P) -> script::Result<Self::Texture>
    where
        P: script::user::ScriptUserStr,
    {
        Err(script::format_err!("TODO: open_texture"))
    }
    type Texture = script::Unimplemented;
}

impl ops::Deref for LuaPlugDesc {
    type Target = LuaPlugBase;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl ops::DerefMut for LuaPlugDesc {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
impl fmt::Display for LuaPlugDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.base, f)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ScriptNotification0(pub ScriptNotification);
impl UserData for ScriptNotification0 {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("id", |_lua, this| Ok(this.0.to_repr()));
        reg.add_method("GetArgsPositional", |_lua, _this, DiscardValues| Ok(()));
    }
}
#[cfg(todo)]
#[derive(Debug, Copy, Clone)]
pub struct ScriptNotificationWith<A> {
    pub id: ScriptNotification,
    pub args: A,
}
#[cfg(todo)]
impl<A> ScriptNotificationWith<A> {
    #[inline(always)]
    pub const fn new(id: ScriptNotification, args: A) -> Self {
        Self { id, args }
    }
}
#[cfg(todo)]
impl<A> UserData for ScriptNotificationWith<A>
where
    for<'a> &'a A: IntoLuaMulti,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("id", |_lua, this| Ok(this.id.to_repr()));
        reg.add_method("GetArgsPositional", |lua, this, DiscardValues| {
            this.args.into_lua_multi(lua)
        });
    }
}

impl UserData for rt::bindings::GameControls {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_function("new_empty", |_lua, ()| Ok(Self::default()));
        reg.add_method("GetAt0", |_lua, this, (idx,): (rt::bindings::Control,)| {
            Ok(this.contains(idx))
        });
        reg.add_method("Bits32At0", |_lua, this, (idx,): (usize,)| {
            // 64bit array, so index into that...
            let i2 = idx / 2;
            Ok(this.bits.data.get(i2).map(|&v| match idx & 1 {
                0 => v as u32,
                _ => (v >> 32) as u32,
            }))
        });
        reg.add_method("IsEmpty", |_lua, this, (): ()| Ok(this.is_empty()));
        reg.add_method_mut("SetAt0", |_lua, this, (idx, v): (rt::bindings::Control, bool)| {
            this.set(idx, v);
            Ok(())
        });
        reg.add_method(
            "NextControlFrom",
            |_lua, this, (idx,): (Option<rt::bindings::Control>,)| {
                let idx = idx.map(|i| i.index as usize);
                let slice = match idx {
                    None => &this.bits[..],
                    Some(i) => this.bits.get(i..).unwrap_or_default(),
                };
                let off = idx.unwrap_or(0);
                Ok(slice
                    .first_one()
                    .map(|i| rt::bindings::Control::with_index((i + off) as u8)))
            },
        );
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(format!("{this:?}"))
        });
    }
}
impl UserData for rt::bindings::Control {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_function("from_index", |_lua, (c,): (i64,)| Ok(Self::try_from_int(c as _)));
        reg.add_field_method_get("Index", |_lua, this| Ok(this.index));
        reg.add_field_method_get("Label", |_lua, this| Ok(this.label_ident()));
        reg.add_meta_function(MetaMethod::Eq.name(), |_lua, (lhs, rhs): (Self, Self)| {
            Ok(lhs == rhs)
        });
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(this.label_ident())
        });
    }
}
impl FromLua for rt::bindings::Control {
    fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
        let try_from_int = |i: i64| {
            Self::try_from_int(i as _)
                .ok_or_else(|| to_lua_error(format_err!("control index out of range")))
        };
        match value {
            mlua::Value::UserData(ud) => ud.borrow::<Self>().map(|v| (*v).clone()),
            mlua::Value::Integer(i) => try_from_int(i),
            v => i64::from_lua(v, lua).and_then(try_from_int),
        }
    }
}

#[derive(RustEmbed)]
#[folder = "data/script/@taimi/"]
#[exclude = "core/*"]
#[prefix = "@taimi/"]
pub(crate) struct EmbeddedLuaTaimi;
impl EmbeddedLuaTaimi {
    fn preload_lib(lua: &RuntimeLua, name: &'static str) -> mlua::Result<()> {
        #[cfg(todo = "unnecessary")]
        let filename = name.strip_prefix("@taimi/");
        let modname = name
            .strip_suffix("/init.lua")
            .or(name.strip_suffix(".lua"))
            .unwrap_or(name);
        let src = Self::get(name).ok_or_else(|| lua::anyhow2lua(format_err!("missing {name}")))?;
        lua.preload_embedded(modname, src.data)
    }
}
