use {
    crate::script::{
        script_unimpl,
        user::{ScriptSourceTag, ScriptUserUntyped},
        Result,
    },
    std::borrow::Cow,
};

pub type SignalId = u32;

#[allow(unused_variables)]
pub trait ScriptApiEvent {
    /// unsubscribe
    fn notifcation_mask(&self, id: SignalId) -> Result<()> {
        script_unimpl!()
    }
    /// subscribe
    fn notifcation_unmask(&self, id: SignalId) -> Result<()> {
        script_unimpl!()
    }
    fn notifcation_oob<S, A>(&self, source: S, msg: NotifyScript<A>) -> Result<()>
    where
        S: ScriptSourceTag,
        A: ScriptUserUntyped,
    {
        script_unimpl!()
    }

    #[cfg(todo)]
    fn notify_script<A>(&self, msg: NotifyScript<A>) -> Result<()>
    where
        A: ScriptUserUntyped,
    {
        script_unimpl!()
    }

    fn all_signals(&self) -> Self::SignalNames;
    fn all_notifications(&self) -> Self::SignalNames;
    type SignalNames: Iterator<Item = (Cow<'static, str>, SignalId)>;
}

#[derive(Debug, Clone)]
pub struct NotifyScript<A = ()> {
    pub id: SignalId,
    pub args: A,
}
impl<A> NotifyScript<A> {
    #[inline]
    pub fn new<I>(id: I, args: A) -> Self
    where
        I: Into<SignalId>,
    {
        Self { id: id.into(), args }
    }
    #[inline]
    pub fn empty<I>(id: I) -> Self
    where
        I: Into<SignalId>,
        A: Default,
    {
        Self::new(id, Default::default())
    }
}

#[derive(Debug, Clone)]
pub struct EventReceiver<A = ()> {
    pub receiver: String,
    pub user_args: A,
}
impl<A> EventReceiver<A> {
    #[inline]
    pub fn new(receiver: String, user_args: A) -> Self {
        Self { receiver, user_args }
    }
    #[inline]
    pub fn empty(receiver: String) -> Self
    where
        A: Default,
    {
        Self::new(receiver, Default::default())
    }
}
