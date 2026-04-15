#[cfg(feature = "extension-nexus")]
pub use nexus::texture::Texture as NexusTexture;
#[cfg(feature = "texture-loader")]
use {
    anyhow::{anyhow, Context},
    std::{io, sync::Weak, thread},
    taimi_hoard::collections::lru::RecentlyUsed,
    taimi_sync::arcs::weak_is_null,
    windows::Win32::Graphics::Dxgi::Common::{self as dxgi, DXGI_FORMAT},
};
use {
    nexus::imgui::TextureId,
    std::{
        collections::{hash_map, HashMap},
        future::Future,
        mem,
        path::PathBuf,
        sync::{Arc, RwLock as StdRwLock},
    },
    taimi_d3d::dx11::{buffer::TextureView2, prelude::*},
    tokio::sync::{self, mpsc, RwLock},
};

#[cfg(feature = "texture-loader")]
pub use crate::resources::Texture;

pub type TextureKey = Arc<str>;
pub type TextureMap = HashMap<TextureKey, TextureSlot>;
#[cfg(feature = "texture-loader")]
type GarbageMap = HashMap<TextureKey, RecentlyUsed>;

pub struct TextureLoader {
    pub textures: StdRwLock<TextureMap>,
    #[cfg(feature = "texture-loader")]
    garbage: StdRwLock<GarbageMap>,
    #[cfg(feature = "texture-loader")]
    loader: StdRwLock<Option<TextureLoaderHandle>>,
}

impl TextureLoader {
    pub fn new() -> Self {
        Self {
            textures: Default::default(),
            garbage: Default::default(),
            #[cfg(feature = "texture-loader")]
            loader: Default::default(),
        }
    }

