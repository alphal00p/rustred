//! Actual native restriction/application; these synthetic rules are not IBPs.
use super::*;
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;

fn refined(max: usize) -> OwnerAppliedLimits {
    OwnerAppliedLimits {
        cell_refinement: OwnerAppliedCellRefinement::SingleFiniteAxis {
            max_cardinality: NonZeroUsize::new(max).unwrap(),
        },
        ..Default::default()
    }
}

fn run(
    p: &CandidateOwnerPrograms<3>,
    lower: [u64; 3],
    upper: [Option<u64>; 3],
    limits: OwnerAppliedLimits,
) -> (Collected, OwnerAppliedStats) {
    let mut out = Collected::default();
    let stats = p
        .visit_owner_applied_successors(
            OWNER,
            &lower,
            &upper,
            Some(10),
            limits,
            &AtomicBool::new(false),
            |event| {
                match event {
                    OwnerAppliedEvent::Classified(piece) => {
                        out.classified.push(piece.disposition())
                    }
                    OwnerAppliedEvent::Successor(edge) => out.edges.push(Edge {
                        source_lower: edge.source_lower.to_vec(),
                        source_upper: edge.source_upper.to_vec(),
                        target: *edge.target_sector,
                        lower: edge.target_lower.to_vec(),
                        upper: edge.target_upper.to_vec(),
                        rank: edge.target_rank_limit,
                        coefficient: edge.coefficient.clone(),
                        nonzero: edge.coefficient_nonzero,
                    }),
                    OwnerAppliedEvent::Problem(problem) => out.problems.push(problem.kind),
                    OwnerAppliedEvent::RuleFinished {
                        successors,
                        problems,
                        ..
                    } => out.finished.push((successors, problems)),
                    OwnerAppliedEvent::OptionalCoefficientRefusal { .. } => {
                        panic!("small exact fixture refused optional algebra")
                    }
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    (out, stats)
}

#[test]
fn application_cell_refinement_matches_once_keeps_original_piece_and_exact_sources() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(7, vec![term(&c, [-1, 0, 0], c.index(0).unwrap())])];
    let lower = [1, 0, 0];
    let upper = [Some(2), Some(0), Some(0)];
    let (broad, broad_stats) = run(&p, lower, upper, Default::default());
    let (out, stats) = run(&p, lower, upper, refined(2));
    assert_eq!(out.classified, broad.classified);
    assert_eq!(out.classified.len(), 1);
    assert_eq!(stats.matching, broad_stats.matching);
    assert_eq!(stats.selected_pieces, 1);
    assert_eq!(out.finished, [(2, 0)]);
    assert_eq!(stats.application_refinement_steps, 1);
    assert_eq!(stats.application_refinement_cells, 2);
    assert_eq!(stats.boundary_cells, 2);
    assert!(out.problems.is_empty());
    for (i, edge) in out.edges.iter().enumerate() {
        assert_eq!(edge.source_lower, [i as u64 + 1, 0, 0]);
        assert_eq!(
            edge.source_upper,
            edge.source_lower
                .iter()
                .copied()
                .map(Some)
                .collect::<Vec<_>>()
        );
        assert_eq!(edge.coefficient, c.integer(i as i64 + 2));
        assert_eq!(edge.nonzero, OwnerAppliedNonzero::Uniform);
        assert_eq!(edge.lower, [i as u64, 0, 0]);
    }
    let mut selected = 0;
    p.visit_owner_applied_successors(
        OWNER,
        &lower,
        &upper,
        Some(10),
        refined(2),
        &AtomicBool::new(false),
        |event| {
            if let OwnerAppliedEvent::Successor(edge) = event {
                assert_eq!(edge.source.lower(), lower);
                assert_eq!(edge.source.upper(), upper);
                assert_eq!(
                    edge.source.disposition(),
                    OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 7 }
                );
                selected += 1;
            }
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    assert_eq!(selected, 2);
}

#[test]
fn application_cell_refinement_off_or_ineligible_preserves_full_native_output_and_work() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.index(0).unwrap())])];
    for (lower, upper, max) in [
        ([1, 0, 0], [Some(2), Some(0), Some(0)], 1),
        ([1, 0, 0], [Some(3), Some(0), Some(0)], 2),
        ([1, 0, 0], [None, Some(0), Some(0)], 2),
        ([1, 0, 0], [Some(2), Some(1), Some(0)], 2),
        ([1, 0, 0], [Some(1), Some(0), Some(0)], 2),
    ] {
        let (reference, expected) = run(&p, lower, upper, Default::default());
        let (actual, stats) = run(&p, lower, upper, refined(max));
        assert_eq!(stats, expected);
        assert_eq!(format!("{actual:?}"), format!("{reference:?}"));
    }
}

