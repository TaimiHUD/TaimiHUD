use {
    super::prelude::*,
    arcffi::nn,
    core::{
        borrow::BorrowMut,
        ffi::CStr,
        marker::PhantomData,
        mem,
        ops::{self, RangeInclusive},
        ptr::{self, NonNull},
    },
};

pub const BUTTON_LMB: u32 = 0;
pub const BUTTON_RMB: u32 = 1;
pub const BUTTON_MMB: u32 = 2;
pub const FLAGS32_NONE: u32 = 0xc0000000;
pub const F32_NONE: f32 = f32::INFINITY;
pub const VEC2_NONE: ImVec2 = ImVec2::new(F32_NONE, 0.0);
pub const SIZE_NONE: ImSize2 = ImSize2::new(VEC2_NONE.x, VEC2_NONE.y);
pub const POS2_NONE: ImPos2 = ImPos2::new(VEC2_NONE.x, VEC2_NONE.y);
pub const BOX2_NONE: glamour::Box2<WindowSpace> = glamour::Box2::new(POS2_NONE, POS2_NONE);

#[cfg(todo)]
pub type ImStrNone = &'static mut dyn ImStr;
pub type ImStrNone = &'static CStr;
/// concrete type for `Option<impl ImStr>`
pub const IM_STR_NONE: Option<ImStrNone> = None;

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Interacted;
impl Interacted {
    #[inline(always)]
    pub const fn new(interacted: bool) -> Option<Self> {
        match interacted {
            true => Some(Self),
            false => None,
        }
    }

    #[inline(always)]
    pub const fn r#bool(i: Option<Self>) -> bool {
        matches!(i, Some(Interacted))
    }

    #[inline]
    pub fn apply(out: &mut bool, interacted: Option<Self>) -> Option<Self> {
        Self::apply_bool(out, interacted.is_some());
        interacted
    }
    #[inline]
    pub fn apply_bool(out: &mut bool, interacted: bool) -> bool {
        if interacted {
            *out ^= true;
        }
        interacted
    }
    #[inline]
    pub fn apply_with_bool<F>(out: &mut bool, f: F) -> bool
    where
        F: FnOnce(bool) -> bool,
    {
        let prev = *out;
        Self::apply_bool(out, f(prev))
    }
}
impl From<Interacted> for bool {
    #[inline(always)]
    fn from(_: Interacted) -> bool {
        true
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BeginVisible;
impl BeginVisible {
    #[inline(always)]
    pub const fn new(visible: bool) -> Option<Self> {
        match visible {
            true => Some(Self),
            false => None,
        }
    }

    #[inline(always)]
    pub const fn r#bool(i: Option<Self>) -> bool {
        matches!(i, Some(BeginVisible))
    }

    #[inline]
    pub fn pop_open<T: IntoTokenGuard>((vis, token): (Option<Self>, T)) -> Option<T::TokenGuardType> {
        let token = token.into_guard();
        match vis {
            Some(Self) => Some(token),
            None => {
                token.end();
                None
            },
        }
    }
}
impl From<BeginVisible> for bool {
    #[inline(always)]
    fn from(_: BeginVisible) -> bool {
        true
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImCondition {
    Always = 1,
    /// once per session
    Startup = 2,
    /// initial setup or fallback if unpersisted
    Initial = 4,
    Appear = 8,
}
impl ImCondition {
    pub const ALWAYS: (bool, Self) = Self::always(true);
    pub const STARTUP: (bool, Self) = Self::startup(true);
    pub const INITIAL: (bool, Self) = Self::initial(true);
    pub const APPEAR: (bool, Self) = Self::appear(true);

    #[inline(always)]
    pub const fn always<T>(v: T) -> (T, Self) {
        (v, Self::Always)
    }
    #[inline(always)]
    pub const fn startup<T>(v: T) -> (T, Self) {
        (v, Self::Startup)
    }
    #[inline(always)]
    pub const fn initial<T>(v: T) -> (T, Self) {
        (v, Self::Initial)
    }
    #[inline(always)]
    pub const fn appear<T>(v: T) -> (T, Self) {
        (v, Self::Appear)
    }
}

/// for `T: ImPrimitive` and `[T] where T: ImPrimitive`
pub unsafe trait ImPrimitiveData: ImPrimitiveContainer {
    type Primitive: ImPrimitive;
    fn data_ptr(&self) -> NonNull<Self::Primitive>;
    fn data_len(&self) -> usize;
    #[inline(always)]
    fn im_is_n(&self) -> Option<usize> {
        None
    }
    #[cfg(feature = "imgui180")]
    #[inline(always)]
    fn im180_data_type() -> sys180::ImGuiDataType_ {
        <Self::Primitive as ImPrimitive>::im180_data_type()
    }
    #[cfg(feature = "imgui192")]
    fn im192_data_type() -> sys192::ImGuiDataType_ {
        <Self::Primitive as ImPrimitive>::im192_data_type()
    }
}
pub unsafe trait ImPrimitive: Copy + Clone + Default + PartialEq + 'static {
    #[cfg(feature = "imgui180")]
    fn im180_data_type() -> sys180::ImGuiDataType_;
    #[cfg(feature = "imgui192")]
    fn im192_data_type() -> sys192::ImGuiDataType_;
}
macro_rules! im_data_type {
    ($($ty:ty => $dataname:ident,)*) => { $(
        unsafe impl ImPrimitiveData for $ty {
            type Primitive = Self;
            #[inline(always)]
            fn data_ptr(&self) -> NonNull<Self> { nn::nonnull_ref(self) }
            #[inline(always)]
            fn data_len(&self) -> usize { 1 }
            #[inline(always)]
            fn im_is_n(&self) -> Option<usize> { None }
        }
        unsafe impl ImPrimitive for $ty {
            #[cfg(feature = "imgui180")]
            #[inline(always)]
            fn im180_data_type() -> sys180::ImGuiDataType_ {
                sys180::$dataname
            }
            #[cfg(feature = "imgui192")]
            #[inline(always)]
            fn im192_data_type() -> sys192::ImGuiDataType_ {
                sys192::$dataname
            }
        }
    )* };
    ($($ty:ty = $dataname:path,)*) => { $(
        unsafe impl ImPrimitiveData for $ty {
            type Primitive = Self;
            #[inline(always)]
            fn data_ptr(&self) -> NonNull<Self> { nn::nonnull_ref(self) }
            #[inline(always)]
            fn data_len(&self) -> usize { 1 }
            #[inline(always)]
            fn im_is_n(&self) -> Option<usize> { None }
        }
        unsafe impl ImPrimitive for $ty {
            #[cfg(feature = "imgui180")]
            #[inline(always)]
            fn im180_data_type() -> sys180::ImGuiDataType_ {
                <$dataname as ImPrimitive>::im180_data_type()
            }
            #[cfg(feature = "imgui192")]
            #[inline(always)]
            fn im192_data_type() -> sys192::ImGuiDataType_ {
                <$dataname as ImPrimitive>::im192_data_type()
            }
        }
    )* };
}
im_data_type! {
    u8 => ImGuiDataType_U8,
    i8 => ImGuiDataType_S8,
    u16 => ImGuiDataType_U16,
    i16 => ImGuiDataType_S16,
    u32 => ImGuiDataType_U32,
    i32 => ImGuiDataType_S32,
    u64 => ImGuiDataType_U64,
    i64 => ImGuiDataType_S64,
    f32 => ImGuiDataType_Float,
    f64 => ImGuiDataType_Double,
}
#[cfg(target_pointer_width = "32")]
pub type ImPrimitiveUsize = u32;
#[cfg(target_pointer_width = "32")]
pub type ImPrimitiveIsize = i32;
#[cfg(not(target_pointer_width = "32"))]
pub type ImPrimitiveUsize = u64;
#[cfg(not(target_pointer_width = "32"))]
pub type ImPrimitiveIsize = i64;
im_data_type! {
    isize = ImPrimitiveIsize,
    usize = ImPrimitiveUsize,
}
unsafe impl<T: ImPrimitive> ImPrimitiveData for [T] {
    type Primitive = T;
    #[inline(always)]
    fn data_ptr(&self) -> NonNull<T> {
        unsafe { nn::nonnull_ref_unchecked(self.as_ptr()) }
    }
    #[inline(always)]
    fn data_len(&self) -> usize {
        self.len()
    }
    #[inline(always)]
    fn im_is_n(&self) -> Option<usize> {
        Some(self.data_len())
    }
}
unsafe impl<T: ImPrimitive, const N: usize> ImPrimitiveData for [T; N] {
    type Primitive = T;
    #[inline(always)]
    fn data_ptr(&self) -> NonNull<T> {
        unsafe { nn::nonnull_ref_unchecked(self.as_ptr()) }
    }
    #[inline(always)]
    fn data_len(&self) -> usize {
        N
    }
    #[inline(always)]
    fn im_is_n(&self) -> Option<usize> {
        Some(self.data_len())
    }
}
/// object-safe variant of [ImPrimitiveData]
pub unsafe trait ImPrimitiveContainer {
    fn im_data_ptr(&self) -> NonNull<()>;
    fn im_data_len(&self) -> Option<usize>;
    #[cfg(feature = "imgui180")]
    fn get_im180_data_type(&self) -> sys180::ImGuiDataType_;
    #[cfg(feature = "imgui192")]
    fn get_im192_data_type(&self) -> sys192::ImGuiDataType_;
}
unsafe impl<T: ?Sized + ImPrimitiveData> ImPrimitiveContainer for T {
    #[inline]
    fn im_data_ptr(&self) -> NonNull<()> {
        self.data_ptr().cast()
    }
    #[inline]
    fn im_data_len(&self) -> Option<usize> {
        self.im_is_n()
    }
    #[cfg(feature = "imgui180")]
    #[inline]
    fn get_im180_data_type(&self) -> sys180::ImGuiDataType_ {
        <T as ImPrimitiveData>::im180_data_type()
    }
    #[cfg(feature = "imgui192")]
    fn get_im192_data_type(&self) -> sys192::ImGuiDataType_ {
        <T as ImPrimitiveData>::im192_data_type()
    }
}
pub unsafe trait ImPrimitiveArgsRange {
    fn im_untyped_flags(&self) -> Option<u32>;
    fn im_range_min(&self) -> NonNull<()>;
    fn im_range_max(&self) -> NonNull<()>;
}
#[allow(dead_code)]
impl dyn ImPrimitiveArgsRange {
    #[inline(always)]
    unsafe fn im_range_read<T, R>(s: &R) -> RangeInclusive<T>
    where
        R: ?Sized + ImPrimitiveArgsRange,
        T: ImPrimitive,
    {
        unsafe {
            ptr::read(s.im_range_min().cast::<T>().as_ptr())
                ..=ptr::read(s.im_range_max().cast::<T>().as_ptr())
        }
    }
    #[inline(always)]
    unsafe fn im_range_into_args<T, R>(s: &R) -> DynArgsWidgetRange<T>
    where
        R: ?Sized + ImPrimitiveArgsRange,
        T: ImPrimitive,
    {
        DynArgsWidgetRange::new(s.im_untyped_flags(), Self::im_range_read(s))
    }
}
unsafe impl<T: ?Sized + ImPrimitive> ImPrimitiveArgsRange for DynArgsWidgetRange<T> {
    #[inline]
    fn im_untyped_flags(&self) -> Option<u32> {
        self.untyped_flags()
    }
    #[inline]
    fn im_range_min(&self) -> NonNull<()> {
        nn::nonnull_ref(self.range.start()).cast()
    }
    fn im_range_max(&self) -> NonNull<()> {
        nn::nonnull_ref(self.range.end()).cast()
    }
}
unsafe impl<T: ?Sized + ImPrimitive> ImPrimitiveArgsRange for RangeInclusive<T> {
    #[inline]
    fn im_untyped_flags(&self) -> Option<u32> {
        None
    }
    #[inline]
    fn im_range_min(&self) -> NonNull<()> {
        nn::nonnull_ref(self.start()).cast()
    }
    fn im_range_max(&self) -> NonNull<()> {
        nn::nonnull_ref(self.end()).cast()
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[cfg(todo)]
pub struct ContainerOpen<T>(pub T);

pub trait BareWidget<'i> {
    type State: 'i;
    type Args;
    type Output;
}
pub trait LabelledWidget<'i> {
    type State: 'i;
    type Args;
    type Output;
}
pub trait ContainerWidget {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynFlagsContainer {
    pub untyped_flags: u32,
    #[cfg(todo)]
    pub open: bool,
}
impl DynFlagsContainer {
    pub const fn new(untyped_flags: Option<u32>) -> Self {
        Self {
            untyped_flags: match untyped_flags {
                Some(f) => f,
                None => FLAGS32_NONE,
            },
        }
    }

