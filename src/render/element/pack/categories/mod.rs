use {
    super::{
        DrawCategoryToggle,
        PackAction,
        PackActionSlot,
        PackDamageReport,
        PackElementState,
        PackTooltip,
        PackTooltipRef,
        PackVisibility,
        UiAction,
    },
    crate::{
        controller::{
            pathing::{
                registry::{PackCategory, PackCategoryInfo, PackInfoSignature, PackRoot, UnloadedReason},
                PathingEvent,
                VisibilityFlagsExt as _,
            },
            Controller,
        },
        exports::runtime as rt,
        render::element::prelude::*,
        settings::state::ui::pathing::PathingFilterFlags,
    },
    std::{collections::BTreeMap, iter, sync::Arc},
    taimi_hoard::{flags::BitSet, iters::tree::DfsPre, loc::LocationRef, str_opt, str_opt_ref},
    taimi_meta::packs::{collections::CategorySet, CategoryIndex, CategoryPath, MapIndex, VisibilityFlags},
    taimi_pack::{
        attributes::InteractionAttributes,
        category::{id::AsFullId, Category, CategoryFlags, CategoryId},
    },
};

pub use self::{
    action::{CategoryAction, CategoryActionSlot},
    filter::{
        CategoryEnableFilterState,
        CategoryFilterQuery,
        CategorySearchFilter,
        CategorySearchQuery,
        PackCategoryMaskState,
    },
};

mod action;
mod filter;

#[derive(Debug)]
pub struct DrawCategoryHeader<'a, 'u, U: ?Sized + 'u> {
    pub ui: &'u mut U,
    pub display_name: &'a str,
    pub open: bool,
    pub open_cond: ImCondition,
    pub toggle_state: bool,
    pub is_leaf: Option<bool>,
    pub is_decorative: bool,
    pub is_header: bool,
    pub button_interact: Option<bool>,
    pub allow_overlap: bool,
    pub filter_selected: Option<bool>,
}
impl<'a, 'u, 'ui, U> DrawCategoryHeader<'a, 'u, U>
where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn draw(&mut self) -> (Option<UiAction>, Option<UiTokenDyn<'ui>>) {
        let tree_leaf = self.is_leaf.unwrap_or(true);
        let leaf = self.is_leaf.unwrap_or(true);
        let mut framed = self.is_header;
        let mut selected = false;
        let mut bullet = self.is_leaf == Some(true) || self.is_header & leaf;
        let mut tree_span_avail = false;
        match self.button_interact {
            Some(false) if self.is_decorative || leaf => tree_span_avail = true,
            #[cfg(todo)]
            None => tree_allow_overlap = true,
            _ => (),
        }
        if self.is_decorative {
            match self.is_leaf {
                Some(true) => selected = true,
                Some(false) =>
                    if self.filter_selected.is_none() && !framed {
                    } else {
                        bullet = true;
                    },
                None => {
                    // needs to stand out more among branches too..?
                    // TODO: less necessary once checkboxes become left-aligned
                    framed = true;
                    // would use this but leaf|framed results in strange text alignment...
                    // framed = true
                },
            }
        }
        let open = framed.then_some((self.open, self.open_cond));
        match self.filter_selected {
            None => (),
            Some(false) if selected => selected = false,
            Some(false) if framed => framed = false,
            Some(false) if bullet && !framed => bullet = false,
            Some(false) => framed ^= true,
            Some(true) if !selected => selected = true,
            Some(true) if !bullet && leaf => bullet = true,
            Some(true) => framed ^= true,
        }
        let header_name;
        let mut label = self.display_name;
        if framed && leaf {
            // results in strange text alignment... visually equivalent to selected,
            // but there's a benefit to the alignment actually, so...
            if self.is_decorative {
                header_name = format!("  {}", &self.display_name);
                label = &header_name[..];
            } else {
                framed = false;
                selected ^= true;
            }
        }
        let overlap = self.allow_overlap;
        let flags = match self.ui.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => imw::DynArgsTreeNode::new(Some({
                let f_common = im180::sys::ImGuiTreeNodeFlags_FramePadding
                    | im180::sys::ImGuiTreeNodeFlags_NoTreePushOnOpen;
                IntoIterator::into_iter([
                    Some(f_common),
                    framed.then_some(im180::sys::ImGuiTreeNodeFlags_Framed),
                    tree_span_avail.then_some(im180::sys::ImGuiTreeNodeFlags_SpanAvailWidth),
                    overlap.then_some(im180::sys::ImGuiTreeNodeFlags_AllowItemOverlap),
                    leaf.then_some(im180::sys::ImGuiTreeNodeFlags_Leaf),
                    bullet.then_some(im180::sys::ImGuiTreeNodeFlags_Bullet),
                    selected.then_some(im180::sys::ImGuiTreeNodeFlags_Selected),
                ])
                .flatten()
                .sum()
            })),
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => imw::DynArgsTreeNode::new(Some({
                let f_common = im192::sys::ImGuiTreeNodeFlags_FramePadding
                    | im192::sys::ImGuiTreeNodeFlags_NoTreePushOnOpen;
                IntoIterator::into_iter([
                    Some(f_common),
                    framed.then_some(im192::sys::ImGuiTreeNodeFlags_Framed),
                    tree_span_avail.then_some(im192::sys::ImGuiTreeNodeFlags_SpanAvailWidth),
                    overlap.then_some(im192::sys::ImGuiTreeNodeFlags_AllowOverlap),
                    leaf.then_some(im192::sys::ImGuiTreeNodeFlags_Leaf),
                    bullet.then_some(im192::sys::ImGuiTreeNodeFlags_Bullet),
                    selected.then_some(im192::sys::ImGuiTreeNodeFlags_Selected),
                ])
                .flatten()
                .sum()
            })),
            _ => Default::default(),
        };
        let open = (!leaf).then_some((self.open, self.open_cond));
        let tree_token = self.ui.begin_tree_node(open, self.display_name, label, flags);
        let action = match (self.open_cond, self.open, &tree_token) {
            (ImCondition::Always, open, token) if !leaf && open != token.is_some() =>
                Some(UiAction::Primary),
            _ if self.ui.is_item_right_clicked() => Some(UiAction::RIGHT_CLICK),
            #[cfg(todo)]
            _ if !framed && self.ui.is_item_clicked() => Some(UiAction::Primary),
            _ if leaf && self.ui.is_item_clicked() => Some(UiAction::LEFT_CLICK),
            _ if self.ui.is_item_hovered() => Some(UiAction::Hovered),
            _ => None,
        };
        (action, tree_token)
    }
}

