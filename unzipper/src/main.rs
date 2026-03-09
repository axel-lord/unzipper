//! Implementation of cli tool.

use ::core::num::NonZero;
use ::std::{
    fmt::format,
    io::{self, Write},
    path::{Path, PathBuf},
    thread::available_parallelism,
};

use ::clap::{CommandFactory, Parser};
use ::clap_complete::{Generator, Shell};
use ::log::LevelFilter;
use ::mimalloc::MiMalloc;
use ::rayon::ThreadPoolBuilder;
use ::unzipper_lib::Unzipper;

use crate::encoding::{ENCODING_NAMES, Encoding};

/// Global allocator is mimalloc
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod encoding;

/// Get default shell
fn default_shell() -> Shell {
    Shell::from_env().unwrap_or(Shell::Bash)
}

/// Get name of binary.
fn binary_name() -> String {
    ::std::env::current_exe()
        .ok()
        .and_then(|exe| exe.to_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| env!("CARGO_BIN_NAME").to_owned())
}

/// Get default thread count.
fn thread_count() -> NonZero<usize> {
    available_parallelism().unwrap_or(const { NonZero::new(1).unwrap() })
}

/// Unzip files, with encoding detection.
#[derive(Debug, Clone, Parser)]
#[command(author, version, long_about = None)]
struct Cli {
    /// Enable verbose logging.
    #[arg(long, short)]
    verbose: bool,

    /// Encoding of file names.
    #[arg(long, short = 'O', hide_possible_values = true, default_value = "auto")]
    encoding: Encoding,

    /// Where to unpack contents, unlike with '-d' this has the same behaviour as running
    /// without a destination while at the given location.
    #[arg(long, requires = "archive")]
    at: Option<PathBuf>,

    /// How many threads should be used.
    #[arg(long, short, default_value_t = thread_count())]
    threads: NonZero<usize>,

    /// Directory to unpack contents into, will be created if missing.
    #[arg(long, short = 'd', conflicts_with = "at", requires = "archive")]
    exdir: Option<PathBuf>,

    /// List available encodings.
    #[arg(long, exclusive = true)]
    list_encodings: bool,

    /// Generate completions.
    #[arg(long, conflicts_with_all = ["at", "exdir", "archive"])]
    completions: bool,

    /// Shell to use if generating completions.
    #[arg(long, requires = "completions", value_enum, default_value_t = default_shell())]
    shell: Shell,

    /// Archive/s to unpack.
    #[arg(required = false)]
    archive: Vec<PathBuf>,
}

fn main() -> ::color_eyre::Result<()> {
    let Cli {
        verbose,
        encoding: Encoding(encoding),
        list_encodings,
        completions,
        shell,
        at,
        exdir,
        archive,
        threads,
    } = Cli::parse();
    ::color_eyre::install()?;
    let level_filter = if verbose {
        LevelFilter::Info
    } else {
        LevelFilter::Warn
    };
    ::env_logger::builder()
        .filter_module("unzipper", level_filter)
        .filter_module("unzipper_lib", level_filter)
        .init();

    if list_encodings {
        let mut stdout = io::stdout().lock();
        for i in ENCODING_NAMES.iter() {
            stdout
                .write_all(i.as_bytes())
                .and_then(|_| stdout.write_all(b"\n"))
                .expect("write to stdout should succeed");
        }
    } else if completions {
        let mut stdout = io::stdout().lock();
        ::clap_complete::generate(shell, &mut Cli::command(), binary_name(), &mut stdout);
        stdout.flush().expect("flush of stdout should succeed");
    } else {
        ThreadPoolBuilder::new()
            .thread_name(|idx| format!("unzipper-worker-{idx}"))
            .num_threads(threads.get())
            .build_global()?;
        let unzipper = Unzipper::builder()
            .encoding(encoding)
            .threads(threads)
            .unfold(false)
            .build();

        for archive in archive {
            if let Err(err) = unzipper.unzip(&archive, Path::new("")) {
                ::log::error!("could not unzip {archive:?}\n{err}");
            };
        }
    }

    Ok(())
}
