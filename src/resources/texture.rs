use {
    anyhow::{anyhow, Context as _},
    crate::exports::runtime::Counter,
    glam::Vec4,
    std::{fmt, path::Path, sync::Arc},
    nexus::texture::Texture as NexusTexture,
    taimi_d3d::{
        dx11::{
            prelude::*,
            buffer::{D3D11_TEXTURE2D_DESC, BindFlags, Texture2, TextureView2, Usage},
        },
        D3dContextBindableSlot,
    },
};
#[cfg(feature = "image")]
use image::{ImageReader, FlatSamples};
#[cfg(feature = "statistics")]
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, PartialEq)]
pub struct Texture {
    pub texture: Texture2,
    pub dimensions: [u32; 2],
    pub view: TextureView2,
}

impl Texture {
    #[deprecated = "crate::texture_schedule_path"]
    pub fn load(device: &Dx11Device, path: &Path) -> anyhow::Result<Arc<Self>> {
        use crate::TEXTURES;
        let key = path.to_string_lossy();

        if let Some(texture) = TEXTURES.lookup_resource(&key) {
            match texture {
                Some(texture) => {
                    log::debug!("deprecated texture interface used for {path:?}");
                    Ok(texture)
                },
                None => {
                    Err(anyhow!("texture {path:?} isn't done loading"))
                },
            }
        } else {
            let texture = Self::new_path(device, path)?;
            let mut textures = TEXTURES.textures.blocking_write();
            let texture = Arc::new(texture);
            textures.insert(key.into(), texture.clone().into());
            Ok(texture)
        }
    }

    pub fn new_bytes<D: fmt::Debug>(device: &Dx11Device, mut bytes: &[u8], name: D) -> anyhow::Result<Self> {
        let read = std::io::Cursor::new(&mut bytes);
        Self::new_image(device, ImageReader::new(read))
            .with_context(|| {
                format!("loading texture {:?}", name)
            })
    }

    pub fn new_path(device: &Dx11Device, path: &Path) -> anyhow::Result<Self> {
        let image_reader = ImageReader::open(path)?;
        Self::new_image(device, image_reader)
            .with_context(|| {
                let filename = path.file_name()
                    .unwrap_or(path.as_os_str());
                format!("loading texture from {}", filename.display())
            })
    }

    const DESC_TEXTURE: D3D11_TEXTURE2D_DESC = D3D11_TEXTURE2D_DESC {
        Width: 0,
        Height: 0,
        Format: dxgi::DXGI_FORMAT_UNKNOWN,
        MipLevels: 1,
        ArraySize: 1,
        SampleDesc: Texture2::DEFAULT_SAMPLE_DESC,
        Usage: Usage::DEFAULT.to_d3d(),
        BindFlags: BindFlags::SHADER_RENDER.to_uint(),
        CPUAccessFlags: 0,
        MiscFlags: d3d11::D3D11_RESOURCE_MISC_GENERATE_MIPS.0 as u32,
    };

    #[cfg(feature = "image")]
    pub fn new_image<R>(device: &Dx11Device, image_reader: ImageReader<R>) -> anyhow::Result<Self> where
        R: std::io::BufRead + std::io::Seek,
    {
        let format = image_reader.format();
        let image = image_reader.with_guessed_format()?.decode()?;
        let rgba_image = image.to_rgba32f();
        let dimensions = rgba_image.dimensions();
        let raw_rgba_image = rgba_image.into_raw();
        let desc = D3D11_TEXTURE2D_DESC {
            Width: dimensions.0,
            Height: dimensions.1,
            Format: dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT,
            .. Self::DESC_TEXTURE
        };
        let samples: &[Vec4] = unsafe {
            use std::slice::from_raw_parts;
            from_raw_parts(raw_rgba_image.as_ptr() as *const Vec4, raw_rgba_image.len() / 4)
        };
        let texture = Texture2::new_with_desc(device, &desc, Some(samples))?;
        let view = TextureView2::new_with_texture2(device, &texture, None)?;
        Ok(Self {
            texture,
            view,
            dimensions: dimensions.into(),
        })
    }

    #[cfg(feature = "extension-nexus")]
    pub fn with_nexus(texture: NexusTexture) -> anyhow::Result<Self> {
        let [w, h] = texture.size();
        let view = TextureView2::from_d3d(texture.resource);
        let texture = view.get_resource()?;
        let texture = Self {
            dimensions: [w as u32, h as u32],
            texture,
            view,
        };
        STATS_TEXTURE_COUNT.increment(1);
        let _size = STATS_TEXTURE_SIZE.increment_by(|| texture.texture_byte_size());
        STATS_TEXTURE_SIZE_CLONED.increment(_size);
        Ok(texture)
    }

