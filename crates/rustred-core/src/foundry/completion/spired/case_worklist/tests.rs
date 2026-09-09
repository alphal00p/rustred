use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, StratumRegistryLimits,
};
use crate::sector::{InteriorBounds, Mask, SectorMonotoneDomain};

use super::{
    SpiredCoordinateCaseEnqueueOutcome, SpiredCoordinateCaseObligation,
    SpiredCoordinateCasePopOutcome, SpiredCoordinateCaseWorklist,
    SpiredCoordinateCaseWorklistError, SpiredCoordinateCaseWorklistLimits,
};

const FAMILY: &str = "rustred.test.spired-coordinate-worklist.family.v1";
const CONTEXT: &str = "rustred.test.spired-coordinate-worklist.context.v1";

fn stratum(
    bounds: &[(i64, i64)],
    guards: impl IntoIterator<Item = GuardBranchIdentity>,
) -> DecoratedStratum {
    let sector = Mask::try_new(std::iter::repeat_n(true, bounds.len())).unwrap();
    let pivot = vec![0_i64; bounds.len()];
    let rhs: [&[i64]; 0] = [];
    let domain = SectorMonotoneDomain::try_new_for_rule(
        sector,
        bounds
            .iter()
            .map(|&(lower, upper)| InteriorBounds::new(lower, upper)),
        &pivot,
        &rhs,
    )
    .unwrap();
    DecoratedStratum::try_new(
        FAMILY,
        CONTEXT,
        domain,
        guards,
        StratumRegistryLimits::default(),
    )
    .unwrap()
}

fn obligation(bounds: &[(i64, i64)]) -> SpiredCoordinateCaseObligation {
    SpiredCoordinateCaseObligation::try_new_root(stratum(bounds, [])).unwrap()
}

fn child(
    parent: &SpiredCoordinateCaseObligation,
    bounds: &[(i64, i64)],
) -> SpiredCoordinateCaseObligation {
    SpiredCoordinateCaseObligation::try_new_child(parent, stratum(bounds, [])).unwrap()
}

