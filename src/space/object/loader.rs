use {
    super::{ObjectBacking, ObjectDescription},
    crate::space::resources::{ObjFile, ShaderLoader},
    glob::Paths,
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::Arc,
    },
    taimi_d3d::dx11::prelude::*,
};

#[derive(Default, Debug)]
pub struct ObjectLoader(pub Vec<ObjectDescription>);

impl ObjectLoader {
    pub fn load_desc(model_dir: &Path) -> anyhow::Result<Self> {
        let mut object_loader = Self::default();

        if model_dir.exists() {
            let object_desc_paths: Paths = glob::glob(
                model_dir
                    .join("*.objdesc")
                    .to_str()
                    .expect("Model load pattern is unparseable"),
            )?;
            for object_desc_path in object_desc_paths {
                let object_desc_path = object_desc_path?;
                log::info!("Loading entities from {:?}", object_desc_path);
                let object_descs = ObjectDescription::load(&object_desc_path)?;
                for object_desc in object_descs.into_iter() {
                    object_loader.0.push(object_desc);
                }
            }
        }
        Ok(object_loader)
    }

    pub fn to_backings(
        &self,
        device: &Dx11Device,
        model_files: &HashMap<PathBuf, ObjFile>,
        shaders: &ShaderLoader,
    ) -> HashMap<String, Arc<ObjectBacking>> {
        self.0
            .iter()
            .filter_map(|o| {
                let backing = o
                    .to_backing(model_files, device, shaders)
                    .ok();
                if let Some(backing) = backing {
                    let backing = Arc::new(backing);
                    Some((backing.name.clone(), backing))
                } else {
                    None
                }
            })
            .collect()
    }
}