    pub fn to_nexus(&self) -> Option<NexusTexture> {
        let resource = self.view.clone().into();
        Some(NexusTexture {
            resource,
            width: self.dimensions[0],
            height: self.dimensions[1],
        })
    }

    #[cfg(feature = "image")]
    pub fn load_rgba8_uncached(
        device: &Dx11Device,
        image: FlatSamples<Vec<u8>>,
    ) -> anyhow::Result<Texture> {
        Self::new_rgba8(
            device,
            &image.samples,
            [image.layout.width, image.layout.height],
            image.layout.height_stride,
        )
    }

    pub fn new_rgba8(
        device: &Dx11Device,
        image: &[u8],
        dimensions: [u32; 2],
        stride: usize,
    ) -> anyhow::Result<Texture> {
        // TODO: Is sRGB correct?
        debug_assert!(stride >= dimensions[0] as usize * 4);
        unsafe {
            Self::new_raw(device, image, dimensions, stride, dxgi::DXGI_FORMAT_R8G8B8A8_UNORM)
        }
    }

    pub unsafe fn new_raw(
        device: &Dx11Device,
        image: &[u8],
        dimensions: [u32; 2],
        stride: usize,
        format: dxgi::DXGI_FORMAT,
    ) -> anyhow::Result<Texture> {
        let [width, height] = dimensions;
        debug_assert!(image.len() >= height as usize * stride);
        let texture = {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                Format: format,
                .. Self::DESC_TEXTURE
            };
            let init_data = d3d11::D3D11_SUBRESOURCE_DATA {
                pSysMem: image.as_ptr() as *const _,
                SysMemPitch: stride as u32,
                SysMemSlicePitch: 0,
            };
            Texture2::new_with_desc_unchecked(device, &desc, Some(&init_data))
        }?;
        let view = TextureView2::new_with_texture2(device, &texture, None)?;

        let texture = Texture {
            texture,
            view,
            dimensions,
        };
        STATS_TEXTURE_COUNT.increment(1);
        STATS_TEXTURE_SIZE.increment_by(|| texture.texture_byte_size());

        // let device_context =
        //     unsafe { device.GetImmediateContext() }.expect("Should always succeed.");
        // texture.generate_mips(&device_context);

        Ok(texture)
    }

    pub fn generate_mips(&self, device_context: &Dx11Context) {
        self.view.generate_mips(device_context);
    }

    /// TODO: this is implemented by arcffi, defer to that once it's used more
    fn format_bpp(format: dxgi::DXGI_FORMAT) -> usize {
        match DxgiFormat::try_from_d3d(format).map_err(|_| format) {
            Ok(DxgiFormat::R32G32B32A32Float)
                => 16,
            Ok(DxgiFormat::R32G32B32Float)
                => 12,
            Ok(DxgiFormat::R32G32Float)
                => 8,
            Ok(DxgiFormat::R8G8B8A8UNorm | DxgiFormat::B8G8R8A8UNorm | DxgiFormat::B8G8R8X8UNorm)
                => 4,
            _f => {
                log::debug!("unrecognized texture DXGI_FORMAT {_f:?}");
                4
            },
        }
    }

    fn texture_byte_size(&self) -> usize {
        let bpp = Self::format_bpp(self.view.get_desc().Format);
        let [w, h] = self.dimensions;
        bpp * w as usize * h as usize
    }
}

impl D3dContextBindableSlot<Dx11Context> for Texture {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        self.view.set(device_context, slot)
    }
}

impl Clone for Texture {
    fn clone(&self) -> Self {
        STATS_TEXTURE_COUNT.increment(1);
        let _size = STATS_TEXTURE_SIZE.increment_by(|| self.texture_byte_size());
        STATS_TEXTURE_SIZE_CLONED.increment(_size);
        Self {
            texture: self.texture.clone(),
            view: self.view.clone(),
            dimensions: self.dimensions,
        }
    }
}

#[cfg(feature = "statistics")]
impl Drop for Texture {
    fn drop(&mut self) {
        STATS_TEXTURE_COUNT.decrement(1);
        STATS_TEXTURE_SIZE.decrement_by(|| self.texture_byte_size());
    }
}

pub static STATS_TEXTURE_COUNT: Counter = Counter::DEFAULT;
pub static STATS_TEXTURE_SIZE: Counter = Counter::DEFAULT;
pub static STATS_TEXTURE_SIZE_CLONED: Counter = Counter::DEFAULT;
