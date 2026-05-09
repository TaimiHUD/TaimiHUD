use {
    crate::{
        buffer::D3dContextBindableIndexBuffer,
        dx11::{
            buffer::{BindFlags, Buffer, BufferFlags, D3D11_BUFFER_DESC},
            prelude::*,
        },
        dxgi::DXGI_FORMAT,
        D3dContextBindable,
    },
    std::{ffi, mem},
};

/// TODO: non-pub fields
#[derive(Debug, Clone, PartialEq)]
pub struct IndexBuffer {
    pub buffer: Buffer,
    pub format: DXGI_FORMAT,
    pub offset: u32,
}

impl IndexBuffer {
    pub const FORMAT_16: DXGI_FORMAT = dxgi::DXGI_FORMAT_R16_UINT;
    pub const FORMAT_32: DXGI_FORMAT = dxgi::DXGI_FORMAT_R32_UINT;

    pub unsafe fn with_parts<B, F>(buffer: B, format: F, offset: usize) -> Self
    where
        B: Into<Buffer>,
        F: Into<DXGI_FORMAT>,
    {
        Self {
            buffer: buffer.into(),
            format: format.into(),
            offset: offset as u32,
        }
    }

    pub fn stride(&self) -> usize {
        match self.format {
            Self::FORMAT_16 => mem::size_of::<u16>(),
            _ => mem::size_of::<u32>(),
        }
    }
    pub fn set_offset(&mut self, offset: usize) -> anyhow::Result<()> {
        let offset = (offset as u32).saturating_mul(self.stride() as u32);
        if offset as usize > self.buffer.size() {
            anyhow::bail!("index buffer offset out of bounds")
        }
        self.offset = offset;
        Ok(())
    }
    pub fn offset(&self) -> usize {
        self.offset as usize / self.stride()
    }

    pub fn new_with_slice<D: D3dBufferData>(
        device: &Dx11Device,
        data: &[D],
        offset: usize,
        flags: BufferFlags,
    ) -> anyhow::Result<Self> {
        let desc = Self::desc_for::<D>(data.len(), flags);
        let init = match data.is_empty() {
            true => None,
            false => Some(data),
        };
        let format = Self::format_for::<D>()?;
        let offset = match offset {
            offset if offset > data.len() => anyhow::bail!("index buffer offset out of bounds"),
            offset => offset * D::stride(),
        };
        Buffer::new_with_desc(device, &desc, init).map(|b| unsafe { Self::with_parts(b, format, offset) })
    }

    pub fn new<D: D3dBufferData>(
        device: &Dx11Device,
        count: usize,
        flags: BufferFlags,
    ) -> anyhow::Result<Self> {
        let format = Self::format_for::<D>()?;
        let desc = Self::desc_for::<D>(count, flags);
        Buffer::new_with_desc::<D>(device, &desc, None).map(|b| unsafe { Self::with_parts(b, format, 0) })
    }
    pub fn format_for<D: D3dBufferData>() -> anyhow::Result<DXGI_FORMAT> {
        Ok(match D::stride() {
            2 => Self::FORMAT_16,
            4 => Self::FORMAT_32,
            _ => anyhow::bail!("index buffer must consist of 32-bit or 16-bit data"),
        })
    }

    pub fn desc_for<D: D3dBufferData>(len: usize, flags: BufferFlags) -> D3D11_BUFFER_DESC {
        Buffer::desc_for::<D, _, _>(len, BindFlags::INDEX, flags)
    }

    pub fn new_snapshot(context: &Dx11Context) -> Option<Self> {
        unsafe {
            let mut out = None;
            let mut format = DXGI_FORMAT::default();
            let mut offset = 0u32;
            context.IAGetIndexBuffer(Some(&mut out), Some(&mut format), Some(&mut offset));

            out.map(|buffer| Self::with_parts(buffer, format, offset as usize))
        }
    }
}

pub unsafe trait D3d11ContextBindableIndexBuffer:
    D3dContextBindableIndexBuffer<Dx11Context>
{
}
unsafe impl<T> D3d11ContextBindableIndexBuffer for T where T: D3dContextBindableIndexBuffer<Dx11Context> {}

unsafe impl D3dContextBindableIndexBuffer<Dx11Context> for IndexBuffer {
    fn index_buffer_ptr(&self) -> *mut ffi::c_void {
        self.buffer.to_ref().as_raw()
    }
    fn index_buffer_format(&self) -> DXGI_FORMAT {
        self.format
    }
    fn index_buffer_offset(&self) -> u32 {
        self.offset
    }
}

impl D3dContextBindable<Dx11Context> for IndexBuffer {
    fn set(&self, device_context: &Dx11Context) {
        unsafe { device_context.IASetIndexBuffer(&self.buffer, self.format, self.offset) }
    }
}
impl D3dContextBindable<Dx11Context> for Option<IndexBuffer> {
    #[inline]
    fn set(&self, device_context: &Dx11Context) {
        self.as_ref().set(device_context)
    }
}
impl D3dContextBindable<Dx11Context> for Option<&'_ IndexBuffer> {
    fn set(&self, device_context: &Dx11Context) {
        match *self {
            Some(buffer) => buffer.set(device_context),
            None => unsafe {
                device_context.IASetIndexBuffer(None, IndexBuffer::FORMAT_32, 0);
            },
        }
    }
}

impl ops::Deref for IndexBuffer {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
