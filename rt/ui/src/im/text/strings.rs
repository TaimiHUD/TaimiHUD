use {
    super::FnWriteSink,
    arcffi::cstr::{CSlice, CStrRef},
    core::{ffi::CStr, fmt, mem},
    num_traits::AsPrimitive,
    std::{borrow::Cow, ffi::CString, io},
};

/// TODO: move most of this to [ImStrExt]?
pub trait ImStr {
    fn im_as_str(&self) -> Option<&str>;
    #[inline]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        self.im_as_str().map(|s| s.as_bytes())
    }
    fn im_append_to(&self, dest: &mut Vec<u8>);
    fn im_as_c_str(&self) -> Option<&CStr> {
        self.im_as_bstr().and_then(|s| match s.last() {
            Some(0u8) => Some(unsafe { CStr::from_bytes_with_nul_unchecked(s) }),
            _ => None,
        })
    }
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display>;
    fn im_take_cstring(&mut self) -> Cow<'_, CStr>;
    #[inline]
    fn im_take_cstring_owned(&mut self) -> CString {
        self.im_take_cstring().into_owned()
    }
    /// assumes other avenues have already been exhausted, such as
    /// [Self::im_as_str] or [Self::im_as_bstr] etc
    fn im_clone_to_string(&self) -> String;
    fn im_clone_to_vec(&self) -> Vec<u8>;
    #[inline]
    fn im_as_id_ptr(&self) -> Option<usize> {
        None
    }
    #[inline]
    fn im_as_id32(&self) -> Option<u32> {
        self.im_as_id_ptr().map(|p| p as u32)
    }
}
pub trait ImStrExt {
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display;

