use futures_util::ready;
use futures_core::stream::Stream;
use tokio::time::{sleep_until, Instant, Sleep};
use core::task::{Context, Poll};
use core::pin::Pin;
use core::future::Future;
use core::mem;

#[derive(Default, Debug)]
pub struct ScheduledStream<E: ?Sized> {
    pub pending: Option<Pin<Box<Sleep>>>,
    pub events: E,
}

impl<E> ScheduledStream<E> {
    pub fn empty() -> Self where
        E: Default,
    {
        Self::new(E::default())
    }

    pub fn new(events: E) -> Self {
        Self {
            events,
            pending: None,
        }
    }

    pub fn set_pending(&mut self, sleep: Sleep) {
        match &mut self.pending {
            pending @ None =>
                *pending = Some(Box::pin(sleep)),
            Some(pending) =>
                pending.set(sleep),
        }
    }
    pub fn next_scheduled(&self) -> Option<Instant> {
        self.pending.as_ref().map(|p| p.deadline())
    }
    fn reschedule(&mut self, when: Instant) {
        self.set_pending(sleep_until(when))
    }
    fn unschedule(&mut self) {
        #[cfg(todo)]
        if let Some(pending) = &mut self.pending {
            pending.set(WallInstant::big_sleep());
        }
        self.pending = None;
    }

    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        match &mut self.pending {
            None => Poll::Pending,
            Some(ref mut pending) =>
                pending.as_mut().poll(cx),
        }
    }
}

use std::collections::{btree_map, BTreeMap};
/// TODO: hoard collections dict etc
impl<V> ScheduledStream<BTreeMap<Instant, V>> {
    pub fn schedule_set(&mut self, when: Instant, what: V) -> Option<V> {
        let replaced = self.events.insert(when, what);
        let reschedule = replaced.is_none().then(||
            self.next_scheduled().map(|next| next > when)
                .unwrap_or(true)
            );
        if let Some(true) = reschedule {
            self.reschedule(when);
        }
        replaced
    }
    /// TODO: could return mut ref to entry, just check time beforehand?
    pub fn schedule_append<T>(&mut self, when: Instant, what: T) where
        T: IntoIterator,
        V: Extend<T::Item> + FromIterator<T::Item>,
    {
        let reschedule = match self.events.entry(when) {
            btree_map::Entry::Vacant(e) => {
                e.insert(what.into_iter().collect());
                self.next_scheduled().map(|next| next > when)
                    .unwrap_or(true)
            },
            btree_map::Entry::Occupied(mut e) => {
                e.get_mut().extend(what);
                false
            },
        };
        if reschedule {
            self.reschedule(when);
        }
    }
    pub fn cancel_if<F: FnOnce(&mut V) -> bool>(&mut self, when: &Instant, f: F) -> Option<V> {
        let mut entry = match self.events.entry(*when) {
            btree_map::Entry::Vacant(..) =>
                return None,
            btree_map::Entry::Occupied(e) => e,
        };
        let cancelled = f(entry.get_mut());
        let cancelled = cancelled.then_some(entry.remove());
        if cancelled.is_some() && self.next_scheduled().as_ref() == Some(when) {
            self.reschedule_next_key();
        }
        cancelled
    }
    #[cfg(todo)]
    pub fn cancel_matching(&mut self) -> impl Iterator<Item = V> {
        self.events.extract_if(todo)
    }
    pub fn cancel_matching<F: FnMut(&Instant, &mut V) -> bool>(&mut self, mut f: F) -> usize {
        #[cfg(todo)]
        use taimi_hoard::collections::TaimiDictOf;

        let waiting_for = self.next_scheduled();
        let waiting_for = waiting_for.as_ref();
        let mut reschedule = false;
        let prev_len = self.events.len();
        self.events.retain(|when, what| {
            if !f(when, what) {
                return true
            }

            if waiting_for == Some(when) {
                reschedule = true;
            }

            false
        });
        if reschedule {
            self.reschedule_next_key();
        }
        self.events.len() - prev_len
    }
    fn reschedule_next_key(&mut self) {
        let when = match self.events.first_key_value() {
            Some((&when, _)) => Some(when),
            _ => None,
        };
        if let Some(when) = when {
            self.reschedule(when);
        } else {
            self.unschedule();
        }
    }

    pub fn poll_scheduled_next(&mut self, cx: &mut Context) -> Poll<(Instant, V)> {
        //#[cfg(todo = "unnecessary")]
        let when = self.next_scheduled();
        ready!(self.poll_ready(cx));
        #[cfg(todo)]
        let when = self.next_scheduled();
        debug_assert!(when.is_some());
        let res = match &when {
            Some(when) => self.events.remove(when),
            None => None,
            #[cfg(todo = "unnecessary")]
            None => unsafe { unreachable_unchecked() },
        };
        self.reschedule_next_key();
        debug_assert!(res.is_some());
        match (when, res) {
            (Some(when), Some(what)) => Poll::Ready((when, what)),
            _ => {
                // shouldn't really happen...
                Poll::Pending
            },
        }
    }
}
/// fused, see also: [Self::infinite_mut]
impl<V> Stream for ScheduledStream<BTreeMap<Instant, V>> {
    type Item = (Instant, V);
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        if self.events.is_empty() {
            return Poll::Ready(None)
        }
        self.get_mut().poll_scheduled_next(cx).map(Some)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let amt = self.events.len();
        (amt, Some(amt))
    }
}
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct InfiniteScheduledStream<T: ?Sized>(pub ScheduledStream<T>);
impl<T: ?Sized> InfiniteScheduledStream<T> {
    #[inline]
    pub const fn from_ref(stream: &ScheduledStream<T>) -> &Self {
        unsafe {
            mem::transmute(stream)
        }
    }
    #[inline]
    pub fn from_mut(stream: &mut ScheduledStream<T>) -> &mut Self {
        unsafe {
            mem::transmute(stream)
        }
    }
    #[inline]
    pub fn from_pin(stream: Pin<&mut ScheduledStream<T>>) -> Pin<&mut Self> where
        T: Sized,
    {
        unsafe {
            stream.map_unchecked_mut(Self::from_mut)
        }
    }

    #[inline]
    pub fn stream(&self) -> &ScheduledStream<T> { &self.0 }
    #[inline]
    pub fn stream_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut ScheduledStream<T>> where
        T: Sized,
    {
        unsafe {
            self.map_unchecked_mut(|this| &mut this.0)
        }
    }
}
impl<V> Stream for InfiniteScheduledStream<BTreeMap<Instant, V>> {
    type Item = (Instant, V);
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.stream_mut().poll_scheduled_next(cx).map(Some)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let amt = self.stream().events.len();
        (amt, None)
    }
}
impl<T> ScheduledStream<T> {
    /// [InfiniteScheduledStream]
    #[inline]
    pub fn infinite_mut(&mut self) -> &mut InfiniteScheduledStream<T> {
        InfiniteScheduledStream::from_mut(self)
    }
}
