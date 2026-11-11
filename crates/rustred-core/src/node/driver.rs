//! Node generation under ONE node order: the face-queue loop of the 5L campaign (formerly the external
//! `face_queue_r9` harness) as engine code.
//!
//! One chart of a one-sided family is solved by the FirstValid case queue: each queued case is searched
//! (modular discovery + exact lift of the first winning trace), its strict descent is checked, its exceptional
//! branches are admitted as child cases (G-split refinement, deferring), children covered by earlier rules (own or
//! cover tables) are dropped. Differences to the harness, all driven by the 5L campaign findings:
//!
//! * the order is a [`NodeOrder`] (presentation permutation, tie priority, source visit order), recorded in the
//!   table header; the family is solved in its presented form and the table is in presented slots;
//! * every lift may be pre-screened ([`super::screen`]); a refused lift is retried with other source visit
//!   orders (other candidate relations for the same pivot) and, if all are refused, the node stops with
//!   [`StopReason::LiftRefused`] naming the case, so that the caller can choose another node order;
//! * the fully fixed (numeric endpoint) cases are solved one by one through the same case search with the node's
//!   source order and exact backend (the r9 harness sent them to the shared numeric solve, which ignores both);
//!   only points the per-point search exhausts go to the shared solve, whose residuals are the chart residuals;
//! * every rule records its raw pivot / recenter, trace rows and lift time (replay authority, probe scores).

use std::cmp::Ordering;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::order::{NodeOrder, SourceOrder};
use super::screen::{LiftScreen, ScreenReport};
use crate::algebra::CoefficientPolynomial;
use crate::family::IntegralFamily;
use crate::input::{Compiler, Limits, LoweringLimits};
use crate::sector::{Mask, zero};
use crate::solver::{
    Case, CaseIntersectionLimits, CaseRefinement, CoordinateCase, ExceptionalConditions, Integral, RuleCandidate,
    SearchEvent, SearchOptions, SectorConfig, SectorRule, SectorSolveOptions, SectorSolver, SolverError, SourceSystem,
    SymbolicExactBackend, UnsupportedBranchPolicy, extract_exceptions, GuardRoute, GuardRouteTaken, extract_exceptions_routed,
};

/// Table schema of node tables: table_dump.v1 records plus node fields (header `node_order`, rule `recenter`, ...).
pub const SCHEMA: &str = "table_dump.v1";
pub const NODE_SCHEMA: &str = "rustred.node.v1";

/// Options of one node (chart) generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeOptions {
    pub order: NodeOrder,
    /// Chart mask in PRESENTED slots; `None` = the top chart of the presented family.
    pub chart: Option<String>,
    /// Initial cases in ORIGINAL slots (fixed values, linear equations); empty = the whole chart.
    pub initial: Vec<CaseSpec>,
    pub exact_backend: String,
    pub max_depth: Option<u32>,
    pub max_cases: Option<usize>,
    pub gsplit: bool,
    /// Fork-timed native guard extraction (seconds); on a timeout the exact content route (rr-5l-api) is used
    /// if `guard_content_fallback`, else the node stops with `GuardTimeout`.
    pub native_try: Option<u64>,
    #[serde(default = "yes")]
    pub guard_content_fallback: bool,
    /// Fork-timed admission of nonlinear exceptional branches (seconds); a timeout defers the region.
    pub admit_try: Option<u64>,
    pub screen: Option<LiftScreen>,
    /// Source visit orders tried, in turn, for a case whose lift the screen refused.
    pub retry_source_orders: Vec<SourceOrder>,
    /// Watchdog (supervised children only): after this many seconds on one case, write `F.spill.json` and
    /// terminate the PROCESS with exit status 9.
    pub case_budget_exit: Option<u64>,
    /// Earlier rules (FirstValid order) pruning queued cases: (table path, rules 0..upto).
    pub cover_tables: Vec<(String, usize)>,
    pub cand_dump: bool,
    pub numeric_depth: u32,
}

fn yes() -> bool {
    true
}

impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            order: NodeOrder::identity(),
            chart: None,
            initial: Vec::new(),
            exact_backend: "targetonlyfactorized".into(),
            max_depth: None,
            max_cases: None,
            gsplit: true,
            native_try: Some(60),
            guard_content_fallback: true,
            admit_try: Some(120),
            screen: None,
            retry_source_orders: vec![SourceOrder::Reverse],
            case_budget_exit: None,
            cover_tables: Vec::new(),
            cand_dump: false,
            numeric_depth: 2,
        }
    }
}

/// A case: fixed values (`None` = free) and linear integer equations in `n<i>` (`= 0`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseSpec {
    pub fixed: Vec<Option<i64>>,
    #[serde(default)]
    pub equations: Vec<String>,
}

impl CaseSpec {
    /// `1,1,_,0 n0+n2-1;n3` (the face-queue `initial` syntax).
    pub fn parse(spec: &str, equations: &str) -> Result<Self, String> {
        let fixed = spec
            .split(',')
            .map(|v| {
                let v = v.trim();
                if v == "_" { Ok(None) } else { v.parse().map(Some).map_err(|e| format!("case value {v}: {e}")) }
            })
            .collect::<Result<_, String>>()?;
        let equations = equations.split(';').map(str::trim).filter(|e| !e.is_empty()).map(str::to_string).collect();
        Ok(Self { fixed, equations })
    }
}

/// Why a node stopped before completing its chart.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StopReason {
    /// Every tried relation for this case (PRESENTED slots) was refused by the lift pre-screen.
    LiftRefused { case: CaseSpec, reports: Vec<ScreenReport> },
    /// The native guard extraction did not finish within `native_try` seconds.
    GuardTimeout { case: CaseSpec, rhs: usize },
    MaxCases { pending: usize },
    Error(String),
}

/// Result of [`generate`]. `complete` = the chart closed (no stop, no error).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodeOutcome {
    pub complete: bool,
    pub rules: usize,
    pub rhs_terms: usize,
    pub residuals: usize,
    pub residual_keys: Vec<String>,
    pub deferred_regions: usize,
    pub deferred_branches: usize,
    pub symbolic_cases: usize,
    pub numerical_cases: usize,
    pub numeric_bulk_fallback: usize,
    pub screened_lifts: usize,
    pub refused_lifts: usize,
    pub source_order_retries: usize,
    #[serde(default)]
    pub content_routes: usize,
    pub not_lower: Vec<String>,
    pub stop: Option<StopReason>,
    pub seconds: f64,
    pub vmhwm_kb: u64,
    pub table: String,
    /// First symbolic rule (probe score): trace rows, lift seconds, RHS terms, max coefficient total degree.
    pub first_rule: Option<RuleScore>,
    /// Scores of every symbolic rule, in queue order (probe portfolios).
    #[serde(default)]
    pub rule_scores: Vec<RuleScore>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleScore {
    pub trace_rows: usize,
    pub lift_seconds: f64,
    pub search_seconds: f64,
    pub rhs_terms: usize,
    pub max_degree: usize,
    pub direct_hit: bool,
    pub branches: usize,
}

/// Progress events of [`generate`] (also written to the queue log).
#[derive(Clone, Debug, Serialize)]
pub enum NodeEvent {
    Prepared { basis: usize, seconds: f64 },
    CaseStarted { case: CaseSpec, pending: usize },
    LiftScreened { report: ScreenReport },
    RelationRetry { case: CaseSpec, source_order: String },
    RuleFound { index: usize, rhs: usize, trace_rows: usize, lift_seconds: f64, branches: usize },
    NumericalPoint { fixed: Vec<i64>, outcome: String },
    Stopped { reason: String },
}

pub(crate) fn q(s: &str) -> String {
    serde_json::to_string(s).expect("json string")
}

fn strings<T: std::fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    let parts: Vec<String> = items.into_iter().map(|x| q(&x.to_string())).collect();
    format!("[{}]", parts.join(","))
}

fn mask_string(bits: &[bool]) -> String {
    bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

pub fn vmhwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        .unwrap_or(0)
}

pub fn parse_backend(name: &str) -> Result<SymbolicExactBackend, String> {
    Ok(match name {
        "sparse" => SymbolicExactBackend::Sparse,
        "factorized" => SymbolicExactBackend::SparseFactorized,
        "targetonly" => SymbolicExactBackend::SparseTargetOnly,
        "targetonlyfactorized" => SymbolicExactBackend::SparseTargetOnlyFactorized,
        #[cfg(feature = "reconstruction")]
        "sampled" => SymbolicExactBackend::Sampled { threads: 1, seed: 0x5eed, min_rows: 600 },
        other => return Err(format!("unknown exact backend {other}")),
    })
}

