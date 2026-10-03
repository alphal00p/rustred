use super::*;
use crate::{policies, source_product};
use rustred::{
    algebra::{CoefficientContext, IndexedAlgebraLimits},
    family::AffineDenominator,
    identity::{
        IndexShift, ParametricIbpGenerator, TranslatedSourceLimits, TranslatedSourceRequest,
    },
};

fn limits() -> Limits {
    Limits {
        arithmetic: ExactAlgebraLimits::default(),
        max_sources: 64,
        max_conditions: 10_000,
        max_operations: 100_000,
    }
}

// A named scientific regression fixture only; the nominator has no topology,
// loop-count, denominator-position or source-ordinal dispatch.
fn family(loop_map: &[usize; 5], axis_map: &[usize; 15]) -> IntegralFamily {
    let momenta = [
        [1, 0, 0, 0, 0],
        [0, 1, 0, 0, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 1, 0],
        [0, 0, 0, 0, 1],
        [1, 0, -1, 0, 0],
        [1, 0, 0, -1, 0],
        [1, 0, 0, 0, -1],
        [0, 1, -1, 0, 0],
        [0, 1, 0, -1, 0],
        [0, 1, 0, 0, -1],
        [0, 0, 1, 0, -1],
        [0, 0, 0, 1, -1],
        [1, 1, 0, -1, 0],
        [0, 0, 1, -1, 0],
    ];
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let mut denominators: Vec<_> = (0..15).map(|_| None).collect();
    for (old_axis, old_momentum) in momenta.iter().enumerate() {
        let mut momentum = [0_i64; 5];
        for i in 0..5 {
            momentum[loop_map[i]] = old_momentum[i];
        }
        let mut coefficients = Vec::new();
        for i in 0..5 {
            for j in i..5 {
                coefficients
                    .push(c.integer(momentum[i] * momentum[j] * if i == j { 1 } else { 2 }));
            }
        }
        denominators[axis_map[old_axis]] =
            Some(AffineDenominator::new(c.integer(-1), coefficients));
    }
    IntegralFamily::new(
        "geometry-nomination-fixture",
        (0..5).map(|i| format!("k{i}")).collect(),
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators.into_iter().map(Option::unwrap).collect(),
        vec![],
        vec![c.zero(); 15],
    )
    .unwrap()
}

fn original_family() -> IntegralFamily {
    family(&[0, 1, 2, 3, 4], &std::array::from_fn(|i| i))
}
fn active() -> [bool; 15] {
    std::array::from_fn(|i| [0, 2, 4, 9, 10, 14].contains(&i))
}

#[test]
fn exact_geometry_reproduces_eight_prescribed_views_and_spectators() {
    let family = original_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let c = generator.context();
    let n = nominate(&family, c, &active(), 0, 13, limits()).unwrap();
    assert_eq!(n.weights.len(), 8);
    assert_eq!(n.report["protected_active_axis"], 0);
    assert_eq!(
        n.report["inactive_spectator_axes"],
        json!([1, 3, 8, 11, 12])
    );
    assert_eq!(n.report["other_dependent_inactive_axes"], json!([5, 6, 7]));
    let mut expected = BTreeMap::new();
    for (contraction, minus_axis, numerator, denominator) in [
        (1, None, 1, 1),
        (1, Some(0), 1, 1),
        (3, None, -1, 1),
        (3, Some(0), -1, 1),
        (0, Some(13), -1, 2),
        (0, Some(0), 1, 2),
        (0, Some(9), 1, 2),
        (0, None, 1, 2),
    ] {
        let mut offset = vec![0; 15];
        offset[13] = 1;
        if let Some(axis) = minus_axis {
            offset[axis] -= 1;
        }
        expected.insert(
            (
                RowId::OrdinaryIbp {
                    contraction_momentum: contraction,
                    differentiated_loop: 0,
                },
                IntegralShift::try_new(offset).unwrap(),
            ),
            c.div_with_limits(
                &c.integer(numerator),
                &c.integer(denominator),
                limits().arithmetic,
            )
            .unwrap(),
        );
    }
    assert_eq!(n.weights, expected);
    assert!(!n.conditions.is_empty());
}

#[test]
fn loop_relabel_and_denominator_permutation_are_equivariant() {
    let original = original_family();
    let generator = ParametricIbpGenerator::try_new(&original).unwrap();
    let n = nominate(&original, generator.context(), &active(), 0, 13, limits()).unwrap();
    let loop_map = [3, 0, 4, 1, 2];
    let axis_map = std::array::from_fn(|i| (i + 7) % 15);
    let permuted = family(&loop_map, &axis_map);
    let generator = ParametricIbpGenerator::try_new(&permuted).unwrap();
    let mut mask = [false; 15];
    for i in 0..15 {
        mask[axis_map[i]] = active()[i];
    }
    let p = nominate(
        &permuted,
        generator.context(),
        &mask,
        loop_map[0],
        axis_map[13],
        limits(),
    )
    .unwrap();
    assert_eq!(n.weights.len(), p.weights.len());
    for ((row, offset), weight) in &n.weights {
        let RowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } = row
        else {
            panic!("non-ordinary nomination");
        };
        let mut permuted_offset = vec![0; 15];
        for i in 0..15 {
            permuted_offset[axis_map[i]] = offset.values()[i];
        }
        let key = (
            RowId::OrdinaryIbp {
                contraction_momentum: loop_map[*contraction_momentum],
                differentiated_loop: loop_map[*differentiated_loop],
            },
            IntegralShift::try_new(permuted_offset).unwrap(),
        );
        assert_eq!(weight.raw(), p.weights[&key].raw());
    }
}

