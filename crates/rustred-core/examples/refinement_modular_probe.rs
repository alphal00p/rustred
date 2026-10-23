//! Read-only, caller-bounded modular-circuit pilot.
//!
//! Usage: refinement_modular_probe INPUT WORKERS ROWS [BATCH_ROWS]
//!                                [--compare-exact | --validate-only]
//!                                [--reference-state ORIGINAL.rrbin]
//!                                [--discovery-only]
//! INPUT is a native exact session, fixture-1 (massive tadpole), fixture-3
//! (three independent massive tadpoles with a complete scalar-product ISP
//! basis), or fixture-2 / fixture-sunset (connected two-loop, three-propagator
//! vacuum sunset, the K3 graph).
//! ROWS bounds this diagnostic cohort, never the production search. All output
//! goes to stdout; no native input or campaign artifact is changed.
//! --discovery-only stops at the first candidate (or the cohort boundary),
//! reporting its source prefix without reconstruction or a relation claim.
//! --validate-only performs a native encode/decode semantic roundtrip without
//! preparing or consuming any sources; ROWS may be zero in this mode.
//!
//! Source preparation and finite-field discovery are timed together. Native
//! reconstruction and exact full-column verification share one measured phase;
//! the public coordinator does not expose separate per-worker product timers.
//! Obtain process CPU and maximum RSS with the caller's external measurement.

use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use rustred::persistence::BinaryIoLimits;
use rustred::reduction::terminal_relations::{
    TerminalRelationExecutionOptions, TerminalRelationExecutor, TerminalRelationLimits,
    TerminalRelationModularOptions, TerminalRelationModularProposalStatus,
    TerminalRelationModularSession, TerminalRelationSession,
};
use std::collections::BTreeSet;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn limits() -> TerminalRelationLimits {
    TerminalRelationLimits {
        max_seeds: usize::MAX,
        max_columns: usize::MAX,
        max_rows: usize::MAX,
        max_nonzeros: usize::MAX,
        ..Default::default()
    }
}

fn fixture(input: &str) -> Result<TerminalRelationSession, Box<dyn Error>> {
    let loops = match input {
        "fixture-1" => 1,
        "fixture-3" => 3,
        "fixture-2" | "fixture-sunset" => 2,
        _ => return Err("unknown fixture".into()),
    };
    let c = CoefficientContext::try_new(["d"])?;
    let coordinates = loops * (loops + 1) / 2;
    let mut denominators = Vec::new();
    let sunset = matches!(input, "fixture-2" | "fixture-sunset");
    if sunset {
        // q1^2-1, q2^2-1, (q1-q2)^2-1 in the declared upper-triangle
        // scalar-product order [q1.q1, q1.q2, q2.q2].
        for row in [[1, 0, 0], [0, 0, 1], [1, -2, 1]] {
            denominators.push(AffineDenominator::new(
                c.integer(-1),
                row.into_iter().map(|value| c.integer(value)).collect(),
            ));
        }
    } else {
        let mut diagonal = Vec::new();
        let mut off_diagonal = Vec::new();
        let mut position = 0;
        for left in 0..loops {
            for right in left..loops {
                if left == right {
                    diagonal.push(position);
                } else {
                    off_diagonal.push(position);
                }
                position += 1;
            }
        }
        // The first `loops` denominators are physical tadpoles. Remaining
        // coordinates are scalar-product ISPs, all inactive in the fixtures.
        for coordinate in diagonal.into_iter().chain(off_diagonal) {
            let physical = denominators.len() < loops;
            denominators.push(AffineDenominator::new(
                c.integer(if physical { -1 } else { 0 }),
                (0..coordinates)
                    .map(|index| c.integer(i64::from(index == coordinate)))
                    .collect(),
            ));
        }
    }
    let family = Arc::new(IntegralFamily::new(
        format!("modular-pilot-{input}"),
        (0..loops).map(|index| format!("q{index}")).collect(),
        Vec::new(),
        c.clone(),
        c.parameter("d")
            .ok_or("fixture dimension parameter is absent")?,
        denominators,
        Vec::new(),
        vec![c.zero(); coordinates],
    )?);
    let active = if sunset { 3 } else { loops };
    let mut corner = vec![0; coordinates];
    corner[..active].fill(1);
    let mut raw = BTreeSet::from([IntegralKey::try_new(corner.clone())?]);
    let max_power = if sunset { 2 } else { 3 };
    for axis in 0..active {
        for power in 2..=max_power {
            let mut powers = corner.clone();
            powers[axis] = power;
            raw.insert(IntegralKey::try_new(powers)?);
        }
    }
    Ok(TerminalRelationSession::new(family, raw, 0, limits())?)
}

