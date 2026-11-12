//! Automatic choice of ONE order per node: bounded depth-1 probes of order variants at the hanging case, and a
//! supervisor that reruns the whole node under the best variant.
//!
//! Workflow of [`generate_auto`] (all in this process tree, no external driver):
//! 1. attempt `k` runs the whole node under order `O_k` in a forked child with a case-budget watchdog and the lift
//!    pre-screen; the child writes `OUTCOME.json` (or `F.spill.json` and exits 9 when one case exceeds the budget);
//! 2. a complete chart ends the loop; otherwise the case it stopped on (screen refusal, guard timeout or budget) is
//!    mapped to the original slots and every untried variant of [`super::order::variants`] is probed there: a forked
//!    child per variant, `max_depth 1` (then 2 if no variant finds a rule), capped in wall time, over a case PORTFOLIO
//!    (the stuck case, then its one-step neighbours, each its own `max_cases 1` run; [`portfolio_cases`]);
//! 3. the best probe (a rule on the whole portfolio; then the WORST coefficient degree, total RHS terms, lift, rows)
//!    becomes `O_{k+1}` and the WHOLE node is regenerated under it. Rules of different attempts are never joined;
//! 4. the final table is checked for strict descent under its one order ([`super::descent`]).

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::driver::{
    _exit, CaseSpec, NodeOptions, NodeOutcome, RuleScore, StopReason, fork, generate, kill, waitpid,
};
use super::order::{NodeOrder, variants};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeOptions {
    pub cap_seconds: u64,
    pub max_depth: u32,
    /// Depth of the second, separately labelled round when no variant finds a rule at `max_depth`.
    pub fallback_depth: Option<u32>,
    pub max_phys_pairs: usize,
    /// Probe children running at once.
    pub parallel: usize,
    /// Cases per probe: the stuck case, then its NEIGHBOUR cases ([`portfolio_cases`]: each free ISP at -1, each free
    /// propagator dotted to 2, equations kept = the affine dotted boundaries), each searched on its own, so that an
    /// order which only moves the cost to the sub-corners is not chosen (coordinator 02:12Z, Astra T211).
    #[serde(default = "default_portfolio")]
    pub portfolio: usize,
    /// FOLLOW-ON cases per probe (D4, coordinator 09:53Z): after the portfolio, the node continues from the stuck
    /// case for up to this many further queue cases (normal depth, the size budget, a per-case budget), so that
    /// oversized rules on cases OUTSIDE the portfolio (T206 under `id`: 15.9M / 15.3M monomials) count in the
    /// ranking. 0 = off.
    #[serde(default = "default_follow_on")]
    pub follow_on: usize,
    /// AFFINE RUNG (affine-faces, opt-in): when the stuck case has equations, add the case's own boundary-first (R1)
    /// order to the variants and run the depth-1 / displaced-ball oracle ([`super::affine`]) on every variant; if no
    /// variant has a depth-1 relation (an affine face no order reduces at depth 1), skip the futile depth-1 round and
    /// probe at the fallback depth ONLY the variants with a small displaced-ball relation, smallest first.
    #[serde(default)]
    pub affine_rung: bool,
}

fn default_follow_on() -> usize {
    6
}

/// The case portfolio of a stuck case (ORIGINAL slots): the case itself, then for every free slot the neighbour with
/// that slot fixed one step into the chart (ISP n = -1, propagator n = 2), keeping the case's equations; at most
/// `k` cases. Neighbours whose fixed value contradicts an equation are dropped by the driver's empty-domain check.
pub fn portfolio_cases(case: &CaseSpec, physical: &[bool], k: usize) -> Vec<CaseSpec> {
    let mut out = vec![case.clone()];
    let free: Vec<usize> = (0..case.fixed.len())
        .filter(|&i| case.fixed[i].is_none())
        .collect();
    // ISP neighbours first (the T211 sub-corners were one-extra-ISP = -1 faces), then dotted propagators
    for &i in free
        .iter()
        .filter(|&&i| !physical.get(i).copied().unwrap_or(true))
        .chain(
            free.iter()
                .filter(|&&i| physical.get(i).copied().unwrap_or(true)),
        )
    {
        if out.len() >= k.max(1) {
            break;
        }
        let mut c = case.clone();
        c.fixed[i] = Some(if physical.get(i).copied().unwrap_or(true) {
            2
        } else {
            -1
        });
        out.push(c);
    }
    out
}

