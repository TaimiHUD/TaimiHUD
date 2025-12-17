use std::{borrow::Cow, hash};

pub mod collections;
pub mod flags;
pub mod lazyfmt;
pub mod loc;
pub mod paths;
pub mod statistics;

pub fn str_opt_ref<S: ?Sized + AsRef<str>>(s: &S) -> Option<&str> {
    str_opt(s).map(|s| s.as_ref())
}
pub fn str_opt<S: AsRef<str>>(s: S) -> Option<S> {
    try_str(s).ok()
}
pub fn try_str<S: AsRef<str>>(s: S) -> Result<S, S> {
    match s.as_ref().is_empty() {
        true => Err(s),
        false => Ok(s),
    }
}

/// `#[serde(skip_serializing_if = "is_default")]`
pub fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}
/// `#[serde(skip_serializing_if = "is_false_ref")]`
pub fn is_false_ref(&v: &bool) -> bool {
    !v
}
/// `#[serde(skip_serializing_if = "is_true_ref")]`
pub fn is_true_ref(&v: &bool) -> bool {
    !v
}
/// `#[serde(default = "default_true")]`
pub fn default_true() -> bool {
    true
}

/// `*dest = v.into_owned()` except [reuse dest](ToOwned::clone_into) if possible
///
/// TODO: would be neat if there were a conversion trait to use to transfer
/// owned data instead of unconditionally nuking the old allocation via assignment
pub fn write_owned<T>(dest: &mut T::Owned, v: Cow<'_, T>)
where
    T: ?Sized + ToOwned,
{
    match v {
        Cow::Owned(v) => {
            *dest = v;
        },
        Cow::Borrowed(v) => v.clone_into(dest),
    }
}

pub fn hash_value<T, H>(hasher: &mut H, value: &T) -> u64
where
    T: ?Sized + hash::Hash,
    H: hash::Hasher,
{
    value.hash(hasher);
    hasher.finish()
}
/// reconsider before reaching for this
pub fn hash_eq<T>(lhs: &T, rhs: &T) -> bool
where
    T: ?Sized + hash::Hash,
{
    use hash::DefaultHasher;
    let lhs = hash_value(&mut DefaultHasher::new(), lhs);
    let rhs = hash_value(&mut DefaultHasher::new(), rhs);
    lhs == rhs
}
/// for when false negatives via [hash_value] acceptable
pub fn hash_then_eq<T>(lhs: &T, rhs: &T) -> bool
where
    T: ?Sized + hash::Hash + PartialEq,
{
    hash_eq(lhs, rhs) && lhs == rhs
}