/// Compile a compact family text.
pub fn compile_family(text: &str) -> Result<IntegralFamily, String> {
    Ok(Compiler::new(Limits::default())
        .map_err(|e| format!("compiler: {e:?}"))?
        .compile_compact(text, None)
        .map_err(|e| format!("compile family: {e:?}"))?
        .into_lowered(LoweringLimits::default())
        .map_err(|e| format!("lower family: {e:?}"))?
        .into_family())
}

/// Generate one chart of `family_text` (ORIGINAL presentation) under `opts.order` into `out_dir`
/// (`F.family.sym` = presented family, `F.table.jsonl`, `F.queue.jsonl`, `F.cert.jsonl`, optional candidate dumps).
pub fn generate(family_text: &str, family_path: &str, out_dir: &Path, opts: &NodeOptions, events: &mut dyn FnMut(&NodeEvent)) -> Result<NodeOutcome, String> {
    let presented = opts.order.present_family(family_text)?;
    let fam = compile_family(&presented)?;
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    std::fs::write(out_dir.join("F.family.sym"), &presented).map_err(|e| format!("family copy: {e}"))?;
    let n = fam.denominator_count();
    opts.order.validate(n)?;
    match n {
        5 => run::<5>(family_text, family_path, &presented, &fam, out_dir, opts, events),
        9 => run::<9>(family_text, family_path, &presented, &fam, out_dir, opts, events),
        12 => run::<12>(family_text, family_path, &presented, &fam, out_dir, opts, events),
        14 => run::<14>(family_text, family_path, &presented, &fam, out_dir, opts, events),
        20 => run::<20>(family_text, family_path, &presented, &fam, out_dir, opts, events),
        other => Err(format!("arity {other}: node driver compiled for 5, 9, 12, 14, 20")),
    }
}

/// The node order's [`SectorConfig`] pieces: legacy order, or the compiled tie-priority program.
fn order_program(order: &NodeOrder, n: usize) -> Result<Option<rustred_order::CompiledOrder>, String> {
    match &order.coordinate_priority {
        None => Ok(None),
        Some(priority) => {
            let mut d = rustred_order::OrderDescriptor::spired_uncut(n);
            d.coordinate_priority = priority.clone();
            rustred_order::CompiledOrder::compile(d, Default::default()).map(Some).map_err(|e| format!("order program: {e:?}"))
        }
    }
}

fn descriptor_json(order: &NodeOrder, n: usize) -> String {
    let mut d = rustred_order::OrderDescriptor::spired_uncut(n);
    if let Some(p) = &order.coordinate_priority {
        d.coordinate_priority = p.clone();
    }
    format!(
        "{{\"support_priority\":{:?},\"coordinate_priority\":{:?},\"two_sided\":{:?}}}",
        d.support_priority, d.coordinate_priority, d.two_sided
    )
}

pub(crate) fn case_spec<const N: usize>(c: &Case<N>, n: usize) -> CaseSpec {
    let fixed = c.fixed()[..n].iter().map(|v| v.map(i64::from)).collect();
    let equations = match c {
        Case::Coordinate(_) => Vec::new(),
        Case::Affine(a) => a.equations().iter().map(|e| e.to_string()).collect(),
    };
    CaseSpec { fixed, equations }
}

fn case_json<const N: usize>(c: &Case<N>, n: usize) -> String {
    serde_json::to_string(&case_spec(c, n)).expect("case json")
}

/// Linear integer expression in `n<i>` (`3*n1-n2+4`) as a coefficient polynomial of the family's index variables.
fn parse_index_poly(expr: &str, template: &CoefficientPolynomial, idx: &[usize]) -> Option<CoefficientPolynomial> {
    let vars = template.variables();
    let s: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    if s.is_empty() {
        return None;
    }
    let mut acc = template.zero();
    let mut monos: Vec<(bool, String)> = Vec::new();
    let mut cur = String::new();
    let mut neg = false;
    for (k, ch) in s.chars().enumerate() {
        if (ch == '+' || ch == '-') && k > 0 && !cur.ends_with('^') {
            monos.push((neg, std::mem::take(&mut cur)));
            neg = ch == '-';
        } else if (ch == '+' || ch == '-') && k == 0 {
            neg = ch == '-';
        } else {
            cur.push(ch);
        }
    }
    monos.push((neg, cur));
    for (neg, body) in monos {
        let mut c = symbolica::domains::integer::Integer::one();
        let mut m = template.one();
        for f in body.split('*') {
            if f.chars().all(|x| x.is_ascii_digit()) && !f.is_empty() {
                c = &c * &symbolica::domains::integer::Integer::from(f.parse::<i64>().ok()?);
            } else {
                let (name, e) = match f.split_once('^') {
                    Some((a, b)) => (a, b.parse::<u32>().ok()?),
                    None => (f, 1),
                };
                let i: usize = name.strip_prefix('n')?.parse().ok()?;
                let v = template.variable(&vars[*idx.get(i)?]).ok()?;
                for _ in 0..e {
                    m = &m * &v;
                }
            }
        }
        if neg {
            c = -c;
        }
        m = m.mul_coeff(c);
        acc = &acc + &m;
    }
    Some(acc)
}

fn build_case<const N: usize>(spec: &CaseSpec, template: &CoefficientPolynomial, idx: &[usize; N], sector: &[bool; N], n: usize) -> Result<Option<Case<N>>, String> {
    if spec.fixed.len() != n {
        return Err(format!("case arity {} != {n}", spec.fixed.len()));
    }
    let mut fixed = [Some(0i16); N];
    for (i, v) in spec.fixed.iter().enumerate() {
        fixed[i] = v.map(|x| i16::try_from(x).map_err(|_| format!("case value {x}"))).transpose()?;
    }
    let face = Case::from(CoordinateCase::new(fixed).map_err(|e| format!("case: {e:?}"))?);
    let polys: Vec<CoefficientPolynomial> =
        spec.equations.iter().map(|e| parse_index_poly(e, template, idx).ok_or_else(|| format!("equation {e}"))).collect::<Result<_, _>>()?;
    face.intersect(&polys, idx, sector).map_err(|e| format!("admit case: {e:?}"))
}

fn max_total_degree(p: &CoefficientPolynomial) -> usize {
    (0..p.nterms()).map(|i| p.exponents(i).iter().map(|e| *e as usize).sum::<usize>()).max().unwrap_or(0)
}

/// Max total degree over numerators and denominators of a rule's RHS coefficients.
pub fn rule_degree<const N: usize>(c: &RuleCandidate<N>) -> usize {
    c.rhs.iter().map(|t| max_total_degree(&t.coefficient.numerator).max(max_total_degree(&t.coefficient.denominator))).max().unwrap_or(0)
}

fn power_byte(p: crate::solver::Power) -> u8 {
    (p.value() as u8 & 0x7f) | if p.is_symbolic() { 0x80 } else { 0 }
}

struct RuleExtras {
    raw_pivot: Option<Vec<u8>>,
    recenter: Option<Vec<i16>>,
    trace_rows: usize,
    lift_seconds: f64,
    source_order: String,
    numerical: bool,
}

