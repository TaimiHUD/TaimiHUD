use {
    super::PackRenderState,
    crate::{
        controller::pathing::{
            registry::LoadedTrailPath,
            shared::{LoadedTrailRef, SharedPackInfo},
            state::LoadedTrailGeometry,
        },
        exports::runtime::{
            textures::{TextureKey, TextureSlot},
            Counter,
        },
        space::{pack::PoiCommonRenderData, resources::Model},
    },
    anyhow::Context,
    std::{mem, ops},
    taimi_d3d::dx11::{buffer::VertexBuffer, prelude::*},
    taimi_hoard::loc::Locator,
    taimi_meta::{
        packs::{
            id::{MarkerId, MarkerIndex},
            TrailSectionIndex,
            TrailSectionPath,
        },
        ui::{LocalContext, MapContext},
    },
    taimi_pack::{
        attributes::{
            cell::{pack_attr, AttrKeyValue, GetAttrDyn, PackKeyId, PackValueCell, SetAttrDyn},
            keys::{self, GetAttr},
        },
        trail::TrailData,
        Trail,
    },
};

/// World render data
pub struct TrailRender {
    pub texture_handle: Option<TextureKey>,
    pub texture: Option<TextureSlot>,
    pub section_vbuffer: Option<VertexBuffer>,
    pub section_vb_ng: Option<super::instance::TrailVertexBuffer>,
    pub vbuffer_section_end: Vec<u32>,
}

impl TrailRender {
    pub fn empty() -> Self {
        Self {
            texture_handle: None,
            texture: None,
            section_vbuffer: None,
            section_vb_ng: None,
            vbuffer_section_end: Vec::new(),
        }
    }

