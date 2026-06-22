use {
    crate::{
        controller::pathing::{
            registry::{LoadedCategoryPath, LoadedMarkerPath, LoadedPoiPath, LoadedTrailPath},
            PathingController,
        },
        exports::runtime as rt,
    },
    anyhow::Context,
    core::cell::LazyCell,
    std::sync::{Arc, LazyLock},
    taimi_hoard::loc::{LocationMut, LocationRef, Locator},
    taimi_meta::packs::{
        CategoryIndex,
        CategoryPath,
        MapIndex,
        MarkerId,
        MarkerIndex,
        MarkerPath,
        PackMapPath,
        PackPath,
        PoiPath,
        VisibilityFlags,
    },
    taimi_pack::{
        attributes::{
            cell::{pack_attr, GetAttrDynExt, PackValueCell, PackValueOf, SetAttrDyn},
            keys,
        },
        category::id::IdCmpRelaxed,
        trail::TrlPath,
    },
    taimi_sync::arcs::ArcPtrCmp,
};

#[cfg(feature = "paths-interact")]
use crate::{
    controller::pathing::{interact::SpaceInteraction, state::interactive::InteractionEvent},
    render::RenderEvent,
};

impl PathingController {
    pub(super) async fn commit_attr_changes(
        &mut self,
        marker: MarkerId,
        write_attrs: &mut (dyn Iterator<Item = PackValueCell> + Send),
    ) {
        let lpath = if let Some(path) = marker.marker_path::<PackPath>() {
            Ok(self
                .gameplay_map()
                .and_then(|map_id| self.map_info.find_marker_index(path, Locator::new_path(map_id)))
                .map(|lp| (Some(path), lp)))
        } else {
            marker
                .marker_path::<PackMapPath>()
                .with_context(|| format!("{marker} not a pack marker path"))
                .map(|lp| Some((None, lp)))
        };
        let Some(Some((path, lpath))) = rt::log::warn_ok(lpath) else { return };
        let (vis_dirty,) = self.commit_attr_keys(lpath, path, write_attrs).await;
        if vis_dirty {
            self.debug_req_config_vis(Some(lpath.root.root), true, None).await;
        }
    }
    async fn commit_attr_keys(
        &mut self,
        lpath: LoadedMarkerPath<PackMapPath>,
        path: Option<MarkerPath<PackPath>>,
        write_attrs: &mut (dyn Iterator<Item = PackValueCell> + Send),
    ) -> (bool,) {
        let info = self.packs.lookup_info(lpath.root.root);
        let Some(map) = self.maps.maps.get_mut(&lpath.root) else {
            return Default::default()
        };

        let map_info = LazyCell::new({
            let map_info = &self.map_info;
            move || map_info.lookup_ref(&lpath.root)
        });
        let fallback_cat = LazyCell::new({
            let map_info = &map_info;
            move || {
                let root_idx: Option<CategoryPath> =
                    info.and_then(|(i, ..)| i.primary_root().map(|r| CategoryPath::new_path(r.index)));
                root_idx.or_else(|| map_info.and_then(|map_info| map_info.categories().next()))
            }
        });

        let (mut poi, mut trail) = (None, None);
        match lpath.path.namespace() {
            MarkerIndex::NS_POI => {
                let poi = poi.insert(
                    match map.pois.get_mut(lpath.path.index_poi_unchecked() as usize) {
                        Some(p) => p,
                        None => return Default::default(),
                    },
                );
                if poi.info.category_path.path == CategoryIndex::MAX {
                    poi.visibility = VisibilityFlags::all();
                    if let Some(root) = *fallback_cat {
                        poi.info.marker_info.category_path = root;
                        #[cfg(todo)]
                        {
                            poi.visibility = map_info
                                .and_then(|i| {
                                    i.category_index(root)
                                        .and_then(|r| map.categories.get(r.path as usize))
                                })
                                .map(|cat| cat.visibility)
                                .unwrap_or(VisibilityFlags::all());
                        }
                    }
                }
            },
            MarkerIndex::NS_TRAIL => {
                let trail = trail.insert(
                    match map.trails.get_mut(lpath.path.trail_index_unchecked() as usize) {
                        Some(p) => p,
                        None => return Default::default(),
                    },
                );
                if trail.info.category_path.path == CategoryIndex::MAX {
                    trail.visibility = VisibilityFlags::all();
                    if let Some(root) = *fallback_cat {
                        trail.info.marker_info.category_path = root;
                        #[cfg(todo)]
                        {
                            trail.visibility = map_info
                                .and_then(|i| {
                                    i.category_index(root)
                                        .and_then(|r| map.categories.get(r.path as usize))
                                })
                                .map(|cat| cat.visibility)
                                .unwrap_or(VisibilityFlags::all());
                        }
                    }
                }
            },
            _ => (),
        };
        let mut cat = LazyCell::new({
            let cats = &mut map.categories;
            let cat_idx = lpath.path.index_category_unchecked() as usize;
            move || match lpath.path.namespace() {
                MarkerIndex::NS_CAT if cats.len() > cat_idx =>
                    Some(unsafe { Arc::make_mut(cats).get_unchecked_mut(cat_idx) }),
                _ => None,
            }
        });
        let cat_lookup = LazyCell::new({
            let shared = &self.rx.shared;
            move || {
                let load = shared
                    .packs
                    .packs
                    .borrow()
                    .lookup_ref(&lpath.root.root)
                    .map(|pack| pack.loaded.clone());
                load.and_then(|l| l.borrow().pack.clone())
            }
        });

        #[cfg(todo = "unnecessary")]
        let changes = write_attrs.map(|v| (v.id(), PackValueDyn::from_cell_dyn(v)));
        let changes = write_attrs.map(|v| (v.id(), v));
        let (mut dirty, mut vis_dirty) = (false, false);
        let is_static = {
            // TODO
            true
        };
        #[cfg(feature = "space")]
        let (mut render_changed, mut render_changed_trailvb, mut trail_invalidate) = (false, false, false);
        #[cfg(feature = "paths-interact")]
        let (mut info_invalidate, mut interact_invalidate) = (false, false);
        let mut new_guid = None;
        for (key, value) in changes {
            #[cfg(all(taimi_debug, todo))]
            log::debug!("- {key}={}", if value.is_valid() { "Some" } else { "None" });
            #[cfg(feature = "space")]
            match lpath.path.namespace() {
                MarkerIndex::NS_POI if crate::space::pack::PoiRender::attr_dirties_render(key) =>
                    render_changed = true,
                MarkerIndex::NS_TRAIL if crate::space::pack::TrailRender::attr_dirties_vb(key) =>
                    render_changed_trailvb = true,
                MarkerIndex::NS_TRAIL if crate::space::pack::TrailRender::attr_dirties_render(key) =>
                    render_changed = true,
                _ => (),
            }
            let applied = pack_attr! { match =id_is(key) {
                = keys::CategoryRef => Some({
                    let path = poi.as_mut().map(|poi|
                        &mut poi.info.marker_info.category_path
                    ).or(trail.as_mut().map(|trail|
                        &mut trail.info.marker_info.category_path
                    ));
                    let id = PackValueOf::<keys::CategoryRef>::from_cell_ref(&value)
                        .map(|id| &id.get()[..]);
                    let cats = cat_lookup.as_ref().map(|c| &c.categories);
                    if let (Some(dest), Some(q), Some(cats)) = (path, id, cats) {
                        let found = cats.all_categories.get_index_of(q)
                            .or_else(|| cats.all_categories.keys().position(|id| IdCmpRelaxed::with_ref(id).eq_with(&q[..])))
                            .map(|idx| CategoryPath::new_path(idx as CategoryIndex));
                        if let Some(p) = found {
                            if dest.path == CategoryIndex::MAX {
                                //trail.visibility = VisibilityFlags::all();
                            }
                            *dest = p;
                            true
                        } else {
                            false
                        }
                    } else {
                        // TODO?
                        false
                    }
                }),
                = keys::DefaultToggle => {
                    let vis = poi.as_mut().map(|poi|
                        &mut poi.visibility
                    ).or(trail.as_mut().map(|trail|
                        &mut trail.visibility
                    )).or(cat.as_mut().map(|cat|
                        &mut cat.visibility
                    ));
                    let on = PackValueOf::<keys::DefaultToggle>::from_cell_ref(&value)
                        .map(|v| *v.get())
                        .unwrap_or_default();
                    Some(if let Some(vis) = vis {
                        vis.set(VisibilityFlags::TOGGLES, on.into());
                        true
                    } else { false })
                },
                = keys::InGameVisibility => {
                    let vis = poi.as_mut().map(|poi|
                        &mut poi.visibility
                    ).or(trail.as_mut().map(|trail|
                        &mut trail.visibility
                    )).or(cat.as_mut().map(|cat|
                        &mut cat.visibility
                    ));
                    let on = PackValueOf::<keys::InGameVisibility>::from_cell_ref(&value)
                        .map(|v| *v.get())
                        .unwrap_or_default();
                    if let Some(vis) = vis {
                        vis.set(VisibilityFlags::TOGGLE_SPACE | VisibilityFlags::DEFAULT_SPACE, on.into());
                        dirty = true;
                        if is_static && lpath.path.namespace() == MarkerIndex::NS_CAT {
                            vis_dirty = true;
                        }
                    }
                    #[cfg(feature = "paths-interact")]
                    if let Some(poi) = &poi {
                        if SpaceInteraction::is_interactive(&*poi) {
                            interact_invalidate = true;
                        }
                    }
                    None
                },
                = keys::MinimapVisibility => {
                    let vis = poi.as_mut().map(|poi|
                        &mut poi.visibility
                    ).or(trail.as_mut().map(|trail|
                        &mut trail.visibility
                    )).or(cat.as_mut().map(|cat|
                        &mut cat.visibility
                    ));
                    let on = PackValueOf::<keys::MinimapVisibility>::from_cell_ref(&value)
                        .map(|v| *v.get())
                        .unwrap_or_default();
                    if let Some(vis) = vis {
                        vis.set(VisibilityFlags::TOGGLE_MINIMAP | VisibilityFlags::DEFAULT_MINIMAP, on.into());
                        dirty = true;
                        if is_static && lpath.path.namespace() == MarkerIndex::NS_CAT {
                            vis_dirty = true;
                        }
                    }
                    None
                },
                = keys::MapVisibility => {
                    let vis = poi.as_mut().map(|poi|
                        &mut poi.visibility
                    ).or(trail.as_mut().map(|trail|
                        &mut trail.visibility
                    )).or(cat.as_mut().map(|cat|
                        &mut cat.visibility
                    ));
                    let on = PackValueOf::<keys::MapVisibility>::from_cell_ref(&value)
                        .map(|v| *v.get())
                        .unwrap_or_default();
                    if let Some(vis) = vis {
                        vis.set(VisibilityFlags::TOGGLE_GLOBAL | VisibilityFlags::DEFAULT_GLOBAL, on.into());
                        dirty = true;
                        if is_static && lpath.path.namespace() == MarkerIndex::NS_CAT {
                            vis_dirty = true;
                        }
                    }
                    None
                },
                = keys::Guid => {
                    new_guid = PackValueOf::<keys::Guid>::from_cell_ref(&value).map(|v| v.get().clone());
                    None
                },
                = keys::PositionX => Some({
                    if let Some(poi) = &mut poi {
                        #[cfg(feature = "paths-interact")]
                        if SpaceInteraction::is_interactive(&*poi) {
                            interact_invalidate = true;
                        }
                        poi.marker_position.x = PackValueOf::<keys::PositionX>::from_cell_ref(&value)
                            .map(|v| f32::from(*v.get()))
                            .unwrap_or_default();
                        true
                    } else { false }
                }),
                = keys::PositionY => Some({
                    if let Some(poi) = &mut poi {
                        #[cfg(feature = "paths-interact")]
                        if SpaceInteraction::is_interactive(&*poi) {
                            interact_invalidate = true;
                        }
                        poi.marker_position.y = PackValueOf::<keys::PositionY>::from_cell_ref(&value)
                            .map(|v| f32::from(*v.get()))
                            .unwrap_or_default();
                        true
                    } else { false }
                }),
                = keys::PositionZ => Some({
                    if let Some(poi) = &mut poi {
                        #[cfg(feature = "paths-interact")]
                        if SpaceInteraction::is_interactive(&*poi) {
                            interact_invalidate = true;
                        }
                        poi.marker_position.z = PackValueOf::<keys::PositionZ>::from_cell_ref(&value)
                            .map(|v| f32::from(*v.get()))
                            .unwrap_or_default();
                        true
                    } else { false }
                }),
                = keys::IconFile => {
                    // TODO: if lpath.path.namespace() == MarkerIndex::NS_POI?
                    let next = PackValueOf::<keys::IconFile>::from_cell_ref(&value).map(|t| t.get());
                    let info = poi.as_ref().map(|t| &t.info().marker_info)
                        .or_else(|| trail.as_ref().map(|p| &p.info().marker_info));
                    let prev = info.and_then(|i| i.get_attr_of::<keys::IconFile>());
                    let no_op = next.map(|t| &t[..]) == prev.as_ref().map(|t| &t[..]);
                    if !no_op {
                        self.space.invalidate_texture(lpath);
                    }
                    None
                },
                = keys::TextureFile => {
                    // TODO: if lpath.path.namespace() == MarkerIndex::NS_TRAIL?
                    let next = PackValueOf::<keys::TextureFile>::from_cell_ref(&value).map(|t| t.get());
                    let info = trail.as_ref().map(|t| &t.info().marker_info)
                        .or_else(|| poi.as_ref().map(|p| &p.info().marker_info));
                    let prev = info.and_then(|i| i.get_attr_of::<keys::TextureFile>());
                    let no_op = next.map(|t| &t[..]) == prev.as_ref().map(|t| &t[..]);
                    if !no_op {
                        self.space.invalidate_texture(lpath);
                    }
                    None
                },
                = keys::TrailDataFile => {
                    let trl = PackValueOf::<keys::TrailDataFile>::from_cell_ref(&value).map(|t| t.get().clone());
                    if let Some(trail) = &mut trail {
                        match (&mut trail.info.trl, trl) {
                            (Some(trl), Some(v)) =>
                                trl.path = v.into(),
                            (trl, v) =>
                                *trl = v.map(|v| TrlPath::new(v.into())),
                        }
                        // TODO: propagate to shared info struct...
                        trail_invalidate = true;
                        Some(true)
                    } else {
                        None
                    }
                },
                = keys::ScriptTrigger => {
                    #[cfg(todo)] {
                        interact_invalidate = true;
                    }
                    None
                },
                = keys::ScriptFocus => {
                    #[cfg(todo)] {
                        interact_invalidate = true;
                    }
                    None
                },
                = keys::TriggerRange => {
                    #[cfg(feature = "paths-interact")]
                    {
                        interact_invalidate = true;
                    }
                    None
                },
                = keys::InfoRange => {
                    #[cfg(feature = "paths-interact")]
                    {
                        interact_invalidate = true;
                    }
                    None
                },
                = keys::Info => {
                    #[cfg(feature = "paths-interact")]
                    {
                        info_invalidate = true;
                    }
                    None
                },
                = keys::AutoTrigger => {
                    // TODO: if previously non-auto and nearby, initiate trigger?
                    #[cfg(feature = "paths-interact")]
                    {
                        info_invalidate = true;
                    }
                    None
                },
                = keys::CopyValue => {
                    #[cfg(feature = "paths-interact")]
                    {
                        info_invalidate = true;
                    }
                    None
                },
                = keys::CopyMessage => {
                    #[cfg(feature = "paths-interact")]
                    {
                        info_invalidate = true;
                    }
                    None
                },
                = keys::TipName => {
                    #[cfg(feature = "paths-interact")]
                    {
                        info_invalidate = true;
                    }
                    None
                },
                = keys::TipDescription => {
                    #[cfg(feature = "paths-interact")]
                    {
                        info_invalidate = true;
                    }
                    None
                },
                _ => None,
            } };
            if let Some(applied) = applied {
                dirty |= applied;
                continue
            }
            let info = poi
                .as_mut()
                .map(|poi| &mut poi.info as &mut dyn SetAttrDyn)
                .or(trail.as_mut().map(|trail| &mut trail.info as &mut dyn SetAttrDyn))
                .or(cat.as_mut().map(|cat| &mut cat.attrs as &mut dyn SetAttrDyn));
            let Some(info) = info else { continue };
            #[cfg(todo = "unnecessary")]
            let value = value
                .map(|v| v.into_inner())
                .unwrap_or_else(|| PackValueCell::new_empty(key));
            dirty |= info.set_attr_dyn(value);
        }
        let (mut info_needs_update, mut map_needs_update) = (false, false);
        #[cfg(feature = "paths-interact")]
        if let (true, Some(path), Some(poi)) = (info_invalidate, path, &poi) {
            let lpoipath = lpath.map_path(|p| p.index_poi_unchecked());
            if !poi.visibility.is_visible() {
                // shrug?
            } else if self.rx.interact.nearby_tx.borrow().contains_loaded_poi(lpoipath) {
                super::InteractMessage::RefreshInfo(lpath, path.unscope()).try_send();
            } else {
                super::InteractMessage::RefreshNearby.try_send();
            }
        }
        #[cfg(feature = "paths-interact")]
        if interact_invalidate {
            super::InteractMessage::RefreshNearby.try_send();
        }
        if trail_invalidate {
            let ltp = lpath.map_path(|p| LoadedTrailPath::new_path(p.trail_index_unchecked()));
            self.space
                .invalidate_trail_geometry(ltp, self.map_info.lookup_mut(&lpath.root));
            info_needs_update = true;
        }
        let guids = match lpath.path.namespace() {
            _ if new_guid.is_none() => None,
            MarkerIndex::NS_POI => Some(&mut map.poi_guids),
            MarkerIndex::NS_TRAIL => Some(&mut map.trail_guids),
            _ => None,
        };
        if let (Some(new_guid), Some(dest)) = (new_guid, guids) {
            let lidx = match lpath.path.namespace() {
                MarkerIndex::NS_TRAIL => lpath.path.index_trail_unchecked() as usize,
                _ => lpath.path.index() as usize,
            };
            let prev = dest.get(lidx).map(|prev| *prev == new_guid);
            if prev != Some(true) {
                let mut guids = dest[..].to_owned();
                let resize = match guids.get_mut(lidx) {
                    Some(dest) => {
                        *dest = new_guid;
                        false
                    },
                    None => !new_guid.is_empty(),
                };
                if resize {
                    guids.reserve_exact(lidx + 1);
                    guids.resize(lidx, keys::Guid::EMPTY);
                    guids.push(new_guid);
                }

                *dest = guids.into_iter().collect();
                dirty = true;
            }
        }
        if dirty | info_needs_update {
            self.rx.shared.gameplay.send_if_modified(move |gp| {
                if gp.map_id != Some(lpath.root.path) {
                    return false
                }
                let (Some(map_info), shared_map) = gp.for_pack_mut(lpath.root.root) else {
                    return false
                };
                #[cfg(todo)]
                {
                    let info = Arc::make_mut(&mut map_info.info);
                    info.info_sig.hash = info.info_sig.hash.wrapping_add(1);
                }
                match lpath.path.namespace() {
                    _ if !dirty => (),
                    #[cfg(todo)]
                    MarkerIndex::NS_CAT => {
                        let info = Arc::make_mut(&mut map_info.info);
                        #[cfg(todo = "unnecessary")]
                        {
                            //info.categories = cats.into_boxed_slice();
                        }
                        info.info_sig.hash = info.info_sig.hash.wrapping_add(1);
                    },
                    MarkerIndex::NS_CAT => (),
                    MarkerIndex::NS_POI => {
                        let poi =
                            unsafe { map.pois.get_unchecked(lpath.path.index_poi_unchecked() as usize) };
                        // TODO: confirm index exists *before* make_mut
                        let pois = Arc::make_mut(&mut map_info.pois.data);
                        if let Some(dest) = pois.get_mut(lpath.path.index_poi_unchecked() as usize) {
                            *dest = poi.info().clone();
                        } else {
                            info_needs_update = true;
                        }
                        let map_poi = shared_map
                            .as_mut()
                            .map(|map| Arc::make_mut(&mut map.pois.data))
                            .and_then(|map_pois| {
                                map_pois.get_mut(lpath.path.index_poi_unchecked() as usize)
                            });
                        if let Some(dest) = map_poi {
                            *dest = super::shared::LoadedPoiShared::with_loaded(poi);
                        } else {
                            map_needs_update = true;
                        }
                    },
                    MarkerIndex::NS_TRAIL => {
                        let trail = unsafe {
                            map.trails
                                .get_unchecked(lpath.path.trail_index_unchecked() as usize)
                        };
                        let trails = Arc::make_mut(&mut map_info.trails.data);
                        if let Some(dest) = trails.get_mut(lpath.path.trail_index_unchecked() as usize) {
                            *dest = trail.info().clone();
                        } else {
                            info_needs_update = true;
                        }
                        let map_trail = shared_map
                            .as_mut()
                            .map(|map| Arc::make_mut(&mut map.trails.data))
                            .and_then(|map_trail| {
                                map_trail.get_mut(lpath.path.trail_index_unchecked() as usize)
                            });
                        if let Some(dest) = map_trail {
                            *dest = super::shared::LoadedTrailShared::with_loaded(trail);
                        } else {
                            map_needs_update = true;
                        }
                    },
                    // we know which index changed, no need to recreate the whole thing...
                    #[cfg(todo = "unnecessary")]
                    MarkerIndex::NS_POI | MarkerIndex::NS_TRAIL => {
                        info_needs_update = true;
                        map_needs_update = true;
                    },
                    _ => (),
                };
                if info_needs_update {
                    match lpath.path.namespace() {
                        MarkerIndex::NS_POI => {
                            map_info.write_with_loaded_pois(map);
                        },
                        MarkerIndex::NS_TRAIL => {
                            map_info.write_with_loaded_trails(map);
                        },
                        _ => (),
                    }
                } else {
                    ArcPtrCmp::from_mut(&mut map_info.poi_guids).clone_from_arc(&map.poi_guids);
                }
                if let Some(shared_map) = shared_map {
                    match lpath.path.namespace() {
                        _ if !dirty => (),
                        #[cfg(todo)]
                        MarkerIndex::NS_CAT => {
                            shared_map.update_static(map);
                        },
                        MarkerIndex::NS_CAT => {
                            map.categories = map.categories.clone();
                        },
                        _ if map_needs_update => {
                            shared_map.write_with_loaded(map);
                        },
                        _ => (),
                    }
                }
                match dirty {
                    // TODO: consider interaction with invalidating trail section info,
                    // and whether that requires broadcasting the update to engine/spacepacks/etc
                    // immediately or not
                    #[cfg(todo)]
                    d => d,
                    _ => true,
                }
            });
            #[cfg(feature = "space")]
            if render_changed_trailvb {
                crate::space::engine::SpaceEvent::DirtyTrailV(lpath).try_send();
                render_changed = true;
            }
            #[cfg(feature = "space")]
            if render_changed {
                crate::space::engine::SpaceEvent::DirtyMarkerI(lpath).try_send();
            }
        }
        (vis_dirty,)
    }
    fn masked_str() -> &'static taimi_pack::attributes::AttrString {
        static MASKED_ATTR_STR: LazyLock<taimi_pack::attributes::AttrString> =
            LazyLock::new(|| taimi_pack::attributes::string_into("__TAIMI_MASKED"));
        &*MASKED_ATTR_STR
    }
    fn masked_attrs_marker() -> impl Iterator<Item = PackValueCell> + Clone {
        static MASKED_ATTRS_COMMON: LazyLock<[PackValueCell; 4]> = LazyLock::new(|| {
            [
                PackValueCell::copy(keys::InGameVisibility::from(false)),
                PackValueCell::copy(keys::MapVisibility::from(false)),
                PackValueCell::copy(keys::MinimapVisibility::from(false)),
                PackValueCell::new(keys::Specializations::from(keys::Specialization(u32::MAX))),
            ]
        });
        MASKED_ATTRS_COMMON.iter().cloned()
    }
    fn masked_attrs_poi() -> impl Iterator<Item = PackValueCell> + Clone {
        static MASKED_ATTRS_POI: LazyLock<[PackValueCell; 6]> = LazyLock::new(|| {
            [
                #[cfg(todo = "unnecessary")]
                PackValueCell::new(keys::IconFile(Self::masked_str().clone())),
                PackValueCell::empty::<keys::IconFile>(),
                PackValueCell::copy(keys::PositionX(taimi_meta::spatial::IRRELEVANT_MID)),
                PackValueCell::copy(keys::PositionY(taimi_meta::spatial::IRRELEVANT_MID)),
                PackValueCell::copy(keys::PositionZ(taimi_meta::spatial::IRRELEVANT_MID)),
                PackValueCell::copy(keys::IconSize::from(0.0)),
                PackValueCell::copy(keys::MapDisplaySize::from(0.0)),
            ]
        });

        Self::masked_attrs_marker().chain(MASKED_ATTRS_POI.iter().cloned())
    }
    fn masked_attrs_trail() -> impl Iterator<Item = PackValueCell> + Clone {
        static MASKED_ATTRS_TRAIL: LazyLock<[PackValueCell; 3]> = LazyLock::new(|| {
            [
                #[cfg(todo = "unnecessary")]
                PackValueCell::new(keys::TextureFile(Self::masked_str().clone())),
                PackValueCell::empty::<keys::TextureFile>(),
                PackValueCell::empty::<keys::TrailDataFile>(),
                PackValueCell::copy(keys::TrailScale::from(0.0)),
            ]
        });

        Self::masked_attrs_marker().chain(MASKED_ATTRS_TRAIL.iter().cloned())
    }
    fn masked_attrs_category() -> impl Iterator<Item = PackValueCell> + Clone {
        static MASKED_ATTRS_CAT: LazyLock<[PackValueCell; 2]> = LazyLock::new(|| {
            [
                PackValueCell::copy(keys::IsHidden::from(true)),
                PackValueCell::new(keys::DisplayName(PathingController::masked_str().clone())),
            ]
        });

        MASKED_ATTRS_CAT.iter().cloned()
    }
    /// remove and disable a loaded marker
    pub fn mask_pack_marker(&mut self, path: MarkerPath<PackPath>) {
        let info = self.packs.lookup_info(path.root).context("unknown pack");
        let Some((_, loaded)) = rt::log::debug_ok(info) else { return };
        loaded.info.dynamics.paths.mask_marker(path.unscope());
    }
    pub async fn commit_marker_masked(&mut self, marker: MarkerId) {
        let lpath = if let Some(path) = marker.marker_path::<PackPath>() {
            // path provided? mask at runtime if loaded, and add it to a blacklist
            self.mask_pack_marker(path);
            Ok(self.gameplay_map().and_then(|map_id| {
                self.map_info
                    .find_marker_index(path, Locator::new_path(map_id))
                    .map(|lp| (Some(path), lp))
            }))
        } else {
            marker
                .marker_path::<PackMapPath>()
                .with_context(|| format!("{marker} not a pack marker path"))
                .map(|lp| Some((None, lp)))
        };
        let Some(Some((path, lpath))) = rt::log::warn_ok(lpath) else { return };
        // mask lpath at runtime
        let loaded_map = self.maps.maps.get_mut(&lpath.root).and_then(|map| {
            let dirty = match lpath.path.namespace() {
                MarkerIndex::NS_POI =>
                    if let Some(poi) = map.pois.get_mut(lpath.path.index_poi_unchecked() as usize) {
                        #[cfg(feature = "paths-interact")]
                        if poi.info.marker_info.has_attr_of::<keys::Info>()
                            || SpaceInteraction::is_interactive(poi)
                        {
                            let poi_lpath = lpath.map_path(|p| p.index_poi_unchecked());
                            let mut was_nearby = false;
                            self.rx.interact.nearby_tx.send_if_modified(|nearby| {
                                was_nearby = nearby.remove_poi(poi_lpath).is_some();
                                // was_nearby?
                                false
                            });
                            // TODO: tell interact reactor directly..?
                            // and/or update its nearby bitfield..?
                            if let Some(path) = path {
                                let poi_path: PoiPath = PoiPath::new_path(path.path.index_poi_unchecked());
                                let key = MarkerId::for_marker(lpath.root.root.rel(poi_path));
                                RenderEvent::MessageDismiss { key }.try_send();
                                if was_nearby {
                                    let gone = InteractionEvent::Gone {
                                        path: poi_path,
                                        loaded_path: lpath.root.rel(lpath.path.index_poi_unchecked()),
                                    };
                                    let _ = self.rx.interact.event_tx.send(gone);
                                }
                            }
                        }
                        #[cfg(todo)]
                        {
                            poi.visibility = VisibilityFlags::empty();
                            poi.marker_position =
                                glamour::Point3::splat(taimi_meta::spatial::IRRELEVANT_MID);
                            for attr in Self::masked_attrs_poi() {
                                poi.info.set_attr_dyn(attr);
                            }
                        }
                        let dirty = !poi.is_invalid();
                        *poi = super::state::LoadedPoi::invalid();
                        let lidx = lpath.path.index_poi_unchecked() as usize;
                        if map.poi_guids.get(lidx).map(|g| g.is_empty()) == Some(false) {
                            let mut guids = map.poi_guids[..].to_owned();
                            unsafe {
                                *guids.get_unchecked_mut(lidx) = keys::Guid::EMPTY;
                            }
                            map.poi_guids = guids.into_iter().collect();
                        }
                        dirty
                    } else {
                        false
                    },
                MarkerIndex::NS_TRAIL =>
                    if let Some(trail) = map.trails.get_mut(lpath.path.trail_index_unchecked() as usize) {
                        #[cfg(todo)]
                        {
                            trail.visibility = VisibilityFlags::empty();
                            for attr in Self::masked_attrs_trail() {
                                trail.info.set_attr_dyn(attr);
                            }
                        }
                        let dirty = !trail.is_invalid();
                        *trail = super::state::LoadedTrail::invalid();
                        dirty
                    } else {
                        false
                    },
                MarkerIndex::NS_CAT =>
                    if let Some(cat) = Arc::make_mut(&mut map.categories)
                        .get_mut(lpath.path.index_category_unchecked() as usize)
                    {
                        #[cfg(todo)]
                        {
                            cat.visibility = VisibilityFlags::empty();
                            for attr in Self::masked_attrs_category() {
                                cat.attrs.set_attr_dyn(attr);
                            }
                        }
                        #[cfg(todo)]
                        let dirty = !cat.is_invalid();
                        let dirty = true;
                        *cat = super::state::LoadedCategory::INVALID;
                        dirty
                    } else {
                        false
                    },
                _ => false,
            };
            dirty.then_some(&*map)
        });
        self.rx.shared.gameplay.send_if_modified(|gp| {
            if gp.map_id != Some(lpath.root.path) {
                return false
            }
            if let (Some(loaded_map), Some(Some(map_info))) =
                (loaded_map, gp.info.lookup_mut(&lpath.root.root))
            {
                match lpath.path.namespace() {
                    MarkerIndex::NS_POI => {
                        map_info.write_with_loaded_pois(&loaded_map);
                    },
                    MarkerIndex::NS_TRAIL => {
                        map_info.write_with_loaded_trails(&loaded_map);
                    },
                    _ => (),
                }
            }
            if let (Some(loaded_map), Some(Some(map_info))) =
                (loaded_map, gp.state.lookup_mut(&lpath.root.root))
            {
                map_info.write_with_loaded(&loaded_map);
            }
            true
        });
    }
    pub async fn allocate_loaded_marker(
        &mut self,
        path: MarkerPath<PackPath>,
        map_id: MapIndex,
    ) -> MarkerPath<PackMapPath> {
        let map_path = path.root.rel(map_id);
        if let Some(lpath) = self.map_info.find_marker_index(path, Locator::new_path(map_id)) {
            // already loaded!
            return map_path.rel(lpath.path)
        }
        let mut loaded_info = self.map_info.lookup_mut(&map_path);
        let lpath = loaded_info
            .as_mut()
            .and_then(|map_info| -> Option<LoadedMarkerPath> {
                match path.path.namespace() {
                    MarkerIndex::NS_POI => {
                        let map_info = Arc::make_mut(&mut map_info.info);
                        let lpath: LoadedPoiPath = LoadedPoiPath::new_path(map_info.poi_count() as _);
                        let idx = path.path.index_poi_unchecked() as usize;
                        if map_info.pois.len() <= idx {
                            map_info.pois.resize(idx + 1, false);
                        }
                        unsafe {
                            let mut bit = map_info.pois.get_unchecked_mut(idx);
                            let dirty = !*bit;
                            *bit = true;
                            dirty.then_some(lpath.pivot_to())
                        }
                    },
                    MarkerIndex::NS_TRAIL => {
                        let map_info = Arc::make_mut(&mut map_info.info);
                        let lpath: LoadedTrailPath = LoadedTrailPath::new_path(map_info.trail_count() as _);
                        let idx = path.path.trail_index_unchecked() as usize;
                        if map_info.trails.len() <= idx {
                            map_info.trails.resize(idx + 1, false);
                        }
                        unsafe {
                            let mut bit = map_info.trails.get_unchecked_mut(idx);
                            let dirty = !*bit;
                            *bit = true;
                            dirty.then_some(lpath.pivot_to())
                        }
                    },
                    MarkerIndex::NS_CAT => {
                        let map_info = Arc::make_mut(&mut map_info.info);
                        let idx = path.path.index_category_unchecked();
                        let mut categories = Vec::with_capacity(map_info.categories.len() + 1);
                        categories.extend_from_slice(&map_info.categories);
                        // TODO: if these aren't added in order lpaths will shift...
                        let insert_at = categories.partition_point(|cat| *cat < idx);
                        #[cfg(taimi_debug)]
                        if categories.get(insert_at) == Some(&idx) {
                            log::error!("DUPE CAT INSERT");
                            return None
                        }
                        categories.insert(insert_at, idx);
                        map_info.categories = categories.into_boxed_slice();
                        let lpath: LoadedCategoryPath = LoadedCategoryPath::new_path(insert_at as _);
                        Some(lpath.pivot_to())
                    },
                    _ => None,
                }
            });
        let Some(lpath) = lpath else {
            #[cfg(taimi_debug)]
            log::warn!("can't allocate {path}");
            return map_path.rel(MarkerIndex::UNK)
        };
        let lidx = lpath.path.index() as usize;
        let loaded_map = self.maps.maps.get_mut(&map_path).and_then(|map| {
            let dirty = match lpath.path.namespace() {
                MarkerIndex::NS_POI => {
                    let mut pois = Vec::with_capacity(map.pois.len().max(lidx + 1));
                    pois.extend_from_slice(&map.pois[..]);
                    if pois.len() < lidx {
                        pois.resize_with(lidx, super::state::LoadedPoi::invalid);
                    }
                    pois.insert(lidx, super::state::LoadedPoi::invalid());
                    map.pois = pois.into_boxed_slice();
                    true
                },
                MarkerIndex::NS_TRAIL => {
                    let mut trails = Vec::with_capacity(map.trails.len().max(lidx + 1));
                    trails.extend_from_slice(&map.trails[..]);
                    if trails.len() < lidx {
                        trails.resize_with(lidx, super::state::LoadedTrail::invalid);
                    }
                    trails.insert(lidx, super::state::LoadedTrail::invalid());
                    map.trails = trails.into_boxed_slice();
                    true
                },
                MarkerIndex::NS_CAT => {
                    let mut categories = Vec::with_capacity(map.categories.len() + 1);
                    categories.extend_from_slice(&map.categories[..]);
                    #[cfg(todo = "unnecessary")]
                    if categories.len() < lidx {
                        categories.resize_with(lidx, super::state::LoadedCategory::INVALID);
                    }
                    categories.insert(lidx, super::state::LoadedCategory::INVALID);
                    true
                },
                _ => false,
            };
            dirty.then_some(&*map)
        });
        self.rx.shared.gameplay.send_if_modified(|gp| {
            if gp.map_id != Some(map_path.path) {
                return false
            }
            let mut map_info = gp.info.lookup_mut(&path.root);
            if let (Some(loaded_map), Some(Some(map_info))) = (loaded_map, map_info.as_mut()) {
                match lpath.path.namespace() {
                    MarkerIndex::NS_POI => {
                        map_info.write_with_loaded_pois(&loaded_map);
                    },
                    MarkerIndex::NS_TRAIL => {
                        map_info.write_with_loaded_trails(&loaded_map);
                    },
                    _ => (),
                }
            }
            if let (Some(loaded_info), Some(Some(map_info))) = (loaded_info, map_info.as_mut()) {
                map_info.update_with_info(&loaded_info.info);
            }
            if let (Some(loaded_map), Some(Some(map))) = (loaded_map, gp.state.lookup_mut(&path.root)) {
                map.write_with_loaded(&loaded_map);
            }
            // XXX: fresh markers need to be setup with attrs before there's any point in telling anyone...
            false
        });
        map_path.rel(lpath.path)
    }
}
