use {
    anyhow::Context,
    core::{
        cell::{Cell, OnceCell, RefCell},
        fmt,
        mem,
        ops,
    },
};

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct MaybeFmt<F: ?Sized = FormatterFn>(pub F);
impl<F> MaybeFmt<F> {
    #[inline(always)]
    pub const fn with(f: F) -> Self {
        Self(f)
    }
    #[inline(always)]
    pub const fn new(f: F) -> Self
    where
        F: Fn(&mut fmt::Formatter) -> fmt::Result,
    {
        Self::with(f)
    }
    pub const fn wrap_lazy_fn<'a, R, FN>(f: FN) -> impl Fn(&mut fmt::Formatter) -> fmt::Result + 'a
    where
        FN: Fn() -> R + 'a,
        R: AsRef<str> + Into<String>,
    {
        move |fmt| fmt.write_str(f().as_ref())
    }
    pub const fn lazy_fn<'a, R>(f: F) -> impl fmt::Display + Into<String> + 'a
    where
        F: Fn() -> R + 'a,
        R: AsRef<str> + Into<String>,
    {
        MaybeFmt::new(Self::wrap_lazy_fn::<R, F>(f))
    }
}
impl<F: ?Sized> MaybeFmt<F> {
    #[inline(always)]
    pub const fn from_ref(f: &F) -> &Self {
        unsafe { mem::transmute(f) }
    }
    #[inline(always)]
    pub fn from_mut(f: &mut F) -> &mut Self {
        unsafe { mem::transmute(f) }
    }
    #[inline(always)]
    pub const fn to_ref(&self) -> &MaybeFmt<F> {
        MaybeFmt::from_ref(&self.0)
    }
    #[inline(always)]
    pub fn to_mut(&mut self) -> &mut MaybeFmt<F> {
        MaybeFmt::from_mut(&mut self.0)
    }
    pub const fn as_dyn<'a>(&'a self) -> &'a MaybeFmt<dyn Fn(&mut fmt::Formatter) -> fmt::Result + 'a>
    where
        F: Fn(&mut fmt::Formatter) -> fmt::Result + Sized,
    {
        MaybeFmt::from_ref(&self.0 as &dyn Fn(&mut fmt::Formatter) -> fmt::Result)
    }
    #[cfg(todo)]
    pub const fn to_dyn<'a>(&'a self) -> MaybeFmt<&'a dyn Fn(&mut fmt::Formatter) -> fmt::Result>
    where
        F: Fn(&mut fmt::Formatter) -> fmt::Result + Sized,
    {
        MaybeFmt::from_ref(&self.0 as &dyn Fn(&mut fmt::Formatter) -> fmt::Result)
    }
}
impl MaybeFmt<FormatterFn> {
    /// format `Some(T)` or fallback
    pub const fn fmt_or<T: fmt::Display, F: fmt::Display>(
        v: Option<T>,
        fallback: F,
    ) -> MaybeFmt<impl Fn(&mut fmt::Formatter) -> fmt::Result> {
        MaybeFmt::new(move |f| match &v {
            Some(v) => fmt::Display::fmt(v, f),
            None => fmt::Display::fmt(&fallback, f),
        })
    }
    /// format `Some(T)` or fallback
    pub const fn fmt_ok_or<T: fmt::Display, U: fmt::Display>(
        v: Result<T, U>,
    ) -> MaybeFmt<impl Fn(&mut fmt::Formatter) -> fmt::Result> {
        MaybeFmt::new(move |f| match &v {
            Ok(v) => fmt::Display::fmt(v, f),
            Err(v) => fmt::Display::fmt(v, f),
        })
    }
}
#[inline(always)]
pub const fn fmt_fn<F: Fn(&mut fmt::Formatter) -> fmt::Result>(f: F) -> MaybeFmt<F> {
    MaybeFmt::with(f)
}
#[inline(always)]
pub const fn fmt_once<F: FnOnce(&mut fmt::Formatter) -> fmt::Result>(f: F) -> MaybeFmtOnce<F> {
    MaybeFmtOnce::with(f)
}
#[inline(always)]
pub const fn fmt_mut<F: FnMut(&mut fmt::Formatter) -> fmt::Result>(f: F) -> MaybeFmtMut<F> {
    MaybeFmtMut::with(f)
}
#[inline(always)]
pub const fn fmt_or<T: fmt::Display, F: fmt::Display>(
    v: Option<T>,
    fallback: F,
) -> MaybeFmt<impl Fn(&mut fmt::Formatter) -> fmt::Result> {
    MaybeFmt::fmt_or(v, fallback)
}
#[inline(always)]
pub const fn or_empty<T: fmt::Display>(
    v: Option<T>,
) -> MaybeFmt<impl Fn(&mut fmt::Formatter) -> fmt::Result> {
    fmt_or(v, "")
}
#[inline(always)]
pub const fn or_unavail<T: fmt::Display>(
    v: Option<T>,
) -> MaybeFmt<impl Fn(&mut fmt::Formatter) -> fmt::Result> {
    fmt_or(v, UNAVAILABLE)
}
#[inline(always)]
pub const fn ok_or<T: fmt::Display, U: fmt::Display>(
    v: Result<T, U>,
) -> MaybeFmt<impl Fn(&mut fmt::Formatter) -> fmt::Result> {
    MaybeFmt::fmt_ok_or(v)
}
impl<F: ?Sized> fmt::Display for MaybeFmt<F>
where
    F: Fn(&mut fmt::Formatter) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self.0)(f)
    }
}
impl<F: ?Sized> fmt::Debug for MaybeFmt<F>
where
    F: Fn(&mut fmt::Formatter) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<F> From<F> for MaybeFmt<F>
