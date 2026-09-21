//! Synthetic fixtures exercise applicability, not IBP provenance or closure.
use super::*;
use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::reduction::{ReductionRequest, ReductionStatistics};
use crate::solver::candidate_reduction::{
    evaluator::CandidateEvaluator,
    model::{CandidateReductionError, PreparedRule, PreparedTerm},
    owner_test_support::{input, programs},
    owners::{CandidateOwnerPrograms, model::PreparedOwnerBatch},
};
use std::ops::ControlFlow;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const OWNER: [bool; 3] = [true, true, false];

fn fixture() -> Arc<CandidateOwnerPrograms<3>> {
    programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input(OWNER, Some(10), vec![], &[])],
        Default::default(),
    )
}
fn batch(p: &mut Arc<CandidateOwnerPrograms<3>>) -> &mut PreparedOwnerBatch<3> {
    let owner = Arc::get_mut(Arc::get_mut(p).unwrap().owners.get_mut(&OWNER).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap()
}
fn rule(ordinal: usize) -> PreparedRule<3> {
    PreparedRule {
        ordinal,
        fixed: [None; 3],
        equalities: vec![],
        exceptions: vec![],
        rhs: vec![],
    }
}
fn poly(c: &IndexedCoefficientContext, v: IndexedCoefficient) -> IndexedPolynomial {
    c.numerator_condition_with_limits(&v, Default::default())
        .unwrap()
}
fn minus(c: &IndexedCoefficientContext, axis: usize, value: i64) -> IndexedPolynomial {
    poly(
        c,
        c.sub(&c.index(axis).unwrap(), &c.integer(value)).unwrap(),
    )
}
fn collect(
    p: &CandidateOwnerPrograms<3>,
    lower: &[u64; 3],
    upper: &[Option<u64>; 3],
    rank: Option<u32>,
) -> Vec<OwnerDomainMatchPiece<3>> {
    let mut out = vec![];
    p.visit_owner_domain_matches(
        OWNER,
        lower,
        upper,
        rank,
        Default::default(),
        &AtomicBool::new(false),
        |piece| {
            out.push(piece);
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    out
}
fn at(pieces: &[OwnerDomainMatchPiece<3>], x: [u64; 3]) -> OwnerDomainMatchDisposition {
    let found: Vec<_> = pieces
        .iter()
        .filter(|p| {
            (0..3).all(|i| x[i] >= p.lower()[i] && p.upper()[i].is_none_or(|u| x[i] <= u))
                && p.max_numerator_rank().is_none_or(|r| x[2] <= u64::from(r))
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "point {x:?} must have exactly one disposition"
    );
    found[0].disposition()
}
fn concrete(p: &CandidateOwnerPrograms<3>, key: IntegralKey) -> OwnerDomainMatchDisposition {
    let shared = &p.context.shared;
    let owner = &p.owners[&OWNER];
    let evaluator = |rules| CandidateEvaluator {
        context: &shared.context,
        root_sector: owner.root,
        ordering: owner.ordering,
        rules,
        source_conditions: &shared.source_conditions,
        zero_sectors: &shared.zero_sectors,
        limits: p.context.limits,
    };
    match evaluator(&[]).validate_target(&key) {
        Ok(()) => {}
        Err(CandidateReductionError::SourceConditionVanished { ordinal, .. }) => {
            return OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal };
        }
        other => panic!("unexpected entry validation {other:?}"),
    }
    for (batch, b) in owner.batches.iter().enumerate() {
        if b.terminals.contains(&key) {
            return OwnerDomainMatchDisposition::Terminal { batch };
        }
        for rule in &b.rules {
            match evaluator(std::slice::from_ref(rule)).apply(
                &key,
                &mut ReductionRequest::default(),
                &mut ReductionStatistics::default(),
            ) {
                Ok(_) => {
                    return OwnerDomainMatchDisposition::SelectedRule {
                        batch,
                        rule: rule.ordinal,
                    };
                }
                Err(CandidateReductionError::Uncovered { .. }) => {}
                other => panic!("unexpected evaluator error {other:?}"),
            }
        }
    }
    OwnerDomainMatchDisposition::ExactGap
}

#[test]
fn finite_partition_matches_existing_evaluator_priority_poles_and_whole_conjunctions() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions = vec![minus(&c, 0, 4)];
    let b = batch(&mut p);
    b.terminals.insert(IntegralKey::try_new([1, 1, 0]).unwrap());
    let mut first = rule(0);
    first.fixed[1] = Some(1);
    first.equalities.push(minus(&c, 0, 2));
    let mut second = rule(1);
    second
        .exceptions
        .push(vec![minus(&c, 0, 1), minus(&c, 1, 1)]);
    // A zero coefficient must not erase its ORIGINAL denominator pole.
    second.rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: minus(&c, 0, 3),
    });
    let mut third = rule(2);
    third.fixed[0] = Some(3);
    b.rules = vec![first, second, third];
    let pieces = collect(&p, &[0; 3], &[Some(4), Some(2), Some(2)], Some(1));
    for x in 0..=4 {
        for y in 0..=2 {
            for z in 0..=1 {
                assert_eq!(
                    at(&pieces, [x, y, z]),
                    concrete(
                        &p,
                        IntegralKey::try_new([x as i64 + 1, y as i64 + 1, -(z as i64)]).unwrap()
                    )
                );
            }
        }
    }
    assert!(
        pieces
            .iter()
            .any(|p| p.disposition() == OwnerDomainMatchDisposition::ExactGap)
    );
    assert!(pieces.iter().any(|p| matches!(
        p.disposition(),
        OwnerDomainMatchDisposition::InvalidSourceCondition { .. }
    )));
}

