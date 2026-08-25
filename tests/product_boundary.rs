#![cfg(feature = "legacy-authored-oracles")]

use rustred::families::{equal_mass_two_loop_vacuum, equal_mass_two_loop_vacuum_reversed};
use rustred::{
    CoefficientContext, Denominator, ExactRational, Integral, ProductBoundaryConfig,
    ProductBoundaryError, ProductBoundaryReducer, VacuumFamily, equal_mass_five_loop_banana,
    equal_mass_four_loop_h,
};

fn two_line_family(
    name: &str,
    first_routing: [i64; 2],
    second_routing: [i64; 2],
    first_mass: i64,
    second_mass: i64,
    reverse_second: bool,
) -> VacuumFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients.parameter("m2").unwrap();
    let route = |routing: [i64; 2]| routing.into_iter().map(ExactRational::from).collect();
    let first = Denominator::propagator(
        route(first_routing),
        coefficients.scale_rational(&mass, ExactRational::from(first_mass)),
    );
    let second_mass = coefficients.scale_rational(&mass, ExactRational::from(second_mass));
    let second = if reverse_second {
        Denominator::reversed_propagator(route(second_routing), second_mass)
    } else {
        Denominator::propagator(route(second_routing), second_mass)
    };
    VacuumFamily::new(
        name,
        2,
        coefficients.clone(),
        "d",
        vec![
            first,
            second,
            Denominator::auxiliary(
                vec![
                    ExactRational::zero(),
                    ExactRational::one(),
                    ExactRational::zero(),
                ],
                coefficients.zero(),
            ),
        ],
        Vec::new(),
    )
    .unwrap()
}

