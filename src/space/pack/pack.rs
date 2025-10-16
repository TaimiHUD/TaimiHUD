use {
    anyhow::anyhow,
    crate::{
        controller::{
            Controller,
            ControllerEvent,
        },
        exports::runtime::{
            self as rt,
            imgui::{self, Ui, Condition, TreeNode},
        },
        render::{
            machine::{RenderMachine, RenderPosition},
            pathing_window::{PathingFilterState, PathingSearchState},
            RenderState,
        },
        space::{
            pack::{FestivalFixup, Pack, MarkerAttributesExt},
            dx11::{InstanceBufferData, RenderBackend},
            render_list::{MapFrustum, RenderEntity, RenderId, RenderList, RenderListBuilder},
            resources::Texture,
            DrawSpace,
            LocalContext, MapContext,
        },
        fl, with_i18n,
    },
    anyhow::Context,
    bitvec::vec::BitVec,
    glamour::Box3,
    indexmap::IndexMap,
    super::{
        poi::{ActivePoi, PoiCommonRenderData},
        trail::{ActiveTrail, TrailParams},
    },
    std::{
        collections::{HashSet, BTreeMap, BTreeSet},
        fs::{create_dir_all, read_dir},
        path::Path,
        sync::{atomic::{AtomicUsize, Ordering}, Arc},
    },
    taimi_d3d::dx11::{
        prelude::*,
        buffer::BufferOf,
    },
    taimi_pack::{
        attributes::{Festival, MarkerAttributes},
        loader::{DirectoryLoader, PackLoaderContext, ZipLoader},
        Category, Poi,
    },
    uuid::Uuid,
};

#[derive(Debug)]
pub enum UnloadedReason {
    Disabled,
    UnknownFormat,
    LoadingFailed(String),
}

pub type LoaderBox = Box<dyn PackLoaderContext + Send + 'static>;

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