#[test]
fn unbounded_positive_tail_and_actual_r11_scope_are_not_readmitted_or_clipped() {
    let mut p = fixture();
    batch(&mut p).rules = vec![rule(0)];
    let pieces = collect(&p, &[0, 0, 11], &[None, Some(0), Some(11)], Some(11));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].upper()[0], None);
    assert_eq!(pieces[0].max_numerator_rank(), Some(11));
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    assert!(collect(&p, &[0, 0, 11], &[None, Some(0), Some(11)], Some(10)).is_empty());
}

#[test]
fn base_parameter_polynomial_is_not_treated_as_a_real_numeric_equality() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let mut first = rule(0);
    first
        .equalities
        .push(poly(&c, c.sub(&c.index(0).unwrap(), &d).unwrap()));
    batch(&mut p).rules = vec![first, rule(1)];
    let pieces = collect(&p, &[0; 3], &[None, None, Some(0)], Some(0));
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
}

#[test]
fn exact_separable_hyperplanes_partition_without_overlap() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let a = c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap();
    let b = c.sub(&c.index(1).unwrap(), &c.integer(2)).unwrap();
    let mut first = rule(0);
    first.equalities.push(poly(&c, c.mul(&a, &b).unwrap()));
    batch(&mut p).rules = vec![first, rule(1)];
    let pieces = collect(&p, &[0; 3], &[Some(2), Some(2), Some(0)], Some(0));
    for x in 0..=2 {
        for y in 0..=2 {
            assert_eq!(
                at(&pieces, [x, y, 0]),
                OwnerDomainMatchDisposition::SelectedRule {
                    batch: 0,
                    rule: if x == 1 || y == 1 { 0 } else { 1 }
                }
            );
        }
    }
}

