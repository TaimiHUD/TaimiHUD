use {
    crate::{
        pack::{PackCategoryArc, PackPoiArc, PackTrailArc},
        script::{
            format_err,
            lua::{
                attributes::MarkerAttrSet,
                to_lua_error,
                HandleToLua,
                IVec3,
                IntoLuaArray,
                LuaCategory,
                LuaGuid,
                RuntimeLua,
                ScriptApiTable,
            },
            pathing::{
                imp::PackArc,
                CategoryHandle,
                CategoryHandleMut,
                InstanceTexture,
                InstanceVec3,
                MapFilterArg,
                MarkerType,
                MenuInstance,
                PackHandle,
                PackHandleFactory,
                PackHandleMut,
                PathableHandle,
                PathableHandleMut,
                PoiHandle,
                PoiHandleMut,
                ScriptApiLookup,
                ScriptApiPack,
                ScriptApiPackAssets,
                ScriptApiSpaceQuery,
                TrailHandle,
                TrailHandleMut,
            },
            user::IntoUserHandle,
        },
    },
    anyhow::Context,
    core::{borrow::Borrow, marker::PhantomData},
    mlua::{
        AnyUserData,
        BorrowedStr,
        FromLua,
        FromLuaMulti,
        IntoLua,
        Lua,
        MetaMethod,
        MultiValue,
        Result as LuaResult,
        String as LuaString,
        Table,
        UserData,
        UserDataFields,
        UserDataMethods,
        UserDataRef,
        UserDataRefMut,
        UserDataRegistry,
        Value as LuaValue,
        ObjectLike,
    },
    std::io,
};

