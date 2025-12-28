use {
    super::PackRenderState, crate::{
        controller::pathing::{
            registry::LoadedTrailPath, shared::{LoadedTrailRef, LoadedTrailShared, SharedPackInfo}, space::SpaceLoader, visible::{LoadedTrail, LoadedTrailGeometry, LoadedTrailSection}
        }, exports::runtime::{
            textures::{TextureKey, TextureSlot},
            Counter,
        }, resources::Texture, space::{
            pack::PoiCommonRenderData,
            resources::Model,
        }
    },
    taimi_hoard::loc::Locator,
    anyhow::Context, std::{mem, ops, sync::Arc}, taimi_d3d::dx11::{
        buffer::VertexBuffer,
        prelude::*,
    }, taimi_hoard::lazyfmt, taimi_meta::{packs::{id::{MarkerId, MarkerIndex}, TrailSectionIndex, TrailSectionPath}, ui::LocalContext}, taimi_pack::attributes::AttrString
};

/// World render data
pub struct TrailRender {
    pub texture_handle: Option<TextureKey>,
    pub texture: Option<TextureSlot>,
    pub section_vbuffer: Option<VertexBuffer>,
    pub vbuffer_section_end: Vec<u32>,
}

impl TrailRender {
    pub fn empty() -> Self {
        Self {
            texture_handle: None,
            texture: None,
            section_vbuffer: None,
            vbuffer_section_end: Vec::new(),
        }
    }

    #[cfg(deleteme)]
    pub fn setup_texture(
        &mut self,
        loader: &mut SpaceLoader<'_>,
        texture_name: &AttrString,
    ) {
        if let Some(handle) = &mut self.texture_handle {
            loader.setup_texture(handle, &mut self.texture)
        } else {
            let handle = loader.register_texture(texture_name);
            let (handle, texture) = loader.get_or_load_texture(handle);
            self.texture_handle = Some(handle);
            self.texture = texture;
        }
    }
    pub fn setup_geometry(
        &mut self,
        device: &Dx11Device,
        geometry: LoadedTrailGeometry,
    ) -> anyhow::Result<()> {
        let model = Model::from_vertices(geometry.vertices);
        let section_vbuffer = model.to_buffer(device).context("Creating trail vbuffer");
        #[cfg(feature = "statistics")]
        let prev_size = self.section_vbuffer.as_ref().map(|v| v.size() as isize).unwrap_or(0);
        match section_vbuffer {
            Ok(vbuffer) => {
                #[cfg(feature = "statistics")]
                STATS_TRAIL_VERTEX_SIZE.adjust_by(|| vbuffer.size() as isize - prev_size);
                self.section_vbuffer = Some(vbuffer);
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

    pub fn update(&mut self, _device: &Dx11Device, pack_info: &SharedPackInfo, ltrail: Option<LoadedTrailRef<'_>>) {
        let texture = ltrail.as_ref().and_then(|ltrail| ltrail.trail_attrs().texture.as_ref());
        SpaceLoader::setup_texture(&mut self.texture_handle, &mut self.texture, pack_info, texture);
    }
    pub fn report_incomplete(&self, id: &MarkerId, draw_state: &mut PackRenderState, path: Locator<LoadedTrailPath, TrailSectionPath>) -> bool {
        let mut incomplete = false;
        if self.section_vbuffer.is_none() {
            if !self.is_empty() {
                // marked broken, ignore this section...
                return true
            }
            let id = match id {
                id if path.path.path != 0 => {
                    // replace section index with 0 since we can't partially load trl data (yet?)
                    let id = id.get_marker_pack_map_path().rel(MarkerIndex::with_trail_section(path.root.path, 0));
                    MarkerId::for_marker(id)
                },
                id => id.clone(),
            };
            draw_state.drawn_incomplete.insert(id);
            incomplete = true;
        }
        if matches!(self.texture, None | Some(TextureSlot::Reserved | TextureSlot::Loading)) {
            let id = id.get_marker_pack_map_path().rel(MarkerIndex::with_trail(path.root.path));
            draw_state.drawn_incomplete.insert(MarkerId::for_marker(id));
        }
        incomplete
    }
    pub fn needs_texture_info(&self) -> bool {
        self.texture.is_none() && self.texture_handle.is_none()
    }

    pub fn bind_texture(&self, device_context: &Dx11Context, common: &PoiCommonRenderData, _ctx: LocalContext) {
        let texture = self.texture.as_ref()
            .and_then(TextureSlot::get)
            .or_else(|| common.fallback_texture.as_ref());
        if let Some(texture) = texture {
            texture.set(device_context, 0);
        } else {
            log::warn!("PATHY: fallback missing??");
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
            log::debug!("TODO: filter out empty sections earlier (prior to binding state)!");
            return
        }
        if let Some(section_vbuffer) = &self.section_vbuffer {
            section_vbuffer.set(device_context, 0);
        } else {
            // TODO log::warn!("PATHY: vbuffer missing??");
        }
        unsafe {
            //PrimitiveTopology::TriangleStrip.set(device_context);
            match ctx {
                LocalContext::World => device_context.Draw(
                    end - start,
                    start,
                ),
                LocalContext::Map(..) => device_context.DrawInstanced(
                    end - start,
                    1,
                    start,
                    0,
                ),
            }
        }
    }

    pub fn section_geometry_vertices(&self, section: TrailSectionIndex) -> Option<ops::Range<u32>> {
        let section = section as usize;
        let end = *self.vbuffer_section_end.get(section)?;
        let start = match section {
            #[cfg(todo = "unnecessary")]
            section => section.checked_sub(1).map(|prev| unsafe {
                *self.vbuffer_section_end.get_unchecked(prev)
            }).unwrap_or(0),
            section =>
                *self.vbuffer_section_end.get(section.wrapping_sub(1))
                    .unwrap_or(&0),
        };
        Some(start..end)
    }

    /// mark broken
    pub fn disable(&mut self) {
        self.section_vbuffer = None;
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
