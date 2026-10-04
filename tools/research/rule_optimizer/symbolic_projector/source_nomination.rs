//! Witness-directed finite source preimages. Nomination only: no correction
//! solve, normalization, source-bank mutation, export, or rule authority.
use super::*;
use rustred::identity::SelectedTranslatedSourceBatch;

pub(super) fn enabled(cfg: &Value) -> Result<bool> {
    let on = match cfg.get("witness_preimage_nomination") {
        None => false,
        Some(Value::Bool(on)) => *on,
        _ => return Err("witness_preimage_nomination must be boolean".into()),
    };
    require(
        !on || checked(dual::enabled(cfg))?,
        "preimage nomination requires exact dual separator",
    )?;
    Ok(on)
}

use super::super::source_preimage::{Census, pairing};

fn census(
    c: &IndexedCoefficientContext,
    inventory: &SelectedTranslatedSourceBatch,
    columns: &[IndexShift],
    values: &[rustred::algebra::IndexedCoefficient],
    pin: usize,
    pin_value: i64,
    coordinate_cap: usize,
    limits: project::Limits,
) -> Result<(Census, usize)> {
    super::super::source_preimage::census(
        c,
        inventory,
        columns,
        values,
        Some((pin, pin_value)),
        coordinate_cap,
        limits,
    )
}

