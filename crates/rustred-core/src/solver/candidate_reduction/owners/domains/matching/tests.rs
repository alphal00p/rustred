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

fn coupled_refinement_fixture(stage: usize) -> Arc<CandidateOwnerPrograms<3>> {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let guard = poly(
        &c,
        c.sub(
            &c.add(&c.index(0).unwrap(), &c.index(2).unwrap()).unwrap(),
            &c.one(),
        )
        .unwrap(),
    );
    let mut first = rule(0);
    match stage {
        0 => {
            Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
                .unwrap()
                .shared
                .source_conditions = vec![guard]
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

fn refined(
    p: &CandidateOwnerPrograms<3>,
    lower: [u64; 3],
    upper: [Option<u64>; 3],
    rank: Option<u32>,
    limits: OwnerDomainMatchLimits,
) -> (OwnerDomainMatchStats, Vec<OwnerDomainMatchPiece<3>>) {
    let mut pieces = Vec::new();
    let stats = p
        .visit_owner_domain_matches(
            OWNER,
            &lower,
            &upper,
            rank,
            limits,
            &AtomicBool::new(false),
            |piece| {
                pieces.push(piece);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    (stats, pieces)
}
fn normalized(
    pieces: &[OwnerDomainMatchPiece<3>],
) -> Vec<(Vec<u64>, Vec<Option<u64>>, Option<u32>, String)> {
    let mut normalized = pieces
        .iter()
        .map(|p| {
            (
                p.lower().to_vec(),
                p.upper().to_vec(),
                p.max_numerator_rank(),
                format!("{:?}", p.disposition()),
            )
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized
}

#[test]
fn bounded_refinement_matches_explicit_slices_and_retries_each_exact_predicate_phase() {
    for stage in 0..4 {
        let p = coupled_refinement_fixture(stage);
        let (disabled, unknown) = refined(
            &p,
            [0; 3],
            [None, Some(0), None],
            Some(2),
            Default::default(),
        );
        assert_eq!(
            (disabled.refinement_cells, disabled.refinement_steps),
            (0, 0)
        );
        assert_eq!(unknown.len(), 1);
        assert!(matches!(
            unknown[0].disposition(),
            OwnerDomainMatchDisposition::Unresolved { .. }
        ));
        let (stats, automatic) = refined(
            &p,
            [0; 3],
            [None, Some(0), None],
            Some(2),
            OwnerDomainMatchLimits {
                max_bounded_refinement_cells: 3,
                ..Default::default()
            },
        );
        assert_eq!((stats.refinement_cells, stats.refinement_steps), (3, 1));
        let mut explicit = Vec::new();
        for k in 0..=2 {
            explicit.extend(collect(&p, &[0, 0, k], &[None, Some(0), Some(k)], Some(2)));
        }
        assert_eq!(
            normalized(&automatic),
            normalized(&explicit),
            "stage {stage}"
        );
        assert!(automatic.iter().all(|p| p.max_numerator_rank() == Some(2)));
        for x in 0..=5 {
            for k in 0..=2 {
                assert_eq!(
                    at(&automatic, [x, 0, k]),
                    concrete(
                        &p,
                        IntegralKey::try_new([x as i64 + 1, 1, -(k as i64)]).unwrap()
                    )
                );
            }
        }
        assert!(automatic.iter().any(|p| p.upper()[0].is_none()));
    }
}

#[test]
fn bounded_refinement_insufficient_full_split_allowance_preserves_original_unknown() {
    let p = coupled_refinement_fixture(2);
    let (baseline, unknown) = refined(
        &p,
        [0; 3],
        [None, Some(0), None],
        Some(2),
        Default::default(),
    );
    for limits in [
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 2,
            ..Default::default()
        },
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 3,
            max_cells: baseline.cells,
            ..Default::default()
        },
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 3,
            max_coordinate_cells: baseline.coordinate_cells,
            ..Default::default()
        },
    ] {
        let (stats, actual) = refined(&p, [0; 3], [None, Some(0), None], Some(2), limits);
        assert_eq!(normalized(&actual), normalized(&unknown));
        assert_eq!(stats, baseline); // no partial reservation or face publication
    }
}

#[test]
fn bounded_refinement_never_enumerates_positive_or_unbounded_inactive_axes() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.equalities.push(poly(
        &c,
        c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
    ));
    batch(&mut p).rules = vec![first, rule(1)];
    let (stats, pieces) = refined(
        &p,
        [0; 3],
        [Some(2), Some(2), Some(2)],
        Some(2),
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 100,
            ..Default::default()
        },
    );
    assert_eq!(stats.refinement_cells, 0); // bounded inactive axis is not in polynomial
    assert_eq!(pieces.len(), 1);
    assert!(matches!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    ));
    let p = coupled_refinement_fixture(1);
    let (stats, pieces) = refined(
        &p,
        [0; 3],
        [None, Some(0), None],
        None,
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 100,
            ..Default::default()
        },
    );
    assert_eq!(stats.refinement_cells, 0); // supported inactive axis has no finite bound
    assert_eq!(pieces.len(), 1);
    assert!(matches!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    ));
}

