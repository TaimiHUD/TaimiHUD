use {
    super::prelude::*,
    core::{mem, ptr::NonNull},
    glamour::{Box2, Point2, Rect, Size2, Transform2, TransformMap, Vector2},
};

pub trait ImFrameArena<'ui> {}
impl<'ui, 'a, U: ?Sized> ImFrameArena<'ui> for &'a U where U: ImFrameArena<'ui> {}
impl<'ui, 'a, U: ?Sized> ImFrameArena<'ui> for &'a mut U where U: ImFrameArena<'ui> {}
pub trait ImUiWindow: ImUi {
    fn get_draw_target_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget>;

    fn cursor_screen_pos(&self) -> ImPos2<ImSpace>;

    fn cursor_start_pos(&self) -> ImPos2<WindowSpace>;
    #[inline]
    fn cursor_window_pos(&self) -> ImPos2<WindowSpace> {
        self.units().map(self.cursor_screen_pos())
    }

    fn content_region_max(&self) -> ImPos2<WindowSpace>;
    #[inline]
    fn content_region_avail(&self) -> ImSize2<WindowSpace> {
        (self.content_region_max() - self.cursor_pos()).to_size()
    }
    /// roughly `-self.scroll_offset()`
    fn window_content_region_min(&self) -> ImPos2<WindowSpace>;
    /// roughly `self.window_size() - self.scroll_offset()`
    fn window_content_region_max(&self) -> ImPos2<WindowSpace>;
    #[deprecated]
    fn window_content_region_width(&self) -> f32 {
        self.window_content_region_max().x - self.window_content_region_min().x
    }
    fn window_pos(&self) -> ImPos2<ImSpace>;
    fn scroll_offset(&self) -> ImVec2<WindowSpace>;
    fn window_size(&self) -> ImSize2<ImSpace>;
    fn window_flags(&self, mask: InteractSignal) -> InteractSignal {
        let mut flags = InteractSignal::EMPTY;
        let flag_getters = [
            (
                InteractSignal::HOVER,
                Self::window_is_hovered as fn(&Self) -> bool,
            ),
            (InteractSignal::FOCUS, Self::window_is_focused),
            (InteractSignal::TRIGGER, Self::window_is_appearing),
            (InteractSignal::OPEN, Self::window_is_collapsed),
        ];
        for (flag, getter) in flag_getters {
            if mask.contains(flag) && getter(self) {
                flags.insert(flag);
            }
        }
        /// OPEN getter is inverted...
        /// VISIBLE is unsupported so assume so...
        /// TODO: copy FOCUS to ACTIVE but idk...
        const INVERT: InteractSignal =
            InteractSignal::from_bits_retain(InteractSignal::OPEN.bits() | InteractSignal::VISIBLE.bits());
        flags ^= mask & INVERT;
        flags
    }
    fn font_scale(&self) -> f32;
    fn viewport_font_scale(&self) -> f32;
    fn viewport_framebuffer_scale(&self) -> ImVec2<f32>;

    #[inline]
    fn to_window_space(&self, v: ImPos2<ImSpace>) -> ImPos2<WindowSpace> {
        (v - self.window_pos()).cast::<WindowSpace>().to_point() + self.scroll_offset()
    }
    #[inline]
    fn from_window_space(&self, v: ImPos2<WindowSpace>) -> ImPos2<ImSpace> {
        (v - self.scroll_offset()).cast::<ImSpace>() + self.window_pos()
    }
    /// prefer [Self::from_window_space]
    #[inline]
    fn window_to_space(&self) -> Transform2<WindowSpace, ImSpace> {
        Transform2::from_translation(self.window_pos().to_vector().cast())
    }
    /// prefer [Self::to_window_space]
    #[inline]
    fn space_to_window(&self) -> Transform2<ImSpace, WindowSpace> {
        Transform2::from_translation(-self.window_pos().to_vector())
    }

    fn item_rect_min(&self) -> ImPos2<ImSpace>;
    fn item_rect_max(&self) -> ImPos2<ImSpace>;
    fn item_rect_size(&self) -> ImSize2<ImSpace>;

