//! Independent adversarial checks for the source-weight materializer.
//!
//! These are matrix fixtures, not topology fixtures. In particular, changing
//! the integral-key arity cannot change admission or algebraic semantics.

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};

use crate::algebra::CoefficientContext;
use crate::solver::{ExactRow, Integral, IntegralOrder, Term};

use super::super::super::{
    CoefficientVariableOrder, MaterializationError, MaterializationEvent, SymbolicExactBackend,
    exact_materialize_using_with_observer,
};
use super::super::{FrameVariables, ProbeFrame};

fn integral<const N: usize>(shift: i16) -> Integral<N> {
    let mut shifts = [0; N];
    shifts[0] = shift;
    Integral::symbolic(shifts).unwrap()
}

fn row<const N: usize>(context: &CoefficientContext, terms: &[(i16, &str)]) -> ExactRow<N> {
    terms
        .iter()
        .map(|(shift, coefficient)| Term {
            integral: integral(*shift),
            coefficient: context.coefficient_fixture(coefficient),
        })
        .collect()
}

fn backend() -> SymbolicExactBackend {
    limited_backend(20000, 200000, 1000)
}

fn limited_backend(images: usize, values: usize, weights: usize) -> SymbolicExactBackend {
    SymbolicExactBackend::SemiNumericalSourceWeights {
        max_degree: 8,
        max_probes: 2000,
        max_attempts: 3,
        max_primes: 4,
        max_cached_images: images,
        max_cached_values: values,
        max_weight_slots: weights,
    }
}

fn run<const N: usize>(
    rows: &[ExactRow<N>],
    target: i16,
    backend: SymbolicExactBackend,
    coefficient_order: CoefficientVariableOrder,
    priority: &[usize],
) -> (
    Result<ExactRow<N>, MaterializationError>,
    Vec<MaterializationEvent<N>>,
) {
    let mut events = Vec::new();
    let result = exact_materialize_using_with_observer(
        rows,
        &IntegralOrder::new([true; N], [false; N]),
        integral(target),
        backend,
        coefficient_order,
        priority,
        |event| events.push(event),
    );
    (result, events)
}

fn assert_no_exact_replay<const N: usize>(events: &[MaterializationEvent<N>]) {
    assert!(!events.iter().any(|event| matches!(
        event,
        MaterializationEvent::SemiNumericalExactReplayStarted { .. }
            | MaterializationEvent::SemiNumericalExactReplayFinished { .. }
            | MaterializationEvent::RowStarted { .. }
            | MaterializationEvent::RowFinished { .. }
    )));
}

fn differential_nonmonotone<const N: usize>() {
    let context = CoefficientContext::new(["unused", "a", "b"]);
    let rows = vec![
        row::<N>(&context, &[(3, "a+1"), (2, "1"), (1, "2")]),
        row(&context, &[(4, "b+2"), (2, "3"), (1, "4")]),
        row(&context, &[(4, "b+2"), (3, "a+1"), (2, "a+5"), (1, "b+9")]),
        // Existing sparse materialization stops before this source. The new
        // backend must still validate it, but may not back-substitute it.
        row(&context, &[(1, "a*b")]),
    ];
    let expected = run(
        &rows,
        2,
        SymbolicExactBackend::Sparse,
        CoefficientVariableOrder::Original,
        &[],
    )
    .0
    .unwrap();
    assert_eq!(expected, row(&context, &[(2, "1"), (1, "(b+3)/(a+1)")]));
    for order in [
        CoefficientVariableOrder::Original,
        CoefficientVariableOrder::Reverse,
        CoefficientVariableOrder::IndicesFirst,
    ] {
        let priority = if order == CoefficientVariableOrder::IndicesFirst {
            &[2, 0, 1][..]
        } else {
            &[]
        };
        let (actual, events) = run(&rows, 2, backend(), order, priority);
        let actual = actual.unwrap();
        assert_eq!(actual, expected);
        for term in &actual {
            assert_eq!(
                term.coefficient.numerator.variables(),
                context.one().numerator.variables()
            );
            assert_eq!(
                term.coefficient.denominator.variables(),
                context.one().denominator.variables()
            );
        }
        assert_no_exact_replay(&events);
    }
}

