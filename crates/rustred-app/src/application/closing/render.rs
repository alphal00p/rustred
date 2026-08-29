use serde::Serialize;

use crate::application::{MAX_OUTPUT_BYTES, error::AppError};

pub(super) fn serialize(output: &impl Serialize) -> Result<String, AppError> {
    let mut serialized = toml::to_string_pretty(output).map_err(|error| {
        AppError::serialization(format!(
            "cannot serialize deterministic closing-artifact TOML output: {error}"
        ))
    })?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    if serialized.len() > MAX_OUTPUT_BYTES {
        return Err(AppError::output_limit(format!(
            "closing-artifact TOML output needs {} bytes, exceeding the {MAX_OUTPUT_BYTES}-byte application limit",
            serialized.len()
        )));
    }
    Ok(serialized)
}
