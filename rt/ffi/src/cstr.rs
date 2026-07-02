//! [null-terminated utf8](Str0)
//!
//! TODO: move into ffi

pub use arcffi::cstr::*;
use {
    arcffi::alloc::{
        borrow::{Borrow, Cow, ToOwned},
        ffi::NulError,
    },
    core::{fmt, mem, ops, str::Utf8Error},
};

#[macro_export]
macro_rules! cstr {
    (0fmt: $($tt:tt)*) => {
        unsafe {
            $crate::cstr::String0::from_vec_with_nul_unchecked(format!("{}\0", format_args! { $($tt)* }).into())
        }
    };
    (0$($tt:tt)*) => {
        unsafe {
            $crate::cstr::Str0::from_c_slice($crate::arcffi::cstr::CSlice::with_cstr($crate::arcffi::cstr! { $($tt)* }))
        }
    };
    ($($tt:tt)*) => {
        $crate::arcffi::cstr! { $($tt)* }
    };
}
pub use cstr;

/// null-terminated utf8
///
/// see also: [String0]
///
/// TODO: `Box<Str0>` conversions, though kinda pointless since String0(CString) is already a `Box<[u8]>`
///
/// TODO: generic storage so it can be a thin pointer too
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Str0(CSlice);
impl Str0 {
    pub const EMPTY: &'static Self = unsafe { Self::from_c_slice(CSlice::EMPTY) };

    #[inline(always)]
    pub const unsafe fn from_bytes_with_nul_unchecked(s: &[u8]) -> &Self {
        Self::from_c_slice(CSlice::from_bytes_with_nul_unchecked(s))
    }
    #[inline(always)]
    pub const unsafe fn from_c_slice(s: &CSlice) -> &Self {
        mem::transmute(s)
    }
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
    #[inline(always)]
    pub fn as_bytes_with_nul(&self) -> &[u8] {
        self.0.to_bytes_with_nul()
    }
    #[inline(always)]
    pub fn as_c_str(&self) -> &CStr {
        self.0.as_c_str()
    }
    #[inline(always)]
    pub fn as_c_ptr(&self) -> CStrPtr<'_> {
        self.as_c_ref().as_c_ptr()
    }
    #[inline(always)]
    pub fn as_c_ref(&self) -> &CStrRef {
        self.0.as_ref()
    }
    #[inline(always)]
    pub const fn as_c_slice(&self) -> &CSlice {
        &self.0
    }
    #[inline(always)]
    pub const fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.0.as_bytes()) }
    }
    #[inline(always)]
    pub unsafe fn c_slice_mut(&mut self) -> &mut CSlice {
        &mut self.0
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        matches!(self.as_bytes_with_nul().first(), Some(0) | None)
    }

    #[inline(always)]
    pub fn to_string(&self) -> String {
        self.as_str().into()
    }
    #[inline(always)]
    pub fn to_string0(&self) -> String0 {
        unsafe { String0::from_c_string_unchecked(self.to_c_string()) }
    }
    #[inline(always)]
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.as_bytes().into()
    }
    #[inline(always)]
    pub fn to_vec_with_nul(&self) -> Vec<u8> {
        self.as_bytes_with_nul().into()
    }
    #[inline(always)]
    pub fn to_c_string(&self) -> CString {
        unsafe { CString::from_vec_with_nul_unchecked(self.to_vec_with_nul()) }
    }
}
impl Default for &'_ Str0 {
    fn default() -> Self {
        Str0::EMPTY
    }
}

