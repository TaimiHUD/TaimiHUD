#[cfg(feature = "nexus-rtapi")]
pub use nexus::rtapi::GameState as NexusGameState;
use {crate::map::MapID, core::num::NonZero};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameplayState {
    /// Loading screens, character selection, cutscenes, etc.
    Intermission {
        prev_map_id: Option<NonZero<MapID>>,
        next_map_id: Option<NonZero<MapID>>,
        /// do we care specifically?
        #[cfg(todo)]
        menu: Page,
        /// Not loaded into any map or character yet
        initial: bool,
    },
    Gameplay {
        map_id: Option<NonZero<MapID>>,
    },
}

impl GameplayState {
    pub const DEFAULT: Self = Self::INITIAL;
    pub const INITIAL: Self = Self::Intermission {
        initial: true,
        prev_map_id: None,
        next_map_id: None,
    };
    pub const INGAME: Self = Self::Gameplay { map_id: None };
    pub const LOADING: Self = Self::Intermission {
        initial: false,
        prev_map_id: None,
        next_map_id: None,
    };
    pub const CHARSEL: Self = Self::LOADING;

    pub const fn is_initial(&self) -> bool {
        matches!(self, GameplayState::Intermission { initial: true, .. })
    }

    pub const fn new_ingame(map_id: MapID) -> Self {
        Self::Gameplay {
            map_id: NonZero::new(map_id),
        }
    }

    pub const fn new_loading(map_id: MapID, prev_map_id: MapID) -> Self {
        let prev_map_id = NonZero::new(prev_map_id);
        Self::Intermission {
            initial: prev_map_id.is_none(),
            prev_map_id,
            next_map_id: NonZero::new(map_id),
        }
    }

    pub fn commit_ingame(&mut self, map_id: MapID) -> Option<GameplayTransition> {
        let next_map_id = NonZero::new(map_id);
        match self {
            Self::Gameplay { map_id } if *map_id != next_map_id => {
                let change = GameplayTransition::Map {
                    prev_map_id: *map_id,
                    next_map_id,
                };
                if !change.is_map_outdated() {
                    *map_id = next_map_id;
                }
                Some(change)
            },
            Self::Gameplay { .. } => None,
            &mut GameplayState::Intermission {
                next_map_id: prev_next,
                prev_map_id,
                initial,
            } => {
                let latest_map_id = next_map_id.or(prev_next);
                let change = GameplayTransition::Loaded {
                    next_map_id: latest_map_id,
                    prev_map_id: prev_next.or(prev_map_id),
                    initial,
                };
                *self = Self::Gameplay {
                    map_id: next_map_id,
                };
                Some(change)
            },
        }
    }

    pub fn commit_loading(
        &mut self,
        next_map_id: Option<NonZero<MapID>>,
    ) -> Option<GameplayTransition> {
        match self {
            &mut Self::Gameplay { map_id, .. } => {
                let change = GameplayTransition::Intermission {
                    prev_map_id: map_id,
                };
                *self = Self::Intermission {
                    prev_map_id: map_id,
                    next_map_id,
                    initial: false,
                };
                Some(change)
            },
            Self::Intermission {
                next_map_id: next @ None,
                prev_map_id,
                ..
            } if next_map_id.is_some() && *next != next_map_id => {
                if *next == *prev_map_id {
                    // TODO: careful here...
                    return None
                }
                let change = GameplayTransition::Map {
                    prev_map_id: *prev_map_id,
                    next_map_id,
                };
                *next = next_map_id;
                Some(change)
            },
            Self::Intermission { .. } => None,
        }
    }

    /// Like [self.commit_loading()] but we won't assume a map change right away
    pub fn commit_intermission(&mut self) -> Option<GameplayTransition> {
        match self {
            &mut Self::Gameplay { map_id, .. } => {
                let change = GameplayTransition::Intermission {
                    prev_map_id: map_id,
                };
                *self = Self::Intermission {
                    prev_map_id: map_id,
                    next_map_id: map_id,
                    initial: false,
                };
                Some(change)
            },
            Self::Intermission { .. } => None,
        }
    }

    pub const fn gameplay_map(&self) -> Option<NonZero<MapID>> {
        match self {
            &Self::Gameplay { map_id } => map_id,
            Self::Intermission { .. } => None,
        }
    }

    pub const fn latest_map(&self) -> Option<NonZero<MapID>> {
        match self {
            &Self::Gameplay { map_id } => map_id,
            &Self::Intermission {
                next_map_id: Some(map_id),
                ..
            } => Some(map_id),
            &Self::Intermission {
                next_map_id: None,
                prev_map_id,
                ..
            } => prev_map_id,
        }
    }

    pub const fn latest_transition(&self) -> GameplayTransition {
        match self {
            &GameplayState::Intermission {
                prev_map_id: None,
                next_map_id,
                ..
            } => GameplayTransition::Intermission {
                prev_map_id: next_map_id,
            },
            &GameplayState::Intermission {
                prev_map_id,
                next_map_id,
                ..
            } =>
            //GameplayTransition::Intermission { prev_map_id: next_map_id.or(prev_map_id) },
            {
                GameplayTransition::Map {
                    prev_map_id,
                    next_map_id,
                }
            },
            &GameplayState::Gameplay { map_id } => GameplayTransition::Map {
                prev_map_id: None,
                next_map_id: map_id,
            },
        }
    }
}

impl Default for GameplayState {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(feature = "nexus-rtapi")]
impl From<NexusGameState> for GameplayState {
    fn from(state: NexusGameState) -> Self {
        match state {
            NexusGameState::CharacterSelection | NexusGameState::CharacterCreation => Self::CHARSEL,
            NexusGameState::Cinematic | NexusGameState::LoadingScreen => Self::LOADING,
            NexusGameState::Gameplay => Self::INGAME,
        }
    }
}
#[cfg(feature = "nexus-rtapi")]
impl From<GameplayState> for NexusGameState {
    fn from(state: GameplayState) -> Self {
        match state {
            GameplayState::Intermission { initial: true, .. } => NexusGameState::CharacterSelection,
            GameplayState::Intermission { initial: false, .. } => NexusGameState::LoadingScreen,
            GameplayState::Gameplay { .. } => NexusGameState::Gameplay,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameplayTransition {
    Map {
        prev_map_id: Option<NonZero<MapID>>,
        next_map_id: Option<NonZero<MapID>>,
    },
    Loaded {
        prev_map_id: Option<NonZero<MapID>>,
        next_map_id: Option<NonZero<MapID>>,
        initial: bool,
    },
    Intermission {
        prev_map_id: Option<NonZero<MapID>>,
    },
}

impl GameplayTransition {
    pub fn is_map_outdated(&self) -> bool {
        match self {
            Self::Map {
                prev_map_id: Some(..),
                next_map_id: None,
            } => true,
            _ => false,
        }
    }

    pub fn is_load_outdated_to(&self, map_id: MapID) -> bool {
        let map_id = NonZero::new(map_id);
        match self {
            &Self::Loaded {
                next_map_id: next_map_id @ Some(..),
                ..
            } if next_map_id != map_id => true,
            _ => false,
        }
    }
}
