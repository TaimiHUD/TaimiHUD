use {
    arcffi::cstr::Str0,
    core::{cmp, ffi::c_void, fmt, hash, mem, ops, ptr},
    std::io,
};

pub type LogDecoratorDyn = dyn LogDecorator + Sync + Send + 'static;

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogMessageStyle {
    /// no special processing requested
    #[default]
    Display,
    /// message will be provided as a buffer
    CStr {
        /// if false, metadata (level and target module) should be formatted as a prefix
        implicit_metadata: bool,
    },
    Decorated(LogMessageDecorator),
}
#[derive(Copy, Clone)]
pub struct LogMessageDecorator(pub &'static LogDecoratorDyn);
impl fmt::Debug for LogMessageDecorator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("LogDecorator")
            .field(&(self.0 as *const _))
            .finish()
    }
}
impl PartialEq for LogMessageDecorator {
    #[inline]
    fn eq(&self, rhs: &Self) -> bool {
        ptr::addr_eq(self.0 as *const LogDecoratorDyn, rhs.0 as *const LogDecoratorDyn)
    }
}
impl Eq for LogMessageDecorator {}
impl PartialOrd for LogMessageDecorator {
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        (self.0 as *const LogDecoratorDyn as *const c_void)
            .partial_cmp(&(rhs.0 as *const LogDecoratorDyn as *const c_void))
    }
}
impl Ord for LogMessageDecorator {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        (self.0 as *const LogDecoratorDyn as *const c_void)
            .cmp(&(rhs.0 as *const LogDecoratorDyn as *const c_void))
    }
}
impl hash::Hash for LogMessageDecorator {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        hash::Hash::hash(&(self.0 as *const LogDecoratorDyn as *const c_void), state)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogDecorated {
    pub message_start: usize,
    pub message_end: usize,
    pub length: usize,
}
impl LogDecorated {
    pub const EMPTY: Self = Self::with_length(0);
    #[inline]
    pub const fn with_length(length: usize) -> Self {
        Self::with_message_range(0..length)
    }
    #[inline]
    pub const fn with_message_range(message: ops::Range<usize>) -> Self {
        Self {
            message_start: message.start,
            message_end: message.end,
            length: message.end,
        }
    }
    #[inline]
    pub fn message_range(&self) -> ops::Range<usize> {
        self.message_start..self.message_end
    }
    #[inline(always)]
    pub const fn prefix_len(&self) -> usize {
        self.message_start
    }
    #[inline(always)]
    pub const fn suffix_len(&self) -> usize {
        self.message_end - self.length
    }
}
impl Default for LogDecorated {
    #[inline]
    fn default() -> Self {
        Self::EMPTY
    }
}
pub trait LogWriter {
    fn log_write_fmt(&mut self, args: fmt::Arguments<'_>) -> usize;
    fn log_write_str(&mut self, s: &str) -> usize;
}
impl LogWriter for String {
    fn log_write_fmt(&mut self, args: fmt::Arguments<'_>) -> usize {
        let prev = self.len();
        let _ = fmt::Write::write_fmt(self, args);
        self.len() - prev
    }
    fn log_write_str(&mut self, s: &str) -> usize {
        LogWriter::log_write_str(unsafe { self.as_mut_vec() }, s)
    }
}
impl LogWriter for Vec<u8> {
    fn log_write_fmt(&mut self, args: fmt::Arguments<'_>) -> usize {
        match () {
            #[cfg(todo = "unnecessary")]
            _ => LogWriter::log_write_fmt(IoLogWriter::from_mut(self), args),
            _ => LogWriter::log_write_fmt(unsafe { arcffi::vec_as_string_mut(self) }, args),
        }
    }
    fn log_write_str(&mut self, s: &str) -> usize {
        let res = s.len();
        self.extend_from_slice(s.as_bytes());
        res
    }
}
impl<W: ?Sized + LogWriter> LogWriter for &'_ mut W {
    #[inline]
    fn log_write_fmt(&mut self, args: fmt::Arguments<'_>) -> usize {
        LogWriter::log_write_fmt(*self, args)
    }
    #[inline]
    fn log_write_str(&mut self, s: &str) -> usize {
        LogWriter::log_write_str(*self, s)
    }
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct IoLogWriter<W: ?Sized = dyn io::Write>(pub W);
impl<W: ?Sized> IoLogWriter<W> {
    pub const fn from_mut(w: &mut W) -> &mut Self {
        unsafe { mem::transmute(w) }
    }
    pub const fn new(w: W) -> Self
    where
        W: Sized,
    {
        Self(w)
    }
}
impl IoLogWriter {
    /// TODO: logging errors while writing logs gets tricky!
    #[cfg(debug_assertions)]
    fn write_failure(e: io::Error) {
        eprintln!("log write fail: {e}");
    }
}
struct IoLogWriterInner<'a, W: ?Sized + io::Write> {
    write: &'a mut W,
    amt: usize,
}
impl<W> io::Write for IoLogWriterInner<'_, W>
where
    W: ?Sized + io::Write,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = self.write.write(buf);
        if let &Ok(amt) = &res {
            self.amt += amt;
        }
        res
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let amt = buf.len();
        let res = self.write.write_all(buf);
        if let Ok(()) = &res {
            self.amt += amt;
        }
        res
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.write.flush()
    }
}
impl<W: ?Sized + io::Write> LogWriter for IoLogWriter<W> {
    fn log_write_fmt(&mut self, args: fmt::Arguments<'_>) -> usize {
        let mut writer = IoLogWriterInner { write: &mut self.0, amt: 0usize };
        match io::Write::write_fmt(&mut writer, args) {
            Ok(()) => writer.amt,
            Err(_e) => {
                #[cfg(debug_assertions)]
                IoLogWriter::write_failure(_e);
                0
            },
        }
    }
    fn log_write_str(&mut self, s: &str) -> usize {
        let res = s.len();
        match self.0.write_all(s.as_bytes()) {
            Ok(()) => res,
            Err(_e) => {
                #[cfg(debug_assertions)]
                IoLogWriter::write_failure(_e);
                0
            },
        }
    }
}
#[cfg(todo)]
impl LogWriter for fs::File {}
pub unsafe trait LogDecorator {
    /// SAFETY: range must not lie
    fn write_record_buffer(&self, buffer: &mut dyn LogWriter, record: &log::Record<'_>) -> LogDecorated;
}
pub trait HostedLogs {
    fn log_filter_meta(&self, metadata: &log::Metadata<'_>) -> bool;
    fn log_filter(&self, record: &log::Record<'_>) -> Option<LogMessageStyle> {
        self.log_filter_meta(record.metadata())
            .then(|| self.log_wants_message())
    }
    /// typically because a null-terminated string is needed
    fn log_wants_message(&self) -> LogMessageStyle;
    #[allow(unused_variables)]
    fn log_record(&self, record: &log::Record<'_>, message: Option<&str>) -> bool {
        false
    }
    fn log_record_c(&self, record: &log::Record<'_>, message: Option<&Str0>) -> bool {
        self.log_record(record, message.map(|m| m.as_str()))
    }
}
#[derive(Debug, Copy, Clone)]
pub struct Undecorated;
unsafe impl LogDecorator for Undecorated {
    fn write_record_buffer(&self, buffer: &mut dyn LogWriter, record: &log::Record<'_>) -> LogDecorated {
        LogDecorated::with_length(buffer.log_write_fmt(*record.args()))
    }
}
