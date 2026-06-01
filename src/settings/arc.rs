use {
    crate::{exports::runtime::bindings::TaimiControls, fl, settings::Settings, with_i18n},
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, fmt, mem},
    windows::Win32::UI::Input::KeyboardAndMouse::{self as vk, VIRTUAL_KEY},
};

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct ArcSettings {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub bind_vks: HashMap<String, u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gamebind_invoke: Option<InvokeMethod>,
}

impl ArcSettings {
    pub const VK_WINDOW_TOGGLE_PRIMARY: ArcVk = ArcVk::new("primary-window-toggle", vk::VK_M);
    pub const VK_WINDOW_TOGGLE_TIMERS: ArcVk = ArcVk::new("timer-window-toggle", vk::VK_K);
    pub const VK_WINDOW_TOGGLE_MARKERS: ArcVk = ArcVk::new("marker-window-toggle", vk::VK_L);
    pub const VK_WINDOW_TOGGLE_PATHING: ArcVk = ArcVk::new("pathing-window-toggle", vk::VK_N);
    pub const VK_RENDER_TOGGLE_PATHING: ArcVk = ArcVk::new("pathing-render-toggle", vk::VK_OEM_COMMA);
    pub const VK_RENDER_TOGGLE_PATHING_MINIMAP: ArcVk =
        ArcVk::new("pathing-render-minimap-toggle", vk::VK_F2);
    pub const VK_RENDER_TOGGLE_PATHING_MAP: ArcVk = ArcVk::new("pathing-render-map-toggle", vk::VK_F1);
    pub const VK_TIMER_RESET: ArcVk = ArcVk::empty("timer-key-reset");
    pub const VK_TIMER_TRIGGERS: [ArcVk; 5] = [
        ArcVk::empty("timer-key-trigger-0"),
        ArcVk::empty("timer-key-trigger-1"),
        ArcVk::empty("timer-key-trigger-2"),
        ArcVk::empty("timer-key-trigger-3"),
        ArcVk::empty("timer-key-trigger-4"),
    ];
    pub const VK_CONTEXT_PRIMARY: ArcVk = ArcVk::new("context-menu-primary", vk::VK_F3);

