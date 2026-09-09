use crate::algebra::{CoefficientContext, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, StratumRegistryLimits,
};
use crate::sector::{InteriorBounds, Mask, SectorMonotoneDomain};

use super::materialize::try_materialize_guards_for_test;
use super::{
    SpiredCoordinateGuardCaseError, SpiredCoordinateGuardCaseIncomplete,
    SpiredCoordinateGuardCaseLimits, SpiredCoordinateGuardCaseOutcome,
    SpiredCoordinateGuardCaseRejection,
};
use crate::foundry::completion::spired::{
    SpiredCoordinateCaseObligation, SpiredCoordinateCaseWorklistError,
};

fn context(scope: &str, arity: usize) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::new(Vec::<String>::new()), scope, arity)
        .unwrap()
}

fn polynomial(
    context: &IndexedCoefficientContext,
    value: crate::algebra::IndexedCoefficient,
) -> IndexedPolynomial {
    context
        .numerator_condition_with_limits(&value, Default::default())
        .unwrap()
}

fn parent_stratum(
    context: &IndexedCoefficientContext,
    active: impl IntoIterator<Item = bool>,
    bounds: impl IntoIterator<Item = InteriorBounds>,
) -> SpiredCoordinateCaseObligation {
    let sector = Mask::try_new(active).unwrap();
    let zero = vec![0_i64; sector.arity()];
    let domain =
        SectorMonotoneDomain::try_new_for_rule(sector, bounds, &zero, &[] as &[&[i64]]).unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        "spired-guard-case-test-family",
        context.fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    SpiredCoordinateCaseObligation::try_new_root(stratum).unwrap()
}

fn exact_cases(
    context: &IndexedCoefficientContext,
    guards: &[IndexedPolynomial],
    parent: &SpiredCoordinateCaseObligation,
) -> super::SpiredCoordinateGuardCases {
    let outcome = try_materialize_guards_for_test(
        context,
        guards,
        parent,
        SpiredCoordinateGuardCaseLimits::default(),
    )
    .unwrap();
    let SpiredCoordinateGuardCaseOutcome::Exact(cases) = outcome else {
        panic!("the fixture must have exact coordinate zero loci")
    };
    cases
}

#[test]
fn k1_endpoint_guard_materializes_one_guard_blind_singleton_case() {
    let context = context("spired-guard-k1-endpoint", 1);
    let n = context.index(0).unwrap();
    let guard = polynomial(&context, context.sub(&n, &context.one()).unwrap());
    let parent = parent_stratum(&context, [true], [InteriorBounds::new(1, 12)]);

    let cases = exact_cases(&context, &[guard], &parent);
    assert_eq!(cases.cases().len(), 1);
    assert_eq!(
        cases.cases()[0].stratum().domain().bounds(),
        [InteriorBounds::new(1, 1)]
    );
    assert!(cases.cases()[0].stratum().guards().is_empty());
    assert_eq!(cases.census().exact_hyperplanes(), 1);
    assert_eq!(cases.census().output_cases(), 1);
}

#[test]
fn independent_guard_zero_cases_overlap_at_their_intersection() {
    let context = context("spired-guard-overlap", 2);
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let g1 = polynomial(&context, context.sub(&n0, &context.integer(2)).unwrap());
    let g2 = polynomial(&context, context.sub(&n1, &context.integer(3)).unwrap());
    let parent = parent_stratum(
        &context,
        [true, true],
        [InteriorBounds::new(1, 5), InteriorBounds::new(1, 5)],
    );

    let cases = exact_cases(&context, &[g1, g2], &parent);
    assert_eq!(cases.cases().len(), 2);
    assert_eq!(
        cases
            .cases()
            .iter()
            .map(|case| case.stratum().domain().bounds().to_vec())
            .collect::<Vec<_>>(),
        [
            vec![InteriorBounds::new(1, 5), InteriorBounds::new(3, 3)],
            vec![InteriorBounds::new(2, 2), InteriorBounds::new(1, 5)],
        ]
    );
    assert_eq!(
        cases
            .cases()
            .iter()
            .filter(|case| case.stratum().domain().contains(&[2, 3]).unwrap())
            .count(),
        2
    );
    assert!(
        cases
            .cases()
            .iter()
            .all(|case| case.stratum().guards().is_empty())
    );
}

