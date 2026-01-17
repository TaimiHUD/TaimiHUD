use {
    crate::{
        controller::{
            pathing::{
                registry::{PackInfoSignature, PackVecOf, UnloadedReason},
                shared::{
                    PathingShared,
                    SharedLoaderPacksInfo,
                    SharedPackConfig,
                    SharedPackInfo,
                    SharedPackLoad,
                    SharedPackLoaded,
                },
                PathingController,
                PathingEvent,
            },
            Controller,
        },
        exports::runtime as rt,
        render::element::prelude::*,
    },
    std::{
        fmt::Write,
        mem,
        sync::{Arc, Weak},
    },
    taimi_hoard::{loc::LocationMut, str_opt, str_opt_ref},
    taimi_meta::packs::{CategoryIndex, CategoryPath, PackIndex, PackPath},
    taimi_pack::{
        attributes::{self, AttrString, MarkerAttributes},
        Pack,
    },
    taimi_sync::{
        arcs::ArcPtrCmp,
        watched::{watch, Watched, Watcher},
    },
};

#[allow(unused_imports)]
pub use self::{
    categories::{
        CategoryAction,
        CategoryActionSlot,
        CategoryCollectionState,
        CategoryInfo,
        DrawCategoryCollection,
        DrawCategoryCollectionTree,
        DrawCategoryHeader,
        DrawCategoryTooltip,
        DrawPackUnloaded,
    },
    menu::{DrawCategoryCollectionMenu, DrawCategoryContextMenu, DrawCategoryMenu, DrawPackContextMenu},
    toggles::{DecorateCategoryHeader, DrawCategoryToggle, DrawPackRoots},
};

mod categories;
mod menu;
/// TODO: un-pub!
pub(in super::super) mod interact;
mod toggles;

#[derive(Debug, Default)]
pub struct PackElements {
    pub shared: Option<Arc<PathingShared>>,
    pub packs_rx: Watcher<SharedLoaderPacksInfo>,
    pub pack_state: PackVecOf<PackElement>,
    pub context_menu: Option<(PackPath, Option<CategoryPath>)>,
}
impl PackElements {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pre_draw(&mut self, visibility: PackVisibility) {
        if self.shared.is_none() {
            Controller::with_sender(|s| {
                if let Some(pathing) = &s.pathing {
                    self.shared = Some(pathing.shared.clone());
                }
            });
            if let Some(shared) = &self.shared {
                self.packs_rx.restart_watching(&shared.packs.packs);
            }
        }
        let Some(_shared) = &self.shared else { return };
        if let Some(packs) = self.packs_rx.try_read_if_changed() {
            let mut packs_iter = packs.values();
            for (pack_state, pack) in self.pack_state.values_mut().zip(&mut packs_iter) {
                let prev_info_sig = pack_state.state.info.sig;
                if ArcPtrCmp::from_mut(&mut pack_state.state.info).clone_from_arc(&pack.info) {
                    pack_state.state.damage.info = Some(prev_info_sig);
                }
            }
            // any remainding are new packs...
            for pack in packs_iter {
                self.pack_state.data.push(PackElement::new(&pack));
            }
        }
        for pack in self.pack_state.values_mut() {
            pack.pre_draw(visibility);
        }
    }
    pub fn draw<'ui, U>(&mut self, ui: &mut U) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        for pack in self.pack_state.values_mut() {
            match self.context_menu {
                Some((path, _)) if path == pack.state.pack_path() => (),
                _ => pack.context_menu = None,
            }
            if matches!(pack.state.unloaded, Some(UnloadedReason::Gravestone)) {
                continue
            }
            pack.draw(ui);
            if let Some(cat_path) = pack.context_menu {
                let new_menu = (pack.state.pack_path(), cat_path);
                if self.context_menu != Some(new_menu) {
                    log::trace!("opening menu for {} {cat_path:?}", pack.state.pack_path());
                }
                self.context_menu = Some((pack.state.pack_path(), cat_path));
            }
        }

        let (mut menu_pack, menu_cat) = match self.context_menu {
            Some((path, cat)) => (self.pack_state.lookup_mut(&path), cat),
            None => (None, None),
        };
        if let Some(_menu) = ui.begin_popup(DrawPackContextMenu::<U>::id(), Default::default()) {
            if let (Some(pack), None) = (menu_pack.as_mut(), menu_cat) {
                pack.draw_pack_context(ui);
            } else {
                ui.close_current_popup();
            }
        } else if let Some((_, None)) = self.context_menu {
            self.context_menu = None;
        }
        if let Some(_menu) = ui.begin_popup(DrawCategoryContextMenu::<U>::id(), Default::default()) {
            if let (Some(pack), Some(cat_path)) = (menu_pack.as_mut(), menu_cat) {
                pack.draw_category_context(ui, cat_path);
            } else {
                ui.close_current_popup();
            }
        } else if let Some((_, Some(..))) = self.context_menu {
            self.context_menu = None;
        }
    }

    pub fn any_loaded(&self) -> bool {
        self.pack_state.values().any(|p| p.state.info.info.is_some())
    }
}
#[derive(Debug)]
pub struct PackElement {
    pub state: PackElementState,
    pub categories: CategoryCollectionState,
    /// displaying the tooltip for a category (or pack pseudo-root)
    pub hovered: Option<Option<CategoryPath>>,
    /// displaying context menu
    pub context_menu: Option<Option<CategoryPath>>,
}
impl PackElement {
    pub fn new(pack: &SharedPackLoad) -> Self {
        Self {
            state: PackElementState::new(pack),
            categories: CategoryCollectionState::default(),
            hovered: None,
            context_menu: None,
        }
    }

