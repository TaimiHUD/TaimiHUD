use {
    super::{attributes::MarkerAttributes, taco_safe_name, taco_xml_to_guid, Pack},
    crate::{
        marker::atomic::MapSpace,
        space::{
            dx11::{RenderBackend, VertexBuffer},
            resources::{Model, ShaderPair, Texture, Vertex},
        },
    },
    anyhow::Context,
    glam::{vec2, vec3, Mat4, Vec3, Vec4},
    glamour::{Box3, Point3, Vector3},
    std::sync::Arc,
    uuid::Uuid,
    windows::Win32::Graphics::{
        Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        Direct3D11::{
            ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, D3D11_BIND_CONSTANT_BUFFER,
            D3D11_BUFFER_DESC, D3D11_SUBRESOURCE_DATA, D3D11_USAGE_DEFAULT,
        },
    },
};

pub struct Poi {
    pub category: String,
    pub guid: Uuid,
    pub map_id: i32,
    pub position: Point3<MapSpace>,
    pub attributes: MarkerAttributes,
}

impl Poi {
    pub fn from_xml(
        pack: &mut Pack,
        attrs: Vec<xml::attribute::OwnedAttribute>,
    ) -> anyhow::Result<Poi> {
        let mut category = String::new();
        let mut map_id = None;
        let mut pos_x = None;
        let mut pos_y = None;
        let mut pos_z = None;
        let mut guid = None;
        let mut attributes = MarkerAttributes::default();

        for attr in attrs {
            if attr.name.local_name.eq_ignore_ascii_case("type") {
                category = taco_safe_name(&attr.value, true);
            } else if attr.name.local_name.eq_ignore_ascii_case("MapID") {
                map_id = Some(attr.value.parse().context("Parse POI MapID")?);
            } else if attr.name.local_name.eq_ignore_ascii_case("xpos") {
                pos_x = Some(attr.value.parse().context("Parse POI xpos")?);
            } else if attr.name.local_name.eq_ignore_ascii_case("ypos") {
                pos_y = Some(attr.value.parse().context("Parse POI ypos")?);
            } else if attr.name.local_name.eq_ignore_ascii_case("zpos") {
                pos_z = Some(attr.value.parse().context("Parse POI zpos")?);
            } else if attr.name.local_name.eq_ignore_ascii_case("guid") {
                guid = Some(taco_xml_to_guid(&attr.value));
            } else if !attributes.try_add(pack, &attr) {
                log::warn!("Unknown POI attribute '{}'", attr.name.local_name);
            }
        }

        let Some(map_id) = map_id else {
            anyhow::bail!("POI must have MapID");
        };

        let (Some(pos_x), Some(pos_y), Some(pos_z)) = (pos_x, pos_y, pos_z) else {
            anyhow::bail!("POI must have xpos, ypos, and zpos");
        };
        let position = glamour::point3!(pos_x, pos_y, pos_z);

        let guid = guid.unwrap_or_default();

        Ok(Poi {
            category,
            guid,
            map_id,
            position,
            attributes,
        })
    }
}

pub struct PoiCommonRenderData {
    // Common fixed data.
    /// POI shader.
    pub shaders: ShaderPair,
    /// Quad trianglestrip.
    quad_vb: VertexBuffer,

    // Common dynamic data.
    /// Billboard transform for current camera.
    billboard: Mat4,
    /// Constant buffer data for POI shader.
    poi_cb: ID3D11Buffer,
}

// NOTES: Please reference https://github.com/blish-hud/Pathing/blob/main/Entity/StandardMarker.World.cs

impl PoiCommonRenderData {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<PoiCommonRenderData> {
        let quad_vb = Model::from_vertices(POI_QUAD_VERTICES.into()).to_buffer(&backend.device)?;

        let poi_cb = create_poi_cb(&backend.device)?;

        Ok(PoiCommonRenderData {
            shaders: ShaderPair(
                backend
                    .shaders
                    .0
                    .get("poi")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Failed to load POI vertex shader"))?,
                backend
                    .shaders
                    .1
                    .get("poi")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Failed to load POI pixel shader"))?,
            ),
            quad_vb,
            billboard: Mat4::IDENTITY,
            poi_cb,
        })
    }

    pub fn camera_update(&mut self, cam_front: Vec3, cam_up: Vec3) {
        let cam_front = cam_front.normalize();
        let cam_right = cam_front.cross(cam_up.normalize()).normalize();
        let cam_up = cam_right.cross(cam_front).normalize();

        self.billboard = Mat4::from_cols(
            cam_right.extend(0.0),
            cam_up.extend(0.0),
            -cam_front.extend(0.0),
            Vec3::ZERO.extend(1.0),
        );
    }
}