pub struct GlobalInstancePack;
impl<T> UserData for ScriptApiTable<GlobalInstancePack, T>
where
    T: ScriptApiPack + 'static,
    for<'a> T::PackAssets<'a>: 'static,
    for<'a> T::PackStore<'a>: 'static,
    for<'a> T::PackWorld<'a>: 'static,
    for<'a> T::PackSpace<'a>: 'static,
    for<'a> T::PackMenu<'a>: MenuInstance + 'static,
    for<'a> <T::PackMenu<'a> as MenuInstance>::Menu: IntoUserHandle + 'static,
    for<'a> <T::PackMenu<'a> as MenuInstance>::RegisteredMenu: IntoUserHandle + 'static,
    <T::Pack as PackHandle>::RootCategory: IntoUserHandle + 'static,
    T::Pack: PackHandleMut + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_pack_info(reg);
    }
}
impl<T> ScriptApiTable<GlobalInstancePack, T>
where
    T: ScriptApiPack + 'static,
    for<'a> T::PackAssets<'a>: 'static,
    for<'a> T::PackStore<'a>: 'static,
    for<'a> T::PackWorld<'a>: 'static,
    for<'a> T::PackSpace<'a>: 'static,
    for<'a> T::PackMenu<'a>: MenuInstance + 'static,
    for<'a> <T::PackMenu<'a> as MenuInstance>::Menu: IntoUserHandle + 'static,
    for<'a> <T::PackMenu<'a> as MenuInstance>::RegisteredMenu: IntoUserHandle + 'static,
    T::Pack: PackHandleMut + 'static,
    <T::Pack as PackHandle>::RootCategory: IntoUserHandle + 'static,
{
    pub fn register_pack_info<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("GetPackHandle", |_lua, this, ()| {
            let pack = this.borrow().current_pack();
            pack.map(|api| ScriptApiTable {
                api,
                _api: PhantomData::<PackInstanceHandle>,
            })
            //pack.map(HandleToLua)
            .map_err(to_lua_error)
        });
        // TODO: avoiding the `into_lua` call probably requires a bound like:
        // for<'a> T: ScriptApiPack<PackWorld<'a> = W>,
        // why isn't for<'a>: 'static enough to let it move without associating the borrow?
        reg.add_function("GetPackAssets", |lua, (ud,): (AnyUserData,)| {
            let assets = ud.borrow::<Self>().and_then(|this| {
                let pack = Borrow::<T>::borrow(&*this)
                    .current_pack_assets()
                    .map_err(to_lua_error);
                pack.and_then(|api| {
                    ScriptApiTable {
                        api,
                        _api: PhantomData::<GlobalInstancePackAssets>,
                    }
                    .into_lua(lua)
                })
            })?;
            if let Some(assets) = assets.as_userdata() {
                assets.set_named_user_value(RuntimeLua::PACK_HANDLE_USERDATA_INFO, ud)?;
            }

            Ok(assets)
        });
        reg.add_method("GetWorldHandle", |lua, this, ()| {
            this.borrow()
                .current_pack_world()
                .map_err(to_lua_error)
                .and_then(|api| {
                    ScriptApiTable {
                        api,
                        _api: PhantomData::<GlobalInstanceWorld>,
                    }
                    .into_lua(lua)
                })
        });
        reg.add_method("GetSpaceHandle", |lua, this, ()| {
            this.borrow()
                .current_pack_space()
                .map_err(to_lua_error)
                .and_then(|api| {
                    ScriptApiTable {
                        api,
                        _api: PhantomData::<GlobalInstanceSpace>,
                    }
                    .into_lua(lua)
                })
        });
        reg.add_method("GetStorage", |lua, this, ()| {
            this.borrow()
                .current_pack_store()
                .map_err(to_lua_error)
                .and_then(|api| {
                    ScriptApiTable {
                        api,
                        _api: PhantomData::<super::PersistInstanceStore>,
                    }
                    .into_lua(lua)
                })
        });
        reg.add_method("GetRootMenu", |lua, this, ()| {
            this.borrow()
                .current_pack_menu()
                .map_err(to_lua_error)
                .and_then(|api| RuntimeLua::new_instance_menu_root(api).into_lua(lua))
        });
        reg.add_method("GetRootCategory", |_lua, this, ()| {
            let root = this.borrow().current_pack().and_then(|pack| pack.root_category());
            /*root.map(|api| ScriptApiTable {
                api,
                _api: PhantomData::<super::InstanceTableCategory>,
            })*/
            root.map(HandleToLua).map_err(to_lua_error)
        });
        reg.add_method("CategoryRoots", |lua, this, ()| {
            this.borrow()
                .current_pack()
                .map_err(to_lua_error)
                .and_then(|pack| {
                    pack.category_roots()
                        .map(|c| IntoLuaArray(c.map(|b| HandleToLua(b))))
                        .map_err(to_lua_error)
                        .and_then(|c| c.into_lua(lua))
                })
        });
        reg.add_method(
            "CategoryChildren",
            |lua,
             this,
             (cat,): (
                UserDataRef<
                    ScriptApiTable<InstanceTableCategory, <T::Pack as PackHandleFactory>::Category>,
                >,
            )| {
                this.borrow()
                    .current_pack()
                    .map_err(to_lua_error)
                    .and_then(|pack| {
                        pack.get_category_children(&cat.api)
                            .map(|c| IntoLuaArray(c.map(|b| HandleToLua(b))))
                            .map_err(to_lua_error)
                            .and_then(|c| c.into_lua(lua))
                    })
            },
        );
        reg.add_method(
            "CategoryDescendents",
            |lua,
             this,
             (cat,): (
                UserDataRef<
                    ScriptApiTable<InstanceTableCategory, <T::Pack as PackHandleFactory>::Category>,
                >,
            )| {
                this.borrow()
                    .current_pack()
                    .map_err(to_lua_error)
                    .and_then(|pack| {
                        pack.get_category_descendents(&cat.api)
                            .map(|c| IntoLuaArray(c.map(|b| HandleToLua(b))))
                            .map_err(to_lua_error)
                            .and_then(|c| c.into_lua(lua))
                    })
            },
        );
        reg.add_method("CategoryByType", |_lua, this, (id,): (mlua::String,)| {
            let cat = this
                .borrow()
                .current_pack()
                .and_then(|pack| pack.get_category(id));
            /*cat.map(|api| ScriptApiTable {
                api,
                _api: PhantomData::<super::InstanceTableCategory>,
            })*/
            cat.map(|cat| cat.map(HandleToLua)).map_err(to_lua_error)
        });
    }
}

