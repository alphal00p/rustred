//! Finite-coordinate partitions preserve ordinary first-rule dispatch. These
//! fixtures claim neither IBP provenance nor recursive successor coverage.
use super::*;

fn limits(faces: usize) -> OwnerDomainMatchLimits {
    OwnerDomainMatchLimits {
        max_bounded_refinement_cells: faces,
        refinement_axes: OwnerDomainRefinementAxes::FiniteAxes,
        ..Default::default()
    }
}

fn diagonal(stage: usize) -> Arc<CandidateOwnerPrograms<3>> {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let guard = poly(
        &c,
        c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
    );
    let mut first = rule(0);
    match stage {
        0 => {
            Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
                .unwrap()
                .shared
                .source_conditions = vec![guard];
        }
        1 => first.equalities.push(guard),
        2 => first.exceptions.push(vec![guard]),
        3 => first.rhs.push(PreparedTerm {
            shift: [0; 3],
            coefficient: c.zero(),
            denominator: guard,
        }),
        _ => unreachable!(),
    }
    batch(&mut p).rules = vec![first, rule(1)];
    p
}

fn parity(
    p: &CandidateOwnerPrograms<3>,
    pieces: &[OwnerDomainMatchPiece<3>],
    lower: [u64; 3],
    upper: [u64; 3],
    rank: Option<u32>,
) {
    for a in lower[0]..=upper[0] {
        for b in lower[1]..=upper[1] {
            for k in lower[2]..=upper[2] {
                if rank.is_some_and(|r| k > u64::from(r)) {
                    continue;
                }
                assert_eq!(
                    at(pieces, [a, b, k]),
                    concrete(
                        p,
                        IntegralKey::try_new([a as i64 + 1, b as i64 + 1, -(k as i64)]).unwrap()
                    ),
                    "point {:?}",
                    [a, b, k]
                );
            }
        }
    }
}

#[test]
fn finite_positive_refinement_default_preserved_and_every_guard_stage_matches_explicit_native() {
    assert_eq!(
        OwnerDomainMatchLimits::default().refinement_axes,
        OwnerDomainRefinementAxes::InactiveOnly
    );
    // Positive lower bounds exceed the inactive minimum (zero). R=0 must not
    // bound/subtract those positive coordinates; R=11 must remain above entry R10.
    for stage in 0..4 {
        for rank in [None, Some(0), Some(11)] {
            let p = diagonal(stage);
            let lower = [4, 4, 0];
            let upper = [Some(6), Some(6), Some(0)];
            let (old, unknown) = refined(
                &p,
                lower,
                upper,
                rank,
                OwnerDomainMatchLimits {
                    max_bounded_refinement_cells: 100,
                    ..Default::default()
                },
            );
            assert_eq!((old.refinement_cells, old.refinement_steps), (0, 0));
            assert_eq!(unknown.len(), 1);
            assert!(matches!(
                unknown[0].disposition(),
                OwnerDomainMatchDisposition::Unresolved { .. }
            ));
            let (stats, automatic) = refined(&p, lower, upper, rank, limits(3));
            assert_eq!((stats.refinement_cells, stats.refinement_steps), (3, 1));
            let mut explicit = Vec::new();
            for a in 4..=6 {
                explicit.extend(collect(&p, &[a, 4, 0], &[Some(a), Some(6), Some(0)], rank));
            }
            assert_eq!(normalized(&automatic), normalized(&explicit));
            assert!(
                automatic
                    .iter()
                    .all(|piece| piece.max_numerator_rank() == rank)
            );
            parity(&p, &automatic, lower, [6, 6, 0], rank);
        }
    }
}

#[test]
fn finite_positive_refinement_unbounded_positive_axes_never_borrow_rank_bounds() {
    let p = diagonal(1);
    for rank in [None, Some(0), Some(11)] {
        let (stats, pieces) = refined(&p, [0; 3], [None, None, Some(0)], rank, limits(1000));
        assert_eq!((stats.refinement_cells, stats.refinement_steps), (0, 0));
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].upper(), [None, None, Some(0)]);
        assert_eq!(pieces[0].max_numerator_rank(), rank);
        assert_eq!(
            pieces[0].disposition(),
            OwnerDomainMatchDisposition::Unresolved {
                predicate: OwnerDomainPredicate::Equality {
                    batch: 0,
                    rule: 0,
                    ordinal: 0
                }
            }
        );
    }
}

