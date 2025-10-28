use {
    crate::{
        resources::Model,
        space::{dx11::SwapChain, ScreenSpace},
    },
    glamour::{Box2, Point2, Size2},
    taimi_d3d::dx11::{
        buffer::{Texture2, VertexBuffer, D3D11_TEXTURE2D_DESC},
        depth::{
            ClearFlags,
            ComparisonFunc,
            DepthState,
            DepthView,
            StencilOp,
            D3D11_DEPTH_STENCIL_DESC,
        },
        prelude::*,
        raster::{
            CullMode,
            RasterizerState,
            RenderTargetView,
            RenderTargetViews,
            D3D11_RASTERIZER_DESC,
        },
    },
    taimi_meta::ui::MapCalibration,
};

#[cfg(feature = "goggles")]
use crate::space::goggles;

type OMDepthState = taimi_d3d::dx11::OMDepthState<DepthState>;

#[allow(unused)]
pub struct DepthHandler {
    pub render_target_view: RenderTargetViews<RenderTargetView>,
    pub depth_stencil_state: OMDepthState,
    pub depth_stencil_state_map: OMDepthState,
    pub depth_stencil_state_mask: OMDepthState,
    #[cfg(feature = "goggles")]
    pub depth_stencil_state_write: OMDepthState,
    #[cfg(feature = "goggles")]
    pub depth_stencil_state_obscured: OMDepthState,
    pub fill_quad: VertexBuffer,
    pub fill_edge: Option<VertexBuffer>,
    pub rasterizer_state: RasterizerState,
}

impl DepthHandler {
    pub fn create(
        display_size: Size2<ScreenSpace>,
        device: &Dx11Device,
        swap_chain: &SwapChain,
    ) -> anyhow::Result<Self> {
        let framebuffer = swap_chain.get_framebuffer11(0)?;
        let depth_stencil_state = DepthState::new_with_desc(device, &Self::DEPTH_DESC_ON)?;
        let depth_stencil_state_map = DepthState::new_with_desc(device, &Self::DEPTH_DESC_MAP)?;
        let depth_stencil_state_mask =
            DepthState::new_with_desc(device, &Self::DEPTH_DESC_FILL_OPAQUE)?;
        #[cfg(feature = "goggles")]
        let depth_stencil_state_write = DepthState::new_with_desc(device, &Self::DEPTH_DESC_FILL)?;
        #[cfg(feature = "goggles")]
        let depth_stencil_state_obscured =
            DepthState::new_with_desc(device, &Self::DEPTH_DESC_OBSCURED)?;

        let depth_stencil_buffer_desc = D3D11_TEXTURE2D_DESC {
            Width: display_size.width as _,
            Height: display_size.height as _,
            ..Self::BUFFER_DESC
        };
        let depth_stencil_buffer =
            Texture2::new_empty_with_desc(device, &depth_stencil_buffer_desc)?;
        let depth_stencil_view =
            DepthView::new_with_texture2(device, &depth_stencil_buffer, Default::default(), 0)?;
        #[cfg(feature = "goggles")]
        let _ = DEPTH_STENCIL_VIEW_VTABLE
            .set(unsafe { &*(depth_stencil_view.view.vtable() as *const _) });

        let fill_quad = Self::new_fill_quad(device, None)?;
        let rasterizer_state = RasterizerState::new_with_desc(device, &Self::RASTER_DESC)?;

        let render_target_view = RenderTargetView::new_with_buffer2(device, &framebuffer)?;
        let render_target_view =
            RenderTargetViews::with_views(render_target_view, Some(depth_stencil_view));
        Ok(Self {
            render_target_view,
            depth_stencil_state: OMDepthState::with_state(depth_stencil_state, Self::STENCIL_REF),
            depth_stencil_state_map: OMDepthState::with_state(
                depth_stencil_state_map,
                Self::STENCIL_REF,
            ),
            depth_stencil_state_mask: OMDepthState::with_state(
                depth_stencil_state_mask,
                Self::STENCIL_REF_MASK,
            ),
            #[cfg(feature = "goggles")]
            depth_stencil_state_write: OMDepthState::with_state(
                depth_stencil_state_write,
                Self::STENCIL_REF,
            ),
            #[cfg(feature = "goggles")]
            depth_stencil_state_obscured: OMDepthState::with_state(
                depth_stencil_state_obscured,
                Self::STENCIL_REF,
            ),
            rasterizer_state,
            fill_quad,
            fill_edge: None,
        })
    }

