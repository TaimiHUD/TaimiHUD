use {
    crate::{
        exports::runtime as rt,
        resources::shader::{ShaderDescription, ShaderPair},
    },
    anyhow::Context,
    futures::future::Either,
    include_dir::include_dir,
    rustc_hash::{FxHashMap, FxHashSet},
    std::{
        borrow::Cow,
        ffi::{c_void, CStr, OsStr},
        fs,
        io::{self, Read},
        path::{Path, PathBuf},
        ptr,
        sync::Mutex,
    },
    taimi_d3d::{
        dx11::{
            prelude::*,
            shader::{InputLayout, ShaderP, ShaderV},
        },
        shader::{ID3DInclude, ID3DInclude_Impl, ShaderKind},
    },
    windows::{
        core::{Result as WinResult, PCSTR},
        Win32::Foundation,
    },
};

pub static SHADERS_DIR: include_dir::Dir = include_dir!("$CARGO_MANIFEST_DIR/data/shaders");

pub type VertexShaders = FxHashMap<String, (ShaderV, InputLayout)>;
pub type PixelShaders = FxHashMap<String, Option<ShaderP>>;

#[derive(Debug, Clone, Default)]
pub struct ShaderLoader {
    pub partial: FxHashMap<String, ShaderDescription>,
    pub vertex: VertexShaders,
    pub pixel: PixelShaders,
}

impl ShaderLoader {
    pub fn embedded_dir() -> &'static include_dir::Dir<'static> {
        &SHADERS_DIR
    }

    /// TODO: load on-demand eventually (and preload intentionally)
    pub fn load_bundled(device: &Dx11Device) -> anyhow::Result<Self> {
        let dir = ShaderDirectory::new();
        let mut shader_descriptions: Vec<ShaderDescription> = Vec::new();
        let shader_description_paths = dir.iter_contents_of("shaderdesc");
        for (path, contents) in shader_description_paths {
            let context = || {
                let file_name = ShaderDirectory::resolve_filename(&path);
                format!("parsing {}", file_name.display())
            };
            let data = match contents {
                Cow::Borrowed(c) => str::from_utf8(c).map(Cow::Borrowed).with_context(context),
                Cow::Owned(c) => String::from_utf8(c).map(Cow::Owned).with_context(context),
            }?;
            let mut shader_description =
                ShaderDescription::load_from_str(data.into_owned()).with_context(context)?;

            // copy to all shaders sharing the same source path...
            let mut defs = None::<taimi_d3d::shader::ShaderDefinitions>;
            for desc in &mut shader_description {
                if desc.defs.is_empty() {
                    if let Some(defs) = &defs {
                        desc.defs = defs.clone();
                    }
                } else {
                    defs = Some(desc.defs.clone());
                }
            }

            shader_descriptions.extend(shader_description);
        }

        Self::load_from(&dir, shader_descriptions, device)
    }

    fn load_from<S>(
        dir: &ShaderDirectory,
        shader_descriptions: S,
        device: &Dx11Device,
    ) -> anyhow::Result<Self>
    where
        S: IntoIterator<Item = ShaderDescription>,
    {
        log::debug!("Beginning shader setup!");
        let mut shaders: ShaderLoader = Self::default();
        let includes = ID3DInclude::new(dir);
        for mut shader_description in shader_descriptions {
            if shader_description.partial {
                shaders
                    .partial
                    .insert(shader_description.identifier.clone(), shader_description);
                continue
            }
            shader_description.defs.terminate();
            let context = || format!("loading shader {}", shader_description.identifier);
            let bytecode = dir
                .get_file_contents(&shader_description.path)
                .and_then(|source| shader_description.compile(&source, Some(&*includes)))
                .with_context(context);
            let Some(bytecode) = rt::log::warn_ok(bytecode) else { continue };
            let layout = match shader_description.target.kind() {
                ShaderKind::Vertex => {
                    let desc = shader_description.input_layout_desc();
                    InputLayout::new_with_desc(device, desc, &bytecode).map(Some)
                },
                ShaderKind::Pixel => Ok(None),
            };
            let res = layout
                .and_then(|layout| {
                    shaders.insert(device, shader_description.identifier.clone(), &bytecode, layout)
                })
                .with_context(context);
            let _ = rt::log::warn_ok(res);
        }
        log::info!(
            "Finished shader setup. {} vertex shaders, {} pixel shaders loaded!",
            shaders.vertex.len(),
            shaders.pixel.len()
        );
        Ok(shaders)
    }
    pub fn insert(
        &mut self,
        device: &Dx11Device,
        identifier: String,
        bytecode: &Blob,
        vertex_layout: Option<InputLayout>,
    ) -> anyhow::Result<()> {
        match vertex_layout {
            Some(layout) => {
                let shader = ShaderV::new_with_bytecode(device, bytecode)?;
                self.vertex.insert(identifier, (shader, layout));
            },
            None => {
                let shader = ShaderP::new_with_bytecode(device, bytecode)?;
                self.pixel.insert(identifier, Some(shader));
            },
        }
        Ok(())
    }
    pub fn load_partial(
        &mut self,
        device: &Dx11Device,
        identifier: &str,
        bytecode: &Blob,
        partial_id: &str,
    ) -> anyhow::Result<()> {
        let context = || format!("loading shader {identifier}");
        let layout = self
            .partial
            .get(partial_id)
            .context("missing")
            .and_then(|partial| match partial.target.kind() {
                ShaderKind::Vertex => {
                    let desc = partial.input_layout_desc();
                    InputLayout::new_with_desc(device, desc, bytecode)
                        .map(Some)
                        .map_err(Into::into)
                },
                ShaderKind::Pixel => Ok(None),
            })
            .with_context(|| format!("template {partial_id:?}"));
        layout
            .and_then(|layout| self.insert(device, identifier.into(), bytecode, layout))
            .with_context(context)
    }

