use std::cmp::Ordering;
use std::collections::BTreeSet;

use rustred::{Coefficient, CoefficientContext};
use rustred_legacy_oracles::families::{
    equal_mass_two_loop_vacuum, equal_mass_two_loop_vacuum_reversed,
};
use rustred_legacy_oracles::{Denominator, Integral, LinearCombination, VacuumFamily};
use rustred_legacy_oracles::{
    TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND, TWO_LOOP_TOP_DOT_IBP_WEIGHTS,
    TWO_LOOP_TOP_DOT_RAW_TERM_BOUND, TwoLoopReductionConfig, TwoLoopReductionPipeline,
    TwoLoopTopDotConfig, TwoLoopTopDotError, TwoLoopTopDotReducer,
};

const PERMUTATIONS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

fn permute(powers: [i32; 3], permutation: [usize; 3]) -> Integral {
    Integral::from([
        powers[permutation[0]],
        powers[permutation[1]],
        powers[permutation[2]],
    ])
}

fn assert_master_coefficient(
    reducer: &TwoLoopTopDotReducer,
    reduction: &LinearCombination,
    master: &Integral,
    expected: &str,
) {
    let zero = reducer.family().coefficients().zero();
    assert_eq!(
        reduction.coefficient(master).unwrap_or(&zero),
        &reducer.family().coefficients().parse(expected).unwrap()
    );
}

fn check_asymmetric_native_provenance(reducer: &TwoLoopTopDotReducer) {
    assert_eq!(TWO_LOOP_TOP_DOT_IBP_WEIGHTS, [[1, -1], [0, 0]]);
    assert_eq!(TWO_LOOP_TOP_DOT_RAW_TERM_BOUND, 15);
    assert_eq!(TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND, 6);

    for target in [
        Integral::from([4, 3, 2]),
        Integral::from([5, 2, 1]),
        Integral::from([3, 3, 2]),
    ] {
        reducer.validate_raw_ibp_provenance(&target).unwrap();
        assert_eq!(
            reducer.raw_ibp(&target).unwrap(),
            reducer.expected_raw_ibp(&target).unwrap()
        );
    }

    // Keep the exact uncanonicalized expansion at one asymmetric point.  It
    // prevents a permutation from being hidden by S3 collection.
    let context = reducer.family().coefficients();
    let mut expected = LinearCombination::new();
    for (powers, coefficient) in [
        ([4, 3, 2], "9*m2"),
        ([3, 3, 2], "d-9"),
        ([2, 3, 3], "-4"),
        ([3, 2, 3], "4"),
        ([4, 3, 1], "3"),
        ([4, 2, 2], "-3"),
    ] {
        expected.add_term(Integral::from(powers), context.parse(coefficient).unwrap());
    }
    assert_eq!(
        reducer.raw_ibp(&Integral::from([4, 3, 2])).unwrap(),
        expected
    );

    let rewrite = reducer
        .rewrite_once(&Integral::from([2, 4, 3]))
        .unwrap()
        .unwrap();
    assert_eq!(rewrite.target(), &Integral::from([4, 3, 2]));
    assert_eq!(rewrite.seed(), &Integral::from([3, 3, 2]));
    assert_eq!(rewrite.provenance().seed_lowered_position(), 0);
    assert_eq!(
        rewrite.provenance().raw_ibp_weights(),
        TWO_LOOP_TOP_DOT_IBP_WEIGHTS
    );
}

fn check_exhaustive_positive_box_and_descent(reducer: &TwoLoopTopDotReducer) {
    let family = reducer.family();
    let mut canonical_targets = BTreeSet::new();
    for a in 1..=4 {
        for b in 1..=4 {
            for c in 1..=4 {
                let powers = [a, b, c];
                let reference = reducer.reduce_integral(&Integral::from(powers)).unwrap();
                assert!(reference.terms().keys().all(|master| {
                    master == reducer.sunset_master() || master == reducer.product_master()
                }));
                for permutation in PERMUTATIONS {
                    assert_eq!(
                        reducer
                            .reduce_integral(&permute(powers, permutation))
                            .unwrap(),
                        reference,
                        "normal form is not S3 invariant for {powers:?}"
                    );
                }

                let canonical = family.canonicalize(&Integral::from(powers)).unwrap();
                canonical_targets.insert(canonical.clone());
                let Some(rewrite) = reducer.rewrite_once(&canonical).unwrap() else {
                    assert_eq!(canonical, Integral::from([1, 1, 1]));
                    continue;
                };
                for output in rewrite.rhs().terms().keys() {
                    assert_eq!(
                        family.compare_integrals(output, rewrite.target()),
                        Ordering::Less,
                        "non-descending branch in {rewrite:?}"
                    );
                    if output.denominator_count() == 3 {
                        assert_eq!(
                            output.dot_degree() + 1,
                            rewrite.target().dot_degree(),
                            "positive branch did not lower dot degree in {rewrite:?}"
                        );
                    }
                }
            }
        }
    }

    // The production rewrite is the proved closed formula and deliberately
    // does not regenerate native rows on every call.  Replay every distinct
    // target in this small box through the independent raw-IBP certificate.
    for target in canonical_targets {
        if &target != reducer.sunset_master() {
            reducer.validate_raw_ibp_provenance(&target).unwrap();
        }
    }

    // A pinch may preserve the numeric dot count; the active-sector drop is
    // the part of the certified ordering which proves descent in this case.
    let pinching = reducer
        .rewrite_once(&Integral::from([3, 2, 1]))
        .unwrap()
        .unwrap();
    assert!(pinching.rhs().terms().keys().any(|output| {
        output.denominator_count() == 2
            && output.dot_degree() == pinching.target().dot_degree()
            && family.compare_integrals(output, pinching.target()) == Ordering::Less
    }));
}