    #[cfg(feature = "texture-loader")]
    pub fn setup(&self) -> Result<(), &'static str> {
        let mut loader = self.loader.write().map_err(|_| "texture loader poisoned")?;
        if loader.is_some() {
            return Err("texture loader already running")
        }
        *loader = Some(Self::setup_loader());
        Ok(())
    }

    #[cfg(feature = "texture-loader")]
    fn setup_loader() -> TextureLoaderHandle {
        let (tx_request, rx_request) = mpsc::channel(32);
        let (tx_response, rx_response) = mpsc::channel(32);
        let background = thread::spawn({
            #[cfg(todo)]
            let tx_response = tx_response.clone();
            move || Self::background_loop(rx_request, tx_response)
        });
        TextureLoaderHandle {
            background,
            sender: tx_request,
            upload_queue: RwLock::new(rx_response),
            #[cfg(todo)]
            upload_queue_sender: tx_response,
        }
    }

    #[cfg(feature = "texture-loader")]
    pub fn wait_for_startup(&self) -> anyhow::Result<()> {
        match self.blocking_responses(|mut responses| responses.blocking_recv())? {
            Some(TextureResponse::LoopEnter { id }) => {
                log::debug!("texture loader {id:?} started");
                Ok(())
            },
            _ => Err(anyhow!("texture loader thread failed to start")),
        }
    }

    pub fn is_available(&self) -> bool {
        match () {
            #[cfg(feature = "texture-loader")]
            _ => self
                .loader
                .try_read()
                .map(|loader| loader.is_some())
                .unwrap_or(false),
            #[cfg(not(feature = "texture-loader"))]
            _ => false,
        }
    }

    /// XXX: technically blocking but only ever written to at shutdown, so...
    #[cfg(feature = "texture-loader")]
    pub fn with_loader<R>(&self, f: impl FnOnce(&TextureLoaderHandle) -> R) -> anyhow::Result<R> {
        match *self.read_loader()? {
            Some(ref loader) => Ok(f(loader)),
            None => Err(anyhow!("texture loader shut down")),
        }
    }

    #[cfg(feature = "texture-loader")]
    pub fn read_loader(
        &self,
    ) -> anyhow::Result<std::sync::RwLockReadGuard<'_, Option<TextureLoaderHandle>>> {
        self.loader.read().map_err(|_| anyhow!("texture loader poisoned"))
    }

    pub async fn report_begin_load(
        &self,
        key: &TextureKey,
        request: impl Future<Output = anyhow::Result<()>>,
    ) -> anyhow::Result<()> {
        {
            let mut textures = self.textures.write().ok().context("texture map poisoned")?;
            let entry = textures.entry(key.clone());
            match entry {
                hash_map::Entry::Occupied(e) if !e.get().can_load() =>
                    return Err(anyhow!("duplicate texture load request")),
                hash_map::Entry::Occupied(mut e) => {
                    e.insert(TextureSlot::Loading);
                },
                hash_map::Entry::Vacant(e) => {
                    e.insert(TextureSlot::Loading);
                },
            }
        }
        match request.await {
            Ok(()) => Ok(()),
            Err(e) => {
                if let Ok(mut textures) = self.textures.write() {
                    textures.insert(key.clone(), TextureSlot::Unavailable);
                }
                Err(e)
            },
        }
    }
    async fn begin_load(
        &self,
        key: &TextureKey,
        request: impl FnOnce() -> TextureRequest,
    ) -> anyhow::Result<()> {
        #[cfg(feature = "texture-loader")]
        let sender = self.with_loader(|loader| loader.sender.clone())?;
        self.report_begin_load(key, async move {
            sender
                .send(request())
                .await
                .map_err(|_| anyhow!("texture loader unavailable"))
        })
        .await
    }

    pub fn lookup_pair_with<R, F: FnOnce(Option<&TextureKey>, &TextureSlot) -> R>(
        &self,
        key: &str,
        f: F,
    ) -> Option<R> {
        let textures = match &self.textures {
            // write locks are held so infrequently that we shouldn't need to care
            // (and if we need to, switch to a lock-free map instead or just cache the slot)
            textures => textures.read(),
            #[cfg(todo = "unnecessary")]
            textures => textures.try_read(),
        };
        let textures = match textures {
            Ok(t) => t,
            // temporary failure, just pretend it's loading or something
            Err(..) => return Some(f(None, &TextureSlot::Loading)),
        };
        textures.get_key_value(key).map(|(k, i)| f(Some(k), i))
    }
    pub fn lookup_with<R, F: FnOnce(&TextureSlot) -> R>(&self, key: &str, f: F) -> Option<R> {
        let mut attention = false;
        let res = self.lookup_pair_with(key, |_k, i| {
            attention = i.needs_attention();
            f(i)
        });
        if attention {
            let mut textures = self.textures.write().ok();
            let tex = textures.as_mut().and_then(|textures| textures.get_mut(key));
            let mut prune = false;
            if let Some(tex) = tex {
                if tex.try_activate().is_none() && matches!(tex, TextureSlot::Inactive(..)) {
                    prune = true;
                }
            }
            match (prune, textures) {
                (true, Some(mut textures)) => {
                    textures.remove(key);
                },
                _ => (),
            }
        }
        res
    }
    /// `Some(None)` if texture isn't ready yet
    pub fn lookup_loaded(&self, key: &str) -> Option<Option<TextureSlot>> {
        self.lookup_with(key, |i| {
            let ready = !matches!(i, TextureSlot::Loading | TextureSlot::Reserved);
            (ready).then_some(i.clone())
        })
    }
    pub fn lookup_slot(&self, key: &str) -> Option<TextureSlot> {
        let textures = match self.textures.read() {
            Ok(t) => t,
            // poisoned, goodbye
            Err(..) => return Some(TextureSlot::Unavailable),
        };
        textures.get(key).cloned()
    }
    #[cfg(feature = "texture-loader")]
    pub fn lookup_resource(&self, key: &str) -> Option<Option<Arc<Texture>>> {
        self.lookup_with(key, |t| t.resource())
    }

    pub fn lookup_imgui(&self, key: &str) -> Option<Option<ImguiTexture>> {
        let res = self.lookup_with(key, |t| t.imgui_texture());
        #[cfg(feature = "texture-loader")]
        if let Some(Some(..)) = res {
            self.mark_used(key);
        }
        res
    }

    #[cfg(feature = "texture-loader")]
    pub fn mark_used(&self, key: &str) {
        match self.garbage.read() {
            Ok(garbage)
                if garbage.get(key).map(|used| used.generation).unwrap_or(0) != 0 =>
                (),
            _ => return,
        }
        if let Ok(mut garbage) = self.garbage.write() {
            garbage.remove(key);
        }
    }
    #[cfg(feature = "texture-loader")]
    pub fn mark_used_many<'a, I>(&self, keys: I) where
        I: IntoIterator<Item = &'a str>,
    {
        let keys = keys.into_iter();
        if let Ok(mut garbage) = self.garbage.write() {
            for key in keys {
                garbage.remove(key);
            }
        }
    }
    #[cfg(feature = "texture-loader")]
    pub fn collect_garbage(&self) {
        #[derive(Copy, Clone)]
        enum SlotStatus {
            Unused,
            Dead,
            Weak,
        }
        let mut unused = HashMap::new();
        if let Ok(textures) = self.textures.read() {
            unused.extend(textures.iter().filter_map(|(key, slot)| match slot {
                TextureSlot::Loaded(tex) if Arc::strong_count(tex) == 1 =>
                    Some((key.clone(), SlotStatus::Unused)),
                TextureSlot::Inactive(weak) if weak.strong_count() == 0 =>
                    Some((key.clone(), SlotStatus::Dead)),
                TextureSlot::Inactive(..) =>
                    Some((key.clone(), SlotStatus::Weak)),
                _ => None,
            }));
        }
        let mut deceased = Vec::new();
        if let Ok(mut garbage) = self.garbage.write() {
            garbage.retain(|key, used| {
                let retain = match unused.remove(key) {
                    None if matches!(Arc::strong_count(key), 1 | 2) =>
                        return false,
                    None => {
                        used.mark_used();
                        true
                    },
                    #[cfg(todo)]
                    Some(SlotStatus::Dead) => false,
                    Some(_) => {
                        used.mark_unused();
                        !used.is_elderly(Self::GC_MAX_AGE)
                    }
                };
                if !retain {
                    deceased.push(key.clone());
                }
                retain
            });
            garbage.extend(unused.into_iter().filter_map(|(key, status)| match status {
                SlotStatus::Dead | SlotStatus::Weak => Some((key, RecentlyUsed {
                    generation: Self::GC_MID_AGE,
                })),
                SlotStatus::Unused => Some((key, RecentlyUsed {
                    generation: 1,
                })),
            }));
        }
        if !deceased.is_empty() {
            if let Ok(mut textures) = self.textures.write() {
                for key in deceased {
                    textures.remove(&key);
                }
            }
        }
    }
    const GC_MID_AGE: u32 = 2;
    const GC_MAX_AGE: u32 = 4;

    /// produces a texture slot unless newly reserved
    pub fn reserve_key_mut(&self, key: &mut TextureKey) -> Option<TextureSlot> {
        let mut replacement = None;
        let slot = {
            let mut textures = match self.textures.write() {
                Ok(t) => t,
                Err(..) => return None,
            };
            let ptr = Arc::as_ptr(key) as *const ();
            let entry = textures.entry(key.clone());
            match entry {
                hash_map::Entry::Occupied(e) => {
                    let key = e.key();
                    if Arc::as_ptr(key) as *const () != ptr {
                        replacement = Some(key.clone());
                    }
                    Some(e.get().clone())
                },
                hash_map::Entry::Vacant(e) => {
                    e.insert(TextureSlot::Reserved);
                    None
                },
            }
        };
        if let Some(replacement) = replacement {
            *key = replacement
        }
        slot
    }
    pub fn try_canonicalize_key(&self, key: &str) -> Option<TextureKey> {
        let textures = match self.textures.read() {
            Ok(t) => t,
            Err(..) => return None,
        };
        textures.get_key_value(key).map(|(canon, _)| canon.clone())
    }
    /// expect temporary failure due to lock contention
    pub fn try_canonicalize_key_mut(&self, key: &mut TextureKey) -> Result<bool, ()> {
        let replacement = {
            let textures = match self.textures.try_read() {
                Ok(t) => t,
                Err(..) => return Err(()),
            };
            let ptr = Arc::as_ptr(key) as *const ();
            match textures.get_key_value(key) {
                None => return Ok(false),
                Some((canon, _)) if Arc::as_ptr(canon) as *const () == ptr =>
                // already canon
                    return Ok(true),
                Some((canon, _)) => canon.clone(),
            }
        };
        *key = replacement;
        // now it is!
        Ok(true)
    }

    #[cfg(todo = "unused")]
    pub async fn request_load_file_relative<R, P>(&self, rel: R, path: P) -> anyhow::Result<()>
    where
        R: AsRef<RelativePath> + Into<String>,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let relpath = rel.as_ref();
        let base = path.parent().ok_or_else(|| {
            anyhow!(
                "parent of path {} required to load texture {relpath}",
                path.display()
            )
        })?;
        let abs = relpath.to_path(base);
        self.request_load_file(rel.into(), abs).await
    }

    pub async fn request_load_file<K: Into<TextureKey>, P: Into<PathBuf>>(
        &self,
        key: K,
        path: P,
    ) -> anyhow::Result<TextureKey> {
        let key = key.into();
        self.begin_load(&key, || TextureRequest::LoadFile {
            key: key.clone(),
            path: path.into(),
        })
        .await
        .map(move |()| key)
    }

    pub async fn request_load_bytes<K: Into<TextureKey>, D: Into<Vec<u8>>>(
        &self,
        key: K,
        bytes: D,
    ) -> anyhow::Result<TextureKey> {
        let key = key.into();
        self.begin_load(&key, || TextureRequest::LoadBytes {
            key: key.clone(),
            bytes: bytes.into(),
        })
        .await
        .map(move |()| key)
    }

    pub fn report_load<K: Into<TextureKey>, T: Into<TextureSlot>>(
        &self,
        key: K,
        texture: anyhow::Result<T>,
    ) {
        let key = key.into();
        let slot = match texture {
            Ok(slot) => slot.into(),
            Err(e) => {
                log::error!("failed to load texture {key}: {e}");
                return self.report_failure(key);
            },
        };
        if let Ok(mut textures) = self.textures.write() {
            textures.insert(key, slot);
        } else {
            log::error!("texture map poisoned");
        }
    }

    pub fn report_failure<K: Into<TextureKey>>(&self, key: K) {
        if let Ok(mut textures) = self.textures.write() {
            textures.insert(key.into(), TextureSlot::Unavailable);
        }
    }

    #[cfg(feature = "texture-loader")]
    pub fn blocking_responses<R>(
        &self,
        f: impl FnOnce(sync::RwLockWriteGuard<mpsc::Receiver<TextureResponse>>) -> R,
    ) -> anyhow::Result<R> {
        self.with_loader(|loader| f(loader.upload_queue.blocking_write()))
    }

    #[cfg(feature = "texture-loader")]
    pub fn try_responses<R>(
        &self,
        f: impl FnOnce(sync::RwLockWriteGuard<mpsc::Receiver<TextureResponse>>) -> R,
    ) -> anyhow::Result<Option<R>> {
        self.with_loader(|loader| loader.upload_queue.try_write().ok().map(f))
    }

    #[cfg(feature = "texture-loader")]
    #[cfg(todo)]
    pub async fn responses_async<R, F>(
        &self,
        f: impl FnOnce(sync::RwLockWriteGuard<mpsc::Receiver<TextureResponse>>) -> F,
    ) -> anyhow::Result<R>
    where
        F: Future<Output = R>,
    {
        match *self.read_loader()? {
            Some(ref loader) => Ok({
                let upload_queue = loader.upload_queue.write().await;
                f(upload_queue).await
            }),
            None => Err(anyhow!("texture loader shut down")),
        }
    }

    pub fn cleanup(&self, can_unload_textures: bool) {
        let unload = match can_unload_textures {
            #[cfg(feature = "texture-loader")]
            false => false,
            _ => true,
        };

        let mut textures = {
            let mut textures = self.textures.write().unwrap_or_else(|e| e.into_inner());
            mem::replace(&mut *textures, HashMap::new())
        };
        match unload {
            false => {
                textures.retain(|_key, texture| match texture {
                    #[cfg(feature = "texture-loader")]
                    TextureSlot::Loaded(..) => true,
                    // as long as Nexus is holding on to a reference of these for us later,
                    // this will never deallocate (unclear if decrementing the SRV refcounts is atomic?)
                    #[cfg(feature = "extension-nexus")]
                    TextureSlot::Nexus(..) if false => true,
                    _ => false,
                });
                if !textures.is_empty() {
                    // not our problem anymore
                    log::warn!("beware of leaky textures");
                    mem::forget(textures);
                }
            },
            true => {
                drop(textures);
            },
        }
    }
    pub fn unload_textures_matching<F: FnMut(&TextureKey, &mut TextureSlot) -> bool>(
        &self,
        immediate: bool,
        mut f: F,
    ) {
        let mut textures = self.textures.write().unwrap_or_else(|e| e.into_inner());
        textures.retain(|key, slot| {
            let remove = f(key, slot);
            match remove {
                true if !immediate => {
                    slot.deactivate(false);
                    true
                },
                remove => !remove,
            }
        });
    }

    pub fn quit(&self) {
        #[cfg(feature = "texture-loader")]
        let _ = self.with_loader(|loader| loader.sender.try_send(TextureRequest::Shutdown));
    }

    #[cfg(feature = "texture-loader")]
    pub fn shutdown(&self) -> anyhow::Result<thread::JoinHandle<anyhow::Result<()>>> {
        let loader = self.loader.write().unwrap_or_else(|e| e.into_inner()).take();
        let loader = match loader {
            Some(loader) => loader,
            None => return Err(anyhow!("texture loader already shutdown?")),
        };
        Ok(loader.shutdown())
    }

    #[cfg(feature = "texture-loader")]
    pub fn wait_for_shutdown(&self) -> anyhow::Result<()> {
        let handle = match self.shutdown() {
            Ok(h) => h,
            Err(e) => {
                // failure just means it was already shut down earlier, which is expected
                // in a number of situations
                log::debug!("{e:#}");
                return Ok(())
            },
        };
        match handle.join() {
            Ok(res) => res,
            Err(e) => Err(crate::with_any_error(&e, |e| {
                anyhow!("texture loader thread panicked: {e}")
            })),
        }
    }

    #[cfg(feature = "texture-loader")]
    fn background_loop(
        mut receiver: mpsc::Receiver<TextureRequest>,
        sender: mpsc::Sender<TextureResponse>,
    ) -> anyhow::Result<()> {
        let id = thread::current().id();
        sender
            .blocking_send(TextureResponse::LoopEnter { id })
            .map_err(|_| anyhow!("texture loader did not wait"))?;

        while let Some(request) = receiver.blocking_recv() {
            match &request {
                TextureRequest::LoadBytes { key, bytes } => log::trace!(
                    "texture loader request received: load {} bytes for {key}",
                    bytes.len()
                ),
                request => log::trace!("texture loader request received: {request:?}"),
            }

            if receiver.is_closed() || sender.is_closed() {
                // no point in processing any remaining requests
                break
            }

            let key = match &request {
                TextureRequest::Shutdown => {
                    log::debug!("texture loader received shutdown request");
                    break
                },
                TextureRequest::LoadFile { key, .. } | TextureRequest::LoadBytes { key, .. } => key.clone(),
            };

            let res = request.process_decode();
            log::trace!("texture loader decode result: {:?}", res.as_ref().map(drop));

            let sent = sender
                .blocking_send(res.unwrap_or_else(|error| TextureResponse::DecodeFailed { key, error }));

            if let Err(..) = sent {
                log::debug!("texture loader hung up");
                // no one's home, goodbye
                break
            }
        }

        let _ = sender.try_send(TextureResponse::LoopExit { id });

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum TextureSlot {
    Loading,
    Reserved,
    Unavailable,
    #[cfg(feature = "texture-loader")]
    Inactive(Weak<Texture>),
    /// TODO: Arc is unnecessary but it's more compatible with Texture::load so...
    #[cfg(feature = "texture-loader")]
    Loaded(Arc<Texture>),
    #[cfg(feature = "extension-nexus")]
    Nexus(NexusTexture),
}

impl TextureSlot {
    pub fn resource_view(&self) -> Option<&TextureView2> {
        match self {
            #[cfg(feature = "texture-loader")]
            Self::Loaded(t) => Some(&t.view),
            #[cfg(feature = "extension-nexus")]
            Self::Nexus(t) => Some(t.resource.as_ref()),
            _ => None,
        }
    }

    pub fn get_imgui_dims(&self) -> Option<[f32; 2]> {
        match self {
            #[cfg(feature = "extension-nexus")]
            Self::Nexus(t) => Some(t.size()),
            #[cfg(feature = "texture-loader")]
            Self::Loaded(t) => {
                let [w, h] = t.dimensions;
                Some([w as f32, h as f32])
            },
            _ => None,
        }
    }
    pub fn imgui_texture(&self) -> Option<ImguiTexture> {
        let id = self
            .resource_view()
            .map(|resource| TextureId::new(resource.to_ref().as_raw() as usize));
        let id = id.unwrap_or(TextureId::new(0));

        self.get_imgui_dims().map(move |size| ImguiTexture { id, size })
    }

    pub fn resource(&self) -> Option<Arc<Texture>> {
        match self {
            #[cfg(feature = "texture-loader")]
            Self::Inactive(t) => Weak::upgrade(t),
            #[cfg(feature = "texture-loader")]
            Self::Loaded(t) => Some(t.clone()),
            #[cfg(feature = "extension-nexus")]
            Self::Nexus(t) => Some({
                let texture = Texture::with_nexus(t.clone()).ok()?;
                Arc::new(texture)
            }),
            _ => None,
        }
    }

    pub fn can_load(&self) -> bool {
        match self {
            // maybe someday...
            //Self::Unloaded => true,
            Self::Reserved => true,
            Self::Inactive(..) => true,
            _ => false,
        }
    }
    /// a texture resource is wanted for use,
    /// but may need reactivation/refresh (or is a candidate for deactivation?)
    pub fn needs_attention(&self) -> bool {
        match self {
            Self::Inactive(..) => true,
            _ => false,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(*self, Self::Loading)
    }
    pub fn get(&self) -> Option<&Self> {
        match self {
            #[cfg(todo)]
            #[cfg(feature = "texture-loader")]
            TextureSlot::Inactive(..) => Some(self),
            #[cfg(feature = "texture-loader")]
            TextureSlot::Loaded(..) => Some(self),
            #[cfg(feature = "extension-nexus")]
            TextureSlot::Nexus(..) => Some(self),
            _ => None,
        }
    }

    #[cfg(feature = "texture-loader")]
    pub fn deactivate(&mut self, prune: bool) -> bool {
        let prev = match self {
            Self::Loading | Self::Reserved | Self::Unavailable => return false,
            Self::Inactive(_t) => {
                #[cfg(todo = "unnecessary")]
                if _t.strong_count() > 0 {
                    *_t = Weak::new();
                }
                return true
            },
            #[cfg(feature = "extension-nexus")]
            Self::Nexus(..) => None,
            Self::Loaded(t) => Some(Arc::downgrade(&*t)),
        }
        .unwrap_or(Weak::new());
        let prev = self.insert_inactive(prev);
        if prev.strong_count() == 0 {
            *prev = Weak::new();
        }
        true
    }
    #[cfg(feature = "texture-loader")]
    pub fn insert_inactive(&mut self, prev: Weak<Texture>) -> &mut Weak<Texture> {
        *self = Self::Inactive(prev);
        match self {
            Self::Inactive(prev) => prev,
            _ => unsafe { core::hint::unreachable_unchecked() },
        }
    }
    #[cfg(feature = "texture-loader")]
    pub fn insert_loaded(&mut self, texture: Arc<Texture>) -> &mut Arc<Texture> {
        *self = Self::Loaded(texture);
        match self {
            Self::Loaded(texture) => texture,
            _ => unsafe { core::hint::unreachable_unchecked() },
        }
    }
    #[cfg(feature = "texture-loader")]
    pub fn try_activate(&mut self) -> Option<&mut Arc<Texture>> {
        let tex = match self {
            Self::Inactive(t) if weak_is_null(t) => None,
            Self::Inactive(t) => Some(Weak::upgrade(&*t)),
            _ => None,
        };
        match tex {
            Some(Some(tex)) => Some(self.insert_loaded(tex)),
            Some(None) => {
                self.insert_inactive(Weak::new());
                None
            },
            None => match self {
                Self::Loaded(tex) => Some(tex),
                _ => None,
            },
        }
    }
    #[cfg(todo)]
    pub fn prune(&mut self) {}

    pub fn diag_texture_byte_size(&self) -> usize {
        match self {
            Self::Loaded(tex) => tex.texture_byte_size(),
            Self::Inactive(tex) if tex.strong_count() > 0 =>
                Weak::upgrade(tex).map(|tex| tex.texture_byte_size()).unwrap_or(0),
            Self::Nexus(tex) => {
                use taimi_d3d::dx11::buffer::ShaderResourceView;
                let bpp =
                    Texture::format_bpp(ShaderResourceView::from_d3d_ref(&tex.resource).get_desc().Format);
                bpp.saturating_mul(tex.width as usize)
                    .saturating_mul(tex.height as usize)
            },
            _ => 0,
        }
    }
}

#[cfg(feature = "extension-nexus")]
impl From<NexusTexture> for TextureSlot {
    fn from(texture: NexusTexture) -> Self {
        Self::Nexus(texture)
    }
}
#[cfg(feature = "texture-loader")]
impl From<Texture> for TextureSlot {
    fn from(texture: Texture) -> Self {
        Self::Loaded(texture.into())
    }
}
#[cfg(feature = "texture-loader")]
impl From<Arc<Texture>> for TextureSlot {
    fn from(texture: Arc<Texture>) -> Self {
        Self::Loaded(texture)
    }
}
#[cfg(feature = "texture-loader")]
impl From<Weak<Texture>> for TextureSlot {
    fn from(texture: Weak<Texture>) -> Self {
        Self::Inactive(texture)
    }
}

#[derive(Debug, Clone)]
pub struct ImguiTexture {
    pub id: TextureId,
    pub size: [f32; 2],
}

impl Default for ImguiTexture {
    fn default() -> Self {
        Self {
            id: TextureId::new(0),
            size: Default::default(),
        }
    }
}

#[cfg(feature = "texture-loader")]
pub struct TextureLoaderHandle {
    pub background: thread::JoinHandle<anyhow::Result<()>>,
    pub sender: mpsc::Sender<TextureRequest>,
    pub upload_queue: RwLock<mpsc::Receiver<TextureResponse>>,
    #[cfg(todo)]
    pub upload_queue_sender: mpsc::Sender<TextureResponse>,
}

#[cfg(feature = "texture-loader")]
impl TextureLoaderHandle {
    pub fn shutdown(self) -> thread::JoinHandle<anyhow::Result<()>> {
        let _ = self.sender.try_send(TextureRequest::Shutdown);
        //let _ = self.upload_queue_sender.try_send(TextureResponse::ExitShutdown);
        self.background
    }
}

#[cfg(feature = "texture-loader")]
#[derive(Debug, Clone)]
pub enum TextureRequest {
    LoadFile { key: TextureKey, path: PathBuf },
    LoadBytes { key: TextureKey, bytes: Vec<u8> },
    Shutdown,
}

#[cfg(feature = "texture-loader")]
impl TextureRequest {
    #[cfg(todo)]
    pub fn key(&self) -> Option<&TextureKey> {
        Some(match self {
            Self::LoadFile { key, .. } | Self::LoadBytes { key, .. } => key,
            _ => return None,
        })
    }

    pub fn process_decode(self) -> anyhow::Result<TextureResponse> {
        match self {
            #[cfg(feature = "image")]
            Self::LoadFile { key, path } => Self::decode_image_read(image::ImageReader::open(path)?, key),
            Self::LoadBytes { key, bytes } => {
                let mut bytes = &bytes[..];
                let read = io::Cursor::new(&mut bytes);
                Self::decode_image_read(image::ImageReader::new(read), key)
            },
            _ => return Err(anyhow!("cannot decode {self:?}")),
        }
    }

    #[cfg(feature = "image")]
    fn decode_image_read<R: io::BufRead + io::Seek>(
        image: image::ImageReader<R>,
        key: TextureKey,
    ) -> anyhow::Result<TextureResponse> {
        let image = image
            .with_guessed_format()
            .with_context(|| format!("loading texture {key}"))?;
        log::debug!("Loading {:?} texture for {key}", image.format());

        let image = image
            .decode()
            .with_context(|| format!("decoding texture {key}"))?;

        let rgba8 = image.to_rgba8().into_flat_samples();

        Ok(TextureResponse::Decoded {
            key,
            // TODO: Is sRGB correct?
            format: dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
            dimensions: [rgba8.layout.width, rgba8.layout.height],
            stride: rgba8.layout.height_stride,
            pixels: rgba8.samples,
        })
    }
}

#[cfg(feature = "texture-loader")]
#[derive(Debug)]
pub enum TextureResponse {
    Decoded {
        key: TextureKey,
        pixels: Vec<u8>,
        stride: usize,
        dimensions: [u32; 2],
        format: DXGI_FORMAT,
    },
    DecodeFailed {
        key: TextureKey,
        error: anyhow::Error,
    },
    LoopEnter {
        id: thread::ThreadId,
    },
    LoopExit {
        id: thread::ThreadId,
    },
}
