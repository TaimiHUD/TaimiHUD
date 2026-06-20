pub use taimi_api_client::model::achievements::AchievementId;
use {
    bitvec::array::BitArray,
    std::{
        collections::{BTreeMap, BTreeSet},
        hash::{Hash, Hasher},
    },
};

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(todo, derive(Deserialize, Serialize))]
pub struct AchievementState {
    #[cfg_attr(todo, serde(default, skip_serializing_if = "BTreeSet::is_empty"))]
    pub completed: BTreeSet<AchievementId>,
    #[cfg_attr(todo, serde(default, skip_serializing_if = "BTreeSet::is_empty"))]
    pub progress: BTreeMap<AchievementId, AchievementBits>,
}
impl AchievementState {
    pub fn is_empty(&self) -> bool {
        match self {
            Self { completed, .. } if !completed.is_empty() => false,
            Self { progress, .. } if !progress.is_empty() => false,
            Self { completed: _, progress: _ } => true,
        }
    }

    pub fn complete(&mut self, id: AchievementId) {
        self.progress.remove(&id);
        self.completed.insert(id);
    }

    pub fn is_complete(&self, id: AchievementId) -> bool {
        self.completed.contains(&id)
    }
    pub fn is_bit_complete(&self, id: AchievementId, bit: u8) -> Option<bool> {
        self.progress.get(&id).map(|p| p.bit_complete(bit))
    }
}
type AchievementBitsRaw = BitArray<[u64; 2]>;
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct AchievementBits {
    pub bits: AchievementBitsRaw,
}
impl AchievementBits {
    pub const fn new(bits: AchievementBitsRaw) -> Self {
        Self { bits }
    }
    pub fn iter(&self) -> impl Iterator<Item = u8> + '_ {
        self.bits.iter_ones().map(|i| i as u8)
    }
    pub fn bit_complete(&self, bit: u8) -> bool {
        self.bits.get(bit as usize).map(|b| *b).unwrap_or(false)
    }
    pub fn count(&self) -> u8 {
        self.bits.count_ones() as _
    }
}
impl FromIterator<u8> for AchievementBits {
    fn from_iter<I: IntoIterator<Item = u8>>(iter: I) -> Self {
        let mut bits = Self::default();
        bits.extend(iter);
        bits
    }
}
impl Extend<u8> for AchievementBits {
    fn extend<I: IntoIterator<Item = u8>>(&mut self, bits: I) {
        for i in bits {
            let i = i as usize;
            if let Some(mut b) = self.bits.get_mut(i) {
                *b = true;
            } else {
                log::error!("achievement bit {i} out of range")
            }
        }
    }
}
/// ignores positions and just hashes [Self::count]
impl Hash for AchievementBits {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.count().hash(state)
    }
}
#[cfg(todo = "unused")]
impl<'de> Deserialize<'de> for AchievementBits {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Deserialize::deserialize(d).map(|data| Self::new(BitArray::new(data)))
        //Vec::<u8>::deserialize(d).map(|bits| bits.into_iter().collect())
    }
}
#[cfg(todo = "unused")]
impl Serialize for AchievementBits {
    fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.bits.data.serialize(s)
        //self.iter().collect::<Vec<u8>>().serialize(s)
    }
}

pub(super) mod serde_imp {
    pub mod achievement_state {
        use {
            super::super::AchievementState,
            serde::{de, Deserialize},
            taimi_api_client::model::achievements::AchievementId,
        };
        #[derive(Deserialize)]
        #[serde(transparent)]
        pub(crate) struct AchievementApi(Vec<AchievementApiEntry>);
        #[derive(Deserialize)]
        struct AchievementApiEntry {
            id: AchievementId,
            done: bool,
            #[serde(default)]
            bits: Vec<u8>,
        }
        impl From<AchievementApi> for AchievementState {
            fn from(achievements: AchievementApi) -> Self {
                let mut out = Self::default();
                for achievement in achievements.0 {
                    if achievement.done {
                        out.complete(achievement.id);
                    } else if !achievement.bits.is_empty() {
                        out.progress
                            .insert(achievement.id, achievement.bits.into_iter().collect());
                    }
                }
                out
            }
        }
        pub fn deserialize<'de, D: de::Deserializer<'de>>(d: D) -> Result<AchievementState, D::Error> {
            AchievementApi::deserialize(d).map(From::from)
        }
    }
}
