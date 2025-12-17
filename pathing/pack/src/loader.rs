#[cfg(feature = "zip")]
use zip::ZipArchive;
use {
    crate::pack::file_path_eq,
    anyhow::{anyhow, Context as _},
    relative_path::PathExt,
    std::{
        borrow::Cow,
        fmt,
        fs,
        io::{self, Cursor, Read as _},
        path::{Path, PathBuf},
    },
};

pub trait LoaderAssetReader: io::BufRead + io::Seek + 'static {}
impl<R> LoaderAssetReader for R where R: io::BufRead + io::Seek + 'static {}

pub type PackFilenameIter<'a> = Box<dyn Iterator<Item = anyhow::Result<Cow<'a, Path>>> + 'a>;

pub trait PackLoaderContext {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader>
    where
        Self: Sized;

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>>;

    /// check if an asset directly corresponds to a real file
    /// (only realistic when backed by [DirectoryLoader])
    ///
    /// TODO: could include byte offset range if in an uncompressed archive or something insane?
    #[inline]
    fn asset_absolute_path(&mut self, name: &str) -> Option<PathBuf> {
        let _ = name;
        None
    }

    fn all_files_with_ext<'a>(&'a self, ext: &'static str) -> PackFilenameIter<'a>;

    fn all_files_with_ext_owned(&self, ext: &'static str) -> Vec<anyhow::Result<PathBuf>>
    where
        Self: Sized,
    {
        self.all_files_with_ext(ext)
            .map(|def| def.map(|asset| asset.into_owned()))
            .collect()
    }

    fn as_dyn(&mut self) -> &mut dyn PackLoaderContext
    where
        Self: Sized,
    {
        self
    }
}

impl<'l> dyn PackLoaderContext + 'l {
    pub fn find_asset_near(
        &mut self,
        relative: &str,
        name: &str,
    ) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        let e = match self.load_asset_dyn(name) {
            Ok(a) => return Ok(a),
            Err(e) => e,
        };

        if let Some(parent) = Path::new(relative).parent() {
            let fallback = format!("{}/{}", parent.display(), name);
            if let Ok(a) = self.load_asset_dyn(&fallback) {
                return Ok(a)
            }
        }

        Err(e)
    }
}

impl PackLoaderContext for &mut dyn PackLoaderContext {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        self.load_asset_dyn(name)
    }

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        PackLoaderContext::load_asset_dyn(*self, name)
    }

    fn all_files_with_ext<'a>(&'a self, ext: &'static str) -> PackFilenameIter<'a> {
        PackLoaderContext::all_files_with_ext(*self, ext)
    }
}
impl<L: PackLoaderContext> PackLoaderContext for &mut L {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        L::load_asset(self, name)
    }

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        PackLoaderContext::load_asset_dyn(*self, name)
    }

    fn all_files_with_ext<'a>(&'a self, ext: &'static str) -> PackFilenameIter<'a> {
        PackLoaderContext::all_files_with_ext(*self, ext)
    }
}

impl PackLoaderContext for Box<dyn PackLoaderContext + '_> {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        self.load_asset_dyn(name)
    }
    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        PackLoaderContext::load_asset_dyn(&mut **self, name)
    }
    fn all_files_with_ext<'a>(&'a self, ext: &'static str) -> PackFilenameIter<'a> {
        PackLoaderContext::all_files_with_ext(&**self, ext)
    }
}
impl PackLoaderContext for Box<dyn PackLoaderContext + Send + '_> {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        self.load_asset_dyn(name)
    }
    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        PackLoaderContext::load_asset_dyn(&mut **self, name)
    }
    fn all_files_with_ext<'a>(&'a self, ext: &'static str) -> PackFilenameIter<'a> {
        PackLoaderContext::all_files_with_ext(&**self, ext)
    }
}
impl PackLoaderContext for Box<dyn PackLoaderContext + Send + Sync + '_> {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        self.load_asset_dyn(name)
    }
    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        PackLoaderContext::load_asset_dyn(&mut **self, name)
    }
    fn all_files_with_ext<'a>(&'a self, ext: &'static str) -> PackFilenameIter<'a> {
        PackLoaderContext::all_files_with_ext(&**self, ext)
    }
}

pub struct DirectoryLoader {
    root: PathBuf,
}

impl DirectoryLoader {
    pub fn new<P: Into<PathBuf>>(root: P) -> DirectoryLoader {
        DirectoryLoader { root: root.into() }
    }
}

