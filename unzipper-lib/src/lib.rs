//! Library used to unzip zip content.

mod encoding;

use ::core::fmt::{self, Debug};
use ::std::{
    ffi::OsStr,
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::{Component, Path},
};

use ::chardetng::EncodingDetector;
use ::tap::Pipe;
use ::zip::{ZipArchive, read::ZipFile};

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
    /// Error returned when filename encoding could not be found.
    #[error("could not determine encoding of filenames")]
    NoEncoding,
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

/// Alias for exact type of zip archives.
type Archive = ZipArchive<BufReader<File>>;

/// Unzipper configuration.
#[derive(Clone, Debug, Copy, ::bon::Builder)]
pub struct Unzipper<'lt> {
    /// Encoding of filenames.
    #[builder(default = Encoding::Auto)]
    pub encoding: Encoding,
    /// Password to use for files.
    pub password: Option<&'lt [u8]>,
    /// Null terminate when listing.
    #[builder(default = false)]
    pub null_terminate: bool,
}

impl<'lt> Unzipper<'lt> {
    /// Open zip archive at src.
    fn open_archive(&self, src: &Path) -> Result<Archive, UnzipError> {
        File::open(src)
            .map(BufReader::new)
            .map_err(UnzipError::OpenSrc)?
            .pipe(ZipArchive::new)
            .map_err(UnzipError::ReadZip)
    }

    /// Detect encoding of an archive.
    fn detect_encoding(
        &self,
        archive: &mut Archive,
        src: &Path,
    ) -> Option<&'static ::encoding_rs::Encoding> {
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
        confident.then_some(encoding)
    }

    /// Get encoding either by detection or, by set value.
    fn get_encoding(&self, archive: &mut Archive, src: &Path) -> &'static ::encoding_rs::Encoding {
        match self.encoding {
            Encoding::Auto => self
                .detect_encoding(archive, src)
                .inspect(|encoding| {
                    ::log::info!("using encoding with confidence, {}", encoding.name());
                })
                .unwrap_or_else(|| {
                    ::log::info!("no encoding confidence, using utf-8");
                    ::encoding_rs::UTF_8
                }),
            Encoding::Set(encoding) => {
                ::log::info!("using set encoding, {}", encoding.name());
                encoding
            }
        }
    }

    /// Attempt to find filename encoding of zip file.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    /// Or if encoding cannot be determined [UnzipError::NoEncoding] is returned.
    pub fn encoding(&self, src: &Path) -> Result<&'static ::encoding_rs::Encoding, UnzipError> {
        let mut archive = self.open_archive(src)?;
        self.detect_encoding(&mut archive, src)
            .ok_or(UnzipError::NoEncoding)
    }

    /// List archive contents.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    pub fn list(&self, src: &Path, write: &mut dyn Write) -> Result<(), UnzipError> {
        let mut archive = self.open_archive(src)?;
        let encoding = self.get_encoding(&mut archive, src);

        for index in 0..archive.len() {
            let zip_file = match archive.by_index_raw(index) {
                Ok(file) => file,
                Err(err) => {
                    ::log::warn!("could not get file with index {index} in {src:?}\n{err}");
                    continue;
                }
            };

            let (file_name, _) = encoding.decode_without_bom_handling(zip_file.name_raw());

            // Get any warnings printed as info if verbose.
            if ::log::log_enabled!(::log::Level::Info) {
                let path = Path::new(&*file_name);
                _ = verify_zip_file(&zip_file, path, src, ::log::Level::Info);
            }

            if let Err(err) = write
                .write_all(file_name.as_bytes())
                .and_then(|_| write.write_all(if self.null_terminate { b"\0" } else { b"\n" }))
            {
                ::log::error!("write to stdout failed\n{err}")
            }
        }
        Ok(())
    }

    /// Unzip src into dest.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    /// Or if dest cannot be created / cannot be written to.
    pub fn unzip(&self, src: &Path, dest: Destination<'_>) -> Result<(), UnzipError> {
        let mut archive = self.open_archive(src)?;

        let encoding = self.get_encoding(&mut archive, src);

        for index in 0..archive.len() {
            handle_file(&mut archive, index, encoding, src, dest, self.password);
        }

        Ok(())
    }
}

/// Verification of zip file,
fn verify_zip_file(
    zip_file: &ZipFile<BufReader<File>>,
    path: &Path,
    src: &Path,
    level: ::log::Level,
) -> bool {
    verify_file_type(zip_file, path, src, level)
        && verify_path_components(path, src, level)
        && verify_has_name(path, src, level).is_some()
}

/// Verify all path components are valid.
fn verify_path_components(path: &Path, src: &Path, level: ::log::Level) -> bool {
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
fn verify_file_type(
    zip_file: &ZipFile<BufReader<File>>,
    path: &Path,
    src: &Path,
    level: ::log::Level,
) -> bool {
    if zip_file.is_symlink() {
        ::log::log!(level, "skipping symlink {path:?} in {src:?}, unsupported");
        false
    } else {
        true
    }
}

/// Verify a path has a filename.
fn verify_has_name<'a>(
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

/// Handle file in archive.
fn handle_file(
    archive: &mut Archive,
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

    if !verify_path_components(path, src, ::log::Level::Warn)
        | !verify_file_type(&zip_file, path, src, ::log::Level::Warn)
    {
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

    let Some((prefix, file_name)) = verify_has_name(path, src, ::log::Level::Warn) else {
        return;
    };

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