fn check_eager_normal_forms_and_pipeline_agreement(reducer: &TwoLoopTopDotReducer) {
    for (powers, sunset, product) in [
        ([1, 1, 1], "1", "0"),
        ([2, 1, 1], "(3-d)/(3*m2)", "0"),
        ([2, 2, 1], "(d-2)*(d-3)/(9*m2^2)", "(d-2)^2/(12*m2^3)"),
        ([3, 1, 1], "(d-8)*(d-3)/(18*m2^2)", "-(d-2)^2/(12*m2^3)"),
        ([-2, 1, 1], "0", "m2^2*(d+4)/d"),
    ] {
        let reduction = reducer.reduce_integral(&Integral::from(powers)).unwrap();
        assert_master_coefficient(reducer, &reduction, reducer.sunset_master(), sunset);
        assert_master_coefficient(reducer, &reduction, reducer.product_master(), product);
    }
    assert!(
        reducer
            .reduce_integral(&Integral::from([-100, 0, 9]))
            .unwrap()
            .is_zero()
    );

    let normal = reducer
        .reduce_integral_with_stats(&Integral::from([4, 3, 2]))
        .unwrap();
    assert!(normal.stats().states() > 1);
    assert!(normal.stats().recurrence_steps() > 0);
    assert!(normal.stats().boundary_calls() > 0);
    assert!(normal.stats().coefficient_operations() > 0);

    let finite = TwoLoopReductionPipeline::build(TwoLoopReductionConfig::default()).unwrap();
    for a in -2..=4 {
        for b in -2..=4 {
            for c in -2..=4 {
                let integral = Integral::from([a, b, c]);
                assert_eq!(
                    reducer.reduce_integral(&integral).unwrap(),
                    finite.reduce_integral(&integral).unwrap(),
                    "all-dot and finite normal forms disagree for {integral}"
                );
            }
        }
    }
}

