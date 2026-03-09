//! Library used to unzip zip content.

mod cloner;
mod encoding;

use ::core::{
    iter,
    num::NonZero,
    sync::atomic::{self, AtomicUsize},
};
use ::std::{
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
    sync::Arc,
};

use ::chardetng::EncodingDetector;
use ::memmap2::Mmap;
use ::parking_lot::Mutex;
use ::rayon::iter::{
    IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use ::tap::Conv;
use ::zip::ZipArchive;

use crate::cloner::fallible_cloner;

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

/// Unzipper configuration.
#[derive(Debug, Clone, ::bon::Builder)]
pub struct Unzipper {
    /// Encoding of filenames.
    pub encoding: Encoding,
    /// Threads to use.
    pub threads: NonZero<usize>,
    /// Should common path prefix be removed.
    pub unfold: bool,
    /// Password to use for files.
    pub password: Option<Vec<u8>>,
}

impl Unzipper {
    /// Unzip src into dest.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    /// Or if dest cannot be created / cannot be written to.
    pub fn unzip(&self, src: &Path, dest: &Path) -> Result<(), UnzipError> {
        let Self {
            encoding,
            threads,
            unfold,
            password,
        } = self;

        let file = File::open(src).map_err(UnzipError::OpenSrc)?;
        if let Err(err) = file.try_lock_shared() {
            ::log::warn!("could not lock {src:?}\n{err}");
        }
        let mem_map = unsafe { Mmap::map(&file) }.map_err(UnzipError::Memmap)?;
        let reader = || Cursor::new(&mem_map);

        let mut metadata = None;
        let mut len = 0;
        let mut archives = (0..threads.get())
            .map(|_| {
                if let Some(metadata) = &metadata {
                    let metadata = Arc::clone(metadata);
                    // SAFETY: Same file as metadata was created for is used.
                    let archive =
                        unsafe { ZipArchive::unsafe_new_with_metadata(reader(), metadata) };
                    Ok(archive)
                } else {
                    match ZipArchive::new(reader()) {
                        Ok(archive) => {
                            metadata = Some(archive.metadata());
                            len = archive.len();
                            Ok(archive)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(UnzipError::ReadZip)?;
        let _ = metadata;
        let len = len;

        let counter = AtomicUsize::new(0);
        let items = archives
            .par_iter_mut()
            .flat_map_iter(|archive| {
                iter::from_fn(|| {
                    let idx = counter.fetch_add(1, atomic::Ordering::Relaxed);
                    if idx < len { Some(idx) } else { None }
                })
                .fuse()
                .filter_map(|index| {
                    let name = archive
                        .by_index_raw(index)
                        .map_err(|err| {
                            ::log::warn!("could not get file with index {index} in {src:?}\n{err}")
                        })
                        .ok()?
                        .name_raw()
                        .conv::<Vec<u8>>();

                    Some((index, name))
                })
            })
            .collect::<Vec<_>>();

        let encoding = match encoding {
            Encoding::Auto => {
                let mut detector = EncodingDetector::new();
                for (_, name) in &items {
                    detector.feed(name, false);
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

        let stack = Mutex::new(items);
        archives.into_par_iter().for_each(|mut archive| {
            loop {
                let item = stack.lock().pop();
                let Some((index, name)) = item else {
                    break;
                };

                let (decoded, has_replaced) = encoding.decode_without_bom_handling(&name);
                let path = Path::new(&*decoded);
            }
        });

        Ok(())
    }
}
