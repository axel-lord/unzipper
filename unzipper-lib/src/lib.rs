//! Library used to unzip zip content.

mod encoding;

use ::std::path::Path;

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
        Ok(())
    }
}
