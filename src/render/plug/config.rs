#[cfg(feature = "scripts-lua")]
use {
    crate::controller::script::lua::{LuaExecContext, LuaMessage},
    anyhow::Context,
    taimi_pack::{
        attributes::keys,
        script::pathing::imp::{MarkerOverrides, MarkerType, PackOverrides},
    },
};
use {
    crate::{
        controller::{
            script::{PlugMenusShared, PlugSharedData, PlugsShared, ScriptMessage},
            Controller,
        },
        exports::runtime as rt,
        render::element::prelude::*,
    },
    core::mem,
    std::{borrow::Cow, fs, sync::Arc},
    taimi_sync::watched::Watched,
};

#[cfg(all(feature = "paths", feature = "scripts-lua"))]
use crate::space::pack::ActivePack;
#[cfg(feature = "paths")]
use crate::{controller::script::PackLoc, space::Engine};

#[derive(Debug, Clone, Default)]
pub struct PlugConfigDesc {}
#[derive(Debug, Clone, Default)]
pub struct PlugConfigState {
    pub applicable: bool,
    pub repl_input: String,
    pub expanded: bool,
    #[cfg(feature = "scripts-lua")]
    pub repl_context: LuaExecContext,
    pub plugs: Watched<PlugsShared>,
}
#[derive(Debug)]
pub struct PlugConfig<'a> {
    pub desc: &'a PlugConfigDesc,
    pub state: &'a mut PlugConfigState,
    pub scratch: &'a mut PlugConfigCache,
    #[cfg(feature = "paths")]
    pub engine: Option<&'a mut Engine>,
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
            let preview = <&str>::from(&*target);
            if let Some(_combo) = ui.begin_combo(c"context", preview) {
                if ui.selectable(c"global", matches!(target, LuaExecContext::Global)) {
                    *target = LuaExecContext::Global;
                }
                #[cfg(feature = "paths")]
                let packs = self
                    .engine
                    .as_ref()
                    .into_iter()
                    .flat_map(|e| {
                        e.packs
                            .loaded_packs
                            .iter()
                            .enumerate()
                            .map(|(i, (_, p))| (PackLoc::new(e.packs.generation, i), p))
                    })
                    .filter(|(_, p)| p.has_scripts());
                #[cfg(feature = "paths")]
                for (path, pack) in packs {
                    let _id = ui.push_id(Arc::as_ptr(&pack.pack));
                    if ui.selectable(
                        im_fmt!("{}", pack.pack.name),
                        *target == LuaExecContext::Pack(path),
                    ) {
                        *target = LuaExecContext::Pack(path);
                    }
                }
                let plugs = self.state.plugs.get_mut();
                for (&path, plug) in plugs.plugs.iter() {
                    let _id = ui.push_id(Arc::as_ptr(plug));
                    if ui.selectable(im_fmt!("{}", plug.name), *target == LuaExecContext::Plugin(path)) {
                        *target = LuaExecContext::Plugin(path);
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
                    context: self.state.repl_context,
                    interactive: true,
                }
                .try_send();
            }
        }
    }
    fn draw_settings<'ui, W, C>(&mut self, ui: &mut W, context: &mut C)
    where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        #[cfg(feature = "scripts-lua")]
        let map_id = self.engine.as_ref().and_then(|e| e.packs.current_map);
        #[cfg(feature = "paths")]
        let packs = self.engine.as_mut().into_iter().flat_map(|e| {
            e.packs
                .loaded_packs
                .iter_mut()
                .enumerate()
                .map(|(i, (_, p))| (PackLoc::new(e.packs.generation, i), p))
        });
        #[cfg(feature = "scripts-lua")]
        for (path, pack) in packs {
            if !pack.script_capable {
                continue
            }
            let target = LuaExecContext::Pack(path);
            let label = im_fmt!("{}", pack.pack.name);
            let node = ui.begin_tree_node_framed(
                ImCondition::startup(false),
                Arc::as_ptr(&pack.pack) as *const (),
                label,
                true,
            );
            let Some(_node) = node else { continue };
            if pack.has_scripts() {
                Self::draw_cats_lua(ui, context, target, pack);
                if ui.button(c"stop") {
                    LuaMessage::Stop { context: target }.try_send();
                    // TODO: message instead!
                    pack.script_data = None;
                }
                #[cfg(todo)]
                {
                    ui.same_line();
                    if ui.button(c"reset") {}
                }
                ui.same_line();
                if ui.button(c"reset markers") {
                    let active_pois = pack
                        .active_pois
                        .values()
                        .filter_map(|poi| (!poi.filtered).then_some(poi.poi_idx))
                        .collect::<Vec<_>>();
                    let active_trails = pack
                        .active_trails
                        .values()
                        .filter_map(|trail| (!trail.filtered).then_some(trail.trail_idx))
                        .collect::<Vec<_>>();
                    let active_pois = active_pois.into_iter().map(|i| (MarkerType::Poi, i));
                    let active_trails = active_trails.into_iter().map(|i| (MarkerType::Trail, i));
                    ScriptMessage::map_prepared_pack(
                        path.generation,
                        path.index,
                        map_id.unwrap_or(0) as u32,
                        active_pois.chain(active_trails),
                    )
                    .try_send();
                }
            } else {
                let src = ui
                    .button(c"pack.lua")
                    .then(|| {
                        pack.with_loader(|l| {
                            l.load_asset_dyn(crate::controller::script::pathing::PACK_ENTRYPOINT)
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
                    &pack.pack.name[..],
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
            if let Some(menus) = pack.script_data.as_ref().map(|d| &d.plug.menus) {
                Self::draw_menus_lua(ui, context, target, menus);
            }
            if let (Some(shared), true) = (pack.script_data.as_ref(), pack.has_scripts()) {
                let debug =
                    ui.begin_tree_node_framed(ImCondition::startup(false), c"watches", c"debug", true);
                if let Some(_node) = debug {
                    Self::draw_watch_lua(ui, context, target, &shared.plug);
                }
            }
        }
        let plugs = self.state.plugs.get_mut();
        for (&path, plug) in plugs.plugs.iter() {
            let target = LuaExecContext::Plugin(path);
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
                    LuaMessage::Stop { context: target }.try_send();
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
    fn draw_watch_lua<'ui, W, C>(
        ui: &mut W,
        _context: &mut C,
        target: LuaExecContext,
        shared: &PlugSharedData,
    ) where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        if ui.button(c"refresh watches") {
            LuaMessage::DebugWatchRefresh { context: target, interactive: false }.try_send();
        }
        ui.same_line();
        if ui.button(c"dump") {
            LuaMessage::DebugWatchRefresh { context: target, interactive: true }.try_send();
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
    fn draw_menus_lua<'ui, W, C>(
        ui: &mut W,
        _context: &mut C,
        target: LuaExecContext,
        menus: &PlugMenusShared,
    ) where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let menus = menus.shared.read().unwrap_or_else(|e| e.into_inner());
        if menus.is_empty() {
            return
        }
        let node = ui.begin_tree_node_framed(ImCondition::startup(false), c"menus", c"menus", true);
        let Some(_node) = node else { return };
        ui.indent();
        for (id, state) in menus.iter() {
            let _id = ui.push_id(id.as_str());
            if let Some(mut checked) = state.checked {
                ui.unindent();
                if ui.checkbox(c"", &mut checked) {
                    ScriptMessage::menu_clicked_with(id.clone(), target).try_send();
                }
                ui.indent();
                ui.same_line();
            }
            ui.text(format_args!("{id}"));
            ui.same_line();
            ui.text(&state.display_name[..]);
            if let Some(tt) = state.tooltip.as_ref() {
                if ui.item_is_hovered() {
                    ui.tooltip_text(&tt[..])
                }
            }
        }
        ui.unindent();
    }
    #[cfg(feature = "scripts-lua")]
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
    fn draw_plugs<'ui, W, C>(&mut self, ui: &mut W, _context: &mut C)
    where
        W: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let plugdir = rt::addon_dir().join("Plugins");
        ui.text(im_fmt!("{}", rt::relative_path(&plugdir).display()));
        ui.same_line();
        let dir = match fs::read_dir(&plugdir) {
            Ok(d) => d,
            Err(e) => {
                ui.text(im_fmt!("prob doesn't exist? {e:#}"));
                return
            },
        };
        crate::render::RenderState::draw_open_path_button(
            ui,
            fl!("open-button", kind = fl!("path")),
            &plugdir,
        );
        for entry in dir {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    ui.text(im_fmt!("{e:#}"));
                    continue
                },
            };
            let name = entry.file_name();
            let suffix = match entry.file_type() {
                Ok(t) if t.is_dir() => "/",
                #[cfg(windows)]
                Ok(t) if std::os::windows::fs::FileTypeExt::is_symlink_dir(&t) => "/",
                _ => "",
            };
            ui.text(im_fmt!("{}{suffix}", name.display()));
            ui.same_line();
            #[cfg(feature = "scripts-lua")]
            if ui.button(c"load plug") {
                let is_dir = !suffix.is_empty();
                let mut path = entry.path();
                if is_dir {
                    path.push("init.lua");
                }
                LuaMessage::SpawnPlug(path).try_send();
            }
        }
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
