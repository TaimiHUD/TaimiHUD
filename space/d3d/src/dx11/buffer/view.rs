pub use crate::{
    d3d::D3D_SRV_DIMENSION,
    dx11::d3d11::{
        ID3D11ShaderResourceView,
        ID3D11View,
        D3D11_SHADER_RESOURCE_VIEW_DESC,
        D3D11_SHADER_RESOURCE_VIEW_DESC_0,
        D3D11_TEX2D_SRV,
    },
};
use crate::{
    dx11::{
        buffer::{Resource, Texture2},
        prelude::*,
    },
    D3dContextBindableSlot,
};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11View;

    @[transparent(Dx11Child <= ID3D11View)]
    pub struct View.view;
}

impl_d3d! {
    unsafe impl Dx11Child for ID3D11ShaderResourceView;

    @[transparent(Dx11Child <= ID3D11ShaderResourceView)]
    pub struct ShaderResourceView {
        pub view: View,
    }
    @into()
    @deref(View);
}

impl View {
    pub fn get_resource(&self) -> anyhow::Result<Dx11Resource> {
        unsafe {
            self.as_d3d()
                .GetResource()
                .context("ID3D11ShaderResourceView::GetResource")
        }
    }
}

impl ShaderResourceView {
    pub fn new_with_desc<R: AsRef<Resource>>(
        device: &Dx11Device,
        resource: R,
        desc: Option<&D3D11_SHADER_RESOURCE_VIEW_DESC>,
    ) -> anyhow::Result<Self> {
        let resource = resource.as_ref();
        let mut out = None;
        unsafe {
            device.CreateShaderResourceView(resource, desc.map(|d| d as *const _), Some(&mut out))
        }
        .map_err(anyhow::Error::from)
        .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce view pointer")))
        .context("CreateShaderResourceView")
        .map(Into::into)
    }

    pub fn bind_set<V>(views: V, context: &Dx11Context, slot: u32)
    where
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
            self.as_d3d().GetDesc(&mut desc);
        }
        desc
    }
}

impl D3dContextBindableSlot<Dx11Context> for ShaderResourceView {
    fn set(&self, context: &Dx11Context, slot: u32) {
        Self::bind_set(self, context, slot)
    }
}

impl AsRef<View> for ID3D11ShaderResourceView {
    #[inline]
    fn as_ref(&self) -> &View {
        let srv: &ShaderResourceView = self.as_ref();
        srv.as_ref()
    }
}
impl From<ID3D11ShaderResourceView> for View {
    #[inline]
    fn from(srv: ID3D11ShaderResourceView) -> Self {
        ShaderResourceView::from(srv).into()
    }
}

impl_d3d! {
    @[transparent(Dx11Child <= ID3D11ShaderResourceView)]
    pub struct TextureView2 {
        pub view: ShaderResourceView,
    }
    @into()
    @deref(ShaderResourceView);
}

impl TextureView2 {
    pub fn with_view(view: ShaderResourceView) -> Self {
        Self { view }
    }

    pub const DESC_DEFAULT: D3D11_TEX2D_SRV = D3D11_TEX2D_SRV {
        MostDetailedMip: 0,
        MipLevels: u32::MAX,
    };

    pub fn desc_for_texture2(
        texture2: &Texture2,
        desc: D3D11_TEX2D_SRV,
    ) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
        let format = texture2.dxgi_format();
        D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: ViewDimension::TEXTURE2.into(),
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: desc },
        }
    }

    pub fn new_with_texture2(
        device: &Dx11Device,
        texture2: &Texture2,
        desc: Option<D3D11_TEX2D_SRV>,
    ) -> anyhow::Result<Self> {
        let desc = desc.unwrap_or(Self::DESC_DEFAULT);
        let desc = Self::desc_for_texture2(texture2, desc);
        ShaderResourceView::new_with_desc(device, texture2, Some(&desc)).map(Self::with_view)
    }

    pub fn generate_mips(&self, context: &Dx11Context) {
        unsafe {
            context.GenerateMips(self);
        }
    }

    pub fn get_resource(&self) -> anyhow::Result<Texture2> {
        unsafe {
            self.as_d3d()
                .GetResource()
                .and_then(|r| r.cast().map(Texture2::from_d3d))
                .context("ID3D11ShaderResourceView::GetResource<Texture2>")
        }
    }

    pub fn get_desc(&self) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
        self.view.get_desc()
    }
}

impl D3dContextBindableSlot<Dx11Context> for TextureView2 {
    fn set(&self, context: &Dx11Context, slot: u32) {
        self.view.set(context, slot)
    }
}

impl_d3d! { impl enum for
    #[derive(Default)]
    pub enum ViewDimension: D3D_SRV_DIMENSION{u32} {
        #[default]
        const UNKNOWN = d3d::D3D11_SRV_DIMENSION_UNKNOWN;
        const BUFFER = d3d::D3D11_SRV_DIMENSION_BUFFER;
        const BUFFEREX = d3d::D3D11_SRV_DIMENSION_BUFFEREX;
        const TEXTURE1 = d3d::D3D11_SRV_DIMENSION_TEXTURE1D;
        const TEXTURE1_ARRAY = d3d::D3D11_SRV_DIMENSION_TEXTURE1DARRAY;
        const TEXTURE2 = d3d::D3D11_SRV_DIMENSION_TEXTURE2D;
        const TEXTURE2_ARRAY = d3d::D3D11_SRV_DIMENSION_TEXTURE2DARRAY;
        const TEXTURE3 = d3d::D3D11_SRV_DIMENSION_TEXTURE3D;
    },
}
