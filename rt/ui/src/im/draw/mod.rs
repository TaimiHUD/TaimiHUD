use super::prelude::*;

pub mod state;

pub use self::state::{DrawContext, InteractSignal};

/// catch-all "alias" for a base feature set
pub trait ImDrawWindow<'ui>:
    ImFrameArena<'ui>
    + ImDraw
    + ImTable
    + ImTableLegacy
    + ImTableStack<'ui>
    + ImDrawText
    + ImFontStack<'ui, (), FontToken = ()>
    + imw::ImDrawWidgetHost<'ui>
    + ImColourStack<'ui, ImColourIndex, StyleTokenColour = UiTokenDyn<'ui>>
    + ImColourContainer<ImColourIndex>
    + ImDrawWindowStack<'ui>
    + ImDrawItemStack<'ui, StyleTokenItemSpacing = UiTokenDyn<'ui>>
    + ImDrawTextStack<
        'ui,
        TextWrapPosToken = UiTokenDyn<'ui>,
        TextColourToken = UiTokenDyn<'ui>,
        TextScaleToken = UiTokenDyn<'ui>,
    >
{
    #[inline(always)]
    fn draw_dyn(&mut self) -> &mut dyn ImDrawWindow<'ui>
    where
        Self: Sized,
    {
        self
    }
}
impl<'ui, U> ImDrawWindow<'ui> for U where
    U: ?Sized
        + ImFrameArena<'ui>
        + ImDrawText
        + ImTable
        + ImTableLegacy
        + ImTableStack<'ui>
        + ImColourStack<'ui, ImColourIndex, StyleTokenColour = UiTokenDyn<'ui>>
        + ImColourContainer<ImColourIndex>
        + imw::ImDrawWidgetHost<'ui>
        + ImDrawWindowStack<'ui>
        + ImDrawItemStack<'ui, StyleTokenItemSpacing = UiTokenDyn<'ui>>
        + ImDrawTextStack<
            'ui,
            TextWrapPosToken = UiTokenDyn<'ui>,
            TextColourToken = UiTokenDyn<'ui>,
            TextScaleToken = UiTokenDyn<'ui>,
        >
{
}

pub trait ImDrawWindowExt<'ui>:
    ImDrawWindow<'ui>
    + ImColourContainer<ImColourIndex>
    + ImColourStack<'ui, ImColourIndex>
    + ImFontStack<'ui, (), FontToken = ()>
{
    #[inline(always)]
    fn draw_dyn(&mut self) -> &mut dyn ImDrawWindow<'ui>
    where
        Self: Sized,
    {
        self
    }
}

pub trait ImWidget<U: ?Sized + ImDraw>: Sized {
    type Output;
    fn draw_widget(self, ui: &mut U) -> Self::Output;
}
impl<U, T, R> ImWidget<U> for T
where
    T: FnOnce(&mut U) -> R,
    U: ImDraw,
{
    type Output = R;
    #[inline]
    fn draw_widget(self, ui: &mut U) -> Self::Output {
        self(ui)
    }
}
pub trait ImContainer<U: ?Sized + ImDraw>: Sized {
    type ContainerToken: UiToken + IntoTokenGuard;
    fn draw_container(self, ui: &mut U) -> Self::ContainerToken;
}
impl<T, U: ?Sized> ImContainer<U> for T
where
    U: ImDraw,
    T: ImWidget<U>,
    <T as ImWidget<U>>::Output: UiToken + IntoTokenGuard,
{
    type ContainerToken = <T as ImWidget<U>>::Output;
    #[inline]
    fn draw_container(self, ui: &mut U) -> Self::ContainerToken {
        self.draw_widget(ui)
    }
}
pub trait ImContainerExt<U: ?Sized + ImDraw>: ImContainer<U> {
    fn begin_widget(
        self,
        ui: &mut U,
    ) -> <<Self as ImContainer<U>>::ContainerToken as IntoTokenGuard>::TokenGuardType;
    fn begin_contain<V>(self, ui: &mut U) -> Option<<V as IntoTokenGuard>::TokenGuardType>
    where
        Self: ImContainer<U, ContainerToken = Option<V>>,
        V: UiToken + IntoTokenGuard;
    fn enter_widget_<R, F: FnOnce(&mut U) -> R>(
        self,
        ui: &mut U,
        inner: F,
    ) -> (
        R,
        <<Self as ImContainer<U>>::ContainerToken as IntoTokenGuard>::TokenGuardType,
    );
    fn draw_widget_with<R, F: FnOnce(&mut U) -> R>(self, ui: &mut U, inner: F) -> R;
    fn contain_with<R, F: FnOnce(&mut U) -> R, V>(self, ui: &mut U, inner: F) -> Option<R>
    where
        Self: ImContainer<U, ContainerToken = Option<V>>,
        V: UiToken + IntoTokenGuard;
}
impl<T, U: ?Sized> ImContainerExt<U> for T
where
    U: ImDraw,
    T: ImContainer<U>,
    //<T as ImWidget<U>>::Output: UiToken,
{
    #[inline]
    fn begin_widget(
        self,
        ui: &mut U,
    ) -> <<Self as ImContainer<U>>::ContainerToken as IntoTokenGuard>::TokenGuardType {
        self.draw_container(ui).into_guard()
    }
    #[inline]
    fn begin_contain<V>(self, ui: &mut U) -> Option<<V as IntoTokenGuard>::TokenGuardType>
    where
        Self: ImContainer<U, ContainerToken = Option<V>>,
        V: IntoTokenGuard,
    {
        self.draw_container(ui).map(IntoTokenGuard::into_guard)
    }
    #[inline]
    fn enter_widget_<R, F: FnOnce(&mut U) -> R>(
        self,
        ui: &mut U,
        inner: F,
    ) -> (
        R,
        <<Self as ImContainer<U>>::ContainerToken as IntoTokenGuard>::TokenGuardType,
    ) {
        let guard = self.begin_widget(ui);
        let res = inner(ui);
        (res, guard)
    }
    #[inline]
    fn draw_widget_with<R, F: FnOnce(&mut U) -> R>(self, ui: &mut U, inner: F) -> R {
        let (res, _guard) = self.enter_widget_(ui, inner);
        res
    }
    #[inline]
    fn contain_with<R, F: FnOnce(&mut U) -> R, V>(self, ui: &mut U, inner: F) -> Option<R>
    where
        Self: ImContainer<U, ContainerToken = Option<V>>,
        V: IntoTokenGuard,
    {
        let _guard = self.draw_container(ui)?.into_guard();
        let res = inner(ui);
        Some(res)
    }
}
