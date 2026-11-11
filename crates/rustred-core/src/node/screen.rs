//! Lift pre-screen policy: before an exact lift, a modular replay of the selected trace estimates the target
//! row (RHS terms, total numerator/denominator degree, common denominator degree); a budget decides whether the
//! exact lift runs, runs through the modular-sampled reconstruction, or is refused so that the caller tries another
//! candidate relation (source order) or another node order instead of hanging.
//!
//! The 5L campaign (coordinator, notes/p5l-exact-lift.md): hanging lifts are heavy-tailed in cost and not predicted
//! by the trace size; T206's slice relation had degree 37 / 4312 RHS terms in one presentation and degree 2 / 5
//! terms in another. The measurement costs a few dozen modular replays of the trace (milliseconds each).
//!
//! The policy is installed per thread with [`with_screen`]; the engine consults it in its exact-lift entry (solver
//! discovery) only when installed, so the default engine path is unchanged. Every screened lift leaves a
//! [`ScreenReport`] that the caller drains with [`take_reports`] (the events of the node driver).

use std::cell::RefCell;

/// What to do with a lift the screen classifies as heavy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HeavyAction {
    /// Fail the case search (`ExactReplay("lift pre-screen refused: ...")`): the caller tries another relation/order.
    Refuse,
    /// Reconstruct the row from modular replays (sampled backend; fresh-prime check + mutation control).
    Sampled,
    /// Record only; lift exactly anyway.
    Proceed,
    /// Gate on the measured relation SIZE and ORACLE COST, not on degree alone: a heavy lift goes through the sampled
    /// reconstruction when its estimated dense box fits the reconstruction ([`LiftScreen::max_box`]) and the estimated
    /// sampled cost (points x primes x measured replay time) fits [`LiftScreen::sampled_budget_seconds`]; otherwise it
    /// is refused (class B: the relation itself must change: order / presentation). Astra T211 (2026-11-11): a
    /// degree-16, 3-variable, 9236-term relation (box 969, 6.63M monomials) was reconstructed in 1170 s.
    Auto,
}

/// The budget of the pre-screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiftScreen {
    /// Traces with fewer source rows are not screened (they lift in seconds) ...
    pub min_rows: usize,
    /// ... unless the target block (columns up to and including the target, the elimination width of the target-only
    /// lift) is at least this wide (Astra's 4L cross run stalled on a 364-column target block with 209 trace rows).
    #[serde(default = "default_min_target_block")]
    pub min_target_block: usize,
    /// Replay cap of the Thiele line (no convergence = degree beyond about max_points/2 = heavy).
    pub max_points: usize,
    /// Heavy when max(numerator, denominator) total degree of a column exceeds this.
    pub max_total_degree: usize,
    /// Heavy when the target row has more RHS terms than this.
    pub max_rhs_terms: usize,
    /// Lifts with at least this many rows that are NOT heavy go through the sampled reconstruction
    /// (the exact elimination of large traces can swell even for small rows, T159); `None` = always exact.
    pub sampled_from_rows: Option<usize>,
    pub sampled_threads: usize,
    pub on_heavy: HeavyAction,
    pub seed: u64,
    /// [`HeavyAction::Auto`]: largest dense reconstruction box (unknowns of one prime image) the sampled route accepts.
    #[serde(default = "default_max_box")]
    pub max_box: usize,
    /// [`HeavyAction::Auto`]: budget of the estimated sampled reconstruction (CPU seconds / `sampled_threads`).
    #[serde(default = "default_sampled_budget")]
    pub sampled_budget_seconds: u64,
    /// [`HeavyAction::Auto`]: primes assumed by the cost estimate (T211 needed 4 for a degree-16 row).
    #[serde(default = "default_est_primes")]
    pub est_primes: usize,
    /// Ladder rung 1 for LARGE traces of small relations (class A, T159/T211/T212 corners: degree 4, 1689-2745 rows,
    /// the exact elimination swells): a non-heavy lift with at least this many rows is refused once so that the
    /// caller searches the case again under the SymbolicFirst seed schedule (10-20x smaller traces); under that
    /// schedule it is lifted (sampled when `sampled_from_rows` applies and the route is feasible). `None` = off; the
    /// node driver clears it when its SymbolicFirst rung is off.
    #[serde(default)]
    pub symbolic_first_from_rows: Option<usize>,
}

