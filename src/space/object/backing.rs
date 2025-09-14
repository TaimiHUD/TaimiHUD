use {
    super::{super::dx11::InstanceBufferData, ObjectRenderBacking, ObjectRenderMetadata},
    crate::{
        space::{
            dx11::{InstanceBuffer, RenderBackend},
            object::PrimitiveTopology,
            resources::{
                obj_format::material::ColouredMaterialTexture, Model, ObjMaterial, ShaderPair,
                Texture,
            },
        },
        timer::TimerMarker,
    },
    glam::{Vec3, Vec4},
    std::path::PathBuf,
    windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext},
};

pub struct ObjectBacking {
    pub name: String,
    pub render: ObjectRenderBacking,
}

impl ObjectBacking {
    pub fn create_marker(
        render_backend: &RenderBackend,
        marker: &TimerMarker,
        path: PathBuf,
    ) -> anyhow::Result<Self> {
        let timer_path = if let Some(timer_path_parent) = path.parent() {
            timer_path_parent.join(marker.texture.clone())
        } else {
            marker.texture.clone()
        };
        log::info!("Loading texture from {timer_path:?}!");
        let texture = Texture::load(&render_backend.device, &timer_path)?;
        let shaders = ShaderPair(
            render_backend.shaders.0["textured"].clone(),
            render_backend.shaders.1["textured"].clone(),
        );
        let model = Model::quad();
        let model_matrix = marker.model_matrix();
        let ibd = [InstanceBufferData {
            world: model_matrix,

            colour: Vec4::ONE,
        }];
        let render = ObjectRenderBacking {
            instance_buffer: InstanceBuffer::create(&render_backend.device, &ibd)?,
            vertex_buffer: model.to_buffer(&render_backend.device)?,
            shaders,
            metadata: ObjectRenderMetadata {
                model,
                material: ObjMaterial {
                    ambient: None,
                    specular: None,
                    shininess: None,
                    dissolve: None,
                    normal: None,
                    diffuse: Some(ColouredMaterialTexture {
                        texture,
                        colour: Vec3::ONE,
                    }),
                },
                model_matrix,
                topology: PrimitiveTopology::TriangleList,
            },
        };
        let marker = Self {
            name: "Marker".to_string(),
            render,
        };
        Ok(marker)
    }

    pub fn set_and_draw(
        &self,
        slot: u32,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[InstanceBufferData],
    ) -> anyhow::Result<()> {
        self.render.set_and_draw(slot, device, device_context, data)
    }
}
