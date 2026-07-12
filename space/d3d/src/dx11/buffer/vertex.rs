use {
    crate::{
        buffer::D3dContextBindableVertexBuffer,
        dx11::{
            buffer::{BindFlags, Buffer, BufferFlags, D3D11_BUFFER_DESC},
            prelude::*,
        },
        state::D3dStateSnapshot,
        D3dContextBindable,
        D3dContextBindableSlot,
    },
    std::{ffi, mem, slice},
};

/// TODO: non-pub fields
#[derive(Debug, Clone, PartialEq)]
pub struct VertexBuffer {
    pub buffer: Buffer,
    pub stride: u32,
    pub offset: u32,
    count: u32,
}

impl VertexBuffer {
    pub const MAX_COUNT: usize = Buffer::VERTEX_SLOT_COUNT;

    pub unsafe fn with_parts<B>(buffer: B, stride: usize, count: usize, offset: usize) -> Self
    where
        B: Into<Buffer>,
    {
        Self {
            buffer: buffer.into(),
            stride: stride as u32,
            count: count as u32,
            offset: offset as u32,
        }
    }

    pub fn new_with_data<D: D3dBufferData>(
        device: &Dx11Device,
        data: &D,
        flags: BufferFlags,
    ) -> anyhow::Result<Self> {
        Self::new_with_slice(device, slice::from_ref(data), flags)
    }

    pub fn new_with_slice<D: D3dBufferData>(
        device: &Dx11Device,
        data: &[D],
        flags: BufferFlags,
    ) -> anyhow::Result<Self> {
        let desc = Self::desc_for::<D>(data.len(), flags);
        let init = match data.is_empty() {
            true => None,
            false => Some(data),
        };
        Buffer::new_with_desc(device, &desc, init)
            .map(|b| unsafe { Self::with_parts(b, D::stride(), data.len() as _, 0) })
    }

    pub fn new<D: D3dBufferData>(
        device: &Dx11Device,
        count: Option<usize>,
        flags: BufferFlags,
    ) -> anyhow::Result<Self> {
        let count = count.unwrap_or(1);
        let desc = Self::desc_for::<D>(count, flags);
        Buffer::new_with_desc::<D>(device, &desc, None)
            .map(|b| unsafe { Self::with_parts(b, D::stride(), count, 0) })
    }

    pub fn desc_for<D: D3dBufferData>(len: usize, flags: BufferFlags) -> D3D11_BUFFER_DESC {
        Buffer::desc_for::<D, _, _>(len, BindFlags::VERTEX, flags)
    }

    pub fn new_snapshot<const N: usize>(context: &Dx11Context, slot: u32) -> [Option<Self>; N] {
        let mut buffers = [const { None }; N];
        let mut strides = [0u32; N];
        let mut offsets = [0u32; N];
        unsafe {
            let mut out = mem::MaybeUninit::<[Option<VertexBuffer>; N]>::uninit();
            let out_slice = out.as_mut_ptr() as *mut [mem::MaybeUninit<Option<Self>>; N];
            Self::new_snapshot_in(
                context,
                slot,
                &mut buffers,
                &mut strides,
                &mut offsets,
                &mut *out_slice,
            );
            out.assume_init()
        }
    }
    unsafe fn new_snapshot_in(
        context: &Dx11Context,
        slot: u32,
        buffers: &mut [Option<Buffer>],
        strides: &mut [u32],
        offsets: &mut [u32],
        out: &mut [mem::MaybeUninit<Option<Self>>],
    ) {
        unsafe {
            context.IAGetVertexBuffers(
                slot,
                buffers.len() as u32,
                Some(buffers.as_mut_ptr() as *mut Option<Dx11Buffer>),
                Some(strides.as_mut_ptr()),
                Some(offsets.as_mut_ptr()),
            );

            let buffers = buffers.iter_mut().zip(strides.iter().zip(offsets.iter()));

            let mut b = out.as_mut_ptr() as *mut Option<VertexBuffer>;
            for (buffer, (&stride, &offset)) in buffers {
                b.write(buffer.take().map(Buffer::from).map(|buffer| VertexBuffer {
                    count: buffer.desc().ByteWidth / stride.max(1),
                    stride,
                    offset,
                    buffer,
                }));
                b = b.add(1);
            }
        }
    }
    pub fn new_snapshot_vec(context: &Dx11Context, slot: ops::Range<u32>) -> Vec<Option<Self>> {
        let len = slot.len() as usize;
        let mut buffers = vec![const { None }; len];
        let mut strides = vec![0u32; len];
        let mut offsets = vec![0u32; len];
        unsafe {
            let mut out = Vec::with_capacity(len);
            Self::new_snapshot_in(
                context,
                slot.start,
                &mut buffers,
                &mut strides,
                &mut offsets,
                out.spare_capacity_mut(),
            );
            out.set_len(len);
            out
        }
    }

    pub fn set_all(
        device_context: &Dx11Context,
        slot: u32,
        buffers: &[&dyn D3d11ContextBindableVertexBuffer],
    ) {
        Buffer::set_all_vertex(buffers, device_context, slot)
    }

    #[inline]
    pub fn vertex_count(&self) -> u32 {
        self.count
    }
    #[inline]
    pub unsafe fn set_count(&mut self, count: u32) {
        self.count = count;
    }
}

pub unsafe trait D3d11ContextBindableVertexBuffer:
    D3dContextBindableVertexBuffer<Dx11Context>
{
}
unsafe impl<T> D3d11ContextBindableVertexBuffer for T where T: D3dContextBindableVertexBuffer<Dx11Context> {}

unsafe impl D3dContextBindableVertexBuffer<Dx11Context> for VertexBuffer {
    fn vertex_buffer_ptr(&self) -> *mut ffi::c_void {
        self.buffer.to_ref().as_raw()
    }
    fn vertex_buffer_stride(&self) -> u32 {
        self.stride
    }
    fn vertex_buffer_offset(&self) -> u32 {
        self.offset
    }
}

impl D3dContextBindableSlot<Dx11Context> for VertexBuffer {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_one_vertex(self, device_context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for Option<VertexBuffer> {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_one_vertex(self, device_context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for Option<&'_ VertexBuffer> {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_one_vertex(self, device_context, slot)
    }
}
impl<const N: usize> D3dContextBindableSlot<Dx11Context> for [&'_ VertexBuffer; N] {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex(self, device_context, slot)
    }
}
impl<const N: usize> D3dContextBindableSlot<Dx11Context> for [VertexBuffer; N] {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex(self, device_context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for [Option<&'_ VertexBuffer>] {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex(self, device_context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for [Option<VertexBuffer>] {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex(self, device_context, slot)
    }
}
impl D3dContextBindable<Dx11Context> for [Option<VertexBuffer>; VertexBuffer::MAX_COUNT] {
    fn set(&self, device_context: &Dx11Context) {
        Buffer::set_all_vertex(self, device_context, 0)
    }
}

impl<const N: usize> D3dStateSnapshot<Dx11Context> for [Option<VertexBuffer>; N] {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok([const { None }; N])
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        VertexBuffer::new_snapshot::<N>(context, 0)
    }
}
impl D3dStateSnapshot<Dx11Context> for Vec<Option<VertexBuffer>> {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok(Vec::new())
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        VertexBuffer::new_snapshot_vec(context, 0..VertexBuffer::MAX_COUNT as u32)
    }
}

impl ops::Deref for VertexBuffer {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
