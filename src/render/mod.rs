#![allow(irrefutable_let_patterns)]

pub mod api_tab;
#[cfg(feature = "extension-arcdps")]
pub mod arc;
pub mod config_tab;
pub mod data_source_tab;
#[cfg(feature = "markers-edit")]
pub mod edit_marker_window;
pub mod element;
#[cfg(feature = "goggles")]
pub mod goggles;
pub mod i18n;
pub mod info_tab;
pub mod machine;
#[cfg(feature = "markers")]
pub mod marker_tab;
pub mod menu;
#[cfg(feature = "paths")]
pub mod message_window;
#[cfg(feature = "space")]
pub mod pathing_tab;
#[cfg(feature = "space")]
pub mod pathing_window;
#[cfg(feature = "scripts")]
pub mod plug;
pub mod primary_window;
pub mod state;
pub mod timer_tab;
pub mod timer_window;

#[cfg(feature = "markers")]
pub mod marker_window;

#[cfg(feature = "extension-arcdps")]
pub use arc::ArcRenderState;
#[allow(unused_imports)]
pub use {
    api_tab::ApiTabState,
    config_tab::ConfigTabState,
    data_source_tab::DataSourceTabState,
    info_tab::InfoTabState,
    primary_window::PrimaryWindowState,
    state::{Alignment, RenderEvent, RenderState, TextFont},
    timer_tab::TimerTabState,
    timer_window::TimerWindowState,
};
#[cfg(feature = "markers")]
pub use {marker_tab::MarkerTabState, marker_window::MarkerWindowState};

#[cfg(feature = "space")]
pub use self::{pathing_tab::PathingConfig, pathing_window::PathingWindowState};
