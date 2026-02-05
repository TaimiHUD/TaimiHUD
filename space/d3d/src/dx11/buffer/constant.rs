use {
    crate::{
        dx11::{
            buffer::{BindFlags, Buffer, BufferFlags, D3D11_BUFFER_DESC},
            prelude::*,
        },
        state::D3dStateSnapshot,
        D3dContextBindableSlot,
    },
    std::{mem, slice},
};

impl_d3d! {
    @[transparent(Dx11Child <= ID3D11Buffer)]
    pub struct ConstantBufferV {
        pub buffer: Buffer,
    }
    @from()
    @into()
    @deref(Buffer);
}

impl ConstantBufferV {
    pub const SLOT_COUNT: usize = Buffer::CONSTANT_SLOT_COUNT;

    pub const fn from_buffer(buffer: Buffer) -> Self {
        Self { buffer }
    }

    pub const fn from_buffer_ref(buffer: &Dx11Buffer) -> &Self {
        unsafe { mem::transmute(buffer) }
    }

    pub fn new_with_data<D: D3dBufferData>(device: &Dx11Device, data: &D) -> anyhow::Result<Self> {
        Self::new_with_slice(device, slice::from_ref(data))
    }

    pub fn new_with_slice<D: D3dBufferData>(device: &Dx11Device, data: &[D]) -> anyhow::Result<Self> {
        let desc = Self::desc_for::<D>(data.len(), None);
        #[cfg(todo)]
        let data = match data.is_empty() {
            true => None,
            false => Some(data),
        };
        let data = Some(data);
        Buffer::new_with_desc(device, &desc, data).map(Into::into)
    }

    #[cfg(todo)]
    pub fn new_empty(device: &Dx11Device, flags: Option<BufferFlags>) -> anyhow::Result<Self> {
        let desc = Self::desc_for::<[f32; 4]>(0, flags);
        Buffer::new_with_desc::<[f32; 4]>(device, &desc, None).map(Into::into)
    }

    pub fn new_singleton<D: D3dBufferData>(
        device: &Dx11Device,
        flags: Option<BufferFlags>,
    ) -> anyhow::Result<Self> {
        let desc = Self::desc_for::<D>(1, flags);
        Buffer::new_with_desc::<D>(device, &desc, None).map(Into::into)
    }

    pub fn desc_for<D: D3dBufferData>(len: usize, flags: Option<BufferFlags>) -> D3D11_BUFFER_DESC {
        let flags = match flags {
            Some(misc) => misc,
            //None if len > 1 => BufferFlags::BUFFER_STRUCTURED,
            None => BufferFlags::default(),
        };
        Buffer::desc_for::<D, _, _>(len, BindFlags::CONSTANT, flags)
    }

    pub fn new_snapshot_in<'v, V: ?Sized>(context: &Dx11Context, slot: u32, out: &'v mut V) where
        V: AsMut<[Option<Self>]>,
    {
        let out = out.as_mut();
        unsafe {
            let out = mem::transmute::<&'v mut [Option<Self>], &'v mut [Option<Dx11Buffer>]>(out);
            context.VSGetConstantBuffers(slot, Some(out));
        }
    }
    pub fn new_snapshot<const N: usize>(context: &Dx11Context, slot: u32) -> [Option<Self>; N] {
        let mut buffers = [const { None::<Self> }; N];
        Self::new_snapshot_in(context, slot, &mut buffers);
        buffers
    }
    pub fn new_snapshot_vec(context: &Dx11Context, slot: ops::Range<u32>) -> Vec<Option<Self>> {
        let mut buffers = vec![const { None::<Self> }; slot.len()];
        Self::new_snapshot_in(context, slot.start, &mut buffers[..]);
        buffers
    }

    pub fn update_singleton<D: D3dBufferData>(&self, device_context: &Dx11Context, data: &D) {
        unsafe {
            self.buffer
                .update_all_unchecked(device_context, slice::from_ref(data), 0)
        }
    }

    pub fn update_all<D: D3dBufferData>(&self, device_context: &Dx11Context, data: &[D]) {
        unsafe { self.buffer.update_all_unchecked(device_context, data, 0) }
    }

    #[inline]
    pub fn slice_as_buffer_mut(buffers: &mut [Option<Self>]) -> &mut [Option<Buffer>] {
        unsafe { mem::transmute(buffers) }
    }
    #[inline]
    pub fn slice_from_buffer_mut(buffers: &mut [Option<Buffer>]) -> &mut [Option<Self>] {
        unsafe { mem::transmute(buffers) }
    }
}

impl_d3d! {
    @[transparent(Dx11Child <= ID3D11Buffer)]
    pub struct ConstantBufferP {
        pub buffer: Buffer,
    }
    @from()
    @into()
    @deref(Buffer);
}

impl ConstantBufferP {
    pub const SLOT_COUNT: usize = Buffer::CONSTANT_SLOT_COUNT;

    pub const fn from_buffer(buffer: Buffer) -> Self {
        Self { buffer }
    }

    pub const fn from_buffer_ref(buffer: &Buffer) -> &Self {
        unsafe { mem::transmute(buffer) }
    }

    pub fn new_with_data<D: D3dBufferData>(device: &Dx11Device, data: &D) -> anyhow::Result<Self> {
        Self::new_with_slice(device, slice::from_ref(data))
    }

