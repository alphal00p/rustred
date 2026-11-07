//! Replay one native reduction request recorded by Vakint, with RustRed alone.
//!
//! `ordered_replay REQUEST.json [--data-root DIR] [--strategy NAME]
//!     [--stop-after-phase N] [--capture-frontier-at-phase N FILE] [--threads N]
//!     [--window N] [--arithmetic SPEC] [--phase-digests]`
//!
//! Vakint writes `REQUEST.json` (schema `vakint.ordered-native-request.v1`) when
//! `VAKINT_DUMP_ORDERED_REQUEST=<dir>` is set, plus `.phases.jsonl` (native
//! counters at every phase end) and `.result.json` (digests of the exact native
//! output) next to it. This example loads the same native program, parses the
//! exact inputs (their canonical round trip is checked), runs the request and
//! prints JSONL records on stdout:
//!
//! * `request`/`loaded`: identity, input count and program loading time (saved
//!   applications: also whether the routes were verified or restored from a
//!   route preparation add-on, `route_preparation`);
//! * `phase`: one per frozen ordered phase (index, kind, support, owner),
//!   its wall seconds, the counters it added (processed keys, emitted terms,
//!   merges, cancellations), the frontier keys/monomials at its end, and
//!   whether its cumulative counters equal Vakint's recorded ones;
//! * `capture`: `--capture-frontier-at-phase N FILE` writes every pending
//!   frontier identity and weight, the terminal output so far and the guards
//!   so far, at the start of phase N (JSONL, schema
//!   `rustred.ordered-frontier-capture.v1`); its cost is excluded from timing;
//! * `stopped`: `--stop-after-phase N` ends the request after phase N drains
//!   (after the capture, when that is requested at the start of phase N+1);
//! * `finished`: native seconds and BLAKE3 output digests (layout
//!   `vakint.ordered-native-output.v1`), compared with Vakint's when present.
//!
//! Programs: 1-3L closed artifacts (`{L}L/ibps/rules.rrbin`), 4L candidate
//! programs (`4L/ibps/<name>.candidates.rrbin.gz` and `.rrnorm.bin`) and the 5L
//! saved application (`5L/ibps`). Recorded absolute paths are used when they
//! exist; otherwise `--data-root` (a vakint-external-data checkout or Vakint's
//! vendored `data` directory) supplies `{L}L/<file>`. File SHA-256 values are
//! printed for manual checks; the output digests are the exactness proof.
//! `--strategy` accepts `ordered` (default, phase records), `depth_first`
//! (5L only) and `depth_first_without_memoization` (1-3L and 5L); the latter
//! two print only the final record.
//!
//! `--arithmetic` selects the Ordered weight arithmetic in its canonical text
//! form (`exact`, the default, `modular_sampled[:key=value,...]` or
//! `modular_polynomial[:key=value,...]`; the historical `modular[:...]` alias is
//! accepted); modular weights appear as `null` in frontier captures. Modular
//! workers are set by the arithmetic's `threads` option.
//!
//! `--threads N` and `--window N` select the Ordered scheduler's workers and
//! window size (`OrderedSchedulerOptions`; closed 1-3L artifacts always run
//! sequentially). Every setting must give identical records apart from
//! timings. `--phase-digests` adds to every `phase` record BLAKE3 digests of
//! the scheduler state at the phase end: the frontier in pop order (identities
//! and exact weight representations, or identities only for modular weights)
//! and an order-independent multiset digest of the same entries
//! (`frontier_set`: the pop order of later-phase Route keys differs between
//! Route layering modes, their pending entries do not),
//! the terminals, and the guard list in its order. `finished` always reports an
//! order-sensitive guard digest too.
//!
//! `--route-layers NAME` selects the 5L saved application's Ordered Route
//! transport (`OrderedRouteLayering`: `off`, `index`, `smallest-power`,
//! `fewest-terms`, `largest-power`; default: the library default,
//! `fewest-terms`; `off` is the historical whole transport). Every
//! setting must give identical output digests; phase counters (processed keys,
//! emitted terms, peaks) differ, so compare them with
//! `--ignore-recorded-counters`.
//!
//! Step-cache options (Ordered only):
//! * `--step-cache DIR` attaches the program's persistent Ordered step cache
//!   in `DIR` (created on save), `--step-cache-save` saves it after the run,
//!   `--step-cache-read-only` disables population. A `step_cache` record
//!   reports hits, misses, inserts, records and bytes (per shard as well);
//! * with `--phase-digests` every `phase` record carries both the scheduler's
//!   `digests` (frontier, terminals, guards separately) and the step cache's
//!   single `digest` of the complete state (exact weights where present);
//! * `--adjoint-tail` / `--no-adjoint-tail` evaluate the request's
//!   single-terminal tail by its adjoint or forward
//!   (`OrderedSchedulerOptions::adjoint_tail`, on by default; the variable
//!   `RUSTRED_ORDERED_ADJOINT_TAIL=0|1` overrides both); with `--step-cache`
//!   the cache's tail table is read (closing the tail when the guards allow
//!   it) and populated, and the `step_cache` record reports it (`tail`);
//! * `--ignore-recorded-counters` reports, but does not fail on, phase
//!   counters that differ from Vakint's recording (e.g. a recording made
//!   before an intended counter change).
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::reduction::{
    OrderedKeyStage, OrderedPhaseBoundary, OrderedPhaseEvent, OrderedSchedulerOptions,
    OrderedStepCache, OrderedStepCacheAdapter, OrderedStepCacheBinding, OrderedStepCacheOptions,
    OrderedWeightArithmetic, ReductionError, ReductionLimits, ReductionStrategy,
};
use rustred::solver::{CandidateOrderedReductionProgress, CandidateRoutedError, UnresolvedPolicy};
use serde_json::{Value, json};
use std::sync::Arc;
use symbolica::atom::AtomCore;
use symbolica::domains::{integer::Z, rational::Q};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const REQUEST_SCHEMA: &str = "vakint.ordered-native-request.v1";
const CAPTURE_SCHEMA: &str = "rustred.ordered-frontier-capture.v1";
const OUTPUT_DIGEST_SCHEME: &str = "vakint.ordered-native-output.v1";
const STOP_DETAIL: &str = "ordered replay stopped after the requested phase";

