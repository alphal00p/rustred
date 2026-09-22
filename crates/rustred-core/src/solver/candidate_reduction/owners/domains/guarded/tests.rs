//! Synthetic formulas test guarded semantics, not IBP provenance or feasibility.
use super::*;
use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::solver::candidate_reduction::owners::domains::{
    OwnerAppliedFailure, OwnerAppliedProblemKind,
};
use crate::solver::candidate_reduction::{
    model::{PreparedRule, PreparedTerm},
    owner_test_support::{input, programs},
    owners::{CandidateOwnerPrograms, model::PreparedOwnerBatch},
};
use crate::solver::{AffineCase, AffineIntersection, Case, CoordinateCase};
use std::ops::ControlFlow;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const OWNER: [bool; 3] = [true, true, false];

#[test]
fn correlated_guarded_pullback_is_explicitly_refused() {
    let powers = crate::solver::candidate_reduction::power_domain::DomainPowerBounds {
        max_positive_power: Some(4),
        ..Default::default()
    };
    let error = fixture()
        .visit_power_bounded_owner_guarded_rule_successors(
            OWNER,
            0,
            0,
            &[0; 3],
            &[None; 3],
            Some(3),
            powers,
            Default::default(),
            &AtomicBool::new(false),
            |_| panic!("unsupported domain must emit nothing"),
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        OwnerGuardedFailure::UnsupportedPowerBounds(powers)
    );
    assert_eq!(error.stats.events, 0);
}
fn fixture() -> Arc<CandidateOwnerPrograms<3>> {
    let mut p = programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input(OWNER, Some(10), vec![], &[])],
        Default::default(),
    );
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
fn diagonal(c: &IndexedCoefficientContext, multiplier: i64) -> IndexedPolynomial {
    poly(
        c,
        &c.sub(
            &c.mul(&c.integer(multiplier), &c.index(0).unwrap()).unwrap(),
            &c.index(1).unwrap(),
        )
        .unwrap(),
    )
}
fn rule(
    p: &CandidateOwnerPrograms<3>,
    multiplier: i64,
    coefficient: IndexedCoefficient,
    shift: [i64; 3],
) -> PreparedRule<3> {
    let c = p.context.coefficient_context();
    let equality = diagonal(c, multiplier);
    let AffineIntersection::Affine(affine) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &[equality.raw().clone()],
        p.context.index_variables(),
        &OWNER,
    )
    .unwrap() else {
        panic!("saved affine fixture");
    };
    PreparedRule {
        ordinal: 7,
        fixed: *affine.face().fixed(),
        case: affine.into(),
        equalities: vec![equality],
        exceptions: vec![],
        rhs: vec![PreparedTerm {
            shift,
            denominator: poly(c, &c.integer(1)),
            coefficient,
        }],
    }
}
#[derive(Default, Debug)]
struct Output {
    residuals: Vec<OwnerGuardedResidualKind>,
    coefficients: Vec<IndexedCoefficient>,
    images: Vec<([i128; 3], Option<u32>, Vec<u64>, Vec<Option<u64>>)>,
    problems: Vec<OwnerAppliedProblemKind>,
    finished: Vec<(usize, usize)>,
    conjunction_widths: Vec<usize>,
    denominator_count: usize,
    admitted: usize,
}
fn collect(
    p: &CandidateOwnerPrograms<3>,
    lower: [u64; 3],
    upper: [Option<u64>; 3],
    rank: Option<u32>,
    limits: OwnerGuardedLimits,
) -> (Output, Result<OwnerGuardedStats, OwnerGuardedError>) {
    let mut out = Output::default();
    let result = p.visit_owner_guarded_rule_successors(
        OWNER,
        0,
        7,
        &lower,
        &upper,
        rank,
        limits,
        &AtomicBool::new(false),
        |event| {
            match event {
                OwnerGuardedEvent::Admitted(d) => {
                    out.admitted += 1;
                    assert_eq!(d.rule_ordinal(), 7);
                    out.conjunction_widths =
                        d.excluded_conjunctions().iter().map(Vec::len).collect();
                    out.denominator_count = d.original_denominators().count();
                }
                OwnerGuardedEvent::Residual { kind, .. } => out.residuals.push(kind),
                OwnerGuardedEvent::Successor(s) => {
                    assert_eq!(s.image.source.owner(), &OWNER);
                    out.coefficients.push(s.coefficient.clone());
                    out.images.push((
                        s.image.argument_shift,
                        s.image.source.max_numerator_rank(),
                        s.image.source_lower.to_vec(),
                        s.image.source_upper.to_vec(),
                    ));
                }
                OwnerGuardedEvent::Problem { problem, .. } => out.problems.push(problem.kind),
                OwnerGuardedEvent::RuleFinished {
                    successors,
                    problems,
                    ..
                } => out.finished.push((successors, problems)),
                OwnerGuardedEvent::OptionalCoefficientRefusal { .. } => {
                    panic!("unexpected optional refusal")
                }
            }
            ControlFlow::Continue(())
        },
    );
    (out, result)
}

