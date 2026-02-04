use {
    crate::{
        controller::pathing::registry::PoiMapPath,
        settings::pathing::TriggerKind,
    },
    taimi_pack::attributes::keys,
    taimi_meta::packs::PoiPath,
};

#[cfg(todo)]
#[derive(Debug, Clone)]
pub struct ShowHideConfig {
    pub category_index: CategoryIndex,
    pub action: ShowHideAction,
}
#[cfg(todo)]
impl ShowHideConfig {
    pub fn category(&self) -> CategoryPath {
        CategoryPath::with_path(self.category_index)
    }
}

#[derive(Debug, Clone)]
#[cfg(todo)]
pub struct BounceConfig {
    pub behaviour: keys::Bounce,
    pub delay: keys::BounceDelay,
    pub duration: keys::BounceDuration,
    pub height: keys::BounceHeight,
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct BehaviourConfig {
    pub mode: keys::Behaviour,
    pub reset_delay: keys::ResetLength,
}
impl BehaviourConfig {
    pub fn new<M: Into<keys::Behaviour>>(mode: M) -> Self {
        Self {
            mode: mode.into(),
            reset_delay: Default::default(),
        }
    }
    pub fn is_empty(&self) -> bool {
        match self {
            Self { mode, .. } if !mode.is_empty() =>
                false,
            Self {
                mode: _,
                reset_delay: _,
            } =>
                true,
        }
    }
}

/// TODO
#[derive(Debug, Clone)]
#[cfg(todo)]
pub struct ScriptConfig {
    pub tick: Option<keys::Script>,
    pub focus: Option<keys::Script>,
    pub trigger: Option<keys::Script>,
    pub filter: Option<keys::Script>,
    pub once: Option<keys::Script>,
}
#[cfg(todo)]
impl ScriptConfig {
    pub fn from_script_attributes(attrs: &ScriptAttributes) -> Self {
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

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum InteractionEvent {
    Nearby {
        path: PoiPath,
        loaded_path: PoiMapPath,
    },
    Gone {
        path: PoiPath,
        loaded_path: PoiMapPath,
    },
    Interact {
        action: InteractionEventAction,
        path: PoiPath,
        loaded_path: PoiMapPath,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum InteractionEventAction {
    Interact,
    AutoTrigger,
    Trigger,
    Dismiss(BehaviourConfig),
    Manual(TriggerKind),
}

impl InteractionEventAction {
    /// Natural in-game events require extra processing for it to act reasonable
    pub fn is_natural(&self) -> bool {
        match self {
            Self::Interact | Self::AutoTrigger =>
                true,
            // anything triggered intentionally (via UI usually) is fair game
            Self::Trigger | Self::Manual(..) | Self::Dismiss(..) => false,
        }
    }
}
