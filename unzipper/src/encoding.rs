//! Encoding cli help.

use ::core::str::FromStr;
use ::std::{
    collections::BTreeSet,
    sync::{LazyLock, OnceLock},
};

use ::clap::{ValueEnum, builder::PossibleValue};
use ::encoding_rs::{
    BIG5, EUC_JP, EUC_KR, GB18030, GBK, IBM866, ISO_2022_JP, ISO_8859_2, ISO_8859_3, ISO_8859_4,
    ISO_8859_5, ISO_8859_6, ISO_8859_7, ISO_8859_8, ISO_8859_8_I, ISO_8859_10, ISO_8859_13,
    ISO_8859_14, ISO_8859_15, ISO_8859_16, KOI8_R, KOI8_U, MACINTOSH, SHIFT_JIS, UTF_8,
    WINDOWS_874, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoding(pub ::unzipper_lib::Encoding);

impl ValueEnum for Encoding {
    fn value_variants<'a>() -> &'a [Self] {
        static VARIANTS: OnceLock<Vec<Encoding>> = OnceLock::new();
        VARIANTS.get_or_init(|| {
            let mut variants = Vec::with_capacity(1 + ENCODINGS.len());
            variants.push(Encoding(::unzipper_lib::Encoding::Auto));
            for encoding in ENCODINGS {
                variants.push(Encoding(::unzipper_lib::Encoding::Set(encoding)));
            }

            variants
        })
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        let Self(encoding) = self;
        Some(PossibleValue::new(encoding.as_str()))
    }

    fn from_str(input: &str, _ignore_case: bool) -> Result<Self, String> {
        ::unzipper_lib::Encoding::from_str(input)
            .map_err(|err| err.to_string())
            .map(Self)
    }
}