    pub fn pair_named(&self, name: &str) -> anyhow::Result<ShaderPair> {
        let context = || format!("shader {name} unavailable");
        let vertex = self.vertex.get(name).with_context(context)?;
        let pixel = self.pixel.get(name).with_context(context)?;
        Ok(ShaderPair(vertex.clone(), pixel.clone()))
    }
    pub fn unload_named(&mut self, name: &str) -> bool {
        let unloaded_p = matches!(self.pixel.remove(name), Some(Some(..)));
        let unloaded_v = matches!(self.vertex.remove(name), Some(..));
        unloaded_p | unloaded_v
    }
    pub fn unload_partial(&mut self, name: &str) {
        let _ = self.partial.remove(name);
    }

    pub fn set_named(&self, context: &Dx11Context, name: &str) {
        let vertex = self.vertex.get(name);
        let pixel = self.pixel.get(name).map(|s| s.as_ref()).flatten();
        if vertex.is_none() && pixel.is_none() {
            // TODO: insert stub shader after warning once...
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
    pub fn unset(&self, context: &Dx11Context) {
        unsafe {
            context.PSSetShader(None, None);
            context.VSSetShader(None, None);
            context.IASetInputLayout(None);
            //context.IASetVertexBuffers()?
        }
    }
}

pub struct ShaderDirectory {
    root: PathBuf,
    fallback: Option<&'static include_dir::Dir<'static>>,
    open_files: Mutex<Vec<Box<[u8]>>>,
}
impl ShaderDirectory {
    pub fn new() -> Self {
        Self {
            root: rt::addon_dir().join("shaders"),
            fallback: Some(&SHADERS_DIR),
            open_files: Default::default(),
        }
    }
    fn resolve_filename<'p>(path: &'p Path) -> &'p OsStr {
        path.file_name().unwrap_or(rt::relative_path(path).as_os_str())
    }
    fn resolve_path<'p>(&self, path: &'p Path) -> Cow<'p, Path> {
        match path.is_absolute() {
            true => Cow::Borrowed(path),
            false => Cow::Owned(self.root.join(path)),
        }
    }
    pub fn get_file<P: ?Sized + AsRef<Path>>(&self, path: &P) -> anyhow::Result<Option<fs::File>> {
        let path = path.as_ref();
        let resolved = self.resolve_path(path);
        match fs::File::open(&resolved) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            f => f.map(Some),
        }
        .with_context(|| format!("missing shader data `{}`", Self::resolve_filename(path).display()))
    }
    pub fn open_file<P: ?Sized + AsRef<Path>>(&self, path: &P) -> anyhow::Result<Box<dyn io::BufRead>> {
        let path = path.as_ref();
        let res = self
            .get_file(path)
            .map(|f| f.map(|f| Box::new(io::BufReader::new(f)) as Box<_>));
        match res {
            Ok(Some(f)) => Ok(f),
            res => match self.fallback.and_then(|f| f.get_file(path)) {
                Some(f) => {
                    if let Err(e) = res {
                        let filename = Self::resolve_filename(path);
                        log::warn!("failed to retrieve {} from disk: {e:#}", filename.display());
                    }
                    let read = io::Cursor::new(f.contents());
                    return Ok(Box::new(read))
                },
                None => {
                    let res = res.transpose().with_context(|| {
                        format!("shader {} not found", Self::resolve_filename(path).display())
                    });
                    match res {
                        Ok(res) => res,
                        Err(e) => Err(e),
                    }
                },
            },
        }
    }

    pub fn get_file_contents<P: ?Sized + AsRef<Path>>(
        &self,
        path: &P,
    ) -> anyhow::Result<Cow<'static, [u8]>> {
        let path = path.as_ref();
        let res = self.get_file(path);
        let mut file = match res {
            Ok(Some(f)) => Ok(f),
            res => match self.fallback.and_then(|f| f.get_file(path)) {
                Some(f) => {
                    if let Err(e) = res {
                        let filename = Self::resolve_filename(path);
                        log::warn!(
                            "failed to retrieve shader {} from disk: {e:#}",
                            filename.display()
                        );
                    }
                    return Ok(Cow::Borrowed(f.contents()))
                },
                None => {
                    let res = res.transpose().with_context(|| {
                        format!("shader {} not found", Self::resolve_filename(path).display())
                    });
                    match res {
                        Ok(res) => res,
                        Err(e) => Err(e),
                    }
                },
            },
        }?;
        let mut out = Vec::new();
        file.read_to_end(&mut out)
            .with_context(|| format!("shader {} not found", Self::resolve_filename(path).display()))
            .map(move |_| Cow::Owned(out))
    }

    pub fn iter_contents_of<'e>(
        &self,
        ext: &'e str,
    ) -> impl Iterator<Item = (PathBuf, Cow<'static, [u8]>)> + 'e {
        let emb = self.fallback.map(|emb| emb.find(&format!("*.{ext}"))).transpose();
        let emb = rt::log::error_ok(emb).flatten().into_iter().flatten();
        let dir = match fs::read_dir(&self.root) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            dir => rt::log::warn_ok(dir),
        }
        .into_iter()
        .flatten();
        let mut emitted = FxHashSet::default();
        let paths = dir.map(Either::Left).chain(emb.map(Either::Right));
        paths.filter_map(move |item| match item {
            Either::Left(p) => {
                let p = rt::log::warn_ok(p)?;
                let path = p.path();
                match path.extension() {
                    Some(e) if e != ext => return None,
                    _ => (),
                }
                let mut out = Vec::new();
                let res = fs::File::open(&path)
                    .and_then(|mut f| f.read_to_end(&mut out))
                    .with_context(|| format!("reading {}", rt::relative_path(&path).display()));
                rt::log::error_ok(res)?;
                emitted.insert(p.file_name());
                Some((path, Cow::Owned(out.into())))
            },
            Either::Right(p) => {
                let file = match p.path().file_name().map(|n| emitted.contains(n)) {
                    Some(true) => {
                        log::trace!("overriding embedded file {}", p.path().display());
                        None
                    },
                    _ => p.as_file(),
                }?;
                Some((p.path().into(), Cow::Borrowed(file.contents())))
            },
        })
    }
}
impl Clone for ShaderDirectory {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            fallback: self.fallback.clone(),
            open_files: Default::default(),
        }
    }
}