    type IntoImStr: Sized + ImStr + ImStrExt
    where
        Self: Sized;
    fn im_into_imstr(self) -> Self::IntoImStr
    where
        Self: Sized;
    #[inline(always)]
    fn im_as_imstr<'a>(&'a self) -> <&'a Self as ImStrExt>::IntoImStr
    where
        &'a Self: ImStrExt,
    {
        ImStrExt::im_into_imstr(self)
    }
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R;
    #[inline(always)]
    fn im_into_string(self) -> String
    where
        Self: Sized,
    {
        self.im_into_imstr().im_clone_to_string()
    }
    #[inline(always)]
    fn im_with_cstr<R, F>(self, f: F) -> R
    where
        Self: Sized,
        F: FnOnce(&CStr) -> R,
    {
        let mut s = self.im_into_imstr();
        f(&s.im_take_cstring())
    }
    /// TODO: just switch to two "bstr" variant methods on the trait so this can be skipped on cstr...
    #[inline(always)]
    fn im_with_bstr<R, F>(self, f: F) -> R
    where
        Self: Sized,
        F: FnOnce(&[u8]) -> R,
    {
        let s = self.im_into_imstr();
        match s.im_as_bstr() {
            Some(bytes) => f(bytes),
            None => f(&ImStr::im_clone_to_vec(&s)),
        }
    }
    #[inline(always)]
    fn im_with_cbstr<R, F>(self, f: F) -> R
    where
        Self: Sized,
        F: FnOnce(Result<&CSlice, &[u8]>) -> R,
    {
        let s = self.im_into_imstr();
        let bstr;
        let cbstr = if let Some(c) = s.im_as_c_str() {
            Ok(CSlice::with_cstr(c))
        } else {
            let bstr = if let Some(b) = s.im_as_bstr() {
                b
            } else {
                bstr = s.im_clone_to_vec();
                &bstr[..]
            };
            match bstr.last() {
                Some(0) => Ok(unsafe { CSlice::from_bytes_with_nul_unchecked(bstr) }),
                _ => Err(bstr),
            }
        };
        f(cbstr)
    }
}
impl dyn ImStr {
    pub fn display_to<S: ?Sized + ImStr + ImStrExt, F: FnMut(&str)>(s: &S, f: &mut F) {
        let fn_writer = FnWriteSink::from_mut(f);
        Self::write_to(fn_writer, s)
    }
    #[inline(always)]
    pub fn write_to<S: ?Sized + ImStrExt, W: ?Sized + fmt::Write>(dest: &mut W, s: &S)
    where
    //for<'a> &'a S: ImStrExt,
    {
        let _ = write!(dest, "{}", ImStrExt::im_as_display(&s));
    }
    #[inline(always)]
    pub fn with_bstr<R, S: ImStrExt, F: FnOnce(&[u8]) -> R>(s: S, f: F) -> R {
        s.im_with_bstr(f)
    }
    #[inline(always)]
    #[cfg(todo)]
    pub fn with_bstr<R, S: ?Sized + ImStr, F: FnOnce(&[u8]) -> R>(s: &S, f: F) -> R {
        match s.im_as_bstr() {
            Some(bytes) => f(bytes),
            None => f(&S::im_clone_to_vec(s)),
        }
    }
    #[inline(always)]
    #[cfg(todo)]
    pub fn with_cbstr<R, S: ?Sized + ImStr, F: FnOnce(Result<&CSlice, &[u8]>) -> R>(s: &S, f: F) -> R {
        Self::with_bstr(s, move |s| {
            f(match s.last() {
                Some(0) => Ok(unsafe { CSlice::from_bytes_with_nul_unchecked(s) }),
                _ => Err(s),
            })
        })
    }
    #[inline(always)]
    pub fn with_cbstr<R, S: ImStrExt, F: FnOnce(Result<&CSlice, &[u8]>) -> R>(s: S, f: F) -> R {
        s.im_with_cbstr(f)
    }
    #[inline(always)]
    #[cfg(todo)]
    pub fn with_cstr<R, S: ImStr, F: FnOnce(&CStr) -> R>(mut s: S, f: F) -> R {
        f(&s.im_take_cstring())
    }
    #[inline(always)]
    pub fn with_cstr<R, S: ImStrExt, F: FnOnce(&CStr) -> R>(s: S, f: F) -> R {
        s.im_with_cstr(f)
    }
    /// default implementation for [ImStr::im_clone_to_string]
    #[inline]
    pub fn fallback_clone_to_string<S: ?Sized>(s: &S) -> String
    where
        S: ImStr + ImStrExt,
    {
        S::im_as_display(&s).to_string()
    }
    /// default implementation for [ImStr::im_clone_to_vec]
    #[inline]
    pub fn fallback_clone_to_vec<S: ?Sized>(s: &S) -> Vec<u8>
    where
        S: ImStr + ImStrExt,
    {
        match s {
            #[cfg(todo)]
            s => Self::fallback_clone_to_string(s),
            s => S::im_clone_to_string(s),
        }
        .into()
    }
    /// default implementation for [ImStr::im_take_cstring]
    pub fn fallback_take_cstring<S: ?Sized>(s: &mut S) -> Cow<'_, CStr>
    where
        S: ImStr + ImStrExt,
    {
        match s.im_as_c_str().map(|s| s as *const CStr) {
            Some(c) => Cow::Borrowed(unsafe { &*c }),
            _ => Cow::Owned(Self::display_to_cstring(S::im_as_display(&&*s))),
        }
    }
    #[inline(always)]
    pub fn display_to_cstring<T: fmt::Display>(s: T) -> CString {
        let c = format!("{s}\0");
        unsafe { CString::from_vec_with_nul_unchecked(c.into()) }
    }
    /// default implementation for [ImStr::im_append_to]
    #[inline]
    #[cfg(todo = "unnecessary")]
    pub fn fallback_append_to<S: ?Sized>(dest: &mut Vec<u8>, s: &S)
    where
        S: ImStr + ImStrExt,
    {
        if let Some(bytes) = s.im_as_bstr() {
            dest.extend_from_slice(bytes.strip_suffix(b"\0"));
        } else {
            let _ = io::Write::write_fmt(dest, format_args!("{}", ImStrExt::im_as_display(&s)));
        }
    }

