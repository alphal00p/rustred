//! Replay one native reduction request recorded by Vakint, with RustRed alone.
//!
//! `ordered_replay REQUEST.json [--data-root DIR] [--strategy NAME]
//!     [--stop-after-phase N] [--capture-frontier-at-phase N FILE] [--threads N]`
//!
//! Vakint writes `REQUEST.json` (schema `vakint.ordered-native-request.v1`) when
//! `VAKINT_DUMP_ORDERED_REQUEST=<dir>` is set, plus `.phases.jsonl` (native
//! counters at every phase end) and `.result.json` (digests of the exact native
//! output) next to it. This example loads the same native program, parses the
//! exact inputs (their canonical round trip is checked), runs the request and
//! prints JSONL records on stdout:
//!
//! * `request`/`loaded`: identity, input count and program loading time;
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
//! two print only the final record. `--threads` is reserved and must be 1.
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::reduction::{
    OrderedKeyStage, OrderedPhaseBoundary, OrderedPhaseEvent, OrderedWeightArithmetic,
    ReductionError, ReductionLimits, ReductionStrategy,
};
use rustred::solver::{CandidateOrderedReductionProgress, CandidateRoutedError, UnresolvedPolicy};
use serde_json::{Value, json};
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
    stop_after_phase: Option<usize>,
    capture: Option<(usize, PathBuf)>,
    threads: usize,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let mut request = None;
        let mut options = Options {
            request: PathBuf::new(),
            data_root: None,
            strategy: ReductionStrategy::Ordered,
            stop_after_phase: None,
            capture: None,
            threads: 1,
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
                "--stop-after-phase" => {
                    options.stop_after_phase = Some(value("--stop-after-phase")?.parse()?)
                }
                "--capture-frontier-at-phase" => {
                    let phase = value("--capture-frontier-at-phase")?.parse()?;
                    options.capture = Some((phase, value("--capture-frontier-at-phase")?.into()));
                }
                "--threads" => options.threads = value("--threads")?.parse()?,
                _ if request.is_none() && !arg.starts_with("--") => request = Some(arg.into()),
                _ => return Err(format!("unexpected argument {arg}").into()),
            }
        }
        options.request = request.ok_or(
            "usage: ordered_replay REQUEST.json [--data-root DIR] [--strategy NAME] \
             [--stop-after-phase N] [--capture-frontier-at-phase N FILE] [--threads N]",
        )?;
        if options.threads != 1 {
            return Err("--threads is reserved for parallel engines; only 1 is supported".into());
        }
        let phased = options.stop_after_phase.is_some() || options.capture.is_some();
        if phased && options.strategy != ReductionStrategy::Ordered {
            return Err("phase options require --strategy ordered".into());
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
    }));
    if terms_match == Some(false) {
        return Err("replayed output terms differ from Vakint's recorded output".into());
    }
    if recorder.is_some_and(|recorder| !recorder.mismatched.is_empty()) {
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
            "prepared_owners": owners.prepared_owners}),
        );
        prepared.set_reduction_strategy(options.strategy);
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
                None => return Ok(()),
            }
        } else {
            (prepared.reduce_routed_linear_combination(inputs)?, None)
        };
        let native_seconds = native.elapsed().as_secs_f64()
            - recorder
                .as_ref()
                .map_or(0.0, |recorder| recorder.capture_seconds);
        emit(json!({"event": "owner_preparation",
            "seconds": (prepared.owner_preparation_statistics().preparation_time - owners_before)
                .as_secs_f64()}));
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
        let artifact = ClosedArtifact::decode_durable(&std::fs::read(&path)?)?;
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
        let native = Instant::now();
        let family = artifact.family_fingerprint();
        match options.strategy {
            ReductionStrategy::Ordered => {
                let mut recorder = PhaseRecorder::new(options, recorded);
                let result = Reducer::reduce_artifact_linear_combination_ordered_with_inspector(
                    &artifact,
                    inputs,
                    limits,
                    OrderedWeightArithmetic::Exact,
                    &mut |event| {
                        if recorder.inspect(event) {
                            Ok(())
                        } else {
                            Err(ReductionError::ReducerInvariant {
                                detail: STOP_DETAIL,
                            })
                        }
                    },
                );
                let Some(reduced) = recorder.conclude(result)? else {
                    return Ok(());
                };
                let native_seconds = native.elapsed().as_secs_f64() - recorder.capture_seconds;
                let output = output_digest(
                    reduced
                        .terms
                        .iter()
                        .map(|(key, value)| (family, key, value)),
                    &reduced.nonzero_conditions,
                );
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
        let (family, mut reducer) = load_generated_candidate_bundle::<10>(
            &gunzip(&program)?,
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
            &std::fs::read(&normalization)?,
            &family,
            reducer.terminals(),
            reducer.ordering(),
            TerminalNormalizationLimits::default(),
            BinaryIoLimits::default(),
        )?;
        reducer.install_terminal_normalization(plan)?;
        reducer.set_unresolved_policy(UnresolvedPolicy::Preserve)?;
        let reducer = reducer.fork_empty_with_limits(ReductionLimits::default());
        let inputs = inputs(request, family.coefficient_context())?;
        emit(
            json!({"event": "loaded", "program": program, "normalization": normalization,
            "seconds": started.elapsed().as_secs_f64(), "inputs": inputs.len()}),
        );
        let native = Instant::now();
        let mut recorder = PhaseRecorder::new(options, recorded);
        let result = reducer.reduce_linear_combination_ordered_with_inspector(
            inputs,
            OrderedWeightArithmetic::Exact,
            &mut |event| {
                if recorder.inspect(event) {
                    Ok(())
                } else {
                    Err(CandidateReductionError::Application(
                        ReductionError::ReducerInvariant {
                            detail: STOP_DETAIL,
                        },
                    ))
                }
            },
        );
        let Some(reduced) = recorder.conclude(result)? else {
            return Ok(());
        };
        let native_seconds = native.elapsed().as_secs_f64() - recorder.capture_seconds;
        let output = output_digest(
            reduced
                .terms()
                .iter()
                .map(|(key, value)| (reduced.family_fingerprint(), key, value)),
            reduced.nonzero_conditions(),
        );
        finish(
            recorded,
            Some(&recorder),
            native_seconds,
            output,
            Some(*reduced.progress()),
        )
    }
}
