use {
    crate::{
        dx11::{
            buffer::{Resource, Texture2},
            prelude::*,
        },
        shader::ShaderKind,
        state::D3dStateSnapshot,
        D3dContextBindable,
        D3dContextBindableSlot,
    },
    std::mem,
};

pub use crate::{
    d3d::D3D_SRV_DIMENSION,
    dx11::d3d11::{
        ID3D11ShaderResourceView,
        ID3D11ShaderResourceView1,
        ID3D11UnorderedAccessView,
        ID3D11UnorderedAccessView1,
        ID3D11View,
        D3D11_SHADER_RESOURCE_VIEW_DESC,
        D3D11_SHADER_RESOURCE_VIEW_DESC1,
        D3D11_SHADER_RESOURCE_VIEW_DESC1_0,
        D3D11_SHADER_RESOURCE_VIEW_DESC_0,
        D3D11_TEX2D_SRV,
        D3D11_UNORDERED_ACCESS_VIEW_DESC,
        D3D11_UNORDERED_ACCESS_VIEW_DESC1,
        D3D11_UNORDERED_ACCESS_VIEW_DESC1_0,
        D3D11_UNORDERED_ACCESS_VIEW_DESC_0,
    },
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
impl_d3d! {
    unsafe impl Dx11Child for ID3D11ShaderResourceView1;

    @[transparent(Dx11Child <= ID3D11ShaderResourceView1)]
    pub struct ShaderResourceView1 {
        pub srv0: ShaderResourceView,
    }
    @into()
    @deref(ShaderResourceView);
}
impl_d3d! {
    unsafe impl Dx11Child for ID3D11UnorderedAccessView;

    @[transparent(Dx11Child <= ID3D11UnorderedAccessView)]
    pub struct UnorderedAccessView {
        pub view: View,
    }
    @into()
    @deref(View);
}
impl_d3d! {
    unsafe impl Dx11Child for ID3D11UnorderedAccessView1;

    @[transparent(Dx11Child <= ID3D11UnorderedAccessView1)]
    pub struct UnorderedAccessView1 {
        pub uav0: UnorderedAccessView,
    }
    @into()
    @deref(UnorderedAccessView);
}

impl View {
    pub fn get_resource(&self) -> anyhow::Result<Resource> {
        unsafe {
            self.as_d3d()
                .GetResource()
                .context("ID3D11ShaderResourceView::GetResource")
                .map(Resource::from_d3d)
        }
    }
}

impl UnorderedAccessView {
    pub fn get_desc(&self) -> D3D11_UNORDERED_ACCESS_VIEW_DESC {
        let mut desc = Default::default();
        unsafe {
            self.as_d3d().GetDesc(&mut desc);
        }
        desc
    }
}
impl UnorderedAccessView1 {
    pub fn get_desc1(&self) -> D3D11_UNORDERED_ACCESS_VIEW_DESC1 {
        let mut desc = Default::default();
        unsafe {
            self.as_d3d().GetDesc1(&mut desc);
        }
        desc
    }
}

impl ShaderResourceView {
    /// up to 128
    pub const MAX_SLOTS: usize = d3d11::D3D11_COMMONSHADER_INPUT_RESOURCE_SLOT_COUNT as usize;

    pub fn new_with_desc<R: AsRef<Resource>>(
        device: &Dx11Device,
        resource: R,
        desc: Option<&D3D11_SHADER_RESOURCE_VIEW_DESC>,
    ) -> anyhow::Result<Self> {
        let resource = resource.as_ref();
        let mut out = None;
        unsafe { device.CreateShaderResourceView(resource, desc.map(|d| d as *const _), Some(&mut out)) }
            .map_err(anyhow::Error::from)
            .and_then(move |()| out.ok_or_else(|| anyhow!("failed to produce view pointer")))
            .context("CreateShaderResourceView")
            .map(Into::into)
    }

    pub fn new_snapshot_in<V: ?Sized>(kind: ShaderKind, context: &Dx11Context, slot: u32, out: &mut V)
    where
        V: AsMut<[Option<Self>]>,
    {
        let out = out.as_mut();
        match kind {
            ShaderKind::Vertex => ShaderResourceViewP::new_snapshot_in(
                context,
                slot,
                ShaderResourceViewP::slice_from_view_mut(out),
            ),
            ShaderKind::Pixel => ShaderResourceViewV::new_snapshot_in(
                context,
                slot,
                ShaderResourceViewV::slice_from_view_mut(out),
            ),
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
    pub fn bind_set<V>(kind: ShaderKind, views: V, context: &Dx11Context, slot: u32)
    where
        V: ID3D11ResourceOf<ID3D11ShaderResourceView>,
    {
        match kind {
            ShaderKind::Vertex => Self::bind_set_vertex(views, context, slot),
            ShaderKind::Pixel => Self::bind_set_pixel(views, context, slot),
        }
    }

    pub fn bind_set_pixel<V>(views: V, context: &Dx11Context, slot: u32)
    where
        V: ID3D11ResourceOf<ID3D11ShaderResourceView>,
    {
        let views = views.as_params_of();
        unsafe {
            context.PSSetShaderResources(slot, Some(views));
        }
    }
    pub fn bind_set_vertex<V>(views: V, context: &Dx11Context, slot: u32)
    where
        V: ID3D11ResourceOf<ID3D11ShaderResourceView>,
    {
        let views = views.as_params_of();
        unsafe {
            context.VSSetShaderResources(slot, Some(views));
        }
    }

    pub fn get_desc(&self) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
        let mut desc = Default::default();
        unsafe {
            self.as_d3d().GetDesc(&mut desc);
        }
        desc
    }

    pub fn vec_from_raw(views: Vec<Option<ID3D11ShaderResourceView>>) -> Vec<Option<Self>> {
        unsafe { mem::transmute(views) }
    }
    pub fn slice_as_raw_mut(views: &mut [Option<Self>]) -> &mut [Option<ID3D11ShaderResourceView>] {
        unsafe { mem::transmute(views) }
    }
}
impl ShaderResourceView1 {
    pub fn get_desc1(&self) -> D3D11_SHADER_RESOURCE_VIEW_DESC1 {
        let mut desc = Default::default();
        unsafe {
            self.as_d3d().GetDesc1(&mut desc);
        }
        desc
    }
}

#[cfg(todo)]
pub struct VertexShaderResourceViews {
    views: V,
}
#[cfg(todo)]
pub struct PixelShaderResourceViews {
    views: V,
}

impl_d3d! {
    @[transparent(Dx11Child <= ID3D11ShaderResourceView)]
    pub struct ShaderResourceViewP {
        pub view: ShaderResourceView,
    }
    @from()
    @into()
    @deref(ShaderResourceView);
}

impl ShaderResourceViewP {
    #[inline(always)]
    pub const fn from_resource_view(view: ShaderResourceView) -> Self {
        Self { view }
    }

    #[inline(always)]
    pub const fn from_resource_view_ref(view: &ID3D11ShaderResourceView) -> &Self {
        unsafe { mem::transmute(view) }
    }

    pub fn new_snapshot_in<V: ?Sized>(context: &Dx11Context, slot: u32, out: &mut V)
    where
        V: AsMut<[Option<Self>]>,
    {
        let views = Self::slice_as_view_mut(out.as_mut());
        unsafe { context.PSGetShaderResources(slot, Some(ShaderResourceView::slice_as_raw_mut(views))) }
    }
    pub fn new_snapshot_vec(context: &Dx11Context, slot: ops::Range<u32>) -> Vec<Option<Self>> {
        let mut views = vec![None::<Self>; slot.len()];
        Self::new_snapshot_in(context, slot.start, &mut views[..]);
        views
    }

    #[inline(always)]
    pub fn slice_as_view_mut(views: &mut [Option<Self>]) -> &mut [Option<ShaderResourceView>] {
        unsafe { mem::transmute(views) }
    }
    #[inline(always)]
    pub fn slice_from_view_mut(views: &mut [Option<ShaderResourceView>]) -> &mut [Option<Self>] {
        unsafe { mem::transmute(views) }
    }
}

impl D3dContextBindableSlot<Dx11Context> for Option<ShaderResourceViewP> {
    #[inline]
    fn set(&self, context: &Dx11Context, slot: u32) {
        ShaderResourceView::bind_set_pixel(self, context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for [Option<ShaderResourceViewP>] {
    #[inline]
    fn set(&self, context: &Dx11Context, slot: u32) {
        ShaderResourceView::bind_set_pixel(self, context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for ShaderResourceViewP {
    #[inline]
    fn set(&self, context: &Dx11Context, slot: u32) {
        ShaderResourceView::bind_set_pixel(self, context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for [ShaderResourceViewP] {
    #[inline]
    fn set(&self, context: &Dx11Context, slot: u32) {
        ShaderResourceView::bind_set_pixel(self, context, slot)
    }
}
impl D3dContextBindable<Dx11Context> for [Option<ShaderResourceViewP>; ShaderResourceView::MAX_SLOTS] {
    #[inline]
    fn set(&self, context: &Dx11Context) {
        ShaderResourceView::bind_set_pixel(self, context, 0)
    }
}
impl_d3d! {
    //impl{D3DC} D3dState<D3DC> for ShaderResourceViewP;
    impl{D3DC} D3dState<D3DC> for [Option<ShaderResourceViewP>; ShaderResourceView::MAX_SLOTS];
}
impl D3dStateSnapshot<Dx11Context> for Vec<Option<ShaderResourceViewP>> {
    #[inline]
    fn empty_state(_device: &Dx11Device) -> anyhow::Result<Self> {
        Ok(Vec::new())
    }

    #[inline]
    fn snapshot_state(context: &Dx11Context) -> Self {
        ShaderResourceViewP::new_snapshot_vec(context, 0..ShaderResourceView::MAX_SLOTS as u32)
    }
}
impl<const N: usize> D3dStateSnapshot<Dx11Context> for [Option<ShaderResourceViewP>; N] {
    #[inline]
    fn empty_state(_device: &Dx11Device) -> anyhow::Result<Self> {
        Ok([const { None }; N])
    }

    #[inline]
    fn snapshot_state(context: &Dx11Context) -> Self {
        let mut snapshot = [const { None }; N];
        ShaderResourceViewP::new_snapshot_in(context, 0, &mut snapshot);
        snapshot
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
    pub struct ShaderResourceViewV {
        pub view: ShaderResourceView,
    }
    @from()
    @into()
    @deref(ShaderResourceView);
}

impl ShaderResourceViewV {
    #[inline(always)]
    pub const fn from_resource_view(view: ShaderResourceView) -> Self {
        Self { view }
    }

    #[inline(always)]
    pub const fn from_resource_view_ref(view: &ID3D11ShaderResourceView) -> &Self {
        unsafe { mem::transmute(view) }
    }

    pub fn new_snapshot_in<V: ?Sized>(context: &Dx11Context, slot: u32, out: &mut V)
    where
        V: AsMut<[Option<Self>]>,
    {
        let views = Self::slice_as_view_mut(out.as_mut());
        unsafe { context.VSGetShaderResources(slot, Some(ShaderResourceView::slice_as_raw_mut(views))) }
    }
    pub fn new_snapshot_vec(context: &Dx11Context, slot: ops::Range<u32>) -> Vec<Option<Self>> {
        let mut views = vec![None::<Self>; slot.len()];
        Self::new_snapshot_in(context, slot.start, &mut views[..]);
        views
    }

    #[inline(always)]
    pub fn slice_as_view_mut(views: &mut [Option<Self>]) -> &mut [Option<ShaderResourceView>] {
        unsafe { mem::transmute(views) }
    }
    #[inline(always)]
    pub fn slice_from_view_mut(views: &mut [Option<ShaderResourceView>]) -> &mut [Option<Self>] {
        unsafe { mem::transmute(views) }
    }
}
impl D3dContextBindableSlot<Dx11Context> for ShaderResourceViewV {
    fn set(&self, context: &Dx11Context, slot: u32) {
        ShaderResourceView::bind_set_vertex(self, context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for Option<ShaderResourceViewV> {
    #[inline]
    fn set(&self, context: &Dx11Context, slot: u32) {
        ShaderResourceView::bind_set_vertex(self, context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for [Option<ShaderResourceViewV>] {
    #[inline]
    fn set(&self, context: &Dx11Context, slot: u32) {
        ShaderResourceView::bind_set_vertex(self, context, slot)
    }
}
impl D3dContextBindableSlot<Dx11Context> for [ShaderResourceViewV] {
    #[inline]
    fn set(&self, context: &Dx11Context, slot: u32) {
        ShaderResourceView::bind_set_vertex(self, context, slot)
    }
}
impl D3dContextBindable<Dx11Context> for [Option<ShaderResourceViewV>; ShaderResourceView::MAX_SLOTS] {
    #[inline]
    fn set(&self, context: &Dx11Context) {
        ShaderResourceView::bind_set_vertex(self, context, 0)
    }
}

impl_d3d! {
    impl{D3DC} D3dState<D3DC> for [Option<ShaderResourceViewV>; ShaderResourceView::MAX_SLOTS];
}
impl D3dStateSnapshot<Dx11Context> for Vec<Option<ShaderResourceViewV>> {
    #[inline]
    fn empty_state(_device: &Dx11Device) -> anyhow::Result<Self> {
        Ok(Vec::new())
    }

    #[inline]
    fn snapshot_state(context: &Dx11Context) -> Self {
        ShaderResourceViewV::new_snapshot_vec(context, 0..ShaderResourceView::MAX_SLOTS as u32)
    }
}
impl<const N: usize> D3dStateSnapshot<Dx11Context> for [Option<ShaderResourceViewV>; N] {
    #[inline]
    fn empty_state(_device: &Dx11Device) -> anyhow::Result<Self> {
        Ok([const { None }; N])
    }

    #[inline]
    fn snapshot_state(context: &Dx11Context) -> Self {
        let mut snapshot = [const { None }; N];
        ShaderResourceViewV::new_snapshot_in(context, 0, &mut snapshot);
        snapshot
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

    #[inline(always)]
    pub fn as_pixel_view(&self) -> &ShaderResourceViewP {
        ShaderResourceViewP::from_resource_view_ref(self.view.as_d3d())
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
    #[inline]
    fn set(&self, context: &Dx11Context, slot: u32) {
        self.as_pixel_view().set(context, slot)
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
