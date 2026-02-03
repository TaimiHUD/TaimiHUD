#[cfg(feature = "built-info")]
use std::{fs, path::PathBuf};

use {
    semver::{BuildMetadata, Prerelease, Version},
    std::env,
};

const FEATURE_BUILT: &'static str = "CARGO_FEATURE_BUILT_INFO";
const FEATURE_NEXUS_CODEGEN: &'static str = "CARGO_FEATURE_EXTENSION_NEXUS_CODEGEN";
const FEATURE_NEXUS_EXTERN: &'static str = "CARGO_FEATURE_EXTENSION_NEXUS_EXTERN";
const FEATURE_NEXUS: &'static str = "CARGO_FEATURE_EXTENSION_NEXUS";
const FEATURE_UPDATES: &'static str = "CARGO_FEATURE_UPDATES";
fn main() {
    println!("cargo::rerun-if-env-changed={FEATURE_BUILT}");
    println!("cargo::rerun-if-env-changed={FEATURE_NEXUS_CODEGEN}");
    println!("cargo::rerun-if-env-changed={FEATURE_NEXUS_EXTERN}");
    println!("cargo::rerun-if-env-changed={FEATURE_NEXUS}");

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
const ADDON_VERSION_NEXUS: &'static str = "ADDONAPI_VERSION";
const ADDON_URL: &'static str = "ADDON_URL";

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
    #[cfg(feature = "built-info")]
    println!("cargo::rerun-if-env-changed=CARGO_MANIFEST_DIR");

    let ci = env::var_os(&format!("{built_env_prefix}{BUILT_ATTR_CI}"));
    let commit = env::var(&format!("{built_env_prefix}{BUILT_ATTR_REF}"));
    #[cfg(feature = "built-info")]
    let (built_git_head, built_git_tagdesc) = {
        let git_root = manifest_dir();
        let head = git_root
            .as_ref()
            .and_then(|dir| built::util::get_repo_head(dir).ok())
            .flatten();
        let desc = git_root
            .as_ref()
            .and_then(|dir| built::util::get_repo_description(dir).ok())
            .flatten();
        let desc = match (desc, &head) {
            (Some((name, dirty)), Some((_ref, _long, short))) if name.ends_with(short) => {
                // filter out "tag" with `-${distance}-g${short}` generated via `git describe`
                Some((None, dirty))
            },
            (desc, _) => desc.map(|(name, dirty)| (Some(name), dirty)),
        };
        (head, desc)
    };
    #[allow(unreachable_patterns)]
    let commit = match commit {
        Ok(commit) => Some(commit),
        #[cfg(feature = "built-info")]
        Err(..) => match (&built_git_head, &built_git_tagdesc) {
            (_, Some((Some(tag), _dirty))) => Some(match tag.starts_with("refs/") {
                false => format!("refs/tags/{tag}"),
                // unlikely but just in case...
                true => tag.clone(),
            }),
            (Some((Some(head), ..)), _) => Some(match head.starts_with("refs/") {
                false => format!("refs/heads/{head}"),
                true => head.clone(),
            }),
            (Some((None, long, ..)), _) => Some(long.clone()),
            _ => None,
        },
        _ => None,
    };
    #[allow(unreachable_patterns)]
    let release = match &commit {
        Some(head) => head
            .strip_prefix("refs/tags/v")
            .map(Ok)
            .or(head.strip_prefix("refs/heads/").map(Err)),
        None => None,
    };
    let dirty = match env::var(&format!("{built_env_prefix}{BUILT_ATTR_REV}")) {
        Ok(rev) => rev.ends_with("-dirty"),
        #[allow(unreachable_patterns)]
        _ => match ci.is_none() {
            #[cfg(feature = "built-info")]
            dirty_default => built_git_tagdesc
                .as_ref()
                .map(|(_tag, dirty)| *dirty)
                .unwrap_or(dirty_default),
            dirty => dirty,
        },
    };
    let mut rev = match env::var(&format!("{built_env_prefix}{BUILT_ATTR_REV}")) {
        Ok(r) if r.is_empty() => None,
        Ok(rev) => Some(rev.strip_suffix("-dirty").map(String::from).unwrap_or(rev)),
        _ => None,
    };
    let mut rev_short = match env::var(&format!("{built_env_prefix}{BUILT_ATTR_REV_SHORT}")) {
        Ok(r) if r.is_empty() => None,
        Ok(rev) if rev.len() >= 7 => Some(rev.strip_suffix("-dirty").map(String::from).unwrap_or(rev)),
        _ => None,
    };
    if rev.is_none() {
        match &commit {
            Some(commit) if release.is_none() && commit.len() == 40 => rev = Some(commit.clone()),
            _ => (),
        }
    }
    #[cfg(feature = "built-info")]
    if rev.is_none() {
        if let Some((_head, long, short)) = built_git_head {
            rev = Some(long);
            let _ = rev_short.get_or_insert(short);
        }
    }
    if rev_short.is_none() {
        if let Some(short) = rev.as_ref().and_then(|rev| rev.get(..10)) {
            rev_short = Some(short.into());
        }
    }
    let debug = env::var_os("PROFILE") != Some("release".into());

    let mut tags = Vec::new();

    println!("cargo::rerun-if-env-changed=CARGO_PKG_VERSION");
    let pkg_version = env::var("CARGO_PKG_VERSION").ok();
    let pkg_build = {
        let ci = ci.is_none().then_some("local");
        if let Some(rev) = &rev_short {
            let ci_sep = ci.is_some().then_some("-").unwrap_or("");
            let ci = ci.unwrap_or("");
            format!("{rev}{ci_sep}{ci}")
        } else {
            ci.unwrap_or("").into()
        }
    };
    let imperative_build = pkg_build == "local";
    let mut version = None;
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
        let version = version.insert(match release_version {
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
                                    branch if branch.contains("/") =>
                                        &*release_channel.insert(branch.replace("/", "-")),
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
        });

        println!("cargo::rustc-env={ADDON_VERSION}_BUILD={}", version.build);
        println!("cargo::rustc-env={ADDON_VERSION}_PRE={}", version.pre);
        println!("cargo::rustc-env={ADDON_VERSION}_MAJOR={}", version.major);
        println!("cargo::rustc-env={ADDON_VERSION}_MINOR={}", version.minor);
        println!("cargo::rustc-env={ADDON_VERSION}_PATCH={}", version.patch);
        println!("cargo::rustc-env={ADDON_VERSION}={version}");

        if version.pre.is_empty() {
            println!("cargo::rustc-env={ADDON_VERSION}_RELEASE=z");
        } else {
            let (mut major, mut minor, mut build, mut rev) = (
                version.major as i16,
                version.minor as i16,
                version.patch as i16,
                0i16,
            );
            if let Some(rc) = version.pre.strip_prefix("rc.") {
                println!("cargo::rustc-env={ADDON_VERSION}_RELEASE={}", version.pre);
                if env::var_os(FEATURE_NEXUS).is_some() {
                    match version.minor.checked_sub(1) {
                        Some(m) => minor = m as i16,
                        None => {
                            major -= 1;
                            minor = 99;
                        },
                    }
                    let pre_rc = rc.split(".").next().unwrap_or(rc);
                    let pre_rc = pre_rc.parse::<u16>().ok().unwrap_or(version.patch as u16);
                    build = 900i16 + pre_rc as i16;
                }
            } else {
                let prerev = version.pre.split(".").nth(1).map(str::parse::<u64>);
                if let Some(Ok(pre)) = prerev {
                    // TODO
                    rev = -0x6c00i16 + pre as i16;
                }
            }
            if env::var_os(FEATURE_NEXUS_CODEGEN).is_some() {
                #[cfg(todo)]
                if !is_rc {
                    mem::swap(&mut build, &mut rev);
                }
                println!("cargo::rustc-env=CARGO_PKG_VERSION_MAJOR={major}");
                println!("cargo::rustc-env=CARGO_PKG_VERSION_MINOR={minor}");
                println!("cargo::rustc-env=CARGO_PKG_VERSION_PATCH={build}");
            }
            if env::var_os(FEATURE_NEXUS_EXTERN).is_some() {
                // TODO? mem::swap(&mut build, &mut rev);
                println!("cargo::rustc-env={ADDON_VERSION_NEXUS}_MAJOR={major}");
                println!("cargo::rustc-env={ADDON_VERSION_NEXUS}_MINOR={minor}");
                println!("cargo::rustc-env={ADDON_VERSION_NEXUS}_BUILD={build}");
                println!("cargo::rustc-env={ADDON_VERSION_NEXUS}_REVISION={rev}");
            }
        }

        release_channel
    } else {
        let ci = ci.is_none().then_some("local");
        if let Some(rev) = &rev_short {
            let ci_sep = ci.is_some().then_some("-").unwrap_or("");
            let ci = ci.unwrap_or("");
            println!("cargo::rustc-env={ADDON_VERSION}_BUILD={rev}{ci_sep}{ci}");
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
            Some(Err("main")) => "dev".into(),
            Some(Err("develop")) => "develop".into(),
            Some(Err(branch)) if branch.contains("/") => branch.replace("/", "-"),
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
    {
        println!("cargo::rerun-if-env-changed=CARGO_PKG_HOMEPAGE");
        if let Ok(webroot) = env::var("CARGO_PKG_HOMEPAGE") {
            println!("cargo::rustc-cfg=taimi_has={:?}", "url-update-base");
            let ext = ".dll";
            let update_base = format!("{webroot}/taimi_data/update/{package}");
            println!("cargo::rustc-env={ADDON_URL}_UPDATE_BASE={update_base}");

            println!("cargo::rustc-cfg=taimi_has={:?}", "url-update-direct");
            let version = version
                .as_ref()
                .map(ToString::to_string)
                .or(pkg_version.clone())
                .unwrap_or_default();
            let channel = release_channel.unwrap_or("release");
            println!("cargo::rustc-env={ADDON_URL}_UPDATE_DIRECT={update_base}/{channel}/{package}{ext}?v={version}");
        }
        let github_org = "TaimiHUD";
        let github_repo = "TaimiHUD";
        let github_url = format!("https://github.com/{github_org}/{github_repo}");
        println!("cargo::rustc-cfg=taimi_has={:?}", "url-github");
        println!("cargo::rustc-env={ADDON_URL}_GITHUB_OWNER={github_org}");
        println!("cargo::rustc-env={ADDON_URL}_GITHUB_REPO={github_repo}");
        println!("cargo::rustc-env={ADDON_URL}_GITHUB={github_url}");
        let has_updates = env::var_os(FEATURE_UPDATES).is_some();
        let update_method = match release_channel {
            None | Some("rc") => "github",
            #[cfg(todo)]
            Some("debug") => "none",
            Some(..) if has_updates => "manual",
            Some(..) if release.as_ref().map(|r| r.is_ok()).unwrap_or(false) => "direct",
            #[cfg(todo)]
            Some(..) => "direct",
            Some(..) => "none",
        };
        println!("cargo::rustc-cfg=taimi_update=\"{update_method}\"");
    }

    tags.push(release_channel.map(|c| match c {
        "rc" => "Prerelease Test",
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

    let mut release_cfg = Vec::new();
    let is_branch = match &release {
        Some(Ok(_)) => {
            release_cfg.push("tag");
            false
        },
        Some(Err("")) | None => false,
        Some(Err(..)) => true,
    };
    let ci_str = ci.as_ref().and_then(|ci| ci.to_str());
    match (release_channel, ci_str) {
        (None, _) => release_cfg.push("release"),
        (Some("debug"), ..) => release_cfg.push("debug"),
        #[cfg(todo)]
        (Some(ch), Some("nix") | Some("drv")) if ch.starts_with("dev") || is_branch => (),
        (Some(ch), None | Some("local")) if ch.starts_with("dev") || is_branch => release_cfg.push("debug"),
        _ if dirty => (),
        (Some("rc"), _) => release_cfg.push("rc"),
        (Some(ch), Some(..)) if ch.starts_with("dev") || is_branch => release_cfg.push("branch"),
        (Some(..), _) => release_cfg.push("pre"),
    }
    let mut a_release = false;
    for dev in release_cfg {
        match dev {
            "debug" | "release" => {
                println!("cargo::rustc-cfg=taimi_{dev}");
                println!("cargo::rustc-cfg=taimi_{dev}={dev:?}");
                if dev == "debug" {
                    println!("cargo::rustc-cfg=taimi_dev={dev:?}");
                } else {
                    a_release = true;
                }
            },
            "drv" | "ci" | "branch" => println!("cargo::rustc-cfg=taimi_dev={dev:?}"),
            "tag" | "rc" | "pre" => {
                a_release = true;
                println!("cargo::rustc-cfg=taimi_release={dev:?}")
            },
            _ => (),
        }
    }
    match ci_str {
        _ if a_release => (),
        None | Some("local") => (),
        Some("nix") | Some("drv") => println!("cargo::rustc-cfg=taimi_dev={:?}", "drv"),
        Some(..) => println!("cargo::rustc-cfg=taimi_dev={:?}", "ci"),
    }

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
    if env::var_os(FEATURE_NEXUS_CODEGEN).is_some() && !imperative_build {
        // hack around inability to customize these...
        // (causes spurious rebuilds)
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
        let manifest_dir = manifest_dir();
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

#[cfg(feature = "built-info")]
fn manifest_dir() -> Option<PathBuf> {
    env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from)
}
