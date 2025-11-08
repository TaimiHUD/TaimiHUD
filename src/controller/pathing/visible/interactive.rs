use bitflags::bitflags;
use crate::{controller::pathing::{registry::{CategoryIndex, CategoryPath, PackMapPath, PoiIndex, PoiPath}, visible::LoadedPoi, MapPackInfo}, exports::runtime::Locator, space::pack::PackSpace};
use glamour::Point3;
use taimi_pack::{attributes::{keys::{self, ShowHideAction}, TacoBehavior}, MarkerAttributes, Pack};

#[derive(Debug, Clone, Default)]
pub struct InteractivePoi {
    index: PoiIndex,
    pub trigger: TriggerConfig,
    #[cfg(todo)]
    pub info_trigger: Option<TriggerConfig>,
    pub behaviour: Option<BehaviourConfig>,
    pub info: Option<InfoConfig>,
    pub copy: Option<CopyConfig>,
    pub show: Option<ShowHideConfig>,
    pub hide: Option<ShowHideConfig>,
    pub toggle: Option<ShowHideConfig>,
    pub reset: Option<ResetConfig>,
    pub bounce: Option<BounceConfig>,
    pub script: Option<ScriptConfig>,
}

impl InteractivePoi {
    pub const INVALID: Self = Self {
        index: PoiIndex::MAX,
        trigger: TriggerConfig {
            auto: false,
            radius: keys::TriggerRange(f32::NAN),
        },
        behaviour: None,
        info: None,
        copy: None,
        show: None,
        hide: None,
        toggle: None,
        reset: None,
        bounce: None,
        script: None,
    };

    pub fn from_pack(index: PoiIndex, path: PoiPath, pack: &Pack) -> Self {
        let Some(poi) = pack.pois.get(path.path as usize) else {
            return Self::INVALID
        };
        let attrs = &poi.attributes;

        let trigger = TriggerConfig {
            auto: attrs.auto_trigger.unwrap_or_default(),
            radius: attrs.info_range.map(keys::TriggerRange::from).unwrap_or_default().into(),
        };
        let behaviour = attrs.taco_behavior.as_ref()
            .and_then(|b| match b {
                TacoBehavior::AlwaysVisible => None,
                b => Some(b),
            }).map(|behaviour| BehaviourConfig {
                mode: behaviour.clone().into(),
                invert: attrs.invert_behavior.unwrap_or_default(),
                reset_delay: attrs.reset_length.map(Into::into).unwrap_or_default(),
            });
        let info = attrs.info.as_ref().map(|info| InfoConfig {
            message: info.clone().into(),
        });
        let copy = attrs.copy_value.as_ref().map(|value| CopyConfig {
            value: value.clone().into(),
            message: attrs.copy_message.clone().map(Into::into),
        });
        let show = attrs.show_category.as_ref().and_then(|cat|
            pack.categories.all_categories.get_index_of(cat)
        ).map(|cat| ShowHideConfig {
            category_index: cat as CategoryIndex,
            action: ShowHideAction::Show,
        });
        let hide = attrs.show_category.as_ref().and_then(|cat|
            pack.categories.all_categories.get_index_of(cat)
        ).map(|cat| ShowHideConfig {
            category_index: cat as CategoryIndex,
            action: ShowHideAction::Hide,
        });
        let toggle = attrs.toggle_category.as_ref().and_then(|cat|
            pack.categories.all_categories.get_index_of(cat)
        ).map(|cat| ShowHideConfig {
            category_index: cat as CategoryIndex,
            action: ShowHideAction::Toggle,
        });
        let reset = attrs.reset_guids.as_ref().map(|guids| ResetConfig {
            guid: guids.iter().copied().collect(),
        });
        let bounce = attrs.bounce_behavior.as_ref().map(|behaviour| BounceConfig {
            behaviour: behaviour.clone().into(),
            delay: attrs.bounce_delay.map(Into::into).unwrap_or_default(),
            duration: attrs.bounce_duration.map(Into::into).unwrap_or_default(),
            height: attrs.bounce_height.map(Into::into).unwrap_or_default(),
        });
        let script = match ScriptConfig::from_marker_attributes(attrs) {
            script if script.is_empty() =>
                None,
            script => Some(script),
        };
        Self {
            index,
            trigger,
            behaviour,
            info,
            copy,
            show,
            hide,
            toggle,
            reset,
            bounce,
            script,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self { index: PoiIndex::MAX, .. } =>
                true,
            Self {
                trigger: TriggerConfig { auto: false, .. },
                behaviour: None,
                info: None,
                copy: None,
                show: None,
                hide: None,
                toggle: None,
                reset: None,
                bounce: None,
                script: None,
                index: _,
            } => true,
            _ => false,
        }
    }