fn check_guards_and_resource_failures() {
    let reducer = TwoLoopTopDotReducer::build(TwoLoopTopDotConfig::default()).unwrap();
    let corner = Integral::from([1, 1, 1]);
    assert_eq!(reducer.rewrite_once(&corner).unwrap(), None);
    assert!(matches!(
        reducer.validate_raw_ibp_provenance(&corner),
        Err(TwoLoopTopDotError::PivotGuardNotSatisfied { first_power: 1, .. })
    ));
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([2, 1, 0])),
        Err(TwoLoopTopDotError::OutsideScalarTopSector {
            position: 2,
            power: 0,
            ..
        })
    ));
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([2, 1])),
        Err(TwoLoopTopDotError::WrongIntegralArity {
            expected: 3,
            actual: 2,
        })
    ));

    let formula_capped = TwoLoopTopDotReducer::build(TwoLoopTopDotConfig {
        max_explicit_terms: 5,
        ..TwoLoopTopDotConfig::default()
    })
    .unwrap();
    assert!(matches!(
        formula_capped.rewrite_once(&Integral::from([2, 1, 1])),
        Err(TwoLoopTopDotError::ResourceLimit {
            resource: "explicit recurrence terms",
            requested: 6,
            limit: 5,
        })
    ));

    let raw_capped = TwoLoopTopDotReducer::build(TwoLoopTopDotConfig {
        max_raw_terms: 14,
        ..TwoLoopTopDotConfig::default()
    })
    .unwrap();
    assert!(matches!(
        raw_capped.raw_ibp(&Integral::from([4, 3, 2])),
        Err(TwoLoopTopDotError::ResourceLimit {
            resource: "native raw derivative terms",
            requested: 15,
            limit: 14,
        })
    ));

    let state_capped = TwoLoopTopDotReducer::build(TwoLoopTopDotConfig {
        max_states: 12,
        ..TwoLoopTopDotConfig::default()
    })
    .unwrap();
    assert!(matches!(
        state_capped.reduce_integral(&Integral::from([2, 1, 1])),
        Err(TwoLoopTopDotError::ResourceLimit {
            resource: "normal-form state upper bound",
            requested: 13,
            limit: 12,
        })
    ));

    let operation_capped = TwoLoopTopDotReducer::build(TwoLoopTopDotConfig {
        max_coefficient_operations: 79,
        ..TwoLoopTopDotConfig::default()
    })
    .unwrap();
    assert!(matches!(
        operation_capped.reduce_integral(&Integral::from([2, 1, 1])),
        Err(TwoLoopTopDotError::ResourceLimit {
            resource: "normal-form coefficient-operation upper bound",
            requested: 80,
            limit: 79,
        })
    ));

    let coefficient_capped = TwoLoopTopDotReducer::build(TwoLoopTopDotConfig {
        max_coefficient_degree: 1,
        ..TwoLoopTopDotConfig::default()
    })
    .unwrap();
    assert!(matches!(
        coefficient_capped.reduce_integral(&Integral::from([-2, 1, 1])),
        Err(TwoLoopTopDotError::CoefficientDegreeLimit {
            requested: 2,
            limit: 1,
        })
    ));

    let boundary_capped = TwoLoopTopDotReducer::build(TwoLoopTopDotConfig {
        max_boundary_formula_iterations: 2,
        ..TwoLoopTopDotConfig::default()
    })
    .unwrap();
    assert!(matches!(
        boundary_capped.reduce_integral(&Integral::from([0, 1, 1])),
        Err(TwoLoopTopDotError::ResourceLimit {
            resource: "boundary formula iteration estimate",
            requested: 3,
            limit: 2,
        })
    ));

    // The whole-request combinatorial preflight rejects extreme indices
    // before allocating a state graph or constructing symbolic coefficients.
    assert!(matches!(
        reducer.reduce_integral(&Integral::from([i32::MAX, i32::MAX, 1])),
        Err(TwoLoopTopDotError::ResourceLimit {
            resource: "normal-form state upper bound",
            ..
        })
    ));
    // The one-step surface separately checks every +1/-2 pattern before row
    // or coefficient construction.
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([i32::MAX; 3])),
        Err(TwoLoopTopDotError::ExponentOverflow { .. })
    ));

    assert!(matches!(
        TwoLoopTopDotReducer::new(
            equal_mass_two_loop_vacuum_reversed().unwrap(),
            TwoLoopTopDotConfig::default(),
        ),
        Err(TwoLoopTopDotError::WrongPropagatorSign { .. })
    ));

    let coefficients = CoefficientContext::new(["d", "m2", "mu2"]);
    let mass = coefficients.parameter("m2").unwrap();
    let other_mass = coefficients.parameter("mu2").unwrap();
    let make_family = |name: &str,
                       routings: [[i64; 2]; 3],
                       masses: [Coefficient; 3],
                       symmetries: Vec<Vec<usize>>| {
        VacuumFamily::new(
            name,
            2,
            coefficients.clone(),
            "d",
            routings
                .into_iter()
                .zip(masses)
                .map(|(routing, shift)| {
                    Denominator::propagator(routing.into_iter().map(Into::into).collect(), shift)
                })
                .collect(),
            symmetries,
        )
        .unwrap()
    };

    let wrong_route = make_family(
        "wrong_route",
        [[1, 0], [0, 1], [1, -1]],
        [mass.clone(), mass.clone(), mass.clone()],
        vec![],
    );
    assert!(matches!(
        TwoLoopTopDotReducer::new(wrong_route, TwoLoopTopDotConfig::default()),
        Err(TwoLoopTopDotError::WrongMomentumRouting)
    ));

    let unequal_mass = make_family(
        "unequal_mass",
        [[1, 0], [0, 1], [1, 1]],
        [mass.clone(), other_mass, mass.clone()],
        vec![],
    );
    assert!(matches!(
        TwoLoopTopDotReducer::new(unequal_mass, TwoLoopTopDotConfig::default()),
        Err(TwoLoopTopDotError::UnequalMasses)
    ));

    let incomplete_symmetry = make_family(
        "incomplete_symmetry",
        [[1, 0], [0, 1], [1, 1]],
        [mass.clone(), mass.clone(), mass],
        vec![],
    );
    assert!(matches!(
        TwoLoopTopDotReducer::new(incomplete_symmetry, TwoLoopTopDotConfig::default()),
        Err(TwoLoopTopDotError::IncompleteSymmetry { actual: 1 })
    ));
}

// Symbolica's restricted runtime must remain on one test worker.  Keep the
// native provenance, exhaustive recurrence, finite-pipeline comparison, and
// resource checks in one exact integration test.
#[test]
fn complete_two_loop_all_dot_normal_form() {
    let reducer = TwoLoopTopDotReducer::build(TwoLoopTopDotConfig::default()).unwrap();
    assert_eq!(reducer.sunset_master(), &Integral::from([1, 1, 1]));
    assert_eq!(reducer.product_master(), &Integral::from([0, 1, 1]));

    check_asymmetric_native_provenance(&reducer);
    check_exhaustive_positive_box_and_descent(&reducer);
    check_eager_normal_forms_and_pipeline_agreement(&reducer);
    check_guards_and_resource_failures();

    // The builder and direct constructor authenticate the same family.
    let family = equal_mass_two_loop_vacuum().unwrap();
    TwoLoopTopDotReducer::new(family, TwoLoopTopDotConfig::default()).unwrap();
}
