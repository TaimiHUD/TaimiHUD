use std::{
    borrow::{Borrow, Cow},
    hash,
};

pub mod cmp;
pub mod collections;
pub mod flags;
pub mod iters;
pub mod lazyfmt;
pub mod loc;
pub mod paths;
pub mod statistics;
pub mod time;
pub mod vec;

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
/// `#[serde(skip_serializing_if = "is_bool_ref::<false>")]`
#[inline(always)]
pub fn is_bool_ref<const N: bool>(&v: &bool) -> bool {
    v == N
}
/// `#[serde(skip_serializing_if = "is_false_ref")]`
#[inline(always)]
pub fn is_false_ref(&v: &bool) -> bool {
    !v
}
/// `#[serde(skip_serializing_if = "is_true_ref")]`
#[inline(always)]
pub fn is_true_ref(&v: &bool) -> bool {
    v
}
/// `#[serde(skip_serializing_if = "bool_or_none::<true>")]`
#[inline(always)]
pub fn bool_or_none<const N: bool>(&v: &Option<bool>) -> bool {
    v != Some(!N)
}
/// `#[serde(default = "a_bool::<true>")]`
#[inline(always)]
pub fn a_bool<const N: bool>() -> bool {
    N
}
/// `#[serde(skip_serializing_if = "f32_or_none::<{2.0f32.to_bits()}>")]`
#[inline(always)]
pub fn f32_or_none<const N: u32>(v: &Option<f32>) -> bool {
    match *v {
        Some(v) => v == f32::from_bits(N),
        None => true,
    }
}
/// `#[serde(skip_serializing_if = "as_a_f32::<{2.0f32.to_bits()}, _>")]`
#[inline(always)]
pub fn as_a_f32<const N: u32, T: Borrow<f32>>(v: T) -> bool {
    is_a_f32::<N>(v.borrow())
}
/// `#[serde(skip_serializing_if = "is_a_f32::<{2.0f32.to_bits()}>")]`
#[inline(always)]
pub fn is_a_f32<const N: u32>(v: &f32) -> bool {
    *v == f32::from_bits(N)
}
/// `#[serde(default = "a_f32::<{2.0f32.to_bits()}>")]`
#[inline(always)]
pub fn a_f32<const N: u32>() -> f32 {
    f32::from_bits(N)
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