fn default_min_target_block() -> usize {
    320
}
fn default_max_box() -> usize {
    crate::solver::sampled_max_box()
}
fn default_sampled_budget() -> u64 {
    1800
}
fn default_est_primes() -> usize {
    4
}

impl Default for LiftScreen {
    fn default() -> Self {
        Self {
            min_rows: 600,
            max_points: 64,
            max_total_degree: 24,
            max_rhs_terms: 6000,
            sampled_from_rows: None,
            sampled_threads: 1,
            on_heavy: HeavyAction::Refuse,
            seed: 0x5eed_1f7,
            min_target_block: default_min_target_block(),
            max_box: default_max_box(),
            sampled_budget_seconds: default_sampled_budget(),
            est_primes: default_est_primes(),
            symbolic_first_from_rows: None,
        }
    }
}

/// Modular measurement of one trace (filled by the engine's sampled module).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScreenMeasure {
    pub rows: usize,
    pub columns: usize,
    pub variables: usize,
    pub rhs_terms: usize,
    pub max_num_degree: usize,
    pub max_den_degree: usize,
    pub common_den_degree: usize,
    pub converged: bool,
    pub points: usize,
    pub bad_points: usize,
    pub seconds: f64,
    pub note: String,
    /// Columns up to and including the target (elimination width of the target-only lift).
    #[serde(default)]
    pub target_block: usize,
    /// Mean seconds of one modular replay of the trace (the sampled oracle cost).
    #[serde(default)]
    pub replay_seconds: f64,
}

impl ScreenMeasure {
    /// Estimated dense reconstruction box of one prime image: max(L + NF, P) with L the common-denominator box,
    /// NF the box of the numerator of a random combination and P the per-column numerator box, all dense in the
    /// active variables up to the measured total degrees (the same boxes the sampled route builds). None when the
    /// degree probe did not converge.
    pub fn box_unknowns(&self) -> Option<usize> {
        if !self.converged {
            return None;
        }
        let v = self.variables;
        let dense = |d: usize| -> usize {
            // C(d + v, v), saturating
            let mut c: u128 = 1;
            for i in 1..=v as u128 {
                c = c * (d as u128 + i) / i;
                if c > usize::MAX as u128 {
                    return usize::MAX;
                }
            }
            c as usize
        };
        let l = dense(self.common_den_degree);
        let nf = dense(self.common_den_degree.max(self.max_num_degree));
        let p = dense(self.max_num_degree);
        Some(l.saturating_add(nf).max(p))
    }

    /// Estimated sampled reconstruction CPU seconds: primes x (2 box + 8) points x replay time (Astra T211: 4 primes x
    /// 1945 points for a box of 969, 1829 replay CPU seconds).
    pub fn sampled_cpu_seconds(&self, primes: usize) -> Option<f64> {
        let b = self.box_unknowns()?;
        Some(primes as f64 * (2.0 * b as f64 + 8.0) * self.replay_seconds)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ScreenDecision {
    Pass,
    Sampled,
    Refuse(String),
}

/// One screened lift.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScreenReport {
    pub measure: ScreenMeasure,
    pub decision: ScreenDecision,
    /// Heavy by the budget (whatever the action).
    pub heavy: Option<String>,
}

impl LiftScreen {
    /// Why the measured lift is heavy, or None.
    pub fn heavy(&self, m: &ScreenMeasure) -> Option<String> {
        if !m.converged {
            return Some(format!(
                "degree probe did not converge within {} replays ({}): total degree beyond ~{}",
                self.max_points,
                m.note,
                self.max_points / 2
            ));
        }
        let degree = m.max_num_degree.max(m.max_den_degree);
        if degree > self.max_total_degree {
            return Some(format!("total coefficient degree {degree} > {}", self.max_total_degree));
        }
        if m.rhs_terms > self.max_rhs_terms {
            return Some(format!("{} RHS terms > {}", m.rhs_terms, self.max_rhs_terms));
        }
        None
    }