impl PackLoaderContext for DirectoryLoader {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        let path = self.root.join(name);
        Ok(io::BufReader::new(fs::File::open(&path).with_context(|| {
            let root = self.root.parent().unwrap_or(&self.root);
            let path = path.strip_prefix(root).unwrap_or(&path);
            format!("Opening {}", path.display())
        })?))
    }

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        Ok(Box::new(self.load_asset(name)?))
    }

    fn all_files_with_ext<'a>(&'a self, ext: &'static str) -> PackFilenameIter<'a> {
        let iter = visit_dir_ext(&self.root, Cow::Borrowed(&self.root), ext);

        Box::new(iter)
    }

    fn asset_absolute_path(&mut self, name: &str) -> Option<PathBuf> {
        Some(self.root.join(name))
    }
}

fn visit_dir_ext<'a>(
    base: &'a Path,
    dir: Cow<'a, Path>,
    ext: &'static str,
) -> impl Iterator<Item = anyhow::Result<Cow<'a, Path>>> + 'a {
    let (read, e) = match fs::read_dir(dir) {
        Ok(read) => (Some(read), None),
        Err(e) => (None, Some(anyhow::Error::from(e))),
    };
    let iter = read.into_iter().flatten().flat_map(move |file| {
        let (f, d, e) = match file {
            Ok(file) => {
                let path = file.path();
                let (f, d) = match path.is_dir() {
                    true => (None, Some((path, file))),
                    false => (Some((path, file)), None),
                };
                (f, d, None)
            },
            Err(e) => (None, None, Some(anyhow::Error::from(e))),
        };

        let f = f
            .into_iter()
            .filter(move |(path, _)| path.extension().unwrap_or_default().eq_ignore_ascii_case(ext))
            .map(move |(path, _)| {
                path.relative_to(base)
                    .map_err(anyhow::Error::from)
                    .map(|f| PathBuf::from(f.into_string()).into())
            });

        let d = d.into_iter().flat_map(move |(path, _)| {
            let iter = visit_dir_ext(base, Cow::Owned(path), ext);
            Box::new(iter) as PackFilenameIter
        });

        e.into_iter().map(Err).chain(f).chain(d)
    });
    e.into_iter().map(Err).chain(iter)
}

#[cfg(feature = "zip")]
pub struct ZipLoader {
    archive: ZipArchive<fs::File>,
}

#[cfg(feature = "zip")]
impl ZipLoader {
    /// Hard to imagine a valid taco data file being over 64MB.
    const SIZE_LIMIT: u64 = 64 * 1024 * 1024;

    pub fn new(path: &Path) -> anyhow::Result<ZipLoader> {
        let file = fs::File::open(path)?;
        let archive = ZipArchive::new(file)?;
        Ok(ZipLoader { archive })
    }

    pub fn load_asset_by_index(
        &mut self,
        index: usize,
        name: impl fmt::Display,
    ) -> anyhow::Result<impl LoaderAssetReader> {
        let res = self.archive.by_index(index);
        let mut file = res.with_context(|| format!("{name} not found in zip archive"))?;
        if file.size() > Self::SIZE_LIMIT {
            anyhow::bail!("{name} is too big at {}MB", file.size() / (1024 * 1024));
        }
        let mut buf = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut buf)
            .with_context(|| format!("Failed to read {name} from zip archive"))?;

        Ok(Cursor::new(buf))
    }
}

#[cfg(feature = "zip")]
impl PackLoaderContext for ZipLoader {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        match self.archive.index_for_name(name) {
            Some(index) => self.load_asset_by_index(index, name),
            None => {
                let index = self
                    .archive
                    .file_names()
                    .position(|filename| file_path_eq(name, filename));
                index
                    .ok_or_else(|| anyhow!("{name} not found in zip archive"))
                    .and_then(|index| self.load_asset_by_index(index, name))
            },
        }
    }

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        Ok(Box::new(self.load_asset(name)?))
    }

    fn all_files_with_ext<'a>(&'a self, ext: &'static str) -> PackFilenameIter<'a> {
        let iter = self
            .archive
            .file_names()
            .filter(|name| name.rsplit_once('.').map(|(_, e)| e) == Some(ext))
            .map(|name| Ok(Path::new(name).into()));
        Box::new(iter)
    }
}
