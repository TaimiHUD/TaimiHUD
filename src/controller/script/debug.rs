use {
    super::PlugStateData,
    core::fmt,
    std::{borrow::Cow, sync::Arc},
    taimi_pack::script::{
        pathing::{ScriptApiDebugLog, ScriptApiVersion},
        user::{ScriptUserHandle, ScriptUserStr, SourceTag},
        Result,
    },
};

pub struct ScriptHostDebug {
    #[cfg(todo)]
    pub watches: Vec<DebugWatch>,
}
impl ScriptHostDebug {
    pub fn new() -> Self {
        Self {
            #[cfg(todo)]
            watches: Default::default(),
        }
    }
}
impl ScriptApiDebugLog for ScriptHostDebug {
    fn debug<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        Ok(msg.with_str(|s| log::debug!("{s}")))
    }
    fn info<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        Ok(msg.with_str(|s| log::info!("{s}")))
    }
    fn warn<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        Ok(msg.with_str(|s| log::warn!("{s}")))
    }
    fn error<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        Ok(msg.with_str(|s| log::error!("{s}")))
    }
    fn print<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        Ok(msg.with_str(|s| log::info!("{s}")))
    }
}

#[derive(Debug, Clone)]
pub struct ScriptHostVersion;
impl ScriptHostVersion {
    pub fn new() -> Self {
        Self
    }
}
impl ScriptApiVersion for ScriptHostVersion {
    fn taimi_version(&self) -> Cow<'_, str> {
        Cow::Borrowed(crate::rt::CRATE_VERSION)
    }
}

pub type DebugWatches = Arc<rustc_hash::FxHashMap<String, String>>;
/// TODO: unpack table trees into flat CategoryId-likes and iterate like a menu would
/// (avoid IdNameBox or AttrString types though, Box<str> is fine even)
#[cfg(todo)]
pub type DebugWatches = Arc<BTreeMap<Box<IdNameSeg>, serde_json::Value>>;

impl PlugStateData {
    pub fn set_debug_watches<V, W>(&mut self, watches: W)
    where
        W: IntoIterator<Item = (String, V)>,
        V: fmt::Debug,
    {
        self.debug_watches = Arc::new(watches.into_iter().map(|(k, v)| (k, format!("{v:#?}"))).collect());
    }
}
