// Keep every Symbolica-backed tensor check in this single test.  Symbolica's
// process-global symbol registry is deliberately not exercised concurrently by
// several Rust test functions in the restricted CI instance.

use rustred::CoefficientContext;
use rustred::tensor::{
    IndexedVector, LoopVector, LorentzIndex, Metric, MetricPairing, ScalarProduct,
    ScalarProductMonomial, SlotPairing, TensorError, TensorMonomial, VacuumTensorProjector,
    perfect_matching_count, perfect_matchings,
};
use symbolica::prelude::*;

fn index(id: u32) -> LorentzIndex {
    LorentzIndex::new(id)
}

fn loop_vector(id: u16) -> LoopVector {
    LoopVector::new(id)
}

fn vector(loop_id: u16, index_id: u32) -> IndexedVector {
    IndexedVector::new(loop_vector(loop_id), index(index_id))
}

fn metrics(pairs: &[(u32, u32)]) -> MetricPairing {
    MetricPairing::new(
        pairs
            .iter()
            .map(|&(left, right)| Metric::new(index(left), index(right))),
    )
}

fn scalar_products(pairs: &[(u16, u16)]) -> ScalarProductMonomial {
    ScalarProductMonomial::from_factors(
        pairs
            .iter()
            .map(|&(left, right)| (ScalarProduct::new(loop_vector(left), loop_vector(right)), 1)),
    )
}

