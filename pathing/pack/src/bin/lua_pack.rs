// incomplete WIP, no point in cleaning it up yet
#![cfg_attr(not(taimi_debug = "wip"), allow(nonstandard_style, unused, unexpected_cfgs))]

use {
    anyhow::Context,
    std::{
        collections::HashMap,
        env,
        ffi::OsStr,
        fs,
        io,
        path::Path,
        sync::{Arc, Mutex},
    },
    taimi_pack::{
        attributes::{
            cell::{GetAttrDyn, SetAttrDyn},
            keys::{self, GetAttr},
        },
        category::id::AsFullId,
        loader,
        script::{self, pathing::imp::PackArc, user::ScriptUserStr, RuntimeLua},
        Pack,
    },
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    log::debug!("lua runtime init...");
    let events = EventApi::default();
    let lua = RuntimeLua::new_script_runtime(Default::default(), None)?;
    lua.setup_package_builtin()?;
    lua.setup_api_rt()?;
    // TODO: make bitop a builtin
    lua.setup_api_version(script::Unimplemented)?;
    preload_lib(&lua, "@taimi/util/init.lua")?;
    preload_lib(&lua, "@taimi/util/ud.lua")?;
    preload_lib(&lua, "@taimi/todo/lson.lua")?;
    lua.setup_api_log(io::stderr())?;
    preload_lib(&lua, "@taimi/debug.lua")?;
    preload_lib(&lua, "@taimi/bitop.lua")?;
    preload_lib(&lua, "@taimi/id.lua")?;
    lua.setup_api_vectors()?;
    // isn't it convenient that there are also test stubs in the repo?
    //lua.setup_api_mumble(script::Unimplemented)?;
    preload_lib(&lua, "@taimi/core/mumblelink.lua")?;
    lua.setup_api_event(events.clone())?;
    //preload_lib(&lua, "@taimi/core/event.lua")?;
    preload_lib(&lua, "@taimi/event.lua")?;
    //lua.setup_api_bindings::<script::Unimplemented, script::Unimplemented, _>()?;
    preload_lib(&lua, "@taimi/core/bindings.lua")?;
    preload_lib(&lua, "@taimi/bindings.lua")?;
    //lua.setup_api_ui_exchange(script::Unimplemented)?;
    preload_lib(&lua, "@taimi/core/ui/exchange.lua")?;
    preload_lib(&lua, "@taimi/ui/exchange.lua")?;
    lua.setup_api_attrs()?;
    preload_lib(&lua, "@taimi/pack/attrs.lua")?;
    // TODO: lua.setup_api_interact();
    preload_lib(&lua, "@taimi/todo/interact.lua")?;
    preload_lib(&lua, "@taimi/pack/interact.lua")?;
    lua.setup_api_ui_menu::<PackApi, PackMenu>()?;
    preload_lib(&lua, "@taimi/ui/menu.lua")?;
    preload_lib(&lua, "@taimi/compat/category.lua")?;
    preload_lib(&lua, "@taimi/compat/trail.lua")?;
    preload_lib(&lua, "@taimi/compat/menu.lua")?;
    preload_lib(&lua, "@taimi/compat/poi.lua")?;
    preload_lib(&lua, "@taimi/compat/env.lua")?;
    preload_lib(&lua, "@taimi/compat/init.lua")?;
    preload_lib(&lua, "@taimi/v0/event/init.lua")?;
    preload_lib(&lua, "@taimi/v0/mumblelink.lua")?;
    preload_lib(&lua, "@taimi/v0/plug/log.lua")?;
    preload_lib(&lua, "@taimi/v0/plug/persist.lua")?;
    preload_lib(&lua, "@taimi/v0/plug/loader.lua")?;
    preload_lib(&lua, "@taimi/v0/plug/init.lua")?;
    preload_lib(&lua, "@taimi/v0/menu/init.lua")?;
    preload_lib(&lua, "@taimi/core/nexus/init.lua")?;
    #[cfg(todo)]
    {
        preload_lib(&lua, "@taimi/v0/nexus/datalink/init.lua")?;
        preload_lib(&lua, "@taimi/v0/nexus/datalink/rtapi.lua")?;
        preload_lib(&lua, "@taimi/v0/nexus/event/init.lua")?;
        preload_lib(&lua, "@taimi/v0/nexus/input/init.lua")?;
        preload_lib(&lua, "@taimi/v0/nexus/paths.lua")?;
        preload_lib(&lua, "@taimi/v0/nexus/quickaccess.lua")?;
        preload_lib(&lua, "@taimi/v0/nexus/texture.lua")?;
        preload_lib(&lua, "@taimi/v0/nexus/init.lua")?;
    }
    preload_lib(&lua, "@taimi/v0/init.lua")?;
    preload_lib(&lua, "@taimi/main/pack.lua")?;
    preload_lib(&lua, "@taimi/main/plug.lua")?;

    let fname = env::args_os().nth(1).expect("marker path to parse");
    let fname = Path::new(&fname);

    let meta = fs::metadata(fname);
    let mut loader =
        if fname.extension().map(|ext| ext.eq_ignore_ascii_case("taco")) == Some(true) || !meta?.is_dir() {
            let loader_zip = Box::new(loader::ZipLoader::new(fname)?);
            Box::leak(loader_zip) as &mut dyn loader::PackLoaderContext
        } else {
            let loader_dir = Box::new(loader::DirectoryLoader::new(fname));
            Box::leak(loader_dir) as &mut dyn loader::PackLoaderContext
        };
    let mut entrypoint = loader.load_asset_dyn("pack.lua").context("not a lua pack?")?;

    let relaxed = env::var_os("TAIMI_RELAXED")
        .as_ref()
        .map(|v| v.as_os_str() == OsStr::new("1"));
    let strict = !relaxed.unwrap_or(false);
    let pack = Arc::new({
        let pack = Pack::load_strict(&mut loader, strict)?;
        #[cfg(todo)]
        {
            pack.categories.trim_attributes();
        }
        pack
    });

    log::debug!("initial script...");
    /*let globals = lua.lua().globals();
    let api_constructors = ();
    let api_debug = std::io::stderr();
    lua.set_api_globals_core(&globals, api_constructors, api_debug)?;
    let api_pathing = script::Unimplemented;
    let api_mumble = script::Unimplemented;
    lua.set_api_globals_pathing(&globals, api_pathing, api_mumble)?;
    let api_world = script::pathing::imp::PackArc::from_ref(&pack);
    let api_pack = (api_world.clone(), Arc::new(Mutex::new(loader)));
    lua.set_api_globals_pack(&globals, api_pack, api_world.clone())?;*/

    let mut pack_lua = Vec::new();
    entrypoint
        .read_to_end(&mut pack_lua)
        .context("reading pack.lua")?;
    let common_globals = lua.pack_globals_shared();
    let pack_globals = {
        let mt = lua.lua().create_table()?;
        mt.set(mlua::MetaMethod::Index.name(), common_globals)?;
        // TODO: mt.set(mlua::MetaMethod::NewIndex.name(), pack_globals_mut);
        let genv = lua.lua().create_table()?;
        genv.set_metatable(Some(mt))?;
        genv
    };
    let chunk = lua
        .lua()
        .load(&pack_lua)
        .set_name("@pack.lua")
        .set_mode(mlua::ChunkMode::Text) // unless user is okay with arbitrary code execution
        .set_environment(pack_globals.clone())
        .into_function()?;
    let pack_info = PackApi {
        pack: pack.clone(),
        loader: Arc::new(Mutex::new(loader)),
        store: Arc::new(Mutex::new(Default::default())),
        overrides: Default::default(),
    };

    let runner = runner_for_pack(&lua).context("preparing pack.lua loader")?;
    let co = runner.call::<taimi_pack::script::lua::LuaCallable>((
        RuntimeLua::new_api_pack_info(pack_info.clone(), pack_globals.clone()),
        chunk,
        &pack_globals,
    ))?;

    loop {
        let signal_nothing = script::lua::IntoLuaTable([("id", 0u32)]);
        let res = co.call::<u32>(signal_nothing).context("starting event loop")?;
        match res {
            100u32 => {
                log::info!("yield(Started)");
                break
            },
            101u32 => {
                log::info!("yield(Ended)");
                break
            },
            102u32 => {
                log::info!("yield(Pending)");
                break
            },
            103u32 => {
                log::info!("yield(Resume)");
            },
            id => {
                log::info!("yield({id})?");
            },
        }
    }
    // now simulate a tick...
    let signal_tick = script::lua::IntoLuaTable([
        ("id", mlua::Value::Integer(2)),
        (
            "GetArgsPositional",
            mlua::Value::Function(
                lua.lua()
                    .create_function(|_, _: script::lua::DiscardValues| Ok(mlua::Nil))?,
            ),
        ),
    ]);
    let mut response = 103u32;
    while response == 103 {
        response = co
            .call::<u32>(signal_tick.clone())
            .context("continuing event loop")?;
        log::info!("yield({response})");
    }

    for (pos, attrs) in pack
        .pois
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                (taimi_pack::script::pathing::imp::MarkerType::Poi, i),
                &p.attributes,
            )
        })
        .chain(pack.trails.iter().enumerate().map(|(i, p)| {
            (
                (taimi_pack::script::pathing::imp::MarkerType::Trail, i),
                &p.attributes,
            )
        }))
    {
        use taimi_pack::script::{pathing::PathableHandle, user::IntoUserHandle};
        if let Some(s) = GetAttr::<keys::ScriptTick>::get_attr(attrs) {
            //log::warn!("TODO: scipt-tick");
        }
        if let Some(s) = GetAttr::<keys::ScriptFocus>::get_attr(attrs) {
            //log::warn!("TODO: scipt-focus");
        }
        if let Some(s) = GetAttr::<keys::ScriptTrigger>::get_attr(attrs) {
            //log::warn!("TODO: scipt-trigger");

            log::info!("trying it? {s:?}");
            let (fname, lazyargs) = lua.prepare_script_attr_args(
                &format!("{pos:?}/focus = {s:?}"),
                s.as_bytes(),
                pack_globals.clone(),
            )?;
            if fname == "Teh_Bounce" {
                continue
            }
        }
        if let Some(s) = GetAttr::<keys::ScriptFilter>::get_attr(attrs) {
            log::warn!("TODO: scipt-filter");
        }
        if let Some(s) = GetAttr::<keys::ScriptOnce>::get_attr(attrs) {
            log::warn!("TODO: scipt-once");

            log::info!("trying it? {s:?}");
            let (fname, lazyargs) = lua.prepare_script_attr_args(
                &format!("{pos:?}/focus = {s:?}"),
                s.as_bytes(),
                pack_globals.clone(),
            )?;
            let f = pack_globals.get::<mlua::Function>(fname)?;
            let eventloop = pack_globals
                .get::<mlua::Table>("Taimi")?
                .get::<mlua::Table>("ctx")?
                .get::<mlua::Table>("events")?;
            let id_trigger = 35u32;
            let is_auto = false;
            let markerkey = match pos.0 {
                taimi_pack::script::pathing::imp::MarkerType::Poi => unsafe {
                    PackPoi::new(
                        taimi_pack::pack::PackPoiArc::new_unchecked(pack.clone(), pos.1),
                        pack_info.overrides.clone(),
                    )
                    .pathable_tag_index()
                },
                taimi_pack::script::pathing::imp::MarkerType::Trail => unsafe {
                    PackTrail::new(
                        taimi_pack::pack::PackTrailArc::new_unchecked(pack.clone(), pos.1),
                        pack_info.overrides.clone(),
                    )
                    .pathable_tag_index()
                },
                _ => unreachable!(),
            };
            let callback = lua
                .lua()
                .create_function(move |lua, a: mlua::MultiValue| f.call::<()>(a))?;
            mlua::ObjectLike::call_method::<()>(
                &eventloop,
                "RegisterMarkerAttr",
                (id_trigger, markerkey, callback, lazyargs),
            )?;
            // and now simulate...
            log::info!("and now sim...");
            let marker = match pos.0 {
                taimi_pack::script::pathing::imp::MarkerType::Poi => unsafe {
                    PackPoi::new(
                        taimi_pack::pack::PackPoiArc::new_unchecked(pack.clone(), pos.1),
                        pack_info.overrides.clone(),
                    )
                    .to_lua_handle(lua.lua())
                }?,
                taimi_pack::script::pathing::imp::MarkerType::Trail => unsafe {
                    PackTrail::new(
                        taimi_pack::pack::PackTrailArc::new_unchecked(pack.clone(), pos.1),
                        pack_info.overrides.clone(),
                    )
                    .to_lua_handle(lua.lua())
                }?,
                _ => unreachable!(),
            };
            let signal_marker = script::lua::IntoLuaTable([
                ("id", mlua::Value::Integer(id_trigger as _)),
                (
                    "GetArgsPositional",
                    mlua::Value::Function(
                        lua.lua()
                            .create_function(move |lua, ()| Ok((marker.clone(), is_auto)))?,
                    ),
                ),
            ]);
            let mut response = 103u32;
            while response == 103 {
                response = co
                    .call::<u32>(signal_marker.clone())
                    .context("signalling marker")?;
                log::info!("yield({response})");
            }
        }
    }
    for poi in pack.pois.iter() {
        #[cfg(todo)]
        if poi.script {}
    }
    for trail in pack.trails.iter() {
        #[cfg(todo)]
        if trail.script {}
    }

    Ok(())
}

