//! Optimized, generation-free finite numerator-symmetry cohort experiment.
//! All algebra, witnessed transport, saved-rule application, guards and sparse
//! elimination use existing RustRed/Symbolica public services.
use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use rustred::algebra::{ExactAlgebraLimits, IndexedAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::persistence::{
    BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientTableBuilder, SectionTag,
    encode_program,
};
use rustred::reduction::ReductionLimits;
use rustred::reduction::terminal_normalization::{
    TerminalNormalizationLimits, TerminalNumeratorSymmetryEquations,
};
use rustred::reduction::terminal_relations::{
    TerminalEquation, TerminalRelationLimits, TerminalRelationRow, TerminalRelationSession,
};
use rustred::sector::symmetry::integral_transport::ExpansionLimits;
use rustred::solver::RoutedCandidateLimits;
use rustred_app::{RoutedCampaignRequest, prepare_saved_integral_equations};
use serde_json::{Value, json};

type R<T> = Result<T, Box<dyn Error>>;

fn limits() -> TerminalRelationLimits {
    TerminalRelationLimits {
        max_seeds: usize::MAX,
        max_columns: usize::MAX,
        max_rows: usize::MAX,
        max_nonzeros: usize::MAX,
        normalization: TerminalNormalizationLimits::default(),
        algebra: IndexedAlgebraLimits::unbounded_resources(),
    }
}

fn bytes_hash(path: &Path) -> R<(Vec<u8>, String)> {
    let data = fs::read(path)?;
    let hash = blake3::hash(&data).to_hex().to_string();
    Ok((data, hash))
}

fn atomic(path: &Path, bytes: &[u8]) -> R<()> {
    let temporary = path.with_extension("partial");
    let mut output = File::create(&temporary)?;
    use std::io::Write;
    output.write_all(bytes)?;
    output.sync_all()?;
    drop(output);
    fs::rename(temporary, path)?;
    File::open(path.parent().ok_or("missing parent")?)?.sync_all()?;
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> R<()> {
    atomic(path, &serde_json::to_vec_pretty(value)?)
}

fn row_display(row: &TerminalRelationRow) -> Value {
    json!(
        row.iter()
            .map(|(key, value)| json!({
                "key":key.powers(), "coefficient_display":value.to_string(),
            }))
            .collect::<Vec<_>>()
    )
}

/// Existing native Symbolica coefficient transport, not expression strings.
/// The tiny structural program uses JSON only for integers/coefficient IDs.
fn encode_equation(equation: &TerminalEquation, proof: &Value) -> R<Vec<u8>> {
    let io = BinaryIoLimits::unbounded_resources();
    let mut table = CoefficientTableBuilder::new(io);
    let mut terms = Vec::new();
    for (key, value) in &equation.terms {
        terms.push(json!({"key":key.powers(),"coefficient":table.intern(value)?.index()}));
    }
    let mut conditions = Vec::new();
    for condition in &equation.nonzero_conditions {
        conditions.push(table.intern(condition)?.index());
    }
    let program = serde_json::to_vec(&json!({
        "schema":"rustred.finite-symmetry-row-evidence.v1",
        "terms":terms,"conditions":conditions,"proof":proof,
    }))?;
    let table = table.finish()?;
    Ok(encode_program(
        BinaryProgramKind::TerminalRelations,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &table.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &table.atoms,
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &program,
            },
        ],
        io,
    )?)
}

fn rank(key: &IntegralKey) -> u128 {
    key.powers()
        .iter()
        .filter(|&&n| n < 0)
        .map(|n| u128::from(n.unsigned_abs()))
        .sum()
}

