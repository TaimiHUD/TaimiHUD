use {
    crate::script::{
        pathing::{TickSpan, TimeSpan},
        value,
    },
    core::fmt,
    mlua::{MetaMethod, UserData, UserDataFields, UserDataMethods, UserDataRef, UserDataRegistry},
    taimi_hoard::lazyfmt,
};

#[derive(Debug, Copy, Clone)]
pub struct ITimeSpan<D: TimeSpan = value::TimeSpan, T: TickSpan = value::TickSpan> {
    pub ticks: T,
    pub duration: D,
}
impl<D: TimeSpan, T: TickSpan> ITimeSpan<D, T> {
    #[inline]
    pub const fn new(ticks: T, duration: D) -> Self {
        Self { ticks, duration }
    }
}
impl<D, T> UserData for ITimeSpan<D, T>
where
    D: TimeSpan + 'static,
    T: TickSpan + 'static,
{
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("Days", |_lua, this| Ok(this.duration.part_d()));
        reg.add_field_method_get("Hours", |_lua, this| Ok(this.duration.part_h()));
        reg.add_field_method_get("Milliseconds", |_lua, this| Ok(this.duration.part_ms()));
        reg.add_field_method_get("Minutes", |_lua, this| Ok(this.duration.part_m()));
        reg.add_field_method_get("Seconds", |_lua, this| Ok(this.duration.part_s()));
        reg.add_field_method_get("Ticks", |_lua, this| Ok(this.ticks.ticks()));
        reg.add_field_method_get("TotalDays", |_lua, this| Ok(this.duration.total_d_float()));
        reg.add_field_method_get("TotalHours", |_lua, this| Ok(this.duration.total_h_float()));
        reg.add_field_method_get("TotalMilliseconds", |_lua, this| {
            Ok(this.duration.total_ms_float())
        });
        reg.add_field_method_get("TotalMinutes", |_lua, this| Ok(this.duration.total_m_float()));
        reg.add_field_method_get("TotalSeconds", |_lua, this| Ok(this.duration.total_s_float()));

        // TODO: __le? __lt?
        reg.add_meta_function(
            MetaMethod::Eq.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                Ok(lhs.duration.total_ms() == rhs.duration.total_ms())
            },
        );
        reg.add_meta_function(
            MetaMethod::Le.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                Ok(lhs.duration.total_ms() <= rhs.duration.total_ms())
            },
        );
        reg.add_meta_function(
            MetaMethod::Lt.name(),
            |_lua, (lhs, rhs): (UserDataRef<Self>, UserDataRef<Self>)| {
                Ok(lhs.duration.total_ms() < rhs.duration.total_ms())
            },
        );
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(time_span_display(&this.duration).to_string())
        });
    }
}
/// TODO: be consistent and intentional about representation
/// (for debug print and otherwise!)
fn time_span_display<D>(duration: &D) -> impl fmt::Display + '_
where
    D: TimeSpan,
{
    let s = duration.total_s_float();
    lazyfmt::fmt_fn(move |f| match s.abs() {
        sa if sa < 0.1 => write!(f, "{}ms", duration.total_ms()),
        sa if sa < 90.0 => write!(f, "{s:.2}s"),
        _ => write!(f, "{:.2}m", duration.total_m_float()),
    })
}
#[cfg(todo)]
impl<D, T> FromLua for ITimeSpan<D, T>
where
    Self: UserData,
    D: TimeSpan,
    T: TickSpan,
{
    fn from_lua(value: LuaValue, lua: &Lua) -> mlua::prelude::LuaResult<Self> {
        match value {
            #[cfg(todo)]
            LuaValue::Integer(ticks) => Self::from_ticks(ticks),
            LuaValue::UserData(ud) => Ok(ud.borrow()?.clone()),
        }
    }
}
impl UserData for value::GameTime {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field_method_get("ElapsedGameTime", |_lua, this| {
            Ok(ITimeSpan::new(this.elapsed_ticks, this.elapsed.clone()))
        });
        reg.add_field_method_get("TotalGameTime", |_lua, this| {
            Ok(ITimeSpan::new(this.total_ticks, this.total.clone()))
        });
        reg.add_meta_method(MetaMethod::ToString.name(), |_lua, this, ()| {
            Ok(time_span_display(&this.elapsed).to_string())
        });
    }
}
