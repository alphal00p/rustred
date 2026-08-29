use std::collections::BTreeMap;

use rustred::sector::Mask;
use serde::Serialize;

use crate::application::MAX_OUTPUT_BYTES;
use crate::application::error::AppError;
use crate::application::model::MetadataValue;

const ROOT_RENDER_OVERHEAD: usize = 4_096;
const FAMILY_RENDER_OVERHEAD: usize = 2_048;
const JOB_RENDER_OVERHEAD: usize = 1_024;
const ATOM_RENDER_FACTOR: usize = 320;
const STRING_ESCAPE_FACTOR: usize = 6;

pub(super) fn render_mask(mask: &Mask) -> Result<String, AppError> {
    let mut rendered = String::new();
    rendered
        .try_reserve_exact(mask.arity())
        .map_err(|_| AppError::output_limit("cannot reserve campaign sector output".to_owned()))?;
    for &active in mask.active_bits() {
        rendered.push(if active { '1' } else { '0' });
    }
    Ok(rendered)
}

pub(super) fn serialize_campaign_output(output: &impl Serialize) -> Result<String, AppError> {
    let mut serialized = toml::to_string_pretty(output).map_err(|error| {
        AppError::serialization(format!(
            "cannot serialize deterministic campaign TOML output: {error}"
        ))
    })?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    if serialized.len() > MAX_OUTPUT_BYTES {
        return Err(AppError::output_limit(format!(
            "campaign TOML output needs {} bytes, exceeding the {MAX_OUTPUT_BYTES}-byte application limit",
            serialized.len()
        )));
    }
    Ok(serialized)
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RenderBound {
    bytes: usize,
}

impl RenderBound {
    pub(super) const fn new() -> Self {
        Self { bytes: 0 }
    }

    pub(super) fn add_atom_payload(&mut self, packed_byte_size: usize) -> Result<(), AppError> {
        self.add(try_mul(
            "campaign canonical-expression render bound",
            packed_byte_size,
            ATOM_RENDER_FACTOR,
        )?)
    }

    pub(super) fn add_root_overhead(&mut self) -> Result<(), AppError> {
        self.add(ROOT_RENDER_OVERHEAD)
    }

    pub(super) fn add_family_overhead(&mut self) -> Result<(), AppError> {
        self.add(FAMILY_RENDER_OVERHEAD)
    }

    pub(super) fn add_job_overhead(&mut self) -> Result<(), AppError> {
        self.add(JOB_RENDER_OVERHEAD)
    }

    fn add(&mut self, bytes: usize) -> Result<(), AppError> {
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(|| {
            AppError::output_limit("campaign TOML render bound overflowed".to_owned())
        })?;
        if self.bytes > MAX_OUTPUT_BYTES {
            return Err(AppError::output_limit(format!(
                "campaign TOML has a conservative {}-byte render bound, exceeding the {MAX_OUTPUT_BYTES}-byte application limit",
                self.bytes
            )));
        }
        Ok(())
    }

    pub(super) fn add_string(&mut self, value: &str) -> Result<(), AppError> {
        let escaped = try_mul(
            "campaign TOML string render bound",
            value.len(),
            STRING_ESCAPE_FACTOR,
        )?;
        self.add(escaped.checked_add(64).ok_or_else(|| {
            AppError::output_limit("campaign TOML string bound overflowed".to_owned())
        })?)
    }

    pub(super) fn add_metadata(
        &mut self,
        metadata: &BTreeMap<String, MetadataValue>,
    ) -> Result<(), AppError> {
        for (key, value) in metadata {
            self.add_string(key)?;
            match value {
                MetadataValue::String(value) => self.add_string(value)?,
                MetadataValue::StringArray(values) => {
                    for value in values {
                        self.add_string(value)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Result<(), AppError> {
        if self.bytes <= MAX_OUTPUT_BYTES {
            Ok(())
        } else {
            Err(AppError::output_limit(format!(
                "campaign TOML render bound {} exceeds the {MAX_OUTPUT_BYTES}-byte application limit",
                self.bytes
            )))
        }
    }
}

fn try_mul(resource: &'static str, left: usize, right: usize) -> Result<usize, AppError> {
    left.checked_mul(right)
        .ok_or_else(|| AppError::output_limit(format!("{resource} overflowed")))
}