struct Options {
    request: PathBuf,
    data_root: Option<PathBuf>,
    strategy: ReductionStrategy,
    arithmetic: OrderedWeightArithmetic,
    stop_after_phase: Option<usize>,
    capture: Option<(usize, PathBuf)>,
    scheduler: OrderedSchedulerOptions,
    phase_digests: bool,
    step_cache: Option<PathBuf>,
    step_cache_save: bool,
    step_cache_read_only: bool,
    ignore_recorded_counters: bool,
    route_layers: Option<rustred::solver::OrderedRouteLayering>,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let mut request = None;
        let mut options = Options {
            request: PathBuf::new(),
            data_root: None,
            strategy: ReductionStrategy::Ordered,
            arithmetic: OrderedWeightArithmetic::Exact,
            stop_after_phase: None,
            capture: None,
            scheduler: OrderedSchedulerOptions::default(),
            phase_digests: false,
            step_cache: None,
            step_cache_save: false,
            step_cache_read_only: false,
            ignore_recorded_counters: false,
            route_layers: None,
        };
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
            match arg.as_str() {
                "--data-root" => options.data_root = Some(value("--data-root")?.into()),
                "--strategy" => {
                    options.strategy = match value("--strategy")?.as_str() {
                        "ordered" => ReductionStrategy::Ordered,
                        "depth_first" => ReductionStrategy::DepthFirst,
                        "depth_first_without_memoization" => {
                            ReductionStrategy::DepthFirstWithoutMemoization
                        }
                        other => return Err(format!("unknown strategy {other}").into()),
                    }
                }
                "--arithmetic" => options.arithmetic = value("--arithmetic")?.parse()?,
                "--stop-after-phase" => {
                    options.stop_after_phase = Some(value("--stop-after-phase")?.parse()?)
                }
                "--capture-frontier-at-phase" => {
                    let phase = value("--capture-frontier-at-phase")?.parse()?;
                    options.capture = Some((phase, value("--capture-frontier-at-phase")?.into()));
                }
                "--threads" => options.scheduler.threads = value("--threads")?.parse()?,
                "--window" => options.scheduler.window_keys = value("--window")?.parse()?,
                "--phase-digests" => options.phase_digests = true,
                "--adjoint-tail" => options.scheduler.adjoint_tail = true,
                "--no-adjoint-tail" => options.scheduler.adjoint_tail = false,
                "--step-cache" => options.step_cache = Some(value("--step-cache")?.into()),
                "--step-cache-save" => options.step_cache_save = true,
                "--step-cache-read-only" => options.step_cache_read_only = true,
                "--ignore-recorded-counters" => options.ignore_recorded_counters = true,
                "--route-layers" => options.route_layers = Some(value("--route-layers")?.parse()?),
                _ if request.is_none() && !arg.starts_with("--") => request = Some(arg.into()),
                _ => return Err(format!("unexpected argument {arg}").into()),
            }
        }
        options.request = request.ok_or(
            "usage: ordered_replay REQUEST.json [--data-root DIR] [--strategy NAME] \
             [--stop-after-phase N] [--capture-frontier-at-phase N FILE] [--threads N] \
             [--window N] [--arithmetic SPEC] [--phase-digests]",
        )?;
        let phased = options.stop_after_phase.is_some()
            || options.capture.is_some()
            || options.phase_digests
            || options.step_cache.is_some()
            || options.arithmetic != OrderedWeightArithmetic::Exact;
        if phased && options.strategy != ReductionStrategy::Ordered {
            return Err(
                "phase, arithmetic and step-cache options require --strategy ordered".into(),
            );
        }
        Ok(options)
    }
}

fn emit(record: Value) {
    let mut stdout = std::io::stdout().lock();
    // A closed stdout must not abort an expensive replay.
    let _ = writeln!(stdout, "{record}").and_then(|()| stdout.flush());
}

fn canonical(coefficient: &Coefficient) -> String {
    coefficient.to_expression().to_canonical_string()
}

fn powers(key: &IntegralKey) -> String {
    key.powers()
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn mask(sector: &[bool]) -> String {
    sector
        .iter()
        .map(|&bit| if bit { '1' } else { '0' })
        .collect()
}

/// Parse and authenticate one recorded coefficient in the program's context.
fn coefficient(context: &CoefficientContext, text: &str) -> Result<Coefficient> {
    let atom = symbolica::try_parse!(text, default_namespace = "rustred")?;
    let value = atom.as_view().try_to_rational_polynomial(
        &Q,
        &Z,
        Some(context.one().get_variables().clone()),
    )?;
    context.validate_with_limits(&value, ExactAlgebraLimits::default())?;
    if canonical(&value) != text {
        return Err(format!("coefficient does not round-trip canonically: {text}").into());
    }
    Ok(value)
}

fn inputs(
    request: &Value,
    context: &CoefficientContext,
) -> Result<Vec<(IntegralKey, Coefficient)>> {
    let rows = request["inputs"]
        .as_array()
        .ok_or("request has no inputs")?;
    let mut hasher = blake3::Hasher::new();
    let mut inputs = Vec::with_capacity(rows.len());
    for row in rows {
        let key = IntegralKey::try_new(serde_json::from_value::<Vec<i64>>(row["powers"].clone())?)?;
        let text = row["coefficient"]
            .as_str()
            .ok_or("coefficient is not a string")?;
        hasher.update(powers(&key).as_bytes());
        hasher.update(b"\n");
        hasher.update(text.as_bytes());
        hasher.update(b"\n");
        inputs.push((key, coefficient(context, text)?));
    }
    if request["input_digest"].as_str() != Some(hasher.finalize().to_hex().as_str()) {
        return Err("parsed inputs do not reproduce the recorded input digest".into());
    }
    Ok(inputs)
}

/// The `vakint.ordered-native-output.v1` digests of one exact native output.
fn output_digest<'k>(
    terms: impl IntoIterator<Item = (&'k str, &'k IntegralKey, &'k Coefficient)>,
    nonzero_conditions: &[Coefficient],
) -> Value {
    let mut hasher = blake3::Hasher::new();
    let mut count = 0usize;
    for (family, key, coefficient) in terms {
        for part in [family, &powers(key), &canonical(coefficient)] {
            hasher.update(part.as_bytes());
            hasher.update(b"\n");
        }
        count += 1;
    }
    let mut ordered_guards = blake3::Hasher::new();
    for guard in nonzero_conditions {
        ordered_guards.update(canonical(guard).as_bytes());
        ordered_guards.update(b"\n");
    }
    let guards: std::collections::BTreeSet<_> = nonzero_conditions.iter().map(canonical).collect();
    let mut guard_hasher = blake3::Hasher::new();
    for guard in &guards {
        guard_hasher.update(guard.as_bytes());
        guard_hasher.update(b"\n");
    }
    json!({
        "scheme": OUTPUT_DIGEST_SCHEME,
        "terms": count,
        "terms_blake3": hasher.finalize().to_hex().to_string(),
        "nonzero_conditions": nonzero_conditions.len(),
        "distinct_nonzero_conditions": guards.len(),
        "nonzero_conditions_blake3": guard_hasher.finalize().to_hex().to_string(),
        "nonzero_conditions_ordered_blake3": ordered_guards.finalize().to_hex().to_string(),
    })
}

/// Feed `std::hash::Hash` output into BLAKE3.
struct Blake3Hash<'a>(&'a mut blake3::Hasher);

