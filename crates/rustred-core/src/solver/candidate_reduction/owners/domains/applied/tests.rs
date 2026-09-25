//! Synthetic native formulas test exact local semantics, not IBP provenance.
use super::*;
use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::reduction::{ReductionRequest, ReductionStatistics};
use crate::solver::candidate_reduction::owners::domains::OwnerDomainMatchDisposition;
use crate::solver::candidate_reduction::{
    evaluator::CandidateEvaluator,
    model::{PreparedRule, PreparedTerm},
    owner_test_support::{input, programs},
    owners::{CandidateOwnerPrograms, model::PreparedOwnerBatch},
};
use std::ops::ControlFlow;
use std::sync::{Arc, atomic::AtomicBool};

mod cell_refinement;
mod optional_refusal;
mod power_bounds;
mod singleton_classification;
mod support_transitions;

const OWNER: [bool; 3] = [true, true, false];

fn fixture() -> Arc<CandidateOwnerPrograms<3>> {
    let mut p = programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input(OWNER, Some(10), vec![], &[])],
        Default::default(),
    );
    // Exercise the synthetic RHS, independent of sunset's authenticated zero census.
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .clear();
    p
}
fn batch(p: &mut Arc<CandidateOwnerPrograms<3>>) -> &mut PreparedOwnerBatch<3> {
    let owner = Arc::get_mut(Arc::get_mut(p).unwrap().owners.get_mut(&OWNER).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap()
}
fn poly(c: &IndexedCoefficientContext, value: &IndexedCoefficient) -> IndexedPolynomial {
    c.numerator_condition_with_limits(value, Default::default())
        .unwrap()
}
fn term(
    c: &IndexedCoefficientContext,
    shift: [i64; 3],
    coefficient: IndexedCoefficient,
) -> PreparedTerm<3> {
    PreparedTerm {
        shift,
        coefficient,
        denominator: poly(c, &c.integer(1)),
    }
}
fn rule(ordinal: usize, rhs: Vec<PreparedTerm<3>>) -> PreparedRule<3> {
    PreparedRule {
        ordinal,
        case: crate::solver::Case::generic(),
        fixed: [None; 3],
        equalities: vec![],
        exceptions: vec![],
        rhs,
    }
}
#[derive(Debug)]
struct Edge {
    source_lower: Vec<u64>,
    source_upper: Vec<Option<u64>>,
    target: [bool; 3],
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    rank: Option<u32>,
    coefficient: IndexedCoefficient,
    nonzero: OwnerAppliedNonzero,
}
#[derive(Default, Debug)]
struct Collected {
    classified: Vec<OwnerDomainMatchDisposition>,
    edges: Vec<Edge>,
    problems: Vec<OwnerAppliedProblemKind>,
    finished: Vec<(usize, usize)>,
}
fn collect(
    p: &CandidateOwnerPrograms<3>,
    lower: [u64; 3],
    upper: [Option<u64>; 3],
    rank: Option<u32>,
) -> (Collected, OwnerAppliedStats) {
    let mut result = Collected::default();
    let stats = p
        .visit_owner_applied_successors(
            OWNER,
            &lower,
            &upper,
            rank,
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                match event {
                    OwnerAppliedEvent::Classified(piece) => {
                        result.classified.push(piece.disposition())
                    }
                    OwnerAppliedEvent::Successor(edge) => result.edges.push(Edge {
                        source_lower: edge.source_lower.to_vec(),
                        source_upper: edge.source_upper.to_vec(),
                        target: *edge.target_sector,
                        lower: edge.target_lower.to_vec(),
                        upper: edge.target_upper.to_vec(),
                        rank: edge.target_rank_limit,
                        coefficient: edge.coefficient.clone(),
                        nonzero: edge.coefficient_nonzero,
                    }),
                    OwnerAppliedEvent::Problem(problem) => result.problems.push(problem.kind),
                    OwnerAppliedEvent::OptionalCoefficientRefusal { .. } => {
                        panic!("existing default-policy fixture unexpectedly refused optional classification")
                    }
                    OwnerAppliedEvent::RuleFinished {
                        successors,
                        problems,
                        ..
                    } => result.finished.push((successors, problems)),
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    (result, stats)
}

#[test]
fn applied_rematches_first_rule_and_preserves_terminal_gap_and_original_poles() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17, vec![term(&c, [-1, 0, 0], c.integer(1))]);
    first.fixed[0] = Some(2);
    let mut second = rule(28, vec![term(&c, [0; 3], c.zero())]);
    second.rhs[0].denominator = poly(&c, &c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap());
    second.fixed[0] = Some(3);
    let b = batch(&mut p);
    b.terminals.insert(IntegralKey::try_new([1, 1, 0]).unwrap());
    b.rules = vec![first, second];
    let (out, _) = collect(&p, [0; 3], [Some(2), Some(0), Some(0)], Some(10));
    assert!(
        out.classified
            .contains(&OwnerDomainMatchDisposition::Terminal { batch: 0 })
    );
    assert!(
        out.classified
            .contains(&OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 })
    );
    assert!(
        out.classified
            .contains(&OwnerDomainMatchDisposition::ExactGap)
    );
    assert!(
        !out.classified
            .contains(&OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 28 })
    );
    assert_eq!(out.edges.len(), 1);
    assert!(out.problems.is_empty());
}