#[test]
fn finite_positive_refinement_mixed_axes_choose_smallest_width_then_original_axis() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.equalities.push(poly(
        &c,
        c.add(
            &c.add(
                &c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
                &c.index(2).unwrap(),
            )
            .unwrap(),
            &c.one(),
        )
        .unwrap(),
    ));
    batch(&mut p).rules = vec![first, rule(1)];
    // x0 width3 ties the actual inactive width3; x1 width5 loses. The finite
    // positive axis wins the tie without modifying the inactive simplex.
    let (stats, pieces) = refined(&p, [4, 4, 1], [Some(6), Some(8), None], Some(3), limits(3));
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (3, 1));
    assert_eq!(pieces.len(), 3);
    for (i, piece) in pieces.iter().enumerate() {
        assert_eq!(piece.lower()[0], 4 + i as u64);
        assert_eq!(piece.upper()[0], Some(4 + i as u64));
        assert_eq!(piece.upper()[2], None);
        assert_eq!(piece.max_numerator_rank(), Some(3));
        assert!(matches!(
            piece.disposition(),
            OwnerDomainMatchDisposition::Unresolved { .. }
        ));
    }
    // Now inactive width2 is genuinely smaller. Existing residual-rank
    // tightening still participates; no finite upper needs to be supplied.
    let (stats, pieces) = refined(&p, [4, 4, 1], [Some(6), Some(8), None], Some(2), limits(2));
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (2, 1));
    assert_eq!(pieces.len(), 2);
    for (i, piece) in pieces.iter().enumerate() {
        assert_eq!(piece.lower()[2], 1 + i as u64);
        assert_eq!(piece.upper()[2], Some(1 + i as u64));
        assert_eq!(piece.upper()[0], Some(6));
    }
}

#[test]
fn finite_positive_refinement_full_split_budget_is_transactional() {
    let p = diagonal(1);
    let lower = [1, 1, 0];
    let upper = [Some(3), Some(3), Some(0)];
    let (baseline, original) = refined(&p, lower, upper, Some(0), limits(0));
    for cap in [
        limits(2),
        OwnerDomainMatchLimits {
            max_cells: baseline.cells,
            ..limits(3)
        },
        OwnerDomainMatchLimits {
            max_coordinate_cells: baseline.coordinate_cells,
            ..limits(3)
        },
    ] {
        let (stats, pieces) = refined(&p, lower, upper, Some(0), cap);
        assert_eq!((stats.refinement_cells, stats.refinement_steps), (0, 0));
        assert_eq!(stats.predicates, baseline.predicates);
        assert_eq!(normalized(&pieces), normalized(&original));
    }
}

#[test]
fn finite_positive_refinement_preflight_retry_keeps_original_phase_and_spent_work() {
    for stage in 0..4 {
        let p = diagonal(stage);
        let lower = [1, 1, 0];
        let upper = [Some(3), Some(3), Some(0)];
        let mut cap = limits(2);
        cap.guard_algebra.max_gcd_factor_work = 64;
        let error = p
            .visit_owner_domain_matches(
                OWNER,
                &lower,
                &upper,
                Some(0),
                cap,
                &AtomicBool::new(false),
                |_| panic!("an unadmitted split must not publish faces"),
            )
            .unwrap_err();
        assert_eq!(
            error.failure,
            OwnerDomainMatchFailure::Algebra(crate::algebra::IndexedAlgebraError::ResourceLimit {
                resource: "guard separable factor work",
                requested: 128,
                limit: 64,
            })
        );
        let expected = match stage {
            0 => OwnerDomainPredicate::SourceCondition { ordinal: 0 },
            1 => OwnerDomainPredicate::Equality {
                batch: 0,
                rule: 0,
                ordinal: 0,
            },
            2 => OwnerDomainPredicate::ExcludedConjunction {
                batch: 0,
                rule: 0,
                branch: 0,
                ordinal: 0,
            },
            _ => OwnerDomainPredicate::OriginalDenominator {
                batch: 0,
                rule: 0,
                term: 0,
            },
        };
        assert_eq!(error.predicate, Some(expected));
        assert_eq!(error.predicate_lower(), Some(lower.as_slice()));
        assert_eq!(error.predicate_upper(), Some(upper.as_slice()));
        assert_eq!(error.stats.refinement_cells, 0);
        cap.max_bounded_refinement_cells = 3;
        let (stats, automatic) = refined(&p, lower, upper, Some(0), cap);
        let mut explicit = Vec::new();
        let mut predicates = 0;
        cap.max_bounded_refinement_cells = 0;
        for a in 1..=3 {
            let (s, pieces) = refined(&p, [a, 1, 0], [Some(a), Some(3), Some(0)], Some(0), cap);
            predicates += s.predicates;
            explicit.extend(pieces);
        }
        assert_eq!(normalized(&automatic), normalized(&explicit));
        assert_eq!(stats.predicates, predicates + 1);
        assert_eq!((stats.refinement_cells, stats.refinement_steps), (3, 1));
        parity(&p, &automatic, lower, [3, 3, 0], Some(0));
    }
}

