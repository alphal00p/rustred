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
}

/// The budget of the pre-screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiftScreen {
    /// Traces with fewer source rows are not screened (they lift in seconds).
    pub min_rows: usize,
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

    pub fn decide(&self, m: &ScreenMeasure) -> ScreenDecision {
        match self.heavy(m) {
            Some(reason) => match self.on_heavy {
                HeavyAction::Refuse => ScreenDecision::Refuse(reason),
                HeavyAction::Sampled => ScreenDecision::Sampled,
                HeavyAction::Proceed => ScreenDecision::Pass,
            },
            None if self.sampled_from_rows.is_some_and(|r| m.rows >= r) && m.variables > 0 => ScreenDecision::Sampled,
            None => ScreenDecision::Pass,
        }
    }
}

thread_local! {
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
