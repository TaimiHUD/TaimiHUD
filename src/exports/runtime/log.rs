#![allow(dead_code)]

use {
    crate::{
        exports::{self, runtime as rt},
        settings::state::BootstrapState,
    },
    anyhow::Context,
    log::{Level, LevelFilter, Log, Metadata, Record},
    std::{
        error::Error as StdError,
        ffi::CStr,
        fmt,
        fs,
        io,
        mem::transmute,
        path::PathBuf,
        slice,
        sync::{Arc, LazyLock, Mutex, OnceLock, TryLockError},
        time,
    },
};

pub static LOG_FILTER: LazyLock<LogFilter> = LazyLock::new(|| {
    BootstrapState::read_with(|s| {
        s.log_filter
            .as_ref()
            .map(LogFilterDesc::to_filter)
            .unwrap_or_default()
    })
});
pub const RT_FORMAT_ERROR: &'static str = "log formatting failure";
pub const LOG_BUFFER_SIZE: usize = 0x400;

#[cfg(feature = "extension-nexus")]
pub use nexus::log::LogLevel as NexusLogLevel;

pub struct TaimiLog {
    // TODO: fallback to a file in addondir or something
    pub log_file: OnceLock<fs::File>,
    pub log_epoch: OnceLock<time::SystemTime>,
    pub buffer: Mutex<LogBuffer>,
}

impl TaimiLog {
    pub const fn new() -> Self {
        Self {
            log_file: OnceLock::new(),
            log_epoch: OnceLock::new(),
            buffer: Mutex::new(LogBuffer::new()),
        }
    }

    /// Setup fails if logging is already set up, but that's usually fine
    pub fn setup() -> Result<(), log::SetLoggerError> {
        log::set_logger(Self::logger())?;
        log::set_max_level(LOG_FILTER.level());
        Ok(())
    }

    pub fn logger() -> &'static Self {
        static LOGGER: TaimiLog = TaimiLog::new();
        &LOGGER
    }

    const TIMESTAMP_WIDTH: usize = rt::NAME.len() - 3 - 1;
    const TIMESTAMP_MAX: f32 = 10u32.pow(Self::TIMESTAMP_WIDTH as _) as f32;

    pub fn timestamp(&self) -> f32 {
        let epoch = *self.log_epoch.get_or_init(|| time::SystemTime::now());
        time::SystemTime::now()
            .duration_since(epoch)
            .map(|d| d.as_secs_f32() % Self::TIMESTAMP_MAX)
            .unwrap_or(0.0)
    }

    pub fn log_path() -> PathBuf {
        // TODO: log pruning, date in filename, etc?
        // could consider a shared logs folder as an alternative?
        rt::addon_dir().join("taimi.log")
    }

    pub fn ensure_available(&self, requester: &str) {
        if rt::nexus_available() || rt::arcdps_available() {
            return
        }
        if self.log_file.get().is_some() {
            return
        }
        if let Ok(_) = self.open_file() {
            if !requester.is_empty() {
                ::log::info!("taimi.log opened for {requester}");
            }
        }
    }

    pub fn open_file(&self) -> io::Result<&fs::File> {
        let log_path = Self::log_path();
        let append = match crate::built_info::IS_TAGGED_VERSION || crate::built_info::CI_PLATFORM.is_some()
        {
            // prevent log from growing forever in production
            #[cfg(not(debug_assertions))]
            true if fs::metadata(&log_path)
                .map(|md| md.len() > 0x400000)
                .unwrap_or(false) =>
                false,
            _ => true,
        };
        let res = fs::OpenOptions::new().create(true).append(append).open(log_path);
        let mut f = match res {
            Ok(f) => f,
            Err(e) =>
                return match self.log_file.get() {
                    Some(f) => Ok(f),
                    None => Err(e),
                },
        };
        Ok(self.log_file.get_or_init(move || {
            use io::Write as _;
            let ts = time::SystemTime::now()
                .duration_since(time::UNIX_EPOCH)
                .map(|d| d.as_secs_f32())
                .unwrap_or(0.0);
            let _ = write!(f, "{:08.3}; log opened at {}\n", self.timestamp(), ts);

            f
        }))
    }

    pub fn with_log_buffer<R, F: FnOnce(&mut LogBuffer, bool) -> R>(&self, f: F) -> R {
        let mut buffer_storage;
        let mut buffer_lock = self.buffer.try_lock().or_else(|e| match e {
            TryLockError::Poisoned(lock) => Ok(lock.into_inner()),
            TryLockError::WouldBlock => Err(()),
        });
        let persistent = buffer_lock.is_ok();
        let buffer = match &mut buffer_lock {
            Ok(buffer_lock) => match buffer_lock.is_empty() {
                true => {
                    buffer_lock.setup_with_capacity(LOG_BUFFER_SIZE);
                    &mut *buffer_lock
                },
                #[cfg(todo = "unnecessary")]
                false => buffer_lock.append(),
                false => &mut *buffer_lock,
            },
            Err(..) => {
                buffer_storage = LogBuffer::with_capacity(LOG_BUFFER_SIZE / 4);
                &mut buffer_storage
            },
        };
        let res = f(buffer, persistent);
        if let Ok(buffer) = &mut buffer_lock {
            buffer.clear();
        }
        res
    }

    #[allow(unreachable_patterns, dropping_references)]
    pub fn close(&self) {
        let f = match self.log_file.get() {
            Some(f) if f.sync_all().is_ok() => f,
            _ => return,
        };

        // oncelock doesn't leave us many options here but
        // there's little harm in causing further writes/drops from erroring anyway
        #[cfg(not(debug_assertions))]
        let f = match f {
            #[cfg(unix)]
            f => {
                use std::os::fd::{AsRawFd, FromRawFd};
                let fd = f.as_raw_fd();
                unsafe { fs::File::from_raw_fd(fd) }
            },
            #[cfg(windows)]
            f => {
                use std::os::windows::io::{AsRawHandle, FromRawHandle};
                let handle = f.as_raw_handle();
                unsafe { fs::File::from_raw_handle(handle) }
            },
            #[cfg(not(any(unix, windows)))]
            f => f,
        };
        drop(f);
    }

    pub fn flush_all(&self) {
        if let Some(mut f) = self.log_file.get() {
            let _ = io::Write::flush(&mut f);
            let _ = f.sync_data();
        }
    }
}