fn default_portfolio() -> usize {
    4
}

impl Default for ProbeOptions {
    fn default() -> Self {
        Self {
            cap_seconds: 120,
            max_depth: 1,
            fallback_depth: Some(2),
            max_phys_pairs: 10,
            parallel: 1,
            portfolio: 4,
            follow_on: 6,
            affine_rung: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeResult {
    pub order: NodeOrder,
    pub depth: u32,
    /// `rule`, `no_rule` (exhausted / refused / stopped), `timeout`, `failed`.
    pub status: String,
    pub score: Option<RuleScore>,
    pub seconds: f64,
    pub detail: String,
    /// Scores of the portfolio rules (the stuck case first).
    #[serde(default)]
    pub portfolio: Vec<RuleScore>,
    /// The portfolio stopped on a refused lift / exhausted search before `portfolio` cases.
    #[serde(default)]
    pub portfolio_incomplete: bool,
    /// Follow-on run: symbolic rules found, rules refused by the size budget, their largest size (monomials).
    #[serde(default)]
    pub follow_rules: usize,
    #[serde(default)]
    pub follow_oversized: usize,
    #[serde(default)]
    pub follow_max_monomials: usize,
}

impl ProbeResult {
    /// Lexicographic cost (lower is better); `None` without a rule on the stuck case. Over the portfolio: an
    /// incomplete portfolio last, then the number of oversized rules on the stuck case's follow-on cases, then the
    /// WORST coefficient degree, total RHS terms, total lift time, total rows.
    pub fn cost(&self) -> Option<(bool, usize, usize, usize, u64, usize)> {
        let first = self.score.as_ref().filter(|_| self.status == "rule")?;
        let all: Vec<&RuleScore> = if self.portfolio.is_empty() {
            vec![first]
        } else {
            self.portfolio.iter().collect()
        };
        Some((
            self.portfolio_incomplete,
            self.follow_oversized,
            all.iter().map(|s| s.max_degree).max().unwrap_or(0),
            all.iter().map(|s| s.rhs_terms).sum(),
            all.iter().map(|s| (s.lift_seconds * 1000.0) as u64).sum(),
            all.iter().map(|s| s.trace_rows).sum(),
        ))
    }
}

/// The best probe with a rule (see [`ProbeResult::cost`]); ties keep the earlier variant.
pub fn best(probes: &[ProbeResult]) -> Option<&ProbeResult> {
    probes
        .iter()
        .filter(|p| p.cost().is_some())
        .min_by_key(|p| p.cost().unwrap())
}

struct Child {
    pid: i32,
    started: Instant,
    index: usize,
}

/// Run `jobs` (each writes its own result file) in forked children, at most `parallel` at once, each killed after
/// `cap` seconds. Returns per job: Some(seconds) if it exited by itself, None on timeout.
fn run_children(
    jobs: Vec<Box<dyn FnOnce() + '_>>,
    parallel: usize,
    cap: Option<u64>,
) -> Vec<(Option<f64>, i32)> {
    let mut results: Vec<(Option<f64>, i32)> = vec![(None, -1); jobs.len()];
    let mut queue: Vec<(usize, Box<dyn FnOnce() + '_>)> = jobs.into_iter().enumerate().collect();
    queue.reverse();
    let mut running: Vec<Child> = Vec::new();
    loop {
        while running.len() < parallel.max(1) {
            let Some((index, job)) = queue.pop() else {
                break;
            };
            // SAFETY: the supervisor holds no locks needed by the child; the child runs the job and _exits.
            let pid = unsafe { fork() };
            if pid == 0 {
                let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).is_ok();
                // SAFETY: leave the forked child without the parent's destructors.
                unsafe { _exit(if ok { 0 } else { 3 }) }
            }
            if pid < 0 {
                results[index] = (None, -2);
                continue;
            }
            running.push(Child {
                pid,
                started: Instant::now(),
                index,
            });
        }
        if running.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        running.retain(|c| {
            let mut status = 0i32;
            // SAFETY: own child, WNOHANG.
            let r = unsafe { waitpid(c.pid, &mut status, 1) };
            if r == c.pid {
                results[c.index] = (
                    Some(c.started.elapsed().as_secs_f64()),
                    (status >> 8) & 0xff,
                );
                return false;
            }
            if cap.is_some_and(|cap| c.started.elapsed().as_secs() >= cap) {
                // SAFETY: own child.
                unsafe {
                    kill(c.pid, 9);
                    waitpid(c.pid, &mut status, 0);
                }
                results[c.index] = (None, 137);
                return false;
            }
            true
        });
    }
    results
}

fn read_outcome(dir: &Path) -> Option<NodeOutcome> {
    serde_json::from_str(&std::fs::read_to_string(dir.join("OUTCOME.json")).ok()?).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// Follow-on statistics from a (possibly capped) run's queue log: symbolic rules, oversized refusals, largest size.
fn follow_on_stats(dir: &Path) -> (usize, usize, usize) {
    let Ok(text) = std::fs::read_to_string(dir.join("F.queue.jsonl")) else {
        return (0, 0, 0);
    };
    let (mut rules, mut over, mut max) = (0usize, 0usize, 0usize);
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match v["type"].as_str() {
            Some("rule") if v["numerical"].as_bool() != Some(true) => rules += 1,
            Some("rule_oversized") => {
                over += 1;
                max = max.max(v["monomials"].as_u64().unwrap_or(0) as usize);
            }
            _ => {}
        }
    }
    (rules, over, max)
}

/// Probe `orders` at `case` (ORIGINAL slots) with depth `depth`; one forked child per order.
#[allow(clippy::too_many_arguments)]
pub fn probe(
    family_text: &str,
    family_path: &str,
    case: &CaseSpec,
    orders: &[NodeOrder],
    base: &NodeOptions,
    popts: &ProbeOptions,
    depth: u32,
    work: &Path,
) -> Vec<ProbeResult> {
    let dirs: Vec<PathBuf> = orders
        .iter()
        .map(|o| work.join(format!("d{depth}-{}", o.name)))
        .collect();
    let jobs: Vec<Box<dyn FnOnce() + '_>> = orders
        .iter()
        .zip(&dirs)
        .map(|(order, dir)| {
            let opts = NodeOptions {
                order: order.clone(),
                initial: vec![case.clone()],
                max_cases: Some(1),
                max_depth: Some(depth),
                screen: None,
                retry_source_orders: Vec::new(),
                case_budget_exit: None,
                cover_tables: Vec::new(),
                cand_dump: false,
                ..base.clone()
            };
            let dir = dir.clone();
            let cases = portfolio_cases(
                case,
                &super::order::physical_slots(family_text),
                popts.portfolio,
            );
            let follow = NodeOptions {
                initial: vec![case.clone()],
                max_cases: Some(1 + popts.follow_on),
                max_depth: base.max_depth,
                case_budget_exit: Some((popts.cap_seconds / 4).max(10)),
                ..opts.clone()
            };
            let follow_on = popts.follow_on;
            Box::new(move || {
                let _ = std::fs::create_dir_all(&dir);
                // the stuck case first (its OUTCOME.json is the probe's outcome), then each neighbour in its own
                // subdirectory (pK/OUTCOME.json); a neighbour run stops the portfolio when it fails
                for (k, c) in cases.iter().enumerate() {
                    let sub = if k == 0 {
                        dir.clone()
                    } else {
                        dir.join(format!("p{k}"))
                    };
                    let _ = std::fs::create_dir_all(&sub);
                    let o = NodeOptions {
                        initial: vec![c.clone()],
                        ..opts.clone()
                    };
                    let result = generate(family_text, family_path, &sub, &o, &mut |_| {});
                    let ok = matches!(&result, Ok(out) if out.first_rule.is_some() || out.complete);
                    let _ = match result {
                        Ok(o) => write_json(&sub.join("OUTCOME.json"), &o),
                        Err(e) => {
                            std::fs::write(sub.join("ERROR.txt"), e).map_err(|e| e.to_string())
                        }
                    };
                    if !ok {
                        break;
                    }
                }
                // follow-on cases from the stuck case (last: a case-budget exit only ends this part); the queue log
                // is read even when the child is capped
                if follow_on > 0 {
                    let f = dir.join("f");
                    let _ = std::fs::create_dir_all(&f);
                    if let Ok(o) = generate(family_text, family_path, &f, &follow, &mut |_| {}) {
                        let _ = write_json(&f.join("OUTCOME.json"), &o);
                    }
                }
            }) as Box<dyn FnOnce() + '_>
        })
        .collect();
    let runs = run_children(jobs, popts.parallel, Some(popts.cap_seconds));
    orders
        .iter()
        .zip(&dirs)
        .zip(runs)
        .map(|((order, dir), (secs, code))| {
            let outcome = read_outcome(dir);
            let (status, score, detail) = match (&secs, &outcome) {
                // the stuck case's run finished (a cap during the portfolio / follow-on only leaves those incomplete)
                (_, Some(o)) if o.first_rule.is_some() => (
                    "rule".to_string(),
                    o.first_rule.clone(),
                    format!("rules {}, stop {:?}", o.rules, o.stop)
                        .chars()
                        .take(400)
                        .collect(),
                ),
                (None, _) => (
                    "timeout".to_string(),
                    None,
                    format!("capped at {} s", popts.cap_seconds),
                ),
                (Some(_), Some(o)) => (
                    "no_rule".to_string(),
                    None,
                    format!("{:?}", o.stop).chars().take(400).collect(),
                ),
                (Some(_), None) => (
                    "failed".to_string(),
                    None,
                    std::fs::read_to_string(dir.join("ERROR.txt"))
                        .unwrap_or_else(|_| format!("exit {code}")),
                ),
            };
            // portfolio: the stuck case's first rule, then the first rule of each neighbour run; a neighbour without a
            // rule (no outcome = capped, or stopped without one) makes the portfolio incomplete
            let wanted = portfolio_cases(
                case,
                &super::order::physical_slots(family_text),
                popts.portfolio,
            )
            .len();
            let mut portfolio: Vec<RuleScore> = outcome
                .as_ref()
                .and_then(|o| o.first_rule.clone())
                .into_iter()
                .collect();
            let mut portfolio_incomplete = outcome
                .as_ref()
                .is_none_or(|o| o.first_rule.is_none() && !o.complete);
            for k in 1..wanted {
                match read_outcome(&dir.join(format!("p{k}"))) {
                    Some(o) if o.first_rule.is_some() => {
                        portfolio.push(o.first_rule.clone().unwrap())
                    }
                    Some(o) if o.complete => {} // empty domain / covered: costs nothing
                    _ => {
                        portfolio_incomplete = true;
                        break;
                    }
                }
            }
            let (follow_rules, follow_oversized, follow_max_monomials) =
                follow_on_stats(&dir.join("f"));
            ProbeResult {
                order: order.clone(),
                depth,
                status,
                score,
                seconds: secs.unwrap_or(popts.cap_seconds as f64),
                detail,
                portfolio,
                portfolio_incomplete,
                follow_rules,
                follow_oversized,
                follow_max_monomials,
            }
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutoOptions {
    pub max_attempts: usize,
    /// Case budget of each attempt (watchdog, seconds).
    pub case_budget: u64,
    /// Wall cap of each attempt (seconds); `None` = unlimited.
    pub attempt_cap: Option<u64>,
    pub probe: ProbeOptions,
}

impl Default for AutoOptions {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            case_budget: 900,
            attempt_cap: None,
            probe: ProbeOptions::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attempt {
    pub order: NodeOrder,
    pub dir: String,
    pub outcome: Option<NodeOutcome>,
    /// Case the attempt stopped on, ORIGINAL slots.
    pub stuck_case: Option<CaseSpec>,
    pub stuck_reason: Option<String>,
    pub seconds: f64,
    pub probes: Vec<ProbeResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutoOutcome {
    pub complete: bool,
    pub order: Option<NodeOrder>,
    pub table: Option<String>,
    pub attempts: Vec<Attempt>,
    pub descent_rules: usize,
    pub descent_terms: usize,
    pub descent_not_lower: usize,
    pub seconds: f64,
    pub failure: Option<String>,
}

fn order_key(o: &NodeOrder, n: usize) -> (Vec<usize>, Option<Vec<usize>>, String) {
    (
        o.new_to_old(n),
        o.coordinate_priority.clone(),
        o.source_order.label(),
    )
}

/// The affine rung (see [`ProbeOptions::affine_rung`]): returns the candidates to probe and whether the depth-1
/// round is futile. Oracle failures leave the candidates unchanged.
fn affine_rung(
    family_text: &str,
    stuck: &CaseSpec,
    mut candidates: Vec<NodeOrder>,
    tried: &[(Vec<usize>, Option<Vec<usize>>, String)],
    n: usize,
    log: &mut dyn FnMut(&str),
) -> (Vec<NodeOrder>, bool) {
    use super::affine::AffinePrepared;
    use super::predict::{OracleFamily, WeightedCase, bfi_case};
    let fam = match OracleFamily::parse(family_text) {
        Ok(f) => f,
        Err(e) => {
            log(&format!("affine rung: oracle family: {e}"));
            return (candidates, false);
        }
    };
    let prep = match AffinePrepared::new(&fam, &stuck.fixed, &stuck.equations, 1) {
        Ok(p) => p,
        Err(e) => {
            log(&format!("affine rung: oracle case: {e}"));
            return (candidates, false);
        }
    };
    let wc = WeightedCase {
        fixed: stuck.fixed.clone(),
        equations: stuck.equations.clone(),
        bytes: 0,
    };
    let r1 = NodeOrder {
        name: "r1_case".into(),
        coordinate_priority: Some(bfi_case(&wc, &fam.phys, &(0..n).collect::<Vec<_>>())),
        ..NodeOrder::identity()
    };
    if !tried.contains(&order_key(&r1, n))
        && !candidates
            .iter()
            .any(|c| order_key(c, n) == order_key(&r1, n))
    {
        candidates.insert(0, r1);
    }
    let prio = |o: &NodeOrder| -> Vec<usize> {
        o.slot_permutation
            .clone()
            .or_else(|| o.coordinate_priority.clone())
            .unwrap_or_else(|| (0..n).collect())
    };
    let diag: Vec<_> = candidates.iter().map(|o| prep.diagnose(&prio(o))).collect();
    for (o, d) in candidates.iter().zip(&diag) {
        log(&format!(
            "  affine oracle {}: depth1 {:?}, displaced-ball best {:?}",
            o.name,
            (d.depth1.rule, d.depth1.trace, d.depth1.rhs),
            d.best.map(|b| (
                &d.shifted[b].shift,
                d.shifted[b].result.trace,
                d.shifted[b].result.rhs
            ))
        ));
    }
    if diag.iter().any(|d| d.depth1.rule) {
        return (candidates, false);
    }
    let mut keep: Vec<(usize, NodeOrder)> = candidates
        .iter()
        .cloned()
        .zip(&diag)
        .filter(|(_, d)| d.small())
        .map(|(o, d)| {
            let s = d.smallest().map_or(usize::MAX, |r| r.trace + r.rhs);
            (s, o)
        })
        .collect();
    keep.sort_by_key(|(s, _)| *s);
    if keep.is_empty() {
        log("affine rung: no variant has a small displaced-ball relation; ordinary rounds");
        return (candidates, false);
    }
    (keep.into_iter().map(|(_, o)| o).collect(), true)
}

/// See the module documentation.
pub fn generate_auto(
    family_text: &str,
    family_path: &str,
    out_dir: &Path,
    base: &NodeOptions,
    auto: &AutoOptions,
    log: &mut dyn FnMut(&str),
) -> Result<AutoOutcome, String> {
    let t0 = Instant::now();
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let n = super::order::family_props(family_text).len();
    let mut order = base.order.clone();
    let mut tried: Vec<(Vec<usize>, Option<Vec<usize>>, String)> = vec![order_key(&order, n)];
    let mut result = AutoOutcome {
        complete: false,
        order: None,
        table: None,
        attempts: Vec::new(),
        descent_rules: 0,
        descent_terms: 0,
        descent_not_lower: 0,
        seconds: 0.0,
        failure: None,
    };
    for k in 0..auto.max_attempts {
        let dir = out_dir.join(format!("attempt{k}-{}", order.name));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let opts = NodeOptions {
            order: order.clone(),
            case_budget_exit: Some(auto.case_budget),
            ..base.clone()
        };
        log(&format!(
            "attempt {k}: order {} -> {}",
            order.to_json(),
            dir.display()
        ));
        let ta = Instant::now();
        let d2 = dir.clone();
        let job: Box<dyn FnOnce() + '_> = Box::new(move || {
            let mut events = std::fs::File::create(d2.join("EVENTS.jsonl")).ok();
            let result = generate(family_text, family_path, &d2, &opts, &mut |e| {
                if let Some(f) = events.as_mut() {
                    use std::io::Write as _;
                    let _ = writeln!(f, "{}", serde_json::to_string(e).unwrap_or_default());
                }
            });
            let _ = match result {
                Ok(o) => write_json(&d2.join("OUTCOME.json"), &o),
                Err(e) => std::fs::write(d2.join("ERROR.txt"), e).map_err(|e| e.to_string()),
            };
        });
        let (secs, code) = run_children(vec![job], 1, auto.attempt_cap)[0];
        let outcome = read_outcome(&dir);
        let mut attempt = Attempt {
            order: order.clone(),
            dir: dir.display().to_string(),
            outcome: outcome.clone(),
            stuck_case: None,
            stuck_reason: None,
            seconds: ta.elapsed().as_secs_f64(),
            probes: Vec::new(),
        };
        // where did it stop?
        let presented_stuck: Option<(CaseSpec, String)> = match &outcome {
            Some(o) if o.complete => None,
            Some(o) => match &o.stop {
                Some(StopReason::LiftRefused { case, reports }) => Some((
                    case.clone(),
                    format!("lift refused ({} screened)", reports.len()),
                )),
                Some(StopReason::GuardTimeout { case, rhs }) => Some((
                    case.clone(),
                    format!("guard extraction timeout ({rhs} RHS)"),
                )),
                Some(other) => {
                    attempt.stuck_reason = Some(format!("{other:?}").chars().take(600).collect());
                    None
                }
                None => {
                    attempt.stuck_reason = Some(format!("not lower: {:?}", o.not_lower.first()));
                    None
                }
            },
            None => {
                let spill: Option<serde_json::Value> =
                    std::fs::read_to_string(dir.join("F.spill.json"))
                        .ok()
                        .and_then(|t| serde_json::from_str(&t).ok());
                match spill {
                    Some(s) => serde_json::from_value::<CaseSpec>(s["current"].clone())
                        .ok()
                        .map(|c| (c, format!("case budget {} s exceeded", auto.case_budget))),
                    None => {
                        attempt.stuck_reason = Some(format!(
                            "attempt ended without outcome (exit {code}, {}): {}",
                            secs.map_or("capped".to_string(), |s| format!("{s:.0} s")),
                            std::fs::read_to_string(dir.join("ERROR.txt")).unwrap_or_default()
                        ));
                        None
                    }
                }
            }
        };
        if let Some(o) = outcome.as_ref().filter(|o| o.complete) {
            log(&format!(
                "attempt {k}: COMPLETE, {} rules, {} residuals, {:.0} s",
                o.rules, o.residuals, attempt.seconds
            ));
            result.attempts.push(attempt);
            result.complete = true;
            result.order = Some(order.clone());
            result.table = Some(o.table.clone());
            break;
        }
        let Some((pcase, reason)) = presented_stuck else {
            log(&format!(
                "attempt {k}: failed without a stuck case: {:?}",
                attempt.stuck_reason
            ));
            result.failure = attempt.stuck_reason.clone();
            result.attempts.push(attempt);
            break;
        };
        let (fixed, equations) = order.original_case(&pcase.fixed, &pcase.equations);
        let stuck = CaseSpec { fixed, equations };
        log(&format!(
            "attempt {k}: stuck ({reason}) on original case {}",
            serde_json::to_string(&stuck).unwrap()
        ));
        attempt.stuck_case = Some(stuck.clone());
        attempt.stuck_reason = Some(reason);
        let candidates: Vec<NodeOrder> = variants(
            family_text,
            &stuck.fixed,
            &stuck.equations,
            auto.probe.max_phys_pairs,
        )
        .into_iter()
        .filter(|o| !tried.contains(&order_key(o, n)))
        .collect();
        let work = dir.join("probes");
        let _ = std::fs::create_dir_all(&work);
        let (candidates, skip_depth1) = if auto.probe.affine_rung && !stuck.equations.is_empty() {
            affine_rung(family_text, &stuck, candidates, &tried, n, log)
        } else {
            (candidates, false)
        };
        if skip_depth1 {
            if let Some(d) = auto.probe.fallback_depth {
                log(&format!(
                    "attempt {k}: affine rung: no variant has a depth-1 relation; depth-{d} probes of {} variants with a small displaced-ball relation",
                    candidates.len()
                ));
            }
        }
        let mut probes = if skip_depth1 && auto.probe.fallback_depth.is_some() {
            Vec::new()
        } else {
            probe(
                family_text,
                family_path,
                &stuck,
                &candidates,
                base,
                &auto.probe,
                auto.probe.max_depth,
                &work,
            )
        };
        if best(&probes).is_none() {
            if let Some(d) = auto.probe.fallback_depth {
                log(&format!(
                    "attempt {k}: no variant found a rule at depth {}; depth {d} round",
                    auto.probe.max_depth
                ));
                // affine rung: the depth-d round searches affine cases coupled-axis-first on depth-d shells
                let affine_base;
                let round_base = if skip_depth1 && base.case_seed_schedule.is_none() {
                    affine_base = NodeOptions {
                        case_seed_schedule: Some(crate::solver::CaseSeedSchedule::ShiftShells {
                            first_depth: d,
                            last_depth: d,
                            axis_priority: (0..n).collect(),
                            positive_first: false,
                        }),
                        case_seed_priority: Some(
                            super::driver::CaseSeedPriority::AffineCoupledFirst,
                        ),
                        ..base.clone()
                    };
                    &affine_base
                } else {
                    base
                };
                probes.extend(probe(
                    family_text,
                    family_path,
                    &stuck,
                    &candidates,
                    round_base,
                    &auto.probe,
                    d,
                    &work,
                ));
            }
        }
        for p in &probes {
            log(&format!(
                "  probe {} d{}: {} {:?} {:.1} s; follow-on {} rules, {} oversized (max {})",
                p.order.name,
                p.depth,
                p.status,
                p.score
                    .as_ref()
                    .map(|s| (s.max_degree, s.rhs_terms, s.trace_rows)),
                p.seconds,
                p.follow_rules,
                p.follow_oversized,
                p.follow_max_monomials
            ));
        }
        let chosen = best(&probes).map(|p| p.order.clone());
        write_json(&work.join("PROBES.json"), &probes)?;
        attempt.probes = probes;
        result.attempts.push(attempt);
        match chosen {
            Some(next) => {
                log(&format!("attempt {k}: next order {}", next.name));
                tried.push(order_key(&next, n));
                order = next;
            }
            None => {
                result.failure = Some("no order variant found a rule at the stuck case".into());
                break;
            }
        }
    }
    if !result.complete && result.failure.is_none() {
        result.failure = Some(format!("not complete after {} attempts", auto.max_attempts));
    }
    if let (Some(table), Some(o)) = (&result.table, &result.order) {
        let (_, report) = super::descent::check_table_file(table, Some(o.clone()))?;
        result.descent_rules = report.rules;
        result.descent_terms = report.terms;
        result.descent_not_lower = report.not_lower.len();
        if !report.passed() {
            result.complete = false;
            result.failure = Some(format!(
                "final table fails strict descent under its order: {:?}",
                report.not_lower.first()
            ));
        }
    }
    result.seconds = t0.elapsed().as_secs_f64();
    write_json(&out_dir.join("NODE.json"), &result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr(name: &str, status: &str, deg: usize, rhs: usize, rows: usize) -> ProbeResult {
        ProbeResult {
            order: NodeOrder {
                name: name.into(),
                ..NodeOrder::identity()
            },
            depth: 1,
            status: status.into(),
            score: Some(RuleScore {
                max_degree: deg,
                rhs_terms: rhs,
                trace_rows: rows,
                ..Default::default()
            }),
            seconds: 1.0,
            detail: String::new(),
            portfolio: Vec::new(),
            portfolio_incomplete: false,
            follow_rules: 0,
            follow_oversized: 0,
            follow_max_monomials: 0,
        }
    }

    #[test]
    fn oversized_follow_on_rules_rank_an_order_down() {
        // T206 under id: a cheap stuck-case rule, but the next queue cases lift 15.9M-monomial rules
        let mut cheap = pr("id", "rule", 2, 30, 100);
        cheap.follow_oversized = 2;
        cheap.follow_max_monomials = 15_900_000;
        let moderate = pr("phys_swap_1_7", "rule", 5, 400, 300);
        assert_eq!(
            best(&[cheap, moderate]).unwrap().order.name,
            "phys_swap_1_7"
        );
    }

    #[test]
    fn follow_on_stats_read_a_capped_queue_log() {
        let dir = std::env::temp_dir().join(format!("rr_auto_follow_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("F.queue.jsonl"),
            "{\"type\":\"rule\",\"index\":0}\n{\"type\":\"rule_oversized\",\"monomials\":5000000}\n{\"type\":\"rule\",\"index\":1,\"numerical\":true}\n{\"type\":\"rule_oversized\",\"monomials\":15900000}\n{\"type\":\"rule\",\"ind",
        )
        .unwrap();
        assert_eq!(follow_on_stats(&dir), (1, 2, 15_900_000));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn portfolio_is_the_case_and_its_neighbours() {
        let case = CaseSpec {
            fixed: vec![Some(1), None, None, Some(0), None],
            equations: vec!["n1+n4+2".into()],
        };
        let phys = [true, true, false, false, false];
        let p = portfolio_cases(&case, &phys, 4);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], case);
        // ISPs first: slot 2 -> -1, slot 4 -> -1, then the propagator slot 1 -> 2; equations kept
        assert_eq!(p[1].fixed, vec![Some(1), None, Some(-1), Some(0), None]);
        assert_eq!(p[2].fixed, vec![Some(1), None, None, Some(0), Some(-1)]);
        assert_eq!(p[3].fixed, vec![Some(1), Some(2), None, Some(0), None]);
        assert!(p.iter().all(|c| c.equations == case.equations));
        assert_eq!(portfolio_cases(&case, &phys, 1), vec![case.clone()]);
    }

    #[test]
    fn ranking_prefers_low_degree_then_rhs_and_ignores_failures() {
        let probes = vec![
            pr("a", "timeout", 0, 0, 0),
            pr("b", "rule", 6, 390, 195),
            pr("c", "rule", 2, 5, 4),
            pr("d", "rule", 2, 9, 1),
        ];
        assert_eq!(best(&probes).unwrap().order.name, "c");
        assert!(best(&probes[..1]).is_none());
        // portfolio: a cheap root whose sub-corners swell loses against a uniformly moderate order
        let mut cheap_root = pr("root", "rule", 1, 10, 4);
        cheap_root.portfolio = vec![
            RuleScore {
                max_degree: 1,
                rhs_terms: 10,
                ..Default::default()
            },
            RuleScore {
                max_degree: 15,
                rhs_terms: 1550,
                ..Default::default()
            },
        ];
        let mut even = pr("even", "rule", 3, 40, 20);
        even.portfolio = vec![
            RuleScore {
                max_degree: 3,
                rhs_terms: 40,
                ..Default::default()
            },
            RuleScore {
                max_degree: 4,
                rhs_terms: 60,
                ..Default::default()
            },
        ];
        assert_eq!(
            best(&[cheap_root.clone(), even.clone()])
                .unwrap()
                .order
                .name,
            "even"
        );
        let mut stopped = even;
        stopped.portfolio_incomplete = true;
        assert_eq!(best(&[cheap_root, stopped]).unwrap().order.name, "root");
    }
}
