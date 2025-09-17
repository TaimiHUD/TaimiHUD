use {
    std::{array, cell::Cell, ffi, mem, ptr::{self, NonNull}},
    super::{prelude::*, InstanceBufferData},
    anyhow::anyhow,
    windows::Win32::Graphics::Direct3D11::{
        D3D11_BUFFER_DESC, D3D11_SUBRESOURCE_DATA,
    },
};

#[derive(Debug)]
pub struct InstanceBuffer {
    buffer: Cell<NonNull<ffi::c_void>>,
    count: Cell<u32>,
    stride: u32,
}

impl InstanceBuffer {
    /// SAFETY: count and stride must be correct
    #[inline]
    pub unsafe fn with_parts(buffer: ID3D11Buffer, count: usize, stride: usize) -> Self {
        Self {
            buffer: Cell::new(Self::buffer_into_ptr(buffer)),
            count: Cell::new(count as u32),
            stride: stride as u32,
        }
    }

    #[inline]
    pub fn get_buffer(&self) -> ID3D11Buffer {
        unsafe {
            self.as_buffer().to_owned()
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> NonNull<ffi::c_void> {
        self.buffer.get()
    }

    #[inline]
    pub unsafe fn as_buffer(&self) -> InterfaceRef<'_, ID3D11Buffer> {
        InterfaceRef::from_raw(self.as_ptr())
    }

    pub fn buffer_size(&self) -> usize {
        self.get_count() * self.stride as usize
    }

    #[inline]
    pub fn get_count(&self) -> usize {
        self.count.get() as usize
    }

    #[inline]
    pub fn get_stride(&self) -> usize {
        self.stride as usize
    }

    pub unsafe fn set_count(&self, count: usize) {
        self.count.set(count as u32)
    }

    #[inline]
    pub fn buffer_into_ptr(buffer: ID3D11Buffer) -> NonNull<ffi::c_void> {
        let raw = buffer.into_raw();
        unsafe {
            NonNull::new_unchecked(raw)
        }
    }

    #[inline]
    pub fn into_buffer(self) -> ID3D11Buffer {
        let this = mem::ManuallyDrop::new(self);
        unsafe {
            ID3D11Buffer::from_raw(this.as_ptr().as_ptr())
        }
    }

    pub fn set_buffer(&self, buffer: ID3D11Buffer) -> ID3D11Buffer {
        let raw = buffer.into_raw();
        let old = unsafe {
            ID3D11Buffer::from_raw(self.as_ptr().as_ptr())
        };
        self.buffer.set(unsafe {
            NonNull::new_unchecked(raw)
        });
        old
    }

    pub fn create_empty(device: &ID3D11Device) -> anyhow::Result<Self> {
        let count = 1;
        let buffer = Self::buffer_with::<InstanceBufferData>(device, Err(count))?;
        Ok(unsafe {
            Self::with_parts(buffer, count, size_of::<InstanceBufferData>())
        })
    }

    pub fn create(device: &ID3D11Device, data: &[InstanceBufferData]) -> anyhow::Result<Self> {
        let stride = size_of::<InstanceBufferData>();
        let buffer = Self::buffer_with(device, Ok(data))?;
        Ok(unsafe {
            Self::with_parts(buffer, data.len(), stride)
        })
    }

    pub fn buffer_with<E: Copy>(device: &ID3D11Device, initial: Result<&[E], usize>) -> anyhow::Result<ID3D11Buffer> {
        let stride = size_of::<E>();
        let (size, ptr) = match &initial {
            Ok(initial) => (stride * initial.len(), initial.as_ptr()),
            Err(len) => (stride * len, ptr::null()),
        };
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: size as u32,
            Usage: d3d11::D3D11_USAGE_DEFAULT,
            BindFlags: d3d11::D3D11_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: stride as u32,
        };

        let subresource_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: ptr.cast(),
            .. D3D11_SUBRESOURCE_DATA::default()
        };

        let mut ptr: Option<ID3D11Buffer> = None;
        let buffer = unsafe { device.CreateBuffer(&desc, Some(&subresource_data), Some(&mut ptr)) }
            .map_err(anyhow::Error::from)
            .and_then(|()| ptr.ok_or_else(|| anyhow!("no per-entity structured buffer")))?;

        Ok(buffer)
    }

    pub fn update<E: Copy>(
        &self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[E],
    ) -> anyhow::Result<()> {
        if size_of_val(data) == self.buffer_size() {
            unsafe {
                device_context.UpdateSubresource(&*self.as_buffer(), 0, None, data.as_ptr().cast(), 0, 0);
            }
        } else {
            debug_assert_eq!(size_of::<E>(), self.stride as usize);
            let buffer = Self::buffer_with(device, Ok(data))?;
            let _ = self.set_buffer(buffer);
            unsafe {
                self.set_count(data.len());
            }
        }
        Ok(())
    }
}

impl Clone for InstanceBuffer {
    fn clone(&self) -> Self {
        unsafe {
            Self::with_parts(self.get_buffer(), self.get_count(), self.stride as usize)
        }
    }
}

unsafe impl Send for InstanceBuffer { }
/// TODO: a lie
/// (fixable via locks/atomics but we're using d3d11 single-threaded mode anyway)
unsafe impl Sync for InstanceBuffer { }

impl Drop for InstanceBuffer {
    fn drop(&mut self) {
        let buffer = unsafe {
            ID3D11Buffer::from_raw(self.as_ptr().as_ptr())
        };
        drop(buffer);
    }
}

unsafe impl D3d11ContextBindableVertexBuffer for InstanceBuffer {
    fn vertex_buffer_ptr(&self) -> *mut ffi::c_void {
        self.as_ptr().as_ptr()
    }
    fn vertex_buffer_stride(&self) -> u32 {
        self.stride
    }
    fn vertex_buffer_offset(&self) -> u32 {
        0
    }
}