    pub fn pre_draw(&mut self, visibility: PackVisibility) {
        if let PackVisibility::Closed = visibility {
            self.hovered = None;
        }
        let damage = self.state.pre_draw(visibility);
        if let Some(..) = self.hovered {
            self.state.populate_display_name();
        }
        let category_visibility = match visibility {
            PackVisibility::Visible if !self.any_roots_open() => PackVisibility::Pending,
            v => v,
        };
        self.categories
            .pre_draw(&self.state, &damage, category_visibility);
    }

    pub fn draw_pack_tooltip<'ui, U>(&mut self, ui: &mut U, title_visible: bool, reason_visible: bool) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.hovered = Some(None);
        let title_template = self
            .state
            .title_template()
            .unwrap_or(DrawCategoryTooltip::<U>::NAME_TEMPLATE);
        DrawCategoryTooltip::draw_tooltip(ui, title_template, |ui| {
            self.draw_pack_tooltip_contents(ui, title_visible, reason_visible)
        });
    }
    pub fn draw_pack_tooltip_contents<'ui, U>(&self, ui: &mut U, title_visible: bool, reason_visible: bool) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let title = (!title_visible).then_some(self.state.display_name()).flatten();
        if let Some(title) = title {
            let _title_font = ui.push_font(NexusLinkFont::Big);
            ui.text(title);
            ui.spacing();
        }
        let path = rt::relative_path(&self.state.info.path);
        let path = path.strip_prefix("addons/Taimi/pathing/").unwrap_or(path);
        ui.text_wrapped(format!("{}", path.display()));

        if let Some(unloaded) = &self.state.unloaded {
            if !reason_visible {
                DrawPackUnloaded::draw_reason_name(ui, Some(unloaded));
            }
            DrawPackUnloaded::<U>::with_reason_details(Some(unloaded), |msg| ui.text_wrapped(&msg));
        }
    }
}

#[derive(Debug)]
pub struct PackElementState {
    damage: PackDamageReport,
    pub info: Arc<SharedPackInfo>,
    pub config: Watched<SharedPackConfig>,
    pub loaded: watch::Receiver<SharedPackLoaded>,
    pub unloaded: Option<UnloadedReason>,
    pub pack: Option<Weak<Pack>>,

    /// info.categories is relied on too heavily atm for this to be useful
    category_flags: Option<()>,
    #[cfg(todo)]
    pub category_flags: Option<PackCategoryFlags>,
    pub display_name: String,
    pub id_name: String,
}
impl PackElementState {
    pub fn new(pack: &SharedPackLoad) -> Self {
        let mut loaded = pack.loaded.subscribe();
        loaded.mark_changed();
        Self {
            info: pack.info.clone(),
            config: Watched::start_watching(&pack.config),
            loaded,
            damage: PackDamageReport {
                info: Some(PackInfoSignature::EMPTY),
                ..PackDamageReport::ALL
            },
            unloaded: None,
            pack: None,
            category_flags: None,
            display_name: String::new(),
            id_name: String::new(),
        }
    }

