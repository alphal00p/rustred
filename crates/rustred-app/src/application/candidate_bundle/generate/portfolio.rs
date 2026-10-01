//! Invocation-local trial costs. Reused sector payloads carry no historical
//! timing, so a complete checkpoint resume reports zero new-sector work.
use rustred::solver::RuleSelectionStats;
use serde::Serialize;

#[derive(Default, Serialize)]
pub(super) struct Report {
    scope: &'static str,
    duration_scope: &'static str,
    pub(super) newly_solved_sectors: usize,
    attempted: u128,
    admitted: u128,
    rejected: u128,
    selected_alternatives: u128,
    trigger_skips: u128,
    duplicate_skips: u128,
    search_exhausted: u128,
    work_limited: u128,
    unlucky_samples: u128,
    unsupported_geometry: u128,
    geometry_budget: u128,
    non_progress: u128,
    fatal: u128,
    search_seeds: u128,
    search_rows: u128,
    independent_rows: u128,
    exact_trace_rows: u128,
    exact_trace_terms: u128,
    exact_lifts: u128,
    guard_branches: u128,
    geometry_calls: u128,
    // Worker-summed durations. Exact materialization is INCLUDED in search_us;
    // none of these sums is whole-campaign wall time.
    search_us: u128,
    exact_materialization_us: u128,
    guard_extraction_us: u128,
    geometry_us: u128,
}

impl Report {
    pub(super) fn new() -> Self {
        Self {
            scope: "newly-solved-sectors-this-invocation; all baseline/alternative attempts",
            duration_scope: "summed-worker; exact_materialization_us is included in search_us",
            ..Self::default()
        }
    }

    pub(super) fn add(&mut self, stats: RuleSelectionStats) {
        macro_rules! sum { ($($field:ident),* $(,)?) => { $(self.$field += stats.$field as u128;)* }; }
        sum!(
            attempted,
            admitted,
            rejected,
            selected_alternatives,
            trigger_skips,
            duplicate_skips,
            search_exhausted,
            work_limited,
            unlucky_samples,
            unsupported_geometry,
            geometry_budget,
            non_progress,
            fatal
        );
        let work = stats.total;
        self.search_seeds += work.search.seeds as u128;
        self.search_rows += work.search.rows as u128;
        self.independent_rows += work.search.independent_rows as u128;
        self.exact_trace_rows += work.search.exact_trace_rows as u128;
        self.exact_trace_terms += work.exact_trace_terms as u128;
        self.exact_lifts += work.exact_lifts as u128;
        self.guard_branches += work.guard_branches as u128;
        self.geometry_calls += work.geometry_calls as u128;
        self.search_us += work.search.elapsed.as_micros();
        self.exact_materialization_us += work.search.exact_materialization.as_micros();
        self.guard_extraction_us += work.guard_extraction.as_micros();
        self.geometry_us += work.geometry.as_micros();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn report_includes_rejected_work_without_turning_it_into_selected_rules() {
        let mut stats = RuleSelectionStats {
            attempted: 3,
            admitted: 1,
            rejected: 2,
            work_limited: 2,
            selected_alternatives: 0,
            ..Default::default()
        };
        stats.total.search.rows = 29;
        stats.total.search.elapsed = Duration::from_micros(100);
        stats.total.search.exact_materialization = Duration::from_micros(70);
        stats.total.exact_lifts = 2;
        let mut report = Report::default();
        report.add(stats);
        report.add(stats);
        assert_eq!(
            (report.attempted, report.admitted, report.rejected),
            (6, 2, 4)
        );
        assert_eq!((report.search_rows, report.exact_lifts), (58, 4));
        assert_eq!(
            (report.search_us, report.exact_materialization_us),
            (200, 140)
        );
        assert_eq!(report.selected_alternatives, 0);
        assert_eq!(Report::default().search_rows, 0);
    }
}
