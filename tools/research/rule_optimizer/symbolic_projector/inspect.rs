//! Bounded read-only saved-rule shape view via the existing public inspector.
use super::*;
pub const SCHEMA: &str = "rustred.symbolic-source-inspection.v1";
pub fn validate(r: &Value) -> Result<()> {
    require(r["schema"] == SCHEMA, "unknown inspection request schema")?;
    let mask = r["owner_mask"].as_str().ok_or("owner mask required")?;
    require(
        !mask.is_empty() && mask.len() <= 16 && mask.bytes().all(|b| b == b'0' || b == b'1'),
        "invalid owner mask",
    )?;
    for key in ["family_fingerprint", "expected_order"] {
        require(
            r[key].as_str().is_some_and(|s| !s.is_empty()),
            "family/order binding required",
        )?;
    }
    number(r, "rule_ordinal")?;
    number(r, "max_rhs_sample_terms")?;
    for key in [
        "max_owner_bytes",
        "max_report_bytes",
        "max_inspection_bytes",
    ] {
        limit(r, key)?;
    }
    owner_limits(r)?;
    Ok(())
}
pub fn run(bytes: &[u8], r: &Value) -> Result<Value> {
    validate(r)?;
    let sector = r["owner_mask"]
        .as_str()
        .unwrap()
        .bytes()
        .map(|b| b == b'1')
        .collect::<Vec<_>>();
    let ordinal = number(r, "rule_ordinal")?;
    let view = checked(rustred_app::inspect_generated_candidate_program(
        bytes,
        owner_limits(r)?.bundle,
        rustred_app::CandidateProgramInspectionOptions {
            sectors: Some(vec![sector.clone()]),
            rule_ordinals: Some(vec![ordinal]),
            include_rhs_coefficients: false,
            max_output_bytes: limit(r, "max_inspection_bytes")?,
        },
    ))?;
    require(
        view.family_fingerprint == r["family_fingerprint"].as_str().unwrap()
            && view.integral_order == r["expected_order"].as_str().unwrap(),
        "inspection family/order differs",
    )?;
    require(
        view.sectors.len() == 1
            && view.sectors[0].sector == sector
            && view.sectors[0].rules.len() == 1,
        "inspection selection differs",
    )?;
    let rule = &view.sectors[0].rules[0];
    require(rule.ordinal == ordinal, "inspection ordinal differs")?;
    let mut shapes = BTreeMap::<(usize, usize), usize>::new();
    for term in &rule.rhs {
        let symbolic = term
            .symbolic_shifts
            .iter()
            .filter(|v| v.is_some_and(|n| n != 0))
            .count();
        let fixed = term
            .fixed_replacements
            .iter()
            .filter(|v| v.is_some())
            .count();
        *shapes.entry((symbolic, fixed)).or_default() += 1;
    }
    let sample = rule
        .rhs
        .iter()
        .take(number(r, "max_rhs_sample_terms")?)
        .collect::<Vec<_>>();
    Ok(
        json!({"schema":SCHEMA,"status":"SAVED_RULE_SHAPE_INSPECTED","request":r,
        "family_fingerprint":view.family_fingerprint,"integral_order":view.integral_order,"root_sector":view.root_sector,
        "sector":sector,"rule_ordinal":rule.ordinal,"dispatch_policy":rule.dispatch_policy,"case":rule.case,"target":rule.target,
        "excluded_all_zero_conjunctions":rule.excluded_all_zero_conjunctions,"retained_source_count":rule.retained_source_count,
        "rhs_terms":rule.rhs.len(),"rhs_shape_counts":shapes.into_iter().map(|((symbolic,fixed),count)|json!({"nonzero_symbolic_shift_axes":symbolic,"fixed_replacement_axes":fixed,"terms":count})).collect::<Vec<_>>(),
        "rhs_sample":sample,"omitted_rhs_terms":rule.rhs.len()-sample.len(),"condition_coefficient_ids":view.coefficients.iter().map(|c|c.id).collect::<Vec<_>>(),
        "coefficient_text_omitted":true,"coefficients_are_not_algebra_inputs":true,"source_replay_claim":false,"dispatch_claim":false,
        "closure_claim":false,"production_modified":false}),
    )
}
