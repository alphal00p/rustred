use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Arc;
use std::time::Instant;

use rustred::family::IntegralFamily;
use rustred::input::{Compiler, Limits, LoweringLimits};
use rustred::order::{CompiledOrder, DegreeRow, OrderDescriptor};
use rustred::sector::{Mask, zero};
use rustred::solver::{
    Case, Integral, SectorConfig, SectorExecutor, SectorRule, SectorSolveOptions, SectorSolver, SourceSystem,
};

/// Bump on any change of the record layout (FORMAT.md).
const SCHEMA: &str = "table_dump.v1";
/// The worktree base: rustred 94ec3300 plus the example and solver/table_export.rs.
const SOURCE_BASE: &str = "94ec3300";

fn q(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn strings<T: std::fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    let parts: Vec<String> = items.into_iter().map(|x| q(&x.to_string())).collect();
    format!("[{}]", parts.join(","))
}

fn mask_string(bits: &[bool]) -> String {
    bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

fn degree_row(row: &DegreeRow) -> String {
    format!("{{\"active\":{:?},\"inactive\":{:?}}}", row.active, row.inactive)
}

fn descriptor_json(d: &OrderDescriptor) -> String {
    let rows = |r: &[DegreeRow]| r.iter().map(degree_row).collect::<Vec<_>>().join(",");
    format!(
        "{{\"pre_support_degree_rows\":[{}],\"support_weights\":{:?},\"support_priority\":{:?},\"degree_rows\":[{}],\"coordinate_priority\":{:?},\"coordinate_groups\":{},\"active_direction\":{},\"inactive_direction\":{},\"two_sided\":{:?}}}",
        rows(&d.pre_support_degree_rows),
        d.support_weights,
        d.support_priority,
        rows(&d.degree_rows),
        d.coordinate_priority,
        q(&format!("{:?}", d.coordinate_groups)),
        q(&format!("{:?}", d.active_direction)),
        q(&format!("{:?}", d.inactive_direction)),
        d.two_sided
    )
}

fn vmhwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM"))
                .and_then(|l| l.split_whitespace().nth(1).map(|v| v.parse().unwrap_or(0)))
        })
        .unwrap_or(0)
}

fn order_for<const N: usize>(name: &str, axes: &[bool; N], n: usize) -> (OrderDescriptor, Option<CompiledOrder>, String) {
    let any = axes.iter().any(|&a| a);
    let resolved = match name {
        "auto" if any => "two_sided",
        "auto" => "legacy",
        other => other,
    };
    let eps_row = DegreeRow { active: axes[..n].iter().map(|&a| u64::from(a)).collect(), inactive: vec![0; n] };
    let descriptor = match resolved {
        "legacy" | "one_sided" => OrderDescriptor::spired_uncut(n),
        "two_sided" => OrderDescriptor::spired_uncut_two_sided(axes[..n].to_vec()),
        "eps_first" | "eps_third" => {
            let mut d = OrderDescriptor::spired_uncut(n);
            d.two_sided = axes[..n].to_vec();
            d.degree_rows.insert(if resolved == "eps_first" { 0 } else { 2 }, eps_row);
            d
        }
        "plain_two_sided" => {
            let mut d = OrderDescriptor::spired_uncut(n);
            d.two_sided = axes[..n].to_vec();
            d
        }
        other => panic!("unknown order {other}"),
    };
    let program = if resolved == "legacy" || resolved == "one_sided" {
        None
    } else {
        Some(CompiledOrder::compile(descriptor.clone(), Default::default()).expect("order"))
    };
    (descriptor, program, resolved.to_string())
}

fn write_rule<const N: usize>(f: &mut impl std::io::Write, chart: &str, index: usize, rule: &SectorRule<N>) -> usize {
    let c = &rule.candidate;
    let rhs: Vec<String> = c.rhs.iter().map(|t| format!("[{},{}]", q(&t.integral.to_string()), q(&t.coefficient.to_string()))).collect();
    let exceptions: Vec<String> = rule.exceptions.branches.iter().map(|b| strings(b.iter())).collect();
    let fixed: Vec<String> = c.case.fixed().iter().map(|v| v.map_or("null".to_string(), |x| x.to_string())).collect();
    let equations = match &c.case {
        Case::Coordinate(_) => "[]".to_string(),
        Case::Affine(a) => strings(a.equations().iter()),
    };
    let mut seeds: Vec<String> = c.sources.iter().map(|s| s.seed.integral.to_string()).collect();
    seeds.sort();
    seeds.dedup();
    writeln!(
        f,
        "{{\"type\":\"rule\",\"chart\":{},\"index\":{},\"case\":{},\"fixed\":[{}],\"equations\":{},\"target\":{},\"rhs\":[{}],\"exceptions\":[{}],\"dispatch\":{},\"nsources\":{},\"seeds\":{},\"direct_hit\":{},\"seconds_search\":{:.6}}}",
        q(chart),
        index,
        q(&format!("{:?}", c.case)),
        fixed.join(","),
        equations,
        q(&c.target.to_string()),
        rhs.join(","),
        exceptions.join(","),
        q(&format!("{:?}", rule.dispatch_policy)),
        c.sources.len(),
        strings(seeds.iter()),
        c.stats.direct_hit,
        c.stats.elapsed.as_secs_f64()
    )
    .unwrap();
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
    .unwrap();
    let contraction: Vec<usize> = (loops..loops + externals).rev().chain(0..loops).collect();
    for (m, row) in sources.rows().iter().enumerate() {
        // SourceSystem order: differentiated-loop major, reversed externals then loops. Momentum j: k_{j+1}, j >= loops: the external.
        let (loop_id, momentum) = (m / contraction.len(), contraction[m % contraction.len()]);
        let terms: Vec<String> = row.iter().map(|t| format!("[{},{}]", q(&t.integral.to_string()), q(&t.coefficient.to_string()))).collect();
        writeln!(
            f,
            "{{\"type\":\"source_row\",\"m\":{m},\"differentiated_loop\":{loop_id},\"contraction_momentum\":{momentum},\"terms\":[{}]}}",
            terms.join(",")
        )
        .unwrap();
    }
}

#[derive(Default)]
struct CertTotals {
    ok: usize,
    failed: usize,
    skipped: usize,
    seconds: f64,
    weights: usize,
    rules_with_weight_poles: usize,
}

fn write_chart_cert<const N: usize>(
    f: &mut impl std::io::Write,
    sources: &SourceSystem<N>,
    chart: [bool; N],
    label: &str,
    config: &SectorConfig<N>,
    rules: &[SectorRule<N>],
    max_rows: usize,
    totals: &mut CertTotals,
) {
    let t0 = Instant::now();
    let (solver, derivation) = match SectorSolver::with_basis_derivation(sources, chart, config.clone()) {
        Ok(x) => x,
        Err(e) => {
            writeln!(f, "{{\"type\":\"cert_chart_error\",\"chart\":{},\"error\":{}}}", q(label), q(&format!("{e:?}"))).unwrap();
            totals.failed += rules.len();
            return;
        }
    };
    for (k, row) in derivation.rows.iter().enumerate() {
        let entries: Vec<String> = row.iter().map(|(m, p)| format!("[{m},{}]", q(&p.to_string()))).collect();
        writeln!(f, "{{\"type\":\"basis_row\",\"chart\":{},\"k\":{k},\"sources\":[{}]}}", q(label), entries.join(",")).unwrap();
    }
    f.flush().unwrap();
    for (index, rule) in rules.iter().enumerate() {
        let rows = rule.candidate.sources.len();
        if rows > max_rows {
            writeln!(f, "{{\"type\":\"cert_skip\",\"chart\":{},\"rule\":{index},\"trace_rows\":{rows}}}", q(label)).unwrap();
            totals.skipped += 1;
            continue;
        }
        let t1 = Instant::now();
        match solver.rule_frame(rule) {
            Ok(frame) => {
                let entries: Vec<String> = frame
                    .entries
                    .iter()
                    .map(|e| format!("[{},{},{}]", e.basis_row, q(&e.at.to_string()), q(&e.weight.to_string())))
                    .collect();
                let poles: Vec<String> = frame.weight_poles.iter().map(|b| strings(b.iter())).collect();
                writeln!(
                    f,
                    "{{\"type\":\"cert_rule\",\"chart\":{},\"rule\":{index},\"via\":{},\"recenter\":{:?},\"trace_rows\":{rows},\"zero_weights\":{},\"seconds\":{:.6},\"weight_poles\":[{}],\"frame\":[{}]}}",
                    q(label),
                    q(frame.via),
                    frame.recenter,
                    frame.zero_weights,
                    t1.elapsed().as_secs_f64(),
                    poles.join(","),
                    entries.join(",")
                )
                .unwrap();
                totals.ok += 1;
                totals.weights += frame.entries.len();
                totals.rules_with_weight_poles += usize::from(!frame.weight_poles.is_empty());
            }
            Err(e) => {
                writeln!(f, "{{\"type\":\"cert_error\",\"chart\":{},\"rule\":{index},\"error\":{}}}", q(label), q(&format!("{e:?}").chars().take(800).collect::<String>())).unwrap();
                totals.failed += 1;
            }
        }
        f.flush().unwrap();
    }
    totals.seconds += t0.elapsed().as_secs_f64();
}