fn preload_lib(lua: &RuntimeLua, name: &'static str) -> anyhow::Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/script")
        .join(name);
    let context = || format!("{}", path.display());
    let contents = fs::read(&path).with_context(context)?;
    let modname = name
        .strip_suffix("/init.lua")
        .or(name.strip_suffix(".lua"))
        .unwrap_or(name);
    lua.preload_embedded(modname, contents.into())
        .with_context(context)
}
fn runner_for_pack(lua: &RuntimeLua) -> mlua::Result<mlua::Function> {
    let require = lua
        .lua()
        .globals()
        .get::<mlua::Function>(RuntimeLua::STD_PACKAGE_REQUIRE);
    require
        .and_then(|req| req.call::<mlua::Table>("@taimi/main/pack"))
        .and_then(|main| main.get::<mlua::Function>("pathing_pack_start"))
}

#[derive(Clone)]
pub struct PackApi {
    pub pack: Arc<Pack>,
    pub loader: Arc<Mutex<&'static mut dyn loader::PackLoaderContext>>,
    pub store: Arc<Mutex<HashMap<String, String>>>,
    pub overrides: script::pathing::imp::PackOverridesShared,
}
impl PackApi {
    pub fn pack(&self) -> &PackArc {
        PackArc::from_ref(&self.pack)
    }
}
impl script::pathing::ScriptApiPack for PackApi {
    fn current_pack(&self) -> script::Result<Self::Pack> {
        Ok(self.clone())
    }
    type Pack = Self;

