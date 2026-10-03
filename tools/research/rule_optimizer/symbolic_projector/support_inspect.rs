//! Bounded seed-offset nomination through the separate native read-only API.
use super::*;
pub const SCHEMA: &str = "rustred.symbolic-source-support-inspection.v2";

fn options(r: &Value) -> Result<rustred_app::CandidateSourceSupportOptions> {
    let mask = r["owner_mask"].as_str().ok_or("owner_mask required")?;
    require(
        !mask.is_empty() && mask.bytes().all(|b| b == b'0' || b == b'1'),
        "invalid owner mask",
    )?;
    let exact = ExactAlgebraLimits {
        max_exponent: checked(u16::try_from(limit(r, "max_exponent")?))?,
        max_polynomial_terms: limit(r, "max_polynomial_terms")?,
        max_term_operations: limit(r, "max_term_operations")?,
    };
    let mut source_generation = rustred::identity::ParametricIbpConfig::default();
    source_generation.context_limits.max_index_variables = mask.len();
    source_generation.relation_limits.arithmetic = IndexedAlgebraLimits {
        exact_algebra: exact,
        max_specialization_power_operations: limit(r, "max_term_operations")?,
        max_specialization_integer_bits: limit(r, "max_term_operations")?,
    };
    source_generation
        .relation_limits
        .identity_conditions
        .max_sources = limit(r, "max_generated_conditions")?;
    Ok(rustred_app::CandidateSourceSupportOptions {
        sector: mask.bytes().map(|b| b == b'1').collect(),
        rule_ordinal: number(r, "rule_ordinal")?,
        source_recenter_nomination: r
            .get("source_recenter_nomination")
            .map(|value| checked(serde_json::from_value(value.clone())))
            .transpose()?,
        max_retained_seeds: limit(r, "max_retained_seeds")?,
        max_unique_offsets: limit(r, "max_unique_offsets")?,
        max_original_rows: limit(r, "max_original_rows")?,
        max_nominated_pairs: limit(r, "max_nominated_pairs")?,
        max_generated_terms: limit(r, "max_generated_terms")?,
        max_generated_conditions: limit(r, "max_generated_conditions")?,
        max_output_bytes: limit(r, "max_inspection_bytes")?,
        source_generation,
    })
}

pub fn validate(r: &Value) -> Result<()> {
    require(
        r["schema"] == SCHEMA,
        "unknown support-inspection request schema",
    )?;
    for key in ["family_fingerprint", "expected_order"] {
        require(
            r[key].as_str().is_some_and(|s| !s.is_empty()),
            "family/order binding required",
        )?;
    }
    options(r)?;
    limit(r, "max_owner_bytes")?;
    limit(r, "max_report_bytes")?;
    owner_limits(r)?;
    Ok(())
}

pub fn run(bytes: &[u8], r: &Value) -> Result<Value> {
    validate(r)?;
    let view = checked(rustred_app::inspect_generated_candidate_source_support(
        bytes,
        owner_limits(r)?.bundle,
        options(r)?,
    ))?;
    require(
        view.family_fingerprint == r["family_fingerprint"].as_str().unwrap()
            && view.integral_order == r["expected_order"].as_str().unwrap(),
        "support-inspection family/order differs",
    )?;
    Ok(
        json!({"schema":SCHEMA,"status":"SAVED_SEED_SUPPORT_NOMINATED","request":r,"support":view,
        "source_replay_claim":false,"dispatch_claim":false,"closure_claim":false,"production_modified":false}),
    )
}
