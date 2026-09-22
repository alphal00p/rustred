//! Same-candidate rejection preserves first-applicable dispatch, not an
//! alternate sufficient-rule policy or a claim to solve positive diagonals.
use super::*;

#[path = "rejection_refinement.rs"]
mod refinement;

fn diagonal(c: &IndexedCoefficientContext) -> IndexedPolynomial {
    poly(
        c,
        c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
    )
}
fn positive(c: &IndexedCoefficientContext) -> IndexedPolynomial {
    poly(
        c,
        c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
    )
}
fn quadratic(c: &IndexedCoefficientContext) -> IndexedPolynomial {
    poly(
        c,
        c.mul(
            &c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap(),
            &c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap(),
        )
        .unwrap(),
    )
}
fn conservative(c: &IndexedCoefficientContext) -> IndexedPolynomial {
    let a = c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap();
    let b = c.sub(&c.index(1).unwrap(), &c.integer(2)).unwrap();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    poly(c, c.add(&a, &c.mul(&d, &b).unwrap()).unwrap())
}
fn equality(ordinal: usize) -> OwnerDomainPredicate {
    OwnerDomainPredicate::Equality {
        batch: 0,
        rule: 17,
        ordinal,
    }
}
fn excluded(branch: usize, ordinal: usize) -> OwnerDomainPredicate {
    OwnerDomainPredicate::ExcludedConjunction {
        batch: 0,
        rule: 17,
        branch,
        ordinal,
    }
}
fn selected() -> OwnerDomainMatchDisposition {
    OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 29 }
}
fn install(p: &mut Arc<CandidateOwnerPrograms<3>>, first: PreparedRule<3>) {
    batch(p).rules = vec![first, rule(29)];
}
fn parity(p: &CandidateOwnerPrograms<3>, pieces: &[OwnerDomainMatchPiece<3>], rank: u64) {
    for x in 0..=3 {
        for y in 0..=3 {
            for z in 0..=rank {
                assert_eq!(
                    at(pieces, [x, y, z]),
                    concrete(
                        p,
                        IntegralKey::try_new([x as i64 + 1, y as i64 + 1, -(z as i64)]).unwrap()
                    )
                );
            }
        }
    }
}
fn refusal(p: &CandidateOwnerPrograms<3>, limits: OwnerDomainMatchLimits) -> OwnerDomainMatchError {
    p.visit_owner_domain_matches(
        OWNER,
        &[0; 3],
        &[None, None, Some(0)],
        Some(11),
        limits,
        &AtomicBool::new(false),
        |_| panic!("failed lookahead must not publish"),
    )
    .unwrap_err()
}

#[test]
fn rejection_lookahead_saved_diagonal_obstructions_reject_without_solving_the_diagonal() {
    for equality_first in [false, true] {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        let mut first = rule(17); // saved ordinal deliberately differs from index
        if equality_first {
            first.equalities.push(diagonal(&c));
        } else {
            first.exceptions.push(vec![diagonal(&c)]);
        }
        // Synthetic equivalent of the two saved obstructions: an unresolved
        // positive diagonal followed by a whole exclusion n_inactive=0 on its
        // fixed-zero face. This models the actual later [n13] / [n0] branches,
        // without baking any saved topology, ordinal or dimensionality in code.
        first.exceptions.push(vec![poly(&c, c.index(2).unwrap())]);
        install(&mut p, first);
        let (stats, pieces) = refined(
            &p,
            [0; 3],
            [None, None, Some(0)],
            Some(11),
            Default::default(),
        );
        assert_eq!(stats.predicates, 2);
        assert_eq!(stats.rules, 2);
        assert_eq!((stats.refinement_cells, stats.split_operations), (0, 0));
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].disposition(), selected());
        assert_eq!(pieces[0].lower(), &[0; 3]);
        assert_eq!(pieces[0].upper(), &[None, None, Some(0)]);
        assert_eq!(pieces[0].max_numerator_rank(), Some(11));
        parity(&p, &pieces, 0);
    }
}

