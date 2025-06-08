use {
    crate::TEXTURES,
    anyhow::{anyhow, Context as _},
    std::{fmt, path::Path, sync::Arc},
    nexus::texture::Texture as NexusTexture,
    windows::Win32::Graphics::{
        Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D,
        Direct3D11::{
            ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView, ID3D11Texture2D,
            D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
            D3D11_RESOURCE_MISC_GENERATE_MIPS, D3D11_SHADER_RESOURCE_VIEW_DESC,
            D3D11_SHADER_RESOURCE_VIEW_DESC_0, D3D11_SUBRESOURCE_DATA, D3D11_TEX2D_SRV,
            D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
        },
        Dxgi::Common::{
            DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
        },
    },
};
#[cfg(feature = "image")]
use image::{ImageReader, FlatSamples};

#[derive(PartialEq)]
pub struct Texture {
    pub texture: ID3D11Texture2D,
    pub dimensions: [u32; 2],
    pub view: Vec<Option<ID3D11ShaderResourceView>>,
}

impl Texture {
    pub fn load(device: &ID3D11Device, path: &Path) -> anyhow::Result<Arc<Self>> {
        let tex_store = TEXTURES.get().unwrap();
        let tex_lock = tex_store.read().unwrap();
        if tex_lock.contains_key(path) {
            log::debug!("Deduplicated {path:?}!");
            Ok(tex_lock[path].clone())
        } else {
            log::debug!("Un-deduplicated {path:?}!");
            drop(tex_lock);
            let texture = Self::new_path(device, path)?;
            let tarc = Arc::new(texture);
            let mut tex_write = tex_store.write().unwrap();
            tex_write.insert(path.to_path_buf(), tarc.clone());
            Ok(tarc.clone())
        }
    }

    pub fn new_bytes<D: fmt::Debug>(device: &ID3D11Device, mut bytes: &[u8], name: D) -> anyhow::Result<Self> {
        let read = std::io::Cursor::new(&mut bytes);
        Self::new_image(device, ImageReader::new(read), name)
    }

    pub fn new_path(device: &ID3D11Device, path: &Path) -> anyhow::Result<Self> {
        let image_reader = ImageReader::open(path)?;
        Self::new_image(device, image_reader, path)
    }

    #[cfg(feature = "image")]
    pub fn new_image<R, D>(device: &ID3D11Device, image_reader: ImageReader<R>, path: D) -> anyhow::Result<Self> where
        R: std::io::BufRead + std::io::Seek,
        D: fmt::Debug,
    {
        let format = image_reader.format();
        log::info!("Loading {:?} texture from {path:?}!", format);
        let image = image_reader.with_guessed_format()?.decode()?;
        let rgba_image = image.to_rgba32f();
        let dimensions = rgba_image.dimensions();
        let raw_rgba_image = rgba_image.into_raw();
        let texture_sample_desc = DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        };
        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: dimensions.0,
            Height: dimensions.1,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            SampleDesc: texture_sample_desc,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: D3D11_RESOURCE_MISC_GENERATE_MIPS.0 as u32,
        };
        let texture_subresource_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: raw_rgba_image.as_ptr().cast(),
            SysMemPitch: (size_of::<f32>() as u32 * dimensions.0 * 4),
            SysMemSlicePitch: 0,
        };
        let mut texture_ptr: Option<ID3D11Texture2D> = None;
        let texture = unsafe {
            device.CreateTexture2D(
                &texture_desc,
                Some(&texture_subresource_data),
                Some(&mut texture_ptr),
            )
        }
        .map_err(anyhow::Error::from)
        .and_then(|()| texture_ptr.ok_or_else(|| anyhow!("no texture for {path:?}")))?;
        log::info!("Creating a shader resource view for {:?}!", path);
        let tex2d_srv = D3D11_TEX2D_SRV {
            MostDetailedMip: 0,
            MipLevels: u32::MAX,
        };
        let view_anonymous = D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
            Texture2D: tex2d_srv,
        };
        let view_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: view_anonymous,
        };
        let mut view_ptr: Option<ID3D11ShaderResourceView> = None;
        let view = unsafe {
            device.CreateShaderResourceView(&texture, Some(&view_desc), Some(&mut view_ptr))
        }
        .map_err(anyhow::Error::from)
        .and_then(|()| view_ptr.ok_or_else(|| anyhow!("no shader resource view")))?;
        let view = vec![Some(view)];
        log::info!("Loaded {:?} texture from {path:?}!", format);
        Ok(Self {
            texture,
            view,
            dimensions: dimensions.into(),
        })
    }

    pub fn to_nexus(&self) -> Option<NexusTexture> {
        self.view.iter().filter_map(|srv| srv.as_ref()).next()
            .map(|resource| NexusTexture {
                resource: resource.clone(),
                width: self.dimensions[0],
                height: self.dimensions[1],
            })
    }

    #[cfg(feature = "image")]
    pub fn load_rgba8_uncached(
        device: &ID3D11Device,
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
        device: &ID3D11Device,
        image: &[u8],
        dimensions: [u32; 2],
        stride: usize,
    ) -> anyhow::Result<Texture> {
        let [width, height] = dimensions;
        debug_assert!(image.len() >= height as usize * stride);
        let texture = {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM, // TODO: Is sRGB correct?
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET).0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: D3D11_RESOURCE_MISC_GENERATE_MIPS.0 as u32,
            };
            let init_data = D3D11_SUBRESOURCE_DATA {
                pSysMem: image.as_ptr() as *const _,
                SysMemPitch: stride as u32,
                SysMemSlicePitch: 0,
            };
            let mut d3d_texture = None;
            unsafe {
                device
                    .CreateTexture2D(&desc, Some(&init_data), Some(&mut d3d_texture))
                    .context("Creating Texture2D")?;
            }
            d3d_texture.expect("This will always be Some because CreateTexture2D returned S_OK")
        };
        let view = {
            let view_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                    Texture2D: D3D11_TEX2D_SRV {
                        MostDetailedMip: 0,
                        MipLevels: u32::MAX,
                    },
                },
            };
            let mut view_ptr = None;
            unsafe {
                device
                    .CreateShaderResourceView(&texture, Some(&view_desc), Some(&mut view_ptr))
                    .context("Creating SRV")?;
            }
            view_ptr
                .expect("This will always be Some because CreateShaderResourceView returned S_OK")
        };

        let texture = Texture {
            texture,
            view: vec![Some(view)],
            dimensions,
        };

        // let device_context =
        //     unsafe { device.GetImmediateContext() }.expect("Should always succeed.");
        // texture.generate_mips(&device_context);

        Ok(texture)
    }

    pub fn generate_mips(&self, device_context: &ID3D11DeviceContext) {
        unsafe {
            let mut itty = self.view.iter();
            while let Some(Some(view)) = itty.next() {
                device_context.GenerateMips(view);
            }
        }
    }

    pub fn set(&self, device_context: &ID3D11DeviceContext, slot: u32) {
        unsafe {
            device_context.PSSetShaderResources(slot, Some(self.view.as_slice()));
        }
    }
}
