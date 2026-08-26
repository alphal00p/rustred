mod args;
mod backend;
mod error;
mod input;
mod io;
mod model;
mod output;

use std::ffi::OsString;
use std::io::Write;

use args::{Command, DeriveArgs, HELP, parse_args};
use backend::lower_project;
use error::CliError;
use input::prepare_input;
use io::{read_input, write_output};
use output::{build_output, serialize_output};

pub(crate) fn main_entry() -> i32 {
    match run(std::env::args_os()) {
        Ok(()) => 0,
        Err(error) => {
            let code = error.exit_code();
            let _ = writeln!(
                std::io::stderr().lock(),
                "rustred: {}: {error}",
                error.category()
            );
            if matches!(error, CliError::Usage(_)) {
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "rustred: usage: run `rustred --help` for the command contract"
                );
            }
            code
        }
    }
}

fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), CliError> {
    match parse_args(arguments)? {
        Command::Help => write_informational_output(HELP),
        Command::Version => {
            write_informational_output(concat!("RustRed ", env!("CARGO_PKG_VERSION"), "\n"))
        }
        Command::Derive(arguments) => derive(arguments),
    }
}

fn derive(arguments: DeriveArgs) -> Result<(), CliError> {
    // Every fallible stage completes before `write_output` sees one byte. In
    // particular, stdout is never left with a truncated TOML document.
    let source = read_input(&arguments.input)?;
    let prepared = prepare_input(&source, arguments.input_format)?;
    let lowered = lower_project(prepared)?;
    let output = build_output(
        lowered,
        arguments.input_format,
        arguments.relations,
        arguments.n_cores,
    )?;
    let serialized = serialize_output(&output)?;
    write_output(&arguments.output, serialized.as_bytes(), arguments.force)
}

fn write_informational_output(contents: &str) -> Result<(), CliError> {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(contents.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| CliError::OutputIo(format!("cannot write standard output: {error}")))
}
