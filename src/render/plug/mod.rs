// incomplete WIP, no point in cleaning it up yet
#![cfg_attr(not(taimi_debug = "wip"), allow(nonstandard_style, unused, unexpected_cfgs))]

use {
    crate::controller::{script::PlugsShared, Controller},
    taimi_sync::watched::Watched,
};

mod config;

pub use self::config::{PlugConfig, PlugConfigCache, PlugConfigDesc, PlugConfigState};

#[derive(Debug, Default)]
pub struct PlugElements {
    pub enabled: bool,
    pub plugs_rx: Watched<PlugsShared>,
    pub plugs_rx_dirty: bool,
}
impl PlugElements {
    pub fn pre_render(&mut self) {
        if !self.plugs_rx.is_watching() {
            let subscribed = Controller::with_sender(|s| {
                s.scripting.as_ref().map(|s| {
                    self.plugs_rx.resubscribe_to(&s.plugs_shared);
                })
            })
            .flatten();
            if subscribed.is_none() {
                return
            }
            self.enabled = crate::SETTINGS
                .get()
                .and_then(|s| s.blocking_read().pathing.as_ref().map(|p| p.scripting_enable))
                .unwrap_or(false);
        }
        if let Some(plugs) = self.plugs_rx.try_read_if_changed() {
            self.plugs_rx_dirty |= true;
            #[cfg(feature = "paths-lua")]
            {
                self.enabled |= !plugs.available_packs.is_empty();
            }
        }
    }
    /// TODO: this dirty field is a hack, if PackElements uses this just give it its own watcher instead
    pub(crate) fn process_dirty_for_packs(&mut self) -> bool {
        let dirty = self.plugs_rx_dirty;
        self.plugs_rx_dirty = false;
        dirty
    }
}
