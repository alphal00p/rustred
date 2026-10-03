use super::*;
use crate::{geometry_tangent, policies, source_product};
use rustred::{
    algebra::{CoefficientContext, ExactAlgebraLimits, IndexedAlgebraLimits, IndexedPolynomial},
    family::AffineDenominator,
    identity::{
        IndexShift, IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits,
        TranslatedSourceRequest,
    },
};
use serde_json::{Value, json};

fn limits() -> Limits {
    Limits {
        arithmetic: ExactAlgebraLimits::default(),
        max_sources: 64,
        max_conditions: 10_000,
        max_operations: 100_000,
    }
}

// Generic two-loop toy: P=(k-t*q)^2-1, Q=q^2-1, J=(k+q)^2-1.
// It is not selected or fitted from the operational held-out cohort.
fn shifted(t: i64, permute: bool) -> IntegralFamily {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let rows = [[1, -2 * t, t * t], [0, 0, 1], [1, 2, 1]];
    let mut denominators: Vec<_> = (0..3).map(|_| None).collect();
    let axis_map = if permute { [2, 0, 1] } else { [0, 1, 2] };
    for (axis, row) in rows.iter().enumerate() {
        let row = if permute {
            [row[2], row[1], row[0]]
        } else {
            *row
        };
        denominators[axis_map[axis]] = Some(AffineDenominator::new(
            c.integer(-1),
            row.into_iter().map(|x| c.integer(x)).collect(),
        ));
    }
    IntegralFamily::new(
        "shifted-gradient-toy",
        vec!["k".into(), "q".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators.into_iter().map(Option::unwrap).collect(),
        vec![],
        vec![c.zero(); 3],
    )
    .unwrap()
}

fn product_request(n: usize) -> Value {
    json!({"chart":{"lower":vec![0;n]},"limits":{"max_source_rows":64,"max_complete_source_rows":64,
        "max_terms":10000,"max_conditions":10000,"max_coordinate_cells":10000,"max_cells":64,
        "max_polynomial_terms":10000,"max_term_operations":100000,"max_exponent":64}})
}

fn full_product(
    family: &IntegralFamily,
    nomination: Nomination,
) -> (
    BTreeMap<IndexShift, IndexedCoefficient>,
    Vec<IndexedPolynomial>,
) {
    let generator = ParametricIbpGenerator::try_new(family).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let count = prepared.len();
    let generated = (0..count).map(|i| prepared.generate(i)).collect();
    let completed = prepared.complete(generated).unwrap();
    let zero = IntegralShift::try_new(vec![0; family.denominator_count()]).unwrap();
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
    let weights: BTreeMap<_, _> = nomination
        .weights
        .into_iter()
        .map(|((row, offset), weight)| (TranslatedSourceRequest::new(ids[&row], offset), weight))
        .collect();
    let request = product_request(family.denominator_count());
    let sources = generator
        .translate_selected_completed_source_rows(
            &completed,
            weights.keys().cloned(),
            policies(&request).unwrap().translated_sources,
        )
        .unwrap();
    source_product(
        generator.context(),
        &sources,
        &weights,
        nomination.conditions,
        &request,
    )
    .unwrap()
}

// The COMPLETE regenerated product has no dependence on the protected power:
// this checks exact g.V cancellation through ordinary-source product rules,
// not through a separately invented polynomial implementation.
fn assert_protected_derivative_cancels(
    family: &IntegralFamily,
    nomination: Nomination,
    protected: usize,
) {
    let generator = ParametricIbpGenerator::try_new(family).unwrap();
    let c = generator.context();
    let (product, guards) = full_product(family, nomination);
    assert!(!product.is_empty());
    assert!(!guards.is_empty());
    assert!(product.keys().all(|s| s.values()[protected] <= 0));
    for value in product.values() {
        let (one, _) = c
            .specialize_fixed_indices(value, &[(protected, 1)], IndexedAlgebraLimits::default())
            .unwrap();
        let (two, _) = c
            .specialize_fixed_indices(value, &[(protected, 2)], IndexedAlgebraLimits::default())
            .unwrap();
        assert_eq!(
            one, two,
            "protected derivative must vanish in every full-source column"
        );
    }
}

#[test]
fn shifted_mass_uses_gradient_and_full_source_protection() {
    let family = shifted(1, false);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    assert!(
        geometry_tangent::nominate(
            &family,
            generator.context(),
            &[true, true, false],
            0,
            2,
            limits()
        )
        .is_err()
    );
    let n = nominate(
        &family,
        generator.context(),
        &[true, true, false],
        0,
        2,
        limits(),
    )
    .unwrap();
    assert_eq!(n.report["protected_active_axis"], 0);
    assert_eq!(n.report["method"], "one-protected-gradient-affine-v1");
    assert_protected_derivative_cancels(&family, n, 0);
}

#[test]
fn radial_limit_has_eightfold_source_scale_and_same_normalized_full_product() {
    let family = shifted(0, false);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let c = generator.context();
    let old = geometry_tangent::nominate(&family, c, &[true, true, false], 0, 2, limits()).unwrap();
    let new = nominate(&family, c, &[true, true, false], 0, 2, limits()).unwrap();
    assert_eq!(old.weights.len(), new.weights.len());
    for (key, value) in &old.weights {
        assert_eq!(
            new.weights[key],
            c.mul_with_limits(value, &c.integer(8), limits().arithmetic)
                .unwrap()
        );
    }
    let (old, _) = full_product(&family, old);
    let (new, _) = full_product(&family, new);
    let target = old
        .keys()
        .find(|s| s.values().iter().all(|&x| x == 0))
        .unwrap();
    assert_eq!(old.len(), new.len());
    for (shift, value) in &old {
        assert_eq!(
            c.div_with_limits(value, &old[target], limits().arithmetic)
                .unwrap(),
            c.div_with_limits(&new[shift], &new[target], limits().arithmetic)
                .unwrap()
        );
    }
}

#[test]
fn shifted_loop_and_denominator_permutation_are_equivariant() {
    let original = shifted(1, false);
    let permuted = shifted(1, true);
    let g = ParametricIbpGenerator::try_new(&original).unwrap();
    let h = ParametricIbpGenerator::try_new(&permuted).unwrap();
    let n = nominate(&original, g.context(), &[true, true, false], 0, 2, limits()).unwrap();
    let m = nominate(&permuted, h.context(), &[true, false, true], 1, 1, limits()).unwrap();
    let axis_map = [2, 0, 1];
    assert_eq!(n.weights.len(), m.weights.len());
    for ((row, offset), weight) in n.weights {
        let RowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } = row
        else {
            panic!("not ordinary")
        };
        let mut shifted = vec![0; 3];
        for i in 0..3 {
            shifted[axis_map[i]] = offset.values()[i];
        }
        let key = (
            RowId::OrdinaryIbp {
                contraction_momentum: 1 - contraction_momentum,
                differentiated_loop: 1 - differentiated_loop,
            },
            IntegralShift::try_new(shifted).unwrap(),
        );
        assert_eq!(weight.raw(), m.weights[&key].raw());
    }
    assert_protected_derivative_cancels(&permuted, m, 2);
}