    fn current_pack_world<'a>(&'a self) -> script::Result<Self::PackWorld<'a>> {
        Ok(self.clone())
    }
    type PackWorld<'a> = Self;

    #[cfg(todo)]
    fn current_pack_space<'a>(&'a self) -> script::Result<Self::PackSpace<'a>> {}
    type PackSpace<'a> = SpaceApi;

    fn current_pack_assets<'a>(&'a self) -> script::Result<Self::PackAssets<'a>> {
        Ok(self.clone())
    }
    type PackAssets<'a> = Self;

    fn current_pack_menu<'a>(&'a self) -> script::Result<Self::PackMenu<'a>> {
        Ok(self.clone())
    }
    type PackMenu<'a> = Self;

    fn current_pack_store<'a>(&'a self) -> script::Result<Self::PackStore<'a>> {
        Ok(self.clone())
    }
    type PackStore<'a> = Self;
}
impl script::pathing::MenuDesc for PackApi {
    fn get_id(&self) -> script::Result<taimi_pack::category::CategoryId> {
        Ok(taimi_pack::category::CategoryId::with_full_id("root.menu"))
    }
    fn get_menu_attr_dyn(
        &self,
        id: taimi_pack::attributes::cell::PackKeyId,
    ) -> script::Result<Option<taimi_pack::attributes::cell::PackValueCell>> {
        taimi_pack::script::pathing::CategoryHandle::get_category_attr_dyn(
            &taimi_pack::script::pathing::PackHandle::root_category(self)?,
            id,
        )
    }
}
impl script::pathing::MenuInstance for PackApi {
    type Menu = PackMenu;
    fn gen_id(
        &self,
        parent: Option<&taimi_pack::category::id::FullIdRef>,
        name: Option<&taimi_pack::category::id::IdNameSeg>,
    ) -> script::Result<String> {
        Ok("menu2".into())
    }
    fn lookup_id(&self, id: &taimi_pack::category::id::FullIdRef) -> script::Result<Option<Self::Menu>> {
        log::warn!("TODO: Menu:LookupId()");
        Ok(None)
    }
    fn remove_id(&self, id: &taimi_pack::category::id::FullIdRef, _recursive: bool) -> script::Result<()> {
        log::warn!("TODO: Menu:Remove()");
        Ok(())
    }
    fn register_id(&self, id: taimi_pack::category::CategoryId) -> script::Result<Self::RegisteredMenu> {
        log::warn!("TODO: Menu:Add()");
        Ok(PackMenu {})
    }
    type RegisteredMenu = PackMenu;
}
impl script::pathing::ScriptApiLookup for PackApi {
    fn poi_by_guid<G>(&self, guid: G) -> script::Result<Option<Self::Poi>>
    where
        G: script::user::ScriptUserGuid,
    {
        self.pack()
            .poi_by_guid(guid)
            .map(|p| p.map(|p| PackPoi::new(p, self.overrides.clone())))
    }