pub struct DrawPackUnloaded<'a, 'u, U: ?Sized + 'u> {
    pub ui: &'u mut U,
    pub state: &'a PackElementState,
}
impl<'a, 'u, 'ui, U> DrawPackUnloaded<'a, 'u, U>
where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn draw(&mut self) -> Option<UiAction> {
        if let Some(UnloadedReason::Gravestone) = &self.state.unloaded {
            return None
        }
        self.header_unloaded()
    }

    fn header_unloaded(&mut self) -> Option<UiAction> {
        let reason = self.state.unloaded.as_ref();
        let is_button = match reason {
            | Some(UnloadedReason::Loading | UnloadedReason::Pending) | None => false,
            Some(..) => true,
        };
        let flags = match self.ui.imgui_version_num() {
            #[cfg(taimi_imgui = "180")]
            Some(im180::VERSION_NUM) => imw::DynArgsTreeNode::new(Some(
                im180::sys::ImGuiTreeNodeFlags_SpanAvailWidth
                    | im180::sys::ImGuiTreeNodeFlags_FramePadding
                    | im180::sys::ImGuiTreeNodeFlags_AllowItemOverlap
                    | im180::sys::ImGuiTreeNodeFlags_NoTreePushOnOpen
                    | is_button
                        .then_some(im180::sys::ImGuiTreeNodeFlags_Leaf)
                        .unwrap_or(0),
            )),
            #[cfg(taimi_imgui = "192")]
            Some(im192::VERSION_NUM) => imw::DynArgsTreeNode::new(Some(
                im192::sys::ImGuiTreeNodeFlags_SpanAvailWidth
                    | im192::sys::ImGuiTreeNodeFlags_FramePadding
                    | im192::sys::ImGuiTreeNodeFlags_AllowOverlap
                    | im192::sys::ImGuiTreeNodeFlags_NoTreePushOnOpen
                    | is_button
                        .then_some(im192::sys::ImGuiTreeNodeFlags_Leaf)
                        .unwrap_or(0),
            )),
            _ => Default::default(),
        };
        let display_name = &self.state.display_name[..];
        let id = self.state.ui_id();
        let node = self
            .ui
            .begin_tree_node(Some(ImCondition::always(false)), id, display_name, flags);
        let hovered = self.ui.is_item_hovered();
        let clicked = self.ui.is_item_clicked();
        let pressed = is_button && clicked;
        let open_context = self.ui.is_item_right_clicked();

        self.ui.same_line();
        Self::draw_reason_name(self.ui, reason);
        //self.ui.table_next_column();
        let res = if let Some(node) = node {
            node.end();
            !is_button || pressed
        } else {
            pressed
        };
        if open_context {
            Some(UiAction::RIGHT_CLICK)
        } else if res {
            Some(UiAction::Primary)
        } else if hovered {
            Some(UiAction::Hovered)
        } else if clicked {
            Some(UiAction::LEFT_CLICK)
        } else {
            None
        }
    }
    pub(super) fn draw_reason_name(ui: &mut U, reason: Option<&UnloadedReason>) {
        match reason {
            Some(UnloadedReason::Disabled | UnloadedReason::Gravestone) =>
                with_i18n!("disabled", |msg| ui.text(msg)),
            Some(UnloadedReason::Loading) => with_i18n!("loading", |msg| ui.text(msg)),
            Some(UnloadedReason::Pending) | None => with_i18n!("unloaded", |msg| ui.text(msg)),
            Some(reason @ (UnloadedReason::LoadingFailed(..) | UnloadedReason::UnknownFormat)) =>
                match reason {
                    UnloadedReason::UnknownFormat => with_i18n!("unknown-pack-format", |msg| ui.text(msg)),
                    _ => with_i18n!("pack-error", |msg| ui.text(msg)),
                },
        }
    }
    pub(super) fn with_reason_details<R, F: FnOnce(&str) -> R>(
        reason: Option<&UnloadedReason>,
        f: F,
    ) -> Option<R> {
        let is_initial = || {
            Controller::with_sender(|s| s.gameplay.as_ref().map(|g| g.borrow().is_initial()))
                .flatten()
                .unwrap_or(false)
        };
        match reason {
            Some(UnloadedReason::LoadingFailed(e)) => Some(f(&format!("{e:#}"))),
            Some(UnloadedReason::Pending) if is_initial() =>
                Some(with_i18n!("render-notice-gameplay-initial", |msg| f(&msg))),
            Some(UnloadedReason::Pending) => Some(with_i18n!("render-notice-gameplay", |msg| f(&msg))),
            Some(UnloadedReason::UnknownFormat) => Some(with_i18n!("pack-format-notice", |msg| f(&msg))),
            _ => None,
        }
    }
}

pub struct DrawCategoryTooltip<'a, 'u, U: ?Sized + 'u> {
    pub ui: &'u mut U,
    pub info: &'a CategoryInfo,
    pub tooltip: PackTooltipRef<'a>,
    pub display_name_visible: bool,
    pub include_copyable: bool,
}
impl<'a, 'u, 'ui, U> DrawCategoryTooltip<'a, 'u, U>
where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub(super) const NAME_TEMPLATE: &'static str = "Generic Copyable Marker Name";
    pub fn draw(&mut self) {
        if self.is_empty() {
            return
        }
        let title_template = self
            .title_template()
            .unwrap_or(DrawCategoryTooltip::<U>::NAME_TEMPLATE);
        if let Some(_token) = Self::begin_tooltip(self.ui, title_template) {
            self.draw_contents();
        }
    }

    pub fn draw_contents(&mut self) {
        let desc = self.tooltip.description();
        let title = match self.tooltip.title() {
            Some(title)
                if self.display_name_visible
                    && self.info.display_name().unwrap_or("").starts_with(title) =>
                None,
            title => title,
        };

        if let Some(title) = title {
            let _title_font = desc.map(|_| self.ui.push_font(NexusLinkFont::Big));
            self.ui.text(title);
        }

        if let Some(tip) = desc {
            self.ui.text_wrapped(tip);
        }

        let copyable = self.include_copyable.then(|| self.info.copyable()).flatten();
        if let Some((copy_value, copy_message)) = copyable {
            Self::draw_tooltip_copyable(self.ui, copy_value, copy_message);
        }
    }
    pub(super) fn title_template(&self) -> Option<&'a str> {
        let display_name = self.info.display_name().and_then(str_opt);
        display_name
    }
    pub fn is_empty(&self) -> bool {
        if self.tooltip.description().is_some() {
            return false
        }
        if let Some(title) = self.tooltip.title() {
            if !self.display_name_visible || !self.info.display_name().unwrap_or("").starts_with(title) {
                return false
            }
        }
        let copyable = self.include_copyable.then(|| self.info.copyable()).flatten();
        if let Some((copy_value, copy_message)) = copyable {
            if Self::copyable_has_tooltip(copy_value, copy_message) {
                return false
            }
        }

        true
    }

    pub(super) fn draw_tooltip<'uu, F: FnOnce(&'uu mut U)>(ui: &'uu mut U, title_template: &str, f: F) {
        if let Some(_token) = Self::begin_tooltip(ui, title_template) {
            f(ui)
        }
    }
    pub(super) fn begin_tooltip(
        ui: &mut U,
        title_template: &str,
    ) -> Option<(UiTokenDyn<'ui>, UiTokenDyn<'ui>)> {
        let id = ui.push_id("category_tooltip");
        let minsize = ui.calc_text_size(title_template);
        ui.window_prepare_size(ImSize2::new(0.0, minsize.height * 1.5), ImCondition::Appear);
        let size = ui.window_prepare_push_size_min_dyn(minsize.cast());
        if let Some(token) = ui.begin_tooltip() {
            size.end();
            {
                let _padding = ui.push_style_item_spacing(ImVec2::splat(f32::EPSILON));
                ui.dummy(minsize.with_height(f32::EPSILON));
            }
            Some((token, id))
        } else {
            None
        }
    }

    fn draw_tooltip_copyable(ui: &mut U, copy_value: &str, copy_message: Option<&str>) {
        let copy_message = match copy_message {
            // TODO: consider stubbing out generic success messages
            // like "x has been copied to your clipboard"
            m => m,
        };
        if let Some(copy_message) = copy_message {
            ui.text_wrapped(copy_message);
        } else if Self::copyable_value_has_message(copy_value) {
            ui.text_wrapped(&format!("\"{copy_value}\""));
        }
    }
}
impl<'a, 'u, 'ui, U: ?Sized> DrawCategoryTooltip<'a, 'u, U> {
    pub(super) fn longest_title<'t, I: IntoIterator<Item = Option<&'t str>>>(titles: I) -> Option<&'t str> {
        titles.into_iter().flatten().max_by_key(|n| n.len())
    }
    fn copyable_has_tooltip(copy_value: &str, copy_message: Option<&str>) -> bool {
        copy_message.is_some() || Self::copyable_value_has_message(copy_value)
    }
    fn named_has_tooltip(tip: PackTooltipRef, display_name: &str) -> bool {
        tip.description().is_some() || !display_name.starts_with(tip.title)
    }

    /// since these aren't intended to be displayed, there's no canon name to use...
    /// if it looks like more than just a location link, we'll try to preview it
    fn copyable_value_has_message(copy_value: &str) -> bool {
        if !copy_value[..].starts_with('[') || !copy_value.ends_with(']') {
            return true
        }
        false
    }
}

