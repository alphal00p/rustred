//! Optional support annotation must not weaken mandatory application checks.
use super::super::{algebra, engine::Budget};
use super::*;
use crate::algebra::IndexedAlgebraError;
use crate::foundry::completion::LatticeBox;
use std::sync::atomic::Ordering;

fn small_limits() -> OwnerAppliedLimits {
    let mut limits = OwnerAppliedLimits::default();
    limits.matching.guard_algebra.max_gcd_factor_work = 64;
    limits
}

fn coupled(c: &IndexedCoefficientContext) -> IndexedCoefficient {
    c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap()
}

fn refusing_fixture() -> Arc<CandidateOwnerPrograms<3>> {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(19, vec![term(&c, [-1, 0, 0], coupled(&c))])];
    p
}

#[derive(Debug)]
struct Refusal {
    disposition: OwnerDomainMatchDisposition,
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    rank: Option<u32>,
    shift: [i64; 3],
    term: Option<usize>,
    error: IndexedAlgebraError,
}

#[derive(Default)]
struct Observed {
    edges: Vec<Edge>,
    refusals: Vec<Refusal>,
    problems: Vec<(OwnerAppliedProblemKind, OwnerAppliedNonzero)>,
    finished: usize,
}

fn inspect(
    p: &CandidateOwnerPrograms<3>,
    lower: [u64; 3],
    limits: OwnerAppliedLimits,
) -> (Observed, Result<OwnerAppliedStats, OwnerAppliedError>) {
    let mut out = Observed::default();
    let result = p.visit_owner_applied_successors(
        OWNER,
        &lower,
        &[None, None, Some(0)],
        Some(11),
        limits,
        &AtomicBool::new(false),
        |event| {
            match event {
                OwnerAppliedEvent::Classified(_) => {}
                OwnerAppliedEvent::OptionalCoefficientRefusal {
                    source,
                    source_lower,
                    source_upper,
                    shift,
                    original_term_ordinal,
                    failure,
                } => out.refusals.push(Refusal {
                    disposition: source.disposition(),
                    lower: source_lower.to_vec(),
                    upper: source_upper.to_vec(),
                    rank: source.max_numerator_rank(),
                    shift: *shift,
                    term: original_term_ordinal,
                    error: failure.clone(),
                }),
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
                OwnerAppliedEvent::Problem(problem) => out
                    .problems
                    .push((problem.kind, problem.coefficient_nonzero)),
                OwnerAppliedEvent::RuleFinished { .. } => out.finished += 1,
            }
            ControlFlow::Continue(())
        },
    );
    (out, result)
}

#[test]
fn optional_refusal_matches_admitted_conditional_support_and_exact_rank() {
    let p = refusing_fixture();
    let (small, stats) = inspect(&p, [1, 0, 0], small_limits());
    let stats = stats.unwrap();
    let (admitted, ordinary) = inspect(&p, [1, 0, 0], OwnerAppliedLimits::default());
    let ordinary = ordinary.unwrap();
    assert_eq!(
        (
            stats.optional_coefficient_refusals,
            stats.optional_original_refusals,
            stats.optional_coalesced_refusals
        ),
        (2, 1, 1)
    );
    assert_eq!(ordinary.optional_coefficient_refusals, 0);
    assert_eq!(stats.native_operations, ordinary.native_operations);
    assert_eq!(stats.events, ordinary.events + 2);
    assert_eq!(
        (small.edges.len(), admitted.edges.len(), small.finished),
        (1, 1, 1)
    );
    assert!(small.problems.is_empty());
    let edge = &small.edges[0];
    assert_eq!(edge.coefficient, admitted.edges[0].coefficient);
    assert_eq!((edge.target, edge.rank), (OWNER, Some(11)));
    assert_eq!(edge.lower, [0, 0, 0]);
    assert_eq!(edge.upper, [None, None, Some(0)]);
    assert_eq!(edge.nonzero, OwnerAppliedNonzero::Conditional);
    assert_eq!(edge.nonzero, admitted.edges[0].nonzero);
    assert_eq!(small.refusals.len(), 2);
    for (index, refusal) in small.refusals.iter().enumerate() {
        assert_eq!(
            refusal.disposition,
            OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 19 }
        );
        assert_eq!(refusal.lower, [1, 0, 0]);
        assert_eq!(refusal.upper, [None, None, Some(0)]);
        assert_eq!(refusal.rank, Some(11));
        assert_eq!(refusal.shift, [-1, 0, 0]);
        assert_eq!(refusal.term, if index == 0 { Some(0) } else { None });
        assert_eq!(
            refusal.error,
            IndexedAlgebraError::ResourceLimit {
                resource: "guard separable factor work",
                requested: 128,
                limit: 64,
            }
        );
    }
}