pub struct PackInstanceHandle;
impl<T> UserData for ScriptApiTable<PackInstanceHandle, T>
where
    //T: ScriptApiPack + 'static, T::Pack: PackHandleMut,
    T: PackHandleMut + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_pack(reg);
    }
}
impl<T> ScriptApiTable<PackInstanceHandle, T>
where
    //T: ScriptApiPack + 'static, T::Pack: PackHandleMut,
    T: PackHandleMut + 'static,
{
    pub fn register_pack<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method(
            "CreateMarker",
            |_lua, this, (mut attrs,): (UserDataRefMut<MarkerAttrSet>,)| {
                this.borrow()
                    .create_poi(attrs.drain_all())
                    .map_err(to_lua_error)
                    .map(HandleToLua)
            },
        );
        reg.add_method(
            "RemoveMarker",
            |_lua, this, (m,): (UserDataRef<ScriptApiTable<InstanceTablePoi, T::Poi>>,)| {
                this.borrow().remove_poi(&m.api).map_err(to_lua_error)
            },
        );
        reg.add_method(
            "CreateTrail",
            |_lua, this, (mut attrs,): (UserDataRefMut<MarkerAttrSet>,)| {
                this.borrow()
                    .create_trail(attrs.drain_all())
                    .map_err(to_lua_error)
                    .map(HandleToLua)
            },
        );
        reg.add_method(
            "RemoveTrail",
            |_lua, this, (m,): (UserDataRef<ScriptApiTable<InstanceTableTrail, T::Trail>>,)| {
                this.borrow().remove_trail(&m.api).map_err(to_lua_error)
            },
        );
        reg.add_method(
            "CreateCategory",
            |_lua, this, (id, mut attrs): (BorrowedStr<'_>, UserDataRefMut<MarkerAttrSet>)| {
                this.borrow()
                    .create_category(&id[..], attrs.drain_all())
                    .map_err(to_lua_error)
                    .map(HandleToLua)
            },
        );
        reg.add_method(
            "RemoveCategory",
            |_lua, this, (m,): (UserDataRef<ScriptApiTable<InstanceTableCategory, T::Category>>,)| {
                this.borrow().remove_category(&m.api).map_err(to_lua_error)
            },
        );
    }
}
pub struct GlobalInstancePackAssets;
impl<T> UserData for ScriptApiTable<GlobalInstancePackAssets, T>
where
    T: ScriptApiPackAssets + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_pack_assets(reg);
    }
}
impl<T> ScriptApiTable<GlobalInstancePackAssets, T>
where
    T: ScriptApiPackAssets + 'static,
{
    pub fn register_pack_assets<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_function(
            "Require",
            |lua, (assets, path, args): (AnyUserData, BorrowedStr<'_>, MultiValue)| {
                let src = assets.borrow::<Self>().and_then(|this| {
                    Borrow::<T>::borrow(&*this)
                        .require_src(&path[..])
                        .map_err(to_lua_error)
                });
                let pack_info = assets
                    .named_user_value::<Option<AnyUserData>>(RuntimeLua::PACK_HANDLE_USERDATA_INFO)?;
                let Some(asset) = src? else {
                    log::warn!("{} required but not found", path);
                    return Ok(LuaValue::Nil)
                };
                let mut src = Vec::new();
                io::Read::read_to_end(&mut { asset }, &mut src)
                    .context("reading required lua src")
                    .map_err(to_lua_error)?;
                let chunk = lua.load(&src).set_name(format!("@{}", path));
                let chunk = match RuntimeLua::lua_is_unsecured(lua) {
                    Some(super::UnsafeRuntime) => chunk,
                    None => chunk.set_mode(mlua::ChunkMode::Text),
                };
                let chunk = match pack_info {
                    Some(pack_info) =>
                        if let Ok(Some(globals)) = pack_info
                            .named_user_value::<Option<Table>>(RuntimeLua::PACK_INFO_USERDATA_GLOBALS)
                        {
                            chunk.set_environment(globals)
                        } else {
                            chunk
                        },
                    None => chunk,
                };
                if args.is_empty() {
                    chunk.eval().map(|()| LuaValue::Nil)
                } else {
                    chunk.call(args)
                }
            },
        );
        reg.add_method("OpenTexture", |lua, this, (path,): (BorrowedStr<'_>,)| {
            this.borrow()
                .open_texture(&path[..])
                .map_err(to_lua_error)
                .and_then(|tex| tex.to_lua_handle(lua))
        });
    }
}
pub struct GlobalInstanceWorld;
impl<T> UserData for ScriptApiTable<GlobalInstanceWorld, T>
where
    T: ScriptApiLookup + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_world_lookup(reg);
    }
}
impl<T> ScriptApiTable<GlobalInstanceWorld, T>
where
    T: ScriptApiLookup + 'static,
{
    pub fn register_world_lookup<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("PathableByGuid", |lua, this, (guid,filter): (LuaGuid,MapFilterArg)| {
            this.borrow()
                .pathable_by_guid(guid, filter)
                .map_err(to_lua_error)
                .and_then(|h| h.map(|h| h.to_lua_handle(lua)).transpose())
        });
        reg.add_method("PathableByTag", |lua, this, (tag,): (u32,)| {
            this.borrow()
                .pathable_by_tag(tag)
                .map_err(to_lua_error)
                .and_then(|h| h.map(|h| h.to_lua_handle(lua)).transpose())
        });
        reg.add_method("PathablesByGuid", |lua, this, (guid,filter): (LuaGuid,MapFilterArg)| {
            this.borrow()
                .pathables_by_guid(guid, filter)
                .map_err(to_lua_error)
                .and_then(|b| lua.create_sequence_from(b.into_iter().map(|b| HandleToLua(b))))
        });
        reg.add_method("MarkerByGuid", |lua, this, (guid,filter): (LuaGuid,MapFilterArg)| {
            this.borrow()
                .poi_by_guid(guid, filter)
                .map_err(to_lua_error)
                .and_then(|h| h.map(|h| h.to_lua_handle(lua)).transpose())
        });
        reg.add_method("TrailByGuid", |lua, this, (guid,filter): (LuaGuid,MapFilterArg)| {
            this.borrow()
                .trail_by_guid(guid, filter)
                .map_err(to_lua_error)
                .and_then(|h| h.map(|h| h.to_lua_handle(lua)).transpose())
        });
        reg.add_method("MarkersInCategory", |lua, this, (id,): (BorrowedStr<'_>,)| {
            this.borrow()
                .pois_in_category(&id[..])
                .map_err(to_lua_error)
                .and_then(|b| lua.create_sequence_from(b.into_iter().map(|b| HandleToLua(b))))
        });
        reg.add_method("MarkersUnderCategory", |lua, this, (id,): (BorrowedStr<'_>,)| {
            this.borrow()
                .pois_under_category(&id[..])
                .map_err(to_lua_error)
                .and_then(|b| lua.create_sequence_from(b.into_iter().map(|b| HandleToLua(b))))
        });
        reg.add_method("TrailsInCategory", |lua, this, (id,): (BorrowedStr<'_>,)| {
            this.borrow()
                .trails_in_category(&id[..])
                .map_err(to_lua_error)
                .and_then(|b| lua.create_sequence_from(b.into_iter().map(|b| HandleToLua(b))))
        });
        reg.add_method("TrailsUnderCategory", |lua, this, (id,): (BorrowedStr<'_>,)| {
            this.borrow()
                .trails_under_category(&id[..])
                .map_err(to_lua_error)
                .and_then(|b| lua.create_sequence_from(b.into_iter().map(|b| HandleToLua(b))))
        });
    }
}
pub struct GlobalInstanceSpace;
impl<T> UserData for ScriptApiTable<GlobalInstanceSpace, T>
where
    T: ScriptApiSpaceQuery + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_world_space_query(reg);
    }
}
impl<T> ScriptApiTable<GlobalInstanceSpace, T>
where
    T: ScriptApiSpaceQuery + 'static,
{
    pub fn register_world_space_query<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method(
            "GetDistanceToPlayer",
            |_lua, this, (poi,): (UserDataRef<ScriptApiTable<InstanceTablePoi, T::Poi>>,)| {
                this.borrow()
                    .get_distance_to_player(&poi.api)
                    .map_err(to_lua_error)
            },
        );
        reg.add_method("GetClosestMarker", |lua, this, (cat,): (Option<LuaCategory>,)| {
            this.borrow()
                .get_closest_poi_in_category(cat)
                .map_err(to_lua_error)
                .and_then(|poi| poi.map(|poi| poi.to_lua_handle(lua)).transpose())
        });
        reg.add_method("GetClosestMarkers", |lua, this, args: MultiValue| {
            let (cat, limit) = match args.len() {
                2 => <(Option<LuaCategory>, usize) as FromLuaMulti>::from_lua_multi(args, lua),
                _ => <(usize,) as FromLuaMulti>::from_lua_multi(args, lua).map(|(limit,)| (None, limit)),
            }?;
            this.borrow()
                .get_closest_pois_in_category(limit, cat)
                .map_err(to_lua_error)
                .and_then(|poi| lua.create_sequence_from(poi.into_iter().map(|poi| HandleToLua(poi))))
        });
    }
}

