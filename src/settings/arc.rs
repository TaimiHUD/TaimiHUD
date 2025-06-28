use anyhow::anyhow;
use std::collections::HashMap; 
use serde::{Serialize, Deserialize};
use windows::Win32::UI::Input::KeyboardAndMouse::{self as vk, VIRTUAL_KEY};

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct ArcSettings {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub bind_vks: HashMap<String, u16>,
}

impl ArcSettings {
    pub const VK_WINDOW_TOGGLE_PRIMARY: ArcVk = ArcVk::new("primary-window-toggle", vk::VK_M);
    pub const VK_WINDOW_TOGGLE_TIMERS: ArcVk = ArcVk::new("timer-window-toggle", vk::VK_K);
    pub const VK_WINDOW_TOGGLE_MARKERS: ArcVk = ArcVk::new("marker-window-toggle", vk::VK_L);
    pub const VK_WINDOW_TOGGLE_PATHING: ArcVk = ArcVk::new("pathing-window-toggle", vk::VK_N);
    pub const VK_RENDER_TOGGLE_PATHING: ArcVk = ArcVk::new("pathing-render-toggle", vk::VK_OEM_COMMA);
    pub const VK_TIMER_TRIGGERS: [ArcVk; 5] = [
        ArcVk::empty("timer-key-trigger-0"),
        ArcVk::empty("timer-key-trigger-1"),
        ArcVk::empty("timer-key-trigger-2"),
        ArcVk::empty("timer-key-trigger-3"),
        ArcVk::empty("timer-key-trigger-4"),
    ];

    pub const VK_WINDOWS: &'static [&'static ArcVk] = &[
        &Self::VK_WINDOW_TOGGLE_PRIMARY,
        &Self::VK_WINDOW_TOGGLE_TIMERS,
        #[cfg(feature = "markers")]
        &Self::VK_WINDOW_TOGGLE_MARKERS,
        #[cfg(feature = "space")]
        &Self::VK_WINDOW_TOGGLE_PATHING,
    ];

    pub fn get_vk(&self, binding: &ArcVk) -> Option<VIRTUAL_KEY> {
        self.bind_vks.get(binding.id).copied().map(VIRTUAL_KEY)
            .or(binding.vkeycode_default())
    }

    pub fn binding_matches(&self, binding: &ArcVk, vkeycode: VIRTUAL_KEY) -> bool {
        match self.get_vk(binding) {
            Some(setting) => vkeycode == setting,
            None => false,
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
        Self {
            id,
            default_vk: default_vk.0,
        }
    }

    pub fn vkeycode_default(&self) -> Option<VIRTUAL_KEY> {
        match self.default_vk {
            0 => None,
            vk => Some(VIRTUAL_KEY(vk)),
        }
    }

    pub fn get_name(&self) -> String {
        if let Some(id) = self.id.strip_prefix("timer-key-trigger-") {
            return crate::fl!("timer-key-trigger", id = id)
        }

        crate::LANGUAGE_LOADER.get(self.id)
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
        let settings = crate::SETTINGS.get()?;
        let settings = settings.try_read().ok()?;
        settings.arc().get_vk(self)
    }

    pub fn set_vkeycode(&self, new: VIRTUAL_KEY) -> anyhow::Result<()> {
        let mut settings = crate::SETTINGS.get()
            .ok_or_else(|| anyhow!("settings unavailable"))?
            .blocking_write();
        settings.arc_mut().bind_vks.insert(self.id.into(), new.0);
        Ok(())
    }
}
