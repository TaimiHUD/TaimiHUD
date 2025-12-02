use {
    crate::{
        controller::pathing::registry::{
            ActivePack,
            PackPath, PackIndex,
            PackInfo,
            PackConfig,
            LoadedPackInfo,
        },
        exports::runtime as rt,
    },
    std::{iter, mem, sync::{Arc, Weak}},
    tokio::sync::watch,
};

pub type SharedLoaderPackInfo = Box<[LoadedPackInfo]>;
pub type SharedLoaderPackData = Box<[Weak<ActivePack>]>;
pub type SharedLoaderPackConfig = Box<[Option<watch::Sender<Arc<PackConfig>>>]>;
#[derive(Debug)]
pub struct SharedPacks {
    pub info: watch::Sender<SharedLoaderPackInfo>,
    pub data: watch::Sender<SharedLoaderPackData>,
    pub config: watch::Sender<SharedLoaderPackConfig>,
}

impl SharedPacks {
    pub fn new() -> Self {
        Self {
            info: Default::default(),
            data: Default::default(),
            config: Default::default(),
        }
    }

    pub fn packs<I: IntoIterator>(packs: I) -> impl Iterator<Item = (PackPath, I::Item)> {
        packs.into_iter().enumerate()
            .map(|(i, p)| (PackPath::with_path(i as PackIndex), p))
    }

    pub fn pack_at<D>(packs: &[D], path: PackPath) -> Option<&D> {
        let idx = path.path as usize;
        packs.get(idx)
    }
    pub fn try_shared_pack_active(packs: &[Weak<ActivePack>], path: PackPath) -> Option<Arc<ActivePack>> {
        Self::pack_at(packs, path)
            .and_then(Weak::upgrade)
    }
    /// TODO: keeping this as a marker to indicate the loader may be able to load on-demand later on
    /// (and anything still using this probably wants to switch to that instead)
    pub fn pack_active(packs: &[Weak<ActivePack>], path: PackPath) -> Option<Arc<ActivePack>> {
        Self::try_shared_pack_active(packs, path)
    }

    pub(crate) fn update_pack_info(&self, path: PackPath, pack: &LoadedPackInfo) {
        self.info.send_if_modified(|shared| {
            let idx = path.path as usize;
            let amt = shared.len();
            match shared.get_mut(idx) {
                Some(out) => match *out == *pack {
                    true => false,
                    false => {
                        out.clone_from(pack);
                        true
                    },
                },
                None if idx == amt => {
                    let info = Vec::from(mem::take(shared));
                    *shared = info.into_iter()
                        .chain(iter::once(pack.clone()))
                        .collect();
                    true
                },
                None => {
                    log::error!("shared updates incomplete, can't reach {}", pack.index);
                    false
                },
            }
        });
    }

    pub(crate) fn update_pack_config(&self, path: PackPath, config: Option<&watch::Sender<Arc<PackConfig>>>) {
        let Some(config) = config else { return };
        let idx = path.path as usize;
        self.config.send_if_modified(|shared| match shared.get_mut(idx) {
            Some(Some(out)) if config.same_channel(out) =>
                false,
            Some(out) => {
                *out = Some(config.clone());
                true
            },
            None => {
                let mut configs = Vec::from(mem::take(shared));
                configs.resize_with(idx, || None);
                configs.push(Some(config.clone()));
                *shared = configs.into_boxed_slice();
                true
            },
        });
    }

    pub(crate) fn update_pack_active(&self, path: PackPath, pack: Option<&Arc<ActivePack>>) {
        self.data.send_if_modified(|shared| {
            let pack_shared = || pack.map(Arc::downgrade).unwrap_or(Weak::new());
            let idx = path.path as usize;
            match shared.get_mut(idx) {
                Some(out) => {
                    match pack.map(Arc::as_ptr) {
                        Some(p) if p == Weak::as_ptr(out) =>
                            false,
                        None if Weak::ptr_eq(&*out, &Weak::new()) =>
                            false,
                        _ => {
                            *out = pack_shared();
                            true
                        },
                    }
                },
                None => {
                    let mut info = Vec::from(mem::take(shared));
                    info.resize_with(idx, || Weak::new());
                    *shared = info.into_iter()
                        .chain(iter::once(pack_shared()))
                        .collect();
                    true
                },
            }
        });
    }
}
