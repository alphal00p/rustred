//! Separate uncertified formula output and independent artifact certification.

use std::io::IsTerminal;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use super::args::{CertifyCandidatesArgs, FamilyCandidatesArgs, StreamPath};
use super::error::CliError;
use super::io::{preflight_output_destination, read_artifact, read_input, write_output};
use super::progress::{FamilyCloseProgressMonitor, FamilyGenerationTelemetry};
use crate::{CandidateCertificationRequest, FamilyCandidatesRequest};

pub(super) fn generate(arguments: FamilyCandidatesArgs) -> Result<(), CliError> {
    preflight_checkpoint_paths(&arguments)?;
    preflight_progress_path(&arguments)?;
    preflight_output_destination(&arguments.output, arguments.force)?;
    if let Some(report) = &arguments.report_output {
        preflight_output_destination(report, arguments.force)?;
    }
    let mut request = FamilyCandidatesRequest::new(read_input(&arguments.input)?);
    request.input_format = arguments.input_format;
    request.n_cores = arguments.n_cores;
    request.exact_backend = arguments.exact_backend;
    request.numerical_depth = arguments.numerical_depth;
    request.max_numerator_rank = arguments.max_numerator_rank;
    request.finite_case_policy = arguments.finite_case_policy;
    request.finite_case_limits = arguments.finite_case_limits;
    request.case_intersection_limits = arguments.case_intersection_limits;
    request.bundle_limits = arguments.bundle_limits;
    request.permutation = arguments.permutation;
    request.integral_order = arguments
        .integral_order
        .map(|path| {
            let text = read_input(&StreamPath::File(path))?;
            crate::CandidateIntegralOrder::from_json(&text).map_err(CliError::from)
        })
        .transpose()?;
    request.discovery_strategy = arguments
        .discovery_strategy
        .map(|path| {
            let text = read_input(&StreamPath::File(path))?;
            crate::CandidateDiscoveryStrategy::from_json(&text).map_err(CliError::from)
        })
        .transpose()?;
    request.nonpositive_indices = arguments.nonpositive_indices;
    request.selected_sectors = arguments.selected_sectors;
    request.checkpoint = arguments.checkpoint;
    let terminal = std::io::stderr().is_terminal();
    let export = arguments.progress_json.is_some();
    let telemetry = Mutex::new(FamilyGenerationTelemetry::new(arguments.progress_json));
    let monitor = Mutex::new(FamilyCloseProgressMonitor::new(
        std::io::stderr(),
        terminal,
        arguments.progress,
        std::env::var_os("NO_COLOR").is_some(),
    ));
    let generated = if terminal || arguments.progress || export {
        crate::family_candidates_with_progress(request, |event| {
            if let Ok(mut monitor) = monitor.lock() {
                monitor.observe(event.clone());
            }
            if let Ok(mut telemetry) = telemetry.lock() {
                telemetry.observe(event);
            }
        })
    } else {
        crate::family_candidates(request)
    };
    let result = generated.map_err(CliError::from).and_then(|result| {
        write_output(&arguments.output, result.bundle(), arguments.force)?;
        if let Some(report) = &arguments.report_output {
            write_output(report, result.to_toml().as_bytes(), arguments.force)?;
        }
        Ok(())
    });
    if let Ok(mut monitor) = monitor.lock() {
        monitor.finish(result.is_ok());
    }
    if let Ok(mut telemetry) = telemetry.lock() {
        telemetry.finish(result.is_ok());
    }
    result
}

/// Export owns only this explicitly selected latest-snapshot path. Refuse local
/// aliases before work starts; no telemetry write may replace an input/output or
/// enter the checkpoint directory managed by the native generator.
fn preflight_progress_path(arguments: &FamilyCandidatesArgs) -> Result<(), CliError> {
    let Some(path) = &arguments.progress_json else {
        return Ok(());
    };
    preflight_output_destination(&StreamPath::File(path.clone()), true)?;
    if path.is_dir() {
        return Err(CliError::Input("--progress-json must name a file".into()));
    }
    let progress = resolved_location(path)?;
    for stream in [&arguments.input, &arguments.output]
        .into_iter()
        .chain(arguments.report_output.iter())
    {
        if let StreamPath::File(path) = stream {
            if resolved_location(path)?.starts_with(&progress) {
                return Err(CliError::Input(
                    "--progress-json must not alias or contain input, bundle or report paths"
                        .into(),
                ));
            }
        }
    }
    for path in arguments
        .discovery_strategy
        .iter()
        .chain(arguments.integral_order.iter())
    {
        if resolved_location(path)?.starts_with(&progress) {
            return Err(CliError::Input(
                "--progress-json must not alias or contain strategy/order inputs".into(),
            ));
        }
    }
    if let Some(checkpoint) = &arguments.checkpoint {
        let directory = resolved_location(&checkpoint.directory)?;
        if progress.starts_with(&directory) || directory.starts_with(&progress) {
            return Err(CliError::Input(
                "--progress-json must be outside, and not contain, the dedicated checkpoint directory".into(),
            ));
        }
    }
    Ok(())
}

