use {
    crate::{
        controller::{
            pathing::{
                registry::{UnloadedReason, PackInfoSignature},
                shared::{PathingShared, SharedPackInfo, SharedPackConfig, SharedPackLoaded, SharedLoaderPacksInfo},
            },
            Controller,
        },
        exports::runtime::imgui::Ui,
    },
    std::{fmt, mem, sync::{Arc, Weak}},
    taimi_sync::watched::{watch, Watcher, Watched},
    taimi_pack::Pack,
};

#[derive(Debug, Default)]
pub struct PackElements {
    pub shared: Option<Arc<PathingShared>>,
    pub packs_rx: Watcher<SharedLoaderPacksInfo>,
    pub pack_state: Vec<PackElementState>,
}
impl PackElements {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pre_draw(&mut self) {
        if self.shared.is_none() {
            Controller::with_sender(|s| if let Some(pathing) = &s.pathing {
                self.shared = Some(pathing.shared.clone());
            });
            if let Some(shared) = &self.shared {
                self.packs_rx.init_sender(shared.packs.packs.clone());
                let _ = self.packs_rx.try_mark_changed();
            }
        }
        let Some(shared) = &self.shared else { return };
        if let Some(packs) = self.packs_rx.try_read_if_changed() {
            log::debug!("TODO: packs damage report and setup etc");
        }
    }
}

pub struct PackElement {
}

impl PackElement {
    pub fn draw(&mut self, ui: &Ui, state: &mut PackElementState) {
    }
}

#[derive(Debug)]
pub struct PackElementState {
    damage: PackDamageReport,
    pub info: Arc<SharedPackInfo>,
    pub config: Watched<SharedPackConfig>,
    pub loaded: watch::Receiver<SharedPackLoaded>,
    pub unloaded: Option<UnloadedReason>,
    pub pack: Option<Weak<Pack>>,
}
impl PackElementState {
    pub fn pre_draw(&mut self) -> PackDamageReport {
        let mut damage = mem::take(&mut self.damage);
        if let Some(_config) = self.config.try_read_if_changed() {
            damage.config = true;
        }
        if self.loaded.has_changed().unwrap_or(false) {
            let loaded = self.loaded.borrow_and_update();
            damage.loaded = true;
            self.unloaded = loaded.unloaded.clone();
            self.pack = loaded.pack.as_ref().map(Arc::downgrade);
        }

        damage
    }
}
#[derive(Debug, Clone, Default)]
struct PackDamageReport {
    info: Option<PackInfoSignature>,
    config: bool,
    loaded: bool,
}