    pub fn setup_geometry(
        &mut self,
        device: &Dx11Device,
        geometry: LoadedTrailGeometry,
    ) -> anyhow::Result<()> {
        let trailv = geometry.vertices.iter()
            .map(|v| super::instance::TrailVertex::from(*v))
            .collect::<Vec<_>>();
        let trailv = crate::exports::runtime::log::error_ok(super::instance::TrailVertex::alloc(device, &trailv));
        let model = Model::from_vertices(geometry.vertices);
        let section_vbuffer = model.to_buffer(device).context("Creating trail vbuffer");
        #[cfg(feature = "statistics")]
        let prev_size = self
            .section_vbuffer
            .as_ref()
            .map(|v| v.size() as isize)
            .unwrap_or(0);
        match section_vbuffer {
            Ok(vbuffer) => {
                #[cfg(feature = "statistics")]
                STATS_TRAIL_VERTEX_SIZE.adjust_by(|| vbuffer.size() as isize - prev_size);
                self.section_vbuffer = Some(vbuffer);
                self.section_vb_ng = trailv;
                self.vbuffer_section_end = geometry.section_lengths;
                let mut start = 0u32;
                for out in &mut self.vbuffer_section_end {
                    *out += start;
                    start = *out;
                }
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    pub fn update(
        &mut self,
        _device: &Dx11Device,
        pack_info: &SharedPackInfo,
        ltrail: Option<LoadedTrailRef<'_>>,
    ) {
        let texture = ltrail
            .as_ref()
            .and_then(|ltrail| ltrail.trail_attrs().texture.as_ref());
        pack_info.setup_texture(&mut self.texture_handle, &mut self.texture, texture);
    }
    pub fn report_incomplete(
        &self,
        id: &MarkerId,
        draw_state: &mut PackRenderState,
        path: Locator<LoadedTrailPath, TrailSectionPath>,
    ) -> bool {
        let mut incomplete = false;
        if self.section_vbuffer.is_none() {
            if !self.is_empty() {
                // marked broken, ignore this section...
                return true
            }
            let id = match id {
                id if path.path.path != 0 => {
                    // replace section index with 0 since we can't partially load trl data (yet?)
                    let id = id
                        .get_marker_pack_map_path()
                        .rel(MarkerIndex::with_trail_section(path.root.path, 0));
                    MarkerId::for_marker(id)
                },
                id => id.clone(),
            };
            draw_state.drawn_incomplete.insert(id);
            incomplete = true;
        }
        if matches!(
            self.texture,
            None | Some(TextureSlot::Reserved | TextureSlot::Loading)
        ) {
            let id = id
                .get_marker_pack_map_path()
                .rel(MarkerIndex::with_trail(path.root.path));
            draw_state.drawn_incomplete.insert(MarkerId::for_marker(id));
        }
        incomplete
    }
    pub fn needs_texture_info(&self) -> bool {
        self.texture.is_none() && self.texture_handle.is_none()
    }

    pub fn bind_texture(
        &self,
        device_context: &Dx11Context,
        common: &PoiCommonRenderData,
        _ctx: LocalContext,
    ) {
        let texture = self
            .texture
            .as_ref()
            .and_then(TextureSlot::get);
        let texture = match texture {
            None if matches!(self.texture, Some(TextureSlot::Unavailable)) =>
                common.fallback_texture2.as_ref(),
            texture => texture.or_else(|| common.fallback_texture.as_ref())
        };
        if let Some(texture) = texture {
            texture.set(device_context, 0);
        }
    }
    /// Draw a trail segment.
    /// PREREQUISITES: Trail shaders and texture must already be set.
    pub fn draw_section(&self, device_context: &Dx11Context, section: TrailSectionPath, ctx: LocalContext) {
        let Some(ops::Range { start, end }) = self.section_geometry_vertices(section.path) else {
            log::error!("attempted to draw invalid {section}");
            return
        };
        if start >= end {
            // ignore empty sections
            log::debug!("BUG? filter empty sections prior to scene or binding");
            return
        }
        if let Some(section_vbuffer) = &self.section_vbuffer {
            section_vbuffer.set(device_context, 0);
        } else {
            log::debug!("BUG? trail vbuffer missing while expecting {start}..{end}");
        }
        unsafe {
            //PrimitiveTopology::TriangleStrip.set(device_context);
            match ctx {
                LocalContext::World => device_context.Draw(end - start, start),
                LocalContext::Map(..) => device_context.DrawInstanced(end - start, 1, start, 0),
            }
        }
    }

    #[cfg(feature = "paths-lua")]
    pub(crate) fn attr_dirties_vb(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            //keys::Alpha,
            //keys::Tint,
            keys::TrailScale,
            keys::IsWall,
            keys::InGameVisibility,
            keys::TrailDataFile,
        ]) }
    }
    #[cfg(feature = "paths-lua")]
    pub(crate) fn attr_dirties_render(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            keys::Alpha,
            keys::Tint,
            keys::InGameVisibility,
            keys::MapVisibility,
            keys::MinimapVisibility,
            keys::GameMap,
        ]) }
    }

    pub fn section_geometry_vertices(&self, section: TrailSectionIndex) -> Option<ops::Range<u32>> {
        let section = section as usize;
        let end = *self.vbuffer_section_end.get(section)?;
        let start = match section {
            #[cfg(todo = "unnecessary")]
            section => section
                .checked_sub(1)
                .map(|prev| unsafe { *self.vbuffer_section_end.get_unchecked(prev) })
                .unwrap_or(0),
            section => *self
                .vbuffer_section_end
                .get(section.wrapping_sub(1))
                .unwrap_or(&0),
        };
        Some(start..end)
    }

    /// mark broken
    pub fn disable(&mut self) {
        self.section_vbuffer = None;
        self.section_vb_ng = None;
        self.vbuffer_section_end.clear();
        self.vbuffer_section_end.push(0);
    }

    pub fn is_empty(&self) -> bool {
        self.section_vbuffer.is_none() && self.vbuffer_section_end.is_empty()
    }

    #[inline]
    pub fn cleanup_background(mut self) {
        mem::forget(self.texture.take());
        mem::forget(self.section_vbuffer.take());
        mem::forget(self.section_vb_ng.take());
    }
}

#[cfg(feature = "statistics")]
impl Drop for TrailRender {
    fn drop(&mut self) {
        if let Some(vbuffer) = &self.section_vbuffer {
            STATS_TRAIL_VERTEX_SIZE.decrement_by(|| vbuffer.size());
        }
    }
}

pub static STATS_TRAIL_VERTEX_SIZE: Counter = Counter::DEFAULT;

#[cfg(deleteme)]
pub struct ActiveTrail {
    pub trail_idx: usize,
    pub category_idx: usize,
    pub filtered: bool,
    pub render_bookmark: usize,

