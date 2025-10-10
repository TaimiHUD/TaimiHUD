use crate::dx11::{
    prelude::*,
    buffer::Resource,
};

pub use crate::dx11::d3d11::{
    ID3D11Texture2D,
    D3D11_TEXTURE2D_DESC,
};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11Texture2D;

    @[transparent(Dx11Child <= ID3D11Texture2D)]
    pub struct Texture2 {
        pub resource: Resource,
    }
    @into()
    @deref(Resource);
}

impl Texture2 {
    pub fn new_with_desc<D: D3dBufferData>(
        device: &Dx11Device,
        desc: &D3D11_TEXTURE2D_DESC,
        data: Option<&[D]>,
    ) -> anyhow::Result<Self> {
        let data_desc = match &data {
            None => None,
            Some(data) => Some(d3d11::D3D11_SUBRESOURCE_DATA {
                pSysMem: data.as_ptr().cast(),
                SysMemPitch: {
                    if (desc.Width as usize * desc.Height as usize) > data.len() {
                        anyhow::bail!("initial texture buffer len={} is too small for {}x{}", data.len(), desc.Width, desc.Height);
                    }
                    D::stride() as u32 * desc.Width
                },
                SysMemSlicePitch: 0,
            }),
        };
        unsafe {
            Self::new_with_desc_unchecked(device, desc, data_desc.as_ref())
        }
    }

    pub unsafe fn new_with_desc_unchecked(
        device: &Dx11Device,
        desc: &D3D11_TEXTURE2D_DESC,
        data_desc: Option<&d3d11::D3D11_SUBRESOURCE_DATA>,
    ) -> anyhow::Result<Self> {
        let mut out: Option<ID3D11Texture2D> = None;
        unsafe {
            device.CreateTexture2D(
                desc,
                data_desc.map(|d| d as *const _),
                Some(&mut out),
            )
        }.map_err(anyhow::Error::from)
        .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce texture pointer")))
        .context("CreateTexture2D")
        .map(Into::into)
    }

    pub fn new_empty_with_desc(
        device: &Dx11Device,
        desc: &D3D11_TEXTURE2D_DESC,
    ) -> anyhow::Result<Self> {
        Self::new_with_desc::<u8>(device, desc, None)
    }

    pub fn desc(&self) -> D3D11_TEXTURE2D_DESC {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            self.as_d3d().GetDesc(&mut desc);
        }
        desc
    }

    pub fn dxgi_format(&self) -> dxgi::DXGI_FORMAT {
        self.desc().Format
    }

    pub const DEFAULT_SAMPLE_DESC: dxgi::DXGI_SAMPLE_DESC = dxgi::DXGI_SAMPLE_DESC {
        Count: 1,
        Quality: 0,
    };
}

impl AsRef<Resource> for ID3D11Texture2D {
    #[inline]
    fn as_ref(&self) -> &Resource {
        let t2: &Texture2 = self.as_ref();
        t2.as_ref()
    }
}
impl From<ID3D11Texture2D> for Resource {
    #[inline]
    fn from(resource: ID3D11Texture2D) -> Self {
        Texture2::from(resource).into()
    }
}
