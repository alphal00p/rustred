//! Reuse only an unchanged restricted coefficient, never an algebraically equal sum.
use super::*;
use std::sync::atomic::Ordering;

const LOWER: [u64; 3] = [1, 0, 0];
const UPPER: [Option<u64>; 3] = [None, Some(0), Some(0)];

#[test]
fn applied_singleton_classification_preserves_uniform_and_conditional_support() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    for (coefficient, expected, operations) in [
        (c.integer(2), OwnerAppliedNonzero::Uniform, 2),
        (c.index(0).unwrap(), OwnerAppliedNonzero::Uniform, 4),
        (
            c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap(),
            OwnerAppliedNonzero::Conditional,
            4,
        ),
    ] {
        batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], coefficient.clone())])];
        let (out, stats) = collect(&p, LOWER, UPPER, Some(10));
        assert!(out.problems.is_empty());
        assert_eq!(out.edges.len(), 1);
        assert_eq!(out.edges[0].coefficient, coefficient);
        assert_eq!(out.edges[0].nonzero, expected);
        assert_eq!(out.finished, [(1, 0)]);
        // One restriction and one original classification, no second attempt.
        assert_eq!(stats.native_operations, operations);
        assert_eq!(stats.coalescing_additions, 0);
        assert_eq!(stats.optional_coefficient_refusals, 0);
        assert_eq!(stats.events, 3);
    }
}

#[test]
fn applied_singleton_classification_survives_zero_term_neighbours() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    for (coefficient, expected) in [
        (c.index(0).unwrap(), OwnerAppliedNonzero::Uniform),
        (
            c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap(),
            OwnerAppliedNonzero::Conditional,
        ),
    ] {
        batch(&mut p).rules = vec![rule(
            0,
            vec![
                term(&c, [-1, 0, 0], c.zero()),
                term(&c, [-1, 0, 0], coefficient.clone()),
                term(&c, [-1, 0, 0], c.index(2).unwrap()),
            ],
        )];
        let (out, stats) = collect(&p, LOWER, UPPER, Some(10));
        assert!(out.problems.is_empty());
        assert_eq!(out.edges.len(), 1);
        assert_eq!(out.edges[0].coefficient, coefficient);
        assert_eq!(out.edges[0].nonzero, expected);
        assert_eq!((stats.term_visits, stats.zero_terms), (3, 2));
        assert_eq!(stats.coalescing_additions, 0);
        assert_eq!(stats.native_operations, 6);
    }
}

#[test]
fn applied_singleton_classification_does_not_change_zero_group_exits() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], c.zero()),
            term(&c, [-1, 0, 0], c.index(2).unwrap()),
        ],
    )];
    let (out, stats) = collect(&p, LOWER, UPPER, Some(10));
    assert!(out.edges.is_empty() && out.problems.is_empty());
    assert_eq!(out.finished, [(0, 0)]);
    assert_eq!((stats.zero_terms, stats.native_operations), (2, 2));
    assert_eq!(stats.cancelled_groups, 0);

    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .insert([false, true, false]);
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-2, 0, 0], c.integer(1)),
            term(&c, [-2, 0, 0], c.integer(-1)),
        ],
    )];
    let (out, stats) = collect(&p, LOWER, [Some(1), Some(0), Some(0)], Some(10));
    assert!(out.edges.is_empty() && out.problems.is_empty());
    assert_eq!(out.finished, [(0, 0)]);
    assert_eq!(stats.zero_sector_groups, 1);
    assert_eq!((stats.coalescing_additions, stats.cancelled_groups), (0, 0));
    assert_eq!((stats.term_visits, stats.native_operations), (2, 4));
}

#[test]
fn applied_singleton_classification_is_invalidated_by_real_addition() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let value = c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap();
    let negative = c.sub(&c.zero(), &value).unwrap();
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], value),
            term(&c, [-1, 0, 0], negative),
            term(&c, [-1, 0, 0], c.integer(1)),
        ],
    )];
    let (out, stats) = collect(&p, LOWER, UPPER, Some(10));
    assert!(out.problems.is_empty());
    assert_eq!(out.edges.len(), 1);
    assert_eq!(out.edges[0].coefficient, c.integer(1));
    assert_eq!(out.edges[0].nonzero, OwnerAppliedNonzero::Uniform);
    assert_eq!(stats.coalescing_additions, 2);
    assert_eq!(stats.cancelled_groups, 0);
    assert_eq!(stats.native_operations, 13);

    batch(&mut p).rules[0].rhs.pop();
    let (out, stats) = collect(&p, LOWER, UPPER, Some(10));
    assert!(out.edges.is_empty() && out.problems.is_empty());
    assert_eq!((stats.coalescing_additions, stats.cancelled_groups), (1, 1));
    assert_eq!(stats.native_operations, 9);
}