fn scoped_obligation(
    family: &str,
    context: &str,
    active: &[bool],
    bounds: &[(i64, i64)],
) -> SpiredCoordinateCaseObligation {
    let sector = Mask::try_new(active.iter().copied()).unwrap();
    let pivot = vec![0_i64; bounds.len()];
    let rhs: [&[i64]; 0] = [];
    let domain = SectorMonotoneDomain::try_new_for_rule(
        sector,
        bounds
            .iter()
            .map(|&(lower, upper)| InteriorBounds::new(lower, upper)),
        &pivot,
        &rhs,
    )
    .unwrap();
    SpiredCoordinateCaseObligation::try_new_root(
        DecoratedStratum::try_guard_blind(
            family,
            context,
            domain,
            StratumRegistryLimits::default(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn drain_ids(queue: &mut SpiredCoordinateCaseWorklist) -> Vec<(usize, String)> {
    let mut drained = Vec::new();
    loop {
        match queue.try_pop().unwrap() {
            SpiredCoordinateCasePopOutcome::Case(case) => drained.push((
                case.free_dimension(),
                case.stratum().id().as_str().to_owned(),
            )),
            SpiredCoordinateCasePopOutcome::Empty => return drained,
        }
    }
}

#[test]
fn guard_bearing_strata_cannot_become_discovery_cases() {
    let guard =
        GuardBranchIdentity::try_new("g", GuardBranch::Zero, StratumRegistryLimits::default())
            .unwrap();
    assert!(matches!(
        SpiredCoordinateCaseObligation::try_new_root(stratum(&[(1, 9)], [guard])),
        Err(SpiredCoordinateCaseWorklistError::GuardedDiscoveryCase { guard_branches: 1 })
    ));
}

#[test]
fn logical_children_allow_only_carrier_full_axes_or_exact_equalities() {
    let root =
        SpiredCoordinateCaseObligation::try_new_root(stratum(&[(1, 9), (1, 9)], [])).unwrap();

    assert_eq!(
        SpiredCoordinateCaseObligation::try_new_child(&root, stratum(&[(2, 7), (1, 9)], []),)
            .unwrap_err(),
        SpiredCoordinateCaseWorklistError::NonEqualityCaseAxis {
            position: 0,
            carrier_lower: 1,
            carrier_upper: 9,
            case_lower: 2,
            case_upper: 7,
        }
    );

    let child =
        SpiredCoordinateCaseObligation::try_new_child(&root, stratum(&[(3, 3), (1, 9)], []))
            .unwrap();
    assert!(root.shares_declared_carrier(&child));
    assert_eq!(root.free_dimension(), 2);
    assert_eq!(child.free_dimension(), 1);
}

#[test]
fn logical_children_cannot_unfix_or_change_a_parent_equality() {
    let root =
        SpiredCoordinateCaseObligation::try_new_root(stratum(&[(1, 9), (1, 9)], [])).unwrap();
    let parent =
        SpiredCoordinateCaseObligation::try_new_child(&root, stratum(&[(3, 3), (1, 9)], []))
            .unwrap();

    for invalid in [
        stratum(&[(1, 9), (4, 4)], []),
        stratum(&[(4, 4), (1, 9)], []),
    ] {
        assert_eq!(
            SpiredCoordinateCaseObligation::try_new_child(&parent, invalid).unwrap_err(),
            SpiredCoordinateCaseWorklistError::ChildOutsideParent,
        );
    }

    let child =
        SpiredCoordinateCaseObligation::try_new_child(&parent, stratum(&[(3, 3), (4, 4)], []))
            .unwrap();
    assert_eq!(child.free_dimension(), 0);
}

#[test]
fn exact_identity_duplicates_are_typed_and_do_not_grow_the_queue() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    let case = obligation(&[(1, 9), (1, 1)]);
    assert_eq!(
        queue.try_enqueue(case.clone()).unwrap(),
        SpiredCoordinateCaseEnqueueOutcome::Inserted {
            removed_subsumed: 0
        }
    );
    assert_eq!(
        queue.try_enqueue(case).unwrap(),
        SpiredCoordinateCaseEnqueueOutcome::ExactDuplicate
    );
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.census().enqueue_attempts(), 2);
    assert_eq!(queue.census().inserted_cases(), 1);
    assert_eq!(queue.census().exact_duplicates(), 1);
    assert_eq!(queue.census().subsumption_checks(), 0);
}

#[test]
fn broader_pending_cases_subsume_stricter_coordinate_equalities() {
    let mut incoming_broad = SpiredCoordinateCaseWorklist::new(Default::default());
    let broad = obligation(&[(1, 9), (1, 8)]);
    let narrow = child(&broad, &[(1, 9), (1, 1)]);
    incoming_broad.try_enqueue(narrow.clone()).unwrap();
    assert_eq!(
        incoming_broad.try_enqueue(broad.clone()).unwrap(),
        SpiredCoordinateCaseEnqueueOutcome::Inserted {
            removed_subsumed: 1
        }
    );
    assert_eq!(incoming_broad.len(), 1);
    assert_eq!(incoming_broad.census().removed_subsumed(), 1);

    let mut pending_broad = SpiredCoordinateCaseWorklist::new(Default::default());
    pending_broad.try_enqueue(broad.clone()).unwrap();
    assert_eq!(
        pending_broad.try_enqueue(narrow).unwrap(),
        SpiredCoordinateCaseEnqueueOutcome::SubsumedByPending
    );
    let SpiredCoordinateCasePopOutcome::Case(actual) = pending_broad.try_pop().unwrap() else {
        panic!("the broader pending case must be retained")
    };
    assert_eq!(actual, broad);
}

#[test]
fn pop_order_is_higher_dimensional_first_and_enqueue_order_independent() {
    let cases = [
        obligation(&[(1, 5), (1, 5)]),
        obligation(&[(6, 10), (1, 5)]),
        obligation(&[(11, 11), (1, 5)]),
    ];
    let mut forward = SpiredCoordinateCaseWorklist::new(Default::default());
    let mut reverse = SpiredCoordinateCaseWorklist::new(Default::default());
    for case in &cases {
        forward.try_enqueue(case.clone()).unwrap();
    }
    for case in cases.iter().rev() {
        reverse.try_enqueue(case.clone()).unwrap();
    }
    let forward = drain_ids(&mut forward);
    let reverse = drain_ids(&mut reverse);
    assert_eq!(forward, reverse);
    assert_eq!(
        forward
            .iter()
            .map(|(dimension, _)| *dimension)
            .collect::<Vec<_>>(),
        vec![2, 2, 1]
    );
}

#[test]
fn empty_pop_and_retained_payload_are_typed_and_exact() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    assert_eq!(
        queue.try_pop().unwrap(),
        SpiredCoordinateCasePopOutcome::Empty
    );
    let case = obligation(&[(1, 5), (1, 1), (2, 2)]);
    let identity_bytes = case.stratum().id().as_str().len();
    queue.try_enqueue(case).unwrap();
    let census = queue.census();
    assert_eq!(census.empty_pops(), 1);
    assert_eq!(census.pending_cases(), 1);
    assert_eq!(census.pending_coordinate_cells(), 6);
    assert_eq!(census.pending_identity_bytes(), identity_bytes);
    assert!(matches!(
        queue.try_pop().unwrap(),
        SpiredCoordinateCasePopOutcome::Case(_)
    ));
    assert!(queue.is_empty());
    assert_eq!(queue.census().popped_cases(), 1);
    assert_eq!(queue.census().pending_cases(), 0);
    assert_eq!(queue.census().pending_coordinate_cells(), 0);
    assert_eq!(queue.census().pending_identity_bytes(), 0);
}

#[test]
fn pending_and_cumulative_budgets_fail_without_mutating_pending_cases() {
    let mut queue = SpiredCoordinateCaseWorklist::new(SpiredCoordinateCaseWorklistLimits {
        max_pending_cases: 1,
        ..Default::default()
    });
    let retained = obligation(&[(1, 2)]);
    queue.try_enqueue(retained.clone()).unwrap();
    assert!(matches!(
        queue.try_enqueue(obligation(&[(3, 4)])),
        Err(SpiredCoordinateCaseWorklistError::ResourceLimit {
            resource: "pending cases",
            requested: 2,
            limit: 1,
        })
    ));
    assert_eq!(queue.len(), 1);
    let SpiredCoordinateCasePopOutcome::Case(actual) = queue.try_pop().unwrap() else {
        panic!("the retained case must survive a failed enqueue")
    };
    assert_eq!(actual, retained);

    let mut checked = SpiredCoordinateCaseWorklist::new(SpiredCoordinateCaseWorklistLimits {
        max_subsumption_checks: 0,
        ..Default::default()
    });
    checked.try_enqueue(obligation(&[(1, 2)])).unwrap();
    assert!(matches!(
        checked.try_enqueue(obligation(&[(3, 4)])),
        Err(SpiredCoordinateCaseWorklistError::ResourceLimit {
            resource: "subsumption checks",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(checked.len(), 1);
}

#[test]
fn prepared_batch_failure_rolls_back_prior_staged_subsumption_and_all_census() {
    let mut queue = SpiredCoordinateCaseWorklist::new(SpiredCoordinateCaseWorklistLimits {
        max_pending_cases: 1,
        ..Default::default()
    });
    let broad = obligation(&[(1, 9), (1, 8)]);
    let retained_narrow = child(&broad, &[(1, 9), (1, 1)]);
    queue.try_enqueue(retained_narrow.clone()).unwrap();
    let before_census = queue.census();
    let before_revision = queue.revision();

    // The first staged child removes the live narrow case in favor of a
    // broader one. The second child exceeds the pending-case budget. Neither
    // the staged removal nor any cumulative work may escape the failed token.
    assert!(matches!(
        queue.try_prepare_enqueue_batch([broad, obligation(&[(10, 12), (1, 8)]),]),
        Err(SpiredCoordinateCaseWorklistError::ResourceLimit {
            resource: "pending cases",
            requested: 2,
            limit: 1,
        })
    ));
    assert_eq!(queue.census(), before_census);
    assert_eq!(queue.revision(), before_revision);
    assert_eq!(queue.len(), 1);
    let SpiredCoordinateCasePopOutcome::Case(actual) = queue.try_pop().unwrap() else {
        panic!("the original narrow case must survive failed batch preparation")
    };
    assert_eq!(actual, retained_narrow);
}

#[test]
fn prepared_batch_accounts_pending_and_intra_batch_duplicates_exactly_once() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    let retained = obligation(&[(1, 2)]);
    let inserted = obligation(&[(3, 4)]);
    queue.try_enqueue(retained.clone()).unwrap();
    let before_census = queue.census();
    let before_revision = queue.revision();

    let prepared = queue
        .try_prepare_enqueue_batch([retained, inserted.clone(), inserted])
        .unwrap();
    assert_eq!(
        prepared.outcomes(),
        &[
            SpiredCoordinateCaseEnqueueOutcome::ExactDuplicate,
            SpiredCoordinateCaseEnqueueOutcome::Inserted {
                removed_subsumed: 0,
            },
            SpiredCoordinateCaseEnqueueOutcome::ExactDuplicate,
        ]
    );
    let prospective = prepared.prospective_census();
    assert_eq!(prospective.enqueue_attempts(), 4);
    assert_eq!(prospective.inserted_cases(), 2);
    assert_eq!(prospective.exact_duplicates(), 2);
    assert_eq!(prospective.pending_cases(), 2);

    // Preparation is observably read-only until its explicit commit.
    assert_eq!(queue.census(), before_census);
    assert_eq!(queue.revision(), before_revision);
    assert_eq!(queue.len(), 1);
    queue
        .try_validate_prepared_enqueue_batch(prepared)
        .unwrap()
        .commit();
    assert_eq!(queue.census(), prospective);
    assert_eq!(queue.revision(), before_revision + 1);
    assert_eq!(queue.len(), 2);
}

#[test]
fn prepared_batch_charges_cumulative_limits_atomically() {
    let mut queue = SpiredCoordinateCaseWorklist::new(SpiredCoordinateCaseWorklistLimits {
        max_enqueue_attempts: 3,
        ..Default::default()
    });
    let retained = obligation(&[(1, 2)]);
    queue.try_enqueue(retained.clone()).unwrap();
    let prepared = queue
        .try_prepare_enqueue_batch([retained.clone(), retained.clone()])
        .unwrap();
    assert_eq!(prepared.prospective_census().enqueue_attempts(), 3);
    assert_eq!(prepared.prospective_census().exact_duplicates(), 2);
    queue.try_commit_prepared_enqueue_batch(prepared).unwrap();

    let committed_census = queue.census();
    let committed_revision = queue.revision();
    assert!(matches!(
        queue.try_prepare_enqueue_batch([retained]),
        Err(SpiredCoordinateCaseWorklistError::ResourceLimit {
            resource: "enqueue attempts",
            requested: 4,
            limit: 3,
        })
    ));
    assert_eq!(queue.census(), committed_census);
    assert_eq!(queue.revision(), committed_revision);
    assert_eq!(queue.len(), 1);
}

#[test]
fn prepared_batch_rejects_revision_drift_without_overwriting_new_work() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    queue.try_enqueue(obligation(&[(1, 2)])).unwrap();
    let prepared = queue
        .try_prepare_enqueue_batch([obligation(&[(3, 4)])])
        .unwrap();
    let prepared_revision = queue.revision();

    let later = obligation(&[(5, 6)]);
    queue.try_enqueue(later.clone()).unwrap();
    let current_revision = queue.revision();
    let current_census = queue.census();
    assert_eq!(
        queue
            .try_commit_prepared_enqueue_batch(prepared)
            .unwrap_err(),
        SpiredCoordinateCaseWorklistError::StalePreparedBatch {
            prepared_revision,
            current_revision,
        }
    );
    assert_eq!(queue.revision(), current_revision);
    assert_eq!(queue.census(), current_census);
    assert_eq!(queue.len(), 2);
    assert!(
        drain_ids(&mut queue)
            .into_iter()
            .any(|(_, identity)| identity == later.stratum().id().as_str())
    );
}

#[test]
fn prepared_batch_is_bound_to_its_originating_queue_identity() {
    let source = SpiredCoordinateCaseWorklist::new(Default::default());
    let prepared = source
        .try_prepare_enqueue_batch([obligation(&[(1, 2)])])
        .unwrap();
    let mut other = SpiredCoordinateCaseWorklist::new(Default::default());
    let before_census = other.census();
    let before_revision = other.revision();
    assert_eq!(
        other
            .try_commit_prepared_enqueue_batch(prepared)
            .unwrap_err(),
        SpiredCoordinateCaseWorklistError::PreparedBatchQueueMismatch
    );
    assert!(other.is_empty());
    assert_eq!(other.census(), before_census);
    assert_eq!(other.revision(), before_revision);
}

#[test]
fn committing_one_same_epoch_batch_makes_its_peer_stale() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    let first = queue
        .try_prepare_enqueue_batch([obligation(&[(1, 2)])])
        .unwrap();
    let second = queue
        .try_prepare_enqueue_batch([obligation(&[(3, 4)])])
        .unwrap();
    queue.try_commit_prepared_enqueue_batch(first).unwrap();
    assert!(matches!(
        queue.try_commit_prepared_enqueue_batch(second),
        Err(SpiredCoordinateCaseWorklistError::StalePreparedBatch {
            prepared_revision: 0,
            current_revision: 1,
        })
    ));
    assert_eq!(queue.len(), 1);
}

#[test]
fn empty_prepared_batch_is_an_exact_noop() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    queue.try_enqueue(obligation(&[(1, 2)])).unwrap();
    let before_census = queue.census();
    let before_revision = queue.revision();
    let prepared = queue.try_prepare_enqueue_batch(std::iter::empty()).unwrap();
    assert!(prepared.outcomes().is_empty());
    assert_eq!(prepared.prospective_census(), before_census);
    queue.try_commit_prepared_enqueue_batch(prepared).unwrap();
    assert_eq!(queue.census(), before_census);
    assert_eq!(queue.revision(), before_revision);
    assert_eq!(queue.len(), 1);
}

#[test]
fn current_case_can_be_inspected_and_cloned_without_mutation() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    let lower_priority = obligation(&[(10, 10), (1, 5)]);
    let current = obligation(&[(1, 5), (1, 5)]);
    queue.try_enqueue(lower_priority).unwrap();
    queue.try_enqueue(current.clone()).unwrap();
    let before_census = queue.census();
    let before_revision = queue.revision();

    assert_eq!(queue.current(), Some(&current));
    assert_eq!(queue.cloned_current(), Some(current));
    assert_eq!(queue.census(), before_census);
    assert_eq!(queue.revision(), before_revision);
    assert_eq!(queue.len(), 2);
}

