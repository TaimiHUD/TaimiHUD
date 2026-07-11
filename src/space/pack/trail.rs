use {
    super::PackRenderState,
    crate::{
        controller::pathing::{
            registry::LoadedTrailPath,
            shared::{LoadedTrailRef, SharedPackInfo},
            state::LoadedTrailGeometry,
        },
        exports::runtime::{
            self as rt,
            textures::{TextureKey, TextureSlot},
            Counter,
        },
        space::{pack::PoiCommonRenderData, resources::Model},
    },
    anyhow::Context,
    glam::{Vec3, Vec4},
    std::{mem, ops},
    taimi_d3d::dx11::{buffer::VertexBuffer, prelude::*},
    taimi_hoard::loc::Locator,
    taimi_meta::{
        packs::{
            id::{MarkerId, MarkerIndex},
            TrailSectionIndex,
            TrailSectionPath,
        },
        ui::LocalContext,
    },
    taimi_pack::attributes::{
        cell::{pack_attr, PackKeyId},
        keys,
    },
};

/// World render data
pub struct TrailRender {
    pub texture_handle: Option<TextureKey>,
    pub texture: Option<TextureSlot>,
    pub section_vbuffer: Option<VertexBuffer>,
    pub section_vb_ng: Option<super::instance::TrailVertexBuffer>,
    pub section_vb_ng_map: Option<super::instance::Map2dVertexBuffer>,
    pub section_vb_map: Option<VertexBuffer>,
    pub vbuffer_section_end: Vec<u32>,
    pub vbuffer_section_end_map: Vec<u32>,
    pub vertex_colour: [u8; 3],
}

impl TrailRender {
    pub fn empty() -> Self {
        Self {
            texture_handle: None,
            texture: None,
            section_vbuffer: None,
            section_vb_ng: None,
            section_vb_ng_map: None,
            section_vb_map: None,
            vbuffer_section_end: Vec::new(),
            vbuffer_section_end_map: Vec::new(),
            vertex_colour: Self::WHITE_U8,
        }
    }