#[test]
fn applied_singleton_classification_keeps_original_child_failure_before_cancellation() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    // Unlike the family's full root, this saved owner forbids activating axis 2.
    Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&OWNER)
            .unwrap(),
    )
    .unwrap()
    .root = OWNER;
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [0, 0, 1], c.index(2).unwrap()),
            term(&c, [0, 0, 1], c.integer(1)),
            term(&c, [0, 0, 1], c.integer(-1)),
        ],
    )];
    let mut problem_ordinals = Vec::new();
    let stats = p
        .visit_owner_applied_successors(
            OWNER,
            &LOWER,
            &UPPER,
            Some(10),
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                if let OwnerAppliedEvent::Problem(problem) = event {
                    assert_eq!(problem.kind, OwnerAppliedProblemKind::InvalidChildRoot);
                    problem_ordinals.push(problem.original_term_ordinal);
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!(problem_ordinals, [Some(1)]);
    assert_eq!((stats.term_visits, stats.zero_terms), (2, 1));
    assert_eq!((stats.coalescing_additions, stats.cancelled_groups), (0, 0));
    assert_eq!(stats.successors, 0);
}

#[test]
fn applied_singleton_classification_respects_actual_work_event_caps_and_cancellation() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.integer(2))])];
    let limits = OwnerAppliedLimits {
        max_native_operations: 2,
        max_events: 3,
        ..Default::default()
    };
    let cancellation = AtomicBool::new(false);
    let stats = p
        .visit_owner_applied_successors(
            OWNER,
            &LOWER,
            &UPPER,
            Some(10),
            limits,
            &cancellation,
            |_| ControlFlow::Continue(()),
        )
        .unwrap();
    assert_eq!((stats.native_operations, stats.events), (2, 3));
    for (limits, expected) in [
        (
            OwnerAppliedLimits {
                max_native_operations: 1,
                ..limits
            },
            "native operations",
        ),
        (
            OwnerAppliedLimits {
                max_events: 2,
                ..limits
            },
            "events",
        ),
    ] {
        let error = p
            .visit_owner_applied_successors(
                OWNER,
                &LOWER,
                &UPPER,
                Some(10),
                limits,
                &cancellation,
                |_| ControlFlow::Continue(()),
            )
            .unwrap_err();
        assert!(matches!(
            error.failure,
            OwnerAppliedFailure::ResourceLimit { resource, .. } if resource == expected
        ));
    }
    // A cached-path successor remains provisional: cancellation after its
    // callback prevents RuleFinished rather than manufacturing completion.
    let error = p
        .visit_owner_applied_successors(
            OWNER,
            &LOWER,
            &UPPER,
            Some(10),
            limits,
            &cancellation,
            |event| {
                match event {
                    OwnerAppliedEvent::Successor(_) => {
                        cancellation.store(true, Ordering::Release);
                    }
                    OwnerAppliedEvent::RuleFinished { .. } => {
                        panic!("cancelled successor must not finish its rule");
                    }
                    _ => {}
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert_eq!(error.failure, OwnerAppliedFailure::Cancelled);
    assert_eq!((error.stats.native_operations, error.stats.events), (2, 2));
    assert_eq!(error.stats.successors, 1);
    let error = p
        .visit_owner_applied_successors(
            OWNER,
            &LOWER,
            &UPPER,
            Some(10),
            limits,
            &cancellation,
            |_| panic!("entry cancellation must precede output"),
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        OwnerAppliedFailure::Cancelled
            | OwnerAppliedFailure::Matching(
                super::super::super::OwnerDomainMatchFailure::Cancelled
            )
    ));
    assert_eq!((error.stats.native_operations, error.stats.events), (0, 0));
}
