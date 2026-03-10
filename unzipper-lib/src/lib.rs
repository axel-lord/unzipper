//! Library used to unzip zip content.

mod encoding;

use ::core::fmt::{self, Debug};
use ::std::{
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::{Component, Path},
};

use ::chardetng::EncodingDetector;
use ::tap::Pipe;
use ::zip::ZipArchive;

pub use self::encoding::{Encoding, EncodingFromStrError};

/// Errors returned by attempts to unzip files.
#[derive(Debug, ::thiserror::Error)]
pub enum UnzipError {
    /// Error returned when source file could not be opened.
    #[error("could not open file\n{0}")]
    OpenSrc(::std::io::Error),
    /// Error returned when source could not be memory mapped.
    #[error("could not memory map file\n{0}")]
    Memmap(::std::io::Error),
    /// Error returned when source file is not a zip archive.
    #[error("file is not a zip archive\n{0}")]
    ReadZip(::zip::result::ZipError),
}

/// Destination to 'extract' files to.
#[derive(Clone, Copy)]
pub enum Destination<'lt> {
    /// Extract into the given directory.
    Exdir(&'lt Path),
    /// List filenames by printing lines to given writer.
    List(&'lt (dyn Sync + for<'a> Fn(&'a Path))),
}

impl Debug for Destination<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exdir(path) => f.debug_tuple("Exdir").field(path).finish(),
            Self::List(..) => f.debug_tuple("List").finish_non_exhaustive(),
        }
    }
}

/// Unzipper configuration.
#[derive(Debug, Clone, ::bon::Builder)]
pub struct Unzipper<'lt> {
    /// Encoding of filenames.
    pub encoding: Encoding,
    /// Password to use for files.
    pub password: Option<&'lt [u8]>,
}

impl<'lt> Unzipper<'lt> {
    /// Unzip src into dest.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    /// Or if dest cannot be created / cannot be written to.
    pub fn unzip(&self, src: &Path, dest: Destination<'_>) -> Result<(), UnzipError> {
        let Self { encoding, password } = self;

        let mut archive = File::open(src)
            .map(BufReader::new)
            .map_err(UnzipError::OpenSrc)?
            .pipe(ZipArchive::new)
            .map_err(UnzipError::ReadZip)?;

        let encoding = match encoding {
            Encoding::Auto => {
                let mut detector = EncodingDetector::new();
                for index in 0..archive.len() {
                    let file = match archive.by_index_raw(index) {
                        Ok(file) => file,
                        Err(err) => {
                            ::log::warn!("could not get file with index {index} in {src:?}\n{err}");
                            continue;
                        }
                    };
                    detector.feed(file.name_raw(), false);
                }
                detector.feed(b"", true);
                let (encoding, confident) = detector.guess_assess(None, true);
                if confident {
                    ::log::info!("using encoding with confidence, {}", encoding.name());
                    encoding
                } else {
                    ::log::info!("no encoding confidence, using utf-8");
                    ::encoding_rs::UTF_8
                }
            }
            Encoding::Set(encoding) => {
                ::log::info!("using set encoding, {}", encoding.name());
                encoding
            }
        };

        for index in 0..archive.len() {
            handle_file(&mut archive, index, encoding, src, dest, *password);
        }

        Ok(())
    }
}

/// Handle file in archive.
fn handle_file(
    archive: &mut ZipArchive<BufReader<File>>,
    index: usize,
    encoding: &'static ::encoding_rs::Encoding,
    src: &Path,
    dest: Destination,
    password: Option<&[u8]>,
) {
    let result = if let Some(password) = password {
        archive.by_index_decrypt(index, password)
    } else {
        archive.by_index(index)
    };
    let mut zip_file = match result {
        Ok(file) => file,
        Err(err) => {
            ::log::warn!("could not get file with index {index} in {src:?}\n{err}");
            return;
        }
    };

    let (decoded, _has_replaced) = encoding.decode_without_bom_handling(zip_file.name_raw());
    let path = Path::new(&*decoded);
    for component in path.components() {
        if let Component::Prefix(_) | Component::RootDir | Component::ParentDir = component {
            ::log::warn!("skipping {path:?} in {src:?}, disallowed path component");
            return;
        }
    }

    if zip_file.is_symlink() {
        ::log::warn!("skipping symlink {path:?} in {src:?}, unsupported");
        return;
    }

    if let Destination::List(list) = dest {
        list(path);
        return;
    }

    let exdir = match dest {
        Destination::Exdir(exdir) => exdir,
        Destination::List(list) => {
            list(path);
            return;
        }
    };

    if zip_file.is_dir() {
        let path = exdir.join(path);
        if let Err(err) = ::std::fs::create_dir_all(&path) {
            ::log::error!("could not create {path:?}\n{err}");
        }
        return;
    }

    let mut components = path.components();
    let file_name = loop {
        match components.next_back() {
            None => {
                ::log::warn!("path {path:?} in {src:?} has no final component");
                return;
            }
            Some(Component::Normal(file_name)) => break file_name,
            Some(_) => {}
        }
    };
    let prefix = components.as_path();

    let mut path = exdir.join(prefix);
    if let Err(err) = ::std::fs::create_dir_all(&path) {
        ::log::error!("could not create {path:?}\n{err}");
        return;
    }
    path.push(file_name);

    let mut file = match ::std::fs::File::create(&path).map(BufWriter::new) {
        Ok(file) => file,
        Err(err) => {
            ::log::error!("could not create {path:?}\n{err}");
            return;
        }
    };

    if let Err(err) = ::std::io::copy(&mut zip_file, &mut file).and_then(|_| file.flush()) {
        ::log::error!("could not extract to {path:?}\n{err}");
    }
}