pub struct InstanceTableCategory;
impl<T> UserData for ScriptApiTable<InstanceTableCategory, T>
where
    T: CategoryHandleMut + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_category(reg);
        Self::register_category_mut(reg);
    }
}
impl<T> ScriptApiTable<InstanceTableCategory, T>
where
    T: CategoryHandle + 'static,
{
    pub fn register_category<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        #[cfg(todo)]
        {
            reg.add_field_method_get("DefaultToggle", |_lua, this| {
                this.borrow().is_default_toggle().map_err(to_lua_error)
            });
            reg.add_field_method_get("IsHidden", |_lua, this| {
                this.borrow().is_hidden().map_err(to_lua_error)
            });
            reg.add_field_method_get("IsSeparator", |_lua, this| {
                this.borrow().is_separator().map_err(to_lua_error)
            });
            reg.add_field_method_get("DisplayName", |_lua, this| {
                this.borrow().get_display_name().map_err(to_lua_error)
            });
            reg.add_field_method_get("Name", |_lua, this| {
                this.borrow().get_id_name().map_err(to_lua_error)
            });
            reg.add_field_method_get("Parent", |lua, this| {
                this.borrow()
                    .get_parent()
                    .map_err(to_lua_error)
                    .and_then(|h| h.map(|h| h.to_lua_handle(lua)).transpose())
            });
            reg.add_method("GetMarkers", |lua, this, (recursive,): (Option<bool>,)| {
                this.borrow()
                    .get_pois(recursive.unwrap_or(false))
                    .map_err(to_lua_error)
                    .and_then(|pois| {
                        lua.create_table_from(
                            pois.map(|poi| IntoLuaFn::new(move |lua| poi.to_lua_handle(lua)))
                                .enumerate(),
                        )
                    })
            });
            reg.add_method("GetTrails", |lua, this, (recursive,): (Option<bool>,)| {
                this.borrow()
                    .get_trails(recursive.unwrap_or(false))
                    .map_err(to_lua_error)
                    .and_then(|trails| {
                        lua.create_table_from(
                            trails
                                .map(|trail| IntoLuaFn::new(move |lua| trail.to_lua_handle(lua)))
                                .enumerate(),
                        )
                    })
            });
            #[cfg(todo)]
            for (i, attr) in super::attributes::CATEGORY_ATTRS[..].iter().enumerate() {
                reg.add_field_method_get(attr.index(), move |lua, this| {
                    let attr = unsafe { super::attributes::CATEGORY_ATTRS[..].get_unchecked(i) };
                    Borrow::<T>::borrow(&*this)
                        .get_category_attr_dyn(attr.attr())
                        .map_err(to_lua_error)
                        .and_then(move |v| v.map(|v| attr.to_lua_dyn(&v, lua)).transpose())
                });
            }
        }
        reg.add_field_method_get("Root", |_lua, this| this.borrow().is_root().map_err(to_lua_error));
        reg.add_field_method_get("LoadedFromPack", |_lua, this| {
            this.borrow().is_dynamic().map(|v| !v).map_err(to_lua_error)
        });
        reg.add_field_method_get("Namespace", |_lua, this| {
            this.borrow().get_id().map_err(to_lua_error)
        });
        reg.add_method("GetAttrByKey", |lua, this, (key,): (BorrowedStr<'_>,)| {
            super::AttrRegistration::for_key(&key)
                .ok_or_else(|| to_lua_error(format_err!("unrecognized category attribute {key}")))
                .and_then(|attr| {
                    Borrow::<T>::borrow(&*this)
                        .get_category_attr_dyn(attr.attr())
                        .map_err(to_lua_error)
                        .and_then(move |v| v.map(|v| attr.to_lua_dyn(&v, lua)).transpose())
                })
        });
    }
}
impl<T> ScriptApiTable<InstanceTableCategory, T>
where
    T: CategoryHandleMut + 'static,
{
    pub fn register_category_mut<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("IsVisible", |_lua, this, ()| {
            this.borrow().is_visible().map_err(to_lua_error)
        });
        reg.add_method("Show", |_lua, this, ()| {
            this.borrow().show().map_err(to_lua_error)
        });
        reg.add_method("Hide", |_lua, this, ()| {
            this.borrow().hide().map_err(to_lua_error)
        });
        reg.add_method(
            "SetAttrByKey",
            |lua, this, (key, value): (BorrowedStr<'_>, LuaValue)| {
                super::AttrRegistration::for_key(&key)
                    .ok_or_else(|| to_lua_error(format_err!("unrecognized category attribute {key}")))
                    .and_then(|attr| attr.from_lua_dyn(value, lua))
                    .and_then(|v| {
                        Borrow::<T>::borrow(&*this)
                            .set_category_attr_dyn(v)
                            .map_err(to_lua_error)
                    })
            },
        );
        #[cfg(todo)]
        {
            for (i, attr) in super::attributes::CATEGORY_ATTRS[..].iter().enumerate() {
                reg.add_field_method_set(attr.index(), move |lua, this, v: LuaValue| {
                    let attr = unsafe { super::attributes::CATEGORY_ATTRS[..].get_unchecked(i) };
                    attr.from_lua_dyn(v, lua).and_then(|v| {
                        Borrow::<T>::borrow(&*this)
                            .set_category_attr_dyn(v)
                            .map_err(to_lua_error)
                    })
                });
            }
        }
    }
}

