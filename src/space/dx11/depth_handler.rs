use {
    anyhow::{anyhow, Context},
    crate::space::max_depth,
    windows::Win32::Graphics::{
        Direct3D11::{
            ID3D11DepthStencilState, ID3D11DepthStencilView, ID3D11Device, ID3D11DeviceContext,
            ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11Texture2D,
            D3D11_BIND_DEPTH_STENCIL, D3D11_CLEAR_DEPTH, D3D11_CLEAR_STENCIL,
            D3D11_COMPARISON_ALWAYS, D3D11_COMPARISON_LESS, D3D11_CULL_BACK, D3D11_CULL_NONE,
            D3D11_DEFAULT_STENCIL_READ_MASK, D3D11_DEFAULT_STENCIL_WRITE_MASK,
            D3D11_DEPTH_STENCILOP_DESC, D3D11_DEPTH_STENCIL_DESC, D3D11_DEPTH_STENCIL_VIEW_DESC,
            D3D11_DEPTH_STENCIL_VIEW_DESC_0, D3D11_DEPTH_WRITE_MASK_ALL,
            D3D11_DSV_DIMENSION_TEXTURE2D, D3D11_FILL_SOLID, D3D11_RASTERIZER_DESC,
            D3D11_STENCIL_OP_KEEP, D3D11_TEX2D_DSV, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
            D3D11_VIEWPORT,
        },
        Dxgi::{
            Common::{DXGI_FORMAT_D24_UNORM_S8_UINT, DXGI_SAMPLE_DESC},
            IDXGISwapChain,
        },
    },
};

#[allow(unused)]
pub struct DepthHandler {
    pub framebuffer: ID3D11Texture2D,
    viewport: D3D11_VIEWPORT,
    pub render_target_view: Vec<Option<ID3D11RenderTargetView>>,
    pub depth_stencil_state: ID3D11DepthStencilState,
    pub depth_stencil_view: ID3D11DepthStencilView,
    pub depth_stencil_buffer: ID3D11Texture2D,
    pub rasterizer_state: ID3D11RasterizerState,
}

impl DepthHandler {
    pub fn create(
        display_size: &[f32; 2],
        device: &ID3D11Device,
        swap_chain: &IDXGISwapChain,
    ) -> anyhow::Result<Self> {
        let framebuffer = Self::get_framebuffer(swap_chain)?;
        let viewport = Self::create_viewport(display_size);
        let render_target_view = vec![Self::create_render_target_view(device, &framebuffer).ok()];
        let depth_stencil_state = Self::create_depth_stencil_state(device)?;
        let depth_stencil_buffer = Self::create_depth_stencil_buffer(device, display_size)?;
        let depth_stencil_view = Self::create_depth_stencil_view(device, &depth_stencil_buffer)?;
        let rasterizer_state = Self::create_rasterizer_state(device)?;
        Ok(Self {
            framebuffer,
            viewport,
            render_target_view,
            depth_stencil_view,
            depth_stencil_state,
            depth_stencil_buffer,
            rasterizer_state,
        })
    }

    pub fn setup(&self, device_context: &ID3D11DeviceContext) {
        unsafe {
            device_context.RSSetState(&self.rasterizer_state);
            device_context.RSSetViewports(Some(&[self.viewport]));
            device_context.OMSetRenderTargets(
                Some(self.render_target_view.as_slice()),
                Some(&self.depth_stencil_view),
            );
            device_context.OMSetDepthStencilState(&self.depth_stencil_state, 1);
            device_context.ClearDepthStencilView(
                &self.depth_stencil_view,
                D3D11_CLEAR_DEPTH.0 | D3D11_CLEAR_STENCIL.0,
                1.0,
                0,
            );
        }
    }

