use {
    super::{super::dx11::InstanceBufferData, ObjectRenderBacking, ObjectRenderMetadata},
    crate::{
        space::{
            dx11::RenderBackend,
            resources::{obj_format::material::ColouredMaterialTexture, ObjMaterial},
        },
        timer::{TimerFileDirection, TimerFileMarker},
    },
    glam::{Vec3, Vec4},
    std::{
        borrow::Cow,
        path::PathBuf,
        sync::{Arc, LazyLock},
    },
    taimi_d3d::{
        dx11::{buffer::BufferOf, prelude::*},
        state::PrimitiveTopology,
    },
};

pub struct ObjectBacking {
    pub name: Arc<str>,
    pub render: ObjectRenderBacking,
}

impl ObjectBacking {
    pub fn create_marker(
        render_backend: &RenderBackend,
        marker: &TimerFileMarker,
        path: PathBuf,
    ) -> anyhow::Result<Self> {
        let key = Arc::<str>::from(marker.texture.to_string_lossy());
        let (material, texture_key) = match crate::TEXTURES.lookup_resource(&key) {
            None => {
                let timer_path = if let Some(timer_path_parent) = path.parent() {
                    Cow::Owned(timer_path_parent.join(&marker.texture))
                } else {
                    Cow::Borrowed(&marker.texture)
                };
                log::debug!("Loading texture from {}!", timer_path.display());
                crate::TEXTURES.request_begin_file(key.clone(), timer_path.into_owned())?;
                (Default::default(), Some(key))
            },
            Some(None) => (Default::default(), Some(key)),
            Some(Some(texture)) => (
                ObjMaterial::new_diffuse(ColouredMaterialTexture { texture, colour: Vec3::ONE }).into(),
                None,
            ),
        };
        let shaders = render_backend.shaders.pair_named("poi")?;
        let model_matrix = marker.model_matrix();
        let colour = Vec4::ONE.with_w(marker.opacity);
        let ibd = [InstanceBufferData { world: model_matrix, colour }];
        let render = ObjectRenderBacking {
            instance_buffer: BufferOf::<InstanceBufferData>::new_with_data(
                &render_backend.device,
                Ok(&ibd),
                (),
            )?
            .buffer
            .into(),
            vertex_buffer: render_backend.shared_quad()?,
            shaders,
            metadata: ObjectRenderMetadata {
                #[cfg(todo)]
                model: Model::quad(),
                material,
                texture_key,
                model_matrix,
                topology: PrimitiveTopology::TriangleList,
            },
        };
        let marker = Self {
            name: Self::marker_backing_name().clone(),
            render,
        };
        Ok(marker)
    }
    fn marker_backing_name() -> &'static Arc<str> {
        static NAME: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from("Marker"));
        &*NAME
    }

    pub fn create_direction(
        render_backend: &RenderBackend,
        dir: &TimerFileDirection,
        path: PathBuf,
    ) -> anyhow::Result<Self> {
        let key = Arc::<str>::from(dir.texture.to_string_lossy());
        let (material, texture_key) = match crate::TEXTURES.lookup_resource(&key) {
            None => {
                let timer_path = if let Some(timer_path_parent) = path.parent() {
                    Cow::Owned(timer_path_parent.join(&dir.texture))
                } else {
                    Cow::Borrowed(&dir.texture)
                };
                log::debug!("Loading texture from {}!", timer_path.display());
                crate::TEXTURES.request_begin_file(key.clone(), timer_path.into_owned())?;
                (Default::default(), Some(key))
            },
            Some(None) => (Default::default(), Some(key)),
            Some(Some(texture)) => (
                ObjMaterial::new_diffuse(ColouredMaterialTexture { texture, colour: Vec3::ONE }).into(),
                None,
            ),
        };
        let shaders = render_backend.shaders.pair_named("trail")?;
        let model_matrix = glam::Mat4::IDENTITY;
        let colour = Vec4::ONE.with_w(dir.opacity);
        let ibd = [InstanceBufferData { world: model_matrix, colour }];
        let render = ObjectRenderBacking {
            instance_buffer: BufferOf::<InstanceBufferData>::new_with_data(
                &render_backend.device,
                Ok(&ibd),
                (),
            )?
            .buffer
            .into(),
            vertex_buffer: render_backend.shared_quad()?,
            shaders,
            metadata: ObjectRenderMetadata {
                material,
                texture_key,
                model_matrix,
                topology: PrimitiveTopology::TriangleList,
            },
        };
        let dir = Self {
            name: Self::direction_backing_name().clone(),
            render,
        };
        Ok(dir)
    }
    fn direction_backing_name() -> &'static Arc<str> {
        static NAME: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from("Direction"));
        &*NAME
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