#[test]
fn rejection_lookahead_fixed_positive_zero_and_later_nonzero_equality_are_generic() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17);
    first.equalities.push(poly(
        &c,
        c.sub(
            &c.add(&c.index(0).unwrap(), &c.index(2).unwrap()).unwrap(),
            &c.integer(1),
        )
        .unwrap(),
    ));
    first.exceptions.push(vec![minus(&c, 1, 1)]);
    install(&mut p, first);
    let pieces = collect(&p, &[0; 3], &[None, Some(0), None], Some(11));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].disposition(), selected());
    assert_eq!(pieces[0].max_numerator_rank(), Some(11));
    for x in 0..=3 {
        for z in 0..=11 {
            assert_eq!(
                at(&pieces, [x, 0, z]),
                concrete(
                    &p,
                    IntegralKey::try_new([x as i64 + 1, 1, -(z as i64)]).unwrap()
                )
            );
        }
    }

    let mut first = rule(17);
    first.equalities = vec![diagonal(&c), conservative(&c), positive(&c)];
    install(&mut p, first);
    let (stats, pieces) = refined(&p, [0; 3], [None; 3], Some(11), Default::default());
    assert_eq!(stats.predicates, 3);
    assert_eq!(stats.split_operations, 0); // discard speculative plane cuts
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].disposition(), selected());
    parity(&p, &pieces, 11);
}

#[test]
fn rejection_lookahead_requires_the_whole_excluded_conjunction_uniformly_zero() {
    for kind in 0..6 {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        let mut first = rule(17);
        first.equalities.push(diagonal(&c));
        let other = match kind {
            0 => poly(&c, c.zero()),
            1 => positive(&c),
            2 => diagonal(&c),
            3 => minus(&c, 0, 2),
            _ => conservative(&c),
        };
        first.exceptions.push(if kind == 5 {
            vec![]
        } else {
            vec![poly(&c, c.index(2).unwrap()), other]
        });
        install(&mut p, first);
        let (stats, pieces) = refined(
            &p,
            [0; 3],
            [None, None, Some(0)],
            Some(11),
            Default::default(),
        );
        assert_eq!(stats.split_operations, 0);
        assert_eq!(pieces.len(), 1);
        assert_eq!(
            pieces[0].disposition(),
            if kind == 0 || kind == 5 {
                selected()
            } else {
                OwnerDomainMatchDisposition::Unresolved {
                    predicate: equality(0),
                }
            }
        );
    }
}

#[test]
fn rejection_lookahead_conservative_interiors_reject_before_exact_dispatch_exhaustion() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17);
    first.equalities.push(conservative(&c));
    first.exceptions.push(vec![poly(&c, c.index(2).unwrap())]);
    // Outside the conservative cover the equality already fails. Inside it,
    // only the separate uniformly zero whole exclusion permits rejection.
    batch(&mut p).rules = vec![first];
    let pieces = collect(&p, &[0; 3], &[None, None, Some(0)], Some(11));
    assert!(!pieces.is_empty());
    assert!(
        pieces
            .iter()
            .all(|piece| piece.disposition() == OwnerDomainMatchDisposition::ExactGap)
    );
    assert!(
        pieces
            .iter()
            .all(|piece| piece.max_numerator_rank() == Some(11))
    );
    parity(&p, &pieces, 0);
}

#[test]
fn rejection_lookahead_checks_original_zero_denominators_but_does_not_bypass_unknown_poles() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17);
    first.equalities.push(diagonal(&c));
    first.rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: poly(&c, c.index(2).unwrap()),
    });
    install(&mut p, first);
    let pieces = collect(&p, &[0; 3], &[None, None, Some(0)], Some(11));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].disposition(), selected());
    parity(&p, &pieces, 0);

    let first = &mut batch(&mut p).rules[0];
    first.equalities.clear();
    first.rhs.insert(
        0,
        PreparedTerm {
            shift: [0; 3],
            coefficient: c.zero(),
            denominator: diagonal(&c),
        },
    );
    let (stats, pieces) = refined(
        &p,
        [0; 3],
        [None, None, Some(0)],
        Some(11),
        Default::default(),
    );
    assert_eq!(stats.predicates, 1); // denominator-stage Unknown is not widened
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::Unresolved {
            predicate: OwnerDomainPredicate::OriginalDenominator {
                batch: 0,
                rule: 17,
                term: 0
            }
        }
    );
}

