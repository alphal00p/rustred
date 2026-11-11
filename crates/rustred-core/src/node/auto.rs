//! Automatic choice of ONE order per node: bounded depth-1 probes of order variants at the hanging case, and a
//! supervisor that reruns the whole node under the best variant.
//!
//! Workflow of [`generate_auto`] (all in this process tree, no external driver):
//! 1. attempt `k` runs the whole node under order `O_k` in a forked child with a case-budget watchdog and the lift
//!    pre-screen; the child writes `OUTCOME.json` (or `F.spill.json` and exits 9 when one case exceeds the budget);
//! 2. a complete chart ends the loop; otherwise the case it stopped on (screen refusal, guard timeout or budget) is
//!    mapped to the original slots and every untried variant of [`super::order::variants`] is probed there: a forked
//!    child per variant, `max_cases 1`, `max_depth 1` (then 2 if no variant finds a rule), capped in wall time;
//! 3. the best probe (a rule found; then lowest coefficient degree, fewest RHS terms, shortest lift, fewest trace rows)
//!    becomes `O_{k+1}` and the WHOLE node is regenerated under it. Rules of different attempts are never joined;
//! 4. the final table is checked for strict descent under its one order ([`super::descent`]).

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::driver::{CaseSpec, NodeOptions, NodeOutcome, RuleScore, StopReason, _exit, fork, generate, kill, waitpid};
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
    /// Cases per probe: the stuck case and the next cases its queue meets (its exceptional children), so that an
    /// order which only moves the cost to the sub-corners is not chosen (coordinator 02:12Z, Astra T211).
    #[serde(default = "default_portfolio")]
    pub portfolio: usize,
}

fn default_portfolio() -> usize {
    4
}

