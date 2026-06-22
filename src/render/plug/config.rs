#[cfg(feature = "scripts-lua")]
use {
    crate::controller::script::lua::LuaMessage,
    anyhow::Context,
    taimi_hoard::loc::LocationRef,
    taimi_meta::packs::MarkerPath,
    taimi_pack::{
        attributes::keys,
        script::pathing::imp::{MarkerOverrides, MarkerType, PackOverrides},
    },
};
use {
    crate::{
        controller::{
            script::{
                PlugMenusShared,
                PlugSharedData,
                PlugsShared,
                ScriptIndex,
                ScriptMessage,
                ScriptPath,
            },
            Controller,
        },
        exports::runtime as rt,
        render::element::prelude::*,
    },
    core::mem,
    std::{borrow::Cow, fs, path::PathBuf, sync::Arc},
    taimi_sync::watched::Watched,
};

#[cfg(feature = "paths-lua")]
use crate::controller::pathing::registry::PackPath;

#[derive(Debug, Clone, Default)]
pub struct PlugConfigDesc {}
#[derive(Debug, Clone)]
pub struct PlugConfigState {
    pub applicable: bool,
    pub repl_input: String,
    pub expanded: bool,
    pub repl_context: ScriptPath,
    pub plugs: Watched<PlugsShared>,
}
impl Default for PlugConfigState {
    fn default() -> Self {
        PlugConfigState {
            applicable: Default::default(),
            repl_input: Default::default(),
            expanded: Default::default(),
            repl_context: ScriptIndex::GLOBAL.to_path(),
            plugs: Default::default(),
        }
    }
}
#[derive(Debug)]
pub struct PlugConfig<'a> {
    pub desc: &'a PlugConfigDesc,
    pub state: &'a mut PlugConfigState,
    pub scratch: &'a mut PlugConfigCache,
    #[cfg(todo)]
    pub plugs: &'a mut PlugElements,
}
impl<'a> PlugConfig<'a> {
    fn draw_repl<'ui, W, C>(&mut self, ui: &mut W, _context: &mut C)
    where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let mut submit = false;
        {
            let _id = ui.push_id(c"replinput");
            let _wide = ui.item_prepare_push_width_dyn(-1.0);
            let addcap = match self.state.expanded {
                true => 512usize,
                false => 96usize,
                #[cfg(todo)]
                _ => 4096usize,
            };
            let flags = match (ui.imgui_version_num(), self.state.expanded) {
                #[cfg(taimi_imgui = "180")]
                (Some(im180::VERSION_NUM), false) => Some(
                    im180::sys::ImGuiInputTextFlags_EnterReturnsTrue
                        | im180::sys::ImGuiInputTextFlags_CtrlEnterForNewLine,
                ),
                #[cfg(taimi_imgui = "180")]
                (Some(im180::VERSION_NUM), true) => Some(im180::sys::ImGuiInputTextFlags_Multiline),
                #[cfg(taimi_imgui = "192")]
                (Some(im192::VERSION_NUM), false) => Some(
                    im192::sys::ImGuiInputTextFlags_EnterReturnsTrue
                        | im192::sys::ImGuiInputTextFlags_CtrlEnterForNewLine,
                ),
                #[cfg(taimi_imgui = "192")]
                (Some(im192::VERSION_NUM), true) => Some(im192::sys::ImGuiInputTextFlags_Multiline),
                _ => None,
            };
            let hint = (!self.state.expanded).then_some(c"repl");
            let changed = ui.input_text_managed(c"", &mut self.state.repl_input, addcap, hint, flags);
            if changed && !self.state.expanded {
                submit = true;
            }
        }

        if ui.button(c"exec") {
            submit = true;
        }
        ui.same_line();
        let expand_label = match self.state.expanded {
            true => c"^",
            false => c"expand",
        };
        if ui.button(expand_label) {
            self.state.expanded ^= true;
            if !self.state.expanded {
                self.state.repl_input.shrink_to_fit();
            }
        }

        ui.same_line();
        if ui.button(c"spinup") {
            LuaMessage::SpinUp.try_send();
        }
        ui.same_line();
        if ui.button(c"teardown") {
            ScriptMessage::TearDown.try_send();
        }

