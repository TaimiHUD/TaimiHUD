use {
    crate::space::{
        dx11::VertexBuffer,
        pack::{ActivePack, TrailSectionExt},
        resources::{Model, Texture, Vertex},
        DrawSpace,
    },
    anyhow::Context,
    core::f32,
    glamour::{Box3, Vector3},
    std::sync::Arc,
    taimi_pack::Trail,
    windows::Win32::Graphics::{
        Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        Direct3D11::{ID3D11Device, ID3D11DeviceContext},
    },
};

pub struct ActiveTrail {
    pub category_idx: usize,
    pub filtered: bool,
    pub render_bookmark: usize,

    // Segment data.
    pub section_bounds: Vec<Box3<DrawSpace>>,

    // World render data.
    pub texture: Arc<Texture>,
    pub section_vbuffer: VertexBuffer,
    pub section_bookmarks: Vec<u32>,

    // Map render data.
    pub map_vbuffer: Option<VertexBuffer>,
}

impl ActiveTrail {
    pub fn build(
        loader: &mut ActivePack,
        trail: &Trail,
        index: usize,
        category_idx: usize,
        render_bookmark: usize,
        device: &ID3D11Device,
    ) -> anyhow::Result<ActiveTrail> {
        let texture_handle = trail.texture_name()
            .ok_or_else(|| anyhow::anyhow!("TODO: Add a fallback texture for trails"))?;
        let texture_handle = loader.register_texture(texture_handle);
        let texture = loader
            .get_or_load_texture(texture_handle, device)
            .context("Loading trail texture")?;

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut section_bookmarks: Vec<u32> = vec![0];
        let mut section_bounds = Vec::new();

        for (isec, section) in trail.data.sections.iter().enumerate() {
            if section.points.is_empty() {
                log::warn!("Section {isec} is empty.");
                continue;
            }

            /// Current hardcoded value in BlishHUD Pathing. We could make it configurable later.
            const RESOLUTION: f32 = 20.0;
            const TRAIL_WIDTH: f32 = 20.0 * 0.0254;

            // Interpolate points to be no more than RESOLUTION apart.
            let mut points = Vec::with_capacity(section.points.len());
            let mut prev_point = section.points[0];
            points.push(prev_point);
            for &point in section.points.iter().skip(1) {
                let dist = prev_point.distance(point);
                let segments = (dist / RESOLUTION) as i32;
                for i in 0..segments {
                    let s = (i + 1) as f32 / (segments + 1) as f32;
                    points.push(prev_point.lerp(point, s));
                }

                points.push(point);
                prev_point = point;
            }

            log::info!(
                "Section {isec} added {} interpolation points ({} -> {}).",
                points.len() - section.points.len(),
                section.points.len(),
                points.len(),
            );

            let mut cur_point = points[0];
            let mut last_offset = Vector3::ZERO;
            let mut flip_over = 1.0f32;
            let normal_offset = TRAIL_WIDTH * trail.scale();
            let mut mod_distance = Vector3::ZERO;

            let mut distance = 0.0f32;
            for &next_point in points.iter().skip(1) {
                let path_direction = next_point - cur_point;
                let offset = path_direction.cross(Vector3::Y);
                let offset = if trail.is_wall() {
                    path_direction.cross(offset)
                } else {
                    offset
                };
                let offset = offset.normalize();

                if last_offset != Vector3::ZERO && offset.dot(last_offset) < 0.0 {
                    flip_over *= -1.0;
                }

                mod_distance = offset * normal_offset * flip_over;

                vertices.push(Vertex {
                    position: (cur_point - mod_distance).into(),
                    colour: glam::Vec3::ONE,
                    normal: glam::Vec3::ZERO,
                    texture: glam::vec2(1.0, distance / (TRAIL_WIDTH * 2.0) - 1.0),
                });
                vertices.push(Vertex {
                    position: (cur_point + mod_distance).into(),
                    colour: glam::Vec3::ONE,
                    normal: glam::Vec3::ZERO,
                    texture: glam::vec2(0.0, distance / (TRAIL_WIDTH * 2.0) - 1.0),
                });

                distance += path_direction.length();
                last_offset = offset;
                cur_point = next_point;
            }

            vertices.push(Vertex {
                position: (cur_point - mod_distance).into(),
                colour: glam::Vec3::ONE,
                normal: glam::Vec3::ZERO,
                texture: glam::vec2(1.0, distance / (TRAIL_WIDTH * 2.0) - 1.0),
            });
            vertices.push(Vertex {
                position: (cur_point + mod_distance).into(),
                colour: glam::Vec3::ONE,
                normal: glam::Vec3::ZERO,
                texture: glam::vec2(0.0, distance / (TRAIL_WIDTH * 2.0) - 1.0),
            });

            section_bookmarks.push(vertices.len() as u32);
            section_bounds.push(section.bounds());
        }

        if vertices.is_empty() {
            log::error!(
                "Empty trail {}:{}",
                trail.category,
                trail.guid,
            );
        }

        let model = Model::from_vertices(vertices);
        let section_vbuffer = model.to_buffer(device).context("Creating trail vbuffer")?;

        Ok(ActiveTrail {
            category_idx,
            filtered: false,
            section_bounds,
            texture: texture.clone(),
            section_vbuffer,
            section_bookmarks,
            map_vbuffer: None,
            render_bookmark,
        })
    }

    pub fn update(pack: &mut ActivePack, trail_idx: usize) {
        let _ = pack;
        let _ = trail_idx;
    }

    /// Draw a trail segment.
    /// PREREQUISITES: Trail shaders must already be set.
    pub fn draw_section(&self, device_context: &ID3D11DeviceContext, section: usize) {
        if self.filtered {
            return;
        }

        self.texture.set(device_context, 0);

        unsafe {
            device_context.IASetVertexBuffers(
                0,
                1,
                Some(&self.section_vbuffer.buffer as *const _ as *const _),
                Some(&self.section_vbuffer.stride),
                Some(&self.section_vbuffer.offset),
            );
            device_context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            device_context.Draw(
                self.section_bookmarks[section + 1] - self.section_bookmarks[section],
                self.section_bookmarks[section],
            );
        }
    }
}