impl std::hash::Hasher for Blake3Hash<'_> {
    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    fn finish(&self) -> u64 {
        0
    }
}

/// The exact representation of a normalized coefficient (integer
/// coefficients and exponents of its numerator and denominator), without
/// printing it.
fn hash_coefficient(hasher: &mut blake3::Hasher, coefficient: &Coefficient) {
    use std::hash::Hash;
    for polynomial in [&coefficient.numerator, &coefficient.denominator] {
        hasher.update(&(polynomial.coefficients.len() as u64).to_le_bytes());
        polynomial.coefficients.hash(&mut Blake3Hash(hasher));
        polynomial.exponents.hash(&mut Blake3Hash(hasher));
    }
}

/// Digests of the scheduler state at one phase boundary.
fn state_digest(event: &OrderedPhaseEvent<'_>) -> Value {
    let mut frontier = blake3::Hasher::new();
    // Order-independent digest of the same entries: the wrapping sum of
    // their individual BLAKE3 digests (a multiset hash), equal for any pop
    // order of the same pending identities and weights.
    let mut set = [0u64; 4];
    let mut entries = 0usize;
    let mut weights = 0usize;
    for entry in event.frontier() {
        let (stage, owner) = match entry.stage {
            OrderedKeyStage::Integral => ("integral", String::new()),
            OrderedKeyStage::Route => ("route", String::new()),
            OrderedKeyStage::Apply { owner_sector } => ("apply", mask(owner_sector)),
            OrderedKeyStage::ClassRoute { class } => ("class-route", class.to_string()),
            OrderedKeyStage::ClassApply { class } => ("class-apply", class.to_string()),
        };
        let line = format!(
            "{}|{stage}|{owner}|{}|",
            entry.phase,
            powers(entry.integral)
        );
        frontier.update(line.as_bytes());
        let mut single = blake3::Hasher::new();
        single.update(line.as_bytes());
        if let Some(weight) = entry.weight {
            hash_coefficient(&mut frontier, weight);
            hash_coefficient(&mut single, weight);
            weights += 1;
        }
        frontier.update(b"\n");
        for (word, bytes) in set
            .iter_mut()
            .zip(single.finalize().as_bytes().chunks_exact(8))
        {
            *word = word.wrapping_add(u64::from_le_bytes(bytes.try_into().expect("eight")));
        }
        entries += 1;
    }
    let mut terminals = blake3::Hasher::new();
    for (key, weight) in event.terminals() {
        terminals.update(powers(key).as_bytes());
        if let Some(weight) = weight {
            hash_coefficient(&mut terminals, weight);
        }
        terminals.update(b"\n");
    }
    let mut guards = blake3::Hasher::new();
    for guard in event.nonzero_conditions() {
        hash_coefficient(&mut guards, guard);
    }
    json!({
        "frontier_entries": entries,
        "frontier_weights": weights,
        "frontier_blake3": frontier.finalize().to_hex().to_string(),
        "frontier_set": set.iter().map(|word| format!("{word:016x}")).collect::<String>(),
        "terminals": event.terminals().len(),
        "terminals_blake3": terminals.finalize().to_hex().to_string(),
        "nonzero_conditions": event.nonzero_conditions().len(),
        "nonzero_conditions_blake3": guards.finalize().to_hex().to_string(),
    })
}

fn progress_json(progress: &CandidateOrderedReductionProgress) -> Value {
    json!({
        "completed_phases": progress.completed_phases,
        "total_phases": progress.total_phases,
        "input_keys": progress.input_keys,
        "processed_keys": progress.processed_keys,
        "emitted_terms": progress.emitted_terms,
        "merged_terms": progress.merged_terms,
        "cancelled_terms": progress.cancelled_terms,
        "terminal_merges": progress.terminal_merges,
        "terminal_cancellations": progress.terminal_cancellations,
        "frontier_keys": progress.frontier_keys,
        "peak_frontier_keys": progress.peak_frontier_keys,
        "terminal_keys": progress.terminal_keys,
        "frontier_coefficient_monomials": progress.frontier_coefficient_monomials,
        "peak_frontier_coefficient_monomials": progress.peak_frontier_coefficient_monomials,
        "terminal_coefficient_monomials": progress.terminal_coefficient_monomials,
        "peak_terminal_coefficient_monomials": progress.peak_terminal_coefficient_monomials,
    })
}

fn hash_integer(hasher: &mut blake3::Hasher, value: &symbolica::domains::integer::Integer) {
    use symbolica::domains::integer::Integer;
    match value {
        Integer::Single(value) => {
            hasher.update(&[0]);
            hasher.update(&value.to_le_bytes());
        }
        other => {
            hasher.update(&[1, u8::from(other.is_negative())]);
            let mut magnitude = Vec::new();
            symbolica::domains::backend::integer::write_lsf_bytes(
                &other.clone().to_multi_prec(),
                &mut magnitude,
            );
            hasher.update(&(magnitude.len() as u64).to_le_bytes());
            hasher.update(&magnitude);
        }
    }
}