#[test]
fn guarded_rational_chart_joint_scaling_and_exact_rank_image() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let n0 = c.index(0).unwrap();
    let coefficient = c.div(&n0, &c.add(&n0, &c.integer(1)).unwrap()).unwrap();
    let r = rule(&p, 2, coefficient, [-1, 0, 0]);
    assert!(!r.case.affine().unwrap().has_integral_chart());
    batch(&mut p).rules = vec![r];
    let (out, stats) = collect(
        &p,
        [1, 1, 0],
        [None, None, Some(0)],
        Some(11),
        Default::default(),
    );
    assert_eq!(stats.unwrap().successors, 1);
    let n1 = c.index(1).unwrap();
    assert_eq!(
        out.coefficients,
        [c.div(&n1, &c.add(&n1, &c.integer(2)).unwrap()).unwrap()]
    );
    assert_eq!(
        out.images,
        [(
            [1, 0, 0],
            Some(11),
            vec![1, 1, 0],
            vec![None, None, Some(0)]
        )]
    );
    assert_eq!(
        out.residuals,
        [OwnerGuardedResidualKind::IncomingComplement]
    );
    assert_eq!(out.finished, [(1, 0)]);
    assert!(out.problems.is_empty());
}

#[test]
fn guarded_pinched_image_retains_source_simplex_beyond_target_rank_envelope() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let r = rule(&p, 1, c.integer(1), [-2, 0, 0]);
    batch(&mut p).rules = vec![r];
    let mut pinches = Vec::new();
    let stats = p
        .visit_owner_guarded_rule_successors(
            OWNER,
            0,
            7,
            &[0; 3],
            &[None; 3],
            Some(11),
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                match event {
                    OwnerGuardedEvent::Successor(s) if !s.target_sector[0] => {
                        assert_eq!(s.image.argument_shift, [2, 0, 0]); // m0+2=m1
                        assert_eq!(s.image.source.max_numerator_rank(), Some(11));
                        assert_eq!(s.image.source_upper[2], None); // exact simplex remains separate
                        pinches.push((s.image.source_lower[0], s.target_rank_limit));
                    }
                    OwnerGuardedEvent::Problem { problem, .. } => panic!("unexpected {problem:?}"),
                    OwnerGuardedEvent::RuleFinished { problems, .. } => assert_eq!(problems, 0),
                    _ => {}
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    pinches.sort_unstable();
    assert_eq!(pinches, [(0, Some(12)), (1, Some(11))]);
    assert_eq!(stats.successors, 3);
}

#[test]
fn guarded_whole_exclusion_and_original_poles_survive_zero_rhs() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut r = rule(&p, 1, c.integer(1), [-1, 0, 0]);
    r.exceptions = vec![vec![diagonal(&c, 1), poly(&c, &c.index(0).unwrap())]];
    batch(&mut p).rules = vec![r];
    let (out, result) = collect(
        &p,
        [1, 1, 0],
        [None, None, Some(0)],
        Some(10),
        Default::default(),
    );
    result.unwrap();
    assert_eq!(out.conjunction_widths, [2]);
    assert_eq!(out.denominator_count, 1);
    assert_eq!(out.finished, [(1, 0)]);
    // An entire AND vanishes on the saved diagonal, not merely one atom.
    batch(&mut p).rules[0].exceptions[0][1] = diagonal(&c, 1);
    let (out, result) = collect(
        &p,
        [1, 1, 0],
        [None, None, Some(0)],
        Some(10),
        Default::default(),
    );
    result.unwrap();
    assert_eq!(
        out.residuals,
        [
            OwnerGuardedResidualKind::IncomingComplement,
            OwnerGuardedResidualKind::ExcludedConjunction { branch: 0 }
        ]
    );
    assert_eq!(out.admitted, 0);
    let r = &mut batch(&mut p).rules[0];
    r.exceptions.clear();
    r.rhs[0].coefficient = c.zero();
    r.rhs[0].denominator = diagonal(&c, 1);
    let (out, result) = collect(
        &p,
        [1, 1, 0],
        [None, None, Some(0)],
        Some(10),
        Default::default(),
    );
    result.unwrap();
    assert_eq!(
        out.residuals,
        [
            OwnerGuardedResidualKind::IncomingComplement,
            OwnerGuardedResidualKind::OriginalDenominatorZero { term: 0 }
        ]
    );
    assert!(out.finished.is_empty() && out.coefficients.is_empty());
}