#[test]
fn rejection_lookahead_uses_current_cut_and_preserves_batch_terminal_priority() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17);
    first.equalities.push(minus(&c, 0, 2));
    first.exceptions.push(vec![poly(
        &c,
        c.sub(
            &c.add(&c.index(1).unwrap(), &c.index(2).unwrap()).unwrap(),
            &c.integer(1),
        )
        .unwrap(),
    )]);
    first.exceptions.push(vec![minus(&c, 0, 2)]);
    install(&mut p, first);
    batch(&mut p)
        .terminals
        .insert(IntegralKey::try_new([1, 1, 0]).unwrap());
    let owner = Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&OWNER)
            .unwrap(),
    )
    .unwrap();
    owner.batches.push(Arc::new(PreparedOwnerBatch {
        rules: vec![],
        terminals: [IntegralKey::try_new([2, 2, 0]).unwrap()].into(),
        coalescing_bound: 0,
        overlay: None,
    }));
    let pieces = collect(&p, &[0; 3], &[None; 3], Some(2));
    parity(&p, &pieces, 2);
    assert_eq!(
        at(&pieces, [0, 0, 0]),
        OwnerDomainMatchDisposition::Terminal { batch: 0 }
    );
    assert_eq!(at(&pieces, [1, 1, 0]), selected()); // later batch terminal cannot shadow
    assert!(!pieces.iter().any(|piece| matches!(
        piece.disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    )));
}

#[test]
fn rejection_lookahead_never_bypasses_source_unknown_or_native_refusal() {
    for source_kind in 0..3 {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        let mut first = rule(17);
        first.equalities.push(diagonal(&c));
        first.exceptions.push(vec![poly(&c, c.zero())]);
        install(&mut p, first);
        Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
            .unwrap()
            .shared
            .source_conditions = vec![match source_kind {
            0 => diagonal(&c),
            1 => poly(&c, c.zero()),
            _ => quadratic(&c),
        }];
        let mut limits = OwnerDomainMatchLimits::default();
        limits.guard_algebra.max_univariate_degree = 1;
        if source_kind == 2 {
            let error = refusal(&p, limits);
            assert_eq!(
                error.predicate,
                Some(OwnerDomainPredicate::SourceCondition { ordinal: 0 })
            );
            assert!(matches!(error.failure, OwnerDomainMatchFailure::Algebra(_)));
            assert_eq!(error.stats.rules, 0);
        } else {
            let (stats, pieces) = refined(&p, [0; 3], [None, None, Some(0)], Some(11), limits);
            assert_eq!(stats.predicates, 1);
            assert_eq!(stats.rules, 0);
            assert_eq!(
                pieces[0].disposition(),
                if source_kind == 0 {
                    OwnerDomainMatchDisposition::Unresolved {
                        predicate: OwnerDomainPredicate::SourceCondition { ordinal: 0 },
                    }
                } else {
                    OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal: 0 }
                }
            );
        }
    }
}

#[test]
fn rejection_lookahead_optional_preflight_is_inconclusive_but_mandatory_error_stays_typed() {
    for stage in 0..4 {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        let mut first = rule(17);
        first.equalities.push(if stage == 0 {
            quadratic(&c)
        } else {
            diagonal(&c)
        });
        let expected = match stage {
            0 => {
                first.exceptions.push(vec![poly(&c, c.zero())]);
                equality(0)
            }
            1 => {
                first.equalities.push(quadratic(&c));
                first.exceptions.push(vec![poly(&c, c.zero())]);
                equality(1)
            }
            2 => {
                first.exceptions = vec![vec![quadratic(&c)], vec![poly(&c, c.zero())]];
                excluded(0, 0)
            }
            _ => {
                first.rhs = vec![
                    PreparedTerm {
                        shift: [0; 3],
                        coefficient: c.zero(),
                        denominator: quadratic(&c),
                    },
                    PreparedTerm {
                        shift: [0; 3],
                        coefficient: c.zero(),
                        denominator: poly(&c, c.zero()),
                    },
                ];
                OwnerDomainPredicate::OriginalDenominator {
                    batch: 0,
                    rule: 17,
                    term: 0,
                }
            }
        };
        install(&mut p, first);
        let mut limits = OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 100,
            ..Default::default()
        };
        limits.guard_algebra.max_univariate_degree = 1;
        if stage != 0 {
            // These are speculative rejection probes behind an unresolved
            // required equality, not mandatory checks establishing a formula.
            // Even the later zero witness is not scanned past a refused probe.
            let (stats, pieces) = refined(&p, [0; 3], [None, None, Some(0)], Some(11), limits);
            assert_eq!(pieces.len(), 1);
            assert_eq!(
                pieces[0].disposition(),
                OwnerDomainMatchDisposition::Unresolved {
                    predicate: equality(0),
                }
            );
            assert_eq!(pieces[0].lower(), &[0; 3]);
            assert_eq!(pieces[0].upper(), &[None, None, Some(0)]);
            assert_eq!(pieces[0].max_numerator_rank(), Some(11));
            assert_eq!(stats.predicates, 2);
            assert_eq!(stats.refinement_cells, 0);
            continue;
        }
        let error = refusal(&p, limits);
        assert_eq!(
            error.failure,
            OwnerDomainMatchFailure::Algebra(crate::algebra::IndexedAlgebraError::ResourceLimit {
                resource: "guard univariate degree",
                requested: 2,
                limit: 1,
            })
        );
        assert_eq!(error.predicate, Some(expected));
        assert_eq!(error.predicate_lower(), Some([0; 3].as_slice()));
        assert_eq!(
            error.predicate_upper(),
            Some([None, None, Some(0)].as_slice())
        );
        assert_eq!(error.max_numerator_rank, Some(11));
        assert_eq!(error.stats.predicates, if stage == 0 { 1 } else { 2 });
        assert_eq!(error.stats.refinement_cells, 0);
        assert_eq!(error.stats.pieces, 0);
    }
}

