use {
    crate::{
        controller::pathing::space::{SpaceTrail, SpaceLoader},
        exports::runtime::Counter,
        space::{
            pack::PoiCommonRenderData,
            resources::Model,
        },
    },
    anyhow::Context,
    core::mem,
    taimi_d3d::dx11::prelude::*,
    taimi_meta::{
        packs::TrailSectionIndex,
        ui::LocalContext,
    },
};

impl SpaceTrail {
    pub fn setup(
        &mut self,
        loader: &mut SpaceLoader<'_>,
        render_bookmark: u32,
    ) -> anyhow::Result<()> {
        self.render_bookmark = render_bookmark;
        let res = if self.texture.is_none() {
            let texture_handle = self.trail_attrs().texture
                .as_ref()
                .context("Trail is missing texture");
            let texture_handle = texture_handle.map(|h| loader.register_texture(h));
            let texture = texture_handle.and_then(|texture_handle| loader
                .get_or_load_texture(texture_handle)
                .context("Loading trail texture"));

            self.texture = match &texture {
                &Ok(texture) => Some(texture),
                Err(..) => None,
            };
            texture.map(drop)
        } else { Ok(()) };

        let res = if self.section_vbuffer.is_none() {
            let model = Model::from_vertices(mem::take(&mut self.vertices));
            let section_vbuffer = model.to_buffer(loader.device).context("Creating trail vbuffer");
            match section_vbuffer {
                Ok(vbuffer) => {
                    STATS_TRAIL_VERTEX_SIZE.increment_by(|| vbuffer.size());
                    self.section_vbuffer = Some(vbuffer);
                    res
                },
                Err(e) => match res {
                    Ok(()) => Err(e),
                    res @ Err(..) => {
                        log::error!("{e:#}");
                        res
                    }
                },
            }
        } else { Ok(()) };

        res
    }

    pub fn bind_texture(&self, device_context: &Dx11Context, common: &PoiCommonRenderData, _ctx: LocalContext) {
        let texture = self.texture.as_ref()
            .or_else(|| common.fallback_texture.as_ref());
        if let Some(texture) = texture {
            texture.set(device_context, 0);
        }
    }
    /// Draw a trail segment.
    /// PREREQUISITES: Trail shaders and texture must already be set.
    pub fn draw_section(&self, device_context: &Dx11Context, section: TrailSectionIndex, ctx: LocalContext) {
        let section = section as usize;

        if let Some(section_vbuffer) = &self.section_vbuffer {
            section_vbuffer.set(device_context, 0);
        }
        let (start, end) = match self.section_bookmarks.get(section..) {
            Some(&[start, end, ..]) => (start, end),
            _ => {
                log::error!("attempted to draw invalid section#{section} of trail in {}", self.category);
                return
            },
        };
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
}

#[cfg(feature = "statistics")]
impl Drop for SpaceTrail {
    fn drop(&mut self) {
        if let Some(vbuffer) = &self.section_vbuffer {
            STATS_TRAIL_VERTEX_SIZE.decrement_by(|| vbuffer.size());
        }
    }
}

pub static STATS_TRAIL_VERTEX_SIZE: Counter = Counter::DEFAULT;
