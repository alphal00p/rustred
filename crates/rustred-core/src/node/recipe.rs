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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
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
pub const DETERMINISM_ID: &str = "rustred.recipe-solve.v1/elim=rustred.recipe-elim.v1/prime=2305843009213693951/seed=0/backend=sparse-target-only-factorized/proof=exact-weights";
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

/// Options of one batch (command line of `rr_node solve-recipe`).
#[derive(Clone, Debug)]
pub struct BatchOptions {
    pub threads: usize,
    pub max_trace_rows: Option<usize>,
    pub proof: bool,
    /// paths and texts echoed into failure records (reproduction line)
    pub family_path: String,
    pub recipe_path: String,
    pub order_text: String,
    pub binary: Value,
}

/// One input key.
#[derive(Clone, Debug)]
pub struct KeyInput {
    pub key: Vec<i64>,
    pub tag: Option<String>,
}

/// Set by SIGTERM / SIGINT (see [`install_stop_handler`]); workers stop taking keys.
pub static STOP: AtomicBool = AtomicBool::new(false);
static SIGNALS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn on_signal(_: i32) {
    if SIGNALS.fetch_add(1, AtomicOrdering::SeqCst) >= 1 {
        // second signal: leave at once (nothing sealed; OUTCOME absent)
        unsafe { libc::_exit(30) };
    }
    STOP.store(true, AtomicOrdering::SeqCst);
}