/// owned [Str0]
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct String0(CString);
impl String0 {
    pub fn empty() -> Self {
        Self(CString::default())
    }
    #[inline]
    pub unsafe fn try_from_string<S: Into<String>>(s: S) -> Result<Self, NulError> {
        CString::new(s.into().into_bytes()).map(|s| unsafe { Self::from_c_string_unchecked(s) })
    }
    #[inline]
    pub unsafe fn try_from_c_string<S: Into<CString>>(s: S) -> Result<Self, Utf8Error> {
        let s = s.into();
        let _ = s.to_str()?;
        Ok(Self::from_c_string_unchecked(s))
    }
    #[inline(always)]
    pub const unsafe fn from_c_string_unchecked(s: CString) -> Self {
        Self(s)
    }
    #[inline(always)]
    pub unsafe fn from_vec_with_nul_unchecked(s: Vec<u8>) -> Self {
        Self(CString::from_vec_with_nul_unchecked(s))
    }
    #[inline(always)]
    pub unsafe fn from_vec_unchecked(s: Vec<u8>) -> Self {
        Self(CString::from_vec_unchecked(s))
    }
    #[inline(always)]
    pub fn as_str0(&self) -> &Str0 {
        unsafe { Str0::from_bytes_with_nul_unchecked(self.as_bytes_with_nul()) }
    }
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.0.as_bytes()) }
    }
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
    #[inline(always)]
    pub fn as_bytes_with_nul(&self) -> &[u8] {
        self.0.as_bytes_with_nul()
    }
    #[inline(always)]
    pub const fn as_c_string(&self) -> &CString {
        &self.0
    }
    #[inline(always)]
    pub unsafe fn c_string_mut(&mut self) -> &mut CString {
        &mut self.0
    }
    #[inline(always)]
    pub fn into_c_string(self) -> CString {
        self.0
    }
    #[inline(always)]
    pub fn into_string(self) -> String {
        unsafe { String::from_utf8_unchecked(self.0.into_bytes()) }
    }
    #[inline(always)]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes()
    }
    #[inline(always)]
    pub fn into_bytes_with_nul(self) -> Vec<u8> {
        self.0.into_bytes_with_nul()
    }

    /// TODO: identify or truncate interior nul bytes?
    pub fn format<D: fmt::Display>(display: D) -> Self {
        let s = format!("{display}\0").into_bytes();
        unsafe { Self::from_vec_with_nul_unchecked(s.into()) }
    }
}

impl fmt::Debug for Str0 {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Str0").field(&self.as_str()).finish()
    }
}
impl fmt::Debug for String0 {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("String0").field(&self.as_str()).finish()
    }
}
impl fmt::Display for Str0 {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
impl fmt::Display for String0 {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
/// TODO: consider `Vec<u8>` backing store instead for growable/writable,
/// or otherwise allocate additional nul bytes at the end for capacity
/// (the cost of strlen for most operations is unlikely worthwhile...)
#[cfg(todo)]
impl fmt::Write for String0 {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let buf = self.0.xxx;
    }
}
/// TODO: identify or truncate interior nul bytes?
impl From<fmt::Arguments<'_>> for String0 {
    fn from(value: fmt::Arguments<'_>) -> Self {
        Self::format(value)
    }
}