    #[inline]
    pub fn untyped_flags(&self) -> Option<u32> {
        match self.untyped_flags {
            self::FLAGS32_NONE => None,
            v => Some(v),
        }
    }
}
impl Default for DynFlagsContainer {
    fn default() -> Self {
        Self { untyped_flags: FLAGS32_NONE }
    }
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DynArgsSubContainer {
    pub untyped_flags: u32,
    pub size: ImSize2,
}
impl DynArgsSubContainer {
    pub const fn new(untyped_flags: Option<u32>, size: Option<ImSize2>) -> Self {
        Self {
            untyped_flags: match untyped_flags {
                Some(f) => f,
                None => FLAGS32_NONE,
            },
            size: match size {
                Some(s) => s,
                None => SIZE_NONE,
            },
        }
    }

    #[inline]
    pub fn untyped_flags(&self) -> Option<u32> {
        match self.untyped_flags {
            self::FLAGS32_NONE => None,
            v => Some(v),
        }
    }
    #[inline]
    pub fn size(&self) -> Option<ImSize2> {
        (!self.size.width.is_infinite()).then_some(self.size)
    }
}
impl Default for DynArgsSubContainer {
    fn default() -> Self {
        Self {
            untyped_flags: FLAGS32_NONE,
            size: SIZE_NONE,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct DynArgsWidgetRange<T: ImPrimitive> {
    pub untyped_flags: u32,
    pub range: RangeInclusive<T>,
}
impl<T: ImPrimitive> DynArgsWidgetRange<T> {
    pub const fn new(untyped_flags: Option<u32>, range: RangeInclusive<T>) -> Self {
        Self {
            untyped_flags: match untyped_flags {
                Some(f) => f,
                None => FLAGS32_NONE,
            },
            range,
        }
    }

    #[inline]
    pub fn untyped_flags(&self) -> Option<u32> {
        match self.untyped_flags {
            self::FLAGS32_NONE => None,
            v => Some(v),
        }
    }
}
impl<T: ImPrimitive> Default for DynArgsWidgetRange<T>
where
    T: num_traits::Bounded + num_traits::Zero,
{
    fn default() -> Self {
        Self {
            untyped_flags: FLAGS32_NONE,
            range: T::zero()..=T::max_value(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct DynArgsWidgetRangeSized<T: ImPrimitive> {
    pub args: DynArgsWidgetRange<T>,
    pub size: ImSize2,
}
impl<T: ImPrimitive> DynArgsWidgetRangeSized<T> {
    pub const fn new(size: ImSize2, untyped_flags: Option<u32>, range: RangeInclusive<T>) -> Self {
        Self {
            args: DynArgsWidgetRange::new(untyped_flags, range),
            size,
        }
    }
    #[inline]
    pub fn size(&self) -> Option<ImSize2> {
        (!self.size.width.is_infinite()).then_some(self.size)
    }
}
impl<T: ImPrimitive> ops::Deref for DynArgsWidgetRangeSized<T> {
    type Target = DynArgsWidgetRange<T>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.args
    }
}
impl<T: ImPrimitive> ops::DerefMut for DynArgsWidgetRangeSized<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.args
    }
}
impl<T: ImPrimitive> Default for DynArgsWidgetRangeSized<T>
where
    T: num_traits::Bounded + num_traits::Zero,
{
    fn default() -> Self {
        Self {
            args: Default::default(),
            size: SIZE_NONE,
        }
    }
}

pub type DynArgsChildWindow = DynArgsSubContainer;
pub type DynArgsWindow = DynFlagsContainer;
pub type DynArgsWidgetSized = DynArgsChildWindow;
pub type DynArgsWidget = DynFlagsContainer;
pub type DynArgsTreeNode = DynArgsWidget;
pub type DynArgsListbox = DynArgsWidgetSized;

pub type DynArgsInput = DynArgsWidget;
pub type DynArgsInputInt = DynArgsInput;
pub type DynArgsInputScalar = DynArgsWidget;
/// TODO: stuff?
pub type DynArgsInputText = DynArgsInput;
pub type DynArgsInputMultiline = DynArgsWidgetSized;

pub type DynArgsTable = DynArgsWidget;
pub type DynArgsTableRow = DynArgsWidget;
pub type DynArgsTableColumn = DynArgsWidget;
pub type DynArgsSlider<T> = DynArgsWidgetRange<T>;
pub type DynArgsVSlider<T> = DynArgsWidgetRangeSized<T>;

#[derive(Debug, Copy, Clone, Default)]
pub struct Window;
impl<'i> LabelledWidget<'i> for Window {
    type State = Option<&'i mut bool>;
    type Args = DynFlagsContainer;
    type Output = bool;
}
impl ContainerWidget for Window {}
impl Window {
    pub const PIVOT_TOPLEFT: ImVec2<f32> = ImVec2::ZERO;
    pub const PIVOT_CENTRE: ImVec2<f32> = ImVec2::new(0.5, 0.5);
    pub const PIVOT_BOTTOMRIGHT: ImVec2<f32> = ImVec2::ONE;

    pub const CONSTRAINT_NONE: ImSize2<ImSpace> = ImSize2::new(-1.0, -1.0);
    pub const SIZE_NONE: ImSize2<ImSpace> = ImSize2::new(0.0, 0.0);
    pub const fn prepare_width(width: f32) -> ImSize2<ImSpace> {
        ImSize2::new(width, Self::SIZE_NONE.height)
    }
    pub const fn prepare_height(height: f32) -> ImSize2<ImSpace> {
        ImSize2::new(Self::SIZE_NONE.width, height)
    }
    pub const fn prepare_width_constraint(width: f32) -> ImSize2<ImSpace> {
        ImSize2::new(width, Self::CONSTRAINT_NONE.height)
    }
    pub const fn prepare_height_constraint(height: f32) -> ImSize2<ImSpace> {
        ImSize2::new(Self::CONSTRAINT_NONE.width, height)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Popup;
impl<'i> LabelledWidget<'i> for Popup {
    type State = ();
    type Args = DynFlagsContainer;
    type Output = bool;
}
impl ContainerWidget for Popup {}

#[derive(Debug, Copy, Clone, Default)]
pub struct PopupModal;
impl<'i> LabelledWidget<'i> for PopupModal {
    type State = Option<&'i mut bool>;
    type Args = DynFlagsContainer;
    type Output = bool;
}
impl ContainerWidget for PopupModal {}

#[derive(Debug, Copy, Clone, Default)]
pub struct ChildWindow;
impl<'i> LabelledWidget<'i> for ChildWindow {
    type State = ();
    type Args = DynArgsSubContainer;
    type Output = bool;
}
impl ContainerWidget for ChildWindow {}
impl ChildWindow {
    pub const CONSTRAINT_NONE: ImSize2 =
        ImSize2::new(Window::CONSTRAINT_NONE.width, Window::CONSTRAINT_NONE.height);
    pub const SIZE_NONE: ImSize2 = ImSize2::new(Window::SIZE_NONE.width, Window::SIZE_NONE.height);
    #[inline(always)]
    pub const fn prepare_width(width: f32) -> ImSize2<ImSpace> {
        let size = Window::prepare_width(width);
        ImSize2::new(size.width, size.height)
    }
    #[inline(always)]
    pub const fn prepare_height(height: f32) -> ImSize2<ImSpace> {
        let size = Window::prepare_height(height);
        ImSize2::new(size.width, size.height)
    }
    #[inline(always)]
    pub const fn prepare_width_constraint(width: f32) -> ImSize2<ImSpace> {
        let size = Window::prepare_width_constraint(width);
        ImSize2::new(size.width, size.height)
    }
    #[inline(always)]
    pub const fn prepare_height_constraint(height: f32) -> ImSize2<ImSpace> {
        let size = Window::prepare_height_constraint(height);
        ImSize2::new(size.width, size.height)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Tooltip;
impl<'i> BareWidget<'i> for Tooltip {
    type State = ();
    type Args = DynArgsWidget;
    type Output = bool;
}
impl ContainerWidget for Tooltip {}

#[derive(Debug, Copy, Clone, Default)]
pub struct Group;
impl<'i> BareWidget<'i> for Group {
    type State = ();
    type Args = ();
    type Output = bool;
}
impl ContainerWidget for Group {}

#[derive(Debug, Copy, Clone, Default)]
pub struct Table;
impl<'i> BareWidget<'i> for Table {
    type State = ();
    type Args = DynArgsWidget;
    type Output = ();
}
impl ContainerWidget for Table {}

#[derive(Debug, Copy, Clone, Default)]
pub struct TableRow;
impl<'i> BareWidget<'i> for TableRow {
    type State = ();
    type Args = DynArgsWidget;
    type Output = ();
}
impl ContainerWidget for TableRow {}

#[derive(Debug, Copy, Clone, Default)]
pub struct TableColumn;
impl<'i> BareWidget<'i> for TableColumn {
    type State = ();
    type Args = DynArgsWidget;
    type Output = bool;
}
impl ContainerWidget for TableColumn {}

#[derive(Debug, Copy, Clone, Default)]
pub struct Selectable;
impl<'i> LabelledWidget<'i> for Selectable {
    type State = bool;
    type Args = DynArgsWidgetSized;
    type Output = bool;
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Menu;
impl<'i> LabelledWidget<'i> for Menu {
    type State = ();
    type Args = DynArgsWidget;
    type Output = bool;
}
impl ContainerWidget for Menu {}
impl Menu {
    /// pseudo-flag for a boolean arg
    pub const FLAGS_DISABLED: u32 = MenuItem::FLAGS_DISABLED;
    pub const ARGS_DISABLED: DynArgsWidget = DynArgsWidget::new(Some(Self::FLAGS_DISABLED));
}
#[derive(Debug, Copy, Clone, Default)]
pub struct MenuItem;
impl<'i> LabelledWidget<'i> for MenuItem {
    type State = (bool, Option<&'i mut dyn ImStr>);
    type Args = DynArgsWidget;
    type Output = bool;
}
impl MenuItem {
    /// pseudo-flag for a boolean arg
    pub const FLAGS_DISABLED: u32 = 0x2000_0000;
    pub const ARGS_DISABLED: DynArgsWidget = DynArgsWidget::new(Some(Self::FLAGS_DISABLED));
}

#[derive(Debug, Copy, Clone, Default)]
pub struct ProgressBar;
impl<'i> BareWidget<'i> for ProgressBar {
    type State = (f32, Option<&'i mut dyn ImStr>);
    type Args = DynArgsWidgetSized;
    type Output = ();
}
impl ProgressBar {
    #[inline(always)]
    pub const fn prepare_height(height: f32) -> ImSize2 {
        ImSize2::new(f32::MIN, height)
    }
    #[inline(always)]
    pub const fn prepare_width(width: f32) -> ImSize2 {
        ImSize2::new(width, 0.0)
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct Slider<T: ?Sized>(pub PhantomData<fn(T)>);
impl<T: ?Sized> Slider<T> {
    pub const MARKER: Self = Self(PhantomData);
    pub const fn from_marker_ref<'a, U: ?Sized>(_: &'a U) -> &'a Self {
        unsafe { mem::transmute::<&'a PhantomData<T>, _>(&PhantomData) }
    }
    pub const fn marker_ref<'a>() -> &'a Self {
        Self::from_marker_ref(&())
    }
}
impl<T: ?Sized + 'static> Slider<T> {
    pub const MARKER_STATIC: &'static Self = Self::marker_ref();
}
impl<'i, T: ?Sized + ImPrimitiveData + 'i> LabelledWidget<'i> for Slider<T> {
    type State = (&'i mut T, Option<&'i mut dyn ImStr>);
    type Args = DynArgsSlider<T::Primitive>;
    type Output = bool;
}
impl Slider<f64> {
    pub const FLOAT_FORMAT_WHOLE: &'static CStr = c"%.0f";
}
#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct VSlider<T: ?Sized>(pub PhantomData<fn(T)>);
impl<T: ?Sized> VSlider<T> {
    pub const MARKER: Self = Self(PhantomData);
    pub const fn from_marker_ref<'a, U: ?Sized>(_: &'a U) -> &'a Self {
        unsafe { mem::transmute::<&'a PhantomData<T>, _>(&PhantomData) }
    }
    pub const fn marker_ref<'a>() -> &'a Self {
        Self::from_marker_ref(&())
    }
}
impl<T: ?Sized + 'static> VSlider<T> {
    pub const MARKER_STATIC: &'static Self = Self::marker_ref();
}
impl<'i, T: ?Sized + ImPrimitiveData + 'i> LabelledWidget<'i> for VSlider<T> {
    type State = (&'i mut T, Option<&'i mut dyn ImStr>);
    type Args = DynArgsVSlider<T::Primitive>;
    type Output = bool;
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(transparent)]
pub struct InputScalar<T: ?Sized>(pub PhantomData<fn(T)>);
impl<T: ?Sized> InputScalar<T> {
    pub const MARKER: Self = Self(PhantomData);
    pub const fn from_marker_ref<'a, U: ?Sized>(_: &'a U) -> &'a Self {
        unsafe { mem::transmute::<&'a PhantomData<T>, _>(&PhantomData) }
    }
    pub const fn marker_ref<'a>() -> &'a Self {
        Self::from_marker_ref(&())
    }
}
impl<T: ?Sized + 'static> InputScalar<T> {
    pub const MARKER_STATIC: &'static Self = Self::marker_ref();
}
impl<'i, T: ?Sized + ImPrimitiveData + 'i> LabelledWidget<'i> for InputScalar<T> {
    type State = (&'i mut T, Option<&'i mut dyn ImStr>);
    #[cfg(todo)]
    type Args = DynArgsInputScalar<T::Primitive>;
    type Args = DynArgsInputScalar;
    type Output = bool;
}
impl InputScalar<f64> {
    pub const FLOAT_FORMAT_WHOLE: &'static CStr = Slider::FLOAT_FORMAT_WHOLE;
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Image;
impl<'i> BareWidget<'i> for Image {
    type State = &'i dyn ImTexture;
    type Args = DynArgsWidgetSized;
    type Output = ();
}

#[derive(Debug, Copy, Clone, Default)]
pub struct TreeNode;
impl<'i> LabelledWidget<'i> for TreeNode {
    type State = &'i mut dyn ImStr;
    type Args = DynArgsTreeNode;
    type Output = bool;
}
impl ContainerWidget for TreeNode {}

#[derive(Debug, Copy, Clone, Default)]
pub struct Combo;
impl<'i> LabelledWidget<'i> for Combo {
    type State = Option<&'i mut dyn ImStr>;
    type Args = DynArgsWidget;
    type Output = bool;
}
impl ContainerWidget for Combo {}

#[derive(Debug, Copy, Clone, Default)]
pub struct Listbox;
impl<'i> LabelledWidget<'i> for Listbox {
    type State = ();
    type Args = DynArgsListbox;
    type Output = bool;
}
impl ContainerWidget for Listbox {}
impl Listbox {
    /// bottom-right aligned
    ///
    /// `ImSize2::splat(f32::MAX)` equivalent to negative values?
    pub const SIZE_MAX: ImSize2 = ImSize2::new(-1.0, -1.0);
    /// about 7 lines tall with default item width
    pub const SIZE_NONE: ImSize2 = ImSize2::new(0.0, 0.0);
}

#[derive(Debug, Copy, Clone, Default)]
pub struct InputInt;
impl<'i> LabelledWidget<'i> for InputInt {
    /// TODO: should this guarantee utf8?
    type State = &'i mut i32;
    type Args = DynArgsInputInt;
    type Output = bool;
}
#[derive(Debug, Copy, Clone, Default)]
pub struct InputText;
impl<'i> LabelledWidget<'i> for InputText {
    /// TODO: should this guarantee utf8?
    type State = (&'i mut String, Option<&'i mut dyn ImStr>);
    type Args = DynArgsInputText;
    type Output = bool;
}
/// TODO: move [InputText] variants to flags/options
#[derive(Debug, Copy, Clone, Default)]
pub struct InputPassword;
impl<'i> LabelledWidget<'i> for InputPassword {
    type State = (&'i mut String, Option<&'i mut dyn ImStr>);
    type Args = <InputText as LabelledWidget<'i>>::Args;
    type Output = <InputText as LabelledWidget<'i>>::Output;
}
#[derive(Debug, Copy, Clone, Default)]
pub struct InputTextMultiline;
impl<'i> LabelledWidget<'i> for InputTextMultiline {
    /// TODO: should this guarantee utf8?
    type State = &'i mut String;
    type Args = DynArgsInputMultiline;
    type Output = bool;
}

pub trait ImWidgetBare<'ui, 'i, W>
where
    W: BareWidget<'i>,
{
    fn bare_widget(&mut self, widget: &'ui W, state: W::State, args: W::Args) -> W::Output;
}
impl<'i, 'ui, U: ?Sized, W> ImWidgetBare<'ui, 'i, W> for U
where
    U: ImDraw,
    W: BareWidget<'i> + 'ui,
    for<'l> (&'ui W, W::State, W::Args): ImWidget<U, Output = W::Output>,
{
    #[inline]
    fn bare_widget(&mut self, widget: &'ui W, state: W::State, args: W::Args) -> W::Output {
        (widget, state, args).draw_widget(self)
    }
}
pub trait ImWidgetLabelled<'ui, 'i, W>
where
    W: LabelledWidget<'i>,
{
    fn labelled_widget(
        &mut self,
        widget: &'ui W,
        label: &mut dyn ImStr,
        state: W::State,
        args: W::Args,
    ) -> W::Output;
}
impl<'i, 'ui, U: ?Sized, W> ImWidgetLabelled<'ui, 'i, W> for U
where
    U: ImDraw,
    W: LabelledWidget<'i> + 'ui,
    for<'l> (&'ui W, &'l mut dyn ImStr, W::State, W::Args): ImWidget<U, Output = W::Output>,
{
    #[inline]
    fn labelled_widget(
        &mut self,
        widget: &'ui W,
        label: &mut dyn ImStr,
        state: W::State,
        args: W::Args,
    ) -> W::Output {
        (widget, label, state, args).draw_widget(self)
    }
}
pub trait ImWidgetBareContainer<'ui, 'i, W>
where
    W: ContainerWidget + BareWidget<'i>,
{
    type BeginOutput: 'ui;
    fn bare_begin_contain(&mut self, widget: &'ui W, state: W::State, args: W::Args) -> Self::BeginOutput;
}
impl<'i, 'ui, U: ?Sized, W, O> ImWidgetBareContainer<'ui, 'i, W> for U
where
    for<'l> (&'ui W, W::State, W::Args): ImWidget<U, Output = O>,
    O: 'ui,
    U: ImDraw,
    W: ContainerWidget + BareWidget<'i> + 'ui,
{
    type BeginOutput = O;
    #[inline]
    fn bare_begin_contain(&mut self, widget: &'ui W, state: W::State, args: W::Args) -> Self::BeginOutput {
        (widget, state, args).draw_widget(self)
    }
}

pub trait ImWidgetLabelledContainer<'ui, 'i, W>
where
    W: ContainerWidget + LabelledWidget<'i>,
{
    type BeginOutput: 'ui;
    fn labelled_begin_contain(
        &mut self,
        widget: &'ui W,
        label: &mut dyn ImStr,
        state: W::State,
        args: W::Args,
    ) -> Self::BeginOutput;
}
impl<'i, 'ui, U: ?Sized, W, O> ImWidgetLabelledContainer<'ui, 'i, W> for U
where
    for<'l> (&'ui W, &'l mut dyn ImStr, W::State, W::Args): ImWidget<U, Output = O>,
    O: 'ui,
    U: ImDraw,
    W: ContainerWidget + LabelledWidget<'i> + 'ui,
{
    type BeginOutput = O;
    #[inline]
    fn labelled_begin_contain(
        &mut self,
        widget: &'ui W,
        label: &mut dyn ImStr,
        state: W::State,
        args: W::Args,
    ) -> Self::BeginOutput {
        (widget, label, state, args).draw_widget(self)
    }
}
pub trait ImWidgetExt<'ui> {
    #[inline(always)]
    fn draw_a_widget_with<'i, W>(&mut self, widget: &'ui W, state: W::State, args: W::Args) -> W::Output
    where
        W: BareWidget<'i> + 'ui,
        Self: ImWidgetBare<'ui, 'i, W>,
    {
        ImWidgetBare::bare_widget(self, widget, state, args)
    }
    #[inline(always)]
    fn draw_a_widget<'i, W>(&mut self, widget: &'ui W, state: W::State) -> W::Output
    where
        W: BareWidget<'i> + 'ui,
        W::Args: Default,
        Self: ImWidgetBare<'ui, 'i, W>,
    {
        ImWidgetExt::draw_a_widget_with(self, widget, state, <W::Args>::default())
    }
    #[inline(always)]
    fn begin_a_widget_with<'i, W>(
        &mut self,
        widget: &'ui W,
        state: W::State,
        args: W::Args,
    ) -> <Self as ImWidgetBareContainer<'ui, 'i, W>>::BeginOutput
    where
        W: BareWidget<'i> + ContainerWidget + 'ui,
        Self: ImWidgetBareContainer<'ui, 'i, W>,
    {
        ImWidgetBareContainer::bare_begin_contain(self, widget, state, args)
    }
    #[inline(always)]
    fn begin_a_container_with<'i, W, O>(
        &mut self,
        widget: &'ui W,
        state: W::State,
        args: W::Args,
    ) -> <Self as ImWidgetBareContainer<'ui, 'i, W>>::BeginOutput
    where
        W: BareWidget<'i> + ContainerWidget + 'ui,
        Self: ImWidgetBareContainer<'ui, 'i, W, BeginOutput = Option<UiTokenDyn<'ui>>>,
    {
        ImWidgetBareContainer::bare_begin_contain(self, widget, state, args)
    }