impl Log for TaimiLog {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        #[cfg(feature = "log-filter")]
        if let filter @ LogFilter::Env(..) = &*LOG_FILTER {
            return filter.enabled(_metadata)
        }
        true
    }

    fn log(&self, record: &Record) {
        use io::Write as _;

        if let Err(e) = log_record(self, record) {
            // what can we do, log the error..?
            if let Some(mut f) = self.log_file.get() {
                let _ = writeln!(f, "unable to log: {e}; {}", record.args());
            }
        }
    }

    fn flush(&self) {
        self.flush_all()
    }
}

pub fn log_record(logger: &TaimiLog, record: &Record) -> rt::RuntimeResult<()> {
    #![allow(unreachable_patterns)]

    let res = logger.with_log_buffer(|buffer, _persistent| -> rt::RuntimeResult<Option<()>> {
        let buffer = buffer.append();
        if !buffer.is_empty() {
            // if anything is left cached from last time, just prepend it to this message
            // TODO: flush this regardless instead, since it's already passed filters even if current record wouldn't!
            let _ = fmt::Write::write_str(buffer, "\n");
        }
        let (message, implicit_target_level) = match () {
            #[cfg(feature = "extension-arcdps")]
            _ if exports::arcdps::log_window_filter(record.metadata()) => {
                let message_bounds = exports::arcdps::log_write_record_buffer(buffer, record)
                    .map_err(|_| RT_FORMAT_ERROR)?;
                let message = buffer.terminate();
                let _ = exports::arcdps::log_window(record.metadata(), message);
                let message = match message_bounds {
                    bounds if bounds.is_empty() => message,
                    bounds => cstr_slice_mut(message, bounds.start..=bounds.end).ok_or(RT_FORMAT_ERROR)?,
                };
                (message, false)
            },
            _ => {
                let implicit_target_level = match () {
                    #[cfg(feature = "extension-nexus")]
                    _ if exports::nexus::available() => true,
                    #[cfg(feature = "extension-arcdps")]
                    _ if exports::arcdps::available() => false,
                    _ => true,
                };
                write_record(buffer, record, implicit_target_level).map_err(|_| RT_FORMAT_ERROR)?;
                let message = buffer.terminate();
                (message, implicit_target_level)
            },
        };
        let res = match () {
            #[cfg(feature = "extension-arcdps")]
            _ => exports::arcdps::log(record.metadata(), &*message).transpose(),
            _ => None,
        };

        #[cfg(feature = "extension-nexus")]
        let res = if exports::nexus::available() {
            let message = match implicit_target_level {
                false =>
                    cstr_slice_from(&*message, LOG_SEGMENT_EXPLICIT_LEN).ok_or("log write truncated?")?,
                true => &*message,
            };

            let res_nexus = exports::nexus::log(record.metadata(), message).transpose();
            match res_nexus {
                Some(Err(e)) if matches!(res, Some(Ok(()))) => Some(Err(e)),
                res_nexus => res.or(res_nexus),
            }
        } else {
            res
        };

        let file = match logger.log_file.get() {
            Some(f) => Some(f),
            None if matches!(res, Some(Ok(()))) && crate::built_info::is_release() => None,
            #[cfg(not(debug_assertions))]
            None if record.metadata().level() > log::Level::Warn => None,
            None => logger.open_file().ok(),
        };

        let res = if let Some(mut file) = file {
            use io::Write as _;
            let timestamp = logger.timestamp();
            let message = {
                // reuse the buffer to avoid race conditions from partial line writes
                let buffer = buffer.append();
                let _ = fmt::Write::write_str(buffer, "\n");
                // and if there's redundant space at the beginning of the line, fill it
                let prefix = match implicit_target_level {
                    false => unsafe { buffer.buffer_mut().get_mut(..LOG_SEGMENT_EXPLICIT_LEN) },
                    true => None,
                };
                if let Some(mut prefix) = prefix {
                    let _ = write!(prefix, "{:08.3}", timestamp);
                }
                buffer.terminate()
            };
            let message = &*message;
            let fres = match implicit_target_level {
                false => Ok(()),
                // if there wasn't space, oh well...
                true => write!(file, "{:08.3};", timestamp),
            };
            let fres = fres
                .and_then(|_| file.write(message.to_bytes()).map(drop))
                .map_err(|_| "log file IO write failed");
            Some(match fres {
                Ok(..) => Ok(()),
                Err(e) if matches!(res, Some(Ok(()))) => Err(e),
                fres => res.unwrap_or(fres),
            })
        } else {
            res
        };

        res.transpose()
    });

    if let Some(res) = res? {
        return Ok(res)
    }

    Err(rt::RT_UNAVAILABLE)
}

