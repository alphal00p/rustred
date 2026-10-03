//! Observation-only modular support nominations from the unchanged exact bank.
use super::*;
use rustred::foundry::modular_nomination::{self as native, PreparedOrdinaryNomination};
use rustred::{algebra::IndexedCoefficientContext, identity::CompletedIbpSourceRows};

pub(super) fn validate(r: &Value, arity: usize) -> Result<()> {
    let v = &r["modular_nomination"];
    let max_samples = number(v, "max_samples")?;
    require(
        max_samples > 0,
        "positive modular sample allowance required",
    )?;
    require(
        number(v, "max_reducer_dense_scan_work")? > 0,
        "positive modular scan allowance required",
    )?;
    let samples = array(v, "samples")?;
    require(
        !samples.is_empty() && samples.len() <= max_samples,
        "modular samples outside allowance",
    )?;
    for sample in samples {
        require(
            sample["prime"].as_u64().is_some(),
            "modular prime must be u64",
        )?;
        let base = sample["base_parameter_residues"]
            .as_object()
            .ok_or("named base residues required")?;
        require(
            base.values().all(|v| v.as_u64().is_some()),
            "base residues must be u64",
        )?;
        let indices = integers(&sample["physical_indices"], arity)?;
        in_chart(r, &indices)?;
    }
    Ok(())
}

fn in_chart(r: &Value, indices: &[i64]) -> Result<()> {
    let mask = r["owner_mask"]
        .as_str()
        .ok_or("owner mask required")?
        .as_bytes();
    require(mask.len() == indices.len(), "sample index arity differs")?;
    for (axis, &index) in indices.iter().enumerate() {
        require(
            (index > 0) == (mask[axis] == b'1'),
            "sample support differs from frozen chart",
        )?;
        let local = if index > 0 {
            i128::from(index) - 1
        } else {
            -i128::from(index)
        };
        let lower = r["chart"]["lower"][axis]
            .as_u64()
            .ok_or("invalid chart lower")?;
        require(local >= i128::from(lower), "sample below frozen chart")?;
        if let Some(upper) = r["chart"]["upper"][axis].as_u64() {
            require(local <= i128::from(upper), "sample above frozen chart")?;
        }
    }
    for (axis, value) in fixed(r)? {
        require(
            indices[axis] == value,
            "sample differs from exact fixed face",
        )?;
    }
    Ok(())
}

fn policy(r: &Value, arity: usize) -> Result<native::Limits> {
    let terms = limit(r, "max_terms")?;
    let guards = limit(r, "max_conditions")?;
    let rows = limit(r, "max_source_rows")?;
    let coords = limit(r, "max_coordinate_cells")?;
    let entries = limit(r, "max_matrix_nonzeros")?;
    let columns = limit(r, "max_augmented_columns")?;
    let mut p = native::Limits::default();
    p.max_plan_coordinate_cells = coords;
    p.corpus.max_source_rows = limit(r, "max_complete_source_rows")?;
    p.corpus.max_point_coordinates = coords;
    p.corpus.max_conditions_per_source = guards;
    p.corpus.max_terms_per_source = terms;
    p.corpus.max_scalar_outputs_per_source = terms
        .checked_mul(2)
        .and_then(|n| n.checked_add(guards))
        .ok_or("modular scalar allowance overflow")?;
    p.corpus.max_polynomial_terms_per_source = limit(r, "max_coefficient_terms")?;
    p.corpus.max_corpus_scalar_outputs = p.corpus.max_scalar_outputs_per_source;
    p.corpus.max_corpus_polynomial_terms = limit(r, "max_coefficient_terms")?;
    p.corpus.max_shift_coordinate_cells_per_source = coords;
    p.kernel.max_rows = rows;
    p.kernel.max_post_hit_rows = 0;
    p.kernel.max_request_shift_components = arity;
    p.kernel.max_forbidden_columns = columns;
    p.kernel.max_structural_terms_per_row = terms;
    p.kernel.max_structural_terms = terms;
    p.kernel.max_retained_nonzeros = entries;
    p.kernel.max_reducer_scratch_cells = columns
        .checked_mul(2)
        .and_then(|n| n.checked_add(2))
        .ok_or("modular scratch allowance overflow")?;
    p.kernel.max_reducer_dense_scan_work =
        number(&r["modular_nomination"], "max_reducer_dense_scan_work")?;
    p.kernel.max_reducer_entries = entries;
    p.kernel.max_trace_nodes = rows;
    p.kernel.max_trace_edges = entries;
    p.kernel.max_retained_request_shift_components = coords;
    p.kernel.max_support_requests = rows;
    p.kernel.max_dependency_order_requests = rows;
    p.kernel.max_hit_trace_edges = entries;
    p.kernel.max_hit_request_shift_components = coords;
    Ok(p)
}

