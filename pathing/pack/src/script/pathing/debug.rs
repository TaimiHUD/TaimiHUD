use {
    crate::script::{
        user::{ScriptUserHandle, ScriptUserStr},
        Result,
    },
    anyhow::Context,
    core::fmt,
    std::io::{self, Write},
};

#[allow(unused_variables)]
pub trait ScriptApiDebugLog {
    fn print<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        self.info(msg)
    }
    fn debug<S: ScriptUserStr>(&self, msg: S) -> Result<()>;
    fn info<S: ScriptUserStr>(&self, msg: S) -> Result<()>;
    fn warn<S: ScriptUserStr>(&self, msg: S) -> Result<()>;
    fn error<S: ScriptUserStr>(&self, msg: S) -> Result<()>;
}
#[allow(unused_variables)]
pub trait ScriptApiDebugWatch {
    fn watch<K, V>(&self, key: K, value: V) -> Result<()>
    where
        K: ScriptUserStr,
        V: ScriptUserHandle,
        V::Handle: fmt::Debug,
    {
        Ok(())
    }
    fn clear_watch<K: ScriptUserStr>(&self, key: K) -> Result<()> {
        Ok(())
    }
}
/// /dev/null
impl ScriptApiDebugLog for () {
    fn debug<S: ScriptUserStr>(&self, _: S) -> Result<()> {
        Ok(())
    }
    fn info<S: ScriptUserStr>(&self, _: S) -> Result<()> {
        Ok(())
    }
    fn warn<S: ScriptUserStr>(&self, _: S) -> Result<()> {
        Ok(())
    }
    fn error<S: ScriptUserStr>(&self, _: S) -> Result<()> {
        Ok(())
    }
}
/// unimplemented
impl ScriptApiDebugWatch for () {}
fn log_lua<S: ScriptUserStr>(filter: &log::LevelFilter, msg: S, level: log::Level) -> Result<()> {
    if *filter < level {
        return Ok(())
    }
    Ok(msg.with_str(|msg| log::log!(level, "{msg}")))
}
impl ScriptApiDebugLog for log::LevelFilter {
    fn debug<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        log_lua(self, msg, log::Level::Debug)
    }
    fn info<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        log_lua(self, msg, log::Level::Info)
    }
    fn warn<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        log_lua(self, msg, log::Level::Warn)
    }
    fn error<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        log_lua(self, msg, log::Level::Error)
    }
}
#[cfg(todo)]
impl ScriptApiDebugLog for log::__private_api::GlobalLogger {}
impl ScriptApiDebugLog for io::Stdout {
    fn info<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        msg.with_str(|msg| writeln!(&mut { self }, "INFO {msg}"))
            .context("Stdout::write")
    }
    fn debug<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        msg.with_str(|msg| writeln!(&mut { self }, "DBG  {msg}"))
            .context("Stdout::write")
    }
    fn warn<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        msg.with_str(|msg| writeln!(&mut { self }, "WARN {msg}"))
            .context("Stdout::write")
    }
    fn error<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        msg.with_str(|msg| writeln!(&mut { self }, "ERR! {msg}"))
            .context("Stdout::write")
    }
}
impl ScriptApiDebugLog for io::Stderr {
    fn debug<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        msg.with_str(|msg| writeln!(&mut { self }, "DBG  {msg}"))
            .context("Stderr::write")
    }
    fn info<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        msg.with_str(|msg| writeln!(&mut { self }, "INFO {msg}"))
            .context("Stderr::write")
    }
    fn warn<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        msg.with_str(|msg| writeln!(&mut { self }, "WARN {msg}"))
            .context("Stderr::write")
    }
    fn error<S: ScriptUserStr>(&self, msg: S) -> Result<()> {
        msg.with_str(|msg| writeln!(&mut { self }, "ERR! {msg}"))
            .context("Stderr::write")
    }
}
