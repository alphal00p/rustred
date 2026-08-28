use rustred::input::LoweringLimits;

use super::error::AppError;
use super::input::PreparedProject;
use super::model::LoweredProject;

/// The one application-to-core lowering seam. Every raw, hybrid, and explicit
/// input reaches this function with the same syntax-authenticated DTO.
pub(crate) fn lower_project(prepared: PreparedProject) -> Result<LoweredProject, AppError> {
    let PreparedProject {
        input_form,
        input_schema,
        metadata,
        normalized,
    } = prepared;
    let lowered = normalized
        .into_lowered(LoweringLimits::default())
        .map_err(|error| {
            AppError::lowering(format!(
                "cannot lower normalized Symbolica input to an affine integral family: {error}"
            ))
        })?;
    Ok(LoweredProject::new(
        input_form,
        input_schema,
        metadata,
        lowered,
    ))
}
