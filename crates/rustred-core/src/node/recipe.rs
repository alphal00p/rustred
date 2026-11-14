//! Recipe rules (R48): solve a stored SOLVE RECIPE (explicit seed set + node order + elimination convention, no
//! coefficients) at integer keys, deterministically, with an exact proof of every reported relation.
//!
//! One call = one recipe, many keys. Per key: the domain test, the recipe's explicit seed list at the key
//! ([`build_seeds`]), the engine's own case search fed by exactly that list (`solver::with_explicit_seeds`: modular
//! first-hit discovery at a fixed prime and sample seed, exact lift of the trace with the witnessed target-only
//! factorized backend) and the exact product check `sum_j w_j row_j == lifted row` with the lift's own source
//! weights (`solver::with_proof_capture`, Symbolica rational-function arithmetic, no evaluation). Interface:
//! `plan/auto-campaign/interfaces/coord-recipe-solve.md` (lane engine-line). Generic in the arity (the node
//! driver's 5, 9, 12, 14, 20); nothing here is specific to a node.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::driver::{
    CaseSpec, build_case, compile_family, covers, numerical_backend, order_program,
    parse_index_poly, vmhwm_kb,
};
use super::order::NodeOrder;
use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::sector::{Mask, zero};
use crate::solver::{
    Case, CaseIntersectionLimits, SearchOptions, SectorConfig, SectorSolver, SolverError,
    SourceSystem, SymbolicExactBackend, with_explicit_seeds, with_proof_capture,
};

/// Version of the recipe file format.
pub const RECIPE_SCHEMA: &str = "rustred.recipe.v1";
/// Version of the elimination convention (seed order, rows, pivot rule, prime, sample seed, lift backend).
pub const ELIMINATION_CONVENTION: &str = "rustred.recipe-elim.v1";
/// The fixed modular prime of the discovery (2^61 - 1) and the fixed sample seed.
pub const PRIME: u64 = (1u64 << 61) - 1;
pub const SAMPLE_SEED: u64 = 0;
/// Identity of the whole convention: a cached relation is valid only for an equal id (and binary, recipe, key).
pub const DETERMINISM_ID: &str = "rustred.recipe-solve.v2/elim=rustred.recipe-elim.v1/prime=2305843009213693951/seed=0/backend=sparse-target-only-factorized/proof=exact-weights/relation-rule=recorded";
/// Upper limit of same-layer eliminations of the m1_first rule per key (then the first admissible relation is kept).
pub const M1_MAX_STEPS: usize = 256;

/// Which relation a solve takes (recipe field `relation_rule`, recorded in every solved record and in the relation
/// digest). `m1_first` (default): a relation whose top-sector right-hand-side keys are all strictly lower in D+r
/// (dots + rank) than the target, obtained from the first admissible (comparator) relation by eliminating its
/// same-layer top-sector keys highest first, each with its own solve from the SAME seed rows; if one of them has no
/// relation in these seeds, the first admissible relation is kept (`relation_rule_taken: "c_fallback"`).
/// `first_admissible`: the engine's first hit under the node comparator (v1 behaviour).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationRule {
    #[default]
    M1First,
    FirstAdmissible,
}

