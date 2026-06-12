use {
    super::{
        poi::{ActivePoi, PoiCommonRenderData},
        trail::{ActiveTrail, TrailParams},
    },
    crate::{
        controller::pathing::{ExternalFilterState, FestivalFixup, PathingController, PathingEvent},
        exports::runtime as rt,
        fl,
        render::{
            element::prelude::*,
            machine::{RenderMachine, RenderPosition},
            pathing_window::PathingSearchState,
        },
        settings::state::ui::pathing::PathingFilterFlags as PathingFilterState,
        space::{
            dx11::{InstanceBufferData, RenderBackend},
            pack::Pack,
            render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder},
            resources::Texture,
            DrawSpace,
        },
        with_i18n,
    },
    anyhow::{anyhow, Context},
    bitvec::vec::BitVec,
    glamour::Box3,
    indexmap::IndexMap,
    std::{
        cell::LazyCell,
        collections::{BTreeMap, BTreeSet, HashSet},
        mem,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
            Mutex,
        },
    },
    taimi_d3d::dx11::{buffer::BufferOf, prelude::*},
    taimi_meta::ui::{LocalContext, MapContext},
    taimi_pack::{
        attributes::{Festival, FilterAttributes, MarkerAttributes},
        category::CategoryId,
        loader::PackLoaderContext,
        Category,
        Poi,
    },
    uuid::Uuid,
};
#[cfg(feature = "paths-lua")]
use {
    crate::controller::script::{PackPlugShared, ScriptMessage},
    std::borrow::Cow,
    taimi_pack::{
        attributes::{
            cell::{pack_attr, GetAttrDyn, PackKeyId, PackValueCell, SetAttrDyn},
            keys::{self, GetAttr},
        },
        script::pathing::imp::{
            MarkerLoc,
            MarkerOverrides,
            MarkerOverridesAttrs,
            MarkerType,
            PackOverrides,
            PackRootCategories,
        },
    },
};

#[derive(Debug)]
pub enum UnloadedReason {
    #[cfg(todo = "unused")]
    Disabled,
    UnknownFormat,
    LoadingFailed(String),
}

pub type LoaderBox = Box<dyn PackLoaderContext + Send + 'static>;
pub type SharedLoader = Arc<Mutex<LoaderBox>>;

pub struct ActivePack {
    pub pack: Arc<Pack>,
    pub loader: SharedLoader,

    // Actively loaded data.
    pub enabled_categories: BitVec,
    pub user_category_state: BitVec,
    pub active_trails: IndexMap<Uuid, ActiveTrail>,
    pub active_pois: IndexMap<Uuid, ActivePoi>,
    // UI and filter state
    pub available_categories: BitVec,
    pub copyable_categories: BTreeSet<usize>,
    pub copyable_pois: BTreeSet<usize>,

    // Internal rendering data.
    texture_list: IndexMap<String, Option<Arc<Texture>>>,
    loaded_textures: BitVec,
    unused_textures: BitVec,
    dirty_trails: BitVec,
    dirty_pois: BitVec,
    render_list_bookmark: Option<usize>,
    render_poi_bookmark: usize,
    poi_bookmark: usize,

    #[cfg(feature = "paths-lua")]
    pub(crate) script_data: Option<Arc<PackPlugShared>>,
    #[cfg(feature = "paths-lua")]
    pub script_capable: bool,
}

impl ActivePack {
    pub fn new(pack: Arc<Pack>, loader: SharedLoader) -> Self {
        let enabled_categories: BitVec = pack
            .categories
            .all_categories
            .values()
            .map(|category| category.default_toggle())
            .collect();

        ActivePack {
            loader,
            pack,
            user_category_state: enabled_categories.clone(),
            enabled_categories,
            active_pois: Default::default(),
            active_trails: Default::default(),
            texture_list: Default::default(),
            available_categories: Default::default(),
            copyable_categories: Default::default(),
            copyable_pois: Default::default(),
            loaded_textures: Default::default(),
            unused_textures: Default::default(),
            dirty_pois: Default::default(),
            dirty_trails: Default::default(),
            render_list_bookmark: Default::default(),
            render_poi_bookmark: Default::default(),
            poi_bookmark: Default::default(),
            #[cfg(feature = "paths-lua")]
            script_data: Default::default(),
            #[cfg(feature = "paths-lua")]
            script_capable: false,
        }
    }

    #[cfg(todo = "unused")]
    pub fn load(loader: impl PackLoaderContext + Send + 'static) -> anyhow::Result<ActivePack> {
        let mut loader = Box::new(loader);
        let pack = Pack::load(&mut *loader)?;
        Ok(Self::new(Arc::new(pack), loader))
    }

    pub fn get_copyable_pois(&self) -> Vec<Poi> {
        let mut current_pois = Vec::new();
        for (_, poi) in &self.active_pois {
            if !poi.filtered {
                let actual_poi = &self.pack.pois[poi.poi_idx];
                let copyable = actual_poi
                    .attributes
                    .interaction
                    .as_ref()
                    .and_then(|i| i.copy_value.as_ref());
                if copyable.is_some() {
                    let actual_poi = actual_poi.clone();
                    current_pois.push(actual_poi);
                }
            }
        }
        current_pois
    }

