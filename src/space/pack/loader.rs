use {
    anyhow::Context as _,
    relative_path::PathExt,
    std::{
        ffi::OsStr,
        io::{Cursor, Read as _},
        path::{Path, PathBuf},
    },
    zip::ZipArchive,
};

pub trait LoaderAssetReader: std::io::BufRead + std::io::Seek + 'static {}
impl<R> LoaderAssetReader for R where R: std::io::BufRead + std::io::Seek + 'static {}

pub trait PackLoaderContext {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader>
    where
        Self: Sized;

    fn load_asset_dyn(&mut self, name: &str) -> anyhow::Result<Box<dyn LoaderAssetReader>>;

    fn all_files_with_ext(&self, ext: &str) -> anyhow::Result<Vec<String>>;
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
        Ok(std::io::BufReader::new(
            std::fs::File::open(&path).with_context(|| format!("Failed to open {path:?}"))?,
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
    for entry in std::fs::read_dir(dir)? {
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

pub struct ZipLoader {
    archive: ZipArchive<std::fs::File>,
}

impl ZipLoader {
    pub fn new(path: &Path) -> anyhow::Result<ZipLoader> {
        let file = std::fs::File::open(path)?;
        let archive = ZipArchive::new(file)?;
        Ok(ZipLoader { archive })
    }
}

/// Hard to imagine a valid taco data file being over 64MB.
const SIZE_LIMIT: u64 = 64 * 1024 * 1024;

impl PackLoaderContext for ZipLoader {
    fn load_asset(&mut self, name: &str) -> anyhow::Result<impl LoaderAssetReader> {
        let mut file = self
            .archive
            .by_name(name)
            .with_context(|| format!("{name} not found in zip archive"))?;
        if file.size() > SIZE_LIMIT {
            anyhow::bail!("{name} is too big at {}MB", file.size() / (1024 * 1024));
        }
        let mut buf = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut buf)
            .with_context(|| format!("Failed to read {name} from zip archive"))?;

        Ok(Cursor::new(buf))
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
