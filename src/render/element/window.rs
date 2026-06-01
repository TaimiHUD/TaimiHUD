use {crate::render::element::prelude::*, core::mem};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct WindowState {
    pub status: ItemStatus,
    /// status bits changed to relay to ui
    pub commit: ItemStatus,
    /// status bits changed by ui interaction
    pub changed: ItemStatus,
    pub size: Option<ImSize2<ImSpace>>,
    pub pos: Option<ImPos2<ImSpace>>,
    pub context_state: ContainerContextState,
    /// TODO: move to scratch? implement for 180?
    #[cfg(taimi_imgui = "192")]
    pub flush_size: bool,
}
impl WindowState {
    pub const FLAG_STATE: ItemStatus = match () {
        #[cfg(todo)]
        _ => ItemStatus::OPEN,
        #[cfg(todo)]
        _ => ItemStatus::VISIBLE,
        #[cfg(todo)]
        _ => ItemStatus::EXTENDED,
        _ => ItemStatus::TRIGGER,
    };
    pub const FLAG_VISIBLE: ItemStatus = ItemStatus::VISIBLE;
    pub const FLAG_CLOSED: ItemStatus = ItemStatus::COMMIT;
    pub const FLAG_UNCOLLAPSED: ItemStatus = ItemStatus::OPEN;
    pub const FLAG_KEYBOARD_FOCUS: ItemStatus = ItemStatus::ACTIVE;
    pub const FLAGS_REMEMBER_ON_CLOSE: ItemStatus = Self::FLAG_UNCOLLAPSED;
    pub const FLAGS_INITIAL: ItemStatus = ItemStatus::from_bits_retain(Self::FLAG_UNCOLLAPSED.bits());
    pub const FLAGS_RESTORE_ON_OPEN: ItemStatus =
        ItemStatus::from_bits_retain(Self::FLAG_UNCOLLAPSED.bits() | ItemStatus::FOCUS.bits());
    pub const FLAGS_RETAIN_ON_OPEN: ItemStatus = ItemStatus::from_bits_retain({
        let restore_collapse = match () {
            #[cfg(todo)]
            _ => Self::FLAG_UNCOLLAPSED.bits(),
            _ => ItemStatus::EMPTY.bits(),
        };
        restore_collapse
    });

    pub fn populate_defaults(&mut self) {
        self.status.insert(Self::FLAGS_INITIAL);
    }

    #[inline]
    pub fn open_state(&self) -> bool {
        self.status.contains(Self::FLAG_STATE)
    }

    pub fn set_state(&mut self, state: bool) {
        if state {
            self.set_state_open()
        } else {
            self.set_state_close()
        }
    }
    pub fn set_state_open(&mut self) {
        self.status.insert(Self::FLAG_STATE | Self::FLAGS_RESTORE_ON_OPEN);
        self.commit.insert(Self::FLAG_STATE);
    }
    pub fn set_state_close(&mut self) {
        self.status.remove(Self::FLAG_STATE);
        self.commit.insert(Self::FLAG_STATE);
    }
    #[deprecated = "TODO"]
    pub fn was_closed(&mut self) -> bool {
        if self.changed.contains(Self::FLAG_STATE) & !self.status.contains(Self::FLAG_STATE) {
            self.changed.remove(Self::FLAG_STATE);
            true
        } else {
            false
        }
    }

