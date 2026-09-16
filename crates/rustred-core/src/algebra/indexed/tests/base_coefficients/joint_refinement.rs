//! Joint coefficient equations may disprove a separable root hyperplane even
//! though no individual coefficient is uniformly nonzero on the full box.

use super::*;
use crate::algebra::IndexedCoefficient;
use crate::algebra::indexed::BaseCoefficientSystem;

fn context(arity: usize) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "joint-root-hyperplane-refinement",
        arity,
    )
    .unwrap()
}

/// Native assembly of c0 + d*c1 + d^2*c2 + ... keeps test coefficient
/// ordering explicit without relying on how a factorization orders roots.
fn system(
    context: &IndexedCoefficientContext,
    coefficients: &[IndexedCoefficient],
) -> BaseCoefficientSystem {
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let mut power = context.one();
    let mut value = context.zero();
    for coefficient in coefficients {
        value = context
            .add(&value, &context.mul(&power, coefficient).unwrap())
            .unwrap();
        power = context.mul(&power, &d).unwrap();
    }
    let polynomial = context
        .numerator_condition_with_limits(&value, Default::default())
        .unwrap();
    context
        .base_coefficient_system(&polynomial, Default::default(), Default::default())
        .unwrap()
}

fn roots(resolution: &IntegerZeroLocusDomainResolution) -> Vec<(usize, Integer)> {
    match resolution {
        IntegerZeroLocusDomainResolution::IntersectsExactHyperplanes(roots)
        | IntegerZeroLocusDomainResolution::IntersectsConservativeCover(roots) => roots
            .iter()
            .map(|root| (root.index_position(), root.root().clone()))
            .collect(),
        other => panic!("expected a retained root cover, found {other:?}"),
    }
}

#[test]
fn actual_four_loop_guard_has_no_joint_coefficient_zero() {
    let context = context(10);
    let a = context.index(0).unwrap();
    let b = context.index(8).unwrap();
    let b_plus_three = context.add(&b, &context.integer(3)).unwrap();
    // Q = (b+3)*(d-a) + 2*b^2 + 5*b + 1.  The d coefficient
    // forces b=-3, but the constant coefficient then equals four for all a.
    let constant = context
        .sub(
            &context
                .add(
                    &context
                        .add(
                            &context
                                .mul(&context.integer(2), &context.mul(&b, &b).unwrap())
                                .unwrap(),
                            &context.mul(&context.integer(5), &b).unwrap(),
                        )
                        .unwrap(),
                    &context.one(),
                )
                .unwrap(),
            &context.mul(&a, &b_plus_three).unwrap(),
        )
        .unwrap();
    let system = system(&context, &[constant, b_plus_three]);
    assert!(!system.has_nonzero_constant_equation());
    assert_eq!(
        context
            .integer_zero_locus_domain_resolution(&system, Default::default(), |axis, root| {
                assert_eq!(axis, 8);
                root <= &Integer::zero()
            })
            .unwrap(),
        IntegerZeroLocusDomainResolution::MissesDomain
    );
    // This is also a proof on all integer indices, not just this vacuum box.
    assert!(
        context
            .integer_zero_locus_misses_domain(&system, Default::default(), |_, _| true)
            .unwrap()
    );
}

#[test]
fn impossible_hyperplane_is_removed_without_losing_exact_sibling() {
    let context = context(2);
    let a = context.index(0).unwrap();
    let b = context.index(1).unwrap();
    let b_minus_one = context.sub(&b, &context.one()).unwrap();
    // b*(b-1)=0 AND (b-1)*(b*a+1)=0: b=0 is impossible,
    // while the complete hyperplane b=1 survives exactly.
    let system = system(
        &context,
        &[
            context.mul(&b, &b_minus_one).unwrap(),
            context
                .mul(
                    &b_minus_one,
                    &context
                        .add(&context.mul(&b, &a).unwrap(), &context.one())
                        .unwrap(),
                )
                .unwrap(),
        ],
    );
    let result = context
        .integer_zero_locus_domain_resolution(&system, Default::default(), |_, _| true)
        .unwrap();
    assert!(matches!(
        result,
        IntegerZeroLocusDomainResolution::IntersectsExactHyperplanes(_)
    ));
    assert_eq!(roots(&result), [(1, Integer::one())]);
}

#[test]
fn impossible_hyperplane_does_not_erase_a_coupled_sibling() {
    let context = context(2);
    let a = context.index(0).unwrap();
    let b = context.index(1).unwrap();
    let b_minus_one = context.sub(&b, &context.one()).unwrap();
    // b*(b-1)=0 AND b*a+1-b=0: b=0 is impossible; b=1
    // leaves a=0 and must remain conservative rather than exact or empty.
    let system = system(
        &context,
        &[
            context.mul(&b, &b_minus_one).unwrap(),
            context
                .sub(&context.mul(&b, &a).unwrap(), &b_minus_one)
                .unwrap(),
        ],
    );
    let result = context
        .integer_zero_locus_domain_resolution(&system, Default::default(), |_, _| true)
        .unwrap();
    assert!(matches!(
        result,
        IntegerZeroLocusDomainResolution::IntersectsConservativeCover(_)
    ));
    assert_eq!(roots(&result), [(1, Integer::one())]);
}

