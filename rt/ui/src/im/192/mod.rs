//! [imgui192_sys](self::sys) and [imgui192](self::imgui)

#[cfg(feature = "imgui192-rs")]
pub use ::imgui192::{
    self as imgui,
    internal::{RawCast, RawWrapper},
    Context,
    FontId,
    Io,
    Style,
    StyleColor,
    StyleVar,
    TextureId,
};
pub use {
    self::text::StyleColor,
    ::imgui192_sys::{
        self as sys,
        ImDrawData,
        ImDrawList,
        ImGuiContext,
        ImGuiContextHook,
        ImGuiIO,
        ImGuiPlatformIO,
        ImGuiStyle,
        ImVectorRaw,
    },
};
use {
    super::prelude::*,
    arcffi::{cstr::CStrPtr, nn},
    core::{
        borrow::BorrowMut,
        ffi::{c_char, c_int, c_uint, c_void, CStr},
        marker::PhantomData,
        mem::{self, MaybeUninit},
        num::NonZero,
        ops::RangeInclusive,
        ptr::{self, NonNull},
    },
    glamour::Box2,
};

pub const VERSION_NUM: NonZero<u32> = match <Ui as ImContext>::IMGUI_VERSION_NUM {
    Some(v) => v,
    None => panic!("imgui version 0?"),
};

pub mod text;
#[cfg(not(feature = "imgui192-rs"))]
pub mod imgui {}
pub mod prelude {
    pub use {
        super::{imgui, Ui},
        crate::im::prelude::*,
    };

    #[cfg(feature = "imgui192-rs")]
    pub use super::{
        imgui::{
            ChildWindow,
            ComboBox,
            Condition,
            MouseButton,
            Selectable,
            Slider,
            StyleVar,
            TreeNode,
            TreeNodeFlags,
            Window,
            WindowFlags,
        },
        RawCast,
    };
}

#[cfg(not(feature = "imgui192-rs"))]
pub type Context = ImGuiContext;
#[cfg(not(feature = "imgui192-rs"))]
pub type Style = ImGuiStyle;
//#[cfg(not(feature = "imgui192-rs"))]
pub type Io = ImGuiIO;
//#[cfg(not(feature = "imgui192-rs"))]
pub type PlatformIo = ImGuiPlatformIO;
//#[cfg(not(feature = "imgui192-rs"))]
pub type DrawIo = ImDrawData;
//#[cfg(not(feature = "imgui192-rs"))]
pub type DrawList = ImDrawList;

#[repr(transparent)]
pub struct Ui<'ui>(PhantomData<&'ui ()>);
impl<'ui> Ui<'ui> {
    #[inline(always)]
    pub unsafe fn godmode<'a>(&'_ self) -> &'a mut Self {
        mem::transmute::<&'a mut (), &'a mut Self>(Box::leak(Box::new(())))
    }
    #[inline]
    pub unsafe fn materialize() -> Self {
        Self(PhantomData)
    }
    #[inline]
    pub unsafe fn from_ctx(context: &Context) -> &Self {
        mem::transmute(context as *const _ as *const ())
    }
    #[inline]
    pub unsafe fn from_ctx_mut(context: &mut Context) -> &mut Self {
        mem::transmute(context as *mut _ as *mut ())
    }
}

/// marker struct for conversions
#[derive(Copy, Clone, Default)]
pub struct ImVersion19200;
impl ImVersion19200 {
    #[inline(always)]
    pub fn flags_for<W>(self, args: impl Into<((W, Self), i32)>) -> i32 {
        self.args1_for::<W, i32>(args)
    }
    #[inline(always)]
    pub fn args1_for<W, A0>(self, args: impl Into<((W, Self), A0)>) -> A0 {
        let (_, a0) = args.into();
        a0
    }
    #[inline(always)]
    pub fn args2_for<W, A0, A1>(self, args: impl Into<((W, Self), A0, A1)>) -> (A0, A1) {
        let (_, a0, a1) = args.into();
        (a0, a1)
    }
    #[inline(always)]
    pub fn args3_for<W, A0, A1, A2>(self, args: impl Into<((W, Self), A0, A1, A2)>) -> (A0, A1, A2) {
        let (_, a0, a1, a2) = args.into();
        (a0, a1, a2)
    }
}
impl imw::ChildWindow {
    /// reuse an unassigned [sys::ImGuiWindowFlags_]
    pub const IM192_BORDER_FLAG: u32 = 1u32 << 30;
}
impl imw::PopupModal {
    pub const IM192_FLAGS_PRESET: sys::ImGuiWindowFlags_ = sys::ImGuiWindowFlags_AlwaysAutoResize;
}
impl imw::Table {
    pub const IM192_FLAGS_PRESET: sys::ImGuiTableFlags_ =
        sys::ImGuiTableFlags_Resizable | sys::ImGuiTableFlags_Borders | sys::ImGuiTableFlags_RowBg;
    pub const IM192_ARGS_PRESET: imw::DynArgsTable = imw::DynArgsTable::new(Some(Self::IM192_FLAGS_PRESET));
}
impl imw::TableColumn {
    pub const IM192_WIDTH_STRETCH: sys::ImGuiTableColumnFlags_ = sys::ImGuiTableColumnFlags_WidthStretch;
    pub const IM192_WIDTH_FIXED: sys::ImGuiTableColumnFlags_ = sys::ImGuiTableColumnFlags_WidthFixed;
}
impl imw::TreeNode {
    pub const IM192_ARGS_FRAMED: imw::DynArgsWidget =
        imw::DynArgsWidget::new(Some(sys::ImGuiTreeNodeFlags_Framed));
    pub const IM192_ARGS_FRAMED_NOPUSH: imw::DynArgsWidget = imw::DynArgsWidget::new(Some(
        sys::ImGuiTreeNodeFlags_Framed | sys::ImGuiTreeNodeFlags_NoTreePushOnOpen,
    ));
}
impl imw::ProgressBar {
    pub const IM192_DEFAULT_SIZE: ImSize2 = ImSize2::new(f32::MIN, 0.0);
}
impl imw::InputPassword {
    pub const IM192_FLAGS_PRESET: sys::ImGuiInputTextFlags_ =
        imw::InputText::IM192_FLAGS_PRESET | sys::ImGuiInputTextFlags_Password;
}
impl imw::InputText {
    pub const IM192_MASK_ENTER: sys::ImGuiInputTextFlags =
        sys::ImGuiInputTextFlags_EnterReturnsTrue as sys::ImGuiInputTextFlags;
    /// TODO: consider IsItemDeactivatedAfterEdit() instead of EnterReturnsTrue
    pub const IM192_FLAGS_PRESET: sys::ImGuiInputTextFlags_ = sys::ImGuiInputTextFlags_AutoSelectAll | sys::ImGuiInputTextFlags_EnterReturnsTrue
        //| sys::ImGuiInputTextFlags_AlwaysInsertMode
        | sys::ImGuiInputTextFlags_NoUndoRedo;
    pub const IM192_FLAGS_READ_ONLY: sys::ImGuiInputTextFlags_ = sys::ImGuiInputTextFlags_ReadOnly;
    pub const IM192_ARGS_READ_ONLY: imw::DynArgsInputText =
        imw::DynArgsWidget::new(Some(Self::IM192_FLAGS_PRESET | Self::IM192_FLAGS_READ_ONLY));
}
impl imw::InputTextMultiline {
    pub const IM192_FLAGS_PRESET: sys::ImGuiInputTextFlags_ = 0;
    pub const IM192_DEFAULT_SIZE: ImSize2 = ImSize2::ZERO;
}
impl imw::Selectable {
    pub const IM192_ARGS_NO_DISMISS_POPUP: imw::DynArgsWidgetSized =
        imw::DynArgsWidgetSized::new(Some(sys::ImGuiSelectableFlags_NoAutoClosePopups), None);
}

impl ImIo for ImGuiIO {
    #[inline]
    fn display_size(&self) -> ImSize2<f32> {
        ImSpaces(self.DisplaySize).into()
    }
    #[inline]
    fn display_framebuffer_scale(&self) -> ImVec2<f32> {
        ImSpaces(self.DisplayFramebufferScale).into()
    }
    #[inline]
    fn frame_dt(&self) -> f32 {
        self.DeltaTime
    }
    #[inline]
    fn frame_rate(&self) -> f32 {
        self.Framerate
    }
    #[inline]
    fn want_text_input(&self) -> bool {
        self.WantTextInput
    }
    #[inline]
    fn want_capture_keyboard(&self) -> bool {
        self.WantCaptureKeyboard
    }
    #[inline]
    fn want_capture_mouse(&self) -> bool {
        self.WantCaptureMouse
    }
    #[inline]
    fn button_is_down_untyped(&self, b: usize) -> bool {
        #[cfg(todo = "unnecessary")]
        return unsafe { sys::igIsMouseDown(b as sys::ImGuiMouseButton) };
        self.MouseDown.get(b).copied().unwrap_or(false)
    }
    #[inline]
    fn button_is_pressed_untyped(&self, b: usize) -> bool {
        #[cfg(todo = "unnecessary")]
        return unsafe { sys::igIsMouseClicked_Bool(b as sys::ImGuiMouseButton, false) };
        let Some(true) = self.MouseDown.get(b) else { return false };
        unsafe { *self.MouseDownDuration.get_unchecked(b) == 0.0 }
    }
    #[inline]
    fn key_is_down_untyped(&self, idx: usize) -> bool {
        self.KeysData.get(idx).map(|state| state.Down).unwrap_or(false)
    }
    #[inline]
    fn key_is_pressed_untyped(&self, idx: usize) -> bool {
        #[cfg(todo = "unnecessary")]
        return unsafe { sys::igIsKeyPressed_Bool(idx, false) };
        self.KeysData
            .get(idx)
            .map(|state| state.Down && state.DownDuration == 0.0)
            .unwrap_or(false)
    }
    #[inline]
    /// TODO: more mappings (see also im180)
    fn key_from_alphanum(&self, c: u8) -> usize {
        match c {
            b' ' => sys::ImGuiKey_Space as usize,
            b'\x08' => sys::ImGuiKey_Backspace as usize,
            b'\x7f' => sys::ImGuiKey_Delete as usize,
            b'\t' => sys::ImGuiKey_Tab as usize,
            b'\n' => sys::ImGuiKey_Enter as usize,
            b'\x1b' => sys::ImGuiKey_Escape as usize,
            c => (sys::ImGuiKey_0 as usize + c.to_ascii_uppercase() as usize).wrapping_sub(b'0' as usize),
        }
    }
}
impl ImPlatformIo for ImGuiPlatformIO {}
impl ImDrawIo for ImDrawData {}
impl ImContextHookInfo for ImGuiContextHook {
    #[inline]
    fn id(&self) -> usize {
        self.HookId as usize
    }
    #[inline]
    fn owner(&self) -> usize {
        self.Owner as usize
    }
    #[inline]
    fn hook_type(&self) -> usize {
        self.Type as usize
    }
    #[inline]
    fn raw_callback(&self) -> Option<ContextHookRaw> {
        unsafe {
            mem::transmute::<Option<unsafe extern "C" fn(*mut ImGuiContext, *mut ImGuiContextHook)>, _>(
                self.Callback,
            )
        }
    }
    fn cancel(&mut self) {
        let user_data =
            mem::replace(&mut self.UserData, ptr::null_mut()).cast::<Box<dyn ContextHookCallback>>();
        unsafe {
            if !user_data.is_null() {
                drop(Box::from_raw(user_data));
            }
        }
    }
}
impl ImDrawTarget for ImDrawList {
    fn clip_rect_min(&self) -> ImPos2<ImSpace> {
        unsafe {
            let mut min = MaybeUninit::uninit();
            sys::ImDrawList_GetClipRectMin(min.as_mut_ptr(), self as *const Self as *mut Self);
            ImVec2::from(ImSpaces(min.assume_init())).to_point()
        }
    }
    fn clip_rect_max(&self) -> ImPos2<ImSpace> {
        unsafe {
            let mut max = MaybeUninit::uninit();
            sys::ImDrawList_GetClipRectMin(max.as_mut_ptr(), self as *const Self as *mut Self);
            ImVec2::from(ImSpaces(max.assume_init())).to_point()
        }
    }