    #[inline(always)]
    pub fn im_display_ref<S: ?Sized>(s: &S) -> &ImStrDisplay<S> {
        ImStrDisplay::from_ref(s)
    }
    #[inline(always)]
    pub fn im_display<S>(s: S) -> ImStrDisplay<S> {
        ImStrDisplay(s)
    }
}
impl<'a, S> ImStr for &'a mut S
where
    S: ?Sized + ImStr + ImStrExt,
{
    #[inline(always)]
    fn im_as_id32(&self) -> Option<u32> {
        ImStr::im_as_id32(&**self)
    }
    #[inline(always)]
    fn im_as_id_ptr(&self) -> Option<usize> {
        ImStr::im_as_id_ptr(&**self)
    }
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        ImStr::im_as_str(&**self)
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        ImStr::im_as_bstr(&**self)
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        ImStr::im_as_c_str(&**self)
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStr::im_append_to(&**self, dest)
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        ImStr::im_as_display_dyn(&**self)
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        ImStr::im_take_cstring(*self)
    }
    #[inline(always)]
    fn im_take_cstring_owned(&mut self) -> CString {
        ImStr::im_take_cstring_owned(*self)
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        ImStr::im_clone_to_string(&**self)
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        ImStr::im_clone_to_vec(&**self)
    }
}
impl<'a, S> ImStrExt for &'a mut S
where
    S: ?Sized + ImStr + ImStrExt,
{
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        ImStrExt::im_as_display(refmut2refref(s))
    }
    type IntoImStr
        = Self
    where
        Self: Sized;
    #[inline(always)]
    fn im_into_imstr(self) -> Self::IntoImStr
    where
        Self: Sized,
    {
        self
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(self)
    }
}
impl<'a, S> ImStr for &'a S
where
    S: ?Sized + ImStr + ImStrExt,
{
    #[inline(always)]
    fn im_as_id32(&self) -> Option<u32> {
        ImStr::im_as_id32(*self)
    }
    #[inline(always)]
    fn im_as_id_ptr(&self) -> Option<usize> {
        ImStr::im_as_id_ptr(*self)
    }
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        ImStr::im_as_str(*self)
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        ImStr::im_as_bstr(*self)
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        ImStr::im_as_c_str(*self)
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStr::im_append_to(*self, dest)
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        ImStr::im_as_display_dyn(*self)
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        <dyn ImStr>::fallback_take_cstring(self)
    }
    #[cfg(todo)]
    #[inline(always)]
    fn im_take_cstring_owned(&mut self) -> CString {}
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        ImStr::im_clone_to_string(*self)
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        ImStr::im_clone_to_vec(*self)
    }
}
impl<'a, S> ImStrExt for &'a S
where
    S: ?Sized + ImStr + ImStrExt,
{
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        ImStrExt::im_as_display(*s)
    }
    type IntoImStr
        = Self
    where
        Self: Sized;
    #[inline(always)]
    fn im_into_imstr(self) -> Self::IntoImStr
    where
        Self: Sized,
    {
        self
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(&mut { self })
    }
}
impl<'a, 'b> ImStrExt for dyn ImStr + 'b {
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        ImStrDisplay::<dyn ImStr>::from_refref(s)
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(self)
    }
}
impl<'a> ImStr for fmt::Arguments<'a> {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        fmt::Arguments::as_str(self)
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        None
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        let _ = io::Write::write_fmt(dest, *self);
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(self)
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        <dyn ImStr>::fallback_take_cstring(self)
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        self.to_string()
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.to_string().into()
    }
    #[cfg(todo)]
    fn im_write_to<W: fmt::Write>(&self, dest: &mut W) {
        let _ = dest.write_fmt(*self);
    }
}
impl<'a> ImStrExt for fmt::Arguments<'a> {
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
    fn im_into_string(self) -> String
    where
        Self: Sized,
    {
        self.to_string()
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(self)
    }
}
impl ImStr for [u8] {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        str::from_utf8(self).ok()
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        Some(self)
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        dest.extend_from_slice(self);
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        None
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        <dyn ImStr>::fallback_take_cstring(self)
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        String::from_utf8_lossy(self).into_owned()
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.into()
    }
}
impl ImStrExt for [u8] {
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        BStrDisplay::from_ref(s)
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(&mut &*self)
    }
}
impl ImStr for Vec<u8> {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        ImStr::im_as_str(&self[..])
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        Some(&self[..])
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        ImStr::im_as_c_str(&self[..])
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStr::im_append_to(&self[..], dest)
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(BStrDisplay::from_ref(self))
    }
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        Cow::Borrowed(match self[..].last().copied() {
            Some(0) => unsafe { CStr::from_bytes_with_nul_unchecked(&self[..]) },
            #[cfg(todo = "unnecessary")]
            _ => {
                let len_prev = self.len();
                self.push(0u8);
                let bytes = &self[..] as *const [u8] as *const CStr;
                unsafe {
                    // terminator is start of unallocated capacity,
                    // but it sems kind to leave it unmodified for after the borrow ends
                    self.set_len(len_prev);
                    Cow::Borrowed(&*bytes)
                }
            },
            _ => CSlice::terminate_bytes(self).as_ref(),
        })
    }
    #[inline]
    fn im_take_cstring_owned(&mut self) -> CString {
        unsafe {
            match self[..].last().copied() {
                Some(0) => CString::from_vec_with_nul_unchecked(mem::take(self)),
                _ => CString::from_vec_unchecked(mem::take(self)),
            }
        }
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        <dyn ImStr>::fallback_clone_to_string(self)
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.clone()
    }
}
impl ImStrExt for Vec<u8> {
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        BStrDisplay::from_ref(*s)
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
        f(self)
    }
}
impl ImStr for str {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        Some(self)
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        Some(self.as_bytes())
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStr::im_append_to(self.as_bytes(), dest)
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        None
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        <dyn ImStr>::fallback_take_cstring(self)
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        self.into()
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        String::from(self).into()
    }
    #[cfg(todo)]
    fn im_write_to<W: fmt::Write>(&self, dest: &mut W) {
        let _ = dest.write_str(self);
    }
}
impl ImStrExt for str {
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        s
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(&mut &*self)
    }
}
impl ImStr for String {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        Some(self.as_str())
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        ImStr::im_as_bstr(self.as_str())
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStr::im_append_to(self.as_bytes(), dest)
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(self)
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        ImStr::im_as_c_str(self.as_str())
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        ImStr::im_take_cstring(unsafe { self.as_mut_vec() })
    }
    #[inline(always)]
    fn im_take_cstring_owned(&mut self) -> CString {
        ImStr::im_take_cstring_owned(unsafe { self.as_mut_vec() })
    }
    #[cfg(todo = "unnecessary")]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        if let Some(s) = self.im_as_c_str() {
            return Cow::Borrowed(s)
        }
        Cow::Owned(unsafe {
            let mut s = mem::take(&mut self).into();
            s.push(0u8);
            CString::from_bytes_with_nul_unchecked(s)
        })
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        self.clone()
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.clone().into()
    }
}
impl ImStrExt for String {
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
    fn im_into_string(self) -> String
    where
        Self: Sized,
    {
        self
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(self)
    }
}
impl<'a> ImStr for Cow<'a, str> {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        Some(&self[..])
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        Some(self.as_bytes())
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStr::im_append_to(self.as_bytes(), dest)
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        ImStr::im_as_c_str(&self[..])
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(self)
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        match self {
            &mut Cow::Borrowed(s) if s.as_bytes().last().copied() == Some(0) =>
                Cow::Borrowed(unsafe { ImStr::im_as_c_str(s).unwrap_unchecked() }),
            s => {
                let s = s.to_mut();
                ImStr::im_take_cstring(unsafe { s.as_mut_vec() })
            },
        }
    }
    #[inline(always)]
    fn im_take_cstring_owned(&mut self) -> CString {
        match self {
            &mut Cow::Borrowed(mut s) => ImStr::im_take_cstring_owned(&mut s),
            Cow::Owned(s) => ImStr::im_take_cstring_owned(unsafe { s.as_mut_vec() }),
        }
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        self[..].into()
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.im_clone_to_string().into()
    }
}
impl<'a> ImStrExt for Cow<'a, str> {
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
    fn im_into_string(self) -> String
    where
        Self: Sized,
    {
        self.into()
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(self)
    }
}
impl ImStr for CStr {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        None
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        Some(self.to_bytes())
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStr::im_append_to(self.to_bytes(), dest)
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(CStrRef::with_cstr(self))
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        Some(self)
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        Cow::Borrowed(&*self)
    }
    #[inline(always)]
    fn im_take_cstring_owned(&mut self) -> CString {
        CString::from(&*self)
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        <dyn ImStr>::fallback_clone_to_string(self)
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.to_bytes().into()
    }
}
impl ImStrExt for CStr {
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        CStrRef::with_cstr(*s)
    }
    #[inline(always)]
    fn with_imstr_dyn<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn ImStr) -> R,
    {
        f(&mut &*self)
    }
}
impl ImStr for CString {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        ImStr::im_as_str(&self[..])
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        ImStr::im_as_bstr(&self[..])
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStr::im_append_to(self.as_bytes(), dest)
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        ImStr::im_as_display_dyn(&self[..])
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        Some(&*self)
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        Cow::Borrowed(&*self)
    }
    #[inline(always)]
    fn im_take_cstring_owned(&mut self) -> CString {
        mem::take(self)
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        <dyn ImStr>::fallback_clone_to_string(self)
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.clone().into()
    }
}
impl ImStrExt for CString {
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        CStrRef::with_cstr(&s[..])
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
        f(self)
    }
    #[inline(always)]
    fn im_with_cbstr<R, F>(self, f: F) -> R
    where
        Self: Sized,
        F: FnOnce(Result<&CSlice, &[u8]>) -> R,
    {
        f(Ok(self.as_ref()))
    }
    #[inline(always)]
    fn im_with_cstr<R, F>(self, f: F) -> R
    where
        Self: Sized,
        F: FnOnce(&CStr) -> R,
    {
        f(&self[..])
    }
}
impl<'a> ImStr for Cow<'a, CStr> {
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        CStr::im_as_str(self)
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        CStr::im_as_bstr(self)
    }
    #[inline(always)]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        CStr::im_append_to(self, dest)
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        Some(self)
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        match self {
            #[cfg(todo = "unnecessary")]
            Cow::Owned(ref c) => Some(BStrDisplay::from_ref(c)),
            Cow::Owned(ref c) => CString::im_as_display_dyn(c),
            Cow::Borrowed(ref c) => Some(CStr::im_as_display(c)),
        }
    }
    #[inline(always)]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        match self {
            Cow::Borrowed(s) => Cow::Borrowed(s),
            Cow::Owned(s) => Cow::Owned(mem::take(s)),
        }
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        match self {
            Cow::Owned(ref c) => CString::im_clone_to_string(c),
            Cow::Borrowed(c) => CStr::im_clone_to_string(c),
        }
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        match self {
            Cow::Owned(ref c) => CString::im_clone_to_vec(c),
            Cow::Borrowed(c) => CStr::im_clone_to_vec(c),
        }
    }
}
impl<'a> ImStrExt for Cow<'a, CStr> {
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        match s {
            #[cfg(todo = "unnecessary")]
            Cow::Owned(ref c) => BStrDisplay::from_ref(c),
            Cow::Owned(ref c) => CStrRef::with_cstr(&c[..]),
            Cow::Borrowed(ref c) => CStr::im_as_display(c),
        }
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
        f(self)
    }
    #[inline(always)]
    fn im_with_cbstr<R, F>(self, f: F) -> R
    where
        Self: Sized,
        F: FnOnce(Result<&CSlice, &[u8]>) -> R,
    {
        f(Ok(CSlice::with_cstr(&self)))
    }
    #[inline(always)]
    fn im_with_cstr<R, F>(self, f: F) -> R
    where
        Self: Sized,
        F: FnOnce(&CStr) -> R,
    {
        f(&*self)
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct ImStrDisplay<T: ?Sized = dyn ImStr>(pub T);
impl<T: ?Sized> ImStrDisplay<T> {
    #[inline(always)]
    pub const fn from_ref(s: &T) -> &Self {
        unsafe { mem::transmute(s) }
    }
    #[inline(always)]
    pub const fn from_refref<'u, 'a>(s: &'u &'a dyn ImStr) -> &'u &'a Self {
        unsafe { mem::transmute(s) }
    }
}
/// TODO: misuse can overflow stack
impl<T: ?Sized> fmt::Display for ImStrDisplay<T>
where
    T: ImStr,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(bstr) = self.0.im_as_bstr() {
            fmt::Display::fmt(BStrDisplay::from_ref(bstr), f)
        } else if let Some(disp) = self.0.im_as_display_dyn() {
            fmt::Display::fmt(disp, f)
        } else {
            fmt::Display::fmt(&self.0.im_clone_to_string(), f)
        }
    }
}
#[cfg(todo)]
impl<T> ImStrExt for ImStrDisplay<T>
where
    T: ImStr + ImStrExt,
    Self: fmt::Display,
{
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        *s
    }
    type IntoImStr
        = T
    where
        Self: Sized;
    #[inline(always)]
    fn im_into_imstr(self) -> Self::IntoImStr
    where
        Self: Sized,
    {
        self.0
    }
}
impl<T> ImStr for ImStrDisplay<T>
where
    T: fmt::Display,
{
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        None
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        None
    }
    #[inline]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        let _ = io::Write::write_fmt(dest, format_args!("{}", self.0));
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        None
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(&self.0)
    }
    #[inline]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        <dyn ImStr>::fallback_take_cstring(self)
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        <dyn ImStr>::fallback_clone_to_string(self)
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        <dyn ImStr>::fallback_clone_to_vec(self)
    }
}
impl<T> ImStrExt for ImStrDisplay<T>
where
    T: fmt::Display,
    Self: ImStr,
{
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        &s.0
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
        f(self)
    }
}
#[cfg(todo)]
impl<'a, T> ImStrExt for &'a ImStrDisplay<T>
where
    T: ?Sized + fmt::Display,
    Self: ImStr,
{
    #[inline(always)]
    fn im_as_display<'u>(s: &'u &Self) -> &'u dyn fmt::Display {
        s
    }
    type IntoImStr = Self;
    #[inline(always)]
    fn im_into_imstr(self) -> Self::IntoImStr {
        self
    }
}

