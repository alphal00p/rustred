use super::*;

type Rows = Vec<(usize, i64)>;

struct TestKernel {
    context: CoefficientContext,
    request: ReductionRequest,
    children: BTreeMap<usize, Rows>,
    terminals: BTreeMap<usize, i64>,
    empty: BTreeSet<usize>,
    visited: Vec<usize>,
    completed: Vec<usize>,
    recorded: Vec<ReductionStatistics>,
    fail_on: Option<usize>,
    cancel_after: Option<usize>,
    limits: ReductionLimits,
}

impl TestKernel {
    fn new(children: impl IntoIterator<Item = (usize, Rows)>) -> Self {
        Self {
            context: CoefficientContext::try_new(["d"]).unwrap(),
            request: ReductionRequest::default(),
            children: children.into_iter().collect(),
            terminals: BTreeMap::new(),
            empty: BTreeSet::new(),
            visited: vec![],
            completed: vec![],
            recorded: vec![],
            fail_on: None,
            cancel_after: None,
            limits: ReductionLimits::default(),
        }
    }

    fn condition(&self, key: usize) -> Coefficient {
        self.context.coefficient_fixture(&format!("d+{key}"))
    }

    fn initial(&self, rows: &[(usize, i64)]) -> Vec<(usize, Coefficient)> {
        rows.iter()
            .map(|&(key, weight)| (key, self.context.integer(weight)))
            .collect()
    }
}

impl DepthFirstKernel for TestKernel {
    type Key = usize;
    type Error = ReductionError;

    fn context(&self) -> &CoefficientContext {
        &self.context
    }

    fn limits(&self) -> ReductionLimits {
        self.limits
    }

    fn request(&mut self) -> &mut ReductionRequest {
        &mut self.request
    }

    fn step(
        &mut self,
        key: &usize,
        work: &mut ReductionStatistics,
    ) -> Result<DepthFirstStep<usize>, ReductionError> {
        self.visited.push(*key);
        work.record_rule_application();
        if self.fail_on == Some(*key) {
            return Err(ReductionError::UncoveredIntegral {
                target: IntegralKey::try_new([*key as i64]).unwrap(),
            });
        }
        let terms = self
            .children
            .get(key)
            .into_iter()
            .flatten()
            .map(|&(child, weight)| (child, self.context.integer(weight)))
            .collect();
        let terminals = if self.empty.contains(key) || self.children.contains_key(key) {
            BTreeMap::new()
        } else {
            BTreeMap::from([(
                IntegralKey::try_new([1]).unwrap(),
                self.context.integer(*self.terminals.get(key).unwrap_or(&1)),
            )])
        };
        Ok(DepthFirstStep {
            terms,
            terminals,
            nonzero_conditions: vec![self.condition(*key)],
        })
    }

    fn cycle_error(&self, key: &usize) -> ReductionError {
        ReductionError::CycleDetected {
            target: IntegralKey::try_new([*key as i64]).unwrap(),
        }
    }

    fn check_cancel(&mut self) -> Result<(), ReductionError> {
        if self
            .cancel_after
            .is_some_and(|count| self.visited.len() >= count)
        {
            return Err(invariant("test request canceled"));
        }
        Ok(())
    }

    fn completed(&mut self, key: &usize, _: usize) -> Result<(), ReductionError> {
        self.completed.push(*key);
        Ok(())
    }

    fn record_work(&mut self, work: ReductionStatistics) {
        self.recorded.push(work);
    }
}

#[test]
fn diamond_recomputes_shared_descendants_and_preserves_postorder_guards() {
    let mut kernel = TestKernel::new([
        (0, vec![(1, 2), (2, -3)]),
        (1, vec![(3, 5)]),
        (2, vec![(3, 7)]),
        (3, vec![(4, 1)]),
    ]);
    let initial = kernel.initial(&[(0, 13)]);
    let initial_condition = kernel.context.coefficient_fixture("d-1");
    let output = run(&mut kernel, initial, vec![initial_condition.clone()]).unwrap();
    assert_eq!(kernel.visited, [0, 1, 3, 4, 2, 3, 4]);
    assert_eq!(kernel.completed, [4, 3, 1, 4, 3, 2, 0]);
    assert_eq!(
        output.terms,
        BTreeMap::from([(
            IntegralKey::try_new([1]).unwrap(),
            kernel.context.integer(-143),
        )])
    );
    assert_eq!(
        output.nonzero_conditions,
        [
            initial_condition,
            kernel.condition(0),
            kernel.condition(1),
            kernel.condition(3),
            kernel.condition(4),
            kernel.condition(2),
        ]
    );
    assert_eq!(output.work.rule_applications(), 7);
    assert_eq!(output.work.cache_hits(), 0);
    assert_eq!(output.work.cached_integrals(), 0);
    assert_eq!(kernel.recorded, [output.work]);
    assert_eq!(kernel.request.pending_frame_count(), 0);
}