impl<A: ?Sized, T> ScriptApiTable<A, T>
where
    T: PathableHandle + 'static,
{
    pub fn register_pathable<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
        T::Guid: IntoLua,
        T::Category: IntoUserHandle,
        T::Behaviour: IntoUserHandle,
    {
        #[cfg(todo)]
        {
            reg.add_field_method_get("Guid", |_lua, this| {
                this.borrow().get_guid().map_err(to_lua_error)
            });
            reg.add_field_method_get("MapId", |_lua, this| {
                this.borrow().get_map_id().map_err(to_lua_error)
            });
        }
        reg.add_field_method_get("BehaviorFiltered", |_lua, this| {
            this.borrow().get_behaviour_filtered().map_err(to_lua_error)
        });
        reg.add_field_method_get("Focused", |_lua, this| {
            this.borrow().get_focused().map_err(to_lua_error)
        });
        reg.add_field_method_get("PathableTagIndex", |_lua, this| {
            //Ok(mlua::LightUserData(this.borrow().pathable_tag_index() as usize as *mut _))
            Ok(LuaValue::Integer(this.borrow().pathable_tag_index() as _))
        });
        reg.add_field_method_get("PathableTagType", |_lua, this| {
            Ok(this.borrow().pathable_tag_type())
        });
        reg.add_meta_function(MetaMethod::Eq.name(), |lua, (lhs,rhs): (LuaValue,LuaValue)| {
            let lhs = match lhs {
                LuaValue::Nil => return Ok(false),
                LuaValue::Integer(idx) => Ok(idx as u32),
                LuaValue::Table(v) => v.get::<u32>("PathableTagIndex"),
                LuaValue::UserData(ud) => ud.get::<u32>("PathableTagIndex"),
                #[cfg(todo)]
                v => u32::from_lua(v, lua),
                _ => return Ok(false),
            }?;
            let rhs = match rhs {
                LuaValue::Nil => return Ok(false),
                LuaValue::Integer(idx) => Ok(idx as u32),
                LuaValue::Table(v) => v.get::<u32>("PathableTagIndex"),
                LuaValue::UserData(ud) => ud.get::<u32>("PathableTagIndex"),
                #[cfg(todo)]
                v => u32::from_lua(v, lua),
                _ => return Ok(false),
            }?;
            Ok(lhs == rhs)
        });
    }
}
impl<A: ?Sized, T> ScriptApiTable<A, T>
where
    T: PathableHandleMut + 'static,
{
    pub fn register_pathable_mut<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
        T::Guid: IntoLua,
    {
        #[cfg(todo)]
        {
            reg.add_field_method_set("Guid", |_lua, this, v: Guid| {
                (*this).borrow().set_guid(v).map_err(to_lua_error)
            });
        }
        reg.add_method("Focus", |_lua, this, ()| {
            this.borrow().focus().map_err(to_lua_error)
        });
        reg.add_method("Unfocus", |_lua, this, ()| {
            this.borrow().unfocus().map_err(to_lua_error)
        });
        reg.add_method("Interact", |_lua, this, (auto_triggered,): (bool,)| {
            this.borrow().interact(auto_triggered).map_err(to_lua_error)
        });
    }
}