    pub fn path(&self, info: &MapPackInfo) -> Option<PoiPath> {
        info.pois().nth(self.index as usize)
    }
    /// Careful, not a [PoiPath]!
    pub fn loaded_index(&self) -> Locator<(), PoiIndex> {
        Locator::with_path(self.index)
    }
    pub fn loaded_poi<'a>(&self, pois: &'a [LoadedPoi]) -> Option<&'a LoadedPoi> {
        pois.get(self.index as usize)
    }
    pub fn loaded_poi_mut<'a>(&self, pois: &'a mut [LoadedPoi]) -> Option<&'a mut LoadedPoi> {
        pois.get_mut(self.index as usize)
    }

    /// Requires passive monitoring, usually related to [TriggerConfig::auto]
    ///
    /// An interaction that might show an unobtrusive notification like [InfoConfig]
    /// counts as well, even if it won't trigger automatically
    pub fn is_passive(&self) -> bool {
        self.trigger.auto || self.info.is_some() || self.copy.is_some()
    }

    pub fn is_nearby(&self, poi_pos: Point3<PackSpace>, player_pos: Point3<PackSpace>) -> Option<f32> {
        self.trigger.is_nearby(poi_pos, player_pos)
    }

    pub fn show_hide(&self) -> impl Iterator<Item = &ShowHideConfig> + Clone + '_ {
        self.show.as_ref().into_iter()
            .chain(self.hide.as_ref())
            .chain(self.toggle.as_ref())
    }
}

#[derive(Debug, Clone)]
pub struct InfoConfig {
    pub message: keys::Info,
}

#[derive(Debug, Clone)]
pub struct CopyConfig {
    pub value: keys::CopyValue,
    pub message: Option<keys::CopyMessage>,
}

#[derive(Debug, Clone)]
pub struct ShowHideConfig {
    pub category_index: CategoryIndex,
    pub action: ShowHideAction,
}
impl ShowHideConfig {
    pub fn category(&self) -> CategoryPath {
        CategoryPath::with_path(self.category_index)
    }
}

#[derive(Debug, Clone)]
pub struct ResetConfig {
    pub guid: keys::ResetGuid,
}

#[derive(Debug, Clone)]
/// TODO
pub struct BounceConfig {
    pub behaviour: keys::Bounce,
    pub delay: keys::BounceDelay,
    pub duration: keys::BounceDuration,
    pub height: keys::BounceHeight,
}

#[derive(Debug, Clone)]
pub struct BehaviourConfig {
    pub mode: keys::Behaviour,
    pub invert: bool,
    pub reset_delay: keys::ResetLength,
}

