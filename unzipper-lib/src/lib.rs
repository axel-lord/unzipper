//! Library used to unzip zip content.

mod encoding;

use ::std::{fs::File, io::BufReader, path::Path, sync::Arc};

use ::parking_lot::Mutex;
use ::zip::ZipArchive;

pub use self::encoding::{Encoding, EncodingFromStrError};

/// Unzipper configuration.
#[derive(Debug, Clone)]
pub struct Unzipper {
    /// Encoding of filenames.
    pub encoding: Encoding,
    /// Additional threads to use.
    pub threads: usize,
    /// Should common path prefix be removed.
    pub unfold: bool,
}

impl Unzipper {
    /// Unzip src into dest.
    ///
    /// # Errors
    /// On io errors related to reading src.
    /// Or if src is not a zip file / is malformed.
    /// Or if dest cannot be created / cannot be written to.
    pub fn unzip(&self, src: &Path, dest: &Path) -> ::std::io::Result<()> {
        let Self {
            encoding,
            threads,
            unfold,
        } = self;

        let file = File::open(src)?;
        let archive = ZipArchive::new(BufReader::new(&file))?;
        let metadata = archive.metadata();

        let thread_archives = (0..*threads)
            .filter_map(|_| {
                let file = file
                    .try_clone()
                    .inspect_err(|err| {
                        ::log::warn!("failed to clone file handle of {src:?}\n{err}")
                    })
                    .ok()
                    .map(BufReader::new)?;
                let meta = Arc::clone(&metadata);

                // SAFETY: Same file as metadata was created for is used.
                let archive = unsafe { ZipArchive::unsafe_new_with_metadata(file, meta) };
                Some(archive)
            })
            .collect::<Vec<_>>();

        Ok(())
    }
}

/// Archive item.
#[derive(Debug)]
struct Item {
    /// Index of archive item.
    index: usize,
}
