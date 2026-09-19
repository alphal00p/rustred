use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::{Coefficient, IndexedCoefficientContext};
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::identity::ParametricIbpGenerator;
use crate::reduction::{Reducer, ReductionError, ReductionLimits};
use crate::sector::{Mask, OrderingPolicy, zero};
use crate::solver::{
    AffineCase, AffineIntersection, CoordinateCase, ExceptionalConditions, Integral, RuleCandidate,
    SearchStats, SectorConfig, SectorRule, SectorSolution, SectorSolveOptions, SectorSolver,
    SectorStats, SourceSystem, Term,
};

use super::{CandidateReducer, CandidateReductionError};

#[path = "tests/factorized_audit.rs"]
mod factorized_audit;
#[path = "tests/terminal_alias_audit.rs"]
mod terminal_alias_audit;
#[path = "tests/terminal_aliases.rs"]
mod terminal_aliases;

fn key<const N: usize>(powers: [i64; N]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn solved<const N: usize>(
    family: &IntegralFamily,
) -> (Vec<([bool; N], SectorSolution<N>)>, Vec<zero::Certificate>) {
    // Exhaustive enumeration is only a tiny K1/K3 test-fixture service.
    assert!(N <= 3);
    let analyzer = zero::Analyzer::try_unrestricted(family).unwrap();
    let mut certificates = Vec::new();
    let mut nonzero = Vec::new();
    let mut zeros = Vec::new();
    for code in 0..(1_usize << N) {
        let sector = std::array::from_fn(|axis| code & (1 << axis) != 0);
        match analyzer.analyze(&Mask::try_new(sector).unwrap()).unwrap() {
            zero::Decision::ProvedZero(certificate) => {
                zeros.push(sector);
                certificates.push(certificate);
            }
            zero::Decision::Inconclusive(_) => nonzero.push(sector),
            zero::Decision::Excluded(_) => {
                panic!("unrestricted vacuum fixture must not exclude sectors")
            }
        }
    }
    let source = SourceSystem::<N>::from_family(family).unwrap();
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros),
        ..Default::default()
    };
    let records = nonzero
        .into_iter()
        .map(|sector| {
            let solution = SectorSolver::new(&source, sector, config.clone())
                .unwrap()
                .solve_sector(SectorSolveOptions::default())
                .unwrap();
            (sector, solution)
        })
        .collect();
    (records, certificates)
}

fn candidate<const N: usize>(
    family: &IntegralFamily,
    limits: ReductionLimits,
) -> CandidateReducer<N> {
    let (records, zeros) = solved(family);
    CandidateReducer::try_new(
        family,
        [true; N],
        OrderingPolicy::SpiredUncutV1,
        records,
        zeros,
        limits,
    )
    .unwrap()
}

#[test]
fn candidate_k1_matches_certified_reducer_and_memoizes_without_inventing_terminals() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut candidate = candidate::<1>(artifact.family(), ReductionLimits::default());
    let mut certified = Reducer::new(&artifact).unwrap();
    assert_eq!(candidate.terminals().len(), 1);
    assert!(candidate.terminals().contains(&key([1])));
    for power in [-9, 0, 1, 2, 3, 7, 12] {
        let target = key([power]);
        let actual = candidate.reduce_unit_mass(&target).unwrap();
        assert_eq!(
            actual.terms(),
            certified.reduce_unit_mass(&target).unwrap().terms()
        );
        assert_eq!(actual.family_fingerprint(), artifact.family_fingerprint());
        for terminal in actual.terms().keys() {
            assert_eq!(
                actual.common_mass_squared_power(terminal).unwrap(),
                i128::from(1 - power)
            );
        }
    }
    let before = candidate.statistics();
    candidate.reduce_unit_mass(&key([12])).unwrap();
    assert_eq!(
        candidate.statistics().rule_applications(),
        before.rule_applications()
    );
    assert_eq!(candidate.statistics().cache_hits(), before.cache_hits() + 1);
    candidate.clear_cache().unwrap();
    assert_eq!(candidate.statistics().cached_integrals(), 0);
    assert_eq!(candidate.statistics().cached_coefficient_terms(), 0);
    assert_eq!(candidate.statistics().cached_coefficient_bytes(), 0);
}

