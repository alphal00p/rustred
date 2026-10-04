//! An independent ordinary-frame certificate of an already proved weighted
//! result. Never relabel a weighted span or remove conditions from its proof.
use super::*;

pub const SCHEMA: &str = "rustred.boundary-original-frame-reproof.v1";

pub fn enabled(cfg: &Value) -> Result<bool> {
    match cfg.get("fresh_original_frame_reproof") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err("fresh_original_frame_reproof must be boolean".into()),
    }
}

fn baseline_assumptions(
    c: &IndexedCoefficientContext,
    baseline: &source::Span,
    request: &OriginalSourceCombinationRequest,
    limits: project::Limits,
) -> Result<()> {
    checked(baseline.validate_fresh_ordinary(c, limits))?;
    require(
        request.retained_conditions.len() == baseline.guards.len()
            && request
                .retained_conditions
                .iter()
                .zip(&baseline.guards)
                .all(|(a, b)| *a == b.polynomial),
        "fresh reproof requires the exact known ordinary baseline assumption inventory",
    )
}

fn mapped_weights(
    c: &IndexedCoefficientContext,
    ordinary: &source::Span,
    request: &OriginalSourceCombinationRequest,
    limits: project::Limits,
) -> Result<project::Weights> {
    checked(ordinary.validate_fresh_ordinary(c, limits))?;
    let positions = ordinary
        .bindings
        .iter()
        .enumerate()
        .map(|(i, b)| ((b.row.clone(), b.offset.clone()), i))
        .collect::<BTreeMap<_, _>>();
    require(
        positions.len() == ordinary.bindings.len(),
        "duplicate ordinary binding",
    )?;
    checked(project::bound(
        request.contributions.len(),
        limits.rows,
        "reproof weights",
    ))?;
    let mut mapped = project::Weights::new();
    for contribution in &request.contributions {
        checked(c.validate_with_limits(&contribution.weight, limits.arithmetic))?;
        let ordinal = *positions
            .get(&(contribution.source_row.clone(), contribution.offset.clone()))
            .ok_or("final contribution absent from regenerated ordinary frame")?;
        require(
            mapped
                .insert(ordinal, contribution.weight.clone())
                .is_none(),
            "duplicate final contribution binding",
        )?;
    }
    Ok(mapped)
}

fn verify_image(
    c: &IndexedCoefficientContext,
    ordinary: &source::Span,
    request: &OriginalSourceCombinationRequest,
    target: &IndexShift,
    expected: &project::Row,
    limits: project::Limits,
) -> Result<Vec<project::Guard>> {
    let weights = mapped_weights(c, ordinary, request, limits)?;
    // These are replay diagnostics, not a substitute for original assumptions.
    // The fresh native checker independently regenerates final weight/RHS poles.
    let mut guards = Vec::new();
    let actual = checked(project::replay(
        c,
        &ordinary.originals,
        &weights,
        &mut guards,
        limits,
    ))?;
    require(
        actual == *expected,
        "fresh ordinary full image differs from weighted candidate",
    )?;
    require(
        actual.get(target) == Some(&c.one()),
        "fresh ordinary target is not one",
    )?;
    let mut rhs = project::Row::new();
    for (shift, value) in &request.rhs {
        checked(c.validate_with_limits(value, limits.arithmetic))?;
        require(
            shift != target && rhs.insert(shift.clone(), value.clone()).is_none(),
            "duplicate/target RHS binding",
        )?;
    }
    let expected_rhs = actual
        .iter()
        .filter(|(s, _)| *s != target)
        .map(|(s, v)| Ok((s.clone(), checked(c.neg_with_limits(v, limits.arithmetic))?)))
        .collect::<Result<project::Row>>()?;
    require(
        rhs == expected_rhs,
        "fresh ordinary RHS differs from weighted candidate",
    )?;
    Ok(guards)
}

