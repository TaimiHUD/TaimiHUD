use mint::IntoMint;
pub use self::bounded::*;

mod bounded;

pub trait ConstNan {
    const NAN: Self;
    const NAN_NEG: Self;
}
impl ConstNan for f32 {
    const NAN: Self = f32::NAN;
    const NAN_NEG: Self = {
        let nan = f32::NAN;
        match nan.is_sign_negative() {
            true => nan,
            false => -nan,
        }
    };
}
impl ConstNan for f64 {
    const NAN: Self = f64::NAN;
    const NAN_NEG: Self = {
        let nan = f64::NAN;
        match nan.is_sign_negative() {
            true => nan,
            false => -nan,
        }
    };
}

pub fn to_nalg<O, T: mint::IntoMint>(value: T) -> O where
    T::MintType: Into<O>,
{
    value.into().into()
}

pub trait MintConv: Sized {
    type Mint
        // From<<Self::MintGlamour as IntoMint>::MintType> + From<<Self::MintNalg as IntoMint>::MintType>
    ;
    type MintGlamour: IntoMint;
    type MintNalg/*: IntoMint*/;
    fn from_glamour(v: Self::MintGlamour) -> Self;
    fn into_glamour(self) -> Self::MintGlamour;
    fn from_mint(v: Self::Mint) -> Self {
        Self::from_glamour(Self::glamour_from_mint(v))
    }
    fn glamour_from_mint(v: Self::Mint) -> Self::MintGlamour;
    fn mint_from_glamour(v: Self::MintGlamour) -> Self::Mint;
    fn nalg_from_mint(v: Self::Mint) -> Self::MintNalg;
    fn mint_from_nalg(v: Self::MintNalg) -> Self::Mint;
    #[inline]
    fn into_nalg(self) -> Self::MintNalg {
        Self::nalg_from_mint(self.into_mint())
    }
    #[inline]
    fn from_nalg(v: Self::MintNalg) -> Self {
        Self::from_mint(Self::mint_from_nalg(v))
    }
    #[inline]
    fn glamour_into_mint(v: Self::MintGlamour) -> <Self::MintGlamour as IntoMint>::MintType {
        v.into()
    }
    #[inline]
    #[cfg(todo)]
    fn nalg_into_mint(v: Self::MintNalg) -> <Self::MintNalg as IntoMint>::MintType {
        v.into()
    }
    #[inline]
    fn into_mint_glamour(self) -> <Self::MintGlamour as IntoMint>::MintType {
        self.into_glamour().into()
    }
    #[inline]
    fn into_mint(self) -> Self::Mint {
        Self::mint_from_glamour(
            self.into_glamour()
        )
    }
}
#[cfg(deleteme)]
pub trait FromMint: MintConv {
    fn raw_from_nalg(v: Self::MintNalg) -> Self::MintGlamour;
    fn from_nalg(v: Self::MintNalg) -> Self;
    //fn from_mint(v: <Self::MintGlamour as IntoMint>::MintType) -> Self;
}
#[cfg(deleteme)]
impl<T: MintConv> FromMint for T where
    <T::MintGlamour as IntoMint>::MintType: Into<T::MintGlamour>,
    <T::MintNalg as IntoMint>::MintType: Into<T::MintNalg>,
{
    #[inline]
    fn raw_from_nalg(v: Self::MintNalg) -> Self::MintGlamour {
        Self::mint_from_nalg(v).into()
    }
    #[inline]
    fn from_nalg(v: Self::MintNalg) -> Self {
        Self::from_mint(Self::mint_from_nalg(v))
    }
    #[cfg(deleteme)]
    #[inline]
    fn from_mint(v: <Self::MintGlamour as IntoMint>::MintType) -> Self {
        Self::from_glamour(v.into())
    }
}
macro_rules! impl_mint_conv {
    (
        mint::$mint:ident for glamour::$glam:ident<$glamunit:tt>, nalgebra::$nalg:ident {
        }
        $($($rest:tt)+)?
    ) => {
        impl<U: glamour::$glamunit> MintConv for glamour::$glam<U> where
            glamour::$glam<impl_mint_conv!{@glamscalar(U, $glamunit)}>:
                IntoMint,
                //+ From<<glamour::$glam<impl_mint_conv!{@glamscalar(U, $glamunit)}> as IntoMint>::MintType>,
            nalgebra::$nalg<impl_mint_conv!{@glamscalar(U, $glamunit)}>:
                Into<
                    mint::$mint<impl_mint_conv!{@glamscalar(U, $glamunit)}>
                >,
                //+ From<<nalgebra::$nalg<impl_mint_conv!{@glamscalar(U, $glamunit)}> as IntoMint>::MintType>,
            mint::$mint<impl_mint_conv!{@glamscalar(U, $glamunit)}>:
                /*From<<nalgebra::$nalg<impl_mint_conv!{@glamscalar(U, $glamunit)}> as IntoMint>::MintType>
                + From<<glamour::$glam<impl_mint_conv!{@glamscalar(U, $glamunit)}> as IntoMint>::MintType>*/
                Into<nalgebra::$nalg<impl_mint_conv!{@glamscalar(U, $glamunit)}>>
                + Into<<glamour::$glam<impl_mint_conv!{@glamscalar(U, $glamunit)}> as IntoMint>::MintType>,
            <glamour::$glam<impl_mint_conv!{@glamscalar(U, $glamunit)}> as IntoMint>::MintType: Into<
                mint::$mint<impl_mint_conv!{@glamscalar(U, $glamunit)}>
            > + Into<
                glamour::$glam<impl_mint_conv!{@glamscalar(U, $glamunit)}>
            >,
        {
            type Mint = mint::$mint<impl_mint_conv!{@glamscalar(U, $glamunit)}>;
            type MintGlamour = glamour::$glam<impl_mint_conv!{@glamscalar(U, $glamunit)}>;
            type MintNalg = nalgebra::$nalg<impl_mint_conv!{@glamscalar(U, $glamunit)}>;

            fn from_glamour(v: Self::MintGlamour) -> Self {
                impl_mint_conv! {
                    @glamscalarcast($glamunit, v.cast())
                }
            }
            #[inline]
            fn into_glamour(self) -> Self::MintGlamour {
                impl_mint_conv! {
                    @glamscalarcast($glamunit, self.to_untyped())
                }
            }
            #[inline]
            fn nalg_from_mint(v: Self::Mint) -> Self::MintNalg {
                //let v: <nalgebra::$nalg<impl_mint_conv!{@glamscalar(U, $glamunit)}> as IntoMint>::MintType = v.into();
                v.into()
            }
            #[inline]
            fn glamour_from_mint(v: Self::Mint) -> Self::MintGlamour {
                let v: <glamour::$glam<impl_mint_conv!{@glamscalar(U, $glamunit)}> as IntoMint>::MintType = v.into();
                v.into()
            }
            #[inline]
            fn mint_from_glamour(v: Self::MintGlamour) -> Self::Mint {
                Self::glamour_into_mint(v).into()
            }
            #[inline]
            fn mint_from_nalg(v: Self::MintNalg) -> Self::Mint {
                v.into()
            }
        }
        $(impl_mint_conv! {$($rest)*})?
    };
    (@glamscalar($unit:ident, Unit)) => {
        <$unit as glamour::Unit>::Scalar
    };
    (@glamscalar($unit:ident, Scalar)) => {
        $unit
    };
    (@glamscalarcast(Scalar, $v:ident.$f:ident())) => {
        $v
    };
    (@glamscalarcast(Unit, $v:ident.$f:ident())) => {
        $v.$f()
    };
}
impl_mint_conv! {
    mint::Point2 for glamour::Point2<Unit>, nalgebra::Point2 {}
    mint::Point3 for glamour::Point3<Unit>, nalgebra::Point3 {}
    mint::Vector4 for glamour::Point4<Unit>, nalgebra::Point4 {}
    mint::Vector2 for glamour::Vector2<Unit>, nalgebra::Vector2 {}
    mint::Vector3 for glamour::Vector3<Unit>, nalgebra::Vector3 {}
    mint::Vector4 for glamour::Vector4<Unit>, nalgebra::Vector4 {}
    mint::ColumnMatrix2 for glamour::Matrix2<Scalar>, nalgebra::Matrix2 {}
    mint::ColumnMatrix3 for glamour::Matrix3<Scalar>, nalgebra::Matrix3 {}
    mint::ColumnMatrix4 for glamour::Matrix4<Scalar>, nalgebra::Matrix4 {}
}