impl RelationRule {
    fn name(self) -> &'static str {
        match self {
            Self::M1First => "m1_first",
            Self::FirstAdmissible => "first_admissible",
        }
    }
}
/// Schema of a failure object (R48 addendum 3).
pub const FAILURE_SCHEMA: &str = "rustred.recipe-failure.v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    #[serde(default)]
    pub min: Vec<Option<i64>>,
    #[serde(default)]
    pub max: Vec<Option<i64>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Domain {
    /// fixed slot values (None = free); absent = every slot free
    #[serde(default)]
    pub fixed: Option<Vec<Option<i64>>>,
    /// index polynomials in n0.. that must vanish at the key
    #[serde(default)]
    pub equations: Vec<String>,
    /// OR of AND-branches; a key on any branch (every polynomial of it zero) is outside the domain
    #[serde(default)]
    pub exceptions: Vec<Vec<String>>,
    #[serde(default)]
    pub bounds: Option<Bounds>,
    /// v1.2: an explicit key set (per-KEY recipe assignment): when present the key must be one of these
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<Vec<i64>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ball {
    pub radius: u32,
    /// slots that may also move DOWN (one step = -unit); every slot may move up
    #[serde(default)]
    pub down: Vec<usize>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generator {
    /// named shells (checker convention): S0, S1, Sm1, S1m1, S2, S3, S2m1, Sm2
    #[serde(default)]
    pub shells: Option<Vec<String>>,
    #[serde(default)]
    pub ball: Option<Ball>,
}

fn default_unit() -> String {
    "harder".into()
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedSpec {
    /// "harder": one step = +1 on a chart line, -1 on any other slot
    #[serde(default = "default_unit")]
    pub unit: String,
    /// offsets from the key; empty = one centre at the key
    #[serde(default)]
    pub centres: Vec<Vec<i64>>,
    pub generator: Generator,
    /// explicit floor: drop a seed with seed[s] - key[s] < v (slot given as a decimal string)
    #[serde(default)]
    pub min_offset: BTreeMap<String, i64>,
    #[serde(default)]
    pub max_offset: BTreeMap<String, i64>,
    /// [D, R, B]: drop a seed whose dots exceed the key's by more than D (D + B for a dot-free key) or whose rank
    /// exceeds the key's by more than R
    #[serde(default)]
    pub layer_cap: Option<Vec<i64>>,
    /// drop seeds outside the chart's top sector (lines >= 1, other slots <= 0) ...
    #[serde(default = "yes")]
    pub sector_filter: bool,
    /// ... on these slots only (None = every slot). A face case's FIXED slots, written out: the symbolic slots of
    /// the case that found the recipe are not filtered there, so they are not listed (explicit, key-independent).
    #[serde(default)]
    pub sector_slots: Option<Vec<usize>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Elimination {
    pub convention: String,
}

impl Default for Elimination {
    fn default() -> Self {
        Self {
            convention: ELIMINATION_CONVENTION.into(),
        }
    }
}

/// A solve recipe (`rustred.recipe.v1`), see the interface file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub schema: String,
    #[serde(default)]
    pub id: String,
    pub arity: usize,
    /// chart mask: '1' = line of the sector (>= 1), '0' = other slot (<= 0)
    pub chart: String,
    #[serde(default)]
    pub domain: Domain,
    pub seeds: SeedSpec,
    #[serde(default)]
    pub symmetry_rows: bool,
    /// v1.2: which relation the solve takes (default m1_first)
    #[serde(default)]
    pub relation_rule: RelationRule,
    #[serde(default)]
    pub elimination: Elimination,
    /// informational: provenance of the recipe (free JSON, part of the digest)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Value>,
}

impl Recipe {
    pub fn parse(text: &str) -> Result<Self, String> {
        let r: Recipe = serde_json::from_str(text).map_err(|e| format!("recipe: {e}"))?;
        if r.schema != RECIPE_SCHEMA {
            return Err(format!(
                "recipe schema {:?} (expected {RECIPE_SCHEMA})",
                r.schema
            ));
        }
        if r.elimination.convention != ELIMINATION_CONVENTION {
            return Err(format!(
                "recipe elimination convention {:?} (this binary implements {ELIMINATION_CONVENTION})",
                r.elimination.convention
            ));
        }
        if r.chart.len() != r.arity || !r.chart.chars().all(|c| c == '0' || c == '1') {
            return Err("recipe chart: a 0/1 mask of the recipe's arity".into());
        }
        if r.seeds.unit != "harder" {
            return Err(format!(
                "recipe seeds.unit {:?} (only \"harder\")",
                r.seeds.unit
            ));
        }
        if r.seeds.centres.iter().any(|c| c.len() != r.arity) {
            return Err("recipe seeds.centres: offsets of the recipe's arity".into());
        }
        let g = &r.seeds.generator;
        match (&g.shells, &g.ball) {
            (Some(s), None) => {
                for name in s {
                    if !SHELLS.contains(&name.as_str()) {
                        return Err(format!("recipe shell {name:?} (known: {SHELLS:?})"));
                    }
                }
            }
            (None, Some(b)) => {
                if b.down.iter().any(|&s| s >= r.arity) {
                    return Err("recipe ball.down: slot outside the arity".into());
                }
            }
            _ => return Err("recipe seeds.generator: exactly one of shells / ball".into()),
        }
        for k in r.seeds.min_offset.keys().chain(r.seeds.max_offset.keys()) {
            if k.parse::<usize>().ok().is_none_or(|s| s >= r.arity) {
                return Err(format!("recipe offset slot {k:?}"));
            }
        }
        if r.seeds
            .sector_slots
            .as_ref()
            .is_some_and(|s| s.iter().any(|&j| j >= r.arity))
        {
            return Err("recipe seeds.sector_slots: slot outside the arity".into());
        }
        if let Some(c) = &r.seeds.layer_cap {
            if c.len() != 3 {
                return Err("recipe seeds.layer_cap: [D, R, B]".into());
            }
        }
        if let Some(f) = &r.domain.fixed {
            if f.len() != r.arity {
                return Err("recipe domain.fixed: arity".into());
            }
        }
        Ok(r)
    }

    /// BLAKE3 of the canonical JSON of the recipe as parsed (sorted keys, defaults filled in).
    pub fn digest(&self) -> String {
        let v = serde_json::to_value(self).expect("recipe json");
        blake3::hash(v.to_string().as_bytes()).to_hex().to_string()
    }

    fn lines(&self) -> Vec<bool> {
        self.chart.chars().map(|c| c == '1').collect()
    }
}

const SHELLS: [&str; 8] = ["S0", "S1", "Sm1", "S1m1", "S2", "S3", "S2m1", "Sm2"];

/// The recipe's explicit, ordered seed list at `key` (absolute keys, each once, first occurrence wins).
pub fn build_seeds(recipe: &Recipe, key: &[i64]) -> Vec<Vec<i64>> {
    let n = recipe.arity;
    let lines = recipe.lines();
    let unit = |j: usize| -> i64 { if lines[j] { 1 } else { -1 } };
    let centres: Vec<Vec<i64>> = if recipe.seeds.centres.is_empty() {
        vec![key.to_vec()]
    } else {
        recipe
            .seeds
            .centres
            .iter()
            .map(|c| key.iter().zip(c).map(|(a, b)| a + b).collect())
            .collect()
    };
    let mut raw: Vec<Vec<i64>> = Vec::new();
    for c in &centres {
        let add = |moves: &[(usize, i64)]| -> Vec<i64> {
            let mut s = c.clone();
            for &(j, sgn) in moves {
                s[j] += sgn * unit(j);
            }
            s
        };
        if let Some(shells) = &recipe.seeds.generator.shells {
            for name in shells {
                match name.as_str() {
                    "S0" => raw.push(add(&[])),
                    "S1" => (0..n).for_each(|j| raw.push(add(&[(j, 1)]))),
                    "Sm1" => (0..n).for_each(|j| raw.push(add(&[(j, -1)]))),
                    "S1m1" => {
                        for a in 0..n {
                            for b in 0..n {
                                if a != b {
                                    raw.push(add(&[(a, 1), (b, -1)]));
                                }
                            }
                        }
                    }
                    "S2" => {
                        for a in 0..n {
                            for b in a..n {
                                raw.push(add(&[(a, 1), (b, 1)]));
                            }
                        }
                    }
                    "S3" => {
                        for a in 0..n {
                            for b in a..n {
                                for e in b..n {
                                    raw.push(add(&[(a, 1), (b, 1), (e, 1)]));
                                }
                            }
                        }
                    }
                    "S2m1" => {
                        for a in 0..n {
                            for b in a..n {
                                for e in 0..n {
                                    if e != a && e != b {
                                        raw.push(add(&[(a, 1), (b, 1), (e, -1)]));
                                    }
                                }
                            }
                        }
                    }
                    "Sm2" => {
                        for a in 0..n {
                            for b in a..n {
                                raw.push(add(&[(a, -1), (b, -1)]));
                            }
                        }
                    }
                    _ => unreachable!("shell names are validated"),
                }
            }
        }
        if let Some(ball) = &recipe.seeds.generator.ball {
            // moves: up on every slot, then down on the listed slots; L1 shells 0..=radius, combinations with
            // repetition of the move list in lexicographic order, an up and a down on one slot never together
            let mut moves: Vec<(usize, i64)> = (0..n).map(|j| (j, 1)).collect();
            let mut down = ball.down.clone();
            down.sort_unstable();
            down.dedup();
            moves.extend(down.iter().map(|&j| (j, -1)));
            fn rec(
                moves: &[(usize, i64)],
                start: usize,
                left: u32,
                cur: &mut Vec<(usize, i64)>,
                out: &mut Vec<Vec<(usize, i64)>>,
            ) {
                if left == 0 {
                    out.push(cur.clone());
                    return;
                }
                for k in start..moves.len() {
                    let (j, s) = moves[k];
                    if cur.iter().any(|&(i, t)| i == j && t != s) {
                        continue;
                    }
                    cur.push(moves[k]);
                    rec(moves, k, left - 1, cur, out);
                    cur.pop();
                }
            }
            for depth in 0..=ball.radius {
                let mut out = Vec::new();
                rec(&moves, 0, depth, &mut Vec::new(), &mut out);
                for m in out {
                    raw.push(add(&m));
                }
            }
        }
    }
    let key_dots: i64 = (0..n).filter(|&j| lines[j]).map(|j| key[j] - 1).sum();
    let key_rank: i64 = (0..n).filter(|&j| !lines[j]).map(|j| -key[j]).sum();
    let mut seen: HashSet<Vec<i64>> = HashSet::new();
    let mut out = Vec::new();
    'seed: for s in raw {
        if recipe.seeds.sector_filter {
            let out_of_sector = |j: usize| if lines[j] { s[j] < 1 } else { s[j] > 0 };
            let bad = match &recipe.seeds.sector_slots {
                None => (0..n).any(out_of_sector),
                Some(slots) => slots.iter().any(|&j| out_of_sector(j)),
            };
            if bad {
                continue;
            }
        }
        for (k, v) in &recipe.seeds.min_offset {
            let j: usize = k.parse().expect("validated");
            if s[j] - key[j] < *v {
                continue 'seed;
            }
        }
        for (k, v) in &recipe.seeds.max_offset {
            let j: usize = k.parse().expect("validated");
            if s[j] - key[j] > *v {
                continue 'seed;
            }
        }
        if let Some(c) = &recipe.seeds.layer_cap {
            let dots: i64 = (0..n).filter(|&j| lines[j]).map(|j| s[j] - 1).sum();
            let rank: i64 = (0..n).filter(|&j| !lines[j]).map(|j| -s[j]).sum();
            let allow = c[0] + if key_dots == 0 { c[2] } else { 0 };
            if dots - key_dots > allow || rank - key_rank > c[1] {
                continue;
            }
        }
        if seen.insert(s.clone()) {
            out.push(s);
        }
    }
    out
}

/// An external stop request (an embedding supervisor's flag, e.g. the child-job contract's signal counter).
pub type StopFn = Arc<dyn Fn() -> bool + Send + Sync>;

/// Options of one batch (command line of `rr_node solve-recipe`).
#[derive(Clone)]
pub struct BatchOptions {
    pub threads: usize,
    pub max_trace_rows: Option<usize>,
    pub proof: bool,
    /// paths and texts echoed into failure records (reproduction line)
    pub family_path: String,
    pub recipe_path: String,
    pub order_text: String,
    pub binary: Value,
    /// per-key wall budget in seconds (time-decided: BUDGET_WALL); None = no budget
    pub key_budget_s: Option<f64>,
    /// check mode: certified relation digests per key; a SOLVED key with another digest is DIGEST_MISMATCH
    pub expect: BTreeMap<Vec<i64>, String>,
    /// echoed into OUTCOME (input files with sizes and digests)
    pub inputs: Value,
    /// job mode: the child-job `job` block (job_id, attempt, kind, spec_blake3, ...) and the OUTCOME path
    pub job: Option<Value>,
    pub outcome_path: Option<PathBuf>,
    /// external stop request (polled with this process's own signal counter and the stop file)
    pub stop: Option<StopFn>,
    /// a stop file (child-job 3.4.5: `RUN/STOP`): its existence is a stop request
    pub stop_file: Option<PathBuf>,
    /// seconds the workers get to finish their current keys after a stop request before the hard stop (default 20)
    pub stop_grace_s: f64,
    /// job mode: paths in failure records are written relative to this directory (the job's run directory)
    pub path_base: Option<PathBuf>,
    /// EVENTS.jsonl location (default `out/EVENTS.jsonl`); job mode: `RUN/EVENTS.jsonl` in the contract's envelope
    pub events_path: Option<PathBuf>,
    /// heartbeat period of the contract events (job mode), seconds
    pub heartbeat_s: f64,
}

impl BatchOptions {
    /// Defaults for an embedding caller (no paths known, one thread, proof on).
    pub fn new(binary: Value) -> Self {
        Self {
            threads: 1,
            max_trace_rows: None,
            proof: true,
            family_path: String::new(),
            recipe_path: String::new(),
            order_text: String::new(),
            binary,
            key_budget_s: None,
            expect: BTreeMap::new(),
            inputs: json!([]),
            job: None,
            outcome_path: None,
            stop: None,
            stop_file: None,
            stop_grace_s: 20.0,
            path_base: None,
            events_path: None,
            heartbeat_s: 10.0,
        }
    }

    fn stop_requested(&self) -> Option<&'static str> {
        if SIGNALS.load(AtomicOrdering::SeqCst) > 0 {
            Some("SIGTERM")
        } else if self.stop.as_ref().is_some_and(|f| f()) {
            Some("supervisor")
        } else if self.stop_file.as_ref().is_some_and(|p| p.exists()) {
            Some("stopfile")
        } else {
            None
        }
    }

    /// `p` relative to `path_base` (job mode), else unchanged.
    fn rel(&self, p: &str) -> String {
        match &self.path_base {
            Some(base) if !p.is_empty() => relative_path(base, Path::new(p)),
            _ => p.to_string(),
        }
    }
}

/// `target` relative to `base` (both made absolute; `..` where needed).
fn relative_path(base: &Path, target: &Path) -> String {
    let abs = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (b, t) = (abs(base), abs(target));
    let bc: Vec<_> = b.components().collect();
    let tc: Vec<_> = t.components().collect();
    let common = bc.iter().zip(&tc).take_while(|(x, y)| x == y).count();
    let mut out = PathBuf::new();
    for _ in common..bc.len() {
        out.push("..");
    }
    for c in &tc[common..] {
        out.push(c.as_os_str());
    }
    if out.as_os_str().is_empty() {
        ".".into()
    } else {
        out.display().to_string()
    }
}

/// UTC time stamp `YYYY-MM-DDTHH:MM:SS.mmmZ` (child-job events).
pub fn utc_now() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil from days (H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        d.subsec_millis()
    )
}

/// EVENTS.jsonl writer: plain lines, or (job mode) the child-job envelope `{"v":1,"seq","t","kind",..}` with `utc`
/// on job_started / heartbeat / job_finished only (child-job.md 3.5).
struct Events {
    file: Mutex<std::fs::File>,
    seq: AtomicUsize,
    t0: Instant,
    contract: bool,
}

