//! `rustred sector-box-reduce --spec SPEC_JSON [--output FILE] [--force]`.
//!
//! Exit codes: 0 done (`ok` or `modular-only`); 1 the engine's verdict is
//! negative (`master-check-failed` under the strict master check, or
//! `certificate-failed`; the report is still written); 4 invalid request;
//! 8 engine or execution failure; 3 and 7 input and output I/O.
use std::ffi::OsString;

use super::args::StreamPath;
use super::error::CliError;
use super::io::{preflight_output_destination, read_input, write_output};

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let (mut spec, mut output, mut force) = (None, None, false);
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        let option = option
            .into_string()
            .map_err(|_| CliError::Input("non-UTF-8 option".into()))?;
        let mut value = || {
            arguments
                .next()
                .ok_or_else(|| CliError::Input(format!("option {option} needs a value")))
        };
        match option.as_str() {
            "--spec" if spec.is_none() => spec = Some(path(value()?)?),
            "--output" if output.is_none() => output = Some(path(value()?)?),
            "--force" if !force => force = true,
            other => {
                return Err(CliError::Input(format!(
                    "unexpected sector-box-reduce option {other:?}"
                )));
            }
        }
    }
    let spec = spec.ok_or_else(|| CliError::Input("sector-box-reduce needs --spec".into()))?;
    let output = output.unwrap_or(StreamPath::Stdio);
    preflight_output_destination(&output, force)?;
    let request: crate::SectorBoxReduceSpec =
        serde_json::from_str(&read_input(&spec)?).map_err(|e| CliError::Input(e.to_string()))?;
    let outcome = crate::sector_box_reduce(&request)?;
    let mut text = serde_json::to_string_pretty(&outcome.report)
        .map_err(|e| CliError::Input(e.to_string()))?;
    text.push('\n');
    write_output(&output, text.as_bytes(), force)?;
    match outcome.status {
        "ok" | "modular-only" => Ok(()),
        verdict => Err(CliError::Verdict {
            incomplete: false,
            message: format!("sector-box-reduce: {verdict}"),
        }),
    }
}

fn path(value: OsString) -> Result<StreamPath, CliError> {
    if value == "-" {
        Ok(StreamPath::Stdio)
    } else if value.is_empty() {
        Err(CliError::Input("empty path".into()))
    } else {
        Ok(StreamPath::File(value.into()))
    }
}