    fn item_is_clicked_with(&self, button_id: u32) -> bool;
    fn item_is_active(&self) -> bool;
    fn item_is_focused(&self) -> bool;
    fn item_is_visible(&self) -> bool;
    fn item_is_hovered_untyped(&self, untyped_flags: Option<u32>) -> bool;
    fn item_is_edited(&self) -> bool;
    fn item_was_activated(&self) -> bool;
    fn item_was_deactivated(&self) -> bool;
    fn item_was_deactivated_after_edit(&self) -> bool;
    fn item_was_toggled_open(&self) -> bool;
    fn item_any_hovered(&self) -> bool;
    fn item_any_active(&self) -> bool;
    fn item_any_focused(&self) -> bool;
    fn item_status_flags(&self, mask: InteractSignal) -> InteractSignal {
        let mut flags = InteractSignal::EMPTY;
        let flag_getters = [
            (InteractSignal::ACTIVE, Self::item_is_active as fn(&Self) -> bool),
            (InteractSignal::VISIBLE, Self::item_is_visible),
            (InteractSignal::FOCUS, Self::item_is_focused),
            (InteractSignal::HOVER, Self::item_is_hovered),
        ];
        for (flag, getter) in flag_getters {
            if mask.contains(flag) && getter(self) {
                flags.insert(flag);
            }
        }
        flags
    }
    fn item_signal_flags(&self, mask: InteractSignal) -> InteractSignal {
        let mut flags = InteractSignal::EMPTY;
        let flag_getters = [
            (InteractSignal::TRIGGER, Self::item_is_edited as fn(&Self) -> bool),
            (InteractSignal::COMMIT, Self::item_was_deactivated_after_edit),
            (InteractSignal::ACTIVE, Self::item_was_activated),
            (InteractSignal::OPEN, Self::item_was_toggled_open),
            // bleh...
            (InteractSignal::EXTENDED, Self::is_item_right_clicked),
        ];
        for (flag, getter) in flag_getters {
            if mask.contains(flag) && getter(self) {
                flags.insert(flag);
            }
        }
        flags
    }

