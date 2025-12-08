use {
    core::{fmt, mem},
    std::{
        ffi::OsString,
        path::{self, Path, PathBuf},
    },
};

#[inline]
pub const fn new_path_const(path: &str) -> &Path {
    unsafe { mem::transmute(path.as_bytes()) }
}

pub fn path_join_append_mut(path: &mut PathBuf) -> &mut OsString {
    let mut path = path.as_mut_os_string();
    match &mut path {
        #[cfg(todo = "unnecessary")]
        path if path.ends_with("/") => (),
        path => path.push(path::MAIN_SEPARATOR_STR),
    }
    path
}

pub fn path_join_append<D: fmt::Display>(path: &mut PathBuf, join: D) {
    use fmt::Write;

    let path = path_join_append_mut(path);
    let _res = write!(path, "{join}");
    #[cfg(debug_assertions)]
    if let Err(_e) = _res {
        log::error!("path_join_append to {path:?} should never fail");
    }
}

pub fn path_join<D: fmt::Display>(mut path: PathBuf, join: D) -> PathBuf {
    path_join_append(&mut path, join);
    path
}