/// Exact structural digest of one coefficient (no printing).
fn hash_coefficient_exact(hasher: &mut blake3::Hasher, coefficient: &Coefficient) {
    for polynomial in [&coefficient.numerator, &coefficient.denominator] {
        hasher.update(&(polynomial.nterms() as u64).to_le_bytes());
        for value in &polynomial.coefficients {
            hash_integer(hasher, value);
        }
        for exponent in &polynomial.exponents {
            hasher.update(&exponent.to_le_bytes());
        }
    }
}

/// One digest of the complete scheduler state at one phase boundary: pending
/// identities with exact weights in pop order, terminals, guards in order.
fn cache_state_digest(event: &OrderedPhaseEvent<'_>) -> Value {
    let mut hasher = blake3::Hasher::new();
    let (mut entries, mut weighted) = (0usize, 0usize);
    for entry in event.frontier() {
        let stage = match entry.stage {
            OrderedKeyStage::Integral => "integral".to_owned(),
            OrderedKeyStage::Route => "route".to_owned(),
            OrderedKeyStage::Apply { owner_sector } => format!("apply {}", mask(owner_sector)),
            OrderedKeyStage::ClassRoute { class } => format!("class-route {class}"),
            OrderedKeyStage::ClassApply { class } => format!("class-apply {class}"),
        };
        hasher.update(format!("f {} {stage} {}\n", entry.phase, powers(entry.integral)).as_bytes());
        if let Some(weight) = entry.weight {
            hash_coefficient_exact(&mut hasher, weight);
            weighted += 1;
        }
        entries += 1;
    }
    let terminals = event.terminals().len();
    for (key, weight) in event.terminals() {
        hasher.update(format!("t {}\n", powers(key)).as_bytes());
        if let Some(weight) = weight {
            hash_coefficient_exact(&mut hasher, weight);
        }
    }
    for guard in event.nonzero_conditions() {
        hasher.update(b"g");
        hash_coefficient_exact(&mut hasher, guard);
    }
    json!({
        "blake3": hasher.finalize().to_hex().to_string(),
        "frontier": entries,
        "weighted": weighted,
        "terminals": terminals,
        "guards": event.nonzero_conditions().len(),
    })
}

fn step_cache_options(options: &Options) -> OrderedStepCacheOptions {
    OrderedStepCacheOptions {
        populate: !options.step_cache_read_only,
        ..Default::default()
    }
}

/// Open the program's step cache when `--step-cache` is given.
fn open_step_cache(
    options: &Options,
    binding: OrderedStepCacheBinding,
) -> Result<Option<Arc<OrderedStepCache>>> {
    let Some(directory) = &options.step_cache else {
        return Ok(None);
    };
    let started = Instant::now();
    let cache = OrderedStepCache::open(directory, binding, step_cache_options(options))?;
    let statistics = cache.statistics();
    emit(json!({"event": "step_cache_open", "directory": directory,
        "seconds": started.elapsed().as_secs_f64(),
        "schema": rustred::reduction::ORDERED_STEP_CACHE_SCHEMA,
        "semantics": rustred::reduction::ORDERED_STEP_SEMANTICS,
        "stamp": cache.stamp().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "shards_on_disk": statistics.shards.len(), "file_bytes": statistics.file_bytes}));
    Ok(Some(cache))
}

/// Save (when requested) and report the cache after a run.
fn report_step_cache(options: &Options, cache: Option<&OrderedStepCache>) -> Result<()> {
    let Some(cache) = cache else {
        return Ok(());
    };
    let save = if options.step_cache_save {
        let started = Instant::now();
        let report = cache.save()?;
        json!({"seconds": started.elapsed().as_secs_f64(), "shards_written": report.shards_written,
            "records_written": report.records_written, "bytes_written": report.bytes_written,
            "shards": report.shards, "records": report.records, "bytes": report.bytes,
            "tail_entries": report.tail_entries, "tail_bytes": report.tail_bytes})
    } else {
        Value::Null
    };
    let statistics = cache.statistics();
    let shards: Vec<Value> = statistics
        .shards
        .iter()
        .map(|shard| {
            json!({"shard": shard.shard.to_string(), "state": format!("{:?}", shard.state),
                "records": shard.records, "loaded_records": shard.loaded_records,
                "hits": shard.hits, "misses": shard.misses, "inserts": shard.inserts,
                "skipped_inserts": shard.skipped_inserts, "opaque_hits": shard.opaque_hits,
                "resident_bytes": shard.resident_bytes, "file_bytes": shard.file_bytes,
                "load_seconds": shard.load_seconds})
        })
        .collect();
    emit(
        json!({"event": "step_cache", "hits": statistics.hits, "misses": statistics.misses,
        "inserts": statistics.inserts, "skipped_inserts": statistics.skipped_inserts,
        "records": statistics.records, "resident_bytes": statistics.resident_bytes,
        "file_bytes": statistics.file_bytes, "load_seconds": statistics.load_seconds,
        "loaded_shards": statistics.loaded_shards,
        "skipped_shards": statistics.skipped_shards, "rejected_shards": statistics.rejected_shards,
        "factor_basis": statistics.factor_basis, "budget_exhausted": statistics.budget_exhausted,
        "save": save, "shards": shards,
        "tail": cache.tail_statistics().map(|tail| json!({"entries": tail.entries,
            "values": tail.values, "guards": tail.guards, "guard_sets": tail.guard_sets,
            "terminals": tail.terminals, "loaded_entries": tail.loaded_entries,
            "edges": tail.edges, "coefficients": tail.coefficients,
            "file_bytes": tail.file_bytes, "load_seconds": tail.load_seconds,
            "dirty": tail.dirty, "rejected": tail.rejected}))}),
    );
    Ok(())
}

/// Vakint's sibling `.result.json` and `.phases.jsonl`, when present.
struct Recorded {
    result: Option<Value>,
    phases: BTreeMap<usize, Value>,
}