#[test]
fn replacement_removes_parent_before_child_subsumption_and_accounts_one_retirement() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    let parent = obligation(&[(1, 9), (1, 9)]);
    let first_child = child(&parent, &[(1, 9), (2, 2)]);
    let second_child = child(&parent, &[(3, 3), (1, 9)]);
    queue.try_enqueue(parent.clone()).unwrap();
    let before = queue.census();

    // Either child is rejected by ordinary enqueue while the parent remains.
    let mut control = SpiredCoordinateCaseWorklist::new(Default::default());
    control.try_enqueue(parent.clone()).unwrap();
    assert_eq!(
        control.try_enqueue(first_child.clone()).unwrap(),
        SpiredCoordinateCaseEnqueueOutcome::SubsumedByPending
    );
    assert_eq!(
        control.try_enqueue(second_child.clone()).unwrap(),
        SpiredCoordinateCaseEnqueueOutcome::SubsumedByPending
    );

    // Replacement stages parent removal first, so both refinements survive.
    let prepared = queue
        .try_prepare_current_replacement(&parent, [first_child.clone(), second_child.clone()])
        .unwrap();
    assert_eq!(
        prepared.child_outcomes(),
        &[
            SpiredCoordinateCaseEnqueueOutcome::Inserted {
                removed_subsumed: 0,
            },
            SpiredCoordinateCaseEnqueueOutcome::Inserted {
                removed_subsumed: 0,
            },
        ]
    );
    let prospective = prepared.prospective_census();
    assert_eq!(prospective.retired_cases(), 1);
    assert_eq!(prospective.popped_cases(), 0);
    assert_eq!(
        prospective.enqueue_attempts(),
        before.enqueue_attempts() + 2
    );
    assert_eq!(prospective.inserted_cases(), before.inserted_cases() + 2);
    assert_eq!(prospective.pending_cases(), 2);

    // Preparation remains a no-op until explicit validation and commit.
    assert_eq!(queue.current(), Some(&parent));
    assert_eq!(queue.census(), before);
    queue
        .try_validate_prepared_replacement(prepared)
        .unwrap()
        .commit();
    assert_eq!(queue.census(), prospective);
    assert_eq!(queue.len(), 2);
    let drained = drain_ids(&mut queue);
    assert!(
        drained
            .iter()
            .any(|(_, identity)| identity == first_child.stratum().id().as_str())
    );
    assert!(
        drained
            .iter()
            .any(|(_, identity)| identity == second_child.stratum().id().as_str())
    );
}

