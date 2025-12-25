use {
    crate::{
        controller::pathing::{
            visible::{LoadedTrail, LoadedTrailGeometry},
            space::SpaceLoader,
            shared::{SharedPackInfo, LoadedTrailRef, LoadedTrailShared},
        },
        exports::runtime::{
            textures::{TextureKey, TextureSlot},
            Counter,
        },
        space::{
            pack::PoiCommonRenderData,
            resources::Model,
        },
        resources::Texture,
    },
    std::{ops, mem},
    anyhow::Context,
    std::sync::Arc,
    taimi_d3d::dx11::{
        buffer::VertexBuffer,
        prelude::*,
    },
    taimi_hoard::lazyfmt,
    taimi_meta::{
        packs::TrailSectionPath,
        ui::LocalContext,
    },
    taimi_pack::attributes::AttrString,
};

/// World render data
pub struct TrailRender {
    pub texture_handle: Option<TextureKey>,
    pub texture: Option<TextureSlot>,
    pub section_vbuffer: Option<VertexBuffer>,
}

impl TrailRender {
    pub fn empty() -> Self {
        Self {
            texture_handle: None,
            texture: None,
            section_vbuffer: None,
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
        loader: &mut SpaceLoader<'_>,
        mut geometry: LoadedTrailGeometry,
    ) -> anyhow::Result<()> {
        if self.section_vbuffer.is_some() {
            return Ok(())
        }
        let model = Model::from_vertices(geometry.take_vertices());
        let section_vbuffer = model.to_buffer(loader.device).context("Creating trail vbuffer");
        match section_vbuffer {
            Ok(vbuffer) => {
                STATS_TRAIL_VERTEX_SIZE.increment_by(|| vbuffer.size());
                self.section_vbuffer = Some(vbuffer);
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    pub fn update(&mut self, _device: &Dx11Device, pack_info: &SharedPackInfo, ltrail: Option<LoadedTrailRef<'_>>) {
        let texture = ltrail.as_ref().and_then(|ltrail| ltrail.trail_attrs().texture.as_ref());
        SpaceLoader::setup_texture(&mut self.texture_handle, &mut self.texture, pack_info, texture);
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
    pub fn draw_section(&self, device_context: &Dx11Context, ltrail: &LoadedTrailShared, section: TrailSectionPath, ctx: LocalContext) {
        let Some(ops::Range { start, end }) = ltrail.section_info.section_geometry_vertices(section) else {
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

    pub fn is_empty(&self) -> bool {
        self.section_vbuffer.is_none()
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