impl Recorded {
    fn load(request: &Path) -> Result<Self> {
        let name = request.to_string_lossy();
        let stem = name.strip_suffix(".request.json").unwrap_or(&name);
        let result = std::fs::read(format!("{stem}.result.json"))
            .ok()
            .map(|bytes| serde_json::from_slice(&bytes))
            .transpose()?;
        let mut phases = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(format!("{stem}.phases.jsonl")) {
            for line in text.lines().filter(|line| !line.is_empty()) {
                let record: Value = serde_json::from_str(line)?;
                let phase = record["phase"]
                    .as_u64()
                    .ok_or("phase record has no index")?;
                phases.insert(phase as usize, record);
            }
        }
        Ok(Self { result, phases })
    }
}

/// Inspector state: phase timing, counter deltas, capture and stop control.
struct PhaseRecorder<'a> {
    options: &'a Options,
    recorded: &'a Recorded,
    started: Instant,
    phase_started: Instant,
    at_start: CandidateOrderedReductionProgress,
    phase_keys: usize,
    capture_seconds: f64,
    compared: usize,
    mismatched: Vec<usize>,
    stopped: Option<usize>,
    failure: Option<String>,
    last: Option<CandidateOrderedReductionProgress>,
}

impl<'a> PhaseRecorder<'a> {
    fn new(options: &'a Options, recorded: &'a Recorded) -> Self {
        Self {
            options,
            recorded,
            started: Instant::now(),
            phase_started: Instant::now(),
            at_start: CandidateOrderedReductionProgress::default(),
            phase_keys: 0,
            capture_seconds: 0.0,
            compared: 0,
            mismatched: Vec::new(),
            stopped: None,
            failure: None,
            last: None,
        }
    }

    /// `Ok(false)` asks the adapter to stop the request.
    fn inspect(&mut self, event: &OrderedPhaseEvent<'_>) -> bool {
        match self.observe(event) {
            Ok(proceed) => proceed,
            Err(error) => {
                self.failure = Some(error.to_string());
                false
            }
        }
    }

    fn observe(&mut self, event: &OrderedPhaseEvent<'_>) -> Result<bool> {
        self.last = Some(event.progress);
        match event.boundary {
            OrderedPhaseBoundary::Started => {
                if self
                    .options
                    .capture
                    .as_ref()
                    .is_some_and(|(phase, _)| *phase == event.phase)
                {
                    self.capture(event)?;
                    // A stop after the previous phase was deferred to this capture.
                    if let Some(previous) = event.phase.checked_sub(1)
                        && self.options.stop_after_phase == Some(previous)
                    {
                        self.stopped = Some(previous);
                        return Ok(false);
                    }
                }
                self.phase_keys = event
                    .frontier()
                    .take_while(|entry| entry.phase == event.phase)
                    .count();
                self.at_start = event.progress;
                self.phase_started = Instant::now();
                Ok(true)
            }
            OrderedPhaseBoundary::Finished => {
                let wall = self.phase_started.elapsed().as_secs_f64();
                let (digest, digests) = if self.options.phase_digests {
                    let started = Instant::now();
                    let digest = cache_state_digest(event);
                    let digests = state_digest(event);
                    let seconds = started.elapsed().as_secs_f64();
                    self.capture_seconds += seconds;
                    (json!({"state": digest, "seconds": seconds}), digests)
                } else {
                    (Value::Null, Value::Null)
                };
                let (before, after) = (&self.at_start, &event.progress);
                let recorded = self.recorded.phases.get(&event.phase);
                let matches = recorded.map(|record| record["progress"] == progress_json(after));
                if let Some(matches) = matches {
                    self.compared += 1;
                    if !matches {
                        self.mismatched.push(event.phase);
                    }
                }
                let descriptor = &event.descriptor;
                emit(json!({
                    "event": "phase",
                    "phase": event.phase,
                    "kind": match descriptor.kind {
                        rustred::solver::CandidateOrderedPhaseKind::Routing => "route",
                        rustred::solver::CandidateOrderedPhaseKind::Reduction => "apply",
                    },
                    "support": descriptor.support,
                    "owner_index": descriptor.owner_index,
                    "owner": descriptor.owner_sector.map(mask),
                    "wall_seconds": wall,
                    "elapsed_seconds": self.started.elapsed().as_secs_f64() - self.capture_seconds,
                    "phase_keys_at_start": self.phase_keys,
                    "processed_keys": after.processed_keys - before.processed_keys,
                    "emitted_terms": after.emitted_terms - before.emitted_terms,
                    "merged_terms": after.merged_terms - before.merged_terms,
                    "cancelled_terms": after.cancelled_terms - before.cancelled_terms,
                    "terminal_merges": after.terminal_merges - before.terminal_merges,
                    "terminal_cancellations":
                        after.terminal_cancellations - before.terminal_cancellations,
                    "frontier_keys": after.frontier_keys,
                    "frontier_coefficient_monomials": after.frontier_coefficient_monomials,
                    "peak_frontier_keys": after.peak_frontier_keys,
                    "peak_frontier_coefficient_monomials":
                        after.peak_frontier_coefficient_monomials,
                    "terminal_keys": after.terminal_keys,
                    "terminal_coefficient_monomials": after.terminal_coefficient_monomials,
                    "progress": progress_json(after),
                    "vakint_counters_match": matches,
                    "vakint_elapsed_seconds": recorded.map(|record| &record["elapsed_seconds"]),
                    "digests": digests,
                    "step_cache_hits": after.step_cache_hits - before.step_cache_hits,
                    "step_cache_misses": after.step_cache_misses - before.step_cache_misses,
                    "digest": digest,
                }));
                let capture_next = self
                    .options
                    .capture
                    .as_ref()
                    .is_some_and(|(phase, _)| *phase == event.phase + 1);
                if self.options.stop_after_phase == Some(event.phase) && !capture_next {
                    self.stopped = Some(event.phase);
                    return Ok(false);
                }
                Ok(true)
            }
        }
    }

