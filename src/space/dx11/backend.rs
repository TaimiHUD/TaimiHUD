use {
    super::{BlendingHandler, DepthHandler, PerspectiveHandler, PerspectiveInputData},
    crate::{
        exports::runtime as rt,
        space::resources::ShaderLoader,
    },
    anyhow::{anyhow, Context},
    glam::Vec4,
    std::path::Path,
    windows::Win32::Graphics::{
        Direct3D11::{
            ID3D11Device, ID3D11SamplerState, D3D11_COMPARISON_ALWAYS,
            D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_WRAP,
        },
        Dxgi::IDXGISwapChain,
    },
};

#[allow(unused)]
pub struct RenderBackend {
    pub depth_handler: DepthHandler,
    pub perspective_handler: PerspectiveHandler,
    pub blending_handler: BlendingHandler,

    pub shaders: ShaderLoader,
    pub sampler_state: Vec<Option<ID3D11SamplerState>>,
    pub device: ID3D11Device,
    pub swap_chain: IDXGISwapChain,
    pub aspect_ratio: Option<f32>,
    pub display_size: Option<[f32; 2]>,
}

impl RenderBackend {
    pub fn setup_sampler(device: &ID3D11Device) -> anyhow::Result<ID3D11SamplerState> {
        let sampler_desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
            MipLODBias: 0.0,
            MinLOD: 0.0,
            MaxLOD: f32::MAX,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_ALWAYS,
            BorderColor: Vec4::new(0.0, 0.0, 0.0, 0.0).into(),
        };
        let mut sampler_state_ptr: Option<ID3D11SamplerState> = None;
        let sampler_state =
            unsafe { device.CreateSamplerState(&sampler_desc, Some(&mut sampler_state_ptr)) }
                .map_err(anyhow::Error::from)
                .and_then(|()| sampler_state_ptr.ok_or_else(|| anyhow!("no sampler state")))?;
        Ok(sampler_state)
    }

    pub fn setup(addon_dir: &Path, display_size: [f32; 2]) -> anyhow::Result<RenderBackend> {
        log::info!("Getting d3d11 device");
        let device = rt::d3d11_device()
            .map_err(|e| anyhow!("D3D11 device unavailable: {e}"))?;
        log::info!("Getting d3d11 device swap chain");
        let swap_chain = rt::dxgi_swap_chain()
            .map_err(|e| anyhow!("DXGI swap chain unavailable: {e}"))?;
        let (device, swap_chain) = device.and_then(|d| swap_chain.map(|sc| (d, sc)))
            .ok_or_else(|| anyhow!("you will not reach heaven today, how are you here?"))?;

        PerspectiveInputData::create();

        let shaders = ShaderLoader::load(addon_dir, &device)
            .context("Shaders failed to load")?;
        let perspective_handler = PerspectiveHandler::setup(&device, &display_size)
            .context("Perspective handler setup failed")?;

        let depth_handler = DepthHandler::create(&display_size, &device, &swap_chain)
            .context("Depth setup failed")?;
        let sampler_state = vec![Self::setup_sampler(&device).ok()];

        let blending_handler = BlendingHandler::setup(&device)
            .context("Blending setup failed")?;
        //log::info!("Setting up device context");
        //let device_context = unsafe { device.GetImmediateContext().expect("I lost my context!") };

        /*
        let path = addon_dir.join("QuitarHero_Hero-Timers/timers/Assets/Raids/Deimos.png");
        if let Ok(quad) = Entity::quad(&device, &shaders, Some(&path)) {
            entities.push(quad);
        }
        for entity in entities.iter() {
            if let Some(texture) = &entity.model.texture {
                texture.generate_mips(&device_context);
            }
        }*/
        Ok(RenderBackend {
            blending_handler,
            depth_handler,
            perspective_handler,

            device,
            swap_chain: swap_chain.clone(),
            shaders,
            sampler_state,
            aspect_ratio: None,
            display_size: None,
        })
    }

    pub fn prepare(&mut self, display_size: &[f32; 2]) {
        self.perspective_handler.update_perspective(display_size);
    }
    /*
    pub fn draw(&mut self, io: &Io) {
        if let Some(settings) = SETTINGS.get().and_then(|settings| settings.try_read().ok()) {
            if settings.enable_katrender {
                let display_size = io.display_size;

                self.perspective_handler.update_perspective(&display_size);
                unsafe {
                    let slot = 0;

                    let device_context = self
                        .device
                        .GetImmediateContext()
                        .expect("I lost my context!");

                    self.perspective_handler.set(&device_context, slot);
                    for entity in &self.entities {
                        entity.rotate(io.delta_time);
                        device_context.UpdateSubresource(
                            &entity.instance_buffer,
                            0,
                            None,
                            entity.model_matrix.borrow().as_ptr() as *const _ as *const _,
                            0,
                            0,
                        );
                    }

                    self.depth_handler.setup(&device_context);
                    device_context.PSSetSamplers(slot, Some(&self.sampler_state));
                    for entity in &self.entities {
                        if let Some(texture) = &entity.model.texture {
                            texture.set(&device_context, slot);
                        }
                        entity.vertex_shader.set(&device_context);
                        entity.pixel_shader.set(&device_context);
                        entity.set_and_draw(&device_context);
                    }
                }
            }
        }
    }*/
}