    // Segment data.
    pub section_bounds: Vec<Box3<DrawSpace>>,

    // World render data.
    pub texture: Arc<Texture>,
    pub section_vbuffer: VertexBuffer,
    pub section_bookmarks: Vec<u32>,

    pub y_offset: f32,
    pub vertex_colour: Vec3,

    pub attr_tint: keys::Tint,
    pub attr_tint_map: Option<keys::MapTint>,
    pub attr_opacity: keys::Alpha,
    pub attr_vis_space: keys::InGameVisibility,
    pub attr_vis_map: keys::MapVisibility,
    pub attr_vis_minimap: keys::MinimapVisibility,
}
#[cfg(deleteme)]
impl ActiveTrail {
    pub fn build<A>(
        loader: &mut ActivePack,
        trail: Option<&Trail>,
        attrs: &A,
        trail_idx: usize,
        category_idx: usize,
        params: &TrailParams,
        render_bookmark: usize,
    ) -> anyhow::Result<ActiveTrail>
    where
        A: GetAttr<keys::Guid>
            + GetAttr<keys::Tint>
            + GetAttr<keys::MapTint>
            + GetAttr<keys::Alpha>
            + GetAttr<keys::TrailScale>
            + GetAttr<keys::InGameVisibility>
            + GetAttr<keys::MapVisibility>
            + GetAttr<keys::MinimapVisibility>
            + GetAttr<keys::IsWall>
            + GetAttr<keys::TextureFile>
            + GetAttr<keys::TrailDataFile>
            + GetAttr<keys::CategoryRef>,
    {
        let attr_tint = GetAttr::<keys::Tint>::get_attr_or_default(attrs).into_owned();
        let attr_tint_map = GetAttr::<keys::MapTint>::get_attr(attrs).map(|v| v.into_owned());
        let vertex_colour = Vec4::from(attr_tint).truncate();
        let trail_width = params.width();
        let resolution = params.resolution();
        let smoothing = params.smoothing();
        let attr_vis_space = GetAttr::<keys::InGameVisibility>::get_attr_or_default(attrs).into_owned();
        let map_only = !bool::from(attr_vis_space);
        let is_wall = bool::from(GetAttr::<keys::IsWall>::get_attr_or_default(attrs).into_owned());
        let trail_scale = f32::from(GetAttr::<keys::TrailScale>::get_attr_or_default(attrs).into_owned());
        let is_wall = is_wall && {
            // geometry is shared between space and maps, so a paper-thin
            // vertical wall is meaningless if not intended to show in-game
            // (heart boundaries sets this combo)
            !map_only
        };
        let mut y_offset = {
            // mitigate z-fighting by fudging y values for (hopefully) unique trails
            let pack_signature = loader.pack.trails.len()
                + loader.pack.pois.len()
                + loader.pack.categories.all_categories.len();
            params.y_offset_for(pack_signature ^ (trail_idx.wrapping_mul(73)))
        };

        let texture_handle = GetAttr::<keys::TextureFile>::get_attr(attrs)
            .ok_or_else(|| anyhow::anyhow!("TODO: Add a fallback texture for trails"))?;
        let texture_handle = loader.register_texture(&texture_handle);
        // TODO: check for override data provided
        let trail_path = GetAttr::<keys::TrailDataFile>::get_attr(attrs)
            .ok_or_else(|| anyhow::anyhow!("no .trl path specified"))?;
        let mut trail_file = loader.with_loader(|l| {
            let mut res = l.load_asset_dyn(&trail_path[..]);
            let parent = trail.and_then(|t| t.parent_path.as_ref());
            if let (Err(..), Some(parent)) = (&res, parent) {
                if let Ok(fallback) = l.find_asset_near(parent, &trail_path[..]) {
                    res = Ok(fallback);
                }
            }
            res
        })?;
        let trail_data = TrailData::read_from_trl(&mut trail_file)
            .with_context(|| format!("Loading trail vertices from {trail_path}"))?;

        let texture = loader
            .get_or_load_texture(texture_handle, device)
            .context("Loading trail texture")?;

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut section_bookmarks: Vec<u32> = vec![0];
        let mut section_bounds = Vec::new();

        #[cfg(taimi_debug)]
        if vertices.is_empty() {
            if let Some(cat) = GetAttr::<keys::CategoryRef>::get_attr(attrs) {
                let guid = GetAttr::<keys::Guid>::get_attr_or_default(attrs);
                log::debug!("Empty trail {cat}:{guid}");
            }
        }

        let model = Model::from_vertices(vertices);
        let section_vbuffer = model.to_buffer(device).context("Creating trail vbuffer")?;
        STATS_TRAIL_VERTEX_SIZE.increment_by(|| section_vbuffer.size());

        Ok(ActiveTrail {
            trail_idx,
            category_idx,
            filtered: false,
            section_bounds,
            texture: texture.clone(),
            section_vbuffer,
            section_bookmarks,
            render_bookmark,
            y_offset,
            vertex_colour,
            attr_tint,
            attr_tint_map,
            attr_opacity: GetAttr::<keys::Alpha>::get_attr_or_default(attrs).into_owned(),
            attr_vis_space,
            attr_vis_map: GetAttr::<keys::MapVisibility>::get_attr_or_default(attrs).into_owned(),
            attr_vis_minimap: GetAttr::<keys::MinimapVisibility>::get_attr_or_default(attrs).into_owned(),
        })
    }
    pub fn new_empty<A>(
        loader: &mut ActivePack,
        attrs: &A,
        trail_idx: usize,
        category_idx: usize,
        device: &Dx11Device,
        render_bookmark: usize,
    ) -> anyhow::Result<ActiveTrail>
    where
        A: GetAttr<keys::Guid>
            + GetAttr<keys::InGameVisibility>
            + GetAttr<keys::MapVisibility>
            + GetAttr<keys::MinimapVisibility>
            + GetAttr<keys::TextureFile>
            + GetAttr<keys::Alpha>
            + GetAttr<keys::Tint>
            + GetAttr<keys::MapTint>,
    {
        let texture_handle = GetAttr::<keys::TextureFile>::get_attr(attrs);
        let texture = texture_handle
            .map(|h| {
                let texture_handle = loader.register_texture(&h);
                loader
                    .get_or_load_texture(texture_handle, device)
                    .context("Loading trail texture")
                    .cloned()
            })
            .unwrap_or_else(|| {
                unsafe {
                    Texture::new_raw(
                        device,
                        &vec![0u8; 32 * 32],
                        [32, 32],
                        32,
                        taimi_d3d::DxgiFormat::A8_UNORM,
                    )
                }
                .map(Arc::new)
                .context("Preparing empty texture")
            })?;
        Ok(ActiveTrail {
            trail_idx,
            category_idx,
            filtered: false,
            texture,
            section_vbuffer: VertexBuffer::new::<Vertex>(device, None, Default::default())?,
            section_bounds: Default::default(),
            section_bookmarks: Default::default(),
            render_bookmark,
            y_offset: 0.0,
            vertex_colour: Vec3::ONE,
            attr_opacity: GetAttr::<keys::Alpha>::get_attr_or_default(attrs).into_owned(),
            attr_tint: GetAttr::<keys::Tint>::get_attr_or_default(attrs).into_owned(),
            attr_tint_map: GetAttr::<keys::MapTint>::get_attr(attrs).map(|v| v.into_owned()),
            attr_vis_space: GetAttr::<keys::InGameVisibility>::get_attr_or_default(attrs).into_owned(),
            attr_vis_map: GetAttr::<keys::MapVisibility>::get_attr_or_default(attrs).into_owned(),
            attr_vis_minimap: GetAttr::<keys::MinimapVisibility>::get_attr_or_default(attrs).into_owned(),
        })
    }
    pub(crate) fn is_visible_for_map(&self, ctx: MapContext) -> bool {
        match ctx {
            MapContext::Global => self.attr_vis_map.into(),
            MapContext::Minimap => self.attr_vis_minimap.into(),
        }
    }
    pub fn tint(&self) -> Option<Vec4> {
        let mut tint = Vec4::from(self.attr_tint);
        tint.w *= f32::from(self.attr_opacity);
        if tint.w >= 0.97 && self.vertex_colour == tint.truncate() {
            return None
        }
        tint *= self.vertex_colour.recip().extend(1.0);
        Some(tint)
    }
    pub fn tint_map(&self) -> Option<Vec4> {
        self.attr_tint_map
            .map(|tint| {
                let mut tint = Vec4::from(tint);
                #[cfg(todo)]
                {
                    tint.w *= f32::from(self.attr_opacity);
                }
                tint *= self.vertex_colour.recip().extend(1.0);
                tint
            })
            .or_else(|| self.tint())
    }

