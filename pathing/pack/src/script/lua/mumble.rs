//! TODO: bring back BlishVec3 goddammit that's probably going to be what makes us split off backcompat

use {
    crate::script::{
        lua::{to_lua_error, ISize2, ITimeSpan, IVec2, IVec3, ScriptApiTable},
        pathing::ScriptApiMumble,
        value::Vec3,
    },
    core::{borrow::Borrow, marker::PhantomData},
    mlua::{IntoLua, UserData, UserDataFields, UserDataRegistry},
};

pub struct GlobalInstanceMumble;
impl<T> UserData for ScriptApiTable<GlobalInstanceMumble, T>
where
    T: ScriptApiMumble + Clone + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_mumble(reg)
    }
}
impl<T> ScriptApiTable<GlobalInstanceMumble, T>
where
    T: ScriptApiMumble + Clone + 'static,
{
    fn register_mumble<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("CurrentMumbleMapName", |lua, this| {
            this.borrow()
                .map_name()
                .map_err(to_lua_error)
                .and_then(|name| name.map(|n| n.into_lua(lua)).transpose())
        });
        reg.add_field_method_get("Tick", |_lua, this| this.borrow().ui_tick().map_err(to_lua_error));
        reg.add_field_method_get("TimeSinceTick", |_lua, this| {
            let this = this.borrow();
            this.ticks_since_ui_tick()
                .and_then(|ticks| this.since_ui_tick().map(|span| (ticks, span)))
                .map(|(ticks, span)| ITimeSpan::new(ticks, span))
                .map_err(to_lua_error)
        });
        reg.add_field_method_get("IsAvailable", |_lua, this| Ok(this.borrow().is_available()));

        #[cfg(todo)]
        {
            // idk I want to wrap the AnyUserData but swap out or the metatable,
            // ideally without stuffing it in a table field to avoid indirection?
            // clone fine for now I guess...
            // or share a singleton otherwise, maybe better than clone...
            reg.add_field_function_get("CurrentMap", |lua, this| {
                let metatable = lua
                    .create_proxy::<ScriptApiTable<MumbleInstanceMap, T>>()?
                    .metatable()?;
                (MetaMethod::Index.name(), |lua, _this, key| {
                    |args: MultiValue| metatable.get(key).call(args)
                })
                    .collect::<LuaTable>()
            });
        }
        reg.add_field_method_get("CurrentMap", |lua, this| {
            ScriptApiTable {
                api: this.borrow().clone(),
                _api: PhantomData::<MumbleInstanceMap>,
            }
            .into_lua(lua)
        });
        reg.add_field_method_get("Info", |lua, this| {
            ScriptApiTable {
                api: this.borrow().clone(),
                _api: PhantomData::<MumbleInstanceInfo>,
            }
            .into_lua(lua)
        });
        reg.add_field_method_get("UI", |lua, this| {
            ScriptApiTable {
                api: this.borrow().clone(),
                _api: PhantomData::<MumbleInstanceUi>,
            }
            .into_lua(lua)
        });
        reg.add_field_method_get("PlayerCamera", |lua, this| {
            ScriptApiTable {
                api: this.borrow().clone(),
                _api: PhantomData::<MumbleInstanceCamera>,
            }
            .into_lua(lua)
        });
        reg.add_field_method_get("PlayerCharacter", |lua, this| {
            ScriptApiTable {
                api: this.borrow().clone(),
                _api: PhantomData::<MumbleInstanceCharacter>,
            }
            .into_lua(lua)
        });
    }
}

pub struct MumbleInstanceMap;
impl<T> UserData for ScriptApiTable<MumbleInstanceMap, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_mumble_map(reg)
    }
}
impl<T> ScriptApiTable<MumbleInstanceMap, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register_mumble_map<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("Id", |_lua, this| this.borrow().map_id().map_err(to_lua_error));
        reg.add_field_method_get("Type", |_lua, this| {
            this.borrow().map_type().map_err(to_lua_error)
        });
        reg.add_field_method_get("IsCompetitiveMode", |_lua, this| {
            this.borrow().map_is_competitive().map_err(to_lua_error)
        });
    }
}

pub struct MumbleInstanceInfo;
impl<T> UserData for ScriptApiTable<MumbleInstanceInfo, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_mumble_info(reg)
    }
}
impl<T> ScriptApiTable<MumbleInstanceInfo, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register_mumble_info<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("BuildId", |_lua, this| {
            this.borrow().game_build().map_err(to_lua_error)
        });
        reg.add_field_method_get("IsGameFocused", |_lua, this| {
            this.borrow().game_focused().map_err(to_lua_error)
        });
    }
}

