use {
    super::{PathingController, PathingEvent},
    crate::controller::runtime::WallInstant,
    core::{iter, mem, slice},
    std::collections::BTreeMap,
    taimi_meta::{packs::id::MarkerId, ui::MapContext},
    taimi_sync::scheduled::ScheduledStream,
    tokio::time::Instant,
};

pub type ScheduledEvents = ScheduledStream<BTreeMap<Instant, PathingEvent>>;
pub type FilterExpiryMap = BTreeMap<MarkerId, Instant>;

impl PathingController {
    pub fn unexpire_at(
        scheduled: &mut ScheduledEvents,
        filter_expiry: &mut FilterExpiryMap,
        item: &MarkerId,
    ) -> bool {
        let Some(when) = filter_expiry.remove(item) else { return false };
        scheduled.cancel_if(&when, |events| {
            let ids = events
                .iter_mut()
                .filter_map(|e| match e {
                    PathingEvent::ResetMarkerIds(ids) => Some(ids.iter_mut()),
                    _ => None,
                })
                .flatten();
            for id in ids {
                *id = MarkerId::EMPTY;
            }
            events.is_empty()
        });
        true
    }
    #[cfg(todo)]
    pub fn unexpire(&mut self, item: impl AsRef<MarkerId>) -> bool {}
    pub fn unexpire_if_not(&mut self, item: &MarkerId, maybe_when: Instant) -> Option<bool> {
        let when = match self.filter_expiry.get(item) {
            Some(when) if *when == maybe_when => return None,
            when => when.copied(),
        };

        let res =
            when.map(|_when| Self::unexpire_at(&mut self.scheduled_events, &mut self.filter_expiry, item));
        Some(res == Some(true))
    }
    /// TODO: move to filter.rs? (and related?)
    pub fn expire_at(&mut self, item: MarkerId, expiry: WallInstant) {
        let when = expiry.instant;
        if let None = self.unexpire_if_not(&item, when) {
            return
        }
        let e = PathingEvent::ResetMarkerIds(vec![item]);
        self.scheduled_events.schedule_append(when, e);
    }
}

impl PathingEvent {
    pub const COLLECT_GARBAGE_PRUNE_ONLY: Self = Self::CollectGarbage { tick: 0, aggressive: false };
    pub const COLLECT_GARBAGE_TICK: Self = Self::CollectGarbage { tick: 1, aggressive: true };
    pub const COLLECT_GARBAGE_NOW: Self = Self::CollectGarbage { tick: 0, aggressive: true };

    #[inline]
    pub fn try_send(self) {
        let _ = PathingController::try_send(self);
    }

    pub const VISIBLE_TOGGLE_SPACE: Self = Self::VisibleToggle { context: None, set: None, ui: true };
    pub const fn visible_toggle(context: MapContext) -> Self {
        Self::VisibleToggle {
            context: Some(context),
            set: None,
            ui: true,
        }
    }
    pub const fn visible_toggle_manual(context: Option<MapContext>, set: Option<bool>) -> Self {
        Self::VisibleToggle { context, set, ui: false }
    }

    /// use push where possible thanks
    #[cfg(todo)]
    pub fn join(mut self, e: Self) -> Self {
        self.push(e);
        self
    }
    pub fn push(&mut self, e: Self) {
        match (self, e) {
            (_, Self::Nop) => (),
            (this @ Self::Nop, e) => *this = e,
            (Self::FanOut(events), Self::FanOut(mut e)) => events.append(&mut e),
            (Self::ResetMarkerIds(ids), Self::ResetMarkerIds(mut e)) => ids.append(&mut e),
            (Self::FanOut(events), e) => events.push(e),
            (this, that) => {
                let prev = mem::replace(this, Self::Nop);
                *this = Self::FanOut(match that {
                    Self::FanOut(mut trailing) => {
                        trailing.insert(0, prev);
                        trailing
                    },
                    that => vec![prev, that],
                });
            },
        }
    }
    pub fn flatten(self) -> Self {
        match self {
            Self::FanOut(e) => match e.len() {
                0 => Self::Nop,
                1 => unsafe { e.into_iter().next().unwrap_unchecked() },
                #[cfg(todo)]
                _ if e.iter().all(|e| matches!(e, Self::ResetMarkerIds(..))) => join_all_iguess,
                _ => Self::FanOut(e),
            },
            e => e,
        }
    }
    #[cfg(todo = "unused")]
    pub fn trim(&mut self) {
        if self.is_empty() {
            *self = Self::Nop;
            return
        }
        match self {
            Self::FanOut(events) => {
                while let Some(..) = events.pop_if(|e| {
                    let is_empty = e.is_empty();
                    if !is_empty {
                        e.trim();
                    }
                    is_empty
                }) {}
            },
            Self::ResetMarkerIds(ids) => while let Some(..) = ids.pop_if(|id| id.is_empty()) {},
            _ => (),
        }
    }
    /// TODO: ew vec
    pub(super) fn into_iter_shallow(self) -> impl Iterator<Item = Self> {
        match self {
            Self::FanOut(e) => Some(e),
            Self::Nop => None,
            e => Some(vec![e]),
        }
        .into_iter()
        .flatten()
    }
    pub(super) fn iter_shallow(&self) -> impl Iterator<Item = &Self> {
        match self {
            Self::FanOut(e) => &e[..],
            Self::Nop => &[],
            e => slice::from_ref(e),
        }
        .iter()
    }
    /// WARNING: recursive/heapy :<
    pub fn iter(&self) -> impl Iterator<Item = &Self> {
        let recurse = matches!(self, Self::FanOut(..));
        self.iter_shallow().flat_map(move |e| match recurse {
            true => Box::new(e.iter()) as Box<dyn Iterator<Item = &Self>>,
            false => Box::new(iter::once(e)) as Box<_>,
        })
    }
    pub(super) fn iter_mut_shallow(&mut self) -> impl Iterator<Item = &mut Self> {
        match self {
            Self::FanOut(e) => &mut e[..],
            Self::Nop => &mut [],
            e => slice::from_mut(e),
        }
        .iter_mut()
    }
    /// WARNING: recursive/heapy :<
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Self> {
        let recurse = matches!(self, Self::FanOut(..));
        self.iter_mut_shallow().flat_map(move |e| match recurse {
            true => Box::new(e.iter_mut()) as Box<dyn Iterator<Item = &mut Self>>,
            false => Box::new(iter::once(e)) as Box<_>,
        })
    }
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Nop => true,
            Self::FanOut(e) => e.iter().all(Self::is_empty),
            Self::ResetMarkerIds(ids) => ids.iter().all(|id| id.is_empty()),
            _ => false,
        }
    }
}
impl FromIterator<Self> for PathingEvent {
    fn from_iter<I: IntoIterator<Item = Self>>(iter: I) -> Self {
        let mut this = Self::Nop;
        this.extend(iter);
        this
    }
}
impl Extend<Self> for PathingEvent {
    fn extend<I: IntoIterator<Item = Self>>(&mut self, iter: I) {
        for e in iter {
            self.push(e);
        }
    }
}
impl IntoIterator for PathingEvent {
    type Item = Self;
    type IntoIter = Box<dyn Iterator<Item = Self>>;

    fn into_iter(self) -> Self::IntoIter {
        let recurse = matches!(self, Self::FanOut(..));
        let iter = self.into_iter_shallow().flat_map(move |e| match recurse {
            true => Box::new(e.into_iter()) as Box<dyn Iterator<Item = Self>>,
            false => Box::new(iter::once(e)) as Box<_>,
        });
        Box::new(iter) as Box<_>
    }
}