    pub(crate) fn gen_points(
        vertices: &mut Vec<Vertex>,
        points: &[Point3],
        trail_width: f32,
        trail_scale: f32,
        is_wall: bool,
        colour: Vec3,
    ) {
        let mut cur_point = points[0];
        let mut last_offset = Vector3::ZERO;
        let mut flip_over = 1.0f32;
        let normal_offset = trail_width * trail_scale / 2.0;
        let mut mod_distance = Vector3::ZERO;

        let mut distance = 0.0f32;
        for &next_point in points.iter().skip(1) {
            let path_direction = next_point - cur_point;
            let offset = path_direction.cross(Vector3::Y);
            let offset = if is_wall { path_direction.cross(offset) } else { offset };
            let offset = offset.normalize();

            if last_offset != Vector3::ZERO && offset.dot(last_offset) < 0.0 {
                flip_over *= -1.0;
            }

            mod_distance = offset * normal_offset * flip_over;
            let normal_scale_dir = mod_distance.to_raw().normalize_or(
                glam::vec3(1.0, 0.0, 1.0)
                    .normalize()
                    .copysign(mod_distance.to_raw()),
            );

            vertices.push(Vertex {
                position: (cur_point - mod_distance).into(),
                colour,
                normal: -normal_scale_dir,
                texture: glam::vec2(1.0, distance / trail_width - 1.0),
            });
            vertices.push(Vertex {
                position: (cur_point + mod_distance).into(),
                colour,
                normal: normal_scale_dir,
                texture: glam::vec2(0.0, distance / trail_width - 1.0),
            });

            distance += path_direction.length();
            last_offset = offset;
            cur_point = next_point;
        }

        let normal_scale_dir = mod_distance.to_raw().normalize_or(
            glam::vec3(1.0, 0.0, 1.0)
                .normalize()
                .copysign(mod_distance.to_raw()),
        );
        vertices.push(Vertex {
            position: (cur_point - mod_distance).into(),
            colour,
            normal: -normal_scale_dir,
            texture: glam::vec2(1.0, distance / trail_width - 1.0),
        });
        vertices.push(Vertex {
            position: (cur_point + mod_distance).into(),
            colour,
            normal: normal_scale_dir,
            texture: glam::vec2(0.0, distance / trail_width - 1.0),
        });
    }
}
#[cfg(deleteme)]
pack_attr! {
    impl Attr{keys::InGameVisibility} for &struct{ActiveTrail}.attr_vis_space {}
    impl Attr{keys::MapVisibility} for &struct{ActiveTrail}.attr_vis_map {}
    impl Attr{keys::MinimapVisibility} for &struct{ActiveTrail}.attr_vis_minimap {}
    impl Attr{keys::Alpha} for &struct{ActiveTrail}.attr_opacity {}
    impl Attr{keys::Tint} for &struct{ActiveTrail}.attr_tint {}
    impl Attr{keys::MapTint} for &struct{ActiveTrail}.attr_tint_map? {}
}
#[cfg(deleteme)]
impl GetAttrDyn for ActiveTrail {
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
            keys::Alpha, keys::Tint, keys::MapTint,
        ]) }
    }
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
            keys::Alpha, keys::Tint, keys::MapTint,
        ] }
        .unwrap_or(false)
    }
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
            keys::Alpha, keys::Tint, keys::MapTint,
        ] }
        .flatten()
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = std::borrow::Cow<'_, dyn AttrKeyValue>> + '_ {
        pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
            keys::Alpha, keys::Tint, keys::MapTint,
        ] }
    }
}
#[cfg(deleteme)]
impl SetAttrDyn for ActiveTrail {
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in [
            keys::InGameVisibility, keys::MapVisibility, keys::MinimapVisibility,
            keys::Alpha, keys::Tint, keys::MapTint,
        ] }
    }
}
