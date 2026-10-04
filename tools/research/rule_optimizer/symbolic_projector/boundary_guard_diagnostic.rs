//! Read-only mirror of the existing priority export guard transport test.
//! This explains a refusal; it neither admits a guard nor changes a proof.
use super::super::{Result, checked, project, require};
use rustred::{
    algebra::{
        CoefficientPolynomial, IndexedAlgebraLimits, IndexedCoefficientContext, IndexedPolynomial,
    },
    foundry::{
        artifact::{CheckedOriginalSourceCombination, OriginalSourceCombinationRequest},
        parametric::ParametricGuardOrigin,
    },
};
use serde_json::{Value, json};
use symbolica::prelude::*;

pub const SOURCE_SHA256: &str = "5b9d709d7974f44bb23278cb05ea21df08142f2394aed224d3f2ae09a3f596ce";

pub fn enabled(cfg: &Value) -> Result<bool> {
    match cfg.get("export_guard_diagnostic") {
        None => Ok(false),
        Some(Value::Bool(v)) => Ok(*v),
        _ => Err("export_guard_diagnostic must be boolean".into()),
    }
}

pub struct Prepared {
    family: String,
    context: String,
    root: Vec<bool>,
    sector: Vec<bool>,
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    ordering: rustred::sector::OrderingPolicy,
    denominators: Vec<(usize, Vec<i64>, IndexedPolynomial)>,
}

fn charge(total: &mut usize, count: usize, max: usize, resource: &'static str) -> Result<()> {
    *total = total
        .checked_add(count)
        .ok_or("guard diagnostic count overflow")?;
    checked(project::bound(*total, max, resource))
}

/// Retain only the actual broad runtime denominators before the checker consumes
/// its request. Fixed restriction/zero removal matches priority.rs:153--176.
pub fn prepare(
    c: &IndexedCoefficientContext,
    family: &str,
    request: &OriginalSourceCombinationRequest,
    algebra: IndexedAlgebraLimits,
    coordinate_limit: usize,
    limits: project::Limits,
) -> Result<Prepared> {
    checked(project::bound(
        request.rhs.len(),
        limits.columns,
        "guard diagnostic RHS columns",
    ))?;
    let coordinates = request
        .rhs
        .len()
        .checked_add(4)
        .and_then(|n| n.checked_mul(c.index_count()))
        .ok_or("guard diagnostic coordinate overflow")?;
    checked(project::bound(
        coordinates,
        coordinate_limit,
        "guard diagnostic coordinates",
    ))?;
    let mut terms = 0;
    for (_, value) in &request.rhs {
        checked(c.validate_with_limits(value, limits.arithmetic))?;
        charge(
            &mut terms,
            value.raw().numerator.nterms(),
            limits.coefficient_terms,
            "guard diagnostic input terms",
        )?;
        charge(
            &mut terms,
            value.raw().denominator.nterms(),
            limits.coefficient_terms,
            "guard diagnostic input terms",
        )?;
    }
    let fixed = request
        .fixed
        .iter()
        .map(|f| (f.position(), f.value()))
        .collect::<Vec<_>>();
    let mut denominators = Vec::new();
    let mut operations = 0;
    for (ordinal, (shift, value)) in request.rhs.iter().enumerate() {
        charge(
            &mut operations,
            2,
            limits.operations,
            "guard diagnostic preparation work",
        )?;
        let (value, _) = checked(c.specialize_fixed_indices(value, &fixed, algebra))?;
        if value.is_zero() {
            continue;
        }
        let denominator =
            checked(c.denominator_condition_with_limits(&value, algebra.exact_algebra))?;
        charge(
            &mut terms,
            denominator.raw().nterms(),
            limits.coefficient_terms,
            "guard diagnostic retained terms",
        )?;
        denominators.push((ordinal, shift.values().to_vec(), denominator));
    }
    Ok(Prepared {
        family: family.into(),
        context: c.fingerprint().into(),
        root: request.root_sector.active_bits().to_vec(),
        sector: request.sector.active_bits().to_vec(),
        lower: request.lower.clone(),
        upper: request.upper.clone(),
        ordering: request.ordering.clone(),
        denominators,
    })
}

fn primitive(value: &IndexedPolynomial) -> Result<CoefficientPolynomial> {
    checked(project::native(|| {
        let mut p = value.raw().clone().make_primitive();
        if p.lcoeff().is_negative() {
            p = p.mul_coeff(Integer::from(-1));
        }
        p
    }))
}