#[test]
fn nonmonotone_prefix_matches_sparse_with_two_unrelated_key_arities_and_all_maps() {
    differential_nonmonotone::<2>();
    differential_nonmonotone::<5>();
}

#[test]
fn one_source_canceled_pole_keeps_only_the_existing_rational_candidate_contract() {
    let context = CoefficientContext::new(["x"]);
    let rows = vec![row::<2>(&context, &[(2, "x"), (1, "x")])];
    let (actual, events) = run(&rows, 2, backend(), CoefficientVariableOrder::Original, &[]);
    assert_eq!(actual.unwrap(), row(&context, &[(2, "1"), (1, "1")]));
    assert_no_exact_replay(&events);
    // The normalized rational row has canceled its 1/x multiplier pole.
    // This is NOT a test or claim of valid specialization at x=0; original
    // source/applicability certification remains the unchanged downstream gate.
}

#[test]
fn empty_dependent_and_earlier_easy_sources_are_not_silently_dropped() {
    let context = CoefficientContext::new(["x"]);
    let harder = row::<2>(&context, &[(3, "x"), (2, "1")]);
    let target = row(&context, &[(2, "x+1"), (1, "1")]);
    for rows in [
        vec![vec![], target.clone()],
        vec![harder.clone(), harder, target.clone()],
        vec![row(&context, &[(1, "x")]), target],
    ] {
        let expected = run(
            &rows,
            2,
            SymbolicExactBackend::Sparse,
            CoefficientVariableOrder::Original,
            &[],
        );
        assert!(expected.0.is_ok());
        let (actual, events) = run(&rows, 2, backend(), CoefficientVariableOrder::Original, &[]);
        assert!(
            actual.is_err(),
            "an inadmissible prefix was silently changed"
        );
        assert_no_exact_replay(&events);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, MaterializationEvent::SemiNumericalFinished { .. }))
        );
    }
}

#[test]
fn malformed_zero_denominator_after_the_hit_is_not_skipped() {
    let context = CoefficientContext::new(["x"]);
    for coefficient in ["0", "x"] {
        let mut rows = vec![
            row::<2>(&context, &[(2, "x"), (1, "1")]),
            row(&context, &[(1, coefficient)]),
        ];
        rows[1][0].coefficient.denominator = context.zero().numerator;
        let (actual, events) = run(&rows, 2, backend(), CoefficientVariableOrder::Original, &[]);
        assert!(actual.is_err());
        assert_no_exact_replay(&events);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, MaterializationEvent::SemiNumericalFinished { .. }))
        );
    }
}

#[test]
fn malformed_coefficient_map_after_the_hit_is_not_skipped() {
    let context = CoefficientContext::new(["x", "y"]);
    let other = CoefficientContext::new(["y", "x"]);
    let mut rows = vec![
        row::<5>(&context, &[(2, "x"), (1, "1")]),
        row(&context, &[(1, "0")]),
    ];
    rows[1][0].coefficient.denominator = other.one().denominator;
    let (actual, events) = run(&rows, 2, backend(), CoefficientVariableOrder::Original, &[]);
    assert!(matches!(
        actual,
        Err(MaterializationError::CoefficientVariableMapMismatch)
    ));
    assert_no_exact_replay(&events);
}

#[test]
fn constant_frame_is_explicitly_unsupported_without_hidden_sparse_fallback() {
    let context = CoefficientContext::new(["unused"]);
    let rows = vec![row::<2>(&context, &[(2, "2"), (1, "3")])];
    let (actual, events) = run(&rows, 2, backend(), CoefficientVariableOrder::Original, &[]);
    assert!(actual.is_err());
    assert_no_exact_replay(&events);
}

#[test]
fn all_aggregate_budget_failures_are_explicit_without_hidden_sparse_fallback() {
    let context = CoefficientContext::new(["a", "b"]);
    let rows = vec![row::<2>(&context, &[(2, "a"), (1, "b")])];
    for limited in [
        limited_backend(0, 200000, 1000),
        limited_backend(20000, 0, 1000),
        limited_backend(20000, 200000, 0),
    ] {
        let (actual, events) = run(&rows, 2, limited, CoefficientVariableOrder::Original, &[]);
        assert!(actual.is_err());
        assert_no_exact_replay(&events);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, MaterializationEvent::SemiNumericalFinished { .. }))
        );
    }
}