pub struct LogBuffer {
    buffer: Vec<u8>,
}

impl LogBuffer {
    pub const fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self { buffer: Vec::with_capacity(cap) }
    }

    pub fn setup_with_capacity(&mut self, cap: usize) {
        match self.buffer.capacity() {
            0 => self.buffer.reserve(cap),
            _ => self.buffer.shrink_to(cap),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn clear(&mut self) {
        self.buffer.clear()
    }

    pub fn terminate(&mut self) -> &mut CStr {
        self.buffer.push(0);
        unsafe { cstr_mut_from_bytes_with_nul_unchecked(self.buffer_mut()) }
    }

    pub fn append(&mut self) -> &mut Self {
        let term = self.buffer.last().copied();
        if term == Some(0) {
            self.buffer.pop();
        }
        self
    }

    pub unsafe fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }
}

impl fmt::Write for LogBuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.buffer.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

pub fn write_record_body<W: fmt::Write>(w: &mut W, record: &Record) -> fmt::Result {
    w.write_fmt(*record.args())
}

pub fn write_record<W: fmt::Write>(w: &mut W, record: &Record, implicit_target_level: bool) -> fmt::Result {
    let prefix_meta = write_metadata_prefix(w, record.metadata(), implicit_target_level)?;
    let prefix_record = write_record_prefix(w, record)?;
    if prefix_meta > 0 || prefix_record > 0 {
        w.write_str(" ")?;
    }
    write_record_body(w, record)?;
    write_record_suffix(w, record)?;
    Ok(())
}

fn strip_crate_root(target: &str) -> Result<&str, Option<&str>> {
    let target = target
        .strip_prefix(rt::CRATE_NAME)
        .map(|target| target.strip_prefix("::").unwrap_or(target))
        .map(Err)
        .unwrap_or(Ok(target));

    match target {
        Err(target) if target.is_empty() => Err(None),
        res => res.map_err(Some),
    }
}

pub fn write_metadata_level<W: fmt::Write>(w: &mut W, meta: &Metadata) -> Result<(), fmt::Error> {
    let level = meta.level().as_str();
    write!(w, "[{level:5}]")
}
const LOG_SEGMENT_LEVEL_LEN_SEP: usize = 2;
const LOG_SEGMENT_LEVEL_LEN: usize = 5 + LOG_SEGMENT_LEVEL_LEN_SEP;

