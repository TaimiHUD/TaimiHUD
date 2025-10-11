use {
    compression_core::{util::PartialBuffer, Level},
    compression_codecs::{
        gzip::GzipEncoder,
        Encode,
    },
    std::{
        env,
        error::Error as StdError,
        fmt,
        fs,
        io::{self, Write, BufRead},
        path::PathBuf,
    },
};

type Error = Box<dyn StdError>;
fn err(m: impl fmt::Display) -> Error {
    Box::new(io::Error::new(io::ErrorKind::Other, m.to_string()))
}

fn main() {
    println!("cargo::rerun-if-env-changed={FEATURE_GZIP}");
    println!("cargo::rerun-if-changed=build.rs");

    match write_map_cache() {
        Ok(Some(map_path)) => {
            println!("cargo::rustc-env={INC_MAP_CACHE_GZ}={}", map_path.display());
        },
        Ok(None) => (),
        Err(e) => {
            println!("cargo::warning=failed to write cache file: {e:?}");
            if let Ok(dest_path) = out_dir().map(map_cache) {
                let _ = fs::remove_file(dest_path);
            }
        }
    }
}

fn write_map_cache() -> Result<Option<PathBuf>, Error> {
    if !env::var_os(FEATURE_GZIP).is_some() {
        if let Ok(mut sign) = data_dir() {
            sign.push(MAP_SIGN_FILENAME);
            println!("cargo::rustc-env={INC_MAP_CACHE}={}", sign.display());
        }
        return Ok(None)
    }
    println!("cargo::rerun-if-env-changed={OUT_DIR}");
    println!("cargo::rerun-if-env-changed={MANIFEST_DIR}");

    let src = data_dir().map(map_cache)?;
    println!("cargo::rerun-if-changed={}", src.display());
    println!("cargo::rustc-env={INC_MAP_CACHE}={}", src.display());
    let dest_path = out_dir().map(map_cache)?;

    let src = fs::File::open(src)?;
    let mut encoder = GzipEncoder::new(Level::Fastest.into());
    let mut dest = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&dest_path)?;

    let mut src = io::BufReader::with_capacity(0x1000, src);

    let mut out = PartialBuffer::new(vec![0u8; 0x800]);
    loop {
        let mut buf = PartialBuffer::new(match src.buffer() {
            b if b.is_empty() =>
                src.fill_buf()?,
            b => b,
        });
        if buf.unwritten().is_empty() {
            break
        }
        encoder.encode(&mut buf, &mut out)?;
        let consumed = buf.written().len();
        src.consume(consumed);
        if out.written().len() > out.unwritten().len() || true {
            dest.write_all(out.written())?;
            out.reset();
        }
    }
    while !encoder.finish(&mut out)? {
        assert!(!out.written().is_empty());
        dest.write_all(out.written())?;
        out.reset();
    }
    dest.write_all(out.written())?;
    out.reset();

    dest.flush()?;
    dest.sync_data()?;

    Ok(Some(dest_path))
}

const FEATURE_GZIP: &'static str = "CARGO_FEATURE_GZIP";
const INC_MAP_CACHE: &'static str = "INC_MAP_CACHE";
const INC_MAP_CACHE_GZ: &'static str = "INC_MAP_CACHE_GZ";
const OUT_DIR: &'static str = "OUT_DIR";
const MANIFEST_DIR: &'static str = "CARGO_MANIFEST_DIR";
fn out_dir() -> Result<PathBuf, Error> {
    env::var_os(OUT_DIR)
        .map(PathBuf::from)
        .ok_or_else(|| err(format_args!("expected {OUT_DIR}")))
}
fn crate_dir() -> Result<PathBuf, Error> {
    env::var_os(MANIFEST_DIR)
        .map(PathBuf::from)
        .ok_or_else(|| err(format_args!("expected {MANIFEST_DIR}")))
}
fn data_dir() -> Result<PathBuf, Error> {
    let mut root = crate_dir()?;
    root.push("data");
    Ok(root)
}
const MAP_FILENAME: &'static str = "maps.json";
const MAP_SIGN_FILENAME: &'static str = "maps-sign.json";
fn map_cache(parent: impl Into<PathBuf>) -> PathBuf {
    let mut parent = parent.into();
    parent.push(MAP_FILENAME);
    parent
}