pub const IM_ID_SEP_APPEND: &'static str = "##";
pub const IM_ID_SEP_REPLACE: &'static str = "###";
/// TODO: treat id as optional and omit ## from format, populate im_as_bstr, etc?
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImStrId<I, S> {
    pub id: I,
    pub label: S,
    /// ID may be appended to disambiguate a label, or replace to act as a canon ID
    pub unique: bool,
}
impl<'i, S> ImStrId<&'i str, S> {
    #[inline(always)]
    pub const fn with_ident(id: &'i str, label: S) -> Self {
        Self::new(id, label)
    }
}
impl<'a, I> ImStrId<I, &'a str> {
    #[inline(always)]
    pub const fn unlabelled(id: I) -> Self {
        Self::new(id, "")
    }
}
impl<'a, I> ImStrId<ImStrDisplay<I>, &'a str> {
    #[inline(always)]
    pub const fn unlabelled_im(imstr: I) -> Self {
        Self::new(ImStrDisplay(imstr), "")
    }
    #[inline(always)]
    pub const fn unlabelled_im_ref<'s>(imstr: &'s I) -> ImStrId<&'s ImStrDisplay<I>, &'a str> {
        ImStrId::new(ImStrDisplay::from_ref(imstr), "")
    }
}
impl<S> ImStrId<u32, S> {
    #[inline(always)]
    pub const fn named_int(id: u32, label: S) -> Self {
        Self::new(id, label)
    }
}
impl<I, S> ImStrId<I, S> {
    #[inline(always)]
    pub const fn disambiguate(id: I, label: S) -> Self {
        Self::from_parts(id, label, false)
    }
    #[inline(always)]
    pub const fn from_parts(id: I, label: S, unique: bool) -> Self {
        Self { id, label, unique }
    }
    #[inline(always)]
    pub const fn new(id: I, label: S) -> Self {
        Self::from_parts(id, label, true)
    }
    #[inline(always)]
    pub fn strip_label(self) -> ImStrId<I, ImIdEmptyLabel> {
        ImStrId::from_parts(self.id, ImIdEmptyLabel, self.unique)
    }
}
impl<I, S> fmt::Display for ImStrId<I, S>
where
    I: fmt::Display,
    S: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(todo = "unnecessary")]
        let sep = match self.unique {
            true => IM_ID_SEP_REPLACE,
            false => IM_ID_SEP_APPEND,
        };
        let sep = self.unique.then_some("#").unwrap_or("");
        write!(f, "{}##{sep}{}", self.label, self.id)
    }
}
impl<I, S> ImStr for ImStrId<I, S>
where
    I: fmt::Display,
    S: fmt::Display,
{
    #[inline(always)]
    fn im_as_str(&self) -> Option<&str> {
        None
    }
    #[inline(always)]
    fn im_as_bstr(&self) -> Option<&[u8]> {
        None
    }
    #[inline]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        let _ = io::Write::write_fmt(dest, format_args!("{self}"));
    }
    #[inline(always)]
    fn im_as_c_str(&self) -> Option<&CStr> {
        None
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(self)
    }
    #[inline]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        <dyn ImStr>::fallback_take_cstring(self)
    }
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        <dyn ImStr>::fallback_clone_to_string(self)
    }
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        <dyn ImStr>::fallback_clone_to_vec(self)
    }
}
impl<I, S> ImStrExt for ImStrId<I, S>
where
    Self: ImStr + fmt::Display,
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
        f(self)
    }
}

