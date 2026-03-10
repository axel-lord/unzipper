//! Library used to unzip zip content.

use ::core::{fmt::Debug, sync::atomic::AtomicU64};
use ::std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};

use ::bytesize::ByteSize;
use ::chardetng::EncodingDetector;
use ::log::log_enabled;
use ::tap::Pipe;
use ::zip::{ZipArchive, read::ZipFile};

use crate::dynamic::ReadSeek;

pub use self::encoding::{Encoding, EncodingFromStrError};

mod dynamic;
mod encoding;
mod verify;

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

/// Alias for reader type.
type Reader<'a> = BufReader<&'a mut dyn ReadSeek>;

/// Crate result type.
pub type Result<T = (), E = UnzipError> = ::core::result::Result<T, E>;

/// Storage for shared progress indication.
#[derive(Debug)]
pub struct Progress {
    /// Target size to extract.
    target: AtomicU64,
    /// Extracted size.
    completed: AtomicU64,
}

impl Progress {
    /// Construct new progress state.
    pub const fn new() -> Self {
        Self {
            target: AtomicU64::new(1),
            completed: AtomicU64::new(1),
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

/// Unzipper configuration.
#[derive(Clone, Debug, Copy)]
pub struct Unzipper<'lt> {
    /// Encoding of filenames.
    pub encoding: Encoding,
    /// Password to use for files.
    pub password: Option<&'lt [u8]>,
    /// Null terminate when listing.
    pub null_terminate: bool,
    /// Print progress to stdout.
    pub print_progress: Option<&'lt Progress>,
    /// Max size of chunks to extract.
    pub chunk_size: Option<u64>,
}

impl<'lt> Default for Unzipper<'lt> {
    fn default() -> Self {
        Self::new()
    }
}

// Builder api.
impl<'lt> Unzipper<'lt> {
    /// Crate a new unzipper with default values.
    pub const fn new() -> Self {
        Self {
            encoding: Encoding::Auto,
            password: None,
            null_terminate: false,
            print_progress: None,
            chunk_size: None,
        }
    }

    /// Set password of unzipper, if not set no password is used.
    pub const fn password(self, password: &'lt [u8]) -> Self {
        Self {
            password: Some(password),
            ..self
        }
    }

    /// Should null characters be used to terminate lines when listing, default is false.
    pub const fn null_terminate(self, yes: bool) -> Self {
        Self {
            null_terminate: yes,
            ..self
        }
    }

    /// Should progress be printed to stdout, default is false.
    pub const fn print_progress(self, progress: &'lt Progress) -> Self {
        Self {
            print_progress: Some(progress),
            ..self
        }
    }

    /// Set which encoding should be used, default is [Encoding::Auto].
    pub const fn encoding(self, encoding: Encoding) -> Self {
        Self { encoding, ..self }
    }

    /// Set chunk size in mib.
    pub fn chunk_size_mib(self, chunk_size: u64) -> Self {
        Self {
            chunk_size: Some(::bytesize::mib(chunk_size)),
            ..self
        }
    }
}

// Public usage api.
impl Unzipper<'_> {
    /// Attempt to find filename encoding of zip file.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    /// Or if encoding cannot be determined [UnzipError::NoEncoding] is returned.
    pub fn detect_encoding(&self, src: &Path) -> Result<&'static ::encoding_rs::Encoding> {
        let mut file = self.open_file(src)?;
        let mut archive = self.read_archive(&mut file)?;
        self.detect_archive_encoding(&mut archive, src)
            .ok_or(UnzipError::NoEncoding)
    }

    /// List archive contents.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    pub fn list(&self, src: &Path, write: &mut dyn Write) -> Result {
        let mut file = self.open_file(src)?;
        self.list_archive(src, write, self.read_archive(&mut file)?)
    }

    /// Unzip src into dest.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    /// Or if dest cannot be created / cannot be written to.
    pub fn unzip(&self, src: &Path, dest: &Path) -> Result {
        let mut file = self.open_file(src)?;

        if let Some(Progress { target, .. }) = self.print_progress
            && let Some(len) = file.metadata().ok().map(|meta| meta.len())
        {
            target.fetch_add(len, ::core::sync::atomic::Ordering::Relaxed);
        }

        self.unzip_archive(src, dest, self.read_archive(&mut file)?)
    }
}