impl Events {
    fn emit(&self, kind: &str, fields: Value) {
        let mut v = if self.contract {
            let t = (self.t0.elapsed().as_secs_f64() * 1000.0).round() / 1000.0;
            let mut v = json!({"v": 1, "seq": self.seq.fetch_add(1, AtomicOrdering::SeqCst), "t": t, "kind": kind});
            if matches!(kind, "job_started" | "heartbeat" | "job_finished") {
                v["utc"] = json!(utc_now());
            }
            v
        } else {
            json!({"kind": kind})
        };
        if let (Some(o), Some(f)) = (v.as_object_mut(), fields.as_object()) {
            for (k, x) in f {
                o.insert(k.clone(), x.clone());
            }
        }
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{v}");
        }
    }

    /// One key's result: plain `key_finished`; in job mode an `engine` event (the contract's slot for engine-specific
    /// progress).
    fn key(&self, unique: usize, verdict: &Value, seconds: &Value) {
        if self.contract {
            self.emit("engine", json!({"event": {"RecipeKey": {"unique": unique, "verdict": verdict, "seconds": seconds}}}));
        } else {
            self.emit(
                "key_finished",
                json!({"unique": unique, "verdict": verdict, "seconds": seconds}),
            );
        }
    }
}

/// One input key.
#[derive(Clone, Debug)]
pub struct KeyInput {
    pub key: Vec<i64>,
    pub tag: Option<String>,
}

/// Signals received by this process (only when the CLI installed [`install_stop_handler`]; an embedding caller
/// passes its own flag as `BatchOptions::stop`). The handler only increments it (async-signal-safe, as the
/// child-job contract's handler); the batch's watchdog does the rest.
static SIGNALS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn on_signal(_: i32) {
    SIGNALS.fetch_add(1, AtomicOrdering::SeqCst);
}

/// SIGTERM / SIGINT / SIGHUP (stand-alone CLI only, never by the library): first = graceful stop (current keys finish
/// within `stop_grace_s`, the rest INTERRUPTED, sealed partial, exit 30); a second signal or the grace running out =
/// hard stop (the keys still running are INTERRUPTED too, outputs sealed, exit 30).
pub fn install_stop_handler() {
    for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        // SAFETY: the handler only increments an atomic (async-signal-safe).
        unsafe {
            libc::signal(s, on_signal as *const () as libc::sighandler_t);
        }
    }
}

/// Run a batch: writes `out/results.jsonl` (sealed with an end record), `out/EVENTS.jsonl`, `out/OUTCOME.json`.
/// Returns the exit code of the interface (0 all SOLVED, 10 some not, 30 interrupted).
pub fn run_batch(
    family_text: &str,
    order: &NodeOrder,
    recipe_text: &str,
    keys: &[KeyInput],
    out: &Path,
    opts: &BatchOptions,
) -> Result<i32, String> {
    let recipe = Recipe::parse(recipe_text)?;
    let presented = order.present_family(family_text)?;
    let fam = compile_family(&presented)?;
    let n = fam.denominator_count();
    order.validate(n)?;
    if n != recipe.arity {
        return Err(format!("recipe arity {} != family arity {n}", recipe.arity));
    }
    match n {
        5 => run::<5>(
            family_text,
            &presented,
            &fam,
            order,
            &recipe,
            keys,
            out,
            opts,
        ),
        9 => run::<9>(
            family_text,
            &presented,
            &fam,
            order,
            &recipe,
            keys,
            out,
            opts,
        ),
        12 => run::<12>(
            family_text,
            &presented,
            &fam,
            order,
            &recipe,
            keys,
            out,
            opts,
        ),
        14 => run::<14>(
            family_text,
            &presented,
            &fam,
            order,
            &recipe,
            keys,
            out,
            opts,
        ),
        20 => run::<20>(
            family_text,
            &presented,
            &fam,
            order,
            &recipe,
            keys,
            out,
            opts,
        ),
        other => Err(format!(
            "arity {other}: recipe solve compiled for 5, 9, 12, 14, 20"
        )),
    }
}

struct Ctx<'a, const N: usize> {
    t0: Instant,
    solver: SectorSolver<'a, N>,
    template: CoefficientPolynomial,
    idx: [usize; N],
    sector: [bool; N],
    domain_case: Option<Case<N>>,
    exceptions: Vec<Vec<CoefficientPolynomial>>,
    recipe: &'a Recipe,
    recipe_digest: String,
    node: Value,
    opts: &'a BatchOptions,
    n: usize,
}