fn write_rule<const N: usize>(f: &mut impl std::io::Write, chart: &str, index: usize, rule: &SectorRule<N>, x: &RuleExtras, n: usize) -> usize {
    let c = &rule.candidate;
    let rhs: Vec<String> = c.rhs.iter().map(|t| format!("[{},{}]", q(&t.integral.to_string()), q(&t.coefficient.to_string()))).collect();
    let exceptions: Vec<String> = rule.exceptions.branches.iter().map(|b| strings(b.iter())).collect();
    let spec = case_spec(&c.case, n);
    let fixed: Vec<String> = spec.fixed.iter().map(|v| v.map_or("null".to_string(), |x| x.to_string())).collect();
    let mut seeds: Vec<String> = c.sources.iter().map(|s| s.seed.integral.to_string()).collect();
    seeds.sort();
    seeds.dedup();
    let opt = |v: &Option<Vec<i16>>| v.as_ref().map_or("null".to_string(), |v| format!("{v:?}"));
    let raw = x.raw_pivot.as_ref().map_or("null".to_string(), |v| format!("{v:?}"));
    writeln!(
        f,
        "{{\"type\":\"rule\",\"chart\":{},\"index\":{index},\"case\":{},\"fixed\":[{}],\"equations\":{},\"target\":{},\"rhs\":[{}],\"exceptions\":[{}],\"dispatch\":{},\"nsources\":{},\"seeds\":{},\"direct_hit\":{},\"seconds_search\":{:.6},\"raw_pivot_encoded\":{raw},\"recenter\":{},\"trace_rows\":{},\"lift_seconds\":{:.6},\"source_order\":{},\"numerical\":{}}}",
        q(chart),
        q(&format!("{:?}", c.case)),
        fixed.join(","),
        strings(spec.equations.iter()),
        q(&c.target.to_string()),
        rhs.join(","),
        exceptions.join(","),
        q(&format!("{:?}", rule.dispatch_policy)),
        c.sources.len(),
        strings(seeds.iter()),
        c.stats.direct_hit,
        c.stats.elapsed.as_secs_f64(),
        opt(&x.recenter),
        x.trace_rows,
        x.lift_seconds,
        q(&x.source_order),
        x.numerical,
    )
    .expect("write rule");
    c.rhs.len()
}

fn write_cert_header<const N: usize>(f: &mut impl std::io::Write, fam: &IntegralFamily, sources: &SourceSystem<N>) {
    let (loops, externals) = (fam.loop_count(), fam.external_count());
    writeln!(
        f,
        "{{\"type\":\"cert_header\",\"schema\":{},\"family\":{},\"loops\":{loops},\"externals\":{externals},\"source_rows\":{},\"conditions\":{}}}",
        q(SCHEMA),
        q(fam.name()),
        sources.rows().len(),
        strings(sources.conditions().iter())
    )
    .expect("cert header");
    let contraction: Vec<usize> = (loops..loops + externals).rev().chain(0..loops).collect();
    for (m, row) in sources.rows().iter().enumerate() {
        let (loop_id, momentum) = (m / contraction.len(), contraction[m % contraction.len()]);
        let terms: Vec<String> = row.iter().map(|t| format!("[{},{}]", q(&t.integral.to_string()), q(&t.coefficient.to_string()))).collect();
        writeln!(f, "{{\"type\":\"source_row\",\"m\":{m},\"differentiated_loop\":{loop_id},\"contraction_momentum\":{momentum},\"terms\":[{}]}}", terms.join(",")).expect("source row");
    }
}

/// Covering check of the harness (`rule_covers`): the rule's case contains `case` and no exceptional branch meets it.
fn covers<const N: usize>(rcase: &Case<N>, branches: &[Vec<CoefficientPolynomial>], case: &Case<N>, idx: &[usize; N], sector: &[bool; N], limits: CaseIntersectionLimits) -> bool {
    rcase.contains(case).unwrap_or(false)
        && branches.iter().all(|b| matches!(case.intersect_many(b, idx, sector, limits), Ok(r) if r.cases.is_empty()))
}

fn prune_subsumed<const N: usize>(cases: &mut Vec<Case<N>>, candidate: &Case<N>) -> bool {
    if cases.iter().any(|queued| queued.contains(candidate).unwrap_or(false)) {
        return false;
    }
    cases.retain(|c| !candidate.contains(c).unwrap_or(false));
    true
}

// ---------------------------------------------------------------------------------------------------------------
// process helpers (fork-timed tries and the case-budget watchdog), as in the campaign harness
unsafe extern "C" {
    pub(crate) fn fork() -> i32;
    pub(crate) fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    pub(crate) fn kill(pid: i32, sig: i32) -> i32;
    pub(crate) fn _exit(code: i32) -> !;
}

static FORK_CHILD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

pub enum Forked {
    Done(String, f64),
    Timeout(f64),
    Failed(String, f64),
}