#[test]
fn applied_native_equal_shift_cancellation_follows_original_term_validity() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let n = c.index(0).unwrap();
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], n.clone()),
            term(&c, [-1, 0, 0], c.sub(&c.zero(), &n).unwrap()),
        ],
    )];
    let (out, stats) = collect(&p, [1, 0, 0], [None, Some(0), Some(0)], Some(10));
    assert!(out.edges.is_empty() && out.problems.is_empty());
    assert_eq!(stats.cancelled_groups, 1);
    assert_eq!(stats.coalescing_additions, 1);
    assert_eq!(out.finished, [(0, 0)]);

    // The same algebraic cancellation must NOT hide an invalid self-dependency.
    for term in &mut batch(&mut p).rules[0].rhs {
        term.shift = [0; 3];
    }
    let (out, stats) = collect(&p, [1, 0, 0], [None, Some(0), Some(0)], Some(10));
    assert!(matches!(
        out.problems.as_slice(),
        [OwnerAppliedProblemKind::DescentNotEstablished { .. }]
    ));
    assert!(out.edges.is_empty());
    assert_eq!(stats.coalescing_additions, 0);
    assert_eq!(stats.cancelled_groups, 0);
    assert_eq!(out.finished, [(0, 1)]);
}

#[test]
fn applied_zero_activation_is_removed_before_child_root_but_saved_root_is_authority() {
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
    .root = OWNER;
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [0, 0, 1], c.index(2).unwrap())])];
    let (out, stats) = collect(&p, [0; 3], [None, None, Some(0)], Some(10));
    assert!(out.edges.is_empty() && out.problems.is_empty());
    assert_eq!(stats.zero_terms, 1);
    batch(&mut p).rules[0].rhs[0].coefficient = c.integer(1);
    let (out, _) = collect(&p, [0; 3], [None, None, Some(0)], Some(10));
    assert_eq!(out.problems, [OwnerAppliedProblemKind::InvalidChildRoot]);
    Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&OWNER)
            .unwrap(),
    )
    .unwrap()
    .root = [true; 3];
    let (out, _) = collect(&p, [0; 3], [None, None, Some(0)], Some(10));
    assert!(matches!(
        out.problems.as_slice(),
        [OwnerAppliedProblemKind::DescentNotEstablished { .. }]
    ));
}

#[test]
fn applied_conditional_coefficient_is_retained_as_source_predicate() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let coefficient = c.sub(&c.index(0).unwrap(), &c.integer(3)).unwrap();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], coefficient)])];
    let (out, stats) = collect(&p, [1, 0, 0], [None, Some(0), Some(0)], Some(10));
    assert_eq!(out.edges.len(), 1);
    assert_eq!(stats.conditional_successors, 1);
    assert_eq!(out.edges[0].nonzero, OwnerAppliedNonzero::Conditional);
    assert_eq!(out.edges[0].upper[0], None);
    assert!(
        c.specialize_sealed(&out.edges[0].coefficient, &[3, 1, 0], Default::default())
            .unwrap()
            .0
            .is_zero()
    );
    assert!(
        !c.specialize_sealed(&out.edges[0].coefficient, &[4, 1, 0], Default::default())
            .unwrap()
            .0
            .is_zero()
    );
}

#[test]
fn applied_child_source_condition_precedes_certified_zero_omission() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let shared = &mut Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared;
    shared.source_conditions = vec![poly(&c, &c.index(0).unwrap())];
    shared.zero_sectors.insert([false, true, false]);
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-2, 0, 0], c.integer(1))])];
    let (out, stats) = collect(&p, [1, 0, 0], [Some(1), None, Some(0)], Some(10));
    assert_eq!(
        out.problems,
        [OwnerAppliedProblemKind::InvalidChildSourceCondition { ordinal: 0 }]
    );
    assert_eq!(stats.zero_sector_groups, 0);
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions
        .clear();
    let (out, stats) = collect(&p, [1, 0, 0], [Some(1), None, Some(0)], Some(10));
    assert!(out.problems.is_empty() && out.edges.is_empty());
    assert_eq!(stats.zero_sector_groups, 1);
}

