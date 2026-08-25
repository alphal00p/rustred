#![cfg(feature = "legacy-authored-oracles")]

use rustred::families::{equal_mass_two_loop_vacuum, equal_mass_two_loop_vacuum_reversed};
use rustred::family::PropagatorSign;
use rustred::two_loop::TwoLoopBoundaryReducer;
use rustred::two_loop_pipeline::{TwoLoopReductionConfig, TwoLoopReductionPipeline};
use rustred::{Coefficient, ExactRational, IbpGenerator, Integral, LinearCombination};

fn odd_total_power(integral: &Integral) -> bool {
    integral
        .powers()
        .iter()
        .filter(|power| power.rem_euclid(2) != 0)
        .count()
        % 2
        != 0
}

fn signed(coefficient: &Coefficient, odd: bool) -> Coefficient {
    if odd {
        -coefficient.clone()
    } else {
        coefficient.clone()
    }
}

fn coefficient_or_zero(
    pipeline: &TwoLoopReductionPipeline,
    combination: &LinearCombination,
    integral: &Integral,
) -> Coefficient {
    combination
        .coefficient(integral)
        .cloned()
        .unwrap_or_else(|| pipeline.family().coefficients().zero())
}

fn assert_raw_ibp_parity(
    positive: &TwoLoopReductionPipeline,
    reversed: &TwoLoopReductionPipeline,
    seed: &Integral,
) {
    let positive_identities = IbpGenerator::new(positive.family()).generate_raw(seed);
    let reversed_identities = IbpGenerator::new(reversed.family()).generate_raw(seed);
    assert_eq!(positive_identities.len(), reversed_identities.len());

    for (positive_identity, reversed_identity) in
        positive_identities.iter().zip(&reversed_identities)
    {
        assert_eq!(
            positive_identity.differentiated_loop,
            reversed_identity.differentiated_loop
        );
        assert_eq!(
            positive_identity.contraction_loop,
            reversed_identity.contraction_loop
        );
        assert_eq!(
            positive_identity
                .equation
                .terms()
                .keys()
                .collect::<Vec<_>>(),
            reversed_identity
                .equation
                .terms()
                .keys()
                .collect::<Vec<_>>()
        );

        // Re-expressing I_-(a)=(-1)^sum(a) I_+(a) turns the reversed
        // equation into one common (-1)^sum(seed) multiple of the positive
        // equation.  This probes the generated derivative constants as well
        // as the denominator-cancellation terms.
        for (integral, positive_coefficient) in positive_identity.equation.terms() {
            let reversed_coefficient = reversed_identity
                .equation
                .coefficient(integral)
                .expect("raw parity-related equations have identical support");
            assert_eq!(
                signed(reversed_coefficient, odd_total_power(integral)),
                signed(positive_coefficient, odd_total_power(seed)),
                "raw IBP parity mismatch for seed {seed}, term {integral}"
            );
        }
    }
}

fn assert_reduction_parity(
    positive: &TwoLoopReductionPipeline,
    reversed: &TwoLoopReductionPipeline,
    integral: Integral,
) {
    let positive_reduction = positive.reduce_integral(&integral).unwrap();
    let reversed_reduction = reversed.reduce_integral(&integral).unwrap();

    for master in [positive.sunset_master(), positive.product_master()] {
        let positive_coefficient = coefficient_or_zero(positive, &positive_reduction, master);
        let reversed_coefficient = coefficient_or_zero(reversed, &reversed_reduction, master);
        let exponent_difference_is_odd = odd_total_power(&integral) != odd_total_power(master);
        assert_eq!(
            reversed_coefficient,
            signed(&positive_coefficient, exponent_difference_is_odd),
            "master-coefficient parity mismatch for {integral} -> {master}"
        );
    }
}

// Restricted Symbolica must remain on one test worker.  Keep construction,
// raw-IBP validation, and the bounded integrated regression in one test.
#[test]
fn reversed_denominators_obey_exact_exponent_parity() {
    let positive_family = equal_mass_two_loop_vacuum().unwrap();
    let reversed_family = equal_mass_two_loop_vacuum_reversed().unwrap();

    assert_ne!(positive_family.fingerprint(), reversed_family.fingerprint());
    for denominator in positive_family.denominators() {
        assert_eq!(
            denominator.propagator_sign(),
            Some(PropagatorSign::Positive)
        );
        assert_eq!(denominator.normalization(), Some(1));
    }
    for denominator in reversed_family.denominators() {
        assert_eq!(
            denominator.propagator_sign(),
            Some(PropagatorSign::Negative)
        );
        assert_eq!(denominator.normalization(), Some(-1));
    }
    assert_eq!(
        reversed_family.denominators()[0].quadratic_form(),
        &[
            ExactRational::from(-1),
            ExactRational::zero(),
            ExactRational::zero(),
        ]
    );
    let m2 = reversed_family.coefficients().parameter("m2").unwrap();
    assert_eq!(reversed_family.denominators()[0].shift(), &-m2);

    let config = TwoLoopReductionConfig {
        max_dots: 4,
        max_numerator_degree: 2,
        max_seed_candidates: 1_000,
        max_boundary_terms: 100_000,
    };
    let positive = TwoLoopReductionPipeline::build_for_family(positive_family, config).unwrap();
    let reversed = TwoLoopReductionPipeline::build_for_family(reversed_family, config).unwrap();
    assert_eq!(
        TwoLoopBoundaryReducer::new(reversed.family())
            .unwrap()
            .propagator_sign(),
        PropagatorSign::Negative
    );

    let mut raw_identities_positive = Vec::new();
    let mut raw_identities_reversed = Vec::new();
    for first in 1..=2 {
        for second in 1..=2 {
            for third in 1..=2 {
                let seed = Integral::from([first, second, third]);
                assert_raw_ibp_parity(&positive, &reversed, &seed);
                raw_identities_positive
                    .extend(IbpGenerator::new(positive.family()).generate_raw(&seed));
                raw_identities_reversed
                    .extend(IbpGenerator::new(reversed.family()).generate_raw(&seed));
            }
        }
    }
    positive
        .validate_identities(&raw_identities_positive)
        .unwrap();
    reversed
        .validate_identities(&raw_identities_reversed)
        .unwrap();

    let mut checked = 0;
    for first in -2..=2 {
        for second in -2..=2 {
            for third in -2..=2 {
                assert_reduction_parity(
                    &positive,
                    &reversed,
                    Integral::from([first, second, third]),
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 5_usize.pow(3));
    assert_reduction_parity(&positive, &reversed, Integral::from([3, 2, 1]));
}