    fn add_line(
        &mut self,
        p0: ImPos2<ImSpace>,
        p1: ImPos2<ImSpace>,
        colour: ImColour,
        thickness: Option<f32>,
    ) {
        let thickness = thickness.unwrap_or(1.0);
        let p0 = ImSpaces(p0).into();
        let p1 = ImSpaces(p1).into();
        unsafe { sys::ImDrawList_AddLine(self, p0, p1, ImSpaces(colour).into(), thickness) }
    }
    fn add_rect_untyped(
        &mut self,
        rect: Box2<ImSpace>,
        colour: ImColour,
        rounding: Option<f32>,
        thickness: Option<Option<f32>>,
        flags_untyped: Option<u32>,
    ) {
        let thickness = thickness.map(|t| t.unwrap_or(1.0));
        let flags = flags_untyped.unwrap_or(0) as sys::ImDrawFlags;
        let rounding = rounding.unwrap_or(0.0);
        let [min, max] = ImSpaces(rect).into();
        unsafe {
            match thickness {
                None =>
                    sys::ImDrawList_AddRectFilled(self, min, max, ImSpaces(colour).into(), rounding, flags),
                Some(thickness) => sys::ImDrawList_AddRect(
                    self,
                    min,
                    max,
                    ImSpaces(colour).into(),
                    rounding,
                    flags,
                    thickness,
                ),
            }
        }
    }
    fn add_quad(&mut self, points: [ImPos2<ImSpace>; 4], colour: ImColour, thickness: Option<Option<f32>>) {
        let thickness = thickness.map(|t| t.unwrap_or(1.0));
        let [p1, p2, p3, p4] = ImSpaces(points).into();
        unsafe {
            match thickness {
                None => sys::ImDrawList_AddQuadFilled(self, p1, p2, p3, p4, ImSpaces(colour).into()),
                Some(thickness) =>
                    sys::ImDrawList_AddQuad(self, p1, p2, p3, p4, ImSpaces(colour).into(), thickness),
            }
        }
    }
    fn add_triangle(
        &mut self,
        points: [ImPos2<ImSpace>; 3],
        colour: ImColour,
        thickness: Option<Option<f32>>,
    ) {
        let thickness = thickness.map(|t| t.unwrap_or(1.0));
        let [p1, p2, p3] = ImSpaces(points).into();
        unsafe {
            match thickness {
                None => sys::ImDrawList_AddTriangleFilled(self, p1, p2, p3, ImSpaces(colour).into()),
                Some(thickness) =>
                    sys::ImDrawList_AddTriangle(self, p1, p2, p3, ImSpaces(colour).into(), thickness),
            }
        }
    }
    fn add_circle(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        thickness: Option<Option<f32>>,
    ) {
        let thickness = thickness.map(|t| t.unwrap_or(1.0));
        let segments = segments.unwrap_or(0) as c_int;
        let mid = ImSpaces(mid).into();
        unsafe {
            match thickness {
                None =>
                    sys::ImDrawList_AddCircleFilled(self, mid, radius, ImSpaces(colour).into(), segments),
                Some(thickness) => sys::ImDrawList_AddCircle(
                    self,
                    mid,
                    radius,
                    ImSpaces(colour).into(),
                    segments,
                    thickness,
                ),
            }
        }
    }
    fn add_ngon(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        thickness: Option<Option<f32>>,
    ) {
        let thickness = thickness.map(|t| t.unwrap_or(1.0));
        let segments = segments.unwrap_or(0) as c_int;
        let mid = ImSpaces(mid).into();
        unsafe {
            match thickness {
                None => sys::ImDrawList_AddNgonFilled(self, mid, radius, ImSpaces(colour).into(), segments),
                Some(thickness) =>
                    sys::ImDrawList_AddNgon(self, mid, radius, ImSpaces(colour).into(), segments, thickness),
            }
        }
    }
    fn add_ellipse(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: ImVec2<ImSpace>,
        colour: ImColour,
        rot: Option<f32>,
        segments: Option<u32>,
        thickness: Option<Option<f32>>,
    ) {
        let thickness = thickness.map(|t| t.unwrap_or(1.0));
        let segments = segments.unwrap_or(0) as c_int;
        let mid = ImSpaces(mid).into();
        let rot = rot.unwrap_or(0.0);
        let radius = ImSpaces(radius).into();
        unsafe {
            match thickness {
                None => sys::ImDrawList_AddEllipseFilled(
                    self,
                    mid,
                    radius,
                    ImSpaces(colour).into(),
                    rot,
                    segments,
                ),
                Some(thickness) => sys::ImDrawList_AddEllipse(
                    self,
                    mid,
                    radius,
                    ImSpaces(colour).into(),
                    rot,
                    segments,
                    thickness,
                ),
            }
        }
    }
}
unsafe fn get_style_size2(offset: usize) -> ImSize2 {
    let style = sys::igGetStyle() as *const ImGuiStyle as *const sys::ImVec2;
    *style.byte_add(offset).cast::<ImSize2>()
}
unsafe fn get_style_f32(offset: usize) -> f32 {
    let style = sys::igGetStyle() as *const ImGuiStyle as *const f32;
    *style.byte_add(offset)
}
impl ImStyle for ImGuiStyle {
    #[inline]
    fn indent_spacing(&self) -> f32 {
        unsafe { get_style_f32(mem::offset_of!(ImGuiStyle, IndentSpacing)) }
    }
    #[inline]
    fn item_spacing(&self) -> ImSize2 {
        unsafe { get_style_size2(mem::offset_of!(ImGuiStyle, ItemSpacing)) }
    }
    #[inline]
    fn frame_padding(&self) -> ImSize2 {
        unsafe { get_style_size2(mem::offset_of!(ImGuiStyle, FramePadding)) }
    }
}
impl<'ui> ImContext for Ui<'ui> {
    const IMGUI_VERSION_NUM: Option<NonZero<u32>> = NonZero::new(sys::IMGUI_VERSION_NUM as u32);
    const IMGUI_VERSION_NAME: Option<&'static CStr> = Some(sys::IMGUI_VERSION);
    type Context = ImGuiContext;
    type Io = ImGuiIO;
    type PlatformIo = ImGuiPlatformIO;
    type DrawIo = ImDrawData;
    type DrawList = ImDrawList;
    type Style = ImGuiStyle;
    #[inline]
    fn get_context_ptr(&self) -> NonNull<Self::Context> {
        unsafe { NonNull::new_unchecked(sys::igGetCurrentContext()) }
    }
    #[inline]
    fn get_style_ptr(&self) -> NonNull<Self::Style> {
        unsafe { NonNull::new_unchecked(sys::igGetStyle()).cast() }
    }
    #[inline]
    fn get_draw_ptr(&self) -> NonNull<Self::DrawIo> {
        unsafe {
            match sys::igGetDrawData() {
                #[cfg(debug_assertions)]
                data => NonNull::new(data).expect("mid-frame"),
                #[cfg(not(debug_assertions))]
                data => NonNull::new_unchecked(data),
            }
            .cast()
        }
    }
    #[inline]
    fn get_draw_fg_ptr(&self) -> NonNull<Self::DrawList> {
        unsafe { NonNull::new_unchecked(sys::igGetForegroundDrawList()).cast() }
    }
    #[inline]
    fn get_draw_bg_ptr(&self) -> NonNull<Self::DrawList> {
        unsafe { NonNull::new_unchecked(sys::igGetBackgroundDrawList()).cast() }
    }
    #[inline]
    fn get_io_ptr(&self) -> NonNull<Self::Io> {
        unsafe { NonNull::new_unchecked(sys::igGetIO()).cast() }
    }
    #[inline]
    fn get_pio_ptr(&self) -> NonNull<Self::PlatformIo> {
        unsafe { NonNull::new_unchecked(sys::igGetPlatformIO()).cast() }
    }
}
impl<'ui> AsUi<'ui, Ui<'ui>> for Ui<'ui> {
    #[inline(always)]
    fn ui(&self) -> &Ui<'ui> {
        self
    }
}
impl<'ui> AsUi<'ui, Ui<'ui>> for Context {
    #[inline(always)]
    fn ui(&self) -> &Ui<'ui> {
        unsafe { Ui::from_ctx(self) }
    }

    #[inline(always)]
    unsafe fn get_style_ref(&self) -> &Style {
        &self.Style
    }
    #[inline(always)]
    unsafe fn get_io_ref(&self) -> &Io {
        &self.IO
    }
}
unsafe impl ImUiContext for ImGuiContext {
    #[inline(always)]
    fn get_style_ptr_dyn(&self) -> NonNull<dyn ImStyle> {
        unsafe {
            match () {
                #[cfg(feature = "imgui192-imp")]
                _ => NonNull::new_unchecked(&raw const self.Style as *mut ImGuiStyle as *mut dyn ImStyle),
                #[cfg(not(feature = "imgui192-imp"))]
                _ => Ui::materialize().get_style_ptr_dyn(),
            }
        }
    }
    #[inline(always)]
    fn get_draw_ptr_dyn(&self) -> NonNull<dyn ImDrawIo> {
        unsafe {
            match () {
                #[cfg(feature = "imgui192-imp")]
                #[cfg(todo)]
                _ => NonNull::new_unchecked(
                    &raw const self.MainViewport.DrawData as *mut ImDrawData as *mut dyn ImDrawIo,
                ),
                //#[cfg(not(feature = "imgui192-imp"))]
                _ => Ui::materialize().get_draw_ptr_dyn(),
            }
        }
    }
    #[inline(always)]
    fn get_draw_fg_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget> {
        unsafe {
            match () {
                #[cfg(feature = "imgui192-imp")]
                #[cfg(todo)]
                _ => (),
                //#[cfg(not(feature = "imgui192-imp"))]
                _ => Ui::materialize().get_draw_fg_ptr_dyn(),
            }
        }
    }
    #[inline(always)]
    fn get_draw_bg_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget> {
        unsafe {
            match () {
                #[cfg(feature = "imgui192-imp")]
                #[cfg(todo)]
                _ => (),
                //#[cfg(not(feature = "imgui192-imp"))]
                _ => Ui::materialize().get_draw_bg_ptr_dyn(),
            }
        }
    }
    #[inline(always)]
    fn get_io_ptr_dyn(&self) -> NonNull<dyn ImIo> {
        unsafe {
            match () {
                #[cfg(feature = "imgui192-imp")]
                _ => NonNull::new_unchecked(&raw const self.IO as *mut ImGuiIO as *mut dyn ImIo),
                #[cfg(not(feature = "imgui192-imp"))]
                _ => Ui::materialize().get_io_ptr_dyn(),
            }
        }
    }
    #[inline(always)]
    fn get_pio_ptr_dyn(&self) -> NonNull<dyn ImPlatformIo> {
        unsafe {
            match () {
                #[cfg(feature = "imgui192-imp")]
                _ => NonNull::new_unchecked(
                    &raw const self.PlatformIO as *mut ImGuiPlatformIO as *mut dyn ImPlatformIo,
                ),
                #[cfg(not(feature = "imgui192-imp"))]
                _ => Ui::materialize().get_pio_ptr_dyn(),
            }
        }
    }
    #[inline(always)]
    fn imgui_version_num(&self) -> Option<NonZero<u32>> {
        Ui::IMGUI_VERSION_NUM
    }
    #[inline(always)]
    fn imgui_version_name(&self) -> Option<CStrPtr<'_>> {
        Ui::IMGUI_VERSION_NAME.map(CStrPtr::with_cstr)
    }
}
unsafe impl ImContextState for ImGuiContext {
    #[inline]
    fn get_ptr(&self) -> NonNull<c_void> {
        nn::nonnull_ref(self).cast()
    }

    #[inline]
    fn is_bound(&self) -> bool {
        let current = unsafe { sys::igGetCurrentContext() };
        current as *const Self == self as *const Self
    }

    #[inline]
    unsafe fn unbind_unchecked(&mut self) {
        sys::igSetCurrentContext(ptr::null_mut())
    }
    #[inline]
    unsafe fn bind_unchecked(&mut self) {
        sys::igSetCurrentContext(self)
    }
    #[inline]
    unsafe fn bind_allocator(
        &mut self,
        malloc: Option<UserMallocFn>,
        free: Option<UserFreeFn>,
        userdata: *mut c_void,
    ) {
        sys::igSetAllocatorFunctions(malloc, free, userdata)
    }
    #[inline]
    fn get_bound_allocator(&self) -> UiAllocatorFns {
        unsafe {
            let mut out = MaybeUninit::<UiAllocatorFns>::uninit();
            sys::igGetAllocatorFunctions(
                &raw mut (*out.as_mut_ptr()).0,
                &raw mut (*out.as_mut_ptr()).1,
                &raw mut (*out.as_mut_ptr()).2,
            );
            out.assume_init()
        }
    }