    fn capture(&mut self, event: &OrderedPhaseEvent<'_>) -> Result<()> {
        let started = Instant::now();
        let (_, path) = self
            .options
            .capture
            .as_ref()
            .expect("capture was requested");
        let mut writer = BufWriter::new(File::create(path)?);
        let descriptor = &event.descriptor;
        writeln!(
            writer,
            "{}",
            json!({
                "schema": CAPTURE_SCHEMA,
                "request": self.options.request,
                "phase": event.phase,
                "support": descriptor.support,
                "owner_index": descriptor.owner_index,
                "owner": descriptor.owner_sector.map(mask),
                "progress": progress_json(&event.progress),
                "terminals": event.terminals().len(),
                "nonzero_conditions": event.nonzero_conditions().len(),
            })
        )?;
        let mut entries = 0usize;
        for entry in event.frontier() {
            let (stage, owner) = match entry.stage {
                OrderedKeyStage::Integral => ("integral", None),
                OrderedKeyStage::Route => ("route", None),
                OrderedKeyStage::Apply { owner_sector } => ("apply", Some(mask(owner_sector))),
                OrderedKeyStage::ClassRoute { class } => ("class-route", Some(class.to_string())),
                OrderedKeyStage::ClassApply { class } => ("class-apply", Some(class.to_string())),
            };
            writeln!(
                writer,
                "{}",
                json!({"kind": "frontier", "phase": entry.phase, "stage": stage, "owner": owner,
                    "powers": entry.integral.powers(), "weight": entry.weight.map(canonical)})
            )?;
            entries += 1;
        }
        for (key, coefficient) in event.terminals() {
            writeln!(
                writer,
                "{}",
                json!({"kind": "terminal", "powers": key.powers(),
                    "coefficient": coefficient.map(canonical)})
            )?;
        }
        for condition in event.nonzero_conditions() {
            writeln!(
                writer,
                "{}",
                json!({"kind": "condition", "value": canonical(condition)})
            )?;
        }
        writer.flush()?;
        let seconds = started.elapsed().as_secs_f64();
        self.capture_seconds += seconds;
        emit(
            json!({"event": "capture", "phase": event.phase, "file": path,
            "frontier_entries": entries, "terminals": event.terminals().len(),
            "nonzero_conditions": event.nonzero_conditions().len(), "seconds": seconds}),
        );
        Ok(())
    }

    /// Classify the outcome of an inspected request; a requested stop is success.
    fn conclude<T, E: std::fmt::Display>(
        &self,
        result: std::result::Result<T, E>,
    ) -> Result<Option<T>> {
        if let Some(failure) = &self.failure {
            return Err(format!("phase inspection failed: {failure}").into());
        }
        match (result, self.stopped) {
            (Ok(value), _) => Ok(Some(value)),
            (Err(_), Some(phase)) => {
                emit(json!({"event": "stopped", "after_phase": phase,
                    "native_seconds": self.started.elapsed().as_secs_f64() - self.capture_seconds,
                    "progress": self.last.as_ref().map(progress_json),
                    "vakint_phase_records_compared": self.compared,
                    "vakint_phase_mismatches": self.mismatched}));
                Ok(None)
            }
            (Err(error), None) => Err(error.to_string().into()),
        }
    }
}

/// Final record: native timing, exact output digests and Vakint comparison.
fn finish(
    recorded: &Recorded,
    recorder: Option<&PhaseRecorder<'_>>,
    native_seconds: f64,
    output: Value,
    progress: Option<CandidateOrderedReductionProgress>,
) -> Result<()> {
    let expected = recorded.result.as_ref().map(|result| &result["output"]);
    let terms_match = expected.map(|expected| expected["terms_blake3"] == output["terms_blake3"]);
    let guards_match = expected.map(|expected| {
        expected["nonzero_conditions_blake3"] == output["nonzero_conditions_blake3"]
    });
    emit(json!({
        "event": "finished",
        "native_seconds": native_seconds,
        "output": output,
        "progress": progress.as_ref().map(progress_json),
        "vakint_wall_seconds": recorded.result.as_ref().map(|result| &result["wall_seconds"]),
        "vakint_terms_match": terms_match,
        "vakint_nonzero_conditions_match": guards_match,
        "vakint_phase_records_compared": recorder.map(|recorder| recorder.compared),
        "vakint_phase_mismatches": recorder.map(|recorder| &recorder.mismatched),
        "step_cache_hits": progress.as_ref().map(|progress| progress.step_cache_hits),
        "step_cache_misses": progress.as_ref().map(|progress| progress.step_cache_misses),
    }));
    if terms_match == Some(false) {
        return Err("replayed output terms differ from Vakint's recorded output".into());
    }
    if recorder.is_some_and(|recorder| {
        !recorder.mismatched.is_empty() && !recorder.options.ignore_recorded_counters
    }) {
        return Err("replayed phase counters differ from Vakint's recorded counters".into());
    }
    Ok(())
}

/// Resolve one recorded program file: its absolute path, else `--data-root`.
fn program_file(request: &Value, options: &Options, name: &str) -> Result<PathBuf> {
    let file = &request["program"]["files"][name];
    if let Some(path) = file["path"].as_str().map(PathBuf::from)
        && path.exists()
    {
        return Ok(path);
    }
    let root = options
        .data_root
        .as_ref()
        .ok_or(format!("{name} has no recorded path; pass --data-root"))?;
    let loop_count = request["loop_count"]
        .as_u64()
        .ok_or("request has no loop count")?;
    Ok(root.join(format!("{loop_count}L")).join(name))
}

fn main() -> Result<()> {
    let options = Options::parse(std::env::args().skip(1))?;
    let request: Value = serde_json::from_slice(&std::fs::read(&options.request)?)?;
    if request["schema"] != REQUEST_SCHEMA {
        return Err(format!("unsupported request schema {}", request["schema"]).into());
    }
    let recorded = Recorded::load(&options.request)?;
    emit(json!({
        "event": "request",
        "file": options.request,
        "loop_count": request["loop_count"],
        "program": request["program"]["kind"],
        "family_fingerprint": request["program"]["family_fingerprint"],
        "recorded_strategy": request["reduction_strategy"],
        "replay_strategy": format!("{:?}", options.strategy),
        "arithmetic": options.arithmetic.to_string(),
        "channel": request["channel"],
        "input_terms": request["input_terms"],
        "vakint_result_recorded": recorded.result.is_some(),
        "vakint_phase_records": recorded.phases.len(),
        "reduction_limits_are_default": request["reduction_limits_are_default"],
    }));
    if request["reduction_limits_are_default"] != true {
        return Err(
            "the request used non-default reduction limits, which replay does not set".into(),
        );
    }
    match request["program"]["kind"].as_str() {
        Some("saved-application") => saved::replay(&request, &options, &recorded),
        Some("closed-artifact") => closed::replay(&request, &options, &recorded),
        Some("candidate-program") => candidate::replay(&request, &options, &recorded),
        other => Err(format!("unsupported program kind {other:?}").into()),
    }
}