/// Prepare the engine once for (family, order, recipe) and run `f` with it (the solver borrows the source system,
/// so the prepared state lives inside this call).
#[allow(clippy::too_many_arguments)]
fn with_ctx<const N: usize, R>(
    family_text: &str,
    presented: &str,
    fam: &crate::family::IntegralFamily,
    order: &NodeOrder,
    recipe: &Recipe,
    opts: &BatchOptions,
    f: impl FnOnce(&Ctx<'_, N>) -> R,
) -> Result<R, String> {
    let t0 = Instant::now();
    let n = fam.denominator_count();
    if fam
        .power_shifts()
        .iter()
        .any(|s| s.to_string().trim() != "0")
    {
        return Err("two-sided (eps) families are not supported by the recipe solve".into());
    }
    let sources = SourceSystem::<N>::from_family_with_capacity(fam, true)
        .map_err(|e| format!("sources: {e:?}"))?;
    let flags = super::order::physical_slots(presented);
    if flags.len() != n {
        return Err("family propagator lines do not match the compiled arity".into());
    }
    let top: [bool; N] = std::array::from_fn(|i| i < n && flags[i]);
    let chart = recipe.lines();
    let base: [bool; N] = std::array::from_fn(|i| i < n && chart[i]);
    if (0..n).any(|i| base[i] && !top[i]) {
        return Err("recipe chart has a line that is not a propagator line of the family".into());
    }
    // the zero-sector census of the node driver (same analyzer, same downset)
    let analyzer =
        zero::Analyzer::try_unrestricted(fam).map_err(|e| format!("zero analyzer: {e:?}"))?;
    let scope: Vec<usize> = (0..n).filter(|&i| top[i]).collect();
    let mut zeros: Vec<[bool; N]> = Vec::new();
    for bits in 0u64..(1u64 << scope.len()) {
        let mut active = [false; N];
        for (j, &i) in scope.iter().enumerate() {
            active[i] = bits >> j & 1 == 1;
        }
        let m = Mask::try_new(active.iter().copied()).map_err(|e| format!("mask: {e:?}"))?;
        if let zero::Decision::ProvedZero(_) = analyzer
            .analyze(&m)
            .map_err(|e| format!("zero analysis: {e:?}"))?
        {
            zeros.push(active);
        }
    }
    if zeros.contains(&base) {
        return Err("the recipe's chart is a proved-zero sector".into());
    }
    let backend = SymbolicExactBackend::SparseTargetOnlyFactorized;
    let case_symmetry = if recipe.symmetry_rows {
        let g = super::symmetry::node_group(presented, &chart)?;
        Some(
            g.elements
                .iter()
                .map(|e| e.action.clone())
                .collect::<Vec<_>>()
                .into(),
        )
    } else {
        None
    };
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros),
        integral_order: order_program(order, n)?,
        symbolic_exact_backend: backend,
        numerical_exact_backend: numerical_backend(backend),
        witness_guards: true,
        case_symmetry,
        ..Default::default()
    };
    let solver = SectorSolver::new(&sources, base, config).map_err(|e| format!("solver: {e:?}"))?;
    let template = sources
        .rows()
        .iter()
        .flat_map(|r| r.iter())
        .next()
        .ok_or("empty source system")?
        .coefficient
        .clone();
    let idx: [usize; N] = *sources.index_variables();
    let sector: [bool; N] = *solver.ordering().sector();
    let dom_spec = CaseSpec {
        fixed: recipe.domain.fixed.clone().unwrap_or_else(|| vec![None; n]),
        equations: recipe.domain.equations.clone(),
    };
    let domain_case = build_case::<N>(&dom_spec, &template, &idx, &sector, n)?;
    let exceptions: Vec<Vec<CoefficientPolynomial>> = recipe
        .domain
        .exceptions
        .iter()
        .map(|b| {
            b.iter()
                .map(|p| {
                    parse_index_poly(p, &template, &idx).ok_or_else(|| format!("exception {p}"))
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .collect::<Result<_, _>>()?;
    let ctx = Ctx {
        t0,
        solver,
        template,
        idx,
        sector,
        domain_case,
        exceptions,
        recipe,
        recipe_digest: recipe.digest(),
        node: json!({
            "family": fam.name(),
            "family_blake3": blake3::hash(family_text.as_bytes()).to_hex().to_string(),
            "order": serde_json::from_str::<Value>(&order.to_json()).unwrap_or(Value::Null),
            "chart": recipe.chart,
        }),
        opts,
        n,
    };
    Ok(f(&ctx))
}

/// Dispatch on the arity: `$body` is run with `N` = the family's arity.
macro_rules! with_arity {
    ($n:expr, $N:ident, $body:block) => {
        match $n {
            5 => {
                const $N: usize = 5;
                $body
            }
            9 => {
                const $N: usize = 9;
                $body
            }
            12 => {
                const $N: usize = 12;
                $body
            }
            14 => {
                const $N: usize = 14;
                $body
            }
            20 => {
                const $N: usize = 20;
                $body
            }
            other => Err(format!(
                "arity {other}: recipe solve compiled for 5, 9, 12, 14, 20"
            )),
        }
    };
}

/// In-memory entry point (for the dispatcher): prepare ONCE for (family, order, recipe) and hand `f` a solver
/// closure that returns one record per key (the same record as `results.jsonl`, without `ordinal` / `tag`; the
/// `--expect` digests of `opts` are checked). No file is written.
pub fn session<R>(
    family_text: &str,
    order: &NodeOrder,
    recipe_text: &str,
    opts: &BatchOptions,
    f: impl FnOnce(&mut dyn FnMut(&[i64]) -> Value) -> R,
) -> Result<R, String> {
    let recipe = Recipe::parse(recipe_text)?;
    let presented = order.present_family(family_text)?;
    let fam = compile_family(&presented)?;
    let n = fam.denominator_count();
    order.validate(n)?;
    if n != recipe.arity {
        return Err(format!("recipe arity {} != family arity {n}", recipe.arity));
    }
    with_arity!(n, M, {
        with_ctx::<M, R>(family_text, &presented, &fam, order, &recipe, opts, |ctx| {
            let mut solve = |key: &[i64]| -> Value {
                let t = Instant::now();
                let mut rec = solve_guarded(ctx, key);
                rec["seconds"] = json!(t.elapsed().as_secs_f64());
                rec
            };
            f(&mut solve)
        })
    })
}

/// In-memory batch: the records of `keys` in input order (with `ordinal` and `tag`), one thread, no files.
pub fn solve_keys(
    family_text: &str,
    order: &NodeOrder,
    recipe_text: &str,
    keys: &[KeyInput],
    opts: &BatchOptions,
) -> Result<Vec<Value>, String> {
    session(family_text, order, recipe_text, opts, |solve| {
        keys.iter()
            .enumerate()
            .map(|(ordinal, k)| {
                let mut r = solve(&k.key);
                r["ordinal"] = json!(ordinal);
                r["tag"] = json!(k.tag);
                r
            })
            .collect()
    })
}

/// One key: the solve with panics caught (a panic becomes an ERROR record) and the --expect digest check.
fn solve_guarded<const N: usize>(ctx: &Ctx<'_, N>, key: &[i64]) -> Value {
    let rec = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| solve_one(ctx, key)))
        .unwrap_or_else(|p| {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            failure(
                ctx,
                key,
                "ERROR",
                "error",
                &format!("panic: {msg}"),
                json!({}),
            )
        });
    match ctx.opts.expect.get(key) {
        Some(want)
            if rec["verdict"] == "SOLVED" && rec["relation_digest"].as_str() != Some(want) =>
        {
            let got = rec["relation_digest"].clone();
            let mut r = failure(
                ctx,
                key,
                "DIGEST_MISMATCH",
                "digest_mismatch",
                "the relation solved at this key differs from the certified one (relation digest)",
                json!({"expected": want, "got": got}),
            );
            r["relation_digest"] = got;
            r
        }
        Some(want) if rec["verdict"] != "SOLVED" => {
            // a certified key that no longer solves: the failure keeps its class, the certified digest is added
            let mut r = rec;
            r["failure"]["detail"]["certified_relation_digest"] = json!(want);
            r
        }
        _ => rec,
    }
}

#[allow(clippy::too_many_arguments)]
fn run<const N: usize>(
    family_text: &str,
    presented: &str,
    fam: &crate::family::IntegralFamily,
    order: &NodeOrder,
    recipe: &Recipe,
    keys: &[KeyInput],
    out: &Path,
    opts: &BatchOptions,
) -> Result<i32, String> {
    with_ctx::<N, _>(family_text, presented, fam, order, recipe, opts, |ctx| {
        run_prepared(ctx, keys, out)
    })?
}

/// Records collected by the workers; per unique key: None (not started / running) or the record.
struct Shared {
    results: Mutex<Vec<Option<Value>>>,
    running: Mutex<BTreeMap<usize, Instant>>,
    expired: Mutex<BTreeSet<usize>>,
}

fn run_prepared<const N: usize>(
    ctx: &Ctx<'_, N>,
    keys: &[KeyInput],
    out: &Path,
) -> Result<i32, String> {
    let opts = ctx.opts;
    let prepare_s = ctx.t0.elapsed().as_secs_f64();
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let events_path = opts
        .events_path
        .clone()
        .unwrap_or_else(|| out.join("EVENTS.jsonl"));
    let events = Events {
        file: Mutex::new(std::fs::File::create(&events_path).map_err(|e| format!("events: {e}"))?),
        seq: AtomicUsize::new(0),
        t0: ctx.t0,
        contract: opts.job.is_some(),
    };
    if let Some(job) = &opts.job {
        events.emit(
            "job_started",
            json!({"job_id": job["job_id"], "attempt": job["attempt"], "spec_blake3": job["spec_blake3"],
                "pid": std::process::id(), "program": "rr_node", "version": env!("CARGO_PKG_VERSION"),
                "git_commit": opts.binary["git_commit"], "resume_from": Value::Null}),
        );
        events.emit("phase", json!({"phase": "case_search"}));
    } else {
        events.emit(
            "job_started",
            json!({"recipe_digest":ctx.recipe_digest,"keys":keys.len(),
            "threads":opts.threads,"prepare_s":prepare_s,"determinism_id":DETERMINISM_ID}),
        );
    }

    // unique keys, solved once each, reported per occurrence
    let mut uniq: Vec<Vec<i64>> = Vec::new();
    let mut uniq_of: Vec<usize> = Vec::with_capacity(keys.len());
    {
        let mut pos: BTreeMap<Vec<i64>, usize> = BTreeMap::new();
        for k in keys {
            let p = *pos.entry(k.key.clone()).or_insert_with(|| {
                uniq.push(k.key.clone());
                uniq.len() - 1
            });
            uniq_of.push(p);
        }
    }
    let shared = Shared {
        results: Mutex::new(vec![None; uniq.len()]),
        running: Mutex::new(BTreeMap::new()),
        expired: Mutex::new(BTreeSet::new()),
    };
    let next = AtomicUsize::new(0);
    let active = AtomicUsize::new(0);
    let threads = opts.threads.max(1);
    let outcome: Result<i32, String> = std::thread::scope(|s| {
        for _ in 0..threads {
            active.fetch_add(1, AtomicOrdering::SeqCst);
            s.spawn(|| {
                loop {
                    if opts.stop_requested().is_some() {
                        break;
                    }
                    let i = next.fetch_add(1, AtomicOrdering::SeqCst);
                    if i >= uniq.len() {
                        break;
                    }
                    shared.running.lock().unwrap().insert(i, Instant::now());
                    let t = Instant::now();
                    let mut rec = solve_guarded(ctx, &uniq[i]);
                    rec["seconds"] = json!(t.elapsed().as_secs_f64());
                    rec["vmhwm_kb"] = json!(vmhwm_kb());
                    shared.running.lock().unwrap().remove(&i);
                    // a key whose wall budget expired keeps its BUDGET_WALL record
                    if !shared.expired.lock().unwrap().contains(&i) {
                        events.key(i, &rec["verdict"], &rec["seconds"]);
                        shared.results.lock().unwrap()[i] = Some(rec);
                    }
                }
                active.fetch_sub(1, AtomicOrdering::SeqCst);
            });
        }
        // watchdog: per-key wall budget (time-decided: BUDGET_WALL), and the end of the batch. When the only work
        // left is keys whose budget expired (their lifts cannot be interrupted in-thread), the outputs are sealed
        // here and the process exits (the stuck threads end with it).
        let mut first_stop: Option<Instant> = None;
        let mut last_beat = Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            // stop request: graceful first (no new key), hard after a second signal or the grace period
            if let Some(source) = opts.stop_requested() {
                if first_stop.is_none() {
                    first_stop = Some(Instant::now());
                    events.emit(
                        "stop_requested",
                        json!({"source": source, "count": SIGNALS.load(AtomicOrdering::SeqCst).max(1)}),
                    );
                }
            }
            let hard = first_stop.is_some_and(|t| {
                SIGNALS.load(AtomicOrdering::SeqCst) >= 2
                    || t.elapsed().as_secs_f64() > opts.stop_grace_s
            });
            if hard && active.load(AtomicOrdering::SeqCst) > 0 {
                let code =
                    write_outputs(ctx, keys, &uniq_of, &shared, out, prepare_s, &events, false);
                std::process::exit(code.unwrap_or(70));
            }
            if events.contract && last_beat.elapsed().as_secs_f64() >= opts.heartbeat_s {
                last_beat = Instant::now();
                let done = shared
                    .results
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|r| r.is_some())
                    .count();
                events.emit(
                    "heartbeat",
                    json!({"phase": "case_search", "case_ordinal": Value::Null,
                        "pending": uniq.len().saturating_sub(done), "rules_committed": done,
                        "counters": counters(done, 0), "rss_kb": rss_kb(), "vmhwm_kb": vmhwm_kb(), "helpers": 0,
                        "stop_requested": first_stop.is_some()}),
                );
            }
            if let Some(budget) = opts.key_budget_s {
                let now: Vec<(usize, f64)> = shared
                    .running
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(i, t)| (*i, t.elapsed().as_secs_f64()))
                    .collect();
                for (i, el) in now {
                    if el > budget && shared.expired.lock().unwrap().insert(i) {
                        let mut rec = failure(
                            ctx,
                            &uniq[i],
                            "BUDGET_WALL",
                            "budget_wall",
                            &format!(
                                "the solve at this key exceeded the wall budget of {budget} s"
                            ),
                            json!({"counter":"wall_seconds","limit":budget,"value":el}),
                        );
                        rec["seconds"] = json!(el);
                        events.key(i, &json!("BUDGET_WALL"), &json!(el));
                        shared.results.lock().unwrap()[i] = Some(rec);
                    }
                }
            }
            if active.load(AtomicOrdering::SeqCst) == 0 {
                break write_outputs(ctx, keys, &uniq_of, &shared, out, prepare_s, &events, true);
            }
            let exhausted = next.load(AtomicOrdering::SeqCst) >= uniq.len() || first_stop.is_some();
            let stuck_only = {
                let running = shared.running.lock().unwrap();
                let expired = shared.expired.lock().unwrap();
                !running.is_empty() && running.keys().all(|i| expired.contains(i))
            };
            if exhausted && stuck_only {
                let code =
                    write_outputs(ctx, keys, &uniq_of, &shared, out, prepare_s, &events, true);
                std::process::exit(code.unwrap_or(70));
            }
        }
    });
    outcome
}