fn main() -> R<()> {
    let mut args = std::env::args_os().skip(1);
    let baseline_path = PathBuf::from(args.next().ok_or("baseline.rrbin required")?);
    let plan_path = PathBuf::from(args.next().ok_or("native symmetry plan required")?);
    let source_report_path = PathBuf::from(
        args.next()
            .ok_or("immutable publication artifact.json required")?,
    );
    let selection_path = PathBuf::from(args.next().ok_or("immutable selection.json required")?);
    let output = PathBuf::from(args.next().ok_or("new output directory required")?);
    let mut max_rank = None;
    let mut workers = 1usize;
    let mut selected_source = None;
    let mut fewest_lines_first = false;
    while let Some(option) = args.next() {
        match option.to_str() {
            Some("--max-numerator-degree") => {
                max_rank = Some(
                    args.next()
                        .ok_or("rank missing")?
                        .to_str()
                        .ok_or("non-UTF8 rank")?
                        .parse::<u128>()?,
                )
            }
            Some("--workers") => {
                workers = args
                    .next()
                    .ok_or("workers missing")?
                    .to_str()
                    .ok_or("non-UTF8 workers")?
                    .parse()?
            }
            Some("--source-key") => {
                let path = PathBuf::from(args.next().ok_or("source-key JSON path missing")?);
                let powers: Vec<i64> = serde_json::from_slice(&fs::read(&path)?)?;
                selected_source = Some(IntegralKey::try_new(powers)?);
            }
            Some("--fewest-active-lines-first") => fewest_lines_first = true,
            _ => return Err("unknown option; use --max-numerator-degree R or --workers N".into()),
        }
    }
    if workers == 0 {
        return Err("workers must be positive".into());
    }
    fs::create_dir(&output)?;
    fs::create_dir(output.join("rows"))?;
    let whole = Instant::now();
    let io = BinaryIoLimits::unbounded_resources();
    let (baseline_bytes, baseline_hash) = bytes_hash(&baseline_path)?;
    let mut session = TerminalRelationSession::from_native_bytes(&baseline_bytes, limits(), io)?;
    let baseline_maps = session.apply_all_terminals()?;
    let before_basis = session.remaining_terminals();
    let baseline_stats = session.statistics();
    if baseline_stats.completed_source_rows != 0 || baseline_stats.pending_rebuild_rows != 0 {
        return Err("assay requires a compact, zero-source-cursor relations-only baseline".into());
    }
    let (plan_bytes, plan_hash) = bytes_hash(&plan_path)?;
    let plan = TerminalNumeratorSymmetryEquations::from_native_bytes(
        &plan_bytes,
        Arc::clone(session.family_owner()),
        Default::default(),
        io,
    )?;
    let (source_report_bytes, report_hash) = bytes_hash(&source_report_path)?;
    let source_report: Value = serde_json::from_slice(&source_report_bytes)?;
    let (selection, selection_hash) = bytes_hash(&selection_path)?;
    if source_report["inputs"]["program_binding"].as_str() != Some(selection_hash.as_str()) {
        return Err("publication selection binding differs".into());
    }
    let mut groups = plan
        .input_keys()
        .iter()
        .filter(|key| {
            max_rank.is_none_or(|bound| rank(key) <= bound)
                && selected_source.as_ref().is_none_or(|source| *key == source)
                && !plan.equations(key).is_empty()
        })
        .cloned()
        .collect::<Vec<_>>();
    groups.sort_by_key(|key| {
        let equations = plan.equations(key);
        let terms: usize = equations.iter().map(|row| row.terms.len()).sum();
        let coefficient_terms: usize = equations
            .iter()
            .flat_map(|row| row.terms.values())
            .map(|c| c.numerator.nterms() + c.denominator.nterms())
            .sum();
        let active = if fewest_lines_first {
            key.powers().iter().filter(|&&n| n > 0).count()
        } else {
            0
        };
        let power_excess: u128 = if fewest_lines_first {
            key.powers()
                .iter()
                .filter(|&&n| n > 0)
                .map(|&n| n as u128 - 1)
                .sum()
        } else {
            0
        };
        (
            rank(key),
            active,
            power_excess,
            terms,
            coefficient_terms,
            key.clone(),
        )
    });
    let selected_rows: usize = groups.iter().map(|key| plan.equations(key).len()).sum();
    let binding = format!(
        "finite-cost-ordered-numerator-row-assay-v1:{baseline_hash}:{plan_hash}:{selection_hash}:{max_rank:?}:{selected_source:?}:{fewest_lines_first}"
    );
    session.enable_assistance(binding.clone())?;
    // One provider ticket per target group; each ticket emits only independently
    // witnessed source equations. There are at least as many finite tickets as
    // prepared target groups, since the plan inventory is the baseline union.
    if groups.len() > session.statistics().pending_assistance_keys {
        return Err("provider target inventory exceeds the original finite key union".into());
    }
    let prepared_seconds = whole.elapsed().as_secs_f64();
    let mut summary = json!({
        "schema":"rustred.cost-ordered-numerator-symmetry-assay.v1",
        "status":"preparing_saved_rules", "baseline":baseline_path,"baseline_blake3":baseline_hash,
        "symmetry_certificate":plan_path,"symmetry_certificate_blake3":plan_hash,
        "source_report":source_report_path,"source_report_blake3":report_hash,
        "selection":selection_path,"selection_blake3":selection_hash,
        "authority_binding":binding,"family_fingerprint":session.family_owner().fingerprint(),
        "raw_terminals":session.raw_terminals().len(),"basis_before":before_basis.len(),
        "selected_max_numerator_degree":max_rank,"selected_groups":groups.len(),
        "selected_source":selected_source.as_ref().map(IntegralKey::powers),
        "fewest_active_lines_first":fewest_lines_first,
        "selected_rows":selected_rows,"deferred_rows":plan.statistics().equations-selected_rows,
        "ordinary_source_rows":0,"independent_master_claim":false,
        "timings_seconds":{"baseline_and_native_witness_replay":prepared_seconds},
        "processed_rows":0,"completed_groups":0,"rows":[],
        "profile":"optimized external driver and release-optimized app/core/Symbolica libraries",
    });
    write_json(&output.join("progress.json"), &summary)?;
    eprintln!(
        "{}",
        json!({"stage":"native_prepared","selected_rows":selected_rows,"selected_groups":groups.len(),"basis_before":before_basis.len(),"seconds":prepared_seconds})
    );
    let mut request = RoutedCampaignRequest::new(String::from_utf8(selection)?, String::new());
    request.owner_base = selection_path
        .parent()
        .ok_or("selection parent missing")?
        .to_path_buf();
    request.workers = workers;
    request.reduction_limits = ReductionLimits {
        exact_algebra: ExactAlgebraLimits::unbounded_resources(),
        indexed_algebra: IndexedAlgebraLimits::unbounded_resources(),
        max_rule_applications: usize::MAX,
        max_cached_integrals: usize::MAX,
        max_cached_coefficient_terms: usize::MAX,
        max_cached_coefficient_bytes: usize::MAX,
        max_pending_frames: usize::MAX,
        max_coalescing_additions: usize::MAX,
        max_factorization_terms: usize::MAX,
    };
    request.trace_limits = RoutedCandidateLimits {
        max_input_targets: usize::MAX,
        max_unique_nodes: usize::MAX,
        max_transport_calls: usize::MAX,
        max_transport_operations: usize::MAX,
        max_transport_endpoints: usize::MAX,
        expansion: ExpansionLimits::unbounded_resources(),
    };
    let digests = source_report["inputs"]["payloads"]
        .as_array()
        .ok_or("payload inventory missing")?
        .iter()
        .map(|row| {
            row["blake3"]
                .as_str()
                .ok_or("payload digest missing")
                .map(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cancel = AtomicBool::new(false);
    let load_start = Instant::now();
    let mut saved = prepare_saved_integral_equations(
        &request,
        session.family_owner().fingerprint(),
        &digests,
        &cancel,
        &|event| {
            eprintln!(
                "{}",
                json!({"stage":"saved_preparation","elapsed_seconds":whole.elapsed().as_secs_f64(),"native":event})
            )
        },
    )?;
    summary["timings_seconds"]["saved_rule_load"] = json!(load_start.elapsed().as_secs_f64());
    let mut receipts = Vec::new();
    let mut exact_zeros = 0usize;
    let mut failed = 0usize;
    let mut processed = 0usize;
    let mut last_save = Instant::now();
    for (group_index, key) in groups.iter().enumerate() {
        let mut reduced_rows = Vec::new();
        for (ordinal, equation) in plan.equations(key).iter().enumerate() {
            let row_started = Instant::now();
            eprintln!(
                "{}",
                json!({"stage":"row_started","completed_rows":processed,"total_rows":selected_rows,
                "source":key.powers(),"numerator_degree":rank(key),"witness":ordinal,"terms":equation.terms.len(),"basis":session.remaining_terminals().len()})
            );
            let (row, diagnostic) = match saved.reduce_equation(equation) {
                Ok(row) => (row, None),
                Err(error) => {
                    failed += 1;
                    (
                        equation.clone(),
                        Some(
                            json!({"kind":error.kind(),"message":error.to_string(),"fallback":"original exact symmetry row; unresolved columns are not new masters"}),
                        ),
                    )
                }
            };
            let proof = json!({"symmetry_certificate_blake3":plan_hash,"source":key.powers(),"witness":ordinal,
                "selection_blake3":selection_hash,"saved_reduction_failure":diagnostic,
                "authority":"exact loop-map source plus existing candidate-rule application; not independent ordinary-IBP source replay"});
            let native_row = encode_equation(&row, &proof)?;
            let row_hash = blake3::hash(&native_row).to_hex().to_string();
            atomic(
                &output.join("rows").join(format!("{row_hash}.rrbin")),
                &native_row,
            )?;
            let is_zero = row.terms.is_empty();
            exact_zeros += usize::from(is_zero);
            let mut receipt = proof;
            receipt["row_blake3"] = json!(row_hash);
            receipt["output_terms"] = json!(row.terms.len());
            receipt["guards"] = json!(row.nonzero_conditions.len());
            receipt["exact_zero"] = json!(is_zero);
            receipt["seconds"] = json!(row_started.elapsed().as_secs_f64());
            receipts.push(receipt);
            // Exact native equality only. Conditions already held by the
            // baseline remain present there; do not feed O(G²) duplicates to
            // the old assisted-row Vec membership boundary.
            let mut row = row;
            let existing: HashSet<_> = session.nonzero_conditions().iter().collect();
            row.nonzero_conditions
                .retain(|guard| !existing.contains(guard));
            drop(existing);
            // Empty homogeneous rows may still carry additional applicability
            // conditions; keep them so no source/rule guard is discarded.
            if !row.terms.is_empty() || !row.nonzero_conditions.is_empty() {
                reduced_rows.push(row);
            }
            processed += 1;
            summary["processed_rows"] = json!(processed);
            summary["rows"] = json!(receipts);
            summary["status"] = json!("reducing_complete_symmetry_rows");
            write_json(&output.join("progress.json"), &summary)?;
        }
        let before = session.statistics();
        // The pending key is a deterministic finite job ticket, not a new
        // physical source. Proof rows retain their actual source key above.
        session.step_with_provider(&cancel, |_| Ok(reduced_rows.clone()))?;
        while session.statistics().pending_assistance_rows != 0 {
            session.step_with_provider(&cancel, |_| {
                unreachable!("row drain may not query another ticket")
            })?;
        }
        let after = session.statistics();
        if after.completed_source_rows != 0 {
            return Err("ordinary IBP cursor advanced".into());
        }
        summary["completed_groups"] = json!(group_index + 1);
        summary["basis_after"] = json!(session.remaining_terminals().len());
        summary["additional_dependencies"] =
            json!(before_basis.len() - session.remaining_terminals().len());
        summary["failed_saved_reductions"] = json!(failed);
        summary["exact_zero_rows"] = json!(exact_zeros);
        if after.independent_rows > before.independent_rows
            || last_save.elapsed() >= Duration::from_secs(60)
        {
            let data = session.to_native_bytes(io)?;
            atomic(&output.join("numerator-symmetry-result.rrbin"), &data)?;
            summary["native_checkpoint"] = json!({"file":"numerator-symmetry-result.rrbin","blake3":blake3::hash(&data).to_hex().to_string(),"completed_groups":group_index+1,"bytes":data.len()});
            last_save = Instant::now();
        }
        write_json(&output.join("progress.json"), &summary)?;
        eprintln!(
            "{}",
            json!({"stage":"group_completed","processed_rows":processed,"total_rows":selected_rows,
            "basis":session.remaining_terminals().len(),"additional_dependencies":summary["additional_dependencies"],"elapsed_seconds":whole.elapsed().as_secs_f64()})
        );
    }
    while session.statistics().pending_assistance_keys != 0
        || session.statistics().pending_assistance_rows != 0
    {
        session.step_with_provider(&cancel, |_| Ok(Vec::new()))?;
    }
    if session.statistics().completed_source_rows != 0 {
        return Err("ordinary IBP cursor advanced".into());
    }
    let after_maps = session.apply_all_terminals()?;
    let after_basis = session.remaining_terminals();
    for row in after_maps.values() {
        if row.keys().any(|key| !after_basis.contains(key)) {
            return Err("auxiliary escaped output basis".into());
        }
    }
    let result = session.to_native_bytes(io)?;
    let cold = TerminalRelationSession::from_native_bytes(&result, limits(), io)?;
    if cold.apply_all_terminals()? != after_maps {
        return Err("cold result changed exact maps".into());
    }
    atomic(&output.join("numerator-symmetry-result.rrbin"), &result)?;
    summary["native_checkpoint"] = json!({"file":"numerator-symmetry-result.rrbin","blake3":blake3::hash(&result).to_hex().to_string(),"bytes":result.len(),"completed_groups":groups.len()});
    summary["changed_raw_maps"] = json!(
        after_maps
            .iter()
            .filter(|(key, row)| baseline_maps.get(*key) != Some(*row))
            .count()
    );
    summary["maps_display"] = json!(after_maps.iter().filter(|(key,row)|baseline_maps.get(*key)!=Some(*row)).map(|(key,row)|json!({"target":key.powers(),"before":row_display(&baseline_maps[key]),"after":row_display(row)})).collect::<Vec<_>>());
    for (path, hash) in [
        (&baseline_path, &baseline_hash),
        (&plan_path, &plan_hash),
        (&selection_path, &selection_hash),
        (&source_report_path, &report_hash),
    ] {
        if bytes_hash(path)?.1 != *hash {
            return Err("immutable source file changed".into());
        }
    }
    summary["status"] = json!("completed_selected_finite_cohort");
    summary["source_hashes_unchanged"] = json!(true);
    summary["all_original_keys_defined"] = json!(true);
    summary["all_outputs_remaining_terminals"] = json!(true);
    summary["timings_seconds"]["total"] = json!(whole.elapsed().as_secs_f64());
    write_json(&output.join("progress.json"), &summary)?;
    write_json(&output.join("result.json"), &summary)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
