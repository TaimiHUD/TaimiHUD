use {
    crate::{
        buffer::D3dContextBindableVertexBuffer,
        dx11::{
            buffer::{BindFlags, BufferFlags, Resource, Usage},
            prelude::*,
        },
        shader::ShaderKind,
        D3dContextBindableSlot,
    },
    std::{fmt, marker::PhantomData},
};

pub use crate::dx11::d3d11::{ID3D11Buffer, D3D11_BOX, D3D11_BUFFER_DESC, D3D11_SUBRESOURCE_DATA};

impl_d3d! {
    unsafe impl Dx11Child for ID3D11Buffer;

    @[transparent(Dx11Child <= ID3D11Buffer)]
    pub struct Buffer {
        pub buffer: Resource,
    }
    @into()
    @deref(Resource);
}

impl Buffer {
    /// 32?
    pub const VERTEX_SLOT_COUNT: usize = d3d11::D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT as usize;
    /// 14?
    pub const CONSTANT_SLOT_COUNT: usize =
        d3d11::D3D11_COMMONSHADER_CONSTANT_BUFFER_API_SLOT_COUNT as usize;

    pub fn desc(&self) -> D3D11_BUFFER_DESC {
        let mut desc = D3D11_BUFFER_DESC::default();
        unsafe {
            self.as_d3d().GetDesc(&mut desc);
        }
        desc
    }

    pub fn size(&self) -> usize {
        self.desc().ByteWidth as usize
    }

    pub fn count_of<D: D3dBufferData>(&self) -> usize {
        self.size() / D::stride()
    }

    pub fn desc_for<D: D3dBufferData, B, M>(len: usize, bind: B, flags: M) -> D3D11_BUFFER_DESC
    where
        B: Into<BindFlags>,
        M: Into<BufferFlags>,
    {
        let flags: BufferFlags = flags.into();
        let bind: BindFlags = bind.into();
        let stride = match flags {
            #[cfg(todo = "unnecessary")]
            flags if !flags.contains(BufferFlags::BUFFER_STRUCTURED) => 0,
            _ => D::stride() as u32,
        };
        let size = D::stride().saturating_mul(len);
        D3D11_BUFFER_DESC {
            ByteWidth: (size as u32).next_multiple_of(16),
            Usage: Usage::DEFAULT.into(),
            BindFlags: bind.into(),
            CPUAccessFlags: 0,
            MiscFlags: flags.into(),
            StructureByteStride: stride,
        }
    }