pub trait IntoImStrId {
    type IntoImStr: ImStr;
    fn im_into_id(self) -> Self::IntoImStr;
}
impl IntoImStrId for u32 {
    type IntoImStr = ImStrId<u32, ImIdEmptyLabel>;
    #[inline(always)]
    fn im_into_id(self) -> Self::IntoImStr {
        ImStrId::int(self)
    }
}
impl IntoImStrId for usize {
    type IntoImStr = ImStrId<usize, ImIdEmptyLabel>;
    #[inline(always)]
    fn im_into_id(self) -> Self::IntoImStr {
        ImStrId::ptr(self)
    }
}
impl<T> IntoImStrId for *const T {
    type IntoImStr = ImStrId<usize, ImIdEmptyLabel>;
    #[inline(always)]
    fn im_into_id(self) -> Self::IntoImStr {
        ImStrId::ptr(self as usize)
    }
}
impl<T> IntoImStrId for *mut T {
    type IntoImStr = ImStrId<usize, ImIdEmptyLabel>;
    #[inline(always)]
    fn im_into_id(self) -> Self::IntoImStr {
        ImStrId::ptr(self as usize)
    }
}
impl<'a, S: ImStr> IntoImStrId for S {
    type IntoImStr = S;
    #[inline(always)]
    fn im_into_id(self) -> Self::IntoImStr {
        self
    }
}

