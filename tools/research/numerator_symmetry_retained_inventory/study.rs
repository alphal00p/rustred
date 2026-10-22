//! Isolated public-core RAW symmetry assay. No candidate or ordinary IBP work.
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustred::algebra::{ExactAlgebraLimits, IndexedAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::persistence::BinaryIoLimits;
use rustred::reduction::terminal_normalization::{
    TerminalNormalizationLimits, TerminalNumeratorSymmetryEquations,
};
use rustred::reduction::terminal_relations::{
    TerminalRelationLimits, TerminalRelationRow, TerminalRelationSession,
};
use serde_json::{Value, json};
type E = Box<dyn Error + Send + Sync>;
type R<T> = Result<T, E>;

fn limits(profile: &str) -> R<TerminalRelationLimits> {
    // Preserve the baseline's named normalization semantics exactly.
    // A looser preparation recipe can select different aliases and is not a
    // valid cold replay of this already-published finite convention.
    let mut normalization = TerminalNormalizationLimits::default();
    match profile {
        "conservative" | "conservative-v1" => {}
        "standard" | "standard-v1" => {
            let symanzik = &mut normalization.parametric.symanzik;
            symanzik.max_polynomial_terms = 4_000_000;
            symanzik.max_exponent_entries = 64_000_000;
            symanzik.max_term_operations = 16_000_000;
            symanzik.max_determinant_ring_operations = 16_000_000;
        }
        _ => return Err("profile must be conservative or standard".into()),
    }
    Ok(TerminalRelationLimits {
        max_seeds: usize::MAX,
        max_columns: usize::MAX,
        max_rows: usize::MAX,
        max_nonzeros: usize::MAX,
        normalization,
        algebra: IndexedAlgebraLimits::unbounded_resources(),
    })
}
fn stat(s: &TerminalRelationSession) -> Value {
    let x = s.statistics();
    json!({"raw":x.raw_terminals,"normalized":x.normalized_terminals,"remaining":x.remaining_terminals,
        "source_rows":x.completed_source_rows,"independent_rows":x.independent_rows,"auxiliary_columns":x.auxiliary_columns,
        "columns":x.columns,"nonzeros":x.nonzeros,"pending_keys":x.pending_assistance_keys,"pending_rows":x.pending_assistance_rows,
        "processed_keys":x.completed_assistance_keys,"processed_rows":x.completed_assistance_rows,"guards":s.nonzero_conditions().len()})
}
fn view(row: &TerminalRelationRow) -> Value {
    json!(
        row.iter()
            .map(|(k, c)| json!({"key":k.powers(),"coefficient_display":c.to_string()}))
            .collect::<Vec<_>>()
    )
}
fn write_json(path: &Path, v: &Value) -> R<()> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    serde_json::to_writer_pretty(file, v)?;
    Ok(())
}
fn save(s: &TerminalRelationSession, path: &Path) -> R<()> {
    let bytes = s.to_native_bytes(BinaryIoLimits::unbounded_resources())?;
    let temporary = path.with_extension("rrbin.tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)?;
    Ok(())
}
fn main() -> R<()> {
    let mut args = std::env::args_os().skip(1);
    let source =
        PathBuf::from(args.next().ok_or(
            "expected baseline provider new-output stop-file profile [target-powers-JSON]",
        )?);
    let provider_path = PathBuf::from(args.next().ok_or("missing provider")?);
    let output = PathBuf::from(args.next().ok_or("missing new output")?);
    let stop_file = PathBuf::from(args.next().ok_or("missing stop file")?);
    let profile = args
        .next()
        .ok_or("missing original normalization profile")?
        .into_string()
        .map_err(|_| "non-UTF8 profile")?;
    let limits = limits(&profile)?;
    let target = args
        .next()
        .map(|value| -> R<IntegralKey> {
            let text = value.into_string().map_err(|_| "non-UTF8 target JSON")?;
            Ok(IntegralKey::try_new(serde_json::from_str::<Vec<i64>>(
                &text,
            )?)?)
        })
        .transpose()?;
    if args.next().is_some() || output.exists() {
        return Err("extra argument or output exists".into());
    }
    fs::create_dir(&output)?;
    let started = Instant::now();
    let before_bytes = fs::read(&source)?;
    let before_hash = blake3::hash(&before_bytes).to_hex().to_string();
    let mut session = TerminalRelationSession::from_native_bytes(
        &before_bytes,
        limits,
        BinaryIoLimits::unbounded_resources(),
    )?;
    let family = Arc::clone(session.family_owner());
    let before_stat = stat(&session);
    let before_maps = session.apply_all_terminals()?;
    let before_guards: HashSet<_> = session.nonzero_conditions().iter().cloned().collect();
    let target_before = target
        .as_ref()
        .and_then(|key| before_maps.get(key))
        .map(view);
    let provider_bytes = fs::read(&provider_path)?;
    let provider_hash = blake3::hash(&provider_bytes).to_hex().to_string();
    let plan = TerminalNumeratorSymmetryEquations::from_native_bytes(
        &provider_bytes,
        Arc::clone(&family),
        Default::default(),
        BinaryIoLimits::unbounded_resources(),
    )?;
    if plan.input_keys() != session.raw_terminals() {
        return Err("provider inventory differs from baseline raw keys".into());
    }
    let prepared_s = started.elapsed().as_secs_f64();
    write_json(
        &output.join("inputs.json"),
        &json!({"baseline":source,"baseline_blake3":before_hash,
        "provider":provider_path,"provider_blake3":provider_hash,"before":before_stat,"prepare_s":prepared_s,
        "family":family.fingerprint(),"provider_keys":plan.input_keys().len(),"provider_equations":plan.statistics().equations,
        "target_key":target.as_ref().map(IntegralKey::powers),"target_before":target_before,"normalization_profile":profile,
        "new_ibp_generation":false,"saved_rule_application":false}),
    )?;
    session.enable_assistance(format!("raw-numerator-symmetry-assay-v1:{provider_hash}"))?;
    let cancel = AtomicBool::new(false);
    let mut last_progress = Instant::now();
    let mut last_save = Instant::now();
    loop {
        let stats = session.statistics();
        if stats.pending_assistance_keys == 0 && stats.pending_assistance_rows == 0 {
            break;
        }
        if stop_file.exists() {
            cancel.store(true, Ordering::Relaxed);
            break;
        }
        session.step_with_provider(&cancel, |key| Ok(plan.equations(key).to_vec()))?;
        if session.statistics().completed_source_rows != 0 {
            return Err("ordinary IBP cursor advanced".into());
        }
        if last_progress.elapsed() >= Duration::from_secs(1) {
            println!(
                "{}",
                json!({"stage":"raw_symmetry_ingestion","elapsed_s":started.elapsed().as_secs_f64(),"statistics":stat(&session)})
            );
            last_progress = Instant::now();
        }
        if last_save.elapsed() >= Duration::from_secs(60) {
            save(&session, &output.join("checkpoint.rrbin"))?;
            last_save = Instant::now();
        }
    }
    save(&session, &output.join("after.rrbin"))?;
    let after_maps = session.apply_all_terminals()?;
    let after_basis = session.remaining_terminals();
    let c = family.coefficient_context();
    let algebra = ExactAlgebraLimits::unbounded_resources();
    let mut products = 0usize;
    let mut changed = 0usize;
    for (key, row) in &before_maps {
        let mut composed = BTreeMap::new();
        for (middle, coefficient) in row {
            let replacement = after_maps
                .get(middle)
                .ok_or("old output missing from raw after map")?;
            for (target, factor) in replacement {
                let value = c.try_mul(coefficient, factor, algebra)?;
                let old = composed.remove(target).unwrap_or_else(|| c.zero());
                let value = c.try_add(&old, &value, algebra)?;
                if !value.is_zero() {
                    composed.insert(target.clone(), value);
                }
                products += 1;
            }
        }
        if composed != after_maps[key] {
            return Err("old map invalidated".into());
        }
        if after_maps[key].keys().any(|key| !after_basis.contains(key)) {
            return Err("unresolved leaf escaped after map".into());
        }
        changed += usize::from(before_maps[key] != after_maps[key]);
    }
    let after_guards: HashSet<_> = session.nonzero_conditions().iter().cloned().collect();
    if !before_guards.is_subset(&after_guards) {
        return Err("old guard dropped".into());
    }
    let after_bytes = fs::read(output.join("after.rrbin"))?;
    let cold = TerminalRelationSession::from_native_bytes(
        &after_bytes,
        limits,
        BinaryIoLimits::unbounded_resources(),
    )?;
    if cold.apply_all_terminals()? != after_maps
        || cold.nonzero_conditions() != session.nonzero_conditions()
    {
        return Err("cold map or guard mismatch".into());
    }
    if fs::read(&source)? != before_bytes || fs::read(&provider_path)? != provider_bytes {
        return Err("an input changed".into());
    }
    let summary = json!({"status":if cancel.load(Ordering::Relaxed){"paused"}else{"completed_raw_symmetry_worklist"},
        "before":before_stat,"after":stat(&session),"changed_raw_maps":changed,"raw_maps_composed":before_maps.len(),
        "exact_coefficient_products":products,"guards_retained":before_guards.len(),"target_key":target.as_ref().map(IntegralKey::powers),
        "target_before":target_before,"target_after":target.as_ref().and_then(|key|after_maps.get(key)).map(view),"new_ibp_generation":false,
        "saved_rule_application":false,"all_inputs_unchanged":true,"cold_verification":true,
        "prepare_s":prepared_s,"wall_s":started.elapsed().as_secs_f64(),"family_closure_claim":false,"minimality_claim":false});
    write_json(&output.join("summary.json"), &summary)?;
    println!("{summary}");
    Ok(())
}