fn exclusive_base_weight(
    c: &IndexedCoefficientContext,
    guard: &IndexedPolynomial,
    origins: &[ParametricGuardOrigin],
    limits: project::Limits,
) -> Result<bool> {
    checked(c.validate_polynomial_with_limits(guard, limits.arithmetic))?;
    if guard.is_zero()
        || origins.is_empty()
        || !origins.iter().all(|o| {
            matches!(
                o,
                ParametricGuardOrigin::SourceCombinationDenominator { .. }
            )
        })
    {
        return Ok(false);
    }
    let base = c.base().one().get_variables().len();
    Ok((0..c.index_count()).all(|axis| guard.raw().degree(base + axis) == 0))
}

fn origins_json(origins: &[ParametricGuardOrigin]) -> Value {
    json!(origins.iter().map(|o| match o {
        ParametricGuardOrigin::SourceCondition { source_ordinal, row_id, condition_ordinal, condition_sources } =>
            json!({"kind":"SourceCondition","source_ordinal":source_ordinal,"row_id":row_id.stable_string(),
                "condition_ordinal":condition_ordinal,"native_condition_sources_debug":format!("{condition_sources:?}")}),
        ParametricGuardOrigin::SourceCoefficientDenominator { source_ordinal,row_id,shift } =>
            json!({"kind":"SourceCoefficientDenominator","source_ordinal":source_ordinal,"row_id":row_id.stable_string(),"shift":shift.values()}),
        ParametricGuardOrigin::ReducerPivotNumerator { source_ordinal,row_id,pivot_column,pivot_shift } =>
            json!({"kind":"ReducerPivotNumerator","source_ordinal":source_ordinal,"row_id":row_id.stable_string(),"pivot_column":pivot_column,"shift":pivot_shift.values()}),
        ParametricGuardOrigin::ReducerPivotDenominator { source_ordinal,row_id,pivot_column,pivot_shift } =>
            json!({"kind":"ReducerPivotDenominator","source_ordinal":source_ordinal,"row_id":row_id.stable_string(),"pivot_column":pivot_column,"shift":pivot_shift.values()}),
        ParametricGuardOrigin::RuleCoefficientDenominator { shift } => json!({"kind":"RuleCoefficientDenominator","shift":shift.values()}),
        ParametricGuardOrigin::SourceCombinationDenominator { source_ordinal,row_id } =>
            json!({"kind":"SourceCombinationDenominator","source_ordinal":source_ordinal,"row_id":row_id.stable_string()}),
        ParametricGuardOrigin::FinalTargetCoefficient => json!({"kind":"FinalTargetCoefficient"}),
        ParametricGuardOrigin::OriginalDomainCondition { condition_ordinal } =>
            json!({"kind":"OriginalDomainCondition","condition_ordinal":condition_ordinal}),
    }).collect::<Vec<_>>())
}

struct Classification {
    constant: bool,
    weight_only: bool,
    primitive: Option<CoefficientPolynomial>,
    matches: Vec<usize>,
    accepted: bool,
}

fn classify(
    c: &IndexedCoefficientContext,
    p: &IndexedPolynomial,
    origins: &[ParametricGuardOrigin],
    denominators: &[CoefficientPolynomial],
    limits: project::Limits,
) -> Result<Classification> {
    let constant = p.is_nonzero_constant();
    let weight_only = exclusive_base_weight(c, p, origins, limits)?;
    let canonical = if p.is_zero() {
        None
    } else {
        Some(primitive(p)?)
    };
    let matches = denominators
        .iter()
        .enumerate()
        .filter_map(|(i, d)| (canonical.as_ref() == Some(d)).then_some(i))
        .collect::<Vec<_>>();
    let accepted = constant || weight_only || (!p.is_zero() && !matches.is_empty());
    Ok(Classification {
        constant,
        weight_only,
        primitive: canonical,
        matches,
        accepted,
    })
}