    fn trail_by_guid<G>(&self, guid: G) -> script::Result<Option<Self::Trail>>
    where
        G: script::user::ScriptUserGuid,
    {
        self.pack()
            .trail_by_guid(guid)
            .map(|t| t.map(|t| PackTrail::new(t, self.overrides.clone())))
    }

    #[cfg(todo)]
    fn pathable_by_guid<G>(&self, guid: G) -> script::Result<Option<Self::Pathable>>
    where
        G: script::user::ScriptUserGuid,
    {
    }

    #[cfg(todo)]
    fn pathables_by_guid<G>(&self, guid: G) -> script::Result<Self::PathablesByGuid<'_>>
    where
        G: script::user::ScriptUserGuid,
    {
    }
    type PathablesByGuid<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Pathable> + 'a>;
    /// TODO: include dynamic ones at end!
    fn pois_in_category<I>(&self, cat: I) -> script::Result<Self::CategoryPois<'_>>
    where
        I: ScriptUserStr,
    {
        let cat =
            cat.with_str(|q| {
                self.pack
                    .categories
                    .all_categories
                    .get_full(q)
                    .map(|(_, id, _)| id)
                    .or_else(|| {
                        self.pack.categories.all_categories.keys().find(|&k| {
                            taimi_pack::category::id::IdCmpRelaxed::with_ref(k.as_id()).eq_with(q)
                        })
                    })
                    .ok_or_else(|| script::format_err!("category {q:?} not found"))
            });
        cat.map(|cat| {
            self.pack
                .pois
                .iter()
                .enumerate()
                .filter(|(_, p)| p.category.as_id() == cat.as_id())
                .map(|(i, p)| unsafe {
                    PackPoi::new(
                        taimi_pack::pack::PackPoiArc::new_unchecked(self.pack.clone(), i),
                        self.overrides.clone(),
                    )
                })
        })
        .map(|i| Box::new(i) as Box<_>)
    }
    fn pois_under_category<I>(&self, cat: I) -> script::Result<Self::CategoryPoisRec<'_>>
    where
        I: ScriptUserStr,
    {
        let cat =
            cat.with_str(|q| {
                self.pack
                    .categories
                    .all_categories
                    .get_full(q)
                    .map(|(_, id, _)| id)
                    .or_else(|| {
                        self.pack.categories.all_categories.keys().find(|&k| {
                            taimi_pack::category::id::IdCmpRelaxed::with_ref(k.as_id()).eq_with(q)
                        })
                    })
                    .ok_or_else(|| script::format_err!("category {q:?} not found"))
            });
        cat.map(|cat| {
            self.pack
                .pois
                .iter()
                .enumerate()
                .filter(|(_, p)| p.category.as_id().id_starts_with(cat.as_id()))
                .map(|(i, p)| unsafe {
                    PackPoi::new(
                        taimi_pack::pack::PackPoiArc::new_unchecked(self.pack.clone(), i),
                        self.overrides.clone(),
                    )
                })
        })
        .map(|i| Box::new(i) as Box<_>)
    }
    #[cfg(todo)]
    fn trails_in_category<I>(&self, cat: I) -> script::Result<Self::CategoryTrails<'_>>
    where
        I: ScriptUserStr,
    {
    }
    #[cfg(todo)]
    fn trails_under_category<I>(&self, cat: I) -> script::Result<Self::CategoryTrailsRec<'_>>
    where
        I: ScriptUserStr,
    {
    }
    type CategoryPois<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Poi> + 'a>;
    type CategoryPoisRec<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Poi> + 'a>;
    type CategoryTrails<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Trail> + 'a>;
    type CategoryTrailsRec<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Trail> + 'a>;
}
impl script::pathing::ScriptApiStorage for PackApi {
    fn remove_key<K, N>(&self, key: K, namespace: Option<N>) -> script::Result<()>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
    {
        let mut store = self.store.lock().unwrap();
        key.with_str(|k| match namespace {
            None => store.remove(k),
            Some(n) => n.with_str(|n| store.remove(&format!("{n}/{k}"))),
        });
        Ok(())
    }
    fn get_string<K, N>(&self, key: K, namespace: Option<N>) -> script::Result<Option<String>>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
    {
        let mut store = self.store.lock().unwrap();
        Ok(key
            .with_str(|k| match namespace {
                None => store.get(k),
                Some(n) => n.with_str(|n| store.get(&format!("{n}/{k}"))),
            })
            .cloned())
    }
    fn insert_string<K, N, V>(
        &self,
        key: K,
        namespace: Option<N>,
        value: V,
    ) -> script::Result<Option<String>>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
        V: ScriptUserStr,
    {
        let key = match namespace {
            None => key.clone_to_string(),
            Some(n) => n.with_str(|n| key.with_str(|k| format!("{n}/{k}"))),
        };
        Ok(self.store.lock().unwrap().insert(key, value.clone_to_string()))
    }
}
impl script::pathing::ScriptApiPackAssets for PackApi {
    fn require_src<S: ScriptUserStr>(&self, path: S) -> script::Result<Option<Self::RequireSrc>> {
        path.with_str(|path| {
            let mut loader = self.loader.lock().unwrap();
            let mut res = loader.load_asset_dyn(path).map(Some);
            if matches!(res, Err(..) | Ok(None)) {
                if let Ok(fallback) = loader.load_asset_dyn(&format!("{path}.lua")) {
                    res = Ok(Some(fallback))
                }
            }
            res
        })
    }
    type RequireSrc = Box<dyn loader::LoaderAssetReader>;