    type BoundContext<'a, 'ui: 'a> = Ui<'ui>;
    #[inline]
    unsafe fn bound_mut<'a, 'ui>(&'a mut self) -> &'a mut Self::BoundContext<'a, 'ui>
    where
        'ui: 'a,
    {
        Ui::from_ctx_mut(self)
    }

    /// TODO: do something with "owner" field? idk if input or arg from backend or what...
    #[inline]
    unsafe fn add_hook_boxed(
        &mut self,
        hook: Box<dyn ContextHookCallback>,
        type_untyped: u32,
        owner: usize,
    ) -> Option<NonZero<usize>> {
        unsafe {
            let ctx = self.get_ptr().cast::<ImGuiContext>().as_ptr();
            let hook = Box::into_raw(Box::new(hook));
            let request = ImGuiContextHook {
                Type: type_untyped as sys::ImGuiContextHookType,
                Owner: owner as sys::ImGuiID,
                Callback: Some(im192_context_hook_callback),
                UserData: hook.cast::<c_void>(),
                ..ImGuiContextHook::default()
            };
            let id = NonZero::new(sys::igAddContextHook(ctx, &request) as usize);
            #[cfg(debug_assertions)]
            if id.is_none() {
                // failure shouldn't really happen...
                drop(Box::from_raw(hook))
            }
            id
        }
    }
    #[inline]
    unsafe fn remove_hook_by_id(&mut self, id: NonZero<usize>) {
        let id = id.get() as sys::ImGuiID;
        unsafe {
            let ctx = self.get_ptr().cast::<ImGuiContext>().as_ptr();
            let hook = {
                let hooks = &*&raw const (*ctx).Hooks;
                hooks
                    .data()
                    .iter()
                    .find(|hook| hook.HookId == id)
                    .map(|hook| hook.UserData as *mut Box<dyn ContextHookCallback>)
            };
            let () = sys::igRemoveContextHook(ctx, id);
            if let Some(hook) = hook {
                drop(Box::from_raw(hook));
            }
        }
    }
}
unsafe extern "C" fn im192_context_hook_callback(ctx: *mut ImGuiContext, hook: *mut ImGuiContextHook) {
    #[cfg(debug_assertions)]
    if hook.is_null() {
        return
    }
    let hook = &mut *hook;
    let Some(user_data) = NonNull::new(hook.UserData) else { return };
    let cb = user_data.cast::<Box<dyn ContextHookCallback>>();
    let hook_type = *&raw const (*hook).Type;
    let hook_id = hook.HookId as usize;
    let ctx = &mut *ctx;
    (*cb.as_ptr()).call_hook_dyn(ctx, hook_id, hook_type, hook)
}
unsafe impl<'ui> ImUiContext for Ui<'ui> {
    #[inline(always)]
    fn get_style_ptr_dyn(&self) -> NonNull<dyn ImStyle> {
        unsafe { NonNull::new_unchecked(self.get_style_ptr().as_ptr() as *mut dyn ImStyle) }
    }
    #[inline(always)]
    fn get_draw_ptr_dyn(&self) -> NonNull<dyn ImDrawIo> {
        unsafe { NonNull::new_unchecked(self.get_draw_ptr().as_ptr() as *mut dyn ImDrawIo) }
    }
    #[inline(always)]
    fn get_draw_fg_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget> {
        unsafe { NonNull::new_unchecked(self.get_draw_fg_ptr().as_ptr() as *mut dyn ImDrawTarget) }
    }
    #[inline(always)]
    fn get_draw_bg_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget> {
        unsafe { NonNull::new_unchecked(self.get_draw_bg_ptr().as_ptr() as *mut dyn ImDrawTarget) }
    }
    #[inline(always)]
    fn get_io_ptr_dyn(&self) -> NonNull<dyn ImIo> {
        unsafe { NonNull::new_unchecked(self.get_io_ptr().as_ptr() as *mut dyn ImIo) }
    }
    #[inline(always)]
    fn get_pio_ptr_dyn(&self) -> NonNull<dyn ImPlatformIo> {
        unsafe { NonNull::new_unchecked(self.get_pio_ptr().as_ptr() as *mut dyn ImPlatformIo) }
    }
    #[inline(always)]
    fn imgui_version_num(&self) -> Option<NonZero<u32>> {
        Self::IMGUI_VERSION_NUM
    }
    #[inline(always)]
    fn imgui_version_name(&self) -> Option<CStrPtr<'_>> {
        Self::IMGUI_VERSION_NAME.map(CStrPtr::with_cstr)
    }
}
unsafe impl<'ui> ImUi for Ui<'ui> {
    #[inline(always)]
    fn get_context_ptr_dyn(&self) -> NonNull<dyn ImContextState> {
        unsafe { NonNull::new_unchecked(self.get_context_ptr().as_ptr() as *mut dyn ImContextState) }
    }
}
impl<'ui> ImFrameArena<'ui> for Ui<'ui> {}
impl<'ui> ImUiWindow for Ui<'ui> {
    fn get_draw_target_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget> {
        unsafe { NonNull::new_unchecked(sys::igGetWindowDrawList() as *mut dyn ImDrawTarget) }
    }
    fn viewport_framebuffer_scale(&self) -> ImVec2<f32> {
        ImSpaces(unsafe { (&*sys::igGetWindowViewport()).FramebufferScale }).into()
    }
    fn viewport_font_scale(&self) -> f32 {
        unsafe {
            let style = sys::igGetStyle();
            let scale_main = *&raw const (*style).FontScaleMain;
            let scale_dpi = *&raw const (*style).FontScaleDpi;
            let scale = scale_main * scale_dpi;
            match () {
                #[cfg(feature = "imgui192-obsolete")]
                _ => {
                    let scale_global = *(&raw const (*sys::igGetIO()).FontGlobalScale);
                    let scale_font =
                        NonNull::new(sys::igGetFont()).map(|font| *(&raw const (*font.as_ptr()).Scale));
                    scale * scale_global * scale_font.unwrap_or(1.0f32)
                },
                #[cfg(not(feature = "imgui192-obsolete"))]
                _ => scale,
            }
        }
    }
    fn font_scale(&self) -> f32 {
        let window_scale = unsafe {
            match () {
                #[cfg(feature = "imgui192-imp")]
                _ => (&*sys::igGetCurrentWindow()).FontWindowScale,
                #[cfg(not(feature = "imgui192-imp"))]
                _ => 1.0f32,
            }
        };
        window_scale * self.viewport_font_scale()
    }

