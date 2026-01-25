use crate::controller::pathing::{PathingController, PathingEvent};
use crate::controller::pathing::registry::PackPath;
use taimi_meta::packs::CategoryPath;
use core::mem;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CategoryAction {
    HoverTooltip,
    /// right-clicked
    ContextMenu,
    Open(Option<bool>),
    OpenChildren(Option<bool>),
    Copy,
    Enable(Option<bool>),
    EnableParents(bool),
    EnableChildren(Option<bool>),
    /// EnableSiblings
    Isolate(Option<bool>),
    ResetChildren,
    ResetSiblings,
}
impl CategoryAction {
    pub const ISOLATE: Self = Self::Isolate(None);
    pub const TOGGLE: Self = Self::Enable(None);
    pub const ENABLE: Self = Self::Enable(Some(true));
    pub const DISABLE: Self = Self::Enable(Some(false));

    pub fn clobber(
        self,
        path: CategoryPath,
        dest: &mut CategoryActionSlot,
    ) -> Result<Option<(CategoryPath, Self)>, Self> {
        match &*dest {
            Some((p, present)) if *p == path && *present == self => return Ok(None),
            Some((_, present)) if *present > self => return Err(self),
            _ => (),
        }
        Ok(mem::replace(dest, Some((path, self))))
    }
    pub fn try_clobber(
        self,
        path: CategoryPath,
        pack_path: PackPath,
        dest: &mut CategoryActionSlot,
    ) -> Result<Option<(CategoryPath, Self)>, Self> {
        if let Some((dest_path, present)) = dest.take() {
            if let Some(couldnt_dismiss) = present.try_act(dest_path, pack_path) {
                *dest = Some((dest_path, couldnt_dismiss));
                if self.try_act(path, pack_path).is_none() {
                    // but we were able to dismiss self, good enough!
                    return Ok(None)
                }
            }
        }
        self.clobber(path, dest)
    }
    pub(crate) fn as_pathing_message(
        self,
        path: CategoryPath,
        pack_path: PackPath,
    ) -> Option<PathingEvent> {
        match self {
            Self::Enable(enable) => Some(PathingEvent::CategoryEnableSet(pack_path, path, enable)),
            Self::EnableChildren(..) | Self::EnableParents(..) => None,
            Self::ResetChildren | Self::ResetSiblings => None,
            Self::Isolate(..) => None,
            #[cfg(todo)]
            Self::Copy => {
                // technically doable via render sender or something if we can find attrs but ew
            },
            // anything else requires context or state we don't have access to
            _unactionable => None,
        }
    }
    pub fn try_act(self, path: CategoryPath, pack_path: PackPath) -> Option<Self> {
        let msg = match self {
            #[cfg(todo)]
            Self::Copy => {
                // technically doable via render sender or something if we can find attrs but ew
            },
            // not important enough to keep around...
            Self::HoverTooltip | Self::ContextMenu => return None,
            #[cfg(todo = "unnecessary")]
            action if path.path == CategoryIndex::MAX => action.as_pack_message(pack_path),
            action => action.as_pathing_message(path, pack_path),
        };
        match msg.map(PathingController::try_send) {
            Some(true) => None,
            _ => Some(self),
        }
    }
    pub fn clobbered_action(res: Result<Option<(CategoryPath, Self)>, Self>) -> Option<Self> {
        match res {
            Err(lost) | Ok(Some((_, lost))) => Some(lost),
            _ => None,
        }
    }
    pub(crate) fn warn_clobbered(
        slot: &CategoryActionSlot,
        res: Result<Option<(CategoryPath, Self)>, Self>,
    ) {
        if let Some(clobbered) = Self::clobbered_action(res) {
            let slot = match slot {
                #[cfg(debug_assertions)]
                None => unreachable!("clobbered {clobbered:?} by nothing? weird..."),
                #[cfg(not(debug_assertions))]
                None => return,
                Some(slot) => slot,
            };
            if let CategoryAction::HoverTooltip = clobbered {
                return
            }
            log::debug!("clobbered action {clobbered:?} in favour of {slot:?}");
        }
    }
}
pub type CategoryActionSlot = Option<(CategoryPath, CategoryAction)>;
