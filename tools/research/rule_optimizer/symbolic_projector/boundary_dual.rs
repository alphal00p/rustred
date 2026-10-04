//! Optional negative certificate for ONE completed finite-span miss.
//! Symbolica solves [C;b] lambda = [0;1]; independent indexed arithmetic
//! replays both products. No source search, custom elimination or rule proof.
use super::super::{guards_json, project};
use project::{Error, Guard, Limits, Row, add, bound, denominator, native, require};
use rustred::{
    algebra::{IndexedCoefficient, IndexedCoefficientContext},
    identity::IndexShift,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use symbolica::{domains::SelfRing, prelude::Z, tensors::matrix::Matrix};

pub(super) fn enabled(cfg: &Value) -> project::Result<bool> {
    match cfg.get("exact_dual_separator") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(Error::Invalid(
            "exact_dual_separator must be boolean".into(),
        )),
    }
}

pub(super) struct Certificate {
    pub(super) values: Vec<IndexedCoefficient>,
    pub(super) guards: Vec<Guard>,
    pub(super) dense_slots: usize,
    pub(super) operation_charge: usize,
}

fn mul(a: usize, b: usize) -> project::Result<usize> {
    a.checked_mul(b).ok_or(Error::Budget("dual count overflow"))
}

/// Replay the complete F products independently of the matrix solver. The
/// binding is the ordered native IndexShift list, never coefficient text.
fn verify(
    c: &IndexedCoefficientContext,
    corrections: &[Row],
    baseline: &Row,
    columns: &[IndexShift],
    values: &[IndexedCoefficient],
    guards: &mut Vec<Guard>,
    limits: Limits,
) -> project::Result<()> {
    require(columns.len() == values.len(), "dual witness shape differs")?;
    require(
        columns.windows(2).all(|w| w[0] < w[1]),
        "dual columns are not canonical",
    )?;
    for value in values {
        c.validate_with_limits(value, limits.arithmetic)?;
        denominator(c, guards, value, "dual witness denominator", limits)?;
    }
    let mut operations = 0;
    for (i, row) in corrections
        .iter()
        .chain(std::iter::once(baseline))
        .enumerate()
    {
        let mut sum = c.zero();
        for (shift, value) in row {
            // Validate even terms outside F, and terms killed by a zero weight.
            c.validate_with_limits(value, limits.arithmetic)?;
            denominator(c, guards, value, "dual replay input denominator", limits)?;
            if let Ok(j) = columns.binary_search(shift) {
                operations = add(operations, 2)?;
                bound(operations, limits.operations, "dual replay operations")?;
                let product = c.mul_with_limits(value, &values[j], limits.arithmetic)?;
                denominator(
                    c,
                    guards,
                    &product,
                    "dual replay product denominator",
                    limits,
                )?;
                sum = c.add_with_limits(&sum, &product, limits.arithmetic)?;
                denominator(c, guards, &sum, "dual replay sum denominator", limits)?;
            }
        }
        require(
            if i < corrections.len() {
                sum.is_zero()
            } else {
                sum == c.one()
            },
            "independent dual product replay failed",
        )?;
    }
    Ok(())
}