    fn open_texture<P>(&self, path: P) -> script::Result<Self::Texture>
    where
        P: ScriptUserStr,
    {
        Ok(TextureApi::new(path.clone_to_string()))
    }
    type Texture = TextureApi;
}
impl script::pathing::PackHandle for PackApi {
    fn get_category<I>(&self, id: I) -> script::Result<Option<Self::Category>>
    where
        I: ScriptUserStr,
    {
        // case-insensitive search
        let canon_id = id.with_str(|id| {
            (!self.pack.categories.all_categories.contains_key(id)).then(|| {
                self.pack
                    .categories
                    .all_categories
                    .keys()
                    .find(|k| taimi_pack::category::id::IdCmpRelaxed::with_ref(k.as_id()).eq_with(id))
                    .map(|k| k.as_str())
            })
        });
        if let Some(Some(id)) = canon_id {
            return self
                .pack()
                .get_category(id)
                .map(|cat| cat.map(|cat| PackCategory::new(cat, self.overrides.clone())));
        }
        self.pack()
            .get_category(id)
            .map(|cat| cat.map(|cat| PackCategory::new(cat, self.overrides.clone())))
    }
    type RootCategory = PackRootCategory;
    fn root_category(&self) -> script::Result<Self::RootCategory> {
        self.pack()
            .root_category()
            .map(|root| PackRootCategory::new(root, self.overrides.clone()))
    }

    fn category_roots(&self) -> script::Result<Self::RootCategories<'_>> {
        let root = script::pathing::imp::PackRootCategories::from_ref(&self.pack);
        let roots = root
            .root_categories()
            .map(|cat| PackCategory::new(cat, self.overrides.clone()))
            .collect::<Box<[_]>>();
        let roots = IntoIterator::into_iter(roots);
        // TODO: append dynamics here!
        Ok(Box::new(roots) as Box<_>)
    }
    fn get_category_children<'a>(
        &'a self,
        parent: &'a Self::Category,
    ) -> script::Result<Self::GetCategories<'a>> {
        let children = parent
            .category()
            .map(|p| {
                let o = script::pathing::imp::PackOverrides::shared_read(&self.overrides);
                p.child_ids()
                    .filter_map(|id| {
                        taimi_pack::pack::PackCategoryArc::get_category(parent.marker.pack(), id)
                    })
                    .filter(|cat| {
                        !o.is_masked((script::pathing::imp::MarkerType::Category, cat.category_idx()))
                    })
                    .collect::<Box<[_]>>()
            })
            .into_iter()
            .flatten()
            .map(|cat| PackCategory::new(cat, self.overrides.clone()));
        // TODO: append dynamics here!
        Ok(Box::new(children) as Box<_>)
    }

    fn get_category_descendents<'a>(
        &'a self,
        parent: &'a Self::Category,
    ) -> script::Result<Self::GetCategoriesRec<'a>> {
        let desc = parent
            .category()
            .map(|p| {
                let o = script::pathing::imp::PackOverrides::shared_read(&self.overrides);
                let masked = o
                    .iter_masked_indices(script::pathing::imp::MarkerType::Category)
                    .collect::<std::collections::BTreeSet<usize>>();
                PackArc::imp_get_category_descendents_with(parent.marker.pack(), &p.full_id)
                    .filter(move |cat| !masked.contains(&cat.category_idx()))
                    .map(|cat| PackCategory::new(cat, self.overrides.clone()))
            })
            .into_iter()
            .flatten();
        // TODO: append dynamics here!
        Ok(Box::new(desc) as Box<_>)
    }

    type RootCategories<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PackHandleFactory>::Category> + 'a>;
    type GetCategories<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PackHandleFactory>::Category> + 'a>;
    type GetCategoriesRec<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PackHandleFactory>::Category> + 'a>;
}
impl script::pathing::PackHandleMut for PackApi {
    fn create_category<I, A>(&self, id: I, attrs: A) -> script::Result<Self::Category>
    where
        I: ScriptUserStr,
        A: IntoIterator<Item = taimi_pack::attributes::cell::PackValueCell>,
    {
        let path = {
            let mut overrides = script::pathing::imp::PackOverrides::shared_write(&self.overrides);
            let path = id.with_str(|id| {
                overrides
                    .allocate_dynamic_cat(taimi_pack::category::CategoryId::with_full_id(id), &self.pack)
            })?;
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| script::pathing::imp::MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs.extend(attrs);
            path
        };

        Ok(unsafe {
            let marker = script::pathing::imp::PackMarkerRef::new_unchecked(self.pack.clone(), path);
            PackCategory::from_marker_unchecked(script::pathing::imp::PackMarkerMut::new(
                marker,
                self.overrides.clone(),
            ))
        })
    }
    fn remove_category(&self, _: &Self::Category) -> script::Result<()> {
        log::warn!("TODO: remove_cat");
        Ok(())
    }