pub fn run(
    r: &Value,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    ids: &BTreeMap<String, usize>,
    baseline: &source::Span,
    checked_baseline: &OriginalSourceCombinationRequest,
    union: &source::Span,
    target: &IndexShift,
    image: &project::Row,
    weighted_request: OriginalSourceCombinationRequest,
    weighted_proof: &CheckedOriginalSourceCombination,
    family: &rustred::family::IntegralFamily,
    p: OriginalSourceCombinationLimits,
    limits: project::Limits,
    export: bool,
) -> Outcome {
    let c = generator.context();
    let mut report = json!({"schema":SCHEMA,"status":"FRESH_ORIGINAL_FRAME_REPROOF_REFUSED_OR_INCOMPLETE",
        "original_source_replay_verified":false,"fresh_original_source_certificate":false,
        "weighted_proof_modified":false,"weighted_conditions_removed":false,
        "fallback_to_weighted_or_baseline_export":false,"affine_infeasibility_claim":false});
    let mut export_request = None;
    let result = (|| -> Result<()> {
        require(
            certificate::enabled(r)?,
            "fresh reproof requires ordinary fresh stage-one certification",
        )?;
        baseline_assumptions(c, baseline, checked_baseline, limits)?;
        require(
            weighted_proof.family_fingerprint() == family.fingerprint()
                && weighted_proof.context_fingerprint() == c.fingerprint()
                && weighted_proof.root_sector() == &weighted_request.root_sector
                && weighted_proof.sector() == &weighted_request.sector
                && weighted_proof.ordering() == &weighted_request.ordering
                && weighted_proof.requested_bounds()
                    == (
                        weighted_request.lower.as_slice(),
                        weighted_request.upper.as_slice(),
                    ),
            "weighted proof and reproof request binding differ",
        )?;
        require(
            checked_baseline.root_sector == weighted_request.root_sector
                && checked_baseline.sector == weighted_request.sector
                && checked_baseline.ordering == weighted_request.ordering
                && checked_baseline.lower == weighted_request.lower
                && checked_baseline.upper == weighted_request.upper
                && checked_baseline
                    .fixed
                    .iter()
                    .map(|f| (f.position(), f.value()))
                    .eq(weighted_request
                        .fixed
                        .iter()
                        .map(|f| (f.position(), f.value()))),
            "fresh reproof changed the baseline chart",
        )?;
        let (entries, terms) = clone_admission(union, limits)?;
        checked(project::bound(
            entries.checked_mul(2).ok_or("reproof entries overflow")?,
            limits.nonzeros,
            "reproof ordinary image copies",
        ))?;
        checked(project::bound(
            terms.checked_mul(2).ok_or("reproof terms overflow")?,
            limits.coefficient_terms,
            "reproof ordinary coefficient copies",
        ))?;
        checked(project::bound(
            union.bindings.len(),
            limits.rows,
            "reproof complete original frame",
        ))?;
        let requested = union
            .bindings
            .iter()
            .map(|b| {
                Ok(TranslatedSourceRequest::new(
                    *ids.get(&b.row.stable_string())
                        .ok_or("original RowId absent from complete inventory")?,
                    b.offset.clone(),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let batch = checked(generator.translate_selected_completed_source_rows(
            completed,
            requested,
            p.translated_sources,
        ))?;
        require(
            batch.len() == union.bindings.len(),
            "regenerated ordinary census changed",
        )?;
        let mut ordinary = checked(source::Span::ordinary(
            c,
            &batch,
            &fixed(r)?,
            p.cell.indexed_algebra,
            limits,
        ))?;
        let old_rows = union
            .bindings
            .iter()
            .zip(&union.originals)
            .map(|(b, row)| ((b.row.clone(), b.offset.clone()), row))
            .collect::<BTreeMap<_, _>>();
        require(
            old_rows.len() == union.bindings.len(),
            "duplicate original union binding",
        )?;
        for (b, row) in ordinary.bindings.iter().zip(&ordinary.originals) {
            require(
                old_rows
                    .get(&(b.row.clone(), b.offset.clone()))
                    .is_some_and(|old| **old == *row),
                "regenerated original binding/full row differs",
            )?;
        }
        // Exact known stage-one original/caller inventory, NOT a filter on the
        // flattened weighted request's guard labels. Include unused originals.
        for guard in &checked_baseline.retained_conditions {
            checked(project::retain(
                c,
                &mut ordinary.guards,
                guard.clone(),
                "known stage-one original/caller assumption",
                limits,
            ))?;
        }
        let replay_guards = verify_image(c, &ordinary, &weighted_request, target, image, limits)?;
        report["declared_original_sources"] = json!(
            array(r, "sources")?.len()
                + array(&r["boundary_correction"], "correction_sources")?.len()
        );
        report["canonical_original_sources"] = json!(ordinary.bindings.len());
        report["final_source_contributions"] = json!(weighted_request.contributions.len());
        report["rhs_terms"] = json!(weighted_request.rhs.len());
        report["ordinary_constructor_validated"] = json!(true);
        report["full_product_equal"] = json!(true);
        report["typed_rhs_equal"] = json!(true);
        report["replay_conditions"] = guards_json(&replay_guards);
        report["all_original_assumptions"] = guards_json(&ordinary.guards);
        let fresh = checked(certificate::fresh(c, &ordinary, weighted_request, limits))?;
        require(
            checked_baseline
                .retained_conditions
                .iter()
                .all(|g| fresh.retained_conditions.contains(g)),
            "fresh request lost a genuine baseline assumption",
        )?;
        let retained = if export {
            Some(clone_for_export(
                c,
                &fresh,
                limit(r, "max_coordinate_cells")?,
                limits,
            )?)
        } else {
            None
        };
        let diagnostic = if guard_diagnostic::enabled(&r["boundary_correction"])? {
            Some(guard_diagnostic::prepare(
                c,
                family.fingerprint(),
                &fresh,
                p.cell.indexed_algebra,
                limit(r, "max_coordinate_cells")?,
                limits,
            ))
        } else {
            None
        };
        let proof = match check_original_source_combination(family, fresh, p) {
            Ok(proof) => proof,
            Err(error) => {
                report["status"] = json!("FRESH_ORIGINAL_FRAME_PROOF_REFUSED");
                report["proof_error"] = error_json(&error);
                return Ok(());
            }
        };
        require(
            proof.cells().count() == weighted_proof.cells().count()
                && (0..proof.cells().count())
                    .all(|i| proof.cell_bounds(i) == weighted_proof.cell_bounds(i))
                && proof
                    .cells()
                    .zip(weighted_proof.cells())
                    .all(|(a, b)| a.rule().right_hand_side() == b.rule().right_hand_side()),
            "fresh proof changed weighted candidate cells/RHS",
        )?;
        report["proof_cells"] = proof_json(&proof);
        report["cell_coverage_and_rhs_equal"] = json!(true);
        report["original_source_replay_verified"] = json!(true);
        report["fresh_original_source_certificate"] = json!(true);
        report["status"] = json!("EXACT_FRESH_ORIGINAL_FRAME_CHART_PROVED");
        if let Some(input) = diagnostic {
            report["export_guard_diagnostic"] =
                guard_diagnostic::report(c, &proof, input, p.cell.indexed_algebra, limits);
        }
        export_request = retained;
        Ok(())
    })();
    if let Err(error) = result {
        report["error"] = json!(error);
        export_request = None;
    }
    Outcome {
        report,
        export_request,
    }
}

#[cfg(test)]
#[path = "boundary_reproof_tests.rs"]
mod tests;