// ---- face_queue_xs harness (round 8 / xstuck): face_queue_x (below) + config keys (all optional, off by default):
//   cand_dump on         right after the search (native strict-descent check passed) and BEFORE GuardExtraction, write
//                        OUT.cand_dump.r<rule>.json (case, target, coefficient variables, pivots = candidate.sources, search
//                        stats, RHS integrals with denominator ids, the distinct coefficient denominators, the exact
//                        factorisation inputs of extract_exceptions, the activation branches) and OUT.cand_terms.r<rule>.jsonl
//                        (every RHS term: integral, numerator, denominator).
//   native_try S         run the engine's extract_exceptions in a FORKED child (own PID, killed after S s). If it returns,
//                        the parent re-runs it in-process (known to finish) and the rule uses the NATIVE exceptions.
//   fallback content|exit   on a native_try timeout: `content` = the exact content route (xs_content_exceptions, same zero
//                        set, proof in its doc comment), itself first tried in a forked child (timeout content_timeout S,
//                        default 3600); `exit` = write exception_timeout + chart_error/end and exit 7 (as face_queue_x).
//   guard_profile S      on a native_try timeout: factor every guard input separately in a forked child (S s each,
//                        smallest first; stop after guard_profile_max timeouts, default 2) -> OUT.guard_profile.jsonl.
//   cert_timeout S       the certificate (rule_frame) of each rule runs in a forked child, killed after S s (cert_timeout record).
// ---- face_queue_x harness (round 8 / wholechart): face_queue_pp + two config keys (both optional, off by default):
//   exc_timeout S        extract_exceptions runs on a helper thread; if it has not returned after S seconds the harness writes an
//                        exception_timeout record (queue log) and the chart_error/end records, then EXITS (code 7). A stuck exact
//                        factorisation (Symbolica, no cancellation point) cannot be interrupted any other way.
//   dump_candidate on    before exception extraction, every RHS term of every symbolic candidate is written to OUT.cand.jsonl
//                        (rule index, case, target, integral, numerator, denominator), plus a per-candidate denominator summary.
// Without the keys the binary behaves as face_queue_pp (and so as face_queue_gs without probe_point).
// ---- face_queue_pp harness (round 6 / native-S1c): face_queue_gs + config key `probe_point SPEC` (SPEC = n0=V,...,n19=V,d=V,
// decimal residues mod 2^61-1). For the INITIAL cases only, the harness sets RUSTRED_PROBE_POINT (rr-probept rlib: the modular
// discovery point of Probe::new) to SPEC ordered by the coefficient variables; every other case uses the engine's random point.
// Without the key the binary behaves as face_queue_gs.
// ---- face_queue_gs harness (round 4 / U1-native): round-2 face_queue re-built against the rr-gsplit rlib (B8 source 8ea1796e)
// with the B8 table_dump semantics --defer-unsupported --case-gsplit for exceptional-case admission:
//   children = case.intersect_many_refined(branch, .., UnsupportedBranchPolicy::Defer, CaseRefinement::all()); every
//   DeferredRegion is written to OUT.queue.jsonl as deferred_region (chart, conjunction, exclusions, dependents, checks).
//   Coverage pruning (rule_covers) keeps the plain intersection, as the engine (SectorSolver::rule_covers).
// Extra CONFIG keys (all optional; defaults = the round-2 behaviour except gsplit/defer which default ON):
//   gsplit on|off                     refined + deferring admission (default on); off = plain intersect_many
//   max_depth K                       symbolic SearchOptions.max_depth (seed-ball radius cap; default None)
//   source_order input|reverse|rot:K|shuffle:S   basis visit order per seed (solve_case_with_source_order_and_observer)
// ---- below: round 2 header ----
// ---- face_queue harness (round 2): the RustRed sector queue started from nominated face cases of the p1 top sector ----
// Usage: face_queue FAMILY OUT.table.jsonl OUT.cert.jsonl CONFIG
// CONFIG lines (# comments):
//   initial SPEC [EQ;EQ;...]          a nominated case (SPEC = 20 comma separated entries, '_' free, integer fixed;
//                                     EQ = integer-linear equation in n0..n19 equal to zero, e.g. n4-n6-1)
//   exclude LABEL SPEC [EQ;...]       an owned elsewhere domain: a child case CONTAINED in it (Case::contains, exact)
//                                     is not solved; it is written to OUT.queue.jsonl as deferred_case with LABEL
//   cert on|off                       rule_frame certificate of each rule right after it is found (default on)
//   max_cases K                       stop (error record) after K symbolic cases (default unbounded)
//   defer_nonlinear on|off            an AND branch whose every polynomial is nonlinear is not intersected: it is
//                                     written as deferred_branch with its exact polynomials (default off)
// Everything else is the round-1b starface_domains harness (table_dump.rs helpers verbatim): same family compile,
// sources, zero sectors (downset of the top mask), SectorConfig and legacy order (SpiredUncutV1, integral_order None)
// as `table_dump T167p1.sym top auto`. The queue below is SectorSolver::solve_case_queue_inline (FirstValid policy,
// no capacity-dispatch, as in the rr-case-lift-b3 rlib) re-driven through public API:
//   pending sorted by Case::queue_cmp; per case solve_case_with_observer (default symbolic SearchOptions) -> the
//   finish_rule checks (strict descent of every RHS term in the solver's ordering, two-sided consistency, exception
//   extraction) -> exceptional children = candidate.case.intersect_many(branch) for each OR branch (exact) ->
//   enqueue (rule_covers pruning by earlier rules, prune_subsumed, numerical cases to the shared depth-2 batch).
// Differences, all explicit in OUT.queue.jsonl: (1) exclusions; (2) an intersect_many error (unsupported, e.g.
// nonlinear, geometry) does not abort: the branch is written as deferred_branch with the exact polynomials and error;
// (3) rules and certificates are streamed as found.
use std::cmp::Ordering;
use rustred::algebra::CoefficientPolynomial;
use rustred::solver::{CaseIntersectionLimits, CaseRefinement, CoordinateCase, SearchOptions, UnsupportedBranchPolicy, extract_exceptions};
use rustred::solver::{ExceptionalConditions, RuleCandidate};
use symbolica::domains::InternalOrdering;
use symbolica::prelude::{Factorize, Integer};

// ======================= round 8 / xstuck additions (face_queue_xs) =======================
// fork-isolated computations: a stuck exact factorisation is killed (own child PID) instead of burning the CPU forever.
unsafe extern "C" {
    fn fork() -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
    fn _exit(code: i32) -> !;
}

// [p5l-repair r9] the live forked child (killed by the case-budget watchdog before a spill exit) and the queue snapshot
static FORK_CHILD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
struct R9Snap {
    started: Option<Instant>,
    applies: bool,
    current: String,
    pending: Vec<String>,
    numerical: Vec<String>,
    rules_done: usize,
}
static R9: std::sync::Mutex<Option<R9Snap>> = std::sync::Mutex::new(None);

/// Case-budget watchdog: when the current case has run longer than `budget` s (and the budget applies to it), write
/// SPILL_PATH = {current case, every pending case, every pending numerical point, rules written so far} and exit 9.
/// The table holds exactly the rules 0..rules_done (it is flushed after every rule); the driver re-plans the rest.
fn r9_watchdog(budget: u64, spill_path: String) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let g = R9.lock().unwrap();
        let Some(s) = g.as_ref() else { continue };
        let Some(t) = s.started else { continue };
        if !s.applies || t.elapsed().as_secs() < budget {
            continue;
        }
        let child = FORK_CHILD.load(std::sync::atomic::Ordering::SeqCst);
        if child > 0 {
            unsafe {
                kill(child, 9);
            }
        }
        let body = format!(
            "{{\"type\":\"spill\",\"budget\":{budget},\"case_seconds\":{:.3},\"rules_done\":{},\"current\":{},\"pending\":[{}],\"numerical\":[{}]}}\n",
            t.elapsed().as_secs_f64(), s.rules_done, s.current, s.pending.join(","), s.numerical.join(",")
        );
        let tmp = format!("{spill_path}.part");
        std::fs::write(&tmp, body.as_bytes()).expect("spill write");
        std::fs::rename(&tmp, &spill_path).expect("spill rename");
        eprintln!("SPILL after {:.1} s on the current case: {} pending, {} numerical, {} rules done", t.elapsed().as_secs_f64(), s.pending.len(), s.numerical.len(), s.rules_done);
        std::process::exit(9);
    });
}

enum Forked {
    Done(String, f64),
    Timeout(f64, f64),
    Failed(String, f64),
}

fn proc_cpu_seconds(pid: i32) -> f64 {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            let rest = s.rsplit_once(')')?.1.to_string();
            let f: Vec<&str> = rest.split_whitespace().collect();
            // after ')': state(0) ppid(1) ... utime = field 14 overall = index 11 here, stime index 12
            let u: f64 = f.get(11)?.parse().ok()?;
            let st: f64 = f.get(12)?.parse().ok()?;
            Some((u + st) / 100.0)
        })
        .unwrap_or(-1.0)
}