/// Run `f` in a forked child (its String result passed through a file in `dir`); kill it after `secs`.
/// The caller must be single-threaded apart from idle helper threads (RAYON_NUM_THREADS=1 in campaign runs).
pub fn forked<F: FnOnce() -> String>(dir: &Path, tag: &str, secs: u64, f: F) -> Forked {
    let path = dir.join(format!("rr_fork_{}_{}.out", std::process::id(), tag));
    let _ = std::fs::remove_file(&path);
    let t = Instant::now();
    // SAFETY: fork in a process whose other threads (if any) hold no locks the child needs; the child only runs
    // `f`, writes one file and _exits.
    let pid = unsafe { fork() };
    if pid < 0 {
        return Forked::Failed("fork failed".into(), 0.0);
    }
    if pid == 0 {
        let s = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| "{\"child_panic\":true}".to_string());
        let tmp = path.with_extension("part");
        let ok = std::fs::write(&tmp, s.as_bytes()).is_ok() && std::fs::rename(&tmp, &path).is_ok();
        // SAFETY: terminate the forked child without running the parent's destructors.
        unsafe { _exit(if ok { 0 } else { 3 }) }
    }
    FORK_CHILD.store(pid, std::sync::atomic::Ordering::SeqCst);
    loop {
        let mut status = 0i32;
        // SAFETY: own child, WNOHANG.
        let r = unsafe { waitpid(pid, &mut status, 1) };
        if r == pid {
            FORK_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
            let used = t.elapsed().as_secs_f64();
            return match std::fs::read_to_string(&path) {
                Ok(s) => {
                    let _ = std::fs::remove_file(&path);
                    Forked::Done(s, used)
                }
                Err(_) => Forked::Failed(format!("child exited, status {status}, no output"), used),
            };
        }
        if t.elapsed().as_secs() >= secs {
            // SAFETY: own child.
            unsafe {
                kill(pid, 9);
                waitpid(pid, &mut status, 0);
            }
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(path.with_extension("part"));
            FORK_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
            return Forked::Timeout(t.elapsed().as_secs_f64());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

struct WatchState {
    started: Option<Instant>,
    current: String,
    pending: Vec<String>,
    numerical: Vec<String>,
    rules_done: usize,
}
static WATCH: Mutex<Option<WatchState>> = Mutex::new(None);

fn start_watchdog(budget: u64, spill: PathBuf) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let g = WATCH.lock().unwrap();
            let Some(s) = g.as_ref() else { continue };
            let Some(t) = s.started else { continue };
            if t.elapsed().as_secs() < budget {
                continue;
            }
            let child = FORK_CHILD.load(std::sync::atomic::Ordering::SeqCst);
            if child > 0 {
                // SAFETY: our own forked child.
                unsafe {
                    kill(child, 9);
                }
            }
            let body = format!(
                "{{\"type\":\"spill\",\"reason\":\"case_budget\",\"budget\":{budget},\"case_seconds\":{:.3},\"rules_done\":{},\"current\":{},\"pending\":[{}],\"numerical\":[{}]}}\n",
                t.elapsed().as_secs_f64(),
                s.rules_done,
                s.current,
                s.pending.join(","),
                s.numerical.join(",")
            );
            let tmp = spill.with_extension("part");
            let _ = std::fs::write(&tmp, body.as_bytes());
            let _ = std::fs::rename(&tmp, &spill);
            eprintln!("SPILL case budget {budget} s exceeded; exiting 9");
            std::process::exit(9);
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run<const N: usize>(
    original_text: &str,
    family_path: &str,
    presented: &str,
    fam: &IntegralFamily,
    out_dir: &Path,
    opts: &NodeOptions,
    events: &mut dyn FnMut(&NodeEvent),
) -> Result<NodeOutcome, String> {
    let n = fam.denominator_count();
    let t_total = Instant::now();
    if fam.power_shifts().iter().any(|s| s.to_string().trim() != "0") {
        return Err("two-sided (eps) families are generated by the engine's two-sided queue (rr-5l-api), not by the node driver".into());
    }
    let sources = SourceSystem::<N>::from_family_with_capacity(fam, true).map_err(|e| format!("sources: {e:?}"))?;
    let flags = super::order::physical_slots(presented);
    if flags.len() != n {
        return Err("family propagator lines do not match the compiled arity".into());
    }
    let top: [bool; N] = std::array::from_fn(|i| i < n && flags[i]);
    let base: [bool; N] = match &opts.chart {
        Some(m) => {
            let b: Vec<bool> = m.chars().map(|c| c == '1').collect();
            if b.len() != n {
                return Err(format!("chart {m}: arity"));
            }
            std::array::from_fn(|i| i < n && b[i])
        }
        None => top,
    };
    let label = mask_string(&base[..n]);
    let analyzer = zero::Analyzer::try_unrestricted(fam).map_err(|e| format!("zero analyzer: {e:?}"))?;
    let scope: Vec<usize> = (0..n).filter(|&i| top[i]).collect();
    let t0 = Instant::now();
    let mut zeros: Vec<[bool; N]> = Vec::new();
    for bits in 0u64..(1u64 << scope.len()) {
        let mut active = [false; N];
        for (j, &i) in scope.iter().enumerate() {
            active[i] = bits >> j & 1 == 1;
        }
        let m = Mask::try_new(active.iter().copied()).map_err(|e| format!("mask: {e:?}"))?;
        if let zero::Decision::ProvedZero(_) = analyzer.analyze(&m).map_err(|e| format!("zero analysis: {e:?}"))? {
            zeros.push(active);
        }
    }
    let zero_seconds = t0.elapsed().as_secs_f64();
    let table_path = out_dir.join("F.table.jsonl");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&table_path).map_err(|e| format!("table: {e}"))?);
    let mut qlog = std::io::BufWriter::new(std::fs::File::create(out_dir.join("F.queue.jsonl")).map_err(|e| format!("queue log: {e}"))?);
    let options_json = serde_json::to_string(opts).map_err(|e| e.to_string())?;
    writeln!(
        out,
        "{{\"type\":\"header\",\"schema\":{},\"node_schema\":{},\"source_base\":\"rustred-node\",\"family\":{},\"family_file\":{},\"family_source\":{},\"original_family_source\":{},\"loops\":{},\"externals\":{},\"denominators\":{n},\"parameters\":{},\"power_shifts\":{},\"mask\":{},\"node_order\":{},\"order_name\":{},\"order_is_program\":{},\"order\":{},\"two_sided_axes\":{:?},\"charts\":{},\"numerical_depth\":{},\"max_numerator_rank\":null,\"source_conditions\":{},\"zero_scope\":\"downset\",\"zero_seconds\":{zero_seconds:.3},\"node_options\":{options_json}}}",
        q(SCHEMA),
        q(NODE_SCHEMA),
        q(fam.name()),
        q(family_path),
        q(presented),
        q(original_text),
        fam.loop_count(),
        fam.external_count(),
        strings(fam.coefficient_context().parameter_names().iter()),
        strings(fam.power_shifts().iter()),
        q(&mask_string(&top[..n])),
        opts.order.to_json(),
        q(&opts.order.name),
        opts.order.coordinate_priority.is_some(),
        descriptor_json(&opts.order, n),
        vec![false; n],
        strings([label.clone()].iter()),
        opts.numeric_depth,
        strings(sources.conditions().iter()),
    )
    .map_err(|e| e.to_string())?;
    writeln!(out, "{{\"type\":\"zero_sectors\",\"scope\":\"downset\",\"count\":{},\"masks\":{}}}", zeros.len(), strings(zeros.iter().map(|z| mask_string(&z[..n])))).map_err(|e| e.to_string())?;
    let guards: Vec<String> = (0..n).map(|i| if base[i] { "n>=1".to_string() } else { "n<=0".to_string() }).collect();
    let mut outcome = NodeOutcome { table: table_path.display().to_string(), ..Default::default() };
    if zeros.contains(&base) {
        writeln!(out, "{{\"type\":\"chart\",\"chart\":{},\"guards\":{},\"zero\":true}}", q(&label), strings(guards.iter())).map_err(|e| e.to_string())?;
        writeln!(out, "{{\"type\":\"end\",\"charts\":1,\"rules\":0,\"rhs_terms\":0,\"residuals\":0,\"chart_errors\":0,\"seconds\":{:.3},\"vmhwm_kb\":{},\"zero_chart\":true,\"native_descent_not_lower\":[]}}", t_total.elapsed().as_secs_f64(), vmhwm_kb()).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        outcome.complete = true;
        return Ok(outcome);
    }
    writeln!(out, "{{\"type\":\"chart\",\"chart\":{},\"guards\":{},\"zero\":false}}", q(&label), strings(guards.iter())).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    let exact_backend = parse_backend(&opts.exact_backend)?;
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros.clone()),
        integral_order: order_program(&opts.order, n)?,
        symbolic_exact_backend: exact_backend,
        ..Default::default()
    };
    let t1 = Instant::now();
    let (solver, derivation) = SectorSolver::with_basis_derivation(&sources, base, config).map_err(|e| format!("solver: {e:?}"))?;
    let nb = solver.basis().len();
    events(&NodeEvent::Prepared { basis: nb, seconds: t1.elapsed().as_secs_f64() });
    let idx: [usize; N] = *sources.index_variables();
    let sector: [bool; N] = *solver.ordering().sector();
    let template = sources.rows().iter().flat_map(|r| r.iter()).next().ok_or("empty source system")?.coefficient.clone();
    // certificate file: header, original source rows, basis derivation (replay authority for retained traces)
    let mut cert = std::io::BufWriter::new(std::fs::File::create(out_dir.join("F.cert.jsonl")).map_err(|e| format!("cert: {e}"))?);
    write_cert_header(&mut cert, fam, &sources);
    for (k, row) in derivation.rows.iter().enumerate() {
        let entries: Vec<String> = row.iter().map(|(m, p)| format!("[{m},{}]", q(&p.to_string()))).collect();
        writeln!(cert, "{{\"type\":\"basis_row\",\"chart\":{},\"k\":{k},\"sources\":[{}]}}", q(&label), entries.join(",")).map_err(|e| e.to_string())?;
    }
    cert.flush().map_err(|e| e.to_string())?;
    let limits = CaseIntersectionLimits::default();
    let mut symbolic = SectorSolveOptions::default().symbolic;
    if opts.max_depth.is_some() {
        symbolic.max_depth = opts.max_depth;
    }
    let node_source = opts.order.source_order.clone();
    writeln!(qlog, "{{\"type\":\"options\",\"node_order\":{},\"basis\":{nb},\"options\":{options_json}}}", opts.order.to_json()).map_err(|e| e.to_string())?;
    // cover tables (rules of earlier pieces, FirstValid order)
    let mut cover: Vec<(String, Case<N>, Vec<Vec<CoefficientPolynomial>>)> = Vec::new();
    for (path, upto) in &opts.cover_tables {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cover table {path}: {e}"))?;
        let (mut loaded, mut skipped) = (0usize, 0usize);
        for line in text.lines().filter(|l| l.starts_with("{\"type\":\"rule\"")) {
            let v: serde_json::Value = serde_json::from_str(line).map_err(|e| format!("cover table {path}: {e}"))?;
            if v["index"].as_u64().is_none_or(|i| i as usize >= *upto) {
                continue;
            }
            let spec = CaseSpec {
                fixed: v["fixed"].as_array().map(|a| a.iter().map(|x| x.as_i64()).collect()).unwrap_or_default(),
                equations: v["equations"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            };
            let case = build_case::<N>(&spec, &template, &idx, &sector, n).ok().flatten();
            let branches: Option<Vec<Vec<CoefficientPolynomial>>> = v["exceptions"].as_array().map(|bs| {
                bs.iter()
                    .map(|b| b.as_array().map(|ps| ps.iter().filter_map(|p| p.as_str().and_then(|p| parse_index_poly(p, &template, &idx))).collect()).unwrap_or_default())
                    .collect()
            });
            match (case, branches) {
                (Some(c), Some(b)) => {
                    cover.push((format!("{path}#{}", v["index"]), c, b));
                    loaded += 1;
                }
                _ => skipped += 1,
            }
        }
        writeln!(qlog, "{{\"type\":\"cover_table\",\"path\":{},\"upto\":{upto},\"loaded\":{loaded},\"skipped\":{skipped}}}", q(path)).map_err(|e| e.to_string())?;
    }
    // initial cases (ORIGINAL slots -> presented)
    let mut initial: Vec<Case<N>> = Vec::new();
    if opts.initial.is_empty() {
        initial.push(Case::from(CoordinateCase::new(std::array::from_fn(|i| if i < n { None } else { Some(0) })).map_err(|e| format!("{e:?}"))?));
    }
    for spec in &opts.initial {
        let (fixed, eqs) = opts.order.present_case(&spec.fixed, &spec.equations);
        match build_case::<N>(&CaseSpec { fixed, equations: eqs }, &template, &idx, &sector, n)? {
            Some(c) => {
                writeln!(qlog, "{{\"type\":\"initial\",\"case\":{}}}", case_json(&c, n)).map_err(|e| e.to_string())?;
                if prune_subsumed(&mut initial, &c) {
                    initial.push(c);
                }
            }
            None => writeln!(qlog, "{{\"type\":\"initial_empty\",\"case\":{}}}", serde_json::to_string(spec).unwrap()).map_err(|e| e.to_string())?,
        }
    }
    let mut pending: Vec<Case<N>> = Vec::new();
    let mut numerical: Vec<CoordinateCase<N>> = Vec::new();
    let mut rules: Vec<SectorRule<N>> = Vec::new();
    let mut discarded = 0usize;
    #[allow(clippy::too_many_arguments)]
    fn enqueue<const N: usize>(
        case: Case<N>,
        from: Option<usize>,
        pending: &mut Vec<Case<N>>,
        numerical: &mut Vec<CoordinateCase<N>>,
        rules: &[SectorRule<N>],
        cover: &[(String, Case<N>, Vec<Vec<CoefficientPolynomial>>)],
        idx: &[usize; N],
        sector: &[bool; N],
        limits: CaseIntersectionLimits,
        qlog: &mut impl std::io::Write,
        discarded: &mut usize,
        n: usize,
    ) {
        let from = from.map_or("null".into(), |x| x.to_string());
        for (label, rc, branches) in cover {
            if covers(rc, branches, &case, idx, sector, limits) {
                let _ = writeln!(qlog, "{{\"type\":\"covered\",\"from_rule\":{from},\"by_cover\":{},\"case\":{}}}", q(label), case_json(&case, n));
                *discarded += 1;
                return;
            }
        }
        for (k, rule) in rules.iter().enumerate() {
            if covers(&rule.candidate.case, &rule.exceptions.branches, &case, idx, sector, limits) {
                let _ = writeln!(qlog, "{{\"type\":\"covered\",\"from_rule\":{from},\"by_rule\":{k},\"case\":{}}}", case_json(&case, n));
                *discarded += 1;
                return;
            }
        }
        if !prune_subsumed(pending, &case) {
            let _ = writeln!(qlog, "{{\"type\":\"subsumed_by_pending\",\"from_rule\":{from},\"case\":{}}}", case_json(&case, n));
            *discarded += 1;
            return;
        }
        if case.is_numerical() {
            let fixed = *case.face();
            if numerical.contains(&fixed) {
                *discarded += 1;
                return;
            }
            let _ = writeln!(qlog, "{{\"type\":\"enqueue_numerical\",\"from_rule\":{from},\"case\":{}}}", case_json(&case, n));
            numerical.push(fixed);
            return;
        }
        match pending.binary_search_by(|queued| queued.queue_cmp(&case)) {
            Ok(_) => *discarded += 1,
            Err(position) => {
                let _ = writeln!(qlog, "{{\"type\":\"enqueue\",\"from_rule\":{from},\"case\":{}}}", case_json(&case, n));
                pending.insert(position, case);
            }
        }
    }
    initial.sort_unstable_by(Case::queue_cmp);
    for case in initial {
        enqueue(case, None, &mut pending, &mut numerical, &rules, &cover, &idx, &sector, limits, &mut qlog, &mut discarded, n);
    }
    qlog.flush().map_err(|e| e.to_string())?;
    if let Some(budget) = opts.case_budget_exit {
        start_watchdog(budget, out_dir.join("F.spill.json"));
    }
    let t2 = Instant::now();
    let mut error: Option<String> = None;
    let mut stop: Option<StopReason> = None;
    let (mut terms, mut deferred_regions, mut deferred_branches, mut branches_total) = (0usize, 0usize, 0usize, 0usize);
    let screen = opts.screen;
    // ---- one case search with the node's source order; the screen may refuse the lift
    struct Searched<const N: usize> {
        candidate: RuleCandidate<N>,
        raw_pivot: Option<Integral<N>>,
        lift_seconds: f64,
        source_order: String,
    }
    let search = |case: &Case<N>, so: &SourceOrder, options: SearchOptions| -> (Result<Searched<N>, SolverError>, Vec<ScreenReport>) {
        let mut raw: Option<Integral<N>> = None;
        let mut started: Option<Instant> = None;
        let mut lift = 0.0f64;
        let result = super::screen::with_screen(screen, || {
            let observe = |event: SearchEvent<N>| match event {
                SearchEvent::ExactStarted { pivot, trace_rows, .. } => {
                    raw = Some(pivot);
                    started = Some(Instant::now());
                    eprintln!("EVENT {:.3}s ExactStarted rows={trace_rows} pivot={pivot}", t2.elapsed().as_secs_f64());
                }
                SearchEvent::CanonicalizationStarted { .. } => {
                    if let Some(s) = started {
                        lift = s.elapsed().as_secs_f64();
                    }
                }
                _ => {}
            };
            match so.visit(nb) {
                None => solver.solve_case_with_observer(case.clone(), options, observe),
                Some(order) => solver.solve_case_with_source_order_and_observer(case.clone(), options, &order, observe),
            }
        });
        let reports = super::screen::take_reports();
        (result.map(|candidate| Searched { candidate, raw_pivot: raw, lift_seconds: lift, source_order: so.label() }), reports)
    };
    let mut symbolic_cases = 0usize;
    while !pending.is_empty() {
        if opts.max_cases.is_some_and(|m| symbolic_cases >= m) {
            stop = Some(StopReason::MaxCases { pending: pending.len() });
            break;
        }
        let current = pending.remove(0);
        if opts.case_budget_exit.is_some() {
            let fixed_json = |f: &CoordinateCase<N>| serde_json::to_string(&CaseSpec { fixed: f.fixed()[..n].iter().map(|v| v.map(i64::from)).collect(), equations: vec![] }).unwrap();
            *WATCH.lock().unwrap() = Some(WatchState {
                started: Some(Instant::now()),
                current: case_json(&current, n),
                pending: pending.iter().map(|c| case_json(c, n)).collect(),
                numerical: numerical.iter().map(fixed_json).collect(),
                rules_done: rules.len(),
            });
        }
        let spec = case_spec(&current, n);
        events(&NodeEvent::CaseStarted { case: spec.clone(), pending: pending.len() });
        writeln!(qlog, "{{\"type\":\"case_started\",\"t\":{:.3},\"pending\":{},\"case\":{}}}", t2.elapsed().as_secs_f64(), pending.len(), case_json(&current, n)).map_err(|e| e.to_string())?;
        qlog.flush().map_err(|e| e.to_string())?;
        let mut orders: Vec<SourceOrder> = vec![node_source.clone()];
        for so in &opts.retry_source_orders {
            if !orders.contains(so) {
                orders.push(so.clone());
            }
        }
        let mut found: Option<Searched<N>> = None;
        let mut refusals: Vec<ScreenReport> = Vec::new();
        for (k, so) in orders.iter().enumerate() {
            if k > 0 {
                outcome.source_order_retries += 1;
                events(&NodeEvent::RelationRetry { case: spec.clone(), source_order: so.label() });
                writeln!(qlog, "{{\"type\":\"relation_retry\",\"source_order\":{},\"case\":{}}}", q(&so.label()), case_json(&current, n)).map_err(|e| e.to_string())?;
            }
            let (result, reports) = search(&current, so, symbolic);
            for r in &reports {
                outcome.screened_lifts += 1;
                events(&NodeEvent::LiftScreened { report: r.clone() });
                writeln!(qlog, "{{\"type\":\"lift_screen\",\"source_order\":{},\"report\":{}}}", q(&so.label()), serde_json::to_string(r).unwrap()).map_err(|e| e.to_string())?;
            }
            let refused = reports.iter().any(|r| matches!(r.decision, super::screen::ScreenDecision::Refuse(_)));
            match result {
                Ok(s) => {
                    found = Some(s);
                    break;
                }
                Err(_) if refused => {
                    outcome.refused_lifts += 1;
                    refusals.extend(reports);
                    continue;
                }
                Err(e) => {
                    error = Some(format!("search error on {current:?}: {e:?}"));
                    writeln!(qlog, "{{\"type\":\"search_error\",\"case\":{},\"error\":{}}}", case_json(&current, n), q(&format!("{e:?}").chars().take(4000).collect::<String>())).map_err(|e| e.to_string())?;
                    break;
                }
            }
        }
        if error.is_some() {
            break;
        }
        let Some(searched) = found else {
            writeln!(qlog, "{{\"type\":\"lift_refused\",\"case\":{}}}", case_json(&current, n)).map_err(|e| e.to_string())?;
            stop = Some(StopReason::LiftRefused { case: spec.clone(), reports: refusals });
            break;
        };
        symbolic_cases += 1;
        let candidate = searched.candidate;
        let nl: Vec<String> = candidate.rhs.iter().filter(|t| solver.ordering().compare(&candidate.target, &t.integral) != Ordering::Less).map(|t| t.integral.to_string()).collect();
        if !nl.is_empty() {
            error = Some(format!("winning rule is not strictly descending on {current:?}"));
            outcome.not_lower.extend(nl);
            break;
        }
        // guard extraction: native, or a fork-timed native try that falls back to the exact content route
        // (rr-5l-api GuardRoute::NativeThenContent); without the fallback a native timeout stops the node
        let te = Instant::now();
        let exceptions: ExceptionalConditions = if let Some(secs) = opts.native_try {
            out.flush().map_err(|e| e.to_string())?;
            qlog.flush().map_err(|e| e.to_string())?;
            if opts.guard_content_fallback {
                let route = GuardRoute::NativeThenContent { native_timeout: std::time::Duration::from_secs(secs) };
                match extract_exceptions_routed(&candidate, &idx, &sector, &[false; N], route) {
                    Ok((x, report)) => {
                        if report.route == GuardRouteTaken::Content {
                            outcome.content_routes += 1;
                        }
                        writeln!(qlog, "{{\"type\":\"guard\",\"rule\":{},\"report\":{}}}", rules.len(), q(&format!("{report:?}"))).map_err(|e| e.to_string())?;
                        x
                    }
                    Err(e) => {
                        error = Some(format!("exception extraction on {current:?}: {e:?}"));
                        break;
                    }
                }
            } else {
                let r = forked(out_dir, &format!("guard_r{}", rules.len()), secs, || match extract_exceptions(&candidate, &idx, &sector) {
                    Ok(_) => "ok".to_string(),
                    Err(e) => format!("err {e:?}"),
                });
                if !matches!(r, Forked::Done(..)) {
                    writeln!(qlog, "{{\"type\":\"guard_timeout\",\"rule\":{},\"case\":{},\"rhs\":{},\"timeout\":{secs}}}", rules.len(), case_json(&current, n), candidate.rhs.len()).map_err(|e| e.to_string())?;
                    stop = Some(StopReason::GuardTimeout { case: spec.clone(), rhs: candidate.rhs.len() });
                    break;
                }
                match extract_exceptions(&candidate, &idx, &sector) {
                    Ok(x) => x,
                    Err(e) => {
                        error = Some(format!("exception extraction on {current:?}: {e:?}"));
                        break;
                    }
                }
            }
        } else {
            match extract_exceptions(&candidate, &idx, &sector) {
                Ok(x) => x,
                Err(e) => {
                    error = Some(format!("exception extraction on {current:?}: {e:?}"));
                    break;
                }
            }
        };
        let rule = SectorRule { candidate, exceptions, dispatch_policy: Default::default() };
        let index = rules.len();
        let extras = rule_extras(&solver, &rule, searched.raw_pivot, searched.lift_seconds, searched.source_order.clone(), false);
        if outcome.rule_scores.len() < 64 {
            let score = RuleScore {
                trace_rows: extras.trace_rows,
                lift_seconds: extras.lift_seconds,
                search_seconds: rule.candidate.stats.elapsed.as_secs_f64(),
                rhs_terms: rule.candidate.rhs.len(),
                max_degree: rule_degree(&rule.candidate),
                direct_hit: rule.candidate.stats.direct_hit,
                branches: rule.exceptions.branches.len(),
            };
            if outcome.first_rule.is_none() {
                outcome.first_rule = Some(score.clone());
            }
            outcome.rule_scores.push(score);
        }
        terms += write_rule(&mut out, &label, index, &rule, &extras, n);
        out.flush().map_err(|e| e.to_string())?;
        branches_total += rule.exceptions.branches.len();
        if opts.cand_dump {
            write_cand_dump(out_dir, index, &rule.candidate, n)?;
        }
        let branch_strings: Vec<String> = rule.exceptions.branches.iter().map(|b| strings(b.iter())).collect();
        writeln!(
            qlog,
            "{{\"type\":\"rule\",\"index\":{index},\"case\":{},\"rhs\":{},\"sources\":{},\"trace_rows\":{},\"lift_seconds\":{:.3},\"search_seconds\":{:.3},\"exception_seconds\":{:.3},\"source_order\":{},\"branches\":[{}]}}",
            case_json(&rule.candidate.case, n),
            rule.candidate.rhs.len(),
            rule.candidate.sources.len(),
            extras.trace_rows,
            extras.lift_seconds,
            rule.candidate.stats.elapsed.as_secs_f64(),
            te.elapsed().as_secs_f64(),
            q(&extras.source_order),
            branch_strings.join(",")
        )
        .map_err(|e| e.to_string())?;
        events(&NodeEvent::RuleFound { index, rhs: rule.candidate.rhs.len(), trace_rows: extras.trace_rows, lift_seconds: extras.lift_seconds, branches: rule.exceptions.branches.len() });
        // exceptional children
        let mut children: Vec<Case<N>> = Vec::new();
        for (bi, branch) in rule.exceptions.branches.iter().enumerate() {
            let nonlinear = branch.iter().any(|p| max_total_degree(p) >= 2);
            if let Some(secs) = opts.admit_try.filter(|_| opts.gsplit && nonlinear) {
                out.flush().map_err(|e| e.to_string())?;
                qlog.flush().map_err(|e| e.to_string())?;
                let case_ref = &rule.candidate.case;
                let r = forked(out_dir, &format!("admit_r{index}_b{bi}"), secs, || {
                    match case_ref.intersect_many_refined(branch, &idx, &sector, limits, None, UnsupportedBranchPolicy::Defer, CaseRefinement::all()) {
                        Ok(_) => "ok".to_string(),
                        Err(e) => format!("err {e:?}"),
                    }
                });
                if !matches!(r, Forked::Done(..)) {
                    deferred_regions += 1;
                    writeln!(qlog, "{{\"type\":\"deferred_region\",\"rule\":{index},\"branch\":{bi},\"reason\":\"AdmissionTimeout({secs} s)\",\"chart\":{},\"conjunction\":{}}}", case_json(&rule.candidate.case, n), strings(branch.iter())).map_err(|e| e.to_string())?;
                    continue;
                }
            }
            let admitted = if opts.gsplit {
                rule.candidate.case.intersect_many_refined(branch, &idx, &sector, limits, None, UnsupportedBranchPolicy::Defer, CaseRefinement::all())
            } else {
                rule.candidate.case.intersect_many(branch, &idx, &sector, limits)
            };
            match admitted {
                Ok(r) => {
                    for (di, d) in r.deferred.iter().enumerate() {
                        deferred_regions += 1;
                        writeln!(qlog, "{{\"type\":\"deferred_region\",\"rule\":{index},\"branch\":{bi},\"k\":{di},\"reason\":{},\"chart\":{},\"conjunction\":{}}}", q(&format!("{:?}", d.reason)), case_json(&d.chart, n), strings(d.conjunction.iter())).map_err(|e| e.to_string())?;
                    }
                    if r.cases.is_empty() && r.deferred.is_empty() {
                        discarded += 1;
                    }
                    for ch in &r.cases {
                        writeln!(qlog, "{{\"type\":\"child\",\"rule\":{index},\"branch\":{bi},\"case\":{}}}", case_json(ch, n)).map_err(|e| e.to_string())?;
                    }
                    children.extend(r.cases);
                }
                Err(e) => {
                    deferred_branches += 1;
                    writeln!(qlog, "{{\"type\":\"deferred_branch\",\"rule\":{index},\"branch\":{bi},\"polys\":{},\"error\":{}}}", strings(branch.iter()), q(&format!("{e:?}").chars().take(4000).collect::<String>())).map_err(|e| e.to_string())?;
                }
            }
        }
        if children.iter().any(|ch| ch == &current) {
            error = Some(format!("NonProgress on {current:?}"));
            rules.push(rule);
            break;
        }
        for ch in children {
            enqueue(ch, Some(index), &mut pending, &mut numerical, &rules, &cover, &idx, &sector, limits, &mut qlog, &mut discarded, n);
        }
        qlog.flush().map_err(|e| e.to_string())?;
        rules.push(rule);
    }
    *WATCH.lock().unwrap() = None;
    // ---- numeric endpoints: one by one under the node's source order and exact backend; exhausted ones in bulk
    let mut residual_keys: Vec<String> = Vec::new();
    if error.is_none() && stop.is_none() && !numerical.is_empty() {
        outcome.numerical_cases = numerical.len();
        let options = SearchOptions { max_depth: Some(opts.numeric_depth), ..symbolic };
        let mut exhausted: Vec<CoordinateCase<N>> = Vec::new();
        for point in numerical.clone() {
            let case = Case::from(point);
            let fixed: Vec<i64> = point.fixed()[..n].iter().map(|v| v.map_or(0, i64::from)).collect();
            // a numeric case may already be covered by a rule found later than its enqueueing
            if let Some(k) = rules.iter().position(|r| covers(&r.candidate.case, &r.exceptions.branches, &case, &idx, &sector, limits)) {
                writeln!(qlog, "{{\"type\":\"numerical_covered\",\"by_rule\":{k},\"fixed\":{fixed:?}}}").map_err(|e| e.to_string())?;
                continue;
            }
            let (result, reports) = search(&case, &node_source, options);
            for r in &reports {
                outcome.screened_lifts += 1;
                writeln!(qlog, "{{\"type\":\"lift_screen\",\"numerical\":true,\"report\":{}}}", serde_json::to_string(r).unwrap()).map_err(|e| e.to_string())?;
            }
            match result {
                Ok(searched) => {
                    let candidate = searched.candidate;
                    let nl: Vec<String> = candidate.rhs.iter().filter(|t| solver.ordering().compare(&candidate.target, &t.integral) != Ordering::Less).map(|t| t.integral.to_string()).collect();
                    outcome.not_lower.extend(nl);
                    let exceptions = extract_exceptions(&candidate, &idx, &sector).map_err(|e| format!("numerical exceptions: {e:?}"))?;
                    if !exceptions.branches.is_empty() {
                        error = Some("a fully fixed rule has index exceptions".into());
                        break;
                    }
                    let rule = SectorRule { candidate, exceptions, dispatch_policy: Default::default() };
                    let index = rules.len();
                    let extras = rule_extras(&solver, &rule, searched.raw_pivot, searched.lift_seconds, searched.source_order, true);
                    terms += write_rule(&mut out, &label, index, &rule, &extras, n);
                    out.flush().map_err(|e| e.to_string())?;
                    if opts.cand_dump {
                        write_cand_dump(out_dir, index, &rule.candidate, n)?;
                    }
                    writeln!(qlog, "{{\"type\":\"rule\",\"index\":{index},\"numerical\":true,\"fixed\":{fixed:?},\"rhs\":{},\"trace_rows\":{},\"lift_seconds\":{:.3},\"source_order\":{}}}", rule.candidate.rhs.len(), extras.trace_rows, extras.lift_seconds, q(&extras.source_order)).map_err(|e| e.to_string())?;
                    events(&NodeEvent::NumericalPoint { fixed, outcome: format!("rule {index}") });
                    rules.push(rule);
                }
                Err(SolverError::SearchExhausted { .. }) => {
                    events(&NodeEvent::NumericalPoint { fixed: fixed.clone(), outcome: "exhausted; shared solve".into() });
                    writeln!(qlog, "{{\"type\":\"numerical_exhausted\",\"fixed\":{fixed:?}}}").map_err(|e| e.to_string())?;
                    exhausted.push(point);
                }
                Err(e) => {
                    error = Some(format!("numerical point {fixed:?}: {e:?}"));
                    break;
                }
            }
        }
        if error.is_none() && !exhausted.is_empty() {
            outcome.numeric_bulk_fallback = exhausted.len();
            match solver.solve_numeric_cases(exhausted, options) {
                Ok(result) => {
                    for candidate in result.rules {
                        let nl: Vec<String> = candidate.rhs.iter().filter(|t| solver.ordering().compare(&candidate.target, &t.integral) != Ordering::Less).map(|t| t.integral.to_string()).collect();
                        outcome.not_lower.extend(nl);
                        let exceptions = extract_exceptions(&candidate, &idx, &sector).map_err(|e| format!("numerical exceptions: {e:?}"))?;
                        let rule = SectorRule { candidate, exceptions, dispatch_policy: Default::default() };
                        let index = rules.len();
                        let extras = rule_extras(&solver, &rule, None, 0.0, "shared".into(), true);
                        terms += write_rule(&mut out, &label, index, &rule, &extras, n);
                        rules.push(rule);
                    }
                    for r in &result.residuals {
                        residual_keys.push(r.integral().to_string());
                    }
                }
                Err(e) => error = Some(format!("numerical (shared): {e:?}")),
            }
        }
    }
    for key in &residual_keys {
        writeln!(out, "{{\"type\":\"residual\",\"chart\":{},\"key\":{}}}", q(&label), q(key)).map_err(|e| e.to_string())?;
    }
    let seconds = t2.elapsed().as_secs_f64();
    if let Some(e) = &error {
        writeln!(out, "{{\"type\":\"chart_error\",\"chart\":{},\"seconds\":{seconds:.3},\"error\":{}}}", q(&label), q(&e.chars().take(4000).collect::<String>())).map_err(|e| e.to_string())?;
    }
    if let Some(s) = &stop {
        let reason = serde_json::to_string(s).unwrap_or_default();
        writeln!(out, "{{\"type\":\"chart_error\",\"chart\":{},\"seconds\":{seconds:.3},\"error\":{},\"stop\":{reason}}}", q(&label), q("node stopped (see stop)")).map_err(|e| e.to_string())?;
        events(&NodeEvent::Stopped { reason });
    }
    writeln!(
        out,
        "{{\"type\":\"chart_end\",\"chart\":{},\"seconds\":{seconds:.3},\"rules\":{},\"rhs_terms\":{terms},\"branches\":{branches_total},\"residuals\":{},\"symbolic_cases\":{symbolic_cases},\"numerical_cases\":{},\"discarded_cases\":{discarded},\"finite_case_policy\":\"node-queue\",\"deferred_cases\":0,\"deferred_branches\":{deferred_branches},\"deferred_regions\":{deferred_regions}}}",
        q(&label),
        rules.len(),
        residual_keys.len(),
        outcome.numerical_cases
    )
    .map_err(|e| e.to_string())?;
    let chart_errors = usize::from(error.is_some() || stop.is_some());
    writeln!(
        out,
        "{{\"type\":\"end\",\"charts\":1,\"rules\":{},\"rhs_terms\":{terms},\"residuals\":{},\"chart_errors\":{chart_errors},\"seconds\":{:.3},\"vmhwm_kb\":{},\"harness\":\"rustred node driver (rr-consolidate)\",\"native_descent_not_lower\":{}}}",
        rules.len(),
        residual_keys.len(),
        t_total.elapsed().as_secs_f64(),
        vmhwm_kb(),
        strings(outcome.not_lower.iter())
    )
    .map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    writeln!(cert, "{{\"type\":\"cert_end\",\"ok\":0,\"failed\":0,\"skipped\":{},\"weights\":0,\"rules_with_weight_poles\":0,\"seconds\":{:.3},\"vmhwm_kb\":{}}}", rules.len(), t_total.elapsed().as_secs_f64(), vmhwm_kb()).map_err(|e| e.to_string())?;
    cert.flush().map_err(|e| e.to_string())?;
    writeln!(qlog, "{{\"type\":\"end\",\"error\":{},\"rules\":{},\"residuals\":{},\"seconds\":{seconds:.3}}}", error.as_deref().map_or("null".to_string(), q), rules.len(), residual_keys.len()).map_err(|e| e.to_string())?;
    qlog.flush().map_err(|e| e.to_string())?;
    if let Some(e) = error {
        stop = Some(StopReason::Error(e));
    }
    outcome.complete = stop.is_none() && outcome.not_lower.is_empty();
    outcome.rules = rules.len();
    outcome.rhs_terms = terms;
    outcome.residuals = residual_keys.len();
    outcome.residual_keys = residual_keys;
    outcome.deferred_regions = deferred_regions;
    outcome.deferred_branches = deferred_branches;
    outcome.symbolic_cases = symbolic_cases;
    outcome.stop = stop;
    outcome.seconds = t_total.elapsed().as_secs_f64();
    outcome.vmhwm_kb = vmhwm_kb();
    Ok(outcome)
}