#[test]
fn coupled_predicate_blocks_later_rules_and_conservative_cover_is_not_exact() {
    for conservative in [false, true] {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        let a = c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap();
        let b = c.sub(&c.index(1).unwrap(), &c.integer(2)).unwrap();
        let value = if conservative {
            let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
            c.add(&a, &c.mul(&d, &b).unwrap()).unwrap() // a=0 AND b=0, not whole a=0
        } else {
            c.add(&a, &b).unwrap()
        };
        let mut first = rule(0);
        first.equalities.push(poly(&c, value));
        batch(&mut p).rules = vec![first, rule(1)];
        let pieces = collect(&p, &[0; 3], &[Some(2), Some(2), Some(0)], Some(0));
        assert!(pieces.iter().any(|p| matches!(
            p.disposition(),
            OwnerDomainMatchDisposition::Unresolved { .. }
        )));
        assert!(matches!(
            at(&pieces, [1, 1, 0]),
            OwnerDomainMatchDisposition::Unresolved { .. }
        ));
        assert!(
            !pieces.iter().any(|p| p.disposition()
                == OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 })
        );
        if conservative {
            assert!(pieces.iter().any(|p| p.disposition()
                == OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }));
        }
    }
}

#[test]
fn later_batch_terminal_does_not_shadow_earlier_formula() {
    let mut p = fixture();
    batch(&mut p).rules = vec![rule(0)];
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
        terminals: [IntegralKey::try_new([1, 1, 0]).unwrap()].into(),
        coalescing_bound: 0,
        overlay: None,
    }));
    assert_eq!(
        collect(&p, &[0; 3], &[Some(0); 3], Some(0))[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
}

#[test]
fn exact_zero_and_source_invalid_precede_owner_dispatch() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let shared = &mut Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared;
    shared.zero_sectors.insert(OWNER);
    shared.source_conditions = vec![minus(&c, 0, 1)];
    let pieces = collect(&p, &[0; 3], &[Some(1), Some(0), Some(0)], Some(0));
    assert_eq!(
        at(&pieces, [0, 0, 0]),
        OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal: 0 }
    );
    assert_eq!(
        at(&pieces, [1, 0, 0]),
        OwnerDomainMatchDisposition::ExactZeroSector
    );
}

#[test]
fn resource_cancellation_consumer_and_malformed_input_are_typed_incomplete() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.equalities.push(minus(&c, 0, 2));
    batch(&mut p).rules = vec![first];
    for limits in [
        OwnerDomainMatchLimits {
            max_rules: 0,
            ..Default::default()
        },
        OwnerDomainMatchLimits {
            max_predicates: 0,
            ..Default::default()
        },
        OwnerDomainMatchLimits {
            max_pieces: 0,
            ..Default::default()
        },
        OwnerDomainMatchLimits {
            max_cells: 0,
            ..Default::default()
        },
        OwnerDomainMatchLimits {
            max_split_operations: 0,
            ..Default::default()
        },
        OwnerDomainMatchLimits {
            max_coordinate_cells: 0,
            ..Default::default()
        },
    ] {
        let e = p
            .visit_owner_domain_matches(
                OWNER,
                &[0; 3],
                &[None, None, Some(0)],
                Some(0),
                limits,
                &AtomicBool::new(false),
                |_| ControlFlow::Continue(()),
            )
            .unwrap_err();
        assert!(
            matches!(e.failure, OwnerDomainMatchFailure::ResourceLimit { .. }),
            "{e:?}"
        );
    }
    let e = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(0),
            Default::default(),
            &AtomicBool::new(true),
            |_| panic!(),
        )
        .unwrap_err();
    assert_eq!(e.failure, OwnerDomainMatchFailure::Cancelled);
    assert_eq!(e.stats.pieces, 0);
    let e = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(0),
            Default::default(),
            &AtomicBool::new(false),
            |_| ControlFlow::Break(()),
        )
        .unwrap_err();
    assert_eq!(e.failure, OwnerDomainMatchFailure::StoppedByConsumer);
    assert_eq!(e.stats.pieces, 1);
    let e = p
        .visit_owner_domain_matches(
            OWNER,
            &[2; 3],
            &[Some(1); 3],
            None,
            Default::default(),
            &AtomicBool::new(false),
            |_| panic!(),
        )
        .unwrap_err();
    assert!(matches!(
        e.failure,
        OwnerDomainMatchFailure::InvalidInput(_)
    ));
    let cancel = AtomicBool::new(false);
    let e = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(0),
            Default::default(),
            &cancel,
            |_| {
                cancel.store(true, Ordering::Release);
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert_eq!(e.failure, OwnerDomainMatchFailure::Cancelled);
    assert_eq!(e.stats.pieces, 1);
}

