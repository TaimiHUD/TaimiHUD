use {
    crate::{resources::Vertex, space::pack::instance},
    anyhow::Context,
    serde::{Deserialize, Serialize},
    std::{
        ffi::{CString, OsStr},
        fs::read_to_string,
        mem::offset_of,
        path::{Path, PathBuf},
    },
    taimi_d3d::{
        blob::Blob,
        dx11::{
            prelude::*,
            shader::{InputLayout, InputLayoutElement, D3D11_INPUT_ELEMENT_DESC},
        },
        shader::{compile, ShaderDefinition, ShaderDefinitions, ShaderTarget, ID3DInclude},
    },
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShaderDescription {
    pub identifier: String,
    /// incomplete so it can be used as a base to construct a configured shader,
    /// disabling any sort of auto-load
    #[serde(default)]
    pub partial: bool,
    #[serde(rename = "kind")]
    pub target: ShaderTarget,
    #[serde(default, skip_serializing_if = "ShaderDefinitions::is_empty")]
    pub defs: ShaderDefinitions,
    pub path: PathBuf,
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_type: Option<ShaderLayout>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ShaderLayout {
    JustVertex,
    VertexInstance,
    SpaceTrail,
    SpacePoi,
    #[serde(rename = "inputs")]
    Inputs(Vec<InputLayoutElement>),
}

impl ShaderDescription {
    pub fn load_from_str(data: String) -> anyhow::Result<Vec<Self>> {
        let mut file_data = data.clone();
        json_strip_comments::strip(&mut file_data)?;
        let shader_description_data: Vec<Self> = serde_json::from_str(&file_data)?;
        Ok(shader_description_data)
    }

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

    pub fn input_layout_desc(&self) -> &[D3D11_INPUT_ELEMENT_DESC] {
        match &self.layout_type {
            None | Some(ShaderLayout::VertexInstance) => &Self::INPUT_LAYOUT_INSTANCED,
            Some(ShaderLayout::JustVertex) => &Self::INPUT_LAYOUT_JUST_VERTEX,
            Some(ShaderLayout::SpaceTrail) => &instance::INPUT_LAYOUT_TRAIL_INSTANCE,
            Some(ShaderLayout::SpacePoi) => &instance::INPUT_LAYOUT_POI_INSTANCE,
            Some(ShaderLayout::Inputs(layout)) => InputLayoutElement::slice_as_desc(layout),
        }
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
    pub fn append_defs<D>(&mut self, defs: D) where
        D: IntoIterator<Item = ShaderDefinition>,
    {
        #[cfg(todo)]
        let prior = self.defs.len();
        self.defs.extend(defs);
    }

    const INPUT_LAYOUT_INSTANCED: [D3D11_INPUT_ELEMENT_DESC; 9] = [
        Self::INPUT_LAYOUT_JUST_VERTEX[0], // POSITION0
        Self::INPUT_LAYOUT_JUST_VERTEX[1], // COLOR0
        Self::INPUT_LAYOUT_JUST_VERTEX[2], // NORMAL0
        Self::INPUT_LAYOUT_JUST_VERTEX[3], // TEXCOORD0
        InputLayout::for_instance(1, c"MODEL", 0, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
        InputLayout::for_instance(1, c"MODEL", 1, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
        InputLayout::for_instance(1, c"MODEL", 2, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
        InputLayout::for_instance(1, c"MODEL", 3, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
        InputLayout::for_instance(1, c"COLOUR", 0, dxgi::DXGI_FORMAT_R32G32B32A32_FLOAT, None),
    ];
    const INPUT_LAYOUT_JUST_VERTEX: [D3D11_INPUT_ELEMENT_DESC; 4] = [
        InputLayout::for_vertex(
            0,
            c"POSITION",
            0,
            dxgi::DXGI_FORMAT_R32G32B32_FLOAT,
            Some(offset_of!(Vertex, position)),
        ),
        InputLayout::for_vertex(
            0,
            c"COLOR",
            0,
            dxgi::DXGI_FORMAT_R32G32B32_FLOAT,
            Some(offset_of!(Vertex, colour)),
        ),
        InputLayout::for_vertex(
            0,
            c"NORMAL",
            0,
            dxgi::DXGI_FORMAT_R32G32B32_FLOAT,
            Some(offset_of!(Vertex, normal)),
        ),
        InputLayout::for_vertex(
            0,
            c"TEXCOORD",
            0,
            dxgi::DXGI_FORMAT_R32G32_FLOAT,
            Some(offset_of!(Vertex, texture)),
        ),
    ];
}