#[test]
fn equal_zero_hyperplanes_are_canonically_deduplicated() {
    let context = context("spired-guard-dedup", 1);
    let n = context.index(0).unwrap();
    let guard = polynomial(&context, context.sub(&n, &context.integer(2)).unwrap());
    let parent = parent_stratum(&context, [true], [InteriorBounds::new(1, 5)]);

    let cases = exact_cases(&context, &[guard.clone(), guard], &parent);
    assert_eq!(cases.cases().len(), 1);
    assert_eq!(cases.census().required_predicates(), 2);
    assert_eq!(cases.census().exact_duplicate_cases(), 1);
}

#[test]
fn guard_implied_zero_rejects_only_the_candidate() {
    let context = context("spired-guard-implied-zero", 1);
    let n = context.index(0).unwrap();
    let guard = polynomial(&context, context.sub(&n, &context.integer(2)).unwrap());
    let parent = parent_stratum(&context, [true], [InteriorBounds::new(2, 2)]);

    assert_eq!(
        try_materialize_guards_for_test(
            &context,
            &[guard],
            &parent,
            SpiredCoordinateGuardCaseLimits::default(),
        )
        .unwrap(),
        SpiredCoordinateGuardCaseOutcome::CandidateUnusable(
            SpiredCoordinateGuardCaseRejection::GuardIdenticallyZero {
                required_predicate_ordinal: 0,
            }
        )
    );
}

#[test]
fn guard_bearing_parent_is_rejected_before_discovery_materialization() {
    let context = context("spired-guard-parent-branch", 1);
    let parent = parent_stratum(&context, [true], [InteriorBounds::new(1, 5)]);
    let branch = GuardBranchIdentity::try_new(
        "owner-only-branch",
        GuardBranch::NonZero,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let guarded = DecoratedStratum::try_new(
        parent.stratum().family_fingerprint(),
        parent.stratum().context_fingerprint(),
        parent.stratum().domain().clone(),
        [branch],
        StratumRegistryLimits::default(),
    )
    .unwrap();

    assert_eq!(
        SpiredCoordinateCaseObligation::try_new_root(guarded).unwrap_err(),
        SpiredCoordinateCaseWorklistError::GuardedDiscoveryCase { guard_branches: 1 }
    );
}

#[test]
fn coupled_or_non_affine_guard_fails_closed_without_partial_cases() {
    let context = context("spired-guard-coupled", 2);
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let guard = polynomial(
        &context,
        context
            .add(&context.mul(&n0, &n1).unwrap(), &context.one())
            .unwrap(),
    );
    let parent = parent_stratum(
        &context,
        [true, true],
        [InteriorBounds::new(1, 5), InteriorBounds::new(1, 5)],
    );

    assert_eq!(
        try_materialize_guards_for_test(
            &context,
            &[guard],
            &parent,
            SpiredCoordinateGuardCaseLimits::default(),
        )
        .unwrap(),
        SpiredCoordinateGuardCaseOutcome::Incomplete(
            SpiredCoordinateGuardCaseIncomplete::UnsupportedCoupledOrNonAffineGeometry {
                required_predicate_ordinal: 0,
            }
        )
    );
}

#[test]
fn output_limit_returns_atomic_incomplete_instead_of_a_partial_prefix() {
    let context = context("spired-guard-output-limit", 2);
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let guards = [
        polynomial(&context, context.sub(&n0, &context.integer(2)).unwrap()),
        polynomial(&context, context.sub(&n1, &context.integer(3)).unwrap()),
    ];
    let parent = parent_stratum(
        &context,
        [true, true],
        [InteriorBounds::new(1, 5), InteriorBounds::new(1, 5)],
    );
    let limits = SpiredCoordinateGuardCaseLimits {
        max_output_cases: 1,
        ..Default::default()
    };

    assert_eq!(
        try_materialize_guards_for_test(&context, &guards, &parent, limits).unwrap(),
        SpiredCoordinateGuardCaseOutcome::Incomplete(
            SpiredCoordinateGuardCaseIncomplete::ResourceLimit {
                resource: "SpIReD guard-case output cases",
                requested: 2,
                limit: 1,
            }
        )
    );
}