mod saved {
    use super::*;
    use rustred_app::{SavedApplicationOptions, load_saved_application, saved_application_arity};

    pub(super) fn replay(request: &Value, options: &Options, recorded: &Recorded) -> Result<()> {
        let program = &request["program"];
        let path = PathBuf::from(program["path"].as_str().ok_or("no application path")?);
        let path = if path.exists() {
            path
        } else {
            let root = options
                .data_root
                .as_ref()
                .ok_or("application path is missing")?;
            root.join("5L").join("ibps")
        };
        let arity = saved_application_arity(&path)?;
        macro_rules! dispatch {
            ($($n:literal),*) => {
                match rustred::campaign_storage_arity(arity) {
                    $($n => run::<$n>(request, options, recorded, &path),)*
                    _ => Err("application arity is not compiled".into()),
                }
            }
        }
        rustred::with_app_runtime_arities!(dispatch)
    }

    fn run<const N: usize>(
        request: &Value,
        options: &Options,
        recorded: &Recorded,
        path: &Path,
    ) -> Result<()> {
        if request["program"]["options_are_defaults_with_preserve"] != true {
            return Err("recorded saved-application options are not replayable defaults".into());
        }
        let started = Instant::now();
        let mut prepared = load_saved_application::<N>(
            path,
            SavedApplicationOptions {
                unresolved_policy: UnresolvedPolicy::Preserve,
                preparation_workers: request["program"]["preparation_workers"]
                    .as_u64()
                    .unwrap_or(1) as usize,
                ..Default::default()
            },
        )?;
        if Some(prepared.family().fingerprint())
            != request["program"]["family_fingerprint"].as_str()
        {
            return Err("saved application family differs from the recorded request".into());
        }
        let inputs = inputs(request, prepared.family().coefficient_context())?;
        let owners = prepared.owner_preparation_statistics();
        emit(
            json!({"event": "loaded", "path": path, "seconds": started.elapsed().as_secs_f64(),
            "inputs": inputs.len(), "declared_owners": owners.declared_owners,
            "prepared_owners": owners.prepared_owners,
            "route_preparation": prepared.route_preparation().to_json()}),
        );
        prepared.set_reduction_strategy(options.strategy);
        prepared.set_ordered_weight_arithmetic(options.arithmetic);
        prepared.set_ordered_scheduler_options(options.scheduler);
        if let Some(layering) = options.route_layers {
            prepared.set_ordered_route_layering(layering);
        }
        emit(
            json!({"event": "route_layers", "layering": prepared.ordered_route_layering().as_str(),
            "adjoint_tail": prepared.ordered_scheduler_options().adjoint_tail}),
        );
        let cache = open_step_cache(options, prepared.ordered_step_cache_binding())?;
        prepared.set_ordered_step_cache(cache.clone())?;
        let owners_before = prepared.owner_preparation_statistics().preparation_time;
        let native = Instant::now();
        let (reduced, recorder) = if options.strategy == ReductionStrategy::Ordered {
            let mut recorder = PhaseRecorder::new(options, recorded);
            let result = prepared.reduce_routed_linear_combination_with_ordered_inspector(
                inputs,
                &mut |event| {
                    if recorder.inspect(event) {
                        Ok(())
                    } else {
                        Err(CandidateRoutedError::Cancelled)
                    }
                },
            );
            match recorder.conclude(result)? {
                Some(reduced) => (reduced, Some(recorder)),
                None => return report_step_cache(options, cache.as_deref()),
            }
        } else {
            (prepared.reduce_routed_linear_combination(inputs)?, None)
        };
        let native_seconds = native.elapsed().as_secs_f64()
            - recorder
                .as_ref()
                .map_or(0.0, |recorder| recorder.capture_seconds);
        let owners = prepared.owner_preparation_statistics();
        emit(json!({"event": "owner_preparation",
            "seconds": (owners.preparation_time - owners_before).as_secs_f64(),
            "prepared_owners": owners.prepared_owners, "declared_owners": owners.declared_owners}));
        report_step_cache(options, cache.as_deref())?;
        let output = output_digest(
            reduced.terms().iter().map(|(terminal, coefficient)| {
                (
                    terminal.family_fingerprint(),
                    terminal.integral(),
                    coefficient,
                )
            }),
            reduced.nonzero_conditions(),
        );
        let last = recorder.as_ref().and_then(|recorder| recorder.last);
        finish(recorded, recorder.as_ref(), native_seconds, output, last)
    }
}

mod closed {
    use super::*;
    use rustred::foundry::artifact::ClosedArtifact;
    use rustred::reduction::Reducer;