// Keep all Symbolica-backed product checks on one restricted worker.
#[test]
fn exact_unimodular_product_boundaries() {
    let five = ProductBoundaryReducer::new(
        equal_mass_five_loop_banana().unwrap(),
        ProductBoundaryConfig::default(),
    )
    .unwrap();
    assert_eq!(
        five.product_master(),
        &Integral::from([1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    );
    let dotted = Integral::from([2, 3, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let reduction = five.reduce_integral(&dotted).unwrap();
    assert_eq!(
        reduction.coefficient(five.product_master()),
        Some(
            &five
                .family()
                .coefficients()
                .parse("(2-d)^2*(4-d)/(16*m2^3)")
                .unwrap()
        )
    );
    for missing in 0..6 {
        let mut powers = vec![0; 15];
        powers[..6].fill(1);
        powers[missing] = 0;
        assert_eq!(
            five.reduce_integral(&Integral::new(powers)).unwrap(),
            five.reduce_integral(five.product_master()).unwrap()
        );
    }
    let numerator = Integral::from([1, 1, 1, 1, 1, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert!(matches!(
        five.reduce_integral(&numerator),
        Err(ProductBoundaryError::UnsupportedNumerator { integral }) if integral == numerator
    ));

    let four = ProductBoundaryReducer::new(
        equal_mass_four_loop_h().unwrap(),
        ProductBoundaryConfig {
            max_sector_candidates: 1_000_000,
            max_tadpole_steps: 2,
            ..ProductBoundaryConfig::default()
        },
    )
    .unwrap();
    let mut high_dot = four.product_master().powers().to_vec();
    let active = high_dot
        .iter()
        .position(|power| *power > 0)
        .expect("a four-loop product master has four active lines");
    high_dot[active] = 4;
    assert!(matches!(
        four.reduce_integral(&Integral::new(high_dot)),
        Err(ProductBoundaryError::ResourceLimit {
            resource: "tadpole recurrence steps",
            requested: 3,
            limit: 2,
        })
    ));

    // The configured work budget cannot override Symbolica's `u16`
    // polynomial-exponent representation.  Reject before the first tadpole
    // coefficient is constructed.
    let coefficient_degree_limited = ProductBoundaryReducer::new(
        equal_mass_two_loop_vacuum().unwrap(),
        ProductBoundaryConfig {
            max_tadpole_steps: usize::MAX,
            ..ProductBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        coefficient_degree_limited.reduce_integral(&Integral::from([0, 65_537, 1])),
        Err(ProductBoundaryError::ResourceLimit {
            resource: "Symbolica coefficient exponent degree",
            requested: 65_536,
            limit: 65_535,
        })
    ));

    // The common reversed-denominator convention changes a product ratio by
    // (-1)^(sum(a_i)-L), relative to the unit-power product master.
    let positive = ProductBoundaryReducer::new(
        equal_mass_two_loop_vacuum().unwrap(),
        ProductBoundaryConfig::default(),
    )
    .unwrap();
    let reversed = ProductBoundaryReducer::new(
        equal_mass_two_loop_vacuum_reversed().unwrap(),
        ProductBoundaryConfig::default(),
    )
    .unwrap();
    assert_eq!(positive.product_master(), reversed.product_master());
    let mut unit = positive.product_master().powers().to_vec();
    let active = unit.iter().position(|power| *power > 0).unwrap();
    for extra in 0..=2 {
        unit[active] = 1 + extra;
        let integral = Integral::new(unit.clone());
        let positive_coefficient = positive
            .reduce_integral(&integral)
            .unwrap()
            .coefficient(positive.product_master())
            .unwrap()
            .clone();
        let reversed_coefficient = reversed
            .reduce_integral(&integral)
            .unwrap()
            .coefficient(reversed.product_master())
            .unwrap()
            .clone();
        assert_eq!(
            reversed_coefficient,
            if extra % 2 == 0 {
                positive_coefficient
            } else {
                -positive_coefficient
            }
        );
    }

    // All sunset pinches are symmetry-related product sectors and map to the
    // stable family-local representative.
    let reference = positive.reduce_integral(positive.product_master()).unwrap();
    for missing in 0..3 {
        let mut pinch = vec![1; 3];
        pinch[missing] = 0;
        assert_eq!(
            positive.reduce_integral(&Integral::new(pinch)).unwrap(),
            reference
        );
    }

    // Typed construction and dispatch failures remain distinct from a proved
    // scaleless zero.
    let product_family = two_line_family("product_dispatch", [1, 0], [0, 1], 1, 1, false);
    let dispatch =
        ProductBoundaryReducer::new(product_family, ProductBoundaryConfig::default()).unwrap();
    assert!(
        dispatch
            .reduce_integral(&Integral::from([1, 0, 0]))
            .unwrap()
            .is_zero()
    );
    assert!(
        dispatch
            .try_reduce_integral(&Integral::from([1, 1, 1]))
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        dispatch.reduce_integral(&Integral::from([1, 1, 1])),
        Err(ProductBoundaryError::NotProductSector { .. })
    ));
    assert!(matches!(
        dispatch.reduce_integral(&Integral::from([1, 1, -1])),
        Err(ProductBoundaryError::UnsupportedNumerator { .. })
    ));
    assert!(matches!(
        dispatch.reduce_integral(&Integral::from([1, 1])),
        Err(ProductBoundaryError::WrongIntegralArity {
            expected: 3,
            actual: 2,
        })
    ));

    let massless = two_line_family("massless_product", [1, 0], [0, 1], 0, 0, false);
    assert!(matches!(
        ProductBoundaryReducer::new(massless, ProductBoundaryConfig::default()),
        Err(ProductBoundaryError::MasslessFamily)
    ));
    let unequal = two_line_family("unequal_product", [1, 0], [0, 1], 1, 2, false);
    assert!(matches!(
        ProductBoundaryReducer::new(unequal, ProductBoundaryConfig::default()),
        Err(ProductBoundaryError::UnequalMasses { position: 1 })
    ));
    let mixed = two_line_family("mixed_product", [1, 0], [0, 1], 1, 1, true);
    assert!(matches!(
        ProductBoundaryReducer::new(mixed, ProductBoundaryConfig::default()),
        Err(ProductBoundaryError::MixedPropagatorSigns { position: 1 })
    ));
    let non_unimodular = two_line_family("scaled_product", [2, 0], [0, 2], 1, 1, false);
    assert!(matches!(
        ProductBoundaryReducer::new(non_unimodular, ProductBoundaryConfig::default()),
        Err(ProductBoundaryError::NoUnimodularProductSector)
    ));
    assert!(matches!(
        ProductBoundaryReducer::new(
            equal_mass_two_loop_vacuum().unwrap(),
            ProductBoundaryConfig {
                max_sector_candidates: 2,
                max_tadpole_steps: 10,
                ..ProductBoundaryConfig::default()
            },
        ),
        Err(ProductBoundaryError::ResourceLimit {
            resource: "unimodular sector candidates",
            requested: 3,
            limit: 2,
        })
    ));

    let combination_limited = ProductBoundaryReducer::new(
        equal_mass_two_loop_vacuum().unwrap(),
        ProductBoundaryConfig {
            max_combination_terms: 1,
            max_combination_tadpole_steps: 1,
            ..ProductBoundaryConfig::default()
        },
    )
    .unwrap();
    let mut too_many_terms = rustred::LinearCombination::new();
    too_many_terms.add_term(
        Integral::from([0, 1, 1]),
        combination_limited.family().coefficients().one(),
    );
    too_many_terms.add_term(
        Integral::from([1, 0, 1]),
        combination_limited.family().coefficients().one(),
    );
    assert!(matches!(
        combination_limited.reduce_combination(&too_many_terms),
        Err(ProductBoundaryError::ResourceLimit {
            resource: "input combination terms",
            requested: 2,
            limit: 1,
        })
    ));

    let mut too_many_steps = rustred::LinearCombination::new();
    too_many_steps.add_term(
        Integral::from([0, 2, 1]),
        combination_limited.family().coefficients().one(),
    );
    too_many_steps.add_term(
        Integral::from([2, 0, 1]),
        combination_limited.family().coefficients().one(),
    );
    let aggregate_limited = ProductBoundaryReducer::new(
        equal_mass_two_loop_vacuum().unwrap(),
        ProductBoundaryConfig {
            max_combination_terms: 2,
            max_combination_tadpole_steps: 1,
            ..ProductBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(matches!(
        aggregate_limited.reduce_combination(&too_many_steps),
        Err(ProductBoundaryError::ResourceLimit {
            resource: "combination tadpole recurrence steps",
            requested: 2,
            limit: 1,
        })
    ));

    // The first key would require 65,535 exact recurrence steps.  A later key
    // takes the aggregate request over its cap, so the complete prepass must
    // reject before constructing the first recurrence coefficient.
    let aggregate_preflight = ProductBoundaryReducer::new(
        equal_mass_two_loop_vacuum().unwrap(),
        ProductBoundaryConfig {
            max_tadpole_steps: 65_535,
            max_combination_tadpole_steps: 65_535,
            ..ProductBoundaryConfig::default()
        },
    )
    .unwrap();
    let mut late_aggregate_failure = rustred::LinearCombination::new();
    late_aggregate_failure.add_term(
        Integral::from([0, 65_536, 1]),
        aggregate_preflight.family().coefficients().one(),
    );
    late_aggregate_failure.add_term(
        Integral::from([2, 0, 1]),
        aggregate_preflight.family().coefficients().one(),
    );
    assert!(matches!(
        aggregate_preflight.reduce_combination(&late_aggregate_failure),
        Err(ProductBoundaryError::ResourceLimit {
            resource: "combination tadpole recurrence steps",
            requested: 65_536,
            limit: 65_535,
        })
    ));

    // A zero aggregate recurrence budget still accepts terms for which no
    // tadpole recurrence runs: scaleless sectors, genuine non-product
    // sectors, and unit-power products.  A single dot is rejected before its
    // recurrence starts.
    let zero_work = ProductBoundaryReducer::new(
        equal_mass_two_loop_vacuum().unwrap(),
        ProductBoundaryConfig {
            max_combination_terms: 3,
            max_combination_tadpole_steps: 0,
            ..ProductBoundaryConfig::default()
        },
    )
    .unwrap();
    let mut recurrence_free = rustred::LinearCombination::new();
    recurrence_free.add_term(
        Integral::from([3, 0, 0]),
        zero_work.family().coefficients().one(),
    );
    recurrence_free.add_term(
        Integral::from([1, 1, 1]),
        zero_work.family().coefficients().one(),
    );
    recurrence_free.add_term(
        Integral::from([0, 1, 1]),
        zero_work.family().coefficients().one(),
    );
    zero_work.reduce_combination(&recurrence_free).unwrap();

    let mut one_recurrence = rustred::LinearCombination::new();
    one_recurrence.add_term(
        Integral::from([0, 2, 1]),
        zero_work.family().coefficients().one(),
    );
    assert!(matches!(
        zero_work.reduce_combination(&one_recurrence),
        Err(ProductBoundaryError::ResourceLimit {
            resource: "combination tadpole recurrence steps",
            requested: 1,
            limit: 0,
        })
    ));

    let zero_terms = ProductBoundaryReducer::new(
        equal_mass_two_loop_vacuum().unwrap(),
        ProductBoundaryConfig {
            max_combination_terms: 0,
            ..ProductBoundaryConfig::default()
        },
    )
    .unwrap();
    assert!(
        zero_terms
            .reduce_combination(&rustred::LinearCombination::new())
            .unwrap()
            .is_zero()
    );
    assert!(matches!(
        zero_terms.reduce_combination(&one_recurrence),
        Err(ProductBoundaryError::ResourceLimit {
            resource: "input combination terms",
            requested: 1,
            limit: 0,
        })
    ));
}