    #[inline(always)]
    fn draw_widget_with<'i, W, S>(
        &mut self,
        widget: &'ui W,
        mut label: S,
        state: W::State,
        args: W::Args,
    ) -> W::Output
    where
        W: LabelledWidget<'i> + 'ui,
        S: ImStrExt,
        Self: ImWidgetLabelled<'ui, 'i, W>,
    {
        label.with_imstr_dyn(|label| ImWidgetLabelled::labelled_widget(self, widget, label, state, args))
    }
    #[inline(always)]
    fn draw_widget_labelled<'i, W, S>(&mut self, widget: &'ui W, label: S, state: W::State) -> W::Output
    where
        W: LabelledWidget<'i> + 'ui,
        W::Args: Default,
        S: ImStrExt,
        Self: ImWidgetLabelled<'ui, 'i, W>,
    {
        ImWidgetExt::draw_widget_with(self, widget, label, state, <W::Args>::default())
    }
    #[inline(always)]
    fn begin_widget_with<'i, W, S, O>(
        &mut self,
        widget: &'ui W,
        mut label: S,
        state: W::State,
        args: W::Args,
    ) -> <Self as ImWidgetLabelledContainer<'ui, 'i, W>>::BeginOutput
    where
        W: LabelledWidget<'i> + ContainerWidget + 'ui,
        S: ImStrExt,
        Self: ImWidgetLabelledContainer<'ui, 'i, W, BeginOutput = (O, UiTokenDyn<'ui>)>,
    {
        label.with_imstr_dyn(|label| {
            ImWidgetLabelledContainer::labelled_begin_contain(self, widget, label, state, args)
        })
    }
    #[inline(always)]
    fn begin_widget_labelled<'i, W, S, O>(
        &mut self,
        widget: &'ui W,
        label: S,
        state: W::State,
    ) -> <Self as ImWidgetLabelledContainer<'ui, 'i, W>>::BeginOutput
    where
        W: LabelledWidget<'i> + ContainerWidget + 'ui,
        W::Args: Default,
        S: ImStrExt,
        Self: ImWidgetLabelledContainer<'ui, 'i, W, BeginOutput = (O, UiTokenDyn<'ui>)>,
    {
        ImWidgetExt::begin_widget_with(self, widget, label, state, <W::Args>::default())
    }
    #[inline(always)]
    fn begin_container_with<'i, W, S>(
        &mut self,
        widget: &'ui W,
        mut label: S,
        state: W::State,
        args: W::Args,
    ) -> Option<UiTokenDyn<'ui>>
    where
        W: LabelledWidget<'i> + ContainerWidget + 'ui,
        S: ImStrExt,
        Self: ImWidgetLabelledContainer<'ui, 'i, W, BeginOutput = Option<UiTokenDyn<'ui>>>,
    {
        label.with_imstr_dyn(|label| {
            ImWidgetLabelledContainer::labelled_begin_contain(self, widget, label, state, args)
        })
    }
    #[inline(always)]
    fn begin_container_labelled<'i, W, S>(
        &mut self,
        widget: &'ui W,
        label: S,
        state: W::State,
    ) -> Option<UiTokenDyn<'ui>>
    where
        W: LabelledWidget<'i> + ContainerWidget + 'ui,
        W::Args: Default,
        S: ImStrExt,
        Self: ImWidgetLabelledContainer<'ui, 'i, W, BeginOutput = Option<UiTokenDyn<'ui>>>,
    {
        ImWidgetExt::begin_container_with(self, widget, label, state, <W::Args>::default())
    }

