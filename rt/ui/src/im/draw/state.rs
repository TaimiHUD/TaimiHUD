use core::{
    any::{Any, TypeId},
    task::{Context, Poll},
};

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct InteractSignal: u8 {
        /// underlying state changed, button clicked, etc
        const TRIGGER = 0x01;
        /// IsItemDeactivatedAfterEdit
        const COMMIT = 0x02;
        /// pressing down on a button
        const ACTIVE = 0x04;
        /// containers
        const OPEN = 0x08;
        const VISIBLE = 0x10;
        /// some other external signal
        const EXTENDED = 0x20;
        /// like [Self::ACTIVE] but less... active?
        const FOCUS = 0x40;
        const HOVER = 0x80;
    }
}
impl InteractSignal {
    pub const EMPTY: Self = Self::empty();

    #[inline]
    pub fn take_next(&mut self) -> Self {
        let pending = self.bits().trailing_zeros() as u32;
        Self::from_bits_retain(1u8.unbounded_shl(pending))
    }
    #[inline]
    pub fn poll_next(&mut self) -> Poll<Self> {
        match self.take_next() {
            Self::EMPTY => Poll::Pending,
            signal => Poll::Ready(signal),
        }
    }
}

pub trait DrawContext<'ui>: TypeContainer<TypeId, Any = dyn Any> + DrawContextSignal<'ui> {}
pub trait DrawContextSignal<'ui>: TypeContainerOf<Context<'ui>> {
    fn signal_interest(&self) -> InteractSignal;
    fn raise_signal_masked(&mut self, signal: InteractSignal);
    fn mask_and_raise_signal(&mut self, signal: InteractSignal) -> InteractSignal {
        let masked = signal & self.signal_interest();
        if !masked.is_empty() {
            self.raise_signal_masked(signal);
        }
        masked
    }
    fn mask_and_signal_slot(&mut self, slot: &mut InteractSignal, signal: InteractSignal) {
        *slot |= self.mask_and_raise_signal(signal);
    }
    fn is_pending(&self) -> bool;
    fn handle_pending(&mut self, mask: InteractSignal) -> InteractSignal;
}
pub trait TypeContainerOf<T: ?Sized> {
    fn concrete_type_in(&self) -> &T;
    fn concrete_type_in_mut(&mut self) -> &mut T;
}

pub unsafe trait TypeContainer<Id = TypeId> {
    type Any: ?Sized;

    fn any_type_in(&self, id: Id) -> Option<&Self::Any>;
    fn any_type_in_mut(&mut self, id: Id) -> Option<&mut Self::Any>;
}
unsafe impl TypeContainer<TypeId> for dyn Any {
    type Any = dyn Any;
    #[inline]
    fn any_type_in(&self, id: TypeId) -> Option<&dyn Any> {
        if Any::type_id(&*self) == id {
            Some(self)
        } else {
            None
        }
    }
    #[inline]
    fn any_type_in_mut(&mut self, id: TypeId) -> Option<&mut dyn Any> {
        if Any::type_id(&*self) == id {
            Some(self)
        } else {
            None
        }
    }
}
#[cfg(todo)]
pub trait ConcreteTypeContainer<Id>: TypeContainer<Id> + TypeLabel<Id> {}
#[cfg(todo)]
unsafe impl<Id, A> TypeContainer<Id> for dyn ConcreteTypeContainer<Id, Any = A>
where
    Id: PartialEq,
{
    type Any = A;
    #[inline]
    fn any_type_in(&self, id: Id) -> Option<&Self::Any> {
        if TypeLabel::type_id_ref(&*self) == id {
            Some(self)
        } else {
            None
        }
    }
    fn any_type_in_mut(&mut self, id: Id) -> Option<&mut Self::Any> {
        if TypeLabel::type_id_ref(&*self) == id {
            Some(self)
        } else {
            None
        }
    }
}
pub unsafe trait TypeLabel<Id = TypeId> {
    fn type_id_ref(&self) -> Id;
}
pub unsafe trait TypeLabelStatic<Id = TypeId> {
    fn type_id() -> Id;
}
unsafe impl<T: 'static + ?Sized> TypeLabelStatic<TypeId> for T {
    #[inline]
    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }
}
unsafe impl<T: 'static + ?Sized> TypeLabel<TypeId> for T {
    #[inline]
    fn type_id_ref(&self) -> TypeId {
        match () {
            #[cfg(todo = "unnecessary")]
            _ => <Self as TypeLabelStatic<TypeId>>::type_id(),
            _ => Any::type_id(self),
        }
    }
}
impl<Id, A: ?Sized> dyn TypeContainer<Id, Any = A> {
    #[inline]
    pub fn type_in_ref<T, C>(container: &C) -> Option<&T>
    where
        C: ?Sized + TypeContainer<Id, Any = A>,
        T: TypeLabelStatic<Id>,
    {
        match container.any_type_in(T::type_id()) {
            Some(any) => Some(unsafe { &*(any as *const C::Any as *const T) }),
            None => None,
        }
    }
    #[inline]
    pub fn type_in_mut<T, C>(container: &mut C) -> Option<&mut T>
    where
        C: ?Sized + TypeContainer<Id, Any = A>,
        T: TypeLabelStatic<Id>,
    {
        match container.any_type_in_mut(T::type_id()) {
            Some(any) => Some(unsafe { &mut *(any as *mut C::Any as *mut T) }),
            None => None,
        }
    }
}
pub trait TypeContainerExt<Id = TypeId>: TypeContainer<Id> {
    #[inline(always)]
    fn type_in_ref<T>(&self) -> Option<&T>
    where
        T: TypeLabelStatic<Id>,
    {
        <dyn TypeContainer<Id, Any = Self::Any>>::type_in_ref::<T, _>(self)
    }
    #[inline(always)]
    fn type_in_mut<T>(&mut self) -> Option<&mut T>
    where
        T: TypeLabelStatic<Id>,
    {
        <dyn TypeContainer<Id, Any = Self::Any>>::type_in_mut::<T, _>(self)
    }
}