impl ActivePack {
    pub fn new(pack: Arc<Pack>, loader: LoaderBox) -> Self {
        let enabled_categories: BitVec = pack.categories.all_categories.values()
            .map(|category| category.default_toggle)
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
                if actual_poi.attributes.copy_value.is_some() {
                    let actual_poi = actual_poi.clone();
                    current_pois.push(actual_poi);
                }
            }
        }
        current_pois
    }

    pub fn draw_categories(&mut self, ui: &Ui, filter_state: PathingFilterState, open_items: &mut HashSet<String>, recompute: &mut bool, search_state: &PathingSearchState) {
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
            Self::draw_category(ui,
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
            let Some((_, category)) = self.pack.categories.all_categories.get_index(leaf) else { continue 'leafies };
            let seps = category.full_id.rmatch_indices(".");
            'parents: for (idx, _) in seps {
                if let Some(parent) = category.full_id.get(..idx) {
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
    }

    pub fn draw_category(
        ui: &Ui,
        category: &Category,
        all_categories: &IndexMap<String, Category>,
        state: &mut BitVec,
        filter_state: PathingFilterState,
        open_items: &mut HashSet<String>,
        is_root: bool,
        recompute: &mut bool,
        search_state: &PathingSearchState,
        category_filter: Option<&BitVec>,
        copyable: (&BTreeSet<usize>, &BTreeSet<usize>, &[Poi]),
    ) {
        let push_token = ui.push_id(&category.full_id);
        if category.is_hidden {
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
                let is_branch_filter = is_branch && filter_state.contains(PathingFilterState::IgnoreBranches);
                let search_filter = search_state.matches_id(&category.full_id);
                let category_filter = category_filter.and_then(|f|
                    f.get(idx).map(|b| *b)
                ).unwrap_or(true);
                let filter = enabled_filter || disabled_filter || is_root_filter || is_leaf_filter || is_branch_filter;
                display = search_filter && category_filter && filter;
            }
        }
        if display {
            let is_copyable = match &category.marker_attributes.copy_value {
                Some(value) if category.sub_categories.is_empty() && !category.is_separator =>
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
                        Self::draw_tooltip_copyable(ui, &category.marker_attributes, Some(&category.display_name));
                    });
                }
                ui.unindent();
                ui.table_next_column();
                ui.table_next_column();
            } else {
                let mut unbuilt = TreeNode::new(&category.display_name)
                    .frame_padding(true)
                    .tree_push_on_open(false)
                    .opened(open_items.contains(&category.full_id), Condition::Always);
                if category.is_separator {
                    unbuilt = unbuilt.leaf(true);
                } else if category.sub_categories.is_empty() {
                    unbuilt = unbuilt.bullet(true);
                } else {
                    unbuilt = unbuilt.framed(true);
                }
                let tree_token = unbuilt.push(ui);
                if ui.is_item_hovered() && Self::category_has_tooltip(category) {
                    Self::draw_tooltip(ui, &category.display_name, || {
                        Self::draw_tooltip_category(ui, category);
                    });
                }
                if category.marker_attributes.copy_value.is_some() {
                    if with_i18n!("copy", |copy| ui.small_button(copy)) {
                        Self::copy_copyable(ui, &category.marker_attributes);
                    }
                    if ui.is_item_hovered() {
                        Self::draw_tooltip(ui, &category.display_name, || {
                            Self::draw_tooltip_copyable(ui, &category.marker_attributes, Some(&category.display_name));
                        });
                    }
                }
                if let Some(idx) = category_idx {
                    let (copyable_categories, copyable_pois, pois) = copyable;
                    if copyable_categories.contains(&idx) {
                        // TODO: revisit or remove once trigger radius and interaction is working
                        let pois = copyable_pois.iter()
                            .filter_map(|&poi_idx| pois.get(poi_idx))
                            //.filter(|poi| poi.category_idx == idx);
                            .filter(|poi| poi.category == category.full_id);
                        for (i, copyable) in pois.enumerate() {
                            if i % 4 != 3 {
                                ui.same_line();
                            }
                            let copied = match &copyable.attributes.tip_name {
                                Some(name) => ui.small_button(&fl!("copy-arg", arg = name)),
                                None => with_i18n!("copy", |copy| ui.small_button(copy)),
                            };
                            if copied {
                                Self::copy_copyable(ui, &copyable.attributes);
                            }
                            if ui.is_item_hovered() {
                                let template = copyable.attributes.tip_name.as_ref()
                                    .map(|n| &n[..])
                                    .unwrap_or("Generic Copyable Marker Name");
                                Self::draw_tooltip(ui, template, || {
                                    Self::draw_tooltip_poi(ui, &copyable.attributes);
                                    Self::draw_tooltip_copyable(ui, &copyable.attributes, None);
                                });
                            }
                        }
                    }
                }
                ui.table_next_column();
                if !category.is_separator {
                    if let Some(idx) = all_categories.get_index_of(&category.full_id) {
                        if let Some(mut substate) = state.get_mut(idx) {
                            if ui.checkbox("", &mut substate) {
                                *recompute = true;
                                Controller::try_send(ControllerEvent::PathingStateUpdate(category.full_id.clone(), *substate));
                            };
                        }
                    }
                }
                let mut internal_closure = || {
                    if !open_items.contains(&category.full_id) {
                        open_items.insert(category.full_id.clone());
                    }
                    if !category.sub_categories.is_empty() {
                        ui.indent(); //_by(1.0);
                    }
                    for (_local, global) in category.sub_categories.iter() {
                        Self::draw_category(ui,
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

    fn copy_copyable(ui: &Ui, attributes: &MarkerAttributes) {
        let Some(copy_value) = &attributes.copy_value else { return };
        ui.set_clipboard_text(copy_value);
        if let Some(copy_message) = &attributes.copy_message {
            let _ = rt::send_alert(ui, copy_message);
        }
    }

    fn draw_tooltip_category(ui: &Ui, category: &Category) {
        let desc = match &category.marker_attributes.tip_description {
            Some(desc) if !desc.is_empty() => Some(&desc[..]),
            _ => None,
        };
        let title = match &category.marker_attributes.tip_name {
            Some(title) if !title.is_empty() && !category.display_name.starts_with(title) =>
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
            ui.text(title);
        }
        if let Some(desc) = &attributes.tip_description {
            ui.text_wrapped(desc);
        }
    }

    fn category_has_tooltip(category: &Category) -> bool {
        match &category.marker_attributes.tip_description {
            Some(desc) if !desc.is_empty() =>
                return true,
            _ => (),
        }
        match &category.marker_attributes.tip_name {
            Some(title) if !title.is_empty() && !category.display_name.starts_with(title) =>
                return true,
            _ => (),
        }

        false
    }

    /// since these aren't intended to be displayed, there's no canon name to use...
    /// if it looks like more than just a location link, we'll try to preview it
    fn copyable_value_has_message(attributes: &MarkerAttributes) -> bool {
        let Some(copy_value) = attributes.copy_value.as_ref() else {
            return false
        };
        if !copy_value.starts_with('[') || !copy_value.ends_with(']') {
            return true
        }
        false
    }

    fn draw_tooltip<F: FnOnce()>(ui: &Ui, title_template: &str, f: F) {
        use imgui::StyleVar;

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
        let copy_message = attributes.copy_message.as_ref().map(|m| &m[..]);
        match &attributes.copy_value {
            Some(copy_value) if (display_name.is_none() || copy_message.is_none()) && Self::copyable_value_has_message(attributes) =>
                ui.text_wrapped(&format!("\"{copy_value}\"")),
            _ => (),
        }
        if let Some(copy_message) = copy_message {
            ui.text_wrapped(copy_message);
        }
    }

    pub fn disable_paths(&mut self, paths: &HashSet<String>, festivals: &BTreeSet<Festival>) {
        for path in paths {
            if let Some(idx) = self.pack.categories.all_categories.get_index_of(path) {
                if let Some(mut state) = self.user_category_state.get_mut(idx) {
                    *state = false;
                }
            }
        }
        self.recompute_enabled(festivals);
    }

    pub fn recompute_enabled(&mut self, festivals: &BTreeSet<Festival>) {
        let all = &self.pack.categories.all_categories;
        for root_category_id in &self.pack.categories.root_categories {
            if let Some(root) = all.get(root_category_id) {
                root.recompute_enabled(all, &mut self.enabled_categories, &self.user_category_state, true);
            }
        }
        for (i, (_, category)) in self.pack.categories.all_categories.iter().enumerate() {
            if category.marker_attributes.festivals.as_ref().map(|f| !f.iter().any(|f|
                festivals.contains(f)
            )).unwrap_or(false) {
                self.enabled_categories.set(i, false)
            }
        }
        // in response to update(...), moving update_filters down here where it should actually be
        // effective to save on useless loops
        self.update_filters();

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

    pub fn get_or_load_texture<'t>(
        &'t mut self,
        handle: PackTextureHandle,
        device: &Dx11Device,
    ) -> anyhow::Result<&'t Arc<Texture>> {
        let PackTextureHandle(idx) = handle;
        let (asset, slot) = self.texture_list.get_index_mut(idx)
            .ok_or_else(|| { anyhow!("Texture {} not in list at all", idx) })?;

        let texture = match slot {
            slot_texture@None => {
                let data = self.loader.load_asset_dyn(asset)?;
                let image = image::ImageReader::new(data)
                    .with_guessed_format().map_err(anyhow::Error::from)
                    .and_then(|image|
                        image.decode().map_err(Into::into)
                    ).with_context(|| "decoding {asset}")?
                    .into_rgba8()
                    .into_flat_samples();

                let texture = Texture::load_rgba8_uncached(device, image)
                    .with_context(|| format!("loading {asset}"))?;
                let texture = Arc::new(texture);
                let texture = slot_texture.insert(texture);
                self.loaded_textures.set(idx, true);
                texture
            }
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

        let trails = pack.trails.iter().enumerate()
            .filter(|(_, t)| t.data.map_id == map_id);
        for (i_trail, pack_trail, ..) in trails {
            if pack_trail.data.map_id != map_id {
                continue;
            }
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

            let category_idx = pack.categories.all_categories
                .get_index_of(&pack_trail.category)
                .unwrap_or(0);
            let trail = ActiveTrail::build(self, pack_trail, i_trail, category_idx, trail_params, render_entities.len(), device)
                .with_context(|| format!("Error loading trail {pack_trail}"));
            let trail = match trail {
                Ok(trail) => trail,
                Err(e) => {
                    log::warn!("{e:#}");
                    continue;
                }
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

            let category_idx = pack.categories.all_categories
                .get_index_of(&pack_poi.category)
                .unwrap_or(0);
            let poi = ActivePoi::build(self, pack_poi, i_poi, category_idx, device)
                .with_context(|| format!("Error loading POI {pack_poi}"));
            let poi = match poi {
                Ok(poi) => poi,
                Err(e) => {
                    log::warn!("{e:#}");
                    continue;
                }
            };

            if pack_poi.attributes.copy_value.is_some() {
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

    fn update_filters(&mut self) {
        for (_i, trail) in &mut self.active_trails {
            let enabled = self.enabled_categories.get(trail.category_idx)
                .map(|b| *b);
            if enabled.is_none() {
                log::error!("unknown category index {} for trail[{_i}] #{}", trail.category_idx, trail.trail_idx);
            }
            trail.filtered = !enabled.unwrap_or(true);
        }
        for (_i, poi) in &mut self.active_pois {
            let enabled = self.enabled_categories.get(poi.category_idx)
                .map(|b| *b);
            if enabled.is_none() {
                log::error!("unknown category index {} for poi[{_i}] #{}", poi.category_idx, poi.poi_idx);
            }
            poi.filtered = !enabled.unwrap_or(true);
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

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PackTextureHandle(usize);

pub struct PackCollection {
    pub loaded_packs: IndexMap<String, ActivePack>,
    pub unloaded_packs: IndexMap<String, UnloadedReason>,

    pub current_map: Option<i32>,
    pub render_list: RenderList,
    pub poi_common: PoiCommonRenderData,
    pub trail_params: TrailParams,

    festival_categories: BTreeMap<&'static str, Festival>,
    pub active_festivals: BTreeSet<Festival>,
}

impl PackCollection {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<PackCollection> {
        let poi_common = PoiCommonRenderData::new(backend)?;
        Ok(PackCollection {
            loaded_packs: IndexMap::new(),
            unloaded_packs: IndexMap::new(),
            current_map: None,
            render_list: RenderListBuilder::default().build(),
            trail_params: TrailParams::default(),
            poi_common,
            festival_categories: FestivalFixup::festival_categories(),
            active_festivals: Default::default(),
        })
    }

    pub fn disable_paths(&mut self, disabled_paths: HashSet<String>) {
        for (_pn, pack) in &mut self.loaded_packs {
            pack.disable_paths(&disabled_paths, &self.active_festivals);
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
                Some(e) if e.eq_ignore_ascii_case(b"taco") => {
                    ZipLoader::new(path).and_then(ActivePack::load)
                }
                _ => {
                    self.unloaded_packs
                        .insert(name.into(), UnloadedReason::UnknownFormat);
                    return;
                }
            }
        };
        let pack = match result {
            Ok(pack) => pack,
            Err(e) => {
                self.unloaded_packs
                    .insert(name.into(), UnloadedReason::LoadingFailed(format!("{e:?}")));
                return;
            }
        };
        self.loaded_packs.insert(name.into(), pack);
    }

    pub fn add_pack(&mut self, pack: Arc<Pack>, loader: LoaderBox) -> usize {
        let name = pack.name.clone();
        let mut active = ActivePack::new(pack, loader);
        self.fixup_pack(&mut active);
        let (idx, old) = self.loaded_packs.insert_full(name, active);
        if let Some(pack) = old {
            log::info!("Pack {} reloaded", pack.pack.name);
            if let Some(bookmark) = pack.render_list_bookmark {
                let end = bookmark + pack.active_pois.len() + pack.active_trails.len();
                for e in self.render_list.entities_mut().get_mut(bookmark..end).into_iter().flatten() {
                    e.disable();
                }
            }
            drop(pack);
        }
        idx
    }

    pub fn fixup_pack(&mut self, pack: &mut ActivePack) {
        for (_name, category) in &mut Arc::make_mut(&mut pack.pack).categories.all_categories {
            let is_festival = FestivalFixup::FESTIVAL_PREFIXES.iter().copied().find(|prefix|
                category.full_id.starts_with(prefix)
            );
            match is_festival {
                Some(prefix) if category.full_id != prefix =>
                    (),
                _ => continue,
            }
            let festival = self.festival_categories.iter().find_map(|(&prefix, &fest)|
                category.full_id.starts_with(prefix).then_some(fest)
            );
            if let Some(festival) = festival {
                Arc::make_mut(&mut category.marker_attributes).festivals = Some(vec![festival]);
            } else {
                log::info!("unrecognized festival category: `{}`", category.full_id);
            }
        }
    }

    pub fn load_pack(&mut self, device: &Dx11Device, pack_idx: usize) -> anyhow::Result<()> {
        let (_, pack) = self.loaded_packs.get_index_mut(pack_idx)
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

    fn build_active_pack(&mut self, pack_idx: usize, device: &Dx11Device, render_entities: Option<&mut Vec<RenderEntity>>, map_id: i32) -> anyhow::Result<()> {
        let (_, pack) = self.loaded_packs.get_index_mut(pack_idx)
            .with_context(|| format!("unrecognized pack index {pack_idx}"))?;

        let (entities, inplace) = match render_entities {
            Some(e) => (e, false),
            None => (self.render_list.entities_mut(), true),
        };
        let res = pack.prepare_new_map(pack_idx, map_id, device, entities, &self.trail_params)
            .with_context(|| format!("loading pack {} for map {map_id}", pack.pack.name));
        if res.is_err() {
            log::info!("pack {} failed to load for map {map_id}, disabling...", pack.pack.name);
            if let Some(bookmark) = pack.render_list_bookmark {
                let _ = entities.drain(bookmark..);
                /*for entity in &mut self.render_list.entities_mut()[bookmark..] {
                    entity.disable();
                }*/
            }
            pack.clear();
            pack.cleanup_textures();
        } else {
            pack.recompute_enabled(&self.active_festivals);
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
            let pack_res = self.build_active_pack(pack_idx, device, Some(&mut render_builder.entities), map_id)
                .with_context(|| format!("Pack {} failed to load", self.loaded_packs.get_index(pack_idx).map(|(_, p)| &p.pack.name[..]).unwrap_or("<badidx>")));
            if let Err(e) = pack_res {
                log::warn!("{e:#}");
                let _ = res.get_or_insert(e);
            } else {
                succ = true;
            }
        }
        match res {
            Some(e) if !succ =>
                return Err(e.into()),
            _ => (),
        }

        log::info!(
            "Loaded {} trails and {} POIs",
            self.loaded_packs.values().map(|p| p.active_trails.len()).sum::<usize>(),
            self.loaded_packs.values().map(|p| p.active_pois.len()).sum::<usize>(),
        );


        //let res = self.recreate_buffers(device);
        self.mark_buffers_dirty();
        let res = Ok(());

        self.render_list = render_builder.build();

        res
    }

    fn recreate_buffers_inner(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        // identity at start for trail drawing
        let mut data_world = vec![InstanceBufferData::IDENTITY; 1];
        let mut data_map = vec![InstanceBufferData::IDENTITY; 1];

        let mut render_poi_bookmark = 1;
        for pack in self.loaded_packs.values_mut() {
            data_world.extend(pack.active_pois.values()
                .map(|poi| poi.instance_data())
            );
            data_map.extend(pack.active_pois.values()
                .map(|poi| poi.instance_data_map(machine))
            );
            pack.render_poi_bookmark = render_poi_bookmark;
            render_poi_bookmark += pack.active_pois.len();
        }
        let (poi_ib_world, poi_ib_map) = (
                Some(BufferOf::new_with_data(device, Ok(&data_world[..]), ())?),
                Some(BufferOf::new_with_data(device, Ok(&data_map[..]), ())?),
        );
        self.poi_common.world_ib = poi_ib_world;
        self.poi_common.map_ib = poi_ib_map;

        Ok(())
    }

    fn recreate_buffers(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        let res = self.recreate_buffers_inner(device, machine)
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

    pub fn prepare(&mut self, device: &Dx11Device, machine: &RenderMachine) -> anyhow::Result<()> {
        if /* !self.loaded_packs.is_empty() &&*/ self.poi_common.is_empty() {
            self.recreate_buffers(device, machine)?;
        }

        Ok(())
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
        let entities = self
            .render_list
            .get_entities_for_drawing(camera, frustum);
        Self::draw_entities(&self.loaded_packs, &self.poi_common, device_context, backend, entities);
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
        let mut num_drawn = 0usize;
        for entity in entities {
            let render_id = match entity.render_id {
                Some(id) => id,
                None => continue,
            };
            match render_id {
                RenderId::TrailSection {
                    pack_idx,
                    trail_idx,
                    section,
                } => {
                    let trail = loaded_packs.get_index(pack_idx)
                        .and_then(|(_, pack)| pack.active_trails.get_index(trail_idx)
                            .and_then(|(_, trail)| pack.pack.trails.get(trail.trail_idx)
                                .map(|info| (trail, info))
                            )
                        );
                    let (trail, info) = match trail {
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing trail#{trail_idx} pack#{pack_idx} section#{section}");
                            continue
                        },
                    };
                    if trail.filtered || !info.attributes.in_game_visibility.unwrap_or(true) {
                        continue
                    }
                    if shader_state != ShaderState::Trail {
                        shader_state = ShaderState::Trail;
                        backend.shaders.set_named(device_context, "trail");
                    }
                    trail.draw_section(device_context, section, LocalContext::World);
                }
                RenderId::Poi { pack_idx, poi_idx } => {
                    let poi = loaded_packs.get_index(pack_idx)
                        .and_then(|(_, pack)| pack.active_pois.get_index(poi_idx)
                            .and_then(|(_, poi)| pack.pack.pois.get(poi.poi_idx)
                                .map(|info| (pack, poi, info))
                            )
                        );
                    let (pack, poi, info) = match poi {
                        Some((pack, _, _)) if pack.render_poi_bookmark == 0 =>
                            continue,
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing trail#{poi_idx} pack#{pack_idx}");
                            continue
                        },
                    };
                    if poi.filtered || !info.attributes.in_game_visibility.unwrap_or(true) {
                        continue
                    }
                    if shader_state != ShaderState::Poi {
                        shader_state = ShaderState::Poi;
                        poi_common.set(device_context);
                    }
                    poi.draw(device_context, pack.render_poi_bookmark + poi_idx, LocalContext::World);
                }
            }
            num_drawn += 1;
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
        let ctx = LocalContext::/*Map(map)*/MAP;
        for entity in entities {
            let render_id = match entity.render_id {
                Some(id) => id,
                None => continue,
            };
            match render_id {
                RenderId::TrailSection {
                    pack_idx,
                    trail_idx,
                    section,
                } => {
                    let trail = loaded_packs.get_index(pack_idx)
                        .and_then(|(_, pack)| pack.active_trails.get_index(trail_idx)
                            .and_then(|(_, trail)| pack.pack.trails.get(trail.trail_idx)
                                .map(|info| (trail, info))
                            )
                        );
                    let (trail, info) = match trail {
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing trail#{trail_idx} pack#{pack_idx} section#{section}");
                            continue
                        },
                    };
                    if trail.filtered || !info.attributes.is_visible_for_map(map) {
                        continue
                    }
                    let scale = info.attributes.scale_on_map_with_zoom;
                    if scale == Some(false) {
                        // idk invert .-.
                    }
                    if shader_state == ShaderState::None {
                        backend.shaders.set_named(device_context, "map");
                        poi_common.set_primitive(device_context);
                        poi_common.set_instance(device_context, ctx);
                    }
                    if shader_state != ShaderState::Trail {
                    }
                    shader_state = ShaderState::Trail;
                    trail.draw_section(device_context, section, ctx);
                }
                RenderId::Poi { pack_idx, poi_idx } => {
                    let poi = loaded_packs.get_index(pack_idx)
                        .and_then(|(_, pack)| pack.active_pois.get_index(poi_idx)
                            .and_then(|(_, poi)| pack.pack.pois.get(poi.poi_idx)
                                .map(|info| (pack, poi, info))
                            )
                        );
                    let (pack, poi, info) = match poi {
                        Some((pack, _, _)) if pack.render_poi_bookmark == 0 =>
                            continue,
                        Some(t) => t,
                        None => {
                            log::error!("Render ID refers to missing trail#{poi_idx} pack#{pack_idx}");
                            continue
                        },
                    };
                    if poi.filtered || !info.attributes.is_visible_for_map(map) {
                        continue
                    }
                    let scale = info.attributes.scale_on_map_with_zoom;
                    if scale == Some(false) {
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
                }
            }
            num_drawn += 1;
        }
        STATS_ENTITY_DRAW_MAP.store(num_drawn, Ordering::Relaxed);
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

    pub fn load_map(&mut self, device: &Dx11Device, _device_context: &Dx11Context, map_id: u32) -> anyhow::Result<()> {
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