#[test]
fn optional_refusal_can_first_occur_after_native_coalescing() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], c.index(0).unwrap()),
            term(
                &c,
                [-1, 0, 0],
                c.sub(&c.zero(), &c.index(1).unwrap()).unwrap(),
            ),
        ],
    )];
    let (out, stats) = inspect(&p, [1, 0, 0], small_limits());
    let stats = stats.unwrap();
    assert_eq!(
        (
            stats.optional_coefficient_refusals,
            stats.optional_original_refusals,
            stats.optional_coalesced_refusals
        ),
        (1, 0, 1)
    );
    assert_eq!(stats.coalescing_additions, 1);
    assert_eq!(out.refusals.len(), 1);
    assert_eq!(out.refusals[0].term, None);
    assert_eq!(out.edges.len(), 1);
    assert_eq!(out.edges[0].coefficient, coupled(&c));
    assert_eq!(out.edges[0].nonzero, OwnerAppliedNonzero::Conditional);
}

#[test]
fn optional_refusal_provenance_is_first_per_stage_not_per_term() {
    let mut p = refusing_fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules[0]
        .rhs
        .push(term(&c, [-2, 0, 0], coupled(&c)));
    let (out, stats) = inspect(&p, [2, 0, 0], small_limits());
    let stats = stats.unwrap();
    assert_eq!(
        (
            stats.optional_coefficient_refusals,
            stats.optional_original_refusals,
            stats.optional_coalesced_refusals
        ),
        (4, 2, 2)
    );
    assert_eq!((out.edges.len(), out.refusals.len()), (2, 2));
    assert_eq!(out.refusals[0].term, Some(1)); // lexicographic shift traversal
    assert_eq!(out.refusals[1].term, None);
    assert!(out.refusals.iter().all(|r| r.shift == [-2, 0, 0]));
    assert!(stats.optional_coefficient_refusals > out.refusals.len());
}

#[test]
fn optional_refusal_preserves_zero_cancellation_and_original_descent_checks() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let value = coupled(&c);
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], value.clone()),
            term(&c, [-1, 0, 0], c.sub(&c.zero(), &value).unwrap()),
            term(&c, [-2, 0, 0], c.zero()),
        ],
    )];
    let (out, stats) = inspect(&p, [2, 0, 0], small_limits());
    let stats = stats.unwrap();
    assert!(out.edges.is_empty() && out.problems.is_empty());
    assert_eq!(
        (
            stats.zero_terms,
            stats.cancelled_groups,
            stats.coalescing_additions
        ),
        (1, 1, 1)
    );
    assert_eq!(
        (
            stats.optional_original_refusals,
            stats.optional_coalesced_refusals
        ),
        (2, 0)
    );
    assert_eq!(out.refusals.len(), 1);
    batch(&mut p).rules[0].rhs.truncate(2);
    for term in &mut batch(&mut p).rules[0].rhs {
        term.shift = [0; 3];
    }
    let (out, stats) = inspect(&p, [2, 0, 0], small_limits());
    let stats = stats.unwrap();
    assert!(out.edges.is_empty());
    assert!(matches!(
        out.problems.as_slice(),
        [(
            OwnerAppliedProblemKind::DescentNotEstablished { .. },
            OwnerAppliedNonzero::Conditional
        )]
    ));
    assert_eq!(
        (
            stats.coalescing_additions,
            stats.cancelled_groups,
            stats.optional_original_refusals
        ),
        (0, 0, 1)
    );
}