#[test]
fn mixed_exact_and_coupled_survivors_keep_the_complete_union() {
    let context = context(2);
    let a = context.index(0).unwrap();
    let b = context.index(1).unwrap();
    let b_minus_one = context.sub(&b, &context.one()).unwrap();
    let b_minus_two = context.sub(&b, &context.integer(2)).unwrap();
    // Roots b=0,1,2: the second equation disproves 0, vanishes on
    // all of 1, and retains a=0 on 2. A mixed union is conservative.
    let system = system(
        &context,
        &[
            context
                .mul(&context.mul(&b, &b_minus_one).unwrap(), &b_minus_two)
                .unwrap(),
            context
                .mul(
                    &b_minus_one,
                    &context
                        .sub(&context.mul(&b, &a).unwrap(), &b_minus_two)
                        .unwrap(),
                )
                .unwrap(),
        ],
    );
    let result = context
        .integer_zero_locus_domain_resolution(&system, Default::default(), |_, _| true)
        .unwrap();
    assert!(matches!(
        result,
        IntegerZeroLocusDomainResolution::IntersectsConservativeCover(_)
    ));
    assert_eq!(roots(&result), [(1, Integer::one()), (1, Integer::from(2))]);
}

#[test]
fn a_nonconstant_restriction_does_not_skip_a_later_constant_contradiction() {
    let context = context(2);
    let a = context.index(0).unwrap();
    let b = context.index(1).unwrap();
    let b_minus_one = context.sub(&b, &context.one()).unwrap();
    let system = system(
        &context,
        &[
            context.mul(&b, &b_minus_one).unwrap(),
            context
                .sub(&context.mul(&b, &a).unwrap(), &b_minus_one)
                .unwrap(),
            context
                .add(&context.mul(&b_minus_one, &a).unwrap(), &context.one())
                .unwrap(),
        ],
    );
    // On b=1 the first two restrictions are 0 and a, so replay must
    // continue to the third coefficient, the contradictory constant 1.
    // Both later equations are irreducibly coupled, so neither can supply
    // an alternative separable-root proof that masks a short-circuited scan.
    assert_eq!(
        context
            .integer_zero_locus_domain_resolution(&system, Default::default(), |_, _| true)
            .unwrap(),
        IntegerZeroLocusDomainResolution::MissesDomain
    );
}

#[test]
fn nonzero_nonconstant_restrictions_never_disprove_a_hyperplane() {
    let context = context(2);
    let system = system(
        &context,
        &[context.index(0).unwrap(), context.index(1).unwrap()],
    );
    // The origin satisfies both equations. Neither coordinate hyperplane
    // is exact, but both are conservative covers and neither is empty.
    let result = context
        .integer_zero_locus_domain_resolution(&system, Default::default(), |_, _| true)
        .unwrap();
    assert!(matches!(
        result,
        IntegerZeroLocusDomainResolution::IntersectsConservativeCover(_)
    ));
    assert_eq!(roots(&result), [(0, Integer::zero())]);
}

#[test]
fn joint_refinement_retains_integer_roots_beyond_machine_ranges() {
    let context = context(2);
    let a = context.index(0).unwrap();
    let b = context.index(1).unwrap();
    for sign in [-1, 1] {
        let large = context
            .mul(&context.integer(i64::MAX), &context.integer(2 * sign))
            .unwrap();
        let shifted_b = context.sub(&b, &large).unwrap();
        let shifted_b_minus_one = context.sub(&shifted_b, &context.one()).unwrap();
        let system = system(
            &context,
            &[
                context.mul(&shifted_b, &shifted_b_minus_one).unwrap(),
                context
                    .mul(
                        &shifted_b_minus_one,
                        &context
                            .add(&context.mul(&shifted_b, &a).unwrap(), &context.one())
                            .unwrap(),
                    )
                    .unwrap(),
            ],
        );
        let result = context
            .integer_zero_locus_domain_resolution(&system, Default::default(), |_, _| true)
            .unwrap();
        assert!(matches!(
            result,
            IntegerZeroLocusDomainResolution::IntersectsExactHyperplanes(_)
        ));
        let expected = Integer::from(i64::MAX) * Integer::from(2 * sign) + Integer::one();
        assert!(expected.to_i64().is_none());
        assert_eq!(roots(&result), [(1, expected)]);
    }
}

#[test]
fn joint_refinement_obeys_full_replay_precharge_and_context_authentication() {
    let context = context(2);
    let a = context.index(0).unwrap();
    let b = context.index(1).unwrap();
    let b_minus_one = context.sub(&b, &context.one()).unwrap();
    let system = system(
        &context,
        &[
            context.mul(&b, &b_minus_one).unwrap(),
            context
                .mul(
                    &b_minus_one,
                    &context
                        .add(&context.mul(&b, &a).unwrap(), &context.one())
                        .unwrap(),
                )
                .unwrap(),
        ],
    );
    // Despite one impossible root, the existing complete two-root by
    // two-equation precharge must occur before native substitution.
    for (limits, resource, requested, limit) in [
        (
            IndexedGuardLimits {
                max_exact_hyperplane_replay_substitutions: 3,
                ..Default::default()
            },
            "guard exact-hyperplane replay substitutions",
            4,
            3,
        ),
        (
            IndexedGuardLimits {
                max_exact_hyperplane_replay_terms: 11,
                ..Default::default()
            },
            "guard exact-hyperplane replay terms",
            12,
            11,
        ),
    ] {
        assert_eq!(
            context.integer_zero_locus_domain_resolution(&system, limits, |_, _| true),
            Err(IndexedAlgebraError::ResourceLimit {
                resource,
                requested,
                limit,
            })
        );
    }
    assert!(matches!(
        context.integer_zero_locus_domain_resolution(
            &system,
            IndexedGuardLimits {
                max_exact_hyperplane_replay_work: 0,
                ..Default::default()
            },
            |_, _| true,
        ),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard exact-hyperplane replay work",
            requested,
            limit: 0,
        }) if requested > 0
    ));
    let foreign = IndexedCoefficientContext::try_new(context.base(), "different-scope", 2).unwrap();
    assert_eq!(
        foreign.integer_zero_locus_domain_resolution(&system, Default::default(), |_, _| true),
        Err(IndexedAlgebraError::WrongContext)
    );
}
