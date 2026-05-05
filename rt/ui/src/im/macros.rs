macro_rules! imvec_newtype {
    (impl{$($imp:tt)*} TransformMap<$src:ty, Output == $glamty:ident<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl<$($imp)*> ::glamour::TransformMap<::glamour::$glamty<$src>> for $ty {
            type Output = ::glamour::$glamty<$dst>;
            $(#[$meta_map])*
            fn map(&$this, $v: ::glamour::$glamty<$src>) -> Self::Output {
                $to_v
            }
        }
        $(crate::im::macros::imvec_newtype! { $($rest)+ } )?
    };
    (impl{$($imp:tt)*} TransformMap<$src:ty, Output = Vec2<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        crate::im::macros::imvec_newtype! {
            impl{$($imp)*} TransformMap<$src, Output = Vector2<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                }
            }
            impl{$($imp)*} TransformMap<$src, Output = Point2<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                    //::glamour::TransformMap::map($this, $v.to_vector()).to_point()
                }
            }
        }
        $(crate::im::macros::imvec_newtype! { $($rest)+ } )?
    };
    (impl{$($imp:tt)*} TransformMap<$src:ty, Output = Vector2<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl<$($imp)*> ::glamour::TransformMap<::glamour::Vector2<$src>> for $ty {
            type Output = ::glamour::Vector2<$dst>;
            $(#[$meta_map])*
            fn map(&$this, $v: ::glamour::Vector2<$src>) -> Self::Output {
                $to_v
            }
        }
        impl<$($imp)*> ::glamour::TransformMap<::glamour::Size2<$src>> for $ty {
            type Output = ::glamour::Size2<$dst>;
            #[inline(always)]
            fn map(&$this, $v: ::glamour::Size2<$src>) -> Self::Output {
                ::glamour::TransformMap::map($this, $v.to_vector()).to_size()
            }
        }
        $(crate::im::macros::imvec_newtype! { $($rest)+ } )?
    };
    (impl{$($imp:tt)*} TransformMap<$src:ty, Output = Point2<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl<$($imp)*> ::glamour::TransformMap<::glamour::Point2<$src>> for $ty {
            type Output = ::glamour::Point2<$dst>;
            #[inline(always)]
            fn map(&$this, $v: ::glamour::Point2<$src>) -> Self::Output {
                $to_v
            }
        }
        impl<$($imp)*> ::glamour::TransformMap<::glamour::Box2<$src>> for $ty {
            type Output = ::glamour::Box2<$dst>;
            #[inline]
            fn map(&$this, $v: ::glamour::Box2<$src>) -> Self::Output {
                ::glamour::Box2::new(
                    ::glamour::TransformMap::map($this, $v.min),
                    ::glamour::TransformMap::map($this, $v.max),
                )
            }
        }
        impl<$($imp)*> ::glamour::TransformMap<::glamour::Rect<$src>> for $ty {
            type Output = ::glamour::Rect<$dst>;
            $(#[$meta_map])*
            fn map(&$this, $v: ::glamour::Rect<$src>) -> Self::Output {
                ::glamour::Rect::new(
                    ::glamour::TransformMap::map($this, $v.origin),
                    ::glamour::TransformMap::map($this, $v.size),
                )
            }
        }
        $(crate::im::macros::imvec_newtype! { $($rest)+ } )?
    };
    (
        $(#[$meta:meta])*
        $vis:vis struct $unit:ident([$scalar:tt; $dim:tt]);
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq)]
        $vis struct $unit {
            // NOTE: a raw type of glam::Vec4 would not align with ImVec4
            pub raw: $crate::im::macros::imvec_newtype! { @ty(Vector<$scalar, [$scalar; $dim]>) },
        }

        impl glamour::Unit for $unit {
            type Scalar = $scalar;
        }

        #[cfg(todo)]
        impl glamour::Transparent for $unit {
            #[cfg(todo)]
            type Wrapped = $crate::im::macros::imvec_newtype! { @ty(ImVec<[$scalar; $dim]>) };
            type Wrapped = <$crate::im::macros::imvec_newtype! { @ty(Vector<$scalar, [$scalar; $dim]>) } as glamour::Transparent>::Wrapped;
        }

        impl $unit {
            #[inline]
            pub const fn from_vector(v: $crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }) -> Self {
                //Self { raw: v.to_untyped() }
                unsafe {
                    ::core::mem::transmute(v)
                }
            }

            #[inline]
            pub fn new<T>(v: $crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }) -> Self where
                T: Into<$crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }>,
            {
                Self::from_vector(v.into())
            }

            #[inline]
            pub const fn from_raw(raw: $crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) }) -> Self {
                // TODO: macro constructor instead?
                //Self::from_raw(v.to_raw())
                unsafe {
                    ::core::mem::transmute(raw)
                }
            }
            #[inline]
            pub const fn from_mint(mint: $crate::im::macros::imvec_newtype! { @ty(mint::Vector<[$scalar; $dim]>) }) -> Self {
                Self::from_raw(unsafe {
                    ::core::mem::transmute(mint)
                })
            }

            #[inline]
            pub const fn to_raw(self) -> $crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) } {
                // TODO: macro constructor instead?
                unsafe {
                    ::core::mem::transmute(self)
                }
            }
            #[inline]
            pub const fn as_raw(&self) -> &$crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            pub fn as_raw_mut(&mut self) -> &mut $crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }
            #[inline]
            pub const fn to_vector(self) -> $crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            pub const fn as_vector(&self) -> &$crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            pub fn as_vector_mut(&mut self) -> &mut $crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            pub const fn to_mint(self) -> $crate::im::macros::imvec_newtype! { @ty(mint::Vector<[$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            pub const fn as_mint(&self) -> &$crate::im::macros::imvec_newtype! { @ty(mint::Vector<[$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            pub fn as_mint_mut(&mut self) -> &mut $crate::im::macros::imvec_newtype! { @ty(mint::Vector<[$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }
        }

        impl ::core::ops::Deref for $unit {
            type Target = $crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) };

            #[inline]
            fn deref(&self) -> &Self::Target {
                self.as_vector()
            }
        }
        impl ::core::ops::DerefMut for $unit {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.as_vector_mut()
            }
        }
        impl ::core::convert::AsRef<$crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) }> for $unit {
            #[inline]
            fn as_ref(&self) -> &$crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) } {
                self.as_raw()
            }
        }
        impl ::core::convert::AsMut<$crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) }> for $unit {
            #[inline]
            fn as_mut(&mut self) -> &mut $crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) } {
                self.as_raw_mut()
            }
        }
        impl ::core::convert::AsMut<$crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn as_mut(&mut self) -> &mut $crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                self.as_vector_mut()
            }
        }
        impl ::core::convert::AsRef<$crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn as_ref(&self) -> &$crate::im::macros::imvec_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                self.as_vector()
            }
        }
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) }> for $unit {
            #[inline]
            fn from(raw: $crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) }) -> Self {
                Self::from_raw(raw)
            }
        }
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(Vector<$scalar, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn from(v: $crate::im::macros::imvec_newtype! { @ty(Vector<$scalar, [$scalar; $dim]>) }) -> Self {
                Self::from_vector(v.cast())
            }
        }
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(Vector<$unit, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn from(v: $crate::im::macros::imvec_newtype! { @ty(Vector<$unit, [$scalar; $dim]>) }) -> Self {
                Self::from_vector(v)
            }
        }
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(Size<$scalar, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn from(v: $crate::im::macros::imvec_newtype! { @ty(Size<$scalar, [$scalar; $dim]>) }) -> Self {
                Self::from_vector(v.to_vector().cast())
            }
        }
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(Size<$unit, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn from(v: $crate::im::macros::imvec_newtype! { @ty(Size<$unit, [$scalar; $dim]>) }) -> Self {
                Self::from_vector(v.to_vector())
            }
        }
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(Point<$scalar, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn from(v: $crate::im::macros::imvec_newtype! { @ty(Point<$scalar, [$scalar; $dim]>) }) -> Self {
                Self::from_vector(v.to_vector().cast())
            }
        }
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(Point<$unit, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn from(v: $crate::im::macros::imvec_newtype! { @ty(Point<$unit, [$scalar; $dim]>) }) -> Self {
                Self::from_vector(v.to_vector())
            }
        }
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(mint::Vector<[$scalar; $dim]>) }> for $unit {
            #[inline(always)]
            fn from(im: $crate::im::macros::imvec_newtype! { @ty(mint::Vector<[$scalar; $dim]>) }) -> Self {
                Self::from_mint(im)
            }
        }
        #[cfg(feature = "imgui180")]
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(im::180::ImVec<[$scalar; $dim]>) }> for $unit {
            #[inline(always)]
            fn from(im: $crate::im::macros::imvec_newtype! { @ty(im::180::ImVec<[$scalar; $dim]>) }) -> Self {
                Self::from_mint(im.into())
            }
        }
        #[cfg(feature = "imgui192")]
        impl ::core::convert::From<$crate::im::macros::imvec_newtype! { @ty(im::192::ImVec<[$scalar; $dim]>) }> for $unit {
            #[inline(always)]
            fn from(im: $crate::im::macros::imvec_newtype! { @ty(im::192::ImVec<[$scalar; $dim]>) }) -> Self {
                Self::from_mint(im.into())
            }
        }
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(Vec<[$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.raw.to_raw()
            }
        }
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(Vector<$unit, [$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.to_vector()
            }
        }
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(Vector<$scalar, [$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.to_vector().to_untyped()
            }
        }
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(Size<$unit, [$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.to_vector().to_size()
            }
        }
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(Size<$scalar, [$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.to_vector().to_untyped().to_size()
            }
        }
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(Point<$unit, [$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.to_vector().to_point()
            }
        }
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(Point<$scalar, [$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.to_vector().to_untyped().to_point()
            }
        }
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(mint::Vector<[$scalar; $dim]>) } {
            #[inline(always)]
            fn from(v: $unit) -> Self {
                v.to_mint()
            }
        }
        #[cfg(feature = "imgui180")]
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(im::180::ImVec<[$scalar; $dim]>) } {
            #[inline(always)]
            fn from(v: $unit) -> Self {
                v.to_mint().into()
            }
        }
        #[cfg(feature = "imgui192")]
        impl ::core::convert::From<$unit> for $crate::im::macros::imvec_newtype! { @ty(im::192::ImVec<[$scalar; $dim]>) } {
            #[inline(always)]
            fn from(v: $unit) -> Self {
                v.to_mint().into()
            }
        }
        impl ::mint::IntoMint for $unit {
            type MintType = crate::im::macros::imvec_newtype! { @ty(mint::Vector<[$scalar; $dim]>) };
        }
    };
    (@ty(Vec<[f32; 4]>)) => {
        glam::Vec4
    };
    (@ty(Vec<[f32; 2]>)) => {
        glam::Vec2
    };
    (@ty(Vec<[u16; 2]>)) => {
        glam::U16Vec2
    };
    (@ty(Vec<[i32; 2]?)) => {
        glam::IVec2
    };
    (@ty(Vec<[f64; 4]>)) => {
        glam::Vec4D
    };
    (@ty(Vec<[f64; 2]>)) => {
        glam::Vec2D
    };
    (@ty(Vec<[$scalar:ty; $dim:tt]>)) => {
        compile_error! { "unsupported imvec glam type" }
    };
    (@ty(im::180::ImVec<[f32; 4]>)) => {
        $crate::im::im180::sys::ImVec4
    };
    (@ty(im::180::ImVec<[f32; 2]>)) => {
        $crate::im::im180::sys::ImVec2
    };
    (@ty(im::192::ImVec<[f32; 4]>)) => {
        $crate::im::im192::sys::ImVec4
    };
    (@ty(im::192::ImVec<[f32; 2]>)) => {
        $crate::im::im192::sys::ImVec2
    };
    (@ty(im::$v:tt<[$scalar:ty; $dim:tt]>)) => {
        compile_error! { "unsupported imvec sys type" }
    };
    (@ty(Vector<$unit:ty, [$scalar:ty; 4]>)) => {
        ::glamour::Vector4<$unit>
    };
    (@ty(Vector<$unit:ty, [$scalar:ty; 2]>)) => {
        ::glamour::Vector2<$unit>
    };
    (@ty(Size<$unit:ty, [$scalar:ty; 2]>)) => {
        ::glamour::Size2<$unit>
    };
    (@ty(Point<$unit:ty, [$scalar:ty; 2]>)) => {
        ::glamour::Point2<$unit>
    };
    (@ty(Vector<$unit:ty, [$scalar:ty; $dim:tt]>)) => {
        compile_error! { "unsupported imvec glamour type" }
    };
    (@ty(Size<$unit:ty, [$scalar:ty; $dim:tt]>)) => {
        compile_error! { "unsupported imvec glamour type" }
    };
    (@ty(Point<$unit:ty, [$scalar:ty; $dim:tt]>)) => {
        compile_error! { "unsupported imvec glamour type" }
    };
    (@ty(mint::Vector<[$scalar:ty; 4]>)) => {
        ::mint::Vector4<$scalar>
    };
    (@ty(mint::Vector<[$scalar:ty; 2]>)) => {
        ::mint::Vector2<$scalar>
    };
    (@ty(mint::Vector<[$scalar:ty; $dim:tt]>)) => {
        compile_error! { "unsupported imvec mint type" }
    };
}
pub(crate) use imvec_newtype;