    fn create_poi<A>(&self, attrs: A) -> script::Result<Self::Poi>
    where
        A: IntoIterator<Item = taimi_pack::attributes::cell::PackValueCell>,
    {
        let path = {
            let mut overrides = script::pathing::imp::PackOverrides::shared_write(&self.overrides);
            let path = overrides.allocate_dynamic(script::pathing::imp::MarkerType::Poi, &self.pack)?;
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| script::pathing::imp::MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs.extend(attrs);
            path
        };

        Ok(unsafe {
            let marker = script::pathing::imp::PackMarkerRef::new_unchecked(self.pack.clone(), path);
            PackPoi::from_marker_unchecked(script::pathing::imp::PackMarkerMut::new(
                marker,
                self.overrides.clone(),
            ))
        })
    }
    fn remove_poi(&self, _: &Self::Poi) -> script::Result<()> {
        log::warn!("TODO: remove_poi");
        Ok(())
    }
    fn create_trail<A>(&self, attrs: A) -> script::Result<Self::Trail>
    where
        A: IntoIterator<Item = taimi_pack::attributes::cell::PackValueCell>,
    {
        let path = {
            let mut overrides = script::pathing::imp::PackOverrides::shared_write(&self.overrides);
            let path = overrides.allocate_dynamic(script::pathing::imp::MarkerType::Trail, &self.pack)?;
            let mut o = overrides
                .overrides
                .get(&path)
                .map(|o| script::pathing::imp::MarkerOverrides::shared_write(o))
                .ok_or_else(|| script::format_err!("dynamic marker storage missing"))?;
            o.attrs.extend(attrs);
            path
        };

        Ok(unsafe {
            let marker = script::pathing::imp::PackMarkerRef::new_unchecked(self.pack.clone(), path);
            PackTrail::from_marker_unchecked(script::pathing::imp::PackMarkerMut::new(
                marker,
                self.overrides.clone(),
            ))
        })
    }
    fn remove_trail(&self, _: &Self::Trail) -> script::Result<()> {
        log::warn!("TODO: remove_trail");
        Ok(())
    }
}
impl script::pathing::PackHandleFactory for PackApi {
    type Category = PackCategory;
    type Behaviour = script::Unimplemented;
    type Guid = taimi_pack::attributes::keys::Guid;
}
impl script::pathing::PathableHandleFactory for PackApi {
    type Trail = PackTrail;
    type Poi = PackPoi;
    type Pathable = script::Unimplemented;
}
impl script::user::IntoUserHandle for PackApi {
    type IntoHandle = Self;
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(RuntimeLua::new_api_pack(self.clone_into_handle()), lua)
    }
}
#[derive(Debug, Clone)]
pub struct PackRootCategory {
    pub root: script::pathing::imp::PackRootCategories,
    #[cfg(todo)]
    pub cat: Option<PackCategory>,
    pub overrides: script::pathing::imp::PackOverridesShared,
}
impl PackRootCategory {
    pub fn new(
        root: script::pathing::imp::PackRootCategories,
        overrides: script::pathing::imp::PackOverridesShared,
    ) -> Self {
        Self { root, overrides }
    }
}
impl script::pathing::CategoryHandle for PackRootCategory {
    type GetTrails<'a> = Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Trail>>;
    type GetPois<'a> = Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Poi>>;
    type GetCategories<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PackHandleFactory>::Category>>;

    fn get_id(&self) -> script::Result<taimi_pack::category::CategoryId> {
        self.root.get_id()
    }
    fn is_root(&self) -> script::Result<bool> {
        self.root.is_root()
    }
    fn is_hidden(&self) -> script::Result<bool> {
        self.root.is_hidden()
    }
    fn is_dynamic(&self) -> script::Result<bool> {
        self.root.is_dynamic()
    }
    fn get_id_name(&self) -> script::Result<String> {
        self.root.get_id_name()
    }
    fn is_separator(&self) -> script::Result<bool> {
        self.root.is_separator()
    }
    fn get_display_name(&self) -> script::Result<String> {
        self.root.get_display_name()
    }
    fn is_default_toggle(&self) -> script::Result<bool> {
        self.root.is_default_toggle()
    }
    fn get_category_attr_dyn(
        &self,
        id: taimi_pack::attributes::cell::PackKeyId,
    ) -> script::Result<Option<taimi_pack::attributes::cell::PackValueCell>> {
        Ok(self
            .root
            .iter_root_categories()
            .filter_map(|c| c.get_attr_dyn(id).map(|a| a.into_owned().into_inner()))
            .next())
    }
}
impl script::pathing::CategoryHandleMut for PackRootCategory {}
impl script::user::IntoUserHandle for PackRootCategory {
    type IntoHandle = Self;
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(
            RuntimeLua::new_instance_category_mut(self.clone_into_handle()),
            lua,
        )
    }
}
impl script::pathing::PackHandleFactory for PackRootCategory {
    type Category = <PackApi as script::pathing::PackHandleFactory>::Category;
    type Behaviour = <PackApi as script::pathing::PackHandleFactory>::Behaviour;
    type Guid = <PackApi as script::pathing::PackHandleFactory>::Guid;
}
impl script::pathing::PathableHandleFactory for PackRootCategory {
    type Trail = <PackApi as script::pathing::PathableHandleFactory>::Trail;
    type Poi = <PackApi as script::pathing::PathableHandleFactory>::Poi;
    type Pathable = <PackApi as script::pathing::PathableHandleFactory>::Pathable;
}
#[derive(Debug, Clone)]
pub struct PackCategory {
    /// TODO: PackMarkerMut<PackCategoryArc>
    pub marker: script::pathing::imp::PackMarkerMut,
}
impl PackCategory {
    pub fn new(
        cat: taimi_pack::pack::PackCategoryArc,
        overrides: script::pathing::imp::PackOverridesShared,
    ) -> Self {
        Self {
            marker: script::pathing::imp::PackMarkerMut::new(cat.into(), overrides),
        }
    }
    pub unsafe fn from_marker_unchecked(marker: script::pathing::imp::PackMarkerMut) -> Self {
        Self { marker }
    }
    pub fn category(&self) -> Option<&taimi_pack::Category> {
        self.marker
            .pack()
            .categories
            .all_categories
            .get_index(self.marker.marker_index())
            .map(|(_, c)| c)
    }
}
use taimi_pack::attributes::cell::AttrKeyValue;
impl script::pathing::CategoryHandle for PackCategory {
    type GetTrails<'a> = Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Trail>>;
    type GetPois<'a> = Box<dyn Iterator<Item = <Self as script::pathing::PathableHandleFactory>::Poi>>;
    type GetCategories<'a> =
        Box<dyn Iterator<Item = <Self as script::pathing::PackHandleFactory>::Category>>;
    fn get_id(&self) -> script::Result<taimi_pack::category::CategoryId> {
        let id = {
            //self.marker.lookup_override::<taimi_pack::attributes::keys::CategoryRef>()
            let o = self.marker.overrides_read();
            o.as_ref()
                .and_then(|o| o.get::<taimi_pack::attributes::keys::CategoryRef>())
                .map(|id| id.and_then(|id| taimi_pack::category::CategoryId::try_with_full_id(&id[..])))
        };

        if let Some(id) = id { id } else { self.category().map(|c| c.full_id.clone()) }
            .ok_or_else(|| script::format_err!("missing cat id"))
    }
    fn get_category_attr_dyn(
        &self,
        id: taimi_pack::attributes::cell::PackKeyId,
    ) -> script::Result<Option<taimi_pack::attributes::cell::PackValueCell>> {
        Ok(self
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }
    fn is_root(&self) -> script::Result<bool> {
        Ok(self
            .category()
            .map(|c| self.marker.pack().categories.root_categories.contains(&c.full_id))
            .unwrap_or(false))
    }
    fn is_dynamic(&self) -> script::Result<bool> {
        Ok(self.category().is_none())
    }
}
impl script::pathing::CategoryHandleMut for PackCategory {
    fn hide(&self) -> script::Result<()> {
        log::warn!("TODO: hide cat");
        Ok(())
    }