        #[cfg(feature = "scripts-lua")]
        {
            ui.same_line();
            let target = &mut self.state.repl_context;
            if let Some(_combo) = ui.begin_combo(c"context", im_fmt!("{target}")) {
                if ui.selectable(c"global", matches!(target.path, ScriptIndex::GLOBAL)) {
                    *target = ScriptIndex::GLOBAL.to_path();
                }
                let plugs = self.state.plugs.get_mut();
                #[cfg(feature = "paths")]
                let packs = plugs.packs.iter();
                #[cfg(feature = "paths")]
                for (&path, pack) in packs {
                    let _id = ui.push_id(Arc::as_ptr(pack) as *const ());
                    let path: ScriptPath = path.pivot_from();
                    if ui.selectable(im_fmt!("{}", pack.plug.name), *target == path) {
                        *target = path;
                    }
                }
                for (path, plug) in plugs.plugs.iter() {
                    let path: ScriptPath = path.pivot_from();
                    let _id = ui.push_id(Arc::as_ptr(plug) as *const ());
                    if ui.selectable(im_fmt!("{}", plug.name), *target == path) {
                        *target = path;
                    }
                }
            }
        }

        if submit {
            let input = mem::take(&mut self.state.repl_input);
            #[cfg(feature = "scripts-lua")]
            {
                LuaMessage::Exec {
                    source: Cow::Owned(input.into()),
                    context: self.state.repl_context.path,
                    interactive: true,
                }
                .try_send();
            }
        }