pub struct MumbleInstanceCamera;
impl<T> UserData for ScriptApiTable<MumbleInstanceCamera, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_mumble_camera(reg)
    }
}
fn blishvec3(v3: Vec3) -> IVec3 {
    use glam::Vec3Swizzles;
    IVec3(v3.xzy())
    //IVec3(v3)
}
impl<T> ScriptApiTable<MumbleInstanceCamera, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register_mumble_camera<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("Position", |_lua, this| {
            this.borrow()
                .camera_position()
                .map(blishvec3)
                .map_err(to_lua_error)
        });
        reg.add_field_method_get("Forward", |_lua, this| {
            this.borrow()
                .camera_forward()
                .map(blishvec3)
                .map_err(to_lua_error)
        });
        reg.add_field_method_get("FieldOfView", |_lua, this| {
            this.borrow().camera_fov().map_err(to_lua_error)
        });
        reg.add_field_method_get("NearPlaneRenderDistance", |_lua, this| {
            this.borrow()
                .camera_clip_planes()
                .map(|depth| depth.start)
                .map_err(to_lua_error)
        });
        reg.add_field_method_get("FarPlaneRenderDistance", |_lua, this| {
            this.borrow()
                .camera_clip_planes()
                .map(|depth| depth.end)
                .map_err(to_lua_error)
        });
        log::warn!("TODO: register_mumble_camera");
    }
}

pub struct MumbleInstanceCharacter;
impl<T> UserData for ScriptApiTable<MumbleInstanceCharacter, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_mumble_character(reg)
    }
}
impl<T> ScriptApiTable<MumbleInstanceCharacter, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register_mumble_character<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("Name", |lua, this| {
            this.borrow()
                .character_name()
                .map_err(to_lua_error)
                .and_then(|name| name.into_lua(lua))
        });
        reg.add_field_method_get("Position", |_lua, this| {
            this.borrow()
                .player_position()
                .map(blishvec3)
                .map_err(to_lua_error)
        });
        reg.add_field_method_get("Forward", |_lua, this| {
            this.borrow()
                .player_forward()
                .map(blishvec3)
                .map_err(to_lua_error)
        });
        reg.add_field_method_get("Race", |_lua, this| {
            this.borrow().player_race().map_err(to_lua_error)
        });
        reg.add_field_method_get("Specialization", |_lua, this| {
            this.borrow().player_spec().map_err(to_lua_error)
        });
        reg.add_field_method_get("TeamColorId", |_lua, this| {
            this.borrow().player_team_colour_id().map_err(to_lua_error)
        });
        reg.add_field_method_get("CurrentMount", |_lua, this| {
            this.borrow()
                .player_mount()
                .map(|mount| mount.map(|m| m.get()).unwrap_or(0))
                .map_err(to_lua_error)
        });
        reg.add_field_method_get("IsCommander", |_lua, this| {
            this.borrow().is_commander().map_err(to_lua_error)
        });
        reg.add_field_method_get("IsInCombat", |_lua, this| {
            this.borrow().is_in_combat().map_err(to_lua_error)
        });
        // TODO: not part of real API?
        reg.add_field_method_get("Profession", |_lua, this| {
            this.borrow().player_profession().map_err(to_lua_error)
        });
    }
}

pub struct MumbleInstanceUi;
impl<T> UserData for ScriptApiTable<MumbleInstanceUi, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        Self::register_mumble_ui(reg)
    }
}
impl<T> ScriptApiTable<MumbleInstanceUi, T>
where
    T: ScriptApiMumble + 'static,
{
    fn register_mumble_ui<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<T>,
    {
        reg.add_field_method_get("CompassRotation", |_lua, this| {
            this.borrow().compass_rotation().map_err(to_lua_error)
        });
        reg.add_field_method_get("CompassSize", |_lua, this| {
            this.borrow()
                .compass_size()
                .map(ISize2::<u32>::from)
                .map_err(to_lua_error)
        });
        reg.add_field_method_get("IsCompassRotationEnabled", |_lua, this| {
            this.borrow().is_compass_rotation_enabled().map_err(to_lua_error)
        });
        reg.add_field_method_get("IsCompassTopRight", |_lua, this| {
            this.borrow().is_compass_top_right().map_err(to_lua_error)
        });
        reg.add_field_method_get("IsMapOpen", |_lua, this| {
            this.borrow().is_map_open().map_err(to_lua_error)
        });
        reg.add_field_method_get("IsTextInputFocused", |_lua, this| {
            this.borrow().is_text_input_focused().map_err(to_lua_error)
        });
        reg.add_field_method_get("MapCenter", |_lua, this| {
            this.borrow().map_centre().map(IVec2).map_err(to_lua_error)
        });
        reg.add_field_method_get("MapPosition", |_lua, this| {
            this.borrow().map_position().map(IVec2).map_err(to_lua_error)
        });
        reg.add_field_method_get("MapScale", |_lua, this| {
            this.borrow().map_scale().map_err(to_lua_error)
        });
        reg.add_field_method_get("UISize", |_lua, this| {
            this.borrow().ui_size().map_err(to_lua_error)
        });
    }
}
