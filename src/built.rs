pub const CI_PLATFORM: Option<&'static str> =
    option_env!(concat!("BUILT_OVERRIDE_", env!("CARGO_PKG_NAME"), "_CI_PLATFORM"));
pub const GIT_HEAD_REF: Option<&'static str> = option_env!(concat!(
    "BUILT_OVERRIDE_",
    env!("CARGO_PKG_NAME"),
    "_GIT_HEAD_REF"
));
pub const GIT_COMMIT_HASH_SHORT: Option<&'static str> = option_env!(concat!(
    "BUILT_OVERRIDE_",
    env!("CARGO_PKG_NAME"),
    "_GIT_COMMIT_HASH_SHORT"
));
