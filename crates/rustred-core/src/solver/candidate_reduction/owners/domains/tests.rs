//! Synthetic rules test the scan, not IBP provenance or recursive coverage.
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::family::IntegralFamily;
use crate::identity::ParametricIbpGenerator;
use crate::solver::candidate_reduction::owner_test_support::{input, programs};
use crate::solver::{
    AffineCase, AffineIntersection, CandidateOwnerPrograms, CoordinateCase, ExceptionalConditions,
    Integral, Power, RuleCandidate, SearchStats, SectorRule, SourceSystem, Term,
};

fn formula<const N: usize>(
    family: &IntegralFamily,
    fixed: [Option<i16>; N],
    shifts: &[([i16; N], i64)],
) -> SectorRule<N> {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    let case = CoordinateCase::new(fixed).unwrap();
    let target = case.integral();
    SectorRule {
        candidate: RuleCandidate {
            case: case.into(),
            target,
            rhs: shifts
                .iter()
                .map(|(shift, coefficient)| Term {
                    integral: Integral::new(std::array::from_fn(|axis| {
                        Power::new(
                            target[axis].is_symbolic(),
                            target[axis].value() + shift[axis],
                        )
                        .unwrap()
                    })),
                    coefficient: context.integer(*coefficient).raw().clone(),
                })
                .collect(),
            sources: vec![],
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions::default(),
    }
}

fn collect<'a, const N: usize>(
    programs: &'a CandidateOwnerPrograms<N>,
    sector: [bool; N],
    rank: Option<u32>,
) -> (OwnerSuccessorStats, Vec<OwnerSuccessorRegion<'a, N>>) {
    let mut regions = vec![];
    let stats = programs
        .visit_owner_rule_successors(
            sector,
            rank,
            Default::default(),
            &AtomicBool::new(false),
            |region| {
                regions.push(region);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    (stats, regions)
}

#[test]
fn same_support_rank_increase_keeps_unbounded_positive_tail_and_actual_requested_rank() {
    let family = Arc::new(crate::solver::tests::sunset());
    let rule = formula(&family, [None; 3], &[([0, 0, -1], 1)]);
    let programs = programs(
        family,
        Some(10),
        vec![input([true, true, false], Some(10), vec![rule], &[])],
        Default::default(),
    );
    for (rank, bound) in [(Some(10), Some(11)), (Some(20), Some(21)), (None, None)] {
        let (stats, regions) = collect(&programs, [true, true, false], rank);
        assert_eq!(stats.regions, 1);
        let region = &regions[0];
        assert_eq!(
            region.source_local_upper(),
            &[None, None, rank.map(u64::from)]
        );
        assert_eq!(region.source_rank_limit(), rank);
        assert_eq!(region.target_rank_upper_bound(), bound);
        assert_eq!(region.same_support_rank_delta(), Some(1));
        assert_eq!(region.transition(), OwnerSuccessorTransition::SameSupport);
        assert!(region.has_installed_target_owner()); // Membership, not closure.
    }
}

#[test]
fn sign_partition_and_rank_bounds_match_small_original_coordinate_oracle() {
    let family = Arc::new(crate::solver::tests::sunset());
    let rule = formula(&family, [None; 3], &[([-2, 1, -1], 1)]);
    let programs = programs(
        family,
        Some(2),
        vec![input([true, false, false], Some(2), vec![rule], &[])],
        Default::default(),
    );
    let (stats, regions) = collect(&programs, [true, false, false], Some(2));
    assert_eq!((stats.regions, stats.split_operations), (4, 3));
    assert!(
        regions
            .iter()
            .any(|r| r.transition() == OwnerSuccessorTransition::StrictPinch)
    );
    assert!(
        regions
            .iter()
            .any(|r| r.transition() == OwnerSuccessorTransition::UnsupportedSupportChange)
    );
    let mut attained = vec![0_u128; regions.len()];
    for x in 0..=5_u64 {
        for y in 0..=2_u64 {
            for z in 0..=2 - y {
                let local = [x, y, z];
                let matches: Vec<_> = regions
                    .iter()
                    .enumerate()
                    .filter(|(_, region)| {
                        (0..3).all(|axis| {
                            local[axis] >= region.source_local_lower()[axis]
                                && region.source_local_upper()[axis]
                                    .is_none_or(|upper| local[axis] <= upper)
                        })
                    })
                    .collect();
                assert_eq!(matches.len(), 1);
                let (i, region) = matches[0];
                let target = [i128::from(x) - 1, 1 - i128::from(y), -i128::from(z) - 1];
                assert_eq!(*region.target_sector(), target.map(|n| n > 0));
                let target_rank: u128 = target
                    .iter()
                    .filter(|n| **n <= 0)
                    .map(|n| n.unsigned_abs())
                    .sum();
                assert!(target_rank <= region.target_rank_upper_bound().unwrap());
                attained[i] = attained[i].max(target_rank);
            }
        }
    }
    // The bound is sharp for these guard-free boxes. Production retains guards
    // and does not claim its bound is attained on their potentially empty loci.
    for (region, maximum) in regions.iter().zip(attained) {
        assert_eq!(region.target_rank_upper_bound(), Some(maximum));
    }
}

#[test]
fn rank_simplex_is_retained_and_fixed_rank_empty_rules_are_only_prefiltered() {
    let family = Arc::new(crate::solver::tests::sunset());
    let rules = vec![
        formula(&family, [None, Some(-2), Some(-2)], &[([0, 0, 0], 1)]),
        formula(&family, [None; 3], &[([0, 0, 0], 1)]),
    ];
    let programs = programs(
        family,
        Some(3),
        vec![input([true, false, false], Some(3), rules, &[])],
        Default::default(),
    );
    let (stats, regions) = collect(&programs, [true, false, false], Some(3));
    assert_eq!(
        (stats.rules, stats.terms, stats.rank_empty_prefilters),
        (2, 1, 1)
    );
    assert_eq!(regions[0].source_local_upper(), &[None, Some(3), Some(3)]);
    assert_eq!(regions[0].source_rank_limit(), Some(3)); // Not the rectangle x1,x2 <= 3.
    assert_eq!(regions[0].target_rank_upper_bound(), Some(3));
    let (_, above) = collect(&programs, [true, false, false], Some(4));
    assert_eq!(above.len(), 2);
    assert_eq!(above[0].target_rank_upper_bound(), Some(4));
}

#[test]
fn native_coupled_guards_whole_exceptions_and_all_original_denominators_are_borrowed() {
    let family = Arc::new(crate::solver::tests::sunset());
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let source = SourceSystem::<3>::from_family(&family).unwrap();
    let equality = context
        .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap()
        .raw()
        .numerator
        .clone();
    let face = CoordinateCase::new([None, None, Some(1)]).unwrap();
    let AffineIntersection::Affine(affine) =
        AffineCase::from_coordinate(&face, &[equality], source.index_variables(), &[true; 3])
            .unwrap()
    else {
        panic!("affine fixture");
    };
    let mut rule = formula(
        &family,
        [None, None, Some(1)],
        &[([-1, 0, 0], 1), ([0, -1, 0], 1)],
    );
    rule.candidate.case = affine.into();
    rule.candidate.rhs[0].coefficient = context
        .div(&context.one(), &context.index(0).unwrap())
        .unwrap()
        .raw()
        .clone();
    rule.exceptions = ExceptionalConditions {
        branches: vec![vec![
            context.index(0).unwrap().raw().numerator.clone(),
            context.index(1).unwrap().raw().numerator.clone(),
        ]],
    };
    let programs = programs(
        family,
        Some(10),
        vec![input([true; 3], Some(10), vec![rule], &[])],
        Default::default(),
    );
    let prepared = &programs.owners[&[true; 3]].batches[0].rules[0];
    let (_, regions) = collect(&programs, [true; 3], Some(10));
    assert_eq!(regions.len(), 4);
    for region in &regions {
        assert!(std::ptr::eq(
            region.equalities(),
            prepared.equalities.as_slice()
        ));
        assert_eq!(region.equalities().len(), 1);
        assert!(std::ptr::eq(
            region.excluded_zero_conjunctions(),
            prepared.exceptions.as_slice()
        ));
        assert_eq!(region.excluded_zero_conjunctions()[0].len(), 2);
        assert!(std::ptr::eq(
            region.coefficient(),
            &prepared.rhs[region.term_ordinal()].coefficient
        ));
        let denominators: Vec<_> = region.original_denominators().collect();
        assert_eq!(denominators.len(), 2);
        assert!(std::ptr::eq(denominators[0], &prepared.rhs[0].denominator));
        assert!(std::ptr::eq(
            region.source_conditions(),
            programs.context.shared.source_conditions.as_slice()
        ));
    }
}

#[test]
fn all_rule_union_preserves_overlap_and_batch_order_without_local_cancellation() {
    let family = Arc::new(crate::solver::tests::sunset());
    let rules = vec![
        formula(&family, [None; 3], &[([0, 0, 0], 1), ([0, 0, 0], -1)]),
        formula(&family, [None; 3], &[([0, 0, -1], 1)]),
    ];
    let mut programs = programs(
        family,
        Some(10),
        vec![input([true, true, false], Some(10), rules, &[])],
        Default::default(),
    );
    // Synthetic duplicate immutable batch tests visitation order only; real
    // overlays are admitted through the separately tested source-bound path.
    let owner = Arc::get_mut(
        Arc::get_mut(&mut programs)
            .unwrap()
            .owners
            .get_mut(&[true, true, false])
            .unwrap(),
    )
    .unwrap();
    owner.batches.push(owner.batches[0].clone());
    let (_, regions) = collect(&programs, [true, true, false], Some(10));
    let observed: Vec<_> = regions
        .iter()
        .map(|r| (r.batch_ordinal(), r.rule_ordinal(), r.term_ordinal()))
        .collect();
    assert_eq!(
        observed,
        vec![
            (0, 0, 0),
            (0, 0, 1),
            (0, 1, 0),
            (1, 0, 0),
            (1, 0, 1),
            (1, 1, 0)
        ]
    );
    assert_eq!(
        collect(&programs, [true, true, false], Some(10)).0.regions,
        6
    );
}

#[test]
fn only_identically_zero_terms_are_skipped_not_potentially_vanishing_coefficients() {
    let family = Arc::new(crate::solver::tests::sunset());
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut rule = formula(&family, [None; 3], &[([0, 0, 0], 0), ([0, 0, 0], 1)]);
    rule.candidate.rhs[1].coefficient = context.index(2).unwrap().raw().clone();
    let programs = programs(
        family,
        Some(0),
        vec![input([true, true, false], Some(0), vec![rule], &[])],
        Default::default(),
    );
    let (stats, regions) = collect(&programs, [true, true, false], Some(0));
    assert_eq!(
        (stats.terms, stats.exact_zero_terms, stats.regions),
        (2, 1, 1)
    );
    assert_eq!(regions[0].term_ordinal(), 1); // Actually zero on this rank-zero face, not solved here.
}

#[test]
fn cancellation_consumer_stop_and_unknown_owner_never_report_complete() {
    let family = Arc::new(crate::solver::tests::sunset());
    let rule = formula(&family, [None; 3], &[([0, 0, 0], 1), ([0, 0, 0], 1)]);
    let programs = programs(
        family,
        Some(10),
        vec![input([true; 3], Some(10), vec![rule], &[])],
        Default::default(),
    );
    let cancel = AtomicBool::new(true);
    let error = programs
        .visit_owner_rule_successors([true; 3], Some(10), Default::default(), &cancel, |_| {
            panic!("no callback")
        })
        .unwrap_err();
    assert_eq!(error.failure, OwnerSuccessorFailure::Cancelled);
    assert_eq!(error.stats, OwnerSuccessorStats::default());
    cancel.store(false, Ordering::Release);
    let error = programs
        .visit_owner_rule_successors([false; 3], Some(10), Default::default(), &cancel, |_| {
            panic!("no callback")
        })
        .unwrap_err();
    assert_eq!(error.failure, OwnerSuccessorFailure::UnknownOwner);
    let error = programs
        .visit_owner_rule_successors([true; 3], Some(10), Default::default(), &cancel, |_| {
            ControlFlow::Break(())
        })
        .unwrap_err();
    assert_eq!(error.failure, OwnerSuccessorFailure::StoppedByConsumer);
    assert_eq!(error.stats.regions, 1);
    let error = programs
        .visit_owner_rule_successors([true; 3], Some(10), Default::default(), &cancel, |_| {
            cancel.store(true, Ordering::Release);
            ControlFlow::Continue(())
        })
        .unwrap_err();
    assert_eq!(error.failure, OwnerSuccessorFailure::Cancelled);
    assert_eq!(error.stats.regions, 1);
}

#[test]
fn all_work_and_scratch_limits_fail_closed_with_prefix_statistics() {
    let family = Arc::new(crate::solver::tests::sunset());
    let rules = vec![
        formula(&family, [None; 3], &[([-1, 0, 0], 1)]),
        formula(&family, [None; 3], &[([-1, 0, 0], 1)]),
    ];
    let programs = programs(
        family,
        Some(10),
        vec![input([true; 3], Some(10), rules, &[])],
        Default::default(),
    );
    for (limits, callbacks) in [
        (
            OwnerSuccessorLimits {
                max_rules: 1,
                ..Default::default()
            },
            2,
        ),
        (
            OwnerSuccessorLimits {
                max_terms: 1,
                ..Default::default()
            },
            2,
        ),
        (
            OwnerSuccessorLimits {
                max_regions: 1,
                ..Default::default()
            },
            1,
        ),
        (
            OwnerSuccessorLimits {
                max_split_operations: 1,
                ..Default::default()
            },
            2,
        ),
        (
            OwnerSuccessorLimits {
                max_scratch_boxes: 5,
                ..Default::default()
            },
            0,
        ),
        (
            OwnerSuccessorLimits {
                max_scratch_coordinate_cells: 35,
                ..Default::default()
            },
            0,
        ),
    ] {
        let mut delivered = 0;
        let error = programs
            .visit_owner_rule_successors(
                [true; 3],
                Some(10),
                limits,
                &AtomicBool::new(false),
                |_| {
                    delivered += 1;
                    ControlFlow::Continue(())
                },
            )
            .unwrap_err();
        assert!(matches!(
            error.failure,
            OwnerSuccessorFailure::ResourceLimit { .. }
        ));
        assert_eq!(delivered, callbacks);
        assert_eq!(error.stats.regions, callbacks);
    }
}

#[test]
fn authenticated_zero_sector_is_labelled_without_discarding_sign_region() {
    let family = Arc::new(crate::solver::tests::sunset());
    let crate::sector::zero::Decision::ProvedZero(proof) =
        crate::sector::zero::Analyzer::try_unrestricted(&family)
            .unwrap()
            .analyze(&crate::sector::Mask::try_new([false; 3]).unwrap())
            .unwrap()
    else {
        panic!("zero fixture");
    };
    let rule = formula(&family, [Some(1), Some(0), Some(0)], &[([-1, 0, 0], 1)]);
    let context = Arc::new(
        crate::solver::CandidateOwnerContext::try_new(
            family,
            crate::solver::CandidateOwnerScope {
                max_numerator_rank: Some(10),
                finite_case_policy: Default::default(),
            },
            vec![proof],
            Default::default(),
        )
        .unwrap(),
    );
    let programs = CandidateOwnerPrograms::try_new(
        context,
        [input([true, false, false], Some(10), vec![rule], &[])],
    )
    .unwrap();
    let (_, regions) = collect(&programs, [true, false, false], Some(10));
    assert_eq!(regions.len(), 1);
    assert!(regions[0].is_exact_zero_sector());
    assert!(!regions[0].has_installed_target_owner());
    assert_eq!(
        regions[0].transition(),
        OwnerSuccessorTransition::StrictPinch
    );
    assert_eq!(regions[0].target_rank_upper_bound(), Some(0));
}

#[test]
fn minimum_wide_shift_keeps_mathematical_positive_tail_without_machine_endpoint_clipping() {
    let family = Arc::new(crate::solver::tests::sunset());
    let rule = formula(&family, [None; 3], &[([0, 0, 0], 1)]);
    let mut programs = programs(
        family,
        Some(0),
        vec![input([true; 3], Some(0), vec![rule], &[])],
        Default::default(),
    );
    // Adversarial private wide shift: ordinary compact source formulas currently
    // produce much smaller shifts. No IntegralKey conversion is part of a scan.
    let owner = Arc::get_mut(
        Arc::get_mut(&mut programs)
            .unwrap()
            .owners
            .get_mut(&[true; 3])
            .unwrap(),
    )
    .unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0].rhs[0].shift[0] = i64::MIN;
    let (_, regions) = collect(&programs, [true; 3], Some(0));
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].source_local_upper()[0], Some((1_u64 << 63) - 1));
    assert_eq!(
        regions[0].target_rank_upper_bound(),
        Some((1_u128 << 63) - 1)
    );
    assert_eq!(
        regions[0].transition(),
        OwnerSuccessorTransition::StrictPinch
    );
    assert_eq!(regions[1].source_local_lower()[0], 1_u64 << 63);
    assert_eq!(regions[1].source_local_upper()[0], None);
    assert_eq!(regions[1].target_rank_upper_bound(), Some(0));
}