pub fn write_metadata_target<W: fmt::Write>(w: &mut W, meta: &Metadata) -> Result<usize, fmt::Error> {
    let target = match strip_crate_root(meta.target()) {
        Ok(target) => {
            write!(w, "{target}:")?;
            return Ok(target.len() + 1)
        },
        Err(Some(target)) => target,
        Err(None) => return Ok(0),
    };

    write!(w, "::{target};")?;
    let amt = target.len() + LOG_SEGMENT_TARGET_LEN_SEP;
    Ok(amt)
}
const LOG_SEGMENT_TARGET_LEN_SEP: usize = 3;

/// Nexus metadata includes our name and level, so can be omitted
pub fn write_metadata_prefix<W: fmt::Write>(
    w: &mut W,
    meta: &Metadata,
    implicit_target_level: bool,
) -> Result<usize, fmt::Error> {
    let amt_level = if !implicit_target_level {
        w.write_str(rt::NAME)?;
        write_metadata_level(w, meta)?;
        LOG_SEGMENT_EXPLICIT_LEN
    } else {
        0
    };
    let amt_target = write_metadata_target(w, meta)?;
    Ok(amt_level + amt_target)
}
const LOG_SEGMENT_NAME_LEN: usize = rt::NAME.len();
const LOG_SEGMENT_EXPLICIT_LEN: usize = LOG_SEGMENT_NAME_LEN + LOG_SEGMENT_LEVEL_LEN;

pub fn write_record_prefix<W: fmt::Write>(_w: &mut W, _record: &Record) -> Result<usize, fmt::Error> {
    // TODO: any special record.key_values() we want to use here? (requires log/kv)
    Ok(0)
}

pub fn write_record_suffix<W: fmt::Write>(w: &mut W, record: &Record) -> Result<usize, fmt::Error> {
    let amt = 0;
    #[cfg(debug_assertions)]
    let amt = amt
        + match record.module_path().map(strip_crate_root) {
            Some(Ok(module)) | Some(Err(Some(module))) => {
                write!(w, " ({module})")?;
                let amt_mod = module.len() + LOG_SEGMENT_MOD_LEN_SEP;
                let amt_line = if let Some(line) = record.line() {
                    write!(w, ":{line}")?;
                    line.ilog10() as usize + 1 + LOG_SEGMENT_LINE_LEN_SEP
                } else {
                    0
                };
                amt_mod + amt_line
            },
            _ => 0,
        };

    Ok(amt)
}
#[cfg(debug_assertions)]
const LOG_SEGMENT_MOD_LEN_SEP: usize = 2;
const LOG_SEGMENT_LINE_LEN_SEP: usize = 1;

/// XXX: passing a mut borrow (iow not moving cstr into here) would be a bad idea
fn cstr_slice_mut<'a, I>(cstr: &'a mut CStr, range: I) -> Option<&'a mut CStr>
where
    I: slice::SliceIndex<[u8], Output = [u8]>,
{
    let bytes = unsafe {
        // TODO? &mut *(cstr.to_bytes_with_nul() as *const [u8] as *mut [u8])
        transmute::<_, &'a mut [u8]>(cstr)
    };
    let subslice = bytes.get_mut(range)?;
    *subslice.last_mut()? = 0;
    Some(unsafe { cstr_mut_from_bytes_with_nul_unchecked(subslice) })
}

fn cstr_slice_from(cstr: &CStr, from: usize) -> Option<&CStr> {
    let terminated = match cstr.to_bytes_with_nul().get(from..) {
        Some(b) if !b.is_empty() => b,
        _ => return None,
    };

    Some(unsafe { CStr::from_bytes_with_nul_unchecked(terminated) })
}

unsafe fn cstr_mut_from_bytes_with_nul_unchecked(terminated: &mut [u8]) -> &mut CStr {
    // TODO? CStr::from_bytes_with_nul_unchecked(terminated) as *const CStr as *mut CStr
    transmute(terminated)
}

