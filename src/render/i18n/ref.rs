use {
    super::{with_i18n_message, FluentBundle},
    arcffi::cstr::CSlice,
    core::{ffi::CStr, fmt, marker::PhantomData, mem},
    fluent::{FluentArgs, FluentValue},
    std::{borrow::Cow, ffi::CString},
    taimi_ui::im::text::{ImStr, ImStrExt},
};

macro_rules! i18n_fmt {
    ($id:literal) => {
        {
            'with_i18n_: {
                #![allow(unreachable_code)]
                let res = $crate::render::i18n::i18n_ref::<&'static str>($id);
                break 'with_i18n_ res;
                let _ = $crate::fl!(@compile_error; $id);
            }
        }
    };
    ($id:literal => [$($key:ident = $value:expr),+$(,)?]) => {
        {
            // TODO: produce one FluentArgs and pass a ref to it
            'with_i18n_: {
                #![allow(unreachable_code)]
                let res = $crate::render::i18n::I18nRef::<'_, &'static str, _>::from_parts($id,
                    [$(
                        (stringify!($key), $value),
                    )*]
                );
                break 'with_i18n_ res;
                // still check ID at compile time...
                let _ = $crate::fl!(@compile_error; $id, $($key = 0u32),*);
            }
        }
    };
    ($id:literal => $($key:ident = $value:expr),+$(,)?) => {
        {
            // TODO: produce one FluentArgs and pass a ref to it
            'with_i18n_: {
                #![allow(unreachable_code)]
                let res = $crate::render::i18n::I18nRef::<'_, &'static str, _>::with_args_fn($id,
                    $crate::render::i18n::i18n_fmt!(@args(fn): => $(
                        (stringify!($key), $value)
                    ),*)
                );
                break 'with_i18n_ res;
                let _ = $crate::fl!(@compile_error; $id, $($key = 0u32),*);
            }
        }
    };
    ($id:expr => $($key:ident = $value:expr),+$(,)?) => {
        {
            $crate::render::i18n::I18nRef::with_args_fn($id,
                $crate::render::i18n::i18n_fmt!(@args(fn): => $(
                    (stringify!($key), $value)
                ),*)
            )
        }
    };
    ($id:expr => move $($key:ident = $value:expr),+$(,)?) => {
        {
            $crate::render::i18n::I18nRef::with_args_fn($id,
                $crate::render::i18n::i18n_fmt!(@args(fn move): => $(
                    (stringify!($key), $value)
                ),*)
            )
        }
    };
    (@args(fn $($move:tt)?): => $(($key:expr, $value:expr)),+$(,)?) => {
        // TODO: produce one FluentArgs and pass a ref to it
        $($move)? |_b| {
            Some(::fluent::FluentArgs::from_iter([$(
                ($key, Into::<::fluent::FluentValue>::into($value)),
            )*]))
        }
    };
    ($id:expr) => {
        $crate::render::i18n::i18n_ref($id)
    };
}
pub(crate) use i18n_fmt;