#[test]
fn application_cell_refinement_keeps_zero_faces_and_never_coalesces_across_children() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let n = c.index(0).unwrap();
    let crossing = c.sub(&n, &c.integer(3)).unwrap();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], crossing)])];
    let (out, stats) = run(&p, [1, 0, 0], [Some(3), Some(0), Some(0)], refined(3));
    assert_eq!(out.edges.len(), 2);
    assert_eq!(out.edges[0].coefficient, c.integer(-1));
    assert_eq!(out.edges[1].coefficient, c.integer(1));
    assert_eq!(out.edges[0].source_lower[0], 1);
    assert_eq!(out.edges[1].source_lower[0], 3);
    assert_eq!(stats.zero_terms, 1);
    assert_eq!(stats.cancelled_groups, 0);
    assert_eq!(out.finished, [(2, 0)]);
    let mut expected = Vec::new();
    for offset in 1..=3 {
        let (point, _) = run(
            &p,
            [offset, 0, 0],
            [Some(offset), Some(0), Some(0)],
            Default::default(),
        );
        expected.extend(point.edges);
    }
    assert_eq!(format!("{:?}", out.edges), format!("{expected:?}"));
    // Equal and opposite terms cancel only within each source cell's complete
    // shift group; neither those terms nor different source faces are jobs.
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], n.clone()),
            term(&c, [-1, 0, 0], c.sub(&c.zero(), &n).unwrap()),
        ],
    )];
    let (out, stats) = run(&p, [1, 0, 0], [Some(2), Some(0), Some(0)], refined(2));
    assert!(out.edges.is_empty() && out.problems.is_empty());
    assert_eq!((stats.coalescing_additions, stats.cancelled_groups), (2, 2));
    assert_eq!(out.finished, [(0, 0)]);
}

#[test]
fn application_cell_refinement_keeps_original_denominators_and_child_validity() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut r = rule(0, vec![term(&c, [-1, 0, 0], c.integer(1))]);
    r.rhs[0].denominator = poly(&c, &c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap());
    batch(&mut p).rules = vec![r];
    let (out, stats) = run(&p, [1, 0, 0], [Some(3), Some(0), Some(0)], refined(3));
    assert!(
        out.classified
            .contains(&OwnerDomainMatchDisposition::ExactGap)
    );
    assert!(out.edges.iter().all(|edge| edge.source_lower[0] != 2));
    assert_eq!(stats.application_refinement_steps, 0); // matcher already split valid faces

    // A cancelling group still cannot suppress each original nonzero term's
    // invalid self-dependency/descent check.
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [0; 3], c.integer(1)),
            term(&c, [0; 3], c.integer(-1)),
        ],
    )];
    let (out, stats) = run(&p, [1, 0, 0], [Some(2), Some(0), Some(0)], refined(2));
    assert_eq!(out.problems.len(), 2);
    assert!(
        out.problems
            .iter()
            .all(|p| matches!(p, OwnerAppliedProblemKind::DescentNotEstablished { .. }))
    );
    assert!(out.edges.is_empty());
    assert_eq!((stats.coalescing_additions, stats.cancelled_groups), (0, 0));
    assert_eq!(out.finished, [(0, 2)]);
}

#[test]
fn application_cell_refinement_keeps_shifted_source_guards_and_original_validity() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let condition = c.sub(&c.index(0).unwrap(), &c.integer(1)).unwrap();
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions = vec![poly(&c, &condition)];
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.integer(1))])];
    // Both original sources n0=2,3 satisfy n0-1 != 0. Their shifted children
    // n0=1,2 do not both satisfy it: this check must still occur per face.
    let (out, stats) = run(&p, [1, 0, 0], [Some(2), Some(0), Some(0)], refined(2));
    assert_eq!(
        out.problems,
        [OwnerAppliedProblemKind::InvalidChildSourceCondition { ordinal: 0 }]
    );
    assert_eq!(out.edges.len(), 1);
    assert_eq!(out.edges[0].source_lower, [2, 0, 0]);
    assert_eq!(out.finished, [(1, 1)]);
    assert_eq!(stats.application_refinement_cells, 2);
}

