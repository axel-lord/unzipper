//! Implementation of cli tool.

use ::clap::Parser;
use ::log::LevelFilter;

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

    Ok(())
}
