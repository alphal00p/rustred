//! Whole excluded conjunctions, not a sufficient-later-rule dispatch policy.
use super::*;

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

fn conservative(c: &IndexedCoefficientContext) -> IndexedPolynomial {
    let a = c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap();
    let b = c.sub(&c.index(1).unwrap(), &c.integer(2)).unwrap();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    poly(c, c.add(&a, &c.mul(&d, &b).unwrap()).unwrap())
}

fn install(p: &mut Arc<CandidateOwnerPrograms<3>>, atoms: Vec<IndexedPolynomial>) {
    // Deliberately different from its vector index: lookahead must resume the
    // same rule and report its saved ordinal, not use that ordinal as an index.
    let mut first = rule(17);
    first.exceptions.push(atoms);
    batch(p).rules = vec![first, rule(29)];
}

fn excluded(ordinal: usize) -> OwnerDomainPredicate {
    OwnerDomainPredicate::ExcludedConjunction {
        batch: 0,
        rule: 17,
        branch: 0,
        ordinal,
    }
}

fn finite_parity(p: &CandidateOwnerPrograms<3>, pieces: &[OwnerDomainMatchPiece<3>], rank: u64) {
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

#[test]
fn excluded_and_lookahead_reordered_atoms_keep_first_rule_positive_tails_and_actual_rank() {
    for nonlinear in [false, true] {
        let mut canonical = None;
        for witness_first in [false, true] {
            let mut p = fixture();
            let c = p.context.coefficient_context().clone();
            let unknown = if nonlinear {
                poly(
                    &c,
                    c.sub(
                        &c.mul(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
                        &c.integer(4),
                    )
                    .unwrap(),
                )
            } else {
                diagonal(&c)
            };
            let atoms = if witness_first {
                vec![positive(&c), unknown]
            } else {
                vec![unknown, positive(&c)]
            };
            install(&mut p, atoms);
            let (stats, pieces) = refined(&p, [0; 3], [None; 3], Some(11), Default::default());
            assert_eq!(stats.predicates, if witness_first { 1 } else { 2 });
            assert_eq!((stats.refinement_cells, stats.refinement_steps), (0, 0));
            assert_eq!(pieces.len(), 1);
            assert_eq!(pieces[0].lower(), &[0; 3]);
            assert_eq!(pieces[0].upper(), &[None; 3]);
            assert_eq!(pieces[0].max_numerator_rank(), Some(11));
            assert_eq!(
                pieces[0].disposition(),
                OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 }
            );
            finite_parity(&p, &pieces, 11);
            if let Some(expected) = &canonical {
                assert_eq!(&normalized(&pieces), expected);
            } else {
                canonical = Some(normalized(&pieces));
            }
        }
    }
}

#[test]
fn excluded_and_lookahead_retains_other_branches_original_poles_and_source_conditions() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    install(&mut p, vec![diagonal(&c), positive(&c)]);
    let first = &mut batch(&mut p).rules[0];
    first.exceptions.push(vec![minus(&c, 0, 2)]);
    // Even a zero RHS coefficient must retain the original denominator pole.
    first.rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: minus(&c, 1, 3),
    });
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions = vec![minus(&c, 0, 4)];
    let pieces = collect(&p, &[0; 3], &[Some(3), Some(3), Some(2)], Some(2));
    finite_parity(&p, &pieces, 2);
    assert_eq!(
        at(&pieces, [0, 0, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 }
    );
    assert_eq!(
        at(&pieces, [1, 0, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 29 }
    );
    assert_eq!(
        at(&pieces, [0, 2, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 29 }
    );
    assert_eq!(
        at(&pieces, [3, 0, 0]),
        OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal: 0 }
    );
}

#[test]
fn excluded_and_lookahead_zero_unknown_and_intersecting_planes_are_not_witnesses() {
    for kind in 0..4 {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        let later = match kind {
            0 => poly(&c, c.zero()),
            1 => diagonal(&c),
            2 => minus(&c, 0, 2),
            _ => conservative(&c),
        };
        install(&mut p, vec![diagonal(&c), later]);
        let (stats, pieces) = refined(&p, [0; 3], [None; 3], Some(11), Default::default());
        assert_eq!(stats.predicates, 2);
        assert_eq!(stats.split_operations, 0, "lookahead must not split planes");
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].lower(), &[0; 3]);
        assert_eq!(pieces[0].upper(), &[None; 3]);
        assert_eq!(pieces[0].max_numerator_rank(), Some(11));
        assert_eq!(
            pieces[0].disposition(),
            OwnerDomainMatchDisposition::Unresolved {
                predicate: excluded(0)
            }
        );
    }
}