    /// Whether the sampled route can take this relation (box and oracle cost within budget); Err = why not.
    pub fn sampled_feasible(&self, m: &ScreenMeasure) -> Result<(), String> {
        let b = m.box_unknowns().ok_or("degree probe did not converge")?;
        if b > self.max_box {
            return Err(format!("dense box {b} > {}", self.max_box));
        }
        let cpu = m.sampled_cpu_seconds(self.est_primes).unwrap_or(f64::INFINITY);
        let wall = cpu / self.sampled_threads.max(1) as f64;
        if wall > self.sampled_budget_seconds as f64 {
            return Err(format!("estimated sampled cost {wall:.0} s > {} s (box {b}, replay {:.3} s)", self.sampled_budget_seconds, m.replay_seconds));
        }
        Ok(())
    }

    /// Whether a trace is screened at all.
    pub fn screens(&self, rows: usize, target_block: usize) -> bool {
        rows >= self.min_rows || target_block >= self.min_target_block
    }

    pub fn decide(&self, m: &ScreenMeasure) -> ScreenDecision {
        self.decide_in(m, false)
    }

    /// [`Self::decide`] knowing whether the search ran under the SymbolicFirst seed schedule.
    pub fn decide_in(&self, m: &ScreenMeasure, symbolic_first: bool) -> ScreenDecision {
        if self.heavy(m).is_none() && !symbolic_first && self.symbolic_first_from_rows.is_some_and(|r| m.rows >= r) {
            return ScreenDecision::Refuse(format!("large trace ({} rows) of a small relation: SymbolicFirst rung first", m.rows));
        }
        match self.heavy(m) {
            Some(reason) => match self.on_heavy {
                HeavyAction::Refuse => ScreenDecision::Refuse(reason),
                HeavyAction::Sampled => ScreenDecision::Sampled,
                HeavyAction::Proceed => ScreenDecision::Pass,
                HeavyAction::Auto => match self.sampled_feasible(m) {
                    Ok(()) if m.variables > 0 => ScreenDecision::Sampled,
                    Ok(()) => ScreenDecision::Pass,
                    Err(why) => ScreenDecision::Refuse(format!("{reason}; class B: {why}")),
                },
            },
            None if self.sampled_from_rows.is_some_and(|r| m.rows >= r) && m.variables > 0 => {
                if self.on_heavy != HeavyAction::Auto {
                    ScreenDecision::Sampled
                } else {
                    match self.sampled_feasible(m) {
                        Ok(()) => ScreenDecision::Sampled,
                        // a LARGE trace of a relation whose box is beyond the reconstruction: class B, never lifted
                        // (T163/T182/T216 c3: 1882-3087-row exact lifts of degree <= 24 ran into the case budget)
                        Err(why) if m.box_unknowns().is_some_and(|b| b > self.max_box) => {
                            ScreenDecision::Refuse(format!("large trace ({} rows), class B: {why}", m.rows))
                        }
                        Err(_) => ScreenDecision::Pass,
                    }
                }
            }
            None => ScreenDecision::Pass,
        }
    }
}

thread_local! {
    static SEED_SCHEDULE: std::cell::Cell<Option<crate::solver::SeedSchedule>> = const { std::cell::Cell::new(None) };
    static ACTIVE: RefCell<Option<LiftScreen>> = const { RefCell::new(None) };
    static REPORTS: RefCell<Vec<ScreenReport>> = const { RefCell::new(Vec::new()) };
}

/// Run `f` with `screen` installed on this thread (restored afterwards, also on unwinding).
pub fn with_screen<R>(screen: Option<LiftScreen>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<LiftScreen>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|a| *a.borrow_mut() = self.0.take());
        }
    }
    let previous = ACTIVE.with(|a| std::mem::replace(&mut *a.borrow_mut(), screen));
    let _restore = Restore(previous);
    f()
}

