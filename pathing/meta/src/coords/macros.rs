#[macro_export]
macro_rules! coord_newtype {
    (impl TransformMap<$src:ty, Output == $glamty:ident<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl ::glamour::TransformMap<::glamour::$glamty<$src>> for $ty {
            type Output = ::glamour::$glamty<$dst>;
            $(#[$meta_map])*
            fn map(&$this, $v: ::glamour::$glamty<$src>) -> Self::Output {
                $to_v
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Vec2<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        crate::coords::coord_newtype! {
            impl TransformMap<$src, Output = Vector2<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                }
            }
            impl TransformMap<$src, Output = Point2<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                    //::glamour::TransformMap::map($this, $v.to_vector()).to_point()
                }
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Vector2<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl ::glamour::TransformMap<::glamour::Vector2<$src>> for $ty {
            type Output = ::glamour::Vector2<$dst>;
            $(#[$meta_map])*
            fn map(&$this, $v: ::glamour::Vector2<$src>) -> Self::Output {
                $to_v
            }
        }
        impl ::glamour::TransformMap<::glamour::Size2<$src>> for $ty {
            type Output = ::glamour::Size2<$dst>;
            #[inline(always)]
            fn map(&$this, $v: ::glamour::Size2<$src>) -> Self::Output {
                ::glamour::TransformMap::map($this, $v.to_vector()).to_size()
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Point2<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl ::glamour::TransformMap<::glamour::Point2<$src>> for $ty {
            type Output = ::glamour::Point2<$dst>;
            #[inline(always)]
            fn map(&$this, $v: ::glamour::Point2<$src>) -> Self::Output {
                $to_v
            }
        }
        impl ::glamour::TransformMap<::glamour::Box2<$src>> for $ty {
            type Output = ::glamour::Box2<$dst>;
            #[inline]
            fn map(&$this, $v: ::glamour::Box2<$src>) -> Self::Output {
                ::glamour::Box2::new(
                    ::glamour::TransformMap::map($this, $v.min),
                    ::glamour::TransformMap::map($this, $v.max),
                )
            }
        }
        impl ::glamour::TransformMap<::glamour::Rect<$src>> for $ty {
            type Output = ::glamour::Rect<$dst>;
            $(#[$meta_map])*
            fn map(&$this, $v: ::glamour::Rect<$src>) -> Self::Output {
                ::glamour::Rect::new(
                    ::glamour::TransformMap::map($this, $v.origin),
                    ::glamour::TransformMap::map($this, $v.size),
                )
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Vec3<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        crate::coords::coord_newtype! {
            impl TransformMap<$src, Output = Vector3<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                }
            }
            impl TransformMap<$src, Output = Point3<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                    //::glamour::TransformMap::map($this, $v.to_vector()).to_point()
                }
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Vector3<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl ::glamour::TransformMap<::glamour::Vector3<$src>> for $ty {
            type Output = ::glamour::Vector3<$dst>;
            $(#[$meta_map])*
            fn map(&$this, $v: ::glamour::Vector3<$src>) -> Self::Output {
                $to_v
            }
        }
        impl ::glamour::TransformMap<::glamour::Size3<$src>> for $ty {
            type Output = ::glamour::Size3<$dst>;
            #[inline(always)]
            fn map(&$this, $v: ::glamour::Size3<$src>) -> Self::Output {
                ::glamour::TransformMap::map($this, $v.to_vector()).to_size()
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Point3<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl ::glamour::TransformMap<::glamour::Point3<$src>> for $ty {
            type Output = ::glamour::Point3<$dst>;
            #[inline(always)]
            fn map(&$this, $v: ::glamour::Point3<$src>) -> Self::Output {
                ::glamour::TransformMap::map($this, $v.to_vector()).to_point()
            }
        }
        impl ::glamour::TransformMap<::glamour::Box3<$src>> for $ty {
            type Output = ::glamour::Box3<$dst>;
            #[inline(always)]
            fn map(&$this, $v: ::glamour::Box3<$src>) -> Self::Output {
                ::glamour::Box3::new(
                    ::glamour::TransformMap::map($this, $v.min),
                    ::glamour::TransformMap::map($this, $v.max),
                )
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Vec4<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        crate::coords::coord_newtype! {
            impl TransformMap<$src, Output = Vector4<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                }
            }
            impl TransformMap<$src, Output = Point4<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                    //::glamour::TransformMap::map($this, $v.to_vector()).to_point()
                }
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Vector4<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl ::glamour::TransformMap<::glamour::Vector4<$src>> for $ty {
            type Output = ::glamour::Vector4<$dst>;
            $(#[$meta_map])*
            fn map(&$this, $v: ::glamour::Vector4<$src>) -> Self::Output {
                $to_v
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Point4<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($($rest:tt)+)?) => {
        impl ::glamour::TransformMap<::glamour::Point4<$src>> for $ty {
            type Output = ::glamour::Point4<$dst>;
            #[inline(always)]
            fn map(&$this, $v: ::glamour::Point4<$src>) -> Self::Output {
                ::glamour::TransformMap::map($this, $v.to_vector()).to_point()
            }
        }
        $(crate::coords::coord_newtype! { $($rest)+ } )?
    };
    (impl TransformMap<$src:ty, Output = Vec<$dst:ty>> for $ty:ty {
        $(#[$meta_map:meta])*
        fn map(&$this:ident, $v:ident) {
            $to_v:expr
        }
    } $($rest:tt)*) => {
        crate::coords::coord_newtype! {
            impl TransformMap<$src, Output = Vec2<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                }
            }
            impl TransformMap<$src, Output = Vec3<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                }
            }
            impl TransformMap<$src, Output = Vec4<$dst>> for $ty {
                $(#[$meta_map])*
                fn map(&$this, $v) {
                    $to_v
                }
            }
            $($rest)*
        }
    };
    (
        $(#[$meta:meta])*
        $vis:vis struct $unit:ident([$scalar:tt; $dim:tt]);
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq)]
        $vis struct $unit {
            pub raw: $crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) },
        }

        impl glamour::Unit for $unit {
            type Scalar = $scalar;
        }

        impl $unit {
            #[inline]
            pub const fn from_vector(v: $crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }) -> Self {
                // TODO: macro constructor instead?
                //Self::from_raw(v.to_raw())
                unsafe {
                    ::core::mem::transmute(v)
                }
            }

            #[inline]
            pub fn new<T>(v: $crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }) -> Self where
                T: Into<$crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }>,
            {
                Self::from_vector(v.into())
            }

            #[inline]
            pub const fn from_raw(raw: $crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) }) -> Self {
                Self {
                    raw,
                }
            }

            #[inline]
            pub const fn to_vector(self) -> $crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                // TODO: macro constructor instead?
                //<$crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }>::from_raw(self.raw)
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            pub const fn as_vector(&self) -> &$crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }

            #[inline]
            pub fn as_vector_mut(&mut self) -> &mut $crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                unsafe {
                    ::core::mem::transmute(self)
                }
            }
        }

        impl ::core::ops::Deref for $unit {
            type Target = $crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) };

            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.raw
            }
        }
        impl ::core::ops::DerefMut for $unit {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.raw
            }
        }
        impl ::core::convert::AsRef<$crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) }> for $unit {
            #[inline]
            fn as_ref(&self) -> &$crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) } {
                &self.raw
            }
        }
        impl ::core::convert::AsMut<$crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) }> for $unit {
            #[inline]
            fn as_mut(&mut self) -> &mut $crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) } {
                &mut self.raw
            }
        }
        impl ::core::convert::AsMut<$crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn as_mut(&mut self) -> &mut $crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                self.as_vector_mut()
            }
        }
        impl ::core::convert::AsRef<$crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) }> for $unit {
            #[inline]
            fn as_ref(&self) -> &$crate::coords::coord_newtype! { @ty(Vector<Self, [$scalar; $dim]>) } {
                self.as_vector()
            }
        }
        impl ::core::convert::From<$crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) }> for $unit {
            #[inline]
            fn from(raw: $crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) }) -> Self {
                Self::from_raw(raw)
            }
        }
        impl ::core::convert::From<$unit> for $crate::coords::coord_newtype! { @ty(Vec<[$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.raw
            }
        }
        impl ::core::convert::From<$unit> for $crate::coords::coord_newtype! { @ty(Vector<$unit, [$scalar; $dim]>) } {
            #[inline]
            fn from(v: $unit) -> Self {
                v.to_vector()
            }
        }
    };
    (@ty(Vec<[f32; 3]>)) => {
        glam::Vec3
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
    (@ty(Vec<[f64; 3]>)) => {
        glam::Vec3D
    };
    (@ty(Vec<[f64; 2]>)) => {
        glam::Vec2D
    };
    (@ty(Vec<[$scalar:ty; $dim:tt]>)) => {
        compile_error! { "unsupported coord raw type" }
    };
    (@ty(Vector<$unit:ty, [$scalar:ty; 3]>)) => {
        glamour::Vector3<$unit>
    };
    (@ty(Vector<$unit:ty, [$scalar:ty; 2]>)) => {
        glamour::Vector2<$unit>
    };
}
pub use coord_newtype;
