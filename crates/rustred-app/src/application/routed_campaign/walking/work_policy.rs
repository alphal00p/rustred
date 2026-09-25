//! Production enumeration policy, separate from bounded in-flight storage.
use super::OwnerDomainWalkRequest;

impl OwnerDomainWalkRequest {
    /// Remove diagnostic cumulative-work stops. Counters remain checked and
    /// allocations grow with actual work, never with these maximum values.
    ///
    /// This does not assert a RAM bound: callers must supervise resident memory.
    /// Input admission, worker buffers, scratch multiplicity and per-operation
    /// algebra safeguards remain unchanged. A remaining native refusal is still
    /// reported, never treated as successful coverage.
    pub fn disable_work_limits(&mut self) {
        self.max_domains = usize::MAX;
        self.max_events = usize::MAX;
        self.max_frontiers = usize::MAX;
        self.max_containment_checks = None;
        self.max_route_masks = usize::MAX;
        let matching = &mut self.matching.match_limits;
        matching.max_rules = usize::MAX;
        matching.max_terminal_checks = usize::MAX;
        matching.max_predicates = usize::MAX;
        matching.max_pieces = usize::MAX;
        matching.max_cells = usize::MAX;
        matching.max_split_operations = usize::MAX;
        matching.max_coordinate_cells = usize::MAX;
        matching.max_bounded_refinement_cells = usize::MAX;
        // The nested copy is overridden at native dispatch. Keep it consistent
        // here too, so direct API inspection faithfully describes the request.
        self.applied_limits.matching = *matching;
        self.applied_limits.max_term_visits = usize::MAX;
        self.applied_limits.max_shift_groups = usize::MAX;
        self.applied_limits.max_boundary_cells = usize::MAX;
        self.applied_limits.max_sign_splits = usize::MAX;
        self.applied_limits.max_native_operations = usize::MAX;
        self.applied_limits.max_events = usize::MAX;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OwnerDomainMatchRequest;

    #[test]
    fn removing_work_caps_preserves_input_scratch_algebra_and_execution_policy() {
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("{}".into(), "{}".into()));
        request.workers = 50;
        request
            .matching
            .match_limits
            .guard_algebra
            .max_univariate_degree = 64;
        request.max_containment_checks = Some(17);
        request.applied_limits.cell_refinement =
            rustred::solver::OwnerAppliedCellRefinement::SingleFiniteAxis {
                max_cardinality: std::num::NonZeroUsize::new(2).unwrap(),
            };
        let before = request.clone();
        request.disable_work_limits();
        assert_eq!(request.max_domains, usize::MAX);
        assert_eq!(request.max_events, usize::MAX);
        assert_eq!(request.max_frontiers, usize::MAX);
        assert_eq!(request.max_route_masks, usize::MAX);
        assert_eq!(request.max_containment_checks, None);
        let matching = request.matching.match_limits;
        for cap in [
            matching.max_rules,
            matching.max_terminal_checks,
            matching.max_predicates,
            matching.max_pieces,
            matching.max_cells,
            matching.max_split_operations,
            matching.max_coordinate_cells,
            matching.max_bounded_refinement_cells,
        ] {
            assert_eq!(cap, usize::MAX);
        }
        let applied = request.applied_limits;
        assert_eq!(
            applied.cell_refinement,
            before.applied_limits.cell_refinement
        );
        for cap in [
            applied.max_term_visits,
            applied.max_shift_groups,
            applied.max_boundary_cells,
            applied.max_sign_splits,
            applied.max_native_operations,
            applied.max_events,
        ] {
            assert_eq!(cap, usize::MAX);
        }
        assert_eq!(
            format!("{:?}", matching.guard_algebra),
            format!("{:?}", before.matching.match_limits.guard_algebra)
        );
        assert_eq!(
            matching.refinement_axes,
            before.matching.match_limits.refinement_axes
        );
        assert_eq!(request.workers, before.workers);
        assert_eq!(request.scheduling_policy, before.scheduling_policy);
        assert_eq!(request.publication_policy, before.publication_policy);
        assert_eq!(request.matching.max_queries, before.matching.max_queries);
        assert_eq!(
            request.matching.max_query_bytes,
            before.matching.max_query_bytes
        );
        assert_eq!(
            request.matching.max_total_pieces,
            before.matching.max_total_pieces
        );
        assert_eq!(
            applied.max_scratch_terms,
            before.applied_limits.max_scratch_terms
        );
        assert_eq!(
            applied.max_scratch_boxes,
            before.applied_limits.max_scratch_boxes
        );
        assert_eq!(
            applied.max_scratch_coordinate_cells,
            before.applied_limits.max_scratch_coordinate_cells
        );
    }
}
