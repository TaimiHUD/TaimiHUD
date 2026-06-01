use {
    crate::render::element::prelude::*,
    core::{
        any::{Any, TypeId},
        mem::ManuallyDrop,
        task::Context,
    },
    img::draw::state::{TypeContainer, TypeContainerOf},
};

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash)]
pub struct ContainerContextState {
    pub signal_pending: ItemStatus,
}
impl ContainerContextState {
    #[inline]
    pub fn new() -> Self {
        Self { signal_pending: ItemStatus::empty() }
    }
}
impl Default for ContainerContextState {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
pub trait FrameStackContext<'ui>:
    img::draw::state::DrawContextSignal<'ui> + AsRef<&'ui RenderFrameUi> + AsMut<RenderFrameUiMut<'ui>>
{
}
impl<'ui, C> FrameStackContext<'ui> for C where
    C: ?Sized
        + img::draw::state::DrawContextSignal<'ui>
        + AsRef<&'ui RenderFrameUi>
        + AsMut<RenderFrameUiMut<'ui>>
{
}
#[derive(Debug, Clone, Default)]
pub struct RenderFrameUi {
    pub ui_state: UiState,
    pub ui_config: UiConfig,
}
#[derive(Debug)]
pub struct RenderFrameUiMut<'ui> {
    pub frame_state: UiFrameState,
    pub waker_ctx: Context<'ui>,
}
impl<'ui> RenderFrameUiMut<'ui> {
    pub const FLAG_SIGNAL_CHILD: ItemStatus = match () {
        #[cfg(todo)]
        _ => ItemStatus::ACTIVE,
        _ => ItemStatus::EXTENDED,
    };
}
impl<'ui> Clone for RenderFrameUiMut<'ui> {
    fn clone(&self) -> Self {
        Self {
            frame_state: self.frame_state.clone(),
            waker_ctx: Context::from_waker(self.waker_ctx.waker()),
        }
    }
}
#[derive(Debug)]
pub struct FrameContainerScope<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> {
    /// TODO: draw desc instead?
    pub ui_state: &'ui RenderFrameUi,
    /// TODO: draw desc instead?
    pub signal_interest: ItemStatus,
    /// TODO: this could be a pointer to container's scratch space instead?
    pub frame_state: ManuallyDrop<RenderFrameUiMut<'ui>>,
    pub context_state: ManuallyDrop<&'f mut ContainerContextState>,
    pub parent: Option<&'f mut C>,
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> FrameContainerScope<'ui, 'f, C> {
    #[inline]
    pub fn new(frame_ui: &'f mut C, context_state: &'f mut ContainerContextState) -> Self {
        Self {
            ui_state: *AsRef::<&'ui RenderFrameUi>::as_ref(&*frame_ui),
            frame_state: ManuallyDrop::new(AsMut::<RenderFrameUiMut<'ui>>::as_mut(frame_ui).clone()),
            parent: Some(frame_ui),
            signal_interest: ItemStatus::all(),
            context_state: ManuallyDrop::new(context_state),
        }
    }
    #[inline]
    pub fn with_parts(
        frame_state: RenderFrameUiMut<'ui>,
        ui_state: &'ui RenderFrameUi,
        parent: Option<&'f mut C>,
        context_state: &'f mut ContainerContextState,
    ) -> Self {
        Self {
            ui_state,
            frame_state: ManuallyDrop::new(frame_state),
            parent,
            signal_interest: ItemStatus::all(),
            context_state: ManuallyDrop::new(context_state),
        }
    }
    #[inline]
    pub fn begin_child<'a>(
        &'a mut self,
        context_state: &'a mut ContainerContextState,
    ) -> FrameContainerScope<'ui, 'a, Self>
    where
        Self: FrameStackContext<'ui>,
    {
        FrameContainerScope::new(self, context_state)
    }
    #[inline]
    pub fn cast_parent_erase(mut self) -> FrameContainerScope<'ui, 'f, dyn FrameStackContext<'ui> + 'f>
    where
        C: Sized + FrameStackContext<'ui>,
    {
        FrameContainerScope {
            parent: self.parent.take().map(|p| p as &mut _),
            ui_state: self.ui_state,
            frame_state: self.frame_state.clone(),
            signal_interest: self.signal_interest,
            context_state: ManuallyDrop::new(unsafe {
                // is this okay since the destructor doesn't touch it? I guess so...
                ManuallyDrop::take(&mut self.context_state)
            }),
        }
    }
}
impl<'ui, 'f> FrameContainerScope<'ui, 'f, EmptyContext> {
    #[inline]
    pub fn new_root(
        frame_state: RenderFrameUiMut<'ui>,
        ui_state: &'ui RenderFrameUi,
        context_state: &'f mut ContainerContextState,
    ) -> Self {
        Self {
            ui_state,
            frame_state: ManuallyDrop::new(frame_state),
            parent: None,
            signal_interest: ItemStatus::all(),
            context_state: ManuallyDrop::new(context_state),
        }
    }
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> Drop for FrameContainerScope<'ui, 'f, C> {
    fn drop(&mut self) {
        unsafe {
            if let Some(parent) = &mut self.parent {
                *parent.as_mut() = ManuallyDrop::take(&mut self.frame_state);
            } else {
                ManuallyDrop::drop(&mut self.frame_state);
            }
        }
    }
}
impl<'f, 'ui, C: ?Sized + FrameStackContext<'ui>> DrawContextSignal<'ui> for FrameContainerScope<'ui, 'f, C>
where
    Self: TypeContainer<TypeId, Any = dyn Any>,
{
    #[inline]
    fn signal_interest(&self) -> ItemStatus {
        self.signal_interest
    }
    #[inline]
    fn is_pending(&self) -> bool {
        !self.context_state.signal_pending.is_empty()
    }
    fn raise_signal_masked(&mut self, signal: ItemStatus) {
        let fired = self.context_state.signal_pending.is_empty() & !signal.is_empty();
        self.context_state.signal_pending |= signal;
        if let (true, Some(parent)) = (fired, self.parent.as_mut()) {
            (**parent).mask_and_raise_signal(RenderFrameUiMut::FLAG_SIGNAL_CHILD);
        }
    }
    fn handle_pending(&mut self, mask: ItemStatus) -> ItemStatus {
        let take = self.context_state.signal_pending & mask;
        self.context_state.signal_pending.remove(mask);
        take
    }
}
impl<'f, 'ui, C: ?Sized + FrameStackContext<'ui>> img::draw::DrawContext<'ui>
    for FrameContainerScope<'ui, 'f, C>
where
    Self: TypeContainer<TypeId, Any = dyn Any> + DrawContextSignal<'ui>,
{
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> AsRef<UiConfig> for FrameContainerScope<'ui, 'f, C> {
    #[inline]
    fn as_ref(&self) -> &UiConfig {
        &self.ui_state.ui_config
    }
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> AsRef<UiState> for FrameContainerScope<'ui, 'f, C> {
    #[inline]
    fn as_ref(&self) -> &UiState {
        &self.ui_state.ui_state
    }
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> AsRef<UiFrameState> for FrameContainerScope<'ui, 'f, C> {
    #[inline]
    fn as_ref(&self) -> &UiFrameState {
        &self.frame_state.frame_state
    }
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> AsMut<UiFrameState> for FrameContainerScope<'ui, 'f, C> {
    #[inline]
    fn as_mut(&mut self) -> &mut UiFrameState {
        &mut self.frame_state.frame_state
    }
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> AsRef<&'ui RenderFrameUi>
    for FrameContainerScope<'ui, 'f, C>
{
    #[inline]
    fn as_ref<'a>(&'a self) -> &'a &'ui RenderFrameUi {
        &self.ui_state
    }
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> AsMut<RenderFrameUiMut<'ui>>
    for FrameContainerScope<'ui, 'f, C>
{
    #[inline]
    fn as_mut(&mut self) -> &mut RenderFrameUiMut<'ui> {
        &mut self.frame_state
    }
}
impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> TypeContainerOf<Context<'ui>>
    for FrameContainerScope<'ui, 'f, C>
where
    C: TypeContainerOf<Context<'ui>>,
{
    fn concrete_type_in(&self) -> &Context<'ui> {
        let frame_state = &self.frame_state;
        self.parent
            .as_ref()
            .map(|p| (&**p).concrete_type_in())
            .unwrap_or_else(|| &frame_state.waker_ctx)
    }
    fn concrete_type_in_mut(&mut self) -> &mut Context<'ui> {
        let frame_state = &mut self.frame_state;
        self.parent
            .as_mut()
            .map(|p| (&mut **p).concrete_type_in_mut())
            .unwrap_or_else(|| &mut frame_state.waker_ctx)
    }
}

/// *shrug* mostly for insurance when wrapping...
///
/// TODO: match on TypeId is apparently bad because provenance, idk
/// if it's actually inconsistent but construct a jump table if needed I guess
unsafe impl<'ui, 'f, C: ?Sized + FrameStackContext<'ui>> TypeContainer<TypeId>
    for FrameContainerScope<'ui, 'f, C>
where
    C: TypeContainer<TypeId, Any = dyn Any>,
{
    type Any = dyn Any;
    fn any_type_in(&self, id: TypeId) -> Option<&Self::Any> {
        if id == TypeId::of::<UiFrameState>() {
            let inner: &UiFrameState = &self.frame_state.frame_state;
            Some(inner)
        } else {
            self.parent.as_ref().and_then(|c| C::any_type_in(&**c, id))
        }
    }
    fn any_type_in_mut(&mut self, id: TypeId) -> Option<&mut Self::Any> {
        if id == TypeId::of::<UiFrameState>() {
            let inner: &mut UiFrameState = &mut self.frame_state.frame_state;
            Some(inner)
        } else {
            self.parent.as_mut().and_then(|c| C::any_type_in_mut(*c, id))
        }
    }
}
const RENDER_FRAME_TYPE: TypeId = TypeId::of::<RenderFrameUi>();

#[derive(Debug, Copy, Clone)]
pub enum EmptyContext {}
unsafe impl<'ui> TypeContainer<TypeId> for EmptyContext {
    type Any = dyn Any;
    #[inline(always)]
    fn any_type_in(&self, _: TypeId) -> Option<&Self::Any> {
        match *self {}
    }
    #[inline(always)]
    fn any_type_in_mut(&mut self, _: TypeId) -> Option<&mut Self::Any> {
        match *self {}
    }
}
impl<'ui> TypeContainerOf<Context<'ui>> for EmptyContext {
    #[inline(always)]
    fn concrete_type_in(&self) -> &Context<'ui> {
        match *self {}
    }
    #[inline(always)]
    fn concrete_type_in_mut(&mut self) -> &mut Context<'ui> {
        match *self {}
    }
}
impl<'ui> img::draw::state::DrawContextSignal<'ui> for EmptyContext {
    #[inline(always)]
    fn signal_interest(&self) -> ItemStatus {
        match *self {}
    }
    #[inline(always)]
    fn raise_signal_masked(&mut self, _: ItemStatus) {
        match *self {}
    }
    #[inline(always)]
    fn handle_pending(&mut self, _: ItemStatus) -> ItemStatus {
        match *self {}
    }
    #[inline(always)]
    fn is_pending(&self) -> bool {
        match *self {}
    }
}
impl<'ui> img::draw::DrawContext<'ui> for EmptyContext {}
impl<'ui> AsRef<&'ui RenderFrameUi> for EmptyContext {
    #[inline(always)]
    fn as_ref<'a>(&'a self) -> &'a &'ui RenderFrameUi {
        match *self {}
    }
}
impl<'ui> AsMut<RenderFrameUiMut<'ui>> for EmptyContext {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut RenderFrameUiMut<'ui> {
        match *self {}
    }
}
