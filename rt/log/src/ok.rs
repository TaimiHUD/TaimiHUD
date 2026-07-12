use {anyhow::Context, core::fmt, log::Level};

/// TODO: wrappers around this should be macros instead
/// so they can pass along `module_path!()` like log usually does
#[track_caller]
pub fn log_ok<R, E>(level: Level, res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    match res {
        Ok(res) => Some(res),
        Err(e) => {
            let target = match () {
                #[cfg(todo)]
                () => format!("{}:{}", caller.file(), caller.column()),
                () if !log::log_enabled!(level) => "::taimi_hud",
                () => {
                    let c = core::panic::Location::caller();
                    let f = c.file();
                    f.strip_suffix(".rs").unwrap_or(f)
                },
            };
            log::log!(target: target, level, "{e:#}");
            None
        },
    }
}
#[inline(always)]
#[track_caller]
pub fn debug_ok_with<C, R, E>(context: C, res: Result<R, E>) -> Option<R>
where
    Result<R, E>: anyhow::Context<R, E>,
    E: Into<anyhow::Error>,
    C: fmt::Display,
{
    if log::log_enabled!(Level::Debug) {
        let res = Context::with_context(res, move || context.to_string());
        log_ok(Level::Debug, res)
    } else {
        res.ok()
    }
}
#[inline(always)]
#[track_caller]
pub fn debug_ok<R, E>(res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    if log::log_enabled!(Level::Debug) {
        log_ok(Level::Debug, res)
    } else {
        res.ok()
    }
}
#[inline]
#[track_caller]
pub fn info_ok<R, E>(res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    log_ok(Level::Info, res)
}
#[inline]
#[track_caller]
pub fn warn_ok<R, E>(res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    log_ok(Level::Warn, res)
}
#[inline]
#[track_caller]
pub fn error_ok<R, E>(res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    log_ok(Level::Error, res)
}
