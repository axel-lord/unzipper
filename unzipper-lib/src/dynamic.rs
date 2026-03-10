//! Traits used for runtime dynamicism.

use ::std::io::{Read, Seek};

/// Trait allowing for Dynamic dispatch of [Read] and [Seek].
pub trait ReadSeek: Read + Seek {}
impl<R: Read + Seek> ReadSeek for R {}