// Private api/helpers.
impl Unzipper<'_> {
    /// Unzip an archive into dest.
    fn unzip_archive(&self, src: &Path, dest: &Path, mut archive: ZipArchive<Reader>) -> Result {
        let encoding = self.get_encoding(&mut archive, src);

        for index in 0..archive.len() {
            self.handle_file(&mut archive, index, encoding, src, dest);
        }

        Ok(())
    }

    /// List archive contents.
    fn list_archive(
        &self,
        src: &Path,
        write: &mut dyn Write,
        mut archive: ZipArchive<Reader>,
    ) -> Result {
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
                _ = verify::zip_file(&zip_file, path, src, ::log::Level::Info);
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

    /// Open file at src.
    /// Open zip archive at src.
    fn read_archive<'a>(&self, src: &'a mut dyn ReadSeek) -> Result<ZipArchive<Reader<'a>>> {
        BufReader::new(src)
            .pipe(ZipArchive::new)
            .map_err(UnzipError::ReadZip)
    }

    /// Short wrapper for [File::open].
    fn open_file(&self, src: &Path) -> Result<File> {
        File::open(src).map_err(UnzipError::OpenSrc)
    }

    /// Detect encoding of an archive.
    fn detect_archive_encoding(
        &self,
        archive: &mut ZipArchive<Reader>,
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
    fn get_encoding(
        &self,
        archive: &mut ZipArchive<Reader>,
        src: &Path,
    ) -> &'static ::encoding_rs::Encoding {
        match self.encoding {
            Encoding::Auto => self
                .detect_archive_encoding(archive, src)
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

    /// Adjust prgress target according to given zip file.
    fn adjust_progress(&self, zip_file: &ZipFile<Reader>) {
        if let Some(Progress { target, .. }) = self.print_progress {
            let size = zip_file.size();
            let compressed = zip_file.compressed_size();

            if compressed < size {
                let diff = size - compressed;
                target.fetch_add(diff, ::core::sync::atomic::Ordering::Relaxed)
            } else {
                let diff = compressed - size;
                target.fetch_sub(diff, ::core::sync::atomic::Ordering::Relaxed)
            };
        }
    }

    /// Get completion percentage.
    fn get_percentage(&self) -> Option<u64> {
        let progress = self.print_progress?;
        let completed = progress
            .completed
            .load(::core::sync::atomic::Ordering::Relaxed);
        let target = progress
            .target
            .load(::core::sync::atomic::Ordering::Relaxed);

        let completed = completed * 10;
        let target = (target / 10).max(1);

        Some(completed / target)
    }

    /// Handle file in archive.
    fn handle_file(
        &self,
        archive: &mut ZipArchive<Reader>,
        index: usize,
        encoding: &'static ::encoding_rs::Encoding,
        src: &Path,
        dest: &Path,
    ) {
        let result = if let Some(password) = self.password {
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

        if !verify::path_components(path, src, ::log::Level::Warn)
            | !verify::file_type(&zip_file, path, src, ::log::Level::Warn)
        {
            return;
        }

        if zip_file.is_dir() {
            let path = dest.join(path);
            if let Err(err) = ::std::fs::create_dir_all(&path) {
                ::log::error!("could not create {path:?}\n{err}");
            }
            return;
        }

        let Some((prefix, file_name)) = verify::has_name(path, src, ::log::Level::Warn) else {
            return;
        };

        let mut path = dest.join(prefix);
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

        if log_enabled!(::log::Level::Info) {
            ::log::info!(
                "extracting {path:?} [{}]",
                ByteSize::b(zip_file.size()).display()
            );
        }

        self.adjust_progress(&zip_file);

        if self.print_progress.is_some() {
            _ = writeln!(
                ::std::io::stdout().lock(),
                "#[{}] {decoded}",
                ByteSize::b(zip_file.size())
            );
        }

        if let Some(chunk_size) = self.chunk_size {
            loop {
                match ::std::io::copy(&mut zip_file.by_ref().take(chunk_size), &mut file) {
                    Ok(0) => break,
                    Ok(count) => {
                        if let Some(Progress { completed, .. }) = self.print_progress {
                            completed.fetch_add(count, ::core::sync::atomic::Ordering::Relaxed);
                        }

                        if let Some(percentage) = self.get_percentage() {
                            _ = writeln!(::std::io::stdout().lock(), "{percentage}")
                        }
                    }
                    Err(err) => {
                        ::log::error!("could not fully extract to {path:?}\n{err}");
                        break;
                    }
                }
            }
        } else {
            if let Err(err) = ::std::io::copy(&mut zip_file, &mut file).and_then(|_| file.flush()) {
                ::log::error!("could not extract to {path:?}\n{err}");
            }
            if let Some(Progress { completed, .. }) = self.print_progress {
                completed.fetch_add(zip_file.size(), ::core::sync::atomic::Ordering::Relaxed);
            }
        }
        if let Some(percentage) = self.get_percentage() {
            _ = writeln!(::std::io::stdout().lock(), "{percentage}")
        }
    }
}