#[allow(clippy::too_many_arguments)]
fn write_outputs<const N: usize>(
    ctx: &Ctx<'_, N>,
    keys: &[KeyInput],
    uniq_of: &[usize],
    shared: &Shared,
    out: &Path,
    prepare_s: f64,
    events: &Events,
    graceful: bool,
) -> Result<i32, String> {
    let opts = ctx.opts;
    let results = shared.results.lock().unwrap().clone();
    let interrupted = results.iter().any(Option::is_none);
    // results.jsonl in INPUT order, then the end record; written to a temporary name and renamed
    let tmp = out.join("results.jsonl.part");
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut f =
        std::io::BufWriter::new(std::fs::File::create(&tmp).map_err(|e| format!("results: {e}"))?);
    for (ordinal, k) in keys.iter().enumerate() {
        let mut rec = match &results[uniq_of[ordinal]] {
            Some(r) => r.clone(),
            None => failure(
                ctx,
                &k.key,
                "INTERRUPTED",
                "interrupted",
                "the batch was stopped (signal, stop file or supervisor) before this key was solved",
                json!({}),
            ),
        };
        rec["ordinal"] = json!(ordinal);
        rec["tag"] = json!(k.tag);
        *counts
            .entry(rec["verdict"].as_str().unwrap_or("?").to_string())
            .or_default() += 1;
        writeln!(f, "{rec}").map_err(|e| e.to_string())?;
    }
    let end = json!({"type": if interrupted { "partial" } else { "end" }, "records": keys.len(),
        "counts": counts, "recipe_digest": ctx.recipe_digest, "determinism_id": DETERMINISM_ID});
    writeln!(f, "{end}").map_err(|e| e.to_string())?;
    f.flush().map_err(|e| e.to_string())?;
    drop(f);
    let path = out.join("results.jsonl");
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let all_solved = counts.keys().all(|k| k == "SOLVED");
    let exit = if interrupted {
        30
    } else if all_solved {
        0
    } else {
        10
    };
    let mut outcome = json!({
        "schema": "rustred.solve-recipe.outcome.v0",
        "status": match exit { 0 => "complete", 10 => "incomplete", _ => "interrupted" },
        "exit_code": exit,
        "recipe": {"id": ctx.recipe.id, "digest": ctx.recipe_digest, "path": opts.recipe_path},
        "determinism_id": DETERMINISM_ID,
        "binary": opts.binary,
        "node": ctx.node,
        "inputs": opts.inputs,
        "outputs": {"results": {"path": "results.jsonl", "bytes": bytes.len(),
            "blake3": blake3::hash(&bytes).to_hex().to_string(), "records": keys.len() + 1,
            "end": if interrupted { "partial" } else { "end" }, "sealed": true}},
        "counts": counts,
        "time_decided": counts.contains_key("BUDGET_WALL"),
        "timing": {"wall_s": ctx.t0.elapsed().as_secs_f64(), "prepare_s": prepare_s},
        "memory": {"vmhwm_kb": vmhwm_kb()},
        "threads": opts.threads.max(1),
    });
    // stop code (child-job 3.7 closed list; `keys_unsolved` is PROPOSED for this kind: some keys typed non-SOLVED)
    let stop_code: Value = match exit {
        0 => Value::Null,
        30 => json!("interrupted"),
        _ => json!("keys_unsolved"),
    };
    if let Some(job) = &opts.job {
        // child-job contract job block (rustred.child-job.outcome.v1 field set; paths relative to the run directory)
        let solved = counts.get("SOLVED").copied().unwrap_or(0);
        let time_decisions: Vec<Value> = results
            .iter()
            .enumerate()
            .filter_map(|(u, r)| {
                r.as_ref()
                    .filter(|r| r["verdict"] == "BUDGET_WALL")
                    .map(|r| (u, r))
            })
            .map(|(u, r)| {
                json!({"what": "case_wall", "case_ordinal": u, "rule": Value::Null,
                "limit_s": opts.key_budget_s, "measured_s": r["seconds"], "taken": "budget_wall"})
            })
            .collect();
        let mut j = job.clone();
        j["status"] = outcome["status"].clone();
        j["exit_code"] = json!(exit);
        j["stop"] = json!({"code": stop_code, "detail": if exit == 0 { Value::Null } else { json!({"counts": counts}) },
            "source": if exit == 30 { json!("signal_or_stopfile") } else { Value::Null }, "graceful": graceful});
        j["program"] = json!({"name": "rr_node", "version": env!("CARGO_PKG_VERSION"),
            "git_commit": opts.binary["git_commit"], "features": ["solve_recipe"], "exe": exe_info()});
        j["effective"] = json!({"subcommand": "solve-recipe", "args": job["args"], "node_options": Value::Null,
            "order": serde_json::from_str::<Value>(&opts.order_text).unwrap_or(Value::Null), "env": {},
            "recipe_digest": ctx.recipe_digest, "determinism_id": DETERMINISM_ID});
        j["seed"] = json!({"sample_seed": SAMPLE_SEED, "point_source": "seed"});
        j["counters"] = counters(solved, keys.len());
        j["budgets"] = json!({"wall": {"case_s": opts.key_budget_s}, "hit": Value::Null});
        j["time_decided"] = json!(!time_decisions.is_empty());
        j["time_decisions"] = json!(time_decisions);
        j["outputs"] = json!({"extra": [{"role": "results", "path": "out/results.jsonl", "bytes": bytes.len(),
            "blake3": outcome["outputs"]["results"]["blake3"], "records": keys.len() + 1,
            "end": outcome["outputs"]["results"]["end"], "sealed": true}]});
        j["timing"] = json!({"started_utc": job["started_utc"], "finished_utc": utc_now(),
            "wall_s": ctx.t0.elapsed().as_secs_f64(), "cpu_s": cpu_s()});
        j["memory"] =
            json!({"vmhwm_kb": vmhwm_kb(), "rss_kb_at_end": rss_kb(), "helpers_peak_rss_kb": 0});
        j["error"] = Value::Null;
        if let Some(o) = j.as_object_mut() {
            o.remove("args");
            o.remove("started_utc");
        }
        outcome["job"] = j;
        // paths in the verb's own block relative to the run directory too
        outcome["recipe"]["path"] = json!(opts.rel(&opts.recipe_path));
        outcome["binary"]["path"] = json!(opts.rel(opts.binary["path"].as_str().unwrap_or("")));
        if let Some(a) = outcome["inputs"].as_array_mut() {
            for i in a {
                let p = i["path"].as_str().unwrap_or("").to_string();
                i["path"] = json!(opts.rel(&p));
            }
        }
    }
    let target = opts
        .outcome_path
        .clone()
        .unwrap_or_else(|| out.join("OUTCOME.json"));
    let otmp = target.with_extension("json.part");
    std::fs::write(
        &otmp,
        serde_json::to_string_pretty(&outcome).unwrap() + "\n",
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&otmp, &target).map_err(|e| e.to_string())?;
    events.emit(
        "job_finished",
        json!({"exit_code": exit, "status": outcome["status"], "stop_code": stop_code}),
    );
    Ok(exit)
}

/// The contract's work counters (child-job 3.3.1); this kind counts keys as cases and solved keys as commits.
fn counters(solved: usize, cases: usize) -> Value {
    json!({"cases": cases, "seeds": 0, "rows": 0, "probe_evals": 0, "reducer_rows": 0, "trace_rows_max": 0,
        "lift_monomials": 0, "rhs_terms": 0, "numeric_points": 0, "rules_committed": solved})
}

fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

fn cpu_s() -> f64 {
    // utime + stime of this process in clock ticks (fields 14, 15 of /proc/self/stat), 100 ticks per second
    std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            let rest = s.rsplit_once(')')?.1;
            let f: Vec<&str> = rest.split_whitespace().collect();
            Some((f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?) / 100.0)
        })
        .unwrap_or(0.0)
}

fn exe_info() -> Value {
    use std::os::unix::fs::MetadataExt;
    let p = std::env::current_exe().ok();
    let m = p.as_ref().and_then(|p| std::fs::metadata(p).ok());
    json!({"path": p.map(|p| p.display().to_string()), "dev": m.as_ref().map(|m| m.dev()),
        "ino": m.as_ref().map(|m| m.ino()), "size": m.as_ref().map(|m| m.size()),
        "mtime": m.as_ref().map(|m| m.mtime())})
}

