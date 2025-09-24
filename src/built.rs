pub const CI_PLATFORM: Option<&'static str> = option_env!(concat!("BUILT_OVERRIDE_", env!("CARGO_PKG_NAME"), "'CI_PLATFORM"));
pub const GIT_HEAD_REF: Option<&'static str> = option_env!(concat!("BUILT_OVERRIDE_", env!("CARGO_PKG_NAME"), "'GIT_HEAD_REF"));
pub const GIT_COMMIT_HASH_SHORT: Option<&'static str> = option_env!(concat!("BUILT_OVERRIDE_", env!("CARGO_PKG_NAME"), "'GIT_COMMIT_HASH_SHORT"));
