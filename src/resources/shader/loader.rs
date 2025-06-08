use {
    super::{PixelShader, ShaderDescription, ShaderKind, VertexShader}, glob::Paths, include_dir::include_dir, std::{collections::HashMap, path::Path, sync::Arc}, windows::Win32::Graphics::Direct3D11::ID3D11Device
};

pub static SHADERS_DIR: include_dir::Dir = include_dir!("$CARGO_MANIFEST_DIR/shaders");

pub type VertexShaders = HashMap<String, Arc<VertexShader>>;
pub type PixelShaders = HashMap<String, Arc<PixelShader>>;

pub struct ShaderLoader(pub VertexShaders, pub PixelShaders);

impl ShaderLoader {
    pub fn load(addon_dir: &Path, device: &ID3D11Device) -> anyhow::Result<Self> {
        log::info!("Beginning shader setup!");
        let mut shader_descriptions: Vec<ShaderDescription> = Vec::new();
        let mut shaders: ShaderLoader = Self(HashMap::new(), HashMap::new());
        let shader_description_paths = SHADERS_DIR.find("*.shaderdesc")?;
        for shader_description_path in shader_description_paths {
            if let Some(file) = shader_description_path.as_file() {
                if let Some(content) = file.contents_utf8() {
                    let shader_description =
                        ShaderDescription::load_from_str(content.to_string())?;
                    shader_descriptions.extend(shader_description);
                }
            }
        }
        for shader_description in shader_descriptions {
            match shader_description.kind {
                ShaderKind::Vertex => {
                    let shader = Arc::new(VertexShader::create(
                        device,
                        &shader_description,
                    )?);
                    shaders.0.insert(shader_description.identifier, shader);
                }
                ShaderKind::Pixel => {
                    let shader = Arc::new(PixelShader::create(
                        device,
                        &shader_description,
                    )?);
                    shaders.1.insert(shader_description.identifier, shader);
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
