#[cfg(feature = "mumblelink-arcloader")]
pub use self::arcloader::{
    gw2_mumble,
    LinkedMem as MumbleLink,
    MumblePtr,
    UIScaling,
    UIState,
    NexusIdentity,
};
#[cfg(all(feature = "mumblelink-nexus", not(feature = "mumblelink-arcloader")))]
pub use self::nexus::{
    gw2_mumble,
    LinkedMem as MumbleLink,
    MumblePtr,
    UIScaling,
    UIState,
};
#[cfg(all(feature = "mumblelink-nexus", feature = "nexus", not(feature = "mumblelink-arcloader")))]
pub use self::nexus::NexusIdentity;

#[cfg(feature = "mumblelink-nexus")]
pub mod nexus {
    use {
        std::mem::transmute,
        crate::ui::{MapContext, MinimapPlacement, UiSize, UiState},
    };

    #[cfg(feature = "nexus")]
    pub use ::nexus::{
        data_link::mumble as gw2_mumble,
        event::MumbleIdentityUpdate as NexusIdentity,
    };
    #[cfg(not(feature = "nexus"))]
    pub use ::gw2_mumble_nexus as gw2_mumble;
    pub use self::gw2_mumble::{
        LinkedMem,
        MumblePtr,
        UiState as UIState,
        UIScaling,
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
            unsafe {
                Self::from_repr_unchecked(size as _)
            }
        }
    }
    impl From<UiSize> for UIScaling {
        fn from(size: UiSize) -> Self {
            unsafe {
                transmute(size.repr() as u8)
            }
        }
    }
}

#[cfg(feature = "mumblelink-arcloader")]
mod arcloader {
    use {
        std::mem::transmute,
        crate::ui::{MapContext, MinimapPlacement, UiSize, UiState},
    };

    pub use arcloader_mumblelink::{
        identity::ImpIdentity as NexusIdentity,
        gw2_mumble::{
            self,
            LinkedMem,
            MumblePtr,
            UiState as UIState,
            UIScaling,
        },
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
            unsafe {
                Self::from_repr_unchecked(size as _)
            }
        }
    }
    impl From<UiSize> for UIScaling {
        fn from(size: UiSize) -> Self {
            unsafe {
                transmute(size.repr() as u8)
            }
        }
    }
}