    fn show(&self) -> script::Result<()> {
        log::warn!("TODO: show cat");
        Ok(())
    }

    fn is_visible(&self) -> script::Result<bool> {
        log::warn!("TODO: cat state");
        Ok(false)
    }
    fn set_category_attr_dyn(
        &self,
        value: taimi_pack::attributes::cell::PackValueCell,
    ) -> script::Result<()> {
        self.marker.overrides_write().attrs.set_attr_dyn(value);
        Ok(())
    }
}
impl script::user::IntoUserHandle for PackCategory {
    type IntoHandle = Self;
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(
            RuntimeLua::new_instance_category_mut(self.clone_into_handle()),
            lua,
        )
    }
}
impl script::pathing::PackHandleFactory for PackCategory {
    type Category = <PackRootCategory as script::pathing::PackHandleFactory>::Category;
    type Behaviour = <PackRootCategory as script::pathing::PackHandleFactory>::Behaviour;
    type Guid = <PackRootCategory as script::pathing::PackHandleFactory>::Guid;
}
impl script::pathing::PathableHandleFactory for PackCategory {
    type Trail = <PackRootCategory as script::pathing::PathableHandleFactory>::Trail;
    type Poi = <PackRootCategory as script::pathing::PathableHandleFactory>::Poi;
    type Pathable = <PackRootCategory as script::pathing::PathableHandleFactory>::Pathable;
}

#[derive(Debug, Clone)]
pub struct PackTrail {
    /// TODO: PackMarkerMut<PackTrailArc>
    pub marker: script::pathing::imp::PackMarkerMut,
}
impl PackTrail {
    pub fn new(
        trail: taimi_pack::pack::PackTrailArc,
        overrides: script::pathing::imp::PackOverridesShared,
    ) -> Self {
        Self {
            marker: script::pathing::imp::PackMarkerMut::new(trail.into(), overrides),
        }
    }
    pub unsafe fn from_marker_unchecked(marker: script::pathing::imp::PackMarkerMut) -> Self {
        Self { marker }
    }
}
impl script::pathing::PathableHandle for PackTrail {
    fn get_marker_attr_dyn(
        &self,
        id: taimi_pack::attributes::cell::PackKeyId,
    ) -> script::Result<Option<taimi_pack::attributes::cell::PackValueCell>> {
        Ok(self
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }
    fn pathable_tag_index(&self) -> u32 {
        let kind = (self.marker.marker_kind() as u8 as u32) << 28;
        kind as u32 | (self.marker.marker_index() as u32)
    }
    fn pathable_tag_type(&self) -> script::pathing::MarkerType {
        script::pathing::MarkerType::Trail
    }
}
impl script::pathing::PathableHandleMut for PackTrail {
    fn set_marker_attr_dyn(&self, v: taimi_pack::attributes::cell::PackValueCell) -> script::Result<()> {
        self.marker.overrides_write().attrs.set_attr_dyn(v);
        Ok(())
    }
}
impl script::pathing::TrailHandle for PackTrail {}
impl script::pathing::TrailHandleMut for PackTrail {}
impl script::user::IntoUserHandle for PackTrail {
    type IntoHandle = Self;
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(RuntimeLua::new_instance_trail_mut(self.clone_into_handle()), lua)
    }
}
impl script::pathing::PackHandleFactory for PackTrail {
    type Category = <PackRootCategory as script::pathing::PackHandleFactory>::Category;
    type Behaviour = <PackRootCategory as script::pathing::PackHandleFactory>::Behaviour;
    type Guid = <PackRootCategory as script::pathing::PackHandleFactory>::Guid;
}