    fn cursor_window_pos(&self) -> ImPos2<WindowSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetCursorPos() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetCursorPos(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    fn cursor_start_pos(&self) -> ImPos2<WindowSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetCursorStartPos() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetCursorStartPos(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    fn cursor_screen_pos(&self) -> ImPos2<ImSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetCursorScreenPos() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetCursorScreenPos(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    fn scroll_offset(&self) -> ImVec2<WindowSpace> {
        unsafe { ImVec2::new(sys::igGetScrollX(), sys::igGetScrollY()) }
    }
    fn window_pos(&self) -> ImPos2<ImSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetWindowPos() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetWindowPos(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    fn window_size(&self) -> ImSize2<ImSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetWindowSize() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetWindowSize(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_size()
            .cast()
    }
    #[cfg(todo)]
    #[cfg(feature = "imgui192-imp")]
    fn window_flags(&self, mask: InteractSignal) -> InteractSignal {}
    fn item_rect_min(&self) -> ImPos2<ImSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetItemRectMin() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetItemRectMin(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    fn item_rect_max(&self) -> ImPos2<ImSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetItemRectMax() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetItemRectMax(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    fn item_rect_size(&self) -> ImSize2<ImSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetItemRectSize() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetItemRectSize(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_size()
            .cast()
    }
    #[cfg(feature = "imgui192-obsolete")]
    fn content_region_max(&self) -> ImPos2<WindowSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetContentRegionMax() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetContentRegionMax(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    #[cfg(not(feature = "imgui192-obsolete"))]
    fn content_region_max(&self) -> ImPos2<WindowSpace> {
        self.units()
            .map(self.cursor_screen_pos() - self.window_pos().to_vector())
            + self.content_region_avail().to_vector()
    }
    fn content_region_avail(&self) -> ImSize2<WindowSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetContentRegionAvail() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetContentRegionAvail(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_size()
            .cast()
    }
    #[cfg(feature = "imgui192-obsolete")]
    fn window_content_region_min(&self) -> ImPos2<WindowSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetWindowContentRegionMin() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetWindowContentRegionMin(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    #[cfg(feature = "imgui192-obsolete")]
    fn window_content_region_max(&self) -> ImPos2<WindowSpace> {
        self.units()
            .map(match () {
                #[cfg(feature = "cimgui-struct-return")]
                _ => unsafe { sys::igGetWindowContentRegionMax() },
                #[cfg(not(feature = "cimgui-struct-return"))]
                _ => unsafe {
                    let mut out = MaybeUninit::uninit();
                    let () = sys::igGetWindowContentRegionMax(out.as_mut_ptr());
                    out.assume_init()
                },
            })
            .to_point()
            .cast()
    }
    #[cfg(not(feature = "imgui192-obsolete"))]
    fn window_content_region_min(&self) -> ImPos2<WindowSpace> {
        ImPos2::ZERO
    }
    #[cfg(not(feature = "imgui192-obsolete"))]
    fn window_content_region_max(&self) -> ImPos2<WindowSpace> {
        self.window_size().to_vector().to_point().cast()
    }

    #[inline]
    fn item_is_clicked_with(&self, button_id: u32) -> bool {
        unsafe { sys::igIsItemClicked(button_id as sys::ImGuiMouseButton) }
    }
    #[inline]
    fn item_is_active(&self) -> bool {
        unsafe { sys::igIsItemActive() }
    }
    #[inline]
    fn item_is_focused(&self) -> bool {
        unsafe { sys::igIsItemFocused() }
    }
    #[inline]
    fn item_is_visible(&self) -> bool {
        unsafe { sys::igIsItemVisible() }
    }
    #[inline]
    fn item_is_hovered_untyped(&self, flags: Option<u32>) -> bool {
        const WINDOW_EXCLUSIVE_FLAGS: u32 = match () {
            #[cfg(feature = "imgui192-imp")]
            _ =>
                sys::ImGuiHoveredFlags_AllowedMaskForIsWindowHovered
                    & !sys::ImGuiHoveredFlags_AllowedMaskForIsItemHovered,
            #[cfg(not(feature = "imgui192-imp"))]
            _ =>
                sys::ImGuiHoveredFlags_AnyWindow
                | sys::ImGuiHoveredFlags_ChildWindows
                | sys::ImGuiHoveredFlags_RootAndChildWindows
                //| sys::ImGuiFocusedFlags_AllowWhenBlockedByActiveItem // not exclusive
                | sys::ImGuiFocusedFlags_NoPopupHierarchy,
        } as u32;
        let flags = match flags {
            Some(f) if f & WINDOW_EXCLUSIVE_FLAGS != 0 => return self.window_is_hovered_untyped(flags),
            Some(f) => f as sys::ImGuiHoveredFlags,
            None => 0,
        };
        unsafe { sys::igIsItemHovered(flags) }
    }
    #[inline]
    fn item_is_edited(&self) -> bool {
        unsafe { sys::igIsItemEdited() }
    }
    #[inline]
    fn item_was_activated(&self) -> bool {
        unsafe { sys::igIsItemActivated() }
    }
    #[inline]
    fn item_was_deactivated(&self) -> bool {
        unsafe { sys::igIsItemDeactivated() }
    }
    #[inline]
    fn item_was_deactivated_after_edit(&self) -> bool {
        unsafe { sys::igIsItemDeactivatedAfterEdit() }
    }
    #[inline]
    fn item_was_toggled_open(&self) -> bool {
        unsafe { sys::igIsItemToggledOpen() }
    }
    #[inline]
    fn item_any_hovered(&self) -> bool {
        unsafe { sys::igIsAnyItemHovered() }
    }
    #[inline]
    fn item_any_active(&self) -> bool {
        unsafe { sys::igIsAnyItemActive() }
    }
    #[inline]
    fn item_any_focused(&self) -> bool {
        unsafe { sys::igIsAnyItemFocused() }
    }
    #[inline]
    fn window_is_appearing(&self) -> bool {
        unsafe { sys::igIsWindowAppearing() }
    }
    #[inline]
    fn window_is_focused_untyped(&self, flags: Option<u32>) -> bool {
        let flags =
            flags.unwrap_or(sys::ImGuiFocusedFlags_RootAndChildWindows as u32) as sys::ImGuiFocusedFlags;
        unsafe { sys::igIsWindowFocused(flags) }
    }
    #[inline]
    fn window_is_hovered_untyped(&self, flags: Option<u32>) -> bool {
        let flags =
            flags.unwrap_or(sys::ImGuiHoveredFlags_RootAndChildWindows as u32) as sys::ImGuiHoveredFlags;
        unsafe { sys::igIsWindowHovered(flags) }
    }
    #[inline]
    fn window_is_collapsed(&self) -> bool {
        unsafe { sys::igIsWindowCollapsed() }
    }
}
impl<'ui> ImDraw for &'_ Ui<'ui> {
    #[inline]
    fn new_line(&mut self) {
        unsafe { sys::igNewLine() }
    }
    #[inline]
    fn same_line(&mut self) {
        self.same_line_with(None, None)
    }
    #[inline]
    fn same_line_with(&mut self, offset: Option<f32>, spacing: Option<f32>) {
        unsafe { sys::igSameLine(offset.unwrap_or(0.0), spacing.unwrap_or(-1.0)) }
    }
    #[inline]
    fn move_cursor(&mut self, pos: ImPos2<WindowSpace>) {
        unsafe { sys::igSetCursorPos(ImSpaces(pos).into()) }
    }
    #[inline]
    fn move_cursor_screen(&mut self, pos: ImPos2<ImSpace>) {
        unsafe { sys::igSetCursorScreenPos(ImSpaces(pos).into()) }
    }
    #[inline]
    fn dummy_space(&mut self, size: ImSize2) {
        unsafe { sys::igDummy(ImSpaces(size).into()) }
    }
    #[inline]
    fn spacing(&mut self) {
        unsafe { sys::igSpacing() }
    }
    #[inline]
    fn separator(&mut self) {
        unsafe { sys::igSeparator() }
    }
    #[inline]
    fn bullet(&mut self) {
        unsafe { sys::igBullet() }
    }
    #[inline]
    fn indent_by(&mut self, amt: Option<f32>) {
        unsafe { sys::igIndent(amt.unwrap_or(0.0)) }
    }
    #[inline]
    fn unindent_by(&mut self, amt: Option<f32>) {
        unsafe { sys::igUnindent(amt.unwrap_or(0.0)) }
    }
    #[inline]
    fn set_clipboard_text_dyn(&mut self, text: &mut dyn ImStr) {
        <dyn ImStr>::with_cstr(text, move |text| unsafe {
            sys::igSetClipboardText(text.as_ptr())
        })
    }
    fn with_clipboard_text_dyn(&mut self, out: &mut dyn FnMut(&mut dyn ImStr) -> usize) -> usize {
        let mut text = unsafe {
            NonNull::new(sys::igGetClipboardText() as *mut _)
                .map(|p| CStr::from_ptr(p.as_ptr()))
                .unwrap_or(c"")
        };

        out(&mut text)
    }
    #[inline]
    fn item_prepare_open(&mut self, open: bool, cond: ImCondition) {
        let ((imw::Window, ImVersion19200), cond) = cond.into();
        unsafe { sys::igSetNextItemOpen(open, cond) }
    }
    #[inline]
    fn item_prepare_width(&mut self, width: f32) {
        unsafe {
            sys::igSetNextItemWidth(width);
        }
    }
    #[inline]
    fn item_prepare_focus(&mut self, offset: isize) {
        unsafe {
            sys::igSetKeyboardFocusHere(offset as c_int);
        }
    }
}
impl<'ui> ImDraw for Ui<'ui> {
    #[inline(always)]
    fn new_line(&mut self) {
        ImDraw::new_line(&mut &*self)
    }
    #[inline(always)]
    fn same_line(&mut self) {
        ImDraw::same_line(&mut &*self)
    }
    #[inline(always)]
    fn same_line_with(&mut self, offset: Option<f32>, spacing: Option<f32>) {
        ImDraw::same_line_with(&mut &*self, offset, spacing)
    }
    #[inline(always)]
    fn move_cursor(&mut self, pos: ImPos2<WindowSpace>) {
        ImDraw::move_cursor(&mut &*self, pos)
    }
    #[inline(always)]
    fn move_cursor_screen(&mut self, pos: ImPos2<ImSpace>) {
        ImDraw::move_cursor_screen(&mut &*self, pos)
    }
    #[inline(always)]
    fn dummy_space(&mut self, size: ImSize2) {
        ImDraw::dummy_space(&mut &*self, size)
    }
    #[inline(always)]
    fn spacing(&mut self) {
        ImDraw::spacing(&mut &*self)
    }
    #[inline(always)]
    fn separator(&mut self) {
        ImDraw::separator(&mut &*self)
    }
    #[inline(always)]
    fn bullet(&mut self) {
        ImDraw::bullet(&mut &*self)
    }
    #[inline(always)]
    fn indent_by(&mut self, amt: Option<f32>) {
        ImDraw::indent_by(&mut &*self, amt)
    }
    #[inline(always)]
    fn unindent_by(&mut self, amt: Option<f32>) {
        ImDraw::unindent_by(&mut &*self, amt)
    }
    #[inline(always)]
    fn set_clipboard_text_dyn(&mut self, text: &mut dyn ImStr) {
        ImDraw::set_clipboard_text_dyn(&mut &*self, text)
    }
    #[inline(always)]
    fn with_clipboard_text_dyn(&mut self, out: &mut dyn FnMut(&mut dyn ImStr) -> usize) -> usize {
        ImDraw::with_clipboard_text_dyn(&mut &*self, out)
    }
    #[inline(always)]
    fn item_prepare_open(&mut self, open: bool, cond: ImCondition) {
        ImDraw::item_prepare_open(&mut &*self, open, cond)
    }
    #[inline(always)]
    fn item_prepare_width(&mut self, width: f32) {
        ImDraw::item_prepare_width(&mut &*self, width)
    }
    #[inline(always)]
    fn item_prepare_focus(&mut self, offset: isize) {
        ImDraw::item_prepare_focus(&mut &*self, offset)
    }
}
impl<'ui> ImTable for &'_ Ui<'ui> {
    #[inline]
    fn table_current_row(&self) -> u32 {
        unsafe { sys::igTableGetRowIndex() as _ }
    }
    #[inline]
    fn table_current_column(&self) -> u32 {
        unsafe { sys::igTableGetColumnIndex() as _ }
    }
    #[inline]
    fn table_column_count(&self) -> u32 {
        unsafe { sys::igTableGetColumnCount() as _ }
    }
    #[inline]
    fn table_column_name(&self, column: u32) -> Option<CStrPtr<'_>> {
        unsafe { CStrPtr::from_ptr(sys::igTableGetColumnName(column as _)) }
    }
    #[inline]
    fn table_column_set_width(&mut self, column: u32, width: f32) {
        unsafe { sys::igTableSetColumnWidth(column as _, width) }
    }
    #[inline]
    fn table_advance_column(&mut self, column: u32) -> bool {
        unsafe { sys::igTableSetColumnIndex(column as _) }
    }
    #[inline]
    fn table_header_row(&mut self) {
        unsafe { sys::igTableHeadersRow() }
    }
    #[inline]
    fn table_header_height(&self) -> f32 {
        unsafe { sys::igTableGetHeaderRowHeight() }
    }
    #[inline]
    fn table_column_setup_dyn_untyped(
        &mut self,
        mut name: Option<&mut dyn ImStr>,
        untyped_flags: Option<u32>,
        init_size: Option<f32>,
        user_id: u32,
    ) {
        let name = name.as_mut().map(|n| n.im_take_cstring());
        let name = name.as_ref().map(|p| p.as_ptr()).unwrap_or(ptr::null());
        let init_width_or_height = init_size.unwrap_or(0.0);
        let flags = untyped_flags.unwrap_or(0) as sys::ImGuiTableFlags;
        unsafe { sys::igTableSetupColumn(name, flags, init_width_or_height, user_id) }
    }
    #[inline]
    fn table_next_column(&mut self) -> bool {
        unsafe { sys::igTableNextColumn() }
    }
    #[inline]
    fn table_next_row_with(&mut self, min_height: Option<f32>) {
        self.table_next_row_untyped(None, min_height)
    }
    #[inline]
    fn table_next_row_untyped(&mut self, flags: Option<u32>, min_height: Option<f32>) {
        let height = min_height.unwrap_or(0.0);
        let flags = flags.unwrap_or(0) as sys::ImGuiTableFlags;
        unsafe { sys::igTableNextRow(flags, height) }
    }
    #[inline]
    fn table_header_dyn(&mut self, text: &mut dyn ImStr) {
        <dyn ImStr>::with_cstr(text, move |text| unsafe { sys::igTableHeader(text.as_ptr()) })
    }
}
impl<'ui> ImTable for Ui<'ui> {
    #[inline(always)]
    fn table_current_row(&self) -> u32 {
        ImTable::table_current_row(&self)
    }
    #[inline(always)]
    fn table_current_column(&self) -> u32 {
        ImTable::table_current_column(&self)
    }
    #[inline(always)]
    fn table_column_count(&self) -> u32 {
        ImTable::table_column_count(&self)
    }
    #[inline(always)]
    fn table_column_name(&self, column: u32) -> Option<CStrPtr<'_>> {
        ImTable::table_column_name(&self, column).map(|p| unsafe { p.immortal() })
    }
    #[inline(always)]
    fn table_column_set_width(&mut self, column: u32, width: f32) {
        ImTable::table_column_set_width(&mut &*self, column, width)
    }
    #[inline(always)]
    fn table_advance_column(&mut self, column: u32) -> bool {
        ImTable::table_advance_column(&mut &*self, column)
    }
    #[inline(always)]
    fn table_header_row(&mut self) {
        ImTable::table_header_row(&mut &*self)
    }
    #[inline(always)]
    fn table_header_height(&self) -> f32 {
        ImTable::table_header_height(&self)
    }
    #[inline(always)]
    fn table_column_setup_dyn_untyped(
        &mut self,
        name: Option<&mut dyn ImStr>,
        untyped_flags: Option<u32>,
        init_size: Option<f32>,
        user_id: u32,
    ) {
        ImTable::table_column_setup_dyn_untyped(&mut &*self, name, untyped_flags, init_size, user_id)
    }
    #[inline(always)]
    fn table_next_column(&mut self) -> bool {
        ImTable::table_next_column(&mut &*self)
    }
    #[inline(always)]
    fn table_next_row_with(&mut self, min_height: Option<f32>) {
        ImTable::table_next_row_with(&mut &*self, min_height)
    }
    #[inline(always)]
    fn table_next_row_untyped(&mut self, flags: Option<u32>, min_height: Option<f32>) {
        ImTable::table_next_row_untyped(&mut &*self, flags, min_height)
    }
    #[inline(always)]
    fn table_header_dyn(&mut self, text: &mut dyn ImStr) {
        ImTable::table_header_dyn(&mut &*self, text)
    }
}
impl<'ui> ImTableLegacy for &'_ Ui<'ui> {
    fn table_legacy_columns_dyn(&mut self, count: u32, ident: &mut dyn ImStr, border: bool) {
        <dyn ImStr>::with_cstr(ident, move |ident| unsafe {
            sys::igColumns(count as c_int, ident.as_ptr(), border)
        })
    }
    fn table_legacy_columns_next(&mut self) {
        unsafe { sys::igNextColumn() }
    }
}
impl<'ui> ImTableLegacy for Ui<'ui> {
    #[inline(always)]
    fn table_legacy_columns_dyn(&mut self, count: u32, ident: &mut dyn ImStr, border: bool) {
        ImTableLegacy::table_legacy_columns_dyn(&mut &*self, count, ident, border)
    }
    #[inline(always)]
    fn table_legacy_columns_next(&mut self) {
        ImTableLegacy::table_legacy_columns_next(&mut &*self)
    }
}
impl<'ui> ImTableStack<'ui> for &'_ Ui<'ui> {
    #[inline]
    fn begin_table_dyn_untyped(
        &mut self,
        ident: &mut dyn ImStr,
        columns: u32,
        untyped_flags: Option<u32>,
        outer_size: Option<ImSize2>,
        inner_width: Option<f32>,
    ) -> Option<UiTokenDyn<'ui>> {
        let flags = untyped_flags.unwrap_or(0);
        let outer_size = outer_size.unwrap_or(ImSize2::ZERO);
        let width = inner_width.unwrap_or(0.0);
        <dyn ImStr>::with_cstr(ident, move |ident| unsafe {
            let size = ImSpaces(outer_size).into();
            let flags = flags as sys::ImGuiTableFlags;
            let cols = columns as c_int;
            sys::igBeginTable(ident.as_ptr(), cols, flags, size, width)
                .then(|| UiTokenFn::new_fn_item(&mut im192_container_end_table))
        })
    }
}
impl<'ui> ImTableStack<'ui> for Ui<'ui> {
    #[inline]
    fn begin_table_dyn_untyped(
        &mut self,
        ident: &mut dyn ImStr,
        columns: u32,
        untyped_flags: Option<u32>,
        outer_size: Option<ImSize2>,
        inner_width: Option<f32>,
    ) -> Option<UiTokenDyn<'ui>> {
        ImTableStack::begin_table_dyn_untyped(
            &mut &*self,
            ident,
            columns,
            untyped_flags,
            outer_size,
            inner_width,
        )
    }
}
impl<'ui> ImDrawItemStack<'ui> for &'_ Ui<'ui> {
    type StyleTokenItemSpacing = UiTokenDyn<'ui>;
    #[inline]
    fn push_style_item_spacing(&mut self, spacing: ImVec2) -> Self::StyleTokenItemSpacing {
        let token = unsafe {
            let () =
                sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_ItemSpacing as _, ImSpaces(spacing).into());
            UiTokenFn::new_fn_item(&mut im192_pop_style_var)
            // <imgui::StyleStackToken<'ui> as UiTokenZst>::materialize()
        };
        token.into()
    }
}
impl<'ui> ImDrawItemStack<'ui> for Ui<'ui> {
    type StyleTokenItemSpacing = UiTokenDyn<'ui>;
    fn push_style_item_spacing(&mut self, spacing: ImVec2) -> Self::StyleTokenItemSpacing {
        ImDrawItemStack::push_style_item_spacing(&mut &*self, spacing).into()
    }
}
impl<'ui> ImDrawWindowStack<'ui> for &'_ Ui<'ui> {
    #[inline]
    fn begin_group_dyn(&mut self) -> UiTokenDyn<'ui> {
        unsafe {
            let () = sys::igBeginGroup();
            UiTokenFn::new_fn_item(&mut im192_container_end_group)
        }
    }
    #[inline]
    fn begin_tabs_dyn(&mut self, ident: &mut dyn ImStr, flags: Option<u32>) -> Option<UiTokenDyn<'ui>> {
        let flags = flags.unwrap_or(0) as sys::ImGuiTabBarFlags;
        unsafe {
            <dyn ImStr>::with_cstr(ident, move |ident| sys::igBeginTabBar(ident.as_ptr(), flags))
                .then(|| UiTokenFn::new_fn_item(&mut im192_container_end_tabs))
        }
    }
    #[inline]
    fn begin_tab_dyn(
        &mut self,
        label: &mut dyn ImStr,
        mut open: Option<&mut bool>,
        flags: Option<u32>,
    ) -> Option<UiTokenDyn<'ui>> {
        let flags = flags.unwrap_or(0) as sys::ImGuiTabItemFlags;
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            let open = open
                .as_mut()
                .map(|&mut &mut ref mut o| o as *mut bool)
                .unwrap_or(ptr::null_mut());
            sys::igBeginTabItem(label.as_ptr(), open, flags)
                .then(|| UiTokenFn::new_fn_item(&mut im192_container_end_tab))
        })
    }
    #[inline]
    fn begin_tooltip_dyn(&mut self) -> Option<UiTokenDyn<'ui>> {
        unsafe { sys::igBeginTooltip().then(|| UiTokenFn::new_fn_item(&mut im192_container_end_tooltip)) }
    }
    #[inline]
    fn push_id32_dyn(&mut self, id: u32) -> UiTokenDyn<'ui> {
        unsafe {
            let () = sys::igPushID_Int(id as c_int);
            UiTokenFn::new_fn_item(&mut im192_pop_id)
        }
    }
    #[inline]
    fn push_ident_dyn(&mut self, ident: &mut dyn ImStr) -> UiTokenDyn<'ui> {
        <dyn ImStr>::with_bstr(ident, |ident| unsafe {
            let ptr = ident.as_ptr() as *const c_char;
            let end = ptr.add(ident.len());
            let () = sys::igPushID_StrStr(ptr, end);
            UiTokenFn::new_fn_item(&mut im192_pop_id)
        })
    }
    #[inline]
    fn close_current_popup(&mut self) {
        unsafe {
            let () = sys::igCloseCurrentPopup();
        }
    }
    #[inline]
    fn open_popup_by_ident_dyn(&mut self, ident: &mut dyn ImStr, untyped_flags: Option<u32>) {
        let flags = untyped_flags.unwrap_or(0);
        if let Some(id32) = ident.im_as_id32() {
            unsafe {
                let () = sys::igOpenPopup_ID(id32 as sys::ImGuiID, flags as c_int);
            }
        } else {
            <dyn ImStr>::with_cstr(ident, move |ident| unsafe {
                let () = sys::igOpenPopup_Str(ident.as_ptr(), flags as c_int);
            })
        }
    }
    #[inline]
    fn item_prepare_push_width_dyn(&mut self, width: f32) -> UiTokenDyn<'ui> {
        unsafe {
            let () = sys::igPushItemWidth(width);
            UiTokenFn::new_fn_item(&mut im192_pop_item_width)
        }
    }
    #[inline]
    fn window_prepare_push_size_min_dyn(&mut self, size: ImSize2<ImSpace>) -> UiTokenDyn<'ui> {
        unsafe {
            let size = ImSpaces(size).into();
            let style_var = sys::ImGuiStyleVar_WindowMinSize as sys::ImGuiStyleVar;
            let () = sys::igPushStyleVar_Vec2(style_var, size);
            UiTokenFn::new_fn_item(&mut im192_pop_style_var)
        }
    }
    #[inline]
    fn window_prepare_size(&mut self, size: ImSize2<ImSpace>, cond: ImCondition) {
        unsafe {
            let cond = cond.to_im192_sys() as sys::ImGuiCond;
            sys::igSetNextWindowSize(ImSpaces(size).into(), cond)
        }
    }
    #[inline]
    fn window_prepare_pos(&mut self, pos: ImPos2<ImSpace>, cond: ImCondition, pivot: ImVec2<f32>) {
        unsafe {
            let cond = cond.to_im192_sys() as sys::ImGuiCond;
            sys::igSetNextWindowPos(ImSpaces(pos).into(), cond, ImSpaces(pivot).into())
        }
    }
    #[inline]
    fn window_prepare_scroll(&mut self, offset: ImPos2) {
        unsafe { sys::igSetNextWindowScroll(ImSpaces(offset).into()) }
    }
    #[inline]
    fn window_prepare_alpha(&mut self, opacity: f32) {
        unsafe { sys::igSetNextWindowBgAlpha(opacity) }
    }
    #[inline]
    fn window_prepare_focus(&mut self) {
        unsafe { sys::igSetNextWindowFocus() }
    }
    #[inline]
    fn window_prepare_content_size(&mut self, size: ImSize2) {
        unsafe { sys::igSetNextWindowContentSize(ImSpaces(size).into()) }
    }
    #[inline]
    fn window_prepare_collapsed(&mut self, collapsed: bool, cond: ImCondition) {
        unsafe {
            let cond = cond.to_im192_sys() as sys::ImGuiCond;
            sys::igSetNextWindowCollapsed(collapsed, cond)
        }
    }
    #[inline]
    fn window_prepare_size_constraints(&mut self, min: ImSize2<ImSpace>, max: ImSize2<ImSpace>) {
        unsafe {
            sys::igSetNextWindowSizeConstraints(
                ImSpaces(min).into(),
                ImSpaces(max).into(),
                None,
                ptr::null_mut(),
            )
        }
    }

    #[inline]
    fn window_defocus_any(&mut self) {
        unsafe { sys::igSetWindowFocus_Str(ptr::null()) }
    }
}
impl<'ui> ImDrawWindowStack<'ui> for Ui<'ui> {
    fn begin_group_dyn(&mut self) -> UiTokenDyn<'ui> {
        ImDrawWindowStack::begin_group_dyn(&mut &*self).into()
    }
    fn begin_tabs_dyn(&mut self, ident: &mut dyn ImStr, flags: Option<u32>) -> Option<UiTokenDyn<'ui>> {
        ImDrawWindowStack::begin_tabs_dyn(&mut &*self, ident, flags)
    }
    fn begin_tab_dyn(
        &mut self,
        label: &mut dyn ImStr,
        open: Option<&mut bool>,
        flags: Option<u32>,
    ) -> Option<UiTokenDyn<'ui>> {
        ImDrawWindowStack::begin_tab_dyn(&mut &*self, label, open, flags)
    }
    fn begin_tooltip_dyn(&mut self) -> Option<UiTokenDyn<'ui>> {
        ImDrawWindowStack::begin_tooltip_dyn(&mut &*self).map(Into::into)
    }
    fn push_id32_dyn(&mut self, id: u32) -> UiTokenDyn<'ui> {
        ImDrawWindowStack::push_id32_dyn(&mut &*self, id).into()
    }
    fn push_ident_dyn(&mut self, ident: &mut dyn ImStr) -> UiTokenDyn<'ui> {
        ImDrawWindowStack::push_ident_dyn(&mut &*self, ident).into()
    }
    fn close_current_popup(&mut self) {
        ImDrawWindowStack::close_current_popup(&mut &*self)
    }
    fn open_popup_by_ident_dyn(&mut self, ident: &mut dyn ImStr, untyped_flags: Option<u32>) {
        ImDrawWindowStack::open_popup_by_ident_dyn(&mut &*self, ident, untyped_flags)
    }
    fn item_prepare_push_width_dyn(&mut self, width: f32) -> UiTokenDyn<'ui> {
        ImDrawWindowStack::item_prepare_push_width_dyn(&mut &*self, width)
    }
    fn window_prepare_push_size_min_dyn(&mut self, size: ImSize2<ImSpace>) -> UiTokenDyn<'ui> {
        ImDrawWindowStack::window_prepare_push_size_min_dyn(&mut &*self, size)
    }
    fn window_prepare_size(&mut self, size: ImSize2<ImSpace>, cond: ImCondition) {
        ImDrawWindowStack::window_prepare_size(&mut &*self, size, cond)
    }
    fn window_prepare_pos(&mut self, pos: ImPos2<ImSpace>, cond: ImCondition, pivot: ImVec2<f32>) {
        ImDrawWindowStack::window_prepare_pos(&mut &*self, pos, cond, pivot)
    }
    fn window_prepare_scroll(&mut self, offset: ImPos2) {
        ImDrawWindowStack::window_prepare_scroll(&mut &*self, offset)
    }
    fn window_prepare_alpha(&mut self, opacity: f32) {
        ImDrawWindowStack::window_prepare_alpha(&mut &*self, opacity)
    }
    fn window_prepare_focus(&mut self) {
        ImDrawWindowStack::window_prepare_focus(&mut &*self)
    }
    fn window_prepare_content_size(&mut self, size: ImSize2) {
        ImDrawWindowStack::window_prepare_content_size(&mut &*self, size)
    }
    fn window_prepare_collapsed(&mut self, collapsed: bool, cond: ImCondition) {
        ImDrawWindowStack::window_prepare_collapsed(&mut &*self, collapsed, cond)
    }
    fn window_prepare_size_constraints(&mut self, min: ImSize2<ImSpace>, max: ImSize2<ImSpace>) {
        ImDrawWindowStack::window_prepare_size_constraints(&mut &*self, min, max)
    }
    fn window_defocus_any(&mut self) {
        ImDrawWindowStack::window_defocus_any(&mut &*self)
    }
}