const POI_QUAD_VERTICES: [Vertex; 4] = [
    Vertex {
        position: vec3(-1.0, -1.0, 0.0),
        colour: Vec3::ONE,
        normal: Vec3::Z,
        texture: vec2(1.0, 0.0),
    },
    Vertex {
        position: vec3(1.0, -1.0, 0.0),
        colour: Vec3::ONE,
        normal: Vec3::Z,
        texture: vec2(0.0, 0.0),
    },
    Vertex {
        position: vec3(-1.0, 1.0, 0.0),
        colour: Vec3::ONE,
        normal: Vec3::Z,
        texture: vec2(1.0, 1.0),
    },
    Vertex {
        position: vec3(1.0, 1.0, 0.0),
        colour: Vec3::ONE,
        normal: Vec3::Z,
        texture: vec2(0.0, 1.0),
    },
];

fn create_poi_cb(device: &ID3D11Device) -> anyhow::Result<ID3D11Buffer> {
    let constant_buffer_desc = D3D11_BUFFER_DESC {
        ByteWidth: size_of::<PoiSpriteData>().next_multiple_of(16) as u32,
        //Usage: D3D11_USAGE_DYNAMIC,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        //CPUAccessFlags: D3D11_CPU_ACCESS_WRITE,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: 0,
    };

    let initial = PoiSpriteData {
        model: Default::default(),
        tint: Default::default(),
    };

    let constant_subresource_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: &initial as *const PoiSpriteData as *const _,
        .. D3D11_SUBRESOURCE_DATA::default()
    };

    let mut constant_buffer_ptr: Option<ID3D11Buffer> = None;
    let constant_buffer = unsafe {
        device
            .CreateBuffer(
                &constant_buffer_desc,
                Some(&constant_subresource_data),
                Some(&mut constant_buffer_ptr),
            )
            .context("Creating POI ConstantBuffer")?;
        constant_buffer_ptr.expect("ptr should never be NULL on S_OK")
    };
    Ok(constant_buffer)
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct PoiSpriteData {
    model: Mat4,
    tint: Vec4,
}

pub struct ActivePoi {
    pub category_idx: usize,
    pub filtered: bool,
    pub bounds: Box3<MapSpace>,
    pub position: Point3<MapSpace>,
    pub tint: Vec4,
    pub opacity: f32,
    pub scale: f32,
    pub icon: Arc<Texture>,
}

impl ActivePoi {
    pub fn build(
        pack: &mut Pack,
        index: usize,
        device: &ID3D11Device,
    ) -> anyhow::Result<ActivePoi> {
        let category_idx = pack
            .categories
            .all_categories
            .get_index_of(&pack.pois[index].category)
            .unwrap_or(0);
        let icon_handle = pack.pois[index]
            .attributes
            .icon_file
            .ok_or_else(|| anyhow::anyhow!("POI is missing icon. TODO: default icon?"))?;
        let icon = pack.get_or_load_texture(icon_handle, device)?;

        let attrs = &pack.pois[index].attributes;
        let position =
            pack.pois[index].position + Vector3::ZERO.with_y(attrs.height_offset.unwrap_or(0.0));
        let scale = attrs.icon_size.unwrap_or(1.0);
        let tint = attrs.tint.unwrap_or(Vec4::ONE);
        let opacity = attrs.alpha.unwrap_or(1.0);

        let edge_len = scale * 2.0;
        let max_diagonal = (edge_len.powi(2) * 2.0).sqrt();
        let bounds = Box3::from_origin_and_size(position, glamour::size3!(max_diagonal));

        Ok(ActivePoi {
            category_idx,
            filtered: false,
            bounds,
            position,
            tint,
            opacity,
            scale,
            icon,
        })
    }

    pub fn update(pack: &mut Pack, poi_idx: usize) {
        let _ = pack;
        let _ = poi_idx;
    }

    pub fn draw(&self, device_context: &ID3D11DeviceContext, poi_common: &mut PoiCommonRenderData) {
        if self.filtered {
            return;
        }

        let sprite_data = PoiSpriteData {
            model: Mat4::from_translation(self.position.into())
                * poi_common.billboard
                * Mat4::from_scale(Vec3::splat(self.scale)),
            tint: self.tint * Vec4::ONE.with_w(self.opacity),
        };

        self.icon.set(device_context, 0);
        unsafe {
            device_context.UpdateSubresource(
                &poi_common.poi_cb,
                0,
                None,
                &sprite_data as *const _ as *const _,
                0,
                0,
            );
            device_context.VSSetConstantBuffers(1, Some(cb_as_cb_list(&poi_common.poi_cb)));

            device_context.IASetVertexBuffers(
                0,
                1,
                Some(&poi_common.quad_vb.buffer as *const _ as *const _),
                Some(&poi_common.quad_vb.stride),
                Some(&poi_common.quad_vb.offset),
            );
            device_context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            device_context.Draw(4, 0);
        }
    }
}

/// SAFETY: std::mem::transmute validates that both types are of the same size, therefore
/// validating that Option<ID3D11Buffer> has the same ABI as ID3D11Buffer.
unsafe fn cb_as_cb_list(cb: &ID3D11Buffer) -> &[Option<ID3D11Buffer>; 1] {
    std::mem::transmute(cb)
}