#[test]
fn bounded_refinement_rank_induced_singleton_progresses_once_then_keeps_positive_coupling() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.equalities.push(poly(
        &c,
        c.add(
            &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.index(2).unwrap(),
        )
        .unwrap(),
    ));
    batch(&mut p).rules = vec![first, rule(1)];
    let (stats, pieces) = refined(
        &p,
        [0; 3],
        [None; 3],
        Some(0),
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 1,
            ..Default::default()
        },
    );
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (1, 1));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].upper(), &[None, None, Some(0)]);
    assert!(matches!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    ));
}

#[test]
fn bounded_refinement_nested_simplex_counts_are_cumulative_and_deterministic() {
    let owner = [true, false, false];
    let mut p = programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input(owner, Some(10), vec![], &[])],
        Default::default(),
    );
    let c = p.context.coefficient_context().clone();
    let mut first = rule(0);
    first.equalities.push(poly(
        &c,
        c.sub(
            &c.add(
                &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
                &c.index(2).unwrap(),
            )
            .unwrap(),
            &c.one(),
        )
        .unwrap(),
    ));
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
    let stats = p
        .visit_owner_domain_matches(
            owner,
            &[0; 3],
            &[None; 3],
            Some(2),
            OwnerDomainMatchLimits {
                max_bounded_refinement_cells: 9,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |piece| {
                pieces.push(piece);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    // First axis x1 costs3, then x2 costs3+2+1 under the inherited simplex.
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (9, 4));
    assert!(pieces.iter().all(|p| !matches!(
        p.disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    )));
    for a in 0..=2 {
        for b in 0..=2 - a {
            for x in 0..=5 {
                let matching = pieces
                    .iter()
                    .filter(|p| {
                        (0..3).all(|i| {
                            [x, a, b][i] >= p.lower()[i]
                                && p.upper()[i].is_none_or(|u| [x, a, b][i] <= u)
                        })
                    })
                    .collect::<Vec<_>>();
                assert_eq!(matching.len(), 1);
                assert_eq!(
                    matching[0].disposition(),
                    OwnerDomainMatchDisposition::SelectedRule {
                        batch: 0,
                        rule: if x == a + b { 0 } else { 1 }
                    }
                );
                assert_eq!(matching[0].max_numerator_rank(), Some(2));
            }
        }
    }
    let mut partial = Vec::new();
    let stats = p
        .visit_owner_domain_matches(
            owner,
            &[0; 3],
            &[None; 3],
            Some(2),
            OwnerDomainMatchLimits {
                max_bounded_refinement_cells: 3,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |piece| {
                partial.push(piece);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (3, 1));
    assert_eq!(partial.len(), 3);
    for (i, piece) in partial.iter().enumerate() {
        assert_eq!(piece.lower()[1], i as u64); // equal cost chooses lower original axis
        assert_eq!(piece.upper()[1], Some(i as u64));
        assert_eq!(piece.upper()[2], None);
        assert!(matches!(
            piece.disposition(),
            OwnerDomainMatchDisposition::Unresolved { .. }
        ));
    }
    let mut cheaper = Vec::new();
    let stats = p
        .visit_owner_domain_matches(
            owner,
            &[0; 3],
            &[None, Some(2), Some(1)],
            Some(2),
            OwnerDomainMatchLimits {
                max_bounded_refinement_cells: 2,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |piece| {
                cheaper.push(piece);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (2, 1));
    assert_eq!(cheaper.len(), 2);
    for (i, piece) in cheaper.iter().enumerate() {
        assert_eq!(piece.lower()[2], i as u64); // lower cost wins over lower axis
        assert_eq!(piece.upper()[2], Some(i as u64));
        assert_eq!(piece.upper()[1], Some(2));
    }
}

#[test]
fn bounded_refinement_prepaid_lazy_faces_cancel_without_visiting_whole_interval() {
    let p = coupled_refinement_fixture(2);
    let cancel = AtomicBool::new(false);
    let mut callbacks = 0;
    let e = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None, Some(0), None],
            Some(999),
            OwnerDomainMatchLimits {
                max_bounded_refinement_cells: 1000,
                ..Default::default()
            },
            &cancel,
            |_| {
                callbacks += 1;
                cancel.store(true, Ordering::Release);
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert_eq!(e.failure, OwnerDomainMatchFailure::Cancelled);
    assert_eq!(callbacks, 1);
    assert_eq!(
        (e.stats.refinement_cells, e.stats.refinement_steps),
        (1000, 1)
    );
    assert!(e.stats.predicates < 10); // continuation does not eagerly resolve 1000 faces
}

#[test]
fn bounded_refinement_above_entry_rank_retains_exact_r11_scope() {
    let p = coupled_refinement_fixture(2);
    let (stats, pieces) = refined(
        &p,
        [0, 0, 10],
        [None, Some(0), None],
        Some(11),
        OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 2,
            ..Default::default()
        },
    );
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (2, 1));
    let mut explicit = Vec::new();
    for k in 10..=11 {
        explicit.extend(collect(&p, &[0, 0, k], &[None, Some(0), Some(k)], Some(11)));
    }
    assert_eq!(normalized(&pieces), normalized(&explicit));
    assert!(pieces.iter().all(|p| p.max_numerator_rank() == Some(11)));
    assert!(pieces.iter().any(|p| p.upper()[0].is_none()));
}

#[test]
fn bounded_refinement_conservative_cover_drops_rank_empty_intersections_before_bound_math() {
    let owner = [true, false, false];
    let mut p = programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input(owner, Some(10), vec![], &[])],
        Default::default(),
    );
    let c = p.context.coefficient_context().clone();
    let a = c.add(&c.index(1).unwrap(), &c.one()).unwrap();
    let b = c.add(&c.index(2).unwrap(), &c.one()).unwrap();
    let coupled = c
        .add(
            &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.index(2).unwrap(),
        )
        .unwrap();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let mut first = rule(0);
    first.equalities.push(poly(
        &c,
        c.add(&c.mul(&a, &b).unwrap(), &c.mul(&d, &coupled).unwrap())
            .unwrap(),
    ));
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
    let stats = p
        .visit_owner_domain_matches(
            owner,
            &[0; 3],
            &[None; 3],
            Some(2),
            OwnerDomainMatchLimits {
                max_bounded_refinement_cells: 1,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |piece| {
                pieces.push(piece);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert!(stats.rank_empty_cells > 0);
    assert!(pieces.iter().all(|p| p.lower()[1] + p.lower()[2] <= 2));
    assert!(pieces.iter().any(|p| matches!(
        p.disposition(),
        OwnerDomainMatchDisposition::Unresolved { .. }
    )));
    for a in 0..=2 {
        for b in 0..=2 - a {
            for x in 0..=4 {
                assert_eq!(
                    pieces
                        .iter()
                        .filter(|p| (0..3).all(|i| [x, a, b][i] >= p.lower()[i]
                            && p.upper()[i].is_none_or(|u| [x, a, b][i] <= u)))
                        .count(),
                    1
                );
            }
        }
    }
}
