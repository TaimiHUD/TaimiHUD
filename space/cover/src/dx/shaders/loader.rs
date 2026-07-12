use {
    super::{ShaderDescription, ShaderLayout, ShaderPair},
    anyhow::Context,
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
            shader::{InputLayout, InputLayoutElement, ShaderP, ShaderV},
        },
        shader::{ID3DInclude, ID3DInclude_Impl, ShaderKind},
    },
    windows::{
        core::{Result as WinResult, PCSTR},
        Win32::Foundation,
    },
};

pub type VertexShaders = FxHashMap<String, (ShaderV, InputLayout)>;
pub type PixelShaders = FxHashMap<String, Option<ShaderP>>;

#[derive(Debug, Clone, Default)]
pub struct ShaderLoader {
    pub partial: FxHashMap<String, ShaderDescription>,
    pub input_layout_defs: FxHashMap<String, &'static [InputLayoutElement]>,
    pub vertex: VertexShaders,
    pub pixel: PixelShaders,
}

impl ShaderLoader {
    pub fn new() -> Self {
        Self::default()
    }
    #[cfg(feature = "serde")]
    pub fn load_bundled(&mut self, device: &Dx11Device, dir: &ShaderDirectory) -> anyhow::Result<()> {
        let mut shader_descriptions: Vec<ShaderDescription> = Vec::new();
        let shader_description_paths = dir.iter_contents_of("shaderdesc");
        for (path, contents) in shader_description_paths {
            let context = || {
                let file_name = ShaderDirectory::resolve_filename(&path);
                format!("parsing {}", file_name.display())
            };
            let mut shader_description =
                ShaderDescription::load_from_bytes(contents).with_context(context)?;

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

        log::trace!("Beginning shader setup!");
        self.load_with_descs(&dir, shader_descriptions, device)?;
        log::debug!(
            "Finished shader setup. {} vertex shaders, {} pixel shaders loaded!",
            self.vertex.len(),
            self.pixel.len()
        );
        Ok(())
    }

    pub fn load_with_descs<S>(
        &mut self,
        dir: &ShaderDirectory,
        shader_descriptions: S,
        device: &Dx11Device,
    ) -> anyhow::Result<()>
    where
        S: IntoIterator<Item = ShaderDescription>,
    {
        let includes = ID3DInclude::new(dir);
        for mut shader_description in shader_descriptions {
            if shader_description.partial {
                self.partial
                    .insert(shader_description.identifier.clone(), shader_description);
                continue
            }
            shader_description.defs.terminate();
            let context = || format!("loading shader {}", shader_description.identifier);
            let bytecode = dir
                .get_file_contents(&shader_description.path)
                .and_then(|source| shader_description.compile(&source, Some(&*includes)))
                .with_context(context);
            let Some(bytecode) = log::warn_ok(bytecode) else { continue };
            let layout = match shader_description.target.kind() {
                ShaderKind::Vertex => {
                    let desc = self.input_layout_descs_for(shader_description.layout_type.as_ref());
                    InputLayout::new_with_desc(device, desc, &bytecode).map(Some)
                },
                ShaderKind::Pixel => Ok(None),
            };
            let res = layout
                .and_then(|layout| {
                    self.insert(device, shader_description.identifier.clone(), &bytecode, layout)
                })
                .with_context(context);
            let _ = log::warn_ok(res);
        }
        Ok(())
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
    pub fn register_layout11<S>(&mut self, name: S, layout: &'static [InputLayoutElement])
    where
        S: Into<String>,
    {
        self.input_layout_defs.insert(name.into(), layout);
    }
    pub fn register_layout11_sys<S>(&mut self, name: S, layout: &'static [d3d11::D3D11_INPUT_ELEMENT_DESC])
    where
        S: Into<String>,
    {
        self.register_layout11::<S>(name, InputLayoutElement::slice_from_d3d(layout))
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
                    let desc = self.input_layout_descs_for(partial.layout_type.as_ref());
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
    pub fn load_reinterpret(
        &mut self,
        device: &Dx11Device,
        identifier: &str,
        bytecode: &Blob,
        layout: &ShaderLayout,
        template_id: &str,
    ) -> anyhow::Result<()> {
        if self.vertex.contains_key(identifier) {
            return Ok(())
        }
        let context = || format!("interpreting shader {template_id} as {identifier}");
        self.vertex
            .get(template_id)
            .context("missing")
            .and_then(|(template, _)| {
                let desc = self.input_layout_descs_for(Some(layout));
                InputLayout::new_with_desc(device, desc, bytecode).map(|l| (template.clone(), l))
            })
            .with_context(context)
            .map(|entry| {
                self.vertex.insert(identifier.into(), entry);
            })
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

    const DEFAULT_LAYOUT_NAME: &'static str = "VertexInstance";
    fn input_layout_descs_for<'a>(
        &'a self,
        layout: Option<&'a ShaderLayout>,
    ) -> &'a [d3d11::D3D11_INPUT_ELEMENT_DESC] {
        let name = match layout {
            Some(ShaderLayout::Inputs(i)) => return InputLayoutElement::slice_as_desc(i),
            Some(ShaderLayout::Named(n)) => &n[..],
            None => Self::DEFAULT_LAYOUT_NAME,
        };
        let fallback = || -> &'a [InputLayoutElement] {
            log::debug!("unrecognized shader layout {name}");
            &[]
        };
        let defs = self
            .input_layout_defs
            .get(name)
            .map(|l| &l[..])
            .unwrap_or_else(fallback);
        InputLayoutElement::slice_as_desc(defs)
    }
}

pub struct ShaderDirectory {
    root: PathBuf,
    fallback: Option<&'static include_dir::Dir<'static>>,
    open_files: Mutex<Vec<Box<[u8]>>>,
}
impl ShaderDirectory {
    pub fn new(root: PathBuf, fallback: Option<&'static include_dir::Dir<'static>>) -> Self {
        Self {
            root,
            fallback,
            open_files: Default::default(),
        }
    }
    fn resolve_filename<'p>(path: &'p Path) -> &'p OsStr {
        path.file_name().unwrap_or(path.as_os_str())
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
        let extos = Some(OsStr::new(ext));
        let emb = self.fallback.into_iter().flat_map(move |emb| {
            emb.entries().iter().filter_map(move |f| match f {
                #[cfg(todo = "unnecessary")]
                include_dir::DirEntry::Dir(d) => recursive_search(d),
                include_dir::DirEntry::File(f) if f.path().extension() == extos => Some(f),
                _ => None,
            })
        });
        let dir = match fs::read_dir(&self.root) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            dir => log::warn_ok(dir),
        }
        .into_iter()
        .flatten();
        let mut emitted = FxHashSet::default();
        let paths = dir.map(Either::Left).chain(emb.map(Either::Right));
        paths.filter_map(move |item| match item {
            Either::Left(p) => {
                let p = log::warn_ok(p)?;
                let path = p.path();
                match path.extension() {
                    Some(e) if e != ext => return None,
                    _ => (),
                }
                let mut out = Vec::new();
                let context = || {
                    let filename = path.file_name().unwrap_or(path.as_os_str());
                    format!("reading {}", filename.display())
                };
                let res = fs::File::open(&path)
                    .and_then(|mut f| f.read_to_end(&mut out))
                    .with_context(context);
                log::error_ok(res)?;
                emitted.insert(p.file_name());
                Some((path, Cow::Owned(out.into())))
            },
            Either::Right(p) => {
                let file = match p.path().file_name().map(|n| emitted.contains(n)) {
                    Some(true) => {
                        log::trace!("overriding embedded file {}", p.path().display());
                        None
                    },
                    _ => Some(p),
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

enum Either<L, R> {
    Left(L),
    Right(R),
}