    #[cfg(taimi_imgui = "180")]
    pub fn window_flags_180(
        &self,
        desc: &WindowDesc,
        config: &UiConfig,
        uistate: &UiState,
    ) -> sys180::ImGuiWindowFlags {
        let focus = self.status.contains(ItemStatus::FOCUS);
        let keyboard_focus = self.status.contains(Self::FLAG_KEYBOARD_FOCUS);
        let appearing = self.commit.contains(Self::FLAG_STATE) && self.open_state();
        let flags = [
            (
                sys180::ImGuiWindowFlags_NoSavedSettings as sys180::ImGuiWindowFlags,
                desc.impersistent,
            ),
            (
                sys180::ImGuiWindowFlags_NoFocusOnAppearing.as_(),
                appearing & !focus,
            ),
            (
                (sys180::ImGuiWindowFlags_NoMove | sys180::ImGuiWindowFlags_NoResize).as_(),
                config.move_requires_focus() & !focus,
            ),
            (sys180::ImGuiWindowFlags_NoNavFocus.as_(), !uistate.nav_allowed),
            (
                sys180::ImGuiWindowFlags_NoNavInputs.as_(),
                !keyboard_focus & (!config.imgui_window_nav() | !focus),
            ),
            (
                (sys180::ImGuiWindowFlags_NoTitleBar | sys180::ImGuiWindowFlags_NoCollapse).as_(),
                desc.no_borders,
            ),
            (
                (sys180::ImGuiWindowFlags_AlwaysAutoResize | sys180::ImGuiWindowFlags_NoResize).as_(),
                desc.fit_contents,
            ),
            (
                sys180::ImGuiWindowFlags_NoScrollWithMouse.as_(),
                config.scroll_requires_focus() & !focus,
            ),
            (
                sys180::ImGuiWindowFlags_NoBringToFrontOnFocus.as_(),
                desc.bg_priority,
            ),
            (
                sys180::ImGuiWindowFlags_NoBackground.as_(),
                !focus && (config.fade_unfocused == Some(0.0) || config.fade_background == Some(0.0)),
            ),
        ];
        IntoIterator::into_iter(flags).filter_map_if().sum_bitor()
    }
    #[cfg(taimi_imgui = "180")]
    #[inline(always)]
    pub fn child_flags_180(
        &self,
        desc: &WindowDesc,
        config: &UiConfig,
        uistate: &UiState,
    ) -> sys180::ImGuiWindowFlags {
        let border_flag = (!desc.no_borders)
            .then_some(imw::ChildWindow::IM180_BORDER_FLAG)
            .unwrap_or(0) as sys180::ImGuiWindowFlags;
        border_flag | self.window_flags_180(desc, config, uistate)
    }
    #[cfg(taimi_imgui = "180")]
    const IM180_FLAGS_INHERIT: sys180::ImGuiWindowFlags = (
        sys180::ImGuiWindowFlags_NoSavedSettings
            | sys180::ImGuiWindowFlags_NoNavFocus
            | sys180::ImGuiWindowFlags_NoNavInputs
            | sys180::ImGuiWindowFlags_NoMouseInputs
            | sys180::ImGuiWindowFlags_NoBackground
            | sys180::ImGuiWindowFlags_NoFocusOnAppearing
        //| sys180::ImGuiWindowFlags_NoScrollbar
    ) as sys180::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "192")]
    pub fn window_flags_192(
        &self,
        desc: &WindowDesc,
        config: &UiConfig,
        uistate: &UiState,
    ) -> sys192::ImGuiWindowFlags {
        let focus = self.status.contains(ItemStatus::FOCUS);
        let keyboard_focus = self.status.contains(Self::FLAG_KEYBOARD_FOCUS);
        let appearing = self.commit.contains(Self::FLAG_STATE) && self.open_state();
        let flags = [
            (
                sys192::ImGuiWindowFlags_NoSavedSettings as sys192::ImGuiWindowFlags,
                desc.impersistent,
            ),
            (
                sys192::ImGuiWindowFlags_NoFocusOnAppearing.as_(),
                appearing & !focus,
            ),
            (
                (sys192::ImGuiWindowFlags_NoMove | sys192::ImGuiWindowFlags_NoResize).as_(),
                config.move_requires_focus() & !focus,
            ),
            (sys192::ImGuiWindowFlags_NoNavFocus.as_(), !uistate.nav_allowed),
            (
                sys192::ImGuiWindowFlags_NoNavInputs.as_(),
                !keyboard_focus & (!config.imgui_window_nav() | !focus),
            ),
            (
                (sys192::ImGuiWindowFlags_NoTitleBar | sys192::ImGuiWindowFlags_NoCollapse).as_(),
                desc.no_borders,
            ),
            (
                (sys192::ImGuiWindowFlags_AlwaysAutoResize | sys192::ImGuiWindowFlags_NoResize).as_(),
                desc.fit_contents,
            ),
            (
                sys192::ImGuiWindowFlags_NoScrollWithMouse.as_(),
                config.scroll_requires_focus() & !focus,
            ),
            (
                (sys192::ImGuiWindowFlags_NoMouseInputs | sys192::ImGuiWindowFlags_NoScrollWithMouse).as_(),
                config.mouse_requires_focus() & !focus,
            ),
            (
                sys192::ImGuiWindowFlags_NoBringToFrontOnFocus.as_(),
                desc.bg_priority,
            ),
            (
                sys192::ImGuiWindowFlags_NoBackground.as_(),
                !focus && (config.fade_unfocused == Some(0.0) || config.fade_background == Some(0.0)),
            ),
        ];
        IntoIterator::into_iter(flags).filter_map_if().sum_bitor()
    }
    #[cfg(taimi_imgui = "192")]
    pub fn child_flags_192(
        &self,
        desc: &WindowDesc,
        config: &UiConfig,
        uistate: &UiState,
    ) -> (sys192::ImGuiChildFlags, sys192::ImGuiWindowFlags) {
        let is_focused = self.status.contains(ItemStatus::FOCUS);
        let appearing = self.commit.contains(Self::FLAG_STATE) && self.open_state();
        let flags = [
            (
                sys192::ImGuiChildFlags_Borders as sys192::ImGuiChildFlags,
                !desc.no_borders,
            ),
            (
                (sys192::ImGuiChildFlags_AutoResizeX | sys192::ImGuiChildFlags_AutoResizeY)
                    as sys192::ImGuiChildFlags,
                desc.fit_contents & !self.flush_size,
            ),
            (
                sys192::ImGuiChildFlags_AlwaysAutoResize.as_(),
                desc.fit_contents & self.flush_size,
            ),
            (
                sys192::ImGuiChildFlags_NavFlattened.as_(),
                config.imgui_keyboard_nav() & uistate.nav_allowed,
            ),
        ];
        let child_flags = IntoIterator::into_iter(flags).filter_map_if().sum_bitor();
        (child_flags, self.window_flags_192(desc, config, uistate))
    }
    #[cfg(taimi_imgui = "192")]
    const IM192_FLAGS_INHERIT: sys192::ImGuiWindowFlags = (
        sys192::ImGuiWindowFlags_NoSavedSettings
            | sys192::ImGuiWindowFlags_NoNavFocus
            | sys192::ImGuiWindowFlags_NoNavInputs
            | sys192::ImGuiWindowFlags_NoMouseInputs
            | sys192::ImGuiWindowFlags_NoBackground
            | sys192::ImGuiWindowFlags_NoFocusOnAppearing
        //| sys192::ImGuiWindowFlags_NoScrollbar
    ) as sys192::ImGuiWindowFlags;
}
#[cfg(todo)]
impl Hash for WindowState {}

