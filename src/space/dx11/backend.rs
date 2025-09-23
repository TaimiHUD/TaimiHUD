use {
    super::{DepthHandler, PerspectiveHandler},
    crate::{
        exports::runtime as rt,
        space::{
            resources::ShaderLoader,
            ScreenSpace,
        },
    },
    anyhow::{anyhow, Context},
    glamour::Size2,
    taimi_d3d::dx11::{
        prelude::*,
        buffer::{D3D11_SAMPLER_DESC, SamplerState, TextureAddressMode},
        blend::{BlendState, D3D11_RENDER_TARGET_BLEND_DESC, OMBlendState},
        SwapChain11,
    },
};

#[allow(unused)]
pub struct RenderBackend {
    pub depth_handler: DepthHandler,
    pub perspective_handler: PerspectiveHandler,
    pub blend_state: OMBlendState<BlendState>,

    pub shaders: ShaderLoader,
    pub sampler_state: SamplerState,
    pub device: Dx11Device,
    pub swap_chain: SwapChain11,
    pub display_size: Size2<ScreenSpace>,
}

impl RenderBackend {
    pub fn setup(display_size: Size2<ScreenSpace>) -> anyhow::Result<RenderBackend> {
        log::info!("Getting d3d11 device swap chain");
        let swap_chain = rt::dxgi_swap_chain()
            .map_err(|e| anyhow!("DXGI swap chain unavailable: {e}"))
            .and_then(|sc| sc.map(SwapChain11::from)
                .ok_or_else(|| anyhow!("you will not reach heaven today, how are you here?"))
            )?;
        let device = swap_chain.get_device11()?;

        let shaders = ShaderLoader::load_bundled(&device)
            .context("Shaders failed to load")?;
        let perspective_handler = PerspectiveHandler::setup(&device, display_size)
            .context("Perspective handler setup failed")?;

        let depth_handler = DepthHandler::create(display_size, &device, &swap_chain)
            .context("Depth setup failed")?;
        let sampler_state = SamplerState::new_with_desc(&device, &Self::SAMPLER_DESC)
            .context("Sampler setup failed")?;

        let blend_desc = BlendState::desc_for_target(Self::BLEND_STATE_DESC_RT, false, false);
        let blend_state = BlendState::new_with_desc(&device, &blend_desc)
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
            blend_state: OMBlendState::new(blend_state, None, None),
            depth_handler,
            perspective_handler,

            device,
            swap_chain,
            shaders,
            sampler_state,
            display_size,
        })
    }

    pub fn prepare(&mut self, display_size: Size2<ScreenSpace>) {
        self.perspective_handler.prepare(display_size);
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

    const BLEND_STATE_DESC_RT: D3D11_RENDER_TARGET_BLEND_DESC = D3D11_RENDER_TARGET_BLEND_DESC {
        .. BlendState::TARGET_DESC_ADDITIVE
    };

    const SAMPLER_DESC: D3D11_SAMPLER_DESC = D3D11_SAMPLER_DESC {
        MinLOD: 0.0,
        ComparisonFunc: d3d11::D3D11_COMPARISON_ALWAYS,
        BorderColor: [0.0; 4],
        .. SamplerState::desc_with_address(TextureAddressMode::WRAP.to_vec3())
    };
}