#[test]
fn applied_pinch_fixes_crossing_coordinates_and_preserves_exact_rank_image() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-2, 0, 0], c.integer(1))])];
    let (out, stats) = collect(&p, [0; 3], [None, None, None], Some(10));
    assert!(out.problems.is_empty());
    assert_eq!(out.edges.len(), 3);
    assert_eq!(stats.boundary_cells, 3);
    let pinches: Vec<_> = out.edges.iter().filter(|e| !e.target[0]).collect();
    assert_eq!(pinches.len(), 2);
    assert!(pinches.iter().any(|e| e.source_lower[0] == 0
        && e.source_upper[0] == Some(0)
        && e.lower[0] == 1
        && e.upper[0] == Some(1)
        && e.rank == Some(11)));
    assert!(pinches.iter().any(|e| e.source_lower[0] == 1
        && e.source_upper[0] == Some(1)
        && e.lower[0] == 0
        && e.upper[0] == Some(0)
        && e.rank == Some(10)));
    // Exact source simplex image, not just a maximum target rank envelope.
    for edge in pinches {
        for x2 in 0..=12_u64 {
            let source_admitted = x2 <= 10;
            let target_admitted = edge.lower[0] + x2 <= u64::from(edge.rank.unwrap());
            assert_eq!(source_admitted, target_admitted);
        }
    }
    assert!(out.edges.iter().all(|e| e.upper[1].is_none()));
}

#[test]
fn applied_same_support_above_entry_rank_and_unbounded_tail_are_preserved() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-2, 0, -1], c.integer(1))])];
    let (out, _) = collect(&p, [2, 0, 0], [None, None, None], Some(10));
    assert!(out.problems.is_empty());
    assert_eq!(out.edges.len(), 1);
    assert_eq!(out.edges[0].target, OWNER);
    assert_eq!(out.edges[0].rank, Some(11));
    assert_eq!(out.edges[0].lower, [0, 0, 1]);
    assert_eq!(out.edges[0].upper, [None; 3]);
}

#[test]
fn applied_unrepresentable_fixed_coordinates_and_rank_images_are_explicit() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-2, 0, -1], c.integer(1))])];
    let large = i64::MAX as u64;
    let (out, _) = collect(&p, [large, 0, 0], [Some(large), None, None], Some(10));
    // An originally fixed, unrepresentable index is unresolved at matcher
    // denominator admission, before any selected RHS is inspected.
    assert!(matches!(
        out.classified.as_slice(),
        [OwnerDomainMatchDisposition::Unresolved { .. }]
    ));
    assert!(out.problems.is_empty() && out.edges.is_empty());
    // Start unbounded instead: the native sign partition introduces this
    // singleton only AFTER matching, exercising the applied carrier boundary.
    batch(&mut p).rules[0].rhs[0].shift = [i64::MIN, 0, -1];
    let (out, _) = collect(&p, [large, 0, 0], [None; 3], Some(10));
    assert!(matches!(
        out.problems.as_slice(),
        [OwnerAppliedProblemKind::UnresolvedFixedCoordinate { axis: 0 }]
    ));
    batch(&mut p).rules[0].rhs[0].shift = [-2, 0, -1];
    let (out, _) = collect(&p, [2, 0, 0], [None, None, None], Some(u32::MAX));
    assert!(matches!(
        out.problems.as_slice(),
        [OwnerAppliedProblemKind::UnresolvedImage { .. }]
    ));
    assert!(out.edges.is_empty());
}

#[test]
fn applied_rank_forced_zero_is_specialized_without_enumerating_positive_powers() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [0, 0, 1], c.index(2).unwrap())])];
    let (out, stats) = collect(&p, [0; 3], [None; 3], Some(0));
    assert!(out.edges.is_empty() && out.problems.is_empty());
    assert_eq!(stats.zero_terms, 1);
    // The inactive tail n2<=-1 is outside the inherited rank simplex.
    assert_eq!(stats.boundary_cells, 1);
}

