use super::*;
use std::sync::atomic::Ordering;

fn partition(stats: OwnerAppliedStats) -> [usize; 3] {
    let counts = [
        stats.same_support_successors,
        stats.strict_subsupport_successors,
        stats.unsupported_support_successors,
    ];
    assert_eq!(counts.iter().sum::<usize>(), stats.successors);
    assert!(stats.conditional_unsupported_support_successors <= counts[2]);
    assert!(stats.conditional_unsupported_support_successors <= stats.conditional_successors);
    counts
}

#[test]
fn support_transitions_partition_every_mask_relation_without_new_overflow_path() {
    let cancellation = AtomicBool::new(false);
    let mut budget = super::super::Budget {
        limits: Default::default(),
        stats: Default::default(),
        cancel: &cancellation,
    };
    for (target, conditional, expected) in [
        (OWNER, false, [1, 0, 0]),
        ([false, true, false], true, [1, 1, 0]),
        ([false; 3], false, [1, 2, 0]),
        ([false, true, true], true, [1, 2, 1]),
        ([false, false, true], false, [1, 2, 2]),
        ([true; 3], true, [1, 2, 3]),
    ] {
        budget.successor(&OWNER, &target, conditional).unwrap();
        assert_eq!(partition(budget.stats), expected);
    }
    assert_eq!(budget.stats.conditional_successors, 3);
    assert_eq!(budget.stats.conditional_unsupported_support_successors, 2);

    // Start at a consistent, nearly exhausted counter state. The pre-existing
    // total charge protects every subset counter without additional failures.
    budget.stats = OwnerAppliedStats {
        successors: usize::MAX - 1,
        conditional_successors: usize::MAX - 1,
        unsupported_support_successors: usize::MAX - 1,
        conditional_unsupported_support_successors: usize::MAX - 1,
        ..Default::default()
    };
    budget.successor(&OWNER, &[true; 3], true).unwrap();
    assert_eq!(partition(budget.stats), [0, 0, usize::MAX]);
    let before = budget.stats;
    assert_eq!(
        budget.successor(&OWNER, &[true; 3], true),
        Err(OwnerAppliedFailure::CountOverflow {
            resource: "successors"
        })
    );
    assert_eq!(budget.stats, before);
}

#[test]
fn support_transitions_use_selected_owner_not_wider_saved_root() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&OWNER)
            .unwrap(),
    )
    .unwrap()
    .root = [true; 3];
    batch(&mut p).terminals.clear();
    for (source, shift, target, expected) in [
        ([1, 0, 0], [-1, 0, 0], OWNER, [1, 0, 0]),
        ([0, 0, 0], [-1, 0, 0], [false, true, false], [0, 1, 0]),
        // Both are admitted by saved-root validity and local descent, but the
        // concrete routed evaluator does not accept their reactivated axis.
        ([0, 0, 0], [-1, 0, 1], [false, true, true], [0, 0, 1]),
        ([0, 0, 0], [-1, -1, 1], [false, false, true], [0, 0, 1]),
    ] {
        batch(&mut p).rules = vec![rule(0, vec![term(&c, shift, c.integer(1))])];
        let (out, stats) = collect(&p, source, source.map(Some), Some(10));
        assert!(out.problems.is_empty(), "{out:?}");
        assert_eq!(out.edges.len(), 1);
        assert_eq!(out.edges[0].target, target);
        assert_eq!(partition(stats), expected);
        assert_eq!(stats.conditional_unsupported_support_successors, 0);
    }
    // A pure activation fails existing descent, not this observational census.
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [0, 0, 1], c.integer(1))])];
    let (out, stats) = collect(&p, [0; 3], [Some(0); 3], Some(10));
    assert!(matches!(
        out.problems.as_slice(),
        [OwnerAppliedProblemKind::DescentNotEstablished { .. }]
    ));
    assert_eq!(partition(stats), [0; 3]);
}

