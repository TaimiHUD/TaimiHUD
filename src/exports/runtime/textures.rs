#[cfg(feature = "extension-nexus")]
use glamour::Size2;
#[cfg(feature = "texture-loader")]
use {
    anyhow::{anyhow, Context},
    std::{io, thread},
    windows::Win32::Graphics::Dxgi::Common::{self as dxgi, DXGI_FORMAT},
};
use {
    std::{
        collections::{hash_map, HashMap},
        future::Future,
        mem,
        path::PathBuf,
        sync::{Arc, RwLock as StdRwLock},
    },
    taimi_d3d::dx11::buffer::TextureView2,
    tokio::sync::{self, mpsc, RwLock},
};

#[cfg(feature = "texture-loader")]
pub use crate::resources::Texture;

pub type TextureMap = HashMap<Arc<str>, TextureSlot>;

pub struct TextureLoader {
    pub textures: StdRwLock<TextureMap>,
    #[cfg(feature = "texture-loader")]
    loader: StdRwLock<Option<TextureLoaderHandle>>,
}

impl TextureLoader {
    pub fn new() -> Self {
        Self {
            textures: Default::default(),
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
        key: &Arc<str>,
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
        key: &Arc<str>,
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

    #[cfg(feature = "texture-loader")]
    pub fn lookup_resource(&self, key: &str) -> Option<Option<Arc<Texture>>> {
        let textures = match self.textures.try_read() {
            Ok(t) => t,
            // temporary failure, just pretend it's loading or something
            Err(..) => return Some(None),
        };
        textures.get(key).map(|texture| texture.resource())
    }

    pub fn lookup_imgui(&self, key: &str) -> Option<Option<ImguiTexture>> {
        let textures = match self.textures.try_read() {
            Ok(t) => t,
            // temporary failure, just pretend it's loading or something
            Err(..) => return Some(None),
        };
        textures.get(key).map(|texture| texture.imgui_texture())
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

    pub async fn request_load_file<K: Into<Arc<str>>, P: Into<PathBuf>>(
        &self,
        key: K,
        path: P,
    ) -> anyhow::Result<()> {
        let key = key.into();
        self.begin_load(&key, || TextureRequest::LoadFile {
            key: key.clone(),
            path: path.into(),
        })
        .await
    }
    #[cfg(feature = "texture-loader")]
    pub fn request_begin_file<K: Into<Arc<str>>, P: Into<PathBuf>>(
        &self,
        key: K,
        path: P,
    ) -> anyhow::Result<()> {
        let key = key.into();
        let req = TextureRequest::LoadFile { key: key.clone(), path: path.into() };
        let sender = self.with_loader(|loader| loader.sender.clone())?;
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
        let request = sender
            .blocking_send(req)
            .map_err(|_| anyhow!("texture loader unavailable"));
        match request {
            Ok(()) => Ok(()),
            Err(e) => {
                if let Ok(mut textures) = self.textures.write() {
                    textures.insert(key.clone(), TextureSlot::Unavailable);
                }
                Err(e)
            },
        }
    }

    pub async fn request_load_bytes<K: Into<Arc<str>>, D: Into<Vec<u8>>>(
        &self,
        key: K,
        bytes: D,
    ) -> anyhow::Result<()> {
        let key = key.into();
        self.begin_load(&key, || TextureRequest::LoadBytes {
            key: key.clone(),
            bytes: bytes.into(),
        })
        .await
    }

    pub fn report_load<K: Into<Arc<str>>, T: Into<TextureSlot>>(&self, key: K, texture: anyhow::Result<T>) {
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

    pub fn report_failure<K: Into<Arc<str>>>(&self, key: K) {
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
                    // this will never deallocate (and we do want decrement the SRV refcounts anyway)
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

#[derive(Debug)]
pub enum TextureSlot {
    Loading,
    Unavailable,
    /// TODO: Arc is unnecessary but it's more compatible with Texture::load so...
    #[cfg(feature = "texture-loader")]
    Loaded(Arc<Texture>),
    #[cfg(feature = "extension-nexus")]
    Nexus(NexusTexture),
}
/// it's complicated...
#[cfg(feature = "extension-nexus")]
unsafe impl Send for TextureSlot {}
#[cfg(feature = "extension-nexus")]
unsafe impl Sync for TextureSlot {}

impl TextureSlot {
    pub fn resource_view(&self) -> Option<&TextureView2> {
        match self {
            #[cfg(feature = "texture-loader")]
            Self::Loaded(t) => Some(&t.view),
            #[cfg(feature = "extension-nexus")]
            Self::Nexus(t) => t.resource().as_ref(),
            _ => None,
        }
    }

    pub fn imgui_texture(&self) -> Option<ImguiTexture> {
        let id = self.resource_view().map(|resource| resource.clone());

        Some(match self {
            #[cfg(feature = "texture-loader")]
            Self::Loaded(t) => ImguiTexture {
                id,
                size: {
                    let [w, h] = t.dimensions;
                    Size2::new(w as f32, h as f32)
                },
            },
            #[cfg(feature = "extension-nexus")]
            Self::Nexus(t) => ImguiTexture { id, size: t.size() },
            _ => return None,
        })
    }

    pub fn resource(&self) -> Option<Arc<Texture>> {
        match self {
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
            _ => false,
        }
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

#[cfg(feature = "extension-nexus")]
#[derive(Debug)]
#[repr(transparent)]
pub struct NexusTexture(mem::ManuallyDrop<nexus::texture::Texture>);
#[cfg(feature = "extension-nexus")]
impl NexusTexture {
    #[inline(always)]
    pub const fn from_nexus(texture: nexus::texture::Texture) -> Self {
        Self(mem::ManuallyDrop::new(texture))
    }
    #[inline]
    pub fn new(srv: TextureView2, size: Size2<u32>) -> Self {
        let Size2 { width, height } = size;
        Self::from_nexus(nexus::texture::Texture { resource: srv.into(), width, height })
    }
    #[inline(always)]
    pub fn resource(&self) -> &Option<TextureView2> {
        unsafe { mem::transmute(&raw const self.0.resource) }
    }
    #[inline]
    pub fn size_u32(&self) -> Size2<u32> {
        Size2::new(self.0.width, self.0.height)
    }
    #[inline]
    pub fn size(&self) -> Size2<f32> {
        self.size_u32().as_()
    }
}
#[cfg(feature = "extension-nexus")]
impl Clone for NexusTexture {
    fn clone(&self) -> Self {
        Self::from_nexus({
            let Size2 { width, height } = self.size_u32();
            let resource = self.resource().clone();
            nexus::texture::Texture {
                width,
                height,
                resource: unsafe { mem::transmute(resource) },
            }
        })
    }
}
#[cfg(feature = "extension-nexus")]
impl Drop for NexusTexture {
    fn drop(&mut self) {
        if let Some(resource) = self.resource() {
            use windows::core::Interface;
            let ptr = resource.as_d3d_raw().as_ptr();
            unsafe {
                (resource.as_d3d().vtable().base__.base__.base__.Release)(ptr);
            }
        }
    }
}
#[cfg(feature = "extension-nexus")]
impl From<NexusTexture> for TextureSlot {
    fn from(texture: NexusTexture) -> Self {
        Self::Nexus(texture)
    }
}
#[cfg(feature = "extension-nexus")]
unsafe impl Send for NexusTexture {}
#[cfg(feature = "extension-nexus")]
unsafe impl Sync for NexusTexture {}

#[derive(Debug, Clone)]
pub struct ImguiTexture {
    pub id: Option<TextureView2>,
    pub size: Size2<f32>,
}
impl ImguiTexture {
    #[inline(always)]
    pub fn im_size(&self) -> taimi_ui::im::ui::ImSize2 {
        self.size.cast()
    }
}

impl Default for ImguiTexture {
    fn default() -> Self {
        Self {
            id: Default::default(),
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
    LoadFile { key: Arc<str>, path: PathBuf },
    LoadBytes { key: Arc<str>, bytes: Vec<u8> },
    Shutdown,
}

#[cfg(feature = "texture-loader")]
impl TextureRequest {
    #[cfg(todo)]
    pub fn key(&self) -> Option<&Arc<str>> {
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
        key: Arc<str>,
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
        key: Arc<str>,
        pixels: Vec<u8>,
        stride: usize,
        dimensions: [u32; 2],
        format: DXGI_FORMAT,
    },
    DecodeFailed {
        key: Arc<str>,
        error: anyhow::Error,
    },
    LoopEnter {
        id: thread::ThreadId,
    },
    LoopExit {
        id: thread::ThreadId,
    },
}