#[test]
fn rejection_lookahead_cumulative_limits_and_cancellation_are_typed_incomplete() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17);
    first.equalities.push(diagonal(&c));
    install(&mut p, first);
    let (baseline, _) = refined(
        &p,
        [0; 3],
        [None, None, Some(0)],
        Some(11),
        Default::default(),
    );
    batch(&mut p).rules[0]
        .exceptions
        .push(vec![poly(&c, c.zero())]);
    for (limits, resource, increment) in [
        (
            OwnerDomainMatchLimits {
                max_predicates: 1,
                ..Default::default()
            },
            "predicates",
            0,
        ),
        (
            OwnerDomainMatchLimits {
                max_cells: baseline.cells,
                ..Default::default()
            },
            "cells",
            1,
        ),
        (
            OwnerDomainMatchLimits {
                max_coordinate_cells: baseline.coordinate_cells,
                ..Default::default()
            },
            "coordinate cells",
            1,
        ),
    ] {
        let error = refusal(&p, limits);
        assert!(
            matches!(error.failure, OwnerDomainMatchFailure::ResourceLimit { resource: actual, .. } if actual == resource)
        );
        assert_eq!(error.predicate, Some(excluded(0, 0)));
        assert_eq!(error.stats.predicates, baseline.predicates + increment);
        assert_eq!(error.stats.cells, baseline.cells);
        assert_eq!(error.stats.coordinate_cells, baseline.coordinate_cells);
        assert_eq!(error.stats.pieces, 0);
    }
    for consumer_stop in [false, true] {
        let cancel = AtomicBool::new(false);
        let mut callbacks = 0;
        let error = p
            .visit_owner_domain_matches(
                OWNER,
                &[0; 3],
                &[None, None, Some(0)],
                Some(11),
                Default::default(),
                &cancel,
                |piece| {
                    callbacks += 1;
                    assert_eq!(piece.disposition(), selected());
                    if consumer_stop {
                        ControlFlow::Break(())
                    } else {
                        cancel.store(true, Ordering::Release);
                        ControlFlow::Continue(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(callbacks, 1);
        assert_eq!(error.stats.pieces, 1);
        assert_eq!(error.stats.predicates, 2);
        assert_eq!(
            error.failure,
            if consumer_stop {
                OwnerDomainMatchFailure::StoppedByConsumer
            } else {
                OwnerDomainMatchFailure::Cancelled
            }
        );
    }
    let error = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(11),
            Default::default(),
            &AtomicBool::new(true),
            |_| panic!(),
        )
        .unwrap_err();
    assert_eq!(error.failure, OwnerDomainMatchFailure::Cancelled);
    assert_eq!(error.stats, OwnerDomainMatchStats::default());
}

#[test]
fn rejection_lookahead_without_witness_preserves_original_refinement_cursor() {
    let mut p = coupled_refinement_fixture(1);
    let limits = OwnerDomainMatchLimits {
        max_bounded_refinement_cells: 3,
        ..Default::default()
    };
    let (before, expected) = refined(&p, [0; 3], [None, Some(0), None], Some(2), limits);
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules[0].exceptions.push(vec![positive(&c)]);
    let (after, pieces) = refined(&p, [0; 3], [None, Some(0), None], Some(2), limits);
    assert_eq!(normalized(&pieces), normalized(&expected));
    assert_eq!((after.refinement_cells, after.refinement_steps), (3, 1));
    assert_eq!(after.refinement_cells, before.refinement_cells);
    assert!(after.predicates > before.predicates);
}