#[test]
fn candidate_finite_scope_reports_reachability_without_claiming_certification() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut candidate = candidate::<1>(artifact.family(), ReductionLimits::default());
    let report = candidate
        .check_targets([key([-3]), key([0]), key([2])])
        .unwrap();
    assert_eq!(report.requested_targets(), 3);
    assert!(report.reachable_integrals() >= report.requested_targets());
    assert_eq!(report.reachable_terminals(), 1);
    assert!(report.max_negative_index_degree() >= 3);
    assert!(report.max_positive_power_sum() >= 2);
    // CandidateReducer remains explicitly uncertified: source provenance is
    // unreplayed, and this finite DAG says nothing about other integer points.
    assert_eq!(candidate.terminals().len(), 1);
}

#[test]
fn candidate_finite_scope_check_fails_closed_on_an_unreachable_target() {
    let family = crate::solver::tests::tadpole();
    let mut solution = one_rule(&family);
    solution.rules.clear();
    let mut candidate = from_one_rule(&family, solution);
    assert!(matches!(
        candidate.check_targets([key([2])]),
        Err(CandidateReductionError::Uncovered { target }) if target == key([2])
    ));
    assert_eq!(candidate.statistics().cached_integrals(), 0);
}

#[test]
fn candidate_k3_matches_certified_reductions_after_exact_finite_basis_mapping() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let mut candidate = candidate::<3>(artifact.family(), ReductionLimits::default());
    let mut certified = Reducer::new(&artifact).unwrap();
    let context = artifact.coefficient_context();
    for powers in [
        [1, 1, 1],
        [2, 1, 1],
        [1, 2, 3],
        [0, 2, 3],
        [-1, 2, 1],
        [3, 0, 2],
        [2, 2, -2],
        [0, 0, 0],
    ] {
        let target = key(powers);
        let actual = candidate.reduce_unit_mass(&target).unwrap();
        let mut mapped: BTreeMap<IntegralKey, Coefficient> = BTreeMap::new();
        // Candidate pinches need not already use the certified owner's S3
        // representative. Compare through its exact basis map, not samples.
        for (terminal, factor) in actual.terms() {
            assert!(candidate.terminals().contains(terminal));
            for (master, coefficient) in certified.reduce_unit_mass(terminal).unwrap().terms() {
                let contribution = context
                    .try_mul(factor, coefficient, Default::default())
                    .unwrap();
                let previous = mapped.remove(master).unwrap_or_else(|| context.zero());
                let sum = context
                    .try_add(&previous, &contribution, Default::default())
                    .unwrap();
                if !sum.is_zero() {
                    mapped.insert(master.clone(), sum);
                }
            }
        }
        assert_eq!(
            &mapped,
            certified.reduce_unit_mass(&target).unwrap().terms(),
            "{powers:?}"
        );
    }
    assert!(candidate.statistics().rule_applications() > 0);
}

fn one_rule(family: &IntegralFamily) -> SectorSolution<1> {
    let (mut sectors, _) = solved::<1>(family);
    assert_eq!(sectors.len(), 1);
    sectors.pop().unwrap().1
}

fn from_one_rule(family: &IntegralFamily, solution: SectorSolution<1>) -> CandidateReducer<1> {
    CandidateReducer::try_new(
        family,
        [true],
        OrderingPolicy::SpiredUncutV1,
        [([true], solution)],
        Vec::new(),
        ReductionLimits::default(),
    )
    .unwrap()
}