#[test]
fn exact_global_vacuum_tensor_projector() {
    let context = CoefficientContext::new(["d", "m2"]);
    let d = context.parameter("d").unwrap();
    let one = context.one();
    let zero = context.zero();
    let mut projector = VacuumTensorProjector::new(&context, "d").unwrap();

    // Perfect matchings and their exact metric contraction Gram entries.
    assert_eq!(perfect_matching_count(0), Some(1));
    assert_eq!(perfect_matching_count(1), Some(0));
    assert_eq!(perfect_matching_count(2), Some(1));
    assert_eq!(perfect_matching_count(4), Some(3));
    assert_eq!(perfect_matching_count(6), Some(15));
    let pairings = perfect_matchings(4, 3).unwrap();
    assert_eq!(pairings[0].pairs(), &[(0, 1), (2, 3)]);
    assert_eq!(pairings[1].pairs(), &[(0, 2), (1, 3)]);
    assert_eq!(pairings[2].pairs(), &[(0, 3), (1, 2)]);
    assert_eq!(pairings[0].contraction_cycles(&pairings[0]).unwrap(), 2);
    assert_eq!(pairings[0].contraction_cycles(&pairings[1]).unwrap(), 1);
    assert_eq!(
        projector
            .pairing_contraction(&pairings[0], &pairings[0])
            .unwrap(),
        d.pow(2)
    );
    assert_eq!(
        projector
            .pairing_contraction(&pairings[0], &pairings[1])
            .unwrap(),
        d
    );

    // Rank zero is the multiplicative identity, while every odd vacuum rank
    // vanishes before any perfect-matching solve is attempted.
    let rank_zero = projector.reduce(&TensorMonomial::default()).unwrap();
    assert_eq!(rank_zero.len(), 1);
    assert_eq!(
        rank_zero.coefficient(&MetricPairing::empty(), &ScalarProductMonomial::one()),
        Some(&one)
    );
    let rank_three = TensorMonomial::new([vector(0, 0), vector(1, 1), vector(0, 2)]);
    assert!(projector.reduce(&rank_three).unwrap().is_zero());

    // Universal rank-two projector:
    // k_a(mu) k_b(nu) -> g(mu,nu) (k_a.k_b) / d.
    let rank_two_input = TensorMonomial::new([vector(0, 10), vector(1, 11)]);
    let rank_two = projector.reduce(&rank_two_input).unwrap();
    let rank_two_metrics = metrics(&[(10, 11)]);
    let rank_two_scalar = scalar_products(&[(0, 1)]);
    let inverse_d = context.parse("1/d").unwrap();
    assert_eq!(rank_two.len(), 1);
    assert_eq!(
        rank_two.coefficient(&rank_two_metrics, &rank_two_scalar),
        Some(&inverse_d)
    );

    let metric_symbol = symbol!("rustred_tensor_test::g"; Symmetric);
    let scalar_product_symbol = symbol!("rustred_tensor_test::sp"; Symmetric);
    let expected_atom = inverse_d.to_expression()
        * Metric::new(index(10), index(11)).to_atom(metric_symbol)
        * ScalarProduct::new(loop_vector(0), loop_vector(1)).to_atom(scalar_product_symbol);
    assert_eq!(
        rank_two.to_atom(metric_symbol, scalar_product_symbol),
        expected_atom
    );

    // For four distinct loop labels, every output metric pairing multiplies
    // every source scalar-product pairing.  The inverse Gram matrix has the
    // standard diagonal (d+1)/(d(d-1)(d+2)) and common negative off diagonal.
    let rank_four_input =
        TensorMonomial::new([vector(0, 20), vector(1, 21), vector(2, 22), vector(3, 23)]);
    let rank_four = projector.reduce(&rank_four_input).unwrap();
    assert_eq!(rank_four.len(), 9);
    let output_metrics = [
        metrics(&[(20, 21), (22, 23)]),
        metrics(&[(20, 22), (21, 23)]),
        metrics(&[(20, 23), (21, 22)]),
    ];
    let source_scalars = [
        scalar_products(&[(0, 1), (2, 3)]),
        scalar_products(&[(0, 2), (1, 3)]),
        scalar_products(&[(0, 3), (1, 2)]),
    ];
    let rank_four_diagonal = context.parse("(d+1)/(d*(d-1)*(d+2))").unwrap();
    let rank_four_off_diagonal = context.parse("-1/(d*(d-1)*(d+2))").unwrap();
    for (output_position, metric_pairing) in output_metrics.iter().enumerate() {
        for (source_position, scalar_monomial) in source_scalars.iter().enumerate() {
            let expected = if output_position == source_position {
                &rank_four_diagonal
            } else {
                &rank_four_off_diagonal
            };
            assert_eq!(
                rank_four.coefficient(metric_pairing, scalar_monomial),
                Some(expected)
            );
        }
    }

    // Recontracting any output pairing recovers exactly its corresponding
    // source contraction, G * G^-1 = 1.
    for contraction_position in 0..pairings.len() {
        for source_position in 0..pairings.len() {
            let mut reconstructed = context.zero();
            for output_position in 0..pairings.len() {
                let gram_entry = projector
                    .pairing_contraction(
                        &pairings[contraction_position],
                        &pairings[output_position],
                    )
                    .unwrap();
                let coefficient = rank_four
                    .coefficient(
                        &output_metrics[output_position],
                        &source_scalars[source_position],
                    )
                    .unwrap();
                reconstructed = &reconstructed + &(&gram_entry * coefficient);
            }
            assert_eq!(
                reconstructed,
                if contraction_position == source_position {
                    one.clone()
                } else {
                    zero.clone()
                }
            );
        }
    }

    // With four identical vectors the three source monomials coincide and
    // combine to the familiar 1/(d(d+2)) coefficient for each metric pairing.
    let identical_rank_four_input =
        TensorMonomial::new([vector(7, 30), vector(7, 31), vector(7, 32), vector(7, 33)]);
    let identical_rank_four = projector.reduce(&identical_rank_four_input).unwrap();
    let squared_norm = ScalarProductMonomial::from_factors([(
        ScalarProduct::new(loop_vector(7), loop_vector(7)),
        2,
    )]);
    let identical_coefficient = context.parse("1/(d*(d+2))").unwrap();
    assert_eq!(identical_rank_four.len(), 3);
    for metric_pairing in [
        metrics(&[(30, 31), (32, 33)]),
        metrics(&[(30, 32), (31, 33)]),
        metrics(&[(30, 33), (31, 32)]),
    ] {
        assert_eq!(
            identical_rank_four.coefficient(&metric_pairing, &squared_norm),
            Some(&identical_coefficient)
        );
    }

    // Pre-existing metrics are contracted exactly before projection.
    let vector_contraction = TensorMonomial::from_parts(
        [vector(2, 40), vector(5, 41)],
        [Metric::new(index(40), index(41))],
        ScalarProductMonomial::one(),
    );
    let contracted = projector.contract_metrics(&vector_contraction).unwrap();
    assert!(contracted.vectors().is_empty());
    assert!(contracted.metrics().is_empty());
    assert_eq!(contracted.coefficient(), &one);
    assert_eq!(contracted.scalar_products(), &scalar_products(&[(2, 5)]));

    let open_chain = TensorMonomial::from_parts(
        [vector(4, 50)],
        [Metric::new(index(50), index(51))],
        ScalarProductMonomial::one(),
    );
    let contracted = projector.contract_metrics(&open_chain).unwrap();
    assert_eq!(contracted.vectors(), &[vector(4, 51)]);
    assert!(contracted.metrics().is_empty());

    let trace = TensorMonomial::from_parts(
        [],
        [Metric::new(index(60), index(60))],
        ScalarProductMonomial::one(),
    );
    let contracted = projector.contract_metrics(&trace).unwrap();
    assert_eq!(contracted.coefficient(), &d);
    assert!(contracted.vectors().is_empty());
    assert!(contracted.metrics().is_empty());

    let shortened_metric_chain = TensorMonomial::from_parts(
        [],
        [
            Metric::new(index(70), index(71)),
            Metric::new(index(71), index(72)),
        ],
        ScalarProductMonomial::one(),
    );
    let contracted = projector.contract_metrics(&shortened_metric_chain).unwrap();
    assert_eq!(contracted.metrics(), &metrics(&[(70, 72)]));
    assert_eq!(contracted.coefficient(), &one);

    // Invalid Einstein multiplicities and explicit resource limits are typed
    // failures rather than accidental table misses or runaway allocations.
    let invalid_index = TensorMonomial::new([vector(0, 80), vector(1, 80), vector(2, 80)]);
    assert!(matches!(
        projector.contract_metrics(&invalid_index),
        Err(TensorError::InvalidIndexMultiplicity {
            index: invalid,
            occurrences: 3
        }) if invalid == index(80)
    ));

    let mut limited = VacuumTensorProjector::new(&context, "d")
        .unwrap()
        .with_max_pairings(2);
    assert!(matches!(
        limited.reduce(&rank_four_input),
        Err(TensorError::PairingLimitExceeded {
            rank: 4,
            pairings: Some(3),
            limit: 2
        })
    ));

    let malformed = SlotPairing::new(4, [(0, 1)]).unwrap_err();
    assert!(matches!(
        malformed,
        TensorError::InvalidPairingSize { rank: 4, pairs: 1 }
    ));
}