    fn window_is_appearing(&self) -> bool;
    fn window_is_collapsed(&self) -> bool;
    fn window_is_focused_untyped(&self, untyped_flags: Option<u32>) -> bool;
    fn window_is_hovered_untyped(&self, untyped_flags: Option<u32>) -> bool;
}
pub trait ImUiWindowExt: ImUiWindow {
    #[inline(always)]
    fn draw<'a>(&'a self) -> impl ImDraw
    where
        &'a Self: ImDraw,
    {
        self
    }
    #[inline(always)]
    fn get_draw_target_dyn(&self) -> &dyn ImDrawTarget {
        unsafe { &*ImUiWindow::get_draw_target_ptr_dyn(&*self).as_ptr() }
    }
    #[inline(always)]
    fn get_draw_target_mut_dyn(&mut self) -> &mut dyn ImDrawTarget {
        unsafe { &mut *ImUiWindow::get_draw_target_ptr_dyn(&*self).as_ptr() }
    }
    #[inline(always)]
    #[cfg(todo)]
    fn get_draw_target_mut(&mut self) -> &mut Self::DrawList
    where
        Self: ImContext,
        Self::DrawList: Sized,
    {
        unsafe { &mut *ImUiWindow::get_draw_target_ptr_dyn(&*self).cast().as_ptr() }
    }

    #[inline(always)]
    fn units(&self) -> &ImSpaces<Self> {
        ImSpaces::from_ref(self)
    }
    #[inline(always)]
    fn cursor_pos(&self) -> ImPos2<WindowSpace> {
        self.cursor_window_pos()
    }
    fn item_bounds(&self) -> Box2<ImSpace> {
        Box2::new(self.item_rect_min(), self.item_rect_max())
    }
    fn item_rect(&self) -> Rect<ImSpace> {
        Rect::new(self.item_rect_min(), self.item_rect_size())
    }

    // shorthand alias and compatibility wrappers
    #[inline(always)]
    fn is_item_clicked(&self) -> bool {
        self.item_is_clicked_with(imw::BUTTON_LMB)
    }
    #[inline(always)]
    fn is_item_right_clicked(&self) -> bool {
        self.item_is_clicked_with(imw::BUTTON_RMB)
    }
    #[inline(always)]
    fn is_item_hovered(&self) -> bool {
        self.item_is_hovered()
    }
    #[inline(always)]
    fn item_is_hovered(&self) -> bool {
        self.item_is_hovered_untyped(None)
    }
    #[inline(always)]
    fn window_is_hovered(&self) -> bool {
        self.window_is_hovered_untyped(None)
    }
    #[inline(always)]
    fn window_is_focused(&self) -> bool {
        self.window_is_focused_untyped(None)
    }

    #[inline(always)]
    fn window_region_size(&self) -> ImSize2<WindowSpace> {
        self.window_content_region_max().to_vector().to_size()
    }
}
impl<U: ?Sized + ImUiWindow> ImUiWindowExt for U {}
#[allow(deprecated)]
impl<'a, U: ?Sized> ImUiWindow for &'a U
where
    Self: ImUi,
    U: ImUiWindow,
{
    #[inline(always)]
    fn get_draw_target_ptr_dyn(&self) -> NonNull<dyn ImDrawTarget> {
        ImUiWindow::get_draw_target_ptr_dyn(*self)
    }
    #[inline(always)]
    fn cursor_screen_pos(&self) -> ImPos2<ImSpace> {
        ImUiWindow::cursor_screen_pos(*self)
    }

    #[inline(always)]
    fn cursor_start_pos(&self) -> ImPos2<WindowSpace> {
        ImUiWindow::cursor_start_pos(*self)
    }
    #[inline(always)]
    fn cursor_window_pos(&self) -> ImPos2<WindowSpace> {
        ImUiWindow::cursor_window_pos(*self)
    }

    #[inline(always)]
    fn content_region_max(&self) -> ImPos2<WindowSpace> {
        ImUiWindow::content_region_max(*self)
    }
    #[inline(always)]
    fn content_region_avail(&self) -> Size2<WindowSpace> {
        ImUiWindow::content_region_avail(*self)
    }
    #[inline(always)]
    fn window_content_region_min(&self) -> ImPos2<WindowSpace> {
        ImUiWindow::window_content_region_min(*self)
    }
    #[inline(always)]
    fn window_content_region_max(&self) -> ImPos2<WindowSpace> {
        ImUiWindow::window_content_region_max(*self)
    }
    #[inline(always)]
    fn window_content_region_width(&self) -> f32 {
        ImUiWindow::window_content_region_width(*self)
    }
    #[inline(always)]
    fn window_pos(&self) -> ImPos2<ImSpace> {
        ImUiWindow::window_pos(*self)
    }
    #[inline(always)]
    fn scroll_offset(&self) -> ImVec2<WindowSpace> {
        ImUiWindow::scroll_offset(*self)
    }
    #[inline(always)]
    fn window_size(&self) -> ImSize2<ImSpace> {
        ImUiWindow::window_size(*self)
    }
    #[inline(always)]
    fn window_flags(&self, mask: InteractSignal) -> InteractSignal {
        ImUiWindow::window_flags(*self, mask)
    }
    #[inline(always)]
    fn font_scale(&self) -> f32 {
        ImUiWindow::font_scale(*self)
    }
    #[inline(always)]
    fn viewport_font_scale(&self) -> f32 {
        ImUiWindow::viewport_font_scale(*self)
    }
    #[inline(always)]
    fn viewport_framebuffer_scale(&self) -> ImVec2<f32> {
        ImUiWindow::viewport_framebuffer_scale(*self)
    }

    #[inline(always)]
    fn to_window_space(&self, v: ImPos2<ImSpace>) -> ImPos2<WindowSpace> {
        ImUiWindow::to_window_space(*self, v)
    }
    #[inline(always)]
    fn from_window_space(&self, v: ImPos2<WindowSpace>) -> ImPos2<ImSpace> {
        ImUiWindow::from_window_space(*self, v)
    }
    #[inline(always)]
    fn window_to_space(&self) -> Transform2<WindowSpace, ImSpace> {
        ImUiWindow::window_to_space(*self)
    }
    #[inline(always)]
    fn space_to_window(&self) -> Transform2<ImSpace, WindowSpace> {
        ImUiWindow::space_to_window(*self)
    }

    #[inline(always)]
    fn item_rect_min(&self) -> ImPos2<ImSpace> {
        ImUiWindow::item_rect_min(*self)
    }
    #[inline(always)]
    fn item_rect_max(&self) -> ImPos2<ImSpace> {
        ImUiWindow::item_rect_max(*self)
    }
    #[inline(always)]
    fn item_rect_size(&self) -> ImSize2<ImSpace> {
        ImUiWindow::item_rect_size(*self)
    }

    #[inline(always)]
    fn item_is_clicked_with(&self, button_id: u32) -> bool {
        ImUiWindow::item_is_clicked_with(*self, button_id)
    }
    #[inline(always)]
    fn item_is_active(&self) -> bool {
        ImUiWindow::item_is_active(*self)
    }
    #[inline(always)]
    fn item_is_focused(&self) -> bool {
        ImUiWindow::item_is_focused(*self)
    }
    #[inline(always)]
    fn item_is_visible(&self) -> bool {
        ImUiWindow::item_is_visible(*self)
    }
    #[inline(always)]
    fn item_is_hovered_untyped(&self, untyped_flags: Option<u32>) -> bool {
        ImUiWindow::item_is_hovered_untyped(*self, untyped_flags)
    }
    #[inline(always)]
    fn item_is_edited(&self) -> bool {
        ImUiWindow::item_is_edited(*self)
    }
    #[inline(always)]
    fn item_was_activated(&self) -> bool {
        ImUiWindow::item_was_activated(*self)
    }
    #[inline(always)]
    fn item_was_deactivated(&self) -> bool {
        ImUiWindow::item_was_deactivated(*self)
    }
    #[inline(always)]
    fn item_was_deactivated_after_edit(&self) -> bool {
        ImUiWindow::item_was_deactivated_after_edit(*self)
    }
    #[inline(always)]
    fn item_was_toggled_open(&self) -> bool {
        ImUiWindow::item_was_toggled_open(*self)
    }
    #[inline(always)]
    fn item_any_hovered(&self) -> bool {
        ImUiWindow::item_any_hovered(*self)
    }
    #[inline(always)]
    fn item_any_active(&self) -> bool {
        ImUiWindow::item_any_active(*self)
    }
    #[inline(always)]
    fn item_any_focused(&self) -> bool {
        ImUiWindow::item_any_focused(*self)
    }
    #[inline(always)]
    fn window_is_focused_untyped(&self, flags: Option<u32>) -> bool {
        ImUiWindow::window_is_focused_untyped(*self, flags)
    }
    #[inline(always)]
    fn window_is_hovered_untyped(&self, flags: Option<u32>) -> bool {
        ImUiWindow::window_is_hovered_untyped(*self, flags)
    }
    #[inline(always)]
    fn window_is_appearing(&self) -> bool {
        ImUiWindow::window_is_appearing(*self)
    }
    #[inline(always)]
    fn window_is_collapsed(&self) -> bool {
        ImUiWindow::window_is_collapsed(*self)
    }
}
pub trait ImDraw: ImUiWindow {
    fn move_cursor(&mut self, pos: ImPos2<WindowSpace>);
    fn move_cursor_screen(&mut self, pos: ImPos2<ImSpace>);
    fn new_line(&mut self);
    fn same_line(&mut self);
    fn same_line_with(&mut self, offset: Option<f32>, spacing: Option<f32>);
    fn dummy_space(&mut self, size: ImSize2);
    fn spacing(&mut self);
    fn separator(&mut self);
    fn bullet(&mut self);

