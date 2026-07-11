#[cfg(feature = "goggles2-project")]
use taimi_d3d::{
    dx11::{self, Dx11Context},
    prelude::*,
    state::{BufferState, PrimitiveTopology},
};

/// set of buffers and state that will change
#[derive(Debug, Copy, Clone)]
pub enum RenderSnapshotPreset {
    Space,
    Imgui,
    /// [Self::Space] is similar but always at end of frame on present,
    /// so some state may become irrelevant
    #[cfg(feature = "goggles2-project")]
    ProjectSpace,
    #[cfg(feature = "goggles2-project")]
    ProjectImgui,
}
#[allow(non_upper_case_globals)]
impl RenderSnapshotPreset {
    #[cfg(feature = "goggles2-project")]
    pub const ProjectImguiBg: Self = Self::ProjectImgui;

    #[cfg(feature = "goggles2-project")]
    pub fn is_project(&self) -> bool {
        matches!(self, Self::ProjectSpace | Self::ProjectImgui)
    }
}
/// TODO: use for imgui frames too if not hosted by nexus/arcdps/etc
pub struct RenderSnapshot<'c> {
    /// TODO: wrapper struct is dumb because it copies this for each field
    #[cfg(todo)]
    pub context: Option<InterfaceRef<'c, Dx11Context>>,
    pub raster: D3dStateToken<'c, Option<dx11::RasterizerState>>,
    /// TODO: increase to max bleh? or is that 8?
    pub rendertarget: D3dStateToken<'c, dx11::RenderTargetViews<[Option<dx11::RenderTargetView>; 8]>>,
    /// TODO: SmallVecs please?
    pub viewports: D3dStateToken<'c, Vec<dx11::Viewport>>,
    /// TODO: SmallVecs please?
    pub scissors: D3dStateToken<'c, Vec<dx11::ScissorRect>>,
    #[cfg_attr(todo, feature = "goggles2-project")]
    pub prim: D3dStateToken<'c, PrimitiveTopology>,
    #[cfg(feature = "goggles2-project")]
    pub blend: D3dStateToken<'c, dx11::OMBlendState<Option<dx11::BlendState>>>,
    #[cfg(feature = "goggles2-project")]
    pub depth: D3dStateToken<'c, dx11::OMDepthState>,
    #[cfg(feature = "goggles2-project")]
    pub shaderp: D3dStateToken<'c, Option<dx11::ShaderP>>,
    #[cfg(feature = "goggles2-project")]
    pub shaderv: D3dStateToken<'c, Option<dx11::ShaderV>>,
    #[cfg(feature = "goggles2-project")]
    pub shaderh: D3dStateToken<'c, Option<dx11::ShaderH>>,
    #[cfg(feature = "goggles2-project")]
    pub shaderg: D3dStateToken<'c, Option<dx11::ShaderG>>,
    #[cfg(feature = "goggles2-project")]
    pub shaderd: D3dStateToken<'c, Option<dx11::ShaderD>>,
    pub shaderc: D3dStateToken<'c, Option<dx11::ShaderC>>,
    #[cfg(feature = "goggles2-project")]
    pub shaderlayout: D3dStateToken<'c, Option<dx11::shader::InputLayout>>,
    #[cfg(feature = "goggles2-project")]
    pub index: D3dStateToken<'c, Option<dx11::buffer::IndexBuffer>>,
    /// TODO: SmallVecs please?
    #[cfg(feature = "goggles2-project")]
    pub cbufferv: D3dStateToken<'c, BufferState<Vec<Option<dx11::buffer::ConstantBufferV>>>>,
    #[cfg(feature = "goggles2-project")]
    pub cbufferp: D3dStateToken<'c, BufferState<Vec<Option<dx11::buffer::ConstantBufferP>>>>,
    #[cfg(feature = "goggles2-project")]
    pub samplers: D3dStateToken<'c, BufferState<Vec<Option<dx11::buffer::SamplerState>>>>,
    #[cfg(feature = "goggles2-project")]
    pub vertices: D3dStateToken<'c, BufferState<Vec<Option<dx11::VertexBuffer>>>>,
    #[cfg_attr(todo, feature = "goggles2-project")]
    pub srvp: D3dStateToken<'c, BufferState<Vec<Option<dx11::buffer::ShaderResourceViewP>>>>,
    #[cfg(todo = "unnecessary")]
    pub srvv: D3dStateToken<'c, BufferState<Vec<Option<dx11::buffer::ShaderResourceViewV>>>>,
}
#[cfg(feature = "goggles2-project")]
impl<'c> RenderSnapshot<'c> {
    pub fn new_snapshot(c: &'c Dx11Context, preset: RenderSnapshotPreset) -> Self {
        #[cfg(feature = "goggles2-project")]
        let is_project = preset.is_project();
        let snap = Self {
            raster: c.get_snapshot(),
            rendertarget: c.get_snapshot(),
            viewports: c.get_snapshot(),
            scissors: c.get_snapshot(),
            prim: c.get_snapshot(),
            #[cfg(feature = "goggles2-project")]
            blend: c.get_snapshot(),
            #[cfg(feature = "goggles2-project")]
            depth: c.get_snapshot(),
            #[cfg(feature = "goggles2-project")]
            shaderp: is_project.then(|| c.get_snapshot()).unwrap_or_default(),
            #[cfg(feature = "goggles2-project")]
            shaderv: is_project.then(|| c.get_snapshot()).unwrap_or_default(),
            #[cfg(feature = "goggles2-project")]
            shaderh: is_project.then(|| c.get_snapshot()).unwrap_or_default(),
            #[cfg(feature = "goggles2-project")]
            shaderg: is_project.then(|| c.get_snapshot()).unwrap_or_default(),
            #[cfg(feature = "goggles2-project")]
            shaderd: is_project.then(|| c.get_snapshot()).unwrap_or_default(),
            shaderc: is_project.then(|| c.get_snapshot()).unwrap_or_default(),
            #[cfg(feature = "goggles2-project")]
            shaderlayout: c.get_snapshot(),
            #[cfg(feature = "goggles2-project")]
            index: is_project.then(|| c.get_snapshot()).unwrap_or_default(),
            #[cfg(feature = "goggles2-project")]
            cbufferv: is_project.then(|| c.get_snapshot_buffers()).unwrap_or_default(),
            #[cfg(feature = "goggles2-project")]
            cbufferp: is_project.then(|| c.get_snapshot_buffers()).unwrap_or_default(),
            #[cfg(feature = "goggles2-project")]
            samplers: c.get_snapshot_buffers(),
            #[cfg(feature = "goggles2-project")]
            vertices: is_project.then(|| c.get_snapshot_buffers()).unwrap_or_default(),
            srvp: c.get_snapshot_buffers(),
            #[cfg(todo = "unnecessary")]
            srvv: c.get_snapshot_buffers(),
        };
        // TODO: fence here? *shrug*
        Self::reset_defaults(c);
        snap
    }
    /// features we don't use or set but random settings could cause issues
    #[inline]
    pub fn reset_defaults(c: &Dx11Context) {
        None::<dx11::ShaderH>.set(c);
        None::<dx11::ShaderG>.set(c);
        None::<dx11::ShaderD>.set(c);
        None::<dx11::ShaderC>.set(c);
    }
    pub fn pop(self) {
        #[cfg(todo)]
        self.restore(context);
        drop(self)
    }
    /// TODO: reuse vec allocations or replace them with arrays/smallvecs!
    #[cfg(todo)]
    pub fn record_snapshot(&mut self, c: &'c Dx11Context, preset: RenderSnapshotPreset) -> Self {
        #[cfg(todo = "unnecessary")]
        self.discard();
        *self = Self::new_snapshot(c, preset);
    }
    /// release held refcounts
    #[cfg(todo)]
    pub fn discard(&mut self) {
        self.blend.blend = None;
        // TODO: etc
    }
    #[cfg(todo)]
    pub fn restore(&self, context: &Dx11Context) {
        self.raster.restore_state(context);
        self.rendertarget.restore_state(context);
        self.viewports.restore_state(context);
        self.scissors.restore_state(context);
        self.prim.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.blend.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.depth.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.shaderp.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.shaderv.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.shaderh.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.shaderg.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.shaderd.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.shaderc.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.shaderlayout.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.index.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.cbufferv.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.cbufferp.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.samplers.restore_state(context);
        #[cfg(feature = "goggles2-project")]
        self.vertices.restore_state(context);
        self.srvp.restore_state(context);
        #[cfg(todo = "unnecessary")]
        self.srvv.restore_state(context);
    }
    #[cfg(todo)]
    pub fn pop_in_place(&mut self, context: &Dx11Context) {
        self.restore(context);
        self.discard();
    }
}
