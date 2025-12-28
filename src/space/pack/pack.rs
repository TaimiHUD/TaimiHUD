use {
    super::{render, PoiCommonRenderData}, crate::{
        controller::pathing::{
            registry::{LoadedPoiNs, LoadedPoiPath, LoadedTrailNs, LoadedTrailIndex, LoadedPoiIndex, LoadedTrailPath, PackIndex, PackRegistryNs, PackVecOf}, shared::{MapPackInfo, SharedGameplayMap, SharedLoaderPacksInfo, SharedMapPackLoaded, SharedMapPackState, SharedPackInfo}, space::{SpacePack, SpacePackCollection, SpacePackShared, TrailGeometryRequests, TextureLoadRequests, TrailParams}, visible::{LoadedPoi, LoadedTrail, LoadedTrailGeometry}, ExternalFilterState, FestivalFixup, PathingController, PathingEvent
        },
        exports::runtime::{
            self as rt,
            imgui::{self, Condition, StyleVar, TreeNode, Ui},
        },
        fl,
        render::{
            machine::{RenderMachine, RenderPosition},
            pathing_window::PathingSearchState,
            RenderState,
        },
        settings::state::ui::pathing::PathingFilterFlags as PathingFilterState,
        space::{
            dx11::{InstanceBufferData, RenderBackend}, pack::{PoiRender, TrailRender}, render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder}, resources::Texture, DrawSpace
        },
        with_i18n,
    },
    taimi_meta::packs::MapIndex,
    anyhow::{anyhow, Context}, bitvec::vec::BitVec, bvh::aabb, glamour::{Box3, Point3}, indexmap::IndexMap, rustc_hash::FxHashSet, std::{collections::{BTreeMap, BTreeSet, BinaryHeap, HashSet}, fs::{create_dir_all, read_dir}, mem, ops, path::Path, sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        }}, taimi_d3d::dx11::{buffer::BufferOf, prelude::*}, taimi_hoard::{iters::IterExt as _, loc::{indexed::IndexedList, LocationMut, LocationRef}, statistics::Counter}, taimi_meta::{packs::{id::{MarkerIndexVariant, MarkerId, MarkerIndex}, PackMapPath, PackPoiNs, PackTrailNs, PoiIndex, TrailIndex, TrailSectionPath}, spatial::box3aabb, ui::{LocalContext, MapContext}}, taimi_pack::{
        attributes::{Festival, FilterAttributes, MarkerAttributes},
        category::CategoryId,
        loader::{DirectoryLoader, PackLoaderContext, ZipLoader},
        Category,
        Poi,
    }, taimi_sync::{arcs::ArcPtrCmp, watched::{watch, Watched}}, uuid::Uuid
};

#[cfg(deleteme)]
pub struct ActivePack {
    pub pack: Arc<Pack>,
    loader: LoaderBox,

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
    // TODO: Scripting.
    //_script_engine: (),
}

