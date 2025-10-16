use std::env;
#[cfg(feature = "built-info")]
use std::{fs, path::PathBuf};

const FEATURE_BUILT: &'static str = "CARGO_FEATURE_BUILT_INFO";
fn main() {
    println!("cargo::rerun-if-env-changed={FEATURE_BUILT}");

    #[cfg(feature = "built-info")]
    write_built_info();

    apply_built_info();
}

const BUILT_ATTR_REF: &'static str = "GIT_HEAD_REF";
const BUILT_ATTR_CI: &'static str = "CI_PLATFORM";
const BUILT_ATTR_REV: &'static str = "GIT_COMMIT_HASH";
const BUILT_ATTR_REV_SHORT: &'static str = "GIT_COMMIT_HASH_SHORT";
const BUILT_ATTRS: &'static [&'static str] = &[
    BUILT_ATTR_CI,
    BUILT_ATTR_REF,
    BUILT_ATTR_REV,
    BUILT_ATTR_REV_SHORT,
];

const ADDON_TITLE: &'static str = "ADDON_TITLE";

fn apply_built_info() {
    println!("cargo::rustc-cfg=taimi_has={:?}", "title");
    let addon_title = "TaimiHUD";
    let package = env::var("CARGO_PKG_NAME");
    let package = match &package {
        Ok(p) => p,
        _ => "taimi_hud",
    };
    let built_env_prefix = format!("BUILT_OVERRIDE_{}_", package);
    if !has_built() {
        // otherwise I'd assume it probably already does this?
        for attr in BUILT_ATTRS {
            println!("cargo::rerun-if-env-changed={built_env_prefix}{attr}");
        }
    }
    println!("cargo::rerun-if-env-changed=PROFILE");
    println!("cargo::rerun-if-env-changed=CARGO_CRATE_NAME");
    println!("cargo::rerun-if-env-changed=CARGO_MANIFEST_DIR");

    let ci = env::var_os(&format!("{built_env_prefix}{BUILT_ATTR_CI}"));
    let commit = env::var(&format!("{built_env_prefix}{BUILT_ATTR_REF}"));
    let release = match &commit {
        Ok(head) => head.strip_prefix("refs/tags/v").map(Ok)
            .or(head.strip_prefix("refs/heads/").map(Err)),
        _ => None,
    };
    let dirty = match env::var(&format!("{built_env_prefix}{BUILT_ATTR_REV}")) {
        Ok(rev) => rev.ends_with("-dirty"),
        _ => true,
    };
    let debug = env::var_os("PROFILE") != Some("release".into());

    let mut tags = Vec::new();

    tags.push(match release {
        Some(Ok(tag)) => match env::var("CARGO_PKG_VERSION").ok().map(|v| tag.strip_prefix(&v)) {
            Some(Some("")) => None,
            Some(Some(suffix)) => Some(suffix),
            Some(None) => Some(tag),
            None => Some(tag),
        },
        Some(Err(branch)) if branch != "main" => Some(branch),
        _ => Some("develop"),
    });
    tags.push(ci.is_none().then_some("local"));
    tags.push(debug.then_some("debug"));
    tags.push(dirty.then_some("dirty"));

    if tags.iter().any(Option::is_some) {
        let tags = tags.into_iter().flatten().collect::<Vec<_>>().join("+");
        println!("cargo::rustc-env={ADDON_TITLE}={addon_title} ({tags})");
    } else {
        println!("cargo::rustc-env={ADDON_TITLE}={addon_title}");
    };
}

fn has_built() -> bool {
    env::var_os(FEATURE_BUILT).is_some()
}

#[cfg(feature = "built-info")]
fn write_built_info() {
    if has_built() {
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