where
    F: Fn(&mut fmt::Formatter) -> fmt::Result,
{
    fn from(f: F) -> Self {
        Self(f)
    }
}
impl<F> From<MaybeFmt<F>> for String
where
    F: Fn(&mut fmt::Formatter) -> fmt::Result,
{
    fn from(f: MaybeFmt<F>) -> Self {
        f.to_string()
    }
}
impl<F: ?Sized> From<&'_ MaybeFmt<F>> for String
where
    F: Fn(&mut fmt::Formatter) -> fmt::Result,
{
    fn from(f: &MaybeFmt<F>) -> Self {
        f.to_string()
    }
}
pub const UNAVAILABLE: &'static str = "<unavail>";
pub type FormatterFn = fn(&mut fmt::Formatter) -> fmt::Result;
pub fn fmt_unavailable(f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(UNAVAILABLE)
}
pub fn fmt_default<S: Default + AsRef<str>>(f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(S::default().as_ref())
}
impl MaybeFmt<FormatterFn> {
    pub const UNAVAILABLE: Self = Self::with(fmt_unavailable);
}
#[derive(Clone)]
pub struct MaybeFmtMut<F = FormatterFn>(pub RefCell<F>);
impl<F> MaybeFmtMut<F> {
    pub const fn with(f: F) -> Self {
        Self(RefCell::new(f))
    }
    pub const fn new(f: F) -> Self
    where
        F: FnMut(&mut fmt::Formatter) -> fmt::Result,
    {
        Self::with(f)
    }
    pub const fn wrap_lazy_fnmut<'a, R, FN>(
        mut f: FN,
    ) -> impl FnMut(&mut fmt::Formatter) -> fmt::Result + 'a
    where
        FN: FnMut() -> R + 'a,
        R: AsRef<str> + Into<String>,
    {
        move |fmt| fmt.write_str(f().as_ref())
    }
    pub const fn lazy_fnmut<'a, R>(f: F) -> impl fmt::Display + Into<String> + 'a
    where
        F: FnMut() -> R + 'a,
        R: AsRef<str> + Into<String>,
    {
        MaybeFmtMut::new(Self::wrap_lazy_fnmut::<R, F>(f))
    }
}
impl MaybeFmtMut<FormatterFn> {
    pub const UNAVAILABLE: Self = Self::with(fmt_unavailable);
}
impl<F> fmt::Display for MaybeFmtMut<F>
where
    F: FnMut(&mut fmt::Formatter) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(mut fun) = self.0.try_borrow_mut() {
            fun(f)
        } else {
            f.write_str(UNAVAILABLE)
        }
    }
}
impl<F> fmt::Debug for MaybeFmtMut<F>
where
    F: FnMut(&mut fmt::Formatter) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<F> From<F> for MaybeFmtMut<F> {
    fn from(f: F) -> Self {
        Self::with(f)
    }
}
impl<F> From<MaybeFmtMut<F>> for String
where
    F: FnMut(&mut fmt::Formatter) -> fmt::Result,
{
    fn from(f: MaybeFmtMut<F>) -> Self {
        f.to_string()
    }
}
#[repr(transparent)]
pub struct MaybeFmtOnce<F = FormatterFn>(pub Cell<Option<F>>);
impl<F> MaybeFmtOnce<F> {
    pub const EMPTY: Self = Self(Cell::new(None));