impl ID3DInclude_Impl for ShaderDirectory {
    fn Open(
        &self,
        _ty: d3d::D3D_INCLUDE_TYPE,
        fname: &PCSTR,
        _parent: *const c_void,
        out: *mut *mut c_void,
        out_len: *mut u32,
    ) -> WinResult<()> {
        let fname = unsafe { CStr::from_ptr(fname.as_ptr() as *const _) };
        let path = fname.to_string_lossy();

        let c = match self.get_file_contents(&path[..]) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("{e:#}");
                return Err(Foundation::ERROR_FILE_NOT_FOUND.into())
            },
        };
        let c = c.into_owned();
        //c.push(0) just in case something assumes it's a string?
        let c = c.into_boxed_slice();
        unsafe {
            ptr::write(out_len, c.len() as u32);
            *out = c.as_ptr() as *mut c_void;
        }
        match self.open_files.lock() {
            Err(..) => {
                #[cfg(debug_assertions)]
                {
                    log::warn!("dir poisoned");
                }
                Box::leak(c);
            },
            Ok(mut open) => {
                open.push(c);
            },
        }
        Ok(())
    }
    fn Close(&self, pdata: *const c_void) -> WinResult<()> {
        if pdata.is_null() {
            return Err(Foundation::ERROR_INVALID_DATA.into())
        }
        if let Ok(mut open) = self.open_files.lock() {
            let idx = open.iter().position(|c| c.as_ptr() as usize == pdata as usize);
            if let Some(idx) = idx {
                open.swap_remove(idx);
            } else {
                #[cfg(debug_assertions)]
                {
                    log::warn!("closing file that doesn't exist");
                    return Err(arcffi::windows::winerror!(ERROR_FILE_NOT_FOUND).into())
                }
            }
        }
        Ok(())
    }
}
