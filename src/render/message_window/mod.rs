use {
    crate::{
        controller::{pathing::registry::PackPath, Controller},
        render::element::prelude::*,
        settings::{
            state::ui::{AnchorPosition, MessageWindowState as UiState, WindowOpen},
            Settings,
        },
    },
    core::{fmt, mem, ops},
    std::{
        borrow::Cow,
        collections::{btree_map, BTreeMap, BTreeSet},
    },
    taimi_hoard::str_opt_ref,
    taimi_pack::attributes::{
        cell::{GetAttrDynExt, PackValueSet},
        keys,
    },
    taimi_sync::watched::Watched,
};
#[cfg(feature = "paths")]
pub use {
    taimi_meta::packs::id::MarkerId as MessageKey,
    taimi_pack::attributes::cell::PackValueCell as MessageAttrValue,
};
#[cfg(not(feature = "paths"))]
pub type MessageKey = usize;
#[cfg(not(feature = "paths"))]
pub type MessageAttrValue = String;
#[derive(Default, Debug, Clone)]
pub struct MessageBaseDesc {
    pub attrs: PackValueSet,
}
impl MessageBaseDesc {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }
    pub fn id(&self) -> Option<&str> {
        self.attrs
            .get_attr_dyn_ref_of::<keys::NameId>()
            .and_then(str_opt_ref)
    }
    pub fn set_id<S: Into<keys::NameId>>(&mut self, id: S) {
        self.attrs.set_attr_dyn_of(id.into());
    }
    pub fn title(&self) -> Option<&str> {
        self.attrs
            .get_attr_dyn_ref_of::<keys::DisplayName>()
            .and_then(str_opt_ref)
    }
    pub fn set_title<S: Into<keys::DisplayName>>(&mut self, title: S) {
        self.attrs.set_attr_dyn_of(title.into());
    }
    pub fn attribution(&self) -> Option<&str> {
        self.attrs
            .get_attr_dyn_ref_of::<keys::Title>()
            .and_then(str_opt_ref)
    }
    pub fn set_attribution<S: Into<keys::Title>>(&mut self, attribution: S) {
        self.attrs.set_attr_dyn_of(attribution.into());
    }
    pub fn message(&self) -> Option<&str> {
        self.attrs
            .get_attr_dyn_ref_of::<keys::Info>()
            .and_then(str_opt_ref)
    }
    pub fn set_message<S: Into<keys::Info>>(&mut self, msg: S) {
        self.attrs.set_attr_dyn_of(msg.into());
    }
    pub fn tooltip_title(&self) -> Option<&str> {
        self.attrs
            .get_attr_dyn_ref_of::<keys::TipName>()
            .and_then(str_opt_ref)
    }
    pub fn set_tooltip_title<S: Into<keys::TipName>>(&mut self, title: S) {
        self.attrs.set_attr_dyn_of(title.into());
    }
    pub fn tooltip_desc(&self) -> Option<&str> {
        self.attrs
            .get_attr_dyn_ref_of::<keys::TipDescription>()
            .and_then(str_opt_ref)
    }
    pub fn set_tooltip_desc<S: Into<keys::TipDescription>>(&mut self, desc: S) {
        self.attrs.set_attr_dyn_of(desc.into());
    }
    #[cfg(todo)]
    pub fn set_icon<T: Into<keys::IconFile>>(&mut self, icon: T) {
        self.attrs.set_attr_dyn_of(icon.into());
    }
}
#[derive(Debug, Default)]
pub struct MessageItemDesc {
    pub base: MessageBaseDesc,
    pub actions: Vec<MessageActionDesc>,
}
impl MessageItemDesc {
    pub fn with_base(base: MessageBaseDesc) -> Self {
        Self { base, actions: Default::default() }
    }
    #[inline]
    pub fn is_actionable(&self) -> bool {
        !self.actions.is_empty()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.base.is_empty() && !self.is_actionable()
    }
}
impl ops::Deref for MessageItemDesc {
    type Target = MessageBaseDesc;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl ops::DerefMut for MessageItemDesc {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
type MessageActionFn = Box<dyn Fn() + Send>;
//#[derive(Clone)]
pub struct MessageActionDesc {
    pub base: MessageBaseDesc,
    pub action: MessageActionFn,
}
impl MessageActionDesc {
    pub fn blank(action: MessageActionFn) -> Self {
        Self { base: Default::default(), action }
    }
    pub fn with_base(action: MessageActionFn, base: MessageBaseDesc) -> Self {
        Self { base, action }
    }
    pub fn act(&self) {
        (self.action)();
    }
    pub fn is_dismiss(&self) -> bool {
        self.attrs.attr_dyn_or_default::<keys::IsHidden>().into()
    }
    pub fn mark_dismiss(&mut self, is_dismiss: bool) {
        self.attrs.set_attr_dyn_of(keys::IsHidden::from(is_dismiss));
    }
    pub fn is_context_menu(&self) -> bool {
        self.attrs.attr_dyn_or_default::<keys::IsSeparator>().into()
    }
    pub fn mark_context_menu(&mut self, is_context: bool) {
        self.attrs.set_attr_dyn_of(keys::IsSeparator::from(is_context));
    }
}
impl fmt::Debug for MessageActionDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MessageActionDesc")
            .field(&self.base)
            .field(&(&*self.action as *const _))
            .finish()
    }
}
impl ops::Deref for MessageActionDesc {
    type Target = MessageBaseDesc;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl ops::DerefMut for MessageActionDesc {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
#[derive(Debug)]
pub struct MessageItemState {
    pub desc: MessageItemDesc,
}
#[derive(Debug, Clone, Default)]
pub struct MessageItemStash {
    pub strings: BTreeMap<usize, String0>,
}
impl MessageItemStash {
    pub fn clear(&mut self) {
        self.strings.clear();
    }
    pub fn stash_str0(&mut self, key: usize, s: String) -> &Str0 {
        match self.strings.entry(key) {
            btree_map::Entry::Vacant(e) => e.insert(unsafe { String0::from_vec_unchecked(s.into()) }),
            btree_map::Entry::Occupied(e) => {
                let e = e.into_mut();
                #[cfg(todo = "unnecessary")]
                unsafe {
                    *e = String0::from_vec_unchecked(s.into());
                }
                e.as_str0()
            },
        }
    }
    pub fn lookup_str0(&mut self, s: &str) -> &Str0 {
        let key = s.as_ptr() as usize;
        self.strings
            .entry(key)
            .or_insert_with(|| unsafe { String0::from_vec_unchecked(s.to_owned().into()) })
            .as_str0()
    }
    pub fn try_lookup_str0(&mut self, key: usize) -> Option<&Str0> {
        self.strings.get(&key).map(|s| s.as_str0())
    }
    pub fn lookup_str0_with_id(&mut self, s: &str, id: &str) -> &Str0 {
        let key = s.as_ptr() as usize;
        let stashed = match self.strings.entry(key) {
            btree_map::Entry::Vacant(e) => e,
            btree_map::Entry::Occupied(s) => return s.into_mut().as_str0(),
        };
        let translated = crate::render::i18n::LOADER
            .with_fluent_message_and_bundle(id, |m, b| {
                let Some(p) = m.value() else { return None };
                let mut errors = Default::default();
                let s = b.format_pattern(p, None, &mut errors);
                Some(match s {
                    _ if !errors.is_empty() => {
                        log::debug!("i18n message failure? {errors:?}");
                        return None
                    },
                    Cow::Borrowed(s) => String0::format(s),
                    Cow::Owned(s) => unsafe { String0::from_vec_unchecked(s.into()) },
                })
            })
            .flatten();
        stashed
            .insert(translated.unwrap_or_else(|| String0::format(s)))
            .as_str0()
    }
    pub fn lookup_str0_with(&mut self, s: &str, id: Option<&str>) -> &Str0 {
        match id {
            Some(id) => self.lookup_str0_with_id(s, id),
            None => self.lookup_str0(s),
        }
    }
}

#[derive(Debug)]
pub struct MessageWindowState {
    explicit: bool,
    ui_state: Watched<UiState>,
    ui_state_pending: bool,
    ui_state_authorative: bool,
    ui_size_dirty: bool,
    items: BTreeMap<MessageKey, MessageItemState>,
    pins: BTreeSet<MessageKey>,
    item_stash: MessageItemStash,
    scratch_s: String,
}
impl MessageWindowState {
    pub fn new() -> Self {
        Self {
            explicit: false,
            ui_state: Watched::EMPTY,
            ui_state_pending: false,
            ui_state_authorative: false,
            ui_size_dirty: false,
            items: Default::default(),
            pins: Default::default(),
            item_stash: Default::default(),
            scratch_s: String::new(),
        }
    }
    pub fn pre_render(&mut self) {
        if !self.ui_state.is_watching() {
            if let Some(settings) = Settings::try_read() {
                self.ui_state
                    .restart_watching(settings.ui_state.message_window.sender());
            } else {
                return
            }
        };
    }
    pub fn pre_draw(&mut self) -> bool {
        if !self.ui_state.is_watching() {
            return false
        }
        let prev_open = self.ui_state.cached.as_ref().map(|s| s.window.open);
        let ui_state = self.ui_state.read_mut();
        let open = ui_state.window.open;
        if let Some(prev_open) = prev_open {
            if prev_open != open && prev_open.is_closed() {
                self.ui_state_authorative = true;
            }
        }
        open.is_active()
    }
    pub fn post_render(&mut self) {
        if mem::take(&mut self.ui_state_pending) && self.ui_state.is_watching() {
            self.ui_state.commit_cloned();
        }
    }
    pub fn draw_window<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let state = &*self.ui_state;
        if state.window.open.is_closed() {
            return
        }
        let size = state.window_size().clone();
        let state = &state.window;
        let open_prev = state.open;
        let collapsed_prev = open_prev.is_collapsed();
        if collapsed_prev && !crate::exports::runtime::is_ingame().unwrap_or(true) {
            return
        }
        let authorative = mem::take(&mut self.ui_state_authorative);
        let cond = match authorative {
            #[cfg(todo = "unnecessary")]
            true => ImCondition::Always,
            _ => ImCondition::Initial,
        };
        let pos = state.position_abs.get().copied();
        let (pivot, pos) = match pos {
            Some(pos) => (state.anchor, pos),
            None => {
                let pos = ui
                    .with_io_dyn(|io| io.display_size().to_vector() * ImVec2::new(0.84, 0.028))
                    .into();
                (AnchorPosition::TopCentre, pos)
            },
        };
        ui.window_prepare_size(size.into(), cond);
        ui.window_prepare_pos(pos.to_point(), cond, pivot.into());
        if authorative && !collapsed_prev {
            // would be cute to have the title bar at reduced opacity when collapsed,
            // but we'd need to make a fake frameless window underneath? so nahhh
            ui.window_prepare_collapsed(collapsed_prev, ImCondition::Always);
        }
        self.explicit = collapsed_prev && ui.im_io_mod_keys().contains(KeyState::ALT | KeyState::SHIFT);
        let interactive = self.explicit | open_prev.is_visible();
        let args = match ui.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => imw::DynArgsWindow::new(Some({
                let flags_c = collapsed_prev.then_some(Self::IM180_FLAGS_COLLAPSE);
                let flags_i = (!interactive)
                    .then_some(Self::IM180_FLAGS_NOINTERACT)
                    .unwrap_or(0);
                (Self::IM180_FLAGS_PRESET | flags_c.unwrap_or(0) | flags_i) as u32
            })),
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => imw::DynArgsWindow::new(Some({
                let flags_c = collapsed_prev.then_some(Self::IM192_FLAGS_COLLAPSE);
                let flags_i = (!interactive)
                    .then_some(Self::IM192_FLAGS_NOINTERACT)
                    .unwrap_or(0);
                (Self::IM192_FLAGS_PRESET | flags_c.unwrap_or(0) | flags_i) as u32
            })),
            _ => Default::default(),
        };
        let min_size = ui.window_prepare_push_size_min_dyn(UiState::MIN_SIZE.into());
        if self.explicit {
            ui.window_prepare_alpha(0.38);
        }
        let label = ImStrId::new("message-info", fl!("message-window"));
        let mut open = bool::from(open_prev);
        let window = ui.begin_window_with(label, Some(&mut open), args);
        min_size.end();
        let (visible, collapsed) = match window.0 {
            Some(..) => (true, collapsed_prev),
            None => (false, open && ui.window_is_collapsed()),
        };
        let new_pos = open.then(|| ui.window_pos().into());
        let open = match open {
            true if collapsed => WindowOpen::Collapsed,
            open => WindowOpen::new(open),
        };
        self.ui_state_pending |= open_prev != open;
        self.ui_state_pending |= match new_pos {
            Some(..) if ui.window_is_appearing() => false,
            Some(..) if !visible && !self.explicit => false,
            Some(new_pos) => pos != new_pos,
            None => false,
        };
        let ui_state = if self.ui_state_pending {
            self.ui_state.read_mut()
        } else {
            self.ui_state.get_mut()
        };
        ui_state.window.open = open;
        if let Some(pos) = new_pos {
            ui_state.window.position_abs = pos;
            ui_state.window.position_rel =
                ui_state.window.position_abs / ui.with_io_dyn(|io| io.display_size()).to_raw();
            if !collapsed {
                ui_state.set_window_size(size);
            }
        }
        if let Some(_token) = imw::BeginVisible::pop_open(window) {
            self.draw_body(ui)
        }
        if self.ui_state_pending && open.is_closed() {
            self.item_stash.clear();
            self.scratch_s = Default::default();
        }
    }
    #[cfg(taimi_imgui = "180")]
    const IM180_FLAGS_COLLAPSE: im180::sys::ImGuiWindowFlags = {
        im180::sys::ImGuiWindowFlags_NoTitleBar
            // unclear if needed on window or just children?
            | im180::sys::ImGuiWindowFlags_AlwaysUseWindowPadding
    } as im180::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "192")]
    const IM192_FLAGS_COLLAPSE: im192::sys::ImGuiWindowFlags =
        im192::sys::ImGuiWindowFlags_NoTitleBar as im192::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "180")]
    const IM180_FLAGS_NOINTERACT: im180::sys::ImGuiWindowFlags = {
        im180::sys::ImGuiWindowFlags_NoMove
            | im180::sys::ImGuiWindowFlags_NoResize
            | im180::sys::ImGuiWindowFlags_NoMouseInputs
            | im180::sys::ImGuiWindowFlags_NoScrollWithMouse
            | im180::sys::ImGuiWindowFlags_NoBackground
        //| im180::sys::ImGuiWindowFlags_NoSavedSettings
    } as im180::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "192")]
    const IM192_FLAGS_NOINTERACT: im192::sys::ImGuiWindowFlags = {
        im192::sys::ImGuiWindowFlags_NoMove
            | im192::sys::ImGuiWindowFlags_NoResize
            | im192::sys::ImGuiWindowFlags_NoMouseInputs
            | im192::sys::ImGuiWindowFlags_NoScrollWithMouse
            | im192::sys::ImGuiWindowFlags_NoBackground
        //| im192::sys::ImGuiWindowFlags_NoSavedSettings
    } as im192::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "180")]
    const IM180_FLAGS_PRESET: im180::sys::ImGuiWindowFlags = {
        im180::sys::ImGuiWindowFlags_NoFocusOnAppearing
            | im180::sys::ImGuiWindowFlags_NoBringToFrontOnFocus
            | im180::sys::ImGuiWindowFlags_NoNav
    } as im180::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "192")]
    const IM192_FLAGS_PRESET: im192::sys::ImGuiWindowFlags = {
        im192::sys::ImGuiWindowFlags_NoFocusOnAppearing
            | im192::sys::ImGuiWindowFlags_NoBringToFrontOnFocus
            | im192::sys::ImGuiWindowFlags_NoNav
    } as im192::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "180")]
    const IM180_BODY_PRESET: im180::sys::ImGuiWindowFlags = {
        im180::sys::ImGuiWindowFlags_NoFocusOnAppearing
            | im180::sys::ImGuiWindowFlags_NoBringToFrontOnFocus
            | im180::sys::ImGuiWindowFlags_NoNavFocus
            | im180::sys::ImGuiWindowFlags_NoSavedSettings
            | im180::sys::ImGuiWindowFlags_NoScrollbar
            | im180::sys::ImGuiWindowFlags_AlwaysUseWindowPadding
        //| imw::ChildWindow::IM180_BORDER_FLAG
    } as im180::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "192")]
    const IM192_BODY_PRESET: im192::sys::ImGuiWindowFlags = {
        im192::sys::ImGuiWindowFlags_NoFocusOnAppearing
            | im192::sys::ImGuiWindowFlags_NoBringToFrontOnFocus
            | im192::sys::ImGuiWindowFlags_NoNavFocus
            | im192::sys::ImGuiWindowFlags_NoSavedSettings
            | im192::sys::ImGuiWindowFlags_NoScrollbar
    } as im192::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "192")]
    const IM192_BODY_CHILD: im192::sys::ImGuiChildFlags = {
        im192::sys::ImGuiChildFlags_AlwaysUseWindowPadding
            //| Self::IM192_BODY_RESIZE
            | im192::sys::ImGuiChildFlags_AutoResizeY
    } as im192::sys::ImGuiChildFlags;
    #[cfg(taimi_imgui = "180")]
    const IM180_BODY_RESIZE: im180::sys::ImGuiWindowFlags =
        im180::sys::ImGuiWindowFlags_AlwaysAutoResize as _;
    #[cfg(all(taimi_imgui = "192", todo = "unnecessary"))]
    const IM192_BODY_RESIZE: im192::sys::ImGuiChildFlags =
        im192::sys::ImGuiChildFlags_AlwaysAutoResize as _;
    #[cfg(taimi_imgui = "180")]
    const IM180_BODY_NOINTERACT: im180::sys::ImGuiWindowFlags = {
        im180::sys::ImGuiWindowFlags_NoMouseInputs
            | im180::sys::ImGuiWindowFlags_NoScrollWithMouse
            | im180::sys::ImGuiWindowFlags_NoNavInputs
    } as im180::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "192")]
    const IM192_BODY_NOINTERACT: im192::sys::ImGuiWindowFlags = {
        im192::sys::ImGuiWindowFlags_NoMouseInputs
            | im192::sys::ImGuiWindowFlags_NoScrollWithMouse
            | im192::sys::ImGuiWindowFlags_NoNavInputs
    } as im192::sys::ImGuiWindowFlags;
    #[cfg(taimi_imgui = "192")]
    const IM192_BODY_INTERACT: im192::sys::ImGuiChildFlags = (
        // im192::sys::ImGuiChildFlags_Borders
        0u32
    ) as im192::sys::ImGuiChildFlags;

    pub fn draw_body<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let active = !self.is_empty();
        let collapsed = self.ui_state.get_mut().window.open.is_collapsed();
        let interactive = self.explicit | !collapsed;
        let _size_dirty = mem::take(&mut self.ui_size_dirty);
        let args = match ui.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => {
                let size_dirty = _size_dirty | ui.window_is_appearing();
                if size_dirty {
                    let size = imw::ChildWindow::SIZE_NONE
                        .with_height(ui.text_line_height())
                        .cast();
                    ui.window_prepare_size(size, ImCondition::Always);
                }
                let flags_s = size_dirty.then_some(Self::IM180_BODY_RESIZE).unwrap_or(0);
                let flags_i = (!interactive).then_some(Self::IM180_BODY_NOINTERACT);
                imw::ChildWindow::args_child180(
                    Self::IM180_BODY_PRESET | flags_s | flags_i.unwrap_or(0),
                    interactive,
                    None,
                )
            },
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => {
                #[cfg(todo = "unnecessary")]
                let cflags_s = _size_dirty.then_some(Self::IM192_BODY_RESIZE).unwrap_or(0);
                let cflags_s = 0;
                let cflags_i = interactive.then_some(Self::IM192_BODY_INTERACT).unwrap_or(0);
                let flags_i = (!interactive).then_some(Self::IM192_BODY_NOINTERACT);
                imw::ChildWindow::args_child192(
                    Self::IM192_BODY_CHILD | cflags_s | cflags_i,
                    Self::IM192_BODY_PRESET | flags_i.unwrap_or(0),
                    None,
                )
            },
            _ => Default::default(),
        };
        if !self.explicit {
            let mut alpha = if collapsed { 0.28f32 } else { 0.44 };
            if active {
                alpha *= 1.6
            }
            ui.window_prepare_alpha(alpha);
        }
        let child = ui.begin_child_with(c"message-content", args);
        let Some(_token) = imw::BeginVisible::pop_open(child) else { return };
        let collapse = if self.explicit {
            ui.small_button(fl!("unhide")).then_some(WindowOpen::Open)
        } else if collapsed {
            ui.text_disabled(fl!("message-window"));
            None
        } else {
            if !active {
                ui.text_disabled(fl!("inactive"));
            }
            None
        };
        if (self.explicit | (!collapsed & !active)) && ui.is_item_hovered() {
            // TODO: wrapped tooltip
            ui.tooltip_text(fl!("message-window-notice"));
        }
        let collapse = collapse.or(if self.explicit {
            ui.same_line();
            ui.small_button(fl!("close")).then_some(WindowOpen::Closed)
        } else {
            None
        });
        if let Some(open) = collapse {
            self.ui_state_pending = true;
            if collapsed && open.is_collapsed() {
                self.ui_state_authorative = true;
            }
            self.ui_state.get_mut().window.open = open;
        } else if active {
            self.draw_content(ui)
        } else if !collapsed {
            ui.same_line();
            if ui.small_button(fl!("message-sample")) {
                let key = MessageKey::with_uuid(uuid::Uuid::new_v4());
                let mut desc = MessageItemDesc::with_base(MessageBaseDesc::default());
                desc.set_title(fl!("message-sample-title").to_string());
                desc.set_id(*fl!("message-sample-body").id_name());
                desc.set_message("");
                desc.set_attribution(fl!("message-sample").to_string());
                desc.set_tooltip_title(fl!("trigger-info").to_string());
                desc.set_tooltip_desc("zzz");
                let mut action = MessageActionDesc::blank(Box::new(move || {
                    crate::render::RenderEvent::MessageDismiss { key }.try_send();
                }) as Box<_>);
                action.set_id(*fl!("poi-activate-info").id_name());
                desc.actions.push(action);
                self.register_item_with_ui(ui, key, desc);
            }
        }
    }
    pub fn draw_content<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let collapsed = self.ui_state.get_mut().window.open.is_collapsed();
        let interactive = self.explicit | !collapsed;

        let mut scratch = &mut self.scratch_s;
        let mut dismiss_item = None;
        let mut pin_item = None;
        for (i, (key, item)) in self.items.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            let _id = ui.push_id_hash(key);
            let group = interactive.then(|| ui.begin_group());
            if let Some(title) = item.desc.title() {
                let title = match title {
                    #[cfg(todo = "unnecessary")]
                    title => {
                        // ui.text() is one of the few interfaces that doesn't really require null termination
                        scratch.clear();
                        scratch.push_str(title);
                        CSlice::terminate_string(&mut scratch)
                    },
                    s => s,
                };
                ui.text_with_font(NexusLinkFont::Big, title);
            }
            if let Some(msg) = item.desc.message() {
                let msg = match (msg, item.desc.id()) {
                    (msg, Some(id)) => self.item_stash.lookup_str0_with_id(msg, id),
                    (m, _) => {
                        // this is more to unify the type with the i18n cache than anything
                        scratch.clear();
                        scratch.push_str(m);
                        let m = CSlice::terminate_string(&mut scratch);
                        unsafe { Str0::from_c_slice(m) }
                    },
                };
                let _font = ui.push_font(NexusLinkFont::Ui);
                let msg = ImTextDisplay::display_for(ui, msg);
                ui.wrap_display(&msg);
                //ui.text_wrapped(msg.as_c_str());
            }
            let pack_path = key.marker_path::<PackPath>();
            let attribution = item.desc.attribution();
            let has_footer = pack_path.is_some() | attribution.is_some();
            if has_footer {
                ui.spacing();
                ui.indent();
                let _disabled = ui.text_colour_push_index(ImColourIndex::Disabled);
                if let Some(pack_path) = pack_path {
                    let pkey =
                        (pack_path.path.repr() as usize).rotate_left(16) ^ pack_path.root.path as usize;
                    let s = if let Some(s) = self.item_stash.try_lookup_str0(pkey) {
                        s
                    } else {
                        use taimi_hoard::loc::LocationRef;
                        let info = Controller::with_sender(|s| {
                            s.pathing.as_ref().and_then(|p| {
                                p.shared
                                    .packs
                                    .packs
                                    .borrow()
                                    .lookup_ref(&pack_path.root)
                                    .map(|p| p.info.clone())
                            })
                        })
                        .flatten();
                        let s = info.as_ref().map(|i| i.to_string()).unwrap_or_default();
                        self.item_stash.stash_str0(pkey, s)
                    };
                    if !s.is_empty() {
                        ui.text_unformatted(s.as_c_str());
                    }
                }
                if let Some(footer) = attribution {
                    ui.text_wrapped(footer);
                }
                ui.unindent();
            }
            // TODO: transient items should be marked in some way, maybe if InfoRange attr is set?
            let pinnable = pack_path.is_some();
            group.end();
            let (tip_title, tip_desc) = (item.desc.tooltip_title(), item.desc.tooltip_desc());
            let actionable = item.desc.is_actionable();
            let was_clicked = actionable && ui.item_is_clicked_with(imw::BUTTON_LMB);
            let wants_context = ui.is_item_right_clicked();
            let has_tip = tip_title.is_some() | tip_desc.is_some();
            if has_tip && ui.item_is_hovered() {
                // TODO: common tooltip draw stuff is very overdue
                if let Some(_token) = ui.begin_tooltip() {
                    if let Some(title) = tip_title {
                        ui.text_with_font(NexusLinkFont::Big, title);
                    }
                    if let Some(msg) = tip_desc {
                        if tip_title.is_some() {
                            ui.text_wrapped(msg);
                        } else {
                            ui.text_unformatted(msg);
                        }
                    }
                }
            }
            if actionable {
                let _id = ui.push_id(c"message-act");
                let actions = item.desc.actions.iter().enumerate();
                let mut liney = false;
                for (i, act) in actions {
                    let dismiss = act.is_dismiss();
                    if dismiss && was_clicked {
                        act.act();
                    }
                    let context = act.is_context_menu();
                    if dismiss | context {
                        continue
                    }
                    let _id = ui.push_id(i);
                    let title = match (act.title(), act.id()) {
                        (s, Some(id)) => self.item_stash.lookup_str0_with_id(s.unwrap_or(id), id),
                        (None, _) => self.item_stash.lookup_str0_with_id("OK", fl!("okay").id_name()),
                        (Some(s), None) => {
                            // this is more to unify the type with the i18n cache than anything
                            scratch.clear();
                            scratch.push_str(s);
                            let s = CSlice::terminate_string(&mut scratch);
                            unsafe { Str0::from_c_slice(s) }
                        },
                    }
                    .as_c_str();
                    if liney {
                        ui.same_line();
                    }
                    liney = true;
                    let acted = match interactive {
                        true if !collapsed => ui.button(title),
                        true => ui.small_button(title),
                        false => {
                            ui.text_disabled(title);
                            false
                        },
                    };
                    if acted {
                        act.act();
                        continue
                    }
                    let (tip_title, tip_desc) = (act.tooltip_title(), act.tooltip_desc());
                    let has_tip = tip_title.is_some() | tip_desc.is_some();
                    if has_tip && ui.item_is_hovered() {
                        // TODO: common tooltip draw stuff is very overdue
                        if let Some(_token) = ui.begin_tooltip() {
                            if let Some(title) = tip_title {
                                ui.text_with_font(NexusLinkFont::Big, title);
                            }
                            if let Some(msg) = tip_desc {
                                if tip_title.is_some() {
                                    ui.text_wrapped(msg);
                                } else {
                                    ui.text_unformatted(msg);
                                }
                            }
                        }
                    }
                }
            }
            let context_id = c"message-action-context";
            if wants_context {
                ui.open_popup(context_id);
            }
            if let Some(_menu) = ui.begin_popup(context_id, Default::default()) {
                let actions = item
                    .desc
                    .actions
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.is_context_menu());
                for (i, act) in actions {
                    let _id = ui.push_id(i);
                    let title = match (act.title(), act.id()) {
                        (s, Some(id)) => self.item_stash.lookup_str0_with_id(s.unwrap_or(id), id),
                        (None, _) => self.item_stash.lookup_str0_with_id("OK", fl!("okay").id_name()),
                        (Some(s), None) => {
                            // this is more to unify the type with the i18n cache than anything
                            scratch.clear();
                            scratch.push_str(s);
                            let s = CSlice::terminate_string(&mut scratch);
                            unsafe { Str0::from_c_slice(s) }
                        },
                    }
                    .as_c_str();
                    if ui.selectable(title, false) {
                        act.act();
                        continue
                    }
                    let (tip_title, tip_desc) = (act.tooltip_title(), act.tooltip_desc());
                    let has_tip = tip_title.is_some() | tip_desc.is_some();
                    if has_tip && ui.item_is_hovered() {
                        // TODO: common tooltip draw stuff is very overdue
                        if let Some(_token) = ui.begin_tooltip() {
                            if let Some(title) = tip_title {
                                ui.text_with_font(NexusLinkFont::Big, title);
                            }
                            if let Some(msg) = tip_desc {
                                if tip_title.is_some() {
                                    ui.text_wrapped(msg);
                                } else {
                                    ui.text_unformatted(msg);
                                }
                            }
                        }
                    }
                }
                let pinnable = pinnable && !self.pins.contains(key);
                if ui.pressable(fl!("message-dismiss")) {
                    dismiss_item = Some(key.clone());
                }
                if pinnable && ui.pressable(fl!("message-pin")) {
                    pin_item = Some(key.clone());
                }
            }
        }
        if let Some(key) = dismiss_item {
            // explicit request forces removal even if pinned
            self.pins.remove(&key);
            self.remove_item(&key);
        }
        if let Some(key) = pin_item {
            self.pin_item(key);
        }
        ui.separator();
        ui.spacing();
        ui.indent();
        let clear = fl!("clear");
        let clearing = match interactive {
            true => ui.small_button(clear),
            false => {
                ui.text_disabled(clear);
                false
            },
        };
        if clearing {
            self.clear_items();
        }
        ui.unindent();
    }
    /// TODO: resize or prepare for the item maybe?
    pub fn register_item_with_ui<'ui, U>(&mut self, _ui: &mut U, id: MessageKey, desc: MessageItemDesc)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.register_item(id, desc);
    }
    pub fn register_item(&mut self, id: MessageKey, desc: MessageItemDesc) {
        let was_empty = self.items.is_empty();
        let item = MessageItemState { desc };
        let replaced = self.items.insert(id, item).is_some();
        self.ui_size_dirty = true;
        if replaced {
            // TODO: just remove IDs from replaced item
            #[cfg(todo)]
            self.item_stash.strings.clear();
        } else if was_empty && self.ui_state.get().window.open.is_closed() {
            let ui_state = self.ui_state.get_mut();
            ui_state.window.open = WindowOpen::Collapsed;
            self.ui_state_authorative = true;
        }
    }
    pub fn remove_item(&mut self, id: &MessageKey) {
        if self.pins.contains(id) {
            #[cfg(taimi_debug)]
            log::debug!("msg remove blocked by pin");
            return
        }
        let removed = self.items.remove(id).is_some();
        self.ui_size_dirty |= removed;
        if removed && self.items.is_empty() {
            let ui_state = self.ui_state.get_mut();
            if ui_state.window.open.is_collapsed() {
                ui_state.window.open = WindowOpen::Closed;
                // self.ui_state_authorative = true;
            }
        }
    }
    pub fn pin_item(&mut self, id: MessageKey) {
        self.pins.insert(id);
    }
    #[cfg(todo = "unused")]
    pub fn update_item_attr(&mut self, id: &MessageKey, value: MessageAttrValue) -> bool {
        let Some(item) = self.items.get_mut(id) else { return false };
        let key = value.id();
        pack_attr! { match =id_is(key) {
            = keys::Info => {
                if let Some(info) = item.desc.attrs.get_attr_dyn_ref_of::<keys::Info>() {
                    self.item_stash.strings.remove(&(info[..].as_ptr() as usize));
                }
            },
        } }
        #[cfg(todo = "unnecessary")]
        if !found {
            // or just clear whole stash if a string is updated?
            self.item_stash.strings.clear();
        }
        item.desc.attrs.set_attr_dyn(value)
    }
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.pins.clear();
        self.item_stash.clear();
        self.ui_size_dirty = true;
    }
    #[inline]
    pub fn clear_items_matching<F>(&mut self, mut filter: F)
    where
        F: FnMut(&MessageKey) -> bool,
    {
        if self.items.is_empty() {
            return
        }
        self.items.retain(|k, _| !filter(k));
        self.ui_size_dirty = true;
        if self.items.is_empty() {
            self.ui_state.get_mut().window.open = WindowOpen::Closed;
            // self.ui_state_authorative = true;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn window_visibility(&self) -> WindowOpen {
        self.ui_state.get().window.open
    }
}