/// Run `f` with the symbolic case search's in-shell seed schedule overridden on this thread (the lift ladder's first
/// rung: `SymbolicFirst` keeps the presentation and the seed ball, p5l-lift-trace). Restored afterwards.
pub fn with_seed_schedule<R>(schedule: Option<crate::solver::SeedSchedule>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<crate::solver::SeedSchedule>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SEED_SCHEDULE.with(|a| a.set(self.0));
        }
    }
    let previous = SEED_SCHEDULE.with(|a| a.replace(schedule));
    let _restore = Restore(previous);
    f()
}

/// The seed schedule override installed on this thread, if any.
pub fn seed_schedule_override() -> Option<crate::solver::SeedSchedule> {
    SEED_SCHEDULE.with(|a| a.get())
}

/// Drain the reports of the lifts screened on this thread so far.
pub fn take_reports() -> Vec<ScreenReport> {
    REPORTS.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

pub(crate) fn active() -> Option<LiftScreen> {
    ACTIVE.with(|a| *a.borrow())
}

pub(crate) fn record(measure: ScreenMeasure, decision: ScreenDecision) {
    let heavy = active().and_then(|s| s.heavy(&measure));
    REPORTS.with(|r| r.borrow_mut().push(ScreenReport { measure, decision, heavy }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(deg: usize, rhs: usize, rows: usize, converged: bool) -> ScreenMeasure {
        ScreenMeasure { rows, variables: 3, rhs_terms: rhs, max_num_degree: deg, max_den_degree: deg, converged, ..Default::default() }
    }

    #[test]
    fn policy_decisions() {
        let s = LiftScreen { max_total_degree: 10, max_rhs_terms: 100, ..Default::default() };
        assert_eq!(s.decide(&m(4, 50, 2000, true)), ScreenDecision::Pass);
        assert!(matches!(s.decide(&m(37, 50, 2000, true)), ScreenDecision::Refuse(r) if r.contains("37")));
        assert!(matches!(s.decide(&m(4, 4312, 2000, true)), ScreenDecision::Refuse(r) if r.contains("4312")));
        assert!(matches!(s.decide(&m(0, 0, 2000, false)), ScreenDecision::Refuse(_)));
        let sampled = LiftScreen { sampled_from_rows: Some(1500), on_heavy: HeavyAction::Sampled, ..s };
        assert_eq!(sampled.decide(&m(4, 50, 2000, true)), ScreenDecision::Sampled);
        assert_eq!(sampled.decide(&m(4, 50, 1000, true)), ScreenDecision::Pass);
        assert_eq!(sampled.decide(&m(40, 50, 1000, true)), ScreenDecision::Sampled);
        let proceed = LiftScreen { on_heavy: HeavyAction::Proceed, ..s };
        assert_eq!(proceed.decide(&m(40, 50, 1000, true)), ScreenDecision::Pass);
    }

    #[test]
    fn auto_gates_on_box_and_oracle_cost_not_degree_alone() {
        let s = LiftScreen { on_heavy: HeavyAction::Auto, max_total_degree: 10, sampled_threads: 3, ..Default::default() };
        // Astra T211 hard-B: 3 variables, degree 16, 9236 terms, ~0.23 s per replay: heavy by degree, sampled feasible
        let t211 = ScreenMeasure { rows: 3814, variables: 3, rhs_terms: 9236, max_num_degree: 16, max_den_degree: 16, common_den_degree: 16, converged: true, replay_seconds: 0.23, ..Default::default() };
        assert_eq!(t211.box_unknowns(), Some(969 + 969));
        assert_eq!(s.decide(&t211), ScreenDecision::Sampled);
        // T206 slice relation (3 variables, degree 26 common denominator): box beyond the reconstruction -> class B
        let t206 = ScreenMeasure { rows: 3581, variables: 3, rhs_terms: 2196, max_num_degree: 25, max_den_degree: 24, common_den_degree: 26, converged: true, replay_seconds: 0.046, ..Default::default() };
        assert!(matches!(s.decide(&t206), ScreenDecision::Refuse(r) if r.contains("class B") && r.contains("dense box")));
        // T182: 7 variables, degree 14: huge box -> class B
        let t182 = ScreenMeasure { rows: 2828, variables: 7, rhs_terms: 8623, max_num_degree: 14, max_den_degree: 14, common_den_degree: 14, converged: true, replay_seconds: 0.32, ..Default::default() };
        assert!(matches!(s.decide(&t182), ScreenDecision::Refuse(_)));
        // a small box with a slow oracle: refused on cost
        let slow = ScreenMeasure { replay_seconds: 50.0, ..t211.clone() };
        assert!(matches!(s.decide(&slow), ScreenDecision::Refuse(r) if r.contains("estimated sampled cost")));
        // not converged: refused
        assert!(matches!(s.decide(&ScreenMeasure { converged: false, ..t211.clone() }), ScreenDecision::Refuse(_)));
        // not heavy: exact
        assert_eq!(s.decide(&ScreenMeasure { max_num_degree: 4, max_den_degree: 4, common_den_degree: 4, rhs_terms: 50, ..t211 }), ScreenDecision::Pass);
        // the target block triggers screening of short traces
        assert!(s.screens(209, 364));
        assert!(!s.screens(209, 100));
        assert!(s.screens(700, 1));
    }

    #[test]
    fn large_small_relation_goes_symbolic_first_then_sampled() {
        let s = LiftScreen { on_heavy: HeavyAction::Auto, symbolic_first_from_rows: Some(1200), sampled_from_rows: Some(1200), ..Default::default() };
        let corner = ScreenMeasure { rows: 1689, variables: 4, rhs_terms: 300, max_num_degree: 4, max_den_degree: 4, common_den_degree: 4, converged: true, replay_seconds: 0.05, ..Default::default() };
        assert!(matches!(s.decide_in(&corner, false), ScreenDecision::Refuse(r) if r.contains("SymbolicFirst")));
        assert_eq!(s.decide_in(&corner, true), ScreenDecision::Sampled);
        assert_eq!(s.decide_in(&ScreenMeasure { rows: 400, ..corner.clone() }, false), ScreenDecision::Pass);
        // infeasible sampled route under SymbolicFirst: exact lift
        assert_eq!(s.decide_in(&ScreenMeasure { replay_seconds: 100.0, ..corner.clone() }, true), ScreenDecision::Pass);
        // large trace of a large relation under SymbolicFirst: class B, refused (box 15 deg, 5 vars = 15504 + ...)
        let big = ScreenMeasure { max_num_degree: 15, max_den_degree: 15, common_den_degree: 15, variables: 5, rows: 2085, ..corner.clone() };
        assert!(matches!(s.decide_in(&big, true), ScreenDecision::Refuse(r) if r.contains("class B")));
        // off by default
        assert_eq!(LiftScreen { on_heavy: HeavyAction::Auto, ..Default::default() }.decide_in(&corner, false), ScreenDecision::Pass);
    }

    #[test]
    fn seed_schedule_override_restores() {
        use crate::solver::SeedSchedule;
        assert_eq!(seed_schedule_override(), None);
        with_seed_schedule(Some(SeedSchedule::SymbolicFirst), || {
            assert_eq!(seed_schedule_override(), Some(SeedSchedule::SymbolicFirst));
        });
        assert_eq!(seed_schedule_override(), None);
    }

    #[test]
    fn install_restore_and_reports() {
        assert!(active().is_none());
        let s = LiftScreen::default();
        with_screen(Some(s), || {
            assert_eq!(active(), Some(s));
            record(m(30, 10, 700, true), ScreenDecision::Refuse("x".into()));
        });
        assert!(active().is_none());
        let reports = take_reports();
        assert_eq!(reports.len(), 1);
        assert!(reports[0].heavy.as_deref().unwrap().contains("degree 30"));
        assert!(take_reports().is_empty());
    }
}
