use {
    crate::space::dx11::prelude::*,
    serde::{Deserialize, Serialize},
    windows::Win32::Graphics::Direct3D::{
        self as d3d,
        D3D_PRIMITIVE_TOPOLOGY,
    },
};

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[repr(u32)]
pub enum PrimitiveTopology {
    Undefined = d3d::D3D11_PRIMITIVE_TOPOLOGY_UNDEFINED.0 as u32,
    PointList = d3d::D3D11_PRIMITIVE_TOPOLOGY_POINTLIST.0 as u32,
    LineList = d3d::D3D11_PRIMITIVE_TOPOLOGY_LINELIST.0 as u32,
    LineStrip = d3d::D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP.0 as u32,
    #[default]
    TriangleList = d3d::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST.0 as u32,
    TriangleStrip = d3d::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP.0 as u32,
    LineListAdj = d3d::D3D11_PRIMITIVE_TOPOLOGY_LINELIST_ADJ.0 as u32,
    LineStripAdj = d3d::D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ.0 as u32,
    TriangleListAdj = d3d::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ.0 as u32,
    TriangleStripAdj = d3d::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ.0 as u32,
}

impl PrimitiveTopology {
    pub fn d3d(&self) -> D3D_PRIMITIVE_TOPOLOGY {
        let repr = *self as u32;
        D3D_PRIMITIVE_TOPOLOGY(repr as _)
    }
}

impl D3d11ContextBindable for PrimitiveTopology {
    fn set(&self, device_context: &ID3D11DeviceContext) {
        unsafe {
            device_context.IASetPrimitiveTopology(self.d3d())
        }
    }
}
