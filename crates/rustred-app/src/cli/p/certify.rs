//! `rustred p-certify` (bebe-cert): exact per-rule IBP certificates of a saved
//! Custom owner payload (a generated candidate bundle), WITHOUT regenerating it.
//!
//! ```text
//! rustred p-certify --payload <owner.rrbin> --out <owner.cert.jsonl>
//!                   [--dot-axes a,b,..] [--only i,j,..] [--summary <json>] [--force]
//! ```
//!
//! Rebuilds the generation's source system, zero census, permutation and
//! integral order from the payload (the preparation `p-generate` used) and, per
//! stored rule, re-instantiates its stored seeds, eliminates exactly with the
//! engine's `SectorSolver::rule_frame` and requires the stored rule back; the
//! weights are composed onto the original IBP rows and multiplied out exactly
//! (`application::rule_certificates`, schema `rustred.p-rule-certificate.v1`).
//! `--dot-axes` names the non-square (dot-product) slots: a weighted seed with
//! a positive power there is flagged. `--only` restricts to table indices.
//! The summary (counts) goes to stdout as JSON, and to `--summary` if given.
//! `p-generate --cert` runs the same certification on each owner it writes.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::application::{CandidateBundleLimits, RuleCertificateOptions, rule_certificates};
use crate::cli::args::{ArgError, StreamPath, next_utf8_value, next_value};
use crate::cli::error::CliError;
use crate::cli::io::preflight_output_destination;

#[derive(Debug, PartialEq)]
struct Arguments {
    payload: PathBuf,
    out: PathBuf,
    dot_axes: Vec<usize>,
    only: Vec<usize>,
    summary: Option<PathBuf>,
    force: bool,
}

fn list(option: &'static str, value: String) -> Result<Vec<usize>, ArgError> {
    value
        .split(',')
        .filter(|item| !item.is_empty())
        .map(|item| {
            item.parse::<usize>().map_err(|_| ArgError::InvalidValue {
                option,
                value: value.clone(),
                expected: "a comma-separated list of non-negative integers",
            })
        })
        .collect()
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut payload, mut out, mut summary) = (None, None, None);
    let (mut dot_axes, mut only, mut force) = (Vec::new(), Vec::new(), false);
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--payload" => "--payload",
            "--out" => "--out",
            "--dot-axes" => "--dot-axes",
            "--only" => "--only",
            "--summary" => "--summary",
            "--force" => "--force",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--payload" => payload = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--out" => out = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--summary" => summary = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--dot-axes" => dot_axes = list(option, next_utf8_value(&mut arguments, option)?)?,
            "--only" => only = list(option, next_utf8_value(&mut arguments, option)?)?,
            _ => force = true,
        }
    }
    Ok(Arguments {
        payload: payload.ok_or(ArgError::MissingRequiredOption("--payload"))?,
        out: out.ok_or(ArgError::MissingRequiredOption("--out"))?,
        dot_axes,
        only,
        summary,
        force,
    })
}

/// Certify `bytes` into `out` (shared with `p-generate --cert`); returns the
/// summary as JSON.
pub(super) fn certify_payload(
    bytes: &[u8],
    out: &std::path::Path,
    options: &RuleCertificateOptions,
) -> Result<serde_json::Value, CliError> {
    let file = std::fs::File::create(out)
        .map_err(|error| CliError::OutputIo(format!("cannot create {}: {error}", out.display())))?;
    let mut writer = std::io::BufWriter::new(file);
    let summary = rule_certificates(
        bytes,
        CandidateBundleLimits::default(),
        options,
        &mut writer,
    )
    .map_err(CliError::Application)?;
    serde_json::to_value(&summary).map_err(|error| CliError::OutputIo(error.to_string()))
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let arguments = parse(arguments).map_err(CliError::Usage)?;
    preflight_output_destination(&StreamPath::File(arguments.out.clone()), arguments.force)?;
    if let Some(path) = &arguments.summary {
        preflight_output_destination(&StreamPath::File(path.clone()), arguments.force)?;
    }
    let bytes = std::fs::read(&arguments.payload).map_err(|error| {
        CliError::Input(format!(
            "cannot read {}: {error}",
            arguments.payload.display()
        ))
    })?;
    let options = RuleCertificateOptions {
        dot_axes: arguments.dot_axes.clone(),
        only: arguments.only.clone(),
        provenance: serde_json::json!({
            "payload": arguments.payload.display().to_string(),
            "payload_bytes": bytes.len(),
            "command": "rustred p-certify",
        }),
    };
    let summary = certify_payload(&bytes, &arguments.out, &options)?;
    let text = format!("{summary}\n");
    if let Some(path) = &arguments.summary {
        std::fs::write(path, &text).map_err(|error| {
            CliError::OutputIo(format!("cannot write {}: {error}", path.display()))
        })?;
    }
    print!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(arguments: &[&str]) -> Result<Arguments, ArgError> {
        parse(arguments.iter().map(OsString::from).collect())
    }

    #[test]
    fn p_certify_parses_its_options() {
        let all = parsed(&[
            "--payload",
            "p",
            "--out",
            "o",
            "--dot-axes",
            "11,12,13",
            "--only",
            "0,5",
        ])
        .unwrap();
        assert_eq!(all.dot_axes, vec![11, 12, 13]);
        assert_eq!(all.only, vec![0, 5]);
        assert!(!all.force);
        assert!(matches!(
            parsed(&["--out", "o"]),
            Err(ArgError::MissingRequiredOption("--payload"))
        ));
        assert!(matches!(
            parsed(&["--payload", "p", "--out", "o", "--dot-axes", "x"]),
            Err(ArgError::InvalidValue { .. })
        ));
    }
}
