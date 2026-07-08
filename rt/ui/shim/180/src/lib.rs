#![cfg_attr(rustfmt, rustfmt::skip)]
//! why is this here? good question.
//! primarily to set [taimi_ui] up to be a single override or `[patch]` point

pub use taimi_ui::im::im180::{
    sys,
    imgui::{
        color::{self, *},
        drag_drop::{self, *},
        draw_list::{self, *},
        internal::{self, *},
        Context, Io, Ui,
        Style, StyleColor, StyleVar,
        // input::*
        Key, FocusedWidget, MouseButton, MouseCursor,
        // window::*
        Window, WindowFlags, WindowFocusedFlags, ChildWindow,
        PopupModal,
        InputText, InputTextFlags,
        InputFloat3,
        // fonts::*
        Font, FontId,
        FontAtlas, FontGlyph, FontGlyphRanges,
        // renderer::*
        DrawData, TextureId,
        // widget::*
        ColorEditFlags, ColorEdit,
        ColorButton, ColorPicker,
        ComboBox,
        Drag,
        Image,
        ListBox,
        MenuItem, MenuToken,
        ButtonFlags,
        ProgressBar,
        Selectable,
        Slider,
        TabItem, TabItemFlags, TabItemToken,
        TreeNode, TreeNodeFlags, TreeNodeToken,
        TableColumnSetup, TableFlags, TableColumnFlags, TableSortDirection, TableToken,
        Id, IdStackToken, StyleStackToken, FontStackToken,
        Condition,
    },
};