    #[inline(always)]
    pub const fn with(f: F) -> Self {
        Self(Cell::new(Some(f)))
    }
    pub const fn new(f: F) -> Self
    where
        F: FnOnce(&mut fmt::Formatter) -> fmt::Result,
    {
        Self::with(f)
    }
    pub const fn wrap_lazy_fnonce<'a, R, FN>(f: FN) -> impl FnOnce(&mut fmt::Formatter) -> fmt::Result + 'a
    where
        FN: FnOnce() -> R + 'a,
        R: AsRef<str> + Into<String>,
    {
        move |fmt| fmt.write_str(f().as_ref())
    }
    pub const fn lazy_fnonce<'a, R>(f: F) -> impl fmt::Display + Into<String> + 'a
    where
        F: FnOnce() -> R + 'a,
        R: AsRef<str> + Into<String>,
    {
        MaybeFmtOnce::new(Self::wrap_lazy_fnonce::<R, F>(f))
    }

    pub fn take(&self) -> Self {
        Self(Cell::new(self.0.take()))
    }
}
impl MaybeFmtOnce<FormatterFn> {
    pub const UNAVAILABLE: Self = Self::with(fmt_unavailable);
}
impl<F> fmt::Display for MaybeFmtOnce<F>
where
    F: FnOnce(&mut fmt::Formatter) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(fun) = self.0.take() {
            fun(f)
        } else {
            f.write_str(UNAVAILABLE)
        }
    }
}
impl<F> fmt::Debug for MaybeFmtOnce<F>
where
    F: FnOnce(&mut fmt::Formatter) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<F> From<F> for MaybeFmtOnce<F> {
    fn from(f: F) -> Self {
        Self::with(f)
    }
}
impl<F> From<MaybeFmtOnce<F>> for String
where
    F: FnOnce(&mut fmt::Formatter) -> fmt::Result,
{
    fn from(f: MaybeFmtOnce<F>) -> Self {
        f.to_string()
    }
}

#[derive(Default, Clone)]
pub struct StrFmt<F> {
    f: F,
    displayed: OnceCell<Box<str>>,
}
impl<F> StrFmt<F> {
    pub const fn new(f: F) -> Self {
        Self { f, displayed: OnceCell::new() }
    }
    pub fn from_str<S: Into<Box<str>>>(s: S) -> Self
    where
        F: Default,
    {
        let s = s.into();
        let this = Self::default();
        let _ = this.displayed.set(s);
        this
    }
}
impl StrFmt<MaybeFmt<FormatterFn>> {
    pub const UNAVAILABLE: Self = Self::new(MaybeFmt::UNAVAILABLE);
}
impl<F> StrFmt<MaybeFmt<F>> {
    pub const fn fmt_f(f: F) -> Self
    where
        F: Fn(&mut fmt::Formatter) -> fmt::Result,
    {
        Self::new(MaybeFmt::with(f))
    }