/// Run `f` in a forked child; the child writes f()'s String to a file in the cwd (the run dir) and _exits.
/// The parent polls; after `secs` it records the child's CPU seconds, SIGKILLs it (own child) and reports a timeout.
fn forked<F: FnOnce() -> String>(tag: &str, secs: u64, f: F) -> Forked {
    let path = format!("xs_fork_{}_{}.out", std::process::id(), tag);
    let _ = std::fs::remove_file(&path);
    let t = Instant::now();
    let pid = unsafe { fork() };
    if pid < 0 {
        return Forked::Failed("fork failed".into(), 0.0);
    }
    if pid > 0 {
        FORK_CHILD.store(pid, std::sync::atomic::Ordering::SeqCst);
    }
    if pid == 0 {
        let s = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| "{\"child_panic\":true}".to_string());
        let tmp = format!("{path}.part");
        let ok = std::fs::write(&tmp, s.as_bytes()).is_ok() && std::fs::rename(&tmp, &path).is_ok();
        unsafe { _exit(if ok { 0 } else { 3 }) }
    }
    loop {
        let mut status = 0i32;
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
            let cpu = proc_cpu_seconds(pid);
            unsafe {
                kill(pid, 9);
                waitpid(pid, &mut status, 0);
            }
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(format!("{path}.part"));
            FORK_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
            return Forked::Timeout(t.elapsed().as_secs_f64(), cpu);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

fn xs_contains_index<const N: usize>(p: &CoefficientPolynomial, idx: &[usize; N]) -> bool {
    if p.nvars() == 0 {
        return false;
    }
    p.exponents_iter().any(|e| idx.iter().any(|&i| e[i] != 0))
}

fn xs_primitive(p: CoefficientPolynomial) -> CoefficientPolynomial {
    let p = p.make_primitive();
    if p.lcoeff() < 0 { -p } else { p }
}

fn xs_sort_unique(v: &mut Vec<CoefficientPolynomial>) {
    v.sort_unstable_by(InternalOrdering::internal_cmp);
    v.dedup();
}

/// The engine's factorisation inputs (exception.rs `extract`): denominators of nonzero RHS coefficients that contain an
/// index variable, made primitive with positive leading coefficient, sorted and deduplicated (InternalOrdering).
fn xs_guard_inputs<const N: usize>(candidate: &RuleCandidate<N>, idx: &[usize; N]) -> Vec<CoefficientPolynomial> {
    let mut d: Vec<CoefficientPolynomial> = candidate
        .rhs
        .iter()
        .filter(|t| !t.coefficient.is_zero())
        .map(|t| &t.coefficient.denominator)
        .filter(|p| xs_contains_index(p, idx))
        .map(|p| xs_primitive(p.clone()))
        .collect();
    xs_sort_unique(&mut d);
    d
}

fn xs_poly_info(p: &CoefficientPolynomial, names: &[String]) -> String {
    let degs: Vec<String> = (0..p.nvars()).filter(|&v| p.degree(v) > 0).map(|v| format!("{}:{}", q(&names[v]), p.degree(v))).collect();
    let tdeg = p.exponents_iter().map(|e| e.iter().map(|&x| x as u32).sum::<u32>()).max().unwrap_or(0);
    let maxdigits = p.coefficients.iter().map(|c| c.to_string().trim_start_matches('-').len()).max().unwrap_or(0);
    format!("\"nterms\":{},\"total_degree\":{tdeg},\"degrees\":{{{}}},\"max_coeff_digits\":{maxdigits}", p.nterms(), degs.join(","))
}

/// Activation branches of exception.rs `extract` (inactive axis, symbolic power, positive shift), verbatim logic.
fn xs_activation_branches<const N: usize>(candidate: &RuleCandidate<N>, idx: &[usize; N], sector: &[bool; N]) -> Vec<Vec<CoefficientPolynomial>> {
    let template = &candidate.rhs[0].coefficient.numerator;
    let mut branches = Vec::new();
    for term in &candidate.rhs {
        for (axis, (&position, active)) in idx.iter().zip(sector).enumerate() {
            let power = term.integral[axis];
            if *active || !power.is_symbolic() || power.value() <= 0 {
                continue;
            }
            for boundary in 0..power.value() {
                let value = Integer::from(-(boundary as i64));
                if term.coefficient.numerator.replace(position, &value).is_zero() {
                    continue;
                }
                let variable = template.variable(&template.variables()[position]).expect("variable");
                let equation = &variable + &template.constant(Integer::from(boundary as i64));
                branches.push(vec![equation]);
            }
        }
    }
    branches
}

/// CONTENT ROUTE (exact, same zero set as the engine's extract_exceptions, legacy order without two-sided axes):
/// for each guard input P (in index variables x and parameters d): P(x, .) == 0 as a polynomial in d  <=>  every
/// d-coefficient c_k(x) = 0. With g = gcd_k c_k and h_k = c_k / g:  V(c) = V(g) u V(h) (if g(x) != 0 then all h_k(x) = 0).
/// The engine instead factors P and, per irreducible factor f, takes the d-coefficients of f; since Q[d] is an integral
/// domain, P(x,.) == 0 <=> some f(x,.) == 0, so both give the same set. Here only g (d-free) is factored; the
/// d-dependent part becomes ONE AND-branch [h_k] (omitted when some h_k is a nonzero constant: then V(h) is empty).
fn xs_content_exceptions<const N: usize>(candidate: &RuleCandidate<N>, idx: &[usize; N], sector: &[bool; N], names: &[String], log: &mut Vec<String>) -> ExceptionalConditions {
    if candidate.rhs.is_empty() {
        return ExceptionalConditions::default();
    }
    let template = &candidate.rhs[0].coefficient.numerator;
    let params: Vec<usize> = (0..template.nvars()).filter(|p| !idx.contains(p)).collect();
    let inputs = xs_guard_inputs(candidate, idx);
    let mut branches: Vec<Vec<CoefficientPolynomial>> = Vec::new();
    for (k, den) in inputs.iter().enumerate() {
        let t = Instant::now();
        let coeffs: Vec<CoefficientPolynomial> = den.to_multivariate_polynomial_list(&params, true).into_values().filter(|c| !c.is_zero()).collect();
        let t_split = t.elapsed().as_secs_f64();
        let mut g = coeffs[0].clone();
        for c in &coeffs[1..] {
            if g.is_constant() {
                break;
            }
            g = g.gcd(c);
        }
        let t_gcd = t.elapsed().as_secs_f64() - t_split;
        let g_const = g.is_constant();
        let mut gfac: Vec<String> = Vec::new();
        let mut nb = 0usize;
        let t2 = Instant::now();
        if !g_const {
            let g = xs_primitive(g.clone());
            for (f, m) in g.factor() {
                if xs_contains_index(&f, idx) {
                    let f = xs_primitive(f);
                    gfac.push(format!("[{},{m}]", q(&f.to_string())));
                    branches.push(vec![f]);
                    nb += 1;
                }
            }
        }
        let t_gfac = t2.elapsed().as_secs_f64();
        let cof: Vec<CoefficientPolynomial> = if g_const { coeffs.clone() } else { coeffs.iter().map(|c| c.try_div(&g).expect("content divides each coefficient")).collect() };
        let cof_empty = cof.iter().any(|h| h.is_constant() && !h.is_zero());
        let mut cof_info = String::from("null");
        if !cof_empty {
            let mut b: Vec<CoefficientPolynomial> = cof.into_iter().filter(|h| !h.is_zero()).map(xs_primitive).collect();
            xs_sort_unique(&mut b);
            let infos: Vec<String> = b.iter().map(|h| format!("{{{}}}", xs_poly_info(h, names))).collect();
            cof_info = format!("[{}]", infos.join(","));
            branches.push(b);
            nb += 1;
        }
        log.push(format!(
            "{{\"type\":\"content_route\",\"input\":{k},\"d_coefficients\":{},\"content_constant\":{g_const},\"content\":{},\"content_factors\":[{}],\"cofactor_system_empty\":{cof_empty},\"cofactor_system\":{cof_info},\"branches\":{nb},\"t_split\":{t_split:.3},\"t_gcd\":{t_gcd:.3},\"t_content_factor\":{t_gfac:.3}}}",
            coeffs.len(),
            if g_const { "null".to_string() } else { q(&g.to_string()) },
            gfac.join(",")
        ));
    }
    branches.extend(xs_activation_branches(candidate, idx, sector));
    branches.sort_unstable_by(InternalOrdering::internal_cmp);
    branches.dedup();
    ExceptionalConditions { branches }
}

fn xs_branches_json(e: &ExceptionalConditions) -> String {
    let b: Vec<String> = e.branches.iter().map(|b| strings(b.iter())).collect();
    format!("[{}]", b.join(","))
}

/// The candidate dump (round 8 / xstuck item 1): written BEFORE any guard extraction.
#[allow(clippy::too_many_arguments)]
fn xs_write_dump<const N: usize>(path: &str, terms_path: &str, candidate: &RuleCandidate<N>, idx: &[usize; N], sector: &[bool; N], names: &[String], rule_index: usize, search_seconds: f64, config_text: &str, probe_env: Option<&str>) {
    let mut tf = std::io::BufWriter::new(std::fs::File::create(terms_path).expect("terms file"));
    let mut dens_raw: Vec<String> = candidate.rhs.iter().map(|t| t.coefficient.denominator.to_string()).collect();
    dens_raw.sort();
    dens_raw.dedup();
    let mut rhs_items: Vec<String> = Vec::new();
    for (k, t) in candidate.rhs.iter().enumerate() {
        let num = t.coefficient.numerator.to_string();
        let den = t.coefficient.denominator.to_string();
        let did = dens_raw.binary_search(&den).unwrap();
        writeln!(tf, "{{\"k\":{k},\"integral\":{},\"num\":{},\"den\":{}}}", q(&t.integral.to_string()), q(&num), q(&den)).unwrap();
        rhs_items.push(format!("{{\"k\":{k},\"integral\":{},\"den_id\":{did},\"num_terms\":{},\"num_chars\":{},\"zero\":{}}}", q(&t.integral.to_string()), t.coefficient.numerator.nterms(), num.len(), t.coefficient.is_zero()));
    }
    tf.flush().unwrap();
    let mut den_items: Vec<String> = Vec::new();
    for (id, s) in dens_raw.iter().enumerate() {
        let p = candidate.rhs.iter().find(|t| &t.coefficient.denominator.to_string() == s).map(|t| &t.coefficient.denominator).unwrap();
        let uses = candidate.rhs.iter().filter(|t| &t.coefficient.denominator.to_string() == s).count();
        den_items.push(format!("{{\"id\":{id},\"uses\":{uses},\"contains_index\":{},{},\"poly\":{}}}", xs_contains_index(p, idx), xs_poly_info(p, names), q(s)));
    }
    let inputs = xs_guard_inputs(candidate, idx);
    let in_items: Vec<String> = inputs.iter().enumerate().map(|(k, p)| format!("{{\"id\":{k},{},\"poly\":{}}}", xs_poly_info(p, names), q(&p.to_string()))).collect();
    let act = xs_activation_branches(candidate, idx, sector);
    let act_items: Vec<String> = act.iter().map(|b| strings(b.iter())).collect();
    let pivots: Vec<String> = candidate.sources.iter().map(|s| format!("{{\"basis_row\":{},\"seed\":{}}}", s.basis_row, q(&format!("{:?}", s.seed)))).collect();
    let st = &candidate.stats;
    let fixed: Vec<String> = candidate.case.fixed().iter().map(|v| v.map_or("null".to_string(), |x| x.to_string())).collect();
    let params: Vec<String> = (0..names.len()).filter(|p| !idx.contains(p)).map(|p| q(&names[p])).collect();
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).expect("dump file"));
    write!(
        f,
        "{{\"schema\":\"xstuck.candidate_dump.v1\",\"created_utc_unix\":{},\"harness\":\"face_queue_xs (round8/xstuck; face_queue_x = face_queue_pp + dump; rr-probept rlib = B8 8ea1796e + opt-in probe point; B8 semantics gsplit on)\",\
\"stage\":\"candidate after solve_case and the native strict-descent check, BEFORE GuardExtraction (extract_exceptions)\",\
\"rule_index\":{rule_index},\"case\":{},\"case_fixed\":[{}],\"target\":{},\"coefficient_variables\":{},\"index_positions\":{:?},\"parameter_variables\":[{}],\"sector\":{:?},\
\"probe_point_env\":{},\"config\":{},\
\"search\":{{\"search_seconds\":{search_seconds:.3},\"seeds\":{},\"rows\":{},\"independent_rows\":{},\"exact_trace_rows\":{},\"direct_hit\":{},\"elapsed\":{:.3},\"exact_materialization\":{:.3},\"discovery\":{}}},\
\"pivots_note\":\"candidate.sources = the source rows of the exact trace: (basis_row = ordinal in SectorSolver::basis after sector-local elimination, seed = the seed shift)\",\
\"pivots\":[{}],\
\"rhs_count\":{},\"rhs\":[{}],\
\"coefficient_denominators_note\":\"distinct exact denominators of the RHS coefficients (as printed by Symbolica), each with its use count\",\
\"coefficient_denominators\":[{}],\
\"guard_inputs_note\":\"the exact inputs of GuardExtraction's factorisation (exception.rs extract): index-containing denominators of nonzero RHS coefficients, primitive, positive lcoeff, sorted+dedup (InternalOrdering). Each irreducible index-containing factor f gives one AND-branch = the d-coefficients of f.\",\
\"guard_inputs\":[{}],\
\"activation_branches\":[{}],\
\"rhs_terms_file\":{}}}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        case_json(&candidate.case),
        fixed.join(","),
        q(&candidate.target.to_string()),
        strings(names.iter()),
        idx,
        params.join(","),
        sector,
        probe_env.map_or("null".to_string(), q),
        q(config_text),
        st.seeds,
        st.rows,
        st.independent_rows,
        st.exact_trace_rows,
        st.direct_hit,
        st.elapsed.as_secs_f64(),
        st.exact_materialization.as_secs_f64(),
        q(&format!("{:?}", st.discovery)),
        pivots.join(","),
        candidate.rhs.len(),
        rhs_items.join(","),
        den_items.join(","),
        in_items.join(","),
        act_items.join(","),
        q(terms_path)
    )
    .unwrap();
    writeln!(f).unwrap();
    f.flush().unwrap();
}

/// One certificate (rule_frame), in-process or in a forked child with a timeout (cert_timeout).
fn xs_cert_one<const N: usize>(solver: &SectorSolver<N>, label: &str, index: usize, rule: &SectorRule<N>, cert_timeout: Option<u64>, qlog: &mut impl std::io::Write) -> (String, bool, usize, bool) {
    match cert_timeout {
        None => cert_line(solver, label, index, rule),
        Some(cs) => {
            qlog.flush().unwrap();
            let r = forked(&format!("cert_r{index}"), cs, || {
                let (line, ok, w, p) = cert_line(solver, label, index, rule);
                format!("{}\t{}\t{}\t{}", u8::from(ok), w, u8::from(p), line)
            });
            match r {
                Forked::Done(s, used) => {
                    let mut it = s.splitn(4, '\t');
                    let ok = it.next() == Some("1");
                    let w: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                    let p = it.next() == Some("1");
                    let line = it.next().unwrap_or("").trim_end().to_string();
                    writeln!(qlog, "{{\"type\":\"cert_forked\",\"rule\":{index},\"status\":\"done\",\"wall\":{used:.3},\"ok\":{ok}}}").unwrap();
                    (line, ok && !s.contains("child_panic"), w, p)
                }
                Forked::Timeout(wall, cpu) => {
                    writeln!(qlog, "{{\"type\":\"cert_forked\",\"rule\":{index},\"status\":\"timeout\",\"wall\":{wall:.3},\"child_cpu\":{cpu:.3}}}").unwrap();
                    (format!("{{\"type\":\"cert_error\",\"chart\":{},\"rule\":{index},\"error\":{}}}", q(&label), q(&format!("cert_timeout: rule_frame exceeded {cs} s (child cpu {cpu:.1} s)"))), false, 0, false)
                }
                Forked::Failed(m, wall) => {
                    writeln!(qlog, "{{\"type\":\"cert_forked\",\"rule\":{index},\"status\":\"failed\",\"wall\":{wall:.3},\"msg\":{}}}", q(&m)).unwrap();
                    (format!("{{\"type\":\"cert_error\",\"chart\":{},\"rule\":{index},\"error\":{}}}", q(&label), q(&format!("cert child failed: {m}"))), false, 0, false)
                }
            }
        }
    }
}
// ======================= end of round 8 / xstuck additions =======================

fn parse_linear(expr: &str, template: &CoefficientPolynomial, idx: &[usize]) -> CoefficientPolynomial {
    let vars = template.variables();
    let mut acc = template.zero();
    let s: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    let mut terms: Vec<(i64, String)> = Vec::new();
    let mut cur = String::new();
    let mut sign = 1i64;
    for (k, c) in s.chars().enumerate() {
        if (c == '+' || c == '-') && k > 0 {
            terms.push((sign, std::mem::take(&mut cur)));
            sign = if c == '-' { -1 } else { 1 };
        } else if (c == '+' || c == '-') && k == 0 {
            sign = if c == '-' { -1 } else { 1 };
        } else {
            cur.push(c);
        }
    }
    terms.push((sign, cur));
    for (sign, t) in terms {
        let (coef, var) = match t.split_once('*') {
            Some((a, b)) => (a.parse::<i64>().expect("coefficient"), Some(b.to_string())),
            None => {
                if t.starts_with('n') {
                    (1, Some(t.clone()))
                } else {
                    (t.parse::<i64>().expect("constant"), None)
                }
            }
        };
        let c = sign * coef;
        let base = match var {
            Some(v) => {
                let i: usize = v.trim_start_matches('n').parse().expect("variable n<k>");
                template.variable(&vars[idx[i]]).expect("index variable")
            }
            None => template.one(),
        };
        for _ in 0..c.unsigned_abs() {
            acc = if c > 0 { &acc + &base } else { &acc - &base };
        }
    }
    acc
}