#[derive(Debug, Copy, Clone, Default, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct ImIdEmptyLabel;
impl ImStrId<u32, ImIdEmptyLabel> {
    #[inline(always)]
    pub const fn int(id: u32) -> Self {
        Self::new(id, ImIdEmptyLabel)
    }
}
impl ImStrId<usize, ImIdEmptyLabel> {
    #[inline(always)]
    pub const fn ptr(id: usize) -> Self {
        Self::new(id, ImIdEmptyLabel)
    }
}
impl<I> ImStr for ImStrId<I, ImIdEmptyLabel>
where
    I: Copy + AsPrimitive<u32> + AsPrimitive<usize> + fmt::Display,
{
    #[inline(always)]
    fn im_as_id_ptr(&self) -> Option<usize> {
        Some(self.id.as_())
    }
    #[inline(always)]
    fn im_as_id32(&self) -> Option<u32> {
        Some(self.id.as_())
    }
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
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.im_clone_to_string().into()
    }

    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        self.id.to_string()
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        Some(&self.id as &dyn fmt::Display)
    }
    #[inline]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        let _ = io::Write::write_fmt(dest, format_args!("{}", self.id));
    }
    #[inline]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        Cow::Owned(<dyn ImStr>::display_to_cstring(self.id))
    }

    // this isn't a named label, use unlabelled explicitly for that instead...
    #[cfg(todo)]
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        None
    }
    #[cfg(todo)]
    #[inline]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        let _ = io::Write::write_fmt(dest, format_args!("{}", ImStrId::unlabelled(self.id)));
    }
    #[cfg(todo)]
    #[inline]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        Cow::Owned(<dyn ImStr>::display_to_cstring(ImStrId::unlabelled(self.id)))
    }
    #[cfg(todo)]
    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        ImStrId::unlabelled(self.id).im_clone_to_string()
    }
}
impl<I, S> ImStr for ImStrId<ImStrId<I, ImIdEmptyLabel>, S>
where
    ImStrId<I, ImIdEmptyLabel>: ImStr,
    I: fmt::Display,
    S: fmt::Display,
{
    #[inline(always)]
    fn im_as_id_ptr(&self) -> Option<usize> {
        if !self.unique {
            return None
        }
        self.id.im_as_id_ptr()
    }
    #[inline(always)]
    fn im_as_id32(&self) -> Option<u32> {
        if !self.unique {
            return None
        }
        self.id.im_as_id32()
    }
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
    #[inline(always)]
    fn im_clone_to_vec(&self) -> Vec<u8> {
        self.im_clone_to_string().into()
    }

    #[inline(always)]
    fn im_clone_to_string(&self) -> String {
        ImStrId::from_parts(&self.id.id, &self.label, self.unique).im_clone_to_string()
    }
    #[inline(always)]
    fn im_as_display_dyn(&self) -> Option<&dyn fmt::Display> {
        None
    }
    #[inline]
    fn im_append_to(&self, dest: &mut Vec<u8>) {
        ImStrId::from_parts(&self.id.id, &self.label, self.unique).im_append_to(dest)
    }
    #[inline]
    fn im_take_cstring(&mut self) -> Cow<'_, CStr> {
        Cow::Owned(<dyn ImStr>::display_to_cstring(ImStrId::from_parts(
            &self.id.id,
            &self.label,
            self.unique,
        )))
    }
}

#[repr(transparent)]
#[doc(hidden)]
pub struct BStrDisplay<T: ?Sized>(T);
impl<T: ?Sized> BStrDisplay<T> {
    #[inline(always)]
    const fn from_ref(t: &T) -> &Self {
        unsafe { mem::transmute(t) }
    }
}
impl<T: ?Sized> fmt::Display for BStrDisplay<T>
where
    T: AsRef<[u8]>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.0.as_ref();
        let utf8 = bytes.strip_suffix(b"\0").unwrap_or(bytes).utf8_chunks();
        Ok(for chunk in utf8 {
            f.write_str(chunk.valid())?;
            match chunk.invalid() {
                invalid if invalid.is_empty() => (),
                invalid =>
                    for _ in invalid {
                        fmt::Write::write_char(f, char::REPLACEMENT_CHARACTER)?;
                    },
            }
        })
    }
}
/// `as` seems unnecessary but I like it here anyway
#[inline(always)]
fn refmut2refref<'u, 'a, S: ?Sized>(s: &'u &'a mut S) -> &'u &'a S {
    unsafe {
        let s = s as *const &'a mut S as *const *mut S;
        mem::transmute::<*const *const S, &&S>(s as *const _)
    }
}
