use std::fs::File;
use std::io::{self, Read, Write};

use crate::cli::args::StreamPath;
use crate::cli::error::CliError;
use crate::{MAX_CLOSING_ARTIFACT_BYTES, MAX_INPUT_BYTES};

pub(crate) fn read_input(source: &StreamPath) -> Result<String, CliError> {
    let bytes = match source {
        StreamPath::Stdio => read_bounded(io::stdin().lock(), "standard input", MAX_INPUT_BYTES)?,
        StreamPath::File(path) => {
            let file = File::open(path).map_err(|error| {
                CliError::InputIo(format!("cannot open input {}: {error}", path.display()))
            })?;
            read_bounded(file, &format!("input {}", path.display()), MAX_INPUT_BYTES)?
        }
    };
    String::from_utf8(bytes).map_err(|error| {
        CliError::Input(format!(
            "input is not UTF-8 (invalid byte at offset {})",
            error.utf8_error().valid_up_to()
        ))
    })
}

pub(crate) fn read_artifact(source: &StreamPath) -> Result<Vec<u8>, CliError> {
    match source {
        StreamPath::Stdio => read_bounded(
            io::stdin().lock(),
            "artifact on standard input",
            MAX_CLOSING_ARTIFACT_BYTES,
        ),
        StreamPath::File(path) => {
            let file = File::open(path).map_err(|error| {
                CliError::InputIo(format!("cannot open artifact {}: {error}", path.display()))
            })?;
            read_bounded(
                file,
                &format!("artifact {}", path.display()),
                MAX_CLOSING_ARTIFACT_BYTES,
            )
        }
    }
}

pub(super) fn read_bounded(
    mut reader: impl Read,
    label: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| CliError::InputIo(format!("cannot read {label}: {error}")))?;
        if read == 0 {
            break;
        }
        let requested = bytes.len().checked_add(read).ok_or_else(|| {
            CliError::Input(format!(
                "{label} length overflowed the {max_bytes}-byte CLI limit"
            ))
        })?;
        if requested > max_bytes {
            return Err(CliError::Input(format!(
                "{label} exceeds the {max_bytes}-byte CLI limit"
            )));
        }
        bytes.try_reserve(read).map_err(|_| {
            CliError::InputIo(format!(
                "cannot reserve {requested} bytes while reading {label}"
            ))
        })?;
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

pub(crate) fn write_output(
    destination: &StreamPath,
    contents: &[u8],
    force: bool,
) -> Result<(), CliError> {
    match destination {
        StreamPath::Stdio => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(contents)
                .and_then(|()| stdout.flush())
                .map_err(|error| {
                    CliError::OutputIo(format!("cannot write standard output: {error}"))
                })
        }
        StreamPath::File(path) => {
            crate::application::atomic_file::write_file_atomically(path, contents, force)
                .map_err(CliError::OutputIo)
        }
    }
}

/// Validate deterministic output policy before expensive semantic work or any
/// write. Atomic installation still repeats these checks to close races.
pub(crate) fn preflight_output_destination(
    destination: &StreamPath,
    force: bool,
) -> Result<(), CliError> {
    let StreamPath::File(path) = destination else {
        return Ok(());
    };
    if path.file_name().is_none() {
        return Err(CliError::OutputIo(format!(
            "output path {} has no file name",
            path.display()
        )));
    }
    if !force && path.exists() {
        return Err(CliError::OutputIo(format!(
            "output {} already exists; use --force to replace it",
            path.display()
        )));
    }
    Ok(())
}