    pub const fn lazy_f<'a, R>(f: F) -> impl fmt::Display + Into<String> + 'a
    where
        F: Fn() -> R + 'a,
        R: AsRef<str> + Into<String>,
    {
        StrFmt::new(MaybeFmt::lazy_fn(f))
    }
}
impl<F> StrFmt<MaybeFmtOnce<F>> {
    pub const fn fmt_fn(f: F) -> Self
    where
        F: FnOnce(&mut fmt::Formatter) -> fmt::Result,
    {
        Self::new(MaybeFmtOnce::with(f))
    }
    pub const fn lazy_fn<'a, R>(f: F) -> impl fmt::Display + Into<String> + 'a
    where
        F: FnOnce() -> R + 'a,
        R: AsRef<str> + Into<String>,
    {
        StrFmt::new(MaybeFmtOnce::lazy_fnonce(f))
    }
}
impl<F> StrFmt<F>
where
    F: fmt::Display,
{
    pub fn get_str(&self) -> &str {
        self.displayed.get_or_init(|| self.f.to_string().into_boxed_str())
    }
    pub fn try_get_str(&self) -> Option<&str> {
        self.displayed.get().map(|s| &s[..])
    }

    pub fn annotate_result<E, T>(&self, res: impl Context<T, E>) -> anyhow::Result<T> {
        res.with_context(move || self.get_str().to_owned())
    }
    pub fn annotate_err<E>(&self, e: E) -> anyhow::Error
    where
        E: Into<anyhow::Error>,
    {
        e.into().context(self.get_str().to_owned())
    }
}
impl<F> fmt::Display for StrFmt<F>
where
    F: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.get_str())
    }
}
impl<F> fmt::Debug for StrFmt<F>
where
    F: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.f, f)
    }
}
impl<F> AsRef<str> for StrFmt<F>
where
    F: fmt::Display,
{
    fn as_ref(&self) -> &str {
        self.get_str()
    }
}
impl<F> ops::Deref for StrFmt<F>
where
    F: fmt::Display,
{
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.get_str()
    }
}
impl<F> From<F> for StrFmt<F> {
    fn from(f: F) -> Self {
        Self::new(f)
    }
}
impl<F> From<StrFmt<F>> for String
where
    F: fmt::Display,
{
    fn from(f: StrFmt<F>) -> Self {
        f.get_str().into()
    }
}
impl<F> From<StrFmt<F>> for Box<str>
where
    F: fmt::Display,
{
    fn from(f: StrFmt<F>) -> Self {
        f.get_str().into()
    }
}
impl<'a, F> From<&'a StrFmt<F>> for &'a str
where
    F: fmt::Display,
{
    fn from(f: &'a StrFmt<F>) -> Self {
        f.get_str()
    }
}

