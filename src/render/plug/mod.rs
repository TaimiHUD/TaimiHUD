// incomplete WIP, no point in cleaning it up yet
#![cfg_attr(not(taimi_debug = "wip"), allow(nonstandard_style, unused, unexpected_cfgs))]

mod config;

pub use self::config::{PlugConfig, PlugConfigCache, PlugConfigDesc, PlugConfigState};