impl ops::Deref for Str0 {
    type Target = str;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
impl ops::Deref for String0 {
    type Target = Str0;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_str0()
    }
}
impl ToOwned for Str0 {
    type Owned = String0;
    #[inline(always)]
    fn to_owned(&self) -> Self::Owned {
        self.to_string0()
    }
}
impl<'a> From<&'a Str0> for Cow<'a, Str0> {
    #[inline(always)]
    fn from(c: &'a Str0) -> Self {
        Cow::Borrowed(c)
    }
}
impl<'a> From<&'a String0> for Cow<'a, Str0> {
    #[inline(always)]
    fn from(c: &'a String0) -> Self {
        Cow::Borrowed(&c)
    }
}
impl From<String0> for Cow<'_, Str0> {
    #[inline(always)]
    fn from(c: String0) -> Self {
        Cow::Owned(c)
    }
}
impl<'a> From<&'a Str0> for Cow<'a, str> {
    #[inline(always)]
    fn from(c: &'a Str0) -> Self {
        Cow::Borrowed(c.as_str())
    }
}
impl<'a> From<&'a String0> for Cow<'a, str> {
    #[inline(always)]
    fn from(c: &'a String0) -> Self {
        Cow::Borrowed(c.as_str())
    }
}
impl From<String0> for Cow<'_, str> {
    #[inline(always)]
    fn from(c: String0) -> Self {
        Cow::Owned(c.into_string())
    }
}
impl<'a> From<&'a Str0> for &'a CStr {
    #[inline(always)]
    fn from(c: &'a Str0) -> Self {
        c.as_c_str()
    }
}
impl<'a> From<&'a Str0> for &'a str {
    #[inline(always)]
    fn from(c: &'a Str0) -> Self {
        c.as_str()
    }
}
impl<'a> From<&'a String0> for &'a CStr {
    #[inline(always)]
    fn from(c: &'a String0) -> Self {
        c.as_c_str()
    }
}
impl<'a> From<&'a String0> for &'a str {
    #[inline(always)]
    fn from(c: &'a String0) -> Self {
        c.as_str()
    }
}
impl From<&'_ Str0> for CString {
    #[inline(always)]
    fn from(c: &Str0) -> Self {
        c.as_c_str().to_owned()
    }
}
impl From<String0> for CString {
    #[inline(always)]
    fn from(c: String0) -> Self {
        c.into_c_string()
    }
}
impl<'a> From<&'a Str0> for String {
    #[inline(always)]
    fn from(c: &'a Str0) -> Self {
        c.as_str().into()
    }
}
impl From<String0> for String {
    #[inline(always)]
    fn from(c: String0) -> Self {
        c.into_string()
    }
}
impl Borrow<Str0> for String0 {
    #[inline(always)]
    fn borrow(&self) -> &Str0 {
        self.as_str0()
    }
}

impl Borrow<str> for Str0 {
    #[inline(always)]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl Borrow<CStr> for Str0 {
    #[inline(always)]
    fn borrow(&self) -> &CStr {
        self.as_c_str()
    }
}
impl Borrow<CSlice> for Str0 {
    #[inline(always)]
    fn borrow(&self) -> &CSlice {
        self.as_c_slice()
    }
}
impl Borrow<CStrRef> for Str0 {
    #[inline(always)]
    fn borrow(&self) -> &CStrRef {
        self.as_c_ref()
    }
}
impl Borrow<str> for String0 {
    #[inline(always)]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl Borrow<CStr> for String0 {
    #[inline(always)]
    fn borrow(&self) -> &CStr {
        self.as_c_str()
    }
}
impl Borrow<CSlice> for String0 {
    #[inline(always)]
    fn borrow(&self) -> &CSlice {
        self.as_c_slice()
    }
}
impl Borrow<CStrRef> for String0 {
    #[inline(always)]
    fn borrow(&self) -> &CStrRef {
        self.as_c_ref()
    }
}

impl AsRef<str> for Str0 {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<CStr> for Str0 {
    #[inline(always)]
    fn as_ref(&self) -> &CStr {
        self.as_c_str()
    }
}
impl AsRef<CStrRef> for Str0 {
    #[inline(always)]
    fn as_ref(&self) -> &CStrRef {
        self.as_c_ref()
    }
}
impl AsRef<CSlice> for Str0 {
    #[inline(always)]
    fn as_ref(&self) -> &CSlice {
        self.as_c_slice()
    }
}
impl AsRef<str> for String0 {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<CStr> for String0 {
    #[inline(always)]
    fn as_ref(&self) -> &CStr {
        self.as_c_str()
    }
}
impl AsRef<CStrRef> for String0 {
    #[inline(always)]
    fn as_ref(&self) -> &CStrRef {
        self.as_c_ref()
    }
}
impl AsRef<CSlice> for String0 {
    #[inline(always)]
    fn as_ref(&self) -> &CSlice {
        self.as_c_slice()
    }
}
