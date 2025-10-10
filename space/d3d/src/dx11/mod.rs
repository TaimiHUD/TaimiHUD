pub mod blend;
pub mod buffer;
pub mod depth;
pub mod raster;
pub mod shader;
pub mod viewport;

pub mod prelude {
    pub use {
        crate::prelude::*,
        super::{
            Dx11Context, Dx11ContextRef, Dx11Device,
            Dx11Buffer, Dx11Resource,
            ID3D11ResourceOf, ID3D11ResourceExt as _,
            D3d11ContextBindable, D3d11ContextBindableSlot,
        },
        windows::Win32::Graphics::Direct3D11::{
            self as d3d11,
            ID3D11Buffer,
            ID3D11Device, ID3D11DeviceContext,
            ID3D11ShaderResourceView,
        },
    };
}

pub use {
    self::{
        buffer::{Buffer, Texture2, VertexBuffer},
        blend::{BlendState, OMBlendState},
        depth::{DepthState, DepthView, OMDepthState},
        raster::{RasterizerState, RenderTargetView, RenderTargetViews},
        shader::{ShaderV, ShaderP},
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
    crate::{
        state::PrimitiveTopology,
        D3dContext, D3dContextBindable, D3dContextBindableSlot,
        D3dDevice,
        D3dInterfacePtr,
    },
    std::mem,
    windows::{
        core::InterfaceRef,
        Win32::Graphics::Direct3D11::{
            ID3D11Buffer, ID3D11DeviceChild, ID3D11Resource, ID3D11View,
        },
    },
};

impl D3dContext for Dx11Context {
    type IDevice = Dx11Device;
}
impl D3dDevice for Dx11Device {
    type IBuffer = Dx11Buffer;
}

impl D3dContextBindable<Dx11Context> for PrimitiveTopology {
    fn set(&self, device_context: &Dx11Context) {
        unsafe {
            device_context.IASetPrimitiveTopology(self.d3d())
        }
    }
}

pub trait D3d11ContextBindable: D3dContextBindable<Dx11Context> {
}
impl<T: D3dContextBindable<Dx11Context>> D3d11ContextBindable for T {}

pub trait D3d11ContextBindableSlot: D3dContextBindableSlot<Dx11Context> {
}
impl<T: D3dContextBindableSlot<Dx11Context>> D3d11ContextBindableSlot for T {}

macro_rules! impl_d3d_ext11 {
    (unsafe impl D3dInterfacePtr<Interface=$out:ty,@transparent> for $ty:ty,
        @field(&$this:ident => &$field:expr)
        ; $($rest:tt)*
    ) => {
        $crate::impl_d3d! {
            unsafe impl D3dInterfacePtr<Interface=$out,@transparent> for $ty;
        }
    };
    (unsafe impl ID3D11ResourceExt<Output=$out:ty, @transparent> for $ty:ty,
        @field(&$this:ident => &$field:expr)
        ; $($rest:tt)*
    ) => {
        $crate::dx11::impl_d3d_ext11! {
            unsafe impl ID3D11ResourceExt<Output=$out> for $ty,
                @field(&$this => &$field);
            unsafe impl D3dInterfacePtr<Interface=$out, @transparent> for $ty,
                @field(&$this => &$field);

            $($rest)*
        }

        impl $crate::dx11::ID3D11ResourceExt for Option<$ty> {
            type Output = $out;

            fn as_params(&self) -> &[Option<$out>] {
                let v = <$ty>::into_ref_opt(self);
                $crate::dx11::ID3D11ResourceExt::as_params(v)
            }
        }
    };
    (unsafe impl ID3D11ResourceExt<Output=$out:ty> for $ty:ty, @field(&$this:ident => $field:expr); $($($rest:tt)+)?) => {
        impl $crate::dx11::ID3D11ResourceExt for $ty {
            type Output = $out;

            fn as_params(&self) -> &[Option<$out>] {
                let $this = self;
                $crate::dx11::ID3D11ResourceExt::as_params($field)
            }
        }
        $(
            $crate::dx11::impl_d3d_ext11! {
                $($rest)*
            }
        )?
    };
    (unsafe impl D3dInterfacePtr for $($ty:ty),+$(,)?; $($($rest:tt)+)?) => {
        $(
            unsafe impl $crate::D3dInterfacePtr for $ty {
                type Interface = $ty;

                #[inline]
                fn as_d3d_param(&self) -> &Option<$ty> {
                    unsafe {
                        ::core::mem::transmute(self)
                    }
                }

                #[inline]
                fn into_d3d_param(self) -> Option<$ty> {
                    Some(self)
                }

                #[inline]
                fn from_d3d_param(param: &$ty) -> &$ty {
                    param
                }
            }
            unsafe impl $crate::D3dInterfacePtr for Option<$ty> {
                type Interface = $ty;

                #[inline]
                fn as_d3d_param(&self) -> &Option<$ty> {
                    self
                }

                #[inline]
                fn into_d3d_param(self) -> Option<$ty> {
                    self
                }

                #[inline]
                fn from_d3d_param(param: &$ty) -> &Option<$ty> {
                    unsafe {
                        ::core::mem::transmute(param)
                    }
                }
            }
            unsafe impl $crate::D3dInterfacePtr for InterfaceRef<'_, $ty> {
                type Interface = $ty;

                #[inline]
                fn as_d3d_param(&self) -> &Option<$ty> {
                    <$ty as $crate::D3dInterfacePtr>::as_d3d_param(self)
                }

                #[inline]
                fn into_d3d_param(self) -> Option<$ty> {
                    Some(self.to_owned())
                }

                #[inline]
                fn from_d3d_param(param: &$ty) -> &Self {
                    unsafe {
                        ::core::mem::transmute(param)
                    }
                }
            }
            unsafe impl $crate::D3dInterfacePtr for Option<InterfaceRef<'_, $ty>> {
                type Interface = $ty;

                #[inline]
                fn as_d3d_param(&self) -> &Option<$ty> {
                    unsafe {
                        ::core::mem::transmute(self)
                    }
                }

                #[inline]
                fn into_d3d_param(self) -> Option<$ty> {
                    self.map(|param| param.to_owned())
                }

                #[inline]
                fn from_d3d_param(param: &$ty) -> &Self {
                    let opt: &Option<$ty> = $crate::D3dInterfacePtr::from_d3d_param(param);
                    unsafe {
                        ::core::mem::transmute(opt)
                    }
                }
            }
        )+
        $(
            $crate::dx11::impl_d3d_ext11! {
                $($rest)*
            }
        )?
    };
    (unsafe impl ID3D11ResourceExt for $($ty:ty),+$(,)?; $($rest:tt)*) => {
        $(
            impl $crate::dx11::ID3D11ResourceExt for $ty {
                type Output = Self;

                fn as_params(&self) -> &[Option<$ty>] {
                    let single: &[Self; 1] = ::core::array::from_ref(self);
                    let out: &[Option<$ty>; 1] = unsafe {
                        mem::transmute(single)
                    };
                    out
                }
            }
            impl $crate::dx11::ID3D11ResourceExt for Option<$ty> {
                type Output = $ty;

                fn as_params(&self) -> &[Option<$ty>] {
                    ::core::slice::from_ref(self)
                }
            }
            impl $crate::dx11::ID3D11ResourceExt for InterfaceRef<'_, $ty> {
                type Output = $ty;

                fn as_params(&self) -> &[Option<$ty>] {
                    let single: &$ty = &**self;
                    single.as_params()
                }
            }

            impl $crate::dx11::ID3D11ResourceExt for Option<InterfaceRef<'_, $ty>> {
                type Output = $ty;

                fn as_params(&self) -> &[Option<$ty>] {
                    let this: &Option<$ty> = unsafe {
                        mem::transmute(self)
                    };
                    this.as_params()
                }
            }
        )+
        $crate::dx11::impl_d3d_ext11! {
            unsafe impl D3dInterfacePtr for $($ty),+;
            $($rest)*
        }
    };
    (impl bitflags for $(
        $(#[$meta:meta])*
        $vis:vis struct $flags:ident: $ty:path{$repr:ty} {
            $(
                $(#[$field_meta:meta])*
                const $field:ident = $field_bits:expr;
            )*
        }
    ),*$(,)? $(; $($rest:tt)*)?
    ) => {
        // TODO: doc(alias) to real flag ident and doc-link back to it
        $(
            ::bitflags::bitflags! {
                $(#[$meta])*
                #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
                $vis struct $flags: $repr {
                    $(
                        $(#[$field_meta])*
                        const $field = $field_bits as $repr;
                    )*
                }
            }

            impl $flags {
                #[inline]
                pub const fn to_d3d(self) -> $ty {
                    $ty(self.to_raw() as _)
                }
                #[inline]
                pub const fn to_raw(self) -> $repr {
                    self.bits() as _
                }
                #[inline]
                pub const fn to_uint(self) -> u32 {
                    self.bits() as _
                }
                #[inline]
                pub const fn to_int(self) -> i32 {
                    self.bits() as _
                }
            }

            impl From<()> for $flags {
                #[inline]
                fn from(_empty: ()) -> Self {
                    Self::empty()
                }
            }
            impl From<$ty> for $flags {
                #[inline]
                fn from(flags: $ty) -> Self {
                    Self::from_bits_retain(flags.0 as _)
                }
            }
            impl From<$flags> for $ty {
                #[inline]
                fn from(flags: $flags) -> Self {
                    flags.to_d3d()
                }
            }
            impl From<$flags> for u32 {
                #[inline]
                fn from(flags: $flags) -> Self {
                    flags.to_uint()
                }
            }
            impl From<$flags> for i32 {
                #[inline]
                fn from(flags: $flags) -> Self {
                    flags.to_int()
                }
            }
        )*
        $(
            $crate::dx11::impl_d3d_ext11! {
                $($rest)*
            }
        )?
    };
    (impl enum for $(
        $(#[$meta:meta])*
        $vis:vis enum $name:ident: $ty:path{$repr:ty} {
            $(
                // TODO: non-uppercase idents
                $(#[$field_meta:meta])*
                const $field:ident = $field_value_name:path;
            )*
        }
    ),*$(,)? $(; $($rest:tt)*)?
    ) => {
        // TODO: doc(alias) to real flag ident and doc-link back to it
        $(
            #[allow(non_camel_case_types)]
            $(#[$meta])*
            #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
            #[repr($repr)]
            $vis enum $name {
                $(
                    $(#[$field_meta])*
                    $field = $field_value_name.0 as $repr,
                )*
            }

            impl $name {
                #[inline]
                pub const fn to_d3d(self) -> $ty {
                    $ty(self.to_raw() as _)
                }
                #[inline]
                pub const fn to_raw(self) -> $repr {
                    self as _
                }
                #[inline]
                pub const fn to_uint(self) -> u32 {
                    self as _
                }
                #[inline]
                pub const fn to_int(self) -> i32 {
                    self as _
                }

                #[inline]
                pub const unsafe fn from_raw_unchecked(value: $repr) -> Self {
                    ::core::mem::transmute(value)
                }
                #[inline]
                pub const unsafe fn from_d3d_unchecked(value: $ty) -> Self {
                    Self::from_raw_unchecked(value.0 as _)
                }

                pub fn try_from_d3d(value: $ty) -> anyhow::Result<Self> {
                    match value {
                        $(
                            $field_value_name
                        )|* => Ok(unsafe {
                            Self::from_d3d_unchecked(value)
                        }),
                        _ => Err(anyhow::anyhow!("unrecognized {} value: {}", stringify!($ty), value.0)),
                    }
                }

                pub fn try_from_raw(value: $repr) -> anyhow::Result<Self> {
                    Self::try_from_d3d($ty(value as _))
                }
            }

            impl TryFrom<$ty> for $name {
                type Error = anyhow::Error;

                fn try_from(value: $ty) -> Result<Self, Self::Error> {
                    Self::try_from_d3d(value)
                }
            }
            impl TryFrom<i32> for $name {
                type Error = anyhow::Error;

                fn try_from(value: i32) -> Result<Self, Self::Error> {
                    Self::try_from_raw(value as $repr)
                }
            }
            impl TryFrom<u32> for $name {
                type Error = anyhow::Error;

                fn try_from(value: u32) -> Result<Self, Self::Error> {
                    Self::try_from_raw(value as $repr)
                }
            }
            impl From<$name> for $ty {
                #[inline]
                fn from(value: $name) -> Self {
                    value.to_d3d()
                }
            }
            impl From<$name> for u32 {
                #[inline]
                fn from(value: $name) -> Self {
                    value.to_uint()
                }
            }
            impl From<$name> for i32 {
                #[inline]
                fn from(value: $name) -> Self {
                    value.to_int()
                }
            }
        )*
        $(
            $crate::dx11::impl_d3d_ext11! {
                $($rest)*
            }
        )?
    };
}
pub(crate) use impl_d3d_ext11;

pub trait ID3D11ResourceExt {
    type Output: windows::core::imp::CanInto<Dx11Child>;

    fn as_params(&self) -> &[Option<Self::Output>];
    fn as_param(&self) -> &Option<Self::Output> where Self: Sized {
        self.as_params().get(0)
            .unwrap_or(arcffi::nn::opt_ref_none::<_>())
    }
}
impl<T: ?Sized + ID3D11ResourceExt> ID3D11ResourceExt for &'_ T {
    type Output = T::Output;

    fn as_params(&self) -> &[Option<Self::Output>] { ID3D11ResourceExt::as_params(*self) }
}
impl<T: ID3D11ResourceExt> ID3D11ResourceExt for Option<&'_ T> {
    type Output = T::Output;

    fn as_params(&self) -> &[Option<Self::Output>] {
        self.map(ID3D11ResourceExt::as_params)
            .unwrap_or_default()
    }
}

pub trait ID3D11ResourceOf<O> {
    fn as_params_of(&self) -> &[Option<O>];
}

impl<T: ?Sized + ID3D11ResourceExt, O> ID3D11ResourceOf<O> for T where
    O: windows::core::Interface,
    //T: D3dInterfacePtr?,
    <T as ID3D11ResourceExt>::Output: windows::core::imp::CanInto<O>,
{
    fn as_params_of(&self) -> &[Option<O>] {
        debug_assert!(!<<T as ID3D11ResourceExt>::Output as windows::core::imp::CanInto<O>>::QUERY);

        let params: &[Option<<T as ID3D11ResourceExt>::Output>] = self.as_params();
        unsafe {
            mem::transmute(params)
        }
    }
}

impl<T> ID3D11ResourceExt for [T] where
    T: D3dInterfacePtr,
    //T: windows::core::TypeKind<TypeKind = windows::core::InterfaceType>,
    <T as D3dInterfacePtr>::Interface: windows::core::imp::CanInto<Dx11Child>,
    //<T as D3dInterfacePtr>::Interface: windows::core::Type<<T as D3dInterfacePtr>::Interface> + windows::core::TypeKind,
{
    type Output = <T as D3dInterfacePtr>::Interface;

    fn as_params(&self) -> &[Option<<Self as ID3D11ResourceExt>::Output>] {
        unsafe {
            mem::transmute(self)
        }
    }
}

impl<const N: usize, T> ID3D11ResourceExt for [T; N] where
    [T]: ID3D11ResourceExt,
{
    type Output = <[T] as ID3D11ResourceExt>::Output;

    fn as_params(&self) -> &[Option<<Self as ID3D11ResourceExt>::Output>] {
        self[..].as_params()
    }
}

impl_d3d_ext11! {
    unsafe impl ID3D11ResourceExt for
        Dx11Buffer, Dx11Resource, Dx11View,
        d3d11::ID3D11BlendState,
        d3d11::ID3D11DepthStencilView,
        d3d11::ID3D11DepthStencilState,
        d3d11::ID3D11InputLayout,
        d3d11::ID3D11PixelShader,
        d3d11::ID3D11RasterizerState,
        d3d11::ID3D11RenderTargetView,
        d3d11::ID3D11SamplerState,
        d3d11::ID3D11ShaderResourceView,
        d3d11::ID3D11Texture2D,
        d3d11::ID3D11VertexShader,
    ;
    unsafe impl D3dInterfacePtr for
        crate::dx::IDXGISwapChain,
        crate::d3d::ID3DInclude,
    ;
}
