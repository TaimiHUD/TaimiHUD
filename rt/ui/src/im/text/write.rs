use {
    crate::im::prelude::*,
    core::{
        borrow::BorrowMut,
        fmt::{self, Write},
        marker::PhantomData,
        mem,
    },
    std::io,
};

/// TODO: reimplement or augment using FnWriteSink?
#[derive(Debug)]
pub struct UiTextWrite<'a, U: ?Sized + ImDrawText + 'a> {
    pub ui: &'a mut U,
    pub start_of_line: Option<ImPos2>,
}
impl<'a, 'ui, U> UiTextWrite<'a, U>
where
    U: ?Sized + ImDrawText + ImDrawItemStack<'ui> + 'a,
{
    #[inline(always)]
    pub const fn new(ui: &'a mut U) -> Self {
        Self { ui, start_of_line: None }
    }
    #[inline]
    pub fn append(&mut self, s: &str) {
        if s.is_empty() {
            return
        }
        let mut _spacingtoken = None;
        if let Some(next_start) = self.start_of_line.take() {
            match next_start {
                s if s.x.is_infinite() => {
                    _spacingtoken = Some(self.ui.push_style_item_spacing(ImVec2::new(f32::EPSILON, 0.0)));
                    self.ui.same_line();
                },
                s => self.ui.set_cursor_pos(s),
            }
        }
        let plain = s.strip_suffix("\n");
        let endln = plain.is_some();
        let can_resume = !endln && !s.as_bytes().contains(&b'\n');
        let prev_start = can_resume.then(|| self.ui.cursor_pos());
        self.ui.text(plain.unwrap_or(s));
        if let Some(prev_start) = prev_start {
            self.start_of_line =
                Some(prev_start + self.ui.item_rect_size().with_height(0.0).to_vector().cast());
        } else if !endln {
            self.start_of_line = Some(ImPos2::INFINITY);
        }
    }
    pub fn end_line(&mut self) {
        match self.start_of_line {
            None => self.ui.new_line(),
            Some(..) => self.start_of_line = None,
        }
    }
    #[inline]
    pub fn draw_line(&mut self, s: &str) {
        self.ui.text(s);
        self.start_of_line = None;
    }
}
impl<'a, 'ui, U> fmt::Write for UiTextWrite<'a, U>
where
    U: ?Sized + ImDrawText + ImDrawItemStack<'ui> + 'a,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Ok(self.append(s))
    }
}
#[derive(Debug)]
pub struct UiText<
    'ui,
    'a,
    U: ?Sized + ImDrawText + ImDrawItemStack<'ui> + 'a,
    B: BorrowMut<String> = String,
> {
    pub text: UiTextWrite<'a, U>,
    pub wrap: bool,
    pub buffer: B,
    pub _arena: PhantomData<&'ui ()>,
}
impl<'a, 'ui, U> UiText<'ui, 'a, U>
where
    U: ?Sized + ImDrawText + ImDrawItemStack<'ui> + 'a,
{
    #[inline]
    pub fn new_wrapped(ui: &'a mut U) -> Self {
        Self::new(ui, true)
    }
}
impl<'a, 'ui, U, B> UiText<'ui, 'a, U, B>
where
    U: ?Sized + ImDrawText + ImDrawItemStack<'ui> + 'a,
    B: BorrowMut<String>,
{
    #[inline]
    pub fn new(ui: &'a mut U, wrap: bool) -> Self
    where
        B: Default,
    {
        Self {
            text: UiTextWrite::new(ui),
            wrap,
            buffer: B::default(),
            _arena: PhantomData,
        }
    }
    #[inline]
    pub fn append_buffer<S: AsRef<str>>(&mut self, s: S) {
        self.buffer.borrow_mut().push_str(s.as_ref());
    }
    #[inline]
    pub fn append(&mut self, s: &str) {
        if s.is_empty() {
            return
        }
        let buffer = self.buffer.borrow_mut();
        let (text, rest) = match self.wrap {
            true => s.rsplit_once("\n").unzip(),
            false => {
                if !buffer.is_empty() {
                    self.text.append(&buffer[..]);
                    buffer.clear();
                    self.text.append(&s);
                }
                (None, Some(""))
            },
        };
        if let Some(mut text) = text {
            if !buffer.is_empty() {
                buffer.push_str(text);
                text = &buffer[..];
            }
            self.text.ui.text_wrapped(text);
            buffer.clear();
        }
        let rest = rest.unwrap_or(s);
        buffer.push_str(rest);
    }
    pub fn display<D: fmt::Display>(&mut self, s: D) {
        let _res = write!(self, "{s}");
        debug_assert!(_res.is_ok());
    }
    pub fn end_line(&mut self) {
        self.flush_buffer();
        self.text.end_line();
    }
    pub fn flush_buffer(&mut self) {
        self.draw_buffer();
        self.buffer.borrow_mut().clear();
    }
    pub fn draw_buffer(&mut self) {
        let buffer = self.buffer.borrow_mut();
        if !buffer.is_empty() {
            match self.wrap {
                true => self.text.ui.text_wrapped(&buffer[..]),
                false => self.text.append(&buffer[..]),
            }
        }
    }
}
impl<'a, 'ui, U, B> fmt::Write for UiText<'ui, 'a, U, B>
where
    U: ?Sized + ImDrawText + ImDrawItemStack<'ui> + 'a,
    B: BorrowMut<String>,
{
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Ok(self.append(s))
    }
}
impl<'a, 'ui, U, B> io::Write for UiText<'ui, 'a, U, B>
where
    U: ?Sized + ImDrawText + ImDrawItemStack<'ui> + 'a,
    B: BorrowMut<String>,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = match buf {
            #[cfg(debug_assertions)]
            buf => str::from_utf8(buf).unwrap(),
            #[cfg(not(debug_assertions))]
            buf => unsafe { str::from_utf8_unchecked(buf) },
        };
        self.append(s);
        Ok(s.len())
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(self.flush_buffer())
    }
}
impl<'a, 'ui, U, B> Drop for UiText<'ui, 'a, U, B>
where
    U: ?Sized + ImDrawText + ImDrawItemStack<'ui> + 'a,
    B: BorrowMut<String>,
{
    fn drop(&mut self) {
        self.draw_buffer();
    }
}

#[repr(transparent)]
pub struct FnWriteSink<F>(F);
impl<F> FnWriteSink<F> {
    #[inline(always)]
    pub fn from_mut(f: &mut F) -> &mut Self {
        unsafe { mem::transmute(f) }
    }
}
impl<F> fmt::Write for FnWriteSink<F>
where
    for<'a> F: FnMut(&'a str),
{
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let Self(f) = self;
        Ok(f(s))
    }
}