#[test]
fn excluded_and_lookahead_passes_inconclusive_atoms_but_does_not_publish_their_cuts() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    install(
        &mut p,
        vec![
            diagonal(&c),
            poly(&c, c.zero()),
            diagonal(&c),
            minus(&c, 0, 2),
            conservative(&c),
            positive(&c),
        ],
    );
    let (stats, pieces) = refined(&p, [0; 3], [None; 3], Some(2), Default::default());
    assert_eq!(stats.predicates, 6);
    assert_eq!(stats.split_operations, 0);
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 }
    );
    finite_parity(&p, &pieces, 2);
}

#[test]
fn excluded_and_lookahead_uses_current_exact_cut_not_original_query_box() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let coupled = poly(
        &c,
        c.sub(
            &c.add(&c.index(1).unwrap(), &c.index(2).unwrap()).unwrap(),
            &c.integer(1),
        )
        .unwrap(),
    );
    install(&mut p, vec![minus(&c, 0, 2), coupled, minus(&c, 0, 1)]);
    // The last atom does vanish on the original box. Only after the first
    // atom restricts n0=2 is it uniformly nonzero; n1/n2 remain coupled.
    let pieces = collect(&p, &[0; 3], &[None; 3], Some(2));
    assert!(pieces.iter().all(|piece| piece.disposition()
        == OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 }));
    assert!(
        pieces
            .iter()
            .any(|piece| piece.lower()[0] == 1 && piece.upper()[0] == Some(1))
    );
    finite_parity(&p, &pieces, 2);
}

#[test]
fn excluded_and_lookahead_handles_conservative_plane_interiors_without_claiming_zero() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    install(&mut p, vec![conservative(&c), positive(&c)]);
    let pieces = collect(&p, &[0; 3], &[Some(3), Some(3), Some(2)], Some(2));
    assert!(pieces.iter().all(|piece| piece.disposition()
        == OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 }));
    finite_parity(&p, &pieces, 2);
}

#[test]
fn excluded_and_lookahead_without_witness_preserves_original_bounded_refinement() {
    let mut p = coupled_refinement_fixture(2);
    let limits = OwnerDomainMatchLimits {
        max_bounded_refinement_cells: 3,
        ..Default::default()
    };
    let (before, expected) = refined(&p, [0; 3], [None, Some(0), None], Some(2), limits);
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules[0].exceptions[0].push(poly(&c, c.zero()));
    let (after, pieces) = refined(&p, [0; 3], [None, Some(0), None], Some(2), limits);
    assert_eq!(normalized(&pieces), normalized(&expected));
    assert_eq!((after.refinement_cells, after.refinement_steps), (3, 1));
    assert_eq!(after.refinement_cells, before.refinement_cells);
    assert!(after.predicates > before.predicates);
    let unknown = collect(&p, &[0; 3], &[None, Some(0), None], Some(2));
    assert_eq!(unknown.len(), 1);
    assert_eq!(
        unknown[0].disposition(),
        OwnerDomainMatchDisposition::Unresolved {
            predicate: OwnerDomainPredicate::ExcludedConjunction {
                batch: 0,
                rule: 0,
                branch: 0,
                ordinal: 0
            }
        }
    );
}

fn refusal(p: &CandidateOwnerPrograms<3>, limits: OwnerDomainMatchLimits) -> OwnerDomainMatchError {
    p.visit_owner_domain_matches(
        OWNER,
        &[0; 3],
        &[None; 3],
        Some(11),
        limits,
        &AtomicBool::new(false),
        |_| panic!("failed lookahead must not publish"),
    )
    .unwrap_err()
}

