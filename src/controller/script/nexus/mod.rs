#[cfg(feature = "scripts-lua")]
use mlua::{UserData, UserDataFields, UserDataRegistry};
use {
    crate::{
        controller::script::event::{ScriptNotification, ScriptNotificationCategory},
        exports::runtime as rt,
    },
    strum::VariantArray,
};

#[derive(Debug, Clone, Default)]
pub struct ScriptHostNexus {}
impl ScriptHostNexus {
    #[inline]
    pub fn is_available(&self) -> bool {
        rt::nexus_available()
    }
    pub fn host_signals() -> impl Iterator<Item = ScriptNotification> + Send + 'static {
        ScriptNotification::VARIANTS
            .iter()
            .filter(|v| v.category() == ScriptNotificationCategory::NexusCallback)
            .copied()
    }
    #[cfg(feature = "scripts-lua")]
    const HOST_SIGNAL_CACHE: &'static str = "tnexus_hostsignals";
}
impl ScriptNotification {
    fn nexus_name(self) -> &'static str {
        let name = self.name();
        name.strip_prefix("Nexus").unwrap_or(name)
    }
}

#[cfg(feature = "scripts-lua")]
impl UserData for ScriptHostNexus {
    fn register(reg: &mut UserDataRegistry<Self>) {
        reg.add_field("supported", true);
        reg.add_field_method_get("available", |_, this| Ok(this.is_available()));
        reg.add_field_function_get("HostSignal", |lua, this| {
            if let Some(v) = this.named_user_value::<Option<mlua::Table>>(Self::HOST_SIGNAL_CACHE)? {
                Ok(v)
            } else if !this.is::<Self>() {
                Err(mlua::Error::UserDataTypeMismatch)
            } else {
                let host_signals = Self::host_signals().map(|v| (v.nexus_name(), v.to_repr()));
                let host_signals = lua.create_table_from(host_signals)?;
                let res = this.set_named_user_value(Self::HOST_SIGNAL_CACHE, &host_signals);
                let _ = rt::log::warn_ok(res);
                Ok(host_signals)
            }
        });
    }
}