    pub(super) fn replay(request: &Value, options: &Options, recorded: &Recorded) -> Result<()> {
        let path = program_file(request, options, "ibps/rules.rrbin")?;
        let started = Instant::now();
        let bytes = std::fs::read(&path)?;
        let artifact = ClosedArtifact::decode_durable(&bytes)?;
        if Some(artifact.family_fingerprint()) != request["program"]["family_fingerprint"].as_str()
        {
            return Err("closed artifact family differs from the recorded request".into());
        }
        let inputs = inputs(request, artifact.coefficient_context())?;
        emit(
            json!({"event": "loaded", "path": path, "seconds": started.elapsed().as_secs_f64(),
            "inputs": inputs.len(),
            "expected_sha256": request["program"]["artifact_sha256"]}),
        );
        let limits = ReductionLimits::default();
        let cache = open_step_cache(
            options,
            OrderedStepCacheBinding::new(
                OrderedStepCacheAdapter::ClosedArtifact,
                artifact.family_fingerprint(),
                artifact.coefficient_context(),
                None,
            ),
        )?;
        let native = Instant::now();
        let family = artifact.family_fingerprint();
        match options.strategy {
            ReductionStrategy::Ordered => {
                let mut recorder = PhaseRecorder::new(options, recorded);
                let result = Reducer::reduce_artifact_linear_combination_ordered_with_step_cache(
                    &artifact,
                    inputs,
                    limits,
                    options.arithmetic,
                    cache.as_deref(),
                    &mut |_, _| Ok(()),
                    Some(&mut |event| {
                        if recorder.inspect(event) {
                            Ok(())
                        } else {
                            Err(ReductionError::ReducerInvariant {
                                detail: STOP_DETAIL,
                            })
                        }
                    }),
                );
                let Some(reduced) = recorder.conclude(result)? else {
                    return report_step_cache(options, cache.as_deref());
                };
                let native_seconds = native.elapsed().as_secs_f64() - recorder.capture_seconds;
                let output = output_digest(
                    reduced
                        .terms
                        .iter()
                        .map(|(key, value)| (family, key, value)),
                    &reduced.nonzero_conditions,
                );
                report_step_cache(options, cache.as_deref())?;
                finish(
                    recorded,
                    Some(&recorder),
                    native_seconds,
                    output,
                    Some(reduced.progress),
                )
            }
            ReductionStrategy::DepthFirstWithoutMemoization => {
                let reduced = Reducer::reduce_artifact_linear_combination_without_memoization(
                    &artifact, inputs, limits,
                )?;
                let output = output_digest(
                    reduced
                        .terms
                        .iter()
                        .map(|(key, value)| (family, key, value)),
                    &reduced.nonzero_conditions,
                );
                finish(recorded, None, native.elapsed().as_secs_f64(), output, None)
            }
            ReductionStrategy::DepthFirst => {
                Err("closed artifacts replay ordered or depth_first_without_memoization".into())
            }
        }
    }
}

mod candidate {
    use super::*;
    use rustred::persistence::BinaryIoLimits;
    use rustred::reduction::terminal_normalization::{
        TerminalNormalizationLimits, TerminalNormalizationPlan,
    };
    use rustred::solver::CandidateReductionError;
    use rustred_app::{
        CandidateBundleLimits, MAX_CANDIDATE_BUNDLE_BYTES, load_generated_candidate_bundle,
    };
    use std::io::Read;

    /// Vakint's shipped-data policy for 4L candidate programs.
    const MAX_COEFFICIENT_BYTES: usize = 512 * 1024 * 1024;

    fn gunzip(path: &Path) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(File::open(path)?)
            .take(MAX_CANDIDATE_BUNDLE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_CANDIDATE_BUNDLE_BYTES {
            return Err("decompressed candidate program exceeds the bundle limit".into());
        }
        Ok(bytes)
    }

    pub(super) fn replay(request: &Value, options: &Options, recorded: &Recorded) -> Result<()> {
        if options.strategy != ReductionStrategy::Ordered {
            return Err("candidate programs replay only the ordered strategy".into());
        }
        let label = request["program"]["label"]
            .as_str()
            .ok_or("no program label")?;
        let name = label.trim_start_matches("four-loop-");
        let program = program_file(
            request,
            options,
            &format!("ibps/{name}.candidates.rrbin.gz"),
        )?;
        let normalization = program_file(request, options, &format!("ibps/{name}.rrnorm.bin"))?;
        let started = Instant::now();
        let program_bytes = gunzip(&program)?;
        let normalization_bytes = std::fs::read(&normalization)?;
        let (family, mut reducer) = load_generated_candidate_bundle::<10>(
            &program_bytes,
            CandidateBundleLimits {
                max_bundle_bytes: MAX_CANDIDATE_BUNDLE_BYTES,
                max_collection_entries: 8_000_000,
                max_total_coefficient_bytes: MAX_COEFFICIENT_BYTES,
                ..CandidateBundleLimits::default()
            },
            ReductionLimits::default(),
        )?;
        if Some(family.fingerprint()) != request["program"]["family_fingerprint"].as_str() {
            return Err("candidate program family differs from the recorded request".into());
        }
        let plan = TerminalNormalizationPlan::decode_generated(
            &normalization_bytes,
            &family,
            reducer.terminals(),
            reducer.ordering(),
            TerminalNormalizationLimits::default(),
            BinaryIoLimits::default(),
        )?;
        reducer.install_terminal_normalization(plan)?;
        reducer.set_unresolved_policy(UnresolvedPolicy::Preserve)?;
        let mut reducer = reducer.fork_empty_with_limits(ReductionLimits::default());
        // The binding names the candidate program's family, variables and
        // policy; one cache directory per program and normalization.
        let cache = open_step_cache(options, reducer.ordered_step_cache_binding())?;
        reducer.set_ordered_step_cache(cache.clone())?;
        let inputs = inputs(request, family.coefficient_context())?;
        emit(
            json!({"event": "loaded", "program": program, "normalization": normalization,
            "seconds": started.elapsed().as_secs_f64(), "inputs": inputs.len()}),
        );
        let native = Instant::now();
        let mut recorder = PhaseRecorder::new(options, recorded);
        let mut inspector = |event: &OrderedPhaseEvent<'_>| {
            if recorder.inspect(event) {
                Ok(())
            } else {
                Err(CandidateReductionError::Application(
                    ReductionError::ReducerInvariant {
                        detail: STOP_DETAIL,
                    },
                ))
            }
        };
        let result = reducer.reduce_linear_combination_ordered_with_scheduler(
            inputs,
            options.arithmetic,
            options.scheduler,
            None,
            Some(&mut inspector),
        );
        let Some(reduced) = recorder.conclude(result)? else {
            return report_step_cache(options, cache.as_deref());
        };
        let native_seconds = native.elapsed().as_secs_f64() - recorder.capture_seconds;
        let output = output_digest(
            reduced
                .terms()
                .iter()
                .map(|(key, value)| (reduced.family_fingerprint(), key, value)),
            reduced.nonzero_conditions(),
        );
        report_step_cache(options, cache.as_deref())?;
        finish(
            recorded,
            Some(&recorder),
            native_seconds,
            output,
            Some(*reduced.progress()),
        )
    }
}
