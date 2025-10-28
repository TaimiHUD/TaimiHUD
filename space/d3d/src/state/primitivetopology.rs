pub use crate::d3d::D3D_PRIMITIVE_TOPOLOGY;
use crate::prelude::*;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
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
