#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use {
    anyhow::Context,
    std::{
        ffi::{CString, OsStr},
        fs::read_to_string,
        path::{Path, PathBuf},
    },
    taimi_d3d::{
        blob::Blob,
        dx11::{prelude::*, shader::InputLayoutElement},
        shader::{compile, ID3DInclude, ShaderDefinition, ShaderDefinitions, ShaderTarget},
    },
};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ShaderDescription {
    pub identifier: String,
    /// incomplete so it can be used as a base to construct a configured shader,
    /// disabling any sort of auto-load
    #[cfg_attr(feature = "serde", serde(default))]
    pub partial: bool,
    #[cfg_attr(feature = "serde", serde(rename = "kind"))]
    pub target: ShaderTarget,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "ShaderDefinitions::is_empty")
    )]
    pub defs: ShaderDefinitions,
    pub path: PathBuf,
    pub entrypoint: String,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub layout_type: Option<ShaderLayout>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ShaderLayout {
    #[cfg_attr(feature = "serde", serde(rename = "inputs"))]
    Inputs(Vec<InputLayoutElement>),
    #[cfg_attr(feature = "serde", serde(untagged))]
    Named(String),
}

impl ShaderDescription {
    #[cfg(feature = "serde")]
    pub fn load_from_bytes(data: impl Into<Vec<u8>>) -> anyhow::Result<Vec<Self>> {
        String::from_utf8(data.into())
            .context("shaderdesc")
            .and_then(Self::load_from_str)
    }
    #[cfg(feature = "serde")]
    pub fn load_from_str(mut file_data: String) -> anyhow::Result<Vec<Self>> {
        json_strip_comments::strip(&mut file_data)?;
        let shader_description_data: Vec<Self> = serde_json::from_str(&file_data)?;
        Ok(shader_description_data)
    }

    #[cfg(feature = "serde")]
    pub fn load(path: &Path) -> anyhow::Result<Vec<Self>> {
        let context = |op: &str| {
            let filename = path.file_name().unwrap_or(path.as_os_str());
            format!("{} shader description `{}`", op, filename.display())
        };
        let file_data = read_to_string(path)
            .and_then(|mut file_data| json_strip_comments::strip(&mut file_data).map(move |()| file_data))
            .with_context(|| context("reading"))?;

        serde_json::from_str(&file_data).with_context(|| context("loading"))
    }

    pub fn file_name(&self) -> &OsStr {
        self.path.file_name().unwrap_or(self.path.as_os_str())
    }

    #[cfg(not(taimi_debug))]
    pub const COMPILE_FLAGS1: u32 = d3d::Fxc::D3DCOMPILE_DEBUG | d3d::Fxc::D3DCOMPILE_OPTIMIZATION_LEVEL3;
    #[cfg(taimi_debug)]
    pub const COMPILE_FLAGS1: u32 = d3d::Fxc::D3DCOMPILE_DEBUG | d3d::Fxc::D3DCOMPILE_ENABLE_STRICTNESS;
    pub const COMPILE_FLAGS2: u32 = 0;

    pub fn compile(&self, source: &[u8], includes: Option<&ID3DInclude>) -> anyhow::Result<Blob> {
        let filename = self.file_name().to_string_lossy();
        let name = CString::new(&filename[..])?;
        let entry_point = CString::new(&self.entrypoint[..])?;
        let (blob, warnings) = compile(
            &name,
            source,
            self.target,
            &entry_point,
            self.defs.as_ref(),
            includes,
            Self::COMPILE_FLAGS1,
            Self::COMPILE_FLAGS2,
        )
        .with_context(|| format!("compiling shader {filename}"))?;

        if log::log_enabled!(log::Level::Debug) && !warnings.is_empty() {
            log::debug!(
                "Shader {} warnings:\n{}",
                &self.identifier,
                warnings.to_string_lossy()
            );
        }

        Ok(blob)
    }

    /// TODO: remove duplicates if overridden
    pub fn append_defs<D>(&mut self, defs: D)
    where
        D: IntoIterator<Item = ShaderDefinition>,
    {
        #[cfg(todo)]
        let prior = self.defs.len();
        self.defs.extend(defs);
    }
}
