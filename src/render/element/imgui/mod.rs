#[cfg(taimi_imgui = "180")]
pub use taimi_ui::im::im180;
#[cfg(taimi_imgui = "192")]
pub use taimi_ui::im::im192;
pub use {
    self::{
        context::{UiContextCell, UiContextStorage, UiFrameStorage},
        draw::{ImDrawWindowExt, UiFrameState, UiState},
        selection::{
            SelectionEnumDesc,
            SelectionEnumDraw,
            SelectionEnumLabels,
            SelectionEnumState,
            SelectionScratch,
        },
        text::NexusLinkFont,
        util::{ComboInput, PositionInput},
    },
    taimi_ui::im as img,
};
use {
    crate::{
        exports::runtime::{self as rt, textures::ImguiTexture},
        settings::ui::UiConfig,
    },
    core::fmt,
    std::{borrow::Cow, sync::Arc},
    taimi_hoard::collections::FlatMap,
    taimi_ui::im::{
        self,
        image::ImTexture,
        io::ImContextState,
        text::ImStrDisplay,
        token::UiTokenDyn,
        ImSize2,
        ImVec2,
    },
};

mod context;
mod draw;
mod selection;
mod text;
mod util;
pub(crate) mod prelude {
    #![allow(unused_imports)]
    pub(crate) use {
        self::InteractSignal as ItemStatus,
        super::{
            im_fmt,
            im_to_string as im_to_s,
            img,
            ComboInput,
            DrawContext,
            DrawContextInput,
            Drawable,
            ImContextExt as _,
            ImDrawWindow,
            ImDrawWindowExt as _,
            ImTextDisplay,
            IntoImStrDisplay as _,
            IntoImTextureD3d as _,
            NexusLinkFont,
            PositionInput,
            UiFrameState,
            UiState,
        },
        crate::{
            exports::runtime::keyboard::KeyState,
            fl,
            render::{
                element::frame::{ContainerContextState, FrameContainerScope},
                i18n::{with_i18n, I18nRef},
            },
            settings::ui::UiConfig,
            with_i18n,
        },
        arcffi::cstr::{cstr, CSlice, Str0, String0},
        core::{ffi::CStr, fmt},
        num_traits::AsPrimitive,
        taimi_hoard::{
            iters::IterExt as _,
            lazyfmt::{
                self,
                fmt_args,
                fmt_default,
                fmt_fn,
                fmt_mut,
                fmt_once,
                fmt_or,
                lazyfmt,
                ok_or as fmt_either,
                or_empty as fmt_opt,
                or_unavail as fmt_unwrap,
            },
        },
        taimi_ui::im::{
            draw::state::{DrawContext as _, DrawContextSignal, InteractSignal},
            prelude::*,
            text::IntoImStrId,
            token::UiTokenDyn,
        },
    };

    #[cfg(taimi_imgui = "180")]
    pub(crate) use super::im180::{self, sys as sys180};
    #[cfg(taimi_imgui = "192")]
    pub(crate) use super::im192::{self, sys as sys192};
}

pub type DrawContextInput<'ui> = &'ui UiFrameStorage;

