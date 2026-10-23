//! Isolated generation-free exact lower-loop symmetry experiment.
//! Compile against the tested release core/app libraries; see README.md.
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rustred::algebra::{Coefficient, ExactAlgebraLimits, IndexedAlgebraLimits};
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::foundry::artifact::ClosedArtifact;
use rustred::persistence::BinaryIoLimits;
use rustred::reduction::terminal_normalization::{
    TerminalNormalizationLimits, TerminalNormalizationPlan, TerminalNumeratorSymmetryEquations,
    TerminalNumeratorSymmetryLimits, VacuumFamilyAliasPlan,
};
use rustred::reduction::terminal_relations::{
    TerminalEquation, TerminalRelationLimits, TerminalRelationRow, TerminalRelationSession,
};
use rustred::reduction::{Reducer, ReductionLimits};
use rustred_app::{CandidateBundleLimits, load_generated_candidate_bundle};
use serde_json::{Value, json};

type E = Box<dyn Error + Send + Sync>;
type R<T> = Result<T, E>;

fn io() -> BinaryIoLimits {
    BinaryIoLimits {
        max_program_bytes: usize::MAX,
        max_sections: usize::MAX,
        max_collection_entries: usize::MAX,
        max_state_bytes: usize::MAX,
        max_atom_bytes: usize::MAX,
        max_total_atom_bytes: usize::MAX,
        exact_algebra: ExactAlgebraLimits::unbounded_resources(),
    }
}

fn reduction_limits() -> ReductionLimits {
    ReductionLimits {
        exact_algebra: ExactAlgebraLimits::unbounded_resources(),
        indexed_algebra: IndexedAlgebraLimits::unbounded_resources(),
        max_rule_applications: usize::MAX,
        max_cached_integrals: usize::MAX,
        max_cached_coefficient_terms: usize::MAX,
        max_cached_coefficient_bytes: usize::MAX,
        max_pending_frames: usize::MAX,
        max_coalescing_additions: usize::MAX,
        max_factorization_terms: usize::MAX,
    }
}

fn relation_limits() -> TerminalRelationLimits {
    TerminalRelationLimits {
        max_seeds: usize::MAX,
        max_columns: usize::MAX,
        max_rows: usize::MAX,
        max_nonzeros: usize::MAX,
        normalization: TerminalNormalizationLimits::default(),
        algebra: IndexedAlgebraLimits::unbounded_resources(),
    }
}

fn own_family(family: &IntegralFamily) -> R<Arc<IntegralFamily>> {
    let copy = IntegralFamily::new(
        family.name().to_string(),
        family.loop_momenta().to_vec(),
        family.external_momenta().to_vec(),
        family.coefficient_context().clone(),
        family.dimension().clone(),
        family.denominators().to_vec(),
        family.external_gram().to_vec(),
        family.power_shifts().to_vec(),
    )?;
    if copy.fingerprint() != family.fingerprint() {
        return Err("family owner reconstruction changed its identity".into());
    }
    Ok(Arc::new(copy))
}

fn add(row: &mut TerminalRelationRow, key: IntegralKey, value: Coefficient) {
    if let Some(old) = row.remove(&key) {
        let value = &old + &value;
        if !value.is_zero() {
            row.insert(key, value);
        }
    } else if !value.is_zero() {
        row.insert(key, value);
    }
}

fn display_row(row: &TerminalRelationRow) -> Value {
    json!(
        row.iter()
            .map(|(key, c)| json!({"key":key.powers(),"coefficient":c.to_string()}))
            .collect::<Vec<_>>()
    )
}

fn maps_view(rows: &BTreeMap<IntegralKey, TerminalRelationRow>) -> Value {
    json!(
        rows.iter()
            .map(|(key, row)| json!({"key":key.powers(),"terms":display_row(row)}))
            .collect::<Vec<_>>()
    )
}

fn save_json(path: &Path, value: &Value) -> R<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    Ok(())
}

fn drain(
    family: Arc<IntegralFamily>,
    raw: &BTreeSet<IntegralKey>,
    rows: &[TerminalEquation],
    binding: &str,
) -> R<TerminalRelationSession> {
    let mut session = TerminalRelationSession::new(family, raw.clone(), 0, relation_limits())?;
    session.enable_assistance(binding.to_string())?;
    let first = raw.first().ok_or("empty raw inventory")?;
    let cancel = AtomicBool::new(false);
    loop {
        let stats = session.statistics();
        if stats.pending_assistance_keys == 0 && stats.pending_assistance_rows == 0 {
            break;
        }
        session.step_with_provider(&cancel, |key| {
            Ok(if key == first {
                rows.to_vec()
            } else {
                Vec::new()
            })
        })?;
    }
    if session.statistics().completed_source_rows != 0 {
        return Err("ordinary IBP source cursor advanced".into());
    }
    Ok(session)
}

