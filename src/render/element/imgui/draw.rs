use {
    super::prelude::*,
    core::{
        fmt,
        hash::{Hash, Hasher},
        num::NonZero,
    },
    rustc_hash::FxHasher,
    taimi_ui::im::{self, UiTokenDyn},
};

#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub nav_allowed: bool,
}
#[derive(Debug, Clone, Default)]
pub struct UiFrameState {
    /// root window flags that must be inherited by child window panes
    pub child_window_flags: u32,
}

pub(crate) const ID_HASH_SEED: usize = 59;

pub trait ImDrawWindowExt<'ui>: ImDrawWindow<'ui> {
    /// TODO: seed or mix in the current stack, then maybe skip imgui's own hashing here?
    fn push_id_hash<I>(&mut self, id: I) -> UiTokenDyn<'ui>
    where
        I: Hash,
    {
        let mut hasher = FxHasher::with_seed(ID_HASH_SEED);
        id.hash(&mut hasher);
        // beware entropy distribution if truncating...
        let hash = hasher.finish() as usize;
        self.push_id(IntoImStrId::im_into_id(hash))
    }

    fn begin_taimi_window<I, S>(
        &mut self,
        id: I,
        label: S,
        size: (ImSize2<ImSpace>, ImCondition),
        open: &mut bool,
    ) -> Option<UiTokenDyn<'ui>>
    where
        I: fmt::Display,
        S: fmt::Display,
        //S: taimi_ui::im::text::ImStr, I: taimi_ui::im::text::IntoImStrId,
    {
        use crate::settings::state::ui::window::WindowState;

        let args = match self.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => imw::DynArgsWindow::new(Some(
                // TODO: exception for pathing since we want the horiz scroll on child panes instead!
                //im180::sys::ImGuiWindowFlags_HorizontalScrollbar
                im180::sys::ImGuiWindowFlags_NoNavFocus
                // TODO: exception for pathing window if filter is open
                // (but also move to WindowDraw and use settings config flags etc)
                | im180::sys::ImGuiWindowFlags_NoFocusOnAppearing
            )),
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => imw::DynArgsWindow::new(Some(
                //im192::sys::ImGuiWindowFlags_HorizontalScrollbar
                im192::sys::ImGuiWindowFlags_NoNavFocus
                | im192::sys::ImGuiWindowFlags_NoFocusOnAppearing
            )),
            _ => Default::default(),
        };
        let min_size = WindowState::MIN_SIZE.vec2.min(size.0.into());
        let min_size = self.window_prepare_push_size_min_dyn(im::ImSpaces(min_size).into());
        self.window_prepare_size(size.0, size.1);
        let token = self.begin_window_with(ImStrId::new(id, label), Some(open), args);
        min_size.end();
        imw::BeginVisible::pop_open(token)
    }
    fn begin_sidebar<I>(&mut self, id: I) -> Option<UiTokenDyn<'ui>>
    where
        I: IntoImStrId,
    {
        let flags = match self.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => Some(
                im180::sys::ImGuiWindowFlags_HorizontalScrollbar
                // WindowDraw would inherit these automatically...
                | im180::sys::ImGuiWindowFlags_NoFocusOnAppearing
                | im180::sys::ImGuiWindowFlags_NoNavFocus
            ),
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => Some(
                im192::sys::ImGuiWindowFlags_HorizontalScrollbar
                | im192::sys::ImGuiWindowFlags_NoFocusOnAppearing
                | im192::sys::ImGuiWindowFlags_NoNavFocus
            ),
            _ => None,
        };
        let args = imw::DynArgsChildWindow::new(flags, None);
        let token = self.begin_child_with(id, args);
        imw::BeginVisible::pop_open(token)
    }
    fn begin_mainbar<I>(&mut self, id: I) -> Option<UiTokenDyn<'ui>>
    where
        I: IntoImStrId,
    {
        let flags = match self.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => Some(
                im180::sys::ImGuiWindowFlags_HorizontalScrollbar
                // WindowDraw would inherit these automatically...
                | im180::sys::ImGuiWindowFlags_NoFocusOnAppearing
                | im180::sys::ImGuiWindowFlags_NoNavFocus
            ),
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => Some(
                im192::sys::ImGuiWindowFlags_HorizontalScrollbar
                | im192::sys::ImGuiWindowFlags_NoFocusOnAppearing
                | im192::sys::ImGuiWindowFlags_NoNavFocus
            ),
            _ => None,
        };
        let args = imw::DynArgsChildWindow::new(flags, None);
        let token = self.begin_child_with(id, args);
        imw::BeginVisible::pop_open(token)
    }
    fn begin_content<I>(&mut self, id: I, tall: bool) -> Option<UiTokenDyn<'ui>>
    where
        I: IntoImStrId,
    {
        if tall {
            self.window_prepare_size_constraints(
                imw::Window::prepare_height_constraint(100.0),
                imw::Window::CONSTRAINT_NONE,
            );
        }
        let flags = match self.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) if tall => Some(im180::sys::ImGuiWindowFlags_AlwaysVerticalScrollbar),
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) if tall => Some(im192::sys::ImGuiWindowFlags_AlwaysVerticalScrollbar),
            _ => None,
        };
        let args = imw::DynArgsChildWindow::new(flags, None);
        let token = self.begin_child_with(id, args);
        imw::BeginVisible::pop_open(token)
    }
    fn begin_sidebar_tree_node<I, S>(
        &mut self,
        open: (bool, ImCondition),
        id: I,
        label: S,
    ) -> Option<UiTokenDyn<'ui>>
    where
        I: IntoImStrId,
        S: ImStrExt,
    {
        let args = match self.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => imw::TreeNode::IM180_ARGS_FRAMED_NOPUSH,
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => imw::TreeNode::IM192_ARGS_FRAMED_NOPUSH,
            _ => Default::default(),
        };
        self.begin_tree_node(Some(open), id, label, args)
    }
    fn begin_tree_node_framed<I, S>(
        &mut self,
        open: (bool, ImCondition),
        id: I,
        label: S,
        padded: bool,
    ) -> Option<UiTokenDyn<'ui>>
    where
        I: IntoImStrId,
        S: ImStrExt,
    {
        let args = match self.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => {
                let mut args = imw::TreeNode::IM180_ARGS_FRAMED;
                if padded {
                    args.untyped_flags |= im180::sys::ImGuiTreeNodeFlags_FramePadding;
                }
                args
            },
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => {
                let mut args = imw::TreeNode::IM192_ARGS_FRAMED;
                if padded {
                    args.untyped_flags |= im192::sys::ImGuiTreeNodeFlags_FramePadding;
                }
                args
            },
            _ => Default::default(),
        };
        self.begin_tree_node(Some(open), id, label, args)
    }
    fn begin_tree_leaf_wide<I, S>(&mut self, id: I, label: S, padded: bool) -> Option<UiTokenDyn<'ui>>
    where
        I: IntoImStrId,
        S: ImStrExt,
    {
        let args = match self.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => {
                let mut args = imw::DynFlagsContainer::new(Some(
                    im180::sys::ImGuiTreeNodeFlags_Leaf
                        | im180::sys::ImGuiTreeNodeFlags_SpanAvailWidth
                        | im180::sys::ImGuiTreeNodeFlags_NoTreePushOnOpen,
                ));
                if padded {
                    args.untyped_flags |= im180::sys::ImGuiTreeNodeFlags_FramePadding;
                }
                args
            },
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => {
                let mut args = imw::DynFlagsContainer::new(Some(
                    im192::sys::ImGuiTreeNodeFlags_Leaf
                        | im192::sys::ImGuiTreeNodeFlags_SpanAvailWidth
                        | im192::sys::ImGuiTreeNodeFlags_NoTreePushOnOpen,
                ));
                if padded {
                    args.untyped_flags |= im192::sys::ImGuiTreeNodeFlags_FramePadding;
                }
                args
            },
            _ => Default::default(),
        };
        self.begin_tree_node(None, id, label, args)
    }
    fn selectable_dismiss<S>(&mut self, label: S, state: bool, dismiss_popups: bool) -> bool
    where
        S: ImStrExt,
    {
        let args = match dismiss_popups {
            false => match self.imgui_version_num() {
                #[cfg(taimi_imgui = "180")]
                Some(im180::VERSION_NUM) => imw::Selectable::IM180_ARGS_NO_DISMISS_POPUP,
                #[cfg(taimi_imgui = "192")]
                Some(im192::VERSION_NUM) => imw::Selectable::IM192_ARGS_NO_DISMISS_POPUP,
                _ => Default::default(),
            },
            _ => Default::default(),
        };
        self.draw_widget_with(&imw::Selectable, label, state, args)
    }

    fn input_text_managed<S, H>(
        &mut self,
        label: S,
        buf: &mut String,
        additional_cap: usize,
        hint: Option<H>,
        flags: Option<u32>,
    ) -> bool
    where
        S: ImStrExt,
        H: ImStr,
    {
        let (flags, multiline) = match flags {
            None => (None, false),
            Some(f) => match self.imgui_version_num() {
                #[cfg(taimi_imgui = "180")]
                Some(im180::VERSION_NUM) => (
                    Some(f & !im180::sys::ImGuiInputTextFlags_Multiline),
                    f & im180::sys::ImGuiInputTextFlags_Multiline != 0,
                ),
                #[cfg(taimi_imgui = "192")]
                Some(im192::VERSION_NUM) => (
                    Some(f & !im192::sys::ImGuiInputTextFlags_Multiline),
                    f & im192::sys::ImGuiInputTextFlags_Multiline != 0,
                ),
                _ => (Some(f), false),
            },
        };
        #[cfg(todo = "unnecessary")]
        let flags = match (flags, multiline) {
            (Some(0), true) => None,
            (f, _) => f,
        };
        let cap_prev = buf.capacity();
        let len_prev = buf.len();
        let mut paste_len = None;
        let buf_grew = if additional_cap > 0 {
            buf.reserve(additional_cap);
            let grew = buf.capacity() != cap_prev;
            let is_pasting = || {
                self.im_io_key_is_shortcut()
                    && self.with_io_dyn(|io| io.want_text_input() && io.key_is_down_alphanum(b'v'))
            };
            if !grew && is_pasting() {
                // okay but we don't know if this is focused eww
                let clip_len = self.with_clipboard_text_dyn(&mut |clip| {
                    clip.im_as_bstr()
                        .and_then(|s| NonZero::new(s.len()))
                        .map(|l| l.get() + 1)
                        .unwrap_or(additional_cap * 4)
                });
                if cap_prev - buf.len() < clip_len && self.window_is_focused_untyped(Some(0)) {
                    paste_len = Some(clip_len);
                    buf.reserve(clip_len);
                    true
                } else {
                    false
                }
            } else {
                grew
            }
        } else {
            false
        };
        let flags = match buf_grew {
            true => {
                let flags = flags.unwrap_or_else(|| match self.imgui_version_num() {
                    #[cfg(taimi_imgui = "180")]
                    Some(im180::VERSION_NUM) if multiline => imw::InputTextMultiline::IM180_FLAGS_PRESET,
                    #[cfg(taimi_imgui = "180")]
                    Some(im180::VERSION_NUM) => imw::InputText::IM180_FLAGS_PRESET,
                    #[cfg(taimi_imgui = "192")]
                    Some(im192::VERSION_NUM) if multiline => imw::InputTextMultiline::IM192_FLAGS_PRESET,
                    #[cfg(taimi_imgui = "192")]
                    Some(im192::VERSION_NUM) => imw::InputText::IM192_FLAGS_PRESET,
                    _ => 0,
                });
                Some(
                    flags
                        | match self.imgui_version_num() {
                            #[cfg(taimi_imgui = "180")]
                            Some(im180::VERSION_NUM) => im180::sys::ImGuiInputTextFlags_ReadOnly,
                            #[cfg(taimi_imgui = "192")]
                            Some(im192::VERSION_NUM) => im192::sys::ImGuiInputTextFlags_ReadOnly,
                            _ => 0,
                        },
                )
            },
            false => flags,
        };
        let changed = match (flags, multiline) {
            (None, false) => self.input_text(label, &mut *buf, hint),
            (Some(f), false) =>
                self.input_text_with(label, &mut *buf, hint, imw::DynArgsInputText::new(Some(f))),
            (None | Some(0), true) => self.input_text_multiline(label, &mut *buf),
            (Some(f), true) => self.input_text_multiline_with(
                label,
                &mut *buf,
                imw::DynArgsInputMultiline::new(Some(f), None),
            ),
        };
        #[cfg(todo = "unnecessary")]
        match paste_len {
            Some(paste_len)
                if (changed || self.item_is_edited()) && self.item_is_active() && len_prev == buf.len() =>
            {
                buf.reserve(paste_len);
                match self.imgui_version_num() {
                    #[cfg(taimi_imgui = "180")]
                    Some(im180::VERSION_NUM) => unsafe { im180::sys::igSetKeyboardFocusHere(-1) },
                    #[cfg(taimi_imgui = "192")]
                    Some(im192::VERSION_NUM) => unsafe { im192::sys::igSetKeyboardFocusHere(-1) },
                    _ => (),
                }
            },
            _ => (),
        }
        changed
    }
    fn input_text_managed_multiline<S>(
        &mut self,
        label: S,
        buf: &mut String,
        additional_cap: usize,
        flags: Option<u32>,
    ) -> bool
    where
        S: ImStrExt,
    {
        let flags = match (self.imgui_version_num(), flags) {
            #[cfg(taimi_imgui = "180")]
            (Some(im180::VERSION_NUM), f) => Some(
                f.unwrap_or(imw::InputTextMultiline::IM180_FLAGS_PRESET)
                    | im180::sys::ImGuiInputTextFlags_Multiline,
            ),
            #[cfg(taimi_imgui = "192")]
            (Some(im192::VERSION_NUM), f) => Some(
                f.unwrap_or(imw::InputTextMultiline::IM192_FLAGS_PRESET)
                    | im192::sys::ImGuiInputTextFlags_Multiline,
            ),
            (_, f) => f,
        };
        self.input_text_managed(label, buf, additional_cap, imw::IM_STR_NONE, flags)
    }

    /// Derived from belst; <https://github.com/belst/nexus-wingman-uploader/blob/master/src/util.rs>
    ///
    /// TODO: pull out as a reusable &mut dyn FnMut() element instead
    fn tip_marker<F>(&mut self, prompt: &str, f: F) -> bool
    where
        F: FnOnce(&mut Self, bool),
    {
        self.same_line();
        self.text_disabled(im_fmt!("({prompt})"));
        if self.is_item_hovered() {
            let clicked = self.is_item_clicked();
            f(self, clicked);
            clicked
        } else {
            false
        }
    }
    #[inline]
    fn help_marker<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(&mut Self, bool),
    {
        self.tip_marker("?", f)
    }
    #[inline]
    fn attention_marker<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(&mut Self, bool),
    {
        self.tip_marker("!", f)
    }
    /// Derived from belst; <https://github.com/belst/nexus-wingman-uploader/blob/master/src/util.rs>
    #[cfg(todo = "unused")]
    fn link<S, U>(&mut self, label: S, url: U)
    where
        S: ImStrExt,
        U: fmt::Display,
    {
        let colour = self.lookup_style_colour(ImColourIndex::NavCursor);
        self.text_unformatted_coloured(label, colour);
        let text_bounds = self.item_rect();
        self.draw_line(text_bounds.min.with_y(text_bounds.max.y), text_bounds.max, colour);
        if self.is_item_clicked() {
            open(url);
        } else if self.is_item_hovered() {
            let url = url.to_string();
            self.tooltip_text(fl!("open-button", kind = &url));
        }
    }
}
impl<'ui, U: ?Sized> ImDrawWindowExt<'ui> for U where U: ImDrawWindow<'ui> {}