#[test]
fn unknown_points_never_become_new_terminals_and_symbolic_residuals_are_rejected() {
    let family = crate::solver::tests::tadpole();
    let mut solution = one_rule(&family);
    solution.rules.clear();
    let mut owner = from_one_rule(&family, solution);
    assert!(matches!(
        owner.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    assert_eq!(owner.statistics().cached_integrals(), 0);
    assert_eq!(owner.terminals().len(), 1);
    let mut solution = one_rule(&family);
    solution
        .finite_residuals
        .push(Integral::symbolic([0]).unwrap());
    assert!(matches!(
        CandidateReducer::try_new(
            &family,
            [true],
            OrderingPolicy::SpiredUncutV1,
            [([true], solution)],
            Vec::new(),
            ReductionLimits::default()
        ),
        Err(CandidateReductionError::InvalidInput(_))
    ));
}

#[test]
fn concrete_denominator_zero_is_not_cancelled_or_cached() {
    let family = crate::solver::tests::tadpole();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut solution = one_rule(&family);
    solution.rules.truncate(1);
    let denominator = context
        .sub(&context.index(0).unwrap(), &context.integer(2))
        .unwrap();
    solution.rules[0].candidate.rhs[0].coefficient = context
        .div(&context.one(), &denominator)
        .unwrap()
        .raw()
        .clone();
    let mut owner = from_one_rule(&family, solution);
    assert!(matches!(
        owner.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    assert_eq!(owner.statistics().cached_integrals(), 0);
}

#[test]
fn factorized_empty_zero_child_does_not_hide_the_original_pole() {
    let family = crate::solver::tests::tadpole();
    let mut owner = candidate::<1>(&family, ReductionLimits::default());
    owner
        .set_cache_representation(super::CandidateCacheRepresentation::Factorized)
        .unwrap();
    assert!(owner.reduce_unit_mass(&key([0])).unwrap().is_zero());
    let context = owner.coefficient_context().clone();
    let pole = context
        .admit_native_polynomial_result_with_limits(
            index_offset(&context, 0, 2),
            Default::default(),
        )
        .unwrap();
    let rules = owner.rules.get_mut(&[true]).unwrap();
    rules.truncate(1);
    rules[0].rhs.truncate(1);
    rules[0].rhs[0].shift = [-2];
    rules[0].rhs[0].denominator = pole;
    let before = owner.statistics();
    assert!(matches!(
        owner.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    assert_eq!(owner.statistics(), before);
}

#[test]
fn non_descending_and_overflowing_candidate_children_fail_closed() {
    let family = crate::solver::tests::tadpole();
    for target in [2, i64::MAX] {
        let mut solution = one_rule(&family);
        solution.rules.truncate(1);
        solution.rules[0].candidate.rhs[0].integral = Integral::symbolic([1]).unwrap();
        let mut owner = from_one_rule(&family, solution);
        let error = owner.reduce_unit_mass(&key([target])).unwrap_err();
        if target == 2 {
            assert!(matches!(
                error,
                CandidateReductionError::NonDescending { .. }
            ));
        } else {
            assert!(matches!(
                error,
                CandidateReductionError::IndexOverflow { .. }
            ));
        }
        assert_eq!(owner.statistics().cached_integrals(), 0);
    }
}

fn index_offset(
    context: &IndexedCoefficientContext,
    axis: usize,
    offset: i64,
) -> crate::algebra::CoefficientPolynomial {
    context
        .sub(&context.index(axis).unwrap(), &context.integer(offset))
        .unwrap()
        .raw()
        .numerator
        .clone()
}

fn affine_test_owner(block: bool) -> CandidateReducer<3> {
    let family = crate::solver::tests::sunset();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let source = SourceSystem::<3>::from_family(&family).unwrap();
    let parent = CoordinateCase::new([None, None, Some(1)]).unwrap();
    let equality = context
        .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap()
        .raw()
        .numerator
        .clone();
    let AffineIntersection::Affine(case) =
        AffineCase::from_coordinate(&parent, &[equality], source.index_variables(), &[true; 3])
            .unwrap()
    else {
        panic!("coupled case expected")
    };
    let target = case.face().integral();
    let rhs = vec![Term {
        integral: Integral::new([
            crate::solver::Power::new(true, -1).unwrap(),
            crate::solver::Power::new(true, 0).unwrap(),
            crate::solver::Power::new(false, 1).unwrap(),
        ]),
        coefficient: context.one().raw().clone(),
    }];
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: case.into(),
            target,
            rhs,
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions {
            branches: vec![vec![
                index_offset(&context, 0, 2),
                index_offset(&context, 1, if block { 2 } else { 3 }),
            ]],
        },
    };
    let solution = SectorSolution {
        rules: vec![rule],
        finite_residuals: vec![Integral::numeric([1, 2, 1]).unwrap()],
        stats: SectorStats::default(),
    };
    CandidateReducer::try_new(
        &family,
        [true; 3],
        OrderingPolicy::SpiredUncutV1,
        [([true; 3], solution)],
        Vec::new(),
        ReductionLimits::default(),
    )
    .unwrap()
}

#[test]
fn fixed_affine_cases_and_whole_exception_conjunctions_are_checked_exactly() {
    let mut owner = affine_test_owner(false);
    let output = owner.reduce_unit_mass(&key([2, 2, 1])).unwrap();
    assert_eq!(output.terms().len(), 1); // first exception factor zero alone is allowed.
    assert!(output.terms().contains_key(&key([1, 2, 1])));
    for target in [[2, 3, 1], [2, 2, 2]] {
        assert!(matches!(
            owner.reduce_unit_mass(&key(target)),
            Err(CandidateReductionError::Uncovered { .. })
        ));
    }
    assert!(matches!(
        affine_test_owner(true).reduce_unit_mass(&key([2, 2, 1])),
        Err(CandidateReductionError::Uncovered { .. })
    ));
}

#[test]
fn request_cache_and_coalescing_budgets_reuse_existing_reduction_services() {
    let family = crate::solver::tests::tadpole();
    for (limits, expected) in [
        (
            ReductionLimits {
                max_rule_applications: 0,
                ..Default::default()
            },
            "rule",
        ),
        (
            ReductionLimits {
                max_pending_frames: 0,
                ..Default::default()
            },
            "frame",
        ),
        (
            ReductionLimits {
                max_cached_integrals: 0,
                ..Default::default()
            },
            "cache",
        ),
        (
            ReductionLimits {
                max_coalescing_additions: 0,
                ..Default::default()
            },
            "add",
        ),
    ] {
        let mut owner = if expected == "add" {
            let (mut records, zeros) = solved::<1>(&family);
            // Coalescing counts actual occupied-entry additions, not insertion
            // of the sole ordinary K1 term. Duplicate that term for this
            // deliberately unproved formula/resource-policy test only.
            let rhs = &mut records[0].1.rules[0].candidate.rhs;
            rhs.push(rhs[0].clone());
            CandidateReducer::try_new(
                &family,
                [true],
                OrderingPolicy::SpiredUncutV1,
                records,
                zeros,
                limits,
            )
            .unwrap()
        } else {
            candidate::<1>(&family, limits)
        };
        let error = owner.reduce_unit_mass(&key([2])).unwrap_err();
        assert!(match (expected, error) {
            (
                "rule",
                CandidateReductionError::Application(ReductionError::RuleApplicationLimit {
                    ..
                }),
            )
            | (
                "frame",
                CandidateReductionError::Application(ReductionError::PendingFrameLimit { .. }),
            )
            | ("cache", CandidateReductionError::Application(ReductionError::CacheLimit { .. }))
            | (
                "add",
                CandidateReductionError::Application(ReductionError::CoalescingAdditionLimit {
                    ..
                }),
            ) => true,
            _ => false,
        });
    }
}

#[test]
fn cached_requests_still_check_source_conditions_arity_and_root_scope() {
    let family = crate::solver::tests::tadpole();
    let mut owner = candidate::<1>(&family, ReductionLimits::default());
    owner.reduce_unit_mass(&key([2])).unwrap();
    let context = owner.coefficient_context();
    let condition = context
        .admit_native_polynomial_result_with_limits(index_offset(context, 0, 2), Default::default())
        .unwrap();
    owner.source_conditions.push(condition);
    assert!(matches!(
        owner.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::SourceConditionVanished { .. })
    ));
    assert!(matches!(
        owner.reduce_unit_mass(&key([1, 2])),
        Err(CandidateReductionError::Application(
            ReductionError::WrongArity { .. }
        ))
    ));
    owner.root_sector = [false];
    assert!(matches!(
        owner.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::OutsideRoot { .. })
    ));
}

#[test]
fn constructor_rejects_duplicate_records_wrong_sector_terminals_and_foreign_zeros() {
    let family = crate::solver::tests::tadpole();
    assert!(
        CandidateReducer::try_new(
            &family,
            [true],
            OrderingPolicy::SpiredUncutV1,
            [([true], one_rule(&family)), ([true], one_rule(&family))],
            Vec::new(),
            ReductionLimits::default()
        )
        .is_err()
    );
    let mut solution = one_rule(&family);
    solution
        .finite_residuals
        .push(Integral::numeric([0]).unwrap());
    assert!(
        CandidateReducer::try_new(
            &family,
            [true],
            OrderingPolicy::SpiredUncutV1,
            [([true], solution)],
            Vec::new(),
            ReductionLimits::default()
        )
        .is_err()
    );
    let other = crate::solver::tests::sunset();
    let zero::Decision::ProvedZero(certificate) = zero::Analyzer::try_unrestricted(&other)
        .unwrap()
        .analyze(&Mask::try_new([false; 3]).unwrap())
        .unwrap()
    else {
        panic!("scaleless sector expected")
    };
    assert!(
        CandidateReducer::try_new(
            &family,
            [true],
            OrderingPolicy::SpiredUncutV1,
            [([true], one_rule(&family))],
            vec![certificate],
            ReductionLimits::default()
        )
        .is_err()
    );
}