pub struct InstanceTablePoi;
impl<T> UserData for ScriptApiTable<InstanceTablePoi, T>
where
    T: PoiHandleMut + 'static,
    T::Guid: IntoLua,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<(), T>::register_pathable(reg);
        Self::register_poi(reg);
        ScriptApiTable::<(), T>::register_pathable_mut(reg);
        Self::register_poi_mut(reg);
    }
}
impl<T> ScriptApiTable<InstanceTablePoi, T>
where
    T: PoiHandle + 'static,
{
    pub fn register_poi<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("Position", |_lua, this| {
            this.borrow()
                .get_pos()
                .map_err(to_lua_error)
                .map(InstanceVec3::into_lua_vec3)
        });
        reg.add_field_method_get("RotationXyz", |_lua, this| {
            this.borrow()
                .get_rot_euler()
                .map_err(to_lua_error)
                .map(InstanceVec3::into_lua_vec3)
        });
        reg.add_method("GetAttrByKey", |lua, this, (key,): (BorrowedStr<'_>,)| {
            super::AttrRegistration::for_key(&key)
                .ok_or_else(|| to_lua_error(format_err!("unrecognized poi attribute {key}")))
                .and_then(|attr| {
                    Borrow::<T>::borrow(&*this)
                        .get_marker_attr_dyn(attr.attr())
                        .map_err(to_lua_error)
                        .and_then(move |v| v.map(|v| attr.to_lua_dyn(&v, lua)).transpose())
                })
        });
        #[cfg(todo)]
        for (i, attr) in super::attributes::POI_ATTRS[..].iter().enumerate() {
            reg.add_field_method_get(attr.index(), move |lua, this| {
                let attr = unsafe { super::attributes::POI_ATTRS[..].get_unchecked(i) };
                Borrow::<T>::borrow(&*this)
                    .get_marker_attr_dyn(attr.attr())
                    .map_err(to_lua_error)
                    .and_then(move |v| v.map(|v| attr.to_lua_dyn(&v, lua)).transpose())
            });
        }
    }
}
impl<T> ScriptApiTable<InstanceTablePoi, T>
where
    T: PoiHandleMut + 'static,
{
    pub fn register_poi_mut<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        #[cfg(todo)]
        {
            reg.add_method("SetTexture", |_lua, this, (path,): (LuaString,)| {
                this.borrow().set_pack_texture(path).map_err(to_lua_error)
            });
            reg.add_method("SetPosX", |_lua, this, (v,): (f32,)| {
                this.borrow().set_pos_x(v).map_err(to_lua_error)
            });
            reg.add_method("SetPosY", |_lua, this, (v,): (f32,)| {
                this.borrow().set_pos_y(v).map_err(to_lua_error)
            });
            reg.add_method("SetPosZ", |_lua, this, (v,): (f32,)| {
                this.borrow().set_pos_z(v).map_err(to_lua_error)
            });
            reg.add_method("SetRotX", |_lua, this, (v,): (f32,)| {
                this.borrow().set_rot_x(v).map_err(to_lua_error)
            });
            reg.add_method("SetRotY", |_lua, this, (v,): (f32,)| {
                this.borrow().set_rot_y(v).map_err(to_lua_error)
            });
            reg.add_method("SetRotZ", |_lua, this, (v,): (f32,)| {
                this.borrow().set_rot_z(v).map_err(to_lua_error)
            });
        }
        reg.add_method("SetPos", |lua, this, args: MultiValue| {
            let pos = match args.len() {
                1 => <(IVec3,) as FromLuaMulti>::from_lua_multi(args, lua).map(|(v,)| v),
                _ => <(f32, f32, f32) as FromLuaMulti>::from_lua_multi(args, lua)
                    .map(|(x, y, z)| IVec3(InstanceVec3::new_vec3([x, y, z]))),
            }?;
            this.borrow().set_pos(pos.0).map_err(to_lua_error)
        });
        reg.add_method("SetRot", |lua, this, args: MultiValue| {
            let rot = match args.len() {
                1 => <(IVec3,) as FromLuaMulti>::from_lua_multi(args, lua).map(|(v,)| v),
                _ => <(f32, f32, f32) as FromLuaMulti>::from_lua_multi(args, lua)
                    .map(|(x, y, z)| IVec3(InstanceVec3::new_vec3([x, y, z]))),
            }?;
            this.borrow().set_rot_euler(rot.0).map_err(to_lua_error)
        });
        #[cfg(todo)]
        for (i, attr) in super::attributes::POI_ATTRS[..].iter().enumerate() {
            reg.add_field_method_set(attr.index(), move |lua, this, v: LuaValue| {
                let attr = unsafe { super::attributes::POI_ATTRS[..].get_unchecked(i) };
                attr.from_lua_dyn(v, lua).and_then(|v| {
                    Borrow::<T>::borrow(&*this)
                        .set_marker_attr_dyn(v)
                        .map_err(to_lua_error)
                })
            });
        }
        reg.add_method(
            "SetAttrByKey",
            |lua, this, (key, value): (BorrowedStr<'_>, LuaValue)| {
                super::AttrRegistration::for_key(&key)
                    .ok_or_else(|| to_lua_error(format_err!("unrecognized poi attribute {key}")))
                    .and_then(|attr| attr.from_lua_dyn(value, lua))
                    .and_then(|v| {
                        Borrow::<T>::borrow(&*this)
                            .set_marker_attr_dyn(v)
                            .map_err(to_lua_error)
                    })
            },
        );
    }
}