#[derive(Debug, Clone)]
pub struct PackPoi {
    /// TODO: PackMarkerMut<PackPoiArc>
    pub marker: script::pathing::imp::PackMarkerMut,
}
impl PackPoi {
    pub fn new(
        poi: taimi_pack::pack::PackPoiArc,
        overrides: script::pathing::imp::PackOverridesShared,
    ) -> Self {
        Self {
            marker: script::pathing::imp::PackMarkerMut::new(poi.into(), overrides),
        }
    }
    pub unsafe fn from_marker_unchecked(marker: script::pathing::imp::PackMarkerMut) -> Self {
        Self { marker }
    }
}
impl script::pathing::PathableHandle for PackPoi {
    fn get_marker_attr_dyn(
        &self,
        id: taimi_pack::attributes::cell::PackKeyId,
    ) -> script::Result<Option<taimi_pack::attributes::cell::PackValueCell>> {
        Ok(self
            .marker
            .lookup_attr_dyn(id)
            .map(|v| v.into_owned().into_inner()))
    }
    fn pathable_tag_index(&self) -> u32 {
        let kind = (self.marker.marker_kind() as u8 as u32) << 28;
        kind as u32 | (self.marker.marker_index() as u32)
    }
    fn pathable_tag_type(&self) -> script::pathing::MarkerType {
        script::pathing::MarkerType::Poi
    }
}
impl script::pathing::PathableHandleMut for PackPoi {
    fn set_marker_attr_dyn(&self, v: taimi_pack::attributes::cell::PackValueCell) -> script::Result<()> {
        self.marker.overrides_write().attrs.set_attr_dyn(v);
        Ok(())
    }
}
impl script::pathing::PoiHandle for PackPoi {
    type Point3 = script::value::Vec3;
    type RotationEuler = script::value::Vec3;
}
impl script::pathing::PoiHandleMut for PackPoi {}
impl script::user::IntoUserHandle for PackPoi {
    type IntoHandle = Self;
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(RuntimeLua::new_instance_poi_mut(self.clone_into_handle()), lua)
    }
}
impl script::pathing::PackHandleFactory for PackPoi {
    type Category = <PackRootCategory as script::pathing::PackHandleFactory>::Category;
    type Behaviour = <PackRootCategory as script::pathing::PackHandleFactory>::Behaviour;
    type Guid = <PackRootCategory as script::pathing::PackHandleFactory>::Guid;
}

type SpaceApi = script::Unimplemented;

#[derive(Debug, Clone)]
pub struct TextureApi {
    pub path: String,
}
impl TextureApi {
    pub fn new(path: String) -> Self {
        Self { path }
    }
}
impl script::pathing::TextureHandle for TextureApi {}
impl script::pathing::InstanceTexture for TextureApi {
    fn get_size(&self) -> script::Result<[u32; 2]> {
        Ok([0, 0])
    }
}
impl script::user::IntoUserHandle for TextureApi {
    type IntoHandle = Self;
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(self.clone_into_handle(), lua)
    }
}
impl mlua::UserData for TextureApi {
    fn register(reg: &mut mlua::UserDataRegistry<Self>) {
        use mlua::UserDataMethods;
        script::lua::ScriptApiTable::<_, Self>::register_texture(reg);
        reg.add_meta_method(mlua::MetaMethod::ToString.name(), |lua, this, ()| {
            mlua::IntoLua::into_lua(&this.path[..], lua)
        });
    }
}

#[derive(Debug, Clone, Default)]
pub struct EventApi {}
impl script::pathing::ScriptApiEvent for EventApi {
    fn all_notifications(&self) -> Self::SignalNames {
        <script::Unimplemented as script::pathing::ScriptApiEvent>::all_notifications(
            &script::Unimplemented,
        )
    }
    fn all_signals(&self) -> Self::SignalNames {
        <script::Unimplemented as script::pathing::ScriptApiEvent>::all_signals(&script::Unimplemented)
    }
    type SignalNames = <script::Unimplemented as script::pathing::ScriptApiEvent>::SignalNames;

    fn notifcation_oob<S, A>(
        &self,
        source: S,
        msg: script::pathing::event::NotifyScript<A>,
    ) -> script::Result<()>
    where
        S: script::user::ScriptSourceTag,
        A: script::user::ScriptUserUntyped,
    {
        log::warn!("TODO: SignalOob");
        Ok(())
    }
    fn notifcation_mask(&self, id: script::pathing::event::SignalId) -> script::Result<()> {
        Ok(())
    }
    fn notifcation_unmask(&self, id: script::pathing::event::SignalId) -> script::Result<()> {
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct PackMenu {}
impl script::pathing::MenuDesc for PackMenu {
    fn get_id(&self) -> script::Result<taimi_pack::category::CategoryId> {
        Ok(taimi_pack::category::CategoryId::with_full_id("root.menu.todo"))
    }
    fn get_menu_attr_dyn(
        &self,
        id: taimi_pack::attributes::cell::PackKeyId,
    ) -> script::Result<Option<taimi_pack::attributes::cell::PackValueCell>> {
        log::warn!("TODO: Menu:GetAttr");
        Ok(None)
    }
}
impl script::pathing::MenuHandle for PackMenu {
    fn get_check_state(&self) -> script::Result<Option<bool>> {
        log::warn!("TODO: Menu:GetState");
        Ok(Some(false))
    }
}
impl script::pathing::MenuHandleMut for PackMenu {
    fn set_check_state(&self, v: Option<bool>) -> script::Result<()> {
        log::warn!("TODO: Menu:SetState");
        Ok(())
    }

    fn set_menu_attr_dyn(&self, v: taimi_pack::attributes::cell::PackValueCell) -> script::Result<()> {
        log::warn!("TODO: Menu:SetAttr({})", v.id());
        Ok(())
    }
}
impl script::user::IntoUserHandle for PackMenu {
    type IntoHandle = Self;
    fn clone_into_handle(&self) -> Self::IntoHandle {
        self.clone()
    }
    fn to_lua_handle(&self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        mlua::IntoLua::into_lua(RuntimeLua::new_instance_menu_mut(self.clone_into_handle()), lua)
    }
}