    pub fn new_with_slice<D: D3dBufferData>(device: &Dx11Device, data: &[D]) -> anyhow::Result<Self> {
        ConstantBufferV::new_with_slice::<D>(device, data).map(Into::into)
    }

    #[cfg(todo)]
    pub fn new_empty(device: &Dx11Device, flags: Option<BufferFlags>) -> anyhow::Result<Self> {
        ConstantBufferV::new_empty(device, flags).map(Into::into)
    }

    pub fn new_singleton<D: D3dBufferData>(
        device: &Dx11Device,
        flags: Option<BufferFlags>,
    ) -> anyhow::Result<Self> {
        ConstantBufferV::new_singleton::<D>(device, flags).map(Into::into)
    }

    pub fn desc_for<D: D3dBufferData>(len: usize, flags: Option<BufferFlags>) -> D3D11_BUFFER_DESC {
        ConstantBufferV::desc_for::<D>(len, flags)
    }

    pub fn new_snapshot_in<'v, V: ?Sized>(context: &Dx11Context, slot: u32, out: &'v mut V) where
        V: AsMut<[Option<Self>]>,
    {
        let out = out.as_mut();
        unsafe {
            let out = mem::transmute::<&'v mut [Option<Self>], &'v mut [Option<Dx11Buffer>]>(out);
            context.PSGetConstantBuffers(slot, Some(out));
        }
    }
    pub fn new_snapshot<const N: usize>(context: &Dx11Context, slot: u32) -> [Option<Self>; N] {
        let mut buffers = [const { None::<Self> }; N];
        Self::new_snapshot_in(context, slot, &mut buffers);
        buffers
    }
    pub fn new_snapshot_vec(context: &Dx11Context, slot: ops::Range<u32>) -> Vec<Option<Self>> {
        let mut buffers = vec![const { None::<Self> }; slot.len()];
        Self::new_snapshot_in(context, slot.start, &mut buffers[..]);
        buffers
    }

    pub fn update_singleton<D: D3dBufferData>(&self, device_context: &Dx11Context, data: &D) {
        ConstantBufferV::from_buffer_ref(self.as_ref()).update_singleton::<D>(device_context, data)
    }

    pub fn update_all<D: D3dBufferData>(&self, device_context: &Dx11Context, data: &[D]) {
        ConstantBufferV::from_buffer_ref(self.as_ref()).update_all::<D>(device_context, data)
    }

    #[inline]
    pub fn slice_as_buffer_mut(buffers: &mut [Option<Self>]) -> &mut [Option<Buffer>] {
        unsafe { mem::transmute(buffers) }
    }
    #[inline]
    pub fn slice_from_buffer_mut(buffers: &mut [Option<Buffer>]) -> &mut [Option<Self>] {
        unsafe { mem::transmute(buffers) }
    }
}

impl D3dContextBindableSlot<Dx11Context> for ConstantBufferV {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_constant_vertex(self, device_context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for [ConstantBufferV] {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_constant_vertex(self, device_context, slot)
    }
}

impl D3dContextBindableSlot<Dx11Context> for ConstantBufferP {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_constant_pixel(self, device_context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for [ConstantBufferP] {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        Buffer::set_all_constant_pixel(self, device_context, slot)
    }
}

impl From<ConstantBufferP> for ConstantBufferV {
    fn from(buffer: ConstantBufferP) -> Self {
        Self::from_buffer(buffer.into())
    }
}
impl From<ConstantBufferV> for ConstantBufferP {
    fn from(buffer: ConstantBufferV) -> Self {
        Self::from_buffer(buffer.into())
    }
}
impl AsRef<ConstantBufferP> for ConstantBufferV {
    fn as_ref(&self) -> &ConstantBufferP {
        unsafe { mem::transmute(self) }
    }
}
impl AsRef<ConstantBufferV> for ConstantBufferP {
    fn as_ref(&self) -> &ConstantBufferV {
        unsafe { mem::transmute(self) }
    }
}

impl<const N: usize> D3dStateSnapshot<Dx11Context> for [Option<ConstantBufferV>; N] {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok([const { None }; N])
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        ConstantBufferV::new_snapshot::<N>(context, 0)
    }
}
impl<const N: usize> D3dStateSnapshot<Dx11Context> for [Option<ConstantBufferP>; N] {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok([const { None }; N])
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        ConstantBufferP::new_snapshot::<N>(context, 0)
    }
}
impl D3dStateSnapshot<Dx11Context> for Vec<Option<ConstantBufferV>> {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok(Vec::new())
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        ConstantBufferV::new_snapshot_vec(context, 0..ConstantBufferV::SLOT_COUNT as u32)
    }
}
impl D3dStateSnapshot<Dx11Context> for Vec<Option<ConstantBufferP>> {
    fn empty_state(_: &Dx11Device) -> anyhow::Result<Self> {
        Ok(Vec::new())
    }
    fn snapshot_state(context: &Dx11Context) -> Self {
        ConstantBufferP::new_snapshot_vec(context, 0..ConstantBufferP::SLOT_COUNT as u32)
    }
}

// TODO: ConstantBufferG and ConstantBufferC
// TODO: impl AsRef/AsMut for various conversions to buffer slices and params
