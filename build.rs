#[cfg(feature = "built-info")]
use std::{fs, path::PathBuf};

use {
    semver::{BuildMetadata, Prerelease, Version},
    std::env,
};

const FEATURE_BUILT: &'static str = "CARGO_FEATURE_BUILT_INFO";
const FEATURE_NEXUS_CODEGEN: &'static str = "CARGO_FEATURE_EXTENSION_NEXUS_CODEGEN";
fn main() {
    println!("cargo::rerun-if-env-changed={FEATURE_BUILT}");
    println!("cargo::rerun-if-env-changed={FEATURE_NEXUS_CODEGEN}");

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
const ADDON_AUTHOR: &'static str = "ADDON_AUTHOR";
const ADDON_VERSION: &'static str = "ADDON_VERSION";

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
        Ok(head) => head
            .strip_prefix("refs/tags/v")
            .map(Ok)
            .or(head.strip_prefix("refs/heads/").map(Err)),
        _ => None,
    };
    let dirty = match env::var(&format!("{built_env_prefix}{BUILT_ATTR_REV}")) {
        Ok(rev) => rev.ends_with("-dirty"),
        _ => true,
    };
    let debug = env::var_os("PROFILE") != Some("release".into());

    let mut tags = Vec::new();

    println!("cargo::rerun-if-env-changed=CARGO_PKG_VERSION");
    let pkg_version = env::var("CARGO_PKG_VERSION").ok();
    let pkg_build = {
        let ci = ci.is_none().then_some("local");
        if let Some(rev) = env::var_os(&format!("{built_env_prefix}{BUILT_ATTR_REV_SHORT}")) {
            let ci_sep = ci.is_some().then_some("-").unwrap_or("");
            let ci = ci.unwrap_or("");
            format!("{}{ci_sep}{ci}", rev.display())
        } else {
            ci.unwrap_or("").into()
        }
    };
    let release_channel = if let Some(pkg_version) =
        pkg_version.as_ref().and_then(|v| v.parse::<Version>().ok())
    {
        println!("cargo::rustc-cfg=taimi_has={:?}", "version");
        let mut release_channel: Option<String>;

        let release_version = release
            .and_then(|r| r.ok())
            .and_then(|r| match r.parse::<Version>() {
                Ok(v) => Some(v),
                Err(e) => {
                    println!("cargo::warning=release version {r:?} not valid semver: {e}");
                    None
                },
            });

        match &release_version {
            Some(release_version) if release_version.cmp_precedence(&pkg_version).is_eq() => (),
            Some(release_version) => {
                let partial_match = Version::new(release_version.major, release_version.minor, 0)
                    == Version::new(pkg_version.major, pkg_version.minor, 0);
                let msg = || format!("release version {release_version} mismatches package: {pkg_version}");
                if !release_version.pre.is_empty() || partial_match {
                    println!("cargo::warning={}", msg())
                } else {
                    panic!("{}", msg())
                }
            },
            None => (),
        }
        let version = match release_version {
            Some(version) => {
                if version.pre.is_empty() {
                    release_channel = None;
                } else {
                    let pre = version.pre.as_str();
                    release_channel = Some(pre.split(".").next().unwrap_or(pre).into());
                }
                version
            },
            None => {
                let mut version = pkg_version;
                if version.pre.is_empty() {
                    release_channel = Some(if debug { "debug" } else { "dev" }.into());
                    let pre = release
                        .map(|r| {
                            r.err().map(|branch| {
                                let channel = match branch {
                                    "main" => "dev",
                                    "develop" => "develop",
                                    branch => &*release_channel.insert(format!("dev-{branch}")),
                                };
                                // TODO?
                                let build_no = 0;
                                Prerelease::new(&format!("{channel}.{build_no}"))
                            })
                        })
                        .unwrap_or_else(|| Some(Prerelease::new("debug")));
                    if let Some(Ok(pre)) = pre {
                        version.pre = pre;
                    }
                } else {
                    let pre = version.pre.as_str();
                    release_channel = Some(pre.split(".").next().unwrap_or(pre).into());
                }
                if version.build.is_empty() && !pkg_build.is_empty() {
                    let build = BuildMetadata::new(&pkg_build);
                    if let Ok(build) = build {
                        version.build = build;
                    }
                }
                version
            },
        };

        println!("cargo::rustc-env={ADDON_VERSION}_BUILD={}", version.build);
        println!("cargo::rustc-env={ADDON_VERSION}_PRE={}", version.pre);
        println!("cargo::rustc-env={ADDON_VERSION}_MAJOR={}", version.major);
        println!("cargo::rustc-env={ADDON_VERSION}_MINOR={}", version.minor);
        println!("cargo::rustc-env={ADDON_VERSION}_PATCH={}", version.patch);
        println!("cargo::rustc-env={ADDON_VERSION}={version}");

        if version.pre.is_empty() {
            println!("cargo::rustc-env={ADDON_VERSION}_RELEASE=1");
        } else if let Some(rc) = version.pre.strip_prefix("rc.") {
            println!("cargo::rustc-env={ADDON_VERSION}_RELEASE={}", version.pre);
            if env::var_os(FEATURE_NEXUS_CODEGEN).is_some() {
                let (major, minor) = match version.minor.checked_sub(1) {
                    Some(minor) => (version.major, minor),
                    None => (version.major - 1, 99),
                };
                let pre_rc = rc.split(".").next().unwrap_or(rc);
                let pre_rc = pre_rc.parse::<u16>().ok().unwrap_or(version.patch as u16);
                println!("cargo::rustc-env=CARGO_PKG_VERSION_MAJOR={}", major);
                println!("cargo::rustc-env=CARGO_PKG_VERSION_MINOR={}", minor);
                println!("cargo::rustc-env=CARGO_PKG_VERSION_PATCH={}", 900 + pre_rc);
            }
        }

        release_channel
    } else {
        let ci = ci.is_none().then_some("local");
        if let Some(rev) = env::var_os(&format!("{built_env_prefix}{BUILT_ATTR_REV_SHORT}")) {
            let ci_sep = ci.is_some().then_some("-").unwrap_or("");
            let ci = ci.unwrap_or("");
            println!(
                "cargo::rustc-env={ADDON_VERSION}_BUILD={}{ci_sep}{ci}",
                rev.display()
            );
        } else {
            println!("cargo::rustc-env={ADDON_VERSION}_BUILD={}", ci.unwrap_or(""));
        }
        if let Some(pre) = env::var_os("CARGO_PKG_VERSION_PRE") {
            println!("cargo::rustc-env={ADDON_VERSION}_PRE={}", pre.display());
        }
        if let Some(major) = env::var_os("CARGO_PKG_VERSION_MAJOR") {
            println!("cargo::rustc-env={ADDON_VERSION}_MAJOR={}", major.display());
        }
        if let Some(minor) = env::var_os("CARGO_PKG_VERSION_MINOR") {
            println!("cargo::rustc-env={ADDON_VERSION}_MINOR={}", minor.display());
        }
        if let Some(patch) = env::var_os("CARGO_PKG_VERSION_PATCH") {
            println!("cargo::rustc-env={ADDON_VERSION}_PATCH={}", patch.display());
        }
        Some(match release {
            Some(Err(branch)) => format!("dev-{branch}"),
            Some(Ok(tag)) => tag.into(),
            None => "debug".into(),
        })
    };
    let release_channel = release_channel.as_ref().map(|c| &c[..]);
    println!(
        "cargo::rustc-env={ADDON_VERSION}_CHANNEL={}",
        release_channel.unwrap_or("")
    );

    tags.push(release_channel.map(|c| match c {
        "rc" => "Release Candidate",
        "debug" => "Debug",
        "dev" => "Main",
        "develop" => "Develop",
        c => c.strip_prefix("dev-").unwrap_or(c),
    }));
    tags.push(ci.is_none().then_some("local"));
    if release_channel != Some("debug") {
        tags.push(debug.then_some("debug"));
    }
    tags.push(dirty.then_some("dirty"));

    if tags.iter().any(Option::is_some) {
        let tags = tags.into_iter().flatten().collect::<Vec<_>>().join("+");
        println!("cargo::rustc-env={ADDON_TITLE}={addon_title} ({tags})");
    } else {
        println!("cargo::rustc-env={ADDON_TITLE}={addon_title}");
    };

    println!("cargo::rerun-if-env-changed=CARGO_PKG_AUTHORS");
    let addon_author = match env::var("CARGO_PKG_AUTHORS") {
        Ok(authors) => authors.split(":").collect::<Vec<_>>().join(", "),
        Err(..) => "TaimiHUD".into(),
    };
    println!("cargo::rustc-cfg=taimi_has={:?}", "author");
    println!("cargo::rustc-env={ADDON_AUTHOR}={addon_author}");
    if env::var_os(FEATURE_NEXUS_CODEGEN).is_some() {
        // hack around inability to customize these...
        println!("cargo::rustc-env=CARGO_PKG_AUTHORS={addon_author}");
    }
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
        let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from);
        let manifest_dir = manifest_dir.as_ref().map(PathBuf::as_path);

        let res = if let Some(out) = built_out {
            let res = built::write_built_file_with_opts(manifest_dir, &out);

            if let Err(e) = res {
                println!("cargo::warning=built failed to produce metadata: {e}");
                let built_empty = "src/built.rs";
                let p = manifest_dir
                    .as_ref()
                    .map(|p| p.join(built_empty))
                    .unwrap_or_else(|| built_empty.into());
                let res = fs::copy(&p, &out);
                res.map(drop).map_err(Some)
            } else {
                Ok(())
            }
        } else {
            Err(None)
        };

        if let Err(e) = res {
            println!("cargo::warning=failed to stub out built metadata: {e:?}");
        }
    }
}
