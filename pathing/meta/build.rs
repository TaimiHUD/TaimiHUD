use {
    compression_codecs::{gzip::GzipEncoder, Encode},
    compression_core::{util::PartialBuffer, Level},
    std::{
        env,
        error::Error as StdError,
        fmt,
        fs,
        io::{self, BufRead, Write},
        path::{Path, PathBuf},
    },
};

type Error = Box<dyn StdError>;
fn err(m: impl fmt::Display) -> Error {
    Box::new(io::Error::new(io::ErrorKind::Other, m.to_string()))
}

fn main() {
    println!("cargo::rerun-if-env-changed={FEATURE_GZIP}");
    println!("cargo::rerun-if-changed=build.rs");

    write_map_cache();
}

fn write_map_cache() {
    if has_gzip() {
        println!("cargo::rerun-if-env-changed={OUT_DIR}");
        println!("cargo::rerun-if-env-changed={MANIFEST_DIR}");
    }
    process_cache_data(
        &INC_MAP_CACHE_GZ,
        map_cache,
        &INC_MAP_CACHE,
        Some(MAP_SIGN_FILENAME.as_ref()),
    );
}

const INC_MAP_CACHE: &'static str = "INC_MAP_CACHE";
const INC_MAP_CACHE_GZ: &'static str = "INC_MAP_CACHE_GZ";
const MAP_FILENAME: &'static str = "maps.json";
const MAP_SIGN_FILENAME: &'static str = "maps-sign.json";
fn map_cache(mut parent: PathBuf) -> PathBuf {
    parent.push(MAP_FILENAME);
    parent
}

fn process_cache_data<F>(
    id: &dyn fmt::Display,
    mut filename: F,
    src_id: &dyn fmt::Display,
    fallback: Option<&Path>,
) where
    F: FnMut(PathBuf) -> PathBuf,
{
    match write_cache_data(src_id, &mut filename, fallback) {
        Ok(Some(map_path)) => {
            println!("cargo::rustc-env={id}={}", map_path.display());
        },
        Ok(None) => (),
        Err(e) => {
            println!("cargo::warning=failed to write {id} cache: {e}");
            if let Ok(dest_path) = out_dir().map(filename) {
                let _ = fs::remove_file(dest_path);
            }
        },
    }
}
fn write_cache_data<F>(
    id: &dyn fmt::Display,
    mut filename: F,
    fallback: Option<&Path>,
) -> Result<Option<PathBuf>, Error>
where
    F: FnMut(PathBuf) -> PathBuf,
{
    let src_dir = data_dir()?;

    if !has_gzip() {
        let src = match fallback {
            Some(fallback) => {
                let mut src = src_dir;
                src.push(fallback);
                src
            },
            None => filename(src_dir),
        };
        println!("cargo::rustc-env={id}={}", src.display());
        return Ok(None)
    }

    let src = filename(src_dir);
    println!("cargo::rerun-if-changed={}", src.display());
    println!("cargo::rustc-env={id}={}", src.display());
    let dest_path = out_dir().map(filename)?;

    let src = fs::File::open(src)?;
    let mut encoder = GzipEncoder::new(Level::Fastest.into());
    let mut dest = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&dest_path)?;

    let mut src = io::BufReader::with_capacity(0x1000, src);

    let mut out = PartialBuffer::new(vec![0u8; 0x800]);
    let mut src_len = 0usize;
    loop {
        let mut buf = PartialBuffer::new(match src.buffer() {
            b if b.is_empty() => src.fill_buf()?,
            b => b,
        });
        if buf.unwritten().is_empty() {
            break
        }
        encoder.encode(&mut buf, &mut out)?;
        let consumed = buf.written().len();
        src.consume(consumed);
        src_len += consumed;
        dest.write_all(out.written())?;
        out.reset();
    }
    while !encoder.finish(&mut out)? {
        assert!(!out.written().is_empty());
        dest.write_all(out.written())?;
        out.reset();
    }
    dest.write_all(out.written())?;
    out.reset();
    println!("cargo::rustc-env={id}_BUFLEN={}", src_len.next_multiple_of(0x200));

    dest.flush()?;
    dest.sync_data()?;

    Ok(Some(dest_path))
}

const FEATURE_GZIP: &'static str = "CARGO_FEATURE_GZIP";
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
fn has_gzip() -> bool {
    env::var_os(FEATURE_GZIP).is_some()
}