impl super::PackElement {
    pub(super) fn any_roots_open(&self) -> bool {
        if !self.categories.open_menu.is_empty() {
            return true
        }

        self.state
            .info
            .info
            .as_ref()
            .map(|i| {
                i.categories
                    .root_paths()
                    .any(|r| self.categories.open_mask.contains(r))
            })
            .unwrap_or(false)
    }

    pub(super) fn act_post_draw<'ui, U>(
        &mut self,
        ui: &mut U,
        act_cat: CategoryActionSlot,
        act_pack: PackActionSlot,
        am_toggle: bool,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        #[cfg(todo = "unnecessary")]
        let was_hovered = self.hovered.is_some();
        let any_action = act_cat.is_some() || act_pack.is_some();
        let mut hovered = None;
        let mut context_menu = None;
        self.perform_acts(ui, act_cat, act_pack, &mut hovered, &mut context_menu);
        match hovered {
            _ if context_menu.is_some() => (),
            Some(Some(path)) => {
                self.draw_category_tooltip(ui, path, true, !am_toggle);
            },
            Some(None) => {
                self.draw_pack_tooltip(ui, true, am_toggle);
            },
            None => (),
        };
        if !any_action && hovered.is_none() {
            self.hovered = None;
        }
        if let Some(context_menu) = context_menu {
            ui.open_popup(match context_menu {
                Some(..) => super::DrawCategoryContextMenu::<U>::id(),
                None => super::DrawPackContextMenu::<U>::id(),
            });
            self.context_menu = Some(context_menu);
        }
    }
    pub(super) fn act_post_draw_context<'ui, U>(
        &mut self,
        ui: &mut U,
        act_cat: CategoryActionSlot,
        act_pack: PackActionSlot,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let (mut _h, mut _c) = (None, None);
        self.perform_acts(ui, act_cat, act_pack, &mut _h, &mut _c);
    }
    fn perform_acts<'ui, U>(
        &mut self,
        ui: &mut U,
        mut act_cat: CategoryActionSlot,
        mut act_pack: PackActionSlot,
        hovered: &mut Option<Option<CategoryPath>>,
        context_menu: &mut Option<Option<CategoryPath>>,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let act_root_cat = match &act_cat {
            Some((path, CategoryAction::ResetSiblings | CategoryAction::Isolate(..)))
                if self.state.info.unique_root().map(|r| r.path()) == Some(*path) =>
                true,
            _ => false,
        };
        if act_root_cat {
            if let Some((_p, act_cat)) = act_cat.take() {
                let clobbered = PackAction::Root(act_cat).clobber(self.state.pack_path(), &mut act_pack);
                PackAction::warn_clobbered(&act_pack, clobbered);
            }
        }
        if let Some((path, act)) = act_cat {
            let msg = act.as_pathing_message(path, self.state.pack_path());
            match act {
                _ if msg.is_some() => (),
                CategoryAction::HoverTooltip => {
                    *hovered = Some(Some(path));
                },
                CategoryAction::ContextMenu => {
                    *context_menu = Some(Some(path));
                },
                CategoryAction::Copy => {
                    let copyable = self.categories.categories.get(&path).and_then(|c| c.copyable());
                    if let Some((copy_value, copy_msg)) = copyable {
                        Self::copy_copyable(ui, copy_value, copy_msg);
                    } else {
                        log::warn!("BUG: lost copy data for {path}");
                    }
                },
                CategoryAction::Open(new_state) => self.categories.update_open(path, new_state),
                CategoryAction::OpenChildren(new_state) => {
                    if let Some((cats, ..)) = self.state.info.category_info() {
                        let cat_is_open = self.categories.open_mask.contains(path);
                        let new_state = new_state.unwrap_or(!cat_is_open);
                        for child in cats.descendents_of(path).chain(iter::once(path)) {
                            self.categories.update_open(child, Some(new_state));
                        }
                    }
                },
                CategoryAction::EnableChildren(new_state) => {
                    if let Some((cats, ..)) = self.state.info.category_info() {
                        let paths = cats.descendents_of(path).chain(iter::once(path));
                        let children_enable = new_state
                            .unwrap_or_else(|| self.state.category_get_visibility(path).is_visible());
                        self.act_cat_enables(Some(children_enable), paths)
                    }
                },
                CategoryAction::EnableParents(parents_enable) => {
                    if let Some((cats, ..)) = self.state.info.category_info() {
                        let paths = cats.ancestors_of(path).chain(iter::once(path));
                        self.act_cat_enables(Some(parents_enable), paths)
                    }
                },
                CategoryAction::Isolate(new_state) => {
                    if let Some((cats, ..)) = self.state.info.category_info() {
                        let siblings_enable = match new_state {
                            None => {
                                let is_enabled =
                                    |path| self.state.category_get_visibility(path).is_visible();
                                let cat_is_enabled = is_enabled(path);
                                if cats
                                    .all_siblings_of(path)
                                    .map(|sib| is_enabled(sib))
                                    .all(|se| se == !cat_is_enabled)
                                {
                                    None
                                } else {
                                    Some(!cat_is_enabled)
                                }
                            },
                            Some(s) => Some(s),
                        };
                        let paths = cats.all_siblings_of(path);
                        self.act_cat_enables(siblings_enable, paths)
                    }
                },
                CategoryAction::ResetSiblings =>
                    if let Some((cats, ..)) = self.state.info.category_info() {
                        self.act_cat_reset(cats.all_siblings_of(path));
                    },
                CategoryAction::ResetChildren =>
                    if let Some((cats, ..)) = self.state.info.category_info() {
                        let paths = cats.descendents_of(path).chain(iter::once(path));
                        self.act_cat_reset(paths);
                    },
                CategoryAction::Enable(..) => {
                    #[cfg(debug_assertions)]
                    unreachable!();
                },
            }
            if let Some(msg) = msg {
                msg.try_send();
            }
        }
        if let Some((_path, act)) = act_pack {
            let msg = act.as_pathing_message(self.state.pack_path());
            match act {
                PackAction::OFFLOAD => {
                    let roots = self
                        .state
                        .info
                        .info
                        .as_ref()
                        .map(|i| i.roots.iter())
                        .into_iter()
                        .flatten();
                    for root in roots {
                        self.categories.open_mask.remove_at(root.path());
                    }
                },
                _ if msg.is_some() => (),
                PackAction::Cat {
                    action: CategoryAction::HoverTooltip,
                    path: _,
                } => {
                    *hovered = Some(None);
                },
                PackAction::Cat {
                    action: CategoryAction::ContextMenu,
                    path: _,
                } => {
                    *context_menu = Some(None);
                },
                PackAction::Cat {
                    action: CategoryAction::EnableChildren(new_state),
                    path: _,
                } =>
                    if let Some((cats, ..)) = self.state.info.category_info() {
                        let paths = cats.root_paths();
                        let enable = new_state.unwrap_or_else(|| {
                            !paths
                                .clone()
                                .any(|p| self.state.category_get_visibility(p).is_visible())
                        });
                        self.act_cat_enables(Some(enable), paths)
                    },
                act => {
                    #[cfg(taimi_debug)]
                    log::error!("DELETEME TODO: {} {act:?}", self.state.info);
                },
            }
            if let Some(msg) = msg {
                msg.try_send();
            }
        }
    }
    pub(super) fn act_cat_enables<P: IntoIterator<Item = CategoryPath>>(
        &self,
        enable: Option<bool>,
        paths: P,
    ) {
        self.act_cat_enables_dyn(Some(enable), &mut paths.into_iter())
    }
    pub(super) fn act_cat_reset<P: IntoIterator<Item = CategoryPath>>(&self, paths: P) {
        self.act_cat_enables_dyn(None, &mut paths.into_iter())
    }
    fn act_cat_enables_dyn(
        &self,
        enable: Option<Option<bool>>,
        paths: &mut dyn Iterator<Item = CategoryPath>,
    ) {
        let mut dirty: CategorySet = Default::default();
        let cat_info = self.state.info.category_info();
        let Some(sender) = self.state.config.watch.get_sender() else { return };
        sender.send_if_modified(|config| {
            for path in paths {
                let dev = config.config.visibility_deviation_for(path);
                let new_dev = match enable {
                    None => VisibilityFlags::empty(),
                    Some(None) => dev ^ VisibilityFlags::TOGGLE,
                    Some(Some(enable)) => {
                        let default = cat_info.map(|(i, ..)| !i.disabled.contains(path)).unwrap_or(true);
                        let mut dev = dev;
                        dev.set(VisibilityFlags::TOGGLE, enable ^ default);
                        dev
                    },
                };
                if dev != new_dev {
                    config.config.set_visibility_deviation(path, new_dev);
                    dirty.insert(path);
                }
            }
            !dirty.is_empty()
        });
        PathingEvent::CategoryEnableCommit(self.state.pack_path(), dirty).try_send();
    }

    pub(super) fn copy_copyable<'ui, U>(ui: &mut U, copy_value: &str, copy_message: Option<&str>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        ui.set_clipboard_text(copy_value);
        if let Some(copy_message) = copy_message {
            let _ = rt::send_alert(ui, copy_message);
        }
    }

    pub fn draw_category_tooltip<'ui, U>(
        &mut self,
        ui: &mut U,
        path: CategoryPath,
        display_name_visible: bool,
        include_copyable: bool,
    ) -> bool
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.hovered = Some(Some(path));
        let info = self
            .categories
            .categories
            .get(&path)
            .unwrap_or(&CategoryInfo::EMPTY);
        let is_root = self
            .state
            .info
            .info
            .as_ref()
            .map(|i| i.categories.is_root(path))
            .unwrap_or(false);
        let mut draw = DrawCategoryTooltip {
            ui: &mut *ui,
            info,
            tooltip: info.tooltip().unwrap_or(PackTooltipRef::EMPTY),
            display_name_visible,
            include_copyable,
        };
        if draw.is_empty() && !is_root {
            self.hovered = None;
            return false
        }
        if is_root {
            let title_template = DrawCategoryTooltip::<U>::longest_title([
                draw.title_template(),
                self.state.title_template(),
            ])
            .unwrap_or(DrawCategoryTooltip::<U>::NAME_TEMPLATE);
            DrawCategoryTooltip::draw_tooltip(ui, title_template, |ui| {
                self.draw_pack_tooltip_contents(ui, display_name_visible, !include_copyable);
                DrawCategoryTooltip {
                    ui,
                    info,
                    tooltip: info.tooltip().unwrap_or(PackTooltipRef::EMPTY),
                    display_name_visible,
                    include_copyable,
                }
                .draw_contents();
            });
        } else {
            draw.draw();
        }
        true
    }

    pub(super) fn reset_filter_interest(&mut self) {
        let cats = self.state.info.category_info().map(|(cats, ..)| &**cats);
        self.categories.filter_state.reset_interest(cats);
    }
    #[cfg(todo)]
    pub(super) fn apply_search_filter<F: CategorySearchFilter>(&mut self, filter: &mut F) {
        let pack_data = self.state.activate_pack_data().ok();
        self.categories
            .filter_state
            .update_search_candidates(self.state.pack_path(), pack_data, filter)
    }
    fn apply_search_filter(&mut self, filter: Option<()>) {
        let clear_mask = match (&self.categories.filter_state.search_candidates, filter) {
            (None, None) => return,
            (Some(..), None) => true,
            _ => false,
        };
        self.categories.filter_state.clear_search_candidates();
        if clear_mask {
            self.categories.filter_state.clear_mask();
        }
    }
}
impl super::PackElements {
    /// TODO: parallel and/or schedule via controller?
    pub fn apply_search_filter(&mut self) {
        for pack in self.pack_state.values_mut() {
            pack.apply_search_filter(self.filter_query.search.as_ref().map(drop));
        }
    }
    pub fn clear_search_filter(&mut self) {
        self.filter_query.search = None;
        self.apply_search_filter();
    }
}