fn prepared<const N: usize>(rows: &[ExactRow<N>], shifts: &[i16]) -> (ProbeFrame, FrameVariables) {
    let variables = FrameVariables::try_new(rows, CoefficientVariableOrder::Original, &[]).unwrap();
    let columns = shifts.iter().copied().map(integral).collect::<Vec<_>>();
    let frame = ProbeFrame::new(
        rows,
        &columns,
        &IntegralOrder::new([true; N], [false; N]),
        &variables,
    )
    .unwrap();
    (frame, variables)
}

#[test]
fn a_bad_rank_specialization_is_a_miss_not_proof_of_failure() {
    let context = CoefficientContext::new(["a"]);
    let rows = vec![
        row::<2>(&context, &[(3, "a"), (1, "1")]),
        row(&context, &[(3, "a"), (2, "a+1"), (1, "2")]),
    ];
    let (frame, _) = prepared(&rows, &[3, 2, 1]);
    let field = Zp64::new(101);
    let unlucky = super::probe_image(&frame, 1, 1000, &field, &[field.to_element(0)])
        .unwrap()
        .unwrap();
    // The image has a target after an earlier easier pivot, but is explicitly
    // rank-inadmissible. Production admission must not discard that source.
    assert!(unlucky.harder_rank < unlucky.pivots.len() - 1);
    let good = super::probe_image(&frame, 1, 1000, &field, &[field.to_element(1)])
        .unwrap()
        .unwrap();
    assert_eq!(good.harder_rank, 1);
    assert_eq!(good.pivots, [Some(0), Some(1)]);
    assert_eq!(good.accepted_sources, [0, 1]);
    assert_eq!(good.accepted_l_rows, [0, 1]);
    // At a=-1 the second source does not pivot on the target at all.
    assert!(
        super::probe_image(&frame, 1, 1000, &field, &[field.to_element(100)])
            .unwrap()
            .is_none()
    );
}

#[test]
fn an_unlucky_prime_cannot_be_mistaken_for_characteristic_zero_rank_loss() {
    let context = CoefficientContext::new(["x"]);
    let rows = vec![
        row::<5>(&context, &[(3, "101*x"), (1, "1")]),
        row(&context, &[(3, "101*x"), (2, "x+1"), (1, "2")]),
    ];
    let (frame, _) = prepared(&rows, &[3, 2, 1]);
    let unlucky_field = Zp64::new(101);
    let unlucky = super::probe_image(
        &frame,
        1,
        1000,
        &unlucky_field,
        &[unlucky_field.to_element(1)],
    )
    .unwrap()
    .unwrap();
    assert_eq!(unlucky.pivots, [Some(2), Some(1)]);
    assert!(unlucky.harder_rank < unlucky.pivots.len() - 1);

    let good_field = Zp64::new(103);
    let good = super::probe_image(&frame, 1, 1000, &good_field, &[good_field.to_element(1)])
        .unwrap()
        .unwrap();
    assert_eq!(good.pivots, [Some(0), Some(1)]);
    assert_eq!(good.harder_rank, good.pivots.len() - 1);
    assert_eq!(
        good.row,
        [
            (1, good_field.to_element(1)),
            (2, good_field.to_element(52))
        ]
    );
}

#[test]
fn admitted_independent_prefix_reconstructs_an_exactly_zero_source_weight() {
    let context = CoefficientContext::new(["a", "b"]);
    let rows = vec![
        row::<2>(&context, &[(4, "a")]),
        row(&context, &[(3, "b"), (2, "1"), (1, "2")]),
        row(&context, &[(3, "b"), (2, "2"), (1, "5")]),
    ];
    let (frame, _) = prepared(&rows, &[4, 3, 2, 1]);
    let field = Zp64::new(101);
    let image = super::probe_image(
        &frame,
        2,
        1000,
        &field,
        &[field.to_element(3), field.to_element(5)],
    )
    .unwrap()
    .unwrap();
    assert_eq!(image.harder_rank, image.pivots.len() - 1);
    assert_eq!(
        image.weights,
        [
            field.to_element(0),
            field.to_element(100),
            field.to_element(1)
        ]
    );
    let (actual, events) = run(&rows, 2, backend(), CoefficientVariableOrder::Original, &[]);
    let expected = run(
        &rows,
        2,
        SymbolicExactBackend::Sparse,
        CoefficientVariableOrder::Original,
        &[],
    )
    .0
    .unwrap();
    assert_eq!(expected, row(&context, &[(2, "1"), (1, "3")]));
    assert_eq!(actual.unwrap(), expected);
    assert_no_exact_replay(&events);
    assert!(events.iter().any(|event| matches!(
        event,
        MaterializationEvent::TargetWeightsFinished { nonzero_weights: 2 }
    )));
}