fn neighbors(basis: &BTreeSet<IntegralKey>, rank: usize) -> R<BTreeSet<IntegralKey>> {
    let mut result = basis.clone();
    let mut frontier = basis.clone();
    for _ in 0..rank {
        let mut next = BTreeSet::new();
        for key in frontier {
            for (axis, &power) in key.powers().iter().enumerate() {
                if power <= 0 {
                    let mut powers = key.powers().to_vec();
                    powers[axis] = power.checked_sub(1).ok_or("neighbor index overflow")?;
                    next.insert(IntegralKey::try_new(powers)?);
                }
            }
        }
        result.extend(next.iter().cloned());
        frontier = next;
    }
    Ok(result)
}

fn study<F>(
    label: &str,
    family: Arc<IntegralFamily>,
    raw: BTreeSet<IntegralKey>,
    seeds: BTreeSet<IntegralKey>,
    output: &Path,
    mut reduce: F,
) -> R<Value>
where
    F: FnMut(&IntegralKey) -> R<(TerminalRelationRow, Vec<Coefficient>)>,
{
    fs::create_dir(output)?;
    let whole = Instant::now();
    let mut baseline_rows = Vec::new();
    let mut reduced_cache = BTreeMap::new();
    for key in &raw {
        let (terms, guards) = reduce(key)?;
        let mut row = BTreeMap::from([(key.clone(), family.coefficient_context().one())]);
        for (child, value) in &terms {
            add(&mut row, child.clone(), -value.clone());
        }
        if !row.is_empty() {
            baseline_rows.push(TerminalEquation {
                terms: row,
                nonzero_conditions: guards.clone(),
            });
        }
        reduced_cache.insert(key.clone(), (terms, guards));
    }
    let baseline = drain(
        Arc::clone(&family),
        &raw,
        &baseline_rows,
        "lower-loop-existing-package-baseline-v1",
    )?;
    let before_maps = baseline.apply_all_terminals()?;
    let before = baseline.remaining_terminals();
    save_json(&output.join("baseline-maps.json"), &maps_view(&before_maps))?;
    fs::write(
        output.join("baseline.rrbin"),
        baseline.to_native_bytes(io())?,
    )?;
    let baseline_s = whole.elapsed().as_secs_f64();
    let mut stages = Vec::new();
    for rank in 0..=2 {
        let stage = Instant::now();
        let mut keys = neighbors(&seeds, rank)?;
        keys.extend(raw.iter().cloned());
        let provider = TerminalNumeratorSymmetryEquations::prepare(
            Arc::clone(&family),
            &keys,
            Default::default(),
        )?;
        let prepare_s = stage.elapsed().as_secs_f64();
        let native = provider.encode_native(io())?;
        fs::write(output.join(format!("rank-{rank}-provider.rrbin")), &native)?;
        let cold = TerminalNumeratorSymmetryEquations::from_native_bytes(
            &native,
            Arc::clone(&family),
            Default::default(),
            io(),
        )?;
        if cold.input_keys() != provider.input_keys() || cold.statistics() != provider.statistics()
        {
            return Err("cold provider differs from prepared provider".into());
        }
        let mut translated = Vec::new();
        let mut all_rows = baseline_rows.clone();
        let mut witnessed = 0usize;
        let mut zeros = 0usize;
        let mut failures = Vec::new();
        let mut attempted = 0usize;
        for key in &keys {
            for witness in cold.witnesses(key) {
                witnessed += 1;
                witness.replay(
                    Arc::clone(&family),
                    TerminalNumeratorSymmetryLimits::default(),
                )?;
            }
            for equation in cold.equations(key) {
                attempted += 1;
                let mut terms = TerminalRelationRow::new();
                let mut guards = equation.nonzero_conditions.clone();
                let mut failure = None;
                for (source, coefficient) in &equation.terms {
                    if !reduced_cache.contains_key(source) {
                        match reduce(source) {
                            Ok(value) => {
                                reduced_cache.insert(source.clone(), value);
                            }
                            Err(error) => {
                                failure = Some(format!("{source:?}: {error}"));
                                break;
                            }
                        }
                    }
                    let (reduced, conditions) = &reduced_cache[source];
                    guards.extend(conditions.iter().cloned());
                    for (target, factor) in reduced {
                        add(&mut terms, target.clone(), coefficient * factor);
                    }
                }
                if let Some(error) = failure {
                    failures.push(json!({"target":key.powers(),"error":error}));
                    continue;
                }
                if terms.is_empty() {
                    zeros += 1;
                } else {
                    translated.push(json!({"target":key.powers(),"terms":display_row(&terms),"nonzero_conditions":guards.iter().map(ToString::to_string).collect::<Vec<_>>()}));
                    all_rows.push(TerminalEquation {
                        terms,
                        nonzero_conditions: guards,
                    });
                }
            }
        }
        let reduced_s = stage.elapsed().as_secs_f64();
        let on = drain(
            Arc::clone(&family),
            &raw,
            &all_rows,
            "lower-loop-package-plus-numerator-symmetry-v1",
        )?;
        if baseline
            .nonzero_conditions()
            .iter()
            .any(|condition| !on.nonzero_conditions().contains(condition))
        {
            return Err("an inherited baseline guard was dropped".into());
        }
        let after = on.remaining_terminals();
        let after_maps = on.apply_all_terminals()?;
        for (key, row) in &before_maps {
            let mut composed = TerminalRelationRow::new();
            for (middle, coefficient) in row {
                let replacement = after_maps
                    .get(middle)
                    .ok_or("old output is not a raw retained key")?;
                for (target, factor) in replacement {
                    add(&mut composed, target.clone(), coefficient * factor);
                }
            }
            if composed != after_maps[key] {
                return Err(format!("old alias not preserved for {key:?}").into());
            }
        }
        if after_maps
            .values()
            .flat_map(BTreeMap::keys)
            .any(|key| !after.contains(key))
        {
            return Err("map contains an unresolved nonterminal".into());
        }
        let encoded = on.to_native_bytes(io())?;
        fs::write(output.join(format!("rank-{rank}-session.rrbin")), &encoded)?;
        let reloaded =
            TerminalRelationSession::from_native_bytes(&encoded, relation_limits(), io())?;
        if reloaded.apply_all_terminals()? != after_maps
            || reloaded.remaining_terminals() != after
            || reloaded.nonzero_conditions() != on.nonzero_conditions()
        {
            return Err("cold session maps changed".into());
        }
        save_json(
            &output.join(format!("rank-{rank}-translated.json")),
            &json!(translated),
        )?;
        save_json(
            &output.join(format!("rank-{rank}-maps.json")),
            &maps_view(&after_maps),
        )?;
        let stats = cold.statistics();
        let summary = json!({"rank_neighborhood":rank,"input_keys":keys.len(),"before":before.len(),"after":after.len(),
            "new_dependencies":before.len().checked_sub(after.len()).ok_or("basis grew")?,
            "provider":{"supports":stats.analyzed_supports,"verified_generators":stats.verified_generators,"equations":stats.equations,
                "equation_targets":stats.equation_targets,"trivial_equations":stats.trivial_equations,"rejected_power_permutations":stats.rejected_power_permutations},
            "witnesses_replayed":witnessed,"equations_attempted":attempted,"exact_zero_after_saved_reduction":zeros,
            "nonzero_equations":translated.len(),"unreduced_equations":failures,
            "ordinary_source_rows":on.statistics().completed_source_rows,"all_raw_aliases_verified":raw.len(),
            "provider_prepare_s":prepare_s,"provider_cold_replay_and_existing_rules_s":reduced_s-prepare_s,
            "exact_elimination_and_cold_session_s":stage.elapsed().as_secs_f64()-reduced_s,
            "stage_wall_s":stage.elapsed().as_secs_f64(),"provider_native_bytes":native.len(),"session_native_bytes":encoded.len(),
            "master_minimality_claim":false});
        save_json(&output.join(format!("rank-{rank}-summary.json")), &summary)?;
        println!("{summary}");
        stages.push(summary);
    }
    let result = json!({"label":label,"family_fingerprint":family.fingerprint(),"raw_keys":raw.len(),"published_seed_keys":seeds.len(),
        "baseline_basis":before.len(),"baseline_preparation_s":baseline_s,"stages":stages,"wall_s":whole.elapsed().as_secs_f64()});
    save_json(&output.join("summary.json"), &result)?;
    Ok(result)
}