impl<'ui> TransformMap<sys::ImVec2> for ImSpaces<Ui<'ui>> {
    type Output = ImVec2<f32>;
    #[inline(always)]
    fn map(&self, v: sys::ImVec2) -> Self::Output {
        ImSpaces(v).into()
    }
}
impl<'ui> TransformMap<sys::ImVec2> for ImSpaces<&'_ Ui<'ui>> {
    type Output = ImVec2<f32>;
    #[inline(always)]
    fn map(&self, v: sys::ImVec2) -> Self::Output {
        ImSpaces(v).into()
    }
}
impl<'ui> TransformMap<sys::ImVec4> for ImSpaces<Ui<'ui>> {
    type Output = glamour::Vector4<f32>;
    #[inline(always)]
    fn map(&self, v: sys::ImVec4) -> Self::Output {
        ImSpaces(v).into()
    }
}
impl<T> ImSpaces<T> {
    #[inline(always)]
    pub fn to_sys2_192(self) -> sys::ImVec2
    where
        T: mint::IntoMint,
        T::MintType: Into<mint::Vector2<f32>>,
    {
        <T as Into<T::MintType>>::into(self.0).into().into()
    }
    #[inline(always)]
    pub fn to_sys4_192(self) -> sys::ImVec4
    where
        T: mint::IntoMint,
        T::MintType: Into<mint::Vector4<f32>>,
    {
        <T as Into<T::MintType>>::into(self.0).into().into()
    }
}
impl<U: glamour::Unit<Scalar = f32>> From<ImSpaces<ImVec2<U>>> for sys::ImVec2
where
    ImVec2<f32>: mint::IntoMint, //<MintType = mint::Vector2<f32>>,
    <ImVec2<f32> as mint::IntoMint>::MintType: Into<mint::Vector2<f32>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<ImVec2<U>>) -> Self {
        ImSpaces(Into::<<ImVec2<f32> as mint::IntoMint>::MintType>::into(
            v.0.to_untyped(),
        ))
        .to_sys2_192()
    }
}
impl<U: glamour::Unit<Scalar = f32>> From<ImSpaces<ImSize2<U>>> for sys::ImVec2
where
    ImSize2<f32>: mint::IntoMint, //<MintType = mint::Vector2<f32>>,
    <ImSize2<f32> as mint::IntoMint>::MintType: Into<mint::Vector2<f32>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<ImSize2<U>>) -> Self {
        ImSpaces(Into::<<ImSize2<f32> as mint::IntoMint>::MintType>::into(
            v.0.to_untyped(),
        ))
        .to_sys2_192()
    }
}
impl<U: glamour::Unit<Scalar = f32>> From<ImSpaces<ImPos2<U>>> for sys::ImVec2
where
    ImPos2<f32>: mint::IntoMint, //<MintType = mint::Vector2<f32>>,
    <ImPos2<f32> as mint::IntoMint>::MintType: Into<mint::Vector2<f32>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<ImPos2<U>>) -> Self {
        ImSpaces(Into::<<ImPos2<f32> as mint::IntoMint>::MintType>::into(
            v.0.to_untyped(),
        ))
        .to_sys2_192()
    }
}
impl<U: glamour::Unit<Scalar = f32>> From<ImSpaces<Box2<U>>> for [sys::ImVec2; 2]
where
    ImSpaces<ImPos2<U>>: Into<sys::ImVec2>,
{
    #[inline(always)]
    fn from(v: ImSpaces<Box2<U>>) -> Self {
        let ImSpaces(Box2 { min, max }) = v;
        [ImSpaces(min).into(), ImSpaces(max).into()]
    }
}
impl<U: glamour::Unit<Scalar = f32>, const N: usize> From<ImSpaces<[ImPos2<U>; N]>> for [sys::ImVec2; N]
where
    ImSpaces<ImPos2<U>>: Into<sys::ImVec2>,
{
    #[inline(always)]
    fn from(v: ImSpaces<[ImPos2<U>; N]>) -> Self {
        unsafe { arcffi::transmute_unchecked(v) }
    }
}
impl<V> From<ImSpaces<V>> for sys::ImVec4
where
    V: mint::IntoMint<MintType = mint::Vector4<f32>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<V>) -> Self {
        v.to_sys4_192()
    }
}
impl From<ImSpaces<ImColour>> for sys::ImColor {
    #[inline(always)]
    fn from(v: ImSpaces<ImColour>) -> Self {
        Self {
            Value: Into::<<ImColour as mint::IntoMint>::MintType>::into(v.0).into(),
        }
    }
}