#[test]
fn zero_child_replacement_completes_exactly_one_case() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    let parent = obligation(&[(1, 9), (1, 9)]);
    queue.try_enqueue(parent.clone()).unwrap();
    let before_revision = queue.revision();
    let prepared = queue
        .try_prepare_current_replacement(&parent, std::iter::empty())
        .unwrap();
    assert!(prepared.child_outcomes().is_empty());
    assert_eq!(prepared.prospective_census().pending_cases(), 0);
    assert_eq!(prepared.prospective_census().retired_cases(), 1);
    assert_eq!(prepared.prospective_census().enqueue_attempts(), 1);
    queue.try_commit_prepared_replacement(prepared).unwrap();

    assert!(queue.is_empty());
    assert_eq!(queue.census().retired_cases(), 1);
    assert_eq!(queue.census().popped_cases(), 0);
    assert_eq!(queue.revision(), before_revision + 1);
}

#[test]
fn failed_replacement_rolls_back_parent_removal_children_and_census() {
    let mut queue = SpiredCoordinateCaseWorklist::new(SpiredCoordinateCaseWorklistLimits {
        max_pending_cases: 1,
        ..Default::default()
    });
    let parent = obligation(&[(1, 9), (1, 9)]);
    queue.try_enqueue(parent.clone()).unwrap();
    let before_census = queue.census();
    let before_revision = queue.revision();

    assert!(matches!(
        queue.try_prepare_current_replacement(
            &parent,
            [
                child(&parent, &[(1, 9), (2, 2)]),
                child(&parent, &[(3, 3), (1, 9)]),
            ],
        ),
        Err(SpiredCoordinateCaseWorklistError::ResourceLimit {
            resource: "pending cases",
            requested: 2,
            limit: 1,
        })
    ));
    assert_eq!(queue.current(), Some(&parent));
    assert_eq!(queue.census(), before_census);
    assert_eq!(queue.revision(), before_revision);
    assert_eq!(queue.len(), 1);
}

