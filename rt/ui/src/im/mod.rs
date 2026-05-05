pub mod colours;
pub mod draw;
pub mod image;
pub mod io;
mod macros;
pub mod tables;
pub mod text;
pub mod token;
pub mod tree;
pub mod ui;
pub mod widgets;

pub use self::{
    io::ImUi,
    token::{UiToken, UiTokenDyn, UiTokenMut},
    ui::{ImPos2, ImSize2, ImSpace, ImSpaces, ImVec2, WindowSpace},
};

#[cfg(not(feature = "imgui180"))]
#[path = "180"]
pub mod im180 {
    #[path = "fallback.rs"]
    pub mod fallback;
}
#[cfg(feature = "imgui180")]
#[path = "180/mod.rs"]
pub mod im180;
#[cfg(not(feature = "imgui192"))]
#[path = "192"]
pub mod im192 {
    #[path = "fallback.rs"]
    pub mod fallback;
}
#[cfg(feature = "imgui192")]
#[path = "192/mod.rs"]
pub mod im192;

pub mod prelude {
    pub use {
        super::{
            colours::{ImColour, ImColourContainer, ImColourIndex, ImColourStack, ImColourStackExt as _},
            draw::{ImContainer, ImContainerExt as _, ImDrawWindow, ImWidget},
            image::{ImTexture, ImTextureExt as _},
            io::{
                AsUi,
                ImContext,
                ImContextState,
                ImContextStateExt as _,
                ImDrawIo,
                ImIo,
                ImIoExt as _,
                ImPlatformIo,
                ImUi,
                ImUiContextExt as _,
            },
            tables::{ImTable, ImTableExt as _},
            text::{
                ImDrawText,
                ImDrawTextExt as _,
                ImDrawTextStack,
                ImFontStack,
                ImStr,
                ImStrExt,
                ImStrId,
                UiFontExt as _,
                UiText,
                UiTextExt as _,
                UiTextWrite,
            },
            token::{IntoTokenGuard as _, UiToken, UiTokenGuarded as _, UiTokenMut as _},
            tree::{ImTree, ImTreeExt as _, ImTreeStack},
            ui::{
                ImDraw,
                ImDrawExt as _,
                ImDrawItemStack,
                ImDrawTarget,
                ImDrawWindowStack,
                ImPos2,
                ImSize2,
                ImSpace,
                ImStyle,
                ImStyleExt as _,
                ImUiWindow,
                ImUiWindowExt as _,
                ImVec2,
                WindowSpace,
            },
            widgets::{
                self as imw,
                ImCondition,
                ImWidgetExt as _,
                ImWidgetLabelled as _,
                ImWidgetLabelledContainer as _,
                IM_STR_NONE,
            },
        },
        glamour::TransformMap as _,
    };
    #[allow(unused_imports)]
    pub(crate) use {
        super::{
            draw::state::InteractSignal,
            io::{ContextHookCallback, ContextHookRaw, ImContextHookInfo, ImUiContext, UiAllocatorFns},
            macros::imvec_newtype,
            tables::{ImTableLegacy, ImTableStack},
            text::{IntoImStrId, UiFontDyn, FMT_CSTR, FMT_STR},
            token::{
                ImGuard,
                IntoTokenGuard,
                UiTokenDrop,
                UiTokenDyn,
                UiTokenFn,
                UiTokenGuard,
                UiTokenGuarded,
                UiTokenMut,
                UiTokenZst,
            },
            ui::{ImFrameArena, ImSpaces},
            widgets::{
                ImPrimitive,
                ImPrimitiveArgsRange,
                ImPrimitiveContainer,
                ImPrimitiveData,
                Interacted,
            },
        },
        arcffi::{UserFreeFn, UserMallocFn},
        glamour::TransformMap,
    };

    #[cfg(feature = "imgui180")]
    pub(crate) use crate::im::im180::sys as sys180;
    #[cfg(feature = "imgui192")]
    pub(crate) use crate::im::im192::sys as sys192;
}
