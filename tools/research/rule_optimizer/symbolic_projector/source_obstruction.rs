//! Completed ordinary-span obstruction and witness-directed raw source preimages.
//! Diagnostic only: never changes the bank, F, chart, solver outcome or artifact.
use super::*;
use rustred::{
    algebra::IndexedCoefficientContext,
    identity::{CompletedIbpSourceRows, SelectedTranslatedSourceBatch},
};

const FIELD: &str = "source_obstruction_diagnostic";
const SOURCE_CAP: &str = "max_obstruction_preimage_sources";
const TERM_CAP: &str = "max_obstruction_translated_terms";

pub(super) fn enabled(r: &Value) -> Result<bool> {
    match r.get(FIELD) {
        None => Ok(false),
        Some(Value::Bool(on)) => Ok(*on),
        _ => Err(format!("{FIELD} must be boolean")),
    }
}

pub(super) fn validate(r: &Value) -> Result<()> {
    if !enabled(r)? {
        return Ok(());
    }
    for field in [
        "boundary_correction",
        "boundary_polynomial",
        "modular_nomination",
        "exact_source_ordinals",
        "source_weight_reconstruction",
    ] {
        require(
            r.get(field).is_none(),
            "source obstruction requires the complete ordinary bank",
        )?;
    }
    require(
        !direct_l::enabled(r)?,
        "source obstruction requires the ordinary augmented projector",
    )?;
    require(
        number(r, "max_refinements")? == 0,
        "source obstruction does not refine",
    )?;
    limit(r, SOURCE_CAP)?;
    limit(r, TERM_CAP)?;
    Ok(())
}

