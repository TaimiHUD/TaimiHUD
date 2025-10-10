#[macro_export]
macro_rules! impl_d3d {
    (@[transparent(Interface <= $ty:ty)]
        $(#[$meta:meta])*
        $vis:vis struct $newty:ident.$field:ident
    $($rest:tt)*) => {
        $crate::impl_d3d! {
            @[transparent(Interface <= $ty)]
            $(#[$meta])*
            $vis struct $newty {
                pub $field: $ty,
            }
            $($rest)*
        }
    };
    (unsafe impl D3dInterfacePtr<Interface=$out:ty,@transparent> for $ty:ty
        ; $($($rest:tt)+)?
    ) => {
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
        impl AsRef<$ty> for ::windows::core::InterfaceRef<'_, $out> {
            #[inline]
            fn as_ref(&self) -> &$ty {
                <$ty>::from_d3d_ref(&**self)
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
        $(
            $crate::impl_d3d! {
                $($rest)*
            }
        )?
    };
    (@[transparent(Interface <= $ty:ty)]
        $(#[$meta:meta])*
        $vis:vis struct $newty:ident {
            pub $field:ident: $field_ty:ty,
        }
        $(@deref($deref_target:ty))?
    ; $($($rest:tt)+)?) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        $(#[$meta])*
        #[repr(transparent)]
        $vis struct $newty {
            pub $field: $field_ty,
        }
        $crate::impl_d3d! {
            unsafe impl D3dInterfacePtr<Interface=$ty,@transparent>
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
    };
}