pub struct InstanceTableTrail;
impl<T> UserData for ScriptApiTable<InstanceTableTrail, T>
where
    T: TrailHandleMut + 'static,
    T::Guid: IntoLua,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<(), T>::register_pathable(reg);
        Self::register_trail(reg);
        ScriptApiTable::<(), T>::register_pathable_mut(reg);
        Self::register_trail_mut(reg);
    }
}
impl<T> ScriptApiTable<InstanceTableTrail, T>
where
    T: TrailHandle + 'static,
{
    pub fn register_trail<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
        T::Guid: IntoLua,
    {
        reg.add_method("GetAttrByKey", |lua, this, (key,): (BorrowedStr<'_>,)| {
            super::AttrRegistration::for_key(&key)
                .ok_or_else(|| to_lua_error(format_err!("unrecognized trail attribute {key}")))
                .and_then(|attr| {
                    Borrow::<T>::borrow(&*this)
                        .get_marker_attr_dyn(attr.attr())
                        .map_err(to_lua_error)
                        .and_then(move |v| v.map(|v| attr.to_lua_dyn(&v, lua)).transpose())
                })
        });
        #[cfg(todo)]
        for (i, attr) in super::attributes::TRAIL_ATTRS[..].iter().enumerate() {
            reg.add_field_method_get(attr.index(), move |lua, this| {
                let attr = unsafe { super::attributes::TRAIL_ATTRS[..].get_unchecked(i) };
                Borrow::<T>::borrow(&*this)
                    .get_marker_attr_dyn(attr.attr())
                    .map_err(to_lua_error)
                    .and_then(move |v| v.map(|v| attr.to_lua_dyn(&v, lua)).transpose())
            });
        }
    }
}
impl<T> ScriptApiTable<InstanceTableTrail, T>
where
    T: TrailHandleMut + 'static,
{
    pub fn register_trail_mut<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_method("SetTexture", |_lua, this, (path,): (LuaString,)| {
            this.borrow().set_pack_texture(path).map_err(to_lua_error)
        });
        reg.add_method("SetPoints", |_lua, this, (points,): (Vec<IVec3>,)| {
            let points = points.into_iter().map(Into::into);
            this.borrow().set_points(points).map_err(to_lua_error)
        });
        reg.add_method(
            "SetAttrByKey",
            |lua, this, (key, value): (BorrowedStr<'_>, LuaValue)| {
                super::AttrRegistration::for_key(&key)
                    .ok_or_else(|| to_lua_error(format_err!("unrecognized trail attribute {key}")))
                    .and_then(|attr| attr.from_lua_dyn(value, lua))
                    .and_then(|v| {
                        Borrow::<T>::borrow(&*this)
                            .set_marker_attr_dyn(v)
                            .map_err(to_lua_error)
                    })
            },
        );
        #[cfg(todo)]
        for (i, attr) in super::attributes::TRAIL_ATTRS[..].iter().enumerate() {
            reg.add_field_method_set(attr.index(), move |lua, this, v: LuaValue| {
                let attr = unsafe { super::attributes::TRAIL_ATTRS[..].get_unchecked(i) };
                attr.from_lua_dyn(v, lua).and_then(|v| {
                    Borrow::<T>::borrow(&*this)
                        .set_marker_attr_dyn(v)
                        .map_err(to_lua_error)
                })
            });
        }
    }
}

