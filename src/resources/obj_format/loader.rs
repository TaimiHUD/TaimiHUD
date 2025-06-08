use {
    super::{super::Model, ObjMaterial, ObjMaterials, ObjModel},
    crate::space::object::ObjectLoader,
    itertools::Itertools,
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
    },
    windows::Win32::Graphics::Direct3D11::ID3D11Device,
};

pub struct ObjFile {
    pub models: Vec<ObjModel>,
    pub materials: Option<ObjMaterials>,
}

impl ObjFile {
    pub fn load(
        models_dir: &Path,
        object_descs: &ObjectLoader,
    ) -> anyhow::Result<HashMap<PathBuf, ObjFile>> {
        let mut model_files: HashMap<PathBuf, ObjFile> = Default::default();
        let model_filenames: Vec<&PathBuf> = object_descs
            .0
            .iter()
            .map(|o| &o.location.file)
            .dedup()
            .collect();
        for model_filename in &model_filenames {
            let model_file = Self::load_file(&models_dir.join(model_filename))?;
            model_files.insert(model_filename.to_path_buf(), model_file);
        }
        Ok(model_files)
    }
    #[allow(dead_code)]
    pub fn load_list(&self, device: &ID3D11Device, idxs: Vec<usize>) -> Vec<ObjInstance> {
        idxs.iter()
            .map(|idx| self.load_idx(device, *idx, false))
            .collect()
    }

    pub fn load_idx(&self, device: &ID3D11Device, idx: usize, xzy: bool) -> ObjInstance {
        let model = &self.models[idx];
        ObjInstance {
            model: model.load(xzy),
            material: self
                .load_material_for_model(device, idx)
                .unwrap_or_default(),
        }
    }

    pub fn load_material_for_model(
        &self,
        device: &ID3D11Device,
        idx: usize,
    ) -> Option<ObjMaterial> {
        let mat_idx = &self.models[idx].0.mesh.material_id?;
        if let Some(materials) = &self.materials {
            materials.load(device, *mat_idx).ok()
        } else {
            None
        }
    }

    pub fn load_file(file: &Path) -> anyhow::Result<Self> {
        log::info!("Attempting to load {file:?}.");
        let (models, materials) = tobj::load_obj(
            file,
            &tobj::LoadOptions {
                merge_identical_points: false,
                reorder_data: false,
                single_index: true,
                triangulate: true,
                ignore_points: true,
                ignore_lines: true,
            },
        )?;
        let models: Vec<_> = models.into_iter().map(ObjModel).collect();
        let folder = file.parent();
        let materials = match (materials, folder) {
            (Ok(mats), Some(folder)) => {
                log::info!("Material load succeeded for obj model file {file:?}!");
                Some(ObjMaterials {
                    materials: mats,
                    folder: folder.to_path_buf(),
                })
            }
            (_, None) => {
                log::warn!("Material load failure for obj model file {file:?}, has no parent");
                None
            }
            (Err(err), _) => {
                log::warn!("Material load error for obj model file {file:?}: {err}");
                None
            }
        };
        if let Some(ref materials) = materials {
            log::info!(
                "File {file:?} loaded, contents: {} models, {} materials.",
                models.len(),
                materials.materials.len()
            );
        } else {
            log::info!(
                "File {file:?} loaded, contents: {} models, no materials.",
                models.len()
            );
        }
        Ok(Self { models, materials })
    }
}

pub struct ObjInstance {
    pub model: Model,
    pub material: ObjMaterial,
}