fn key_text(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// A structured non-SOLVED record (R48 addendum 3): identity, recipe, key, class, determinism id, binary, a
/// one-line reproduction command, and a context slot for the caller (request key, rule chain).
fn failure<const N: usize>(
    ctx: &Ctx<'_, N>,
    key: &[i64],
    verdict: &str,
    class: &str,
    message: &str,
    detail: Value,
) -> Value {
    let o = ctx.opts;
    let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let keyarg = key
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let exe = o.rel(o.binary["path"].as_str().unwrap_or("rr_node"));
    // job mode: every path relative to the job's run directory (run the line there), else absolute paths
    let out_dir = if o.path_base.is_some() {
        format!(
            "repro-{}-{}",
            &ctx.recipe_digest[..12],
            keyarg.replace([',', '-'], "_")
        )
    } else {
        format!(
            "/tmp/recipe-repro-{}-{}",
            &ctx.recipe_digest[..12],
            keyarg.replace([',', '-'], "_")
        )
    };
    let reproduce = format!(
        "{} solve-recipe --family {} --order {} --recipe {} --key {} --out {}",
        q(&exe),
        q(&o.rel(&o.family_path)),
        q(&o.order_text),
        q(&o.rel(&o.recipe_path)),
        keyarg,
        q(&out_dir)
    );
    let mut binary = o.binary.clone();
    if o.path_base.is_some() {
        binary["path"] = json!(exe);
    }
    json!({
        "type": "relation", "key": key, "verdict": verdict, "target": key_text(key),
        "rhs": Value::Null, "relation_digest": Value::Null,
        "determinism_id": DETERMINISM_ID, "recipe_digest": ctx.recipe_digest,
        "failure": {
            "schema": FAILURE_SCHEMA, "class": class, "message": message, "detail": detail,
            "node": ctx.node,
            "recipe": {"id": ctx.recipe.id, "digest": ctx.recipe_digest, "path": o.rel(&o.recipe_path)},
            "key": key, "determinism_id": DETERMINISM_ID, "binary": binary,
            "cwd": if o.path_base.is_some() { json!("job run directory") } else { Value::Null },
            "reproduce": reproduce,
            "context": {"request_key": Value::Null, "rule_chain": []},
        }
    })
}

/// Domain test; Err(record) when the key is outside.
fn domain_check<const N: usize>(ctx: &Ctx<'_, N>, key: &[i64]) -> Result<Case<N>, Value> {
    let n = ctx.n;
    let out = |class_detail: Value, msg: String| {
        Err(failure(
            ctx,
            key,
            "NOT_IN_DOMAIN",
            "not_in_domain",
            &msg,
            class_detail,
        ))
    };
    if key.len() != n {
        return out(
            json!({"reason":"arity","want":n,"got":key.len()}),
            format!("key of arity {}", key.len()),
        );
    }
    let lines = ctx.recipe.lines();
    for j in 0..n {
        let ok = if lines[j] { key[j] >= 1 } else { key[j] <= 0 };
        if !ok {
            return out(
                json!({"reason":"sector","slot":j,"got":key[j]}),
                format!(
                    "slot {j} = {} is outside the chart {}'s top sector",
                    key[j], ctx.recipe.chart
                ),
            );
        }
    }
    if let Some(f) = &ctx.recipe.domain.fixed {
        for (j, v) in f.iter().enumerate() {
            if let Some(v) = v {
                if key[j] != *v {
                    return out(
                        json!({"reason":"fixed_slot","slot":j,"want":v,"got":key[j]}),
                        format!(
                            "slot {j} = {} but the recipe's domain fixes it at {v}",
                            key[j]
                        ),
                    );
                }
            }
        }
    }
    if let Some(b) = &ctx.recipe.domain.bounds {
        for (j, v) in b.min.iter().enumerate() {
            if let Some(v) = v {
                if key.get(j).is_some_and(|x| x < v) {
                    return out(
                        json!({"reason":"bound","slot":j,"min":v,"got":key[j]}),
                        format!("slot {j} below {v}"),
                    );
                }
            }
        }
        for (j, v) in b.max.iter().enumerate() {
            if let Some(v) = v {
                if key.get(j).is_some_and(|x| x > v) {
                    return out(
                        json!({"reason":"bound","slot":j,"max":v,"got":key[j]}),
                        format!("slot {j} above {v}"),
                    );
                }
            }
        }
    }
    if let Some(set) = &ctx.recipe.domain.keys {
        if !set.iter().any(|k| k.as_slice() == key) {
            return out(
                json!({"reason":"key_set","keys":set.len()}),
                "the key is not in the recipe's explicit key set".into(),
            );
        }
    }
    let spec = CaseSpec {
        fixed: key.iter().map(|&x| Some(x)).collect(),
        equations: vec![],
    };
    let point = match build_case::<N>(&spec, &ctx.template, &ctx.idx, &ctx.sector, n) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return out(
                json!({"reason":"sector"}),
                "the key is not an admissible point of the sector".into(),
            );
        }
        Err(e) => return out(json!({"reason":"key","message":e}), format!("key: {e}")),
    };
    let limits = CaseIntersectionLimits::default();
    match &ctx.domain_case {
        None => {
            return out(
                json!({"reason":"empty_domain"}),
                "the recipe's domain case is empty".into(),
            );
        }
        Some(d) => {
            if !d.contains(&point).unwrap_or(false) {
                return out(
                    json!({"reason":"equation","equations":ctx.recipe.domain.equations}),
                    "the key violates the domain's equations".into(),
                );
            }
        }
    }
    for (b, branch) in ctx.exceptions.iter().enumerate() {
        if !covers(
            &point,
            std::slice::from_ref(branch),
            &point,
            &ctx.idx,
            &ctx.sector,
            limits,
        ) {
            return out(
                json!({"reason":"exception_branch","branch":b,"polynomials":ctx.recipe.domain.exceptions[b]}),
                format!("the key lies on exception branch {b}"),
            );
        }
    }
    Ok(point)
}