/// The source-bank cap is unchanged. Only the known normalization equation is
/// augmented; dense storage, arithmetic and polynomial charges remain native.
fn separator(
    c: &IndexedCoefficientContext,
    images: &[project::Row],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    incoming: &[project::Guard],
    limits: project::Limits,
) -> Result<(Vec<IndexShift>, exact_dual::Certificate)> {
    checked(project::bound(
        images.len(),
        limits.rows,
        "actual obstruction source rows",
    ))?;
    require(!forbidden.contains(target), "target cannot be forbidden")?;
    checked(project::bound(
        forbidden.len().checked_add(1).ok_or("column overflow")?,
        limits.columns,
        "obstruction F plus target",
    ))?;
    let columns = forbidden
        .iter()
        .cloned()
        .chain(std::iter::once(target.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let unit = project::Row::from([(target.clone(), c.one())]);
    let mut augmented = limits;
    augmented.rows = limits
        .rows
        .checked_add(1)
        .ok_or("normalization row allowance overflow")?;
    let certificate = checked(exact_dual::solve(
        c, images, &unit, &columns, incoming, augmented,
    ))?;
    let j = columns
        .binary_search(target)
        .map_err(|_| "target binding disappeared")?;
    require(
        certificate.values[j] == c.one(),
        "separator target normalization differs",
    )?;
    Ok((columns, certificate))
}

fn raw_inventory_json(inventory: &SelectedTranslatedSourceBatch) -> Value {
    json!({"family_fingerprint":inventory.family_fingerprint(),
        "context_fingerprint":inventory.context_fingerprint(),"before_fixed_specialization":true,
        "rows":inventory.sources().iter().map(|s|json!({
            "source_row":s.row_id().stable_string(),"source_ordinal":s.provenance().source_ordinal(),
            "offset":s.provenance().offset().values(),
            "raw_shifts":s.terms().keys().map(|s|s.values()).collect::<Vec<_>>()
        })).collect::<Vec<_>>()})
}

#[allow(clippy::too_many_arguments)]
fn preimages(
    r: &Value,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    inventory: &SelectedTranslatedSourceBatch,
    family: &rustred::family::IntegralFamily,
    target: &IndexShift,
    universe: &BTreeSet<IndexShift>,
    columns: &[IndexShift],
    certificate: &exact_dual::Certificate,
    mut p: OriginalSourceCombinationLimits,
    mut limits: project::Limits,
) -> Value {
    let mut report = json!({"schema":"rustred.general-source-preimages.v1",
        "status":"SOURCE_PREIMAGES_REFUSED_OR_INCOMPLETE","census_complete":false,
        "translation_started":false,"pairing_complete":false,
        "source_bank_modified":false,"correction_attempted":false,"exported":false,
        "filter":"none; source rank, sign, activation and target-containing images do not exclude nominees",
        "maximum_source_numerator_rank":null,"rank_bound_claim":false,
        "rank_scope":"No finite rank bound inferred from chart lower corners; inactive rays may have unbounded numerator rank.",
        "scope":"Exact generic unnormalized E*lambda only. A nonzero pairing breaks this witness, not every obstruction; new source columns may introduce additional mandatory F. It may vanish on every point of a correlated finite demand. No demand-point nonzero, rule, chart-feasibility or benefit certificate."});
    let result = (|| -> Result<()> {
        let c = generator.context();
        require(
            inventory.family_fingerprint() == family.fingerprint(),
            "raw inventory family differs",
        )?;
        // These two allowances apply ONLY to the diagnostic census/translation.
        // The original projection bank and original-source proof limits stay put.
        limits.rows = limit(r, SOURCE_CAP)?;
        let terms = limit(r, TERM_CAP)?;
        p.translated_sources.max_requested_source_translations = limits.rows;
        p.translated_sources.max_requested_offsets = limits.rows;
        p.translated_sources.max_translated_sources = limits.rows;
        p.translated_sources.max_translated_term_entries = terms;
        let (found, pairs) = source_preimage::census(
            c,
            inventory,
            columns,
            &certificate.values,
            None,
            limit(r, "max_coordinate_cells")?,
            limits,
        )?;
        report["census_complete"] = json!(true);
        report["raw_inventory"] = raw_inventory_json(inventory);
        report["raw_support_pair_count"] = json!(pairs);
        report["canonical_preimage_count"] = json!(found.len());
        report["diagnostic_source_allowance"] = json!(limits.rows);
        report["diagnostic_translated_term_allowance"] = json!(terms);
        checked(project::bound(
            found.len(),
            limits.rows,
            "complete generic preimage census rows",
        ))?;
        report["preimages"] =
            json!(found.iter().map(|(q,w)|json!({
            "source_row":inventory.sources()[q.source_ordinal()].row_id().stable_string(),
            "source_ordinal":q.source_ordinal(),"offset":q.offset().values(),
            "raw_support_witnesses":w.iter().map(|(j,tau)|json!({"column":j,
                "shift":columns[*j].values(),"raw_shift":tau.values()})).collect::<Vec<_>>()
        })).collect::<Vec<_>>());
        let mut guards = certificate.guards.clone();
        let mut rows = Vec::new();
        let mut operations = 0usize;
        let mut nonzero = 0usize;
        if !found.is_empty() {
            report["translation_started"] = json!(true);
            let batch = checked(generator.translate_selected_completed_source_rows(
                completed,
                found.keys().cloned(),
                p.translated_sources,
            ))?;
            require(
                batch.requests().iter().eq(found.keys()),
                "translated preimage bindings differ",
            )?;
            let input_terms = batch
                .sources()
                .iter()
                .flat_map(|s| s.terms().values())
                .try_fold(0usize, |n, v| {
                    n.checked_add(v.raw().numerator.nterms())
                        .and_then(|n| n.checked_add(v.raw().denominator.nterms()))
                        .ok_or("coefficient count overflow")
                })?;
            checked(project::bound(
                input_terms.checked_mul(2).ok_or("clone count overflow")?,
                limits.coefficient_terms,
                "generic preimage full-source coefficient copies",
            ))?;
            let images = checked(source::Span::ordinary(
                c,
                &batch,
                &fixed(r)?,
                p.cell.indexed_algebra,
                limits,
            ))?;
            checked(project::bound(
                guards
                    .len()
                    .checked_add(images.guards.len())
                    .ok_or("guard count overflow")?,
                limits.guards,
                "generic preimage retained conditions",
            ))?;
            guards.extend(images.guards);
            for ((binding, image), q) in images
                .bindings
                .iter()
                .zip(&images.images)
                .zip(batch.requests())
            {
                require(
                    binding.offset == *q.offset()
                        && binding.row == inventory.sources()[q.source_ordinal()].row_id().clone(),
                    "generic preimage image binding differs",
                )?;
                let value = source_preimage::pairing(
                    c,
                    image,
                    columns,
                    &certificate.values,
                    &mut guards,
                    &mut operations,
                    limits,
                )?;
                nonzero += usize::from(!value.is_zero());
                rows.push(json!({"source_row":binding.row.stable_string(),
                    "source_ordinal":q.source_ordinal(),"offset":binding.offset.values(),
                    "full_image_terms":image.len(),
                    "full_image_shifts":image.keys().map(|s|s.values()).collect::<Vec<_>>(),
                    "target_present":image.contains_key(target),
                    "columns_outside_frozen_universe":image.keys().filter(|s|!universe.contains(*s)).map(|s|s.values()).collect::<Vec<_>>(),
                    "pairing_zero":value.is_zero(),"pairing":value.raw().to_string(),"display_only":true}));
            }
        }
        report["zero_pairings"] = json!(rows.len() - nonzero);
        report["nonzero_pairings"] = json!(nonzero);
        report["pairing_operations"] = json!(operations);
        report["conditions"] = guards_json(&guards);
        report["rows"] = json!(rows);
        report["status"] = json!("EXACT_SOURCE_PREIMAGE_PAIRINGS_COMPLETE");
        report["pairing_complete"] = json!(true);
        require(
            serde_json::to_vec(&report)
                .map_err(|e| e.to_string())?
                .len()
                <= limit(r, "max_report_bytes")?,
            "generic preimage report budget exceeded",
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        report["status"] = json!("SOURCE_PREIMAGES_REFUSED_OR_INCOMPLETE");
        report["pairing_complete"] = json!(false);
        report["error"] = json!(error);
    }
    report
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_report(
    r: &Value,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    inventory: &SelectedTranslatedSourceBatch,
    family: &rustred::family::IntegralFamily,
    span: &source::Span,
    visited_rows: usize,
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    universe: &BTreeSet<IndexShift>,
    incoming: &[project::Guard],
    p: OriginalSourceCombinationLimits,
    limits: project::Limits,
    attempt: &mut Value,
) {
    if matches!(enabled(r), Ok(false)) {
        return;
    }
    let mut report = json!({"schema":"rustred.source-obstruction-diagnostic.v1",
        "status":"SOURCE_OBSTRUCTION_REFUSED_OR_INCOMPLETE","separator_verified":false,
        "source_bank_modified":false,"rule_certificate":false,"exported":false,
        "pointwise_impossibility_claim":false,"family_impossibility_claim":false,
        "scope":"One exact generic rational-function finite-span separator; no claim at exceptional index/base specializations.",
        "solver":"shared Symbolica Matrix::solve_any plus independent complete indexed products",
        "normalization_equations":1,"actual_source_bank_allowance":limits.rows,
        "internal_pivot_guards_exposed":false,
        "assumption_policy":"All incoming/input/witness/replay denominators retained. Solver pivots are not a rule or domain certificate. Internal polynomial scratch remains under outer RSS/deadline limits."});
    let result = (|| -> Result<()> {
        validate(r)?;
        require(
            visited_rows == span.images.len()
                && span.images.len() == span.originals.len()
                && span.images.len() == array(r, "sources")?.len(),
            "diagnostic requires every ordinary row visited",
        )?;
        let c = generator.context();
        checked(span.validate_fresh_ordinary(c, limits))?;
        let (columns, certificate) =
            separator(c, &span.images, target, forbidden, incoming, limits)?;
        report["separator_verified"] = json!(true);
        report["columns"] = json!(columns.iter().enumerate().map(|(j,s)|json!({
            "column":j,"shift":s.values(),"target":s==target,
            "source_rows":span.images.iter().enumerate().filter_map(|(i,row)|row.contains_key(s).then_some(i)).collect::<Vec<_>>()
        })).collect::<Vec<_>>());
        report["source_rows"] = r["sources"].clone();
        report["independent_replay"] = json!({"source_products":span.images.len(),
            "all_source_products_zero":true,"target_coordinate_one":true});
        report["dense_slot_charge"] = json!(certificate.dense_slots);
        report["field_operation_charge"] = json!(certificate.operation_charge);
        report["conditions"] = guards_json(&certificate.guards);
        report["lambda"] = json!(
            certificate
                .values
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_zero())
                .map(|(j, v)| json!({"column":j,"shift":columns[j].values(),
                "coefficient":v.raw().to_string(),"display_only":true}))
                .collect::<Vec<_>>()
        );
        report["preimages"] = preimages(
            r,
            generator,
            completed,
            inventory,
            family,
            target,
            universe,
            &columns,
            &certificate,
            p,
            limits,
        );
        report["status"] = json!(if report["preimages"]["pairing_complete"] == true {
            "EXACT_SOURCE_OBSTRUCTION_AND_PREIMAGES"
        } else {
            "EXACT_SOURCE_OBSTRUCTION_PREIMAGES_INCOMPLETE"
        });
        require(
            serde_json::to_vec(&report)
                .map_err(|e| e.to_string())?
                .len()
                <= limit(r, "max_report_bytes")?,
            "source obstruction report budget exceeded",
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        report["error"] = json!(error);
        report["status"] = json!("SOURCE_OBSTRUCTION_REFUSED_OR_INCOMPLETE");
    }
    attempt[FIELD] = report;
}

#[cfg(test)]
#[path = "source_obstruction_tests.rs"]
mod tests;