    /// TODO: token?
    fn indent_by(&mut self, amt: Option<f32>);
    fn unindent_by(&mut self, amt: Option<f32>);

    fn item_prepare_open(&mut self, open: bool, cond: ImCondition);
    fn item_prepare_width(&mut self, width: f32);
    fn item_prepare_focus(&mut self, offset: isize);

    fn set_clipboard_text_dyn(&mut self, text: &mut dyn ImStr);
    /// NOTE: failure to retrieve clipboard contents will produce an empty string
    fn with_clipboard_text_dyn(&mut self, out: &mut dyn FnMut(&mut dyn ImStr) -> usize) -> usize;
    #[cfg(todo)]
    fn get_clipboard_text_dyn(&mut self, out: &mut dyn io::Write) -> io::Result<usize>;
    #[cfg(todo)]
    fn get_clipboard_text_into(&mut self, out: &mut Vec<u8>) -> usize;
}
pub trait ImDrawExt: ImDraw {
    #[inline(always)]
    fn set_cursor_pos(&mut self, pos: impl Into<ImPos2<WindowSpace>>) {
        self.move_cursor(pos.into())
    }
    #[inline(always)]
    fn set_cursor_screen_pos(&mut self, pos: impl Into<ImPos2<ImSpace>>) {
        self.move_cursor_screen(pos.into())
    }
    #[inline(always)]
    fn dummy(&mut self, size: impl Into<ImSize2>) {
        self.dummy_space(size.into())
    }
    #[inline(always)]
    fn indent(&mut self) {
        self.indent_by(None)
    }
    #[inline(always)]
    fn unindent(&mut self) {
        self.unindent_by(None)
    }
    #[inline(always)]
    fn set_clipboard_text<S>(&mut self, mut text: S)
    where
        S: ImStrExt,
    {
        text.with_imstr_dyn(|text| self.set_clipboard_text_dyn(text))
    }
    #[inline(always)]
    fn push_item_width<'ui>(&mut self, width: f32) -> UiTokenDyn<'ui>
    where
        Self: ImDrawWindowStack<'ui>,
    {
        self.item_prepare_push_width_dyn(width)
    }
    #[inline(always)]
    fn push_window_size_min<'ui, S>(&mut self, size: S) -> UiTokenDyn<'ui>
    where
        Self: ImDrawWindowStack<'ui>,
        S: Into<ImSize2<ImSpace>>,
    {
        self.window_prepare_push_size_min_dyn(size.into())
    }
    #[inline(always)]
    fn push_child_size_min<'ui, S>(&mut self, size: S) -> UiTokenDyn<'ui>
    where
        Self: ImDrawWindowStack<'ui>,
        S: Into<ImSize2<WindowSpace>>,
    {
        self.window_prepare_push_size_min_dyn(size.into().cast())
    }
    #[inline(always)]
    fn child_prepare_size<'ui, S>(&mut self, size: S, cond: ImCondition)
    where
        Self: ImDrawWindowStack<'ui>,
        S: Into<ImSize2<WindowSpace>>,
    {
        self.window_prepare_size(size.into().cast(), cond)
    }
    #[inline(always)]
    fn child_prepare_size_constraints<'ui, S>(&mut self, min: S, max: S)
    where
        Self: ImDrawWindowStack<'ui>,
        S: Into<ImSize2<WindowSpace>>,
    {
        self.window_prepare_size_constraints(min.into().cast(), max.into().cast())
    }