    pub fn pre_draw(&mut self, visibility: PackVisibility) -> PackDamageReport {
        let mut damage = mem::take(&mut self.damage);
        self.damage.visibility = Some(visibility);
        if damage.visibility == Some(visibility) {
            damage.visibility = None;
        }
        if let Some(_config) = self.config.try_read_if_changed() {
            damage.config = true;
        }
        if self.loaded.has_changed().unwrap_or(false) {
            let loaded = self.loaded.borrow_and_update();
            damage.loaded = true;
            self.unloaded = loaded.unloaded.clone();
            self.pack = loaded.pack.as_ref().map(Arc::downgrade);
        }
        if let PackVisibility::Closed = visibility {
            self.cleanup_cache();
            return damage
        }
        if damage.info.is_some() {
            self.category_flags = None;
            self.display_name.clear();
            self.id_name.clear();
        }
        if let PackVisibility::Visible = visibility {
            self.populate_display_name();
        }
        if self.id_name.is_empty() && self.info.datasource.is_none() {
            if let Some(fname) = self.info.path.file_name() {
                let _ = write!(&mut self.id_name, "{}", fname.display());
            }
        }

        damage
    }
    /// deallocate cached data commonly useful for displaying UI
    fn cleanup_cache(&mut self) {
        self.display_name = String::new();
        self.id_name = String::new();
        self.category_flags = None;
    }
    fn populate_display_name(&mut self) {
        if self.display_name.is_empty() {
            let _ = write!(&mut self.display_name, "{}", self.info);
        }
    }

    pub fn display_name(&self) -> Option<&str> {
        str_opt_ref(&self.display_name)
    }
    pub(super) fn title_template(&self) -> Option<&str> {
        let display_name = self.display_name();
        let file_name = self.info.path.file_name().and_then(|n| n.to_str());
        let datasource_name = self.info.datasource.as_ref().map(|ds| &ds.path[..]);
        DrawCategoryTooltip::<()>::longest_title([display_name, file_name, datasource_name])
    }
    pub fn ui_id(&self) -> *const () {
        let id_name =
            str_opt_ref(&self.id_name).or_else(|| self.info.datasource.as_ref().map(|ds| &ds.path[..]));
        //let id_name = id_name.or(str_opt_ref(&self.display_name));
        id_name
            .map(|n| n.as_ptr() as *const ())
            .unwrap_or(self.info.index.path as usize as *const ())
    }
    pub fn pack_path(&self) -> PackPath {
        self.info.index
    }

    pub fn pack_data(&self) -> Option<Arc<Pack>> {
        self.pack.as_ref().and_then(Weak::upgrade)
    }

    pub fn activate_pack_data(&self) -> Result<Option<Arc<Pack>>, ()> {
        if let Some(pack_data) = self.pack_data() {
            return Ok(Some(pack_data))
        }
        let can_reactivate = match self.unloaded.as_ref() {
            Some(u) => u.can_reactivate(false),
            None => match self.loaded.borrow().pack.clone() {
                Some(pack) => return Ok(Some(pack)),
                None => true,
            },
        };
        match can_reactivate {
            true => {
                self.request_activate_data();
                Ok(None)
            },
            false => Err(()),
        }
    }
    fn request_activate_data(&self) -> bool {
        PathingController::try_send(PathingEvent::LoadPack(self.pack_path()))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackTooltip {
    pub title: Option<AttrString>,
    pub description: Option<AttrString>,
}
impl PackTooltip {
    pub const EMPTY: Self = Self { title: None, description: None };

    pub fn new<S: Into<Box<str>>>(title: Option<S>, description: Option<S>) -> Self {
        Self {
            title: title.map(attributes::string_into),
            description: description.map(attributes::string_into),
        }
    }
    pub fn from_attrs(attrs: &MarkerAttributes) -> Self {
        Self {
            title: attrs.tip_name.clone(),
            description: attrs.tip_description.clone(),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self { title: None, description: None })
    }
    pub fn get(&self) -> Option<&Self> {
        (!self.is_empty()).then_some(self)
    }

    pub fn borrowed(&self) -> PackTooltipRef<'_> {
        PackTooltipRef::from_tip(self)
    }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackTooltipRef<'a> {
    pub title: &'a str,
    pub description: &'a str,
}
impl<'a> PackTooltipRef<'a> {
    pub const EMPTY: Self = Self { title: "", description: "" };