    pub fn new_with_desc<D: D3dBufferData>(
        device: &Dx11Device,
        desc: &D3D11_BUFFER_DESC,
        initial: Option<&[D]>,
    ) -> anyhow::Result<Self> {
        let initial_desc = D3D11_SUBRESOURCE_DATA {
            pSysMem: match &initial {
                &Some(data) => {
                    if desc.ByteWidth as usize > (data.len() * D::stride()).next_multiple_of(16) {
                        anyhow::bail!(
                            "initial buffer {}x{} is too small for size={}",
                            data.len(),
                            D::stride(),
                            desc.ByteWidth
                        );
                    }
                    data.as_ptr().cast()
                },
                None => ptr::null(),
            },
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let mut out: Option<ID3D11Buffer> = None;
        unsafe {
            device.CreateBuffer(
                desc,
                Some(&initial_desc), //.map(|d| d as *const _),
                Some(&mut out),
            )
        }
        .map_err(anyhow::Error::from)
        .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce buffer pointer")))
        .context("CreateBuffer")
        .map(Into::into)
    }

    pub fn default_with_data<D: D3dBufferData>(
        device: &Dx11Device,
        initial: Result<&[D], usize>,
        bind: BindFlags,
    ) -> anyhow::Result<Self> {
        let len = match &initial {
            Ok(initial) => initial.len(),
            &Err(len) => len,
        };
        let desc = D3D11_BUFFER_DESC {
            ..Self::desc_for::<D, _, _>(len, bind, BufferFlags::empty())
        };

        Self::new_with_desc(device, &desc, initial.ok())
    }

    pub fn replace<D: D3dBufferData>(
        &mut self,
        device: &Dx11Device,
        device_context: &Dx11Context,
        data: &[D],
    ) -> anyhow::Result<bool> {
        let size = mem::size_of_val(data).next_multiple_of(16) as u32;
        let mut desc = self.desc();
        if size == desc.ByteWidth {
            unsafe {
                self.update_all_unchecked(device_context, data, 0);
            }
            Ok(false)
        } else {
            desc.ByteWidth = size;
            *self = Self::new_with_desc(device, &desc, Some(data))?;
            Ok(true)
        }
    }

    pub fn update_all<D: D3dBufferData>(&self, device_context: &Dx11Context, data: &[D]) {
        debug_assert_eq!(mem::size_of_val(data).next_multiple_of(16), self.size());
        unsafe { self.update_all_unchecked(device_context, data, 0) }
    }

    pub fn update_singleton<D: D3dBufferData>(&self, device_context: &Dx11Context, data: &D) {
        self.update_all(device_context, slice::from_ref(data))
    }

    pub unsafe fn update_all_unchecked<D: D3dBufferData>(
        &self,
        device_context: &Dx11Context,
        data: &[D],
        subresource: u32,
    ) {
        unsafe {
            device_context.UpdateSubresource(&self.buffer, subresource, None, data.as_ptr().cast(), 0, 0);
        }
    }

    pub fn offset_box1(offset: ops::Range<u32>) -> D3D11_BOX {
        let min = Point2::new(offset.start, 0);
        let max = Point2::new(offset.end, 1);
        Self::offset_box2(Box2::new(min, max))
    }

    pub fn offset_box2(offset: Box2<u32>) -> D3D11_BOX {
        let min = offset.min.extend(0);
        let max = offset.max.extend(1);
        Self::offset_box3(Box3::new(min, max))
    }

    pub fn offset_box3(offset: Box3<u32>) -> D3D11_BOX {
        D3D11_BOX {
            left: offset.min.x,
            right: offset.max.y,
            top: offset.min.y,
            bottom: offset.max.y,
            front: offset.min.z,
            back: offset.max.z,
        }
    }

    pub unsafe fn update_element_at<D: D3dBufferData>(
        &self,
        device_context: &Dx11Context,
        data: &D,
        offset: usize,
        subresource: u32,
    ) {
        let (dst, row_pitch, depth_pitch) = match offset {
            0 => (None, 0, 0),
            offset => {
                let offset = offset as u32;
                let end = (offset + 1) as u32;
                let stride = D::stride() as u32;
                (Some(Self::offset_box1(offset..end)), stride, end * stride)
            },
        };
        unsafe {
            let dst = dst.as_ref().map(|d| d as *const _);
            let src = data as *const D;
            device_context.UpdateSubresource(
                &self.buffer,
                subresource,
                dst,
                src.cast(),
                row_pitch,
                depth_pitch,
            );
        }
    }

    pub unsafe fn update_at<D: D3dBufferData>(
        &self,
        device_context: &Dx11Context,
        data: &[D],
        offset: usize,
        subresource: u32,
    ) {
        let (dst, row_pitch, depth_pitch) = {
            let offset = offset as u32;
            let start = Point2::new(0, offset);
            let end = Point2::new(1, offset + data.len() as u32);
            (
                Some(Self::offset_box2(Box2::new(start, end))),
                D::stride() as u32,
                self.size() as u32,
            )
        };
        unsafe {
            let dst = dst.as_ref().map(|d| d as *const _);
            device_context.UpdateSubresource(
                &self.buffer,
                subresource,
                dst,
                data.as_ptr().cast(),
                row_pitch,
                depth_pitch,
            );
        }
    }

    pub fn set_all_constant_vertex<B>(buffers: B, device_context: &Dx11Context, slot: u32)
    where
        B: ID3D11ResourceOf<ID3D11Buffer>,
    {
        let buffers = buffers.as_params_of();
        unsafe {
            device_context.VSSetConstantBuffers(slot, Some(buffers));
        }
    }

    pub fn set_all_constant_pixel<B>(buffers: B, device_context: &Dx11Context, slot: u32)
    where
        B: ID3D11ResourceOf<ID3D11Buffer>,
    {
        let buffers = buffers.as_params_of();
        unsafe {
            device_context.PSSetConstantBuffers(slot, Some(buffers));
        }
    }

    #[cfg(todo)]
    const SET_VERTEX_LIMIT: usize = Self::VERTEX_SLOT_COUNT;
    const SET_VERTEX_LIMIT: usize = 4;
    pub fn set_all_vertex_params<B>(buffers: &[B], device_context: &Dx11Context, slot: u32)
    where
        [B]: ID3D11ResourceOf<ID3D11Buffer>,
        B: D3dContextBindableVertexBuffer<Dx11Context>,
    {
        // TODO: MaybeUninit
        let mut strides = [0u32; Self::SET_VERTEX_LIMIT];
        let mut offsets = [0u32; Self::SET_VERTEX_LIMIT];
        let mut strides_storage;
        let mut offsets_storage;
        let (strides, offsets) = match buffers.len() {
            count if count <= Self::SET_VERTEX_LIMIT => (&mut strides[..], &mut offsets[..]),
            count => {
                //log::info!("binding {count} vertex buffer slots, consider reducing!");
                strides_storage = vec![0u32; count];
                offsets_storage = vec![0u32; count];
                (&mut strides_storage[..], &mut offsets_storage[..])
            },
        };
        let outputs = strides.iter_mut().zip(offsets.iter_mut());
        for (buffer, (stride, offset)) in buffers.iter().zip(outputs) {
            *stride = buffer.vertex_buffer_stride();
            *offset = buffer.vertex_buffer_offset();
        }
        let buffers = buffers.as_params_of();
        unsafe {
            device_context.IASetVertexBuffers(
                slot,
                buffers.len() as u32,
                Some(buffers.as_ptr()),
                Some(strides.as_ptr()),
                Some(offsets.as_ptr()),
            );
        }
    }

    pub fn set_all_vertex<B, I>(buffers: I, device_context: &Dx11Context, slot: u32)
    where
        I: IntoIterator<Item = B>,
        I::IntoIter: ExactSizeIterator,
        B: D3dContextBindableVertexBuffer<Dx11Context>,
    {
        let mut bufs = [ptr::null_mut(); Self::SET_VERTEX_LIMIT];
        let mut strides = [0u32; Self::SET_VERTEX_LIMIT];
        let mut offsets = [0u32; Self::SET_VERTEX_LIMIT];
        let mut bufs_storage;
        let mut strides_storage;
        let mut offsets_storage;
        let buffers = buffers.into_iter();
        let buflen = buffers.len();
        let (bufs, strides, offsets) = match buflen {
            count if count <= Self::SET_VERTEX_LIMIT => (&mut bufs[..], &mut strides[..], &mut offsets[..]),
            count => {
                //log::info!("binding {count} vertex buffer slots, consider reducing!");
                bufs_storage = vec![ptr::null_mut(); count];
                strides_storage = vec![0u32; count];
                offsets_storage = vec![0u32; count];
                (
                    &mut bufs_storage[..],
                    &mut strides_storage[..],
                    &mut offsets_storage[..],
                )
            },
        };
        let outputs = bufs.iter_mut().zip(strides.iter_mut()).zip(offsets.iter_mut());
        for (buffer, ((ptr, stride), offset)) in buffers.into_iter().zip(outputs) {
            *ptr = buffer.vertex_buffer_ptr();
            *stride = buffer.vertex_buffer_stride();
            *offset = buffer.vertex_buffer_offset();
        }
        unsafe {
            device_context.IASetVertexBuffers(
                slot,
                buflen as u32,
                Some(bufs.as_ptr() as *const Option<ID3D11Buffer>),
                Some(strides.as_ptr()),
                Some(offsets.as_ptr()),
            );
        }
    }

    pub fn set_one_vertex<B>(buffer: &B, device_context: &Dx11Context, slot: u32)
    where
        B: ?Sized + D3dContextBindableVertexBuffer<Dx11Context>,
    {
        let buf = buffer.vertex_buffer_ptr();
        let stride = buffer.vertex_buffer_stride();
        let offset = buffer.vertex_buffer_offset();
        let buf = &buf;
        unsafe {
            device_context.IASetVertexBuffers(
                slot,
                1,
                Some(buf as *const *mut _ as *const Option<ID3D11Buffer>),
                Some(&stride),
                Some(&offset),
            );
        }
    }

    pub fn new_snapshot_in<'v, V: ?Sized>(
        kind: ShaderKind,
        context: &Dx11Context,
        slot: u32,
        out: &'v mut V,
    ) where
        V: AsMut<[Option<Self>]>,
    {
        let out = out.as_mut();
        match kind {
            ShaderKind::Vertex => {
                use super::ConstantBufferV;
                ConstantBufferV::new_snapshot_in(context, slot, ConstantBufferV::slice_from_buffer_mut(out))
            },
            ShaderKind::Pixel => {
                use super::ConstantBufferP;
                ConstantBufferP::new_snapshot_in(context, slot, ConstantBufferP::slice_from_buffer_mut(out))
            },
        }
    }
    pub fn new_snapshot_vec(
        kind: ShaderKind,
        context: &Dx11Context,
        slot: ops::Range<u32>,
    ) -> Vec<Option<Self>> {
        let mut views = vec![None::<Self>; slot.len()];
        Self::new_snapshot_in(kind, context, slot.start, &mut views[..]);
        views
    }
}