#[cfg(todo)]
impl ActivePack {
    pub fn new(pack: Arc<Pack>, loader: LoaderBox) -> Self {
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
        }
    }

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

    pub fn draw_categories(
        &mut self,
        ui: &Ui,
        filter_state: PathingFilterState,
        open_items: &mut HashSet<CategoryId>,
        recompute: &mut bool,
        search_state: &PathingSearchState,
    ) {
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

    pub fn draw_category(
        ui: &Ui,
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
    ) {
        let push_token = ui.push_id(&category.full_id);
        if category.is_hidden() {
            push_token.pop();
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
                if ui.small_button(&fl!("copy-arg", arg = (&category.display_name[..]))) {
                    Self::copy_copyable(ui, &category.marker_attributes);
                }
                if ui.is_item_hovered() {
                    Self::draw_tooltip(ui, &category.display_name, || {
                        Self::draw_tooltip_category(ui, category);
                        Self::draw_tooltip_copyable(
                            ui,
                            &category.marker_attributes,
                            Some(&category.display_name),
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
                            state_checkbox = Some(move || {
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
                        checkbox_gap = Some(ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0])));
                        #[cfg(todo = "unnecessary")]
                        let _inner_gap = ui.push_style_var(StyleVar::ItemInnerSpacing([0.0, 0.0]));
                        checkbox();
                        ui.same_line();
                    }
                }

                let (copyable_categories, copyable_pois, pois) = copyable;
                let has_copyable_pois = category_idx
                    .map(|idx| copyable_categories.contains(&idx))
                    .unwrap_or(false);

                let mut unbuilt = TreeNode::new(&category.display_name);
                if (category.is_separator() || category.sub_categories.is_empty())
                    && copy_value.is_none()
                    && !has_copyable_pois
                {
                    unbuilt = unbuilt.flags(imgui::TreeNodeFlags::SPAN_AVAIL_WIDTH);
                }
                unbuilt = unbuilt
                    .frame_padding(true)
                    .tree_push_on_open(false)
                    .allow_item_overlap(state_checkbox.is_some());
                if category.is_separator() {
                    unbuilt = unbuilt.leaf(true);
                } else if category.sub_categories.is_empty() {
                    unbuilt = unbuilt.bullet(true);
                } else {
                    unbuilt = unbuilt
                        .framed(true)
                        .opened(open_items.contains(&category.full_id), Condition::Always);
                }
                let tree_token = unbuilt.push(ui);
                drop(checkbox_gap);
                if ui.is_item_hovered() && Self::category_has_tooltip(category) {
                    Self::draw_tooltip(ui, &category.display_name, || {
                        Self::draw_tooltip_category(ui, category);
                    });
                }
                if let Some(mut checkbox) = state_checkbox.take() {
                    ui.same_line();
                    ui.dummy([4.0, 0.0]);
                    ui.same_line();
                    checkbox();
                } else if has_state_checkbox {
                    ui.indent();
                }
                if copy_value.is_some() {
                    ui.same_line();
                    if with_i18n!("copy", |copy| ui.small_button(copy)) {
                        Self::copy_copyable(ui, &category.marker_attributes);
                    }
                    if ui.is_item_hovered() {
                        Self::draw_tooltip(ui, &category.display_name, || {
                            Self::draw_tooltip_copyable(
                                ui,
                                &category.marker_attributes,
                                Some(&category.display_name),
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
                            Some(name) => ui.small_button(&fl!("copy-arg", arg = (&name[..]))),
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
                            Self::draw_tooltip(ui, template, || {
                                Self::draw_tooltip_poi(ui, &copyable.attributes);
                                Self::draw_tooltip_copyable(ui, &copyable.attributes, None);
                            });
                        }
                    }
                }
                let mut internal_closure = || {
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
                    internal_closure();
                    token.pop();
                } else {
                    if open_items.contains(&category.full_id) {
                        open_items.remove(&category.full_id);
                    }
                }
            }
        }
        push_token.pop();
    }

    pub(crate) fn copy_copyable(ui: &Ui, attributes: &MarkerAttributes) {
        let interaction = attributes.interaction();
        let Some(copy_value) = &interaction.copy_value else { return };
        ui.set_clipboard_text(&copy_value[..]);
        if let Some(copy_message) = &interaction.copy_message {
            let _ = rt::send_alert(ui, &copy_message[..]);
        }
    }

    pub(crate) fn draw_tooltip_category(ui: &Ui, category: &Category) {
        let desc = match &category.marker_attributes.tip_description {
            Some(desc) if !desc.is_empty() => Some(&desc[..]),
            _ => None,
        };
        let title = match &category.marker_attributes.tip_name {
            Some(title) if !title.is_empty() && !category.display_name.starts_with(&title[..]) =>
                Some(&title[..]),
            _ => None,
        };

        if let Some(title) = title {
            let _title_font = desc.map(|_| RenderState::push_font("big", ui));
            ui.text(title);
        }

        if let Some(tip) = desc {
            ui.text_wrapped(tip);
        }
    }

    fn draw_tooltip_poi(ui: &Ui, attributes: &MarkerAttributes) {
        let desc = match &attributes.tip_description {
            Some(desc) if !desc.is_empty() => Some(&desc[..]),
            _ => None,
        };

        if let Some(title) = &attributes.tip_name {
            let _title_font = desc.map(|_| RenderState::push_font("big", ui));
            ui.text(&title[..]);
        }
        if let Some(desc) = &attributes.tip_description {
            ui.text_wrapped(&desc[..]);
        }
    }

    pub(crate) fn category_has_tooltip(category: &Category) -> bool {
        match &category.marker_attributes.tip_description {
            Some(desc) if !desc.is_empty() => return true,
            _ => (),
        }
        match &category.marker_attributes.tip_name {
            Some(title) if !title.is_empty() && !category.display_name[..].starts_with(&title[..]) =>
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

    pub(crate) fn draw_tooltip<F: FnOnce()>(ui: &Ui, title_template: &str, f: F) {
        let _id = ui.push_id("category_tooltip");
        let [minwidth, lineheight] = ui.calc_text_size(title_template);
        unsafe {
            imgui::sys::igSetNextWindowSize([0.0, lineheight * 1.5].into(), Condition::Appearing as _);
        };
        let _size = ui.push_style_var(StyleVar::WindowMinSize([minwidth, lineheight]));
        ui.tooltip(|| {
            {
                let _padding = ui.push_style_var(StyleVar::ItemSpacing([f32::EPSILON, f32::EPSILON]));
                ui.dummy([minwidth, f32::EPSILON]);
            }
            f()
        })
    }

    fn draw_tooltip_copyable(ui: &Ui, attributes: &MarkerAttributes, display_name: Option<&str>) {
        let interaction = attributes.interaction();
        let copy_message = interaction.copy_message.as_ref().map(|m| &m[..]);
        match &interaction.copy_value {
            Some(copy_value)
                if (display_name.is_none() || copy_message.is_none())
                    && Self::copyable_value_has_message(attributes) =>
                ui.text_wrapped(&format!("\"{copy_value}\"")),
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

    pub fn update(&mut self, render_list: &mut RenderList) {
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

        for trail_idx in self.dirty_trails.iter_ones() {
            let trail = &self.active_trails[trail_idx];
            for i_section in 0..trail.section_bounds.len() {
                render_list.update(trail.render_bookmark + i_section);
            }
        }
        for poi_idx in self.dirty_pois.iter_ones() {
            render_list.update(self.poi_bookmark + poi_idx);
        }
    }

    pub fn register_texture(&mut self, asset: &str) -> PackTextureHandle {
        if let Some(id) = self.texture_list.get_index_of(asset) {
            return PackTextureHandle(id);
        }

        self.loaded_textures.push(false);
        self.unused_textures.push(false);
        let idx = self.texture_list.insert_full(asset.to_string(), None).0;
        PackTextureHandle(idx)
    }

    pub fn loader(&mut self) -> &mut dyn PackLoaderContext {
        let loader: &mut (dyn PackLoaderContext + Send) = &mut self.loader;
        loader
    }

    pub fn get_or_load_texture<'t>(
        &'t mut self,
        handle: PackTextureHandle,
        device: &Dx11Device,
    ) -> anyhow::Result<&'t Arc<Texture>> {
        let PackTextureHandle(idx) = handle;
        let (asset, slot) = self
            .texture_list
            .get_index_mut(idx)
            .ok_or_else(|| anyhow!("Texture {} not in list at all", idx))?;

        let texture = match slot {
            slot_texture @ None => {
                let data = self.loader.load_asset_dyn(asset)?;
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
                self.loaded_textures.set(idx, true);
                texture
            },
            Some(texture) => texture,
        };
        self.unused_textures.set(idx, false);
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

            let category_idx = pack
                .categories
                .all_categories
                .get_index_of(pack_trail.category.as_id())
                .unwrap_or(0);
            let trail = ActiveTrail::build(
                self,
                pack_trail,
                i_trail,
                category_idx,
                trail_params,
                render_entities.len(),
                device,
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

            let category_idx = pack
                .categories
                .all_categories
                .get_index_of(pack_poi.category.as_id())
                .unwrap_or(0);
            let poi = ActivePoi::build(self, pack_poi, i_poi, category_idx, device)
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
#[cfg(deleteme)]
impl AsRef<Pack> for ActivePack {
    fn as_ref(&self) -> &Pack {
        &self.pack
    }
}

#[cfg(deleteme)]
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PackTextureHandle(usize);

/// Internal rendering data.
pub struct PackRenderData {
    pub info: Arc<SharedPackInfo>,
    pub map_info: Option<SharedMapPackLoaded>,
    pub map_state: SharedMapPackState,
    #[cfg(todo)]
    pub render_list_bookmark: Option<usize>,
    pub pois: IndexedList<LoadedPoiNs, LoadedPoiIndex, Vec<PoiRender>>,
    pub trails: IndexedList<LoadedTrailNs, LoadedTrailIndex, Vec<TrailRender>>,
    render_poi_bookmark: usize,
    #[cfg(todo)]
    poi_bookmark: usize,
}

impl PackRenderData {
    pub fn new() -> Self {
        Self {
            info: Default::default(),
            map_info: None,
            map_state: Default::default(),
            pois: Default::default(),
            trails: Default::default(),
            render_poi_bookmark: 0,
        }
    }

    pub fn render_poi_bookmarks(&self) -> ops::Range<PoiIndex> {
        match self.render_poi_bookmark {
            0 => 0..0,
            start => {
                let end = (start + self.pois.len()) as PoiIndex;
                let start = start as PoiIndex;
                start..end
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        let trails = self.trails.is_empty();
        let pois = self.render_poi_bookmark == 0 || self.pois.is_empty();
        trails && pois
    }

    pub fn clear(&mut self) {
        self.pois.clear();
        self.trails.clear();
        self.map_info = None;
        self.render_poi_bookmark = 0;
    }

    pub fn cleanup_background(mut self) {
        // mostly just make a point of not cleaning up render resources...
        for poi in self.pois.drain(..) {
            poi.cleanup_background();
        }
        for trail in self.trails.drain(..) {
            trail.cleanup_background();
        }
    }
}

pub struct PackRender {
    pub pack_data: PackVecOf<PackRenderData>,
    #[cfg(todo)]
    pub render_list: RenderList,
    pub poi_common: PoiCommonRenderData,

    pub spacepacks: Watched<Arc<SpacePackCollection>>,
    pub trail_rx: TrailGeometryRequests,
    pub texture_rx: TextureLoadRequests,
    packs_rx: Option<watch::Receiver<SharedLoaderPacksInfo>>,
    packs_map: Option<watch::Receiver<SharedGameplayMap>>,
    pub render_list: PackRenderList,
    pub draw_state: PackRenderState,
}

impl PackRender {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<Self> {
        let poi_common = PoiCommonRenderData::new(backend)?;
        Ok(Self {
            spacepacks: Default::default(),
            trail_rx: TrailGeometryRequests::empty(),
            texture_rx: TextureLoadRequests::empty(),
            packs_rx: None,
            packs_map: None,
            pack_data: Default::default(),
            render_list: Default::default(),
            draw_state: Default::default(),
            poi_common,
        })
    }

    pub fn any_loaded(&self) -> bool {
        log::debug!("TODO: any_loaded?");
        // !self.packs.is_empty()
        true
    }

    fn mark_buffers_dirty(&mut self) {
        self.poi_common.clear();
        for pack in self.pack_data.values_mut() {
            pack.render_poi_bookmark = 0;
        }
    }

    pub fn destroy_buffers(&mut self) {
        self.mark_buffers_dirty();
    }

    /// `Ok(false)` if not ready to render
    ///
    /// won't render if not in a map, or if too early in load and
    /// more setup may be pending
    pub fn prepare(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<bool> {
        let Some(pathing) = &machine.pathing else {
            anyhow::bail!("no shared data")
        };
        let packs_rx = self.packs_rx.get_or_insert_with(|| {
            let mut rx = pathing.packs.packs.subscribe();
            rx.mark_changed();
            rx
        });
        let packs_map = self.packs_map.get_or_insert_with(|| {
            let mut rx = pathing.gameplay.subscribe();
            rx.mark_changed();
            rx
        });
        if !self.spacepacks.is_watching() {
            self.spacepacks.restart_watching(&pathing.space.collection);
        }
        if !self.trail_rx.is_watching() {
            self.trail_rx.subscribe_to(&pathing.space.trail_geometry);
        }
        if !self.texture_rx.is_watching() {
            self.texture_rx.subscribe_to(&pathing.space.texture_loads);
        }
        if packs_rx.has_changed().unwrap_or(false) {
            let packs = packs_rx.borrow_and_update();
            if self.pack_data.len() < packs.len() {
                log::debug!("PATHY: space packs resized to {}", packs.len());
                self.pack_data.data.resize_with(packs.len(), PackRenderData::new);
            }
            for (pack, dest) in packs.values().zip(self.pack_data.values_mut()) {
                #[cfg(todo)]
                let prev_sig = dest.info.sig;
                dest.info = pack.info.clone();
            }
        }
        if let Some(spacepacks) = self.spacepacks.try_read_if_changed() {
            ArcPtrCmp::from_mut(&mut self.render_list.spacepacks).clone_from_arc(&*spacepacks);
        }
        let map_id = match self.render_list.spacepacks.map_id {
            map_id if map_id != machine.is_ingame() =>
                None,
            map_id => map_id,
        };
        if let Some(map_id) = map_id {
            if packs_map.has_changed().unwrap_or(false) {
                let packs_map = packs_map.borrow_and_update();
                log::debug!("PATHY: gameplay maps rx @ {map_id}");
                if let Some(maps) = packs_map.get_ref(map_id) {
                    for (pack_path, pack) in self.pack_data.iter_mut() {
                        let Some((packmap_path, map_info)) = maps.get_info_for(pack_path) else {
                            log::debug!("PATHY: nothing for {pack_path}@{map_id:?}?");
                            pack.clear();
                            continue
                        };
                        let map_info = pack.map_info.insert(map_info.clone());

                        let poi_len = map_info.info.poi_count();
                        if pack.pois.len() != poi_len {
                            pack.pois.resize_with(poi_len, PoiRender::empty);
                        }

                        let trail_len = map_info.trail_count();
                        if pack.trails.len() != trail_len {
                            pack.trails.resize_with(trail_len, TrailRender::empty);
                        }

                        let map = maps.get_state(packmap_path);
                        if let Some(map) = map {
                            pack.map_state.clone_from(map);

                            let empty_trails = pack.trails.iter()
                                .filter(|(path, trail)| {
                                    if !trail.is_empty() { return false }
                                    if map_info.info.is_trail_info_loaded(*path) { return false }
                                    if let Some(ltrail) = map.trails().lookup_ref(path) {
                                        if !ltrail.visibility.is_visible() { return false }
                                    }
                                    true
                                });
                            for (ltrail_path, _trail) in empty_trails {
                                // schedule geometry load imminently
                                let ltrail_path = packmap_path.rel(ltrail_path.path);
                                self.draw_state.drawn_incomplete.insert(SpacePackShared::trail_geometry_id(&ltrail_path));
                            }
                        }


                        #[cfg(todo)]
                        for (poi, lpoi) in pack.pois.values_mut().zip(pack.map_state.loaded_pois(map_info)) {
                            poi.update(device, &pack.info, Some(lpoi))
                        }
                        #[cfg(todo)]
                        for (trail, ltrail) in pack.trails.values_mut().zip(pack.map_state.loaded_trails(map_info)) {
                            trail.update(device, &pack.info, Some(ltrail))
                        }
                    }
                }
            }
            for (ltrail_path, trail_incoming) in self.trail_rx.try_recv_fulfilled() {
                self.draw_state.drawn_incomplete.remove(&SpacePackShared::trail_geometry_id(&ltrail_path));
                match self.render_list.spacepacks.map_id {
                    Some(mid) if mid != ltrail_path.root.path => {
                        log::info!("received outdated geometry for {ltrail_path}?");
                        continue
                    },
                    _ => (),
                }
                let Some(pack_data) = self.pack_data.lookup_mut(&ltrail_path.root.root) else {
                    log::error!("received geometry for {ltrail_path} - unrecognized pack?");
                    continue
                };
                let path: LoadedTrailPath = ltrail_path.unscope();
                let Some(trail) = pack_data.trails.lookup_mut(&path) else {
                    log::error!("received geometry for {ltrail_path} - unrecognized trail?");
                    continue
                };
                let res = match trail_incoming {
                    geometry if geometry.is_empty() => None,
                    geometry => rt::log::error_ok(
                        trail.setup_geometry(device, geometry).context("loading trail geometry")
                    ),
                };
                if res.is_none() {
                    trail.disable();
                }
            }
            for (marker_path, texture) in self.texture_rx.try_recv_fulfilled() {
                use crate::exports::runtime::textures::TextureSlot;
                // texture loader should be notified, so no need to do anything really?
                let id = MarkerId::for_marker(marker_path);
                if texture.is_none() {
                    log::warn!("request for tex {marker_path} failed?");
                }
                self.draw_state.drawn_incomplete.remove(&id);
                let Some(pack_data) = self.pack_data.lookup_mut(&marker_path.root.root) else {
                    log::error!("received tex for {marker_path} - unrecognized pack?");
                    continue
                };
                match marker_path.path.variant() {
                    MarkerIndexVariant::Poi(poii) => {
                        let path: LoadedPoiPath = LoadedPoiPath::with_path(poii);
                        let Some(poi) = pack_data.pois.lookup_mut(&path) else {
                            log::error!("received tex for {marker_path} - unrecognized poi?");
                            continue
                        };
                        match texture {
                            Some(key) => {
                                poi.icon_handle = Some(key);
                                poi.icon = None;
                            },
                            None => {
                                poi.icon = Some(TextureSlot::Unavailable);
                            },
                        }
                    },
                    MarkerIndexVariant::Trail(traili) | MarkerIndexVariant::TrailSection(traili, _) => {
                        let path: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                        let Some(trail) = pack_data.trails.lookup_mut(&path) else {
                            log::error!("received tex for {marker_path} - unrecognized trail?");
                            continue
                        };
                        match texture {
                            Some(key) => {
                                trail.texture_handle = Some(key);
                                trail.texture = None;
                            },
                            None => {
                                trail.texture = Some(TextureSlot::Unavailable);
                            },
                        }
                    },
                    _ => {
                        log::error!("received tex for {marker_path} - invalid path?");
                    },
                }
            }
            let mut incomplete_trail_geometry = BTreeSet::new();
            let mut incomplete_textures = BTreeSet::new();
            for id in self.draw_state.drawn_incomplete.iter() {
                let Some(path) = id.marker_path::<PackMapPath>() else {
                    log::error!("invalid incomplete marker {id}??");
                    continue
                };
                let _pack_path = match path.root {
                    path if path.path != map_id => continue,
                    path => path.root,
                };
                let pack_data = self.pack_data.lookup_mut(&path.root.root);
                match path.path.variant() {
                    MarkerIndexVariant::Poi(poii) => {
                        let has_texture = pack_data.and_then(|pack_data| {
                            let lpath: LoadedPoiPath = LoadedPoiPath::with_path(poii);
                            let info = &pack_data.info;
                            match pack_data.pois.lookup_mut(&lpath) {
                                Some(poi) if poi.needs_texture_info() =>
                                    None,
                                poi => poi.map(move |r| (info, r)),
                            }
                        });
                        match has_texture {
                            Some((pack_info, poi)) => {
                                poi.update(device, pack_info, None);
                            },
                            None => {
                                incomplete_textures.insert(path);
                            },
                        }
                    },
                    MarkerIndexVariant::Trail(traili) => {
                        let has_texture = pack_data.and_then(|pack_data| {
                            let lpath: LoadedTrailPath = LoadedTrailPath::with_path(traili);
                            let info = &pack_data.info;
                            match pack_data.trails.lookup_mut(&lpath) {
                                Some(trail) if trail.needs_texture_info() =>
                                    None,
                                trail => trail.map(move |r| (info, r)),
                            }
                        });
                        match has_texture {
                            Some((pack_info, trail)) => {
                                trail.update(device, pack_info, None);
                            },
                            None => {
                                incomplete_textures.insert(path);
                            },
                        }
                    },
                    MarkerIndexVariant::TrailSection(traili, _sectioni) => {
                        // we load the whole thing at once so ignore sections...
                        let path = path.root.rel(traili);
                        incomplete_trail_geometry.insert(path);
                    },
                    _ => (),
                }
            }
            self.trail_rx.request_many(incomplete_trail_geometry);
            self.texture_rx.request_many(incomplete_textures);
        }
        self.draw_state.clear_active();
        let mut ibs_dirty = false;
        if self.pack_data.values().any(|p| p.render_poi_bookmarks().len() != p.pois.len()) {
            self.allocate_poi_buffers(1);
            ibs_dirty = true;
        }
        self.poi_common.update(device, machine, &self.pack_data)?;
        let ib_pack_len = self.poi_common.ib_len_for_packs(&self.pack_data);
        let ib_len = self.poi_common.ib_len();
        ibs_dirty |= ib_pack_len != ib_len;
        if self.poi_common.is_empty() || ibs_dirty {
            self.recreate_buffers(device, machine)?;
        }

        Ok(map_id.is_some())
    }
    fn recreate_buffers(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        let res = self
            .recreate_buffers_inner(device, machine)
            .context("preparing POI instance buffers");
        #[cfg(todo)]
        if res.is_err() {
            self.mark_buffers_dirty();
        }
        res
    }
    fn recreate_buffers_inner(
        &mut self,
        device: &Dx11Device,
        machine: &RenderMachine,
    ) -> anyhow::Result<()> {
        self.allocate_poi_buffers(1);
        self.poi_common.rebuild_ib(device, machine, &self.pack_data)?;

        Ok(())
    }

    /// offset (starting len) currently = 1 to leave space for an identity buffer
    /// at index 0 for drawing trails with
    ///
    /// also [PackRenderData::render_poi_bookmark] of 0 is treated as empty so uh don't
    /// use that
    pub fn allocate_poi_buffers(&mut self, mut offset: usize) -> usize {
        for pack in self.pack_data.values_mut() {
            pack.render_poi_bookmark = offset;
            let poi_count = match &pack.map_info {
                Some(map_info) => map_info.poi_count(),
                None => 0,
            };
            offset += poi_count;
        }
        offset
    }
    pub fn reset_poi_buffers(&mut self) {
        for pack in self.pack_data.values_mut() {
            pack.render_poi_bookmark = 0;
        }
    }

    pub fn draw(
        &mut self,
        camera: RenderPosition,
        frustum: &MapFrustum,
        backend: &RenderBackend,
        device_context: &Dx11Context,
    ) {
        let Some(spacepacks) = self.spacepacks.cached.as_ref() else { return };
        let entities = self.render_list.iter_markers_visible(
            self.pack_data.map_ref_as_slice(),
            frustum,
            camera,
        );
        #[cfg(deleteme)]
        let entities = self.render_list.iter_markers_all(
            self.pack_data.map_ref_as_slice(),
        );
        Self::draw_entities(
            &mut self.draw_state,
            &self.poi_common,
            device_context,
            backend,
            entities,
        );
        STATS_ENTITY_COUNT.reset_with(|| spacepacks.render_entities.entities.len() as _);
    }
    #[cfg(feature = "goggles")]
    pub fn draw_obscured(
        &mut self,
        camera: RenderPosition,
        frustum: &MapFrustum,
        backend: &RenderBackend,
        device_context: &Dx11Context,
    ) {
        let entities = self.render_list.iter_markers_visible(
            self.pack_data.map_ref_as_slice(),
            frustum,
            camera,
        );
        Self::draw_entities(
            &mut self.draw_state,
            &self.poi_common,
            device_context,
            backend,
            entities,
        );
    }

    pub fn draw_entities<'e, E>(
        draw_state: &mut PackRenderState,
        poi_common: &PoiCommonRenderData,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        entities: E,
    ) where
        E: IntoIterator<Item = (&'e PackRenderData, &'e MarkerId)>,
    {
        poi_common.set_primitive(device_context);

        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0usize;
        for (pack_data, marker_id) in entities {
            let render_id = marker_id.get_marker_index();
            match render_id.namespace() {
                MarkerIndex::NS_TRAIL => {
                    let path = {
                        let (t, s) = render_id.index_trail_section_unchecked();
                        LoadedTrailPath::with_path(t).rel(TrailSectionPath::with_path(s))
                    };
                    let trail = pack_data.trails.lookup_ref(&path.root).and_then(|trail|
                        pack_data.map_state.trails().lookup_ref(&path.root)
                        .map(|ltrail| (trail, ltrail))
                    );
                    let Some((trail, ltrail)) = trail else {
                        log::error!("Render ID refers to missing {path} in {}", pack_data.info);
                        continue
                    };
                    if !ltrail.visibility.is_visible_for_space() {
                        continue
                    }
                    if trail.report_incomplete(&marker_id, draw_state, path) {
                        continue
                    }
                    if shader_state != ShaderState::Trail {
                        shader_state = ShaderState::Trail;
                        backend.shaders.set_named(device_context, "trail");
                    }
                    trail.bind_texture(device_context, poi_common, LocalContext::World);
                    trail.draw_section(device_context, path.path, LocalContext::World);
                },
                MarkerIndex::NS_POI => {
                    let path = LoadedPoiPath::with_path(render_id.index_poi_unchecked());
                    let poi = (
                        pack_data.pois.lookup_ref(&path),
                        pack_data.map_state.pois().lookup_ref(&path),
                    );
                    let (Some(poi), Some(lpoi)) = poi else {
                        log::error!("Render ID refers to missing {path} in {}", pack_data.info);
                        continue
                    };
                    if !lpoi.visibility.is_visible_for_space() {
                        continue
                    }
                    if poi.report_incomplete(&marker_id, draw_state) {
                        continue
                    }
                    if shader_state != ShaderState::Poi {
                        shader_state = ShaderState::Poi;
                        poi_common.set(device_context);
                    }
                    poi.bind_texture(device_context, poi_common, LocalContext::World);
                    poi.draw(
                        device_context,
                        pack_data.render_poi_bookmark + path.path as usize,
                        LocalContext::World,
                    );
                },
                _ => {
                    log::error!("Render ID {render_id} refers to invalid marker {marker_id}");
                },
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW.reset(num_drawn as _);
    }
    pub fn draw_map_entities<'e, E>(
        draw_state: &mut PackRenderState,
        poi_common: &PoiCommonRenderData,
        device_context: &Dx11Context,
        backend: &RenderBackend,
        map: MapContext,
        entities: E,
    ) where
        E: IntoIterator<Item = (&'e PackRenderData, &'e MarkerId)>,
    {
        let mut shader_state = ShaderState::None;
        let mut num_drawn = 0usize;
        let ctx = LocalContext::/*Map(map)*/MAP;
        for (pack_data, marker_id) in entities {
            let render_id = marker_id.get_marker_index();
            match render_id.namespace() {
                MarkerIndex::NS_TRAIL => {
                    let path = {
                        let (t, s) = render_id.index_trail_section_unchecked();
                        LoadedTrailPath::with_path(t).rel(TrailSectionPath::with_path(s))
                    };
                    let trail = pack_data.trails.lookup_ref(&path.root).and_then(|trail|
                        pack_data.map_state.trails().lookup_ref(&path.root)
                        .map(|ltrail| (trail, ltrail))
                    );
                    let Some((trail, ltrail)) = trail else {
                        log::error!("Render ID refers to missing {path} in {}", pack_data.info);
                        continue
                    };
                    if !ltrail.visibility.is_visible_for_map(map) {
                        continue
                    }
                    if trail.report_incomplete(&marker_id, draw_state, path) {
                        continue
                    }
                    if shader_state == ShaderState::None {
                        backend.shaders.set_named(device_context, "map");
                        poi_common.set_primitive(device_context);
                        poi_common.set_instance(device_context, ctx);
                    }
                    if shader_state != ShaderState::Trail {
                        shader_state = ShaderState::Trail;
                    }
                    trail.bind_texture(device_context, poi_common, ctx);
                    trail.draw_section(device_context, path.path, ctx);
                },
                MarkerIndex::NS_POI => {
                    let path = LoadedPoiPath::with_path(render_id.index_poi_unchecked());
                    let poi = (
                        pack_data.pois.lookup_ref(&path),
                        pack_data.map_state.pois().lookup_ref(&path),
                    );
                    let (Some(poi), Some(lpoi)) = poi else {
                        log::error!("Render ID refers to missing {path} in {}", pack_data.info);
                        continue
                    };
                    if !lpoi.visibility.is_visible_for_map(map) {
                        continue
                    }
                    if poi.report_incomplete(&marker_id, draw_state) {
                        continue
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
                    poi.bind_texture(device_context, poi_common, ctx);
                    poi.draw(device_context, pack_data.render_poi_bookmark + path.path as usize, ctx);
                },
                _ => {
                    log::error!("Render ID {render_id} refers to invalid marker {marker_id}");
                },
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW_MAP.reset(num_drawn as _);
    }

    pub fn clear(&mut self) {
        self.clear_packs();
        self.draw_state.clear();
    }
    pub fn clear_packs(&mut self) {
        for pack in self.pack_data.values_mut() {
            pack.clear();
        }
    }
    pub fn stop(&mut self) {
        self.clear_packs();
        self.cleanup_textures();
        self.poi_common.clear();
    }
    /// See [crate::space::engine::Engine::cleanup_background]
    ///
    /// TODO: revisit, avoid, etc
    pub fn cleanup_background(self) {
        let Self { pack_data, poi_common, .. } = self;
        poi_common.cleanup_background();
        for pack in pack_data.data.into_iter() {
            pack.cleanup_background();
        }
    }

    pub fn cleanup_textures(&mut self) {
        let todo = ();
    }
}

#[derive(Debug, Default)]
pub struct PackRenderState {
    pub drawn_incomplete: FxHashSet<MarkerId>,
}
impl PackRenderState {
    pub fn clear(&mut self) {
        self.drawn_incomplete = Default::default();
    }
    pub fn clear_active(&mut self) {
        self.drawn_incomplete.clear();
    }
}

#[derive(Default)]
pub struct PackRenderList {
    spacepacks: Arc<SpacePackCollection>,
    draw_order_heap: render::RenderOrderHeap<usize>,
}
impl PackRenderList {
    /// adding some wiggle room around the map edges...
    pub fn map_bounds_to_query(
        _map: MapContext,
        mut bounds: Box3<DrawSpace>,
    ) -> aabb::Aabb<f32, 3> {
        let buffer = bounds.size() * 0.15;
        bounds.min.x -= buffer.width;
        bounds.min.z -= buffer.depth;
        bounds.max.x += buffer.width;
        bounds.max.z += buffer.depth;
        box3aabb(bounds)
    }
    /// TODO: filter by visibility flags here?
    pub fn iter_markers_map<'a, 'e, Q: aabb::IntersectsAabb<f32, 3>>(
        &'a self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
        _map: MapContext,
        query: &'a Q,
    ) -> impl Iterator<Item = (&'e PackRenderData, &'a MarkerId)> {
        self.spacepacks.bvh_traverse(query).filter_map(move |(_idx, id)| {
            let pack_path = id.get_marker_pack_path();
            pack_data.lookup_ref(&pack_path)
                .map(|p| (p, id))
        })
    }
    pub fn iter_markers_all<'a, 'e>(
        &'a self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
    ) -> impl Iterator<Item = (&'e PackRenderData, &'a MarkerId)> {
        let shapes = &self.spacepacks.render_entities.entities[..];
        let mut x = false;
        shapes.iter().filter_map(move |shape| {
            let id = &shape.value.id;
            let pack_path = id.get_marker_pack_path();
            let pack = pack_data.lookup_ref(&pack_path);
            if x == false && pack.is_none() {
                x = true;
                log::info!("PATHY: {pack_path} of {id} missing?");
            }
            pack
                .map(|p| (p, id))
        })
    }
    pub fn iter_markers_visible<'a, 'e, Q: aabb::IntersectsAabb<f32, 3>>(
        &'a mut self,
        pack_data: &'e IndexedList<PackRegistryNs, PackIndex, [PackRenderData]>,
        query: &'a Q,
        camera: RenderPosition,
    ) -> impl Iterator<Item = (&'e PackRenderData, &'a MarkerId)> {
        self.iter_entities_visible(query, camera).filter_map(|(_idx, id)| {
            let pack_path = id.get_marker_pack_path();
            pack_data.lookup_ref(&pack_path)
                .map(|p| (p, id))
        })
    }
    fn iter_entities_visible<'a, Q: aabb::IntersectsAabb<f32, 3>>(
        &'a mut self,
        query: &'a Q,
        (cam_origin, cam_dir, _cam_up): RenderPosition,
    ) -> impl Iterator<Item = (usize, &'a MarkerId)> + 'a {
        let shapes = &self.spacepacks.render_entities.entities[..];
        let extra = &self.spacepacks.render_entities.extra[..];
        self.draw_order_heap.clear();
        self.draw_order_heap.reserve(shapes.len() / 8);

        let bvh_iter = self.spacepacks.bvh_traverse(query).filter_map(move |(idx, _id)| {
            let ignore_draw_order = _id.get_marker_index().namespace() == MarkerIndex::NS_TRAIL;
            extra.get(idx).map(|extra| {
                let pos = match ignore_draw_order {
                    true => Point3::INFINITY,
                    false => extra.position,
                };
                (pos, idx)
            })
        });
        let ordered = render::RenderOrderBuilder {
            bvh_iter,
            cam_origin,
            cam_dir,
            draw_order_heap: &mut self.draw_order_heap,
        };
        let iter = ordered.map(move |idx| {
            let mid = unsafe {
                shapes.get_unchecked(idx)
            };
            (idx, &mid.value.id)
        });
        iter
    }
}

#[cfg(todo)]
impl PackCollection {

    pub fn disable_paths(&mut self, disabled_paths: &HashSet<String>) {
        let external = PathingController::external_filter_state();
        for (_pn, pack) in &mut self.loaded_packs {
            pack.disable_paths(&disabled_paths, external.as_ref());
        }
    }

    pub fn clear(&mut self) {
        self.loaded_packs.clear();
        self.unloaded_packs.clear();

        self.render_list.clear();
        self.poi_common.clear();
    }

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

    pub fn add_pack(&mut self, pack: Arc<Pack>, loader: LoaderBox) -> usize {
        let name = pack.name.clone();
        let active = ActivePack::new(pack, loader);
        let (idx, old) = self.loaded_packs.insert_full(name, active);
        if let Some(pack) = old {
            log::info!("Pack {} reloaded", pack.pack.name);
            if let Some(bookmark) = pack.render_list_bookmark {
                let end = bookmark + pack.active_pois.len() + pack.active_trails.len();
                for e in self
                    .render_list
                    .entities_mut()
                    .get_mut(bookmark..end)
                    .into_iter()
                    .flatten()
                {
                    e.disable();
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
        let res = pack
            .prepare_new_map(pack_idx, map_id, device, entities, &self.trail_params)
            .with_context(|| format!("loading pack {} for map {map_id}", pack.pack.name));
        if res.is_err() {
            log::info!(
                "pack {} failed to load for map {map_id}, disabling...",
                pack.pack.name
            );
            if let Some(bookmark) = pack.render_list_bookmark {
                let _ = entities.drain(bookmark..);
                /*for entity in &mut self.render_list.entities_mut()[bookmark..] {
                    entity.disable();
                }*/
            }
            pack.clear();
            pack.cleanup_textures();
        } else {
            let external = PathingController::external_filter_state();
            pack.recompute_enabled(external.as_ref());
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

    pub fn update(&mut self) {
        for (_, pack) in &mut self.loaded_packs {
            pack.update(&mut self.render_list);
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
        STATS_ENTITY_COUNT.reset_with(|| self.render_list.entities_count());
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
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ShaderState {
    None,
    Trail,
    Poi,
}

pub static STATS_ENTITY_DRAW: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_COUNT: Counter = Counter::DEFAULT;
pub static STATS_ENTITY_DRAW_MAP: Counter = Counter::DEFAULT;
