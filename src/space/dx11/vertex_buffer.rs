use {
    std::{ffi, ptr::NonNull},
    super::prelude::*,
};

#[derive(Clone, PartialEq)]
pub struct VertexBuffer {
    pub buffer: ID3D11Buffer,
    pub stride: u32,
    pub offset: u32,
    pub count: u32,
}

impl VertexBuffer {
    pub fn set_all<const N: usize>(device_context: &ID3D11DeviceContext, slot: u32, buffers: &[&dyn D3d11ContextBindableVertexBuffer; N]) {
        buffers.set(device_context, slot)
    }
}

#[cfg(todo)]
impl D3d11ContextBindableSlot for VertexBuffer {
    fn set(&self, device_context: &ID3D11DeviceContext, slot: u32) {
        unsafe {
            device_context.IASetVertexBuffers(
                slot,
                1,
                Some(self.buffer.as_params().as_ptr()),
                Some(&self.stride),
                Some(&self.offset),
            );
        }
    }
}

pub unsafe trait D3d11ContextBindableVertexBuffer: D3d11ContextBindableSlot {
    fn vertex_buffer_ptr(&self) -> *mut std::ffi::c_void;
    fn vertex_buffer_stride(&self) -> u32;
    fn vertex_buffer_offset(&self) -> u32;

    unsafe fn vertex_buffer_buffer(&self) -> Option<InterfaceRef<'_, ID3D11Buffer>> {
        NonNull::new(self.vertex_buffer_ptr() as *mut _)
            .map(|raw| InterfaceRef::from_raw(raw))
    }
}

unsafe impl D3d11ContextBindableVertexBuffer for VertexBuffer {
    fn vertex_buffer_ptr(&self) -> *mut ffi::c_void {
        self.buffer.as_raw()
    }
    fn vertex_buffer_stride(&self) -> u32 {
        self.stride
    }
    fn vertex_buffer_offset(&self) -> u32 {
        self.offset
    }
}

unsafe impl<B: ?Sized + D3d11ContextBindableVertexBuffer> D3d11ContextBindableVertexBuffer for &'_ B {
    fn vertex_buffer_ptr(&self) -> *mut ffi::c_void {
        D3d11ContextBindableVertexBuffer::vertex_buffer_ptr(*self)
    }
    fn vertex_buffer_stride(&self) -> u32 {
        D3d11ContextBindableVertexBuffer::vertex_buffer_offset(*self)
    }
    fn vertex_buffer_offset(&self) -> u32 {
        D3d11ContextBindableVertexBuffer::vertex_buffer_stride(*self)
    }
    unsafe fn vertex_buffer_buffer(&self) -> Option<InterfaceRef<'_, ID3D11Buffer>> {
        D3d11ContextBindableVertexBuffer::vertex_buffer_buffer(*self)
    }
}

impl<B: ?Sized + D3d11ContextBindableVertexBuffer> D3d11ContextBindableSlot for B {
    fn set(&self, device_context: &ID3D11DeviceContext, slot: u32) {
        let stride = self.vertex_buffer_stride();
        let offset = self.vertex_buffer_offset();
        let buffer = unsafe {
            self.vertex_buffer_buffer()
        };
        unsafe {
            device_context.IASetVertexBuffers(
                slot,
                1,
                Some(buffer.as_params().as_ptr()),
                Some(&stride),
                Some(&offset),
            );
        }
    }
}

impl<const N: usize, B: ?Sized + D3d11ContextBindableVertexBuffer> D3d11ContextBindableSlot for [&'_ B; N] {
    fn set(&self, device_context: &ID3D11DeviceContext, slot: u32) {
        let mut strides = [0u32; N];
        let mut offsets = [0u32; N];
        let mut buffers = [None::<InterfaceRef<ID3D11Buffer>>; N];
        let outputs = strides.iter_mut()
            .zip(offsets.iter_mut())
            .zip(buffers.iter_mut());
        for (vb, o) in self.iter().zip(outputs) {
            let ((stride, offset), buffer) = o;
            *stride = vb.vertex_buffer_stride();
            *offset = vb.vertex_buffer_offset();
            *buffer = unsafe { vb.vertex_buffer_buffer() };
        }
        unsafe {
            device_context.IASetVertexBuffers(
                slot,
                N as u32,
                Some(buffers.as_ptr() as *const Option<ID3D11Buffer>),
                Some(strides.as_ptr()),
                Some(offsets.as_ptr()),
            );
        }
    }
}