/// TODO
#[derive(Debug, Clone)]
pub struct ScriptConfig {
    pub tick: Option<keys::Script>,
    pub focus: Option<keys::Script>,
    pub trigger: Option<keys::Script>,
    pub filter: Option<keys::Script>,
    pub once: Option<keys::Script>,
}
impl ScriptConfig {
    pub fn from_marker_attributes(attrs: &MarkerAttributes) -> Self {
        Self {
            tick: attrs.script_tick.clone().map(Into::into),
            focus: attrs.script_focus.clone().map(Into::into),
            trigger: attrs.script_trigger.clone().map(Into::into),
            filter: attrs.script_filter.clone().map(Into::into),
            once: attrs.script_once.clone().map(Into::into),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                tick: None,
                focus: None,
                trigger: None,
                filter: None,
                once: None,
            } =>
                true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TriggerConfig {
    pub radius: keys::TriggerRange,
    pub auto: bool,
}
impl TriggerConfig {
    pub fn new(radius: keys::TriggerRange, auto: bool) -> Self {
        Self {
            radius,
            auto,
        }
    }

    pub fn radius_squared(&self) -> f32 {
        self.radius.0.powi(2)
    }

    pub fn is_nearby(&self, poi_pos: Point3<PackSpace>, player_pos: Point3<PackSpace>) -> Option<f32> {
        let distdist = poi_pos.distance_squared(player_pos);
        let thresh = self.radius_squared();
        (thresh >= distdist).then_some(distdist)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InteractionEvent {
    Nearby {
        path: PoiPath,
        loaded_path: Locator<PackMapPath, PoiIndex>,
        interactive_path: PoiPath<()>,
    },
    Gone {
        path: PoiPath,
        loaded_path: Locator<PackMapPath, PoiIndex>,
        interactive_path: PoiPath<()>,
    },
    Interact {
        action: InteractionEventAction,
        path: PoiPath,
        loaded_path: Locator<PackMapPath, PoiIndex>,
        interactive_path: PoiPath<()>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InteractionEventAction {
    Interact,
    AutoTrigger,
    #[cfg(todo)]
    Dismiss,
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
    pub struct TriggerKind: u16 {
        const AUTO = 0x0001;
        const COPY = 0x0002;
        const INFO = 0x0004;
        const RESET = 0x0008;
        const TOGGLE = 0x0010;
        const SHOW = 0x0020;
        const HIDE = 0x0040;
        const SCRIPT = 0x0080;
        const BOUNCE = 0x0100;
    }
}

/// TODO: probably just replacing this with a collection of [keys]
#[derive(Debug, Clone, Default)]
pub struct FilterConfig {
    pub achievement: Option<AchievementConfig>,
    pub schedule: Option<ScheduleConfig>,
    #[cfg(todo)]
    pub profession: Professions,
    #[cfg(todo)]
    pub specialization: Specialization,
    #[cfg(todo)]
    pub race: Race,
    #[cfg(todo)]
    pub mount: Mounts,
    #[cfg(todo)]
    pub festival: Festivals,
    #[cfg(todo)]
    pub raid: Raids,
    #[cfg(todo)]
    /// kinda is one?
    pub visibility: Option<VisibilityFlags>,
    /// this seems mostly pointless until scripting works
    #[cfg(todo)]
    pub map_type: keys::MapTypes,
}
impl FilterConfig {
    pub const EMPTY: Self = Self {
        achievement: None,
        schedule: None,
    };

    pub fn from_attributes(attrs: &MarkerAttributes) -> Self {
        let achievement = attrs.achievement_id.map(|id| AchievementConfig {
            id: id.into(),
            bit: attrs.achievement_bit.map(keys::AchievementBit::from).unwrap_or_default(),
        });
        let schedule = attrs.schedule.as_ref().map(|schedule| ScheduleConfig {
            schedule: schedule.clone().into(),
            // TODO: warn if not provided together?
            duration: attrs.schedule_duration.clone().map(Into::into).unwrap_or_default(),
        });
        Self {
            achievement,
            schedule,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self {
                achievement: None,
                schedule: None,
            } => true,
            _ => false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct AchievementConfig {
    pub id: keys::AchievementId,
    pub bit: keys::AchievementBit,
}
#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    pub schedule: keys::Schedule,
    pub duration: keys::ScheduleDuration,
}