#[test]
fn guarded_original_child_source_validity_precedes_native_cancellation() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut r = rule(&p, 1, c.integer(1), [-1, 0, 0]);
    let opposite = PreparedTerm {
        shift: r.rhs[0].shift,
        coefficient: c.integer(-1),
        denominator: r.rhs[0].denominator.clone(),
    };
    r.rhs.push(opposite);
    batch(&mut p).rules = vec![r];
    // Incoming C=1 on n0=n1. The actual child shift makes C=0.
    let condition = poly(
        &c,
        &c.add(
            &c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.integer(1),
        )
        .unwrap(),
    );
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions = vec![condition];
    let (out, result) = collect(
        &p,
        [1, 1, 0],
        [None, None, Some(0)],
        Some(10),
        Default::default(),
    );
    let stats = result.unwrap();
    assert_eq!(
        out.problems,
        [OwnerAppliedProblemKind::InvalidChildSourceCondition { ordinal: 0 }]
    );
    assert_eq!(out.finished, [(0, 1)]);
    assert_eq!(stats.applied.coalescing_additions, 0);
    assert_eq!(stats.applied.cancelled_groups, 0);
}

#[test]
fn guarded_non_tangent_physical_translation_precedes_case_restriction() {
    let p = fixture();
    let c = p.context.coefficient_context();
    let r = rule(&p, 1, c.integer(1), [-1, 0, 0]);
    let equation = diagonal(c, 1);
    let shifted = c
        .translate_polynomial_sealed(&equation, &[1, 0, 0], Default::default())
        .unwrap();
    let cancel = AtomicBool::new(false);
    let mut budget = super::super::applied::Budget {
        limits: Default::default(),
        stats: Default::default(),
        cancel: &cancel,
    };
    let restricted = super::super::applied::restriction::polynomial(
        c,
        &shifted,
        &[],
        r.case.affine(),
        Default::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(restricted, poly(c, &c.integer(1)));
    // P(m-s), with s=(1,0,0), remains the translated diagonal, not P(m).
    let image = c
        .translate_polynomial_sealed(&equation, &[-1, 0, 0], Default::default())
        .unwrap();
    let expected = poly(
        c,
        &c.sub(
            &c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.integer(1),
        )
        .unwrap(),
    );
    assert_eq!(image, expected);
    assert_eq!(
        super::engine::inverse_shift(&[i64::MIN, i64::MAX, 0]),
        [1i128 << 63, -i128::from(i64::MAX), 0]
    );
}

#[test]
fn guarded_source_invalidity_and_empty_rank_are_not_whole_input_gaps() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let r = rule(&p, 1, c.integer(1), [-1, 0, 0]);
    batch(&mut p).rules = vec![r];
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions = vec![diagonal(&c, 1)];
    let (out, result) = collect(
        &p,
        [1, 1, 0],
        [None, None, Some(0)],
        Some(10),
        Default::default(),
    );
    result.unwrap();
    assert_eq!(
        out.residuals,
        [
            OwnerGuardedResidualKind::IncomingComplement,
            OwnerGuardedResidualKind::InvalidSourceCondition { ordinal: 0 }
        ]
    );
    assert!(out.finished.is_empty());
    let (out, result) = collect(&p, [1, 1, 12], [None; 3], Some(11), Default::default());
    assert_eq!(result.unwrap().applied.native_operations, 0);
    assert_eq!(out.residuals, [OwnerGuardedResidualKind::RankEmpty]);
}

#[test]
fn guarded_reuses_strict_descent_and_equal_shift_coalescence() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut r = rule(&p, 1, c.index(0).unwrap(), [-1, 0, 0]);
    let opposite = PreparedTerm {
        shift: r.rhs[0].shift,
        coefficient: c.sub(&c.zero(), &r.rhs[0].coefficient).unwrap(),
        denominator: r.rhs[0].denominator.clone(),
    };
    r.rhs.push(opposite);
    batch(&mut p).rules = vec![r];
    let (out, result) = collect(
        &p,
        [1, 1, 0],
        [None, None, Some(0)],
        Some(10),
        Default::default(),
    );
    assert_eq!(result.unwrap().applied.cancelled_groups, 1);
    assert_eq!(out.finished, [(0, 0)]);
    for t in &mut batch(&mut p).rules[0].rhs {
        t.shift = [0; 3];
    }
    let (out, result) = collect(
        &p,
        [1, 1, 0],
        [None, None, Some(0)],
        Some(10),
        Default::default(),
    );
    assert_eq!(result.unwrap().applied.coalescing_additions, 0);
    assert!(matches!(
        out.problems.as_slice(),
        [OwnerAppliedProblemKind::DescentNotEstablished { .. }]
    ));
    assert_eq!(out.finished, [(0, 1)]);
}