#[test]
fn applied_budget_refusal_cancel_and_consumer_stop_keep_typed_prefix() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-2, 0, 0], c.integer(1))])];
    let defaults = OwnerAppliedLimits::default();
    let cases = [
        (
            OwnerAppliedLimits {
                max_term_visits: 0,
                ..defaults
            },
            "RHS term visits",
        ),
        (
            OwnerAppliedLimits {
                max_shift_groups: 0,
                ..defaults
            },
            "shift groups",
        ),
        (
            OwnerAppliedLimits {
                max_boundary_cells: 0,
                ..defaults
            },
            "boundary cells",
        ),
        (
            OwnerAppliedLimits {
                max_sign_splits: 0,
                ..defaults
            },
            "sign splits",
        ),
        (
            OwnerAppliedLimits {
                max_native_operations: 0,
                ..defaults
            },
            "native operations",
        ),
        (
            OwnerAppliedLimits {
                max_events: 0,
                ..defaults
            },
            "events",
        ),
        (
            OwnerAppliedLimits {
                max_scratch_terms: 0,
                ..defaults
            },
            "staged term indices",
        ),
        (
            OwnerAppliedLimits {
                max_scratch_boxes: 0,
                ..defaults
            },
            "scratch boxes",
        ),
        (
            OwnerAppliedLimits {
                max_scratch_coordinate_cells: 0,
                ..defaults
            },
            "scratch coordinate cells",
        ),
    ];
    for (limits, expected) in cases {
        let error = p
            .visit_owner_applied_successors(
                OWNER,
                &[0; 3],
                &[None; 3],
                Some(10),
                limits,
                &AtomicBool::new(false),
                |_| ControlFlow::Continue(()),
            )
            .unwrap_err();
        assert!(
            matches!(error.failure, OwnerAppliedFailure::ResourceLimit {resource,..} if resource == expected),
            "{error:?}"
        );
        assert_eq!(error.stats.successors, 0);
    }
    let error = p
        .visit_owner_applied_successors(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(10),
            defaults,
            &AtomicBool::new(true),
            |_| panic!("cancelled before output"),
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        OwnerAppliedFailure::Cancelled
            | OwnerAppliedFailure::Matching(super::super::OwnerDomainMatchFailure::Cancelled)
    ));
    let error = p
        .visit_owner_applied_successors(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(10),
            defaults,
            &AtomicBool::new(false),
            |_| ControlFlow::Break(()),
        )
        .unwrap_err();
    assert_eq!(error.failure, OwnerAppliedFailure::StoppedByConsumer);
    assert_eq!(error.stats.events, 1);
    batch(&mut p).rules[0].rhs.clear();
    let error = p
        .visit_owner_applied_successors(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(10),
            OwnerAppliedLimits {
                max_scratch_boxes: 0,
                ..defaults
            },
            &AtomicBool::new(false),
            |_| ControlFlow::Continue(()),
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        OwnerAppliedFailure::ResourceLimit {
            resource: "scratch boxes",
            ..
        }
    ));
}

#[test]
fn applied_small_integer_oracle_matches_native_evaluator_and_noninvolutive_order() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let priority =
        crate::sector::CoordinatePriority::try_new(3, &[1, 2, 0], Default::default()).unwrap();
    Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&OWNER)
            .unwrap(),
    )
    .unwrap()
    .ordering =
        crate::sector::OrderingPolicy::try_spired_with_coordinate_priority(&priority).unwrap();
    batch(&mut p).rules = vec![rule(
        19,
        vec![
            term(&c, [-1, 0, 0], c.index(0).unwrap()),
            term(&c, [-1, 0, 0], c.integer(2)),
        ],
    )];
    for x0 in 1..=3_u64 {
        for x1 in 0..=2_u64 {
            for x2 in 0..=2_u64 {
                let local = [x0, x1, x2];
                let powers = [x0 as i64 + 1, x1 as i64 + 1, -(x2 as i64)];
                let (out, _) = collect(&p, local, local.map(Some), Some(10));
                assert!(out.problems.is_empty());
                assert_eq!(out.edges.len(), 1);
                let edge = &out.edges[0];
                let child: [i64; 3] = std::array::from_fn(|axis| {
                    if edge.target[axis] {
                        edge.lower[axis] as i64 + 1
                    } else {
                        -(edge.lower[axis] as i64)
                    }
                });
                let owner = &p.owners[&OWNER];
                let shared = &p.context.shared;
                let actual = CandidateEvaluator {
                    context: &shared.context,
                    root_sector: owner.root,
                    ordering: owner.ordering,
                    rules: &owner.batches[0].rules,
                    source_conditions: &shared.source_conditions,
                    zero_sectors: &shared.zero_sectors,
                    limits: p.context.limits,
                }
                .apply(
                    &IntegralKey::try_new(powers).unwrap(),
                    &mut ReductionRequest::default(),
                    &mut ReductionStatistics::default(),
                )
                .unwrap();
                assert_eq!(actual.len(), 1);
                assert_eq!(
                    actual[&IntegralKey::try_new(child).unwrap()],
                    c.specialize_sealed(&edge.coefficient, &powers, Default::default())
                        .unwrap()
                        .0
                );
            }
        }
    }
}
