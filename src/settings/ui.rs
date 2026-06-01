use {
    serde::{Deserialize, Serialize},
    taimi_hoard::{bool_or_none, f32_or_none},
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UiConfig {
    /// apply fixes for input and focus context-wide
    #[serde(default, skip_serializing_if = "bool_or_none::<{Self::DEFAULT_IMGUI_PATCH}>")]
    pub imgui_patch: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_IMGUI_INPUT_BUFFER}>"
    )]
    pub imgui_input_buffer: Option<bool>,
    /// defocus windows when clicking away
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_IMGUI_DEFOCUS}>"
    )]
    pub imgui_defocus: Option<bool>,
    /// whether ctrl+tab to cycle window focus is allowed
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_IMGUI_WINDOW_NAV}>"
    )]
    pub imgui_window_nav: Option<bool>,
    /// whether tab key cycles widget focus
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_IMGUI_KEYBOARD_NAV}>"
    )]
    pub imgui_keyboard_nav: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_WINDOW_ESCAPE}>"
    )]
    pub window_escape: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_MOUSE_REQUIRES_FOCUS}>"
    )]
    pub mouse_requires_focus: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_MOVE_REQUIRES_TITLEBAR}>"
    )]
    pub move_requires_titlebar: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_MOVE_REQUIRES_MODS}>"
    )]
    pub move_requires_mods: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_MOVE_REQUIRES_FOCUS}>"
    )]
    pub move_requires_focus: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "bool_or_none::<{Self::DEFAULT_SCROLL_REQUIRES_FOCUS}>"
    )]
    pub scroll_requires_focus: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "f32_or_none::<{Self::DEFAULT_SCROLL_SENSITIVITY.to_bits()}>"
    )]
    pub scroll_sensitivity: Option<f32>,

    #[serde(
        default,
        skip_serializing_if = "f32_or_none::<{Self::DEFAULT_FADE_UNFOCUSED.to_bits()}>"
    )]
    pub fade_unfocused: Option<f32>,
    #[serde(
        default,
        skip_serializing_if = "f32_or_none::<{Self::DEFAULT_FADE_BACKGROUND.to_bits()}>"
    )]
    pub fade_background: Option<f32>,
}

impl UiConfig {
    const DEFAULT_IMGUI_PATCH: bool = false;
    const DEFAULT_IMGUI_INPUT_BUFFER: bool = false;
    const DEFAULT_IMGUI_DEFOCUS: bool = false;
    const DEFAULT_IMGUI_WINDOW_NAV: bool = true;
    const DEFAULT_IMGUI_KEYBOARD_NAV: bool = true;
    const DEFAULT_WINDOW_ESCAPE: bool = true;
    const DEFAULT_MOUSE_REQUIRES_FOCUS: bool = false;
    const DEFAULT_MOVE_REQUIRES_TITLEBAR: bool = false;
    const DEFAULT_MOVE_REQUIRES_MODS: bool = false;
    const DEFAULT_MOVE_REQUIRES_FOCUS: bool = false;
    const DEFAULT_SCROLL_REQUIRES_FOCUS: bool = false;
    const DEFAULT_SCROLL_SENSITIVITY: f32 = 1.0;
    const DEFAULT_FADE_UNFOCUSED: f32 = 0.95f32;
    const DEFAULT_FADE_BACKGROUND: f32 = 0.8f32;

    pub fn imgui_patch(&self) -> bool {
        self.imgui_patch.unwrap_or(Self::DEFAULT_IMGUI_PATCH)
    }
    pub fn imgui_input_buffer(&self) -> bool {
        self.imgui_input_buffer
            .unwrap_or(Self::DEFAULT_IMGUI_INPUT_BUFFER)
    }
    pub fn imgui_defocus(&self) -> bool {
        self.imgui_defocus.unwrap_or(Self::DEFAULT_IMGUI_DEFOCUS)
    }
    pub fn imgui_window_nav(&self) -> bool {
        self.imgui_window_nav.unwrap_or(Self::DEFAULT_IMGUI_WINDOW_NAV)
    }
    pub fn imgui_keyboard_nav(&self) -> bool {
        self.imgui_keyboard_nav
            .unwrap_or(Self::DEFAULT_IMGUI_KEYBOARD_NAV)
    }
    pub fn window_escape(&self) -> bool {
        self.window_escape.unwrap_or(Self::DEFAULT_WINDOW_ESCAPE)
    }
    pub fn mouse_requires_focus(&self) -> bool {
        self.mouse_requires_focus
            .unwrap_or(Self::DEFAULT_MOUSE_REQUIRES_FOCUS)
    }
    pub fn move_requires_titlebar(&self) -> bool {
        self.move_requires_titlebar
            .unwrap_or(Self::DEFAULT_MOVE_REQUIRES_TITLEBAR)
    }
    pub fn move_requires_mods(&self) -> bool {
        self.move_requires_mods
            .unwrap_or(Self::DEFAULT_MOVE_REQUIRES_MODS)
    }
    pub fn move_requires_focus(&self) -> bool {
        self.move_requires_focus
            .unwrap_or(Self::DEFAULT_MOVE_REQUIRES_FOCUS)
    }
    pub fn scroll_requires_focus(&self) -> bool {
        self.scroll_requires_focus
            .unwrap_or(Self::DEFAULT_SCROLL_REQUIRES_FOCUS)
    }
    pub fn scroll_sensitivity(&self) -> f32 {
        self.scroll_sensitivity
            .unwrap_or(Self::DEFAULT_SCROLL_SENSITIVITY)
    }

    /// TODO: populate default from ImGuiCol_TitleBg alpha?
    pub fn fade_unfocused(&self) -> f32 {
        self.fade_unfocused.unwrap_or(Self::DEFAULT_FADE_UNFOCUSED)
    }
    pub fn fade_background(&self) -> f32 {
        self.fade_background.unwrap_or(Self::DEFAULT_FADE_BACKGROUND)
    }
}