#[test]
fn application_cell_refinement_affine_adapter_preserves_complete_event_and_work_trace() {
    use crate::solver::candidate_reduction::owners::domains::OwnerDomainMatchPiece;
    use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.index(0).unwrap())])];
    // The affine condition is already satisfied on fixed axes; axis 0 is the
    // sole remaining nonfixed finite coordinate and would otherwise qualify.
    let condition = c
        .sub(
            &c.add(&c.index(1).unwrap(), &c.index(2).unwrap()).unwrap(),
            &c.integer(1),
        )
        .unwrap();
    let equation = poly(&c, &condition);
    let AffineIntersection::Affine(affine) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &[equation.raw().clone()],
        p.context.index_variables(),
        &OWNER,
    )
    .unwrap() else {
        panic!("affine fixture must retain a coupled chart")
    };
    let piece = OwnerDomainMatchPiece::guarded_candidate(
        OWNER,
        crate::foundry::completion::LatticeBox::try_new([1, 0, 0], [Some(2), Some(0), Some(0)])
            .unwrap(),
        Some(10),
        0,
        0,
    );
    let cancel = AtomicBool::new(false);
    let mut results = Vec::new();
    for limits in [OwnerAppliedLimits::default(), refined(2)] {
        let mut budget = engine::Budget {
            limits,
            stats: Default::default(),
            cancel: &cancel,
        };
        let mut events = Vec::new();
        p.apply_piece(&piece, Some(&affine), &mut budget, &mut |event| {
            events.push(format!("{event:?}"));
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(budget.stats.application_refinement_steps, 0);
        results.push((events, budget.stats));
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn application_cell_refinement_keeps_conditional_unknowns_on_ineligible_sources() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(
        0,
        vec![term(
            &c,
            [-1, 0, 0],
            c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap(),
        )],
    )];
    let (off, expected) = run(&p, [1, 0, 0], [None, Some(0), Some(0)], Default::default());
    let (on, actual) = run(&p, [1, 0, 0], [None, Some(0), Some(0)], refined(2));
    assert_eq!(actual, expected);
    assert_eq!(format!("{off:?}"), format!("{on:?}"));
    assert_eq!(on.edges[0].nonzero, OwnerAppliedNonzero::Conditional);
}

#[test]
fn application_cell_refinement_never_resets_finite_work_or_falls_back_after_a_prefix() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.integer(1))])];
    for (limits, resource, expected_edges) in [
        (
            OwnerAppliedLimits {
                max_boundary_cells: 1,
                ..refined(2)
            },
            "boundary cells",
            0,
        ),
        (
            OwnerAppliedLimits {
                max_term_visits: 1,
                ..refined(2)
            },
            "RHS term visits",
            1,
        ),
        (
            OwnerAppliedLimits {
                max_native_operations: 2,
                ..refined(2)
            },
            "native operations",
            1,
        ),
        (
            OwnerAppliedLimits {
                max_events: 2,
                ..refined(2)
            },
            "events",
            1,
        ),
    ] {
        let mut edges = 0;
        let mut finished = 0;
        let error = p
            .visit_owner_applied_successors(
                OWNER,
                &[1, 0, 0],
                &[Some(2), Some(0), Some(0)],
                Some(10),
                limits,
                &AtomicBool::new(false),
                |event| {
                    match event {
                        OwnerAppliedEvent::Successor(_) => edges += 1,
                        OwnerAppliedEvent::RuleFinished { .. } => finished += 1,
                        _ => {}
                    }
                    ControlFlow::Continue(())
                },
            )
            .unwrap_err();
        assert!(
            matches!(error.failure, OwnerAppliedFailure::ResourceLimit { resource: actual, .. } if actual == resource)
        );
        assert_eq!(edges, expected_edges);
        assert_eq!(finished, 0);
        assert!(error.stats.term_visits <= limits.max_term_visits);
        assert!(error.stats.native_operations <= limits.max_native_operations);
        assert!(error.stats.events <= limits.max_events);
    }
}

#[test]
fn application_cell_refinement_cancel_or_consumer_stop_cannot_finish_or_repeat_parent() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.integer(1))])];
    for consumer_stop in [false, true] {
        let cancel = AtomicBool::new(false);
        let mut edges = 0;
        let error = p
            .visit_owner_applied_successors(
                OWNER,
                &[1, 0, 0],
                &[Some(2), Some(0), Some(0)],
                Some(10),
                refined(2),
                &cancel,
                |event| {
                    match event {
                        OwnerAppliedEvent::Successor(_) => {
                            edges += 1;
                            if consumer_stop {
                                return ControlFlow::Break(());
                            }
                            cancel.store(true, Ordering::Release);
                        }
                        OwnerAppliedEvent::RuleFinished { .. } => {
                            panic!("partial source cannot finish")
                        }
                        _ => {}
                    }
                    ControlFlow::Continue(())
                },
            )
            .unwrap_err();
        assert_eq!(
            error.failure,
            if consumer_stop {
                OwnerAppliedFailure::StoppedByConsumer
            } else {
                OwnerAppliedFailure::Cancelled
            }
        );
        assert_eq!(edges, 1);
        assert_eq!(error.stats.application_refinement_cells, 1);
        assert_eq!(error.stats.application_refinement_steps, 1);
    }
}