fn rule_extras<const N: usize>(solver: &SectorSolver<N>, rule: &SectorRule<N>, raw: Option<Integral<N>>, lift_seconds: f64, source_order: String, numerical: bool) -> RuleExtras {
    let c = &rule.candidate;
    let n = solver.ordering().physical_arity();
    let (raw_pivot, recenter) = if c.case.is_numerical() {
        (None, Some(vec![0i16; n]))
    } else if let Some(p) = raw {
        // canonicalize: target = raw shifted by -raw offsets on symbolic axes; recenter = target - raw offset
        let rec: Vec<i16> = (0..n).map(|i| if p.powers()[i].is_symbolic() { -p.powers()[i].value() } else { 0 }).collect();
        (Some(p.powers()[..n].iter().map(|x| power_byte(*x)).collect()), Some(rec))
    } else if c.stats.direct_hit {
        // a direct hit has no ExactStarted event: the recenter of its one-row frame is exact and cheap
        match solver.rule_frame(rule) {
            Ok(frame) => {
                let rec: Vec<i16> = frame.recenter[..n].to_vec();
                let raw: Vec<u8> = (0..n)
                    .map(|i| {
                        let t = c.target.powers()[i];
                        if t.is_symbolic() { power_byte(crate::solver::Power::new(true, t.value() - rec[i]).unwrap_or(t)) } else { power_byte(t) }
                    })
                    .collect();
                (Some(raw), Some(rec))
            }
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };
    RuleExtras { raw_pivot, recenter, trace_rows: c.stats.exact_trace_rows, lift_seconds, source_order, numerical }
}

fn write_cand_dump<const N: usize>(dir: &Path, index: usize, c: &RuleCandidate<N>, n: usize) -> Result<(), String> {
    let pivots: Vec<String> = c.sources.iter().map(|s| format!("{{\"basis_row\":{},\"seed\":{}}}", s.basis_row, q(&format!("{:?}", s.seed)))).collect();
    let spec = case_spec(&c.case, n);
    let body = format!(
        "{{\"schema\":\"xstuck.candidate_dump.v1\",\"producer\":\"rustred node driver\",\"rule_index\":{index},\"case\":{},\"case_fixed\":{},\"target\":{},\"rhs_count\":{},\"pivots\":[{}]}}\n",
        serde_json::to_string(&spec).unwrap(),
        serde_json::to_string(&spec.fixed).unwrap(),
        q(&c.target.to_string()),
        c.rhs.len(),
        pivots.join(",")
    );
    std::fs::write(dir.join(format!("F.cand_dump.r{index}.json")), body).map_err(|e| format!("cand dump: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::order::NodeOrder;
    use crate::node::screen::{HeavyAction, LiftScreen};

    pub(crate) const SUNSET: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2-q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rr-node-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn quiet() -> NodeOptions {
        NodeOptions { native_try: None, admit_try: None, ..NodeOptions::default() }
    }

    #[test]
    fn sunset_chart_closes_records_order_and_replays() {
        for order in [NodeOrder::identity(), NodeOrder { name: "tie_isp_rev".into(), coordinate_priority: Some(vec![0, 1, 2, 4, 3]), ..NodeOrder::identity() }] {
            let dir = tmp(&format!("sunset-{}", order.name));
            let opts = NodeOptions { order: order.clone(), ..quiet() };
            let out = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |_| {}).unwrap();
            assert!(out.complete, "{:?}", out.stop);
            assert!(out.rules > 0);
            assert_eq!(out.residuals, 1, "the massless sunset has one master in its top sector");
            // the order is part of the artefact
            let header: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&out.table).unwrap().lines().next().unwrap()).unwrap();
            assert_eq!(crate::node::descent::header_order(&header).unwrap(), order);
            let (_, rep) = crate::node::descent::check_table_file(&out.table, None).unwrap();
            assert!(rep.passed() && rep.rules == out.rules);
            // independent replay of the whole table: every symbolic rule carries its raw pivot authority
            let args = crate::node::replay::ReplayArgs {
                family: None,
                table: PathBuf::from(&out.table),
                cert: Some(dir.join("F.cert.jsonl")),
                cand_dump_dir: None,
                bindings: None,
                rules: None,
                chart: None,
                out: dir.join("replay"),
                dimensions: crate::node::replay::DIMS.iter().map(|s| s.to_string()).collect(),
                sample_attempts: 128,
            };
            let (status, doc) = crate::node::replay::run(&args).unwrap();
            assert_eq!(status, "PASS_RS2_RS3_SAMPLED", "{}", serde_json::to_string(&doc["rules"]).unwrap().chars().take(2000).collect::<String>());
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn replay_rejects_a_mutated_table_coefficient() {
        let dir = tmp("mutant");
        let out = generate(SUNSET, "sunset.sym", &dir, &quiet(), &mut |_| {}).unwrap();
        let text = std::fs::read_to_string(&out.table).unwrap();
        // corrupt the first coefficient of the first symbolic rule with at least one RHS term: c -> (c)+1
        let mut done = false;
        let lines: Vec<String> = text
            .lines()
            .map(|l| {
                if done || !l.starts_with("{\"type\":\"rule\"") {
                    return l.to_string();
                }
                let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
                if v["rhs"].as_array().is_none_or(|a| a.is_empty()) {
                    return l.to_string();
                }
                let c = v["rhs"][0][1].as_str().unwrap().to_string();
                v["rhs"][0][1] = serde_json::json!(format!("({c})+1"));
                done = true;
                serde_json::to_string(&v).unwrap()
            })
            .collect();
        assert!(done);
        let bad = dir.join("F.bad.table.jsonl");
        std::fs::write(&bad, lines.join("\n") + "\n").unwrap();
        let args = crate::node::replay::ReplayArgs {
            family: None,
            table: bad,
            cert: None,
            cand_dump_dir: None,
            bindings: None,
            rules: None,
            chart: None,
            out: dir.join("replay-bad"),
            dimensions: crate::node::replay::DIMS.iter().map(|s| s.to_string()).collect(),
            sample_attempts: 128,
        };
        let (status, _) = crate::node::replay::run(&args).unwrap();
        assert_eq!(status, "FAIL");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn screen_refusal_stops_with_the_case_or_reports_nothing() {
        let dir = tmp("screen");
        let screen = LiftScreen { min_rows: 1, max_total_degree: 0, max_rhs_terms: 0, on_heavy: HeavyAction::Refuse, ..LiftScreen::default() };
        let opts = NodeOptions { screen: Some(screen), ..quiet() };
        let out = generate(SUNSET, "sunset.sym", &dir, &opts, &mut |_| {}).unwrap();
        if out.screened_lifts > 0 {
            assert!(out.refused_lifts > 0);
            assert!(matches!(out.stop, Some(StopReason::LiftRefused { .. })), "{:?}", out.stop);
            assert!(out.source_order_retries > 0, "a refused lift is retried with another source order first");
        } else {
            assert!(out.complete);
        }
        // a permissive screen changes nothing
        let dir2 = tmp("screen-ok");
        let opts = NodeOptions { screen: Some(LiftScreen { min_rows: 1, ..LiftScreen::default() }), ..quiet() };
        let ok = generate(SUNSET, "sunset.sym", &dir2, &opts, &mut |_| {}).unwrap();
        assert!(ok.complete);
        let plain = generate(SUNSET, "sunset.sym", &tmp("plain"), &quiet(), &mut |_| {}).unwrap();
        assert_eq!((ok.rules, ok.residuals), (plain.rules, plain.residuals));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[test]
    fn presented_family_is_solved_and_case_specs_parse() {
        let dir = tmp("perm");
        let order = NodeOrder { name: "isp_rev_all".into(), slot_permutation: Some(vec![0, 1, 2, 4, 3]), ..NodeOrder::identity() };
        let out = generate(SUNSET, "sunset.sym", &dir, &NodeOptions { order, ..quiet() }, &mut |_| {}).unwrap();
        assert!(out.complete);
        let presented = std::fs::read_to_string(dir.join("F.family.sym")).unwrap();
        assert!(presented.contains("prop(D4,(k2+q)^2,0)"));
        assert_eq!(CaseSpec::parse("1,_,0", "n1-2").unwrap(), CaseSpec { fixed: vec![Some(1), None, Some(0)], equations: vec!["n1-2".into()] });
        let _ = std::fs::remove_dir_all(&dir);
    }
}