    pub fn create_viewport(display_size: &[f32; 2]) -> D3D11_VIEWPORT {
        log::debug!(
            "Setting up viewport with dimensions ({},{})",
            display_size[0],
            display_size[1]
        );
        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: display_size[0],
            Height: display_size[1],
            MinDepth: 0.0,
            MaxDepth: max_depth(),
        };
        log::debug!(
            "Set up viewport with dimensions ({},{})",
            display_size[0],
            display_size[1]
        );
        viewport
    }

    pub fn get_framebuffer(swap_chain: &IDXGISwapChain) -> anyhow::Result<ID3D11Texture2D> {
        log::info!("Setting up framebuffer");
        let framebuffer: ID3D11Texture2D =
            unsafe { swap_chain.GetBuffer(0) }
            .context("getting swap chain's framebuffer")?;
        log::info!("Set up framebuffer");
        Ok(framebuffer)
    }

    pub fn create_render_target_view(
        device: &ID3D11Device,
        framebuffer: &ID3D11Texture2D,
    ) -> anyhow::Result<ID3D11RenderTargetView> {
        log::debug!("Setting up render target view");
        let mut render_target_view_ptr: Option<ID3D11RenderTargetView> = None;
        let render_target_view = unsafe {
            device.CreateRenderTargetView(framebuffer, None, Some(&mut render_target_view_ptr))
        }
        .context("creating render target view")
        .and_then(|()| render_target_view_ptr.ok_or_else(|| anyhow!("no render target view")))?;
        log::debug!("Set up render target view");
        Ok(render_target_view)
    }

    pub fn create_depth_stencil_state(
        device: &ID3D11Device,
    ) -> anyhow::Result<ID3D11DepthStencilState> {
        log::info!("Setting up depth stencil state");
        let depth_stencil_frontface_desc = D3D11_DEPTH_STENCILOP_DESC {
            StencilFunc: D3D11_COMPARISON_ALWAYS,
            StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
            StencilFailOp: D3D11_STENCIL_OP_KEEP,
            StencilPassOp: D3D11_STENCIL_OP_KEEP,
        };
        let depth_stencil_backface_desc = D3D11_DEPTH_STENCILOP_DESC {
            StencilFunc: D3D11_COMPARISON_ALWAYS,
            StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
            StencilFailOp: D3D11_STENCIL_OP_KEEP,
            StencilPassOp: D3D11_STENCIL_OP_KEEP,
        };
        let depth_stencil_state_desc = D3D11_DEPTH_STENCIL_DESC {
            DepthEnable: true.into(),
            DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
            DepthFunc: D3D11_COMPARISON_LESS,
            StencilEnable: false.into(),
            StencilReadMask: D3D11_DEFAULT_STENCIL_READ_MASK as u8,
            StencilWriteMask: D3D11_DEFAULT_STENCIL_WRITE_MASK as u8,
            FrontFace: depth_stencil_frontface_desc,
            BackFace: depth_stencil_backface_desc,
        };
        let mut depth_stencil_state_ptr: Option<ID3D11DepthStencilState> = None;
        let depth_stencil_state = unsafe {
            device.CreateDepthStencilState(
                &depth_stencil_state_desc,
                Some(&mut depth_stencil_state_ptr),
            )
        }.context("creating depth state")
        .and_then(|()| depth_stencil_state_ptr.ok_or_else(|| anyhow!("no depth stencil state")))?;
        log::info!("Set up depth stencil state");
        Ok(depth_stencil_state)
    }

    pub fn create_depth_stencil_buffer(
        device: &ID3D11Device,
        display_size: &[f32; 2],
    ) -> anyhow::Result<ID3D11Texture2D> {
        log::info!("Setting up depth stencil buffer");
        let depth_stencil_buffer_sample_desc = DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        };
        let depth_stencil_buffer_desc = D3D11_TEXTURE2D_DESC {
            Width: display_size[0] as u32,
            Height: display_size[1] as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
            SampleDesc: depth_stencil_buffer_sample_desc,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_DEPTH_STENCIL.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        log::info!("Set up depth stencil buffer");
        let mut depth_stencil_buffer_ptr: Option<ID3D11Texture2D> = None;
        let depth_stencil_buffer = unsafe {
            device.CreateTexture2D(
                &depth_stencil_buffer_desc,
                None,
                Some(&mut depth_stencil_buffer_ptr),
            )
        }.context("creating depth buffer")
        .and_then(|()| {
            depth_stencil_buffer_ptr.ok_or_else(|| anyhow!("no depth stencil buffer"))
        })?;
        Ok(depth_stencil_buffer)
    }

    pub fn create_depth_stencil_view(
        device: &ID3D11Device,
        buffer: &ID3D11Texture2D,
    ) -> anyhow::Result<ID3D11DepthStencilView> {
        let dsv_tex2d = D3D11_TEX2D_DSV { MipSlice: 0 };
        let depth_stencil_view_anonymous = D3D11_DEPTH_STENCIL_VIEW_DESC_0 {
            Texture2D: dsv_tex2d,
        };
        let depth_stencil_view_desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
            ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
            Flags: 0,
            Anonymous: depth_stencil_view_anonymous,
        };
        log::info!("Setting up depth stencil view");
        let mut depth_stencil_view_ptr: Option<ID3D11DepthStencilView> = None;
        let depth_stencil_view = unsafe {
            device.CreateDepthStencilView(
                buffer,
                Some(&depth_stencil_view_desc),
                Some(&mut depth_stencil_view_ptr),
            )
        }.context("creating depth view")
        .and_then(|()| depth_stencil_view_ptr.ok_or_else(|| anyhow!("no depth stencil view")))?;
        log::info!("Set up depth stencil view");
        Ok(depth_stencil_view)
    }

    pub fn create_rasterizer_state(device: &ID3D11Device) -> anyhow::Result<ID3D11RasterizerState> {
        log::info!("Setting up rasterizer state");
        let rasterizer_state_desc = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: D3D11_CULL_NONE,
            FrontCounterClockwise: true.into(),
            DepthBias: 0,
            DepthBiasClamp: 0.0,
            SlopeScaledDepthBias: 0.0,
            DepthClipEnable: false.into(),
            ScissorEnable: false.into(),
            MultisampleEnable: false.into(),
            AntialiasedLineEnable: false.into(),
        };
        let mut rasterizer_state_ptr: Option<ID3D11RasterizerState> = None;
        let rasterizer_state = unsafe {
            device.CreateRasterizerState(&rasterizer_state_desc, Some(&mut rasterizer_state_ptr))
        }
        .context("creating rasterizer state")
        .and_then(|()| rasterizer_state_ptr.ok_or_else(|| anyhow!("no rasterizer state")))?;
        log::info!("Set up rasterizer state");
        Ok(rasterizer_state)
    }
}