/// Total degree of an exception polynomial in the index variables, from its exact printed form (d-free guards).
fn total_degree(p: &CoefficientPolynomial) -> u32 {
    let s = p.to_string().replace(' ', "");
    let mut best = 0u32;
    let mut term = String::new();
    let mut flush = |t: &str, best: &mut u32| {
        let mut deg = 0u32;
        for f in t.split('*') {
            if let Some(rest) = f.strip_prefix('n') {
                deg += match rest.split_once('^') {
                    Some((_, e)) => e.parse::<u32>().unwrap_or(99),
                    None => 1,
                };
            } else if f.chars().any(|c| c.is_ascii_alphabetic()) {
                deg += 99; // a non-index symbol: treat as nonlinear (never expected in index guards)
            }
        }
        *best = (*best).max(deg);
    };
    for (k, ch) in s.chars().enumerate() {
        if (ch == '+' || ch == '-') && k > 0 && !s[..k].ends_with('^') {
            flush(&term, &mut best);
            term.clear();
        } else if ch != '+' && ch != '-' {
            term.push(ch);
        }
    }
    flush(&term, &mut best);
    best
}

fn parse_spec<const N: usize>(spec: &str) -> [Option<i16>; N] {
    let parts: Vec<&str> = spec.split(',').collect();
    assert_eq!(parts.len(), N, "spec arity");
    std::array::from_fn(|i| if parts[i].trim() == "_" { None } else { Some(parts[i].trim().parse::<i16>().expect("spec value")) })
}

fn case_from<const N: usize>(spec: &str, eqs: &str, template: &CoefficientPolynomial, idx: &[usize; N], sector: &[bool; N]) -> Option<Case<N>> {
    let face = Case::from(CoordinateCase::new(parse_spec::<N>(spec)).expect("coordinate case"));
    let polys: Vec<CoefficientPolynomial> = eqs.split(';').map(str::trim).filter(|e| !e.is_empty()).map(|e| parse_linear(e, template, idx)).collect();
    face.intersect(&polys, idx, sector).expect("admit nominated case")
}

/// One table rule record -> Some(Some((case, branches))) if index < upto and parsable, Some(None) if not parsable, None if
/// index >= upto. Minimal JSON field extraction (the records are written by write_rule above).
fn r9_rule_cover<const N: usize>(line: &str, template: &CoefficientPolynomial, idx: &[usize; N], sector: &[bool; N], upto: usize)
    -> Option<Option<(Case<N>, Vec<Vec<CoefficientPolynomial>>)>> {
    let field = |key: &str| -> Option<&str> {
        let k = format!("\"{key}\":");
        let at = line.find(&k)? + k.len();
        let rest = &line[at..];
        let mut depth = 0i32;
        let mut instr = false;
        for (i, ch) in rest.char_indices() {
            match ch {
                '"' => instr = !instr,
                '[' | '{' if !instr => depth += 1,
                ']' | '}' if !instr => {
                    depth -= 1;
                    if depth < 0 {
                        return Some(&rest[..i]);
                    }
                    if depth == 0 {
                        return Some(&rest[..=i]);
                    }
                }
                ',' if !instr && depth == 0 => return Some(&rest[..i]),
                _ => (),
            }
        }
        None
    };
    let index: usize = field("index")?.trim().parse().ok()?;
    if index >= upto {
        return None;
    }
    let strs = |s: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut instr = false;
        for ch in s.chars() {
            if ch == '"' {
                if instr {
                    out.push(std::mem::take(&mut cur));
                }
                instr = !instr;
            } else if instr {
                cur.push(ch);
            }
        }
        out
    };
    let fixed = field("fixed")?;
    let spec: Vec<String> = fixed.trim_matches(|c| c == '[' || c == ']').split(',').map(|v| if v.trim() == "null" { "_".to_string() } else { v.trim().to_string() }).collect();
    if spec.len() != N {
        return Some(None);
    }
    let eqs = strs(field("equations")?).join(";");
    let case = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case_from::<N>(&spec.join(","), &eqs, template, idx, sector))).ok().flatten();
    let Some(case) = case else { return Some(None) };
    // exceptions: [[..],[..]] -> split the outer list into its inner lists
    let ex = field("exceptions")?;
    let inner = &ex[1..ex.len() - 1];
    let mut branches = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut instr = false;
    for (i, ch) in inner.char_indices() {
        match ch {
            '"' => instr = !instr,
            '[' if !instr => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            ']' if !instr => {
                depth -= 1;
                if depth == 0 {
                    let mut b = Vec::new();
                    for p in strs(&inner[start..=i]) {
                        match r9_parse_poly(&p, template, idx) {
                            Some(x) => b.push(x),
                            None => return Some(None),
                        }
                    }
                    branches.push(b);
                }
            }
            _ => (),
        }
    }
    Some(Some((case, branches)))
}

/// Integer polynomial in the index variables n<i> (sum of c*n_i^e*.. monomials) -> CoefficientPolynomial; None if it has
/// another symbol or a coefficient beyond i64 (then that rule is simply not used for pruning).
fn r9_parse_poly(expr: &str, template: &CoefficientPolynomial, idx: &[usize]) -> Option<CoefficientPolynomial> {
    let vars = template.variables();
    let s: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
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
        let mut c: i64 = 1;
        let mut m = template.one();
        for f in body.split('*') {
            if f.chars().all(|x| x.is_ascii_digit()) {
                c = c.checked_mul(f.parse::<i64>().ok()?)?;
            } else {
                let (name, e) = match f.split_once('^') {
                    Some((a, b)) => (a, b.parse::<u32>().ok()?),
                    None => (f, 1),
                };
                let i: usize = name.strip_prefix('n')?.parse().ok()?;
                if i >= idx.len() {
                    return None;
                }
                let v = template.variable(&vars[idx[i]]).ok()?;
                for _ in 0..e {
                    m = &m * &v;
                }
            }
        }
        let k = if neg { -c } else { c };
        m = m.mul_coeff(Integer::from(k));
        acc = &acc + &m;
    }
    Some(acc)
}

fn case_json<const N: usize>(c: &Case<N>) -> String {
    let fixed: Vec<String> = c.fixed().iter().map(|v| v.map_or("null".to_string(), |x| x.to_string())).collect();
    let equations = match c {
        Case::Coordinate(_) => "[]".to_string(),
        Case::Affine(a) => strings(a.equations().iter()),
    };
    format!("{{\"fixed\":[{}],\"equations\":{}}}", fixed.join(","), equations)
}

fn rule_covers<const N: usize>(rule: &SectorRule<N>, case: &Case<N>, idx: &[usize; N], sector: &[bool; N], limits: CaseIntersectionLimits) -> bool {
    if !rule.candidate.case.contains(case).unwrap_or(false) {
        return false;
    }
    for branch in &rule.exceptions.branches {
        match case.intersect_many(branch, idx, sector, limits) {
            Ok(r) if r.cases.is_empty() => (),
            _ => return false,
        }
    }
    true
}

fn prune_subsumed<const N: usize>(cases: &mut Vec<Case<N>>, candidate: &Case<N>) -> bool {
    for queued in cases.iter() {
        if queued.contains(candidate).unwrap_or(false) {
            return false;
        }
    }
    let mut position = 0;
    while position < cases.len() {
        if candidate.contains(&cases[position]).unwrap_or(false) {
            cases.remove(position);
        } else {
            position += 1;
        }
    }
    true
}

fn cert_line<const N: usize>(solver: &SectorSolver<N>, label: &str, index: usize, rule: &SectorRule<N>) -> (String, bool, usize, bool) {
    let rows = rule.candidate.sources.len();
    let t5 = Instant::now();
    match solver.rule_frame(rule) {
        Ok(frame) => {
            let entries: Vec<String> = frame.entries.iter().map(|e| format!("[{},{},{}]", e.basis_row, q(&e.at.to_string()), q(&e.weight.to_string()))).collect();
            let poles: Vec<String> = frame.weight_poles.iter().map(|b| strings(b.iter())).collect();
            (
                format!(
                    "{{\"type\":\"cert_rule\",\"chart\":{},\"rule\":{index},\"via\":{},\"recenter\":{:?},\"trace_rows\":{rows},\"zero_weights\":{},\"seconds\":{:.6},\"weight_poles\":[{}],\"frame\":[{}]}}",
                    q(label), q(frame.via), frame.recenter, frame.zero_weights, t5.elapsed().as_secs_f64(), poles.join(","), entries.join(",")
                ),
                true,
                frame.entries.len(),
                !frame.weight_poles.is_empty(),
            )
        }
        Err(e) => (format!("{{\"type\":\"cert_error\",\"chart\":{},\"rule\":{index},\"error\":{}}}", q(label), q(&format!("{e:?}").chars().take(800).collect::<String>())), false, 0, false),
    }
}

