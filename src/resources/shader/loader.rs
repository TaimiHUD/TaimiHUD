use {
    anyhow::Context,
    crate::{
        exports::runtime as rt,
        resources::shader::{ShaderDescription, ShaderPair},
    },
    include_dir::include_dir,
    std::{
        borrow::Cow,
        collections::HashMap,
        fs,
        io,
        path::Path,
    },
    taimi_d3d::{
        dx11::{
            prelude::*,
            shader::{InputLayout, ShaderP, ShaderV},
        },
        shader::ShaderKind,
    },
};

pub static SHADERS_DIR: include_dir::Dir = include_dir!("$CARGO_MANIFEST_DIR/shaders");

pub type VertexShaders = HashMap<String, (ShaderV, InputLayout)>;
pub type PixelShaders = HashMap<String, Option<ShaderP>>;

#[derive(Debug, Clone, Default)]
pub struct ShaderLoader {
    pub vertex: VertexShaders,
    pub pixel: PixelShaders,
}

impl ShaderLoader {
    pub fn embedded_dir() -> &'static include_dir::Dir<'static> {
        &SHADERS_DIR
    }

    pub fn get_file<P: AsRef<Path>>(path: &P) -> anyhow::Result<Box<dyn io::BufRead>> {
        let path = path.as_ref();
        let path = match Self::embedded_dir().get_file(path) {
            Some(f) => {
                let read = io::Cursor::new(f.contents());
                return Ok(Box::new(read))
            },
            None if path.is_absolute() => Cow::Borrowed(path),
            None => Cow::Owned(rt::addon_dir().join(path)),
        };
        fs::File::open(&path)
            .with_context(|| {
                let filename = path.file_name()
                    .unwrap_or(path.as_os_str());
                format!("missing shader data `{}`", filename.display())
            }).map(|f| Box::new(io::BufReader::new(f)) as Box<_>)
    }

    pub fn get_file_contents<P: AsRef<Path>>(path: &P) -> anyhow::Result<Cow<'static, [u8]>> {
        use std::io::Read;

        let path = path.as_ref();
        let path = match Self::embedded_dir().get_file(path) {
            Some(f) => return Ok(Cow::Borrowed(f.contents())),
            None if path.is_absolute() => Cow::Borrowed(path),
            None => Cow::Owned(rt::addon_dir().join(path)),
        };
        let mut out = Vec::new();
        fs::File::open(&path)
            .and_then(|mut f| f.read_to_end(&mut out)
                .map(move |_| out)
            ).with_context(|| {
                let filename = path.file_name()
                    .unwrap_or(path.as_os_str());
                format!("missing shader data `{}`", filename.display())
            }).map(Cow::Owned)
    }

    pub fn load_bundled(device: &Dx11Device) -> anyhow::Result<Self> {
        let mut shader_descriptions: Vec<ShaderDescription> = Vec::new();
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

        Self::load_from(shader_descriptions, device)
    }

    #[cfg(todo)]
    pub fn load_dir(addon_dir: &std::path::Path, device: &Dx11Device) -> anyhow::Result<Self> {
        use glob::Paths;
    }

    pub fn load_from<S>(shader_descriptions: S, device: &Dx11Device) -> anyhow::Result<Self> where
        S: IntoIterator<Item = ShaderDescription>,
    {
        log::debug!("Beginning shader setup!");
        let mut shaders: ShaderLoader = Self::default();
        for shader_description in shader_descriptions {
            let context = || format!("loading shader {}", shader_description.identifier);
            let bytecode = Self::get_file_contents(&shader_description.path)
                .and_then(|source| shader_description.compile(&source))
                .with_context(context)?;
            match shader_description.target.kind() {
                ShaderKind::Vertex => {
                    let shader = ShaderV::new_with_bytecode(device, &bytecode)?;
                    let desc = shader_description.input_layout_desc();
                    let layout = InputLayout::new_with_desc(device, desc, &bytecode)?;
                    shaders.vertex.insert(shader_description.identifier, (shader, layout));
                }
                ShaderKind::Pixel => {
                    let shader = ShaderP::new_with_bytecode(device, &bytecode)?;
                    shaders.pixel.insert(shader_description.identifier, Some(shader));
                }
            }
        }
        log::info!(
            "Finished shader setup. {} vertex shaders, {} pixel shaders loaded!",
            shaders.vertex.len(),
            shaders.pixel.len()
        );
        Ok(shaders)
    }

    pub fn pair_named(&self, name: &str) -> anyhow::Result<ShaderPair> {
        let context = || format!("shader {name} unavailable");
        let vertex = self.vertex.get(name)
            .with_context(context)?;
        let pixel = self.pixel.get(name)
            .with_context(context)?;
        Ok(ShaderPair(vertex.clone(), pixel.clone()))
    }

    pub fn set_named(&self, context: &Dx11Context, name: &str) {
        let vertex = self.vertex.get(name);
        let pixel = self.pixel.get(name)
            .map(|s| s.as_ref()).flatten();
        if vertex.is_none() && pixel.is_none() {
            log::warn!("shader {name} unavailable!");
        }
        if let Some((shader, layout)) = vertex {
            layout.set(context);
            shader.set(context);
        }
        if let Some(shader) = pixel {
            shader.set(context);
        }
    }
}
