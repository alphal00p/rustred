use rustred::SymbolicaProjectLoweringLimits;

use crate::cli::error::CliError;
use crate::cli::input::PreparedCliProject;
use crate::cli::model::LoweredCliProject;

/// The one CLI-to-library lowering seam. Every raw, hybrid, and explicit
/// frontend reaches this function with the same syntax-authenticated DTO.
pub(crate) fn lower_project(prepared: PreparedCliProject) -> Result<LoweredCliProject, CliError> {
    let PreparedCliProject {
        input_form,
        input_schema,
        metadata,
        normalized,
    } = prepared;
    let lowered = normalized
        .into_lowered(SymbolicaProjectLoweringLimits::default())
        .map_err(|error| {
            CliError::Input(format!(
                "cannot lower normalized Symbolica input to an affine integral family: {error}"
            ))
        })?;
    Ok(LoweredCliProject::new(
        input_form,
        input_schema,
        metadata,
        lowered,
    ))
}
