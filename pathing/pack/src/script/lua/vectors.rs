use {
    crate::{
        attributes::keys,
        script::{
            lua::{to_lua_error, RuntimeLua},
            pathing::{InstanceColour, InstanceVec3},
            value::{Colour, Vec2, Vec3},
        },
    },
    core::{
        borrow::{Borrow, BorrowMut},
        result::Result as StdResult,
    },
    glamour::{Scalar, Unit},
    mlua::{
        Error as LuaError,
        FromLua,
        IntoLua,
        Lua,
        MetaMethod,
        Result as LuaResult,
        String as LuaString,
        UserData,
        UserDataFields,
        UserDataMethods,
        UserDataRef,
        UserDataRegistry,
        Value as LuaValue,
    },
    std::sync::Arc,
};

#[derive(Debug, Copy, Clone)]
pub struct IVec3(pub Vec3);
impl IVec3 {
    fn coerce_from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        match &value {
            LuaValue::String(s) if s.as_bytes().contains(&b',') => Self::parse_str(s),
            LuaValue::String(..) | LuaValue::Boolean(..) | LuaValue::Number(..) | LuaValue::Integer(..) =>
                <f32 as FromLua>::from_lua(value, lua).map(Vec3::splat).map(Self),
            _ => IVec3::from_lua(value, lua),
        }
    }
    fn parse_str(s: &LuaString) -> LuaResult<Self> {
        let res = s.to_str().and_then(|s| {
            s[..]
                .parse::<keys::Point3>()
                .map(|v| Self(v.0.into()))
                .map_err(|e| LuaError::ExternalError(Arc::new(e) as Arc<_>))
        });
        mlua::ErrorContext::context(res, "Vector3::from_str")
    }
}
/// TODO: this is supposed to use the trait!
impl UserData for IVec3 {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("X", |_lua, this| Ok(this.0.x));
        reg.add_field_method_get("Y", |_lua, this| Ok(this.0.y));
        reg.add_field_method_get("Z", |_lua, this| Ok(this.0.z));
        reg.add_field_method_set("X", |_lua, this, value: f32| Ok(this.0.set_x(value)));
        reg.add_field_method_set("Y", |_lua, this, value: f32| Ok(this.0.set_y(value)));
        reg.add_field_method_set("Z", |_lua, this, value: f32| Ok(this.0.set_z(value)));
        reg.add_method("Length", |_lua, this, ()| Ok(this.0.vec3_length()));
        reg.add_method("xzy", |_lua, this, ()| {
            use glam::Vec3Swizzles;
            Ok(Self(this.0.xzy()))
        });
        reg.add_method("Normalize", |lua, this, ()| {
            let mut norm = this.0;
            norm.vec3_norm();
            Self(norm).into_lua(lua)
        });
        reg.add_method("Dot", |_lua, this, (rhs,): (UserDataRef<Self>,)| {
            Ok(this.0.vec3_dot(rhs.0))
        });
        reg.add_method("Cross", |lua, this, (rhs,): (UserDataRef<Self>,)| {
            Self(this.0.vec3_cross(rhs.0).into()).into_lua(lua)
        });
        reg.add_meta_method(MetaMethod::Unm.name(), |lua, this, ()| {
            let mut neg = this.0;
            neg.vec3_negate();
            Self(neg).into_lua(lua)
        });
        reg.add_meta_function(MetaMethod::Add.name(), |lua, (lhs, rhs): (LuaValue, LuaValue)| {
            let lhs = IVec3::coerce_from_lua(lhs, lua)?;
            let rhs = IVec3::coerce_from_lua(rhs, lua)?;
            Ok(Self(lhs.0 + rhs.0))
        });
        reg.add_meta_function(MetaMethod::Sub.name(), |lua, (lhs, rhs): (LuaValue, LuaValue)| {
            let lhs = IVec3::coerce_from_lua(lhs, lua)?;
            let rhs = IVec3::coerce_from_lua(rhs, lua)?;
            Ok(Self(lhs.0 - rhs.0))
        });
        reg.add_meta_function(MetaMethod::Mul.name(), |lua, (lhs, rhs): (LuaValue, LuaValue)| {
            let lhs = IVec3::coerce_from_lua(lhs, lua)?;
            let rhs = IVec3::coerce_from_lua(rhs, lua)?;
            Ok(Self(lhs.0 * rhs.0))
        });
        reg.add_meta_function(MetaMethod::Div.name(), |lua, (lhs, rhs): (LuaValue, LuaValue)| {
            let lhs = IVec3::coerce_from_lua(lhs, lua)?;
            let rhs = IVec3::coerce_from_lua(rhs, lua)?;
            Ok(Self(lhs.0 / rhs.0))
        });
        // TODO: __le? __lt?
        reg.add_meta_function(MetaMethod::Eq.name(), |lua, (lhs, rhs): (LuaValue, LuaValue)| {
            let lhs = IVec3::coerce_from_lua(lhs, lua)?;
            let rhs = IVec3::coerce_from_lua(rhs, lua)?;
            Ok(lhs.0.abs_diff_eq(rhs.0, 2e-4))
        });
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(format_args!("{:?}", this.0).to_string())
        });
    }
}
impl IVec3 {
    pub(super) fn register_constructor<U>(reg: &mut UserDataRegistry<U>) {
        reg.add_function("Vec3", |_lua, (x, y, z): (f32, f32, f32)| {
            Ok(Self([x, y, z].into()))
        });
    }
}
impl FromLua for IVec3 {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        value
            .as_userdata()
            .map(|ud| ud.borrow::<Self>().map(|v| v.clone()))
            .unwrap_or_else(|| Err(to_lua_error(anyhow::anyhow!("expected vec3"))))
    }
}
impl From<Vec3> for IVec3 {
    #[inline(always)]
    fn from(v: Vec3) -> Self {
        Self(v)
    }
}
impl From<IVec3> for Vec3 {
    #[inline(always)]
    fn from(IVec3(v): IVec3) -> Self {
        v
    }
}
impl Borrow<Vec3> for IVec3 {
    #[inline(always)]
    fn borrow(&self) -> &Vec3 {
        &self.0
    }
}
impl BorrowMut<Vec3> for IVec3 {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut Vec3 {
        &mut self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IColour<T: ?Sized + InstanceColour = Colour>(pub T);
impl<T: InstanceColour> IColour<T> {
    fn coerce_from_lua(value: LuaValue, lua: &Lua) -> LuaResult<StdResult<Self, f32>>
    where
        Self: FromLua,
    {
        match &value {
            LuaValue::String(s)
                if s.as_bytes().starts_with(&b"#"[..]) || matches!(s.as_bytes().len(), 6 | 8) =>
                Self::parse_str(s).map(Ok),
            &LuaValue::Number(v) => Ok(Err(v as f32)),
            _ => Self::from_lua(value, lua).map(Ok),
        }
    }
    fn parse_str(s: &LuaString) -> LuaResult<Self> {
        let res = s.to_str().and_then(|s| {
            s[..]
                .parse::<keys::Colour>()
                .map(|v| v.0.into())
                .map_err(|e| LuaError::ExternalError(Arc::new(e) as Arc<_>))
        });
        mlua::ErrorContext::context(res, "Colour::from_str")
    }
}
impl<T: InstanceColour> From<glam::Vec4> for IColour<T> {
    #[inline]
    fn from(c: glam::Vec4) -> Self {
        let [r, g, b, a] = c.to_array();
        Self(InstanceColour::new_colour([r as u8, g as u8, b as u8, a as u8]))
    }
}
impl<T: InstanceColour> From<IColour<T>> for glam::Vec4 {
    #[inline]
    fn from(c: IColour<T>) -> Self {
        Self::from(&c)
    }
}
impl<T: InstanceColour> From<&'_ IColour<T>> for glam::Vec4 {
    #[inline]
    fn from(c: &IColour<T>) -> Self {
        let [r, g, b, a] = c.0.get4();
        glam::Vec4::new(r as f32, g as f32, b as f32, a as f32)
    }
}
impl<T: InstanceColour> From<keys::Colour> for IColour<T> {
    #[inline]
    fn from(c: keys::Colour) -> Self {
        (c.0 * 255.0).into()
    }
}
impl<T: InstanceColour> From<IColour<T>> for keys::Colour {
    #[inline]
    fn from(c: IColour<T>) -> Self {
        Self(glam::Vec4::from(c) / 255.0)
    }
}
impl<T> UserData for IColour<T>
where
    T: InstanceColour + PartialEq + Clone + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("R", |_lua, this| Ok(this.0.get_r()));
        reg.add_field_method_get("G", |_lua, this| Ok(this.0.get_g()));
        reg.add_field_method_get("B", |_lua, this| Ok(this.0.get_b()));
        reg.add_field_method_get("A", |_lua, this| Ok(this.0.get_a()));
        reg.add_field_method_set("R", |_lua, this, value: u8| Ok(this.0.set_r(value)));
        reg.add_field_method_set("G", |_lua, this, value: u8| Ok(this.0.set_g(value)));
        reg.add_field_method_set("B", |_lua, this, value: u8| Ok(this.0.set_b(value)));
        reg.add_field_method_set("A", |_lua, this, value: u8| Ok(this.0.set_a(value)));
        reg.add_meta_function(
            MetaMethod::Add.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                let [r, g, b, a] = lhs.0.get4();
                let [r_r, r_g, r_b, r_a] = rhs.0.get4();
                Ok(Self(InstanceColour::new_colour([
                    r.saturating_add(r_r),
                    g.saturating_add(r_g),
                    b.saturating_add(r_b),
                    a.saturating_add(r_a),
                ])))
            },
        );
        reg.add_meta_function(
            MetaMethod::Add.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                let [r, g, b, a] = lhs.0.get4();
                let [r_r, r_g, r_b, r_a] = rhs.0.get4();
                Ok(Self(InstanceColour::new_colour([
                    r.saturating_sub(r_r),
                    g.saturating_sub(r_g),
                    b.saturating_sub(r_b),
                    a.saturating_sub(r_a),
                ])))
            },
        );
        reg.add_meta_function(
            MetaMethod::Mul.name(),
            |lua, (lhs, rhs): (UserDataRef<Self>, LuaValue)| {
                let lhs = glam::Vec4::from(&*lhs);
                Ok(Self::from(match Self::coerce_from_lua(rhs, lua)? {
                    Ok(rhs) => lhs * keys::Colour::from(rhs).0,
                    Err(scalar) => lhs * scalar,
                }))
            },
        );
        reg.add_meta_function(
            MetaMethod::Div.name(),
            |lua, (lhs, rhs): (UserDataRef<Self>, LuaValue)| {
                let lhs = glam::Vec4::from(&*lhs);
                Ok(Self::from(match Self::coerce_from_lua(rhs, lua)? {
                    Ok(rhs) => lhs / keys::Colour::from(rhs).0,
                    Err(scalar) => lhs / scalar,
                }))
            },
        );
        // TODO: __le? __lt?
        reg.add_meta_function(
            MetaMethod::Eq.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| Ok(lhs.0 == rhs.0),
        );
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(format_args!("{}", keys::Colour::from(this.clone())).to_string())
        });
    }
}
impl<T: InstanceColour> IColour<T>
where
    Self: IntoLua,
{
    pub(super) fn register_constructor<U>(reg: &mut UserDataRegistry<U>) {
        reg.add_function("Colour", |_lua, (r, g, b, a): (u8, u8, u8, Option<u8>)| {
            let c4 = [r, g, b, a.unwrap_or(u8::MAX)];
            Ok(Self(InstanceColour::new_colour(c4)))
        });
    }
}
impl<T: InstanceColour + Clone + 'static> FromLua for IColour<T> {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        value
            .as_userdata()
            .map(|ud| ud.borrow::<Self>().map(|v| v.clone()))
            .unwrap_or_else(|| Err(to_lua_error(anyhow::anyhow!("expected colour"))))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IVec2(pub Vec2);