impl ImCondition {
    pub fn to_im192_sys(self) -> sys::ImGuiCond_ {
        match self {
            Self::Always => sys::ImGuiCond_Always,
            Self::Startup => sys::ImGuiCond_Once,
            Self::Initial => sys::ImGuiCond_FirstUseEver,
            Self::Appear => sys::ImGuiCond_Appearing,
        }
    }
}
#[cfg(todo)]
impl ImTexture for imgui::TextureId {
    fn im192_texture_ref(&self) -> Option<sys::ImTextureRef> {
        Some(self.id())
    }
    #[cfg(feature = "imgui180")]
    fn im180_texture_id(&self) -> Option<sys180::ImTextureID> {
        //self.id().get_tex_id();
        self.id().tex_id().map(|id| id as usize as _)
    }
}
#[cfg(todo)]
impl UiToken for imgui::FontStackToken<'_> {
    #[inline]
    #[cfg(todo = "unused")]
    fn token_empty(&self) -> bool {
        false
    }
    #[inline]
    fn token_pop(self) {
        self.pop()
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        ptr::drop_in_place(self)
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        true
    }
    type TokenGuardType
        = Self
    where
        Self: Sized;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType
    where
        Self: Sized,
    {
        self
    }
}
#[cfg(todo)]
impl UiTokenGuard for imgui::FontStackToken<'_> {
    type GuardInner = ();
    #[inline(always)]
    fn guard_leak(self) -> Self::GuardInner {
        mem::forget(self)
    }
}
#[cfg(todo)]
unsafe impl<'ui> UiTokenZst for imgui::FontStackToken<'ui> {
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        &mut *ptr::dangling_mut()
    }
}
#[cfg(todo)]
impl<'ui> From<imgui::FontStackToken<'ui>> for UiTokenDyn<'ui> {
    #[inline]
    fn from(token: imgui::FontStackToken<'ui>) -> Self {
        Self::new(token)
    }
}

#[cfg(todo)]
impl UiToken for imgui::StyleStackToken<'_> {
    #[inline]
    fn token_pop(self) {
        self.pop()
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        ptr::drop_in_place(self)
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        true
    }
    type TokenGuardType
        = Self
    where
        Self: Sized;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType
    where
        Self: Sized,
    {
        self
    }
}
#[cfg(todo)]
impl UiTokenGuard for imgui::StyleStackToken<'_> {
    type GuardInner = ();
    #[inline(always)]
    fn guard_leak(self) -> Self::GuardInner {
        mem::forget(self)
    }
}
#[cfg(todo)]
unsafe impl<'ui> UiTokenZst for imgui::StyleStackToken<'ui> {
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        &mut *ptr::dangling_mut()
    }
}
#[cfg(todo)]
impl<'ui> From<imgui::StyleStackToken<'ui>> for UiTokenDyn<'ui> {
    #[inline]
    fn from(token: imgui::StyleStackToken<'ui>) -> Self {
        Self::new(token)
    }
}

#[cfg(todo)]
impl UiToken for imgui::ColorStackToken<'_> {
    #[inline]
    fn token_pop(self) {
        self.pop()
    }
    #[inline]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        ptr::drop_in_place(self)
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        true
    }
    type TokenGuardType
        = Self
    where
        Self: Sized;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType
    where
        Self: Sized,
    {
        self
    }
}
#[cfg(todo)]
impl UiTokenGuard for imgui::ColorStackToken<'_> {
    type GuardInner = u32;
    #[inline(always)]
    fn guard_leak(self) -> Self::GuardInner {
        mem::forget(self);
        1
    }
}
#[cfg(todo)]
unsafe impl<'ui> UiTokenZst for imgui::ColorStackToken<'ui> {
    #[inline(always)]
    unsafe fn materialize_mut<'a>() -> &'a mut Self {
        &mut *ptr::dangling_mut()
    }
}
#[cfg(todo)]
impl<'ui> From<imgui::ColorStackToken<'ui>> for UiTokenDyn<'ui> {
    #[inline]
    fn from(token: imgui::ColorStackToken<'ui>) -> Self {
        Self::new(token)
    }
}
/// TODO: manual token, why..?
#[cfg(todo)]
impl UiTokenGuard for imgui::ItemWidthStackToken {}
#[cfg(todo)]
impl<'ui> UiToken for imgui::ItemWidthStackToken<'ui> {
    #[inline]
    fn token_pop(self) {
        let is_empty = match () {
            #[cfg(debug_assertions)]
            _ => unsafe {
                let ptr: *mut imgui::Context = mem::transmute(core::ptr::read(&self));
                ptr.is_null()
            },
            #[cfg(not(debug_assertions))]
            _ => false,
        };
        match is_empty {
            #[cfg(todo)]
            true => return,
            #[cfg(todo)]
            _ => self.pop(&*ptr::dangling()),
            _ => unsafe {
                let mut token = core::mem::ManuallyDrop::new(self);
                token.token_pop_mut_unchecked()
            },
        }
    }
    #[inline(always)]
    unsafe fn token_pop_mut_unchecked(&mut self) {
        sys::igPopTextWrapPos()
    }

    #[inline(always)]
    fn token_impls_guard() -> bool {
        false
    }
    type TokenGuardType
        = ImGuard<Self>
    where
        Self: Sized;
    #[inline(always)]
    fn into_guard(self) -> Self::TokenGuardType
    where
        Self: Sized,
    {
        ImGuard::new(self)
    }
}

fn im192_pop_id() {
    unsafe { sys::igPopID() }
}
fn im192_pop_style_var() {
    unsafe { sys::igPopStyleVar(1) }
}
fn im192_pop_item_width() {
    unsafe { sys::igPopItemWidth() }
}
fn im192_container_end_window() {
    unsafe { sys::igEnd() }
}
fn im192_container_end_child_window() {
    unsafe { sys::igEndChild() }
}
fn im192_container_end_popup() {
    unsafe { sys::igEndPopup() }
}
fn im192_container_end_group() {
    unsafe { sys::igEndGroup() }
}
fn im192_container_end_tooltip() {
    unsafe { sys::igEndTooltip() }
}
fn im192_container_end_menu() {
    unsafe { sys::igEndMenu() }
}
fn im192_container_end_tabs() {
    unsafe { sys::igEndTabBar() }
}
fn im192_container_end_tab() {
    unsafe { sys::igEndTabItem() }
}
fn im192_container_end_table() {
    unsafe { sys::igEndTable() }
}
fn im192_container_end_tree_node() {
    unsafe { sys::igTreePop() }
}
fn im192_container_end_combo() {
    unsafe { sys::igEndCombo() }
}
fn im192_container_end_listbox() {
    unsafe { sys::igEndListBox() }
}