/// SIGTERM / SIGINT / SIGHUP: first = graceful stop (current keys finish, the rest INTERRUPTED, sealed partial),
/// second = immediate exit 30.
pub fn install_stop_handler() {
    for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        // SAFETY: the handler only touches atomics and calls _exit (async-signal-safe).
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
    let prepare_s = t0.elapsed().as_secs_f64();

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let events = Mutex::new(
        std::fs::File::create(out.join("EVENTS.jsonl")).map_err(|e| format!("events: {e}"))?,
    );
    let event = |v: Value| {
        if let Ok(mut f) = events.lock() {
            let _ = writeln!(f, "{v}");
        }
    };
    event(
        json!({"kind":"job_started","recipe_digest":ctx.recipe_digest,"keys":keys.len(),
        "threads":opts.threads,"prepare_s":prepare_s,"determinism_id":DETERMINISM_ID}),
    );

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
    let results: Mutex<Vec<Option<Value>>> = Mutex::new(vec![None; uniq.len()]);
    let next = AtomicUsize::new(0);
    let threads = opts.threads.max(1);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    if STOP.load(AtomicOrdering::SeqCst) {
                        break;
                    }
                    let i = next.fetch_add(1, AtomicOrdering::SeqCst);
                    if i >= uniq.len() {
                        break;
                    }
                    let t = Instant::now();
                    let rec = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        solve_one(&ctx, &uniq[i])
                    }))
                    .unwrap_or_else(|p| {
                        let msg = p
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| "panic".into());
                        failure(
                            &ctx,
                            &uniq[i],
                            "ERROR",
                            "error",
                            &format!("panic: {msg}"),
                            json!({}),
                        )
                    });
                    let mut rec = rec;
                    rec["seconds"] = json!(t.elapsed().as_secs_f64());
                    rec["vmhwm_kb"] = json!(vmhwm_kb());
                    event(
                        json!({"kind":"key_finished","unique":i,"verdict":rec["verdict"],
                        "seconds":rec["seconds"]}),
                    );
                    results.lock().unwrap()[i] = Some(rec);
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
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
                &ctx,
                &k.key,
                "INTERRUPTED",
                "interrupted",
                "the batch was stopped by a signal before this key was solved",
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
    let outcome = json!({
        "schema": "rustred.solve-recipe.outcome.v0",
        "status": match exit { 0 => "complete", 10 => "incomplete", _ => "interrupted" },
        "exit_code": exit,
        "recipe": {"id": recipe.id, "digest": ctx.recipe_digest, "path": opts.recipe_path},
        "determinism_id": DETERMINISM_ID,
        "binary": opts.binary,
        "node": ctx.node,
        "outputs": {"results": {"path": "results.jsonl", "bytes": bytes.len(),
            "blake3": blake3::hash(&bytes).to_hex().to_string(), "records": keys.len() + 1,
            "end": if interrupted { "partial" } else { "end" }, "sealed": true}},
        "counts": counts,
        "timing": {"wall_s": t0.elapsed().as_secs_f64(), "prepare_s": prepare_s},
        "memory": {"vmhwm_kb": vmhwm_kb()},
        "threads": threads,
    });
    let otmp = out.join("OUTCOME.json.part");
    std::fs::write(
        &otmp,
        serde_json::to_string_pretty(&outcome).unwrap() + "\n",
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&otmp, out.join("OUTCOME.json")).map_err(|e| e.to_string())?;
    event(json!({"kind":"job_finished","exit_code":exit}));
    Ok(exit)
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
    let exe = o.binary["path"].as_str().unwrap_or("rr_node").to_string();
    let reproduce = format!(
        "{} solve-recipe --family {} --order {} --recipe {} --key {} --out {}",
        q(&exe),
        q(&o.family_path),
        q(&o.order_text),
        q(&o.recipe_path),
        keyarg,
        q(&format!(
            "/tmp/recipe-repro-{}-{}",
            &ctx.recipe_digest[..12],
            keyarg.replace([',', '-'], "_")
        ))
    );
    json!({
        "type": "relation", "key": key, "verdict": verdict, "target": key_text(key),
        "rhs": Value::Null, "relation_digest": Value::Null,
        "determinism_id": DETERMINISM_ID, "recipe_digest": ctx.recipe_digest,
        "failure": {
            "schema": FAILURE_SCHEMA, "class": class, "message": message, "detail": detail,
            "node": ctx.node,
            "recipe": {"id": ctx.recipe.id, "digest": ctx.recipe_digest, "path": o.recipe_path},
            "key": key, "determinism_id": DETERMINISM_ID, "binary": o.binary,
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
            &[branch.clone()],
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
    let options = SearchOptions {
        max_depth: None,
        prime: PRIME,
        sample_seed: SAMPLE_SEED,
    };
    let (res, cap) = with_proof_capture::<N, _>(|| {
        with_explicit_seeds(Arc::new(s16), || {
            ctx.solver.solve_case(point.clone(), options)
        })
    });
    let candidate = match res {
        Ok(c) => c,
        Err(SolverError::SearchExhausted { rows, .. }) => {
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
        Err(SolverError::ExactReplay(m)) => {
            return failure(
                ctx,
                key,
                "IRREGULAR",
                "irregular",
                &format!("exact lift: {m}"),
                json!({"pole": m}),
            );
        }
        Err(e) => {
            return failure(
                ctx,
                key,
                "ERROR",
                "error",
                &format!("{e:?}"),
                json!({"message": format!("{e:?}")}),
            );
        }
    };
    let target: Vec<i64> = (0..ctx.n)
        .map(|i| i64::from(candidate.target[i].value()))
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
        if candidate.sources.len() > limit {
            return failure(
                ctx,
                key,
                "BUDGET_WORK",
                "budget_work",
                "trace rows above --max-trace-rows",
                json!({"counter":"trace_rows","limit":limit,"value":candidate.sources.len()}),
            );
        }
    }
    let order = ctx.solver.ordering();
    let lines = ctx.recipe.lines();
    let mut rhs = Vec::new();
    let mut rhs_canon = Vec::new();
    let mut top = Vec::new();
    let mut all_lower = true;
    let mut first_not_lower: Option<Vec<i64>> = None;
    let mut poles: BTreeSet<String> = BTreeSet::new();
    for t in &candidate.rhs {
        let k: Vec<i64> = (0..ctx.n)
            .map(|i| i64::from(t.integral[i].value()))
            .collect();
        let lower = order.compare(&candidate.target, &t.integral) == std::cmp::Ordering::Less;
        if !lower {
            all_lower = false;
            first_not_lower.get_or_insert(k.clone());
        }
        if (0..ctx.n).all(|j| !lines[j] || k[j] >= 1) {
            top.push(json!({"key": k, "lower": lower}));
        }
        if !t.coefficient.denominator.is_constant() {
            poles.insert(t.coefficient.denominator.to_string());
        }
        let c = t.coefficient.to_string();
        rhs.push(json!([t.integral.to_string(), c]));
        rhs_canon.push(json!([k, c]));
    }
    for p in &candidate.hidden_poles {
        poles.insert(p.to_string());
    }
    let canon = json!({"rhs": rhs_canon, "target": key});
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
    // exact proof with the lift's own weights
    let mut seeds_used: Vec<Vec<i64>> = Vec::new();
    let proof = if !ctx.opts.proof {
        Value::Null
    } else {
        let Some(cap) = cap else {
            return failure(
                ctx,
                key,
                "PROOF_FAILED",
                "proof_failed",
                "the lift recorded no proof capture",
                json!({}),
            );
        };
        let Some(weights) = cap.weights.as_ref() else {
            return failure(
                ctx,
                key,
                "PROOF_FAILED",
                "proof_failed",
                "the lift recorded no source weights",
                json!({}),
            );
        };
        if cap.rows.len() != candidate.sources.len() {
            return failure(
                ctx,
                key,
                "PROOF_FAILED",
                "proof_failed",
                "trace rows and sources differ in length",
                json!({}),
            );
        }
        let mut acc: BTreeMap<crate::solver::Integral<N>, Coefficient> = BTreeMap::new();
        for (j, w) in weights {
            let Some(row) = cap.rows.get(*j) else {
                return failure(
                    ctx,
                    key,
                    "PROOF_FAILED",
                    "proof_failed",
                    "a weight names a missing row",
                    json!({"row": j}),
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
        for term in &cap.exact {
            match acc.get_mut(&term.integral) {
                Some(e) => *e = &*e - &term.coefficient,
                None => {
                    acc.insert(term.integral, -term.coefficient.clone());
                }
            }
        }
        if let Some((col, _)) = acc.iter().find(|(_, v)| !v.is_zero()) {
            return failure(
                ctx,
                key,
                "PROOF_FAILED",
                "proof_failed",
                "sum_j w_j row_j differs from the lifted row",
                json!({"column": col.to_string()}),
            );
        }
        let mut used: BTreeSet<Vec<i64>> = BTreeSet::new();
        for (j, w) in weights {
            if !w.is_zero() {
                let s = &candidate.sources[*j].seed.integral;
                used.insert(
                    (0..ctx.n)
                        .map(|i| i64::from(s[i].value()) - key[i])
                        .collect(),
                );
            }
        }
        seeds_used = used.into_iter().collect();
        json!({"kind": "exact-weights", "weights": weights.len(), "ok": true,
            "modulo": "proved-zero sectors (their integrals vanish)"})
    };
    json!({
        "type": "relation", "key": key, "verdict": if ctx.opts.proof { "SOLVED" } else { "SOLVED_UNPROVED" },
        "determinism_id": DETERMINISM_ID, "recipe_digest": ctx.recipe_digest,
        "target": candidate.target.to_string(), "rhs": rhs, "relation_digest": relation_digest,
        "rhs_count": candidate.rhs.len(), "top_rhs": top, "all_lower": all_lower,
        "seeds_total": n_seeds, "rows": candidate.stats.rows, "trace_rows": candidate.sources.len(),
        "seeds_used": seeds_used, "proof": proof, "d_poles": poles.into_iter().collect::<Vec<_>>(),
        "failure": Value::Null,
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
        BatchOptions {
            threads,
            max_trace_rows: None,
            proof: true,
            family_path: "kite.sym".into(),
            recipe_path: "kite.recipe.json".into(),
            order_text: NodeOrder::identity().to_json(),
            binary: json!({"path": "test", "blake3": null, "git_commit": "test"}),
        }
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