#[test]
fn optional_refusal_does_not_swallow_denominator_or_child_source_preflight() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut original = term(&c, [-1, 0, 0], c.zero());
    original.denominator = poly(&c, &coupled(&c));
    batch(&mut p).rules = vec![rule(0, vec![original])];
    let (out, result) = inspect(&p, [1, 0, 0], small_limits());
    let error = result.unwrap_err();
    assert!(matches!(
        error.failure,
        OwnerAppliedFailure::Matching(super::super::super::OwnerDomainMatchFailure::Algebra(
            IndexedAlgebraError::ResourceLimit {
                resource: "guard separable factor work",
                requested: 128,
                limit: 64,
            }
        ))
    ));
    assert!(out.edges.is_empty() && out.refusals.is_empty());
    assert_eq!(error.stats.optional_coefficient_refusals, 0);

    // Incoming n0+n1 is uniformly positive, so matcher affine admission passes;
    // the strict shifted-source resolver still owns the same native refusal.
    batch(&mut p).rules = vec![rule(0, vec![term(&c, [-1, 0, 0], c.integer(1))])];
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions = vec![poly(
        &c,
        &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
    )];
    // Two native Integer limbs per endpoint, two terms: affine scan + 64
    // endpoint work fits 96 here, whereas native separable admission is 128.
    assert!(2 * (2 * (c.base().parameter_names().len() + 3) + 1) + 64 <= 96);
    let mut child_limits = small_limits();
    child_limits.matching.guard_algebra.max_gcd_factor_work = 96;
    let (out, result) = inspect(&p, [1, 0, 0], child_limits);
    let error = result.unwrap_err();
    assert_eq!(
        error.failure,
        OwnerAppliedFailure::Algebra(IndexedAlgebraError::ResourceLimit {
            resource: "guard separable factor work",
            requested: 128,
            limit: 96,
        })
    );
    assert_eq!(error.stats.optional_coefficient_refusals, 0);
    assert!(out.edges.is_empty() && out.refusals.is_empty());
}

#[test]
fn optional_refusal_keeps_input_context_and_global_native_errors_strict() {
    let p = fixture();
    let c = p.context.coefficient_context();
    let cell = LatticeBox::try_new([1, 0, 0], [None, None, Some(0)]).unwrap();
    let cancel = AtomicBool::new(false);
    let mut budget = Budget {
        limits: small_limits(),
        stats: Default::default(),
        cancel: &cancel,
    };
    budget.limits.matching.guard_algebra.max_input_terms = 0;
    let error = algebra::coefficient(
        c,
        &coupled(c),
        &cell,
        &OWNER,
        Some(11),
        p.context.limits().indexed_algebra,
        &mut budget,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        OwnerAppliedFailure::Algebra(IndexedAlgebraError::ResourceLimit {
            resource: "guard coefficient split input terms",
            ..
        })
    ));
    assert_eq!(budget.stats.optional_coefficient_refusals, 0);
    let foreign =
        IndexedCoefficientContext::try_new(c.base(), "optional-refusal-foreign", 3).unwrap();
    budget.limits = small_limits();
    let error = algebra::coefficient(
        c,
        &foreign.index(0).unwrap(),
        &cell,
        &OWNER,
        Some(11),
        p.context.limits().indexed_algebra,
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(
        error,
        OwnerAppliedFailure::Algebra(IndexedAlgebraError::WrongContext)
    );

    let p = refusing_fixture();
    let (out, result) = inspect(
        &p,
        [1, 0, 0],
        OwnerAppliedLimits {
            max_native_operations: 3,
            ..small_limits()
        },
    );
    let error = result.unwrap_err();
    assert_eq!(
        error.failure,
        OwnerAppliedFailure::ResourceLimit {
            resource: "native operations",
            requested: 4,
            limit: 3
        }
    );
    assert_eq!(
        (
            error.stats.native_operations,
            error.stats.optional_coefficient_refusals
        ),
        (3, 0)
    );
    assert!(out.refusals.is_empty());
}

