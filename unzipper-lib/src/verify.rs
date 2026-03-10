//! Property verification.

use ::std::{
    ffi::OsStr,
    path::{Component, Path},
};

use ::zip::read::ZipFile;

use crate::Reader;

/// Verification of zip file,
pub fn zip_file(zip_file: &ZipFile<Reader>, path: &Path, src: &Path, level: ::log::Level) -> bool {
    file_type(zip_file, path, src, level)
        && path_components(path, src, level)
        && has_name(path, src, level).is_some()
}

/// Verify all path components are valid.
pub fn path_components(path: &Path, src: &Path, level: ::log::Level) -> bool {
    for component in path.components() {
        if let Component::Prefix(_) | Component::RootDir | Component::ParentDir = component {
            ::log::log!(
                level,
                "skipping {path:?} in {src:?}, disallowed path component"
            );
            return false;
        }
    }
    true
}

/// Verify a file is not a symlink.
pub fn file_type(zip_file: &ZipFile<Reader>, path: &Path, src: &Path, level: ::log::Level) -> bool {
    if zip_file.is_symlink() {
        ::log::log!(level, "skipping symlink {path:?} in {src:?}, unsupported");
        false
    } else {
        true
    }
}

/// Verify a path has a filename.
pub fn has_name<'a>(
    path: &'a Path,
    src: &Path,
    level: ::log::Level,
) -> Option<(&'a Path, &'a OsStr)> {
    let mut components = path.components();
    let file_name = loop {
        match components.next_back() {
            None => {
                ::log::log!(level, "path {path:?} in {src:?} has no final component");
                return None;
            }
            Some(Component::Normal(file_name)) => break file_name,
            Some(_) => {}
        }
    };
    let prefix = components.as_path();

    Some((prefix, file_name))
}