#[test]
fn finite_positive_refinement_preserves_nonlinear_guard_whole_and_pole_and_source_priority() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions = vec![minus(&c, 0, 5)];
    let mut first = rule(0);
    first.equalities.push(poly(
        &c,
        c.sub(
            &c.mul(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.integer(12),
        )
        .unwrap(),
    ));
    first
        .exceptions
        .push(vec![minus(&c, 0, 2), minus(&c, 1, 6)]);
    // Even an identically zero original term retains its denominator guard.
    first.rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: minus(&c, 0, 3),
    });
    batch(&mut p).rules = vec![first, rule(1)];
    let (_, pieces) = refined(
        &p,
        [0; 3],
        [Some(5), Some(5), Some(0)],
        Some(0),
        limits(100),
    );
    parity(&p, &pieces, [0; 3], [5, 5, 0], Some(0));
    assert_eq!(
        at(&pieces, [1, 5, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
    assert_eq!(
        at(&pieces, [2, 3, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
    assert_eq!(
        at(&pieces, [3, 2, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    assert_eq!(
        at(&pieces, [4, 0, 0]),
        OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal: 0 }
    );
}

#[test]
fn finite_positive_refinement_prepaid_lazy_faces_keep_cancellation_prefix() {
    let p = diagonal(1);
    let cancel = AtomicBool::new(false);
    let mut callbacks = 0;
    let error = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[Some(999), None, Some(0)],
            Some(0),
            limits(1000),
            &cancel,
            |_| {
                callbacks += 1;
                cancel.store(true, Ordering::Release);
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert_eq!(error.failure, OwnerDomainMatchFailure::Cancelled);
    assert_eq!(callbacks, 1);
    assert_eq!(
        (error.stats.refinement_cells, error.stats.refinement_steps),
        (1000, 1)
    );
    assert!(error.stats.predicates < 10);
}

#[test]
fn finite_positive_refinement_large_fixed_coordinates_remain_unknown_not_truncated() {
    let p = diagonal(1);
    let huge = i64::MAX as u64;
    let (stats, pieces) = refined(
        &p,
        [huge, huge, 0],
        [Some(huge + 1), Some(huge + 1), Some(0)],
        Some(0),
        limits(2),
    );
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (2, 1));
    assert_eq!(pieces.len(), 2);
    for (i, piece) in pieces.iter().enumerate() {
        assert_eq!(piece.lower()[0], huge + i as u64);
        assert_eq!(piece.upper()[0], Some(huge + i as u64));
        assert_eq!(
            piece.disposition(),
            OwnerDomainMatchDisposition::Unresolved {
                predicate: OwnerDomainPredicate::Equality {
                    batch: 0,
                    rule: 0,
                    ordinal: 0
                },
            }
        );
    }
}

#[test]
fn finite_positive_refinement_full_u64_width_does_not_wrap() {
    let p = diagonal(1);
    let (stats, pieces) = refined(
        &p,
        [0; 3],
        [Some(u64::MAX), Some(u64::MAX), Some(0)],
        Some(0),
        limits(usize::MAX),
    );
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (0, 0));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].upper(), [Some(u64::MAX), Some(u64::MAX), Some(0)]);
    assert!(matches!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    ));
}

#[test]
fn finite_positive_refinement_exact_gaps_and_global_refusals_are_not_bypassed() {
    let mut p = diagonal(1);
    batch(&mut p).rules.truncate(1);
    let (_, pieces) = refined(&p, [0; 3], [Some(2), Some(2), Some(0)], Some(0), limits(3));
    parity(&p, &pieces, [0; 3], [2, 2, 0], Some(0));
    assert_eq!(
        at(&pieces, [0, 1, 0]),
        OwnerDomainMatchDisposition::ExactGap
    );
    assert_eq!(
        at(&pieces, [1, 1, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    let error = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[Some(2), Some(2), Some(0)],
            Some(0),
            OwnerDomainMatchLimits {
                max_predicates: 0,
                ..limits(3)
            },
            &AtomicBool::new(false),
            |_| panic!("global refusal must not publish a gap or selected rule"),
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        OwnerDomainMatchFailure::ResourceLimit {
            resource: "predicates",
            requested: 1,
            limit: 0
        }
    );
    assert_eq!(error.stats.refinement_cells, 0);
    assert_eq!(
        error.predicate,
        Some(OwnerDomainPredicate::Equality {
            batch: 0,
            rule: 0,
            ordinal: 0
        })
    );
}