#[test]
fn excluded_and_lookahead_cumulative_predicate_and_geometry_caps_identify_later_atom() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    install(&mut p, vec![diagonal(&c)]);
    let (baseline, _) = refined(&p, [0; 3], [None; 3], Some(11), Default::default());
    batch(&mut p).rules[0].exceptions[0].push(positive(&c));
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
        assert_eq!(error.predicate, Some(excluded(1)));
        assert_eq!(error.predicate_lower(), Some([0; 3].as_slice()));
        assert_eq!(error.predicate_upper(), Some([None; 3].as_slice()));
        assert_eq!(error.max_numerator_rank, Some(11));
        assert_eq!(error.stats.predicates, baseline.predicates + increment);
        assert_eq!(error.stats.cells, baseline.cells);
        assert_eq!(error.stats.coordinate_cells, baseline.coordinate_cells);
        assert_eq!(error.stats.pieces, 0);
    }
}

#[test]
fn excluded_and_lookahead_native_refusals_are_strict_and_never_trigger_initial_lookahead() {
    for refusal_first in [false, true] {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        let quadratic = poly(
            &c,
            c.mul(
                &c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap(),
                &c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap(),
            )
            .unwrap(),
        );
        let atoms = if refusal_first {
            vec![quadratic, positive(&c)]
        } else {
            // A witness even farther ahead must not swallow this typed error.
            vec![diagonal(&c), quadratic, positive(&c)]
        };
        install(&mut p, atoms);
        let mut limits = OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 100,
            ..Default::default()
        };
        limits.guard_algebra.max_univariate_degree = 1;
        let error = refusal(&p, limits);
        assert_eq!(
            error.failure,
            OwnerDomainMatchFailure::Algebra(crate::algebra::IndexedAlgebraError::ResourceLimit {
                resource: "guard univariate degree",
                requested: 2,
                limit: 1
            })
        );
        assert_eq!(
            error.predicate,
            Some(excluded(if refusal_first { 0 } else { 1 }))
        );
        assert_eq!(error.stats.predicates, if refusal_first { 1 } else { 2 });
        assert_eq!(
            (error.stats.refinement_cells, error.stats.refinement_steps),
            (0, 0)
        );
    }
}

#[test]
fn excluded_and_lookahead_cannot_override_unknown_source_or_required_equality() {
    for source in [false, true] {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        install(&mut p, vec![diagonal(&c), positive(&c)]);
        let predicate = if source {
            Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
                .unwrap()
                .shared
                .source_conditions = vec![diagonal(&c)];
            OwnerDomainPredicate::SourceCondition { ordinal: 0 }
        } else {
            batch(&mut p).rules[0].equalities.push(diagonal(&c));
            OwnerDomainPredicate::Equality {
                batch: 0,
                rule: 17,
                ordinal: 0,
            }
        };
        let (stats, pieces) = refined(&p, [0; 3], [None; 3], Some(11), Default::default());
        // Source Unknown still blocks immediately. An equality Unknown now
        // admits one inconclusive later-branch rejection probe; neither path
        // may bypass the required original predicate.
        assert_eq!(stats.predicates, if source { 1 } else { 2 });
        assert_eq!(pieces.len(), 1);
        assert_eq!(
            pieces[0].disposition(),
            OwnerDomainMatchDisposition::Unresolved { predicate }
        );
    }
}

#[test]
fn excluded_and_lookahead_cancellation_and_consumer_stop_retain_exact_incomplete_prefix() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    install(&mut p, vec![diagonal(&c), positive(&c)]);
    for consumer_stop in [false, true] {
        let cancel = AtomicBool::new(false);
        let mut callbacks = 0;
        let error = p
            .visit_owner_domain_matches(
                OWNER,
                &[0; 3],
                &[None; 3],
                Some(11),
                Default::default(),
                &cancel,
                |piece| {
                    callbacks += 1;
                    assert_eq!(
                        piece.disposition(),
                        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 }
                    );
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
