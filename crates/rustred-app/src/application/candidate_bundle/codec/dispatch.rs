//! Versioned program-dispatch metadata. Equation/source records stay unchanged.
use super::super::model::*;
use crate::AppError;
use rustred::propagator::bundle_wire;
use rustred::solver::RuleDispatchPolicy;

#[cfg(test)]
mod tests;

// The exact pre-policy v2 wire layout lives with the records in rustred-core
// (`bundle_wire`); default writers keep these bytes.
#[cfg(test)]
pub(super) fn decode_v2(
    bytes: &[u8],
    allowance: usize,
) -> Result<(ProgramRecord, usize), AppError> {
    Ok(bundle_wire::decode_v2_program_record(bytes, allowance)?)
}

pub(in crate::application::candidate_bundle) fn encode(
    records: &ProgramRecord,
) -> Result<Vec<u8>, AppError> {
    bundle_wire::encode_program_record(records).map_err(|e| AppError::serialization(e.to_string()))
}

pub(super) fn validate(
    records: &ProgramRecord,
    limits: CandidateBundleLimits,
) -> Result<(), AppError> {
    if limits.diagnostics_enabled && records.rule_dispatch.len() > limits.max_collection_entries {
        return Err(AppError::limit(
            "candidate dispatch table exceeds collection budget",
        ));
    }
    Ok(bundle_wire::validate_dispatch(records)?)
}

pub(in crate::application::candidate_bundle) fn policy(
    records: &ProgramRecord,
    sector: usize,
    rule: usize,
) -> RuleDispatchPolicy {
    records.dispatch_policy(sector, rule)
}
