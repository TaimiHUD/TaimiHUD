use {
    anyhow::{anyhow, Context},
    windows::Win32::Graphics::Direct3D11::{
        ID3D11BlendState, ID3D11Device, ID3D11DeviceContext, D3D11_BLEND_DESC,
        D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD, D3D11_BLEND_SRC_ALPHA,
        D3D11_BLEND_ZERO, D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_RENDER_TARGET_BLEND_DESC,
    },
};

pub struct BlendingHandler {
    blend_state: ID3D11BlendState,
}

impl BlendingHandler {
    pub fn setup(device: &ID3D11Device) -> anyhow::Result<Self> {
        log::debug!("Setting up blending handler",);
        let rt_blend_desc = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            SrcBlend: D3D11_BLEND_SRC_ALPHA,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_ZERO,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        let rt_blend_descs = [rt_blend_desc; 8];
        let blend_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: false.into(),
            IndependentBlendEnable: false.into(),
            RenderTarget: rt_blend_descs,
        };
        let mut ptr: Option<ID3D11BlendState> = None;
        let blend_state = unsafe { device.CreateBlendState(&blend_desc, Some(&mut ptr)) }
            .context("blend state creation")
            .map_err(anyhow::Error::from)
            .and_then(|()| ptr.ok_or_else(|| anyhow!("no blend state")))?;
        log::debug!("Set up blending handler",);
        Ok(Self { blend_state })
    }

    pub fn set(&self, context: &ID3D11DeviceContext) {
        unsafe {
            context.OMSetBlendState(&self.blend_state, None, u32::MAX);
        }
    }
}
