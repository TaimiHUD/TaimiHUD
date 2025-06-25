use std::{convert::Infallible, ffi::{CStr, CString}, fmt::{self, Write}};
use log::{Log, Metadata, Record, Level, LevelFilter};
use crate::exports::{self, runtime as rt};

pub const LOG_LEVEL_FILTER: LevelFilter = match () {
    #[cfg(debug_assertions)]
    () => LevelFilter::Trace,
    #[cfg(not(debug_assertions))]
    () => LevelFilter::Debug,
};

#[cfg(todo)]
pub const LOG_FILTER: &'static str = match () {
    #[cfg(debug_assertions)]
    () => "all",
    #[cfg(not(debug_assertions))]
    () => "debug",
};

pub const RT_FORMAT_ERROR: &'static str = "log formatting failure";
pub const LOG_BUFFER_SIZE: usize = 0x400;

#[cfg(feature = "extension-nexus")]
pub use nexus::log::LogLevel as NexusLogLevel;

pub struct TaimiLog {
    // TODO: fallback to a file in addondir or something
}

impl TaimiLog {
    pub const LOGGER: &'static Self = &Self::new();

    pub const fn new() -> Self {
        Self {}
    }

    /// Setup fails if logging is already set up, but that's usually fine
    pub fn setup() -> Result<(), log::SetLoggerError> {
        log::set_logger(&Self::LOGGER)?;
        log::set_max_level(LOG_LEVEL_FILTER);
        Ok(())
    }

    pub fn with_log_buffer<R, F: FnOnce(&mut LogBuffer) -> R>(f: F) -> R {
        let mut buffer = LogBuffer::with_capacity(LOG_BUFFER_SIZE);
        f(&mut buffer)
    }
}

impl Log for TaimiLog {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if let Err(_e) = log_record(record) {
            // what can we do, log the error..?
        }
    }
    fn flush(&self) {}
}

pub fn log_record(record: &Record) -> rt::RuntimeResult<()> {
    #[cfg(feature = "extension-arcdps")]
    if exports::arcdps::loaded() {
        let res = TaimiLog::with_log_buffer(|buffer| -> rt::RuntimeResult<Option<()>> {
            let message_bounds = exports::arcdps::log_write_record_buffer(buffer, record)
                .map_err(|_| RT_FORMAT_ERROR)?;

            let message = buffer.terminate();
            let _ = exports::arcdps::log_window(record.metadata(), message);
            let message = match message_bounds {
                bounds if bounds.is_empty() => message,
                bounds => unsafe {
                    *buffer.buffer_mut().get_unchecked_mut(bounds.end) = 0;
                    let message = buffer.buffer_mut().get_unchecked(bounds.start..=bounds.end);
                    CStr::from_bytes_with_nul_unchecked(message)
                },
            };
            let res = exports::arcdps::log(record.metadata(), message)?;

            #[cfg(feature = "extension-nexus")]
            let res = if exports::nexus::available() {
                let message = unsafe {
                    // exclude level from nexus logs
                    const PREFIX_LEN: usize = 5 + 3 + rt::CRATE_NAME.len() + 2;
                    let message_bytes = message.to_bytes_with_nul();
                    CStr::from_bytes_with_nul_unchecked(message_bytes
                        .get(PREFIX_LEN..)
                        .unwrap_or(message_bytes)
                    )
                };
                let res_nexus = exports::nexus::log(record.metadata(), message);
                res.or(res_nexus.ok().flatten())
            } else { res };

            Ok(res)
        })?;
        if let Some(res) = res {
            return Ok(res)
        }
    }

    #[cfg(feature = "extension-nexus")]
    if let Some(res) = TaimiLog::with_log_buffer(|b| exports::nexus::log_record_buffer(b, record))? {
        return Ok(res)
    }

    Err(rt::RT_UNAVAILABLE)
}

pub struct LogBuffer {
    buffer: Vec<u8>,
}

impl LogBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn terminate(&mut self) -> &CStr {
        self.buffer.push(0);
        unsafe {
            CStr::from_bytes_with_nul_unchecked(&self.buffer[..])
        }
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

/// Nexus metadata includes target and level, so can be omitted
pub fn write_metadata_prefix<W: fmt::Write>(w: &mut W, meta: &Metadata, implicit_target_level: bool) -> Result<bool, fmt::Error> {
    match implicit_target_level {
        true => (),
        false => {
            let level = meta.level().as_str();
            write!(w, "[{level:5}] ")?;
        },
    };
    let target_written = match meta.target() {
        target if implicit_target_level && target == rt::CRATE_NAME =>
            false,
        target => {
            write!(w, "{target}")?;
            true
        },
    };
    Ok(target_written)
}

pub fn write_record_prefix<W: fmt::Write>(w: &mut W, record: &Record, target_written: bool) -> Result<bool, fmt::Error> {
    #[cfg(debug_assertions)]
    let module_prefix = match target_written {
        true => "::",
        false => "",
    };
    #[cfg(debug_assertions)]
    let target_written = match (record.module_path(), record.line()) {
        (Some(module), Some(line)) => {
            write!(w, "{module_prefix}{module}:{line}")?;
            true
        },
        (Some(module), None) => {
            write!(w, "{module_prefix}{module}")?;
            true
        },
        (None, ..) => target_written,
    };

    if target_written {
        write!(w, "; ")?;
    }
    Ok(target_written)
}

pub fn write_record_body<W: fmt::Write>(w: &mut W, record: &Record) -> fmt::Result {
    w.write_fmt(*record.args())
}

pub fn write_record<W: fmt::Write>(w: &mut W, record: &Record) -> fmt::Result {
    let target_written = write_metadata_prefix(w, record.metadata(), false)?;
    write_record_prefix(w, record, target_written)?;
    write_record_body(w, record)
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
