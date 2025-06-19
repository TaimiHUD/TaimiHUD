use {
    super::{
        attributes::MarkerAttributes, loader::PackLoaderContext, taco_safe_name, taco_xml_to_guid,
        Pack,
    },
    crate::{
        marker::atomic::MapSpace,
        space::{
            dx11::VertexBuffer,
            resources::{Model, Texture, Vertex},
        },
    },
    anyhow::Context,
    core::f32,
    glamour::{point3, vec3, Box3, Point3, Union, Vector3},
    std::{io::BufReader, sync::Arc},
    uuid::Uuid,
    windows::{
        core::Interface as _,
        Win32::Graphics::{
            Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            Direct3D11::{ID3D11Buffer, ID3D11Device, ID3D11DeviceContext},
        },
    },
};

pub struct Trail {
    pub category: String,
    pub guid: Uuid,
    pub data: TrailData,
    pub attributes: MarkerAttributes,
}

impl Trail {
    pub fn from_xml(
        pack: &mut Pack,
        ctx: &mut impl PackLoaderContext,
        attrs: Vec<xml::attribute::OwnedAttribute>,
    ) -> anyhow::Result<Trail> {
        let mut category = String::new();
        let mut trail_path = None;
        let mut guid = None;
        let mut attributes = MarkerAttributes::default();

        for attr in attrs {
            if attr.name.local_name.eq_ignore_ascii_case("type") {
                category = taco_safe_name(&attr.value, true);
            } else if attr.name.local_name.eq_ignore_ascii_case("traildata") {
                trail_path = Some(attr.value);
            } else if attr.name.local_name.eq_ignore_ascii_case("guid") {
                guid = Some(taco_xml_to_guid(&attr.value));
            } else if !attributes.try_add(pack, &attr) {
                log::warn!("Unknown Trail attribute '{}'", attr.name.local_name);
            }
        }

        if category.is_empty() {
            anyhow::bail!("No 'type' specified for Trail");
        }

        let Some(trail_path) = trail_path else {
            anyhow::bail!("No 'trailData' specified for Trail '{category}'");
        };

        let data = read_trl_file(BufReader::new(ctx.load_asset(&trail_path)?), &trail_path)?;
        let guid = guid.unwrap_or_default();

        Ok(Trail {
            category,
            guid,
            data,
            attributes,
        })
    }
}

pub struct TrailData {
    pub map_id: i32,
    pub sections: Vec<TrailSection>,
}

pub struct TrailSection {
    pub points: Vec<Point3<MapSpace>>,
    pub bounds: Box3<MapSpace>,
}

pub fn read_trl_file(mut reader: impl std::io::Read, name: &str) -> anyhow::Result<TrailData> {
    let mut buf32 = [0u8; 4];
    reader
        .read_exact(&mut buf32)
        .context("Reading trail version")?;
    if i32::from_le_bytes(buf32) != 0 {
        anyhow::bail!("Trl version '0' is the only known valid format version");
    }

    reader
        .read_exact(&mut buf32)
        .context("Reading trail map_id")?;
    let map_id = i32::from_le_bytes(buf32);

    let mut sections = vec![];
    let mut current_section = vec![];

    const NEG_BOX: Box3<MapSpace> = glamour::Box3 {
        min: point3!(f32::INFINITY, f32::INFINITY, f32::INFINITY),
        max: point3!(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
    };

    let mut bounds = NEG_BOX;
    let mut read_more = true;
    while read_more {
        let point_data = match read_point(&mut reader) {
            Ok(point_data) => point_data,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                read_more = false;
                EMPTY_POINT
            }
            Err(e) => return Err(e).context("Reading trail sections"),
        };

        if point_data == EMPTY_POINT {
            if !current_section.is_empty() {
                sections.push(TrailSection {
                    points: std::mem::take(&mut current_section),
                    bounds,
                });
                bounds = NEG_BOX;
            } else {
                log::warn!("Empty trail section in {name}");
            }
        } else {
            let x = f32::from_le_bytes(point_data[0]);
            let y = f32::from_le_bytes(point_data[1]);
            let z = f32::from_le_bytes(point_data[2]);
            let point = point3!(x, y, z);
            current_section.push(point);
            bounds = bounds.union(Box3::new(point, point));
        }
    }

    Ok(TrailData { map_id, sections })
}

const EMPTY_POINT: [[u8; 4]; 3] = [[0; 4]; 3];

fn read_point(reader: &mut impl std::io::Read) -> std::io::Result<[[u8; 4]; 3]> {
    let mut point_data = [[0; 4]; 3];
    reader.read_exact(&mut point_data[0])?;
    reader.read_exact(&mut point_data[1])?;
    reader.read_exact(&mut point_data[2])?;
    Ok(point_data)
}

pub struct ActiveTrail {
    pub category_idx: usize,
    pub filtered: bool,
    pub render_bookmark: usize,

    // Segment data.
    pub section_bounds: Vec<Box3<MapSpace>>,

    // World render data.
    pub texture: Arc<Texture>,
    pub section_vbuffer: VertexBuffer,
    pub section_bookmarks: Vec<u32>,

    // Map render data.
    pub map_vbuffer: Option<VertexBuffer>,
}

impl ActiveTrail {
    pub fn build(
        pack: &mut Pack,
        index: usize,
        render_bookmark: usize,
        device: &ID3D11Device,
    ) -> anyhow::Result<ActiveTrail> {
        let category_idx = pack
            .categories
            .all_categories
            .get_index_of(&pack.trails[index].category)
            .unwrap_or(0);
        let texture_handle = pack.trails[index]
            .attributes
            .texture
            .ok_or_else(|| anyhow::anyhow!("TODO: Add a fallback texture for trails"))?;
        let texture = pack
            .get_or_load_texture(texture_handle, device)
            .context("Loading trail texture")?;

        let attrs = &pack.trails[index].attributes;
        let is_wall = attrs.is_wall.unwrap_or(false);
        let trail_scale = attrs.trail_scale.unwrap_or(1.0);

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut section_bookmarks: Vec<u32> = vec![0];
        let mut section_bounds = Vec::new();

        for (isec, section) in pack.trails[index].data.sections.iter().enumerate() {
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
            let normal_offset = TRAIL_WIDTH * trail_scale;
            let mut mod_distance = Vector3::ZERO;

            let mut distance = 0.0f32;
            for &next_point in points.iter().skip(1) {
                let path_direction = next_point - cur_point;
                let offset = path_direction.cross(Vector3::Y);
                let offset = if is_wall {
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
            section_bounds.push(section.bounds);
        }

        if vertices.is_empty() {
            log::error!(
                "Empty trail {}:{}",
                pack.trails[index].category,
                pack.trails[index].guid,
            );
        }

        let model = Model::from_vertices(vertices);
        let section_vbuffer = model.to_buffer(device).context("Creating trail vbuffer")?;

        Ok(ActiveTrail {
            category_idx,
            filtered: false,
            section_bounds,
            texture,
            section_vbuffer,
            section_bookmarks,
            map_vbuffer: None,
            render_bookmark,
        })
    }

    pub fn update(pack: &mut Pack, trail_idx: usize) {
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