pub struct InstanceTableTexture;
impl<T> UserData for ScriptApiTable<InstanceTableTexture, T>
where
    T: InstanceTexture + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_texture(reg);
    }
}
impl<T> ScriptApiTable<InstanceTableTexture, T>
where
    T: InstanceTexture + 'static,
{
    pub fn register_texture<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("Width", |_lua, this| {
            this.borrow().get_size().map(|[w, _]| w).map_err(to_lua_error)
        });
        reg.add_field_method_get("Height", |_lua, this| {
            this.borrow().get_size().map(|[_, h]| h).map_err(to_lua_error)
        });
    }
}

/// can't impl for Arc since external type :<
#[cfg(todo)]
impl UserData for Arc<Pack> {}
impl UserData for PackArc {
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<_, PackArc>::register_world_lookup(reg);
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(match this.name.is_empty() {
                false => this.name.clone(),
                true => "pack".into(),
            })
        });
    }
}
impl UserData for PackCategoryArc {
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<_, PackCategoryArc>::register_category(reg);
        ScriptApiTable::<_, PackCategoryArc>::register_category_mut(reg);
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(this.full_id.clone())
        });
    }
}
impl UserData for PackPoiArc {
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<(), PackPoiArc>::register_pathable(reg);
        ScriptApiTable::<_, PackPoiArc>::register_poi(reg);
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| Ok(this.to_string()));
    }
}
impl UserData for PackTrailArc {
    fn register(reg: &mut UserDataRegistry<Self>) {
        ScriptApiTable::<(), PackTrailArc>::register_pathable(reg);
        ScriptApiTable::<_, PackTrailArc>::register_trail(reg);
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| Ok(this.to_string()));
    }
}
impl IntoLua for MarkerType {
    fn into_lua(self, _lua: &Lua) -> LuaResult<LuaValue> {
        Ok(LuaValue::Integer(match self {
            Self::Poi => 1,
            Self::Trail => 2,
            Self::Category => 3,
        }))
    }
}
impl FromLua for MarkerType {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::Integer(1) => Ok(Self::Poi),
            LuaValue::Integer(2) => Ok(Self::Trail),
            LuaValue::Integer(3) => Ok(Self::Category),
            _ => Err(to_lua_error(format_err!("expected markerty"))),
        }
    }
}
