use {
    crate::controller::pathing::{
        shared::HiddenGuids,
        state::{
            filter::{self, HiddenAlways, HiddenForCharacter, HiddenForMap, MarkerFilter},
            LoadedMaps,
        },
    },
    std::{collections::BTreeMap, num::NonZero, sync::Arc},
    taimi_hoard::time::Timestamp,
    taimi_meta::packs::{MapIndex, MarkerId},
    taimi_pack::attributes::keys::Guid,
};

#[derive(Debug, Clone, Default, Hash)]
pub struct MarkerState {
    pub hidden: BTreeMap<MarkerId, HiddenMarker>,
}
impl MarkerState {
    pub fn hide_contexts_for<'a>(&'a self, id: &MarkerId) -> impl Iterator<Item = &'a HideContext> + Clone {
        self.hidden
            .get(id)
            .into_iter()
            .flat_map(|hidden| hidden.contexts())
    }

    pub fn is_hidden(
        &self,
        id: &MarkerId,
        map: &filter::MapMetadata,
        character: &filter::CharacterMetadata,
    ) -> bool {
        self.hide_contexts_for(id).any(|ctx| {
            let filtered = match ctx {
                HideContext::Global => filter::FILTER_HIDDEN,
                HideContext::Local(filter) => filter.is_visible(map),
                HideContext::Character(filter) => filter.is_visible(character),
            };
            matches!(filtered, filter::FILTER_HIDDEN)
        })
    }

    pub fn next_expiry(&self) -> Option<Timestamp> {
        self.hidden
            .values()
            .filter_map(|hidden| match hidden.reset {
                AutoReset::Expiry { expiry } => Some(expiry),
                _ => None,
            })
            .min()
    }

    pub fn marker_mut(&mut self, id: impl Into<MarkerId>) -> &mut HiddenMarker {
        let id = id.into();
        self.hidden
            .entry(id)
            .or_insert(HiddenMarker::global(AutoReset::Never))
    }
    pub fn expire_at(
        &mut self,
        id: impl Into<MarkerId>,
        expiry: impl Into<Timestamp>,
    ) -> (&mut HiddenMarker, bool) {
        let ts = expiry.into();
        let entry = self.marker_mut(id);
        let changed = match &mut entry.reset {
            AutoReset::Expiry { expiry } if *expiry == ts => false,
            reset => {
                *reset = AutoReset::expire_at_timestamp(ts);
                true
            },
        };
        (entry, changed)
    }
    pub fn reset(&mut self, id: impl AsRef<MarkerId>) -> bool {
        self.hidden.remove(id.as_ref()).is_some()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty()
    }

    #[cfg(deleteme)]
    pub fn expire_at_timestamp(
        &mut self,
        id: impl Into<MarkerId>,
        expiry_timestamp: u64,
        now: &SystemTime,
        now_mono: &Instant,
    ) {
        let id = id.into();
        let entry = self
            .hidden
            .entry(id)
            .or_insert(HiddenMarker::global(AutoReset::Never));
        entry.reset = AutoReset::expiry_with_timestamp(expiry_timestamp, now, now_mono);
    }

    pub fn reset_expired(&mut self, now: &Timestamp) -> bool {
        let prev_len = self.hidden.len();
        self.hidden.retain(|_, hidden| match &hidden.reset {
            AutoReset::Expiry { expiry } if expiry <= now => false,
            _ => true,
        });
        prev_len != self.hidden.len()
    }
    pub fn reset_map_leave(&mut self) {
        self.hidden.retain(|_, hidden| match &hidden.reset {
            AutoReset::MapChange | AutoReset::Distance => false,
            _ => true,
        })
    }

    pub(crate) fn populate_from_settings(
        &mut self,
        hidden_guids: &HiddenGuids,
        all_ids: &mut dyn Iterator<Item = &MarkerId>,
        now: Option<Timestamp>,
    ) -> bool {
        let mut dirty = false;
        for id in all_ids {
            let duplicate_expired = match (self.hidden.get(id), &now) {
                (
                    Some(HiddenMarker {
                        reset: AutoReset::Expiry { expiry }, ..
                    }),
                    Some(now),
                ) => Some(expiry <= now),
                (Some(..), _) => Some(false),
                (None, _) => None,
            };
            if let Some(expired) = duplicate_expired {
                if expired {
                    self.hidden.remove(id);
                    dirty = true;
                }
                continue
            }
            let Some(&expiry_timestamp) = hidden_guids.get(Guid::from_uuid_ref(id)) else {
                continue
            };
            match now {
                Some(now) if expiry_timestamp <= now => continue,
                _ => (),
            }
            dirty |= self.expire_at(id.clone(), expiry_timestamp).1;
        }
        dirty
    }
}

/// TODO: could be a map of HideContext?
#[derive(Debug, Clone, Hash)]
pub struct HiddenMarker {
    pub contexts: Vec<HideContext>,
    pub reset: AutoReset,
}
impl HiddenMarker {
    pub const fn global(reset: AutoReset) -> Self {
        Self { contexts: Vec::new(), reset }
    }
    pub fn with_contexts<C: IntoIterator<Item = HideContext>>(reset: AutoReset, contexts: C) -> Self {
        let mut hidden = Self {
            reset,
            contexts: Vec::from_iter(contexts),
        };
        if hidden.is_global() {
            hidden.contexts = Vec::new();
        }
        hidden
    }

    pub fn contexts(&self) -> impl Iterator<Item = &HideContext> + Clone {
        let fallback = self.contexts.is_empty().then_some(&HideContext::GLOBAL);
        self.contexts.iter().chain(fallback)
    }

    pub fn is_global(&self) -> bool {
        match &self.contexts[..] {
            [] | [HideContext::Global] => true,
            _ => false,
        }
    }
}
impl From<AutoReset> for HiddenMarker {
    fn from(reset: AutoReset) -> Self {
        Self::global(reset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AutoReset {
    Never,
    /// Upon leaving the trigger range
    Distance,
    Expiry {
        expiry: Timestamp,
    },
    MapChange,
}
impl AutoReset {
    pub const fn expire_at_timestamp(expiry: Timestamp) -> Self {
        Self::Expiry { expiry }
    }
    #[cfg(deleteme)]
    pub fn expiry_with_timestamp(expiry_timestamp: u64, now: &SystemTime, now_mono: &Instant) -> Self {
        #[cfg(todo = "unnecessary")]
        let Some(expiry) = SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(expiry_timestamp)) else {
            return Self::Never
        };
        let offset = now.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
        let remaining = expiry_timestamp.saturating_sub(offset.as_secs());
        now_mono
            .checked_add(Duration::from_secs(remaining))
            .map(Self::expire_at_mono)
            .unwrap_or(Self::Never)
    }
}

/// Locality of [AutoReset]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HideContext {
    Global,
    /// Cleared on map change
    Local(Arc<HiddenForMap>),
    /// Typically recorded per-character and reset daily
    Character(Arc<HiddenForCharacter>),
}
impl HideContext {
    pub const GLOBAL: Self = Self::Global;

    pub fn for_map(map: MapIndex, shard: Option<NonZero<u32>>) -> Self {
        Self::Local(Arc::new(HiddenForMap { map, shard }))
    }
    pub fn for_character(name: Arc<[u8]>) -> Self {
        Self::Character(Arc::new(HiddenForCharacter { name }))
    }

    pub fn to_filter_state(&self) -> filter::FilterStateFilter {
        match self {
            Self::Global => HiddenAlways::singleton().clone() as filter::FilterStateFilter,
            Self::Local(filter) => filter.clone(),
            Self::Character(filter) => filter.clone(),
        }
    }
}