fn run(
    c: &IndexedCoefficientContext,
    proof: &CheckedOriginalSourceCombination,
    input: Prepared,
    algebra: IndexedAlgebraLimits,
    limits: project::Limits,
) -> Result<Value> {
    require(
        input.family == proof.family_fingerprint()
            && input.context == c.fingerprint()
            && input.context == proof.context_fingerprint()
            && input.root == proof.root_sector().active_bits()
            && input.sector == proof.sector().active_bits()
            && &input.ordering == proof.ordering()
            && (input.lower.as_slice(), input.upper.as_slice()) == proof.requested_bounds(),
        "guard diagnostic proof/context/request binding differs",
    )?;
    let guard_count = proof
        .cells()
        .try_fold(0usize, |n, cell| {
            n.checked_add(cell.rule().nonzero_guards().len())
        })
        .ok_or("guard diagnostic count overflow")?;
    checked(project::bound(
        guard_count,
        limits.guards,
        "guard diagnostic all-cell guards",
    ))?;
    let mut operations = 0;
    let mut terms = 0;
    let mut evaluations = Vec::new();
    let mut mismatches = Vec::new();
    let mut cell_reports = Vec::new();
    for (cell_ordinal, cell) in proof.cells().enumerate() {
        let (lower, upper) = proof
            .cell_bounds(cell_ordinal)
            .ok_or("missing checked cell bounds")?;
        let fixed = lower
            .iter()
            .zip(upper)
            .enumerate()
            .filter_map(|(axis, (&lo, &hi))| {
                (hi == Some(lo)).then_some((
                    axis,
                    if input.sector[axis] {
                        i128::from(lo) + 1
                    } else {
                        -i128::from(lo)
                    },
                ))
            })
            .map(|(axis, v)| {
                Ok((
                    axis,
                    i64::try_from(v).map_err(|_| "guard diagnostic fixed overflow")?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut denominators = Vec::new();
        let mut denominator_reports = Vec::new();
        for (rhs_ordinal, shift, polynomial) in &input.denominators {
            charge(
                &mut operations,
                2,
                limits.operations,
                "guard diagnostic native work",
            )?;
            let p = checked(c.specialize_fixed_polynomial(polynomial, &fixed, algebra))?;
            require(
                !p.is_zero(),
                "runtime RHS denominator vanishes on checked cell",
            )?;
            charge(
                &mut terms,
                p.raw().nterms(),
                limits.coefficient_terms,
                "guard diagnostic output terms",
            )?;
            let canonical = primitive(&p)?;
            denominator_reports.push(json!({"rhs_ordinal":rhs_ordinal,"shift":shift,
                "polynomial":p.raw().to_string(),"primitive":canonical.to_string(),"display_only":true}));
            denominators.push(canonical);
        }
        for (guard_ordinal, g) in cell.rule().nonzero_guards().iter().enumerate() {
            charge(
                &mut operations,
                denominators
                    .len()
                    .checked_add(3)
                    .ok_or("guard diagnostic operation overflow")?,
                limits.operations,
                "guard diagnostic native work",
            )?;
            let p = checked(c.specialize_fixed_polynomial(g.polynomial(), &fixed, algebra))?;
            charge(
                &mut terms,
                p.raw().nterms(),
                limits.coefficient_terms,
                "guard diagnostic output terms",
            )?;
            let classification = classify(c, &p, g.origins(), &denominators, limits)?;
            let matches = classification
                .matches
                .iter()
                .map(|&i| input.denominators[i].0)
                .collect::<Vec<_>>();
            let row = json!({"cell":cell_ordinal,"guard":guard_ordinal,
                "lower":lower,"upper":upper,"original_polynomial":g.polynomial().raw().to_string(),
                "specialized_polynomial":p.raw().to_string(),"primitive":classification.primitive.map(|v|v.to_string()),
                "display_only":true,"origins":origins_json(g.origins()),"nonzero_constant":classification.constant,
                "exclusive_base_field_weight_origin":classification.weight_only,"matching_rhs_ordinals":matches,
                "mirrored_transport_accepts":classification.accepted});
            if !classification.accepted {
                mismatches.push(row.clone());
            }
            evaluations.push(row);
        }
        cell_reports.push(json!({"cell":cell_ordinal,"lower":lower,"upper":upper,
            "runtime_denominators":denominator_reports}));
    }
    Ok(
        json!({"status":"EXACT_PRIORITY_GUARD_TRANSPORT_DIAGNOSTIC","complete":true,
        "family_fingerprint":input.family,"context_fingerprint":input.context,
        "root":input.root,"sector":input.sector,"requested_lower":input.lower,"requested_upper":input.upper,
        "ordering":input.ordering.stable_id().to_string(),
        "mirrored_transport_accepts":mismatches.is_empty(),"first_mismatch":mismatches.first(),
        "mismatches":mismatches,"evaluations":evaluations,"cells":cell_reports,
        "native_operation_charge":operations}),
    )
}

pub fn report(
    c: &IndexedCoefficientContext,
    proof: &CheckedOriginalSourceCombination,
    input: Result<Prepared>,
    algebra: IndexedAlgebraLimits,
    limits: project::Limits,
) -> Value {
    let result = input.and_then(|input| run(c, proof, input, algebra, limits));
    let mut value = match result {
        Ok(value) => value,
        Err(error) => {
            json!({"status":"GUARD_DIAGNOSTIC_REFUSED_OR_INCOMPLETE","complete":false,"error":error})
        }
    };
    value["schema"] = json!("rustred.priority-guard-diagnostic.v1");
    value["mirrored_priority_source_sha256"] = json!(SOURCE_SHA256);
    value["native_export_authority"] = json!(false);
    value["proof_changed"] = json!(false);
    value["conditions_removed"] = json!(false);
    value["internal_native_scratch_bounded_by_outer_guard"] = json!(true);
    value
}

#[cfg(test)]
#[path = "boundary_guard_diagnostic_tests.rs"]
mod tests;
