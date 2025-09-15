use {
    anyhow::{anyhow, Context as _},
    crate::pack::file_path_eq,
    relative_path::PathExt,
    std::{
        ffi::OsStr,
        fmt,
        fs,
        io::{self, Cursor, Read as _},
        path::{Path, PathBuf},
    },
};
#[cfg(feature = "zip")]
use zip::ZipArchive;

pub trait LoaderAssetReader: io::BufRead + io::Seek + 'static {}
impl<R> LoaderAssetReader for R where R: io::BufRead + io::Seek + 'static {}

pub trait PackLoaderContext {
    fn find_asset_near(&mut self, relative: &str, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> where
        Self: Sized,
    {
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

    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader>
    where
        Self: Sized;

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>>;

    fn all_files_with_ext(&self, ext: &str) -> anyhow::Result<Vec<String>>;
}

impl PackLoaderContext for &mut dyn PackLoaderContext {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        self.load_asset_dyn(name)
    }

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        PackLoaderContext::load_asset_dyn(*self, name)
    }

    fn all_files_with_ext(&self, ext: &str) -> anyhow::Result<Vec<String>> {
        PackLoaderContext::all_files_with_ext(*self, ext)
    }
}

impl PackLoaderContext for Box<dyn PackLoaderContext> {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        self.load_asset_dyn(name)
    }

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        PackLoaderContext::load_asset_dyn(&mut **self, name)
    }

    fn all_files_with_ext(&self, ext: &str) -> anyhow::Result<Vec<String>> {
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
        Ok(io::BufReader::new(
            fs::File::open(&path).with_context(|| format!("Failed to open {path:?}"))?,
        ))
    }

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        Ok(Box::new(self.load_asset(name)?))
    }

    fn all_files_with_ext(&self, ext: &str) -> anyhow::Result<Vec<String>> {
        let mut files = vec![];

        visit_dir_ext(&mut files, &self.root, &self.root, ext)?;

        Ok(files)
    }
}

fn visit_dir_ext(
    files: &mut Vec<String>,
    base: &Path,
    dir: &Path,
    ext: &str,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_dir_ext(files, base, &path, ext)?;
        } else if path.extension() == Some(OsStr::new(ext)) {
            files.push(path.relative_to(base)?.into_string());
        }
    }
    Ok(())
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

    pub fn load_asset_by_index(&mut self, index: usize, name: impl fmt::Display) -> anyhow::Result<impl LoaderAssetReader> {
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
                let index = self.archive.file_names()
                    .position(|filename| file_path_eq(name, filename));
                index.ok_or_else(|| anyhow!("{name} not found in zip archive"))
                    .and_then(|index| self.load_asset_by_index(index, name))
            },
        }
    }

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        Ok(Box::new(self.load_asset(name)?))
    }

    fn all_files_with_ext(&self, ext: &str) -> anyhow::Result<Vec<String>> {
        Ok(self
            .archive
            .file_names()
            .filter(|name| name.rsplit_once('.').map(|(_, e)| e) == Some(ext))
            .map(|s| s.to_string())
            .collect())
    }
}