fn stats(s: &native::Stats) -> Value {
    json!({"rows_evaluated":s.rows_evaluated,"structural_terms_evaluated":s.structural_terms_evaluated,
        "registered_forbidden_columns":s.registered_forbidden_columns,"forbidden_rank":s.forbidden_rank,"augmented_rank":s.augmented_rank})
}

pub(super) fn run(
    r: &Value,
    context: &IndexedCoefficientContext,
    completed: &CompletedIbpSourceRows,
    requested: &[TranslatedSourceRequest],
    forbidden: &BTreeSet<IndexShift>,
    target: &IndexShift,
) -> Result<Value> {
    validate(r, context.index_count())?;
    let policy = policy(r, context.index_count())?;
    let restrictions = fixed(r)?
        .into_iter()
        .map(|(a, v)| FixedIndexRestriction::new(a, v))
        .collect::<Vec<_>>();
    let prepared = checked(PreparedOrdinaryNomination::try_new(
        context,
        completed,
        requested,
        forbidden,
        target,
        &restrictions,
        policy,
    ))?;
    let names = context.base().parameter_names();
    // Check EVERY explicit sample before the first kernel invocation. Chart
    // membership does not turn sampled residues into exact proof authority.
    let samples = array(&r["modular_nomination"], "samples")?
        .iter()
        .map(|s| {
            let named = s["base_parameter_residues"]
                .as_object()
                .ok_or("named base residues required")?;
            require(
                named.len() == names.len() && names.iter().all(|name| named.contains_key(name)),
                "sample base parameter names differ",
            )?;
            let residues = names
                .iter()
                .map(|name| {
                    named[name]
                        .as_u64()
                        .ok_or_else(|| "invalid base residue".into())
                })
                .collect::<Result<Vec<_>>>()?;
            Ok((
                s["prime"].as_u64().unwrap(),
                residues,
                integers(&s["physical_indices"], context.index_count())?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut reports = Vec::with_capacity(samples.len());
    for (ordinal, (prime, base, indices)) in samples.iter().enumerate() {
        progress::event(
            "modular_sample_start",
            || json!({"sample":ordinal,"prime":prime,"bank_rows":requested.len(),"f_size":forbidden.len()}),
        );
        let started = Instant::now();
        let outcome = prepared.sample(*prime, base, indices);
        let mut report = match outcome {
            Ok(native::Outcome::TargetSupport(hit)) => {
                require(
                    hit.requests.len() == hit.input_ordinals.len()
                        && hit
                            .requests
                            .iter()
                            .zip(hit.input_ordinals.iter())
                            .all(|(source, &i)| requested.get(i) == Some(source)),
                    "native nominated support differs from frozen request positions",
                )?;
                json!({"status":"SAMPLED_TARGET_SUPPORT","stats":stats(&hit.stats),
                "input_ordinals":hit.input_ordinals,"support_count":hit.requests.len(),"trace_edges":hit.trace_edges,
                "nominated_sources":hit.input_ordinals.iter().map(|&i| r["sources"][i].clone()).collect::<Vec<_>>(),
                "numeric_weights_returned":false,"exact_complete_f_lift_required":true})
            }
            Ok(native::Outcome::SampledMiss(s)) => {
                json!({"status":"SAMPLED_MISS","stats":stats(&s),"exact_miss_claim":false})
            }
            Ok(native::Outcome::UnluckySample {
                input_ordinal,
                cause,
                stats: s,
            }) => {
                json!({"status":"UNLUCKY_SAMPLE","input_ordinal":input_ordinal,"cause":format!("{cause:?}"),"stats":stats(&s),"exact_miss_claim":false})
            }
            Err(error) => {
                json!({"status":"NOMINATION_REFUSED","detail":error.to_string(),"exact_miss_claim":false})
            }
        };
        report["sample_ordinal"] = json!(ordinal);
        report["sample"] = r["modular_nomination"]["samples"][ordinal].clone();
        report["seconds"] = json!(started.elapsed().as_secs_f64());
        progress::event(
            "modular_sample_finished",
            || json!({"sample":ordinal,"status":report["status"],"stats":report["stats"],"seconds":report["seconds"]}),
        );
        reports.push(report);
    }
    Ok(
        json!({"schema":SCHEMA,"status":"MODULAR_NOMINATION_OBSERVATIONS","request":r,
        "samples":reports,"base_parameter_order":names,"native_policy":format!("{policy:?}"),
        "full_forbidden_shifts":forbidden.iter().map(|s|s.values()).collect::<Vec<_>>(),
        "frozen_bank_rows":requested.len(),"borrowed_ordinary_corpus_reused":true,
        "source_proof_passed":false,"export_authorized":false,"sample_zero_is_not_source_deletion":true,
        "exact_shortlist_policy":"Regenerate original rows; retain complete F including structural zero columns, all explicit input assumptions, and unchanged chart/order; then exact replay/proof/export.",
        "production_modified":false,"recursive_walk":false,"coverage_or_cost_claim":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(r: &mut Value) {
        r["modular_nomination"] = json!({"max_samples":3,"max_reducer_dense_scan_work":100000,
            "samples":[{"prime":101,"base_parameter_residues":{"d":7},"physical_indices":[3]},
                       {"prime":103,"base_parameter_residues":{"d":11},"physical_indices":[4]}]});
    }
    #[test]
    fn same_bank_samples_reuse_corpus_and_never_export() {
        let (bytes, mut r) = crate::tests::tadpole();
        config(&mut r);
        r["forbid_cofinally_higher_columns"] = json!(true);
        let (report, artifact) = run_mode::<1>(&bytes, &r, false, true).unwrap();
        assert!(artifact.is_none());
        assert_eq!(report["source_proof_passed"], false);
        assert_eq!(report["full_forbidden_shifts"], json!([[1]]));
        for sample in report["samples"].as_array().unwrap() {
            assert_eq!(sample["status"], "SAMPLED_TARGET_SUPPORT");
            assert_eq!(sample["input_ordinals"], json!([1]));
            assert_eq!(sample["nominated_sources"], json!([r["sources"][1]]));
        }
        assert!(crate::run::<1>(&bytes, &r, true).is_err());
        assert!(crate::run::<1>(&bytes, &r, false).is_err());
    }
    #[test]
    fn samples_must_obey_exact_chart_names_and_bounds() {
        let (bytes, mut r) = crate::tests::tadpole();
        config(&mut r);
        r["modular_nomination"]["samples"][0]["physical_indices"] = json!([1]);
        assert!(validate(&r, 1).is_err());
        config(&mut r);
        r["modular_nomination"]["samples"][0]["base_parameter_residues"] = json!({"foreign":7});
        assert!(run_mode::<1>(&bytes, &r, false, true).is_err());
        config(&mut r);
        r["modular_nomination"]["max_samples"] = json!(1);
        assert!(validate(&r, 1).is_err());
    }
}