fn run<const N: usize>(family_path: &str, family_text: &str, fam: &IntegralFamily, out_path: &str, cert_path: &str, config_path: &str) -> i32 {
    let n = fam.denominator_count();
    let t_total = Instant::now();
    let config_text = std::fs::read_to_string(config_path).expect("config");
    let mut sources = SourceSystem::<N>::from_family_with_capacity(fam, true).expect("sources");
    // [p5l-repair r9] pre-scan: `order NAME` (legacy default = the 5L campaign; auto = two-sided on the eps axes, = table_dump
    // top auto) and `chart MASK` (one chart of the 2^j two-sided charts, as table_dump --charts; default = the top chart).
    let mut order_req = String::from("legacy");
    let mut chart_req: Option<String> = None;
    for line in config_text.lines() {
        let line = line.split('#').next().unwrap().trim();
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.first() {
            Some(&"order") => order_req = w[1].to_string(),
            Some(&"chart") => chart_req = Some(w[1].to_string()),
            _ => (),
        }
    }
    let axes: [bool; N] = if matches!(order_req.as_str(), "legacy" | "one_sided") { [false; N] } else { sources.set_two_sided_eps_axes(fam).expect("eps axes") };
    let (descriptor, program, order_name) = order_for::<N>(&order_req, &axes, n);
    let mask: Vec<bool> = family_text
        .lines()
        .filter(|l| l.trim_start().starts_with("prop("))
        .map(|l| l.trim_end().trim_end_matches(',').ends_with(",1)"))
        .collect();
    assert_eq!(mask.len(), n);
    let top: [bool; N] = std::array::from_fn(|i| mask[i] || axes[i]);
    let base: [bool; N] = match &chart_req {
        Some(m) => {
            let b: Vec<bool> = m.chars().map(|c| c == '1').collect();
            assert_eq!(b.len(), n, "chart mask arity");
            for i in 0..n {
                assert!(axes[i] || b[i] == top[i], "chart {m} differs from the top mask off the eps axes");
            }
            std::array::from_fn(|i| b[i])
        }
        None => top,
    };
    let label = mask_string(&base[..n]);
    let analyzer = zero::Analyzer::try_unrestricted(fam).expect("analyzer");
    let scope: Vec<usize> = (0..n).filter(|&i| top[i]).collect();
    let t0 = Instant::now();
    let mut zeros: Vec<[bool; N]> = Vec::new();
    for bits in 0u64..(1u64 << scope.len()) {
        let mut active = [false; N];
        for (j, &i) in scope.iter().enumerate() {
            active[i] = bits >> j & 1 == 1;
        }
        let m = Mask::try_new(active.iter().copied()).unwrap();
        if let zero::Decision::ProvedZero(_) = analyzer.analyze(&m).expect("analyze") {
            zeros.push(active);
        }
    }
    let zero_seconds = t0.elapsed().as_secs_f64();
    let mut out = std::io::BufWriter::new(std::fs::File::create(out_path).expect("output file"));
    writeln!(
        out,
        "{{\"type\":\"header\",\"schema\":{},\"source_base\":{},\"family\":{},\"family_file\":{},\"family_source\":{},\"loops\":{},\"externals\":{},\"denominators\":{n},\"parameters\":{},\"power_shifts\":{},\"mask\":{},\"order_name\":{},\"order_is_program\":{},\"order\":{},\"order_bytes\":{},\"two_sided_axes\":{:?},\"charts\":{},\"numerical_depth\":2,\"max_numerator_rank\":null,\"source_conditions\":{},\"zero_scope\":\"downset\",\"zero_seconds\":{zero_seconds:.3},\"face_queue_config\":{}}}",
        q(SCHEMA),
        q(SOURCE_BASE),
        q(fam.name()),
        q(family_path),
        q(family_text),
        fam.loop_count(),
        fam.external_count(),
        strings(fam.coefficient_context().parameter_names().iter()),
        strings(fam.power_shifts().iter()),
        q(&mask_string(&mask)),
        q(&order_name),
        program.is_some(),
        descriptor_json(&descriptor),
        q(""),
        &axes[..n],
        strings([label.clone()].iter()),
        strings(sources.conditions().iter()),
        q(&config_text)
    )
    .unwrap();
    writeln!(out, "{{\"type\":\"zero_sectors\",\"scope\":\"downset\",\"count\":{},\"masks\":{}}}", zeros.len(), strings(zeros.iter().map(|z| mask_string(&z[..n])))).unwrap();
    let guards: Vec<String> = (0..n)
        .map(|i| match (axes[i], base[i]) {
            (true, true) => "two_sided:n>=1".to_string(),
            (true, false) => "two_sided:n<=0".to_string(),
            (false, true) => "n>=1".to_string(),
            (false, false) => "n<=0".to_string(),
        })
        .collect();
    if zeros.contains(&base) {
        writeln!(out, "{{\"type\":\"chart\",\"chart\":{},\"guards\":{},\"zero\":true}}", q(&label), strings(guards.iter())).unwrap();
        writeln!(out, "{{\"type\":\"end\",\"charts\":1,\"rules\":0,\"rhs_terms\":0,\"residuals\":0,\"chart_errors\":0,\"seconds\":{:.3},\"vmhwm_kb\":{},\"zero_chart\":true,\"native_descent_not_lower\":[]}}", t_total.elapsed().as_secs_f64(), vmhwm_kb()).unwrap();
        out.flush().unwrap();
        println!("{{\"chart\":\"{label}\",\"zero\":true}}");
        return 0;
    }
    writeln!(out, "{{\"type\":\"chart\",\"chart\":{},\"guards\":{},\"zero\":false}}", q(&label), strings(guards.iter())).unwrap();
    out.flush().unwrap();
    let qpath = format!("{}.queue.jsonl", out_path.trim_end_matches(".table.jsonl"));
    let mut qlog = std::io::BufWriter::new(std::fs::File::create(&qpath).expect("queue log"));
    // [round8/xstuck v3] exact_backend sparse|factorized|seminumerical MAXDEG MAXPROBES MAXATTEMPTS MAXPRIMES (pre-scan: the
    // backend is part of SectorConfig, built before the key loop). Default sparse = the established B8 engine.
    let mut exact_backend = rustred::solver::SymbolicExactBackend::Sparse;
    for line in config_text.lines() {
        let line = line.split('#').next().unwrap().trim();
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.first() == Some(&"exact_backend") {
            exact_backend = match w[1] {
                "sparse" => rustred::solver::SymbolicExactBackend::Sparse,
                "factorized" => rustred::solver::SymbolicExactBackend::SparseFactorized,
                "seminumerical" => rustred::solver::SymbolicExactBackend::SemiNumerical {
                    diagnostics_enabled: false,
                    max_degree: w[2].parse().unwrap(),
                    max_probes: w[3].parse().unwrap(),
                    max_attempts: w[4].parse().unwrap(),
                    max_primes: w[5].parse().unwrap(),
                },
                // [coordinator 2026-11-10] the lifts that avoid the full characteristic-zero sparse elimination
                "targetonly" => rustred::solver::SymbolicExactBackend::SparseTargetOnly,
                "targetonlyfactorized" => rustred::solver::SymbolicExactBackend::SparseTargetOnlyFactorized,
                "sourceweights" => rustred::solver::SymbolicExactBackend::SemiNumericalSourceWeights {
                    diagnostics_enabled: false,
                    max_degree: w[2].parse().unwrap(),
                    max_probes: w[3].parse().unwrap(),
                    max_attempts: w[4].parse().unwrap(),
                    max_primes: w[5].parse().unwrap(),
                    max_cached_images: w[6].parse().unwrap(),
                    max_cached_values: w[7].parse().unwrap(),
                    max_weight_slots: w[8].parse().unwrap(),
                },
                other => panic!("exact_backend {other}"),
            };
        }
    }
    eprintln!("EXACT_BACKEND {exact_backend:?}");
    let config = SectorConfig { zero_sectors: Arc::from(zeros.clone()), integral_order: program.clone(), symbolic_exact_backend: exact_backend, ..Default::default() };
    let t1 = Instant::now();
    let (solver, derivation) = SectorSolver::with_basis_derivation(&sources, base, config.clone()).expect("solver with basis derivation");
    eprintln!("PREPARED basis={} seconds={:.3}", solver.basis().len(), t1.elapsed().as_secs_f64());
    let idx: [usize; N] = *sources.index_variables();
    let sector: [bool; N] = *solver.ordering().sector();
    let template = sources.rows().iter().flat_map(|r| r.iter()).next().expect("a source term").coefficient.clone();
    // config
    let mut initial: Vec<Case<N>> = Vec::new();
    let mut excludes: Vec<(String, Case<N>)> = Vec::new();
    let mut want_cert = true;
    let mut max_cases: Option<usize> = None;
    let mut defer_nonlinear = false;
    let mut gsplit = true;
    let mut max_depth: Option<u32> = None;
    let mut source_order_spec = String::from("input");
    let mut exc_timeout: Option<u64> = None;
    let mut dump_candidate = false;
    let mut probe_spec: Option<String> = None;
    let mut cand_dump = false;
    let mut native_try: Option<u64> = None;
    let mut fallback = String::from("exit");
    let mut content_timeout: u64 = 3600;
    let mut guard_profile: Option<u64> = None;
    let mut guard_profile_max: usize = 2;
    let mut cert_timeout: Option<u64> = None;
    let mut cert_defer = false;
    let mut rule_secs: Vec<(usize, f64)> = Vec::new();
    let mut case_budget: Option<u64> = None;
    let mut budget_initial = true;
    let mut admit_try: Option<u64> = None;
    for line in config_text.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        match words[0] {
            "initial" => {
                let c = case_from::<N>(words[1], words.get(2).copied().unwrap_or(""), &template, &idx, &sector);
                match c {
                    Some(c) => {
                        writeln!(qlog, "{{\"type\":\"initial\",\"case\":{}}}", case_json(&c)).unwrap();
                        if prune_subsumed(&mut initial, &c) {
                            initial.push(c);
                        }
                    }
                    None => writeln!(qlog, "{{\"type\":\"initial_empty\",\"spec\":{}}}", q(line)).unwrap(),
                }
            }
            "exclude" => {
                let c = case_from::<N>(words[2], words.get(3).copied().unwrap_or(""), &template, &idx, &sector);
                match c {
                    Some(c) => {
                        writeln!(qlog, "{{\"type\":\"exclude\",\"label\":{},\"case\":{}}}", q(words[1]), case_json(&c)).unwrap();
                        excludes.push((words[1].to_string(), c));
                    }
                    None => writeln!(qlog, "{{\"type\":\"exclude_empty\",\"spec\":{}}}", q(line)).unwrap(),
                }
            }
            "cert" => want_cert = words[1] == "on",
            "max_cases" => max_cases = Some(words[1].parse().unwrap()),
            "defer_nonlinear" => defer_nonlinear = words[1] == "on",
            "gsplit" => gsplit = words[1] == "on",
            "max_depth" => max_depth = Some(words[1].parse().unwrap()),
            "source_order" => source_order_spec = words[1].to_string(),
            "exc_timeout" => exc_timeout = Some(words[1].parse().unwrap()),
            "dump_candidate" => dump_candidate = words[1] == "on",
            "probe_point" => probe_spec = Some(words[1].to_string()),
            "cand_dump" => cand_dump = words[1] == "on",
            "native_try" => native_try = Some(words[1].parse().unwrap()),
            "fallback" => fallback = words[1].to_string(),
            "content_timeout" => content_timeout = words[1].parse().unwrap(),
            "guard_profile" => guard_profile = Some(words[1].parse().unwrap()),
            "guard_profile_max" => guard_profile_max = words[1].parse().unwrap(),
            "cert_timeout" => cert_timeout = Some(words[1].parse().unwrap()),
            "cert_defer" => cert_defer = words[1] == "on",
            "exact_backend" | "order" | "chart" | "cover_table" => {}
            "case_budget" => case_budget = Some(words[1].parse().unwrap()),
            "budget_initial" => budget_initial = words[1] == "on",
            "admit_try" => admit_try = Some(words[1].parse().unwrap()),
            "exc_route" => unsafe { std::env::set_var("RUSTRED_EXC_ROUTE", words[1]) },
            other => panic!("config keyword {other}"),
        }
    }
    qlog.flush().unwrap();
    // certificate header + basis rows
    let mut c = std::io::BufWriter::new(std::fs::File::create(cert_path).expect("cert file"));
    write_cert_header(&mut c, fam, &sources);
    for (k, row) in derivation.rows.iter().enumerate() {
        let entries: Vec<String> = row.iter().map(|(m, p)| format!("[{m},{}]", q(&p.to_string()))).collect();
        writeln!(c, "{{\"type\":\"basis_row\",\"chart\":{},\"k\":{k},\"sources\":[{}]}}", q(&label), entries.join(",")).unwrap();
    }
    c.flush().unwrap();
    let limits = CaseIntersectionLimits::default();
    let mut symbolic = SectorSolveOptions::default().symbolic;
    if max_depth.is_some() {
        symbolic.max_depth = max_depth;
    }
    let nb = solver.basis().len();
    let source_order: Option<Vec<usize>> = match source_order_spec.as_str() {
        "input" => None,
        "reverse" => Some((0..nb).rev().collect()),
        s if s.starts_with("rot:") => {
            let k: usize = s[4..].parse().unwrap();
            Some((0..nb).map(|i| (i + k) % nb).collect())
        }
        s if s.starts_with("shuffle:") => {
            let mut state: u64 = s[8..].parse::<u64>().unwrap() ^ 0x9E37_79B9_7F4A_7C15;
            let mut v: Vec<usize> = (0..nb).collect();
            for i in (1..nb).rev() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let j = (state % (i as u64 + 1)) as usize;
                v.swap(i, j);
            }
            Some(v)
        }
        other => panic!("source_order {other}"),
    };
    writeln!(qlog, "{{\"type\":\"options\",\"gsplit\":{gsplit},\"defer_nonlinear\":{defer_nonlinear},\"max_depth\":{},\"source_order\":{},\"basis\":{nb}}}", max_depth.map_or("null".to_string(), |k| k.to_string()), q(&source_order_spec)).unwrap();
    // probe point: SPEC -> residues ordered by the coefficient variables (index variable n_i at position idx[i], d = the one other)
    let nvars = template.variables().len();
    let probe_env: Option<String> = probe_spec.as_ref().map(|spec| {
        let mut vals: Vec<Option<String>> = vec![None; nvars];
        let mut dval: Option<String> = None;
        for kv in spec.split(',') {
            let (k, v) = kv.split_once('=').expect("probe_point name=value");
            v.parse::<u64>().expect("probe_point residue");
            if k == "d" {
                dval = Some(v.to_string());
            } else {
                let i: usize = k.trim_start_matches('n').parse().expect("probe_point n<i>");
                assert!(vals[idx[i]].is_none(), "probe_point repeated {k}");
                vals[idx[i]] = Some(v.to_string());
            }
        }
        let rest: Vec<usize> = (0..nvars).filter(|p| vals[*p].is_none()).collect();
        assert_eq!(rest.len(), 1, "probe_point must name every index variable n0..n{} and d", N - 1);
        vals[rest[0]] = Some(dval.expect("probe_point d"));
        let joined: Vec<String> = vals.into_iter().map(|v| v.unwrap()).collect();
        joined.join(",")
    });
    writeln!(qlog, "{{\"type\":\"probe_point\",\"nvars\":{nvars},\"spec\":{},\"env\":{}}}", probe_spec.as_deref().map_or("null".to_string(), q), probe_env.as_deref().map_or("null".to_string(), q)).unwrap();
    // [p5l-repair r9 v2] `cover_table PATH UPTO`: the rules 0..UPTO of an ancestor table (case + exception branches)
    let mut covers: Vec<(String, Case<N>, Vec<Vec<CoefficientPolynomial>>)> = Vec::new();
    for line in config_text.lines() {
        let line = line.split('#').next().unwrap().trim();
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.first() != Some(&"cover_table") {
            continue;
        }
        let upto: usize = w[2].parse().unwrap();
        let text = std::fs::read_to_string(w[1]).expect("cover table");
        let (mut loaded, mut skipped) = (0usize, 0usize);
        for l in text.lines() {
            if !l.starts_with("{\"type\":\"rule\"") {
                continue;
            }
            let Some(r) = r9_rule_cover::<N>(l, &template, &idx, &sector, upto) else { continue };
            match r {
                Some(x) => {
                    covers.push((format!("{}#{}", w[1], loaded), x.0, x.1));
                    loaded += 1;
                }
                None => skipped += 1,
            }
        }
        writeln!(qlog, "{{\"type\":\"cover_table\",\"path\":{},\"upto\":{upto},\"loaded\":{loaded},\"skipped\":{skipped}}}", q(w[1])).unwrap();
        eprintln!("COVER_TABLE {} upto {upto}: {loaded} rules loaded, {skipped} skipped (unparsed)", w[1]);
    }
    let initial_keys: Vec<String> = initial.iter().map(|c| case_json(c)).collect();
    let mut pending: Vec<Case<N>> = Vec::new();
    let mut numerical: Vec<CoordinateCase<N>> = Vec::new();
    let mut rules: Vec<SectorRule<N>> = Vec::new();
    let (mut cert_ok, mut cert_failed, mut nweights, mut poles_n) = (0usize, 0usize, 0usize, 0usize);
    let mut deferred_regions = 0usize;
    let (mut symbolic_cases, mut discarded, mut deferred_cases, mut deferred_branches, mut lifted, mut modular_empty) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut not_lower: Vec<String> = Vec::new();
    let mut terms = 0usize;
    let mut branches_total = 0usize;
    // enqueue closure as a macro-like fn over the explicit state
    #[allow(clippy::too_many_arguments)]
    fn enqueue<const N: usize>(
        case: Case<N>, from: Option<usize>, pending: &mut Vec<Case<N>>, numerical: &mut Vec<CoordinateCase<N>>, rules: &[SectorRule<N>],
        excludes: &[(String, Case<N>)], idx: &[usize; N], sector: &[bool; N], limits: CaseIntersectionLimits, qlog: &mut impl std::io::Write,
        discarded: &mut usize, deferred: &mut usize, covers: &[(String, Case<N>, Vec<Vec<CoefficientPolynomial>>)],
    ) {
        // [p5l-repair r9 v2] rules of the ancestor pieces (FirstValid-earlier in the assembled table) prune like own rules
        for (label, rc, branches) in covers {
            if rc.contains(&case).unwrap_or(false)
                && branches.iter().all(|b| matches!(case.intersect_many(b, idx, sector, limits), Ok(r) if r.cases.is_empty()))
            {
                writeln!(qlog, "{{\"type\":\"covered\",\"from_rule\":{},\"by_ancestor\":{},\"case\":{}}}", from.map_or("null".into(), |x| x.to_string()), q(label), case_json(&case)).unwrap();
                *discarded += 1;
                return;
            }
        }
        for (k, rule) in rules.iter().enumerate() {
            if rule_covers(rule, &case, idx, sector, limits) {
                writeln!(qlog, "{{\"type\":\"covered\",\"from_rule\":{},\"by_rule\":{k},\"case\":{}}}", from.map_or("null".into(), |x| x.to_string()), case_json(&case)).unwrap();
                *discarded += 1;
                return;
            }
        }
        for (lab, ex) in excludes {
            if ex.contains(&case).unwrap_or(false) {
                writeln!(qlog, "{{\"type\":\"deferred_case\",\"owner\":{},\"from_rule\":{},\"case\":{}}}", q(lab), from.map_or("null".into(), |x| x.to_string()), case_json(&case)).unwrap();
                *deferred += 1;
                return;
            }
        }
        if !prune_subsumed(pending, &case) {
            writeln!(qlog, "{{\"type\":\"subsumed_by_pending\",\"from_rule\":{},\"case\":{}}}", from.map_or("null".into(), |x| x.to_string()), case_json(&case)).unwrap();
            *discarded += 1;
            return;
        }
        if case.is_numerical() && std::env::var("T211_FORCE_SINGLE_TARGET").as_deref() != Ok("1") {
            let fixed = *case.face();
            if numerical.iter().any(|x| x == &fixed) {
                *discarded += 1;
                return;
            }
            writeln!(qlog, "{{\"type\":\"enqueue_numerical\",\"from_rule\":{},\"case\":{}}}", from.map_or("null".into(), |x| x.to_string()), case_json(&case)).unwrap();
            numerical.push(fixed);
            return;
        }
        match pending.binary_search_by(|queued| queued.queue_cmp(&case)) {
            Ok(_) => *discarded += 1,
            Err(position) => {
                writeln!(qlog, "{{\"type\":\"enqueue\",\"from_rule\":{},\"case\":{}}}", from.map_or("null".into(), |x| x.to_string()), case_json(&case)).unwrap();
                pending.insert(position, case);
            }
        }
    }
    initial.sort_unstable_by(Case::queue_cmp);
    for case in initial {
        enqueue(case, None, &mut pending, &mut numerical, &rules, &excludes, &idx, &sector, limits, &mut qlog, &mut discarded, &mut deferred_cases, &covers);
    }
    qlog.flush().unwrap();
    let t2 = Instant::now();
    let mut error: Option<String> = None;
    while !pending.is_empty() {
        if max_cases.is_some_and(|m| symbolic_cases >= m) {
            error = Some(format!("max_cases {symbolic_cases} reached, {} pending", pending.len()));
            break;
        }
        let current = pending.remove(0);
        if let Some(b) = case_budget {
            let mut g = R9.lock().unwrap();
            if g.is_none() {
                r9_watchdog(b, format!("{}.spill.json", out_path.trim_end_matches(".table.jsonl")));
            }
            let fixed_json = |f: &CoordinateCase<N>| {
                let v: Vec<String> = f.fixed().iter().map(|x| x.map_or("null".to_string(), |y| y.to_string())).collect();
                format!("{{\"fixed\":[{}],\"equations\":[]}}", v.join(","))
            };
            *g = Some(R9Snap {
                started: Some(Instant::now()),
                applies: budget_initial || !initial_keys.contains(&case_json(&current)),
                current: case_json(&current),
                pending: pending.iter().map(|c| case_json(c)).collect(),
                numerical: numerical.iter().map(fixed_json).collect(),
                rules_done: rules.len(),
            });
        }
        let use_probe = probe_env.is_some() && initial_keys.contains(&case_json(&current));
        // SAFETY: single-threaded harness (RAYON_NUM_THREADS=1); the variable is read only inside Probe::new of this solve.
        unsafe {
            if use_probe {
                std::env::set_var("RUSTRED_PROBE_POINT", probe_env.as_deref().unwrap());
            } else {
                std::env::remove_var("RUSTRED_PROBE_POINT");
            }
        }
        writeln!(qlog, "{{\"type\":\"probe_mode\",\"probe_point\":{use_probe},\"case\":{}}}", case_json(&current)).unwrap();
        eprintln!("SEVENT {:.3}s CaseStarted pending={} case={:?}", t2.elapsed().as_secs_f64(), pending.len(), current);
        writeln!(qlog, "{{\"type\":\"case_started\",\"t\":{:.3},\"pending\":{},\"case\":{}}}", t2.elapsed().as_secs_f64(), pending.len(), case_json(&current)).unwrap();
        qlog.flush().unwrap();
        let ts = Instant::now();
        let observe = |event: rustred::solver::SearchEvent<N>| {
            let s = format!("{event:?}");
            if !s.starts_with("ExactProgress") {
                eprintln!("EVENT {:.3}s {}", t2.elapsed().as_secs_f64(), s.chars().take(400).collect::<String>());
            }
        };
        let solved = match &source_order {
            None => solver.solve_case_with_observer(current.clone(), symbolic.clone(), observe),
            Some(order) => solver.solve_case_with_source_order_and_observer(current.clone(), symbolic.clone(), order, observe),
        };
        let candidate = match solved {
            Ok(c) => c,
            Err(e) => {
                error = Some(format!("search error on {current:?}: {e:?}"));
                writeln!(qlog, "{{\"type\":\"search_error\",\"case\":{},\"error\":{}}}", case_json(&current), q(&format!("{e:?}").chars().take(4000).collect::<String>())).unwrap();
                break;
            }
        };
        symbolic_cases += 1;
        let search_seconds = ts.elapsed().as_secs_f64();
        let nl: Vec<String> = candidate.rhs.iter().filter(|t| solver.ordering().compare(&candidate.target, &t.integral) != Ordering::Less).map(|t| t.integral.to_string()).collect();
        if !nl.is_empty() {
            error = Some(format!("winning rule is not strictly descending on {current:?}"));
            not_lower.extend(nl);
            break;
        }
        if dump_candidate {
            let cpath = format!("{}.cand.jsonl", out_path.trim_end_matches(".table.jsonl"));
            let mut cf = std::io::BufWriter::new(std::fs::OpenOptions::new().create(true).append(true).open(&cpath).expect("cand file"));
            let mut dens: Vec<String> = Vec::new();
            for t in candidate.rhs.iter() {
                let num = t.coefficient.numerator.to_string();
                let den = t.coefficient.denominator.to_string();
                writeln!(cf, "{{\"type\":\"cand_term\",\"rule\":{},\"target\":{},\"integral\":{},\"num_len\":{},\"den_len\":{},\"num\":{},\"den\":{}}}",
                    rules.len(), q(&candidate.target.to_string()), q(&t.integral.to_string()), num.len(), den.len(), q(&num), q(&den)).unwrap();
                dens.push(den);
            }
            dens.sort();
            dens.dedup();
            let maxlen = dens.iter().map(|d| d.len()).max().unwrap_or(0);
            writeln!(cf, "{{\"type\":\"cand_summary\",\"rule\":{},\"case\":{},\"target\":{},\"rhs\":{},\"sources\":{},\"search_seconds\":{:.3},\"distinct_denominators\":{},\"max_den_chars\":{}}}",
                rules.len(), case_json(&candidate.case), q(&candidate.target.to_string()), candidate.rhs.len(), candidate.sources.len(), search_seconds, dens.len(), maxlen).unwrap();
            cf.flush().unwrap();
            eprintln!("CAND dumped rule {} rhs={} distinct_dens={} max_den_chars={}", rules.len(), candidate.rhs.len(), dens.len(), maxlen);
        }
        // ---- round 8 / xstuck: candidate dump BEFORE GuardExtraction
        let names: Vec<String> = template.variables().iter().map(|v| v.to_string()).collect();
        if cand_dump {
            let base = out_path.trim_end_matches(".table.jsonl");
            let dpath = format!("{base}.cand_dump.r{}.json", rules.len());
            let tpath = format!("{base}.cand_terms.r{}.jsonl", rules.len());
            let td = Instant::now();
            xs_write_dump(&dpath, &tpath, &candidate, &idx, &sector, &names, rules.len(), search_seconds, &config_text, if use_probe { probe_env.as_deref() } else { None });
            writeln!(qlog, "{{\"type\":\"cand_dump\",\"rule\":{},\"file\":{},\"terms_file\":{},\"seconds\":{:.3}}}", rules.len(), q(&dpath), q(&tpath), td.elapsed().as_secs_f64()).unwrap();
            qlog.flush().unwrap();
            eprintln!("CAND_DUMP rule {} rhs={} -> {dpath}", rules.len(), candidate.rhs.len());
        }
        let te = Instant::now();
        let mut guard_route = "native";
        let extracted = if let Some(secs) = native_try {
            out.flush().unwrap();
            qlog.flush().unwrap();
            c.flush().unwrap();
            let r = forked(&format!("native_r{}", rules.len()), secs, || {
                let t = Instant::now();
                match extract_exceptions(&candidate, &idx, &sector) {
                    Ok(e) => format!("{{\"ok\":true,\"seconds\":{:.3},\"branches\":{}}}", t.elapsed().as_secs_f64(), xs_branches_json(&e)),
                    Err(e) => format!("{{\"ok\":false,\"error\":{}}}", q(&format!("{e:?}"))),
                }
            });
            match r {
                Forked::Done(s, used) => {
                    writeln!(qlog, "{{\"type\":\"native_try\",\"rule\":{},\"status\":\"done\",\"wall\":{used:.3},\"child\":{}}}", rules.len(), s.trim()).unwrap();
                    qlog.flush().unwrap();
                    eprintln!("NATIVE_TRY done in {used:.1} s on rule {}", rules.len());
                    (extract_exceptions(&candidate, &idx, &sector), candidate)
                }
                other => {
                    let (status, wall, cpu, msg) = match other {
                        Forked::Timeout(w, cpu) => ("timeout", w, cpu, String::new()),
                        Forked::Failed(m, w) => ("failed", w, -1.0, m),
                        Forked::Done(..) => unreachable!(),
                    };
                    writeln!(qlog, "{{\"type\":\"native_try\",\"rule\":{},\"status\":\"{status}\",\"wall\":{wall:.3},\"child_cpu\":{cpu:.3},\"timeout\":{secs},\"msg\":{}}}", rules.len(), q(&msg)).unwrap();
                    qlog.flush().unwrap();
                    eprintln!("NATIVE_TRY {status} after {wall:.1} s (child cpu {cpu:.1} s) on rule {}", rules.len());
                    if let Some(ps) = guard_profile {
                        let inputs = xs_guard_inputs(&candidate, &idx);
                        let mut order: Vec<usize> = (0..inputs.len()).collect();
                        order.sort_by_key(|&k| (inputs[k].nterms(), k));
                        let ppath = format!("{}.guard_profile.jsonl", out_path.trim_end_matches(".table.jsonl"));
                        let mut pf = std::io::BufWriter::new(std::fs::OpenOptions::new().create(true).append(true).open(&ppath).expect("profile file"));
                        let mut touts = 0usize;
                        for k in order {
                            let p = &inputs[k];
                            if touts >= guard_profile_max {
                                writeln!(pf, "{{\"rule\":{},\"input\":{k},\"status\":\"skipped\",{}}}", rules.len(), xs_poly_info(p, &names)).unwrap();
                                continue;
                            }
                            let r = forked(&format!("prof_r{}_{k}", rules.len()), ps, || {
                                let t = Instant::now();
                                let fs = p.factor();
                                let items: Vec<String> = fs.iter().map(|(f, m)| format!("{{\"m\":{m},\"index\":{},{},\"f\":{}}}", xs_contains_index(f, &idx), xs_poly_info(f, &names), q(&f.to_string()))).collect();
                                format!("{{\"seconds\":{:.3},\"factors\":[{}]}}", t.elapsed().as_secs_f64(), items.join(","))
                            });
                            let line = match r {
                                Forked::Done(s, w) => format!("{{\"rule\":{},\"input\":{k},\"status\":\"done\",\"wall\":{w:.3},{},\"result\":{}}}", rules.len(), xs_poly_info(p, &names), s.trim()),
                                Forked::Timeout(w, cpu) => {
                                    touts += 1;
                                    format!("{{\"rule\":{},\"input\":{k},\"status\":\"timeout\",\"wall\":{w:.3},\"child_cpu\":{cpu:.3},{}}}", rules.len(), xs_poly_info(p, &names))
                                }
                                Forked::Failed(m, w) => format!("{{\"rule\":{},\"input\":{k},\"status\":\"failed\",\"wall\":{w:.3},\"msg\":{}}}", rules.len(), q(&m)),
                            };
                            writeln!(pf, "{line}").unwrap();
                            pf.flush().unwrap();
                            eprintln!("GUARD_PROFILE {}", line.chars().take(300).collect::<String>());
                        }
                    }
                    let mut content: Option<ExceptionalConditions> = None;
                    if fallback == "content" {
                        let r = forked(&format!("content_r{}", rules.len()), content_timeout, || {
                            let mut log = Vec::new();
                            let t = Instant::now();
                            let e = xs_content_exceptions(&candidate, &idx, &sector, &names, &mut log);
                            format!("{{\"seconds\":{:.3},\"branches\":{},\"log\":[{}]}}", t.elapsed().as_secs_f64(), xs_branches_json(&e), log.join(","))
                        });
                        match r {
                            Forked::Done(s, w) => {
                                writeln!(qlog, "{{\"type\":\"content_try\",\"rule\":{},\"status\":\"done\",\"wall\":{w:.3},\"child\":{}}}", rules.len(), s.trim()).unwrap();
                                qlog.flush().unwrap();
                                eprintln!("CONTENT_TRY done in {w:.1} s on rule {}", rules.len());
                                let mut log = Vec::new();
                                content = Some(xs_content_exceptions(&candidate, &idx, &sector, &names, &mut log));
                            }
                            Forked::Timeout(w, cpu) => {
                                writeln!(qlog, "{{\"type\":\"content_try\",\"rule\":{},\"status\":\"timeout\",\"wall\":{w:.3},\"child_cpu\":{cpu:.3}}}", rules.len()).unwrap();
                            }
                            Forked::Failed(m, w) => {
                                writeln!(qlog, "{{\"type\":\"content_try\",\"rule\":{},\"status\":\"failed\",\"wall\":{w:.3},\"msg\":{}}}", rules.len(), q(&m)).unwrap();
                            }
                        }
                        qlog.flush().unwrap();
                    }
                    match content {
                        Some(e) => {
                            guard_route = "content";
                            (Ok(e), candidate)
                        }
                        None => {
                            writeln!(qlog, "{{\"type\":\"exception_timeout\",\"rule\":{},\"case\":{},\"rhs\":{},\"sources\":{},\"search_seconds\":{search_seconds:.3},\"timeout\":{secs},\"fallback\":{}}}", rules.len(), case_json(&current), candidate.rhs.len(), candidate.sources.len(), q(&fallback)).unwrap();
                            qlog.flush().unwrap();
                            writeln!(out, "{{\"type\":\"chart_error\",\"chart\":{},\"seconds\":{:.3},\"error\":{}}}", q(&label), t2.elapsed().as_secs_f64(), q(&format!("guard extraction: native_try {status} after {secs} s and no fallback on {current:?}"))).unwrap();
                            writeln!(out, "{{\"type\":\"end\",\"rules\":{},\"exception_timeout\":true,\"vmhwm_kb\":{}}}", rules.len(), vmhwm_kb()).unwrap();
                            out.flush().unwrap();
                            eprintln!("EXCEPTION_TIMEOUT (native_try {status}, no fallback) on rule {}; exiting", rules.len());
                            std::process::exit(7);
                        }
                    }
                }
            }
        } else {
            match exc_timeout {
                None => (extract_exceptions(&candidate, &idx, &sector), candidate),
                Some(secs) => {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let (idx2, sector2) = (idx, sector);
                    let nrhs = candidate.rhs.len();
                    let nsrc = candidate.sources.len();
                    std::thread::spawn(move || {
                        let r = extract_exceptions(&candidate, &idx2, &sector2);
                        let _ = tx.send((r, candidate));
                    });
                    match rx.recv_timeout(std::time::Duration::from_secs(secs)) {
                        Ok(x) => x,
                        Err(_) => {
                            writeln!(qlog, "{{\"type\":\"exception_timeout\",\"rule\":{},\"case\":{},\"rhs\":{nrhs},\"sources\":{nsrc},\"search_seconds\":{search_seconds:.3},\"timeout\":{secs}}}", rules.len(), case_json(&current)).unwrap();
                            qlog.flush().unwrap();
                            writeln!(out, "{{\"type\":\"chart_error\",\"chart\":{},\"seconds\":{:.3},\"error\":{}}}", q(&label), t2.elapsed().as_secs_f64(), q(&format!("exception extraction exceeded {secs} s on {current:?}"))).unwrap();
                            writeln!(out, "{{\"type\":\"end\",\"rules\":{},\"exception_timeout\":true,\"vmhwm_kb\":{}}}", rules.len(), vmhwm_kb()).unwrap();
                            out.flush().unwrap();
                            eprintln!("EXCEPTION_TIMEOUT after {secs} s on rule {} (rhs {nrhs}); exiting", rules.len());
                            std::process::exit(7);
                        }
                    }
                }
            }
        };
        let (extracted, candidate) = extracted;
        let exceptions = match extracted {
            Ok(x) => x,
            Err(e) => {
                error = Some(format!("exception extraction on {current:?}: {e:?}"));
                break;
            }
        };
        let rule = SectorRule { candidate, exceptions, dispatch_policy: Default::default() };
        let index = rules.len();
        terms += write_rule(&mut out, &label, index, &rule);
        out.flush().unwrap();
        branches_total += rule.exceptions.branches.len();
        let branch_strings: Vec<String> = rule.exceptions.branches.iter().map(|b| strings(b.iter())).collect();
        writeln!(
            qlog,
            "{{\"type\":\"rule\",\"index\":{index},\"guard_route\":\"{guard_route}\",\"case\":{},\"rhs\":{},\"sources\":{},\"search_seconds\":{search_seconds:.3},\"exception_seconds\":{:.3},\"branches\":[{}]}}",
            case_json(&rule.candidate.case), rule.candidate.rhs.len(), rule.candidate.sources.len(), te.elapsed().as_secs_f64(), branch_strings.join(",")
        )
        .unwrap();
        eprintln!("SEVENT {:.3}s RuleFound index={index} rhs={} sources={} branches={:?}", t2.elapsed().as_secs_f64(), rule.candidate.rhs.len(), rule.candidate.sources.len(), branch_strings);
        // exceptional children (admit every branch before mutating the queue)
        let mut children: Vec<Case<N>> = Vec::new();
        for (bi, branch) in rule.exceptions.branches.iter().enumerate() {
            if defer_nonlinear && branch.iter().all(|p| total_degree(p) >= 2) {
                deferred_branches += 1;
                let degs: Vec<u32> = branch.iter().map(total_degree).collect();
                writeln!(qlog, "{{\"type\":\"deferred_branch\",\"rule\":{index},\"branch\":{bi},\"polys\":{},\"error\":\"not attempted: every polynomial of this AND branch is nonlinear (total degrees {degs:?}); defer_nonlinear on\"}}", strings(branch.iter())).unwrap();
                continue;
            }
            if let Some(secs) = admit_try.filter(|_| gsplit && branch.iter().any(|p| total_degree(p) >= 2)) {
                out.flush().unwrap();
                qlog.flush().unwrap();
                c.flush().unwrap();
                let case_ref = &rule.candidate.case;
                let r = forked(&format!("admit_r{index}_b{bi}"), secs, || {
                    let t = Instant::now();
                    match case_ref.intersect_many_refined(branch, &idx, &sector, limits, None, UnsupportedBranchPolicy::Defer, CaseRefinement::all()) {
                        Ok(r) => format!("{{\"ok\":true,\"seconds\":{:.3},\"cases\":{},\"deferred\":{}}}", t.elapsed().as_secs_f64(), r.cases.len(), r.deferred.len()),
                        Err(e) => format!("{{\"ok\":false,\"error\":{}}}", q(&format!("{e:?}").chars().take(400).collect::<String>())),
                    }
                });
                let (status, wall) = match &r {
                    Forked::Done(_, w) => ("done", *w),
                    Forked::Timeout(w, _) => ("timeout", *w),
                    Forked::Failed(_, w) => ("failed", *w),
                };
                writeln!(qlog, "{{\"type\":\"admit_try\",\"rule\":{index},\"branch\":{bi},\"status\":\"{status}\",\"wall\":{wall:.3}}}").unwrap();
                qlog.flush().unwrap();
                eprintln!("ADMIT_TRY {status} after {wall:.1} s on rule {index} branch {bi}");
                if !matches!(r, Forked::Done(..)) {
                    // the exact admission did not finish: the branch is a deferred region (never a silent gap); the driver's
                    // R3 classifies its integer points on the rule's case exactly.
                    deferred_regions += 1;
                    writeln!(
                        qlog,
                        "{{\"type\":\"deferred_region\",\"rule\":{index},\"branch\":{bi},\"k\":0,\"reason\":\"AdmissionTimeout({secs} s, {status})\",\"chart\":{},\"conjunction\":{},\"exclusions\":[],\"dependents\":\"[]\",\"checks\":\"[]\"}}",
                        case_json(&rule.candidate.case), strings(branch.iter())
                    )
                    .unwrap();
                    continue;
                }
            }
            let admitted = if gsplit {
                rule.candidate.case.intersect_many_refined(branch, &idx, &sector, limits, None, UnsupportedBranchPolicy::Defer, CaseRefinement::all())
            } else {
                rule.candidate.case.intersect_many(branch, &idx, &sector, limits)
            };
            match admitted {
                Ok(r) => {
                    for (di, d) in r.deferred.iter().enumerate() {
                        deferred_regions += 1;
                        writeln!(
                            qlog,
                            "{{\"type\":\"deferred_region\",\"rule\":{index},\"branch\":{bi},\"k\":{di},\"reason\":{},\"chart\":{},\"conjunction\":{},\"exclusions\":{},\"dependents\":{},\"checks\":{}}}",
                            q(&format!("{:?}", d.reason)), case_json(&d.chart), strings(d.conjunction.iter()), strings(d.exclusions.iter()),
                            q(&format!("{:?}", d.dependents)), q(&format!("{:?}", d.checks))
                        )
                        .unwrap();
                    }
                    if gsplit {
                        writeln!(qlog, "{{\"type\":\"branch_admission\",\"rule\":{index},\"branch\":{bi},\"cases\":{},\"deferred\":{},\"refinement\":{},\"resolutions\":{}}}", r.cases.len(), r.deferred.len(), q(&format!("{:?}", r.refinement)), r.resolutions.len()).unwrap();
                    }
                    lifted += r.stats.lifted_branches;
                    modular_empty += r.stats.modular_empty_branches;
                    if r.cases.is_empty() && r.deferred.is_empty() {
                        discarded += 1;
                        writeln!(qlog, "{{\"type\":\"branch_empty\",\"rule\":{index},\"branch\":{bi},\"polys\":{}}}", strings(branch.iter())).unwrap();
                    }
                    for ch in &r.cases {
                        writeln!(qlog, "{{\"type\":\"child\",\"rule\":{index},\"branch\":{bi},\"case\":{}}}", case_json(ch)).unwrap();
                    }
                    children.extend(r.cases);
                }
                Err(e) => {
                    deferred_branches += 1;
                    writeln!(qlog, "{{\"type\":\"deferred_branch\",\"rule\":{index},\"branch\":{bi},\"polys\":{},\"error\":{}}}", strings(branch.iter()), q(&format!("{e:?}").chars().take(4000).collect::<String>())).unwrap();
                }
            }
        }
        if children.iter().any(|ch| ch == &current) {
            error = Some(format!("NonProgress on {current:?}"));
            rules.push(rule);
            break;
        }
        for ch in children {
            enqueue(ch, Some(index), &mut pending, &mut numerical, &rules, &excludes, &idx, &sector, limits, &mut qlog, &mut discarded, &mut deferred_cases, &covers);
        }
        qlog.flush().unwrap();
        rule_secs.push((index, search_seconds));
        if want_cert && !cert_defer {
            out.flush().unwrap();
            qlog.flush().unwrap();
            c.flush().unwrap();
            let (line, ok, w, p) = xs_cert_one(&solver, &label, index, &rule, cert_timeout, &mut qlog);
            writeln!(c, "{line}").unwrap();
            c.flush().unwrap();
            qlog.flush().unwrap();
            if ok { cert_ok += 1 } else { cert_failed += 1 }
            nweights += w;
            poles_n += usize::from(p);
            eprintln!("CERT rule {index} ok={ok} weights={w}");
        }
        rules.push(rule);
    }
    if let Ok(mut g) = R9.lock() {
        *g = None;
    }
    let mut residual_points: Vec<String> = Vec::new();
    let mut numerical_cases = 0;
    if error.is_none() && !numerical.is_empty() {
        numerical_cases = numerical.len();
        eprintln!("SEVENT {:.3}s NumericalStarted cases={numerical:?}", t2.elapsed().as_secs_f64());
        match solver.solve_numeric_cases(numerical.clone(), SearchOptions { max_depth: Some(2), ..symbolic.clone() }) {
            Ok(result) => {
                for candidate in result.rules {
                    let nl: Vec<String> = candidate.rhs.iter().filter(|t| solver.ordering().compare(&candidate.target, &t.integral) != Ordering::Less).map(|t| t.integral.to_string()).collect();
                    not_lower.extend(nl);
                    let exceptions = extract_exceptions(&candidate, &idx, &sector).expect("numerical exceptions");
                    assert!(exceptions.branches.is_empty(), "a fully fixed rule has no index exceptions");
                    let rule = SectorRule { candidate, exceptions, dispatch_policy: Default::default() };
                    let index = rules.len();
                    terms += write_rule(&mut out, &label, index, &rule);
                    out.flush().unwrap();
                    writeln!(qlog, "{{\"type\":\"rule\",\"index\":{index},\"case\":{},\"rhs\":{},\"sources\":{},\"numerical\":true,\"branches\":[]}}", case_json(&rule.candidate.case), rule.candidate.rhs.len(), rule.candidate.sources.len()).unwrap();
                    if want_cert {
                        let (line, ok, w, p) = cert_line(&solver, &label, index, &rule);
                        writeln!(c, "{line}").unwrap();
                        c.flush().unwrap();
                        if ok { cert_ok += 1 } else { cert_failed += 1 }
                        nweights += w;
                        poles_n += usize::from(p);
                    }
                    rules.push(rule);
                }
                for r in &result.residuals {
                    residual_points.push(r.integral().to_string());
                }
            }
            Err(e) => error = Some(format!("numerical: {e:?}")),
        }
    }
    for key in &residual_points {
        writeln!(out, "{{\"type\":\"residual\",\"chart\":{},\"key\":{}}}", q(&label), q(key)).unwrap();
    }
    let seconds = t2.elapsed().as_secs_f64();
    if let Some(e) = &error {
        writeln!(out, "{{\"type\":\"chart_error\",\"chart\":{},\"seconds\":{seconds:.3},\"error\":{}}}", q(&label), q(&e.chars().take(4000).collect::<String>())).unwrap();
    }
    writeln!(
        out,
        "{{\"type\":\"chart_end\",\"chart\":{},\"seconds\":{seconds:.3},\"rules\":{},\"rhs_terms\":{terms},\"branches\":{branches_total},\"residuals\":{},\"symbolic_cases\":{symbolic_cases},\"numerical_cases\":{numerical_cases},\"discarded_cases\":{discarded},\"case_lifts\":{lifted},\"case_modular_empty\":{modular_empty},\"finite_points_visited\":0,\"max_numerator_rank\":null,\"finite_case_policy\":\"face-queue\",\"deferred_cases\":{deferred_cases},\"deferred_branches\":{deferred_branches},\"deferred_regions\":{deferred_regions}}}",
        q(&label), rules.len(), residual_points.len()
    )
    .unwrap();
    writeln!(
        out,
        "{{\"type\":\"end\",\"charts\":1,\"rules\":{},\"rhs_terms\":{terms},\"residuals\":{},\"chart_errors\":{},\"seconds\":{:.3},\"vmhwm_kb\":{},\"harness\":\"face_queue_gs (round 4, round-2 face queue on the rr-gsplit rlib, refined+deferring admission)\",\"native_descent_not_lower\":{}}}",
        rules.len(), residual_points.len(), usize::from(error.is_some()), t_total.elapsed().as_secs_f64(), vmhwm_kb(), strings(not_lower.iter())
    )
    .unwrap();
    out.flush().unwrap();
    if want_cert && cert_defer {
        writeln!(qlog, "{{\"type\":\"queue_done\",\"rules\":{},\"error\":{},\"seconds\":{seconds:.3}}}", rules.len(), error.as_deref().map_or("null".to_string(), q)).unwrap();
        qlog.flush().unwrap();
        let mut order = rule_secs.clone();
        order.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)));
        for (index, secs) in order {
            eprintln!("CERT_DEFERRED start rule {index} (search {secs:.1} s)");
            let (line, ok, w, p) = xs_cert_one(&solver, &label, index, &rules[index], cert_timeout, &mut qlog);
            writeln!(c, "{line}").unwrap();
            c.flush().unwrap();
            qlog.flush().unwrap();
            if ok { cert_ok += 1 } else { cert_failed += 1 }
            nweights += w;
            poles_n += usize::from(p);
            eprintln!("CERT rule {index} ok={ok} weights={w}");
        }
    }
    writeln!(c, "{{\"type\":\"cert_end\",\"ok\":{cert_ok},\"failed\":{cert_failed},\"skipped\":0,\"weights\":{nweights},\"rules_with_weight_poles\":{poles_n},\"seconds\":{:.3},\"vmhwm_kb\":{}}}", t_total.elapsed().as_secs_f64(), vmhwm_kb()).unwrap();
    c.flush().unwrap();
    writeln!(qlog, "{{\"type\":\"end\",\"error\":{},\"rules\":{},\"symbolic_cases\":{symbolic_cases},\"numerical_cases\":{numerical_cases},\"residuals\":{},\"deferred_cases\":{deferred_cases},\"deferred_branches\":{deferred_branches},\"deferred_regions\":{deferred_regions},\"discarded\":{discarded},\"seconds\":{seconds:.3}}}", error.as_deref().map_or("null".to_string(), q), rules.len(), residual_points.len()).unwrap();
    qlog.flush().unwrap();
    println!("{{\"rules\":{},\"rhs\":{terms},\"residuals\":{},\"deferred_regions\":{deferred_regions},\"deferred_cases\":{deferred_cases},\"deferred_branches\":{deferred_branches},\"not_lower\":{},\"cert_ok\":{cert_ok},\"cert_failed\":{cert_failed},\"error\":{},\"seconds\":{seconds:.3},\"vmhwm_kb\":{}}}", rules.len(), residual_points.len(), not_lower.len(), error.as_deref().map_or("null".to_string(), q), vmhwm_kb());
    i32::from(!not_lower.is_empty() || cert_failed != 0 || error.is_some())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(args.len() >= 5, "usage: face_queue FAMILY OUT.table.jsonl OUT.cert.jsonl CONFIG");
    let family_text = std::fs::read_to_string(&args[1]).expect("read family");
    let fam = Compiler::new(Limits::default())
        .expect("compiler")
        .compile_compact(&family_text, None)
        .expect("compile compact family")
        .into_lowered(LoweringLimits::default())
        .expect("lower family")
        .into_family();
    eprintln!("family {} loops={} externals={} denominators={}", fam.name(), fam.loop_count(), fam.external_count(), fam.denominator_count());
    assert_eq!(fam.denominator_count(), 20, "T211-only frontend arity");
    let code = run::<20>(&args[1], &family_text, &fam, &args[2], &args[3], &args[4]);
    std::process::exit(code);
}