pub trait ImDrawWindow<'ui>:
    im::draw::ImDrawWindow<'ui>
    + im::text::ImFontStack<'ui, NexusLinkFont, FontToken = Option<UiTokenDyn<'ui>>>
    + im::colours::ImColourContainer<rt::alert::LogWarningColour>
{
    #[cfg(taimi_imgui = "180")]
    fn im180_escape_hatch(&self) -> Option<&im::im180::Ui<'ui>> {
        None
    }
    #[cfg(taimi_imgui = "192")]
    fn im192_escape_hatch(&self) -> Option<&im::im192::Ui<'ui>> {
        None
    }

    fn im_io_display_size(&self) -> (ImSize2<f32>, ImVec2<f32>);
    fn im_io_mod_keys(&self) -> rt::keyboard::KeyState;
    /// ctrl is held, or super on mac
    fn im_io_key_is_shortcut(&self) -> bool;

    fn im_font_char_replacements(&self, c: char) -> char {
        match c {
            '’' => 0xb4u8 as char,
            c => c,
        }
    }
    fn im_font_has_char(&self, c: char) -> bool {
        !['’'].contains(&c)
    }
    fn im_text_zerowidth8(&self) -> u8;
    fn im_text_zerowidth(&self) -> char {
        self.im_text_zerowidth8() as char
    }
}
#[cfg(taimi_imgui = "180")]
impl<'ui> ImDrawWindow<'ui> for &'_ im::im180::Ui<'ui> {
    fn im180_escape_hatch(&self) -> Option<&im::im180::Ui<'ui>> {
        Some(*self)
    }
    fn im_io_display_size(&self) -> (ImSize2<f32>, ImVec2<f32>) {
        use prelude::*;
        self.with_io(|io| (io.display_size.into(), io.display_framebuffer_scale.into()))
    }
    fn im_io_mod_keys(&self) -> rt::keyboard::KeyState {
        use prelude::*;
        self.with_io(|io| {
            IntoIterator::into_iter([
                io.key_alt.then_some(rt::keyboard::KeyState::ALT),
                io.key_ctrl.then_some(rt::keyboard::KeyState::CTRL),
                io.key_shift.then_some(rt::keyboard::KeyState::SHIFT),
                //io.key_super.then_some(rt::keyboard::KeyState::SUPER),
            ])
            .flatten()
            .collect()
        })
    }
    fn im_io_key_is_shortcut(&self) -> bool {
        use prelude::*;
        self.with_io(|io| match io.config_mac_os_behaviors {
            true => io.key_super,
            false => io.key_ctrl,
        })
    }
    /// TODO: check all fonts
    #[inline]
    fn im_text_zerowidth8(&self) -> u8 {
        0x0d
    }
}
#[cfg(taimi_imgui = "192")]
impl<'ui> ImDrawWindow<'ui> for im::im192::Ui<'ui> {
    fn im192_escape_hatch(&self) -> Option<&im::im192::Ui<'ui>> {
        Some(self)
    }
    fn im_io_display_size(&self) -> (ImSize2<f32>, ImVec2<f32>) {
        use prelude::*;
        self.with_io(|io| {
            (
                im::ImSpaces(io.DisplaySize).into(),
                im::ImSpaces(io.DisplayFramebufferScale).into(),
            )
        })
    }
    fn im_io_mod_keys(&self) -> rt::keyboard::KeyState {
        use prelude::*;
        self.with_io(|io| {
            IntoIterator::into_iter([
                io.KeyAlt.then_some(rt::keyboard::KeyState::ALT),
                io.KeyCtrl.then_some(rt::keyboard::KeyState::CTRL),
                io.KeyShift.then_some(rt::keyboard::KeyState::SHIFT),
                //io.key_super.then_some(rt::keyboard::KeyState::SUPER),
            ])
            .flatten()
            .collect()
        })
    }
    fn im_io_key_is_shortcut(&self) -> bool {
        use prelude::*;
        self.with_io(|io| match io.ConfigMacOSXBehaviors {
            true => io.KeySuper,
            false => io.KeyCtrl,
        })
    }
    /// TODO: check fonts
    #[inline]
    fn im_text_zerowidth8(&self) -> u8 {
        b'\r'
    }
}
#[cfg(todo)]
impl<'ui, U: ?Sized> ImDrawWindow<'ui> for U where U: im::draw::ImDrawWindow<'ui> {}
pub trait ImContextExt: ImContextState {
    #[cfg(todo = "unused")]
    unsafe fn bind_imgui_ctx(
        &mut self,
        malloc: arcffi::UserMallocFn,
        free: arcffi::UserFreeFn,
        user_data: *mut arcffi::c_void,
    );
    unsafe fn bound_mut_dyn_unchecked<'a, 'ui>(&'a mut self) -> &'a mut dyn ImDrawWindow<'ui>
    where
        'ui: 'a;
}
impl<U> ImContextExt for U
where
    U: ImContextState + 'static,
    for<'ui> U::BoundContext<'ui, 'ui>: ImDrawWindow<'ui> + Sized,
{
    #[cfg(todo = "unused")]
    unsafe fn bind_imgui_ctx(
        &mut self,
        malloc: arcffi::UserMallocFn,
        free: arcffi::UserFreeFn,
        user_data: *mut arcffi::c_void,
    ) {
        if !self.is_bound() {
            self.bind_allocator(Some(malloc), Some(free), user_data);
            self.bind_unchecked();
        }
    }
    /// XXX: bounds can't express this..?
    unsafe fn bound_mut_dyn_unchecked<'a, 'ui>(&'a mut self) -> &'a mut dyn ImDrawWindow<'ui>
    where
        'ui: 'a,
    {
        //self.bound_mut::<'a, 'ui>()
        core::mem::transmute(self.bound_mut::<'a, 'a>() as &'a mut dyn ImDrawWindow<'a>)
    }
}
#[cfg(todo)]
impl<U> ImContextExt for im::im180::FallbackContext {}
#[cfg(todo)]
impl<U> ImContextExt for im::im192::FallbackContext {}

pub trait DrawContext<'ui>:
    im::draw::DrawContext<'ui>
    + im::draw::state::DrawContextSignal<'ui>
    + AsRef<UiConfig>
    + AsRef<UiState>
    + AsRef<UiFrameState>
    + AsMut<UiFrameState>
    + super::frame::FrameStackContext<'ui>
{
}
impl<'ui, C> DrawContext<'ui> for C where
    C: ?Sized
        + im::draw::DrawContext<'ui>
        + im::draw::state::DrawContextSignal<'ui>
        + AsRef<UiConfig>
        + AsRef<UiState>
        + AsRef<UiFrameState>
        + AsMut<UiFrameState>
        + super::frame::FrameStackContext<'ui>
{
}

/// a visible thing
///
/// NOTE: use context *very* sparingly, ideally not at all?
/// but how else do you want dependency injection...
#[allow(unused_variables)]
pub trait Drawable<W: ?Sized, C: ?Sized = ()> {
    fn draw_on_window(&mut self, window: &mut W, context: &mut C);
    /// TODO: use! when present but scrolled past etc
    fn draw_obscured(&mut self, window: &mut W, context: &mut C) {}
    /// TODO: use! after having been visible (parent window closed or obscured...)
    fn draw_stop(&mut self, context: &C) {}
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct ImTextureD3d11<T: ?Sized> {
    pub texture: T,
}
impl<T: ?Sized> ImTextureD3d11<T> {
    #[inline(always)]
    pub fn new(texture: T) -> Self
    where
        T: Sized,
    {
        Self { texture }
    }
    #[inline(always)]
    pub fn from_ref(texture: &T) -> &Self {
        unsafe { core::mem::transmute(texture) }
    }
}
#[cfg(todo)]
impl<T> ImTexture for ImTextureD3d11<T>
where
    T: ?Sized + AsRef<taimi_d3d::dx11::buffer::TextureView2>,
{
    #[inline]
    #[cfg(taimi_imgui = "180")]
    fn im180_texture_id(&self) -> Option<im::im180::sys::ImTextureID> {
        ImTexture::im180_texture_id(&self)
    }
    #[inline]
    #[cfg(taimi_imgui = "192")]
    fn im192_texture_ref(&self) -> Option<im::im192::sys::ImTextureRef> {
        ImTexture::im192_texture_id(&self)
    }
    #[cfg(todo)]
    fn as_any(&self) -> Option<&dyn core::any::Any> {
        Some(self)
    }
}
impl<T> ImTexture for &'_ ImTextureD3d11<T>
where
    T: ?Sized + AsRef<taimi_d3d::dx11::buffer::TextureView2>,
{
    #[inline]
    #[cfg(taimi_imgui = "180")]
    fn im180_texture_id(&self) -> Option<im::im180::sys::ImTextureID> {
        Some(self.texture.as_ref().as_d3d_raw().as_ptr())
    }
    #[inline]
    #[cfg(taimi_imgui = "192")]
    fn im192_texture_ref(&self) -> Option<im::im192::sys::ImTextureRef> {
        Some(im::im192::sys::ImTextureRef::from_id(
            self.texture.as_ref().as_d3d_raw().as_ptr() as usize as u64,
        ))
    }
    #[cfg(todo)]
    fn as_any(&self) -> Option<&dyn core::any::Any> {
        Some(self)
    }
}
pub trait IntoImTextureD3d {
    fn as_im_tex2(&self) -> &ImTextureD3d11<taimi_d3d::dx11::buffer::TextureView2>;
}
impl IntoImTextureD3d for taimi_d3d::dx11::buffer::TextureView2 {
    #[inline(always)]
    fn as_im_tex2(&self) -> &ImTextureD3d11<taimi_d3d::dx11::buffer::TextureView2> {
        ImTextureD3d11::from_ref(self)
    }
}
impl IntoImTextureD3d for taimi_d3d::dx11::buffer::ID3D11ShaderResourceView {
    #[inline(always)]
    fn as_im_tex2(&self) -> &ImTextureD3d11<taimi_d3d::dx11::buffer::TextureView2> {
        ImTextureD3d11::from_ref(unsafe {
            taimi_d3d::dx11::buffer::TextureView2::from_d3d_raw_ref(
                &*(self as *const _ as *const core::ptr::NonNull<_>),
            )
        })
    }
}
impl IntoImTextureD3d for nexus::texture::Texture {
    #[inline(always)]
    fn as_im_tex2(&self) -> &ImTextureD3d11<taimi_d3d::dx11::buffer::TextureView2> {
        self.resource.as_im_tex2()
    }
}

impl ImTexture for ImguiTexture {
    #[inline]
    #[cfg(taimi_imgui = "180")]
    fn im180_texture_id(&self) -> Option<im::im180::sys::ImTextureID> {
        self.id.as_ref().map(|srv| srv.as_d3d_raw().as_ptr() as _)
    }
    #[inline]
    #[cfg(taimi_imgui = "192")]
    fn im192_texture_ref(&self) -> Option<im::im192::sys::ImTextureRef> {
        self.id.as_ref().map(|srv| {
            let ptr = srv.as_d3d_raw().as_ptr();
            im::im192::sys::ImTextureRef::from_id(ptr as usize as _)
        })
    }
    fn as_any(&self) -> Option<&dyn core::any::Any> {
        Some(&self.id)
    }
}

pub struct ImTextDisplay<'r, T: ?Sized = String> {
    pub replacements: &'r FlatMap<char, &'static str>,
    pub inner: T,
}
impl<'r, T: ?Sized> ImTextDisplay<'r, T> {
    #[inline]
    pub fn display_fallback(inner: T) -> Self
    where
        T: Sized,
    {
        Self {
            replacements: ImTextDisplay::placeholder_nexus_replacements(),
            inner,
        }
    }
    /// TODO: track font for imgui context
    #[inline]
    pub fn display_for<U>(ui: &mut U, inner: T) -> Self
    where
        T: Sized,
        U: ?Sized + ImDrawWindow<'r>,
    {
        Self {
            replacements: ImTextDisplay::replacements_for(ui),
            inner,
        }
    }
    #[inline(always)]
    pub fn format_for<U>(ui: &mut U, inner: T) -> String
    where
        U: ?Sized + ImDrawWindow<'r>,
        T: Sized + fmt::Display,
    {
        use core::fmt::Write;
        let mut writer = ImTextDisplay {
            replacements: ImTextDisplay::replacements_for(ui),
            inner: Default::default(),
        };
        let _ = {
            let writer = &mut writer as &mut dyn Write;
            write!(writer, "{inner}")
        };
        writer.inner
    }
}
impl<'r> ImTextDisplay<'r> {
    pub fn display_str<'a, U>(ui: &mut U, s: &'a str) -> Cow<'a, str>
    where
        U: ?Sized + ImDrawWindow<'r>,
    {
        if s.as_bytes().iter().all(|&b| b <= 0x7f) {
            return Cow::Borrowed(s)
        }
        // TODO: check replacements
        Cow::Owned(ImTextDisplay::format_for(ui, s))
    }

    /// need to truncate or omit a utf8 char?
    /// use this as padding to avoid resizing mutable strings :3
    pub const IMGUI_ZEROWIDTH_ASCII: u8 = b'\r';

    /// TODO
    pub fn replacements_for<U>(_ui: &mut U) -> &'r FlatMap<char, &'static str>
    where
        U: ?Sized + ImDrawWindow<'r>,
    {
        ImTextDisplay::placeholder_nexus_replacements()
    }
    pub fn placeholder_nexus_replacements() -> &'static FlatMap<char, &'static str> {
        use std::sync::LazyLock;
        static REPLACEMENTS: LazyLock<FlatMap<char, &'static str>> = LazyLock::new(|| {
            IntoIterator::into_iter([('’', "\u{b4}")])
                .map(Into::into)
                .collect()
        });
        &*REPLACEMENTS
    }
}
impl<'r, T> fmt::Display for ImTextDisplay<'r, T>
where
    T: ?Sized + fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use fmt::Write;

        let mut proxy = ImTextDisplay {
            replacements: self.replacements,
            inner: f,
        };
        write!(proxy, "{}", &self.inner)
    }
}
impl<'r, T> fmt::Write for ImTextDisplay<'r, T>
where
    T: ?Sized + fmt::Write,
{
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let any_uni = s.as_bytes().iter().position(|&c| c >= 0x80);
        let (ascii, uni) = match any_uni {
            None => (s, ""),
            Some(start) => unsafe {
                let (ascii, uni) = s.as_bytes().split_at_unchecked(start);
                (str::from_utf8_unchecked(ascii), str::from_utf8_unchecked(uni))
            },
        };
        self.inner.write_str(ascii)?;
        let mut chars = uni.char_indices();
        loop {
            let uni = chars.as_str();
            let start = chars.offset();
            if uni.is_empty() {
                break Ok(())
            }

            let replacement = chars.find_map(|(o, c)| self.replacements.find(&c).map(|r| (o, c, r)));
            let (ok, replacement) = match replacement {
                Some((o, _, r)) => unsafe { (uni.get_unchecked(..o - start), Some(r)) },
                None => (uni, None),
            };
            self.inner.write_str(ok)?;
            let Some(replacement) = replacement else { continue };
            self.inner.write_str(&*replacement)?;
        }
    }
    fn write_char(&mut self, c: char) -> std::fmt::Result {
        if let Some(r) = self.replacements.find(&c) {
            self.inner.write_str(&*r)
        } else {
            self.inner.write_char(c)
        }
    }
}
impl<'r, T> From<ImTextDisplay<'r, T>> for String
where
    ImTextDisplay<'r, T>: fmt::Display,
{
    fn from(display: ImTextDisplay<'r, T>) -> Self {
        display.to_string()
    }
}
impl<'r, T> From<ImTextDisplay<'r, T>> for Arc<str>
where
    ImTextDisplay<'r, T>: fmt::Display,
{
    fn from(display: ImTextDisplay<'r, T>) -> Self {
        String::from(display)[..].into()
    }
}
impl<'r, T> From<ImTextDisplay<'r, T>> for Box<Arc<str>>
where
    ImTextDisplay<'r, T>: fmt::Display,
{
    fn from(display: ImTextDisplay<'r, T>) -> Self {
        Box::new(display.into())
    }
}
impl<'r, T> From<ImTextDisplay<'r, T>> for arcffi::cstr::String0
where
    ImTextDisplay<'r, T>: fmt::Display,
{
    fn from(display: ImTextDisplay<'r, T>) -> Self {
        Self::format(display)
    }
}
impl<'r, T> From<ImTextDisplay<'r, T>> for std::ffi::CString
where
    ImTextDisplay<'r, T>: fmt::Display,
{
    fn from(display: ImTextDisplay<'r, T>) -> Self {
        arcffi::cstr::String0::format(display).into()
    }
}