#[test]
fn denominator_poles_and_the_structural_sentinel_cannot_supply_a_hit() {
    let context = CoefficientContext::new(["x"]);
    let rows = vec![row::<5>(&context, &[(2, "1/x"), (1, "1")])];
    let (frame, _) = prepared(&rows, &[2, 1]);
    let field = Zp64::new(101);
    assert!(
        super::probe_image(&frame, 0, 1000, &field, &[field.to_element(0)])
            .unwrap()
            .is_none()
    );
    assert!(
        super::probe_image(&frame, 0, 1000, &field, &[field.to_element(1)])
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        super::probe_image(&frame, 2, 1000, &field, &[field.to_element(1)]),
        Err(MaterializationError::TargetAbsent)
    ));
}

#[test]
fn original_source_l_row_and_accepted_u_ids_remain_distinct() {
    let context = CoefficientContext::new(["x"]);
    let rows = vec![
        vec![],
        row::<2>(&context, &[(1, "2")]),
        row(&context, &[(1, "6")]),
        row(&context, &[(3, "4"), (1, "1")]),
        row(&context, &[(3, "8"), (2, "3*x"), (1, "5")]),
    ];
    let (frame, _) = prepared(&rows, &[3, 2, 1]);
    let field = Zp64::new(101);
    let image = super::probe_image(&frame, 1, 1000, &field, &[field.to_element(1)])
        .unwrap()
        .unwrap();
    assert_eq!(image.pivots, [None, Some(2), None, Some(0), Some(1)]);
    assert_eq!(image.accepted_sources, [1, 3, 4]);
    assert_eq!(image.accepted_l_rows, [0, 2, 3]);
    assert_eq!(
        image
            .weights
            .iter()
            .map(|value| field.from_element(value))
            .collect::<Vec<_>>(),
        [0, 0, 0, 33, 34]
    );
    assert_eq!(
        image.row,
        [(1, field.to_element(1)), (2, field.to_element(1))]
    );
    assert_eq!(image.harder_rank, 1);
    // The correct row-span identity is still NOT an admitted canonical-row
    // certificate for this dependent/easier-prefix frame.
    assert_ne!(image.harder_rank, image.pivots.len() - 1);
}

#[test]
fn full_native_product_rejects_wrong_weights_rhs_omitted_tail_and_sentinel() {
    let context = CoefficientContext::new(["unused", "a", "c", "d"]);
    let rows = vec![
        row::<2>(&context, &[(3, "a"), (2, "1"), (1, "c")]),
        row(&context, &[(3, "a"), (2, "a+2"), (1, "d")]),
    ];
    let (frame, variables) = prepared(&rows, &[3, 2, 1]);
    let mapped = |text: &str| {
        variables
            .map_coefficient(&context.coefficient_fixture(text))
            .unwrap()
    };
    let weights = vec![mapped("-1/(a+1)"), mapped("1/(a+1)")];
    let expected = vec![(1, mapped("1")), (2, mapped("(d-c)/(a+1)"))];
    let exact = super::validate_product(&frame, 2, 1, &weights, &expected, &variables).unwrap();
    assert_eq!(
        exact,
        vec![
            (1, context.one()),
            (2, context.coefficient_fixture("(d-c)/(a+1)"))
        ]
    );
    let wrong_weights = vec![mapped("0"), mapped("1/(a+1)")];
    assert!(super::validate_product(&frame, 2, 1, &wrong_weights, &expected, &variables).is_err());
    for wrong_row in [
        vec![(1, mapped("1")), (2, mapped("(d-c)/(a+1)+1"))],
        vec![(1, mapped("1"))],
        vec![
            (0, mapped("1")),
            (1, mapped("1")),
            (2, mapped("(d-c)/(a+1)")),
        ],
        vec![(1, mapped("2")), (2, mapped("(d-c)/(a+1)"))],
        vec![
            (1, mapped("1")),
            (2, mapped("(d-c)/(a+1)")),
            (3, mapped("1")),
        ],
    ] {
        assert!(super::validate_product(&frame, 2, 1, &weights, &wrong_row, &variables).is_err());
    }
    assert!(super::validate_product(&frame, 2, 1, &weights[..1], &expected, &variables).is_err());
    let foreign = CoefficientContext::new(["d", "c", "a"]);
    let wrong_map = vec![foreign.coefficient_fixture("-1/(a+1)"), mapped("1/(a+1)")];
    assert!(super::validate_product(&frame, 2, 1, &wrong_map, &expected, &variables).is_err());
}

