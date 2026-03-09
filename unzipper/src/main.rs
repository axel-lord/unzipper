//! Implementation of cli tool.

use ::std::io::{self, Write};

use ::clap::Parser;
use ::log::LevelFilter;
use ::mimalloc::MiMalloc;

use crate::encoding::ENCODING_NAMES;

/// Global allocator is mimalloc
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod encoding {
    //! Encoding cli help.

    use ::std::{collections::BTreeSet, sync::LazyLock};

    use ::encoding_rs::{
        BIG5, EUC_JP, EUC_KR, GB18030, GBK, IBM866, ISO_2022_JP, ISO_8859_2, ISO_8859_3,
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
}

/// Unzip files, with encoding detection.
#[derive(Debug, Clone, Parser)]
#[command(author, version, long_about = None)]
struct Cli {
    /// Enable verbose logging.
    #[arg(long, short)]
    verbose: bool,
}

fn main() -> ::color_eyre::Result<()> {
    let Cli { verbose } = Cli::parse();
    ::color_eyre::install()?;
    ::env_logger::builder()
        .filter_module(
            "unzipper_lib",
            if verbose {
                LevelFilter::Info
            } else {
                LevelFilter::Warn
            },
        )
        .init();

    let mut stdout = io::stdout().lock();
    for i in ENCODING_NAMES.iter() {
        stdout
            .write_all(i.as_bytes())
            .and_then(|_| stdout.write_all(b"\n"))
            .expect("write to stdout should succeed");
    }

    Ok(())
}