pub trait FluentBundleArgs<'a> {
    fn fluent_args<'b>(&'b self, bundle: &'b FluentBundle) -> Option<FluentArgs<'b>>
    where
        'a: 'b;
}
impl<'a, S, A, const N: usize> FluentBundleArgs<'a> for [(&'static str, I18nRef<'a, S, A>); N]
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a> + 'a,
{
    fn fluent_args<'b>(&'b self, bundle: &'b FluentBundle) -> Option<FluentArgs<'b>>
    where
        'a: 'b,
    {
        let mut errors = Vec::new();
        Some(
            self.iter()
                .map(|&(key, ref i18n)| {
                    let id = i18n.id.as_ref();
                    let value = bundle
                        .get_message(id)
                        .and_then(|m| m.value())
                        .map(|p| bundle.format_pattern(p, i18n.args(bundle).as_ref(), &mut errors));
                    let value = value.unwrap_or(Cow::Borrowed(id));
                    for e in errors.drain(..) {
                        log::warn!("TODO: i18n {key} = {id} failed: {e})");
                    }
                    (key, fluent::FluentValue::from(value))
                })
                .collect(),
        )
    }
}
impl<'a, T> FluentBundleArgs<'a> for T
where
    T: for<'b> Fn(&'b FluentBundle) -> Option<FluentArgs<'a>>,
{
    fn fluent_args<'b>(&'b self, bundle: &'b FluentBundle) -> Option<FluentArgs<'b>>
    where
        'a: 'b,
    {
        self(bundle)
    }
}
impl<'a> FluentBundleArgs<'a> for () {
    fn fluent_args<'b>(&'b self, _: &'b FluentBundle) -> Option<FluentArgs<'b>>
    where
        'a: 'b,
    {
        None
    }
}
#[cfg(todo = "unnecessary")]
fn no_args<'a, 'b>(_: &'b FluentBundle) -> Option<FluentArgs<'a>> {
    None
}
#[derive(Copy, Clone)]
pub struct I18nRef<'a, S = &'static str, A: 'a = ()> {
    id: S,
    args: A,
    _args: PhantomData<&'a A>,
}
#[cfg(todo = "unnecessary")]
pub fn i18n_ref<'a, S: AsRef<str>>(id: S) -> I18nRef<'a, S, impl FluentBundleArgs<'a> + Copy> {
    I18nRef::from_parts(id, no_args)
}
pub fn i18n_ref<'a, S: AsRef<str>>(id: S) -> I18nRef<'a, S> {
    I18nRef::new(id)
}
impl<'a, S> I18nRef<'a, S> {
    #[inline(always)]
    pub const fn new(id: S) -> Self {
        Self::from_parts(id, ())
    }
}
impl<'a, S, A: 'a> I18nRef<'a, S, A> {
    #[inline(always)]
    pub const fn from_parts(id: S, args: A) -> Self {
        Self { id, args, _args: PhantomData }
    }
    #[inline(always)]
    pub const fn id_name(&self) -> &S {
        &self.id
    }
}
impl<'a, S, A> I18nRef<'a, S, A>
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    pub const fn with_args_fn(id: S, args: A) -> Self
    where
        A: for<'b> Fn(&'b FluentBundle) -> Option<FluentArgs<'a>>,
    {
        Self::from_parts(id, args)
    }
    #[inline]
    pub fn into_string(&self) -> String {
        self.with_str(|s| s.into())
    }
    #[inline]
    pub fn with_str<R, F>(&self, f: F) -> R
    where
        F: FnOnce(Cow<str>) -> R,
    {
        let id = self.id.as_ref();
        with_i18n_message(id, |m, errors| {
            let msg = m.and_then(|(m, b)| {
                m.value()
                    .map(|p| b.format_pattern(p, self.args(b).as_ref(), errors))
            });
            match msg {
                Some(msg) => f(msg),
                None => f(Cow::Borrowed(id)),
            }
        })
    }
    #[inline]
    pub fn args<'b>(&'b self, b: &'b FluentBundle) -> Option<FluentArgs<'b>>
    where
        'a: 'b,
    {
        self.args.fluent_args(b)
    }
}
impl<'a, S, A: 'a> fmt::Display for I18nRef<'a, S, A>
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let id = self.id.as_ref();
        let res = with_i18n_message(id, |m, errors| {
            m.and_then(|(m, b)| {
                #[cfg(todo)]
                let is_fallback = b.locales.get(0) != Some(super::current_language());
                m.value()
                    .map(|p| b.write_pattern(f, p, self.args(b).as_ref(), errors))
            })
        });
        match res {
            Some(res) => res,
            None => f.write_str(id),
        }
    }
}
/// TODO: a debug iter trait for args
impl<'a, S, A: 'a> fmt::Debug for I18nRef<'a, S, A>
where
    S: fmt::Debug,
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("I18nRef")
            .field(&self.id)
            .field(&self.args)
            .finish()
    }
}
impl<'a, S, A> From<I18nRef<'a, S, A>> for String
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    fn from(i18n: I18nRef<'a, S, A>) -> Self {
        i18n.into_string()
    }
}
impl<'a, S, A> From<&'_ I18nRef<'a, S, A>> for String
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    fn from(i18n: &I18nRef<'a, S, A>) -> Self {
        i18n.into_string()
    }
}
impl<'a, S, A> From<I18nRef<'a, S, A>> for Cow<'a, str>
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    fn from(i18n: I18nRef<'a, S, A>) -> Self {
        Cow::Owned(i18n.into_string())
    }
}
impl<'a, S, A> From<&'_ I18nRef<'a, S, A>> for Cow<'a, str>
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    fn from(i18n: &I18nRef<'a, S, A>) -> Self {
        Cow::Owned(i18n.into_string())
    }
}
impl<'a, S, A> From<&'_ I18nRef<'a, S, A>> for FluentValue<'_>
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    fn from(i18n: &I18nRef<'a, S, A>) -> Self {
        i18n.into_string().into()
    }
}
impl<'a, S, A> From<I18nRef<'a, S, A>> for FluentValue<'_>
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    fn from(i18n: I18nRef<'a, S, A>) -> Self {
        i18n.into_string().into()
    }
}
impl<'a, S, A> ImStr for I18nRef<'a, S, A>
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        None
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        None
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        None
    }

    fn im_append_to(&self, dest: &mut Vec<u8>) {
        use std::io::Write;
        struct WriteAdapter(Vec<u8>);
        impl WriteAdapter {
            #[inline(always)]
            fn from_mut(w: &mut Vec<u8>) -> &mut Self {
                unsafe { mem::transmute(w) }
            }
        }
        impl fmt::Write for WriteAdapter {
            fn write_fmt(&mut self, f: fmt::Arguments) -> fmt::Result {
                match self.0.write_fmt(f) {
                    #[cfg(debug_assertions)]
                    Err(..) => Err(fmt::Error),
                    _ => Ok(()),
                }
            }
            fn write_str(&mut self, s: &str) -> fmt::Result {
                match self.0.write_all(s.as_bytes()) {
                    #[cfg(debug_assertions)]
                    Err(..) => Err(fmt::Error),
                    _ => Ok(()),
                }
            }
        }
        let w = WriteAdapter::from_mut(dest);
        let _ = fmt::Write::write_fmt(w, format_args!("{self}"));
    }

    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        Cow::Owned(self.im_take_cstring_owned())
    }
    fn im_take_cstring_owned(&mut self) -> CString {
        unsafe { CString::from_vec_unchecked(self.into_string().into()) }
    }

    #[inline]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.im_clone_to_string().into()
    }

    #[inline]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(self)
    }

    #[inline]
    fn im_clone_to_string(&self) -> String {
        self.into_string()
    }
}
/// TODO: actual wrapper around str and variants that assumes null-termination is very unlikely
impl<'a, S, A> ImStrExt for I18nRef<'a, S, A>
where
    S: AsRef<str>,
    A: FluentBundleArgs<'a>,
{
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        *s
    }
    type IntoImStr = Self;
    #[inline(always)]
    fn im_into_imstr(self) -> Self::IntoImStr {
        self
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        self.with_str(|s| f(&mut { s }))
    }

    #[inline(always)]
    fn im_with_bstr<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        self.with_str(|s| f(s.as_bytes()))
    }
    #[inline(always)]
    fn im_with_cbstr<R, F>(self, f: F) -> R
    where
        F: FnOnce(Result<&CSlice, &[u8]>) -> R,
    {
        self.with_str(|s| f(Err(s.as_bytes())))
    }
}