#[repr(transparent)]
pub struct BufferOf<D: D3dBufferData, const OFF: usize = 0> {
    pub buffer: Buffer,
    pub _data: PhantomData<fn() -> D>,
}

impl<const OFF: usize, D: D3dBufferData> BufferOf<D, OFF> {
    pub fn new_with_data<M>(
        device: &Dx11Device,
        initial: Result<&[D], usize>,
        flags: M,
    ) -> anyhow::Result<Self>
    where
        M: Into<BufferFlags>,
    {
        let len = match &initial {
            Ok(initial) => initial.len(),
            &Err(len) => len,
        };
        let desc = Buffer::desc_for::<D, _, _>(len, BindFlags::VERTEX, flags.into());

        Buffer::new_with_desc(device, &desc, initial.ok()).map(Self::with_buffer)
    }

    pub const fn with_buffer(buffer: Buffer) -> Self {
        Self { buffer, _data: PhantomData }
    }
    pub const fn with_buffer_ref(buffer: &Buffer) -> &Self {
        unsafe { mem::transmute(buffer) }
    }
    pub const fn from_d3d_ref(buffer: &ID3D11Buffer) -> &Self {
        Self::with_buffer_ref(Buffer::from_d3d_ref(buffer))
    }

    pub fn count(&self) -> usize {
        self.buffer.count_of::<D>()
    }
}