fn solve(
    c: &IndexedCoefficientContext,
    corrections: &[Row],
    baseline: &Row,
    columns: &[IndexShift],
    incoming: &[Guard],
    limits: Limits,
) -> project::Result<Certificate> {
    let m = add(corrections.len(), 1)?;
    let n = columns.len();
    require(n > 0, "empty forbidden matrix cannot have a separator")?;
    require(
        columns.windows(2).all(|w| w[0] < w[1]),
        "dual columns are not canonical",
    )?;
    bound(m, limits.rows, "dual matrix rows")?;
    bound(n, limits.columns, "dual matrix columns")?;
    // Admission charge covers input, augmented/reduced copies, RHS and result.
    // It is NOT a bound on Symbolica's internal polynomial growth: arithmetic
    // admission/postchecks and the existing outer RSS/deadline guard still apply.
    let cells = mul(m, n)?;
    let dense_slots = add(mul(3, mul(m, add(n, 1)?)?)?, mul(2, n)?)?;
    bound(dense_slots, limits.nonzeros, "dual dense coefficient slots")?;
    let pivots = m.min(n);
    let operation_charge = add(mul(mul(mul(2, m)?, pivots)?, add(n, 1)?)?, mul(2, cells)?)?;
    bound(
        operation_charge,
        limits.operations,
        "dual conservative field-operation charge",
    )?;
    let mut guards = Vec::new();
    for guard in incoming {
        project::retain(
            c,
            &mut guards,
            guard.polynomial.clone(),
            guard.origin.clone(),
            limits,
        )?;
    }
    let zero_terms = add(
        c.zero().raw().numerator.nterms(),
        c.zero().raw().denominator.nterms(),
    )?;
    let mut input_terms = mul(cells, zero_terms)?;
    for row in corrections.iter().chain(std::iter::once(baseline)) {
        for (shift, value) in row {
            c.validate_with_limits(value, limits.arithmetic)?;
            require(!value.is_zero(), "explicit zero in dual source image")?;
            denominator(
                c,
                &mut guards,
                value,
                "dual original input denominator",
                limits,
            )?;
            if columns.binary_search(shift).is_ok() {
                input_terms = add(
                    input_terms,
                    add(
                        value.raw().numerator.nterms(),
                        value.raw().denominator.nterms(),
                    )?,
                )?;
            }
        }
    }
    bound(
        add(mul(3, input_terms)?, dense_slots)?,
        limits.coefficient_terms,
        "dual dense input coefficient terms",
    )?;
    let m32 = u32::try_from(m).map_err(|_| Error::Budget("dual native rows"))?;
    let n32 = u32::try_from(n).map_err(|_| Error::Budget("dual native columns"))?;
    // No variable compaction is needed for this small diagnostic. Native
    // admission below restores/checks precisely this indexed variable map.
    let variables = project::native_variables::Variables::new(c, &[], false, limits)?;
    let mut data = vec![c.zero().raw().clone(); cells];
    for (i, row) in corrections
        .iter()
        .chain(std::iter::once(baseline))
        .enumerate()
    {
        for (shift, value) in row {
            if let Ok(j) = columns.binary_search(shift) {
                data[i * n + j] = variables.map(value.raw())?;
            }
        }
    }
    let mut rhs = vec![c.zero().raw().clone(); m];
    rhs[m - 1] = c.one().raw().clone();
    let a = native(|| Matrix::from_linear(data, m32, n32, project::Field::new(Z)))?
        .map_err(|_| Error::Invalid("dual native matrix shape refused".into()))?;
    let rhs = native(|| Matrix::from_linear(rhs, m32, 1, project::Field::new(Z)))?
        .map_err(|_| Error::Invalid("dual native RHS shape refused".into()))?;
    // Public dense solve_any owns elimination and free-variable selection. It
    // does not use sparse::from_matrix_checked or construct a full nullspace.
    let solution = native(|| a.solve_any(&rhs))?
        .map_err(|_| Error::Invalid("native dual solve did not return a solution".into()))?;
    require(
        solution.nrows() == n && solution.ncols() == 1,
        "native dual result shape differs",
    )?;
    let mut values = Vec::with_capacity(n);
    let mut terms = 0;
    for j in 0..n32 {
        // solve_any creates unassigned free variables with Field::zero(),
        // whose map is empty. Only that exact native zero needs no remapping;
        // every nonzero result must carry/authenticate the original map.
        let raw = &solution[(j, 0)];
        let value = if raw.is_zero() {
            c.zero()
        } else {
            variables.admit(c, raw, limits)?
        };
        terms = add(
            terms,
            add(
                value.raw().numerator.nterms(),
                value.raw().denominator.nterms(),
            )?,
        )?;
        bound(
            terms,
            limits.coefficient_terms,
            "dual witness coefficient terms",
        )?;
        denominator(
            c,
            &mut guards,
            &value,
            "normalized dual witness denominator",
            limits,
        )?;
        values.push(value);
    }
    verify(
        c,
        corrections,
        baseline,
        columns,
        &values,
        &mut guards,
        limits,
    )?;
    Ok(Certificate {
        values,
        guards,
        dense_slots,
        operation_charge,
    })
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