impl<W: Default> Into<((W, ImVersion19200), sys::ImGuiCond)> for ImCondition {
    #[inline(always)]
    fn into(self) -> ((W, ImVersion19200), sys::ImGuiCond) {
        let cond = self.to_im192_sys() as sys::ImGuiCond;
        (Default::default(), cond)
    }
}
impl Into<((imw::Window, ImVersion19200), sys::ImGuiWindowFlags)> for imw::DynArgsWindow {
    #[inline(always)]
    fn into(self) -> ((imw::Window, ImVersion19200), sys::ImGuiWindowFlags) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags as sys::ImGuiWindowFlags)
    }
}
impl<'ui, S, I, F> ImWidget<Ui<'ui>> for (&'ui imw::Window, S, Option<I>, F)
where
    S: ImStrExt,
    I: BorrowMut<bool>,
    F: Into<((imw::Window, ImVersion19200), sys::ImGuiWindowFlags)>,
{
    /// for mostly legacy reasons, token is unconditional
    type Output = (Option<imw::BeginVisible>, UiTokenDyn<'ui>);
    #[inline]
    fn draw_widget(self, _: &mut Ui<'ui>) -> Self::Output {
        let (_, label, mut state, flags) = self;
        let (_, flags) = flags.into();
        let mut open = state.as_mut().map(BorrowMut::borrow_mut);
        let vis = imw::BeginVisible::new(<dyn ImStr>::with_cstr(label, move |label| unsafe {
            let open = open
                .as_mut()
                .map(|&mut &mut ref mut o| o as *mut bool)
                .unwrap_or(ptr::null_mut());
            sys::igBegin(label.as_ptr(), open, flags)
        }));
        let token = unsafe { UiTokenFn::new_fn_item(&mut im192_container_end_window) };
        (vis, token)
    }
}
impl
    Into<(
        (imw::ChildWindow, ImVersion19200),
        sys::ImGuiWindowFlags,
        sys::ImVec2,
        sys::ImGuiChildFlags,
    )> for imw::DynArgsChildWindow
{
    #[inline(always)]
    fn into(
        self,
    ) -> (
        (imw::ChildWindow, ImVersion19200),
        sys::ImGuiWindowFlags,
        sys::ImVec2,
        sys::ImGuiChildFlags,
    ) {
        let flags = self.untyped_flags().unwrap_or(0);
        let border = flags & imw::ChildWindow::IM192_BORDER_FLAG != 0;
        let mut child_flags = sys::ImGuiChildFlags_::default();
        if border {
            child_flags |= sys::ImGuiChildFlags_Borders;
        }
        (
            Default::default(),
            (flags & !imw::ChildWindow::IM192_BORDER_FLAG) as sys::ImGuiWindowFlags,
            ImSpaces(self.size().unwrap_or(ImSize2::ZERO)).into(),
            child_flags as sys::ImGuiChildFlags,
        )
    }
}
impl<'ui, S, F> ImWidget<Ui<'ui>> for (&'ui imw::ChildWindow, S, (), F)
where
    S: ImStrExt,
    F: Into<(
        (imw::ChildWindow, ImVersion19200),
        sys::ImGuiWindowFlags,
        sys::ImVec2,
        sys::ImGuiChildFlags,
    )>,
{
    /// for mostly legacy reasons, token is unconditional
    type Output = (Option<imw::BeginVisible>, UiTokenDyn<'ui>);
    #[inline]
    fn draw_widget(self, _: &mut Ui<'ui>) -> Self::Output {
        let (_, mut label, (), flags) = self;
        let (_, flags, size, child_flags) = flags.into();
        let id32 = label.with_imstr_dyn(|label| label.im_as_id32());
        let vis = imw::BeginVisible::new(match id32 {
            Some(id) => unsafe { sys::igBeginChild_ID(id as c_uint, size, child_flags, flags) },
            None => <dyn ImStr>::with_cstr(label, move |label| unsafe {
                sys::igBeginChild_Str(label.as_ptr(), size, child_flags, flags)
            }),
        });
        let token = unsafe { UiTokenFn::new_fn_item(&mut im192_container_end_child_window) };
        (vis, token)
    }
}
impl Into<((imw::Popup, ImVersion19200), sys::ImGuiWindowFlags)> for imw::DynArgsWindow {
    #[inline(always)]
    fn into(self) -> ((imw::Popup, ImVersion19200), sys::ImGuiWindowFlags) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags as sys::ImGuiWindowFlags)
    }
}
impl<'ui, S, F> ImWidget<Ui<'ui>> for (&'ui imw::Popup, S, (), F)
where
    S: ImStrExt,
    F: Into<((imw::Popup, ImVersion19200), sys::ImGuiWindowFlags)>,
{
    type Output = Option<UiTokenDyn<'ui>>;
    #[inline]
    fn draw_widget(self, _: &mut Ui<'ui>) -> Self::Output {
        let (_, label, (), flags) = self;
        let (_, flags) = flags.into();
        #[cfg(todo)]
        if let Some(id) = label.im_as_id32() {
            return unsafe {
                sys::igBeginPopupEx(
                    id as sys::ImGuiID,
                    flags
                        | sys::ImGuiWindowFlags_AlwaysAutoResize
                        | sys::ImGuiWindowFlags_NoTitleBar
                        | sys::ImGuiWindowFlags_NoSavedSettings,
                )
            }
        }
        let vis = <dyn ImStr>::with_cstr(label, move |label| unsafe {
            sys::igBeginPopup(label.as_ptr(), flags)
        });
        let token = vis.then(|| unsafe { UiTokenFn::new_fn_item(&mut im192_container_end_popup) });
        token
    }
}
impl Into<((imw::PopupModal, ImVersion19200), sys::ImGuiWindowFlags)> for imw::DynArgsWindow {
    #[inline(always)]
    fn into(self) -> ((imw::PopupModal, ImVersion19200), sys::ImGuiWindowFlags) {
        let flags = self
            .untyped_flags()
            .unwrap_or(imw::PopupModal::IM192_FLAGS_PRESET);
        (Default::default(), flags as sys::ImGuiWindowFlags)
    }
}
impl<'ui, S, O, F> ImWidget<Ui<'ui>> for (&'ui imw::PopupModal, S, Option<O>, F)
where
    S: ImStrExt,
    O: BorrowMut<bool>,
    F: Into<((imw::PopupModal, ImVersion19200), sys::ImGuiWindowFlags)>,
{
    type Output = Option<UiTokenDyn<'ui>>;
    #[inline]
    fn draw_widget(self, _: &mut Ui<'ui>) -> Self::Output {
        let (_, label, mut open, flags) = self;
        let (_, flags) = flags.into();
        let mut open = open.as_mut().map(BorrowMut::borrow_mut);
        let vis = <dyn ImStr>::with_cstr(label, move |label| unsafe {
            let open = open
                .as_mut()
                .map(|&mut &mut ref mut o| o as *mut bool)
                .unwrap_or(ptr::null_mut());
            sys::igBeginPopupModal(label.as_ptr(), open, flags)
        });
        let token = vis.then(|| unsafe { UiTokenFn::new_fn_item(&mut im192_container_end_popup) });
        token
    }
}
impl<'ui, F, P> ImWidget<Ui<'ui>> for (&'ui imw::Tooltip, P, F)
where
//F: Into<((imw::Tooltip, ImVersion19200), ())>,
{
    //type Output = Option<imw::ContainerOpen<Combo>>;
    type Output = Option<UiTokenDyn<'ui>>;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, _state, _flags) = self;
        #[cfg(todo = "unnecessary")]
        unsafe {
            let () = sys::igBeginTooltip();
            UiTokenFn::new_fn_item(&mut im192_container_end_tooltip)
        }
        _ui.begin_tooltip_dyn()
    }
}

impl<'ui, F, P> ImWidget<Ui<'ui>> for (&'ui imw::Group, P, F)
where
//F: Into<((imw::Group, ImVersion19200), ())>,
{
    //type Output = Option<imw::ContainerOpen<Combo>>;
    type Output = UiTokenDyn<'ui>;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, _state, _flags) = self;
        #[cfg(todo = "unnecessary")]
        unsafe {
            let () = sys::igBeginGroup();
            UiTokenFn::new_fn_item(&mut im192_container_end_group)
        }
        _ui.begin_group_dyn()
    }
}

impl Into<((imw::Image, ImVersion19200), sys::ImVec2)> for imw::DynArgsWidgetSized {
    #[inline(always)]
    fn into(self) -> ((imw::Image, ImVersion19200), sys::ImVec2) {
        #[cfg(todo)]
        let _flags = self.untyped_flags().unwrap_or(0);
        (
            Default::default(),
            ImSpaces(self.size().unwrap_or(ImSize2::ZERO)).into(),
        )
    }
}
impl<'ui, T, F> ImWidget<Ui<'ui>> for (&'ui imw::Image, T, F)
where
    T: ImTexture,
    F: Into<((imw::Image, ImVersion19200), sys::ImVec2)>,
{
    type Output = ();
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, texture, flags) = self;
        let (_, size) = flags.into();
        let Some(tex_id) = texture.im192_texture_ref() else { return };
        let tint = texture.tint();
        let uv = texture.uv_bounds();
        // TODO?
        let bg_colour = ImColour::splat(0.0);
        match tint {
            #[cfg(feature = "imgui192-obsolete")]
            t if t == ImColourIndex::V4_WHITE => unsafe {
                sys::igImage(tex_id, size, ImSpaces(uv.min).into(), ImSpaces(uv.max).into())
            },
            tint => unsafe {
                sys::igImageWithBg(
                    tex_id,
                    size,
                    ImSpaces(uv.min).into(),
                    ImSpaces(uv.max).into(),
                    ImSpaces(bg_colour).into(),
                    ImSpaces(tint).into(),
                )
            },
        }
    }
}

impl Into<((imw::ProgressBar, ImVersion19200), sys::ImVec2)> for imw::DynArgsWidgetSized {
    #[inline(always)]
    fn into(self) -> ((imw::ProgressBar, ImVersion19200), sys::ImVec2) {
        #[cfg(todo)]
        let _flags = self.untyped_flags().unwrap_or(0);
        (
            Default::default(),
            ImSpaces(self.size().unwrap_or(imw::ProgressBar::IM192_DEFAULT_SIZE)).into(),
        )
    }
}
impl<'ui, O, S, F> ImWidget<Ui<'ui>> for (&'ui imw::ProgressBar, (S, Option<O>), F)
where
    S: Into<f32>,
    O: ImStr,
    F: Into<((imw::ProgressBar, ImVersion19200), sys::ImVec2)>,
{
    type Output = ();
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, (progress, mut overlay), args) = self;
        let (_, size) = args.into();
        let overlay = overlay.as_mut().map(|o| o.im_take_cstring());
        let overlay = overlay.as_ref().map(|o| o.as_ptr()).unwrap_or(ptr::null());
        unsafe { sys::igProgressBar(progress.into(), size, overlay) }
    }
}

impl<T: ?Sized + ImPrimitiveData>
    Into<(
        PhantomData<(imw::Slider<T>, ImVersion19200)>,
        sys::ImGuiSliderFlags,
        RangeInclusive<T::Primitive>,
    )> for imw::DynArgsSlider<T::Primitive>
{
    #[inline(always)]
    fn into(
        self,
    ) -> (
        PhantomData<(imw::Slider<T>, ImVersion19200)>,
        sys::ImGuiSliderFlags,
        RangeInclusive<T::Primitive>,
    ) {
        let flags = self.untyped_flags().unwrap_or(0);
        (PhantomData, flags as sys::ImGuiSliderFlags, self.range)
    }
}
impl<'ui, T, L, D, S, F> ImWidget<Ui<'ui>> for (&'ui imw::Slider<T>, L, (S, Option<D>), F)
where
    T: ?Sized + ImPrimitiveData,
    S: BorrowMut<T>,
    L: ImStrExt,
    D: ImStr,
    F: Into<(
        PhantomData<(imw::Slider<T>, ImVersion19200)>,
        sys::ImGuiSliderFlags,
        RangeInclusive<T::Primitive>,
    )>,
{
    type Output = bool;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, (mut state, mut format), args) = self;
        let (_, flags, range) = args.into();
        let state = state.borrow_mut();
        let format = format.as_mut().map(|o| o.im_take_cstring());
        let format = format.as_ref().map(|o| o.as_ptr()).unwrap_or(ptr::null());
        let data_type = T::im192_data_type() as sys::ImGuiDataType;
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            let min = range.start() as *const T::Primitive as *const c_void;
            #[cfg(todo)]
            let min = range.im_range_min().cast::<c_void>().as_ptr() as *const c_void;
            let max = range.end() as *const T::Primitive as *const c_void;
            let state_ptr = state.im_data_ptr().cast::<c_void>().as_ptr();
            match state.im_is_n() {
                None => sys::igSliderScalar(label.as_ptr(), data_type, state_ptr, min, max, format, flags),
                Some(n) => sys::igSliderScalarN(
                    label.as_ptr(),
                    data_type,
                    state_ptr,
                    n as c_int,
                    min,
                    max,
                    format,
                    flags,
                ),
            }
        })
    }
}
impl<T: ?Sized + ImPrimitiveData>
    Into<(
        PhantomData<(imw::VSlider<T>, ImVersion19200)>,
        sys::ImGuiSliderFlags,
        RangeInclusive<T::Primitive>,
        sys::ImVec2,
    )> for imw::DynArgsVSlider<T::Primitive>
{
    #[inline(always)]
    fn into(
        self,
    ) -> (
        PhantomData<(imw::VSlider<T>, ImVersion19200)>,
        sys::ImGuiSliderFlags,
        RangeInclusive<T::Primitive>,
        sys::ImVec2,
    ) {
        let flags = self.untyped_flags().unwrap_or(0);
        let size = self.size().unwrap_or(ImSize2::ZERO);
        (
            PhantomData,
            flags as sys::ImGuiSliderFlags,
            self.args.range,
            ImSpaces(size).into(),
        )
    }
}
impl<'ui, T, L, D, S, F> ImWidget<Ui<'ui>> for (&'ui imw::VSlider<T>, L, (S, Option<D>), F)
where
    T: ImPrimitive,
    S: BorrowMut<T>,
    L: ImStrExt,
    D: ImStr,
    F: Into<(
        PhantomData<(imw::VSlider<T>, ImVersion19200)>,
        sys::ImGuiSliderFlags,
        RangeInclusive<T>,
        sys::ImVec2,
    )>,
{
    type Output = bool;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, (mut state, mut format), args) = self;
        let (_, flags, range, size) = args.into();
        let state = state.borrow_mut();
        let format = format.as_mut().map(|o| o.im_take_cstring());
        let format = format.as_ref().map(|o| o.as_ptr()).unwrap_or(ptr::null());
        let data_type = T::im192_data_type() as sys::ImGuiDataType;
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            let min = range.start() as *const T as *const c_void;
            let max = range.end() as *const T as *const c_void;
            let state_ptr = state as *mut T as *mut c_void;
            sys::igVSliderScalar(
                label.as_ptr(),
                size,
                data_type,
                state_ptr,
                min,
                max,
                format,
                flags,
            )
        })
    }
}

impl<T: ?Sized + ImPrimitiveData>
    Into<(
        PhantomData<(imw::InputScalar<T>, ImVersion19200)>,
        sys::ImGuiInputTextFlags,
        (Option<T::Primitive>, Option<T::Primitive>),
    )> for imw::DynArgsInputScalar
{
    #[inline(always)]
    fn into(
        self,
    ) -> (
        PhantomData<(imw::InputScalar<T>, ImVersion19200)>,
        sys::ImGuiInputTextFlags,
        (Option<T::Primitive>, Option<T::Primitive>),
    ) {
        let flags = self.untyped_flags().unwrap_or(0);
        let step = None;
        let step_fast = None;
        (PhantomData, flags as sys::ImGuiInputTextFlags, (step, step_fast))
    }
}
impl<'ui, T, L, D, S, F> ImWidget<Ui<'ui>> for (&'ui imw::InputScalar<T>, L, (S, Option<D>), F)
where
    T: ?Sized + ImPrimitiveData,
    S: BorrowMut<T>,
    L: ImStrExt,
    D: ImStr,
    F: Into<(
        PhantomData<(imw::InputScalar<T>, ImVersion19200)>,
        sys::ImGuiInputTextFlags,
        (Option<T::Primitive>, Option<T::Primitive>),
    )>,
{
    type Output = bool;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, (mut state, mut format), args) = self;
        let (_, flags, (step, step_fast)) = args.into();
        let state = state.borrow_mut();
        let format = format.as_mut().map(|o| o.im_take_cstring());
        let format = format.as_ref().map(|o| o.as_ptr()).unwrap_or(ptr::null());
        let data_type = T::im192_data_type() as sys::ImGuiDataType;
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            let step = step
                .as_ref()
                .map(|s| s as *const T::Primitive as *const c_void)
                .unwrap_or(ptr::null());
            let step_fast = step_fast
                .as_ref()
                .map(|s| s as *const T::Primitive as *const c_void)
                .unwrap_or(ptr::null());
            let state_ptr = state.im_data_ptr().cast::<c_void>().as_ptr();
            match state.im_is_n() {
                None => sys::igInputScalar(
                    label.as_ptr(),
                    data_type,
                    state_ptr,
                    step,
                    step_fast,
                    format,
                    flags,
                ),
                Some(n) => sys::igInputScalarN(
                    label.as_ptr(),
                    data_type,
                    state_ptr,
                    n as c_int,
                    step,
                    step_fast,
                    format,
                    flags,
                ),
            }
        })
    }
}

impl
    Into<(
        (imw::Selectable, ImVersion19200),
        sys::ImGuiSelectableFlags,
        sys::ImVec2,
    )> for imw::DynArgsWidgetSized
{
    #[inline(always)]
    fn into(
        self,
    ) -> (
        (imw::Selectable, ImVersion19200),
        sys::ImGuiSelectableFlags,
        sys::ImVec2,
    ) {
        let flags = self.untyped_flags().unwrap_or(0);
        (
            Default::default(),
            flags as sys::ImGuiSelectableFlags,
            ImSpaces(self.size().unwrap_or(ImSize2::ZERO)).into(),
        )
    }
}
impl<'ui, S, F> ImWidget<Ui<'ui>> for (&'ui imw::Selectable, S, bool, F)
where
    S: ImStrExt,
    F: Into<(
        (imw::Selectable, ImVersion19200),
        sys::ImGuiSelectableFlags,
        sys::ImVec2,
    )>,
{
    type Output = bool;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, state, flags) = self;
        let (_, flags, size) = flags.into();
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            sys::igSelectable_Bool(label.as_ptr(), state, flags, size)
        })
    }
}

impl Into<((imw::MenuItem, ImVersion19200), bool)> for imw::DynArgsWidget {
    #[inline(always)]
    fn into(self) -> ((imw::MenuItem, ImVersion19200), bool) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags & imw::MenuItem::FLAGS_DISABLED == 0)
    }
}
impl<'ui, L, S, F> ImWidget<Ui<'ui>> for (&'ui imw::MenuItem, L, (bool, Option<S>), F)
where
    L: ImStrExt,
    S: ImStr,
    F: Into<((imw::MenuItem, ImVersion19200), bool)>,
{
    type Output = bool;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, (state, mut shortcut), flags) = self;
        let (_, enabled) = flags.into();
        let shortcut = shortcut.as_mut().map(|p| p.im_take_cstring());
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            let shortcut = shortcut.as_ref().map(|p| p.as_ptr()).unwrap_or(ptr::null());
            sys::igMenuItem_Bool(label.as_ptr(), shortcut, state, enabled)
        })
    }
}
impl Into<((imw::Menu, ImVersion19200), bool)> for imw::DynArgsWidget {
    #[inline(always)]
    fn into(self) -> ((imw::Menu, ImVersion19200), bool) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags & imw::MenuItem::FLAGS_DISABLED == 0)
    }
}
impl<'ui, L, F> ImWidget<Ui<'ui>> for (&'ui imw::Menu, L, (), F)
where
    L: ImStrExt,
    F: Into<((imw::Menu, ImVersion19200), bool)>,
{
    type Output = Option<UiTokenDyn<'ui>>;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, (), flags) = self;
        let (_, enabled) = flags.into();
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            sys::igBeginMenu(label.as_ptr(), enabled)
                .then(|| UiTokenFn::new_fn_item(&mut im192_container_end_menu))
        })
    }
}

