use {
    super::{PixelShader, ShaderDescription, ShaderKind, VertexShader},
    glob::Paths,
    std::{collections::HashMap, path::Path, sync::Arc},
    windows::Win32::Graphics::Direct3D11::ID3D11Device,
};

pub type VertexShaders = HashMap<String, Arc<VertexShader>>;
pub type PixelShaders = HashMap<String, Arc<PixelShader>>;

pub struct ShaderLoader(pub VertexShaders, pub PixelShaders);

impl ShaderLoader {
    pub fn load(addon_dir: &Path, device: &ID3D11Device) -> anyhow::Result<Self> {
        log::info!("Beginning shader setup!");
        let shader_folder = addon_dir.join("shaders");
        let mut shader_descriptions: Vec<ShaderDescription> = Vec::new();
        let mut shaders: ShaderLoader = Self(HashMap::new(), HashMap::new());
        if shader_folder.exists() {
            let shader_description_paths: Paths = glob::glob(
                shader_folder
                    .join("*.shaderdesc")
                    .to_str()
                    .expect("Shader load pattern is unparseable"),
            )?;
            for shader_description_path in shader_description_paths {
                let shader_description =
                    ShaderDescription::load(&shader_folder.join(shader_description_path?))?;
                shader_descriptions.extend(shader_description);
            }
            for shader_description in shader_descriptions {
                match shader_description.kind {
                    ShaderKind::Vertex => {
                        let shader = Arc::new(VertexShader::create(
                            &shader_folder,
                            device,
                            &shader_description,
                        )?);
                        shaders.0.insert(shader_description.identifier, shader);
                    }
                    ShaderKind::Pixel => {
                        let shader = Arc::new(PixelShader::create(
                            &shader_folder,
                            device,
                            &shader_description,
                        )?);
                        shaders.1.insert(shader_description.identifier, shader);
                    }
                }
            }
        }
        log::info!(
            "Finished shader setup. {} vertex shaders, {} pixel shaders loaded!",
            shaders.0.len(),
            shaders.1.len()
        );
        Ok(shaders)
    }
}