#[cfg(feature = "extension-nexus")]
pub const fn nexus_log_level(level: Level) -> NexusLogLevel {
    match level {
        Level::Trace => NexusLogLevel::Trace,
        Level::Debug => NexusLogLevel::Debug,
        Level::Info => NexusLogLevel::Info,
        Level::Warn => NexusLogLevel::Warning,
        Level::Error => NexusLogLevel::Critical,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum LogFilterDesc {
    Level(LevelFilter),
    Env(String),
}

impl LogFilterDesc {
    pub const DEFAULT: Self = Self::Level(Self::DEFAULT_LEVEL);
    pub const DEFAULT_LEVEL: LevelFilter = match () {
        #[cfg(debug_assertions)]
        () => LevelFilter::Trace,
        #[cfg(not(debug_assertions))]
        () => LevelFilter::Debug,
    };

    pub fn to_filter(&self) -> LogFilter {
        match self {
            Self::Level(level) => LogFilter::Level(*level),
            Self::Env(env) => match &env[..] {
                #[cfg(feature = "log-filter")]
                env => Some(LogFilter::Env(
                    env_logger::Builder::new().parse_filters(env).build(),
                )),
                #[cfg(not(feature = "log-filter"))]
                e if e.eq_ignore_ascii_case("all") => Some(LogFilter::Level(LevelFilter::max())),
                #[cfg(not(feature = "log-filter"))]
                env => match env.parse::<LevelFilter>().context("log-filter feature required") {
                    Err(e) => {
                        log::warn!(logger: DeferredLogger::BEST_EFFORT, "{e:#}");
                        None
                    },
                    Ok(level) => Some(LogFilter::Level(level)),
                },
            }
            .unwrap_or_default(),
        }
    }
}

impl Default for LogFilterDesc {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug)]
pub enum LogFilter {
    Level(LevelFilter),
    #[cfg(feature = "log-filter")]
    Env(env_logger::Logger),
}

impl LogFilter {
    pub const DEFAULT: Self = Self::Level(LogFilterDesc::DEFAULT_LEVEL);

    pub fn level(&self) -> LevelFilter {
        match self {
            Self::Level(filter) => *filter,
            #[cfg(feature = "log-filter")]
            Self::Env(env) => env.filter(),
        }
    }

    pub fn enabled(&self, metadata: &Metadata) -> bool {
        match self {
            Self::Level(filter) => *filter >= metadata.level(),
            #[cfg(feature = "log-filter")]
            Self::Env(env) => env.enabled(metadata),
        }
    }

    #[cfg(todo = "unnecessary")]
    pub fn matches(&self, record: &Record) -> bool {
        match self {
            #[cfg(feature = "log-filter")]
            Self::Env(env) => env.matches(record),
            _ => self.enabled(record.metadata()),
        }
    }
}

impl Default for LogFilter {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub struct DeferredLogger {
    pub sync: bool,
}

impl DeferredLogger {
    pub const BEST_EFFORT: Self = Self { sync: false };
    pub const BLOCKING: Self = Self { sync: true };
}
impl Log for DeferredLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        LogFilter::DEFAULT.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return
        }

        let buffer = match self.sync {
            true => TaimiLog::logger().buffer.lock().map_err(drop),
            false => TaimiLog::logger().buffer.try_lock().map_err(drop),
        };
        if let Ok(mut buffer) = buffer {
            let buffer = &mut *buffer;
            if !buffer.is_empty() {
                let _ = fmt::Write::write_str(buffer, "\n");
            }
            if let Err(_e) = write_record(buffer, record, false) {
                let _ = fmt::Write::write_str(buffer, RT_FORMAT_ERROR);
            }
        }
    }

    fn flush(&self) {
        // TODO: could flush to log file if it happens to be open, but why bother?
    }
}

pub fn log_ok<R, E>(level: Level, res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    match res {
        Ok(res) => Some(res),
        Err(e) => {
            log::log!(level, "{e:#}");
            None
        },
    }
}
#[inline(always)]
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
pub fn info_ok<R, E>(res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    log_ok(Level::Info, res)
}
#[inline]
pub fn warn_ok<R, E>(res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    log_ok(Level::Warn, res)
}
#[inline]
pub fn error_ok<R, E>(res: Result<R, E>) -> Option<R>
where
    E: fmt::Display,
{
    log_ok(Level::Error, res)
}

pub type DynError = dyn StdError + Send + Sync;
/// TODO: update anyhow?
pub fn anyhow_into_box(e: anyhow::Error) -> Box<DynError> {
    match e {
        #[cfg(todo)]
        e => e.into_boxed_dyn_error(),
        e => error_into_box(e),
    }
}
#[inline]
pub fn anyhow_into_arc(e: anyhow::Error) -> Arc<DynError> {
    error_into_arc(anyhow_into_box(e))
}
#[inline]
pub fn error_into_box<E>(e: E) -> Box<DynError>
where
    E: Into<Box<DynError>>,
{
    e.into()
}
pub fn error_into_arc<E>(e: E) -> Arc<DynError>
where
    E: Into<Box<DynError>>,
{
    Arc::from(error_into_box(e))
}