    pub fn draw_categories<'ui, U>(
        &mut self,
        ui: &mut U,
        filter_state: PathingFilterState,
        open_items: &mut HashSet<CategoryId>,
        recompute: &mut bool,
        search_state: &PathingSearchState,
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let map_filter = match filter_state.contains(PathingFilterState::CurrentMap) {
            true => {
                if self.available_categories.is_empty() {
                    self.update_available_categories();
                }
                Some(&self.available_categories)
            },
            false => None,
        };
        let root = &self.pack.categories.root_categories;
        let is_root = true;
        let all_categories = &self.pack.categories.all_categories;
        let enabled_categories = &mut self.user_category_state;
        for cat_name in root.iter() {
            Self::draw_category(
                ui,
                &all_categories[cat_name],
                all_categories,
                enabled_categories,
                filter_state,
                open_items,
                is_root,
                recompute,
                search_state,
                map_filter,
                (&self.copyable_categories, &self.copyable_pois, &self.pack.pois),
            );
        }
    }

    /// All categories relevant to the current map
    pub fn update_available_categories(&mut self) {
        let category_count = self.pack.categories.all_categories.len();
        let available = &mut self.available_categories;
        available.clear();
        available.reserve(category_count);
        available.set_uninitialized(false);
        unsafe {
            available.set_len(category_count);
        }
        for trail in self.active_trails.values() {
            available.set(trail.category_idx, true);
        }
        for poi in self.active_pois.values() {
            available.set(poi.category_idx, true);
        }
        let leaves = available.clone();
        'leafies: for leaf in leaves.iter_ones() {
            // a real tree would probably make this more sane,
            // but it's run once per map so who cares really...
            let Some((_, category)) = self.pack.categories.all_categories.get_index(leaf) else {
                continue 'leafies
            };
            let mut id = category.full_id.as_id();
            'parents: while let Some(parent) = id.parent() {
                id = parent;
                if let Some(parent_idx) = self.pack.categories.all_categories.get_index_of(parent) {
                    if available[parent_idx] {
                        // we've already been here before
                        break 'parents
                    }
                    available.set(parent_idx, true)
                }
            }
        }
    }

    pub fn recompute_enabled_category(
        category: &Category,
        all_categories: &IndexMap<CategoryId, Category>,
        enabled_categories: &mut BitVec,
        user_category_state: &BitVec,
        parent: bool,
    ) {
        if let Some(idx) = all_categories.get_index_of(&category.full_id) {
            if let Some(cur) = user_category_state.get(idx) {
                let res = parent && *cur;
                if let Some(mut cat) = enabled_categories.get_mut(idx) {
                    *cat = res;
                }
                for global in category.sub_categories.iter() {
                    let Some(child) = all_categories.get(global) else { continue };
                    Self::recompute_enabled_category(
                        child,
                        all_categories,
                        enabled_categories,
                        user_category_state,
                        res,
                    );
                }
            }
        }
    }

    pub fn draw_category<'ui, U>(
        ui: &mut U,
        category: &Category,
        all_categories: &IndexMap<CategoryId, Category>,
        state: &mut BitVec,
        filter_state: PathingFilterState,
        open_items: &mut HashSet<CategoryId>,
        is_root: bool,
        recompute: &mut bool,
        search_state: &PathingSearchState,
        category_filter: Option<&BitVec>,
        copyable: (&BTreeSet<usize>, &BTreeSet<usize>, &[Poi]),
    ) where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let _push_token = ui.push_id(category.full_id.as_str());
        if category.is_hidden() {
            return;
        }
        let mut display = true;
        let category_idx = all_categories.get_index_of(&category.full_id);
        if let Some(idx) = category_idx {
            if let Some(substate) = state.get(idx) {
                let enabled_filter = *substate && filter_state.contains(PathingFilterState::Enabled);
                let disabled_filter = !*substate && filter_state.contains(PathingFilterState::Disabled);
                let is_root_filter = is_root && filter_state.contains(PathingFilterState::IgnoreRoot);
                let is_leaf = category.sub_categories.is_empty();
                let is_branch = !is_leaf;
                let is_leaf_filter = is_leaf && filter_state.contains(PathingFilterState::IgnoreLeaves);
                let is_branch_filter =
                    is_branch && filter_state.contains(PathingFilterState::IgnoreBranches);
                let search_filter = search_state.matches_id(category.full_id.as_str());
                let category_filter = category_filter
                    .and_then(|f| f.get(idx).map(|b| *b))
                    .unwrap_or(true);
                let filter =
                    enabled_filter | disabled_filter | is_root_filter | is_leaf_filter | is_branch_filter;
                display = search_filter && category_filter && filter;
            }
        }
        if display {
            let copy_value = category
                .marker_attributes
                .interaction
                .as_ref()
                .and_then(|i| i.copy_value.as_ref());
            let is_copyable = match &copy_value {
                Some(value) if category.sub_categories.is_empty() && !category.is_separator() =>
                    Some(value),
                _ => None,
            };
            if let Some(..) = is_copyable {
                ui.indent();
                if ui.small_button(fl!("copy-arg", arg = (category.display_name()))) {
                    Self::copy_copyable(ui, &category.marker_attributes);
                }
                if ui.is_item_hovered() {
                    Self::draw_tooltip(ui, category.display_name(), |ui| {
                        Self::draw_tooltip_category(ui, category);
                        Self::draw_tooltip_copyable(
                            ui,
                            &category.marker_attributes,
                            category.get_display_name(),
                        );
                    });
                }
                ui.unindent();
                ui.table_next_column();
            } else {
                let mut state_checkbox = None;
                if !category.is_separator() {
                    if let Some(idx) = all_categories.get_index_of(&category.full_id) {
                        if state.get(idx).is_some() {
                            let (state, recompute) = (&mut *state, &mut *recompute);
                            state_checkbox = Some(move |ui: &mut U| {
                                if let Some(mut substate) = state.get_mut(idx) {
                                    if ui.checkbox("", &mut substate) {
                                        *recompute = true;
                                        PathingEvent::PathingStateUpdate(
                                            category.full_id.clone(),
                                            *substate,
                                        )
                                        .try_send();
                                    }
                                }
                            });
                        }
                    }
                }
                let has_state_checkbox = !is_root && state_checkbox.is_some();
                let mut checkbox_gap = None;
                if !is_root {
                    if let Some(mut checkbox) = state_checkbox.take() {
                        ui.unindent();
                        checkbox_gap = Some(ui.push_style_item_spacing(ImVec2::ZERO));
                        #[cfg(todo = "unnecessary")]
                        let _inner_gap = ui.push_style_var(StyleVar::ItemInnerSpacing([0.0, 0.0]));
                        checkbox(ui);
                        ui.same_line();
                    }
                }

                let (copyable_categories, copyable_pois, pois) = copyable;
                let has_copyable_pois = category_idx
                    .map(|idx| copyable_categories.contains(&idx))
                    .unwrap_or(false);

                let flag_leaf = category.is_separator();
                let flag_bullet = !flag_leaf && category.sub_categories.is_empty();
                let flag_framed = !flag_leaf && !flag_bullet;
                let opened =
                    flag_framed.then(|| ImCondition::always(open_items.contains(&category.full_id)));
                let flag_span_avail = (category.is_separator() || category.sub_categories.is_empty())
                    && copy_value.is_none()
                    && !has_copyable_pois;
                let flags = match ui.imgui_version_num() {
                    #[cfg(taimi_imgui = "180")]
                    Some(im180::VERSION_NUM) => {
                        let mut flags = im180::sys::ImGuiStyleVar_FramePadding
                            | im180::sys::ImGuiTreeNodeFlags_NoTreePushOnOpen;
                        if state_checkbox.is_some() {
                            flags |= im180::sys::ImGuiTreeNodeFlags_AllowItemOverlap;
                        }
                        if flag_leaf {
                            flags |= im180::sys::ImGuiTreeNodeFlags_Leaf;
                        }
                        if flag_bullet {
                            flags |= im180::sys::ImGuiTreeNodeFlags_Bullet;
                        }
                        if flag_framed {
                            flags |= im180::sys::ImGuiTreeNodeFlags_Framed;
                        }
                        if flag_span_avail {
                            flags |= im180::sys::ImGuiTreeNodeFlags_SpanAvailWidth;
                        }
                        imw::DynArgsTreeNode::new(Some(flags))
                    },
                    #[cfg(taimi_imgui = "192")]
                    Some(im192::VERSION_NUM) => {
                        let mut flags = im192::sys::ImGuiStyleVar_FramePadding
                            | im192::sys::ImGuiTreeNodeFlags_NoTreePushOnOpen;
                        if state_checkbox.is_some() {
                            flags |= im192::sys::ImGuiTreeNodeFlags_AllowOverlap;
                        }
                        if flag_leaf {
                            flags |= im192::sys::ImGuiTreeNodeFlags_Leaf;
                        }
                        if flag_bullet {
                            flags |= im192::sys::ImGuiTreeNodeFlags_Bullet;
                        }
                        if flag_framed {
                            flags |= im192::sys::ImGuiTreeNodeFlags_Framed;
                        }
                        if flag_span_avail {
                            flags |= im192::sys::ImGuiTreeNodeFlags_SpanAvailWidth;
                        }
                        imw::DynArgsTreeNode::new(Some(flags))
                    },
                    _ => Default::default(),
                };
                let tree_token =
                    ui.begin_tree_node(opened, category.full_id.as_str(), category.display_name(), flags);
                drop(checkbox_gap);
                if ui.is_item_hovered() && Self::category_has_tooltip(category) {
                    Self::draw_tooltip(ui, category.display_name(), |ui| {
                        Self::draw_tooltip_category(ui, category);
                    });
                }
                if let Some(mut checkbox) = state_checkbox.take() {
                    ui.same_line();
                    ui.dummy([4.0, 0.0]);
                    ui.same_line();
                    checkbox(ui);
                } else if has_state_checkbox {
                    ui.indent();
                }
                if copy_value.is_some() {
                    ui.same_line();
                    if with_i18n!("copy", |copy| ui.small_button(copy)) {
                        Self::copy_copyable(ui, &category.marker_attributes);
                    }
                    if ui.is_item_hovered() {
                        Self::draw_tooltip(ui, category.display_name(), |ui| {
                            Self::draw_tooltip_copyable(
                                ui,
                                &category.marker_attributes,
                                category.get_display_name(),
                            );
                        });
                    }
                }
                if has_copyable_pois {
                    // TODO: revisit or remove once trigger radius and interaction is working
                    let pois = copyable_pois
                        .iter()
                        .filter_map(|&poi_idx| pois.get(poi_idx))
                        //.filter(|poi| poi.category_idx == idx);
                        .filter(|poi| category.full_id == poi.category);
                    for (i, copyable) in pois.enumerate() {
                        if i % 4 != 3 {
                            ui.same_line();
                        }
                        let copied = match &copyable.attributes.tip_name {
                            Some(name) => ui.small_button(fl!("copy-arg", arg = (&name[..]))),
                            None => with_i18n!("copy", |copy| ui.small_button(copy)),
                        };
                        if copied {
                            Self::copy_copyable(ui, &copyable.attributes);
                        }
                        if ui.is_item_hovered() {
                            let template = copyable
                                .attributes
                                .tip_name
                                .as_ref()
                                .map(|n| &n[..])
                                .unwrap_or("Generic Copyable Marker Name");
                            Self::draw_tooltip(ui, template, |ui| {
                                Self::draw_tooltip_poi(ui, &copyable.attributes);
                                Self::draw_tooltip_copyable(ui, &copyable.attributes, None);
                            });
                        }
                    }
                }
                let mut internal_closure = |ui: &mut U| {
                    if !open_items.contains(&category.full_id)
                        && !category.is_separator()
                        && !category.sub_categories.is_empty()
                    {
                        open_items.insert(category.full_id.clone());
                    }
                    if !category.sub_categories.is_empty() {
                        ui.indent(); //_by(1.0);
                    }
                    for global in category.sub_categories.iter() {
                        Self::draw_category(
                            ui,
                            &all_categories[global],
                            all_categories,
                            state,
                            filter_state,
                            open_items,
                            false,
                            recompute,
                            search_state,
                            category_filter,
                            copyable,
                        );
                    }
                    if !category.sub_categories.is_empty() {
                        ui.unindent(); //_by(1.0);
                    }
                };
                ui.table_next_column();
                if let Some(token) = tree_token {
                    internal_closure(ui);
                    token.end();
                } else {
                    if open_items.contains(&category.full_id) {
                        open_items.remove(&category.full_id);
                    }
                }
            }
        }
    }

    pub(crate) fn copy_copyable<'ui, U>(ui: &mut U, attributes: &MarkerAttributes)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let interaction = attributes.interaction();
        let Some(copy_value) = &interaction.copy_value else { return };
        ui.set_clipboard_text(&copy_value[..]);
        if let Some(copy_message) = &interaction.copy_message {
            let _ = rt::send_alert(ui, &copy_message[..]);
        }
    }

    pub(crate) fn draw_tooltip_category<'ui, U>(ui: &mut U, category: &Category)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let desc = category
            .marker_attributes
            .tip_description
            .as_deref()
            .and_then(taimi_hoard::str_opt_ref);
        let title = match &category.marker_attributes.tip_name {
            Some(title) if !title.is_empty() && !category.display_name().starts_with(&title[..]) =>
                Some(&title[..]),
            _ => None,
        };

        if let Some(title) = title {
            let _title_font = NexusLinkFont::Big.push_font(ui);
            ui.text(title);
        }

        if let Some(tip) = desc {
            ui.text_wrapped(tip);
        }
    }

    fn draw_tooltip_poi<'ui, U>(ui: &mut U, attributes: &MarkerAttributes)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let desc = attributes
            .tip_description
            .as_deref()
            .and_then(taimi_hoard::str_opt_ref);

        if let Some(title) = &attributes.tip_name {
            let _title_font = NexusLinkFont::Big.push_font(ui);
            ui.text(&title[..]);
        }
        if let Some(desc) = desc {
            ui.text_wrapped(&desc[..]);
        }
    }

    pub(crate) fn category_has_tooltip(category: &Category) -> bool {
        match &category.marker_attributes.tip_description {
            Some(desc) if !desc.is_empty() => return true,
            _ => (),
        }
        match &category.marker_attributes.tip_name {
            Some(title) if !title.is_empty() && !category.display_name().starts_with(&title[..]) =>
                return true,
            _ => (),
        }

        false
    }

    /// since these aren't intended to be displayed, there's no canon name to use...
    /// if it looks like more than just a location link, we'll try to preview it
    fn copyable_value_has_message(attributes: &MarkerAttributes) -> bool {
        let copy_value = attributes
            .interaction
            .as_ref()
            .and_then(|i| i.copy_value.as_ref());
        let Some(copy_value) = copy_value else { return false };
        if !copy_value[..].starts_with('[') || !copy_value.ends_with(']') {
            return true
        }
        false
    }

    pub(crate) fn draw_tooltip<'ui, U, F>(ui: &mut U, title_template: &str, f: F)
    where
        U: ?Sized + ImDrawWindow<'ui>,
        F: FnOnce(&mut U),
    {
        let _id = ui.push_id("category_tooltip");
        let ImSize2 { width: minwidth, height: lineheight } = ui.calc_text_size(title_template);
        ui.window_prepare_size(imw::Window::prepare_height(lineheight * 1.5), ImCondition::Always);
        let size_token = ui.push_window_size_min([minwidth, lineheight]);
        let tooltip = ui.begin_tooltip();
        size_token.end();
        if let Some(_tooltip) = tooltip {
            {
                let _padding = ui.push_style_item_spacing(ImVec2::splat(f32::EPSILON));
                ui.dummy([minwidth, f32::EPSILON]);
            }
            f(ui)
        }
    }

    fn draw_tooltip_copyable<'ui, U>(ui: &mut U, attributes: &MarkerAttributes, display_name: Option<&str>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let interaction = attributes.interaction();
        let copy_message = interaction.copy_message.as_ref().map(|m| &m[..]);
        match &interaction.copy_value {
            Some(copy_value)
                if (display_name.is_none() || copy_message.is_none())
                    && Self::copyable_value_has_message(attributes) =>
                ui.text_wrapped(im_fmt!("\"{copy_value}\"")),
            _ => (),
        }
        if let Some(copy_message) = copy_message {
            ui.text_wrapped(copy_message);
        }
    }

    pub fn disable_paths(&mut self, paths: &HashSet<String>, external: Option<&ExternalFilterState>) {
        for path in paths {
            if let Some(idx) = self.pack.categories.all_categories.get_index_of(&path[..]) {
                if let Some(mut state) = self.user_category_state.get_mut(idx) {
                    *state = false;
                }
            }
        }
        self.recompute_enabled(external);
    }

    pub fn recompute_enabled(&mut self, external: Option<&ExternalFilterState>) {
        let all = &self.pack.categories.all_categories;
        for root_category_id in &self.pack.categories.root_categories {
            if let Some(root) = all.get(root_category_id) {
                Self::recompute_enabled_category(
                    root,
                    all,
                    &mut self.enabled_categories,
                    &self.user_category_state,
                    true,
                );
            }
        }
        if let Some((festivals, ..)) = external {
            for (i, (_, category)) in self.pack.categories.all_categories.iter().enumerate() {
                let f = category
                    .marker_attributes
                    .filters
                    .as_ref()
                    .and_then(|f| f.festivals);
                if f.map(|f| !f.is_empty() && !f.intersects(*festivals))
                    .unwrap_or(false)
                {
                    self.enabled_categories.set(i, false)
                }
            }
        }
        // in response to update(...), moving update_filters down here where it should actually be
        // effective to save on useless loops
        self.update_filters(external);
    }

    pub fn update(
        &mut self,
        render_list: &mut RenderList,
        poi_common: &mut PoiCommonRenderData,
        machine: &RenderMachine,
        context: &Dx11Context,
    ) {
        // why are we doing 4 for loops over all trails and pois currently active every frame?
        // ::update(...) is a no-op, filters should NOT be changing every frame and even then
        // should be a matter of when recompute_enabled(); is called :s
        /*self.update_filters();

        for trail_idx in 0..self.active_trails.len() {
            ActiveTrail::update(self, trail_idx);
        }
        for poi_idx in 0..self.active_pois.len() {
            ActivePoi::update(self, poi_idx);
        }*/

        // TODO: Scripting engine update.

        #[cfg(todo = "unnecessary")]
        let dirty_trails = self.dirty_trails.any().then(|| {
            let dirty = self.dirty_trails.clone();
            self.dirty_trails.fill(false);
            dirty
        });
        #[cfg(todo = "unnecessary")]
        let dirty_trails = dirty_trails.as_ref().into_iter().flat_map(|p| p.iter_ones());
        let dirty_trails = core::iter::empty::<usize>();
        for trail_idx in dirty_trails {
            let trail = &self.active_trails[trail_idx];
            for (i_section, bounds) in trail.section_bounds.iter().enumerate() {
                render_list.update_bounds(trail.render_bookmark + i_section, *bounds);
            }
        }
        let dirty_pois = self.dirty_pois.any().then(|| {
            let dirty = self.dirty_pois.clone();
            self.dirty_pois.fill(false);
            dirty
        });
        let dirty_pois = dirty_pois.as_ref().into_iter().flat_map(|p| p.iter_ones());
        for poi_idx in dirty_pois {
            #[cfg(feature = "paths-lua")]
            let (bvh, ibd, ibd_map) = ActivePoi::update(self, poi_idx);
            let poi = LazyCell::new(|| unsafe { self.active_pois.get_index(poi_idx).unwrap_unchecked().1 });
            #[cfg(feature = "paths-lua")]
            {
                let ib_update = (ibd && self.render_poi_bookmark > 0)
                    .then_some(poi_common.world_ib.as_ref())
                    .flatten();
                let ib_update_map = (ibd_map && self.render_poi_bookmark > 0)
                    .then_some(poi_common.map_ib.as_ref())
                    .flatten();
                if let Some(ib) = ib_update {
                    unsafe {
                        ib.update_element_at(
                            context,
                            &poi.instance_data(),
                            self.render_poi_bookmark + poi_idx,
                            0,
                        );
                    }
                }
                if let Some(ib) = ib_update_map {
                    unsafe {
                        ib.update_element_at(
                            context,
                            &poi.instance_data_map(machine),
                            self.render_poi_bookmark + poi_idx,
                            0,
                        );
                    }
                }
                if !bvh {
                    continue
                }
            }
            render_list.update_bounds(poi.render_bookmark as usize, poi.bounds);
        }
    }

    #[cfg(feature = "paths-lua")]
    pub fn has_scripts(&self) -> bool {
        self.script_data.is_some()
    }

    /// TODO: go and apply attrs as if a late map load had happened?
    #[cfg(feature = "paths-lua")]
    fn script_start(
        &mut self,
        _device: &Dx11Device,
        _machine: &mut RenderMachine,
        shared: Arc<PackPlugShared>,
    ) {
        let _overrides = self.script_data.insert(shared);
    }
    #[cfg(feature = "paths-lua")]
    fn script_create_trail(
        &mut self,
        current_map: u32,
        device: &Dx11Device,
        bookmark: usize,
        _machine: &RenderMachine,
        trail_idx: usize,
    ) -> bool {
        let Some(po) = self.script_data.clone() else {
            log::warn!("received create from untracked script");
            return false
        };
        let path = (MarkerType::Trail, trail_idx);
        let po = PackOverrides::shared_read(&po.overrides);
        let Some(o) = po.overrides.get(&path) else {
            log::warn!("dynamic attrs missing?");
            return false
        };
        let o = MarkerOverrides::shared_read(o);

        let on_map = o
            .get::<keys::GameMap>()
            .and_then(|map| map.map(|map| map.get().0 == current_map))
            .unwrap_or(false);
        if !on_map {
            return false
        }

        let cat_idx = match o.get::<keys::CategoryRef>() {
            Some(Some(cat)) => self.pack.categories.all_categories.get_index_of(&cat[..]),
            _ => None,
        };
        let cat_idx = cat_idx
            .or_else(|| {
                PackRootCategories::from_ref(&self.pack)
                    .primary_root()
                    .and_then(|r| {
                        self.pack
                            .categories
                            .all_categories
                            .get_index_of(r.full_id.as_id())
                    })
            })
            .context("dynamic trail missing category");
        let Some(cat_idx) = rt::log::warn_ok(cat_idx) else { return false };
        let new_trail = self.script_build_trail(device, trail_idx, cat_idx, &o, bookmark, None);
        let Some((_activei, complete)) = rt::log::warn_ok(new_trail) else {
            return false
        };

        self.dirty_trails.push(!complete);
        true
    }
    #[cfg(feature = "paths-lua")]
    fn script_build_trail(
        &mut self,
        device: &Dx11Device,
        trail_idx: usize,
        cat_idx: usize,
        o: &MarkerOverrides,
        bookmark: usize,
        trail_params: Option<&TrailParams>,
    ) -> anyhow::Result<(usize, bool)> {
        let id = match o.get::<keys::Guid>() {
            Some(Some(guid)) if !self.active_trails.contains_key(guid.get()) => *guid.get(),
            _ => Uuid::new_v4().into(),
        };

        let is_complete = GetAttr::<keys::TrailDataFile>::has_attr(&o.attrs)
            && GetAttr::<keys::TextureFile>::has_attr(&o.attrs);
        let mut new_trail = match (is_complete, trail_params) {
            (true, Some(params)) =>
                ActiveTrail::build(self, None, &o.attrs, trail_idx, cat_idx, params, device, bookmark),
            _ => ActiveTrail::new_empty(self, &o.attrs, trail_idx, cat_idx, device, bookmark),
        }
        .context("preparing dynamic trail")?;
        if self.enabled_categories.get(cat_idx).map(|v| *v) == Some(false) {
            new_trail.filtered = true;
        }

        let active_trail_idx = self.active_trails.len();
        let _replaced = self.active_trails.insert(id.into(), new_trail);
        #[cfg(taimi_debug)]
        assert!(_replaced.is_none());
        Ok((active_trail_idx, is_complete))
    }
    #[cfg(feature = "paths-lua")]
    fn script_create_poi(
        &mut self,
        current_map: u32,
        device: &Dx11Device,
        bookmark: usize,
        _machine: &RenderMachine,
        poi_idx: usize,
    ) -> bool {
        let Some(po) = self.script_data.clone() else {
            log::warn!("received create from untracked script");
            return false
        };
        let path = (MarkerType::Poi, poi_idx);
        let po = PackOverrides::shared_read(&po.overrides);
        let Some(o) = po.overrides.get(&path) else {
            log::warn!("dynamic attrs missing?");
            return false
        };
        let o = MarkerOverrides::shared_read(o);

        let on_map = o
            .get::<keys::GameMap>()
            .and_then(|map| map.map(|map| map.get().0 == current_map))
            .unwrap_or(false);
        if !on_map {
            return false
        }

        let cat_idx = match o.get::<keys::CategoryRef>() {
            Some(Some(cat)) => self.pack.categories.all_categories.get_index_of(&cat[..]),
            _ => None,
        };
        let cat_idx = cat_idx
            .or_else(|| {
                PackRootCategories::from_ref(&self.pack)
                    .primary_root()
                    .and_then(|r| {
                        self.pack
                            .categories
                            .all_categories
                            .get_index_of(r.full_id.as_id())
                    })
            })
            .context("dynamic poi missing category");
        let Some(cat_idx) = rt::log::warn_ok(cat_idx) else { return false };

        let new_poi = self.script_build_poi(device, poi_idx, cat_idx, &o, bookmark);
        let Some((_activeidx, complete)) = rt::log::warn_ok(new_poi) else {
            return false
        };

        self.dirty_pois.push(!complete);
        true
    }
    #[cfg(feature = "paths-lua")]
    fn script_build_poi(
        &mut self,
        device: &Dx11Device,
        poi_idx: usize,
        cat_idx: usize,
        o: &MarkerOverrides,
        bookmark: usize,
    ) -> anyhow::Result<(usize, bool)> {
        let id = match o.get::<keys::Guid>() {
            Some(Some(guid)) if !self.active_pois.contains_key(guid.get()) => *guid.get(),
            _ => Uuid::new_v4().into(),
        };

        let is_complete = GetAttr::<keys::IconFile>::has_attr(&o.attrs);
        let mut new_poi = match is_complete {
            true => ActivePoi::build(self, &o.attrs, poi_idx, cat_idx, device, bookmark),
            _ => ActivePoi::new_empty(self, &o.attrs, poi_idx, cat_idx, device, bookmark),
        }
        .context("preparing dynamic poi")?;
        if self.enabled_categories.get(cat_idx).map(|v| *v) == Some(false) {
            new_poi.filtered = true;
        }

        let active_poi_idx = self.active_pois.len();
        let _replaced = self.active_pois.insert(id.into(), new_poi);
        #[cfg(taimi_debug)]
        assert!(_replaced.is_none());
        Ok((active_poi_idx, is_complete))
    }
    #[cfg(feature = "paths-lua")]
    fn script_update_trail(
        &mut self,
        device: &Dx11Device,
        _machine: &RenderMachine,
        trail_idx: usize,
        changed_attrs: &mut dyn Iterator<Item = PackKeyId>,
    ) {
        let mut trail = {
            let trails = &mut self.active_trails;
            LazyCell::new(move || trails.values_mut().find(|t| t.trail_idx == trail_idx))
        };
        let po = {
            let so = self.script_data.as_ref();
            LazyCell::new(move || so.map(|d| PackOverrides::shared_read(&d.overrides)))
        };
        let mo = {
            let path = (MarkerType::Trail, trail_idx);
            let po = &po;
            move || {
                po.as_ref().and_then(|o| {
                    (!o.is_masked(path))
                        .then_some(o.overrides.get(&path))
                        .flatten()
                        .map(MarkerOverrides::shared_read)
                })
            }
        };
        let overrides = std::cell::OnceCell::new();
        for key in changed_attrs {
            let interest = match overrides.get() {
                None if !ActiveTrail::holds_attr_dyn(key) => None,
                _ => overrides.get_or_init(mo).as_ref(),
            };
            let interest = if let Some(o) = interest {
                if let &mut Some(ref mut trail) = &mut *trail {
                    let value = match o.get_dyn(key) {
                        Some(Some(v)) => Some(Cow::Borrowed(v.get_dyn())),
                        Some(None) => None,
                        None => self
                            .pack
                            .trails
                            .get(trail_idx)
                            .and_then(|trail| trail.get_attr_dyn(key)),
                    }
                    .map(|v| v.into_owned().into_inner())
                    .unwrap_or_else(|| PackValueCell::new_empty(key));
                    trail.set_attr_dyn(value)
                } else {
                    false
                }
            } else {
                false
            };
            if let (Some(trail), false) = (&mut *trail, interest) {
                pack_attr! { match =id_is(key) {
                    = keys::TextureFile => if let Some(Some(tex)) = overrides.get_or_init(mo).as_ref().and_then(|o| o.get::<keys::IconFile>()) {
                        let h = Self::register_texture_with(
                            &mut self.texture_list,
                            &tex,
                            &mut self.loaded_textures,
                            &mut self.unused_textures,
                        );
                        let tex = Self::get_or_load_texture_with(
                            &mut self.texture_list,
                            h,
                            device,
                            &self.loader,
                            &mut self.loaded_textures,
                            &mut self.unused_textures,
                        ).cloned().with_context(|| format!("Loading trail texture {tex}"));
                        if let Some(tex) = rt::log::warn_ok(tex) {
                            trail.texture = tex;
                        }
                    },
                    = keys::TrailDataFile => {
                        log::debug!("TODO: dynamic trail data");
                    },
                    // TODO? = keys::GameMap => todo!(),
                } }
            }
        }
    }
    #[cfg(feature = "paths-lua")]
    fn script_update_poi(
        &mut self,
        device: &Dx11Device,
        _machine: &RenderMachine,
        poi_idx: usize,
        changed_attrs: &mut dyn Iterator<Item = PackKeyId>,
    ) {
        let active_poi_idx = self.active_pois.values().position(|poi| poi.poi_idx == poi_idx);
        let mut poi = {
            let pois = &mut self.active_pois;
            LazyCell::new(move || {
                active_poi_idx.map(|i| unsafe { pois.get_index_mut(i).unwrap_unchecked().1 })
            })
        };
        let po = {
            let so = self.script_data.as_ref();
            LazyCell::new(move || so.map(|d| PackOverrides::shared_read(&d.overrides)))
        };
        let mo = {
            let path = (MarkerType::Poi, poi_idx);
            let po = &po;
            move || {
                po.as_ref().and_then(|o| {
                    (!o.is_masked(path))
                        .then_some(o.overrides.get(&path))
                        .flatten()
                        .map(MarkerOverrides::shared_read)
                })
            }
        };
        let overrides = std::cell::OnceCell::new();
        for key in changed_attrs {
            let interest = match overrides.get() {
                None if !ActivePoi::holds_attr_dyn(key) => None,
                _ => overrides.get_or_init(mo).as_ref(),
            };
            let interest = if let Some(o) = interest {
                if let &mut Some(ref mut poi) = &mut *poi {
                    let value = match o.get_dyn(key) {
                        Some(Some(v)) => Some(Cow::Borrowed(v.get_dyn())),
                        Some(None) => None,
                        None => self.pack.pois.get(poi_idx).and_then(|poi| poi.get_attr_dyn(key)),
                    }
                    .map(|v| v.into_owned().into_inner())
                    .unwrap_or_else(|| PackValueCell::new_empty(key));
                    let changed = poi.set_attr_dyn(value);
                    if let (Some(i), true) = (active_poi_idx, changed && poi.is_dirty()) {
                        if let Some(mut d) = self.dirty_pois.get_mut(i) {
                            *d = true;
                        }
                    }
                    changed
                } else {
                    false
                }
            } else {
                false
            };
            if let (Some(poi), false) = (&mut *poi, interest) {
                pack_attr! { match =id_is(key) {
                    = keys::IconFile => if let Some(Some(tex)) = overrides.get_or_init(mo).as_ref().and_then(|o| o.get::<keys::IconFile>()) {
                        let h = Self::register_texture_with(
                            &mut self.texture_list,
                            &tex,
                            &mut self.loaded_textures,
                            &mut self.unused_textures,
                        );
                        let tex = Self::get_or_load_texture_with(
                            &mut self.texture_list,
                            h,
                            device,
                            &self.loader,
                            &mut self.loaded_textures,
                            &mut self.unused_textures,
                        ).cloned().with_context(|| format!("Loading poi texture {tex}"));
                        if let Some(tex) = rt::log::warn_ok(tex) {
                            poi.icon = tex;
                        }
                    },
                    // TODO? = keys::GameMap => todo!(),
                } }
            }
        }
    }

    pub fn register_texture(&mut self, asset: &str) -> PackTextureHandle {
        Self::register_texture_with(
            &mut self.texture_list,
            asset,
            &mut self.loaded_textures,
            &mut self.unused_textures,
        )
    }
    fn register_texture_with(
        texture_list: &mut IndexMap<String, Option<Arc<Texture>>>,
        asset: &str,
        loaded_textures: &mut BitVec,
        unused_textures: &mut BitVec,
    ) -> PackTextureHandle {
        if let Some(id) = texture_list.get_index_of(asset) {
            return PackTextureHandle(id);
        }

        loaded_textures.push(false);
        unused_textures.push(false);
        let idx = texture_list.insert_full(asset.to_string(), None).0;
        PackTextureHandle(idx)
    }

    pub fn with_shared_loader<F, R>(loader: &SharedLoader, f: F) -> R
    where
        F: FnOnce(&mut dyn PackLoaderContext) -> R,
    {
        let mut loader = loader.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut *loader)
    }
    pub fn with_loader<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut dyn PackLoaderContext) -> R,
    {
        Self::with_shared_loader(&self.loader, f)
    }

    pub fn get_or_load_texture<'t>(
        &'t mut self,
        handle: PackTextureHandle,
        device: &Dx11Device,
    ) -> anyhow::Result<&'t Arc<Texture>> {
        Self::get_or_load_texture_with(
            &mut self.texture_list,
            handle,
            device,
            &self.loader,
            &mut self.loaded_textures,
            &mut self.unused_textures,
        )
    }
    fn get_or_load_texture_with<'t>(
        texture_list: &'t mut IndexMap<String, Option<Arc<Texture>>>,
        handle: PackTextureHandle,
        device: &Dx11Device,
        loader: &SharedLoader,
        loaded_textures: &mut BitVec,
        unused_textures: &mut BitVec,
    ) -> anyhow::Result<&'t Arc<Texture>> {
        let PackTextureHandle(idx) = handle;
        let (asset, slot) = texture_list
            .get_index_mut(idx)
            .ok_or_else(|| anyhow!("Texture {} not in list at all", idx))?;

        let texture = match slot {
            slot_texture @ None => {
                let data = Self::with_shared_loader(&loader, |l| l.load_asset_dyn(asset))?;
                let image = image::ImageReader::new(data)
                    .with_guessed_format()
                    .map_err(anyhow::Error::from)
                    .and_then(|image| image.decode().map_err(Into::into))
                    .with_context(|| "decoding {asset}")?
                    .into_rgba8()
                    .into_flat_samples();

                let texture = Texture::load_rgba8_uncached(device, image)
                    .with_context(|| format!("loading {asset}"))?;
                let texture = Arc::new(texture);
                let texture = slot_texture.insert(texture);
                loaded_textures.set(idx, true);
                texture
            },
            Some(texture) => texture,
        };
        unused_textures.set(idx, false);
        Ok(texture)
    }

    fn prepare_new_map(
        &mut self,
        pack_idx: usize,
        map_id: i32,
        device: &Dx11Device,
        render_entities: &mut Vec<RenderEntity>,
        trail_params: &TrailParams,
    ) -> anyhow::Result<()> {
        self.clear();
        self.render_list_bookmark = Some(render_entities.len());

        let pack = self.pack.clone();

        #[cfg(feature = "paths-lua")]
        let script_data = self.script_data.clone();
        #[cfg(feature = "paths-lua")]
        let script_overrides = script_data
            .as_ref()
            .map(|d| PackOverrides::shared_read(&d.overrides));

        let trails = pack
            .trails
            .iter()
            .enumerate()
            .filter(|(_, t)| t.map_id == Some(map_id));
        for (i_trail, pack_trail, ..) in trails {
            let mut id = pack_trail.guid;
            if self.active_trails.contains_key(&id) {
                log::trace!(
                    "Pack {} contains a duplicate trail GUID `{id}`. \
                    Randomizing to ensure it may still be rendered.",
                    pack.name
                );
                while self.active_trails.contains_key(&id) {
                    id = Uuid::new_v4();
                }
            }

            #[cfg(feature = "paths-lua")]
            let trail_overrides = {
                let path = (MarkerType::Trail, i_trail);
                match script_overrides.as_ref() {
                    Some(o) if o.is_masked(path) => continue,
                    Some(o) => o.overrides.get(&path).map(MarkerOverrides::shared_read),
                    None => None,
                }
            };
            #[cfg(feature = "paths-lua")]
            let trail_attrs_o;
            let trail_attrs = match () {
                #[cfg(feature = "paths-lua")]
                _ => {
                    trail_attrs_o = trail_overrides
                        .as_ref()
                        .map(|o| MarkerOverridesAttrs::wrap_with_overrides(pack_trail, o))
                        .unwrap_or(MarkerOverridesAttrs::empty(pack_trail));
                    &trail_attrs_o
                },
                #[cfg(not(feature = "paths-lua"))]
                _ => pack_trail,
            };

            let cat_id = pack_trail.category.as_str();
            #[cfg(feature = "paths-lua")]
            let cat_id = trail_overrides
                .as_ref()
                .and_then(|o| o.get::<keys::CategoryRef>().map(|id| id.map(|id| &id.get()[..])))
                .or(Some(Some(cat_id)))
                .flatten()
                .unwrap_or("");
            let category_idx = pack.categories.all_categories.get_index_of(cat_id);
            #[cfg(feature = "paths-lua")]
            let category_idx = script_overrides
                .as_ref()
                .and_then(|o| o.cat_overrides.get(cat_id).copied())
                .or(category_idx);
            #[cfg(feature = "paths-lua")]
            let category_idx = category_idx.or_else(|| {
                PackRootCategories::from_ref(&self.pack)
                    .primary_root()
                    .and_then(|r| {
                        self.pack
                            .categories
                            .all_categories
                            .get_index_of(r.full_id.as_id())
                    })
            });
            let category_idx = category_idx.unwrap_or(0);

            let trail = ActiveTrail::build(
                self,
                Some(pack_trail),
                trail_attrs,
                i_trail,
                category_idx,
                trail_params,
                device,
                render_entities.len(),
            )
            .with_context(|| format!("Error loading trail {pack_trail}"));
            let trail = match trail {
                Ok(trail) => trail,
                Err(e) => {
                    log::warn!("{e:#}");
                    continue;
                },
            };

            let trail_idx = self.active_trails.len();
            for i_section in 0..trail.section_bounds.len() {
                let entity = RenderEntity {
                    bounds: trail.section_bounds[i_section],
                    position: {
                        let mut pos = trail.section_bounds[i_section].center();
                        pos.y += trail.y_offset;
                        pos
                    },
                    // TODO: just sort by y and reverse draw order if camera dir.y is negative? :p
                    // then only intersecting paths are an issue...
                    //draw_ordered: true,
                    draw_ordered: false,
                    render_id: Some(RenderId::TrailSection {
                        pack_idx,
                        trail_idx,
                        section: i_section,
                    }),
                };
                render_entities.push(entity);
            }

            self.active_trails.insert(id, trail);
            self.dirty_trails.push(false);
        }

        self.poi_bookmark = render_entities.len();

        for (i_poi, pack_poi) in pack.pois.iter().enumerate() {
            if pack_poi.map_id != map_id {
                continue;
            }
            let mut id = pack_poi.guid;
            if self.active_pois.contains_key(&id) {
                log::trace!(
                    "Pack {} contains a duplicate poi GUID `{id}`. \
                    Randomizing to ensure it may still be rendered.",
                    self.pack.name
                );
                while self.active_pois.contains_key(&id) {
                    id = Uuid::new_v4();
                }
            }

            #[cfg(feature = "paths-lua")]
            let poi_overrides = {
                let path = (MarkerType::Poi, i_poi);
                match script_overrides.as_ref() {
                    Some(o) if o.is_masked(path) => continue,
                    Some(o) => o.overrides.get(&path).map(MarkerOverrides::shared_read),
                    None => None,
                }
            };
            #[cfg(feature = "paths-lua")]
            let poi_attrs_o;
            let poi_attrs = match () {
                #[cfg(feature = "paths-lua")]
                _ => {
                    poi_attrs_o = poi_overrides
                        .as_ref()
                        .map(|o| MarkerOverridesAttrs::wrap_with_overrides(pack_poi, o))
                        .unwrap_or(MarkerOverridesAttrs::empty(pack_poi));
                    &poi_attrs_o
                },
                #[cfg(not(feature = "paths-lua"))]
                _ => pack_poi,
            };

            let cat_id = pack_poi.category.as_str();
            #[cfg(feature = "paths-lua")]
            let cat_id = poi_overrides
                .as_ref()
                .and_then(|o| o.get::<keys::CategoryRef>().map(|id| id.map(|id| &id.get()[..])))
                .or(Some(Some(cat_id)))
                .flatten()
                .unwrap_or("");
            let category_idx = pack.categories.all_categories.get_index_of(cat_id);
            #[cfg(feature = "paths-lua")]
            let category_idx = script_overrides
                .as_ref()
                .and_then(|o| o.cat_overrides.get(cat_id).copied())
                .or(category_idx);
            #[cfg(feature = "paths-lua")]
            let category_idx = category_idx.or_else(|| {
                PackRootCategories::from_ref(&self.pack)
                    .primary_root()
                    .and_then(|r| {
                        self.pack
                            .categories
                            .all_categories
                            .get_index_of(r.full_id.as_id())
                    })
            });
            let category_idx = category_idx.unwrap_or(0);

            let poi = ActivePoi::build(
                self,
                poi_attrs,
                i_poi,
                category_idx,
                device,
                render_entities.len(),
            )
            .with_context(|| format!("Error loading POI {pack_poi}"));
            let poi = match poi {
                Ok(poi) => poi,
                Err(e) => {
                    log::warn!("{e:#}");
                    continue;
                },
            };

            if pack_poi
                .attributes
                .interaction
                .as_ref()
                .and_then(|i| i.copy_value.as_ref())
                .is_some()
            {
                self.copyable_pois.insert(i_poi);
                self.copyable_categories.insert(category_idx);
            }

            let poi_idx = self.active_pois.len();
            let entity = RenderEntity {
                bounds: poi.bounds,
                position: poi.position,
                draw_ordered: true,
                render_id: Some(RenderId::Poi { pack_idx, poi_idx }),
            };
            render_entities.push(entity);
            self.active_pois.insert(id, poi);
            self.dirty_pois.push(false);
        }

        self.cleanup_textures();

        //self.recompute_enabled();
        #[cfg(feature = "paths-lua")]
        if let Some(overrides) = script_overrides {
            let dynamics = overrides
                .iter_dynamic_all()
                .map(|(loc, o)| (loc, o.clone()))
                .collect::<Vec<_>>();
            #[cfg(todo)]
            {
                // TODO: weh make cat_overrides an arc for cheap clones maybe?
                drop(overrides);
            }
            for (loc, o) in dynamics {
                let o = MarkerOverrides::shared_read(&o);
                let on_map = o
                    .get::<keys::GameMap>()
                    .flatten()
                    .map(|map| map.get().0 == map_id as u32)
                    .unwrap_or(false);
                let cat_idx = || {
                    o.get::<keys::CategoryRef>()
                        .flatten()
                        .and_then(|id| {
                            self.pack
                                .categories
                                .all_categories
                                .get_index_of(&id.get()[..])
                                .or_else(|| overrides.cat_overrides.get(&id.get()[..]).copied())
                        })
                        .or_else(|| {
                            PackRootCategories::from_ref(&self.pack)
                                .primary_root()
                                .and_then(|r| {
                                    self.pack
                                        .categories
                                        .all_categories
                                        .get_index_of(r.full_id.as_id())
                                })
                        })
                };
                match loc.0 {
                    MarkerType::Category =>
                        if self.user_category_state.len() < loc.1 {
                            self.user_category_state.resize(loc.1 + 1, false);
                            let default_toggle = o
                                .get::<keys::DefaultToggle>()
                                .flatten()
                                .map(|v| bool::from(*v.get()));
                            if default_toggle != Some(false) {
                                unsafe {
                                    self.user_category_state.set_unchecked(loc.1, true);
                                }
                            }
                        },
                    _ if !on_map => continue,
                    MarkerType::Poi => {
                        let cat_idx =
                            rt::log::debug_ok(cat_idx().context("no cat for dyn poi")).unwrap_or(0);
                        let res = self.script_build_poi(device, loc.1, cat_idx, &o, render_entities.len());
                        let Some((activei, complete)) = rt::log::warn_ok(res) else { continue };
                        self.dirty_pois.push(false);
                        let poi = unsafe { self.active_pois.get_index_mut(activei).unwrap_unchecked().1 };
                        poi.render_bookmark = render_entities.len() as u32;
                        let entity = RenderEntity {
                            bounds: match complete {
                                true => poi.bounds,
                                false => RenderList::BOUNDS_NONE,
                            },
                            position: poi.position,
                            draw_ordered: true,
                            render_id: Some(RenderId::Poi { pack_idx, poi_idx: activei }),
                        };
                        render_entities.push(entity);
                    },
                    MarkerType::Trail => {
                        let cat_idx =
                            rt::log::debug_ok(cat_idx().context("no cat for dyn trail")).unwrap_or(0);
                        let res = self.script_build_trail(
                            device,
                            loc.1,
                            cat_idx,
                            &o,
                            render_entities.len(),
                            Some(trail_params),
                        );
                        let Some((activei, _complete)) = rt::log::warn_ok(res) else { continue };
                        self.dirty_trails.push(false);
                        let trail = unsafe { self.active_trails.get_index(activei).unwrap_unchecked().1 };

                        // TODO: move this into a common fn...
                        for i_section in 0..trail.section_bounds.len() {
                            let entity = RenderEntity {
                                bounds: trail.section_bounds[i_section],
                                position: trail.section_bounds[i_section].center(),
                                draw_ordered: false,
                                render_id: Some(RenderId::TrailSection {
                                    pack_idx,
                                    trail_idx: activei,
                                    section: i_section,
                                }),
                            };
                            render_entities.push(entity);
                        }
                    },
                }
            }
        }

        Ok(())
    }

    fn is_filtered(filters: &FilterAttributes, external: &ExternalFilterState) -> bool {
        let (festivals, clears, achievements) = external;
        if let Some(id) = filters.achievement_id {
            let completed = filters
                .achievement_bit
                .and_then(|bit| achievements.is_bit_complete(id as _, bit as _))
                .unwrap_or_else(|| achievements.is_complete(id as _));
            if completed {
                return true
            }
        }
        if let Some(raids) = &filters.raids {
            let completed = !raids.is_empty() && raids.iter().all(|r| clears.contains(r));
            if completed {
                return true
            }
        }
        if let Some(f) = &filters.festivals {
            if !f.is_empty() && !f.intersects(*festivals) {
                return true
            }
        }
        false
    }
    fn update_filters(&mut self, external: Option<&ExternalFilterState>) {
        for (_i, trail) in &mut self.active_trails {
            let enabled = self.enabled_categories.get(trail.category_idx).map(|b| *b);
            if enabled.is_none() {
                log::error!(
                    "unknown category index {} for trail[{_i}] #{}",
                    trail.category_idx,
                    trail.trail_idx
                );
            }
            trail.filtered = !enabled.unwrap_or(true);
            if trail.filtered {
                continue
            }
            if let Some(external) = external {
                let filters = self
                    .pack
                    .trails
                    .get(trail.trail_idx)
                    .and_then(|trail| trail.attributes.filters.as_ref());
                let filtered = filters.map(|attrs| Self::is_filtered(attrs, external));
                if let Some(true) = filtered {
                    trail.filtered = true;
                }
            }
        }
        for (_i, poi) in &mut self.active_pois {
            let enabled = self.enabled_categories.get(poi.category_idx).map(|b| *b);
            if enabled.is_none() {
                log::error!(
                    "unknown category index {} for poi[{_i}] #{}",
                    poi.category_idx,
                    poi.poi_idx
                );
            }
            poi.filtered = !enabled.unwrap_or(true);
            if poi.filtered {
                continue
            }
            if let Some(external) = external {
                let filters = self
                    .pack
                    .pois
                    .get(poi.poi_idx)
                    .and_then(|poi| poi.attributes.filters.as_ref());
                let filtered = filters.map(|attrs| Self::is_filtered(attrs, external));
                if let Some(true) = filtered {
                    poi.filtered = true;
                }
            }
        }
    }

    pub fn clear(&mut self) {
        //self.unused_textures.copy_from_bitslice(&self.loaded_textures);
        self.unused_textures |= &self.loaded_textures;
        self.active_trails.clear();
        self.active_pois.clear();
        self.dirty_trails.clear();
        self.dirty_pois.clear();
        self.available_categories.clear();
        self.copyable_categories.clear();
        self.copyable_pois.clear();
        self.render_list_bookmark = None;
        self.render_poi_bookmark = 0;
        self.poi_bookmark = 0;
    }

    /// Unload no longer needed textures.
    pub fn cleanup_textures(&mut self) {
        for handle in self.unused_textures.iter_ones() {
            self.texture_list[handle] = None;
            self.loaded_textures.set(handle, false);
        }
        self.unused_textures.fill(false);
    }
}
impl AsRef<Pack> for ActivePack {
    fn as_ref(&self) -> &Pack {
        &self.pack
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PackTextureHandle(usize);

pub struct PackCollection {
    pub loaded_packs: IndexMap<String, ActivePack>,
    pub unloaded_packs: IndexMap<String, UnloadedReason>,
    pub generation: usize,

    pub current_map: Option<i32>,
    pub render_list: RenderList,
    pub poi_common: PoiCommonRenderData,
    pub trail_params: TrailParams,

    festival_categories: BTreeMap<&'static str, Festival>,
}

impl PackCollection {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<PackCollection> {
        let poi_common = PoiCommonRenderData::new(backend)?;
        Ok(PackCollection {
            loaded_packs: IndexMap::new(),
            unloaded_packs: IndexMap::new(),
            generation: 0,
            current_map: None,
            render_list: RenderListBuilder::default().build(),
            trail_params: TrailParams::default(),
            poi_common,
            festival_categories: FestivalFixup::festival_categories(),
        })
    }

    pub fn disable_paths(&mut self, disabled_paths: &HashSet<String>) {
        let external = PathingController::external_filter_state();
        for (_pn, pack) in &mut self.loaded_packs {
            pack.disable_paths(&disabled_paths, external.as_ref());
        }
    }

    pub fn clear(&mut self) {
        self.loaded_packs.clear();
        self.unloaded_packs.clear();
        self.generation = self.generation.wrapping_add(1);

        self.render_list.clear();
        self.poi_common.clear();
    }

    #[cfg(todo = "unused")]
    pub fn load_all(&mut self, base_dir: &Path) -> anyhow::Result<()> {
        if !base_dir.exists() {
            create_dir_all(base_dir)?;
        }
        self.clear();
        for entry in read_dir(base_dir)? {
            let entry = entry?;
            self.load(&entry.file_name().to_string_lossy(), &entry.path());
        }
        Ok(())
    }

    #[cfg(todo = "unused")]
    pub fn load(&mut self, name: &str, path: &Path) {
        let result = if path.is_dir() {
            let loader = DirectoryLoader::new(path);
            ActivePack::load(loader)
        } else {
            match path.extension().map(|e| e.as_encoded_bytes()) {
                Some(e) if e.eq_ignore_ascii_case(b"taco") =>
                    ZipLoader::new(path).and_then(ActivePack::load),
                _ => {
                    self.unloaded_packs
                        .insert(name.into(), UnloadedReason::UnknownFormat);
                    return;
                },
            }
        };
        let pack = match result {
            Ok(pack) => pack,
            Err(e) => {
                self.unloaded_packs
                    .insert(name.into(), UnloadedReason::LoadingFailed(format!("{e:?}")));
                return;
            },
        };
        self.loaded_packs.insert(name.into(), pack);
    }

    pub fn add_pack(&mut self, pack: Arc<Pack>, loader: SharedLoader) -> usize {
        let name = pack.name.clone();
        let active = ActivePack::new(pack, loader);
        let (idx, old) = self.loaded_packs.insert_full(name, active);
        if let Some(pack) = old {
            log::info!("Pack {} reloaded", pack.pack.name);
            if let Some(..) = pack.render_list_bookmark {
                let bookmarks = pack
                    .active_trails
                    .values()
                    .flat_map(|trail| {
                        trail.render_bookmark..(trail.render_bookmark + trail.section_bounds.len())
                    })
                    .chain(pack.active_pois.values().map(|poi| poi.render_bookmark as usize));
                let entities = self.render_list.entities_mut();
                for i in bookmarks {
                    if let Some(e) = entities.get_mut(i) {
                        e.disable();
                    }
                }
            }
            drop(pack);
        }
        idx
    }

    #[cfg(todo = "unnecessary")]
    pub fn fixup_active_pack(&self, pack: &mut ActivePack) {
        self.fixup_pack(&mut Arc::make_mut(&mut pack.pack))
    }
    pub fn fixup_pack(&self, pack: &mut Pack) {
        for (_name, category) in &mut pack.categories.all_categories {
            let is_festival = FestivalFixup::FESTIVAL_PREFIXES
                .iter()
                .copied()
                .find(|prefix| category.full_id.as_str().starts_with(prefix));
            match is_festival {
                Some(prefix) if category.full_id != prefix => (),
                _ => continue,
            }
            let festival = self
                .festival_categories
                .iter()
                .find_map(|(&prefix, &fest)| category.full_id.as_str().starts_with(prefix).then_some(fest));
            if let Some(festival) = festival {
                category.attributes_mut().filters_mut().festivals = Some(festival.into());
            } else {
                log::info!("unrecognized festival category: `{}`", category.full_id);
            }
        }
    }

    pub fn load_pack(&mut self, device: &Dx11Device, pack_idx: usize) -> anyhow::Result<()> {
        let (_, pack) = self
            .loaded_packs
            .get_index_mut(pack_idx)
            .with_context(|| format!("unrecognized pack index {pack_idx}"))?;
        if pack.render_list_bookmark.is_some() {
            log::info!("skipping pack {}, already loaded?", pack.pack.name);
            return Ok(())
        }
        let map_id = match self.current_map {
            Some(map_id) => map_id,
            None => {
                log::trace!("delaying pack {} load once in-game", pack.pack.name);
                return Ok(())
            },
        };

        log::debug!("Preparing pack #{pack_idx} {} for rendering...", pack.pack.name);
        self.build_active_pack(pack_idx, device, None, map_id)?;

        if log::log_enabled!(log::Level::Info) {
            let pack = &self.loaded_packs[pack_idx];
            if !pack.active_trails.is_empty() || !pack.active_pois.is_empty() {
                log::info!(
                    "Loaded {} trails and {} POIs from pack #{pack_idx} {}",
                    pack.active_trails.len(),
                    pack.active_pois.len(),
                    pack.pack.name,
                );
            }
        }

        //self.recreate_buffers(device)?;
        self.mark_buffers_dirty();

        Ok(())
    }

    pub fn load_failed(&mut self, name: String, reason: UnloadedReason) {
        self.unloaded_packs.insert(name, reason);
    }

    fn build_active_pack(
        &mut self,
        pack_idx: usize,
        device: &Dx11Device,
        render_entities: Option<&mut Vec<RenderEntity>>,
        map_id: i32,
    ) -> anyhow::Result<()> {
        let (_, pack) = self
            .loaded_packs
            .get_index_mut(pack_idx)
            .with_context(|| format!("unrecognized pack index {pack_idx}"))?;

        let (entities, inplace) = match render_entities {
            Some(e) => (e, false),
            None => (self.render_list.entities_mut(), true),
        };
        let bookmark_start = entities.len();
        let res = pack
            .prepare_new_map(pack_idx, map_id, device, entities, &self.trail_params)
            .with_context(|| format!("loading pack {} for map {map_id}", pack.pack.name));
        if res.is_err() {
            log::info!(
                "pack {} failed to load for map {map_id}, disabling...",
                pack.pack.name
            );
            if let Some(..) = pack.render_list_bookmark {
                let _ = entities.drain(bookmark_start..);
                /*for entity in &mut self.render_list.entities_mut()[bookmark..] {
                    entity.disable();
                }*/
            }
            pack.clear();
            pack.cleanup_textures();
        } else {
            let external = PathingController::external_filter_state();
            pack.recompute_enabled(external.as_ref());
            #[cfg(feature = "paths-lua")]
            {
                let active_pois = pack
                    .active_pois
                    .values()
                    .filter_map(|poi| (!poi.filtered).then_some(poi.poi_idx))
                    .collect::<Vec<_>>();
                let active_trails = pack
                    .active_trails
                    .values()
                    .filter_map(|trail| (!trail.filtered).then_some(trail.trail_idx))
                    .collect::<Vec<_>>();
                let active_pois = active_pois.into_iter().map(|i| (MarkerType::Poi, i));
                let active_trails = active_trails.into_iter().map(|i| (MarkerType::Trail, i));
                ScriptMessage::map_prepared_pack(
                    self.generation,
                    pack_idx,
                    map_id as u32,
                    active_pois.chain(active_trails),
                )
                .try_send();
            }
        }
        if inplace {
            self.render_list.entities_mut_end();
        }
        res
    }

    pub fn prepare_new_map(&mut self, map_id: i32, device: &Dx11Device) -> anyhow::Result<()> {
        if self.current_map == Some(map_id) {
            return Ok(());
        }
        self.current_map = Some(map_id);
        let mut render_builder = self.render_list.rebuild();

        let mut succ = false;
        let mut res = None;
        let packs_len = self.loaded_packs.len();
        for pack_idx in 0..packs_len {
            let pack_res = self
                .build_active_pack(pack_idx, device, Some(&mut render_builder.entities), map_id)
                .with_context(|| {
                    format!(
                        "Pack {} failed to load",
                        self.loaded_packs
                            .get_index(pack_idx)
                            .map(|(_, p)| &p.pack.name[..])
                            .unwrap_or("<badidx>")
                    )
                });
            if let Err(e) = pack_res {
                log::warn!("{e:#}");
                let _ = res.get_or_insert(e);
            } else {
                succ = true;
            }
        }
        match res {
            Some(e) if !succ => return Err(e.into()),
            _ => (),
        }

        log::info!(
            "Loaded {} trails and {} POIs",
            self.loaded_packs
                .values()
                .map(|p| p.active_trails.len())
                .sum::<usize>(),
            self.loaded_packs
                .values()
                .map(|p| p.active_pois.len())
                .sum::<usize>(),
        );

        //let res = self.recreate_buffers(device);
        self.mark_buffers_dirty();
        let res = Ok(());

        self.render_list = render_builder.build();

        res
    }

    fn recreate_buffers_inner(
        &mut self,
        device: &Dx11Device,
        machine: &RenderMachine,
    ) -> anyhow::Result<()> {
        // identity at start for trail drawing
        let mut data_world = vec![InstanceBufferData::IDENTITY; 1];
        let mut data_map = vec![InstanceBufferData::IDENTITY; 1];

        let mut render_poi_bookmark = 1;
        for pack in self.loaded_packs.values_mut() {
            data_world.extend(pack.active_pois.values_mut().map(|poi| {
                #[cfg(feature = "paths-lua")]
                {
                    poi.ibd_dirty_space = false;
                }
                poi.instance_data()
            }));
            data_map.extend(pack.active_pois.values_mut().map(|poi| {
                #[cfg(feature = "paths-lua")]
                {
                    poi.ibd_dirty_map = false;
                }
                poi.instance_data_map(machine)
            }));
            pack.render_poi_bookmark = render_poi_bookmark;
            render_poi_bookmark += pack.active_pois.len();
        }
        let (data_world, data_map) = (&data_world[..], &data_map[..]);
        super::poi::STATS_POI_INSTANCE_SIZE
            .reset_with(|| (size_of_val(data_map) + size_of_val(data_world)) as _);
        let (poi_ib_world, poi_ib_map) = (
            Some(BufferOf::new_with_data(device, Ok(data_world), ())?),
            Some(BufferOf::new_with_data(device, Ok(data_map), ())?),
        );
        self.poi_common.world_ib = poi_ib_world;
        self.poi_common.map_ib = poi_ib_map;

        Ok(())
    }

    fn recreate_buffers(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        let res = self
            .recreate_buffers_inner(device, machine)
            .context("preparing POI instance buffers");
        if res.is_err() {
            self.mark_buffers_dirty();
        }
        res
    }

    fn mark_buffers_dirty(&mut self) {
        self.poi_common.clear();
        for pack in self.loaded_packs.values_mut() {
            pack.render_poi_bookmark = 0;
        }
    }

    pub fn destroy_buffers(&mut self) {
        self.mark_buffers_dirty();
    }

    pub fn prepare(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        if
        /* !self.loaded_packs.is_empty() &&*/
        self.poi_common.is_empty() {
            self.recreate_buffers(device, machine)?;
        }

        Ok(())
    }

    pub fn update(&mut self, machine: &RenderMachine, _: &Dx11Device, context: &Dx11Context) {
        for (_, pack) in &mut self.loaded_packs {
            pack.update(&mut self.render_list, &mut self.poi_common, machine, context);
        }
    }

    #[cfg(feature = "paths-lua")]
    pub(crate) fn script_start(
        &mut self,
        device: &Dx11Device,
        machine: &mut RenderMachine,
        (generation, pack_idx): (usize, usize),
        shared: Arc<PackPlugShared>,
    ) {
        if generation != self.generation {
            // TODO: inform it to shut down?
            return
        }
        let Some((_, pack)) = self.loaded_packs.get_index_mut(pack_idx) else {
            #[cfg(taimi_debug)]
            log::warn!("received update for missing pack#{pack_idx}");
            return
        };
        pack.script_start(device, machine, shared);
    }
    #[cfg(feature = "paths-lua")]
    pub(crate) fn script_update_marker(
        &mut self,
        device: &Dx11Device,
        machine: &mut RenderMachine,
        (generation, pack_idx): (usize, usize),
        (kind, marker_idx): MarkerLoc,
        changed_attrs: &mut dyn Iterator<Item = PackKeyId>,
    ) {
        if generation != self.generation {
            // XXX: inform it to shut down maybe? could be spammy though...
            return
        }
        let for_trail = match kind {
            MarkerType::Trail => true,
            MarkerType::Poi => false,
            // TODO: consider stashing updates somewhere for ui to see!
            // TODO: interactive menus in particular could go somewhere on rendermachine!
            MarkerType::Category => return,
        };
        let Some((_, pack)) = self.loaded_packs.get_index_mut(pack_idx) else {
            #[cfg(taimi_debug)]
            log::warn!("received update for missing pack#{pack_idx}");
            return
        };

        if for_trail {
            pack.script_update_trail(device, machine, marker_idx, changed_attrs);
        } else {
            pack.script_update_poi(device, machine, marker_idx, changed_attrs);
        }
    }
    #[cfg(feature = "paths-lua")]
    pub(crate) fn script_create(
        &mut self,
        device: &Dx11Device,
        machine: &mut RenderMachine,
        (generation, pack_idx): (usize, usize),
        (kind, marker_idx): MarkerLoc,
    ) {
        if generation != self.generation {
            // XXX: inform it to shut down maybe? could be spammy though...
            return
        }
        let Some((_, pack)) = self.loaded_packs.get_index_mut(pack_idx) else {
            #[cfg(taimi_debug)]
            log::warn!("received create for missing pack#{pack_idx}");
            return
        };

        match kind {
            MarkerType::Category => {
                // TODO: consider stashing updates somewhere for ui to see!
                // TODO: interactive menus in particular could go somewhere on rendermachine!
                if pack.user_category_state.len() < marker_idx {
                    pack.user_category_state.resize(marker_idx + 1, false);
                }
                let default_toggle = {
                    let o = pack
                        .script_data
                        .as_ref()
                        .map(|d| PackOverrides::shared_read(&d.overrides))
                        .as_ref()
                        .and_then(|o| o.overrides.get(&(kind, marker_idx)))
                        .cloned();
                    o.as_ref().map(MarkerOverrides::shared_read).and_then(|o| {
                        o.get::<keys::DefaultToggle>()
                            .flatten()
                            .map(|v| bool::from(*v.get()))
                    })
                };
                if default_toggle != Some(false) {
                    unsafe {
                        pack.user_category_state.set_unchecked(marker_idx, true);
                    }
                }
            },
            MarkerType::Trail => {
                let Some(current_map) = self.current_map else { return };
                let bookmark = self.render_list.entities_count();
                let trail_idx = pack.active_trails.len();
                if pack.script_create_trail(current_map as _, device, bookmark, machine, marker_idx) {
                    if let Some((_, new_trail)) = pack.active_trails.last_mut() {
                        let e = self.render_list.entities_mut();
                        for (i_section, bounds) in new_trail.section_bounds.iter().copied().enumerate() {
                            let entity = RenderEntity {
                                bounds,
                                position: bounds.center(),
                                draw_ordered: false,
                                render_id: Some(RenderId::TrailSection {
                                    pack_idx,
                                    trail_idx,
                                    section: i_section,
                                }),
                            };
                            e.push(entity);
                        }
                        if let Some(mut d) = pack.dirty_trails.last_mut() {
                            d.set(false);
                        }
                        self.render_list.entities_mut_end();
                    }
                }
            },
            MarkerType::Poi => {
                let Some(current_map) = self.current_map else { return };
                let bookmark = self.render_list.entities_count();
                let poi_idx = pack.active_pois.len();
                if pack.script_create_poi(current_map as _, device, bookmark, machine, marker_idx) {
                    if let Some((_, new_poi)) = pack.active_pois.last_mut() {
                        let e = self.render_list.entities_mut();
                        new_poi.render_bookmark = e.len() as u32;
                        let render_id = RenderId::Poi { pack_idx, poi_idx };
                        let has_bounds = !new_poi.bounds_dirty();
                        let bounds = match has_bounds {
                            false => RenderList::BOUNDS_NONE,
                            true => new_poi.bounds,
                        };
                        e.push(RenderEntity {
                            bounds,
                            position: new_poi.position,
                            draw_ordered: true,
                            render_id: Some(render_id),
                        });
                        //self.render_list.entities_mut_end();
                        if let (Some(mut d), true) = (pack.dirty_pois.last_mut(), has_bounds) {
                            d.set(false);
                        }
                    }
                    self.render_list.entities_mut_end();
                    self.mark_buffers_dirty();
                }
            },
        }
    }
    #[cfg(feature = "paths-lua")]
    pub(crate) fn script_mask(
        &mut self,
        _device: &Dx11Device,
        _machine: &mut RenderMachine,
        (generation, pack_idx): (usize, usize),
        (kind, marker_idx): MarkerLoc,
    ) {
        if generation != self.generation {
            // XXX: inform it to shut down maybe? could be spammy though...
            return
        }
        let for_trail = match kind {
            MarkerType::Trail => true,
            MarkerType::Poi => false,
            // TODO: consider stashing updates somewhere for ui to see!
            // TODO: interactive menus in particular could go somewhere on rendermachine!
            MarkerType::Category => return,
        };
        let Some((_, pack)) = self.loaded_packs.get_index_mut(pack_idx) else {
            #[cfg(taimi_debug)]
            log::warn!("received remove for missing pack#{pack_idx}");
            return
        };

        if for_trail {
            let trail = pack.active_trails.values().find(|t| t.trail_idx == marker_idx);
            if let Some(trail) = trail {
                for i_section in 0..trail.section_bounds.len() {
                    self.render_list.disable(trail.render_bookmark + i_section);
                }
            }
        } else {
            let poi = pack.active_pois.values_mut().find(|p| p.poi_idx == marker_idx);
            if let Some(poi) = poi {
                self.render_list.disable(poi.render_bookmark as _);
            }
        }
    }

    pub fn draw(
        &mut self,
        camera: RenderPosition,
        frustum: &MapFrustum,
        backend: &RenderBackend,
        device_context: &Dx11Context,
    ) {
        let entities = self.render_list.get_entities_for_drawing(camera, frustum);
        Self::draw_entities(
            &self.loaded_packs,
            &self.poi_common,
            device_context,
            backend,
            entities,
        );
        STATS_ENTITY_COUNT.store(self.render_list.entities_count(), Ordering::Relaxed);
    }

    pub fn draw_entities<'e, E>(
        loaded_packs: &IndexMap<String, ActivePack>,
        poi_common: &PoiCommonRenderData,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        entities: E,
    ) where
        E: IntoIterator<Item = &'e RenderEntity>,
    {
        poi_common.set_primitive(device_context);

        let mut shader_state = ShaderState::None;
        let mut trail_colour = None;
        let mut poi_billboarding = true;
        let mut num_drawn = 0usize;
        for entity in entities {
            let render_id = match entity.render_id {
                Some(id) => id,
                None => continue,
            };
            match render_id {
                RenderId::TrailSection { pack_idx, trail_idx, section } => {
                    let trail = loaded_packs.get_index(pack_idx).and_then(|(_, pack)| {
                        pack.active_trails.get_index(trail_idx).map(|(_, trail)| trail)
                    });
                    let trail = match trail {
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing trail#{trail_idx} pack#{pack_idx} section#{section}");
                            continue
                        },
                    };
                    if trail.filtered || !bool::from(trail.attr_vis_space) {
                        continue
                    }
                    let colour = match (trail_colour, trail.tint()) {
                        (_, Some(tint)) => Some(tint),
                        (Some(..), None) => Some(glam::Vec4::ONE),
                        _ => None,
                    };
                    if let (Some(colour), Some(ib)) = (colour, poi_common.world_ib.as_ref()) {
                        let coloured = InstanceBufferData { colour, ..InstanceBufferData::IDENTITY };
                        unsafe {
                            ib.update_element_at(device_context, &coloured, 0, 0);
                        }
                        trail_colour = match colour {
                            c if c == glam::Vec4::ONE => None,
                            c => Some(c),
                        };
                    }
                    if shader_state == ShaderState::None {
                        poi_common.set_instance(device_context, LocalContext::World);
                        poi_common.set_primitive(device_context);
                    }
                    if shader_state != ShaderState::Trail {
                        shader_state = ShaderState::Trail;
                        backend.shaders.set_named(device_context, "trail");
                    }
                    trail.draw_section(device_context, section, LocalContext::World);
                },
                RenderId::Poi { pack_idx, poi_idx } => {
                    let poi = loaded_packs.get_index(pack_idx).and_then(|(_, pack)| {
                        pack.active_pois.get_index(poi_idx).map(|(_, poi)| (pack, poi))
                    });
                    let (pack, poi) = match poi {
                        Some((pack, ..)) if pack.render_poi_bookmark == 0 => continue,
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing poi#{poi_idx} pack#{pack_idx}");
                            continue
                        },
                    };
                    if poi.filtered || !bool::from(poi.attr_vis_space) {
                        continue
                    }
                    if shader_state != ShaderState::Poi {
                        shader_state = ShaderState::Poi;
                        poi_common.set(device_context);
                    }
                    let was_billboarding = mem::replace(&mut poi_billboarding, poi.is_billboard());
                    if was_billboarding != poi_billboarding {
                        backend.perspective_handler.select_billboard_cb(
                            device_context,
                            0,
                            poi_billboarding,
                        );
                    }
                    poi.draw(
                        device_context,
                        pack.render_poi_bookmark + poi_idx,
                        LocalContext::World,
                    );
                },
            }
            num_drawn += 1;
        }
        if !poi_billboarding {
            backend
                .perspective_handler
                .select_billboard_cb(device_context, 0, true);
        }
        if trail_colour.is_some() {
            // reset back to default...
            if let Some(ib) = poi_common.world_ib.as_ref() {
                unsafe {
                    ib.update_element_at(device_context, &InstanceBufferData::IDENTITY, 0, 0);
                }
            }
        }
        STATS_ENTITY_DRAW.store(num_drawn, Ordering::Relaxed);
    }

    #[cfg(feature = "goggles")]
    pub fn entities_obscured<'a>(
        &'a self,
        frustum: &'a MapFrustum,
    ) -> impl Iterator<Item = &'a RenderEntity> + 'a {
        self.render_list.visible_entities(frustum)
    }

    pub fn draw_map_entities<'e, E>(
        loaded_packs: &IndexMap<String, ActivePack>,
        poi_common: &PoiCommonRenderData,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        map: MapContext,
        entities: E,
    ) where
        E: IntoIterator<Item = &'e RenderEntity>,
    {
        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0usize;
        let mut trail_colour = None;
        let ctx = LocalContext::/*Map(map)*/MAP;
        for entity in entities {
            let render_id = match entity.render_id {
                Some(id) => id,
                None => continue,
            };
            match render_id {
                RenderId::TrailSection { pack_idx, trail_idx, section } => {
                    let trail = loaded_packs.get_index(pack_idx).and_then(|(_, pack)| {
                        pack.active_trails.get_index(trail_idx).map(|(_, trail)| trail)
                    });
                    let trail = match trail {
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing trail#{trail_idx} pack#{pack_idx} section#{section}");
                            continue
                        },
                    };
                    if trail.filtered || !trail.is_visible_for_map(map) {
                        continue
                    }
                    if shader_state == ShaderState::None {
                        backend.shaders.set_named(device_context, "map");
                        poi_common.set_primitive(device_context);
                        poi_common.set_instance(device_context, ctx);
                    }
                    let colour = match (trail_colour, trail.tint_map()) {
                        (_, Some(tint)) => Some(tint),
                        (Some(..), None) => Some(glam::Vec4::ONE),
                        _ => None,
                    };
                    if let (Some(colour), Some(ib)) = (colour, poi_common.map_ib.as_ref()) {
                        let coloured = InstanceBufferData { colour, ..InstanceBufferData::IDENTITY };
                        unsafe {
                            ib.update_element_at(device_context, &coloured, 0, 0);
                        }
                        trail_colour = match colour {
                            c if c == glam::Vec4::ONE => None,
                            c => Some(c),
                        };
                    }
                    shader_state = ShaderState::Trail;
                    trail.draw_section(device_context, section, ctx);
                },
                RenderId::Poi { pack_idx, poi_idx } => {
                    let poi = loaded_packs.get_index(pack_idx).and_then(|(_, pack)| {
                        pack.active_pois.get_index(poi_idx).map(|(_, poi)| (pack, poi))
                    });
                    let (pack, poi) = match poi {
                        Some((pack, ..)) if pack.render_poi_bookmark == 0 => continue,
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing poi#{poi_idx} pack#{pack_idx}");
                            continue
                        },
                    };
                    if poi.filtered || !poi.is_visible_for_map(map) {
                        continue
                    }
                    #[cfg(todo)]
                    if !poi.attr_scale_on_map_with_zoom {
                        // idk invert .-.
                    }
                    if shader_state == ShaderState::None {
                        backend.shaders.set_named(device_context, "map");
                        poi_common.set_primitive(device_context);
                        poi_common.set_instance(device_context, ctx);
                    }
                    if shader_state != ShaderState::Poi {
                        shader_state = ShaderState::Poi;
                        poi_common.set_vertex(device_context, ctx);
                    }
                    poi.draw(device_context, pack.render_poi_bookmark + poi_idx, ctx);
                },
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW_MAP.store(num_drawn, Ordering::Relaxed);
        if trail_colour.is_some() {
            // reset back to default...
            if let Some(ib) = poi_common.map_ib.as_ref() {
                unsafe {
                    ib.update_element_at(device_context, &InstanceBufferData::IDENTITY, 0, 0);
                }
            }
        }
    }

    pub fn entities_map<'a>(
        &'a self,
        mut bounds: Box3<DrawSpace>,
    ) -> impl Iterator<Item = &'a RenderEntity> + 'a {
        // adding some wiggle room around the map edges...
        let buffer = bounds.size() * 0.15;
        bounds.min.x -= buffer.width;
        bounds.min.z -= buffer.depth;
        bounds.max.x += buffer.width;
        bounds.max.z += buffer.depth;

        self.render_list.map_entities(bounds)
    }

    pub fn unload_map(&mut self, _device_context: &Dx11Context, _map_id: u32) -> anyhow::Result<()> {
        //if self.current_map != Some(_map_id) { return }
        self.clear_active();
        self.current_map = None;

        Ok(())
    }

    pub fn load_map(
        &mut self,
        device: &Dx11Device,
        _device_context: &Dx11Context,
        map_id: u32,
    ) -> anyhow::Result<()> {
        self.prepare_new_map(map_id as i32, device)
    }

    pub fn clear_active(&mut self) {
        self.render_list.clear();
        for pack in self.loaded_packs.values_mut() {
            pack.clear();
        }

        self.poi_common.clear();
    }

    pub fn cleanup_textures(&mut self) {
        for pack in self.loaded_packs.values_mut() {
            pack.cleanup_textures();
        }
    }

    /// See [crate::space::engine::Engine::cleanup_background]
    ///
    /// TODO: revisit, avoid, etc
    pub fn cleanup_background(self) {
        let Self { loaded_packs, poi_common, .. } = self;
        mem::forget((loaded_packs, poi_common));
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ShaderState {
    None,
    Trail,
    Poi,
}

pub static STATS_ENTITY_DRAW: AtomicUsize = AtomicUsize::new(0);
pub static STATS_ENTITY_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static STATS_ENTITY_DRAW_MAP: AtomicUsize = AtomicUsize::new(0);
