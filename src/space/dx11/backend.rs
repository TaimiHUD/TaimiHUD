use {
    super::{DepthHandler, PerspectiveHandler},
    crate::{
        exports::runtime::{self as rt, Device, SwapChain},
        space::{resources::ShaderLoader, ScreenSpace},
    },
    anyhow::Context,
    glamour::{Rect, Size2},
    std::sync::OnceLock,
    taimi_d3d::dx11::{
        blend::{BlendState, OMBlendState, D3D11_RENDER_TARGET_BLEND_DESC},
        buffer::{SamplerState, TextureAddressMode, VertexBuffer, D3D11_SAMPLER_DESC},
        prelude::*,
        viewport::Viewport,
    },
};

#[allow(unused)]
pub struct RenderBackend {
    pub depth_handler: DepthHandler,
    pub perspective_handler: PerspectiveHandler,
    pub blend_state: OMBlendState<BlendState>,
    #[cfg(feature = "goggles2")]
    pub blend_state_shadow: OMBlendState<BlendState>,

    pub shaders: ShaderLoader,
    pub sampler_state: SamplerState,
    pub device: Device,
    pub swap_chain: SwapChain,
    pub display_size: Size2<ScreenSpace>,
    pub viewport: Viewport,

    pub shared_quad: OnceLock<VertexBuffer>,
}

impl RenderBackend {
    pub fn setup(display_size: Size2<ScreenSpace>) -> anyhow::Result<RenderBackend> {
        log::debug!("Getting d3d11 device swap chain");
        let (device, swap_chain) = rt::d3d11_device()?;

        let viewport = Viewport::with_size(display_size.extend(1.0));

        let shaders = ShaderLoader::load_bundled(&device).context("Shaders failed to load")?;
        let perspective_handler =
            PerspectiveHandler::setup(&device).context("Perspective handler setup failed")?;

        let depth_handler =
            DepthHandler::create(display_size, &device, &swap_chain).context("Depth setup failed")?;
        let sampler_state =
            SamplerState::new_with_desc(&device, &Self::SAMPLER_DESC).context("Sampler setup failed")?;

        let blend_desc = BlendState::desc_for_target(Self::BLEND_STATE_DESC_RT, false, false);
        let blend_state =
            BlendState::new_with_desc(&device, &blend_desc).context("Blending setup failed")?;
        let blend_desc_shadow = BlendState::desc_for_target(Self::BLEND_STATE_DESC_G2_SHADOW, false, false);
        let blend_state_shadow =
            BlendState::new_with_desc(&device, &blend_desc_shadow).context("Blending setup failed")?;
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
            #[cfg(feature = "goggles2")]
            blend_state_shadow: OMBlendState::new(blend_state_shadow, None, None),
            depth_handler,
            perspective_handler,

            device,
            swap_chain,
            shaders,
            sampler_state,
            display_size,
            viewport,

            shared_quad: OnceLock::new(),
        })
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

    pub fn shared_quad(&self) -> anyhow::Result<VertexBuffer> {
        use crate::resources::Model;

        if let Some(quad) = self.shared_quad.get() {
            return Ok(quad.clone())
        }

        let vbuf = Model::quad().to_buffer(&self.device)?;
        Ok(self.shared_quad.get_or_init(move || vbuf).clone())
    }

    /// See [crate::space::engine::Engine::cleanup_background]
    ///
    /// TODO: revisit, avoid, etc
    pub fn cleanup_background(self) {
        std::mem::forget(self)
    }

    const BLEND_STATE_DESC_RT: D3D11_RENDER_TARGET_BLEND_DESC =
        D3D11_RENDER_TARGET_BLEND_DESC {
            #[cfg(feature = "goggles2")]
            RenderTargetWriteMask: (d3d11::D3D11_COLOR_WRITE_ENABLE_RED.0 | d3d11::D3D11_COLOR_WRITE_ENABLE_GREEN.0 | d3d11::D3D11_COLOR_WRITE_ENABLE_BLUE.0) as _,
            ..BlendState::TARGET_DESC_ADDITIVE
        };

    #[cfg(feature = "goggles2")]
    const BLEND_STATE_DESC_G2_SHADOW: D3D11_RENDER_TARGET_BLEND_DESC =
        D3D11_RENDER_TARGET_BLEND_DESC {
            RenderTargetWriteMask: (d3d11::D3D11_COLOR_WRITE_ENABLE_ALPHA.0 | d3d11::D3D11_COLOR_WRITE_ENABLE_RED.0) as _,
            //SrcBlend: taimi_d3d::dx11::blend::BlendFactor::ONE,
            //BlendOp: taimi_d3d::dx11::blend::BlendOp::Sub,
            BlendOpAlpha: taimi_d3d::dx11::blend::BlendOp::MIN,
            //DestBlendAlpha: taimi_d3d::dx11::blend::BlendFactor::SRC_ALPHA,
            ..Self::BLEND_STATE_DESC_RT
        };

    const SAMPLER_DESC: D3D11_SAMPLER_DESC = D3D11_SAMPLER_DESC {
        MinLOD: 0.0,
        ComparisonFunc: d3d11::D3D11_COMPARISON_ALWAYS,
        BorderColor: [0.0; 4],
        ..SamplerState::desc_with_address(TextureAddressMode::WRAP.to_vec3())
    };

    #[inline]
    pub fn viewport_rect(&self) -> Rect<ScreenSpace> {
        let r = self.viewport.rect();
        Rect::new(
            r.origin.cast(),
            r.size.cast(),
        )
    }
}