impl PackElementState {
    pub fn category_flags(&self, path: CategoryPath) -> CategoryFlags {
        #[cfg(todo)]
        let category_flags = self.category_flags.as_ref().and_then(|f| f.lookup_get(&path));
        let category_flags = None;
        if let Some(category_flags) = category_flags {
            category_flags
        } else if let Some(info) = &self.info.info {
            info.categories.lookup_flags(path)
        } else {
            CategoryFlags::empty()
        }
    }
    pub fn category_info(&self, path: CategoryPath) -> Option<&PackCategory> {
        self.info
            .info
            .as_ref()
            .and_then(|info| info.categories.all().lookup_ref(&path))
    }
    pub fn category_visibility_deviation(&self, path: CategoryPath) -> VisibilityFlags {
        self.config
            .cached
            .as_ref()
            .map(|config| config.config.visibility_deviation_for(path))
            .unwrap_or(VisibilityFlags::empty())
    }
    pub fn category_get_visibility(&self, path: CategoryPath) -> VisibilityFlags {
        let dev = self.category_visibility_deviation(path);
        VisibilityFlags::from_category_flags(self.category_flags(path)) ^ dev
    }
}

pub struct DrawCategoryCollection<'a, 'u, 'ui, U: ?Sized + 'u> {
    pub ui: &'u mut U,
    pub state: &'a CategoryCollectionState,
    pub pack: &'a PackElementState,
    pub path_stack: Vec<CategoryPath>,
    /// XXX: these are ZSTs, just make a collection type for this?
    pub id_stack: Vec<UiTokenDyn<'ui>>,
}
impl<'a, 'u, 'ui, U> DrawCategoryCollection<'a, 'u, 'ui, U>
where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn new(ui: &'u mut U, state: &'a CategoryCollectionState, pack: &'a PackElementState) -> Self {
        Self {
            ui,
            state,
            pack,
            path_stack: Vec::new(),
            id_stack: Vec::new(),
        }
    }

    const DEPTH_LIMIT: usize = 78;
    #[cfg(todo)]
    pub fn draw_root(&mut self, path: CategoryPath, pseudo_root: bool) {
        self.push_and_draw(path, Some(pseudo_root));
        let cats = self.pack.info.info.as_ref().map(|i| &i.categories);

        while self.node_contents_visible() {
            if self.path_stack.len() >= Self::DEPTH_LIMIT {
                log::warn!("category nesting limit reached");
                break
            }
            let Some(cats) = cats else { continue };
            let cat_info = cats.all().lookup_ref(&self.category_path());
            let mut next = None;
            let popping = if let Some(child) = cat_info.and_then(|c| c.child()) {
                next = Some(child);
                false
            } else if let Some(sibling) = cat_info.and_then(|c| c.sibling()) {
                next = Some(sibling);
                self.ui.table_next_column();
                true
            } else {
                true
            };
            if popping && self.pop_to(path).is_none() {
                break
            }
            if let Some(next) = next.map(CategoryPath::with_path) {
                self.draw_one(next);
            }
        }
        while let Some(..) = self.pop_to(path) {}
        #[cfg(todo)]
        if draw_footer_idk {
            self.pop_draw(token);
            footer_stuff();
        }
        self.pop();
    }

    pub(super) fn prepare_toggle(
        &mut self,
        path: CategoryPath,
        pseudo_root: Option<bool>,
    ) -> DrawCategoryToggle<'a, '_, U> {
        let cats = self.pack.info.info.as_ref().map(|i| &i.categories);
        let cat = self.pack.category_info(path);
        let mut vis = VisibilityFlags::TOGGLES;
        let info = match self.state.categories.get(&path) {
            Some(info) => {
                vis = info.visibility;
                info
            },
            None => &CategoryInfo::EMPTY,
        };
        vis ^= self.pack.category_visibility_deviation(path);
        let is_lonely = match pseudo_root {
            Some(..) => false,
            None => cats.map(|cats| cats.lonely.contains(path)).unwrap_or(false),
        };
        let mut filter_whitelisted = None;
        let mut filter_whitelisted =
            || *filter_whitelisted.get_or_insert_with(|| self.state.filter_state.contains_category(path));
        let filter_selected = match self.state.filter_state.is_active() {
            false => None,
            true => match (pseudo_root, self.state.filter_state.all_filtered()) {
                (Some(..), true) => Some(false),
                (None, true) => None,
                (pseudo_root, false) if self.state.filter_state.is_matching() =>
                    match self.state.filter_state.matches_category(path) {
                        false
                            if pseudo_root.is_some()
                                && self.state.filter_state.search_candidates.is_none() =>
                            None,
                        matches => Some(matches),
                    },
                (pseudo_root, false) if pseudo_root.is_none() || is_lonely => match filter_whitelisted() {
                    false => Some(false),
                    true => None,
                },
                (_, false) => None,
            },
        };

        DrawCategoryToggle {
            ui: self.ui,
            info,
            #[cfg(todo)]
            category_path: path.pivot(self.pack.pack_path()),
            flags: self.pack.category_flags(path),
            toggle_state: vis,
            open_state: self.state.open_mask.contains(path),
            is_lonely,
            is_copyable: info.copyable().is_some(),
            has_children: cat.map(|cat| cat.child().is_some()).unwrap_or(true),
            // caller should decide this...
            #[cfg(todo)]
            pseudo_root: cats
                .map(|cats| cats.root_paths().all(|p| p == path))
                .unwrap_or(false),
            pseudo_root: pseudo_root.unwrap_or(false),
            filter_selected,
        }
    }

    pub fn pop(&mut self) -> Option<CategoryPath> {
        let path = self.path_stack.pop();
        drop(self.id_stack.pop());
        path
    }
    pub fn pop_to(&mut self, root: CategoryPath) -> Option<(CategoryPath, Option<UiTokenDyn<'ui>>)> {
        let path = self.path_stack.pop_if(|p| *p != root);
        if path.is_none() && self.path_stack.is_empty() {
            log::error!("cat stack missing {root}?");
        }
        path.map(|path| (path, self.id_stack.pop()))
    }
    pub fn push(&mut self, path: CategoryPath) -> Option<()> {
        if self.path_stack.len() >= Self::DEPTH_LIMIT {
            log::warn!("category nesting limit reached");
            return None
        }
        self.path_stack.push(path);
        let id = self.ui.push_id(self.category_info().ui_id(path));
        self.id_stack.push(id);
        Some(())
    }

    pub fn root_path(&self) -> CategoryPath {
        self.path_stack
            .first()
            .copied()
            .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX))
    }
    pub fn category_path(&self) -> CategoryPath {
        self.path_stack
            .last()
            .copied()
            .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX))
    }
    #[cfg(todo)]
    pub fn category_visibility_defaults(&self) -> VisibilityFlags {
        self.state
            .categories
            .get(&self.category_path())
            .map(|info| info.visibility)
            .unwrap_or_else(|| VisibilityFlags::from_category_flags(self.pack.category_flags()))
    }
    pub fn category_info(&self) -> &'a CategoryInfo {
        self.state
            .categories
            .get(&self.category_path())
            .unwrap_or(&CategoryInfo::EMPTY)
    }
}