impl Into<((imw::TreeNode, ImVersion19200), sys::ImGuiTreeNodeFlags)> for imw::DynArgsWidget {
    #[inline(always)]
    fn into(self) -> ((imw::TreeNode, ImVersion19200), sys::ImGuiTreeNodeFlags) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags as sys::ImGuiTreeNodeFlags)
    }
}
impl<'ui, S, I, F> ImWidget<Ui<'ui>> for (&'ui imw::TreeNode, S, I, F)
where
    F: Into<((imw::TreeNode, ImVersion19200), sys::ImGuiTreeNodeFlags)>,
    S: ImStrExt,
    I: ImStr,
{
    type Output = Option<UiTokenDyn<'ui>>;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, mut id, flags) = self;
        let (_, flags) = flags.into();
        let id = if let Some(id) = id.im_as_id_ptr() {
            Ok(id)
        } else {
            Err(id.im_take_cstring())
        };
        let no_push_id = flags & sys::ImGuiTreeNodeFlags_NoTreePushOnOpen as sys::ImGuiTreeNodeFlags != 0;
        <dyn ImStr>::with_cbstr(label, move |label| match (id, label) {
            (Ok(id), Ok(c)) => unsafe {
                sys::igTreeNodeEx_Ptr(id as *const _, flags, FMT_CSTR.as_ptr(), c.as_ptr())
            },
            (Err(id), Ok(c)) => unsafe {
                sys::igTreeNodeEx_StrStr(id.as_ptr(), flags, FMT_CSTR.as_ptr(), c.as_ptr())
            },
            (Ok(id), Err(label)) => unsafe {
                let ptr = label.as_ptr() as *const c_char;
                sys::igTreeNodeEx_Ptr(id as *const _, flags, FMT_STR.as_ptr(), label.len() as c_int, ptr)
            },
            (Err(id), Err(label)) => unsafe {
                let ptr = label.as_ptr() as *const c_char;
                sys::igTreeNodeEx_StrStr(id.as_ptr(), flags, FMT_STR.as_ptr(), label.len() as c_int, ptr)
            },
        })
        .then(|| match no_push_id {
            true => UiTokenDyn::empty(),
            false => unsafe { UiTokenFn::new_fn_item(&mut im192_container_end_tree_node) },
        })
    }
}
impl Into<((imw::Listbox, ImVersion19200), sys::ImVec2)> for imw::DynArgsWidgetSized {
    #[inline(always)]
    fn into(self) -> ((imw::Listbox, ImVersion19200), sys::ImVec2) {
        (
            Default::default(),
            ImSpaces(self.size().unwrap_or(imw::Listbox::SIZE_NONE)).into(),
        )
    }
}
impl<'ui, S, F> ImWidget<Ui<'ui>> for (&'ui imw::Listbox, S, (), F)
where
    F: Into<((imw::Listbox, ImVersion19200), sys::ImVec2)>,
    S: ImStrExt,
{
    type Output = Option<UiTokenDyn<'ui>>;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, (), flags) = self;
        let (_, size) = flags.into();
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            sys::igBeginListBox(label.as_ptr(), size)
        })
        .then(|| unsafe { UiTokenFn::new_fn_item(&mut im192_container_end_listbox) })
    }
}
impl Into<((imw::Combo, ImVersion19200), sys::ImGuiComboFlags)> for imw::DynArgsWidget {
    #[inline(always)]
    fn into(self) -> ((imw::Combo, ImVersion19200), sys::ImGuiComboFlags) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags as sys::ImGuiComboFlags)
    }
}
impl<'ui, S, F, P> ImWidget<Ui<'ui>> for (&'ui imw::Combo, S, Option<P>, F)
where
    F: Into<((imw::Combo, ImVersion19200), sys::ImGuiComboFlags)>,
    S: ImStrExt,
    P: ImStr,
{
    //type Output = Option<imw::ContainerOpen<Combo>>;
    type Output = Option<UiTokenDyn<'ui>>;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, mut preview, flags) = self;
        let (_, flags) = flags.into();
        let preview = preview.as_mut().map(|p| p.im_take_cstring());
        <dyn ImStr>::with_cstr(label, move |label| {
            let preview = preview.as_ref().map(|p| p.as_ptr()).unwrap_or(ptr::null());
            unsafe { sys::igBeginCombo(label.as_ptr(), preview, flags.into()) }
        })
        .then(|| unsafe { UiTokenFn::new_fn_item(&mut im192_container_end_combo) })
    }
}
impl Into<((imw::InputText, ImVersion19200), sys::ImGuiInputTextFlags)> for imw::DynArgsInputText {
    #[inline(always)]
    fn into(self) -> ((imw::InputText, ImVersion19200), sys::ImGuiInputTextFlags) {
        let flags = self.untyped_flags().unwrap_or(imw::InputText::IM192_FLAGS_PRESET);
        (Default::default(), flags as sys::ImGuiInputTextFlags)
    }
}
/// TODO: callback
impl<'ui, S, F, I, H> ImWidget<Ui<'ui>> for (&'ui imw::InputText, S, (I, Option<H>), F)
where
    F: Into<((imw::InputText, ImVersion19200), sys::ImGuiInputTextFlags)>,
    S: ImStrExt,
    H: ImStr,
    I: BorrowMut<String>,
{
    //type Output = Option<imw::ContainerOpen<Combo>>;
    type Output = bool;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, (mut buffer, mut preview), flags) = self;
        let (_, flags) = flags.into();
        let preview = preview.as_mut().map(|p| p.im_take_cstring());
        let buffer = buffer.borrow_mut();
        let enter_true = flags & imw::InputText::IM192_MASK_ENTER != 0;
        // XXX: without a callback, use return value to tell if buffer was modified
        let flags = flags & !imw::InputText::IM192_MASK_ENTER;
        let prev_len = buffer.len();
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            let preview = preview.as_ref().map(|p| p.as_ptr());
            let buffer = buffer.as_mut_vec();
            match buffer.spare_capacity_mut().first_mut() {
                Some(eos) => {
                    // ensure null-termination if treated as a cstring,
                    eos.write(0u8);
                },
                None => {
                    // we have a problem when len==cap because oh god strn* is not used...
                    #[cfg(taimi_debug)]
                    log::warn!("InputText required to terminate buf, consider reserving space!");
                    //let _ = arcffi::cstr::CSlice::terminate_bytes(buffer);
                    buffer.push(0u8);
                    buffer.set_len(prev_len);
                },
            };
            let cap = buffer.capacity();
            let buf = buffer.as_mut_ptr().cast::<std::os::raw::c_char>();
            let (cb, userdata, flags) = (None, ptr::null_mut(), flags.into());
            let buf_modified = match preview {
                Some(hint) =>
                    sys::igInputTextWithHint(label.as_ptr(), hint, buf, cap as _, flags, cb, userdata),
                None => sys::igInputText(label.as_ptr(), buf, cap as _, flags, cb, userdata),
            };
            if buf_modified {
                let mut ptr = buffer.as_ptr();
                let mut new_len = {
                    let prev_end = ptr.add(prev_len);
                    match prev_end.read() {
                        0u8 => 0,
                        _ => {
                            ptr = prev_end.add(1);
                            prev_len + 1
                        },
                    }
                };
                while new_len < cap {
                    if ptr.read() == 0 {
                        break
                    }
                    ptr = ptr.add(1);
                    new_len += 1;
                }
                buffer.set_len(new_len);
            }
            match enter_true {
                true =>
                    sys::igIsItemDeactivatedAfterEdit()
                        && (sys::igIsKeyPressed_Bool(sys::ImGuiKey_Enter as sys::ImGuiKey, false)
                            || sys::igIsKeyPressed_Bool(sys::ImGuiKey_KeypadEnter as sys::ImGuiKey, false)),
                false => buf_modified,
            }
        })
    }
}
impl Into<((imw::InputPassword, ImVersion19200), sys::ImGuiInputTextFlags)> for imw::DynArgsInputText {
    #[inline(always)]
    fn into(self) -> ((imw::InputPassword, ImVersion19200), sys::ImGuiInputTextFlags) {
        let flags = self
            .untyped_flags()
            .unwrap_or(imw::InputPassword::IM192_FLAGS_PRESET);
        (Default::default(), flags as sys::ImGuiInputTextFlags)
    }
}
impl<'ui, S, I, H, F> ImWidget<Ui<'ui>> for (&'ui imw::InputPassword, S, (I, Option<H>), F)
where
    F: Into<((imw::InputPassword, ImVersion19200), sys::ImGuiInputTextFlags)>,
    S: ImStrExt,
    H: ImStr,
    I: BorrowMut<String>,
{
    type Output = bool;
    #[inline]
    fn draw_widget(self, ui: &mut Ui<'ui>) -> Self::Output {
        let (tag, label, buf, flags) = self;
        let tag = unsafe { mem::transmute::<&'ui imw::InputPassword, &'ui imw::InputText>(tag) };
        let ((_ftag, tag_ver), flags) = flags.into();
        let flags = ((imw::InputText, tag_ver), flags);
        (tag, label, buf, flags).draw_widget(ui)
    }
}
impl
    Into<(
        (imw::InputTextMultiline, ImVersion19200),
        sys::ImGuiInputTextFlags,
        sys::ImVec2,
    )> for imw::DynArgsInputMultiline
{
    #[inline(always)]
    fn into(
        self,
    ) -> (
        (imw::InputTextMultiline, ImVersion19200),
        sys::ImGuiInputTextFlags,
        sys::ImVec2,
    ) {
        let flags = self
            .untyped_flags()
            .unwrap_or(imw::InputTextMultiline::IM192_FLAGS_PRESET);
        let size = self.size().unwrap_or(imw::InputTextMultiline::IM192_DEFAULT_SIZE);
        (
            Default::default(),
            flags as sys::ImGuiInputTextFlags,
            ImSpaces(size).into(),
        )
    }
}
/// TODO: callback
impl<'ui, S, F, I> ImWidget<Ui<'ui>> for (&'ui imw::InputTextMultiline, S, I, F)
where
    F: Into<(
        (imw::InputTextMultiline, ImVersion19200),
        sys::ImGuiInputTextFlags,
        sys::ImVec2,
    )>,
    S: ImStrExt,
    I: BorrowMut<String>,
{
    //type Output = Option<imw::ContainerOpen<Combo>>;
    type Output = bool;
    #[inline]
    fn draw_widget(self, _ui: &mut Ui<'ui>) -> Self::Output {
        let (_, label, mut buffer, flags) = self;
        let (_, flags, size) = flags.into();
        let buffer = buffer.borrow_mut();
        let prev_len = buffer.len();
        <dyn ImStr>::with_cstr(label, move |label| unsafe {
            let buffer = buffer.as_mut_vec();
            match buffer.spare_capacity_mut().first_mut() {
                Some(eos) => {
                    // ensure null-termination if treated as a cstring,
                    eos.write(0u8);
                },
                None => {
                    // we have a problem when len==cap because oh god strn* is not used...
                    #[cfg(taimi_debug)]
                    log::warn!("InputText required to terminate buf, consider reserving space!");
                    //let _ = arcffi::cstr::CSlice::terminate_bytes(buffer);
                    buffer.push(0u8);
                    buffer.set_len(prev_len);
                },
            };
            let cap = buffer.capacity() as _;
            let buf = buffer.as_mut_ptr().cast::<std::os::raw::c_char>();
            let (cb, userdata, flags) = (None, ptr::null_mut(), flags.into());
            let buf_modified =
                sys::igInputTextMultiline(label.as_ptr(), buf, cap, size, flags, cb, userdata);
            if buf_modified {
                let mut ptr = buffer.as_ptr();
                let mut new_len = {
                    let prev_end = ptr.add(prev_len);
                    match prev_end.read() {
                        0u8 => 0,
                        _ => {
                            ptr = prev_end.add(1);
                            prev_len + 1
                        },
                    }
                };
                while new_len < cap {
                    if ptr.read() == 0 {
                        break
                    }
                    ptr = ptr.add(1);
                    new_len += 1;
                }
                buffer.set_len(new_len);
            }
            buf_modified
        })
    }
}

/// TODO...
impl Into<((imw::Table, ImVersion19200), sys::ImGuiTableFlags)> for imw::DynArgsTable {
    #[inline(always)]
    fn into(self) -> ((imw::Table, ImVersion19200), sys::ImGuiTableFlags) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags as sys::ImGuiWindowFlags)
    }
}
/// TODO...
impl Into<((imw::TableRow, ImVersion19200), sys::ImGuiTableRowFlags)> for imw::DynArgsTableRow {
    #[inline(always)]
    fn into(self) -> ((imw::TableRow, ImVersion19200), sys::ImGuiTableRowFlags) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags as sys::ImGuiWindowFlags)
    }
}
/// TODO...
impl Into<((imw::TableColumn, ImVersion19200), sys::ImGuiTableColumnFlags)> for imw::DynArgsTableColumn {
    #[inline(always)]
    fn into(self) -> ((imw::TableColumn, ImVersion19200), sys::ImGuiTableColumnFlags) {
        let flags = self.untyped_flags().unwrap_or(0);
        (Default::default(), flags as sys::ImGuiWindowFlags)
    }
}