#[test]
fn duplicate_child_occurrences_execute_before_exact_cancellation() {
    let mut kernel = TestKernel::new([(0, vec![(1, 1), (1, -1)])]);
    let initial = kernel.initial(&[(0, 1)]);
    let output = run(&mut kernel, initial, vec![]).unwrap();
    assert_eq!(kernel.visited, [0, 1, 1]);
    assert!(output.terms.is_empty());
    assert_eq!(
        output.nonzero_conditions,
        [kernel.condition(0), kernel.condition(1)]
    );
    assert_eq!(output.work.coalescing_additions(), 1);
    assert_eq!(output.work.cache_hits(), 0);
}

#[test]
fn raw_child_occurrences_keep_their_original_unsorted_order() {
    let mut kernel = TestKernel::new([(0, vec![(9, 1), (1, 2), (9, 3)])]);
    let initial = kernel.initial(&[(0, 1)]);
    let output = run(&mut kernel, initial, vec![]).unwrap();
    assert_eq!(kernel.visited, [0, 9, 1, 9]);
    assert_eq!(kernel.completed, [9, 1, 9, 0]);
    assert_eq!(
        output.terms,
        BTreeMap::from([(
            IntegralKey::try_new([1]).unwrap(),
            kernel.context.integer(6),
        )])
    );
    assert_eq!(
        output.nonzero_conditions,
        [
            kernel.condition(0),
            kernel.condition(9),
            kernel.condition(1)
        ]
    );
}

#[test]
fn empty_child_guards_and_canceled_root_guards_survive() {
    let mut kernel = TestKernel::new([(0, vec![(1, 2), (2, 1)])]);
    kernel.empty.insert(1);
    let initial = kernel.initial(&[(0, 1), (2, -1)]);
    let output = run(&mut kernel, initial, vec![]).unwrap();
    assert_eq!(kernel.visited, [0, 1, 2, 2]);
    assert!(output.terms.is_empty());
    assert_eq!(
        output.nonzero_conditions,
        [
            kernel.condition(0),
            kernel.condition(1),
            kernel.condition(2)
        ]
    );
}

#[test]
fn active_path_cycles_fail_and_restore_an_existing_pending_baseline() {
    let mut kernel = TestKernel::new([(0, vec![(1, 1)]), (1, vec![(0, 1)])]);
    kernel.request.retain_pending_frame(None).unwrap();
    let initial = kernel.initial(&[(0, 1)]);
    assert_eq!(
        run(&mut kernel, initial, vec![]).err(),
        Some(ReductionError::CycleDetected {
            target: IntegralKey::try_new([0]).unwrap(),
        })
    );
    assert_eq!(kernel.visited, [0, 1]);
    assert_eq!(kernel.request.pending_frame_count(), 1);
    assert_eq!(kernel.recorded.len(), 1);
    assert_eq!(kernel.recorded[0].rule_applications(), 2);
}

#[test]
fn failures_after_a_returned_child_keep_attempted_work_and_release_frames() {
    let mut kernel = TestKernel::new([(0, vec![(1, 1), (2, 1)])]);
    kernel.fail_on = Some(2);
    let initial = kernel.initial(&[(0, 1)]);
    assert!(matches!(
        run(&mut kernel, initial, vec![]),
        Err(ReductionError::UncoveredIntegral { .. })
    ));
    assert_eq!(kernel.visited, [0, 1, 2]);
    assert_eq!(kernel.completed, [1]);
    assert_eq!(kernel.recorded.len(), 1);
    assert_eq!(kernel.recorded[0].rule_applications(), 3);
    assert_eq!(kernel.recorded[0].cache_hits(), 0);
    assert_eq!(kernel.request.pending_frame_count(), 0);
    kernel.fail_on = None;
    let initial = kernel.initial(&[(0, 1)]);
    assert!(run(&mut kernel, initial, vec![]).is_ok());
    assert_eq!(kernel.visited, [0, 1, 2, 0, 1, 2]);
    assert_eq!(kernel.recorded.len(), 2);
}