/// TODO: don't hard-code toggles .-.
pub struct DrawCategoryCollectionTree<'a, 'u, 'ui, U: ?Sized + 'u> {
    pub draw: DrawCategoryCollection<'a, 'u, 'ui, U>,
    /// XXX: same as [self.draw.id_stack]
    pub node_stack: Vec<Option<UiTokenDyn<'ui>>>,
    pub act: CategoryActionSlot,
    pub unfilter_interest: Option<CategoryPath>,
}
impl<'a, 'u, 'ui, U> DrawCategoryCollectionTree<'a, 'u, 'ui, U>
where
    U: ?Sized + ImDrawWindow<'ui> + 'u,
{
    pub fn new(draw: DrawCategoryCollection<'a, 'u, 'ui, U>) -> Self {
        Self {
            draw,
            node_stack: Vec::new(),
            act: Default::default(),
            unfilter_interest: None,
        }
    }

    pub fn draw_root_then<R, F: FnOnce(&mut Self) -> R>(
        &mut self,
        path: CategoryPath,
        pseudo_root: bool,
        f: F,
    ) -> Option<R> {
        self.push_and_draw(path, Some(pseudo_root));

        let res = self.node_contents_visible().then(|| f(self));

        while let Some(..) = self.pop_to(path) {}

        #[cfg(todo)]
        if draw_footer_idk {
            self.pop_draw(token);
            footer_stuff();
        }
        self.pop();

        if res.is_none() {
            self.draw.ui.table_next_column();
        }

        res
    }
    pub fn draw_root(&mut self, path: CategoryPath, pseudo_root: bool) {
        self.draw_root_then(path, pseudo_root, move |draw| draw.draw_children_of(path));
    }
    pub fn draw_children_of(&mut self, path: CategoryPath) {
        let Some(cats) = self.draw.pack.info.info.as_ref().map(|i| &i.categories) else {
            return
        };
        #[cfg(todo)]
        return self.draw_children(path, &mut cats.nested_descendents_of(path));

        let mut cat_iter = cats.nested_descendents_of(path);
        let initial_depth = cat_iter.depth();
        let mut prev_depth = initial_depth;
        let mut pending_row = false;
        let mut children_filtered = match self.draw.state.filter_state.is_active() {
            true => vec![0usize],
            false => Vec::new(),
        };
        while let Some(cat_path) = cat_iter.next() {
            let depth = cat_iter.depth();
            if let Some(popping) = prev_depth.checked_sub(depth) {
                if !self.pop_amt_from(&mut children_filtered, path, popping) {
                    break
                }
            }
            match depth.checked_sub(prev_depth) {
                _ if children_filtered.is_empty() => (),
                Some(pushing) => {
                    children_filtered.extend(iter::repeat_n(0usize, pushing + 1));
                },
                _ => (),
            };
            let visible = self.draw.state.category_is_whitelisted(&self.draw.pack, cat_path);
            if visible {
                if depth <= prev_depth {
                    if pending_row {
                        self.draw.ui.table_next_column();
                        pending_row = false;
                    }
                } else {
                    {
                        let _padding = self.draw.ui.push_style_item_spacing(ImVec2::splat(f32::EPSILON));
                        //self.draw.ui.spacing();
                        self.draw.ui.dummy([1.0, 1.0]);
                    }
                }
            }
            prev_depth = depth;
            let open = if !visible {
                // avoid messing with node stack pop counts...
                self.push(cat_path);
                if let Some(filtered) = children_filtered.iter_mut().nth_back(1) {
                    *filtered = filtered.saturating_add(1);
                }
                Some(false)
            } else {
                let drawn = self.draw_one(cat_path).map(|drawn| drawn.is_some());
                if drawn.is_some() {
                    pending_row = true;
                    if !self
                        .draw
                        .state
                        .filter_state
                        .flags
                        .contains(PathingFilterFlags::ShowHidden)
                    {
                        // at least one child rendered, so don't complain anymore
                        if let Some(filtered) = children_filtered.iter_mut().nth_back(1) {
                            *filtered = usize::MAX;
                        }
                    }
                }
                drawn
            };
            if matches!(open, Some(false)) {
                cat_iter.skip_to_sibling();
            }
        }
        if let Some(popping) = prev_depth.checked_sub(initial_depth) {
            self.pop_amt_from(&mut children_filtered, path, popping);
        }
        debug_assert!(children_filtered.len() <= 1);
        if pending_row || true {
            self.draw.ui.table_next_column();
        }
    }
    fn pop_amt_from(
        &mut self,
        children_filtered: &mut Vec<usize>,
        path: CategoryPath,
        popping: usize,
    ) -> bool {
        for _ in 0..=popping {
            let was_open = self.node_contents_visible();
            let child_path = self.pop_to(path);
            let filtered = children_filtered.pop();
            match was_open.then_some(filtered) {
                Some(Some(0)) | Some(Some(usize::MAX)) => (),
                Some(Some(amt)) => {
                    if child_path.is_some() {
                        self.draw.ui.indent();
                    }
                    let checkpoint = self.draw.ui.cursor_pos();
                    let msg = format!("{amt} hidden by filter");
                    self.draw.ui.text_disabled(&msg);
                    if self.draw.ui.is_item_clicked() {
                        self.unfilter_interest = Some(child_path.unwrap_or(path));
                    } else if self.draw.ui.is_item_hovered() {
                        self.draw.ui.set_cursor_pos(checkpoint);
                        self.draw.ui.text(&msg);
                    }
                    if child_path.is_some() {
                        self.draw.ui.unindent();
                    } else {
                        self.draw.ui.spacing();
                    }
                },
                _ => (),
            }
            if child_path.is_none() {
                return false
            }
        }
        true
    }
    pub fn draw_children(
        &mut self,
        root_path: Option<CategoryPath>,
        cat_iter: &mut dyn DfsPre<Item = CategoryPath>,
    ) {
        let mut start_depth = None;
        let mut prev_depth: Option<usize> = None;
        let mut prev_closed = None;
        let mut pending_row = false;
        'cats: loop {
            let next = match prev_closed {
                Some(true) => cat_iter.node_next_sibling(),
                _ => cat_iter.next().map(Ok),
            };
            let Some(Ok(cat_path) | Err(cat_path)) = next else { break };

            let depth = cat_iter.node_depth();
            if start_depth.is_none() {
                start_depth = depth;
            }
            if let Some(depth) = depth {
                let pop_depth = prev_depth.and_then(|prev| prev.checked_sub(depth));
                if let Some(popping) = pop_depth {
                    for _ in 0..=popping {
                        let popped = match root_path {
                            Some(root_path) => self.pop_to(root_path),
                            None => self.pop(),
                        };
                        if popped.is_none() {
                            break 'cats
                        }
                    }
                }
                if depth <= prev_depth.unwrap_or(depth) {
                    self.draw.ui.table_next_column();
                    pending_row = false;
                } else {
                    self.draw.ui.spacing();
                }
            }
            prev_depth = depth;
            let drawn = self.draw_one(cat_path);
            pending_row = true;
            prev_closed = drawn.map(|d| d.is_none());
        }
        match root_path {
            Some(root_path) => while let Some(..) = self.pop_to(root_path) {},
            None => {
                let rem_depth =
                    start_depth.and_then(|start| prev_depth.and_then(|prev| prev.checked_sub(start)));
                if let Some(rem) = rem_depth {
                    for _ in 0..=rem {
                        let popped = match root_path {
                            Some(root_path) => self.pop_to(root_path),
                            None => self.pop(),
                        };
                        if self.pop().is_none() {
                            break
                        }
                    }
                }
            },
        }
        if pending_row {
            self.draw.ui.table_next_column();
        }
    }
    pub fn draw_one(&mut self, path: CategoryPath) -> Option<Option<()>> {
        let token = self.push_and_draw(path, None);
        token
    }

    fn push_and_draw(&mut self, path: CategoryPath, pseudo_root: Option<bool>) -> Option<Option<()>> {
        self.push(path)?;
        let mut toggle = self.draw.prepare_toggle(path, pseudo_root);
        let prev_toggle = toggle.toggle_state.is_visible();
        let (act, token) = toggle.draw();
        let res = token.as_ref().map(drop);
        let toggle_state = toggle.toggle_state;
        let is_copyable = toggle.is_copyable;
        self.set_draw_node(token);
        let act = match act {
            _ if prev_toggle != toggle_state.is_visible() =>
                Some(CategoryAction::Enable(Some(toggle_state.is_visible()))),
            Some(UiAction::Primary) => Some(CategoryAction::Open(Some(res.is_some()))),
            Some(UiAction::LEFT_CLICK) if is_copyable => Some(CategoryAction::Copy),
            Some(UiAction::RIGHT_CLICK) => Some(CategoryAction::ContextMenu),
            Some(UiAction::Hovered) => Some(CategoryAction::HoverTooltip),
            Some(act) => {
                #[cfg(taimi_debug)]
                log::debug!("DELETEME: category action {act:?} unexpected");
                None
            },
            None => None,
        };
        if let Some(act) = act {
            let clobbered = act.clobber(path, &mut self.act);
            CategoryAction::warn_clobbered(&self.act, clobbered);
        }
        Some(res)
    }

    /// in case we want to keep id token active for footer/menus/etc
    pub fn pop_draw(&mut self) {
        if self.pop_node().is_some() {
            self.node_stack.push(None);
        }
    }
    fn pop_node(&mut self) -> Option<Option<UiTokenDyn<'ui>>> {
        let token = self.node_stack.pop();
        if let Some(Some(..)) = &token {
            self.draw.ui.unindent();
        }
        token
    }

    pub fn pop(&mut self) -> Option<CategoryPath> {
        drop(self.pop_node());
        self.draw.pop()
    }
    pub fn pop_all(&mut self) {
        while self.pop().is_some() {}
    }
    pub fn pop_to(&mut self, root: CategoryPath) -> Option<CategoryPath> {
        let (path, id) = self.draw.pop_to(root)?;
        drop(self.pop_node());
        drop(id);
        Some(path)
    }
    pub fn push(&mut self, path: CategoryPath) -> Option<()> {
        let token = self.draw.push(path)?;
        self.node_stack.push(None);
        Some(token)
    }
    pub fn set_draw_node(&mut self, token: Option<UiTokenDyn<'ui>>) {
        match self.node_stack.pop_if(|n| n.is_none()) {
            Some(..) => {
                if token.is_some() {
                    self.draw.ui.indent();
                }
                self.node_stack.push(token);
            },
            None => log::error!("cat node tree({}) inconsistent", self.node_stack.len()),
        }
    }

    pub fn node_contents_visible(&self) -> bool {
        self.node_stack.last().map(Option::is_some).unwrap_or(false)
    }
}

