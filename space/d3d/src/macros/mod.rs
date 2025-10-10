#[cfg(feature = "serde")]
pub mod serde;

#[macro_export]
macro_rules! impl_d3d {
    (@[transparent($int:tt <= $ty:ty)]
        $(#[$meta:meta])*
        $vis:vis struct $newty:ident.$field:ident
    $($rest:tt)*) => {
        $crate::macros::impl_d3d! {
            @[transparent($int <= $ty)]
            $(#[$meta])*
            $vis struct $newty {
                pub $field: $ty,
            }
            $($rest)*
        }
    };
    (unsafe impl AsD3d<Interface=$out:ty,@transparent> for $ty:ty
    ; $($($rest:tt)+)?) => {
        impl $ty {
            #[inline]
            pub const fn from_d3d(v: $out) -> $ty {
                unsafe {
                    ::core::mem::transmute(v)
                }
            }
            #[inline]
            pub const fn from_d3d_ref(v: &$out) -> &$ty {
                unsafe {
                    ::core::mem::transmute(v)
                }
            }
            #[inline]
            pub fn from_d3d_mut(v: &mut $out) -> &mut $ty {
                unsafe {
                    ::core::mem::transmute(v)
                }
            }

            #[inline]
            pub const fn from_ref_opt(v: &Option<$out>) -> &Option<$ty> {
                unsafe {
                    ::core::mem::transmute(v)
                }
            }

            #[inline]
            pub const fn into_ref_opt(v: &Option<$ty>) -> &Option<$out> {
                unsafe {
                    ::core::mem::transmute(v)
                }
            }

            #[inline]
            pub const fn as_d3d(&self) -> &$out {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }
            #[inline]
            pub const fn as_d3d_mut(&mut self) -> &mut $out {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }
            #[inline]
            pub const fn into_d3d(self) -> $out {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }
        }

        impl<'a> From<&'a $out> for &'a $ty {
            #[inline]
            fn from(v: &'a $out) -> Self {
                <$ty>::from_d3d_ref(v)
            }
        }
        impl From<$out> for $ty {
            #[inline]
            fn from(v: $out) -> Self {
                <$ty>::from_d3d(v)
            }
        }
        impl From<$ty> for $out {
            #[inline]
            fn from(v: $ty) -> Self {
                v.into_d3d()
            }
        }
        impl<'a> Into<&'a $out> for &'a $ty {
            #[inline]
            fn into(self) -> &'a $out {
                self.as_d3d()
            }
        }

        impl AsRef<$ty> for $ty {
            #[inline]
            fn as_ref(&self) -> &$ty {
                self
            }
        }
        impl AsRef<Option<$ty>> for $ty {
            #[inline]
            fn as_ref(&self) -> &Option<$ty> {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }
        }
        impl AsRef<$ty> for $out {
            #[inline]
            fn as_ref(&self) -> &$ty {
                <$ty>::from_d3d_ref(self)
            }
        }
        impl AsRef<$out> for $ty {
            #[inline]
            fn as_ref(&self) -> &$out {
                self.as_d3d()
            }
        }
        impl AsMut<$ty> for $ty {
            #[inline]
            fn as_mut(&mut self) -> &mut $ty {
                self
            }
        }
        impl AsMut<$out> for $ty {
            #[inline]
            fn as_mut(&mut self) -> &mut $out {
                self.as_d3d_mut()
            }
        }
        impl AsMut<$ty> for $out {
            #[inline]
            fn as_mut(&mut self) -> &mut $ty {
                <$ty>::from_d3d_mut(self)
            }
        }
    };
    (unsafe impl D3dInterfacePtr<Interface=$out:ty,@transparent> for $ty:ty
    ; $($rest:tt)*) => {
        $crate::macros::impl_d3d! {
            unsafe impl AsD3d<Interface=$out,@transparent> for $ty
            ; $($rest)*
        }
        impl $ty {
            #[inline]
            pub fn to_ref(&self) -> ::windows::core::InterfaceRef<'_, $out> {
                ::windows::core::Interface::to_ref(self.as_d3d())
            }
        }

        unsafe impl $crate::D3dInterfacePtr for $ty {
            type Interface = $out;

            #[inline]
            fn as_d3d_param(&self) -> &Option<$out> {
                let this: &Option<$ty> = unsafe {
                    ::core::mem::transmute(self)
                };
                <$ty>::into_ref_opt(this)
            }

            #[inline]
            fn into_d3d_param(self) -> Option<$out> {
                Some(self.into_d3d())
            }

            #[inline]
            fn from_d3d_param(param: &$out) -> &$ty {
                <$ty>::from_d3d_ref(param)
            }
        }
        unsafe impl $crate::D3dInterfacePtr for Option<$ty> {
            type Interface = $out;

            #[inline]
            fn as_d3d_param(&self) -> &Option<$out> {
                <$ty>::into_ref_opt(self)
            }

            #[inline]
            fn into_d3d_param(self) -> Option<$out> {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            fn from_d3d_param(param: &$out) -> &Option<$ty> {
                let param: &Option<$out> = unsafe {
                    ::core::mem::transmute(param)
                };
                <$ty>::from_ref_opt(param)
            }
        }

        impl ::windows::core::Param<$out> for &'_ $ty {
            unsafe fn param(self) -> windows::core::ParamValue<$out> {
                windows::core::Param::<$out>::param(self.as_d3d())
            }
        }

        impl AsRef<$ty> for ::windows::core::InterfaceRef<'_, $out> {
            #[inline]
            fn as_ref(&self) -> &$ty {
                <$ty>::from_d3d_ref(&**self)
            }
        }
    };
    (unsafe impl Dx11Child<Interface=$out:ty, @transparent> for $ty:ty,
        @field(&$this:ident => &$field:expr)
        ; $($rest:tt)*
    ) => {
        $crate::macros::impl_d3d! {
            unsafe impl Dx11Child<Interface=$out> for $ty,
                @field(&$this => &$field);
            unsafe impl D3dInterfacePtr<Interface=$out, @transparent> for $ty;

            $($rest)*
        }

        impl $crate::dx11::ID3D11ResourceExt for Option<$ty> {
            type Output = $out;

            #[inline]
            fn as_params(&self) -> &[Option<$out>] {
                let v = <$ty>::into_ref_opt(self);
                $crate::dx11::ID3D11ResourceExt::as_params(v)
            }
        }
    };
    (unsafe impl Dx11Child<Interface=$out:ty, @transparent> for $ty:ty
    ; $($rest:tt)*) => {
        $crate::macros::impl_d3d! {
            unsafe impl D3dInterfacePtr<Interface=$out, @transparent> for $ty;

            $($rest)*
        }

        impl $crate::dx11::ID3D11ResourceExt for $ty {
            type Output = $out;

            #[inline]
            fn as_params(&self) -> &[Option<$out>] {
                $crate::dx11::ID3D11ResourceExt::as_params(::core::convert::Into::<&$out>::into(self))
            }
        }

        impl $crate::dx11::ID3D11ResourceExt for Option<$ty> {
            type Output = $out;

            fn as_params(&self) -> &[Option<$out>] {
                let v = <$ty>::into_ref_opt(self);
                $crate::dx11::ID3D11ResourceExt::as_params(v)
            }
        }
    };
    (unsafe impl Dx11Child<Interface=$out:ty> for $ty:ty
        , @field(&$this:ident => $field:expr)
    ; $($($rest:tt)+)?) => {
        impl $crate::dx11::ID3D11ResourceExt for $ty {
            type Output = $out;

            fn as_params(&self) -> &[Option<$out>] {
                let $this = self;
                $crate::dx11::ID3D11ResourceExt::as_params($field)
            }
        }
        $(
            $crate::macros::impl_d3d! {
                $($rest)*
            }
        )?
    };
    (unsafe impl Dx11Child for $($ty:ty),+$(,)?
    ; $($rest:tt)*) => {
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
            impl $crate::dx11::ID3D11ResourceExt for ::windows::core::InterfaceRef<'_, $ty> {
                type Output = $ty;

                fn as_params(&self) -> &[Option<$ty>] {
                    let single: &$ty = &**self;
                    single.as_params()
                }
            }

            impl $crate::dx11::ID3D11ResourceExt for Option<::windows::core::InterfaceRef<'_, $ty>> {
                type Output = $ty;

                fn as_params(&self) -> &[Option<$ty>] {
                    let this: &Option<$ty> = unsafe {
                        mem::transmute(self)
                    };
                    this.as_params()
                }
            }
        )+
        $crate::macros::impl_d3d! {
            unsafe impl D3dInterfacePtr for $($ty),+;
            $($rest)*
        }
    };
    (@[transparent($int:tt <= $ty:ty)]
        $(#[$meta:meta])*
        $vis:vis struct $newty:ident {
            pub $field:ident: $field_ty:ty,
        }
        $(@from($($from:tt)*))?
        $(@into($($into:tt)*))?
        $(@deref($deref_target:ty))?
    ; $($($rest:tt)+)?) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        $(#[$meta])*
        #[repr(transparent)]
        $vis struct $newty {
            pub $field: $field_ty,
        }
        $crate::macros::impl_d3d! {
            unsafe impl $int<Interface=$ty,@transparent>
                for $newty;
            $($($rest)*)?
        }
        $(
            impl ::core::ops::Deref for $newty {
                type Target = $deref_target;
                fn deref(&self) -> &Self::Target {
                    &self.$field
                }
            }
            impl ::core::ops::DerefMut for $newty {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.$field
                }
            }
        )?
        $(
            impl ::core::convert::From<$field_ty $($from)*> for $newty {
                #[inline]
                fn from($field: $field_ty) -> Self {
                    Self { $field }
                }
            }
            impl ::core::convert::AsRef<$newty $($from)*> for $field_ty {
                #[inline]
                fn as_ref(&self) -> &$newty {
                    unsafe {
                        ::core::mem::transmute(self)
                    }
                }
            }
            impl<'a> ::core::convert::From<&'a $field_ty $($from)*> for &'a $newty {
                #[inline]
                fn from(v: &'a $field_ty) -> Self {
                    v.as_ref()
                }
            }
            impl<'a> ::core::convert::From<&'a mut $field_ty $($from)*> for &'a mut $newty {
                #[inline]
                fn from(v: &'a mut $field_ty) -> Self {
                    v.as_mut()
                }
            }
            impl ::core::convert::AsMut<$newty $($from)*> for $field_ty {
                fn as_mut(&mut self) -> &mut $newty {
                    unsafe {
                        ::core::mem::transmute(self)
                    }
                }
            }
            impl<'a> ::core::convert::From<&'a mut $newty $($from)*> for &'a mut $field_ty {
                fn from(v: &'a mut $newty) -> Self {
                    &mut v.$field
                }
            }
            impl ::core::convert::AsMut<$field_ty $($from)*> for $newty {
                fn as_mut(&mut self) -> &mut $field_ty {
                    &mut self.$field
                }
            }
        )?
        $(
            impl ::core::convert::From<$newty $($into)*> for $field_ty {
                #[inline]
                fn from(v: $newty) -> Self {
                    v.$field
                }
            }
            impl ::core::convert::AsRef<$field_ty $($into)*> for $newty {
                #[inline]
                fn as_ref(&self) -> &$field_ty {
                    &self.$field
                }
            }
            impl<'a> ::core::convert::From<&'a $newty $($into)*> for &'a $field_ty {
                #[inline]
                fn from(v: &'a $newty) -> Self {
                    &v.$field
                }
            }
        )?
    };
    (unsafe impl D3dInterfacePtr
        for $($ty:ty),+$(,)?
    ; $($($rest:tt)+)?) => {
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
            unsafe impl $crate::D3dInterfacePtr for ::windows::core::InterfaceRef<'_, $ty> {
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
            unsafe impl $crate::D3dInterfacePtr for Option<::windows::core::InterfaceRef<'_, $ty>> {
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
            $crate::macros::impl_d3d! {
                $($rest)*
            }
        )?
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
                #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
                $(#[$meta])*
                $vis struct $flags: $repr {
                    $(
                        $(#[$field_meta])*
                        const $field = $field_bits as $repr;
                    )*
                }
            }

            #[cfg(feature = "serde")]
            impl serde::Serialize for $flags {
                fn serialize<S: serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                    self.bits().serialize(serializer)
                }
            }
            #[cfg(feature = "serde")]
            impl<'de> serde::Deserialize<'de> for $flags {
                fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> ::core::result::Result<Self, D::Error> {
                    <$repr as serde::Deserialize<'de>>::deserialize(deserializer)
                        .map(Self::from_bits_retain)
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
            $crate::macros::impl_d3d! {
                $($rest)*
            }
        )?
    };
    (impl enum for
        $(#[$meta:meta])*
        $vis:vis enum $name:ident: $ty:ident{$repr:ty} {
            with $path:path =>
            $(
                $(#[$field_meta:meta])*
                $variant:ident(const $field:ident) = $field_value_name:ident,
            )*
        }
    $($rest:tt)*) => {
        crate::macros::impl_d3d! { impl enum for
            $(#[$meta])*
            $vis enum $name: $path::$ty{$repr} {
                @![doc(alias = ::core::stringify! { $ty })]
                $(
                    @[doc(alias = ::core::stringify! { $field_value_name })]
                    $(#[$field_meta:meta])*
                    $variant(const $field) = $path::$field_value_name,
                )*
            } $($rest)*
        }
    };
    (impl enum for) => {};
    (impl enum for , $($($rest:tt)+)?) => {
        $($crate::macros::impl_d3d! { impl enum for $($rest)+ })?
    };
    (impl enum for ; $($($rest:tt)+)?) => {
        $($crate::macros::impl_d3d! {
            $($rest)+
        })?
    };
    (impl enum for
        $(#[$meta:meta])*
        $vis:vis enum $name:ident: $ty:path{$repr:ty} {
            $(
                $(#[$field_meta:meta])*
                const $field:ident = $field_value_name:path;
            )*
        }
    $($rest:tt)*) => {
        $crate::macros::impl_d3d! { impl enum for
            $(#[$meta])*
            $vis enum $name: $ty{$repr} {
                @![doc(alias = $ty)]
                $(
                    @[doc(alias = $field_value_name)]
                    $(#[$field_meta])*
                    $field = $field_value_name,
                )*
            } $($rest)*
        }
    };
    (impl enum for
        $(#[$meta:meta])*
        $vis:vis enum $name:ident: $ty:path{$repr:ty} {
            $(@![doc(alias = $($doc_alias_ty:tt)+)])?
            $(
                // TODO: non-uppercase idents
                $(@[doc(alias = $($doc_alias:tt)*)])?
                $(#[$field_meta:meta])*
                $variant:ident$((const $field:ident))? = $field_value_name:path,
            )*
        }
    $($($rest:tt)+)?) => {
        // TODO: manual serde impl that supports (prefers?) repr values too.. serde(variant_identifier) would almost be there?
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        // TODO: $(#[doc(alias = $($doc_alias_ty)*)])?
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $(#[$meta])*
        #[repr($repr)]
        #[allow(non_camel_case_types)]
        $vis enum $name {
            $(
                // TODO: $(#[doc(alias = $($doc_alias)*)])?
                $(#[$field_meta])*
                $variant = $field_value_name.0 as $repr,
            )*
        }

        impl $name {
            $(
                // $variant
                $($vis const $field: $ty = $field_value_name;)?
            )*

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

        $($crate::macros::impl_d3d! {
            impl enum for $($rest)*
        })?
    };
}
pub use impl_d3d;