impl Default for ProbeOptions {
    fn default() -> Self {
        Self { cap_seconds: 120, max_depth: 1, fallback_depth: Some(2), max_phys_pairs: 10, parallel: 1, portfolio: 4 }
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
}

impl ProbeResult {
    /// Lexicographic cost (lower is better); `None` without a rule on the stuck case. Over the portfolio: an
    /// incomplete portfolio last, then the WORST coefficient degree, total RHS terms, total lift time, total rows.
    pub fn cost(&self) -> Option<(bool, usize, usize, u64, usize)> {
        let first = self.score.as_ref().filter(|_| self.status == "rule")?;
        let all: Vec<&RuleScore> = if self.portfolio.is_empty() { vec![first] } else { self.portfolio.iter().collect() };
        Some((
            self.portfolio_incomplete,
            all.iter().map(|s| s.max_degree).max().unwrap_or(0),
            all.iter().map(|s| s.rhs_terms).sum(),
            all.iter().map(|s| (s.lift_seconds * 1000.0) as u64).sum(),
            all.iter().map(|s| s.trace_rows).sum(),
        ))
    }
}

/// The best probe with a rule (see [`ProbeResult::cost`]); ties keep the earlier variant.
pub fn best(probes: &[ProbeResult]) -> Option<&ProbeResult> {
    probes.iter().filter(|p| p.cost().is_some()).min_by_key(|p| p.cost().unwrap())
}

struct Child {
    pid: i32,
    started: Instant,
    index: usize,
}

/// Run `jobs` (each writes its own result file) in forked children, at most `parallel` at once, each killed after
/// `cap` seconds. Returns per job: Some(seconds) if it exited by itself, None on timeout.
fn run_children(jobs: Vec<Box<dyn FnOnce() + '_>>, parallel: usize, cap: Option<u64>) -> Vec<(Option<f64>, i32)> {
    let mut results: Vec<(Option<f64>, i32)> = vec![(None, -1); jobs.len()];
    let mut queue: Vec<(usize, Box<dyn FnOnce() + '_>)> = jobs.into_iter().enumerate().collect();
    queue.reverse();
    let mut running: Vec<Child> = Vec::new();
    loop {
        while running.len() < parallel.max(1) {
            let Some((index, job)) = queue.pop() else { break };
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
            running.push(Child { pid, started: Instant::now(), index });
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
                results[c.index] = (Some(c.started.elapsed().as_secs_f64()), (status >> 8) & 0xff);
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
    std::fs::write(path, serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

/// Probe `orders` at `case` (ORIGINAL slots) with depth `depth`; one forked child per order.
pub fn probe(family_text: &str, family_path: &str, case: &CaseSpec, orders: &[NodeOrder], base: &NodeOptions, popts: &ProbeOptions, depth: u32, work: &Path) -> Vec<ProbeResult> {
    let dirs: Vec<PathBuf> = orders.iter().map(|o| work.join(format!("d{depth}-{}", o.name))).collect();
    let jobs: Vec<Box<dyn FnOnce() + '_>> = orders
        .iter()
        .zip(&dirs)
        .map(|(order, dir)| {
            let opts = NodeOptions {
                order: order.clone(),
                initial: vec![case.clone()],
                max_cases: Some(popts.portfolio.max(1)),
                max_depth: Some(depth),
                screen: None,
                retry_source_orders: Vec::new(),
                case_budget_exit: None,
                cover_tables: Vec::new(),
                cand_dump: false,
                ..base.clone()
            };
            let dir = dir.clone();
            Box::new(move || {
                let _ = std::fs::create_dir_all(&dir);
                let result = generate(family_text, family_path, &dir, &opts, &mut |_| {});
                let _ = match result {
                    Ok(o) => write_json(&dir.join("OUTCOME.json"), &o),
                    Err(e) => std::fs::write(dir.join("ERROR.txt"), e).map_err(|e| e.to_string()),
                };
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
                (None, _) => ("timeout".to_string(), None, format!("capped at {} s", popts.cap_seconds)),
                (Some(_), Some(o)) if o.first_rule.is_some() => ("rule".to_string(), o.first_rule.clone(), format!("rules {}, stop {:?}", o.rules, o.stop).chars().take(400).collect()),
                (Some(_), Some(o)) => ("no_rule".to_string(), None, format!("{:?}", o.stop).chars().take(400).collect()),
                (Some(_), None) => ("failed".to_string(), None, std::fs::read_to_string(dir.join("ERROR.txt")).unwrap_or_else(|_| format!("exit {code}"))),
            };
            let portfolio = outcome.as_ref().map(|o| o.rule_scores.clone()).unwrap_or_default();
            let portfolio_incomplete = outcome.as_ref().is_some_and(|o| !o.complete && !matches!(o.stop, Some(StopReason::MaxCases { .. })));
            ProbeResult { order: order.clone(), depth, status, score, seconds: secs.unwrap_or(popts.cap_seconds as f64), detail, portfolio, portfolio_incomplete }
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
        Self { max_attempts: 4, case_budget: 900, attempt_cap: None, probe: ProbeOptions::default() }
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
    (o.new_to_old(n), o.coordinate_priority.clone(), o.source_order.label())
}

/// See the module documentation.
pub fn generate_auto(family_text: &str, family_path: &str, out_dir: &Path, base: &NodeOptions, auto: &AutoOptions, log: &mut dyn FnMut(&str)) -> Result<AutoOutcome, String> {
    let t0 = Instant::now();
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let n = super::order::family_props(family_text).len();
    let mut order = base.order.clone();
    let mut tried: Vec<(Vec<usize>, Option<Vec<usize>>, String)> = vec![order_key(&order, n)];
    let mut result = AutoOutcome { complete: false, order: None, table: None, attempts: Vec::new(), descent_rules: 0, descent_terms: 0, descent_not_lower: 0, seconds: 0.0, failure: None };
    for k in 0..auto.max_attempts {
        let dir = out_dir.join(format!("attempt{k}-{}", order.name));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let opts = NodeOptions { order: order.clone(), case_budget_exit: Some(auto.case_budget), ..base.clone() };
        log(&format!("attempt {k}: order {} -> {}", order.to_json(), dir.display()));
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
        let mut attempt = Attempt { order: order.clone(), dir: dir.display().to_string(), outcome: outcome.clone(), stuck_case: None, stuck_reason: None, seconds: ta.elapsed().as_secs_f64(), probes: Vec::new() };
        // where did it stop?
        let presented_stuck: Option<(CaseSpec, String)> = match &outcome {
            Some(o) if o.complete => None,
            Some(o) => match &o.stop {
                Some(StopReason::LiftRefused { case, reports }) => Some((case.clone(), format!("lift refused ({} screened)", reports.len()))),
                Some(StopReason::GuardTimeout { case, rhs }) => Some((case.clone(), format!("guard extraction timeout ({rhs} RHS)"))),
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
                let spill: Option<serde_json::Value> = std::fs::read_to_string(dir.join("F.spill.json")).ok().and_then(|t| serde_json::from_str(&t).ok());
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
            log(&format!("attempt {k}: COMPLETE, {} rules, {} residuals, {:.0} s", o.rules, o.residuals, attempt.seconds));
            result.attempts.push(attempt);
            result.complete = true;
            result.order = Some(order.clone());
            result.table = Some(o.table.clone());
            break;
        }
        let Some((pcase, reason)) = presented_stuck else {
            log(&format!("attempt {k}: failed without a stuck case: {:?}", attempt.stuck_reason));
            result.failure = attempt.stuck_reason.clone();
            result.attempts.push(attempt);
            break;
        };
        let (fixed, equations) = order.original_case(&pcase.fixed, &pcase.equations);
        let stuck = CaseSpec { fixed, equations };
        log(&format!("attempt {k}: stuck ({reason}) on original case {}", serde_json::to_string(&stuck).unwrap()));
        attempt.stuck_case = Some(stuck.clone());
        attempt.stuck_reason = Some(reason);
        let candidates: Vec<NodeOrder> = variants(family_text, &stuck.fixed, &stuck.equations, auto.probe.max_phys_pairs)
            .into_iter()
            .filter(|o| !tried.contains(&order_key(o, n)))
            .collect();
        let work = dir.join("probes");
        let _ = std::fs::create_dir_all(&work);
        let mut probes = probe(family_text, family_path, &stuck, &candidates, base, &auto.probe, auto.probe.max_depth, &work);
        if best(&probes).is_none() {
            if let Some(d) = auto.probe.fallback_depth {
                log(&format!("attempt {k}: no variant found a rule at depth {}; depth {d} round", auto.probe.max_depth));
                probes.extend(probe(family_text, family_path, &stuck, &candidates, base, &auto.probe, d, &work));
            }
        }
        for p in &probes {
            log(&format!("  probe {} d{}: {} {:?} {:.1} s", p.order.name, p.depth, p.status, p.score.as_ref().map(|s| (s.max_degree, s.rhs_terms, s.trace_rows)), p.seconds));
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
            result.failure = Some(format!("final table fails strict descent under its order: {:?}", report.not_lower.first()));
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
            order: NodeOrder { name: name.into(), ..NodeOrder::identity() },
            depth: 1,
            status: status.into(),
            score: Some(RuleScore { max_degree: deg, rhs_terms: rhs, trace_rows: rows, ..Default::default() }),
            seconds: 1.0,
            detail: String::new(),
            portfolio: Vec::new(),
            portfolio_incomplete: false,
        }
    }

    #[test]
    fn ranking_prefers_low_degree_then_rhs_and_ignores_failures() {
        let probes = vec![pr("a", "timeout", 0, 0, 0), pr("b", "rule", 6, 390, 195), pr("c", "rule", 2, 5, 4), pr("d", "rule", 2, 9, 1)];
        assert_eq!(best(&probes).unwrap().order.name, "c");
        assert!(best(&probes[..1]).is_none());
        // portfolio: a cheap root whose sub-corners swell loses against a uniformly moderate order
        let mut cheap_root = pr("root", "rule", 1, 10, 4);
        cheap_root.portfolio = vec![RuleScore { max_degree: 1, rhs_terms: 10, ..Default::default() }, RuleScore { max_degree: 15, rhs_terms: 1550, ..Default::default() }];
        let mut even = pr("even", "rule", 3, 40, 20);
        even.portfolio = vec![RuleScore { max_degree: 3, rhs_terms: 40, ..Default::default() }, RuleScore { max_degree: 4, rhs_terms: 60, ..Default::default() }];
        assert_eq!(best(&[cheap_root.clone(), even.clone()]).unwrap().order.name, "even");
        let mut stopped = even;
        stopped.portfolio_incomplete = true;
        assert_eq!(best(&[cheap_root, stopped]).unwrap().order.name, "root");
    }
}