impl<const OFF: usize, D: D3dBufferData> ops::Deref for BufferOf<D, OFF> {
    type Target = Buffer;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
impl<const OFF: usize, D: D3dBufferData> AsRef<Buffer> for BufferOf<D, OFF> {
    #[inline]
    fn as_ref(&self) -> &Buffer { &self.buffer }
}

impl<const OFF: usize, D: D3dBufferData> From<BufferOf<D, OFF>> for Buffer {
    #[inline]
    fn from(buffer: BufferOf<D, OFF>) -> Self {
        buffer.buffer
    }
}

unsafe impl<const OFF: usize, D: D3dBufferData> D3dInterfacePtr for BufferOf<D, OFF> {
    type Interface = ID3D11Buffer;

    fn as_d3d_param(&self) -> &Option<Self::Interface> {
        self.buffer.as_d3d_param()
    }

    fn into_d3d_param(self) -> Option<Self::Interface> {
        self.buffer.into_d3d_param()
    }

    fn from_d3d_param(param: &Self::Interface) -> &Self {
        Self::with_buffer_ref(D3dInterfacePtr::from_d3d_param(param))
    }
}

impl<const OFF: usize, D: D3dBufferData> crate::dx11::ID3D11ResourceExt for BufferOf<D, OFF> {
    type Output = ID3D11Buffer;

    fn as_params(&self) -> &[Option<Self::Output>] {
        self.buffer.as_params()
    }
}

unsafe impl<const OFF: usize, D: D3dBufferData> D3dInterfacePtr for Option<BufferOf<D, OFF>> {
    type Interface = ID3D11Buffer;