        let clear_settings = ui
            .button(c"clear all stored settings")
            .then(|| crate::SETTINGS.get().map(|s| s.blocking_write()))
            .flatten();
        if let Some(mut settings) = clear_settings {
            settings.data_storage.source_kv.clear();
            settings.mark_dirty();
        }
    }
    fn draw_settings<'ui, W, C>(&mut self, ui: &mut W, context: &mut C)
    where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let plugs = self.state.plugs.get_mut();
        #[cfg(feature = "scripts-lua")]
        #[cfg(todo)]
        let map_id = self.engine.as_ref().and_then(|e| e.packs.current_map);
        #[cfg(feature = "paths")]
        let packs = plugs.packs.iter();
        #[cfg(feature = "scripts-lua")]
        for (&path, pack) in packs {
            #[cfg(deleteme)]
            if !pack.script_capable {
                continue
            }
            let _id = ui.push_id(Arc::as_ptr(pack) as *const ());
            let target: ScriptPath = path.pivot_from();
            let label = im_fmt!("{}", pack.plug.name);
            let node = ui.begin_tree_node_framed(
                ImCondition::startup(false),
                Arc::as_ptr(pack) as *const (),
                label,
                true,
            );
            let Some(_node) = node else { continue };
            #[cfg(todo)]
            Self::draw_cats_lua(ui, context, target, pack);

            if ui.button(c"stop") {
                LuaMessage::Stop { context: target.path }.try_send();
                #[cfg(deleteme)]
                {
                    // TODO: message instead!
                    pack.script_data = None;
                }
            }
            #[cfg(todo)]
            {
                ui.same_line();
                if ui.button(c"reset") {}
            }
            #[cfg(deleteme)]
            {
                ui.same_line();
                if ui.button(c"dbgmarkerrefresh") {
                    LuaMessage::DebugFlushMarkerChanges(path).try_send();
                }
            }
            ui.same_line();
            if ui.button(c"initial map markers") {
                Controller::with_sender(|s| {
                    s.pathing.as_ref().and_then(|p| {
                        let map = p.shared.gameplay.borrow();
                        let map_id = map.map_id;
                        let Some(map_info) =
                            map_id.and_then(|map_id| map.get_info(path.pivot_from().rel(map_id)))
                        else {
                            return None
                        };
                        let pois = map_info.info.pois().map(|poi_path| poi_path.pivot_from());
                        let trails = map_info.info.trails().map(|trail_path| trail_path.pivot_from());
                        let markers = pois.chain(trails).collect::<Vec<MarkerPath>>();
                        ScriptMessage::map_prepared_pack(
                            path.pivot_from(),
                            map_id.map(|id| id.get()).unwrap_or(0) as u32,
                            markers,
                        )
                        .try_send();
                        Some(())
                    })
                });
            }
            #[cfg(todo)]
            if pack.has_scripts() {
                Self::draw_cats_lua(ui, context, target, pack);
                ui.same_line();
            } else {
                let src = ui
                    .button(c"pack.lua")
                    .then(|| {
                        pack.get_loader()
                            .and_then(|l| {
                                l.blocking_lock()
                                    .load_asset_dyn(crate::controller::script::pathing::PACK_ENTRYPOINT)
                            })
                            .context("pack.lua")
                    })
                    .transpose();
                if let Some(Some(src)) = rt::log::error_ok(src) {
                    LuaMessage::SpawnPack(
                        pack.pack.clone(),
                        pack.loader.clone(),
                        src,
                        path.generation,
                        path.index,
                    )
                    .try_send();
                }
            }
            ui.same_line();
            let clear_settings = ui
                .button(c"clear stored settings")
                .then(|| crate::SETTINGS.get().map(|s| s.blocking_write()))
                .flatten();
            if let Some(mut settings) = clear_settings {
                #[cfg(todo)]
                let prefix_ns = {
                    use taimi_pack::script::pathing::imp::PackRootCategories;
                    let root = PackRootCategories::from_ref(&pack.pack);
                    root.primary_root()
                        .or(root.iter_root_categories().next())
                        .map(|cat| &cat.full_id)
                };
                let id = crate::controller::script::persistence::ScriptHostPersistence::id_prefix_for_pack(
                    &pack.plug.name[..],
                )
                .to_string();
                let mut changed = false;
                settings.data_storage.source_kv.retain(|k, _| {
                    let keep = !k.starts_with(&id[..]);
                    changed |= !keep;
                    keep
                });
                if changed {
                    settings.mark_dirty();
                }
            }
            Self::draw_menus_lua(ui, context, target, &pack.plug.menus);
            let debug = ui.begin_tree_node_framed(ImCondition::startup(false), c"watches", c"debug", true);
            if let Some(_node) = debug {
                Self::draw_watch_lua(ui, context, target, &pack.plug);
            }
        }
        for (&path, plug) in plugs.plugs.iter() {
            let _id = ui.push_id(Arc::as_ptr(plug) as *const ());
            let target = path.pivot_from();
            let label = im_fmt!("{}", &plug.name[..]);
            let node = ui.begin_tree_node_framed(
                ImCondition::startup(false),
                Arc::as_ptr(&plug) as *const (),
                label,
                true,
            );
            let Some(_node) = node else { continue };
            Self::draw_menus_lua(ui, context, target, &plug.menus);
            if plug.is_active() {
                if ui.button(c"stop") {
                    LuaMessage::Stop { context: target.path }.try_send();
                }
                #[cfg(todo)]
                {
                    ui.same_line();
                    if ui.button(c"reset") {}
                }
            } else {
                if ui.button(c"pack.lua") {
                    log::warn!("TODO: reload");
                }
            }
            ui.same_line();
            let clear_settings = ui
                .button(c"clear stored settings")
                .then(|| crate::SETTINGS.get().map(|s| s.blocking_write()))
                .flatten();
            if let Some(mut settings) = clear_settings {
                #[cfg(todo)]
                let prefix_ns = {
                    use taimi_pack::script::pathing::imp::PackRootCategories;
                    let root = PackRootCategories::from_ref(&pack.pack);
                    root.primary_root()
                        .or(root.iter_root_categories().next())
                        .map(|cat| &cat.full_id)
                };
                let id = crate::controller::script::persistence::ScriptHostPersistence::id_for_pack_fmt(
                    "@plug",
                    &plug.name[..],
                )
                .to_string();
                let mut changed = false;
                settings.data_storage.source_kv.retain(|k, _| {
                    let keep = !k.starts_with(&id[..]);
                    changed |= !keep;
                    keep
                });
                if changed {
                    settings.mark_dirty();
                }
            }
            #[cfg(feature = "scripts-lua")]
            let debug = plug.is_active().then(|| {
                ui.begin_tree_node_framed(ImCondition::startup(false), c"watches", c"debug", true)
            });
            #[cfg(feature = "scripts-lua")]
            if let Some(_node) = debug {
                Self::draw_watch_lua(ui, context, target, plug);
            }
        }
    }
    /// TODO: move to a general non-lua variant (target arg)
    #[cfg(feature = "scripts-lua")]
    fn draw_watch_lua<'ui, W, C>(ui: &mut W, _context: &mut C, target: ScriptPath, shared: &PlugSharedData)
    where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        if ui.button(c"refresh watches") {
            LuaMessage::DebugWatchRefresh {
                context: target.path,
                interactive: false,
            }
            .try_send();
        }
        ui.same_line();
        if ui.button(c"dump") {
            LuaMessage::DebugWatchRefresh {
                context: target.path,
                interactive: true,
            }
            .try_send();
        } else if ui.item_is_hovered() {
            ui.tooltip_text("click and check logs");
        }

        let (plug_state, _gen) = shared.state.read();
        let watches = plug_state.debug_watches.clone();
        drop(plug_state);
        for (k, v) in watches.iter() {
            let linecount_approx = v.as_bytes().iter().filter(|&&b| b == b'\n').count();
            let collapsible = linecount_approx > 8;
            let node;
            match collapsible {
                true => {
                    node = ui.begin_tree_node_framed(ImCondition::appear(false), &k[..], &k[..], true);
                    if node.is_none() {
                        continue
                    }
                },
                _ => {
                    #[cfg(todo = "unnecessary")]
                    let _id = ui.push_id(&k[..]);
                    ui.text_with_font(NexusLinkFont::Big, im_fmt!("{}: ", &k[..]));
                    ui.indent();
                    if linecount_approx == 0 || (v.starts_with("{\n") | v.starts_with("[\n")) {
                        ui.same_line();
                    }
                },
            }
            ui.text(im_fmt!("{v}"));
            if !collapsible {
                ui.unindent();
            }
        }
    }
    fn draw_menus_lua<'ui, W, C>(ui: &mut W, _context: &mut C, target: ScriptPath, menus: &PlugMenusShared)
    where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let menus = menus.shared.read().unwrap_or_else(|e| e.into_inner());
        if menus.is_empty() {
            return
        }
        let node = ui.begin_tree_node_framed(ImCondition::startup(false), c"menus", c"menus", true);
        let Some(_node) = node else { return };
        //ui.indent();
        for (id, state) in menus.iter() {
            let _id = ui.push_id(id.as_str());
            ui.unindent();
            if let Some(mut checked) = state.checked {
                if ui.checkbox(c"", &mut checked) {
                    ScriptMessage::menu_clicked_with(id.clone(), target).try_send();
                }
            } else if ui.button(c"O") {
                ScriptMessage::menu_clicked_with(id.clone(), target).try_send();
            }
            ui.indent();
            ui.same_line();
            ui.text(format_args!("{id}"));
            ui.text(&state.display_name[..]);
            if let Some(tt) = state.tooltip.as_ref() {
                if ui.item_is_hovered() {
                    ui.tooltip_text(&tt[..])
                }
            }
        }
        //ui.unindent();
    }
    #[cfg(feature = "scripts-lua")]
    #[cfg(todo)]
    fn draw_cats_lua<'ui, W, C>(
        ui: &mut W,
        _context: &mut C,
        _target: LuaExecContext,
        pack: &mut ActivePack,
    ) where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let Some(shared) = &pack.script_data else { return };
        let po = PackOverrides::shared_read(&shared.overrides);
        if po.cat_overrides.is_empty() {
            return
        }
        let node = ui.begin_tree_node_framed(ImCondition::startup(false), c"cats", c"cats", true);
        let Some(_node) = node else { return };
        ui.indent();
        for (id, &idx) in po.cat_overrides.iter() {
            let path = (MarkerType::Category, idx);
            let Some(attrs) = po.overrides.get(&path) else { continue };
            let attrs = MarkerOverrides::shared_try_read(attrs);
            let _id = ui.push_id(id.as_str());

            let is_sep = attrs
                .as_ref()
                .and_then(|a| {
                    a.get::<keys::IsSeparator>()
                        .and_then(|v| v.map(|v| bool::from(*v.get())))
                })
                .unwrap_or(false);
            if let (false, Some(mut state)) = (is_sep, pack.user_category_state.get_mut(idx)) {
                ui.unindent();
                ui.checkbox(c"", &mut *state);
                ui.indent();
                ui.same_line();
            }
            ui.text(format_args!("{id}"));
            let Some(attrs) = attrs else { continue };
            if let Some(Some(name)) = attrs.get::<keys::DisplayName>() {
                ui.same_line();
                ui.text(&name[..]);
            }
            let tip = attrs.get::<keys::TipName>().flatten();
            let tip_body = attrs.get::<keys::TipDescription>().flatten();
            if (tip.is_some() | tip_body.is_some()) && ui.item_is_hovered() {
                let sep = (tip.is_some() & tip_body.is_some()).then_some("\n").unwrap_or("");
                let tip = lazyfmt::or_empty(tip);
                let body = lazyfmt::or_empty(tip_body);
                ui.tooltip_text(format_args!("{tip}{sep}{body}"));
            }
        }
        ui.unindent();
    }
    /// TODO: hack, move into script controller!
    fn refresh_plugs(&mut self) -> anyhow::Result<impl Iterator<Item = PathBuf>> {
        let plugdir = rt::addon_dir().join("plugins");
        let dir = match fs::read_dir(&plugdir) {
            Ok(d) => d,
            Err(e) => {
                return Err(anyhow::anyhow!("{} prob doesn't exist? {e:#}", plugdir.display()));
            },
        };
        Ok(dir.into_iter().filter_map(|entry| {
            let Some(entry) = rt::log::warn_ok(entry) else { return None };
            let suffix = match entry.file_type() {
                Ok(t) if t.is_dir() => "/",
                #[cfg(windows)]
                Ok(t) if std::os::windows::fs::FileTypeExt::is_symlink_dir(&t) => "/",
                _ => "",
            };
            let is_dir = !suffix.is_empty();
            let mut path = entry.path();
            if is_dir {
                path.push("init.lua");
            }
            Some(path)
        }))
    }
    #[cfg(feature = "paths-lua")]
    fn refresh_packs(&mut self) -> anyhow::Result<impl Iterator<Item = PackPath>> {
        let shared = Controller::with_sender(|s| s.pathing.as_ref().map(|p| p.shared.clone()))
            .flatten()
            .context("pathing offline")?;
        let packs = shared
            .packs
            .packs
            .borrow()
            .iter()
            .filter_map(|(path, pack)| pack.loaded.borrow().loader.clone().map(|l| (path, l)))
            .collect::<Vec<_>>();
        Ok(packs.into_iter().filter_map(|(path, l)| {
            let has = l
                .blocking_lock()
                .contains_asset(crate::controller::script::pathing::PACK_ENTRYPOINT)
                .context("detecting pack.lua");
            rt::log::warn_ok(has).unwrap_or(false).then_some(path)
        }))
    }
    fn draw_plugs<'ui, W, C>(&mut self, ui: &mut W, _context: &mut C)
    where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let plugdir = rt::addon_dir().join("plugins");
        ui.text(im_fmt!("{}", rt::relative_path(&plugdir).display()));
        ui.same_line();
        crate::render::RenderState::draw_open_path_button(
            ui,
            fl!("open-button", kind = fl!("path")),
            &plugdir,
        );
        ui.same_line();
        if ui.button(c"refresh") {
            let found_plugs = rt::log::error_ok(self.refresh_plugs());
            if let Some(found) = found_plugs {
                self.state.plugs.write_with(|shared| {
                    shared.available_plugs = found.map(|p| p.into()).collect();
                });
            }
            #[cfg(todo)]
            let found_packs = rt::log::error_ok(self.refresh_packs());
            #[cfg(todo)]
            if let Some(found) = found_packs {
                self.state.plugs.write_with(|shared| {
                    shared.available_packs = found.map(|p| p.into()).collect();
                });
            }
            ScriptMessage::RefreshPacks.try_send();
        }
        let plugs = self.state.plugs.get_mut();
        for path in &plugs.available_plugs {
            let _id = ui.push_id(Arc::as_ptr(path) as *const ());
            #[cfg(todo)]
            let (name, suffix) = match path.file_name() {
                #[cfg(feature = "scripts-lua")]
                Some(name) if name.eq_ascii("init.lua") => (entry.parent().unwrap_or(path), "/"),
                _ => (path, ""),
            };
            let (name, suffix) = (rt::relative_path(path), "");
            ui.text(im_fmt!("{}{suffix}", name.display()));
            ui.same_line();
            if ui.button(c"load plug") {
                LuaMessage::SpawnPlug((&**path).into()).try_send();
            }
        }
        #[cfg(feature = "paths-lua")]
        Controller::with_sender(|s| {
            let packs = s.pathing.as_ref().map(|p| &p.shared.packs);
            let Some(packs) = packs else { return };
            let sharedpacks = packs.packs.borrow();
            for &path in &plugs.available_packs {
                let _id = ui.push_id(path.path as usize);
                ui.text(im_fmt!("#{}", path.path));
                let Some(pack) = sharedpacks.lookup_ref(&path) else { continue };
                ui.same_line();
                ui.text(im_fmt!("{}", pack.info));
                #[cfg(todo = "unnecessary")]
                {
                    // this is probably a bad idea, enjoy
                    let loaded = pack.loaded.borrow();
                    if let Some(unloaded) = &loaded.unloaded {
                        ui.text(im_fmt!("{unloaded}"));
                        continue
                    } else if loaded.loader.is_none() {
                        ui.text_disabled(c"unloaded");
                        continue
                    }
                }
                ui.indent();
                if ui.button(c"pack.lua") {
                    let loader = {
                        let loaded = pack.loaded.borrow();
                        loaded
                            .loader
                            .clone()
                            .and_then(|l| loaded.pack.as_ref().map(|_| l))
                    };
                    let asset = loader.context("unloaded").and_then(|l| {
                        let asset = l
                            .blocking_lock()
                            .load_asset_dyn(crate::controller::script::pathing::PACK_ENTRYPOINT);
                        asset.context("pack.lua")
                    });
                    if let Some(asset) = rt::log::error_ok(asset) {
                        LuaMessage::SpawnPack(path, asset).try_send();
                    }
                }
                ui.unindent();
            }
        });
    }
}
impl<'a, 'ui, W, C> Drawable<W, C> for PlugConfig<'a>
where
    W: ?Sized + ImDrawWindow<'ui>,
    C: ?Sized + DrawContext<'ui>,
{
    fn draw_on_window(&mut self, ui: &mut W, context: &mut C) {
        #[cfg(not(feature = "scripts-lua"))]
        {
            ui.text("TODO: lua excluded from dll");
        }
        if !self.state.plugs.is_watching() {
            let subscribed = Controller::with_sender(|s| {
                s.scripting
                    .as_ref()
                    .map(|s| self.state.plugs.restart_watching(&s.plugs_shared))
            })
            .flatten()
            .is_some();
            if !subscribed {
                return
            }
        }
        if self.state.plugs.try_read_mut().is_none() {
            return
        }

        if let Some(_node) =
            ui.begin_tree_node_framed(ImCondition::initial(false), c"repl", c"debug repl", true)
        {
            self.draw_repl(ui, context);
        }
        // else if just_closed && self.expanded { repl_input.shrink_to_fit() }
        if let Some(_node) = ui.begin_tree_node_framed(
            ImCondition::initial(false),
            c"settings",
            c"settings/menus/cats",
            true,
        ) {
            self.draw_settings(ui, context);
        }

        if let Some(_node) =
            ui.begin_tree_node_framed(ImCondition::initial(false), c"plugs", c"plugs", true)
        {
            self.draw_plugs(ui, context);
        }

        ui.spacing();
        ui.separator();
        ui.spacing();
        ui.text_wrapped(fl!("experimental-notice-alpha"));
    }
    /// TODO: sub-widget so this applies when treenode collapsed too!
    fn draw_stop(&mut self, _context: &C) {
        self.state.repl_input.shrink_to_fit();
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlugConfigCache {
    #[cfg(todo)]
    pub scrollback: Vec<String0>,
}
