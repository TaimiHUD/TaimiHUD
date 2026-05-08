#[cfg(feature = "mumblelink-arcloader")]
pub use self::arcloader::{
    gw2_mumble,
    LinkedMem as MumbleLink,
    MumblePtr,
    NexusIdentity,
    UIScaling,
    UIState,
};
#[cfg(all(
    feature = "mumblelink-nexus",
    feature = "nexus",
    not(feature = "mumblelink-arcloader")
))]
pub use self::nexus::NexusIdentity;
#[cfg(all(feature = "mumblelink-nexus", not(feature = "mumblelink-arcloader")))]
pub use self::nexus::{gw2_mumble, LinkedMem as MumbleLink, MumblePtr, UIScaling, UIState};

#[cfg(feature = "mumblelink-nexus")]
pub mod nexus {
    #[cfg(not(feature = "nexus"))]
    pub use ::gw2_mumble_nexus as gw2_mumble;
    #[cfg(feature = "nexus")]
    pub use ::nexus::{data_link::mumble as gw2_mumble, event::MumbleIdentityUpdate as NexusIdentity};
    use {
        crate::ui::{MapContext, MinimapPlacement, UiSize, UiState},
        std::mem::transmute,
    };

    pub use self::gw2_mumble::{LinkedMem, MumblePtr, UIScaling, UiState as UIState};

    impl From<UIState> for UiState {
        #[inline]
        fn from(ui_state: UIState) -> Self {
            Self::from_bits_retain(ui_state.bits())
        }
    }

    impl From<UiState> for UIState {
        #[inline]
        fn from(ui_state: UiState) -> Self {
            Self::from_bits_retain(ui_state.bits())
        }
    }

    impl From<UIState> for MinimapPlacement {
        #[inline]
        fn from(ui_state: UIState) -> Self {
            UiState::from(ui_state).into()
        }
    }

    impl From<UIState> for MapContext {
        #[inline]
        fn from(ui_state: UIState) -> Self {
            UiState::from(ui_state).into()
        }
    }

    impl From<UIScaling> for UiSize {
        fn from(size: UIScaling) -> Self {
            unsafe { Self::from_repr_unchecked(size as _) }
        }
    }
    impl From<UiSize> for UIScaling {
        fn from(size: UiSize) -> Self {
            unsafe { transmute(size.repr() as u8) }
        }
    }
}

#[cfg(feature = "mumblelink-arcloader")]
mod arcloader {
    pub use arcloader_mumblelink::gw2_mumble::{self, LinkedMem, MumblePtr, UIScaling, UiState as UIState};
    #[cfg(feature = "nexus")]
    pub use arcloader_mumblelink::identity::ImpIdentity as NexusIdentity;
    use {
        crate::ui::{MapContext, MinimapPlacement, UiSize, UiState},
        std::mem::transmute,
    };

    impl From<UIState> for UiState {
        #[inline]
        fn from(ui_state: UIState) -> Self {
            Self::from_bits_retain(ui_state.bits())
        }
    }

    impl From<UiState> for UIState {
        #[inline]
        fn from(ui_state: UiState) -> Self {
            Self::from_bits_retain(ui_state.bits())
        }
    }

    impl From<UIState> for MinimapPlacement {
        #[inline]
        fn from(ui_state: UIState) -> Self {
            UiState::from(ui_state).into()
        }
    }

    impl From<UIState> for MapContext {
        #[inline]
        fn from(ui_state: UIState) -> Self {
            UiState::from(ui_state).into()
        }
    }

    impl From<UIScaling> for UiSize {
        fn from(size: UIScaling) -> Self {
            unsafe { Self::from_repr_unchecked(size as _) }
        }
    }
    impl From<UiSize> for UIScaling {
        fn from(size: UiSize) -> Self {
            unsafe { transmute(size.repr() as u8) }
        }
    }
}