    #[inline(always)]
    fn push_id<'ui>(&mut self, id: impl IntoImStrId) -> UiTokenDyn<'ui>
    where
        Self: ImDrawWindowStack<'ui>,
    {
        let mut id = id.im_into_id();
        if let Some(id32) = id.im_as_id32() {
            self.push_id32_dyn(id32)
        } else {
            self.push_ident_dyn(&mut id)
        }
    }
    #[inline(always)]
    fn open_popup_by_ident<'ui>(&mut self, ident: impl IntoImStrId, args: imw::DynFlagsContainer)
    where
        Self: ImDrawWindowStack<'ui>,
    {
        let mut ident = ident.im_into_id();
        self.open_popup_by_ident_dyn(&mut ident, args.untyped_flags())
    }
    #[inline(always)]
    fn open_popup<'ui>(&mut self, ident: impl IntoImStrId)
    where
        Self: ImDrawWindowStack<'ui>,
    {
        Self::open_popup_by_ident(self, ident, Default::default())
    }
    #[inline(always)]
    fn tab_bar<'ui>(&mut self, ident: impl IntoImStrId) -> Option<UiTokenDyn<'ui>>
    where
        Self: ImDrawWindowStack<'ui>,
    {
        let mut ident = ident.im_into_id();
        Self::begin_tabs_dyn(self, &mut ident, Default::default())
    }
    #[inline(always)]
    fn tab_item<'ui>(&mut self, mut label: impl ImStr) -> Option<UiTokenDyn<'ui>>
    where
        Self: ImDrawWindowStack<'ui>,
    {
        Self::begin_tab_dyn(self, &mut label, None, Default::default())
    }