fn map_digest(session: &TerminalRelationSession) -> Result<String, Box<dyn Error>> {
    let mut hash = blake3::Hasher::new();
    // This is a diagnostic digest, never an artifact codec or equality proof.
    for (source, terms) in session.apply_all_terminals()? {
        hash.update(format!("{:?}", source.powers()).as_bytes());
        for (key, value) in terms {
            hash.update(format!("{:?}={value};", key.powers()).as_bytes());
        }
    }
    Ok(hash.finalize().to_hex().to_string())
}

fn quote(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            value if value <= '\u{1f}' => result.push_str(&format!("\\u{:04x}", value as u32)),
            value => result.push(value),
        }
    }
    result.push('"');
    result
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut input_arguments = std::env::args().skip(1);
    let mut arguments = Vec::new();
    let mut compare_exact = false;
    let mut validate_only = false;
    let mut discovery_only = false;
    let mut reference_path = None;
    while let Some(argument) = input_arguments.next() {
        match argument.as_str() {
            "--compare-exact" => compare_exact = true,
            "--validate-only" => validate_only = true,
            "--discovery-only" => discovery_only = true,
            "--reference-state" => {
                if reference_path.is_some() {
                    return Err("reference state was provided twice".into());
                }
                reference_path = Some(
                    input_arguments
                        .next()
                        .ok_or("--reference-state requires a native state path")?,
                );
            }
            _ => arguments.push(argument),
        }
    }
    if discovery_only && (compare_exact || validate_only || reference_path.is_some()) {
        return Err(
            "discovery-only cannot be combined with exact, validation or reference controls".into(),
        );
    }
    if !(3..=4).contains(&arguments.len()) {
        return Err(
            "expected INPUT WORKERS ROWS [BATCH_ROWS] [--compare-exact | --validate-only] [--reference-state PATH]".into(),
        );
    }
    let input = arguments[0].as_str();
    let workers: usize = arguments[1].parse()?;
    let requested_rows: usize = arguments[2].parse()?;
    let batch_rows: usize = arguments
        .get(3)
        .map_or(Ok(requested_rows), |value| value.parse())?;
    if workers == 0 || (!validate_only && (requested_rows == 0 || batch_rows == 0)) {
        return Err("workers, rows and batch rows must be positive".into());
    }
    let io = BinaryIoLimits::unbounded_resources();
    let loaded = Instant::now();
    let (session, input_bytes, input_hash) = if input.starts_with("fixture-") {
        let session = fixture(input)?;
        let bytes = session.to_native_bytes(io)?;
        let input_hash = blake3::hash(&bytes).to_hex().to_string();
        (session, bytes.len(), input_hash)
    } else {
        let bytes = std::fs::read(input)?;
        let input_hash = blake3::hash(&bytes).to_hex().to_string();
        (
            TerminalRelationSession::from_native_bytes(&bytes, limits(), io)?,
            bytes.len(),
            input_hash,
        )
    };
    let load_seconds = loaded.elapsed().as_secs_f64();
    if validate_only {
        let before = session.statistics();
        let maps = if before.pending_rebuild_rows == 0 {
            Some(session.apply_all_terminals()?)
        } else {
            None
        };
        let encode = Instant::now();
        let output = session.to_native_bytes(io)?;
        let encode_seconds = encode.elapsed().as_secs_f64();
        let output_hash = blake3::hash(&output).to_hex().to_string();
        let reload = Instant::now();
        let restored = TerminalRelationSession::from_native_bytes(&output, limits(), io)?;
        let reload_seconds = reload.elapsed().as_secs_f64();
        let maps_equal = match maps {
            Some(maps) => Some(maps == restored.apply_all_terminals()?),
            None => None,
        };
        let terminal_map_check = match maps_equal {
            Some(true) => "equal",
            Some(false) => "different",
            None => "skipped-pending-rebuild",
        };
        let semantic_roundtrip_equal = before == restored.statistics()
            && maps_equal.unwrap_or(true)
            && session.raw_terminals() == restored.raw_terminals()
            && session.nonzero_conditions() == restored.nonzero_conditions();
        let encoded_roundtrip_equal = input_bytes == output.len() && input_hash == output_hash;
        println!(
            "{{\"mode\":\"validate-only\",\"input\":{},\"input_bytes\":{input_bytes},\"output_bytes\":{},\"input_blake3\":\"{input_hash}\",\"output_blake3\":\"{output_hash}\",\"load_seconds\":{load_seconds},\"encode_seconds\":{encode_seconds},\"reload_seconds\":{reload_seconds},\"semantic_roundtrip_equal\":{semantic_roundtrip_equal},\"encoded_roundtrip_equal\":{encoded_roundtrip_equal},\"terminal_map_check\":\"{terminal_map_check}\",\"source_rows_advanced\":0,\"completed_source_rows\":{},\"pending_rebuild_rows\":{},\"independent_rows\":{},\"columns\":{},\"nonzeros\":{},\"terminal_relations\":{},\"remaining_terminals\":{},\"complete\":{}}}",
            quote(input),
            output.len(),
            before.completed_source_rows,
            before.pending_rebuild_rows,
            before.independent_rows,
            before.columns,
            before.nonzeros,
            before.terminal_relations,
            before.remaining_terminals,
            before.complete
        );
        if !semantic_roundtrip_equal {
            return Err("read-only validation changed checkpoint semantics".into());
        }
        return Ok(());
    }
    if session.statistics().pending_rebuild_rows != 0 {
        return Err("finish the saved exact rebuild before measuring a source cohort".into());
    }
    let original_cursor = session.statistics().completed_source_rows;
    let original_relations = session.statistics().terminal_relations;
    let original_map_digest = map_digest(&session)?;
    let control_bytes = compare_exact
        .then(|| session.to_native_bytes(io))
        .transpose()?;
    let mut options = TerminalRelationModularOptions::default();
    options.batch_rows = batch_rows.min(requested_rows);
    // At most one candidate is generated per new source per ordinary image.
    // This caller-specified cohort grant drains its ordinary pending lifts
    // without silently advancing beyond the requested source prefix.
    options.max_candidates = requested_rows.max(options.max_candidates);
    let initialized = Instant::now();
    let mut modular = TerminalRelationModularSession::new_with_io(session, options, io)?;
    modular.set_workers(workers)?;
    let initialization_seconds = initialized.elapsed().as_secs_f64();
    let solving = Instant::now();
    let mut preparation_discovery = Duration::ZERO;
    let mut lift_verification = Duration::ZERO;
    let cancel = AtomicBool::new(false);
    while modular.statistics().source_rows_scanned < requested_rows as u64
        && !modular.statistics().search_exhausted
    {
        let remaining = requested_rows - modular.statistics().source_rows_scanned as usize;
        if modular.options().batch_rows > remaining {
            let mut options = modular.options();
            options.batch_rows = remaining;
            modular.set_options(options)?;
        }
        let mut previous = Instant::now();
        let mut lifting = false;
        modular.step_observed(&cancel, |progress| {
            let now = Instant::now();
            if lifting {
                lift_verification += now.duration_since(previous);
            } else {
                preparation_discovery += now.duration_since(previous);
            }
            if progress.stage == "source-weight reconstruction and exact products" {
                lifting = true;
            }
            previous = now;
            if discovery_only && progress.statistics.candidates_found != 0 {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        })?;
        let elapsed = previous.elapsed();
        if lifting {
            lift_verification += elapsed;
        } else {
            preparation_discovery += elapsed;
        }
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
    }
    let solver_seconds = solving.elapsed().as_secs_f64();
    if discovery_only {
        let stats = modular.statistics();
        println!(
            "{{\"mode\":\"discovery-only\",\"input\":{},\"workers\":{workers},\"source_rows_scanned\":{},\"candidates\":{},\"pending_candidates\":{},\"initialization_seconds\":{initialization_seconds},\"solver_seconds\":{solver_seconds},\"exact_cursor_unchanged\":{},\"accepted_relations\":0,\"relation_or_closure_claim\":false}}",
            quote(input),
            stats.source_rows_scanned,
            stats.candidates_found,
            stats.pending_candidates,
            modular.session().statistics().completed_source_rows == original_cursor
        );
        return Ok(());
    }
    if modular.statistics().pending_candidates != 0 {
        return Err("cohort retains pending lifts; increase the caller's candidate grant".into());
    }
    if modular.session().statistics().completed_source_rows != original_cursor {
        return Err("modular discovery changed the exact source cursor".into());
    }
    let statistics = modular.statistics();
    let final_map_digest = map_digest(modular.session())?;
    let encoding = Instant::now();
    let bytes = modular.to_native_bytes(io)?;
    let encode_seconds = encoding.elapsed().as_secs_f64();
    let cold = Instant::now();
    let restored = TerminalRelationModularSession::from_native_bytes(&bytes, limits(), io)?;
    let cold_load_seconds = cold.elapsed().as_secs_f64();
    let restored_maps_equal =
        restored.session().apply_all_terminals()? == modular.session().apply_all_terminals()?;
    if !restored_maps_equal || restored.to_native_bytes(io)? != bytes {
        return Err("cold reload changed exact maps or checkpoint bytes".into());
    }
    let rejected = modular
        .proposals()
        .filter(|(_, status)| {
            matches!(
                status,
                TerminalRelationModularProposalStatus::Rejected { .. }
            )
        })
        .count();
    let mut exact = String::from("null");
    if let Some(control_bytes) = control_bytes {
        let load = Instant::now();
        let mut control = TerminalRelationSession::from_native_bytes(&control_bytes, limits(), io)?;
        let exact_load_seconds = load.elapsed().as_secs_f64();
        let mut execution = TerminalRelationExecutionOptions::new(workers);
        execution.batch_rows = (statistics.source_rows_scanned as usize).max(1);
        execution.block_rows = 8;
        let executor = TerminalRelationExecutor::new(execution)?;
        let solve = Instant::now();
        while control.statistics().completed_source_rows - original_cursor
            < statistics.source_rows_scanned
        {
            executor.step(&mut control, &AtomicBool::new(false))?;
        }
        let exact_solver_seconds = solve.elapsed().as_secs_f64();
        let exact_state_hash = blake3::hash(&control.to_native_bytes(io)?).to_hex();
        let maps_equal =
            control.apply_all_terminals()? == modular.session().apply_all_terminals()?;
        exact = format!(
            "{{\"load_seconds\":{exact_load_seconds},\"solver_seconds\":{exact_solver_seconds},\"source_rows\":{},\"terminal_relations_added\":{},\"remaining_terminals\":{},\"terminal_maps_equal\":{maps_equal},\"state_blake3\":\"{exact_state_hash}\",\"terminal_map_digest\":\"{}\"}}",
            statistics.source_rows_scanned,
            control.statistics().terminal_relations - original_relations,
            control.statistics().remaining_terminals,
            map_digest(&control)?
        );
    }
    let mut reference = String::from("null");
    if let Some(path) = reference_path {
        let load = Instant::now();
        let bytes = std::fs::read(&path)?;
        let hash = blake3::hash(&bytes).to_hex();
        let state = TerminalRelationSession::from_native_bytes(&bytes, limits(), io)?;
        let reference_load_seconds = load.elapsed().as_secs_f64();
        if state.family_owner().fingerprint() != modular.session().family_owner().fingerprint()
            || state.raw_terminals() != modular.session().raw_terminals()
        {
            return Err(
                "reference state must have the same family and raw terminal inventory".into(),
            );
        }
        if state.statistics().pending_rebuild_rows != 0 {
            return Err(
                "reference-state map comparison requires a finished row-space rebuild".into(),
            );
        }
        let maps_equal = state.apply_all_terminals()? == modular.session().apply_all_terminals()?;
        let guards_retained = state
            .nonzero_conditions()
            .iter()
            .all(|condition| modular.session().nonzero_conditions().contains(condition));
        if !maps_equal || !guards_retained {
            return Err(
                "known-relation recovery disagrees with the reference maps or guards".into(),
            );
        }
        reference = format!(
            "{{\"load_seconds\":{reference_load_seconds},\"state_blake3\":\"{hash}\",\"terminal_maps_equal\":{maps_equal},\"reference_conditions_retained\":{guards_retained},\"terminal_relations\":{},\"remaining_terminals\":{},\"terminal_map_digest\":\"{}\"}}",
            state.statistics().terminal_relations,
            state.statistics().remaining_terminals,
            map_digest(&state)?
        );
    }
    println!(
        "{{\"input\":{},\"loops\":{},\"workers\":{workers},\"requested_rows\":{requested_rows},\"source_rows_scanned\":{},\"batch_rows\":{batch_rows},\"load_seconds\":{load_seconds},\"initialization_seconds\":{initialization_seconds},\"source_preparation_discovery_seconds\":{},\"lifting_and_exact_verification_seconds\":{},\"solver_seconds\":{solver_seconds},\"encode_seconds\":{encode_seconds},\"cold_load_seconds\":{cold_load_seconds},\"candidates\":{},\"accepted_relations\":{},\"rejected_proposals\":{rejected},\"reconstruction_misses\":{},\"search_exhausted\":{},\"exact_cursor_unchanged\":true,\"remaining_terminals\":{},\"checkpoint_bytes\":{},\"checkpoint_blake3\":\"{}\",\"initial_terminal_map_digest\":\"{original_map_digest}\",\"terminal_map_digest\":\"{final_map_digest}\",\"cold_reload_maps_equal\":{restored_maps_equal},\"oracle_probes\":null,\"separate_product_timing_available\":false,\"exact_control\":{exact},\"reference_state\":{reference}}}",
        quote(input),
        modular.session().family_owner().loop_count(),
        statistics.source_rows_scanned,
        preparation_discovery.as_secs_f64(),
        lift_verification.as_secs_f64(),
        statistics.candidates_found,
        statistics.accepted_relations,
        statistics.reconstruction_misses,
        statistics.search_exhausted,
        modular.session().statistics().remaining_terminals,
        bytes.len(),
        blake3::hash(&bytes).to_hex()
    );
    Ok(())
}
