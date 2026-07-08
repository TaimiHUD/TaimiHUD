use {
    crate::render::{RenderEvent, RenderState},
    core::time::Duration,
    std::borrow::Cow,
    taimi_pack::script::{pathing::ScriptApiUser, user::ScriptUserStr, Result},
};

pub struct ScriptHostUiX {}
impl ScriptHostUiX {
    pub fn new() -> Self {
        Self {}
    }
}
impl ScriptApiUser for ScriptHostUiX {
    fn set_clipboard<S: ScriptUserStr, M: ScriptUserStr>(
        &self,
        value: S,
        message: Option<M>,
    ) -> Result<()> {
        RenderState::try_send(RenderEvent::ClipboardSend(
            value.clone_to_string(),
            message.map(|m| m.clone_to_string()),
        ));
        Ok(())
    }
    fn info_notify<S: ScriptUserStr>(&self, message: S, time: Option<Duration>) -> Result<()> {
        RenderState::try_send(RenderEvent::AlertNotify(message.clone_to_string(), time));
        Ok(())
    }
    fn info_show<S: ScriptUserStr>(&self, message: S) -> Result<String> {
        self.info_notify(message, None).map(|()| String::new())
    }
    fn info_hide<S: ScriptUserStr>(&self, _token: S) -> Result<()> {
        Ok(())
    }
}