#[test]
fn support_transitions_preserve_conditional_unsupported_successors() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&OWNER)
            .unwrap(),
    )
    .unwrap()
    .root = [true; 3];
    batch(&mut p).terminals.clear();
    let coefficient = c.sub(&c.index(1).unwrap(), &c.integer(3)).unwrap();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 1], coefficient)])];
    let (out, stats) = collect(&p, [0; 3], [Some(0), None, Some(0)], Some(10));
    assert!(out.problems.is_empty(), "{out:?}");
    assert_eq!(out.edges.len(), 1);
    assert_eq!(out.edges[0].nonzero, OwnerAppliedNonzero::Conditional);
    assert_eq!(partition(stats), [0, 0, 1]);
    assert_eq!(stats.conditional_successors, 1);
    assert_eq!(stats.conditional_unsupported_support_successors, 1);
    assert_eq!(out.finished, [(1, 0)]);
}

#[test]
fn support_transitions_exclude_zero_terms_cancelled_groups_and_known_zero_sectors() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).terminals.clear();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [0, 0, 1], c.index(2).unwrap())])];
    let (out, stats) = collect(&p, [1, 0, 0], [Some(1), Some(0), Some(0)], Some(10));
    assert!(out.problems.is_empty() && out.edges.is_empty());
    assert_eq!(stats.zero_terms, 1);
    assert_eq!(partition(stats), [0; 3]);

    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], c.integer(1)),
            term(&c, [-1, 0, 0], c.integer(-1)),
        ],
    )];
    let (out, stats) = collect(&p, [1, 0, 0], [Some(1), Some(0), Some(0)], Some(10));
    assert!(out.problems.is_empty() && out.edges.is_empty());
    assert_eq!(stats.cancelled_groups, 1);
    assert_eq!(partition(stats), [0; 3]);

    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .insert([false, true, false]);
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.integer(1))])];
    let (out, stats) = collect(&p, [0; 3], [Some(0); 3], Some(10));
    assert!(out.problems.is_empty() && out.edges.is_empty());
    assert_eq!(stats.zero_sector_groups, 1);
    assert_eq!(partition(stats), [0; 3]);
}

#[test]
fn support_transitions_remain_provisional_when_later_group_has_a_problem() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], c.integer(1)),
            term(&c, [0; 3], c.integer(1)),
        ],
    )];
    let (out, stats) = collect(&p, [1, 0, 0], [Some(1), Some(0), Some(0)], Some(10));
    assert_eq!(out.edges.len(), 1);
    assert_eq!(out.problems.len(), 1);
    assert_eq!(partition(stats), [1, 0, 0]);
    assert_eq!(out.finished, [(1, 1)]);
}

#[test]
fn support_transitions_partition_attempts_on_event_limit_consumer_stop_and_cancel() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.integer(1))])];
    for mode in 0..3 {
        let cancellation = AtomicBool::new(false);
        let mut delivered = 0;
        let mut finished = 0;
        let limits = OwnerAppliedLimits {
            max_events: if mode == 0 { 1 } else { usize::MAX },
            ..Default::default()
        };
        let error = p
            .visit_owner_applied_successors(
                OWNER,
                &[1, 0, 0],
                &[Some(1), Some(0), Some(0)],
                Some(10),
                limits,
                &cancellation,
                |event| {
                    if let OwnerAppliedEvent::Successor(_) = event {
                        delivered += 1;
                        if mode == 1 {
                            return ControlFlow::Break(());
                        }
                        if mode == 2 {
                            cancellation.store(true, Ordering::Release);
                        }
                    }
                    if let OwnerAppliedEvent::RuleFinished { .. } = event {
                        finished += 1;
                    }
                    ControlFlow::Continue(())
                },
            )
            .unwrap_err();
        match mode {
            0 => assert!(matches!(
                error.failure,
                OwnerAppliedFailure::ResourceLimit {
                    resource: "events",
                    ..
                }
            )),
            1 => assert_eq!(error.failure, OwnerAppliedFailure::StoppedByConsumer),
            2 => assert_eq!(error.failure, OwnerAppliedFailure::Cancelled),
            _ => unreachable!(),
        }
        assert_eq!(delivered, usize::from(mode != 0));
        assert_eq!(finished, 0);
        assert_eq!(partition(error.stats), [1, 0, 0]);
    }
}
