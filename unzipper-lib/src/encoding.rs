//! [Encoding] impl.

use ::core::{
    fmt::{self, Display},
    str::FromStr,
};
use ::std::{borrow::Cow, rc::Rc, sync::Arc};

/// Encoding to use when unzipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Encoding {
    /// Determine encoding by content.
    #[default]
    Auto,
    /// Use specified encoding.
    Set(&'static ::encoding_rs::Encoding),
}

impl Encoding {
    /// Get a string repr of self, either encoder name or auto.
    pub fn as_str<'a>(&self) -> &'a str {
        match self {
            Encoding::Auto => "auto",
            Encoding::Set(encoding) => encoding.name(),
        }
    }
}

impl Display for Encoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when an [Encoding] cannot be parsed from a label/string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, ::thiserror::Error)]
#[error("could not parse encoding from label")]
pub struct EncodingFromStrError;

impl FromStr for Encoding {
    type Err = EncodingFromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl TryFrom<Arc<str>> for Encoding {
    type Error = EncodingFromStrError;

    fn try_from(value: Arc<str>) -> Result<Self, Self::Error> {
        Self::try_from(value.as_bytes())
    }
}

impl TryFrom<Rc<str>> for Encoding {
    type Error = EncodingFromStrError;

    fn try_from(value: Rc<str>) -> Result<Self, Self::Error> {
        Self::try_from(value.as_bytes())
    }
}

impl<'a> TryFrom<Cow<'a, str>> for Encoding {
    type Error = EncodingFromStrError;

    fn try_from(value: Cow<'a, str>) -> Result<Self, Self::Error> {
        Self::try_from(value.as_bytes())
    }
}

impl TryFrom<String> for Encoding {
    type Error = EncodingFromStrError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_bytes())
    }
}

impl TryFrom<&str> for Encoding {
    type Error = EncodingFromStrError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.as_bytes())
    }
}

impl<const N: usize> TryFrom<[u8; N]> for Encoding {
    type Error = EncodingFromStrError;

    fn try_from(value: [u8; N]) -> Result<Self, Self::Error> {
        Self::try_from(value.as_slice())
    }
}

impl<const N: usize> TryFrom<&[u8; N]> for Encoding {
    type Error = EncodingFromStrError;

    fn try_from(value: &[u8; N]) -> Result<Self, Self::Error> {
        Self::try_from(value.as_slice())
    }
}

impl TryFrom<&[u8]> for Encoding {
    type Error = EncodingFromStrError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.eq_ignore_ascii_case(b"auto") {
            Ok(Self::Auto)
        } else {
            ::encoding_rs::Encoding::for_label(value)
                .map(Self::Set)
                .ok_or(EncodingFromStrError)
        }
    }
}