#[test]
fn external_gradient_keeps_native_gram_constants_and_indexing() {
    let c = CoefficientContext::try_new(["d", "s"]).unwrap();
    let s = c.parameter("s").unwrap();
    let constant = c.try_sub(&s, &c.one(), limits().arithmetic).unwrap();
    let family = IntegralFamily::new(
        "external-gradient-toy",
        vec!["k".into()],
        vec!["p".into()],
        c.clone(),
        c.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(constant.clone(), vec![c.one(), c.integer(-2)]),
            AffineDenominator::new(constant, vec![c.one(), c.integer(2)]),
        ],
        vec![vec![s]],
        vec![c.zero(); 2],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let n = nominate(&family, generator.context(), &[true, false], 0, 1, limits()).unwrap();
    assert!(n.weights.keys().any(|(r, _)| matches!(
        r,
        RowId::OrdinaryIbp {
            contraction_momentum: 1,
            ..
        }
    )));
    assert_protected_derivative_cancels(&family, n, 0);
}

#[test]
fn parametric_shift_retains_the_canceled_inverse_basis_pole_in_full_product() {
    let c = CoefficientContext::try_new(["d", "a"]).unwrap();
    let a = c.parameter("a").unwrap();
    let a2 = c.try_mul(&a, &a, limits().arithmetic).unwrap();
    let minus_twice_a = c.try_mul(&c.integer(-2), &a, limits().arithmetic).unwrap();
    let family = IntegralFamily::new(
        "parametric-gradient-toy",
        vec!["k".into(), "q".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(c.integer(-1), vec![c.one(), minus_twice_a, a2]),
            AffineDenominator::new(c.integer(-1), vec![c.zero(), c.zero(), c.one()]),
            AffineDenominator::new(c.integer(-1), vec![c.one(), c.integer(2), c.one()]),
        ],
        vec![],
        vec![c.zero(); 3],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let ic = generator.context();
    let n = nominate(&family, ic, &[true, true, false], 0, 2, limits()).unwrap();
    let a_plus_one = ic
        .add_with_limits(&ic.lift(&a).unwrap(), &ic.one(), limits().arithmetic)
        .unwrap();
    let inverse = ic
        .div_with_limits(&ic.one(), &a_plus_one, limits().arithmetic)
        .unwrap();
    let pole = ic
        .denominator_condition_with_limits(&inverse, limits().arithmetic)
        .unwrap();
    assert!(n.conditions.contains(&pole));
    let (product, guards) = full_product(&family, n);
    assert!(!product.is_empty());
    assert!(
        guards.contains(&pole),
        "the explicit a+1 pole must reach the common proof input even if affine weights cancel it"
    );
}

#[test]
fn degeneracy_multiple_actives_and_resource_misses_fail_closed() {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let family = IntegralFamily::new(
        "linear-gradient-degeneracy",
        vec!["k".into(), "q".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(c.zero(), vec![c.zero(), c.one(), c.zero()]),
            AffineDenominator::new(c.integer(-1), vec![c.zero(), c.zero(), c.one()]),
            AffineDenominator::new(c.integer(-1), vec![c.one(), c.zero(), c.zero()]),
        ],
        vec![],
        vec![c.zero(); 3],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    assert!(
        nominate(
            &family,
            generator.context(),
            &[true, true, false],
            0,
            2,
            limits()
        )
        .is_err()
    );
    let family = shifted(1, false);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    assert!(
        nominate(
            &family,
            generator.context(),
            &[true, false, true],
            0,
            1,
            limits()
        )
        .is_err()
    );
    for small in [
        Limits {
            max_operations: 1,
            ..limits()
        },
        Limits {
            max_sources: 1,
            ..limits()
        },
        Limits {
            max_conditions: 1,
            ..limits()
        },
    ] {
        assert!(
            nominate(
                &family,
                generator.context(),
                &[true, true, false],
                0,
                2,
                small
            )
            .is_err()
        );
    }
    let rows = [
        [1, 0, 0, 0, 0, 0],
        [0, 1, 0, 0, 0, 0],
        [0, 0, 1, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [2, 0, 0, 0, 1, 0],
    ];
    let family = IntegralFamily::new(
        "zero-effective-cross-direction",
        vec!["k".into(), "q".into(), "r".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        rows.into_iter()
            .map(|v| {
                AffineDenominator::new(c.integer(-1), v.into_iter().map(|x| c.integer(x)).collect())
            })
            .collect(),
        vec![],
        vec![c.zero(); 6],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    assert!(
        nominate(
            &family,
            generator.context(),
            &[true, false, false, true, true, false],
            0,
            5,
            limits()
        )
        .is_err()
    );
}
