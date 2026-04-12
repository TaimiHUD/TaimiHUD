mod arc;
pub mod goggles;
mod needs_update;
pub mod pathing;
mod progress_bar_config;
mod settings_struct;
pub(crate) mod source;
pub mod state;
pub mod ui;
mod v1;

pub use {
    arc::{ArcSettings, ArcVk, InvokeMethod},
    pathing::PathingSettings,
    progress_bar_config::ProgressBarSettings,
    settings_struct::{
        MarkerAutoPlaceSettings,
        MarkerSettings,
        NeedsUpdate,
        Settings,
        SettingsLock,
        SettingsSave,
        SquadCondition,
    },
    source::{
        data::DataStorage,
        sources::{DeserializedSource, RemoteAssetForm, SourceKind, SourcesFile},
        DirectSource,
        GitHubSource,
        Source,
    },
    ui::UiConfig,
    v1::{RemoteState, TimerSettings},
};

#[derive(
    Debug,
    Copy,
    Clone,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    strum::VariantArray,
)]
pub enum IconStyle {
    Plain,
    #[default]
    Scanlines1,
    #[cfg(todo)]
    Scanlines2,
    #[cfg(todo)]
    Scanlines3,
}
impl IconStyle {
    pub const DEFAULT: Self = Self::Scanlines1;
}
impl IconStyle {
    pub const NEUTRAL_ON_OFF: bool = true;

    pub fn name_id(&self) -> &'static str {
        match self {
            Self::Plain => "icon-style-plain",
            Self::Scanlines1 => "icon-style-scanlines-1",
        }
    }

    fn is_default(&self) -> bool {
        match *self {
            IconStyle::DEFAULT => true,
            _ => false,
        }
    }
}
