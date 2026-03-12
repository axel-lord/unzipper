//! Encoding cli help.

use ::core::{fmt::Display, str::FromStr};
use ::std::{
    collections::BTreeSet,
    sync::{LazyLock, OnceLock},
};

use ::clap::{ValueEnum, builder::PossibleValue};
use ::encoding_rs::{
    BIG5, EUC_JP, EUC_KR, Encoding, GB18030, GBK, IBM866, ISO_2022_JP, ISO_8859_2, ISO_8859_3,
    ISO_8859_4, ISO_8859_5, ISO_8859_6, ISO_8859_7, ISO_8859_8, ISO_8859_8_I, ISO_8859_10,
    ISO_8859_13, ISO_8859_14, ISO_8859_15, ISO_8859_16, KOI8_R, KOI8_U, MACINTOSH, SHIFT_JIS,
    UTF_8, WINDOWS_874, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254,
    WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258, X_MAC_CYRILLIC,
};

/// Encodings used for command completion.
static ENCODINGS: &[&::encoding_rs::Encoding] = &[
    BIG5,
    EUC_JP,
    EUC_KR,
    GB18030,
    GBK,
    IBM866,
    ISO_2022_JP,
    ISO_8859_2,
    ISO_8859_3,
    ISO_8859_4,
    ISO_8859_5,
    ISO_8859_6,
    ISO_8859_7,
    ISO_8859_8,
    ISO_8859_8_I,
    ISO_8859_10,
    ISO_8859_13,
    ISO_8859_14,
    ISO_8859_15,
    ISO_8859_16,
    KOI8_R,
    KOI8_U,
    MACINTOSH,
    SHIFT_JIS,
    UTF_8,
    WINDOWS_874,
    WINDOWS_1250,
    WINDOWS_1251,
    WINDOWS_1252,
    WINDOWS_1253,
    WINDOWS_1254,
    WINDOWS_1255,
    WINDOWS_1256,
    WINDOWS_1257,
    WINDOWS_1258,
    X_MAC_CYRILLIC,
];

/// Names of available encodings.
pub static ENCODING_NAMES: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| ENCODINGS.iter().map(|enc| enc.name()).collect());

/// Encoding wrapper usable as a value enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncodingArg {
    /// Detect encoding.
    #[default]
    Auto,
    /// Use a specific encoding.
    Set(&'static Encoding),
}

impl EncodingArg {
    /// Get current value as a string.
    fn as_str<'a>(self) -> &'a str {
        match self {
            EncodingArg::Auto => "auto",
            EncodingArg::Set(encoding) => encoding.name(),
        }
    }

    /// If an encoding is set, get it.
    pub const fn encoding(self) -> Option<&'static Encoding> {
        if let Self::Set(encoding) = self {
            Some(encoding)
        } else {
            None
        }
    }
}

/// Error returned when an [EncodingArg] cannot be parsed from a label/string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, ::thiserror::Error)]
#[error("could not parse encoding from label")]
pub struct EncodingArgFromStrError;

impl FromStr for EncodingArg {
    type Err = EncodingArgFromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("auto") {
            Ok(Self::Auto)
        } else {
            Encoding::for_label_no_replacement(s.as_bytes())
                .map(Self::Set)
                .ok_or(EncodingArgFromStrError)
        }
    }
}

impl Display for EncodingArg {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ValueEnum for EncodingArg {
    fn value_variants<'a>() -> &'a [Self] {
        static VARIANTS: OnceLock<Vec<EncodingArg>> = OnceLock::new();
        VARIANTS.get_or_init(|| {
            let mut variants = Vec::with_capacity(1 + ENCODINGS.len());
            variants.push(EncodingArg::Auto);
            for encoding in ENCODINGS {
                variants.push(EncodingArg::Set(encoding));
            }

            variants
        })
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.as_str()))
    }

    fn from_str(input: &str, _ignore_case: bool) -> Result<Self, String> {
        <Self as FromStr>::from_str(input).map_err(|_| format!("{input} is not a valid encoding"))
    }
}
