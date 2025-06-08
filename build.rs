#[cfg(feature = "built-info")]
use std::{env, fs, path::PathBuf};

const FEATURE_BUILT: &'static str = "CARGO_FEATURE_BUILT_INFO";
fn main() {
    println!("cargo::rerun-if-env-changed={FEATURE_BUILT}");
    #[cfg(feature = "built-info")]
    write_built_info();
}

#[cfg(feature = "built-info")]
fn write_built_info() {
    if env::var_os(FEATURE_BUILT).is_some() {
        let built_out = env::var_os("OUT_DIR")
            .map(PathBuf::from)
            .map(|p| p.join("built.rs"));
        let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
            .map(PathBuf::from);
        let manifest_dir = manifest_dir.as_ref().map(PathBuf::as_path);

        let res = if let Some(out) = built_out {
            let res = built::write_built_file_with_opts(manifest_dir, &out);

            if let Err(e) = res {
                println!("cargo::warning=built failed to produce metadata: {e}");
                let built_empty = "src/built.rs";
                let p = manifest_dir.as_ref()
                    .map(|p| p.join(built_empty))
                    .unwrap_or_else(|| built_empty.into());
                let res = fs::copy(&p, &out);
                res.map(drop).map_err(Some)
            } else { Ok(()) }
        } else { Err(None) };

        if let Err(e) = res {
            println!("cargo::warning=failed to stub out built metadata: {e:?}");
        }
    }
}
