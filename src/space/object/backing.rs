use {
    super::{super::dx11::InstanceBufferData, ObjectRenderBacking, ObjectRenderMetadata},
    crate::{
        space::{
            dx11::RenderBackend,
            resources::{
                obj_format::material::ColouredMaterialTexture,
                Model,
                ObjMaterial,
                Texture,
            },
        },
        timer::TimerMarker,
    },
    glam::{Vec3, Vec4},
    std::path::PathBuf,
    taimi_d3d::{
        dx11::{buffer::BufferOf, prelude::*},
        state::PrimitiveTopology,
    },
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
        let shaders = render_backend.shaders.pair_named("textured")?;
        let model = Model::quad();
        let model_matrix = marker.model_matrix();
        let ibd = [InstanceBufferData {
            world: model_matrix,

            colour: Vec4::ONE,
        }];
        let render = ObjectRenderBacking {
            instance_buffer: BufferOf::<InstanceBufferData>::new_with_data(
                &render_backend.device,
                Ok(&ibd),
                (),
            )?
            .buffer
            .into(),
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
        device: &Dx11Device,
        device_context: &Dx11Context,
        data: &[InstanceBufferData],
    ) -> anyhow::Result<()> {
        self.render.set_and_draw(slot, device, device_context, data)
    }
}
