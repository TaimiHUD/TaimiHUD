mod bootstrap;
mod save;

pub use self::{
    bootstrap::{AddonHostName, BootstrapState, UpdatePreference},
    save::SaveState,
};