    pub fn setup_geometry(
        &mut self,
        device: &Dx11Device,
        geometry: LoadedTrailGeometry,
        arcrender: bool,
    ) -> anyhow::Result<()> {
        // TODO: average colours if we ever support gradients?
        self.vertex_colour = geometry
            .legacy
            .vertices
            .first()
            .and_then(|v| match v.colour {
                c if c.abs_diff_eq(Self::WHITE_F32.truncate(), Self::COMPONENT_THRESH) => None,
                c => Some(c),
            })
            .map(|c| {
                let [r, g, b] = (c * 255.0).to_array();
                [r as u8, g as u8, b as u8]
            })
            .unwrap_or(Self::WHITE_U8);
        let trailv = arcrender
            .then(|| {
                let trailv = geometry
                    .legacy
                    .vertices
                    .iter()
                    .map(|v| super::instance::TrailVertex::from(*v))
                    .collect::<Vec<_>>();
                crate::exports::runtime::log::error_ok(super::instance::TrailVertex::alloc(device, &trailv))
            })
            .flatten();
        let trailv_map = arcrender
            .then(|| {
                let trailv = geometry
                    .map
                    .vertices
                    .iter()
                    .map(|v| super::instance::Map2dVertex::from(*v))
                    .collect::<Vec<_>>();
                crate::exports::runtime::log::error_ok(super::instance::Map2dVertex::alloc(device, &trailv))
            })
            .flatten();
        let section_vb_map = match arcrender {
            true => rt::log::warn_ok({
                // TODO: if not visible on maps, don't bother or something idk
                let model = Model::from_vertices(geometry.map.vertices);
                model.to_buffer(device).context("Creating map trail vbuffer")
            }),
            false => None,
        };
        let section_vbuffer = match arcrender {
            true if section_vb_map.is_some() => Ok(None),
            _ => {
                let model = Model::from_vertices(geometry.legacy.vertices);
                model.to_buffer(device).context("Creating trail vbuffer")
            }
            .map(Some),
        };
        #[cfg(feature = "statistics")]
        let prev_size_vb = self
            .section_vbuffer
            .as_ref()
            .map(|v| v.size() as isize)
            .unwrap_or(0);
        #[cfg(feature = "statistics")]
        let prev_size_map = self
            .section_vb_map
            .as_ref()
            .map(|v| v.size() as isize)
            .unwrap_or(0);
        #[cfg(feature = "statistics")]
        let prev_size_ng = self
            .section_vb_ng
            .as_ref()
            .map(|v| v.size() as isize)
            .unwrap_or(0);
        #[cfg(feature = "statistics")]
        let prev_size_ng_map = self
            .section_vb_ng_map
            .as_ref()
            .map(|v| v.size() as isize)
            .unwrap_or(0);
        match section_vbuffer {
            Ok(vbuffer) => {
                #[cfg(feature = "statistics")]
                STATS_TRAIL_VERTEX_SIZE.adjust_by(|| {
                    let vb_size = vbuffer.as_ref().map(|vb| vb.size() as isize).unwrap_or_default();
                    vb_size - prev_size_vb - prev_size_ng - prev_size_ng_map - prev_size_map
                });
                self.section_vbuffer = vbuffer;
                self.section_vb_ng = trailv;
                self.section_vb_ng_map = trailv_map;
                self.section_vb_map = section_vb_map;
                #[cfg(feature = "statistics")]
                if let Some(vb) = &self.section_vb_ng {
                    STATS_TRAIL_VERTEX_SIZE.adjust_by(|| vb.size() as isize);
                }
                #[cfg(feature = "statistics")]
                if let Some(vb) = &self.section_vb_ng_map {
                    STATS_TRAIL_VERTEX_SIZE.adjust_by(|| vb.size() as isize);
                }
                #[cfg(feature = "statistics")]
                if let Some(vb) = &self.section_vb_map {
                    STATS_TRAIL_VERTEX_SIZE.adjust_by(|| vb.size() as isize);
                }
                self.vbuffer_section_end = geometry.legacy.section_lengths;
                self.vbuffer_section_end_map = geometry.map.section_lengths;
                let mut start = 0u32;
                for out in &mut self.vbuffer_section_end {
                    *out += start;
                    start = *out;
                }
                let mut start = 0u32;
                for out in &mut self.vbuffer_section_end_map {
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
        arcrender: bool,
    ) -> bool {
        let mut incomplete = false;
        let vb_empty = match arcrender {
            true => self.section_vb_ng.is_none(),
            _ => self.section_vbuffer.is_none() & self.section_vb_map.is_none(),
        };
        if vb_empty {
            let no_vb = self.section_vbuffer.is_none()
                & self.section_vb_map.is_none()
                & self.section_vb_ng.is_none();
            if no_vb && !self.is_empty() {
                // marked broken, ignore this section...
                return true
            }
            let id_storage;
            let id = match id {
                id if path.path.path != 0 => {
                    // replace section index with 0 since we can't partially load trl data (yet?)
                    let id = id
                        .get_marker_pack_map_path()
                        .rel(MarkerIndex::with_trail_section(path.root.path, 0));
                    id_storage = MarkerId::for_marker(id);
                    &id_storage
                },
                id => id,
            };
            draw_state.mark_incomplete(id);
            incomplete = true;
        }
        if matches!(
            self.texture,
            None | Some(TextureSlot::Reserved | TextureSlot::Loading)
        ) {
            let id = id
                .get_marker_pack_map_path()
                .rel(MarkerIndex::with_trail(path.root.path));
            if !draw_state.mark_incomplete(&MarkerId::for_marker(id)) {
                incomplete = true;
            }
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
        let texture = self.texture.as_ref().and_then(TextureSlot::get);
        let texture = match texture {
            None if matches!(self.texture, Some(TextureSlot::Unavailable)) =>
                common.fallback_texture2.as_ref(),
            texture => texture.or_else(|| common.fallback_texture.as_ref()),
        };
        if let Some(texture) = texture {
            texture.set(device_context, 0);
        }
    }
    /// Draw a trail segment.
    /// PREREQUISITES: Trail shaders and texture must already be set.
    pub fn draw_section(&self, device_context: &Dx11Context, section: TrailSectionPath, ctx: LocalContext) {
        let vb_legacy = self.section_vbuffer.as_ref().map(|vb| (vb, LocalContext::World));
        let vb_map = self.section_vb_map.as_ref().map(|vb| (vb, LocalContext::GLOBAL));
        let vbuffer = match ctx {
            LocalContext::World => vb_legacy.or(vb_map),
            LocalContext::Map(..) => vb_map.or(vb_legacy),
        };
        let sections = vbuffer.and_then(|(vb, vb_ctx)| {
            self.section_geometry_vertices(section.path, vb_ctx)
                .map(|s| (vb, s))
        });
        let Some((vbuffer, ops::Range { start, end })) = sections else {
            log::error!("attempted to draw invalid {section}");
            return
        };
        if start >= end {
            // ignore empty sections
            log::debug!("BUG? filter empty sections prior to scene or binding");
            return
        }
        vbuffer.set(device_context, 0);
        unsafe {
            //PrimitiveTopology::TriangleStrip.set(device_context);
            match ctx {
                LocalContext::World => device_context.Draw(end - start, start),
                LocalContext::Map(..) => device_context.DrawInstanced(end - start, 1, start, 0),
            }
        }
    }

    #[cfg(feature = "paths-dyn")]
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
    #[cfg(feature = "paths-dyn")]
    pub(crate) fn attr_dirties_render(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            keys::AnimSpeed, keys::IsWall,
            keys::Alpha,
            keys::Tint,
            keys::InGameVisibility,
            keys::MapVisibility,
            keys::MinimapVisibility,
            keys::GameMap,
            keys::CanFade, keys::Cull, keys::FadeNear, keys::FadeFar,
        ]) }
    }

    pub fn section_geometry_vertices(
        &self,
        section: TrailSectionIndex,
        ctx: LocalContext,
    ) -> Option<ops::Range<u32>> {
        let section = section as usize;
        let ends = match ctx {
            LocalContext::Map(..) if !self.vbuffer_section_end_map.is_empty() =>
                &self.vbuffer_section_end_map[..],
            _ => &self.vbuffer_section_end[..],
        };
        let end = *ends.get(section)?;
        let start = match section {
            #[cfg(todo = "unnecessary")]
            section => section
                .checked_sub(1)
                .map(|prev| unsafe { *ends.get_unchecked(prev) })
                .unwrap_or(0),
            section => *ends.get(section.wrapping_sub(1)).unwrap_or(&0),
        };
        Some(start..end)
    }

    /// mark broken
    pub fn disable(&mut self) {
        self.invalidate();
        self.vbuffer_section_end.push(0);
        //self.vbuffer_section_end_map.push(0);
    }
    /// refresh/unload
    pub fn invalidate(&mut self) {
        self.section_vbuffer = None;
        self.section_vb_ng = None;
        self.section_vb_map = None;
        self.vbuffer_section_end.clear();
        self.vbuffer_section_end_map.clear();
    }

    pub fn is_empty(&self) -> bool {
        let no_vb =
            self.section_vbuffer.is_none() && self.section_vb_ng.is_none() && self.section_vb_map.is_none();
        no_vb && self.vbuffer_section_end.is_empty()
    }

    #[inline]
    pub fn vertex_colour(&self) -> Vec4 {
        (self.vertex_colour256() / 255.0f32).extend(1.0)
    }
    #[inline]
    fn vertex_colour256(&self) -> Vec3 {
        let [r, g, b] = self.vertex_colour;
        Vec3::new(r as f32, g as f32, b as f32)
    }
    pub fn tint_to(&self, target: Vec4) -> Option<Vec4> {
        if Self::WHITE_F32.abs_diff_eq(target, Self::COMPONENT_THRESH) {
            return None
        }
        Some(target * 255.0 / self.vertex_colour256().extend(1.0))
    }
    pub const WHITE_U8: [u8; 3] = [0xffu8; 3];
    pub const WHITE_F32: Vec4 = Vec4::ONE;
    /// ~1/255
    #[cfg(todo)]
    const COMPONENT_THRESH: f32 = 4e-3f32;
    /// ~3/255
    const COMPONENT_THRESH: f32 = 1e-2f32;

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
        if let Some(vbuffer) = &self.section_vb_ng {
            STATS_TRAIL_VERTEX_SIZE.decrement_by(|| vbuffer.size());
        }
    }
}

pub static STATS_TRAIL_VERTEX_SIZE: Counter = Counter::DEFAULT;
