use {
    crate::{prelude::*, state::D3dState, D3dContextBindable},
    core::mem,
};

pub use crate::d3d::D3D_PRIMITIVE_TOPOLOGY;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum PrimitiveTopology {
    Undefined = Self::UNDEFINED,
    PointList = d3d::D3D11_PRIMITIVE_TOPOLOGY_POINTLIST.0 as u32,
    LineList = d3d::D3D11_PRIMITIVE_TOPOLOGY_LINELIST.0 as u32,
    LineStrip = d3d::D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP.0 as u32,
    #[default]
    TriangleList = d3d::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST.0 as u32,
    TriangleStrip = Self::TRIANGLESTRIP,
    LineListAdj = Self::LINELIST_ADJ,
    LineStripAdj = d3d::D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ.0 as u32,
    TriangleListAdj = d3d::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ.0 as u32,
    TriangleStripAdj = Self::TRIANGLESTRIP_ADJ,
    /// 1 to 32...
    #[cfg(todo)]
    D3D11_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST = Self::PATCHLIST_1,
}

impl PrimitiveTopology {
    pub fn d3d(&self) -> D3D_PRIMITIVE_TOPOLOGY {
        D3D_PRIMITIVE_TOPOLOGY(self.repr() as _)
    }
    pub fn from_d3d(repr: D3D_PRIMITIVE_TOPOLOGY) -> Self {
        Self::try_from_d3d(repr).unwrap_or(Self::Undefined)
    }
    pub unsafe fn from_d3d_unchecked(repr: D3D_PRIMITIVE_TOPOLOGY) -> Self {
        Self::from_repr_unchecked(repr.0 as _)
    }
    pub fn try_from_d3d(repr: D3D_PRIMITIVE_TOPOLOGY) -> Option<Self> {
        match repr.0 as u32 {
            | Self::UNDEFINED..=Self::TRIANGLESTRIP | Self::LINELIST_ADJ..=Self::TRIANGLESTRIP_ADJ =>
                Some(unsafe { Self::from_d3d_unchecked(repr) }),
            Self::PATCHLIST_1..=Self::PATCHLIST_32 => {
                log::error!("TODO: CONTROL_POINT_PATCHLIST");
                None
            },
            _ => None,
        }
    }
    pub fn repr(self) -> u32 {
        self as _
    }
    pub unsafe fn from_repr_unchecked(repr: u32) -> Self {
        mem::transmute(repr)
    }
}

/// TODO
impl PrimitiveTopology {
    pub const UNDEFINED: u32 = d3d::D3D11_PRIMITIVE_TOPOLOGY_UNDEFINED.0 as _;
    pub const TRIANGLESTRIP: u32 = d3d::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP.0 as _;
    pub const LINELIST_ADJ: u32 = d3d::D3D11_PRIMITIVE_TOPOLOGY_LINELIST_ADJ.0 as _;
    pub const TRIANGLESTRIP_ADJ: u32 = d3d::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ.0 as _;
    pub const PATCHLIST_1: u32 = d3d::D3D11_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST.0 as _;
    pub const PATCHLIST_32: u32 = d3d::D3D11_PRIMITIVE_TOPOLOGY_32_CONTROL_POINT_PATCHLIST.0 as _;
}
impl<D3DC: D3dContext> D3dState<D3DC> for PrimitiveTopology
where
    Self: D3dContextBindable<D3DC>,
{
    #[inline]
    fn restore_state(&self, device_context: &D3DC) {
        self.set(device_context);
    }
}