#[test]
fn denominator_only_variable_survives_full_product_and_restoration() {
    let context = CoefficientContext::new(["unused", "x", "y"]);
    let rows = vec![row::<5>(&context, &[(2, "x"), (1, "1/y")])];
    let (actual, events) = run(&rows, 2, backend(), CoefficientVariableOrder::Reverse, &[]);
    assert_eq!(actual.unwrap(), row(&context, &[(2, "1"), (1, "1/(x*y)")]));
    assert_no_exact_replay(&events);
}

fn cache_limits(images: usize, values: usize, weights: usize) -> super::Limits {
    super::Limits {
        max_degree: 8,
        max_probes: 2000,
        max_attempts: 3,
        max_primes: 4,
        max_cached_images: images,
        max_cached_values: values,
        max_weight_slots: weights,
    }
}

#[test]
fn nonzero_cache_limits_charge_repeated_and_invalid_images_correctly() {
    let context = CoefficientContext::new(["x"]);
    let rows = vec![row::<2>(&context, &[(2, "1/x"), (1, "1")])];
    let (frame, _) = prepared(&rows, &[2, 1]);
    let field = Zp64::new(101);
    let mut one_image = super::ImageCache::new(&frame, 0, cache_limits(1, 1000, 100));
    assert!(
        one_image
            .image(&field, &[field.to_element(1)])
            .unwrap()
            .is_some()
    );
    // A hit does not consume a second entry in the shared scalar-oracle cache.
    assert!(
        one_image
            .image(&field, &[field.to_element(1)])
            .unwrap()
            .is_some()
    );
    assert!(one_image.image(&field, &[field.to_element(2)]).is_err());

    // A failed pole image still owns its key. Two coordinates (prime+point)
    // fit, but another point must exceed this three-slot aggregate budget.
    let mut invalid_images = super::ImageCache::new(&frame, 0, cache_limits(100, 3, 100));
    assert!(
        invalid_images
            .image(&field, &[field.to_element(0)])
            .unwrap()
            .is_none()
    );
    assert!(
        invalid_images
            .image(&field, &[field.to_element(1)])
            .is_err()
    );

    // Native output storage is charged in addition to the key coordinates.
    let mut values = super::ImageCache::new(&frame, 0, cache_limits(100, 2, 100));
    assert!(values.image(&field, &[field.to_element(1)]).is_err());
}

#[test]
fn changed_source_chronology_is_an_unusable_reconstruction_sample() {
    let context = CoefficientContext::new(["a"]);
    let rows = vec![
        row::<2>(&context, &[(3, "a"), (2, "a")]),
        row(&context, &[(3, "1"), (2, "2"), (1, "1")]),
        row(&context, &[(2, "1"), (1, "2")]),
    ];
    let (frame, _) = prepared(&rows, &[3, 2, 1]);
    let field = Zp64::new(101);
    let mut cache = super::ImageCache::new(&frame, 1, cache_limits(100, 10000, 100));
    let generic = cache
        .image(&field, &[field.to_element(2)])
        .unwrap()
        .unwrap();
    let chronology = generic.pivots.clone();
    assert_eq!(chronology, [Some(0), Some(1)]);
    cache.freeze(chronology);
    assert!(
        cache
            .image(&field, &[field.to_element(0)])
            .unwrap()
            .is_none()
    );
    assert!(
        cache
            .image(&field, &[field.to_element(3)])
            .unwrap()
            .is_some()
    );
}