#[derive(Debug, Clone, Default)]
pub struct WindowScratch {
    pub title: String0,
}

#[derive(Debug, Default)]
pub struct WindowDesc {
    pub id: &'static Str0,
    /// save no state to ini
    pub impersistent: bool,
    /// always auto-resize
    pub fit_contents: bool,
    /// child window
    pub embed: bool,
    /// no bring-to-front
    pub bg_priority: bool,
    pub no_borders: bool,
    pub size: Option<ImSize2<ImSpace>>,
}
impl WindowDesc {
    #[inline]
    pub fn position_pivot(&self) -> ImVec2<f32> {
        imw::Window::PIVOT_TOPLEFT
    }
}

#[derive(Debug)]
pub struct WindowDraw<'d, 's> {
    pub desc: &'d WindowDesc,
    pub state: &'s mut WindowState,
    pub scratch: &'s mut WindowScratch,
}

#[must_use]
pub struct WindowToken<'ui, 'c, C: ?Sized + DrawContext<'ui>> {
    token: UiTokenDyn<'ui>,
    pub context: FrameContainerScope<'ui, 'c, C>,
}

impl<'d, 's> WindowDraw<'d, 's> {
    pub fn begin_draw<'ui, 'c, U, C>(
        &'c mut self,
        ui: &mut U,
        context: &'c mut C,
    ) -> Option<WindowToken<'ui, 'c, C>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        let prev_open = self.state.open_state();
        let mut open_state = prev_open;
        if !prev_open {
            // if we're confident imgui doesn't want to forcibly open the window via Condition or something,
            // skip it altogether... it may want to be told to close it though?
            return None
        }
        let dims_cond = match self.desc.impersistent {
            false => ImCondition::Initial,
            true => ImCondition::Appear,
        };
        for commit in mem::take(&mut self.state.commit) {
            let set = self.state.status.contains(commit);
            match commit {
                #[cfg(todo)]
                WindowState::FLAG_STATE | ItemStatus::VISIBLE => (),
                WindowState::FLAG_UNCOLLAPSED => ui.window_prepare_collapsed(!set, ImCondition::Always),
                ItemStatus::FOCUS =>
                    if set {
                        ui.window_prepare_focus()
                    } else {
                        ui.window_defocus_any()
                    },
                WindowState::FLAG_KEYBOARD_FOCUS if set => ui.window_prepare_item_focus(),
                _ => (),
            }
        }

