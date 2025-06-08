use {
    serde::{Deserialize, Serialize},
    std::{
        ffi::CString,
        fs::read_to_string,
        path::{Path, PathBuf},
    },
    strum_macros::Display,
    windows_strings::{s, HSTRING, PCSTR},
};

#[derive(Display, Debug, Serialize, Deserialize)]
pub enum ShaderKind {
    Vertex,
    Pixel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShaderDescription {
    pub identifier: String,
    pub kind: ShaderKind,
    pub path: PathBuf,
    pub entrypoint: String,
    pub layout_type: Option<ShaderLayout>,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum ShaderLayout {
    JustVertex,
    VertexInstance,
}

impl ShaderDescription {
    pub fn load(path: &PathBuf) -> anyhow::Result<Vec<Self>> {
        log::debug!("Attempting to load the shader description file at \"{path:?}\".");
        let mut file_data = read_to_string(path)?;
        json_strip_comments::strip(&mut file_data)?;
        let shader_description_data: Vec<Self> = serde_json::from_str(&file_data)?;
        Ok(shader_description_data)
    }
    pub fn get(&self, shader_folder: &Path) -> anyhow::Result<(HSTRING, PCSTR, CString)> {
        let filename = HSTRING::from(shader_folder.join(&self.path).as_os_str());
        let target = match self.kind {
            ShaderKind::Vertex => s!("vs_5_0"),
            ShaderKind::Pixel => s!("ps_5_0"),
        };
        let entrypoint_cstring = CString::new(self.entrypoint.clone())?;

        Ok((filename, target, entrypoint_cstring))
    }
}