#[test]
fn optional_refusal_event_limit_cancel_and_consumer_stop_keep_spent_prefix() {
    let p = refusing_fixture();
    let (out, result) = inspect(
        &p,
        [1, 0, 0],
        OwnerAppliedLimits {
            max_events: 1,
            ..small_limits()
        },
    );
    let error = result.unwrap_err();
    assert_eq!(
        error.failure,
        OwnerAppliedFailure::ResourceLimit {
            resource: "events",
            requested: 2,
            limit: 1
        }
    );
    assert_eq!(
        (
            error.stats.optional_coefficient_refusals,
            error.stats.optional_original_refusals,
            error.stats.optional_coalesced_refusals
        ),
        (1, 1, 0)
    );
    assert_eq!((error.stats.events, error.stats.native_operations), (1, 4));
    assert!(out.refusals.is_empty() && out.edges.is_empty());
    for stop in [false, true] {
        let cancel = AtomicBool::new(false);
        let mut observed = 0;
        let error = p
            .visit_owner_applied_successors(
                OWNER,
                &[1, 0, 0],
                &[None, None, Some(0)],
                Some(11),
                small_limits(),
                &cancel,
                |event| {
                    if matches!(event, OwnerAppliedEvent::OptionalCoefficientRefusal { .. }) {
                        observed += 1;
                        if stop {
                            return ControlFlow::Break(());
                        }
                        cancel.store(true, Ordering::Release);
                    }
                    assert!(!matches!(event, OwnerAppliedEvent::Successor(_)));
                    ControlFlow::Continue(())
                },
            )
            .unwrap_err();
        assert_eq!(
            error.failure,
            if stop {
                OwnerAppliedFailure::StoppedByConsumer
            } else {
                OwnerAppliedFailure::Cancelled
            }
        );
        assert_eq!(observed, 1);
        assert_eq!(
            (
                error.stats.events,
                error.stats.native_operations,
                error.stats.optional_coefficient_refusals
            ),
            (2, 4, 1)
        );
        assert_eq!(error.stats.successors, 0);
    }
}

#[test]
fn optional_refusal_counter_overflow_is_atomic() {
    let cancel = AtomicBool::new(false);
    let mut budget = Budget {
        limits: Default::default(),
        stats: Default::default(),
        cancel: &cancel,
    };
    assert!(budget.optional_refusal(true).unwrap());
    assert!(budget.optional_refusal(false).unwrap());
    assert!(!budget.optional_refusal(true).unwrap());
    assert_eq!(
        (
            budget.stats.optional_coefficient_refusals,
            budget.stats.optional_original_refusals,
            budget.stats.optional_coalesced_refusals
        ),
        (3, 2, 1)
    );
    budget.stats.optional_coefficient_refusals = usize::MAX;
    budget.stats.optional_original_refusals = usize::MAX - 1;
    let before = budget.stats;
    assert!(matches!(
        budget.optional_refusal(true),
        Err(OwnerAppliedFailure::CountOverflow { .. })
    ));
    assert_eq!(budget.stats, before);
    // Even a malformed internal counter state cannot partially commit total.
    for original in [false, true] {
        budget.stats = OwnerAppliedStats {
            optional_original_refusals: usize::MAX,
            optional_coalesced_refusals: usize::MAX,
            ..Default::default()
        };
        let before = budget.stats;
        assert!(matches!(
            budget.optional_refusal(original),
            Err(OwnerAppliedFailure::CountOverflow { .. })
        ));
        assert_eq!(budget.stats, before);
    }
}

#[test]
fn optional_refusal_preserves_constant_and_uniform_labels() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    batch(&mut p).rules = vec![rule(
        0,
        vec![
            term(&c, [-1, 0, 0], c.integer(2)),
            term(&c, [-2, 0, 0], c.index(0).unwrap()),
        ],
    )];
    let (out, result) = inspect(&p, [2, 0, 0], small_limits());
    let stats = result.unwrap();
    assert_eq!(out.edges.len(), 2);
    assert!(
        out.edges
            .iter()
            .all(|e| e.nonzero == OwnerAppliedNonzero::Uniform)
    );
    assert!(out.refusals.is_empty());
    assert_eq!(stats.optional_coefficient_refusals, 0);
}
