//! Application used to print file contents using a given encoding.

use ::core::{fmt::Arguments, str::FromStr};
use ::std::{
    io::{Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use ::bytesize::ByteSize;
use ::clap::Parser;
use ::encoding_arg::EncodingArg;
use ::encoding_rs::Decoder;
use ::mimalloc::MiMalloc;

/// Global allocator is mimalloc
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Print stdin and/or file contents using given encoding, or autodetect it.
#[derive(Debug, Parser)]
struct Cli {
    /// Encoding to use for output.
    #[arg(long, short = 'o', value_enum, default_value_t)]
    encoding: EncodingArg,
    /// Size in bytes of buffer to use for encoding detection.
    #[arg(long, default_value = "512K", value_parser = parse_size)]
    buffer_size: u64,
    /// Files to concatenate to stdout, if empty or on '-' read stdin.
    file: Vec<PathBuf>,
}

/// Crate error type.
#[derive(Debug, ::thiserror::Error)]
enum Error {
    /// Forwarded io error.
    #[error(transparent)]
    IO(#[from] ::std::io::Error),
}

/// Print error message or panic.
fn print_err(args: Arguments) {
    ::std::io::stderr()
        .lock()
        .write_fmt(args)
        .unwrap_or_else(|err| panic!("could not write to stderr, {err}"))
}

/// Parse a [ByteSize].
fn parse_size(arg: &str) -> Result<u64, String> {
    ByteSize::from_str(arg)
        .map_err(|_| format!("could not parse {arg} as an amount of bytes"))
        .map(|size| size.as_u64())
}

/// Decode contents of from.
fn decode(decoder: &Decoder, from: &mut dyn Read, to: &mut dyn Write) -> Result<(), Error> {
    let mut out_buf = [0u8; 1024];
    let mut in_buf = [0u8; 1024];



    loop {}

    Ok(())
}

fn main() -> ExitCode {
    let Cli {
        encoding,
        file,
        buffer_size,
    } = Cli::parse();

    match encoding {
        EncodingArg::Auto => todo!(),
        EncodingArg::Set(encoding) => {
            let decoder = encoding.new_decoder_without_bom_handling();
            match file.as_slice() {
                [] => {}
                files => {}
            }
        }
    }

    ExitCode::SUCCESS
}