/// A checkpoint directory is dedicated to managed files. Reject collisions
/// before parsing input or starting work, including existing symlink aliases.
/// This is ordinary local-path policy, not hostile-filesystem authentication.
fn preflight_checkpoint_paths(arguments: &FamilyCandidatesArgs) -> Result<(), CliError> {
    for strategy in arguments
        .discovery_strategy
        .iter()
        .chain(arguments.integral_order.iter())
    {
        let strategy = resolved_location(strategy)?;
        for stream in std::iter::once(&arguments.output).chain(arguments.report_output.iter()) {
            if let StreamPath::File(path) = stream {
                if resolved_location(path)? == strategy {
                    return Err(CliError::Input(
                        "strategy/order input must differ from bundle/report destinations".into(),
                    ));
                }
            }
        }
    }
    let Some(checkpoint) = &arguments.checkpoint else {
        return Ok(());
    };
    let directory = resolved_location(&checkpoint.directory)?;
    for path in arguments
        .discovery_strategy
        .iter()
        .chain(arguments.integral_order.iter())
    {
        if resolved_location(path)?.starts_with(&directory) {
            return Err(CliError::Input(
                "strategy/order input must be outside the dedicated checkpoint directory".into(),
            ));
        }
    }
    for stream in [&arguments.input, &arguments.output]
        .into_iter()
        .chain(arguments.report_output.iter())
    {
        if let StreamPath::File(path) = stream {
            if resolved_location(path)?.starts_with(&directory) {
                return Err(CliError::Input("input, final bundle and report paths must be outside the dedicated checkpoint directory".into()));
            }
        }
    }
    Ok(())
}

/// Resolve the existing ancestor first, then normalize the missing suffix.
/// Unlike lexical normalization before canonicalization this respects a
/// symlink followed by `..`. It also works before a new checkpoint exists.
pub(super) fn resolved_location(path: &Path) -> Result<PathBuf, CliError> {
    let mut ancestor = std::path::absolute(path).map_err(|error| {
        CliError::InputIo(format!("cannot resolve {}: {error}", path.display()))
    })?;
    let mut suffix = Vec::new();
    loop {
        match std::fs::canonicalize(&ancestor) {
            Ok(mut resolved) => {
                for part in suffix.into_iter().rev() {
                    if part == std::ffi::OsStr::new("..") {
                        resolved.pop();
                    } else {
                        resolved.push(part);
                    }
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let part = match ancestor.components().next_back() {
                    Some(Component::Normal(name)) => name.to_owned(),
                    Some(Component::ParentDir) => "..".into(),
                    Some(Component::CurDir) => {
                        ancestor.pop();
                        continue;
                    }
                    _ => {
                        return Err(CliError::InputIo(format!(
                            "cannot resolve {}: {error}",
                            path.display()
                        )));
                    }
                };
                suffix.push(part);
                ancestor.pop();
            }
            Err(error) => {
                return Err(CliError::InputIo(format!(
                    "cannot resolve {}: {error}",
                    path.display()
                )));
            }
        }
    }
}

pub(super) fn certify(arguments: CertifyCandidatesArgs) -> Result<(), CliError> {
    preflight_output_destination(&arguments.output, arguments.force)?;
    if let Some(report) = &arguments.report_output {
        preflight_output_destination(report, arguments.force)?;
    }
    // This helper is bounded binary I/O only; it does not interpret the
    // candidate payload as a closed artifact or bypass its separate decoder.
    let mut request = CandidateCertificationRequest::new(read_artifact(&arguments.input)?);
    request.publication_limits = arguments.resources.publication_limits();
    request.max_negative_index_degree = arguments.max_negative_index_degree;
    request.max_total_excess_degree = arguments.max_total_excess_degree;
    let result = crate::certify_candidates(request)?;
    write_output(&arguments.output, result.artifact(), arguments.force)?;
    if let Some(report) = &arguments.report_output {
        write_output(report, result.to_toml().as_bytes(), arguments.force)?;
    }
    Ok(())
}

#[cfg(test)]
mod telemetry_path_tests;