    fn as_d3d_param(&self) -> &Option<Self::Interface> {
        let buffer: &Option<Buffer> = unsafe { mem::transmute(self) };
        buffer.as_d3d_param()
    }

    fn into_d3d_param(self) -> Option<Self::Interface> {
        self.and_then(D3dInterfacePtr::into_d3d_param)
    }

    fn from_d3d_param(param: &Self::Interface) -> &Self {
        unsafe { mem::transmute(Option::<Buffer>::from_d3d_param(param)) }
    }
}

impl<const OFF: usize, D: D3dBufferData> crate::dx11::ID3D11ResourceExt for Option<BufferOf<D, OFF>> {
    type Output = ID3D11Buffer;

    fn as_params(&self) -> &[Option<Self::Output>] {
        let buffer: &Option<Buffer> = unsafe { mem::transmute(self) };
        buffer.as_params()
    }
}

unsafe impl<const OFF: usize, D: D3dBufferData> D3dContextBindableVertexBuffer<Dx11Context>
    for BufferOf<D, OFF>
{
    fn vertex_buffer_ptr(&self) -> *mut std::ffi::c_void {
        self.buffer.to_ref().as_raw()
    }
    fn vertex_buffer_stride(&self) -> u32 {
        D::stride() as u32
    }
    fn vertex_buffer_offset(&self) -> u32 {
        OFF as u32
    }
}
impl<const OFF: usize, D: D3dBufferData> D3dContextBindableSlot<Dx11Context> for BufferOf<D, OFF> {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_one_vertex(self, device_context, slot)
    }
}
impl<const OFF: usize, D: D3dBufferData> D3dContextBindableSlot<Dx11Context> for Option<BufferOf<D, OFF>> {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_one_vertex(self, device_context, slot)
    }
}
impl<const OFF: usize, D: D3dBufferData> D3dContextBindableSlot<Dx11Context>
    for Option<&'_ BufferOf<D, OFF>>
{
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_one_vertex(self, device_context, slot)
    }
}
impl<const OFF: usize, D: D3dBufferData> D3dContextBindableSlot<Dx11Context> for [BufferOf<D, OFF>] {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex_params(self, device_context, slot)
    }
}
impl<const OFF: usize, D: D3dBufferData> D3dContextBindableSlot<Dx11Context>
    for [Option<BufferOf<D, OFF>>]
{
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex_params(self, device_context, slot)
    }
}
impl<const OFF: usize, D: D3dBufferData> D3dContextBindableSlot<Dx11Context> for [&'_ BufferOf<D, OFF>] {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex(self, device_context, slot)
    }
}
impl<const OFF: usize, D: D3dBufferData> D3dContextBindableSlot<Dx11Context>
    for [&'_ Option<BufferOf<D, OFF>>]
{
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex(self, device_context, slot)
    }
}
impl<const OFF: usize, D: D3dBufferData> D3dContextBindableSlot<Dx11Context>
    for [Option<&'_ BufferOf<D, OFF>>]
{
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_vertex(self, device_context, slot)
    }
}

impl<const OFF: usize, D: D3dBufferData> Clone for BufferOf<D, OFF> {
    fn clone(&self) -> Self {
        Self::with_buffer(Clone::clone(&self.buffer))
    }
    fn clone_from(&mut self, other: &Self) {
        self.buffer.clone_from(&other.buffer)
    }
}

impl<const OFF: usize, D: D3dBufferData, B> PartialEq<B> for BufferOf<D, OFF> where
    B: AsRef<Buffer>
{
    fn eq(&self, rhs: &B) -> bool {
        self.buffer.eq(rhs.as_ref())
    }
}
impl<const OFF: usize, D: D3dBufferData> Eq for BufferOf<D, OFF> {}

impl<const OFF: usize, D: D3dBufferData> fmt::Debug for BufferOf<D, OFF> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_struct("BufferOf");
        if OFF > 0 {
            f.field("offset", &OFF);
        }
        f.field("buffer", &self.buffer)
            .finish()
    }
}