fn seed_summary(r: &Value, offset: &[i64]) -> Result<Value> {
    let mask = r["owner_mask"].as_str().ok_or("mask required")?;
    let activated = offset
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| (mask.as_bytes()[i] == b'0' && v > 0).then_some(i))
        .collect::<Vec<_>>();
    Ok(
        json!({"maximum_source_numerator_rank":maximum_rank(r,offset)?,
        "activated_parent_inactive_axes":activated,
        "source_within_parent_support":activated.is_empty(),
        "source_rank_used_as_filter":false}),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    r: &Value,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    inventory: &SelectedTranslatedSourceBatch,
    family: &rustred::family::IntegralFamily,
    target: &IndexShift,
    columns: &[IndexShift],
    separator: &dual::Certificate,
    p: OriginalSourceCombinationLimits,
    limits: project::Limits,
) -> Value {
    let mut report = json!({"schema":"rustred.witness-preimage-nomination.v1",
        "status":"WITNESS_PREIMAGE_REFUSED_OR_INCOMPLETE","census_complete":false,
        "translation_started":false,"pairing_complete":false,"source_bank_modified":false,
        "correction_attempted":false,"exported":false,"rule_certificate":false,
        "scope":"Generic exact unnormalized pairings only; nonzero can vanish on chart faces and does not prove P9 correction feasibility.",
        "filter":"declared fixed pinch source exponent nonpositive; no source-rank or inactive-activation filter"});
    let result = (|| -> Result<()> {
        let c = generator.context();
        require(
            inventory.family_fingerprint() == family.fingerprint(),
            "raw inventory family differs",
        )?;
        let (pin, value) = pin_value(r)?;
        let (found, pin_matches) = census(
            c,
            inventory,
            columns,
            &separator.values,
            pin,
            value,
            limit(r, "max_coordinate_cells")?,
            limits,
        )?;
        report["census_complete"] = json!(true);
        report["raw_inventory_rows"] = json!(inventory.len());
        report["raw_inventory"] = json!({"family_fingerprint":inventory.family_fingerprint(),
            "context_fingerprint":inventory.context_fingerprint(),"before_fixed_specialization":true,
            "rows":inventory.sources().iter().map(|s|json!({
                "source_row":s.row_id().stable_string(),"source_ordinal":s.provenance().source_ordinal(),
                "offset":s.provenance().offset().values(),
                "raw_shifts":s.terms().keys().map(|s|s.values()).collect::<Vec<_>>()
            })).collect::<Vec<_>>()});
        report["raw_inventory_terms"] = json!(
            inventory
                .sources()
                .iter()
                .map(|s| s.terms().len())
                .sum::<usize>()
        );
        report["raw_support_pair_count"] = json!(
            inventory
                .sources()
                .iter()
                .map(|s| s.terms().len())
                .sum::<usize>()
                * separator.values.iter().filter(|v| !v.is_zero()).count()
        );
        report["nonpositive_pin_support_pairs"] = json!(pin_matches);
        report["canonical_preimage_count"] = json!(found.len());
        report["source_row_allowance"] = json!(limits.rows);
        let mut pin_seeds = BTreeMap::<String, usize>::new();
        for q in found.keys() {
            *pin_seeds
                .entry((i128::from(value) + i128::from(q.offset().values()[pin])).to_string())
                .or_default() += 1;
        }
        report["source_pin_seed_counts"] = json!(pin_seeds);
        // Census is complete even on this refusal; never translate a prefix.
        checked(project::bound(
            found.len(),
            limits.rows,
            "complete preimage census rows",
        ))?;
        report["preimages"]=json!(found.iter().map(|(q,w)|json!({
            "source_row":inventory.sources()[q.source_ordinal()].row_id().stable_string(),
            "source_ordinal":q.source_ordinal(),"offset":q.offset().values(),
            "raw_support_witnesses":w.iter().map(|(j,tau)|json!({"column":j,"shift":columns[*j].values(),"raw_shift":tau.values()})).collect::<Vec<_>>()
        })).collect::<Vec<_>>());
        if found.is_empty() {
            report["rows"] = json!([]);
            report["zero_pairings"] = json!(0);
            report["nonzero_pairings"] = json!(0);
        } else {
            report["translation_started"] = json!(true);
            let batch = checked(generator.translate_selected_completed_source_rows(
                completed,
                found.keys().cloned(),
                p.translated_sources,
            ))?;
            require(
                batch.requests().iter().eq(found.keys()),
                "native translated preimage bindings differ",
            )?;
            let input_terms = batch
                .sources()
                .iter()
                .flat_map(|s| s.terms().values())
                .try_fold(0usize, |sum, v| {
                    sum.checked_add(v.raw().numerator.nterms())
                        .and_then(|n| n.checked_add(v.raw().denominator.nterms()))
                        .ok_or("nomination coefficient count overflow")
                })?;
            checked(project::bound(
                input_terms
                    .checked_mul(2)
                    .ok_or("nomination clone count overflow")?,
                limits.coefficient_terms,
                "nomination full-source coefficient copies",
            ))?;
            let images = checked(source::Span::ordinary(
                c,
                &batch,
                &fixed(r)?,
                p.cell.indexed_algebra,
                limits,
            ))?;
            // All translated source denominators and fixed-specialization witnesses
            // enter before any pairing, even for subsequently zero pairings.
            checked(project::bound(
                separator
                    .guards
                    .len()
                    .checked_add(images.guards.len())
                    .ok_or("condition count overflow")?,
                limits.guards,
                "nomination retained conditions",
            ))?;
            let mut guards = separator.guards.clone();
            guards.extend(images.guards);
            let mut operations = 0usize;
            let mut rows = Vec::new();
            let mut nonzero = 0usize;
            let mut leaks = 0usize;
            for ((binding, image), q) in images
                .bindings
                .iter()
                .zip(&images.images)
                .zip(batch.requests())
            {
                require(
                    binding.offset == *q.offset()
                        && binding.row == inventory.sources()[q.source_ordinal()].row_id().clone(),
                    "nomination image binding differs",
                )?;
                let dot = pairing(
                    c,
                    image,
                    columns,
                    &separator.values,
                    &mut guards,
                    &mut operations,
                    limits,
                )?;
                nonzero += usize::from(!dot.is_zero());
                let unpinched = image
                    .keys()
                    .filter(|s| i128::from(value) + i128::from(s.values()[pin]) > 0)
                    .map(|s| s.values())
                    .collect::<Vec<_>>();
                leaks += usize::from(image.contains_key(target) || !unpinched.is_empty());
                rows.push(json!({"source_row":binding.row.stable_string(),"source_ordinal":q.source_ordinal(),
                    "offset":binding.offset.values(),"source_pin_seed":i128::from(value)+i128::from(binding.offset.values()[pin]),
                    "source":seed_summary(r,binding.offset.values())?,
                    "full_image_terms":image.len(),"full_image_shifts":image.keys().map(|s|s.values()).collect::<Vec<_>>(),
                    "target_leak":image.contains_key(target),
                    "unpinched_leaks":unpinched,
                    "pairing_zero":dot.is_zero(),"pairing":dot.raw().to_string(),"display_only":true}));
            }
            report["zero_pairings"] = json!(rows.len() - nonzero);
            report["nonzero_pairings"] = json!(nonzero);
            report["pairing_operations"] = json!(operations);
            report["conditions"] = guards_json(&guards);
            report["rows"] = json!(rows);
            report["leaking_rows"] = json!(leaks);
            require(
                leaks == 0,
                "native full image contains target/unpinched leaks",
            )?;
        }
        report["status"] = json!("EXACT_WITNESS_PREIMAGE_PAIRINGS_COMPLETE");
        report["pairing_complete"] = json!(true);
        require(
            serde_json::to_vec(&report)
                .map_err(|e| e.to_string())?
                .len()
                <= limit(r, "max_report_bytes")?,
            "nomination report budget exceeded",
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        report["status"] = json!("WITNESS_PREIMAGE_REFUSED_OR_INCOMPLETE");
        report["pairing_complete"] = json!(false);
        report["error"] = json!(error);
    }
    report
}

#[cfg(test)]
#[path = "source_nomination_tests.rs"]
mod tests;