    const STENCIL_REF_MASK: u32 = 0x40;
    const STENCIL_REF: u32 = 0x01;
    const STENCIL_CLEAR: u8 = 0x00;
    const DEPTH_DESC_ON: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        DepthFunc: ComparisonFunc::LESS_EQUAL,
        StencilWriteMask: 0,
        //StencilReadMask: 0xff,
        StencilEnable: BOOL(1),
        FrontFace: Self::STENCILOP_ON,
        BackFace: Self::STENCILOP_ON,
        ..DepthState::DESC_DEFAULT
    };
    const DEPTH_DESC_MAP: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        ..Self::DEPTH_DESC_IGNORE
    };
    const DEPTH_DESC_IGNORE: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: BOOL(0),
        DepthWriteMask: d3d11::D3D11_DEPTH_WRITE_MASK_ZERO,
        DepthFunc: ComparisonFunc::ALWAYS,
        StencilReadMask: 0,
        StencilEnable: BOOL(0),
        ..Self::DEPTH_DESC_ON
    };
    const DEPTH_DESC_FILL: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        DepthWriteMask: d3d11::D3D11_DEPTH_WRITE_MASK_ALL,
        DepthFunc: ComparisonFunc::ALWAYS,
        DepthEnable: BOOL(1),
        StencilEnable: BOOL(0),
        ..DepthState::DESC_DEFAULT
    };
    const DEPTH_DESC_FILL_OPAQUE: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        DepthWriteMask: d3d11::D3D11_DEPTH_WRITE_MASK_ZERO,
        DepthEnable: BOOL(0),
        StencilEnable: BOOL(1),
        StencilReadMask: 0,
        //StencilWriteMask: 0xff,
        FrontFace: Self::STENCILOP_FILL,
        BackFace: Self::STENCILOP_FILL,
        ..Self::DEPTH_DESC_FILL
    };
    #[cfg(feature = "goggles")]
    const DEPTH_DESC_OBSCURED: D3D11_DEPTH_STENCIL_DESC = D3D11_DEPTH_STENCIL_DESC {
        DepthWriteMask: d3d11::D3D11_DEPTH_WRITE_MASK_ZERO,
        DepthFunc: ComparisonFunc::GREATER,
        StencilEnable: BOOL(1),
        FrontFace: Self::STENCILOP_ON,
        BackFace: Self::STENCILOP_ON,
        ..Self::DEPTH_DESC_ON
    };
    const STENCILOP_ON: d3d11::D3D11_DEPTH_STENCILOP_DESC = d3d11::D3D11_DEPTH_STENCILOP_DESC {
        StencilFunc: ComparisonFunc::GREATER,
        ..DepthState::STENCILOP_DEFAULT
    };
    const STENCILOP_FILL: d3d11::D3D11_DEPTH_STENCILOP_DESC = d3d11::D3D11_DEPTH_STENCILOP_DESC {
        StencilFunc: ComparisonFunc::ALWAYS,
        StencilPassOp: StencilOp::REPLACE,
        ..DepthState::STENCILOP_DEFAULT
    };

    const BUFFER_DESC: D3D11_TEXTURE2D_DESC = DepthView::BUFFER2D_DESC_UNSIZED;

    pub fn setup(&self, device_context: &Dx11Context) {
        let (dsview, clear_depth) = self.depth_stencil_view();
        self.rasterizer_state.set(device_context);
        dsview.set(device_context);
        self.depth_stencil_state.set(device_context);
        if let Some(clear_depth) = clear_depth {
            dsview.clear_depth(
                device_context,
                ClearFlags::DEPTH_STENCIL,
                clear_depth,
                Self::STENCIL_CLEAR,
            );
        }
    }

    pub fn depth_stencil_view(
        &self,
    ) -> (
        RenderTargetViews<
            InterfaceRef<'_, d3d11::ID3D11RenderTargetView>,
            InterfaceRef<'_, d3d11::ID3D11DepthStencilView>,
        >,
        Option<f32>,
    ) {
        let dsview = self
            .render_target_view
            .to_ref()
            .map_depth(|d| d.map(|d| d.to_ref()))
            .map_views(RenderTargetView::to_ref);
        let clear_depth = Some(1.0f32);
        #[cfg(feature = "goggles")]
        let (dsview, clear_depth) = match goggles::current_lens() {
            Some(depth) => {
                let dsview = dsview.map_depth(|_| Some(depth));
                (dsview, None)
            },
            None => (dsview, clear_depth),
        };

        (dsview, clear_depth)
    }

    #[cfg(feature = "goggles")]
    pub fn depth_stencil_view_vtbl() -> Option<&'static d3d11::ID3D11DepthStencilView_Vtbl> {
        DEPTH_STENCIL_VIEW_VTABLE.get().copied()
    }

    #[cfg(feature = "goggles")]
    pub fn set_state_obscured(&self, device_context: &Dx11Context, obscured: bool) {
        let state = match obscured {
            true => &self.depth_stencil_state_obscured,
            false => &self.depth_stencil_state,
        };
        state.set(device_context);
    }

    const RASTER_DESC: D3D11_RASTERIZER_DESC = D3D11_RASTERIZER_DESC {
        CullMode: CullMode::NONE,
        FrontCounterClockwise: BOOL(1),
        DepthClipEnable: BOOL(0),
        ScissorEnable: BOOL(1),
        // TODO: on or off?
        MultisampleEnable: BOOL(1),
        AntialiasedLineEnable: BOOL(1),
        ..RasterizerState::DESC_DEFAULT
    };

    pub fn setup_map(&self, device_context: &Dx11Context) {
        self.rasterizer_state.set(device_context);
        //self.render_target_view.to_ref().without_depth().set(device_context);
        //self.render_target_view.set(device_context);
        self.depth_stencil_view().0.set(device_context);
        self.depth_stencil_state_map.set(device_context);
    }

    pub fn setup_depth_write(&self, device_context: &Dx11Context, enable: Option<bool>) {
        if enable.is_none() {
            self.depth_stencil_state.set(device_context);
        }
        self.depth_stencil_view().0.set(device_context);
    }

    pub fn setup_minimap_scissor(&self, device_context: &Dx11Context, bounds: &Box2<ScreenSpace>) {
        let minimap_bounds = Box2::new(
            Point2::new(
                bounds.min.x.round_ties_even(),
                bounds.min.y.round_ties_even(),
            ),
            Point2::new(
                bounds.max.x.round_ties_even(),
                bounds.max.y.round_ties_even(),
            ),
        );
        self.set_scissor(device_context, minimap_bounds)
    }

    pub fn set_scissor(&self, device_context: &Dx11Context, bounds: Box2<ScreenSpace>) {
        let bounds = windows::Win32::Foundation::RECT {
            left: bounds.min.x as i32,
            top: bounds.min.y as i32,
            right: bounds.max.x as i32,
            bottom: bounds.max.y as i32,
        };
        unsafe {
            device_context.RSSetScissorRects(Some(&[bounds]));
        }
    }

    pub fn setup_fill(&self, device_context: &Dx11Context) {
        self.depth_stencil_state_mask.set(device_context);
        unsafe {
            device_context.PSSetShader(None, None);
        }
    }

    pub fn fill_clipped(&self, device_context: &Dx11Context) {
        self.fill_quad.set(device_context, 0);

        unsafe {
            device_context.Draw(Self::FILL_QUAD_VERTEX_COUNT, 0);
        }
    }

    pub fn fill_corners(&self, device_context: &Dx11Context, depth: bool) {
        let geometry = match &self.fill_edge {
            Some(fill) => fill,
            None => return,
        };
        let state = match depth {
            #[cfg(feature = "goggles")]
            true => &self.depth_stencil_state_write,
            _ => &self.depth_stencil_state_mask,
        };
        state.set(device_context);
        geometry.set(device_context, 0);

        unsafe {
            //device_context.DrawInstanced(Self::FILL_QUAD_VERTEX_COUNT, Self::FILL_QUAD_EDGE_COUNT, Self::FILL_QUAD_VERTEX_COUNT * 1, 0);
            for i in 1..=Self::FILL_QUAD_EDGE_COUNT {
                device_context.Draw(
                    Self::FILL_QUAD_VERTEX_COUNT,
                    Self::FILL_QUAD_VERTEX_COUNT * i,
                );
            }
        }
    }

    pub fn regen_edge(
        &mut self,
        device: &Dx11Device,
        edge_scale: Option<(f32, &MapCalibration)>,
    ) -> anyhow::Result<()> {
        self.fill_edge = edge_scale
            .map(|scale| Self::new_fill_quad(device, Some(scale)))
            .transpose()?;
        Ok(())
    }

    const FILL_QUAD_VERTEX_COUNT: u32 = 4;
    const FILL_QUAD_EDGE_COUNT: u32 = 4;
    /// TODO: hack .-.
    pub fn new_fill_quad(
        device: &Dx11Device,
        edge_scale: Option<(f32, &MapCalibration)>,
    ) -> anyhow::Result<VertexBuffer> {
        let mut verts: Vec<_> =
            crate::space::pack::poi::PoiCommonRenderData::quad(crate::space::LocalContext::GLOBAL)
                .into();

        if let Some((edge_scale, calibration)) = edge_scale {
            use {
                crate::resources::Vertex,
                glam::{vec2, Vec3},
                glamour::Box2,
                taimi_meta::ui::MinimapPlacement,
            };

            fn quad_verts(b: Box2) -> [Vertex; 4] {
                [
                    Vertex {
                        position: Vec3::new(b.min.x, 0.0, b.max.y),
                        colour: Vec3::ONE,
                        normal: Vec3::Y,
                        ..Default::default()
                    },
                    Vertex {
                        position: Vec3::new(b.max.x, 0.0, b.max.y),
                        colour: Vec3::ONE,
                        normal: Vec3::Y,
                        ..Default::default()
                    },
                    Vertex {
                        position: Vec3::new(b.min.x, 0.0, b.min.y),
                        colour: Vec3::ONE,
                        normal: Vec3::Y,
                        ..Default::default()
                    },
                    Vertex {
                        position: Vec3::new(b.max.x, 0.0, b.min.y),
                        colour: Vec3::ONE,
                        normal: Vec3::Y,
                        ..Default::default()
                    },
                ]
            }

            //let bottom_h = 0.15 * edge_scale;
            // just enough for the xp bar...
            let bottom_h = 0.0175 * edge_scale;
            let bottom = Box2::new(vec2(-1.0, -1.0).into(), vec2(1.0, -1.0 + bottom_h).into());
            verts.extend_from_slice(&quad_verts(bottom));

            // use feather for this instead...
            /*
            let top_h = 0.045 * edge_scale;
            let top = Box2::new(
                vec2(-1.0, 1.0).into(),
                vec2(1.0, 1.0 - top_h).into(),
            );
            verts.extend_from_slice(&quad_verts(top));*/

            let left_w = 0.18 * edge_scale;
            let tl_h = 0.485 * edge_scale;
            let top_left = Box2::new(
                vec2(-1.0, 1.0 - tl_h).into(),
                vec2(-1.0 + left_w, 1.0).into(),
            );
            verts.extend_from_slice(&quad_verts(top_left));
            // b-l
            let bleft_w = 0.3 * edge_scale;
            let bl_h = 0.5 * edge_scale;
            let bottom_left = Box2::new(
                vec2(-1.0, -1.0).into(),
                vec2(-1.0 + bleft_w, -1.0 + bl_h).into(),
            );
            verts.extend_from_slice(&quad_verts(bottom_left));

            let sign = match calibration.compass_position {
                MinimapPlacement::Top => -1.0f32,
                MinimapPlacement::Bottom => 1.0,
            };
            let corn_w = 0.2 * edge_scale * sign;
            let corn_h = 0.4 * edge_scale * sign;
            let right = Box2::from_points([
                vec2(1.0 - corn_w, sign * (1.0 - corn_h)).into(),
                vec2(1.0, sign).into(),
            ]);
            verts.extend_from_slice(&quad_verts(right));
        }

        Model::from_vertices(verts).to_buffer(device)
    }
}

#[cfg(feature = "goggles")]
static DEPTH_STENCIL_VIEW_VTABLE: std::sync::OnceLock<&'static d3d11::ID3D11DepthStencilView_Vtbl> =
    std::sync::OnceLock::new();