impl UserData for IVec2 {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("X", |_lua, this| Ok(this.0.x));
        reg.add_field_method_get("Y", |_lua, this| Ok(this.0.y));
        reg.add_field_method_set("X", |_lua, this, value: f32| Ok(this.0.x = value));
        reg.add_field_method_set("Y", |_lua, this, value: f32| Ok(this.0.y = value));
        Self::register_vec2(reg);
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(format_args!("{:?}", this.0).to_string())
        });
    }
}
impl IVec2 {
    fn register_vec2<U>(reg: &mut UserDataRegistry<U>)
    where
        U: Borrow<Vec2>,
    {
        reg.add_method("Length", |_lua, this, ()| Ok(this.borrow().length()));
        reg.add_method("Normalize", |lua, this, ()| {
            Self(this.borrow().normalize_or_zero()).into_lua(lua)
        });
        reg.add_method("Dot", |_lua, this, (rhs,): (UserDataRef<Self>,)| {
            let rhs: Vec2 = *(*rhs).borrow();
            Ok(this.borrow().dot(rhs))
        });
        reg.add_meta_function(
            MetaMethod::Add.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                let (lhs, rhs): (Vec2, Vec2) = (*(*lhs).borrow(), *(*rhs).borrow());
                Ok(Self(lhs + rhs))
            },
        );
        reg.add_meta_function(
            MetaMethod::Sub.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                let (lhs, rhs): (Vec2, Vec2) = (*(*lhs).borrow(), *(*rhs).borrow());
                Ok(Self(lhs - rhs))
            },
        );
        reg.add_meta_function(
            MetaMethod::Mul.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                let (lhs, rhs): (Vec2, Vec2) = (*(*lhs).borrow(), *(*rhs).borrow());
                Ok(Self(lhs * rhs))
            },
        );
        reg.add_meta_function(
            MetaMethod::Div.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                let (lhs, rhs): (Vec2, Vec2) = (*(*lhs).borrow(), *(*rhs).borrow());
                Ok(Self(lhs / rhs))
            },
        );
        // TODO: __le? __lt?
        reg.add_meta_function(
            MetaMethod::Eq.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                let (lhs, rhs): (Vec2, Vec2) = (*(*lhs).borrow(), *(*rhs).borrow());
                Ok(lhs.abs_diff_eq(rhs, 2e-4))
            },
        );
    }
    #[cfg(todo)]
    pub(super) fn register_constructor<U>(reg: &mut UserDataRegistry<U>) {
        reg.add_function("Vec2", |_lua, (x, y): (f32, f32)| Ok(Self([x, y].into())));
    }
}
impl FromLua for IVec2 {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        value
            .as_userdata()
            .map(|ud| ud.borrow::<Self>().map(|v| v.clone()))
            .unwrap_or_else(|| Err(to_lua_error(anyhow::anyhow!("expected vec2"))))
    }
}
impl Borrow<Vec2> for IVec2 {
    #[inline(always)]
    fn borrow(&self) -> &Vec2 {
        &self.0
    }
}
impl BorrowMut<Vec2> for IVec2 {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut Vec2 {
        &mut self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ISize2<U: Unit = f32>(pub glamour::Vector2<U>);
impl UserData for ISize2<f32>
where
//U: Unit, U::Scalar: IntoLua + FromLua,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("Width", |_lua, this| Ok(this.0.x));
        reg.add_field_method_get("Height", |_lua, this| Ok(this.0.y));
        reg.add_field_method_set("Width", |_lua, this, value: f32| Ok(this.0.x = value));
        reg.add_field_method_set("Height", |_lua, this, value: f32| Ok(this.0.y = value));
        IVec2::register_vec2(reg);
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(format_args!("{:?}", glamour::Size2::<f32>::from(this.0)).to_string())
        });
    }
}
impl UserData for ISize2<u32>
where
//U: Unit, U::Scalar: IntoLua + FromLua,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("Width", |_lua, this| Ok(this.0.x));
        reg.add_field_method_get("Height", |_lua, this| Ok(this.0.y));
        reg.add_field_method_set("Width", |_lua, this, value: u32| Ok(this.0.x = value));
        reg.add_field_method_set("Height", |_lua, this, value: u32| Ok(this.0.y = value));
        // TODO: IVec2::register_vec2(reg);
        reg.add_meta_function(MetaMethod::Concat.name(), RuntimeLua::imp_concat_tostring);
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(format_args!("{:?}", glamour::Size2::<u32>::from(this.0)).to_string())
        });
    }
}
impl From<Vec2> for ISize2<f32> {
    #[inline]
    fn from(v: Vec2) -> Self {
        Self(v.to_array().into())
    }
}
impl<U: Scalar> From<[U; 2]> for ISize2<U> {
    #[inline]
    fn from(v: [U; 2]) -> Self {
        Self(v.into())
    }
}
impl From<ISize2<f32>> for Vec2 {
    #[inline]
    fn from(v: ISize2<f32>) -> Self {
        v.0.into()
    }
}
impl Borrow<Vec2> for ISize2<f32> {
    #[inline(always)]
    fn borrow(&self) -> &Vec2 {
        unsafe { &*(self.0.as_array() as *const [f32; 2] as *const glam::Vec2) }
    }
}
impl BorrowMut<Vec2> for ISize2<f32> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut Vec2 {
        unsafe { &mut *(self.0.as_array_mut() as *mut [f32; 2] as *mut glam::Vec2) }
    }
}