#[derive(Debug, Default)]
pub struct CategoryCollectionState {
    pub info_sig: PackInfoSignature,
    pub categories: BTreeMap<CategoryPath, CategoryInfo>,
    pub open_menu: Vec<CategoryPath>,
    pub open_mask: BitSet,
    /// TODO: visible draw elements (or acting on an open event) could just
    /// mark specific cats as dirty/visible instead?
    pub open_sig_prev: CategoryIndex,
    pub filter_state: PackCategoryMaskState,
}
impl CategoryCollectionState {
    pub fn pre_draw(
        &mut self,
        pack: &PackElementState,
        filter_query: &CategoryFilterQuery,
        pack_damage: &PackDamageReport,
        visibility: PackVisibility,
    ) {
        self.filter_state.set_flags(filter_query.flags);
        let open_menu_sig = self
            .open_menu
            .last()
            .copied()
            .unwrap_or(CategoryPath::with_path(CategoryIndex::MAX))
            .path;
        let open_sig = self.open_mask.count() as CategoryIndex ^ open_menu_sig;
        let map_id = pack.map_info.as_ref().map(|i| i.path.path.get()).unwrap_or(0);
        let cats_dirty = pack_damage.info.is_some()
            || pack_damage.loaded
            || pack_damage.visibility.is_some()
            || self.open_sig_prev != open_sig;
        if pack_damage.info.is_some() {
            if pack.info.sig.is_empty() {
                self.cleanup_cache(true);
            } else if self.info_sig != pack.info.sig {
                self.categories.clear();
                self.filter_state.info_invalidated();
                //self.open_mask.clear();
            }
            self.info_sig = pack.info.sig;
        }
        if let PackVisibility::Closed = visibility {
            if pack_damage.visibility.is_some() {
                self.cleanup_cache(false);
            }
            return
        }
        self.open_sig_prev = open_sig;

        let mut filter_dirty = false;
        let category_info = pack.info.info.as_ref().map(|info| &info.categories);
        if self.filter_state.is_dirty_hidden(category_info) {
            self.filter_state.update_hidden(category_info);
        }
        let category_info = category_info.map(|cats| &**cats);
        let loaded_map_info = self
            .filter_state
            .flags
            .contains(PathingFilterFlags::CurrentMap)
            .then_some(pack.map_info.as_ref());
        if pack_damage.map.is_some() || self.filter_state.is_dirty_loaded(loaded_map_info) {
            match loaded_map_info {
                Some(map_info) => {
                    let map_info = match map_info {
                        Some(map_info) => Ok(map_info),
                        None if pack.pack.is_some() => Err(false),
                        None => Err({
                            let map_id = rt::mumble_link_ptr()
                                .ok()
                                .and_then(|ml| MapIndex::new(ml.read_map_id()));
                            map_id.map(|map_id| pack.info.has_map(map_id)).unwrap_or(false)
                        }),
                    };
                    self.filter_state.update_loaded(category_info, map_info)
                },
                _ => self.filter_state.clear_loaded(),
            }
            filter_dirty |= !self.filter_state.is_dirty_loaded(loaded_map_info);
        }
        let pack_config = pack.config.cached.as_ref().map(|c| &c.config);
        let enable_filter = self.filter_state.flags.enable_filter();
        if self.filter_state.is_dirty_enable(enable_filter) {
            match enable_filter {
                Some(enable) => self
                    .filter_state
                    .update_enable(pack_config, category_info, enable),
                _ => self.filter_state.clear_enable(),
            }
            filter_dirty |= !self.filter_state.is_dirty_enable(enable_filter);
        }
        if self.filter_state.is_dirty_search(filter_query.search.as_ref()) {
            match filter_query.search.as_ref() {
                Some(query) => {
                    let filtered_loaded = match loaded_map_info {
                        Some(Some(..)) => self.filter_state.loaded.as_ref().map(|loaded| loaded.is_empty()),
                        _ => None,
                    };
                    let filtered_enabled = match enable_filter {
                        Some(en) if en != matches!(pack.unloaded, Some(UnloadedReason::Disabled)) =>
                            Some(en),
                        Some(false) if pack.unloaded.is_some() => Some(false),
                        _ => None,
                    };
                    let pack_data = match pack.pack_data() {
                        Some(pd) => Some(Some(Some(pd))),
                        None if filtered_loaded == Some(true) => Some(None),
                        None if filtered_loaded == Some(false) => None,
                        None if filtered_enabled == Some(true) => Some(None),
                        None if filtered_enabled == Some(false) => None,
                        _ => Some(None),
                    };
                    if let Some(pack_data) = pack_data {
                        let pack_data = pack_data.as_ref().map(|pd| pd.as_ref().map(|pd| &**pd));
                        self.filter_state.update_search_candidates(
                            pack.pack_path(),
                            category_info,
                            pack_data,
                            query,
                        )
                    }
                },
                None => self.filter_state.clear_search_candidates(),
            }
            filter_dirty |= !self.filter_state.is_dirty_search(filter_query.search.as_ref());
        }

        let Some(info) = &pack.info.info else {
            self.filter_state.clear_mask();
            return
        };

        if filter_dirty {
            let category_info = match () {
                #[cfg(todo = "unnecessary")]
                _ => category_info,
                _ => Some(&*info.categories),
            };
            self.filter_state.refresh_mask(category_info);
        }

        if cats_dirty {
            let cats = &info.categories;
            match visibility {
                PackVisibility::Visible => {
                    let visible_cats = info
                        .categories
                        .root_paths()
                        .flat_map(|root| self.all_visible_children(cats, root).chain(iter::once(root)));
                    let mut missing_info = visible_cats
                        .filter(|path| !self.categories.contains_key(path))
                        .peekable();
                    let pack_data = match missing_info.peek() {
                        None => None,
                        _ => pack.activate_pack_data().ok().flatten(),
                    };
                    if let Some(pack_data) = pack_data {
                        let missing_info: BitSet = missing_info.collect();
                        for path in missing_info.iter_of::<CategoryPath>() {
                            let Some((_id, category)) =
                                pack_data.categories.all_categories.get_index(path.path as usize)
                            else {
                                log::error!("missing {path} from {}", pack.info);
                                continue
                            };
                            self.categories
                                .insert(path, CategoryInfo::from_pack_category(category));
                        }
                    }
                },
                PackVisibility::Pending => {
                    self.categories.retain(|&path, _| {
                        Self::is_path_visible(&cats, &self.open_mask, &self.open_menu, path)
                    });
                },
                _ => (),
            }
            for root in &info.roots {
                let path = root.path();
                self.categories
                    .entry(path)
                    .or_insert_with(|| CategoryInfo::from_pack_root(root));
            }
        }
    }
    fn cleanup_cache(&mut self, purge: bool) {
        self.categories = Default::default();
        if purge {
            self.open_mask = Default::default();
            self.filter_state.clear();
        } else {
            if self.filter_state.is_active() {
                self.filter_state.clear_active();
            }
        }
        self.open_sig_prev = 0;
        self.info_sig = PackInfoSignature::EMPTY;
    }