#[test]
fn roots_outside_box_carrier_stay_unresolved_not_false_gaps() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let huge = c.add(&c.integer(i64::MAX), &c.integer(i64::MAX)).unwrap();
    let huge = c.add(&huge, &c.integer(5)).unwrap(); // > u64::MAX + 1
    let mut first = rule(0);
    first
        .equalities
        .push(poly(&c, c.sub(&c.index(0).unwrap(), &huge).unwrap()));
    batch(&mut p).rules = vec![first, rule(1)];
    let pieces = collect(&p, &[0; 3], &[None, Some(0), Some(0)], Some(0));
    assert_eq!(pieces.len(), 1);
    assert!(matches!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    ));
}

#[test]
fn native_guard_limits_fail_as_typed_algebra_resource_errors() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.equalities.push(minus(&c, 0, 2));
    batch(&mut p).rules = vec![first];
    let mut limits = OwnerDomainMatchLimits::default();
    limits.guard_algebra.max_input_terms = 0;
    let e = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None, Some(0), Some(0)],
            Some(0),
            limits,
            &AtomicBool::new(false),
            |_| panic!(),
        )
        .unwrap_err();
    assert!(matches!(
        e.failure,
        OwnerDomainMatchFailure::Algebra(crate::algebra::IndexedAlgebraError::ResourceLimit { .. })
    ));
}

#[test]
fn rank_simplex_excludes_hyperplanes_reachable_in_the_rectangle_only() {
    let owner = [true, false, false];
    let mut p = programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input(owner, Some(10), vec![], &[])],
        Default::default(),
    );
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.equalities.push(minus(&c, 1, -2));
    let installed = Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&owner)
            .unwrap(),
    )
    .unwrap();
    Arc::get_mut(&mut installed.batches[0]).unwrap().rules = vec![first, rule(1)];
    let mut pieces = Vec::new();
    p.visit_owner_domain_matches(
        owner,
        &[0, 0, 1],
        &[None, Some(2), Some(2)],
        Some(2),
        Default::default(),
        &AtomicBool::new(false),
        |piece| {
            pieces.push(piece);
            ControlFlow::Continue(())
        },
    )
    .unwrap();
    // n1=-2 intersects the rectangle, but already x2>=1 forces rank>=3.
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
    assert_eq!(pieces[0].max_numerator_rank(), Some(2));
    // Keep the simplex, not an invented rectangular replacement.
    assert_eq!(pieces[0].upper(), &[None, Some(2), Some(2)]);
}

#[test]
fn terminal_work_is_bounded_and_empty_exclusion_skips_the_whole_rule() {
    let mut p = fixture();
    let b = batch(&mut p);
    b.terminals.insert(IntegralKey::try_new([1, 1, 0]).unwrap());
    let mut first = rule(0);
    first.exceptions.push(vec![]);
    b.rules = vec![first, rule(1)];
    let pieces = collect(&p, &[0; 3], &[Some(1), Some(0), Some(0)], Some(0));
    assert_eq!(
        at(&pieces, [0, 0, 0]),
        OwnerDomainMatchDisposition::Terminal { batch: 0 }
    );
    assert_eq!(
        at(&pieces, [1, 0, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
    let e = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(0),
            OwnerDomainMatchLimits {
                max_terminal_checks: 0,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_| panic!(),
        )
        .unwrap_err();
    assert!(matches!(
        e.failure,
        OwnerDomainMatchFailure::ResourceLimit {
            resource: "terminal checks",
            ..
        }
    ));
}
