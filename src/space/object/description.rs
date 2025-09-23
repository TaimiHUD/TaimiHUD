use {
    crate::{
        space::{
            dx11::InstanceBufferData,
            object::{ObjectBacking, ObjectRenderBacking, ObjectRenderMetadata},
        },
        resources::{
            ModelKind, ObjFile, ObjInstance, ShaderPair,
            ShaderLoader,
        },
    },
    anyhow::anyhow,
    glam::Mat4,
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        fs::read_to_string,
        path::PathBuf,
    },
    taimi_d3d::{
        dx11::{
            prelude::*,
            buffer::BufferOf,
        },
        state::PrimitiveTopology,
    },
};

// TODO: cut down on this
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ModelLocationDescription {
    #[serde(default)]
    pub kind: ModelKind,
    pub file: PathBuf,
    pub index: usize,
}
fn default_pixel_shader() -> String {
    "generic".to_string()
}
fn default_vertex_shader() -> String {
    "generic".to_string()
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ObjectDescription {
    pub name: String,
    pub location: ModelLocationDescription,
    #[serde(default = "default_vertex_shader")]
    pub vertex_shader: String,
    #[serde(default = "default_pixel_shader")]
    pub pixel_shader: String,
    #[serde(default)]
    pub model_matrix: Mat4,
    #[serde(default)]
    pub xzy: bool,
    #[serde(default)]
    pub topology: PrimitiveTopology,
}

impl ObjectDescription {
    pub fn load(path: &PathBuf) -> anyhow::Result<Vec<Self>> {
        log::debug!("Attempting to load the entity description file at \"{path:?}\".");
        let mut file_data = read_to_string(path)?;
        json_strip_comments::strip(&mut file_data)?;
        let entity_description_data: Vec<Self> = serde_json::from_str(&file_data)?;
        Ok(entity_description_data)
    }

    pub fn get_shaders(
        &self,
        shaders: &ShaderLoader,
    ) -> ShaderPair {
        let vertex_shader =
            shaders.vertex.get(&self.vertex_shader).unwrap().clone();
        let pixel_shader = shaders.pixel.get(&self.pixel_shader).cloned().flatten();
        ShaderPair(vertex_shader, pixel_shader)
    }

    // TODO: make non-obj specific
    pub fn get_model_and_material(
        &self,
        device: &Dx11Device,
        model_files: &HashMap<PathBuf, ObjFile>,
    ) -> anyhow::Result<ObjInstance> {
        let model_file = model_files
            .get(&self.location.file)
            .ok_or_else(|| anyhow!("Could not load model file!"))?;
        let obj_model_data = model_file.load_idx(device, self.location.index, self.xzy);
        Ok(obj_model_data)
    }

    // TODO: make non-obj specific
    pub fn to_backing(
        &self,
        model_files: &HashMap<PathBuf, ObjFile>,
        device: &Dx11Device,
        shaders: &ShaderLoader,
    ) -> anyhow::Result<ObjectBacking> {
        let shaders = self.get_shaders(shaders);
        let obj_data = self.get_model_and_material(device, model_files)?;
        let model = obj_data.model;
        let material = obj_data.material;
        let vertex_buffer = model.to_buffer(device)?;
        let render_metadata = ObjectRenderMetadata {
            model,
            material,
            model_matrix: self.model_matrix,
            topology: self.topology,
        };

        let render_backing = ObjectRenderBacking {
            metadata: render_metadata,
            instance_buffer: BufferOf::<InstanceBufferData>::new_with_data(device, Err(0), ())?.buffer.into(),
            vertex_buffer,
            shaders,
        };

        let backing = ObjectBacking {
            name: self.name.clone(),
            render: render_backing,
        };

        Ok(backing)
    }
}