    #[inline(always)]
    fn begin_window_with<S>(
        &mut self,
        label: S,
        state: Option<&mut bool>,
        args: DynArgsWindow,
    ) -> (Option<BeginVisible>, UiTokenDyn<'ui>)
    where
        for<'i> Self: ImWidgetLabelledContainer<
            'ui,
            'i,
            Window,
            BeginOutput = (Option<BeginVisible>, UiTokenDyn<'ui>),
        >,
        Self: ImDraw,
        S: ImStrExt,
    {
        self.begin_widget_with(&Window, label, state, args)
    }
    #[inline(always)]
    fn begin_child_with<I>(
        &mut self,
        id: I,
        args: DynArgsChildWindow,
    ) -> (Option<BeginVisible>, UiTokenDyn<'ui>)
    where
        for<'i> Self: ImWidgetLabelledContainer<
            'ui,
            'i,
            ChildWindow,
            BeginOutput = (Option<BeginVisible>, UiTokenDyn<'ui>),
        >,
        Self: ImDraw,
        I: IntoImStrId,
    {
        let mut label = id.im_into_id();
        self.begin_widget_with(&ChildWindow, &mut label as &mut dyn ImStr, (), args)
    }
    #[inline(always)]
    fn begin_tooltip(&mut self) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetBareContainer<'ui, 'i, Tooltip, BeginOutput = Option<UiTokenDyn<'ui>>>,
    {
        self.begin_a_widget_with(&Tooltip, (), Default::default())
    }
    #[inline(always)]
    fn begin_group(&mut self) -> UiTokenDyn<'ui>
    where
        for<'i> Self: ImWidgetBareContainer<'ui, 'i, Group, BeginOutput = UiTokenDyn<'ui>>,
    {
        self.begin_a_widget_with(&Group, (), Default::default())
    }
    #[inline(always)]
    fn begin_popup<S>(&mut self, id: S, args: DynArgsWindow) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, Popup, BeginOutput = Option<UiTokenDyn<'ui>>>,
        S: ImStrExt,
    {
        self.begin_container_with(&Popup, id, (), args)
    }
    #[inline(always)]
    fn begin_popup_modal<S>(
        &mut self,
        label: S,
        args: DynArgsWindow,
        open: Option<&mut bool>,
    ) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, PopupModal, BeginOutput = Option<UiTokenDyn<'ui>>>,
        S: ImStrExt,
    {
        self.begin_container_with(&PopupModal, label, open, args)
    }
    #[inline(always)]
    fn begin_tree_node<I, S>(
        &mut self,
        open: Option<(bool, ImCondition)>,
        id: I,
        label: S,
        args: DynArgsWidget,
    ) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, TreeNode, BeginOutput = Option<UiTokenDyn<'ui>>>,
        Self: ImDraw,
        I: IntoImStrId,
        S: ImStrExt,
    {
        if let Some((open, cond)) = open {
            self.item_prepare_open(open, cond);
        }
        let mut id = id.im_into_id();
        self.begin_container_with(&TreeNode, label, &mut id, args)
    }
    #[inline(always)]
    fn begin_combo_opt<S, P>(&mut self, label: S, preview: Option<P>) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, Combo, BeginOutput = Option<UiTokenDyn<'ui>>>,
        S: ImStrExt,
        P: ImStrExt,
    {
        match preview {
            Some(preview) => self.begin_combo(label, preview),
            None => self.begin_container_labelled(&Combo, label, None),
        }
    }
    #[inline(always)]
    fn begin_combo<S, P>(&mut self, label: S, mut preview: P) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, Combo, BeginOutput = Option<UiTokenDyn<'ui>>>,
        S: ImStrExt,
        P: ImStrExt,
    {
        preview.with_imstr_dyn(|preview| self.begin_container_labelled(&Combo, label, Some(preview)))
    }
    #[inline(always)]
    fn begin_listbox<S>(&mut self, label: S) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, Listbox, BeginOutput = Option<UiTokenDyn<'ui>>>,
        S: ImStrExt,
    {
        self.begin_container_labelled(&Listbox, label, ())
    }
    #[inline(always)]
    fn begin_listbox_sized<S>(&mut self, label: S, size: ImSize2) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, Listbox, BeginOutput = Option<UiTokenDyn<'ui>>>,
        S: ImStrExt,
    {
        ImWidgetExt::begin_container_with(self, &Listbox, label, (), DynArgsSubContainer::new(None, Some(size)))
    }
    #[inline(always)]
    fn begin_menu<L>(&mut self, label: L) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, Menu, BeginOutput = Option<UiTokenDyn<'ui>>>,
        L: ImStrExt,
    {
        self.begin_container_labelled(&Menu, label, ())
    }
    #[inline(always)]
    fn begin_menu_with_enabled<L>(&mut self, label: L, enabled: bool) -> Option<UiTokenDyn<'ui>>
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, Menu, BeginOutput = Option<UiTokenDyn<'ui>>>,
        L: ImStrExt,
    {
        let args = match enabled {
            false => MenuItem::ARGS_DISABLED,
            true => Default::default(),
        };
        //self.draw_widget_labelled(&MenuItem, label, (state, None))
        self.begin_container_with(&Menu, label, (), args)
    }

    /// imgui-rs compat api
    ///
    /// TODO: manually draw preview to avoid the need to backtrack/clone?
    /// BeginComboPreview() is a thing but not in 1.80...
    #[inline(always)]
    fn combo<S, I>(&mut self, label: S, state: &mut usize, items: I) -> bool
    where
        for<'i> Self: ImWidgetLabelledContainer<'ui, 'i, Combo, BeginOutput = Option<UiTokenDyn<'ui>>>
            + ImWidgetLabelled<'ui, 'i, Selectable>,
        I: IntoIterator + Clone,
        I::Item: ImStrExt,
        S: ImStrExt,
    {
        let preview = items.clone().into_iter().nth(*state);
        let mut selected = false;
        let combo = match preview {
            Some(mut p) => p.with_imstr_dyn(|preview| self.begin_combo(label, preview)),
            None => self.begin_combo_opt(label, IM_STR_NONE),
        };
        let Some(_combo) = combo else { return selected };
        for (i, item) in items.into_iter().enumerate() {
            if self.selectable(item, i == *state) {
                *state = i;
                selected = true;
            }
        }
        selected
    }

    #[inline(always)]
    fn progress_bar<S>(&mut self, progress: f32, overlay: Option<S>, size: Option<ImSize2>)
    where
        for<'i> Self: ImWidgetBare<'ui, 'i, ProgressBar>,
        S: ImStrExt,
    {
        let args = DynArgsWidgetSized::new(None, size);
        match overlay {
            Some(mut overlay) => overlay.with_imstr_dyn(|overlay| {
                self.draw_a_widget_with(&ProgressBar, (progress, Some(overlay)), args)
            }),
            None => self.draw_a_widget_with(&ProgressBar, (progress, None), args),
        }
    }
    #[inline(always)]
    fn slider<T, L, F>(
        &mut self,
        label: L,
        state: &mut T,
        range: RangeInclusive<T::Primitive>,
        mut format: Option<F>,
    ) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, Slider<T>>,
        L: ImStrExt,
        F: ImStr,
        T: ?Sized + ImPrimitiveData + 'ui,
    {
        let args = DynArgsSlider::new(None, range);
        let format = format.as_mut().map(|f| f as &mut dyn ImStr);
        self.draw_widget_with(Slider::<T>::marker_ref(), label, (state, format), args)
    }
    /// being explicit can help type inference...
    #[inline(always)]
    fn sliders<T, L, F>(
        &mut self,
        label: L,
        state: &mut [T],
        range: RangeInclusive<T>,
        format: Option<F>,
    ) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, Slider<[T]>>,
        L: ImStrExt,
        F: ImStr,
        [T]: ImPrimitiveData<Primitive = T> + 'ui,
    {
        self.slider(label, state, range, format)
    }
    #[inline(always)]
    fn vslider<T, L, F>(
        &mut self,
        label: L,
        size: ImSize2,
        state: &mut T,
        range: RangeInclusive<T::Primitive>,
        mut format: Option<F>,
    ) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, VSlider<T>>,
        L: ImStrExt,
        F: ImStr,
        T: ?Sized + ImPrimitiveData + 'ui,
    {
        let args = DynArgsVSlider::new(size, None, range);
        let format = format.as_mut().map(|f| f as &mut dyn ImStr);
        self.draw_widget_with(VSlider::<T>::marker_ref(), label, (state, format), args)
    }

    #[inline(always)]
    fn selectable<S>(&mut self, label: S, state: bool) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, Selectable>,
        S: ImStrExt,
    {
        self.draw_widget_labelled(&Selectable, label, state)
    }
    /// `self.selectable(label, false)` for use as a button
    #[inline(always)]
    fn pressable<S>(&mut self, label: S) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, Selectable>,
        S: ImStrExt,
    {
        self.selectable(label, false)
    }
    #[inline(always)]
    fn menu_item<L>(&mut self, label: L, state: bool) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, MenuItem>,
        L: ImStrExt,
    {
        //self.draw_widget_labelled(&MenuItem, label, (state, None))
        self.menu_item_with(label, state, IM_STR_NONE, true)
    }
    #[inline(always)]
    fn menu_item_enabled<L>(&mut self, label: L, state: bool, enabled: bool) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, MenuItem>,
        L: ImStrExt,
    {
        //self.draw_widget_labelled(&MenuItem, label, (state, None))
        self.menu_item_with(label, state, IM_STR_NONE, enabled)
    }
    #[inline(always)]
    fn menu_item_with<L, S>(&mut self, label: L, state: bool, shortcut: Option<S>, enabled: bool) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, MenuItem>,
        L: ImStrExt,
        S: ImStrExt,
    {
        let args = match enabled {
            false => MenuItem::ARGS_DISABLED,
            true => Default::default(),
        };
        match shortcut {
            Some(mut shortcut) => shortcut.with_imstr_dyn(|shortcut| {
                self.draw_widget_with(&MenuItem, label, (state, Some(shortcut)), args)
            }),
            None => self.draw_widget_with(&MenuItem, label, (state, None), args),
        }
    }
    #[inline(always)]
    #[cfg(todo)]
    fn menu_item_mut_with<L, S>(
        &mut self,
        label: L,
        state: &'i mut bool,
        mut shortcut: Option<S>,
        enabled: bool,
    ) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, MenuItem<&'i mut bool>>,
        L: ImStrExt,
        S: ImStr,
    {
        self.draw_widget_with(&MenuItem, label, (state, shortcut), args)
    }

    #[inline(always)]
    fn image<T, S>(&mut self, texture: T, size: S)
    where
        for<'i> Self: ImWidgetBare<'ui, 'i, Image>,
        T: ImTexture,
        S: Into<ImSize2>,
    {
        let args = DynArgsSubContainer::new(None, Some(size.into()));
        self.draw_a_widget_with(&Image, &texture, args)
    }

    /// TODO: step+step_fast
    #[inline(always)]
    fn input_scalar<S, T, F>(&mut self, label: S, state: &mut T, mut format: Option<F>) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, InputScalar<T>>,
        T: ?Sized + ImPrimitiveData + 'ui,
        S: ImStrExt,
        F: ImStr,
    {
        let format = format.as_mut().map(|f| f as &mut dyn ImStr);
        let args = DynArgsInputScalar::new(None);
        self.draw_widget_with(InputScalar::<T>::marker_ref(), label, (state, format), args)
    }
    #[inline(always)]
    fn inputs_scalar<S, T, F>(&mut self, label: S, state: &mut [T], format: Option<F>) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, InputScalar<[T]>>,
        [T]: ImPrimitiveData<Primitive = T> + 'ui,
        S: ImStrExt,
        F: ImStr,
    {
        self.input_scalar(label, state, format)
    }
    #[inline(always)]
    fn input_int<S, B>(&mut self, label: S, mut buffer: B) -> bool
    where
        //for<'i> Self: ImWidgetLabelled<'ui, 'i, InputInt>,
        for<'i> Self: ImWidgetLabelled<'ui, 'i, InputScalar<i32>>,
        S: ImStrExt,
        B: BorrowMut<i32>,
    {
        self.input_scalar(label, buffer.borrow_mut(), IM_STR_NONE)
    }
    #[inline(always)]
    fn input_text<S, B, H>(&mut self, label: S, mut buffer: B, mut hint: Option<H>) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, InputText>,
        S: ImStrExt,
        B: BorrowMut<String>,
        H: ImStr,
    {
        let hint = hint.as_mut().map(|p| p as &mut dyn ImStr);
        self.draw_widget_labelled(&InputText, label, (buffer.borrow_mut(), hint))
    }
    #[inline(always)]
    fn input_text_with<S, B, H, F>(&mut self, label: S, mut buffer: B, mut hint: Option<H>, args: F) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, InputText>,
        S: ImStrExt,
        B: BorrowMut<String>,
        H: ImStr,
        for<'i> F: Into<<InputText as LabelledWidget<'i>>::Args>,
    {
        let hint = hint.as_mut().map(|p| p as &mut dyn ImStr);
        let args = args.into();
        self.draw_widget_with(&InputText, label, (buffer.borrow_mut(), hint), args)
    }
    #[inline(always)]
    fn input_password<S, B, H>(&mut self, label: S, mut buffer: B, mut hint: Option<H>) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, InputPassword>,
        S: ImStrExt,
        B: BorrowMut<String>,
        H: ImStr,
    {
        let hint = hint.as_mut().map(|p| p as &mut dyn ImStr);
        self.draw_widget_labelled(&InputPassword, label, (buffer.borrow_mut(), hint))
    }
    #[inline(always)]
    fn input_text_multiline<S, B>(&mut self, label: S, mut buffer: B) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, InputTextMultiline>,
        S: ImStrExt,
        B: BorrowMut<String>,
    {
        self.draw_widget_labelled(&InputTextMultiline, label, buffer.borrow_mut())
    }
    #[inline(always)]
    fn input_text_multiline_with<S, B, F>(&mut self, label: S, mut buffer: B, args: F) -> bool
    where
        for<'i> Self: ImWidgetLabelled<'ui, 'i, InputTextMultiline>,
        S: ImStrExt,
        B: BorrowMut<String>,
        for<'i> F: Into<<InputTextMultiline as LabelledWidget<'i>>::Args>,
    {
        let args = args.into();
        self.draw_widget_with(&InputTextMultiline, label, buffer.borrow_mut(), args)
    }
}
impl<'ui, 'i, U: ?Sized> ImWidgetExt<'ui> for U where U: ImDraw {}

pub trait ImDrawWidgetHost<'ui>:
    for<'i> ImWidgetBareContainer<'ui, 'i, Tooltip, BeginOutput = Option<UiTokenDyn<'ui>>>
    + for<'i> ImWidgetBareContainer<'ui, 'i, Group, BeginOutput = UiTokenDyn<'ui>>
    + for<'i> ImWidgetBare<'ui, 'i, ProgressBar>
    + for<'i> ImWidgetBare<'ui, 'i, Image>
    + for<'i> ImWidgetLabelledContainer<
        'ui,
        'i,
        Window,
        BeginOutput = (Option<BeginVisible>, UiTokenDyn<'ui>),
    > + for<'i> ImWidgetLabelledContainer<
        'ui,
        'i,
        ChildWindow,
        BeginOutput = (Option<BeginVisible>, UiTokenDyn<'ui>),
    > + for<'i> ImWidgetLabelledContainer<'ui, 'i, Popup, BeginOutput = Option<UiTokenDyn<'ui>>>
    + for<'i> ImWidgetLabelledContainer<'ui, 'i, PopupModal, BeginOutput = Option<UiTokenDyn<'ui>>>
    + for<'i> ImWidgetLabelledContainer<'ui, 'i, TreeNode, BeginOutput = Option<UiTokenDyn<'ui>>>
    + for<'i> ImWidgetLabelledContainer<'ui, 'i, Combo, BeginOutput = Option<UiTokenDyn<'ui>>>
    + for<'i> ImWidgetLabelledContainer<'ui, 'i, Listbox, BeginOutput = Option<UiTokenDyn<'ui>>>
    + for<'i> ImWidgetLabelledContainer<'ui, 'i, Menu, BeginOutput = Option<UiTokenDyn<'ui>>>
    + for<'i> ImWidgetLabelled<'ui, 'i, Selectable>
    + for<'i> ImWidgetLabelled<'ui, 'i, MenuItem>
    + for<'i> ImWidgetLabelled<'ui, 'i, InputText>
    + for<'i> ImWidgetLabelled<'ui, 'i, InputPassword>
    + for<'i> ImWidgetLabelled<'ui, 'i, InputTextMultiline>
    + for<'i> ImWidgetLabelled<'ui, 'i, InputScalar<i32>>
    + for<'i> ImWidgetLabelled<'ui, 'i, InputScalar<f32>>
    + for<'i> ImWidgetLabelled<'ui, 'i, InputScalar<[f32]>>
    + for<'i> ImWidgetLabelled<'ui, 'i, Slider<f32>>
    + for<'i> ImWidgetLabelled<'ui, 'i, Slider<u8>>
    + for<'i> ImWidgetLabelled<'ui, 'i, Slider<u64>>
    + for<'i> ImWidgetLabelled<'ui, 'i, Slider<i64>>
    + for<'i> ImWidgetLabelled<'ui, 'i, VSlider<f32>>
    + for<'i> ImWidgetLabelled<'ui, 'i, VSlider<i64>>
{
}
impl<'ui, U: ?Sized> ImDrawWidgetHost<'ui> for U where
    for<'i> U: ImWidgetBareContainer<'ui, 'i, Tooltip, BeginOutput = Option<UiTokenDyn<'ui>>>
        + ImWidgetBareContainer<'ui, 'i, Group, BeginOutput = UiTokenDyn<'ui>>
        + ImWidgetBare<'ui, 'i, ProgressBar>
        + ImWidgetBare<'ui, 'i, Image>
        + ImWidgetLabelledContainer<'ui, 'i, Window, BeginOutput = (Option<BeginVisible>, UiTokenDyn<'ui>)>
        + ImWidgetLabelledContainer<
            'ui,
            'i,
            ChildWindow,
            BeginOutput = (Option<BeginVisible>, UiTokenDyn<'ui>),
        > + ImWidgetLabelledContainer<'ui, 'i, Popup, BeginOutput = Option<UiTokenDyn<'ui>>>
        + ImWidgetLabelledContainer<'ui, 'i, PopupModal, BeginOutput = Option<UiTokenDyn<'ui>>>
        + ImWidgetLabelledContainer<'ui, 'i, TreeNode, BeginOutput = Option<UiTokenDyn<'ui>>>
        + ImWidgetLabelledContainer<'ui, 'i, Combo, BeginOutput = Option<UiTokenDyn<'ui>>>
        + ImWidgetLabelledContainer<'ui, 'i, Listbox, BeginOutput = Option<UiTokenDyn<'ui>>>
        + ImWidgetLabelledContainer<'ui, 'i, Menu, BeginOutput = Option<UiTokenDyn<'ui>>>
        + ImWidgetLabelled<'ui, 'i, Slider<f32>>
        + ImWidgetLabelled<'ui, 'i, Slider<u8>>
        + ImWidgetLabelled<'ui, 'i, Slider<u64>>
        + ImWidgetLabelled<'ui, 'i, Slider<i64>>
        + ImWidgetLabelled<'ui, 'i, VSlider<f32>>
        + ImWidgetLabelled<'ui, 'i, VSlider<i64>>
        + ImWidgetLabelled<'ui, 'i, InputScalar<i32>>
        + ImWidgetLabelled<'ui, 'i, InputScalar<f32>>
        + ImWidgetLabelled<'ui, 'i, InputScalar<[f32]>>
        + ImWidgetLabelled<'ui, 'i, InputText>
        + ImWidgetLabelled<'ui, 'i, InputPassword>
        + ImWidgetLabelled<'ui, 'i, InputTextMultiline>
        + ImWidgetLabelled<'ui, 'i, Selectable>
        + ImWidgetLabelled<'ui, 'i, MenuItem>
{
}
