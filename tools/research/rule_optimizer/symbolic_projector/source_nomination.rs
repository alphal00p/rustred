//! Witness-directed finite source preimages. Nomination only: no correction
//! solve, normalization, source-bank mutation, export, or rule authority.
use super::*;
use rustred::identity::{RowId, SelectedTranslatedSourceBatch};

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

type Census = BTreeMap<TranslatedSourceRequest, BTreeSet<(usize, IndexShift)>>;

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
    require(
        columns.len() == values.len() && pin < c.index_count(),
        "nomination witness shape differs",
    )?;
    require(
        columns.windows(2).all(|w| w[0] < w[1]),
        "nomination columns not canonical",
    )?;
    require(
        inventory.is_complete_ordinary()
            && inventory.len() == inventory.completed_source_row_count()
            && inventory.context_fingerprint() == c.fingerprint(),
        "raw ordinary inventory scope differs",
    )?;
    for (i, source) in inventory.sources().iter().enumerate() {
        require(
            source.provenance().source_ordinal() == i
                && matches!(source.row_id(), RowId::OrdinaryIbp { .. })
                && source
                    .provenance()
                    .offset()
                    .values()
                    .iter()
                    .all(|&v| v == 0),
            "nomination requires complete zero-offset raw inventory before fixed specialization",
        )?;
        for (shift, value) in source.terms() {
            require(
                shift.values().len() == c.index_count(),
                "raw shift arity differs",
            )?;
            checked(c.validate_with_limits(value, limits.arithmetic))?;
            require(!value.is_zero(), "explicit zero in raw ordinary support")?;
        }
    }
    for value in values {
        checked(c.validate_with_limits(value, limits.arithmetic))?;
    }
    let support = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| (!v.is_zero()).then_some(i))
        .collect::<Vec<_>>();
    require(!support.is_empty(), "empty separator support")?;
    let raw_terms = inventory.sources().iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row.terms().len())
            .ok_or("raw support count overflow")
    })?;
    let pairs = raw_terms
        .checked_mul(support.len())
        .ok_or("preimage pair count overflow")?;
    checked(project::bound(
        pairs,
        limits.operations,
        "raw preimage enumeration work",
    ))?;
    checked(project::bound(
        pairs,
        limits.nonzeros,
        "raw preimage binding storage",
    ))?;
    checked(project::bound(
        pairs
            .checked_mul(c.index_count())
            .ok_or("preimage coordinate overflow")?,
        coordinate_cap,
        "raw preimage coordinates",
    ))?;
    let mut found = Census::new();
    let mut pin_matches = 0usize;
    for source in inventory.sources() {
        for tau in source.terms().keys() {
            for &column in &support {
                let offset = columns[column]
                    .values()
                    .iter()
                    .zip(tau.values())
                    .map(|(&f, &t)| f.checked_sub(t).ok_or("source preimage offset overflow"))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if i128::from(pin_value) + i128::from(offset[pin]) > 0 {
                    continue;
                }
                pin_matches += 1;
                let request = TranslatedSourceRequest::new(
                    source.provenance().source_ordinal(),
                    checked(IntegralShift::try_new(offset))?,
                );
                found
                    .entry(request)
                    .or_default()
                    .insert((column, tau.clone()));
            }
        }
    }
    Ok((found, pin_matches))
}

fn pairing(
    c: &IndexedCoefficientContext,
    row: &project::Row,
    columns: &[IndexShift],
    values: &[rustred::algebra::IndexedCoefficient],
    guards: &mut Vec<project::Guard>,
    operations: &mut usize,
    limits: project::Limits,
) -> Result<rustred::algebra::IndexedCoefficient> {
    require(
        columns.len() == values.len(),
        "pairing witness shape differs",
    )?;
    let mut sum = c.zero();
    for value in values {
        checked(project::denominator(
            c,
            guards,
            value,
            "nomination separator denominator",
            limits,
        ))?;
    }
    for (shift, value) in row {
        checked(c.validate_with_limits(value, limits.arithmetic))?;
        checked(project::denominator(
            c,
            guards,
            value,
            "nomination full image denominator",
            limits,
        ))?;
        if let Ok(i) = columns.binary_search(shift) {
            *operations = operations
                .checked_add(2)
                .ok_or("nomination operation overflow")?;
            checked(project::bound(
                *operations,
                limits.operations,
                "all nomination pairings",
            ))?;
            let product = checked(c.mul_with_limits(value, &values[i], limits.arithmetic))?;
            checked(project::denominator(
                c,
                guards,
                &product,
                "nomination pairing product",
                limits,
            ))?;
            sum = checked(c.add_with_limits(&sum, &product, limits.arithmetic))?;
            checked(project::denominator(
                c,
                guards,
                &sum,
                "nomination pairing sum",
                limits,
            ))?;
        }
    }
    Ok(sum)
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