        let token = if self.desc.embed {
            let (mut flags, inheritable) = match ui.imgui_version_num() {
                #[cfg(taimi_imgui = "180")]
                Some(im180::VERSION_NUM) => {
                    let flags = self
                        .state
                        .child_flags_180(self.desc, context.as_ref(), context.as_ref());
                    (Some(flags as u32), WindowState::IM180_FLAGS_INHERIT as u32)
                },
                #[cfg(taimi_imgui = "192")]
                Some(im192::VERSION_NUM) => {
                    let (child_flags, mut flags) =
                        self.state
                            .child_flags_192(self.desc, context.as_ref(), context.as_ref());
                    if child_flags & sys192::ImGuiChildFlags_Borders as sys192::ImGuiChildFlags != 0 {
                        // TODO: support passing these flags properly, or abstract it away better!
                        flags |= imw::ChildWindow::IM192_BORDER_FLAG as sys192::ImGuiWindowFlags;
                    }
                    (Some(flags as u32), WindowState::IM192_FLAGS_INHERIT as u32)
                },
                _ => (None::<u32>, 0u32),
            };
            if let Some(flags) = &mut flags {
                let inheritable = *flags & inheritable;
                let uiframe: &mut UiFrameState = context.as_mut();
                *flags |= uiframe.child_window_flags;
                uiframe.child_window_flags |= inheritable;
            }
            let size = self.desc.size.map(|sz| sz.cast());
            let args = imw::DynArgsChildWindow::new(flags, size);
            ui.begin_child_with(self.desc.id.as_c_str(), args)
        } else {
            let (flags, inheritable) = match ui.imgui_version_num() {
                #[cfg(taimi_imgui = "180")]
                Some(im180::VERSION_NUM) => {
                    let flags = self
                        .state
                        .window_flags_180(self.desc, context.as_ref(), context.as_ref());
                    (Some(flags as u32), WindowState::IM180_FLAGS_INHERIT as u32)
                },
                #[cfg(taimi_imgui = "192")]
                Some(im192::VERSION_NUM) => {
                    let flags = self
                        .state
                        .window_flags_192(self.desc, context.as_ref(), context.as_ref());
                    (Some(flags as u32), WindowState::IM192_FLAGS_INHERIT as u32)
                },
                _ => (None::<u32>, 0u32),
            };
            if let Some(flags) = &flags {
                let inheritable = *flags & inheritable;
                let uiframe: &mut UiFrameState = context.as_mut();
                uiframe.child_window_flags = inheritable;
            }
            let size = self.desc.size;
            #[cfg(todo = "unnecessary")]
            let size = (!self.desc.fit_contents).then_some(size).flatten();
            if let Some(size) = size.or(self.desc.size) {
                ui.window_prepare_size(size, dims_cond);
            }
            if let Some(pos) = self.state.pos {
                ui.window_prepare_pos(pos, dims_cond, self.desc.position_pivot());
            }
            if self.scratch.title.is_empty() {
                self.scratch.title =
                    String0::format(ImStrId::with_ident(self.desc.id, I18nRef::new(self.desc.id)));
            }
            let args = imw::DynArgsWidget::new(flags);
            let label_id = self.scratch.title.as_c_str();
            ui.begin_window_with(label_id, Some(&mut open_state), args)
        };
        let mut status = match token.0.is_some() | open_state {
            true => ui.window_flags(
                ItemStatus::TRIGGER | ItemStatus::OPEN | ItemStatus::FOCUS | ItemStatus::HOVER,
            ),
            false => {
                let remember = self.state.status & WindowState::FLAGS_REMEMBER_ON_CLOSE;
                prev_open.then_some(ItemStatus::TRIGGER).unwrap_or_default() | remember
            },
        };
        let appearing_or_disappearing = status.contains(ItemStatus::TRIGGER);
        status.set(WindowState::FLAG_STATE, open_state);
        if token.0.is_some() {
            if status.contains(ItemStatus::FOCUS) && ui.with_io_dyn(|io| io.want_text_input()) {
                status.insert(WindowState::FLAG_KEYBOARD_FOCUS);
            }
            if status.contains(WindowState::FLAG_UNCOLLAPSED) {
                status.insert(WindowState::FLAG_VISIBLE);
            }
            self.state.size = Some(ui.window_size());
            self.state.pos = Some(ui.window_pos());
        }
        context.mask_and_signal_slot(&mut self.state.changed, self.state.status ^ status);
        self.state.status = status;

        if appearing_or_disappearing && open_state {
            // wait a frame to see the actual size maybe...
            self.state.size = None;
        }
        #[cfg(taimi_imgui = "192")]
        {
            self.state.flush_size = false;
        }

        let token = imw::BeginVisible::pop_open(token)?;
        Some(WindowToken {
            token,
            context: FrameContainerScope::new(context, &mut self.state.context_state),
        })
    }
    #[cfg(todo)]
    pub fn end_draw<'ui, U>(state: &mut WindowState, _ui: &mut U, token: WindowToken<'ui>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        #[cfg(todo)]
        if state.size.is_none() {
            state.size = ui.window_size();
        }
        token.0.end();
    }
}