    #[inline]
    pub const fn new(title: &'a str, description: &'a str) -> Self {
        Self { title, description }
    }
    #[inline]
    pub const fn with_title(title: &'a str) -> Self {
        Self::new(title, "")
    }

    #[inline]
    pub fn from_tip(tip: &'a PackTooltip) -> Self {
        Self {
            title: tip.title.as_ref().map(|n| &n[..]).unwrap_or(""),
            description: tip.description.as_ref().map(|n| &n[..]).unwrap_or(""),
        }
    }
    #[inline]
    pub fn from_attrs(attrs: &'a MarkerAttributes) -> Self {
        Self {
            title: attrs.tip_name.as_ref().map(|n| &n[..]).unwrap_or(""),
            description: attrs.tip_description.as_ref().map(|n| &n[..]).unwrap_or(""),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self { title: "", description: "" })
    }

    pub fn title(&self) -> Option<&'a str> {
        str_opt(self.title)
    }
    pub fn description(&self) -> Option<&'a str> {
        str_opt(self.description)
    }
    #[cfg(todo = "unused")]
    fn to_tip(&self) -> PackTooltip {
        PackTooltip::new(self.title(), self.description())
    }
}
#[cfg(todo)]
impl ToOwned for PackTooltipRef<'_> {
    type Owned = PackTooltip;

    fn to_owned(&self) -> Self::Owned {
        PackTooltip::new(
            self.title().map(attributes::string_into),
            self.description().map(attributes::string_into),
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackDamageReport {
    visibility: Option<PackVisibility>,
    info: Option<PackInfoSignature>,
    config: bool,
    loaded: bool,
}
impl PackDamageReport {
    pub const CLEAN: Self = Self {
        visibility: None,
        info: None,
        config: false,
        loaded: false,
    };
    pub const ALL: Self = Self {
        visibility: Some(PackVisibility::Pending),
        info: Some(PackInfoSignature::EMPTY),
        config: true,
        loaded: true,
    };
}
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackVisibility {
    Visible = 4,
    /// would be visible, but scrolled off-screen
    Offset = 3,
    /// relevant and available (can be navigated to),
    /// usually inside a collapsed tree node or menu
    #[default]
    Pending = 2,
    /// window closed
    Closed = 1,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[must_use]
pub enum UiAction {
    Hovered,
    Dismissed,
    /// toggled/selected/committed/etc
    Primary,
    Clicked(KeyState),
}
impl UiAction {
    pub const LEFT_CLICK: Self = Self::Clicked(KeyState::BUTTON_L);
    pub const RIGHT_CLICK: Self = Self::Clicked(KeyState::BUTTON_R);
    #[cfg(todo = "unused")]
    pub const MIDDLE_CLICK: Self = Self::Clicked(KeyState::BUTTON_M);
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackAction {
    Unload {
        hard: bool,
    },
    Reload {
        hard: bool,
    },
    Load(Option<bool>),
    Cat {
        action: CategoryAction,
        /// None to operate on pack and/or root
        path: Option<CategoryPath>,
    },
}
impl PackAction {
    pub const ACTIVATE: Self = Self::Load(Some(true));
    pub const OFFLOAD: Self = Self::Load(Some(false));
    pub const UNLOAD: Self = Self::Unload { hard: false };
    pub const REMOVE: Self = Self::Unload { hard: true };
    pub const RELOAD: Self = Self::Reload { hard: false };
    pub const REFRESH: Self = Self::Reload { hard: true };
    #[cfg(todo)]
    pub const TOGGLE_LOADED: Self = Self::Load(None);
    pub const ENABLE: Self = Self::Root(CategoryAction::ENABLE);
    pub const DISABLE: Self = Self::Root(CategoryAction::DISABLE);

    #[allow(non_snake_case)]
    pub const fn Root(action: CategoryAction) -> Self {
        Self::Cat { path: None, action }
    }
    pub fn clobber(
        self,
        path: PackPath,
        dest: &mut PackActionSlot,
    ) -> Result<Option<(PackPath, Self)>, Self> {
        match &*dest {
            Some((p, present)) if *p == path && *present == self => return Ok(None),
            Some((_, present)) if *present > self => return Err(self),
            _ => (),
        }
        Ok(mem::replace(dest, Some((path, self))))
    }
    pub fn try_clobber(
        self,
        path: PackPath,
        dest: &mut PackActionSlot,
    ) -> Result<Option<(PackPath, Self)>, Self> {
        if let Some((dest_path, present)) = dest.take() {
            if let Some(couldnt_dismiss) = present.try_act(dest_path) {
                *dest = Some((dest_path, couldnt_dismiss));
                if self.try_act(path).is_none() {
                    // but we were able to dismiss self, good enough!
                    return Ok(None)
                }
            }
        }
        self.clobber(path, dest)
    }
    pub(crate) fn as_pathing_message(self, path: PackPath) -> Option<PathingEvent> {
        match self {
            Self::Cat { path: Some(cat_path), action } => return action.as_pathing_message(cat_path, path),
            Self::Cat { path: None, action } => action.as_pack_message(path),
            Self::Reload { hard } => Some(PathingEvent::ReloadPack(path, hard)),
            Self::Unload { hard } => Some(PathingEvent::UnloadPack(path, hard)),
            Self::Load(Some(false)) => Some(PathingEvent::OffloadPack(path)),
            Self::Load(Some(true)) => Some(PathingEvent::LoadPack(path)),
            Self::Load(None) => {
                log::info!("TODO: toggle pack load?");
                match () {
                    #[cfg(todo)]
                    _ => Some(PathingEvent::TogglePack(path)),
                    _ => None,
                }
            },
            // anything else requires context or state we don't have access to
            _unactionable => None,
        }
    }
    pub fn try_act(self, path: PackPath) -> Option<Self> {
        if path.path == PackIndex::MAX {
            return Some(self)
        }
        let msg = match self {
            Self::Cat { path: Some(cat_path), action } =>
                return action
                    .try_act(cat_path, path)
                    .map(|action| action.as_pack(cat_path)),
            #[cfg(todo)]
            Self::Copy => {
                // technically doable via render sender or something if we can find attrs but ew
            },
            // not important enough to keep around...
            Self::Cat {
                path: None,
                action: CategoryAction::HoverTooltip | CategoryAction::ContextMenu,
            } => return None,
            action => action.as_pathing_message(path),
        };
        match msg.map(PathingController::try_send) {
            Some(true) => None,
            _ => Some(self),
        }
    }
    pub fn clobbered_action(res: Result<Option<(PackPath, Self)>, Self>) -> Option<Self> {
        match res {
            Err(lost) | Ok(Some((_, lost))) => Some(lost),
            _ => None,
        }
    }
    pub(super) fn warn_clobbered(slot: &PackActionSlot, res: Result<Option<(PackPath, Self)>, Self>) {
        if let Some(clobbered) = Self::clobbered_action(res) {
            let slot = match slot {
                #[cfg(debug_assertions)]
                None => unreachable!("clobbered {clobbered:?} by nothing? weird..."),
                #[cfg(not(debug_assertions))]
                None => return,
                Some(slot) => slot,
            };
            if let Self::Cat {
                action: CategoryAction::HoverTooltip, ..
            } = clobbered
            {
                return
            }
            log::debug!("clobbered action {clobbered:?} in favour of {slot:?}");
        }
    }
}
impl From<CategoryAction> for PackAction {
    #[inline]
    fn from(action: CategoryAction) -> Self {
        Self::Root(action)
    }
}
pub type PackActionSlot = Option<(PackPath, PackAction)>;
impl CategoryAction {
    pub const fn as_pack(self, path: CategoryPath) -> PackAction {
        let path = match path.path {
            CategoryIndex::MAX => None,
            _ => Some(path),
        };
        PackAction::Cat { path, action: self }
    }
    pub const fn as_pack_root(self, path: CategoryPath) -> PackAction {
        PackAction::Cat { path: Some(path), action: self }
    }

    pub(crate) fn as_pack_message(self, pack_path: PackPath) -> Option<PathingEvent> {
        match self {
            Self::Enable(enable) => PackAction::Load(enable).as_pathing_message(pack_path),
            _ => None,
        }
    }
}
