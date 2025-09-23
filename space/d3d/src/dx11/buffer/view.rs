use crate::{
    dx11::{
        impl_d3d_ext11,
        buffer::ViewDimension,
        prelude::*,
        Texture2,
    },
    D3dContextBindableSlot,
};

pub use crate::dx11::d3d11::{
    ID3D11ShaderResourceView,
    D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_SHADER_RESOURCE_VIEW_DESC_0,
    D3D11_TEX2D_SRV,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct ShaderResourceView {
    pub view: ID3D11ShaderResourceView,
}

impl ShaderResourceView {
    pub fn new_with_desc(device: &Dx11Device, resource: &Dx11Resource, desc: Option<&D3D11_SHADER_RESOURCE_VIEW_DESC>) -> anyhow::Result<Self> {
        let mut out = None;
        unsafe {
            device.CreateShaderResourceView(resource, desc.map(|d| d as *const _), Some(&mut out))
        }.map_err(anyhow::Error::from)
        .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce view pointer")))
        .context("CreateShaderResourceView")
        .map(Into::into)
    }

    pub fn bind_set<V>(views: V, context: &Dx11Context, slot: u32) where
        V: ID3D11ResourceOf<ID3D11ShaderResourceView>,
    {
        let views = views.as_params_of();
        unsafe {
            context.PSSetShaderResources(slot, Some(views));
        }
    }

    pub fn get_desc(&self) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
        let mut desc = Default::default();
        unsafe {
            self.view.GetDesc(&mut desc);
        }
        desc
    }

    pub fn get_resource(&self) -> anyhow::Result<Dx11Resource> {
        unsafe {
            self.view.GetResource()
                .context("ID3D11ShaderResourceView::GetResource")
        }
    }
}

impl D3dContextBindableSlot<Dx11Context> for ShaderResourceView {
    fn set(&self, context: &Dx11Context, slot: u32) {
        Self::bind_set(self, context, slot)
    }
}

impl_d3d_ext11! {
    unsafe impl ID3D11ResourceExt<Output=ID3D11ShaderResourceView,@transparent> for ShaderResourceView,
        @field(&this => &this.view);
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct TextureView2 {
    pub view: ShaderResourceView,
}

impl TextureView2 {
    pub fn with_view(view: ShaderResourceView) -> Self {
        Self {
            view,
        }
    }

    pub const DESC_DEFAULT: D3D11_TEX2D_SRV = D3D11_TEX2D_SRV {
        MostDetailedMip: 0,
        MipLevels: u32::MAX,
    };

    pub fn desc_for_texture2(texture2: &Texture2, desc: D3D11_TEX2D_SRV) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
        let format = texture2.dxgi_format();
        D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: ViewDimension::TEXTURE2.into(),
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: desc,
            },
        }
    }

    pub fn new_with_texture2(device: &Dx11Device, texture2: &Texture2, desc: Option<D3D11_TEX2D_SRV>) -> anyhow::Result<Self> {
        let desc = desc.unwrap_or(Self::DESC_DEFAULT);
        let desc = Self::desc_for_texture2(texture2, desc);
        ShaderResourceView::new_with_desc(device, &texture2.resource, Some(&desc))
            .map(Self::with_view)
    }

    pub fn generate_mips(&self, context: &Dx11Context) {
        unsafe {
            context.GenerateMips(&self.view.view);
        }
    }

    pub fn get_resource(&self) -> anyhow::Result<Texture2> {
        unsafe {
            self.view.view.GetResource()
                .and_then(|r| r.cast().map(Texture2::from_d3d))
                .context("ID3D11ShaderResourceView::GetResource<Texture2>")
        }
    }

    pub fn get_desc(&self) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
        self.view.get_desc()
    }
}

impl_d3d_ext11! {
    unsafe impl ID3D11ResourceExt<Output=ID3D11ShaderResourceView,@transparent> for TextureView2,
        @field(&this => &this.view.view);
}

impl D3dContextBindableSlot<Dx11Context> for TextureView2 {
    fn set(&self, context: &Dx11Context, slot: u32) {
        self.view.set(context, slot)
    }
}

impl From<TextureView2> for ShaderResourceView {
    fn from(view: TextureView2) -> Self {
        view.view
    }
}
