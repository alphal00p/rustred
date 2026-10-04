//! Boundary-specific report adapter for the shared exact dual kernel.
//! Existing boundary diagnostic schema and admission semantics remain intact.
pub(super) use super::super::exact_dual::Certificate;
#[cfg(test)]
use super::super::exact_dual::verify;
use super::super::{exact_dual::solve, guards_json, project};
use project::{Error, Guard, Limits, Row};
use rustred::{algebra::IndexedCoefficientContext, identity::IndexShift};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn enabled(cfg: &Value) -> project::Result<bool> {
    match cfg.get("exact_dual_separator") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(Error::Invalid(
            "exact_dual_separator must be boolean".into(),
        )),
    }
}

pub(super) fn append_report(
    c: &IndexedCoefficientContext,
    cfg: &Value,
    corrections: &[Row],
    baseline: &Row,
    forbidden: &BTreeSet<IndexShift>,
    incoming: &[Guard],
    limits: Limits,
    report: &mut Value,
) -> Option<(Vec<IndexShift>, Certificate)> {
    if matches!(enabled(cfg), Ok(false)) {
        return None;
    }
    // Called ONLY after full projection NoTarget. Its status is never changed
    // by this optional diagnostic, including native/budget/replay refusal.
    let columns = forbidden.iter().cloned().collect::<Vec<_>>();
    let mut diagnostic = json!({
        "schema":"rustred.boundary-dual-separator.v1",
        "status":"DUAL_DIAGNOSTIC_REFUSED_OR_INCOMPLETE",
        "scope":"generic finite rational-function span over the authenticated indexed coefficient field",
        "pointwise_impossibility_claim":false,"family_impossibility_claim":false,
        "rule_certificate":false,"source_generation":false,"exported":false,
        "solver":"Symbolica public dense Matrix::solve_any; one vector, no full nullspace",
        "correction_rows":corrections.len(),"forbidden_columns":columns.len(),
        "columns":columns.iter().enumerate().map(|(j,s)| json!({
            "column":j,"shift":s.values(),
            "correction_rows":corrections.iter().enumerate().filter_map(|(i,row)|row.contains_key(s).then_some(i)).collect::<Vec<_>>(),
            "baseline_nonzero":baseline.contains_key(s)})).collect::<Vec<_>>(),
        "internal_pivot_guards_exposed":false,
        "assumption_policy":"Retain incoming conditions and input/witness/replay denominators, including canceled terms. Exact rational products certify the separator; internal solver pivots are not a rule/domain certificate. No claim on exceptional specializations.",
        "resource_policy":"Dense storage/input terms and conservative field operations precharged; native internal polynomial scratch remains under existing outer RSS/deadline guard; returned coefficients independently admitted and replayed."
    });
    let result =
        enabled(cfg).and_then(|_| solve(c, corrections, baseline, &columns, incoming, limits));
    let certificate = match result {
        Err(error) => {
            diagnostic["error"] = json!(error.to_string());
            None
        }
        Ok(certificate) => {
            diagnostic["status"] = json!("EXACT_FINITE_SPAN_SEPARATOR");
            diagnostic["independent_replay"] = json!({"correction_products":corrections.len(),
                "all_correction_products_zero":true,"baseline_product_one":true});
            diagnostic["dense_slot_charge"] = json!(certificate.dense_slots);
            diagnostic["field_operation_charge"] = json!(certificate.operation_charge);
            diagnostic["conditions"] = guards_json(&certificate.guards);
            diagnostic["lambda"] = json!(certificate.values.iter().enumerate().filter(|(_,v)|!v.is_zero())
                .map(|(j,v)|json!({"column":j,"shift":columns[j].values(),"coefficient":v.raw().to_string(),"display_only":true})).collect::<Vec<_>>());
            Some(certificate)
        }
    };
    report["exact_dual_separator"] = diagnostic;
    certificate.map(|certificate| (columns, certificate))
}

#[cfg(test)]
#[path = "boundary_dual_tests.rs"]
mod tests;