#[test]
fn cancellation_is_checked_after_a_child_without_returning_a_partial_answer() {
    let mut kernel = TestKernel::new([(0, vec![(1, 1), (2, 1)])]);
    kernel.cancel_after = Some(2);
    let initial = kernel.initial(&[(0, 1)]);
    assert!(matches!(
        run(&mut kernel, initial, vec![]),
        Err(ReductionError::ReducerInvariant {
            detail: "test request canceled"
        })
    ));
    assert_eq!(kernel.visited, [0, 1]);
    assert_eq!(kernel.recorded[0].rule_applications(), 2);
    assert_eq!(kernel.request.pending_frame_count(), 0);
}

#[test]
fn cancellation_during_the_last_empty_root_is_not_reported_as_success() {
    let mut kernel = TestKernel::new([]);
    kernel.empty.insert(0);
    kernel.cancel_after = Some(1);
    let initial = kernel.initial(&[(0, 1)]);
    assert!(matches!(
        run(&mut kernel, initial, vec![]),
        Err(ReductionError::ReducerInvariant {
            detail: "test request canceled"
        })
    ));
    assert_eq!(kernel.visited, [0]);
    assert_eq!(kernel.recorded.len(), 1);
    assert_eq!(kernel.recorded[0].rule_applications(), 1);
    assert_eq!(kernel.request.pending_frame_count(), 0);
}

#[test]
fn explicit_pending_diagnostic_restores_the_counter_on_admission_failure() {
    let mut kernel = TestKernel::new([]);
    kernel.limits.diagnostics_enabled = true;
    kernel.limits.max_pending_frames = 0;
    let initial = kernel.initial(&[(0, 1)]);
    assert!(matches!(
        run(&mut kernel, initial, vec![]),
        Err(ReductionError::PendingFrameLimit {
            requested: 1,
            limit: 0
        })
    ));
    assert!(kernel.visited.is_empty());
    assert_eq!(kernel.request.pending_frame_count(), 0);
    assert_eq!(kernel.recorded.len(), 1);
}

#[test]
fn failed_parent_coalescing_keeps_work_done_after_the_last_checked_step() {
    let mut kernel = TestKernel::new([(0, vec![(1, 1), (2, 1)])]);
    kernel.limits.diagnostics_enabled = true;
    kernel.limits.max_coalescing_additions = 0;
    let initial = kernel.initial(&[(0, 1)]);
    assert!(matches!(
        run(&mut kernel, initial, vec![]),
        Err(ReductionError::CoalescingAdditionLimit {
            requested: 1,
            limit: 0
        })
    ));
    assert_eq!(kernel.visited, [0, 1, 2]);
    assert_eq!(kernel.completed, [1, 2]);
    assert_eq!(kernel.recorded.len(), 1);
    assert_eq!(kernel.recorded[0].rule_applications(), 3);
    assert_eq!(kernel.recorded[0].coalescing_additions(), 1);
    assert_eq!(kernel.request.pending_frame_count(), 0);
}

#[test]
fn zero_root_weights_do_not_expand_and_separate_calls_never_reuse_results() {
    let mut kernel = TestKernel::new([]);
    let condition = kernel.context.coefficient_fixture("d-2");
    let initial = kernel.initial(&[(0, 0)]);
    let zero = run(&mut kernel, initial, vec![condition.clone()]).unwrap();
    assert!(zero.terms.is_empty());
    assert_eq!(zero.nonzero_conditions, [condition]);
    assert!(kernel.visited.is_empty());
    for _ in 0..2 {
        let initial = kernel.initial(&[(0, 1)]);
        let result = run(&mut kernel, initial, vec![]).unwrap();
        assert_eq!(result.work.rule_applications(), 1);
        assert_eq!(result.work.cache_hits(), 0);
    }
    assert_eq!(kernel.visited, [0, 0]);
}