pub trait IntoImStrDisplay {
    #[inline(always)]
    fn display_imstr(self) -> ImStrDisplay<Self>
    where
        Self: Sized + fmt::Display,
    {
        ImStrDisplay(self)
    }
    #[inline(always)]
    fn display_imstr_ref<'a>(&'a self) -> ImStrDisplay<&'a Self>
    where
        &'a Self: fmt::Display,
    {
        ImStrDisplay(self)
    }
}
impl<T: ?Sized + fmt::Display> IntoImStrDisplay for T {}

macro_rules! im_fmt {
    (move |$($rest:tt)+) => {
        $crate::render::element::im::im_fmt! { @im_display;
            ::taimi_hoard::lazyfmt! {
                move |$($rest)+
            }
        }
    };
    (|$($rest:tt)+) => {
        $crate::render::element::im::im_fmt! { @im_display;
            ::taimi_hoard::lazyfmt! {
                |$($rest)+
            }
        }
    };
    (i18n: $($rest:tt)*) => {
        $crate::render::i18n::i18n_fmt! { $($rest)* }
    };
    /*(i18n&: $id:literal) => {
        $crate::render::element::im::im_fmt! { @im_display;
            ::taimi_hoard::lazyfmt! {
                move |f| $crate::with_i18n!($id, |label| f.write_str(&label))
            }
        }
    };*/
    // internal use...
    (@im_display; $($tt:tt)*) => {
        <dyn ::taimi_ui::im::text::ImStr>::im_display(fmt_args!(
            $($tt)*
        ))
    };
    (=> $fmt:literal $($fargs:tt)*) => {
        $crate::render::element::im::im_fmt! { @im_display;
            $fmt $($fargs)*
        }
    };
    // TODO: may use a scratch buffer or other mechanism in future
    ($fmt:literal $($fargs:tt)*) => {
        format_args! { $fmt $($fargs)* }
    };
}
/// just [`im_fmt!("{thing}")`](im_fmt!)
macro_rules! im_to_string {
    ($thing:expr) => {
        $crate::render::element::im::im_fmt! {
            "{}", $thing
        }
    };
}
pub(crate) use {im_fmt, im_to_string};