    fn is_cursor_inline(&self) -> Option<f32> {
        let x = self.cursor_pos().x;
        let start_x = self.cursor_start_pos().x;
        #[cfg(todo)]
        let start_x = {
            let min_x = self.window_content_region_min().x;
            start_x.max(min_x)
        };
        ((x - start_x).abs() > 2e-1).then_some(x)
    }
    fn reserve_line_checkbox(&mut self, label: &str) -> bool
    where
        Self: ImDrawText + ImUiContext,
    {
        let inline = self.is_cursor_inline();
        let prior_edge = match inline {
            #[cfg(todo)]
            Some(x) => x,
            _ => {
                let startx = self.window_pos().x;
                self.item_rect_max().x - startx
            },
        };
        let is_inline = inline.is_some();
        let (box_w, spacing_w) =
            self.with_style_dyn(|style| (style.indent_spacing(), style.item_spacing().width));
        let text_w = self.calc_text_size(label).width;
        let max_x = self.content_region_max().x;
        let threshold = box_w + spacing_w * 2.0;
        if (max_x - text_w - threshold) > prior_edge {
            let is_inline = false;
            if !is_inline {
                self.same_line();
            }
            true
        } else {
            false
        }
    }
}
impl<U: ?Sized + ImDraw> ImDrawExt for U {}
pub trait ImDrawWindowStack<'ui>: ImDraw {
    #[cfg(todo)]
    fn begin_window(&mut self);
    #[cfg(todo)]
    fn begin_child_window(&mut self);
    #[cfg(todo)]
    fn begin_popup(&mut self);
    #[cfg(todo)]
    fn begin_popup_modal(&mut self);
    #[must_use]
    fn begin_tooltip_dyn(&mut self) -> Option<UiTokenDyn<'ui>>;
    #[must_use]
    fn begin_group_dyn(&mut self) -> UiTokenDyn<'ui>;
    #[must_use]
    fn begin_tabs_dyn(
        &mut self,
        ident: &mut dyn ImStr,
        untyped_flags: Option<u32>,
    ) -> Option<UiTokenDyn<'ui>>;
    #[must_use]
    fn begin_tab_dyn(
        &mut self,
        label: &mut dyn ImStr,
        open: Option<&mut bool>,
        untyped_flags: Option<u32>,
    ) -> Option<UiTokenDyn<'ui>>;
    #[must_use]
    fn push_id32_dyn(&mut self, id: u32) -> UiTokenDyn<'ui>;
    #[must_use]
    fn push_ident_dyn(&mut self, ident: &mut dyn ImStr) -> UiTokenDyn<'ui>;

    fn close_current_popup(&mut self);
    /// ew
    fn open_popup_by_ident_dyn(&mut self, ident: &mut dyn ImStr, untyped_flags: Option<u32>);

    fn item_prepare_push_width_dyn(&mut self, width: f32) -> UiTokenDyn<'ui>;
    /// TODO: this is a style var token...
    fn window_prepare_push_size_min_dyn(&mut self, size: ImSize2<ImSpace>) -> UiTokenDyn<'ui>;
    fn window_prepare_size(&mut self, size: ImSize2<ImSpace>, cond: ImCondition);
    fn window_prepare_pos(&mut self, pos: ImPos2<ImSpace>, cond: ImCondition, pivot: ImVec2<f32>);
    fn window_prepare_focus(&mut self);
    fn window_prepare_item_focus(&mut self) {
        self.item_prepare_focus(0)
    }
    fn window_prepare_content_size(&mut self, size: ImSize2);
    fn window_prepare_collapsed(&mut self, collapsed: bool, cond: ImCondition);
    fn window_prepare_size_constraints(&mut self, min: ImSize2<ImSpace>, max: ImSize2<ImSpace>);
    #[cfg(todo)]
    fn window_prepare_size_constraints_dyn<'a>(
        &mut self,
        min: ImSize2<ImSpace>,
        max: ImSize2<ImSpace>,
        cb: &'a mut dyn FnMut(ImPos2<ImSpace>, ImSize2<ImSpace>, &mut ImSize2<ImSpace>),
    ) -> UiTokenDyn<'a>
    where
        'ui: 'a;

    fn window_defocus_any(&mut self);
}
pub trait ImDrawItemStack<'ui> {
    type StyleTokenItemSpacing: UiToken;
    #[must_use]
    fn push_style_item_spacing(&mut self, spacing: ImVec2) -> Self::StyleTokenItemSpacing;
}
impl<'ui, U: ?Sized> ImDrawItemStack<'ui> for &'_ mut U
where
    U: ImDrawItemStack<'ui>,
    U::StyleTokenItemSpacing: Into<UiTokenDyn<'ui>>,
{
    type StyleTokenItemSpacing = UiTokenDyn<'ui>;
    #[inline(always)]
    fn push_style_item_spacing(&mut self, spacing: ImVec2) -> Self::StyleTokenItemSpacing {
        ImDrawItemStack::push_style_item_spacing(*self, spacing).into()
    }
}
pub trait ImStyle {
    fn indent_spacing(&self) -> f32;
    fn item_spacing(&self) -> ImSize2;
    fn frame_padding(&self) -> ImSize2;
}
impl ImStyle for () {
    fn indent_spacing(&self) -> f32 {
        Default::default()
    }
    fn item_spacing(&self) -> ImSize2 {
        Default::default()
    }
    fn frame_padding(&self) -> ImSize2 {
        Default::default()
    }
}
pub trait ImStyleExt: ImStyle {}
impl<U: ?Sized + ImStyle> ImStyleExt for U {}
pub trait ImDrawTarget {
    fn clip_rect_min(&self) -> ImPos2<ImSpace>;
    fn clip_rect_max(&self) -> ImPos2<ImSpace>;
    /// TODO: move to ext
    fn clip_rect(&self) -> Box2<ImSpace> {
        Box2::new(self.clip_rect_min(), self.clip_rect_max())
    }

