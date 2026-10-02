//! Versioned program-dispatch metadata. Equation/source records stay unchanged.
use super::super::model::*;
use crate::AppError;
use rustred::solver::RuleDispatchPolicy;

#[cfg(test)]
mod tests;

// Exact pre-policy v2 wire layout; default writers keep these bytes.
#[derive(bincode::Decode)]
struct V2Record {
    schema: String,
    status: String,
    solver_policy: String,
    family_source: String,
    input_format: String,
    family_fingerprint: String,
    root_sector: Vec<bool>,
    permutation: Option<Vec<usize>>,
    integral_order: String,
    sectors: Vec<SectorRecord>,
}
#[derive(bincode::Encode)]
struct V2Ref<'a> {
    schema: &'a str,
    status: &'a str,
    solver_policy: &'a str,
    family_source: &'a str,
    input_format: &'a str,
    family_fingerprint: &'a str,
    root_sector: &'a [bool],
    permutation: &'a Option<Vec<usize>>,
    integral_order: &'a str,
    sectors: &'a [SectorRecord],
}

pub(super) fn decode_v2(bytes: &[u8]) -> Result<(ProgramRecord, usize), AppError> {
    let (old, consumed): (V2Record, usize) = bincode::decode_from_slice(
        bytes,
        bincode::config::standard().with_limit::<MAX_CANDIDATE_BUNDLE_BYTES>(),
    )
    .map_err(|e| AppError::schema(format!("invalid v2 candidate record: {e}")))?;
    Ok((
        ProgramRecord {
            schema: old.schema,
            status: old.status,
            solver_policy: old.solver_policy,
            family_source: old.family_source,
            input_format: old.input_format,
            family_fingerprint: old.family_fingerprint,
            root_sector: old.root_sector,
            permutation: old.permutation,
            integral_order: old.integral_order,
            sectors: old.sectors,
            rule_dispatch: Vec::new(),
        },
        consumed,
    ))
}

pub(in crate::application::candidate_bundle) fn encode(
    records: &ProgramRecord,
) -> Result<Vec<u8>, AppError> {
    if records.rule_dispatch.is_empty() {
        let wire = V2Ref {
            schema: CANDIDATE_BUNDLE_SCHEMA,
            status: &records.status,
            solver_policy: &records.solver_policy,
            family_source: &records.family_source,
            input_format: &records.input_format,
            family_fingerprint: &records.family_fingerprint,
            root_sector: &records.root_sector,
            permutation: &records.permutation,
            integral_order: &records.integral_order,
            sectors: &records.sectors,
        };
        bincode::encode_to_vec(wire, bincode::config::standard())
            .map_err(|e| AppError::serialization(e.to_string()))
    } else {
        bincode::encode_to_vec(records, bincode::config::standard())
            .map_err(|e| AppError::serialization(e.to_string()))
    }
}

pub(super) fn validate(
    records: &ProgramRecord,
    limits: CandidateBundleLimits,
) -> Result<(), AppError> {
    if records.rule_dispatch.len() > limits.max_collection_entries {
        return Err(AppError::limit(
            "candidate dispatch table exceeds collection budget",
        ));
    }
    if records.rule_dispatch.is_empty() == (records.schema == DISPATCH_CANDIDATE_BUNDLE_SCHEMA) {
        return Err(AppError::schema("candidate dispatch table/schema mismatch"));
    }
    let mut previous = None;
    for entry in &records.rule_dispatch {
        let key = (entry.sector, entry.rule);
        if entry.policy != 1
            || previous.is_some_and(|old| old >= key)
            || records
                .sectors
                .get(entry.sector)
                .is_none_or(|sector| entry.rule >= sector.rules.len())
        {
            return Err(AppError::input(
                "invalid, duplicate or unordered candidate dispatch entry",
            ));
        }
        previous = Some(key);
    }
    Ok(())
}

pub(in crate::application::candidate_bundle) fn policy(
    records: &ProgramRecord,
    sector: usize,
    rule: usize,
) -> RuleDispatchPolicy {
    match records
        .rule_dispatch
        .binary_search_by_key(&(sector, rule), |entry| (entry.sector, entry.rule))
    {
        Ok(_) => RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
        Err(_) => RuleDispatchPolicy::Partition,
    }
}