#[test]
fn parametric_geometry_keeps_native_weights_and_pre_cancellation_poles() {
    let c = CoefficientContext::try_new(["d", "a"]).unwrap();
    let a = c.parameter("a").unwrap();
    let a2 = c.try_mul(&a, &a, limits().arithmetic).unwrap();
    let twice_a = c.try_mul(&c.integer(2), &a, limits().arithmetic).unwrap();
    let family = IntegralFamily::new(
        "parameterized-geometry",
        vec!["k".into(), "p".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(c.integer(-1), vec![c.one(), c.zero(), c.zero()]),
            AffineDenominator::new(c.integer(-1), vec![c.zero(), c.zero(), c.one()]),
            AffineDenominator::new(c.integer(-1), vec![c.one(), twice_a, a2]),
        ],
        vec![],
        vec![c.zero(); 3],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let n = nominate(
        &family,
        generator.context(),
        &[true, true, false],
        0,
        2,
        limits(),
    )
    .unwrap();
    let key = (
        RowId::OrdinaryIbp {
            contraction_momentum: 1,
            differentiated_loop: 0,
        },
        IntegralShift::try_new([0, 0, 1]).unwrap(),
    );
    assert_eq!(n.weights[&key], generator.context().lift(&a).unwrap());
    assert!(
        n.conditions.iter().any(|g| !g.is_nonzero_constant()),
        "inverse-basis poles survive even where the affine sum cancels them"
    );
}

#[test]
fn unsupported_incidence_or_allowance_refuses_instead_of_dropping_terms() {
    let family = original_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let c = generator.context();
    let mut mask = active();
    mask[5] = true;
    assert!(nominate(&family, c, &mask, 0, 13, limits()).is_err()); // Two active dependents.
    mask[0] = false;
    assert!(nominate(&family, c, &mask, 0, 13, limits()).is_err()); // Unique but non-radial.
    assert!(nominate(&family, c, &active(), 0, 1, limits()).is_err()); // Spectator, not a numerator direction.
    assert!(nominate(&family, c, &active(), 99, 13, limits()).is_err());
    assert!(
        nominate(
            &family,
            c,
            &active(),
            0,
            13,
            Limits {
                max_sources: 1,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(
        nominate(
            &family,
            c,
            &active(),
            0,
            13,
            Limits {
                max_conditions: 1,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(
        nominate(
            &family,
            c,
            &active(),
            0,
            13,
            Limits {
                max_operations: 1,
                ..limits()
            }
        )
        .is_err()
    );
}

fn product_request() -> Value {
    json!({"chart":{"lower":vec![0;15]},"limits":{
        "max_source_rows":64,"max_complete_source_rows":64,"max_terms":10000,"max_conditions":10000,
        "max_coordinate_cells":10000,"max_cells":64,"max_polynomial_terms":10000,"max_term_operations":100000,"max_exponent":64}})
}

fn full_product(
    family: &IntegralFamily,
) -> (
    BTreeMap<IndexShift, IndexedCoefficient>,
    Vec<IndexedPolynomial>,
) {
    let generator = ParametricIbpGenerator::try_new(family).unwrap();
    let n = nominate(family, generator.context(), &active(), 0, 13, limits()).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let count = prepared.len();
    let rows = (0..count).map(|i| prepared.generate(i)).collect();
    let completed = prepared.complete(rows).unwrap();
    let zero = IntegralShift::try_new([0_i64; 15]).unwrap();
    let inventory = generator
        .translate_selected_completed_source_rows(
            &completed,
            (0..count).map(|i| TranslatedSourceRequest::new(i, zero.clone())),
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let ids: BTreeMap<_, _> = inventory
        .sources()
        .iter()
        .map(|s| {
            (
                s.provenance().source_row().clone(),
                s.provenance().source_ordinal(),
            )
        })
        .collect();
    let weights: BTreeMap<_, _> = n
        .weights
        .into_iter()
        .map(|((row, offset), weight)| (TranslatedSourceRequest::new(ids[&row], offset), weight))
        .collect();
    let sources = generator
        .translate_selected_completed_source_rows(
            &completed,
            weights.keys().cloned(),
            policies(&product_request()).unwrap().translated_sources,
        )
        .unwrap();
    source_product(
        generator.context(),
        &sources,
        &weights,
        n.conditions,
        &product_request(),
    )
    .unwrap()
}

#[test]
fn full_native_product_retains_mixed_derivatives_but_not_spectator_dependence() {
    let family = original_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let c = generator.context();
    let (product, guards) = full_product(&family);
    assert!(!guards.is_empty());
    let mixed: Vec<_> = product.iter().filter(|(s, _)| s.values()[5] > 0).collect();
    assert!(
        !mixed.is_empty(),
        "a second dependent numerator must have full product-rule tails"
    );
    for (_, value) in mixed {
        let (at_zero, _) = c
            .specialize_fixed_indices(value, &[(5, 0)], IndexedAlgebraLimits::default())
            .unwrap();
        let (at_negative, _) = c
            .specialize_fixed_indices(value, &[(5, -2)], IndexedAlgebraLimits::default())
            .unwrap();
        assert!(at_zero.is_zero());
        assert!(!at_negative.is_zero());
    }
    for value in product.values() {
        let (at_zero, _) = c
            .specialize_fixed_indices(value, &[(1, 0)], IndexedAlgebraLimits::default())
            .unwrap();
        let (at_negative, _) = c
            .specialize_fixed_indices(value, &[(1, -3)], IndexedAlgebraLimits::default())
            .unwrap();
        assert_eq!(
            at_zero, at_negative,
            "derivative-independent spectator must not enter the identity"
        );
    }
    // No claim that the mixed free chart descends: only the ordinary full-product
    // identity is inspected here. Native chart proof/export remains a later gate.
}
