//! Implementation of cli tool.

use ::core::num::NonZero;
use ::std::{
    io::{self, Write},
    path::{Path, PathBuf},
    thread::available_parallelism,
};

use ::clap::{ArgGroup, CommandFactory, Parser};
use ::clap_complete::Shell;
use ::color_eyre::eyre::eyre;
use ::log::LevelFilter;
use ::mimalloc::MiMalloc;
use ::rayon::{
    ThreadPoolBuilder,
    iter::{IntoParallelRefIterator, ParallelIterator},
};
use ::unzipper_lib::{Progress, UnzipError, Unzipper};

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
#[command(author, version, long_about = None, group = ArgGroup::new("action"))]
struct Cli {
    /// Enable verbose logging.
    #[arg(long, short)]
    verbose: bool,

    /// List archive contents.
    #[arg(long, short, visible_alias = "ls", requires = "archive")]
    list: bool,

    /// Encoding to use for file names in zip.
    #[arg(
        long,
        short = 'e',
        hide_possible_values = true,
        default_value = "auto",
        requires = "archive"
    )]
    encoding: Encoding,

    /// Where to unpack contents, this has the same behaviour as running
    /// without a destination while at the given location.
    #[arg(long, requires = "archive")]
    at: Option<PathBuf>,

    /// Maximum amount of threads to use when extracting multiple files.
    #[arg(long, short, default_value_t = thread_count())]
    threads: NonZero<usize>,

    /// Detect encoding of given archive.
    #[arg(long, visible_alias = "de", group = "action")]
    detect_encoding: Option<PathBuf>,

    /// List available encodings.
    #[arg(long, group = "action")]
    list_encodings: bool,

    /// Generate completions.
    #[arg(long, group = "action")]
    completions: bool,

    /// Shell to use if generating completions.
    #[arg(long, requires = "completions", value_enum, default_value_t = default_shell())]
    shell: Shell,

    /// Print progress for indicators.
    #[arg(long, requires = "archive", conflicts_with = "list")]
    print_progress: bool,

    /// When listing contents terminate with a null character instead of newline.
    #[arg(
        long,
        short = '0',
        visible_short_alias = 'z',
        visible_alias = "print0",
        visible_alias = "null",
        requires = "list"
    )]
    null_terminate: bool,

    /// Chunk size in mib to use when copying file content from archive to filesystem.
    /// Best used with --print-progress to split up large file extractions.
    #[arg(long)]
    chunk_size: Option<u64>,

    /// Archive/s to unpack.
    #[arg(group = "action")]
    archive: Vec<PathBuf>,
}

fn main() -> ::color_eyre::Result<()> {
    let Cli {
        verbose,
        encoding: Encoding(encoding),
        list_encodings,
        completions,
        detect_encoding,
        shell,
        null_terminate,
        at,
        list,
        archive,
        threads,
        print_progress,
        chunk_size,
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
    } else if let Some(src) = detect_encoding {
        let encoding = Unzipper::new().detect_encoding(&src).or_else(|err| {
            if let UnzipError::NoEncoding = err {
                Ok(::encoding_rs::UTF_8)
            } else {
                Err(err)
            }
        })?;
        writeln!(io::stdout().lock(), "{}", encoding.name())?;
    } else {
        let unzipper = Unzipper::new()
            .encoding(encoding)
            .null_terminate(null_terminate);

        let progress;
        let unzipper = if print_progress {
            progress = Progress::new();
            unzipper.print_progress(&progress)
        } else {
            unzipper
        };
        let unzipper = if let Some(chunk_size) = chunk_size {
            unzipper.chunk_size_mib(chunk_size)
        } else {
            unzipper
        };

        if list {
            for archive in archive {
                let mut stdout = io::stdout().lock();
                if let Err(err) = unzipper.list(&archive, &mut stdout) {
                    ::log::error!("error listing {archive:?}\n{err}");
                }
            }
        } else {
            let at = at
                .map_or_else(::std::env::current_dir, Ok)
                .map_err(|err| eyre!("could not get current directory").wrap_err(err))?;
            let unzip = |src: &Path| {
                let Some(name) = src.file_stem() else {
                    ::log::error!("could not get filename of {src:?}");
                    return;
                };
                let dest = at.join(name);
                ::log::info!("extracting to {dest:?}");

                if let Err(err) = ::std::fs::create_dir(&dest) {
                    ::log::error!("could not create directory {dest:?} for archive {src:?}\n{err}");
                    return;
                }

                let result = unzipper.unzip(src, &dest);
                if let Err(err) = result {
                    ::log::error!("could not unzip {archive:?} to {dest:?}\n{err}");
                }
            };

            match archive.as_slice() {
                [] => {
                    ::log::warn!("no archives to unzip");
                }
                [archive] => {
                    unzip(archive);
                }
                archives => {
                    ThreadPoolBuilder::new()
                        .thread_name(|idx| format!("unzipper-worker-{idx}"))
                        .num_threads(threads.get().min(archive.len()))
                        .build_global()?;

                    archives.par_iter().for_each(|path| unzip(path));
                }
            }
        }
    }

    Ok(())
}