#[test]
fn retirement_budget_failure_is_a_complete_noop() {
    let mut queue = SpiredCoordinateCaseWorklist::new(SpiredCoordinateCaseWorklistLimits {
        max_retired_cases: 0,
        ..Default::default()
    });
    let parent = obligation(&[(1, 9)]);
    queue.try_enqueue(parent.clone()).unwrap();
    let before_census = queue.census();
    let before_revision = queue.revision();
    assert!(matches!(
        queue.try_prepare_current_replacement(&parent, std::iter::empty()),
        Err(SpiredCoordinateCaseWorklistError::ResourceLimit {
            resource: "retired cases",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(queue.current(), Some(&parent));
    assert_eq!(queue.census(), before_census);
    assert_eq!(queue.revision(), before_revision);
}

#[test]
fn replacement_rejects_a_noncurrent_expected_case_without_removal() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    let noncurrent = obligation(&[(10, 10), (1, 5)]);
    let current = obligation(&[(1, 5), (1, 5)]);
    queue.try_enqueue(noncurrent.clone()).unwrap();
    queue.try_enqueue(current.clone()).unwrap();
    let before_census = queue.census();
    let before_revision = queue.revision();

    assert_eq!(
        queue
            .try_prepare_current_replacement(&noncurrent, std::iter::empty())
            .unwrap_err(),
        SpiredCoordinateCaseWorklistError::ExpectedCurrentCaseMismatch
    );
    assert_eq!(queue.current(), Some(&current));
    assert_eq!(queue.census(), before_census);
    assert_eq!(queue.revision(), before_revision);
    assert_eq!(queue.len(), 2);
}

#[test]
fn replacement_rejects_foreign_outside_and_nonprogressing_children_atomically() {
    let mut queue = SpiredCoordinateCaseWorklist::new(Default::default());
    let parent = obligation(&[(1, 9), (1, 9)]);
    queue.try_enqueue(parent.clone()).unwrap();

    let invalid = [
        (
            scoped_obligation("foreign-family", CONTEXT, &[true, true], &[(2, 2), (1, 9)]),
            SpiredCoordinateCaseWorklistError::ReplacementChildFamilyMismatch,
        ),
        (
            scoped_obligation(FAMILY, "foreign-context", &[true, true], &[(2, 2), (1, 9)]),
            SpiredCoordinateCaseWorklistError::ReplacementChildContextMismatch,
        ),
        (
            scoped_obligation(FAMILY, CONTEXT, &[true, false], &[(2, 2), (-1, -1)]),
            SpiredCoordinateCaseWorklistError::ReplacementChildSectorMismatch,
        ),
        (
            obligation(&[(10, 10), (2, 2)]),
            SpiredCoordinateCaseWorklistError::ReplacementChildCarrierMismatch,
        ),
        (
            parent.clone(),
            SpiredCoordinateCaseWorklistError::ReplacementChildDimensionNotReduced {
                parent_dimension: 2,
                child_dimension: 2,
            },
        ),
        (
            obligation(&[(2, 8), (1, 9)]),
            SpiredCoordinateCaseWorklistError::ReplacementChildCarrierMismatch,
        ),
    ];

    for (child, expected) in invalid {
        let before_census = queue.census();
        let before_revision = queue.revision();
        let before_len = queue.len();
        assert_eq!(
            queue
                .try_prepare_current_replacement(&parent, [child])
                .unwrap_err(),
            expected
        );
        assert_eq!(queue.current(), Some(&parent));
        assert_eq!(queue.census(), before_census);
        assert_eq!(queue.revision(), before_revision);
        assert_eq!(queue.len(), before_len);
    }

    let valid_child = child(&parent, &[(2, 2), (1, 9)]);
    let prepared = queue
        .try_prepare_current_replacement(&parent, [valid_child.clone()])
        .unwrap();
    queue.try_commit_prepared_replacement(prepared).unwrap();
    assert_eq!(queue.current(), Some(&valid_child));
}

#[test]
fn replacement_rejects_stale_and_foreign_tokens_without_mutation() {
    let parent = obligation(&[(1, 9)]);
    let child = child(&parent, &[(2, 2)]);

    let mut stale_queue = SpiredCoordinateCaseWorklist::new(Default::default());
    stale_queue.try_enqueue(parent.clone()).unwrap();
    let stale = stale_queue
        .try_prepare_current_replacement(&parent, [child.clone()])
        .unwrap();
    let prepared_revision = stale_queue.revision();
    stale_queue.try_enqueue(obligation(&[(10, 11)])).unwrap();
    let current_revision = stale_queue.revision();
    let current_census = stale_queue.census();
    assert_eq!(
        stale_queue
            .try_commit_prepared_replacement(stale)
            .unwrap_err(),
        SpiredCoordinateCaseWorklistError::StalePreparedReplacement {
            prepared_revision,
            current_revision,
        }
    );
    assert_eq!(stale_queue.census(), current_census);
    assert_eq!(stale_queue.revision(), current_revision);

    let source = {
        let mut source = SpiredCoordinateCaseWorklist::new(Default::default());
        source.try_enqueue(parent.clone()).unwrap();
        let prepared = source
            .try_prepare_current_replacement(&parent, [child])
            .unwrap();
        (source, prepared)
    };
    let (_source_queue, foreign) = source;
    let mut other = SpiredCoordinateCaseWorklist::new(Default::default());
    other.try_enqueue(parent.clone()).unwrap();
    let before_census = other.census();
    let before_revision = other.revision();
    assert_eq!(
        other.try_commit_prepared_replacement(foreign).unwrap_err(),
        SpiredCoordinateCaseWorklistError::PreparedReplacementQueueMismatch
    );
    assert_eq!(other.current(), Some(&parent));
    assert_eq!(other.census(), before_census);
    assert_eq!(other.revision(), before_revision);
}

#[test]
fn replacement_child_membership_and_pop_order_are_input_order_independent() {
    let parent = obligation(&[(1, 9), (1, 9)]);
    let children = [
        child(&parent, &[(1, 9), (2, 2)]),
        child(&parent, &[(3, 3), (1, 9)]),
        child(&parent, &[(4, 4), (5, 5)]),
    ];
    let mut forward = SpiredCoordinateCaseWorklist::new(Default::default());
    let mut reverse = SpiredCoordinateCaseWorklist::new(Default::default());
    forward.try_enqueue(parent.clone()).unwrap();
    reverse.try_enqueue(parent.clone()).unwrap();
    let forward_prepared = forward
        .try_prepare_current_replacement(&parent, children.iter().cloned())
        .unwrap();
    let reverse_prepared = reverse
        .try_prepare_current_replacement(&parent, children.iter().rev().cloned())
        .unwrap();
    forward
        .try_commit_prepared_replacement(forward_prepared)
        .unwrap();
    reverse
        .try_commit_prepared_replacement(reverse_prepared)
        .unwrap();

    assert_eq!(drain_ids(&mut forward), drain_ids(&mut reverse));
}
