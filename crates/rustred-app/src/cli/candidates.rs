//! Separate uncertified formula output and independent artifact certification.

use std::io::IsTerminal;
use std::sync::Mutex;

use super::args::{CertifyCandidatesArgs, FamilyCandidatesArgs};
use super::error::CliError;
use super::io::{preflight_output_destination, read_artifact, read_input, write_output};
use super::progress::FamilyCloseProgressMonitor;
use crate::{CandidateCertificationRequest, FamilyCandidatesRequest};

pub(super) fn generate(arguments: FamilyCandidatesArgs) -> Result<(), CliError> {
    preflight_output_destination(&arguments.output, arguments.force)?;
    if let Some(report) = &arguments.report_output {
        preflight_output_destination(report, arguments.force)?;
    }
    let mut request = FamilyCandidatesRequest::new(read_input(&arguments.input)?);
    request.input_format = arguments.input_format;
    request.n_cores = arguments.n_cores;
    request.exact_backend = arguments.exact_backend;
    request.numerical_depth = arguments.numerical_depth;
    request.permutation = arguments.permutation;
    request.nonpositive_indices = arguments.nonpositive_indices;
    let terminal = std::io::stderr().is_terminal();
    let monitor = Mutex::new(FamilyCloseProgressMonitor::new(
        std::io::stderr(),
        terminal,
        arguments.progress,
        std::env::var_os("NO_COLOR").is_some(),
    ));
    let generated = if terminal || arguments.progress {
        crate::family_candidates_with_progress(request, |event| {
            if let Ok(mut monitor) = monitor.lock() {
                monitor.observe(event);
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
    result
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
    let result = crate::certify_candidates(request)?;
    write_output(&arguments.output, result.artifact(), arguments.force)?;
    if let Some(report) = &arguments.report_output {
        write_output(report, result.to_toml().as_bytes(), arguments.force)?;
    }
    Ok(())
}
