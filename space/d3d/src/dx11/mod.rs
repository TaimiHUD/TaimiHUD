pub mod blend;
pub mod buffer;
pub mod context;
pub mod depth;
pub mod device;
pub mod raster;
pub mod shader;
pub mod viewport;
#[cfg(feature = "serde")]
#[doc(hidden)]
pub mod serde_imp {
    pub use super::shader::serde_imp::{input_classification, input_layout_element};
}

pub mod prelude {
    pub use {
        super::{
            D3d11ContextBindable,
            D3d11ContextBindableSlot,
            Dx11Buffer,
            Dx11Context,
            Dx11ContextRef,
            Dx11Device,
            Dx11Resource,
            ID3D11ResourceExt as _,
            ID3D11ResourceOf,
        },
        crate::prelude::*,
        windows::Win32::Graphics::Direct3D11::{
            self as d3d11,
            ID3D11Buffer,
            ID3D11Device,
            ID3D11DeviceContext,
            ID3D11ShaderResourceView,
        },
    };
}

pub use {
    self::{
        blend::{BlendState, OMBlendState},
        buffer::{Buffer, Resource, Texture2, VertexBuffer, View},
        depth::{DepthState, DepthView, OMDepthState},
        raster::{RasterizerState, RenderTargetView, RenderTargetViews},
        shader::{ShaderP, ShaderV},
        viewport::Viewport,
    },
    windows::Win32::Graphics::Direct3D11 as d3d11,
};

pub type Dx11Device = d3d11::ID3D11Device;
pub type Dx11Context = d3d11::ID3D11DeviceContext;
pub type Dx11ContextRef<'a> = InterfaceRef<'a, Dx11Context>;
pub type Dx11Buffer = ID3D11Buffer;
pub type Dx11View = ID3D11View;
pub type Dx11Resource = ID3D11Resource;
pub type Dx11Child = ID3D11DeviceChild;

use {
    crate::{D3dContextBindable, D3dContextBindableSlot, D3dInterfacePtr},
    std::mem,
    windows::{
        core::InterfaceRef,
        Win32::Graphics::Direct3D11::{ID3D11Buffer, ID3D11DeviceChild, ID3D11Resource, ID3D11View},
    },
};

pub trait D3d11ContextBindable: D3dContextBindable<Dx11Context> {}
impl<T: D3dContextBindable<Dx11Context>> D3d11ContextBindable for T {}

pub trait D3d11ContextBindableSlot: D3dContextBindableSlot<Dx11Context> {}
impl<T: D3dContextBindableSlot<Dx11Context>> D3d11ContextBindableSlot for T {}

pub trait ID3D11ResourceExt {
    type Output: windows::core::imp::CanInto<Dx11Child>;

    fn as_params(&self) -> &[Option<Self::Output>];
    fn as_param(&self) -> &Option<Self::Output>
    where
        Self: Sized,
    {
        match self.as_params().get(0) {
            Some(p) => p,
            #[cfg(todo = "unnecessary")]
            #[cfg(feature = "arcffi")]
            None => arcffi::nn::opt_ref_none::<_>(),
            #[cfg(todo = "unnecessary")]
            #[cfg(not(feature = "arcffi"))]
            None => unimplemented!("empty params require arcffi"),
            None => &None,
        }
    }
}
impl<T: ?Sized + ID3D11ResourceExt> ID3D11ResourceExt for &'_ T {
    type Output = T::Output;

    fn as_params(&self) -> &[Option<Self::Output>] {
        ID3D11ResourceExt::as_params(*self)
    }
}
impl<T: ID3D11ResourceExt> ID3D11ResourceExt for Option<&'_ T> {
    type Output = T::Output;

    fn as_params(&self) -> &[Option<Self::Output>] {
        self.map(ID3D11ResourceExt::as_params).unwrap_or_default()
    }
}

pub trait ID3D11ResourceOf<O> {
    fn as_params_of(&self) -> &[Option<O>];
}

impl<T: ?Sized + ID3D11ResourceExt, O> ID3D11ResourceOf<O> for T
where
    O: windows::core::Interface,
    //T: D3dInterfacePtr?,
    <T as ID3D11ResourceExt>::Output: windows::core::imp::CanInto<O>,
{
    fn as_params_of(&self) -> &[Option<O>] {
        debug_assert!(!<<T as ID3D11ResourceExt>::Output as windows::core::imp::CanInto<O>>::QUERY);

        let params: &[Option<<T as ID3D11ResourceExt>::Output>] = self.as_params();
        unsafe { mem::transmute(params) }
    }
}

impl<T> ID3D11ResourceExt for [T]
where
    T: D3dInterfacePtr,
    //T: windows::core::TypeKind<TypeKind = windows::core::InterfaceType>,
    <T as D3dInterfacePtr>::Interface: windows::core::imp::CanInto<Dx11Child>,
    //<T as D3dInterfacePtr>::Interface: windows::core::Type<<T as D3dInterfacePtr>::Interface> + windows::core::TypeKind,
{
    type Output = <T as D3dInterfacePtr>::Interface;

    fn as_params(&self) -> &[Option<<Self as ID3D11ResourceExt>::Output>] {
        unsafe { mem::transmute(self) }
    }
}

impl<const N: usize, T> ID3D11ResourceExt for [T; N]
where
    [T]: ID3D11ResourceExt,
{
    type Output = <[T] as ID3D11ResourceExt>::Output;

    fn as_params(&self) -> &[Option<<Self as ID3D11ResourceExt>::Output>] {
        self[..].as_params()
    }
}