fn solve_one<const N: usize>(ctx: &Ctx<'_, N>, key: &[i64]) -> Value {
    let point = match domain_check(ctx, key) {
        Ok(p) => p,
        Err(rec) => return rec,
    };
    let seeds = build_seeds(ctx.recipe, key);
    let mut s16 = Vec::with_capacity(seeds.len());
    for s in &seeds {
        let mut v = Vec::with_capacity(s.len());
        for &x in s {
            match i16::try_from(x) {
                Ok(y) if (crate::solver::Power::MIN..=crate::solver::Power::MAX).contains(&y) => {
                    v.push(y)
                }
                _ => {
                    return failure(
                        ctx,
                        key,
                        "ERROR",
                        "error",
                        &format!("seed power {x} outside the engine range"),
                        json!({"seed":s}),
                    );
                }
            }
        }
        s16.push(v);
    }
    let n_seeds = seeds.len();
    let seeds_arc = Arc::new(s16);
    let main = match raw_solve(ctx, &point, &seeds_arc) {
        Ok(x) => x,
        Err(RawErr::Exhausted(rows)) => {
            return failure(
                ctx,
                key,
                "NO_RELATION_IN_SEEDS",
                "no_relation_in_seeds",
                &format!(
                    "the {n_seeds} seeds ({rows} rows) of recipe {} never reach {} as a pivot",
                    ctx.recipe.id,
                    key_text(key)
                ),
                json!({"seeds": n_seeds, "rows": rows}),
            );
        }
        Err(RawErr::Irregular(m)) => {
            return failure(
                ctx,
                key,
                "IRREGULAR",
                "irregular",
                &format!("exact lift: {m}"),
                json!({"pole": m}),
            );
        }
        Err(RawErr::Error(m)) => {
            return failure(ctx, key, "ERROR", "error", &m, json!({"message": m}));
        }
    };
    let target_int = main.target;
    let target: Vec<i64> = (0..ctx.n)
        .map(|i| i64::from(target_int[i].value()))
        .collect();
    if target != key {
        return failure(
            ctx,
            key,
            "ERROR",
            "error",
            &format!("the search returned target {}", key_text(&target)),
            json!({}),
        );
    }
    if let Some(limit) = ctx.opts.max_trace_rows {
        if main.trace_rows > limit {
            return failure(
                ctx,
                key,
                "BUDGET_WORK",
                "budget_work",
                "trace rows above --max-trace-rows",
                json!({"counter":"trace_rows","limit":limit,"value":main.trace_rows}),
            );
        }
    }
    let order = ctx.solver.ordering();
    let lines = ctx.recipe.lines();
    let to_vec = |k: &crate::solver::Integral<N>| -> Vec<i64> {
        (0..ctx.n).map(|i| i64::from(k[i].value())).collect()
    };
    let in_top = |k: &[i64]| (0..ctx.n).all(|j| !lines[j] || k[j] >= 1);
    let layer = |k: &[i64]| -> i64 {
        (0..ctx.n)
            .map(|j| if lines[j] { k[j] - 1 } else { -k[j] })
            .sum()
    };
    // relation rule: m1_first eliminates the same-layer top-sector keys of the first admissible relation
    let rule = ctx.recipe.relation_rule;
    let mut rel = main.clone();
    let mut taken = "first_admissible";
    let mut m1_steps = 0usize;
    let mut fallback_reason: Option<String> = None;
    if rule == RelationRule::M1First {
        taken = "m1";
        let lt = layer(key);
        loop {
            let bad = rel
                .p
                .keys()
                .filter(|k| **k != target_int)
                .filter(|k| {
                    let v = to_vec(k);
                    in_top(&v) && layer(&v) >= lt
                })
                .min_by(|a, b| order.compare(a, b))
                .copied();
            let Some(s) = bad else { break };
            if m1_steps >= M1_MAX_STEPS {
                fallback_reason = Some(format!("more than {M1_MAX_STEPS} same-layer eliminations"));
                break;
            }
            let sv = to_vec(&s);
            let spec = CaseSpec {
                fixed: sv.iter().map(|&x| Some(x)).collect(),
                equations: vec![],
            };
            let sub = match build_case::<N>(&spec, &ctx.template, &ctx.idx, &ctx.sector, ctx.n) {
                Ok(Some(c)) => raw_solve(ctx, &c, &seeds_arc).map_err(|e| e.text()),
                Ok(None) => Err("not an admissible point".to_string()),
                Err(e) => Err(e),
            };
            match sub {
                Ok(rs) if rs.target == s => {
                    let a = rel.p.get(&s).cloned().expect("bad key is a term");
                    rel.subtract(&a, &rs);
                    m1_steps += 1;
                }
                Ok(_) => {
                    fallback_reason = Some(format!(
                        "the solve at {} returned another target",
                        key_text(&sv)
                    ));
                    break;
                }
                Err(m) => {
                    fallback_reason = Some(format!(
                        "no relation for the same-layer key {} in these seeds ({m})",
                        key_text(&sv)
                    ));
                    break;
                }
            }
        }
        if fallback_reason.is_some() {
            rel = main.clone();
            taken = "c_fallback";
        }
    }
    // the relation's terms (target excluded), comparator order (highest first), rhs coefficient = -P_k
    let mut terms: Vec<(&crate::solver::Integral<N>, &Coefficient)> =
        rel.p.iter().filter(|(k, _)| **k != target_int).collect();
    terms.sort_by(|a, b| order.compare(a.0, b.0));
    let mut rhs = Vec::new();
    let mut rhs_canon = Vec::new();
    let mut top = Vec::new();
    let mut n_sub = 0usize;
    let mut all_lower = true;
    let mut first_not_lower: Option<Vec<i64>> = None;
    let mut poles: BTreeSet<String> = rel.hidden.iter().cloned().collect();
    for (ki, pc) in &terms {
        let k = to_vec(ki);
        let coef = -(*pc).clone();
        let lower = order.compare(&target_int, ki) == std::cmp::Ordering::Less;
        if !lower {
            all_lower = false;
            first_not_lower.get_or_insert(k.clone());
        }
        let it = in_top(&k);
        if it {
            top.push(json!({"key": k, "lower": lower, "layer_lower": layer(&k) < layer(key)}));
        } else {
            n_sub += 1;
        }
        if !coef.denominator.is_constant() {
            poles.insert(coef.denominator.to_string());
        }
        let c = coef.to_string();
        // every RHS term: key text, exact coefficient, top sector or a proper subsector (proved-zero sectors never appear)
        rhs.push(json!([ki.to_string(), c, if it { "top" } else { "sub" }]));
        rhs_canon.push(json!([k, c]));
    }
    let canon = json!({"relation_rule": rule.name(), "rhs": rhs_canon, "target": key});
    let relation_digest = blake3::hash(format!("{canon}\n").as_bytes())
        .to_hex()
        .to_string();
    if !all_lower {
        let mut r = failure(
            ctx,
            key,
            "NOT_LOWER",
            "not_lower",
            "a right-hand-side key is not strictly lower",
            json!({"offending_rhs_key": first_not_lower}),
        );
        r["relation_digest"] = json!(relation_digest);
        return r;
    }
    // exact proof: sum_r W_r * row_r == P on every column (the combined weights of all solves of the rule)
    let mut seeds_used: Vec<Vec<i64>> = Vec::new();
    let proof = if !ctx.opts.proof {
        Value::Null
    } else {
        let mut acc: BTreeMap<crate::solver::Integral<N>, Coefficient> = BTreeMap::new();
        for (rk, w) in &rel.w {
            let Some(row) = rel.rows.get(rk) else {
                return failure(
                    ctx,
                    key,
                    "PROOF_FAILED",
                    "proof_failed",
                    "a weight names a missing row",
                    json!({}),
                );
            };
            for term in row {
                let prod = w * &term.coefficient;
                match acc.get_mut(&term.integral) {
                    Some(e) => *e = &*e + &prod,
                    None => {
                        acc.insert(term.integral, prod);
                    }
                }
            }
        }
        for (k, c) in &rel.p {
            match acc.get_mut(k) {
                Some(e) => *e = &*e - c,
                None => {
                    acc.insert(*k, -c.clone());
                }
            }
        }
        if let Some((col, _)) = acc.iter().find(|(_, v)| !v.is_zero()) {
            return failure(
                ctx,
                key,
                "PROOF_FAILED",
                "proof_failed",
                "sum_j w_j row_j differs from the relation",
                json!({"column": col.to_string()}),
            );
        }
        let mut used: BTreeSet<Vec<i64>> = BTreeSet::new();
        for (rk, w) in &rel.w {
            if !w.is_zero() {
                used.insert(
                    (0..ctx.n)
                        .map(|i| i64::from(rk.1[i].value()) - key[i])
                        .collect(),
                );
            }
        }
        seeds_used = used.into_iter().collect();
        json!({"kind": "exact-weights", "weights": rel.w.len(), "ok": true,
            "modulo": "proved-zero sectors (their integrals vanish)"})
    };
    json!({
        "type": "relation", "key": key, "verdict": if ctx.opts.proof { "SOLVED" } else { "SOLVED_UNPROVED" },
        "determinism_id": DETERMINISM_ID, "recipe_digest": ctx.recipe_digest,
        "relation_rule": rule.name(), "relation_rule_taken": taken, "m1_steps": m1_steps,
        "m1_fallback_reason": fallback_reason,
        "target": target_int.to_string(), "rhs": rhs, "relation_digest": relation_digest,
        "rhs_count": terms.len(), "rhs_sub_count": n_sub, "top_rhs": top, "all_lower": all_lower,
        "seeds_total": n_seeds, "rows": main.stats_rows, "trace_rows": main.trace_rows,
        "seeds_used": seeds_used, "proof": proof, "d_poles": poles.into_iter().collect::<Vec<_>>(),
        "failure": Value::Null,
    })
}

/// Identity of one trace row across the solves of a key: basis row (or the symmetry-row marker), seed, and EVERY
/// integral of the row (two symmetry rows at one seed differ only in their image keys).
type RowKey<const N: usize> = (
    usize,
    crate::solver::Integral<N>,
    Vec<crate::solver::Integral<N>>,
);

/// A relation in row form `P = I(target) - sum rhs`, normalised to target coefficient 1, with its exact weights on the
/// trace rows (`sum W_r row_r == P`).
#[derive(Clone)]
struct LinRel<const N: usize> {
    target: crate::solver::Integral<N>,
    p: BTreeMap<crate::solver::Integral<N>, Coefficient>,
    w: BTreeMap<RowKey<N>, Coefficient>,
    rows: BTreeMap<RowKey<N>, crate::solver::ExactRow<N>>,
    hidden: Vec<String>,
    stats_rows: usize,
    trace_rows: usize,
}

impl<const N: usize> LinRel<N> {
    /// self -= a * other (exact)
    fn subtract(&mut self, a: &Coefficient, other: &LinRel<N>) {
        for (k, c) in &other.p {
            let d = a * c;
            match self.p.get_mut(k) {
                Some(e) => *e = &*e - &d,
                None => {
                    self.p.insert(*k, -d);
                }
            }
        }
        self.p.retain(|_, v| !v.is_zero());
        for (k, c) in &other.w {
            let d = a * c;
            match self.w.get_mut(k) {
                Some(e) => *e = &*e - &d,
                None => {
                    self.w.insert(k.clone(), -d);
                }
            }
        }
        self.w.retain(|_, v| !v.is_zero());
        for (k, r) in &other.rows {
            self.rows.entry(k.clone()).or_insert_with(|| r.clone());
        }
        self.hidden.extend(other.hidden.iter().cloned());
        self.trace_rows += other.trace_rows;
    }
}

enum RawErr {
    Exhausted(usize),
    Irregular(String),
    Error(String),
}

impl RawErr {
    fn text(&self) -> String {
        match self {
            Self::Exhausted(rows) => format!("search exhausted, {rows} rows"),
            Self::Irregular(m) | Self::Error(m) => m.clone(),
        }
    }
}

/// One engine solve of a fully fixed case with the explicit seed list: the first admissible relation in row form with
/// its exact weights.
fn raw_solve<const N: usize>(
    ctx: &Ctx<'_, N>,
    point: &Case<N>,
    seeds: &Arc<Vec<Vec<i16>>>,
) -> Result<LinRel<N>, RawErr> {
    let options = SearchOptions {
        max_depth: None,
        prime: PRIME,
        sample_seed: SAMPLE_SEED,
    };
    let (res, cap) = with_proof_capture::<N, _>(|| {
        with_explicit_seeds(seeds.clone(), || {
            ctx.solver.solve_case(point.clone(), options)
        })
    });
    let c = match res {
        Ok(c) => c,
        Err(SolverError::SearchExhausted { rows, .. }) => return Err(RawErr::Exhausted(rows)),
        Err(SolverError::ExactReplay(m)) => return Err(RawErr::Irregular(m)),
        Err(e) => return Err(RawErr::Error(format!("{e:?}"))),
    };
    let cap = cap.ok_or_else(|| RawErr::Error("the lift recorded no proof capture".into()))?;
    let weights = cap
        .weights
        .as_ref()
        .ok_or_else(|| RawErr::Error("the lift recorded no source weights".into()))?;
    if cap.rows.len() != c.sources.len() {
        return Err(RawErr::Error(
            "trace rows and sources differ in length".into(),
        ));
    }
    let lead = cap
        .exact
        .iter()
        .find(|t| t.integral == c.target)
        .map(|t| t.coefficient.clone())
        .ok_or_else(|| RawErr::Error("the lifted row lacks its target".into()))?;
    let mut p = BTreeMap::new();
    for t in &cap.exact {
        p.insert(t.integral, &t.coefficient / &lead);
    }
    let mut w = BTreeMap::new();
    let mut rows = BTreeMap::new();
    for (j, wt) in weights {
        let row = &cap.rows[*j];
        let src = &c.sources[*j];
        let rk: RowKey<N> = (
            src.basis_row,
            src.seed.integral,
            row.iter().map(|t| t.integral).collect(),
        );
        let v = wt / &lead;
        rows.entry(rk.clone()).or_insert_with(|| row.clone());
        match w.get_mut(&rk) {
            Some(e) => *e = &*e + &v,
            None => {
                w.insert(rk, v);
            }
        }
    }
    Ok(LinRel {
        target: c.target,
        p,
        w,
        rows,
        hidden: c.hidden_poles.iter().map(|p| p.to_string()).collect(),
        stats_rows: c.stats.rows,
        trace_rows: c.sources.len(),
    })
}