    /// don't assume order (DFS atm)
    ///
    /// excludes the root, and produces nothing if root isn't open
    ///
    /// TODO: use the nested iter that can skip across closed branches
    pub fn all_visible_children<'a, 'c>(
        &'a self,
        cats: &'c PackCategoryInfo,
        root: CategoryPath,
    ) -> impl Iterator<Item = CategoryPath> + 'a + 'c
    where
        'a: 'c,
        'c: 'a,
    {
        let open_mask = &self.open_mask;
        let open_menu = &self.open_menu[..];
        cats.descendents_of(root)
            .filter(|&path| Self::is_path_visible(cats, open_mask, open_menu, path))
    }
    fn is_path_open_menu(open_menu: &[CategoryPath], path: CategoryPath) -> bool {
        open_menu.contains(&path)
    }

    /// TODO: also apply filters like current-map
    /// (beware multiple UI elements may have differing filter states?)
    pub fn is_path_visible(
        cats: &PackCategoryInfo,
        open_mask: &BitSet,
        open_menu: &[CategoryPath],
        path: CategoryPath,
    ) -> bool {
        if open_mask.contains(path) || cats.is_root(path) || Self::is_path_open_menu(open_menu, path) {
            return true
        }
        let direct_parent_open = cats
            .parent_of(path)
            .map(|p| open_mask.contains(p) || Self::is_path_open_menu(open_menu, p));
        direct_parent_open.unwrap_or(true)
    }
    pub fn iter_whitelisted<'a>(
        &'a self,
        pack: &'a PackElementState,
    ) -> impl Iterator<Item = CategoryPath> + 'a {
        let category_info = pack.info.info.as_ref().map(|i| &*i.categories);
        self.filter_state.iter_categories(category_info)
    }
    pub fn category_is_whitelisted(&self, _pack: &PackElementState, path: CategoryPath) -> bool {
        self.filter_state.visible_category(path)
    }
    pub(crate) fn category_is_loaded(&self, pack: &PackElementState, path: CategoryPath) -> Option<bool> {
        pack.map_info
            .as_ref()
            .map(|map_info| map_info.info.category_index(path).is_some())
    }

    pub fn update_open(&mut self, path: CategoryPath, open: Option<bool>) {
        let open = match open {
            open @ Some(..) => open,
            None => match self.open_mask.remove_at(path) {
                Some(true) => None,
                _ => Some(true),
            },
        };
        if let Some(open) = open {
            self.open_mask.insert_at_if(path, open);
            if open && self.filter_state.is_active() {}
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CategoryInfo {
    /// TODO: probably not yet used?
    pub id: Option<CategoryId>,
    pub display_name: Option<Arc<str>>,
    pub tooltip: PackTooltip,
    pub interaction: Option<Arc<InteractionAttributes>>,
    pub visibility: VisibilityFlags,
}
impl CategoryInfo {
    pub const EMPTY: Self = Self {
        id: None,
        display_name: None,
        tooltip: PackTooltip::EMPTY,
        interaction: None,
        visibility: VisibilityFlags::empty(),
    };

    pub fn from_pack_category(category: &Category) -> Self {
        Self {
            id: Some(category.full_id.clone()),
            display_name: category.display_name.clone(),
            tooltip: PackTooltip::from_attrs(&category.marker_attributes),
            interaction: category.marker_attributes.interaction.clone(),
            visibility: VisibilityFlags::from_pack_category(category),
        }
    }
    pub fn from_pack_root(root: &PackRoot) -> Self {
        Self {
            id: Some(root.id.clone()),
            display_name: root.display_name.clone(),
            visibility: VisibilityFlags::from_pack_root(root),
            tooltip: PackTooltip::EMPTY,
            interaction: None,
        }
    }

    pub(super) fn is_root(&self) -> bool {
        self.id.as_ref().map(|id| id.id_is_root()).unwrap_or(false)
    }
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_ref().map(|n| &n[..])
    }
    pub fn tooltip(&self) -> Option<PackTooltipRef<'_>> {
        self.tooltip.get().map(PackTooltip::borrowed)
    }
    pub fn ui_id(&self, path: CategoryPath) -> *const () {
        self.id
            .as_ref()
            .map(|id| id.as_str().as_ptr() as *const ())
            .unwrap_or(path.path as usize as *const ())
    }

    pub fn copyable(&self) -> Option<(&str, Option<&str>)> {
        let interaction = self.interaction.as_ref()?;
        let copy_value = interaction
            .copy_value
            .as_ref()
            .map(|c| &c[..])
            .and_then(str_opt_ref)?;
        let copy_message = interaction
            .copy_message
            .as_ref()
            .map(|c| &c[..])
            .and_then(str_opt_ref);
        Some((copy_value, copy_message))
    }
}