#[inline(always)]
pub const fn display2debug<'a, T: fmt::Display + 'a>(v: T) -> impl fmt::Debug + 'a {
    DisplayDebug(v)
}
#[inline(always)]
pub const fn debug2display<'a, T: fmt::Debug + 'a>(v: T) -> impl fmt::Display + 'a {
    DebugDisplay(v)
}
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct DisplayDebug<T: ?Sized>(pub T);
impl<T: ?Sized> DisplayDebug<T> {
    #[inline(always)]
    pub const fn from_ref(v: &T) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
impl<T> fmt::Display for DisplayDebug<T>
where
    T: ?Sized + fmt::Display,
{
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
impl<T> fmt::Debug for DisplayDebug<T>
where
    T: ?Sized + fmt::Display,
{
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct DebugDisplay<T: ?Sized>(pub T);
impl<T: ?Sized> DebugDisplay<T> {
    #[inline(always)]
    pub const fn from_ref(v: &T) -> &Self {
        unsafe { mem::transmute(v) }
    }
}
impl<T> fmt::Display for DebugDisplay<T>
where
    T: ?Sized + fmt::Debug,
{
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

#[macro_export]
macro_rules! fmt_args {
    (dyn $($tt:tt)*) => {
        $crate::lazyfmt::fmt_args!(
            $($tt)*
        ).as_dyn()
    };
    (move $($tt:tt)*) => {
        $crate::lazyfmt::lazyfmt! {
            @closureargs(move)(; $($tt)*);
        }
    };
    (* dyn $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::fmt_args! {
            dyn * $fmt$(, $($tt)*)?
        }
    };
    (&dyn mut $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::fmt_args! {
            dyn &mut $fmt$(, $($tt)*)?
        }
    };
    (&dyn ref $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::fmt_args! {
            dyn &ref $fmt$(, $($tt)*)?
        }
    };
    (&dyn $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::fmt_args! {
            dyn &ref $fmt$(, $($tt)*)?
        }
    };
    (*& $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::lazyfmt! {
            @closureargs()(; * $fmt$(, $($tt)*)?);
        }
    };
    (* $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::lazyfmt! {
            @closureargs(move)(; * $fmt$(, $($tt)*)?);
        }
    };
    (&mut $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::lazyfmt! {
            @closureargs()(; &mut $fmt$(, $($tt)*)?);
        }
    };
    (&ref $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::lazyfmt! {
            @closureargs()(; &ref $fmt$(, $($tt)*)?);
        }
    };
    (& $fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::lazyfmt! {
            @closureargs()(; &ref $fmt$(, $($tt)*)?);
        }
    };
    ($fmt:literal$(, $($tt:tt)*)?) => {
        $crate::lazyfmt::lazyfmt! {
            @closureargs(move)(; $fmt$(, $($tt)*)?);
        }
    };
}
#[macro_export]
macro_rules! lazyfmt {
    (move |$f:ident| $expr:expr) => {
        $crate::lazyfmt::lazyfmt! {
            @closure(move)(fmt_fn; |f| $expr);
        }
    };
    (* |$f:ident| $expr:expr) => {
        $crate::lazyfmt::lazyfmt! {
            @closure()(fmt_once; |f| $expr);
        }
    };
    (&mut |$f:ident| $expr:expr) => {
        $crate::lazyfmt::lazyfmt! {
            @closure()(fmt_mut; |f| $expr);
        }
    };
    (&ref |$f:ident| $expr:expr) => {
        $crate::lazyfmt::lazyfmt! {
            @closure()(fmt_fn; |f| $expr);
        }
    };
    (& |$f:ident| $expr:expr) => {
        $crate::lazyfmt::lazyfmt! {
            @closure()(fmt_fn; |f| $expr);
        }
    };
    (|$f:ident| $expr:expr) => {
        $crate::lazyfmt::lazyfmt! {
            @closure()(fmt_fn; |f| $expr);
        }
    };
    (@closure($($preclosure:tt)*)($constructor:ident; |$f:ident| $expr: expr);) => {
        $crate::lazyfmt::$constructor($($preclosure)* |$f: &mut ::core::fmt::Formatter| -> ::core::fmt::Result {
            $expr
        })
    };
    (@closureargs $preclosure:tt (; * $($fmt:tt)+);) => {
        $crate::lazyfmt::lazyfmt! {
            @closure $preclosure (fmt_once; |f| f.write_fmt(format_args!($($fmt)*)));
        }
    };
    (@closureargs $preclosure:tt (; &mut $($fmt:tt)+);) => {
        $crate::lazyfmt::lazyfmt! {
            @closure $preclosure (fmt_mut; |f| f.write_fmt(format_args!($($fmt)*)));
        }
    };
    (@closureargs $preclosure:tt (; &ref $($fmt:tt)+);) => {
        $crate::lazyfmt::lazyfmt! {
            @closure $preclosure (fmt_fn; |f| f.write_fmt(format_args!($($fmt)*)));
        }
    };
    (@closureargs $preclosure:tt (; $fmt:literal $($fmt_arg:tt)*);) => {
        $crate::lazyfmt::lazyfmt! {
            @closure $preclosure (fmt_fn; |f| f.write_fmt(format_args!($fmt $($fmt_arg)*)));
        }
    };
}
pub use {fmt_args, lazyfmt};