fn main() -> R<()> {
    let mut args = std::env::args_os().skip(1);
    let label = args
        .next()
        .ok_or("usage: study <k6|h|fg|bmw|x|combined> <input-root> <new-output-dir>")?
        .into_string()
        .map_err(|_| "non-UTF8 label")?;
    let input = PathBuf::from(args.next().ok_or("missing input root")?);
    let output = PathBuf::from(args.next().ok_or("missing new output directory")?);
    if args.next().is_some() || output.exists() {
        return Err("extra argument or output already exists".into());
    }
    if label == "combined" {
        fs::create_dir(&output)?;
        let mut results = Vec::new();
        for stage in [
            "baseline",
            "rank-0-session",
            "rank-1-session",
            "rank-2-session",
        ] {
            let sessions = ["h", "fg", "bmw", "x"]
                .iter()
                .map(|parent| {
                    let bytes = fs::read(input.join(parent).join(format!("{stage}.rrbin")))?;
                    Ok(TerminalRelationSession::from_native_bytes(
                        &bytes,
                        relation_limits(),
                        io(),
                    )?)
                })
                .collect::<R<Vec<_>>>()?;
            let inventories = sessions
                .iter()
                .map(|session| {
                    (
                        Arc::clone(session.family_owner()),
                        session.remaining_terminals(),
                    )
                })
                .collect::<Vec<_>>();
            let plan = VacuumFamilyAliasPlan::prepare(&inventories, Default::default())?;
            let view = json!({
                "stage":stage,
                "family_local_outputs":plan.raw_terminals().len(),
                "cross_family_positive_U_outputs":plan.canonical_terminals().len(),
                "exact_cross_family_aliases":plan.aliases().len(),
                "authority":"existing exact full-U equivalences only; no new diagonal IBP rows",
                "master_minimality_claim":false,
            });
            println!("{view}");
            results.push(view);
        }
        save_json(&output.join("summary.json"), &json!(results))?;
        return Ok(());
    }
    let loaded = Instant::now();
    let result = if label == "k6" {
        let artifact =
            ClosedArtifact::decode_durable(&fs::read(input.join("unit_mass_vacuum_k6.rrbin"))?)?;
        let family = own_family(artifact.family())?;
        let raw = artifact.masters().clone();
        let mut reducer = Reducer::with_limits(&artifact, reduction_limits())?;
        let seeds = raw.clone();
        eprintln!(
            "loaded K6 in {:.3}s: {} raw keys",
            loaded.elapsed().as_secs_f64(),
            raw.len()
        );
        study(&label, family, raw, seeds, &output, |key| {
            let (value, conditions, _) =
                reducer.reduce_unit_mass_guarded(key, reduction_limits())?;
            Ok((value.terms().clone(), conditions))
        })?
    } else {
        if !["h", "fg", "bmw", "x"].contains(&label.as_str()) {
            return Err("unknown fixture label".into());
        }
        let directory = input.join("four_loop");
        let limits = CandidateBundleLimits {
            max_bundle_bytes: usize::MAX,
            max_collection_entries: usize::MAX,
            max_coefficient_bytes: usize::MAX,
            max_total_coefficient_bytes: usize::MAX,
            exact_algebra: ExactAlgebraLimits::unbounded_resources(),
        };
        let (family, mut reducer) = load_generated_candidate_bundle::<16>(
            &fs::read(directory.join(format!("{label}.candidates.rrbin")))?,
            limits,
            reduction_limits(),
        )?;
        let raw = reducer.terminals().clone();
        let normalization = TerminalNormalizationPlan::decode_generated(
            &fs::read(directory.join(format!("{label}.rrnorm.bin")))?,
            &family,
            &raw,
            reducer.ordering(),
            Default::default(),
            io(),
        )?;
        let seeds = normalization.canonical_terminals().clone();
        reducer.install_terminal_normalization(normalization)?;
        eprintln!(
            "loaded {label} in {:.3}s: {} raw, {} published outputs",
            loaded.elapsed().as_secs_f64(),
            raw.len(),
            seeds.len()
        );
        study(&label, Arc::new(family), raw, seeds, &output, |key| {
            let (value, conditions, _) =
                reducer.reduce_unit_mass_guarded(key, reduction_limits())?;
            Ok((value.terms().clone(), conditions))
        })?
    };
    eprintln!(
        "completed {}: {} -> {}",
        label, result["raw_keys"], result["baseline_basis"]
    );
    Ok(())
}