    pub const VK_WINDOWS: &'static [&'static ArcVk] = &[
        &Self::VK_WINDOW_TOGGLE_PRIMARY,
        #[cfg(feature = "timers")]
        &Self::VK_WINDOW_TOGGLE_TIMERS,
        #[cfg(feature = "markers")]
        &Self::VK_WINDOW_TOGGLE_MARKERS,
        #[cfg(feature = "space")]
        &Self::VK_WINDOW_TOGGLE_PATHING,
    ];
    pub const VK_CONTEXT_MENUS: &'static [&'static ArcVk] = &[&Self::VK_CONTEXT_PRIMARY];

    pub fn get_vk(&self, binding: &ArcVk) -> Option<VIRTUAL_KEY> {
        self.bind_vks
            .get(binding.id)
            .copied()
            .map(VIRTUAL_KEY)
            .or(binding.vkeycode_default())
    }

    pub fn binding_matches(&self, binding: &ArcVk, vkeycode: VIRTUAL_KEY) -> bool {
        match self.get_vk(binding) {
            Some(setting) => vkeycode == setting,
            None => false,
        }
    }

    #[cfg(todo = "unused")]
    pub fn update_preference(&self) -> &ArcUpdatePreference {
        self.update_preference
            .as_ref()
            .unwrap_or(&ArcUpdatePreference::ASK)
    }

    #[cfg(todo = "unused")]
    pub fn set_update_preference(&mut self, preference: ArcUpdatePreference) {
        match &preference {
            ArcUpdatePreference::Never => {
                self.update_remote_version = None;
            },
            ArcUpdatePreference::Ask {
                authorized: Some(Ok(xthorized) | Err(xthorized)),
            } if Some(xthorized) != self.update_remote_version.as_ref() => {
                self.update_remote_version = None;
            },
            _ => (),
        }
        self.update_preference = Some(preference);
    }

    #[cfg(todo = "unused")]
    pub fn update_available(&self) -> Option<NeedsUpdate> {
        match &self.update_remote_version {
            None => None,
            Some(v) if v.is_empty() => Some(NeedsUpdate::Unknown),
            Some(v) => Some(NeedsUpdate::Known(v != rt::CRATE_VERSION, v.clone())),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ArcVk {
    pub id: &'static str,
    pub default_vk: u16,
}

impl ArcVk {
    pub const VK_EMPTY: VIRTUAL_KEY = VIRTUAL_KEY(0);

    pub const fn empty(id: &'static str) -> Self {
        Self::new(id, Self::VK_EMPTY)
    }

    pub const fn new(id: &'static str, default_vk: VIRTUAL_KEY) -> Self {
        Self { id, default_vk: default_vk.0 }
    }

    pub fn vkeycode_default(&self) -> Option<VIRTUAL_KEY> {
        match self.default_vk {
            0 => None,
            vk => Some(VIRTUAL_KEY(vk)),
        }
    }

    pub const TIMER_KEY_TRIGGER_PREFIX: &'static str = "timer-key-trigger-";

    pub fn control(&self) -> Option<TaimiControls> {
        Some(match *self {
            ArcSettings::VK_WINDOW_TOGGLE_PRIMARY => TaimiControls::WINDOW_PRIMARY,
            #[cfg(feature = "timers")]
            ArcSettings::VK_WINDOW_TOGGLE_TIMERS => TaimiControls::WINDOW_TIMERS,
            #[cfg(feature = "markers")]
            ArcSettings::VK_WINDOW_TOGGLE_MARKERS => TaimiControls::WINDOW_MARKERS,
            #[cfg(feature = "space")]
            ArcSettings::VK_WINDOW_TOGGLE_PATHING => TaimiControls::WINDOW_PATHING,
            ArcSettings::VK_RENDER_TOGGLE_PATHING => TaimiControls::PATHING_SPACE,
            ArcSettings::VK_RENDER_TOGGLE_PATHING_MINIMAP => TaimiControls::PATHING_MINIMAP,
            ArcSettings::VK_RENDER_TOGGLE_PATHING_MAP => TaimiControls::PATHING_MAP,
            ArcSettings::VK_TIMER_RESET => TaimiControls::TIMER_RESET,
            ArcSettings::VK_CONTEXT_PRIMARY => TaimiControls::MENU_PRIMARY,
            _ => {
                if let Some(trigger_index) = ArcSettings::VK_TIMER_TRIGGERS.iter().position(|t| t == self) {
                    return Some(TaimiControls::from_index(
                        TaimiControls::TIMER_TRIGGER_0.index() + trigger_index as u8,
                    ))
                }
                return None
            },
        })
    }

    pub fn window_name(&self) -> Option<&'static str> {
        Some(match *self {
            ArcSettings::VK_WINDOW_TOGGLE_PRIMARY => crate::WINDOW_PRIMARY,
            ArcSettings::VK_WINDOW_TOGGLE_TIMERS => crate::WINDOW_TIMERS,
            #[cfg(feature = "markers")]
            ArcSettings::VK_WINDOW_TOGGLE_MARKERS => crate::WINDOW_MARKERS,
            #[cfg(feature = "space")]
            ArcSettings::VK_WINDOW_TOGGLE_PATHING => crate::WINDOW_PATHING,
            _ => return None,
        })
    }

    pub fn get_setting_vkeycode(&self) -> Option<VIRTUAL_KEY> {
        let settings = Settings::try_read()?;
        settings.arc().get_vk(self)
    }

    pub fn set_vkeycode(&self, new: VIRTUAL_KEY) -> anyhow::Result<()> {
        Settings::write_with_blocking(|settings| {
            let bind_vks = &mut settings.arc_mut().bind_vks;
            match new.0 {
                0 => bind_vks.remove(self.id),
                v => bind_vks.insert(self.id.into(), v),
            }
        })
        .map(drop)
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum InvokeMethod {
    /// SendInput
    Input,
    /// PostMessage
    #[default]
    Message,
    /// Hybrid hack requires moving the mouse in a way that Nexus can detect
    /// prior to invoking
    Nexus,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum ArcUpdatePreference {
    Always,
    Ask { authorized: Option<Result<String, String>> },
    Never,
    Once { authorized: String },
}

impl ArcUpdatePreference {
    pub const ASK: Self = Self::Ask { authorized: None };

    pub const OPTIONS: [Self; 3] = [Self::Never, Self::ASK, Self::Always];

    pub fn ask_allow<V: Into<String>>(version: V) -> Self {
        Self::Ask { authorized: Some(Ok(version.into())) }
    }

    #[cfg(todo = "unused")]
    pub fn ask_deny<V: Into<String>>(version: V) -> Self {
        Self::Ask { authorized: Some(Err(version.into())) }
    }

    pub fn only_once<V: Into<String>>(version: V) -> Self {
        Self::Once { authorized: version.into() }
    }

    pub fn as_option(&self) -> Self {
        match self {
            Self::Always => Self::Always,
            Self::Ask { .. } => Self::ASK,
            Self::Never | Self::Once { .. } => Self::Never,
        }
    }

    pub fn label_ident(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Ask { .. } => "ask",
            _ => "never",
        }
    }
    /// TODO: remove and stop using display for UI without adapters
    pub fn label_english(&self) -> &'static str {
        match self {
            Self::Always => "Always",
            Self::Ask { .. } => "Ask",
            _ => "Never",
        }
    }

    pub fn authorizes_version(&self, version: &str) -> Option<bool> {
        match self {
            Self::Once { authorized } | Self::Ask { authorized: Some(Ok(authorized)) }
                if authorized == version =>
                Some(true),
            Self::Once { .. } => Some(false),
            Self::Ask { authorized: Some(Err(unauthorized)) } if unauthorized == version => Some(false),
            _ => self.blanket_authorization(),
        }
    }

    pub fn will_authorize(&self) -> Option<bool> {
        match self {
            Self::Once { .. } | Self::Ask { authorized: Some(Ok(..)) } => Some(true),
            _ => self.blanket_authorization(),
        }
    }

    pub fn blanket_authorization(&self) -> Option<bool> {
        match self {
            Self::Always => Some(true),
            Self::Never => Some(false),
            _ => None,
        }
    }

    pub fn authorize_update(&mut self, version: String, authorize: bool) {
        match (&mut *self, authorize) {
            (Self::Always, true) => (),
            (Self::Always, false) => {
                *self = Self::ask_allow(version);
            },
            (Self::Never, false) => (),
            (Self::Never, true) => {
                *self = Self::only_once(version);
            },
            (Self::Ask { ref mut authorized }, true) => {
                *authorized = Some(Ok(version));
            },
            (Self::Ask { ref mut authorized }, false) => {
                *authorized = Some(Err(version));
            },
            (Self::Once { ref mut authorized }, true) => {
                *authorized = version;
            },
            (Self::Once { .. }, false) => {
                *self = Self::Never;
            },
        }
    }

    pub fn take_authorization(&mut self) -> Option<Result<String, String>> {
        let mut undo_once;
        let auth = match self {
            auth @ Self::Once { .. } => {
                undo_once = match mem::replace(auth, Self::Never) {
                    Self::Once { authorized } => Self::Ask { authorized: Some(Ok(authorized)) },
                    m => m,
                };
                &mut undo_once
            },
            auth => auth,
        };
        match auth {
            Self::Once { .. } => None,
            Self::Ask { authorized } => authorized.take(),
            Self::Always | Self::Never => None,
        }
    }
}

impl fmt::Display for ArcUpdatePreference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Ask { authorized: Some(Ok(v)) } => write!(f, "Allow {v}"),
            Self::Ask { authorized: Some(Err(v)) } => write!(f, "Ignore {v}"),
            Self::Once { authorized: v } => write!(f, "Just {v}"),
            pref => f.write_str(pref.label_english()),
        }
    }
}