#[test]
fn guarded_scratch_inventory_events_and_cancellation_are_bounded() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let r = rule(&p, 1, c.integer(1), [-1, 0, 0]);
    batch(&mut p).rules = vec![r];
    let limits = OwnerGuardedLimits::default();
    for kind in 0..4 {
        let mut cap = limits;
        match kind {
            0 => cap.max_predicates = 0,
            1 => cap.applied.max_scratch_boxes = 0,
            2 => cap.applied.max_scratch_coordinate_cells = 29, // outer 10*N
            _ => cap.max_predicate_terms = 0,
        }
        let (out, result) = collect(&p, [1, 1, 0], [None, None, Some(0)], Some(10), cap);
        assert!(result.is_err());
        assert_eq!(out.admitted, 0);
        assert!(out.residuals.is_empty());
    }
    let mut cap = limits;
    cap.applied.max_native_operations = 0;
    let (out, result) = collect(&p, [1, 1, 0], [None, None, Some(0)], Some(10), cap);
    let error = result.unwrap_err();
    assert!(matches!(
        error.failure,
        OwnerGuardedFailure::Applied(OwnerAppliedFailure::ResourceLimit {
            resource: "native operations",
            ..
        })
    ));
    assert_eq!(error.stats.applied.native_operations, 0);
    assert_eq!(
        out.residuals,
        [OwnerGuardedResidualKind::IncomingComplement]
    );
    assert_eq!(out.admitted, 0);
    let mut cap = limits;
    cap.applied.max_scratch_boxes = 16; // outer5 + inner12 cannot coexist
    let (out, result) = collect(&p, [1, 1, 0], [None, None, Some(0)], Some(10), cap);
    assert!(matches!(
        result.unwrap_err().failure,
        OwnerGuardedFailure::Applied(OwnerAppliedFailure::ResourceLimit {
            resource: "scratch boxes",
            ..
        })
    ));
    assert!(out.finished.is_empty());
    let mut cap = limits;
    cap.max_events = 1;
    let (out, result) = collect(&p, [1, 1, 0], [None, None, Some(0)], Some(10), cap);
    assert!(matches!(
        result.unwrap_err().failure,
        OwnerGuardedFailure::ResourceLimit {
            resource: "guarded events",
            ..
        }
    ));
    assert_eq!(
        out.residuals,
        [OwnerGuardedResidualKind::IncomingComplement]
    );
    let cancel = AtomicBool::new(false);
    let error = p
        .visit_owner_guarded_rule_successors(
            OWNER,
            0,
            7,
            &[1, 1, 0],
            &[None, None, Some(0)],
            Some(10),
            limits,
            &cancel,
            |_| {
                cancel.store(true, Ordering::Release);
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert_eq!(error.failure, OwnerGuardedFailure::Cancelled);
    assert_eq!(error.stats.events, 1);
    let error = p
        .visit_owner_guarded_rule_successors(
            OWNER,
            0,
            7,
            &[1, 1, 0],
            &[None, None, Some(0)],
            Some(10),
            limits,
            &AtomicBool::new(false),
            |_| ControlFlow::Break(()),
        )
        .unwrap_err();
    assert_eq!(error.failure, OwnerGuardedFailure::StoppedByConsumer);
}

#[test]
fn guarded_preparation_retains_arc_case_without_forging_selection() {
    use crate::solver::{ExceptionalConditions, RuleCandidate, SearchStats, SectorRule};
    let p = fixture();
    let c = p.context.coefficient_context();
    let r = rule(&p, 1, c.integer(1), [-1, 0, 0]);
    let saved = r.case.clone();
    let candidate = RuleCandidate {
        target: saved.integral(),
        case: saved.clone(),
        rhs: vec![],
        sources: vec![],
        stats: SearchStats::default(),
    };
    let prepared = CandidateOwnerPrograms::try_new(
        p.context.clone(),
        vec![input(
            OWNER,
            Some(10),
            vec![SectorRule {
                candidate,
                exceptions: ExceptionalConditions::default(),
            }],
            &[],
        )],
    )
    .unwrap();
    let retained = &prepared.owners[&OWNER].batches[0].rules[0].case;
    match (&saved, retained) {
        (Case::Affine(a), Case::Affine(b)) => assert!(Arc::ptr_eq(a, b)),
        _ => panic!("retained chart"),
    }
    let error = prepared
        .visit_owner_guarded_rule_successors(
            OWNER,
            0,
            999,
            &[0; 3],
            &[None; 3],
            Some(11),
            Default::default(),
            &AtomicBool::new(false),
            |_| panic!("invalid candidate cannot publish"),
        )
        .unwrap_err();
    assert_eq!(error.failure, OwnerGuardedFailure::UnknownCandidate);
}