    fn add_line(
        &mut self,
        p0: ImPos2<ImSpace>,
        p1: ImPos2<ImSpace>,
        colour: ImColour,
        thickness: Option<f32>,
    );
    fn add_rect_untyped(
        &mut self,
        rect: Box2<ImSpace>,
        colour: ImColour,
        rounding: Option<f32>,
        filled_thickness: Option<Option<f32>>,
        flags_untyped: Option<u32>,
    );
    fn add_quad(
        &mut self,
        points: [ImPos2<ImSpace>; 4],
        colour: ImColour,
        filled_thickness: Option<Option<f32>>,
    );
    fn add_triangle(
        &mut self,
        points: [ImPos2<ImSpace>; 3],
        colour: ImColour,
        filled_thickness: Option<Option<f32>>,
    );
    fn add_circle(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    );
    fn add_ngon(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    );
    fn add_ellipse(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: ImVec2<ImSpace>,
        colour: ImColour,
        rot: Option<f32>,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    );

    #[cfg(todo)]
    fn add_text();
    #[cfg(todo)]
    fn add_bezier_cubic();
    #[cfg(todo)]
    fn add_bezier_quadratic();
}
#[allow(unused)]
impl ImDrawTarget for () {
    #[inline(always)]
    fn clip_rect(&self) -> Box2<ImSpace> {
        Box2::ZERO
    }
    #[inline(always)]
    fn clip_rect_min(&self) -> ImPos2<ImSpace> {
        ImPos2::ZERO
    }
    #[inline(always)]
    fn clip_rect_max(&self) -> ImPos2<ImSpace> {
        ImPos2::ZERO
    }
    #[inline(always)]
    fn add_line(
        &mut self,
        p0: ImPos2<ImSpace>,
        p1: ImPos2<ImSpace>,
        colour: ImColour,
        thickness: Option<f32>,
    ) {
    }
    #[inline(always)]
    fn add_rect_untyped(
        &mut self,
        rect: Box2<ImSpace>,
        colour: ImColour,
        rounding: Option<f32>,
        filled_thickness: Option<Option<f32>>,
        flags_untyped: Option<u32>,
    ) {
    }
    #[inline(always)]
    fn add_quad(
        &mut self,
        points: [ImPos2<ImSpace>; 4],
        colour: ImColour,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
    #[inline(always)]
    fn add_triangle(
        &mut self,
        points: [ImPos2<ImSpace>; 3],
        colour: ImColour,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
    #[inline(always)]
    fn add_circle(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
    #[inline(always)]
    fn add_ngon(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: f32,
        colour: ImColour,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
    #[inline(always)]
    fn add_ellipse(
        &mut self,
        mid: ImPos2<ImSpace>,
        radius: ImVec2<ImSpace>,
        colour: ImColour,
        rot: Option<f32>,
        segments: Option<u32>,
        filled_thickness: Option<Option<f32>>,
    ) {
    }
}
pub trait ImDrawTargetStack<'ui> {
    fn push_clip_rect(&mut self, bounds: Box2<ImSpace>, intersect_current: bool) -> UiTokenDyn<'ui>;
    fn push_clip_rect_fullscreen(&mut self) -> UiTokenDyn<'ui>;
    #[cfg(todo)]
    fn push_texture(&mut self) -> UiTokenDyn<'ui>;
}

imvec_newtype! {
    pub struct ImSpace([f32; 2]);
}
imvec_newtype! {
    pub struct WindowSpace([f32; 2]);
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ImSpaces<U: ?Sized>(pub U);
impl<U: ?Sized> ImSpaces<U> {
    #[inline(always)]
    pub const fn from_ref(spaces: &U) -> &Self {
        unsafe { mem::transmute(spaces) }
    }
}
imvec_newtype! {
    impl{U: ?Sized} TransformMap<WindowSpace, Output = Vector2<ImSpace>> for ImSpaces<U> {
        #[inline(always)]
        fn map(&self, v) { v.cast() }
    }
    impl{U: ?Sized + ImUiWindow} TransformMap<WindowSpace, Output = Point2<ImSpace>> for ImSpaces<U> {
        #[inline(always)]
        fn map(&self, v) { self.0.from_window_space(v) }
    }
    impl{U: ?Sized} TransformMap<ImSpace, Output = Vector2<WindowSpace>> for ImSpaces<U> {
        #[inline(always)]
        fn map(&self, v) { v.cast() }
    }
    impl{U: ?Sized + ImUiWindow} TransformMap<ImSpace, Output = Point2<WindowSpace>> for ImSpaces<U> {
        #[inline(always)]
        fn map(&self, v) { self.0.to_window_space(v) }
    }
}
pub type ImSize2<U = WindowSpace> = Size2<U>;
pub type ImPos2<U = WindowSpace> = Point2<U>;
pub type ImVec2<U = WindowSpace> = Vector2<U>;

impl<V, U> From<ImSpaces<V>> for ImVec2<U>
where
    U: glamour::Unit<Scalar = f32>,
    V: mint::IntoMint<MintType = mint::Vector2<U::Scalar>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<V>) -> ImVec2<U> {
        ImVec2::<U::Scalar>::from(Into::<mint::Vector2<U::Scalar>>::into(v.0)).cast()
    }
}
impl<V, U> From<ImSpaces<V>> for ImSize2<U>
where
    U: glamour::Unit<Scalar = f32>,
    V: mint::IntoMint<MintType = mint::Vector2<U::Scalar>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<V>) -> ImSize2<U> {
        ImSize2::<U::Scalar>::from(Into::<mint::Vector2<U::Scalar>>::into(v.0)).cast()
    }
}
impl<V, U> From<ImSpaces<V>> for ImPos2<U>
where
    U: glamour::Unit<Scalar = f32>,
    V: mint::IntoMint<MintType = mint::Point2<U::Scalar>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<V>) -> ImPos2<U> {
        ImPos2::<U::Scalar>::from(Into::<mint::Point2<U::Scalar>>::into(v.0)).cast()
    }
}
impl<V, U> From<ImSpaces<V>> for glamour::Vector4<U>
where
    U: glamour::Unit<Scalar = f32>,
    V: mint::IntoMint<MintType = mint::Vector4<U::Scalar>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<V>) -> glamour::Vector4<U> {
        glamour::Vector4::<U>::from(Into::<mint::Vector4<U::Scalar>>::into(v.0)).cast()
    }
}
impl<V> From<ImSpaces<V>> for glam::Vec4
where
    V: mint::IntoMint<MintType = mint::Vector4<f32>>,
{
    #[inline(always)]
    fn from(v: ImSpaces<V>) -> glam::Vec4 {
        glamour::Vector4::<f32>::from(v).to_raw()
    }
}