/// Parse a KEYS.jsonl text: `[ints]` or `{"key":[ints], "tag":".."}` per non-empty line.
pub fn parse_keys(text: &str) -> Result<Vec<KeyInput>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| format!("keys line {}: {e}", i + 1))?;
        let (k, tag) = match &v {
            Value::Array(_) => (v.clone(), None),
            Value::Object(o) => (
                o.get("key")
                    .cloned()
                    .ok_or(format!("keys line {}: no key", i + 1))?,
                o.get("tag").and_then(Value::as_str).map(str::to_string),
            ),
            _ => return Err(format!("keys line {}: not a key", i + 1)),
        };
        let key: Vec<i64> =
            serde_json::from_value(k).map_err(|e| format!("keys line {}: {e}", i + 1))?;
        out.push(KeyInput { key, tag });
    }
    Ok(out)
}

/// The binary's identity for records (path, BLAKE3 of the executable, build commit if exported at build time).
pub fn binary_identity() -> Value {
    let path: Option<PathBuf> = std::env::current_exe().ok();
    let digest = path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| blake3::hash(&b).to_hex().to_string());
    json!({
        "path": path.map(|p| p.display().to_string()),
        "blake3": digest,
        "git_commit": option_env!("RUSTRED_BUILD_GIT_COMMIT").unwrap_or("unknown"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KITE: &str = "I(\n  name(TOY2kite),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(-k1-k2)^2,1),\n  prop(D3,(k2)^2,1),\n  prop(D4,(k1+k2-q)^2,1),\n  prop(D5,(-k2+q)^2,1),\n  gram(q,q,1)\n)\n";

    fn recipe(shells: &[&str], extra: &str) -> String {
        format!(
            r#"{{"schema":"rustred.recipe.v1","id":"test/kite","arity":5,"chart":"11111",
               "seeds":{{"centres":[],"generator":{{"shells":{}}}{extra}}}}}"#,
            serde_json::to_string(shells).unwrap()
        )
    }

    fn opts(threads: usize) -> BatchOptions {
        let mut o =
            BatchOptions::new(json!({"path": "test", "blake3": null, "git_commit": "test"}));
        o.threads = threads;
        o.family_path = "kite.sym".into();
        o.recipe_path = "kite.recipe.json".into();
        o.order_text = NodeOrder::identity().to_json();
        o
    }

    fn records(dir: &Path) -> Vec<Value> {
        std::fs::read_to_string(dir.join("results.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// 50 keys of the kite (dots and one numerator-free family: lines only), solved twice: 1 thread in input order,
    /// 4 threads in reversed order; every key's relation digest must agree, every SOLVED record carries an exact
    /// proof and only lower keys.
    #[test]
    fn determinism_50_keys_threads_and_order() {
        let mut keys = Vec::new();
        'outer: for a in 1..=4i64 {
            for b in 1..=3 {
                for c in 1..=3 {
                    for e in 1..=2 {
                        if a + b + c + e - 4 > 0 && a + b + c + e - 4 <= 5 {
                            keys.push(KeyInput {
                                key: vec![a, b, c, 1 + (a + e) % 2, e],
                                tag: None,
                            });
                            if keys.len() == 50 {
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(keys.len(), 50);
        let r = recipe(&["S0", "S1", "Sm1", "S1m1", "S2"], "");
        let base = std::env::temp_dir().join(format!("rr_recipe_det_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let o = NodeOrder::identity();
        run_batch(KITE, &o, &r, &keys, &base.join("a"), &opts(1)).unwrap();
        let mut rev = keys.clone();
        rev.reverse();
        run_batch(KITE, &o, &r, &rev, &base.join("b"), &opts(4)).unwrap();
        let a = records(&base.join("a"));
        let b = records(&base.join("b"));
        let digest = |v: &[Value]| -> BTreeMap<String, (String, String)> {
            v.iter()
                .filter(|r| r["type"] == "relation")
                .map(|r| {
                    (
                        r["key"].to_string(),
                        (r["verdict"].to_string(), r["relation_digest"].to_string()),
                    )
                })
                .collect()
        };
        let (da, db) = (digest(&a), digest(&b));
        assert_eq!(da.len(), 50);
        assert_eq!(da, db);
        let solved = a.iter().filter(|r| r["verdict"] == "SOLVED").count();
        assert!(solved >= 40, "only {solved} of 50 solved");
        for r in a.iter().filter(|r| r["verdict"] == "SOLVED") {
            assert_eq!(r["proof"]["ok"], true);
            assert_eq!(r["all_lower"], true);
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Typed verdicts: a key outside the chart's sector or the domain is NOT_IN_DOMAIN; a recipe with only the
    /// centre seed cannot reach a dotted key (NO_RELATION_IN_SEEDS), never a wrong relation; every failure carries
    /// the reproduction line.
    #[test]
    fn typed_failures() {
        let keys = vec![
            KeyInput {
                key: vec![0, 1, 1, 1, 1],
                tag: None,
            },
            KeyInput {
                key: vec![3, 2, 1, 1, 1],
                tag: None,
            },
            KeyInput {
                key: vec![2, 1, 1, 1, 1],
                tag: None,
            },
        ];
        let base = std::env::temp_dir().join(format!("rr_recipe_fail_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let o = NodeOrder::identity();
        let text = json!({"schema":"rustred.recipe.v1","id":"test/kite-centre-only","arity":5,"chart":"11111",
            "domain":{"fixed":[null,null,1,1,1],"equations":[],"exceptions":[["n1-2"]]},
            "seeds":{"centres":[],"generator":{"shells":["S0"]}}})
        .to_string();
        run_batch(KITE, &o, &text, &keys, &base, &opts(1)).unwrap();
        let v = records(&base);
        assert_eq!(v[0]["verdict"], "NOT_IN_DOMAIN");
        assert_eq!(v[1]["verdict"], "NOT_IN_DOMAIN");
        assert_eq!(v[1]["failure"]["detail"]["reason"], "exception_branch");
        assert_eq!(v[2]["verdict"], "NO_RELATION_IN_SEEDS");
        for r in &v[..3] {
            assert!(
                r["failure"]["reproduce"]
                    .as_str()
                    .unwrap()
                    .contains("solve-recipe")
            );
            assert_eq!(r["failure"]["schema"], FAILURE_SCHEMA);
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The in-memory entry point gives the batch's relations; the check mode turns a changed digest into
    /// DIGEST_MISMATCH and keeps equal ones SOLVED; every RHS term is marked top / sub.
    #[test]
    fn in_memory_session_and_digest_check() {
        let keys = vec![
            KeyInput {
                key: vec![2, 1, 1, 1, 1],
                tag: Some("a".into()),
            },
            KeyInput {
                key: vec![1, 2, 1, 1, 1],
                tag: None,
            },
        ];
        let r = recipe(&["S0", "S1", "Sm1", "S1m1", "S2"], "");
        let o = NodeOrder::identity();
        let mem = solve_keys(KITE, &o, &r, &keys, &opts(1)).unwrap();
        assert_eq!(mem.len(), 2);
        assert!(mem.iter().all(|x| x["verdict"] == "SOLVED"));
        for x in &mem {
            for term in x["rhs"].as_array().unwrap() {
                assert!(term[2] == "top" || term[2] == "sub");
            }
        }
        let base = std::env::temp_dir().join(format!("rr_recipe_mem_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        run_batch(KITE, &o, &r, &keys, &base, &opts(2)).unwrap();
        let file = records(&base);
        assert_eq!(file[0]["relation_digest"], mem[0]["relation_digest"]);
        assert_eq!(file[1]["relation_digest"], mem[1]["relation_digest"]);
        let mut chk = opts(1);
        chk.expect.insert(
            keys[0].key.clone(),
            mem[0]["relation_digest"].as_str().unwrap().into(),
        );
        chk.expect.insert(keys[1].key.clone(), "0".repeat(64));
        let v = solve_keys(KITE, &o, &r, &keys, &chk).unwrap();
        assert_eq!(v[0]["verdict"], "SOLVED");
        assert_eq!(v[1]["verdict"], "DIGEST_MISMATCH");
        assert_eq!(v[1]["failure"]["class"], "digest_mismatch");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn shells_count_and_order() {
        let r = Recipe::parse(&recipe(
            &["S0", "S1", "Sm1", "S1m1", "S2"],
            r#","sector_filter":false"#,
        ))
        .unwrap();
        let s = build_seeds(&r, &[2, 2, 2, 2, 2]);
        // 1 + 5 + 5 + 20 + 15, all distinct
        assert_eq!(s.len(), 46);
        assert_eq!(s[0], vec![2, 2, 2, 2, 2]);
        assert_eq!(s[1], vec![3, 2, 2, 2, 2]);
        // explicit floor on slot 0: no seed below key - 1 there; sector filter drops the lowered lines at 1
        let r = Recipe::parse(&recipe(&["S0", "Sm1", "S1m1"], r#","min_offset":{"0":0}"#)).unwrap();
        let s = build_seeds(&r, &[1, 2, 2, 2, 2]);
        assert!(s.iter().all(|x| x[0] >= 1));
        let r = Recipe::parse(&recipe(&["S0"], "")).unwrap();
        assert_eq!(
            r.digest(),
            Recipe::parse(&recipe(&["S0"], "")).unwrap().digest()
        );
    }
}
