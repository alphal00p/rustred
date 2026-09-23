use symbolica::domains::integer::Integer;

use super::*;
use crate::algebra::indexed::specialization::fixed_index_execution_order;

#[test]
fn fixed_execution_eliminates_zero_terms_before_nonzero_powers() {
    assert_eq!(
        fixed_index_execution_order(&[(0, 0), (1, i64::MAX), (2, 0), (3, -2)]).collect::<Vec<_>>(),
        [(2, 0), (0, 0), (3, -2), (1, i64::MAX)]
    );
    assert_eq!(
        fixed_index_execution_order(&[(0, i64::MAX), (1, 0)]).collect::<Vec<_>>(),
        [(1, 0), (0, i64::MAX)]
    );
    assert_eq!(fixed_index_execution_order(&[]).count(), 0);
}

#[test]
fn fixed_native_execution_matches_ascending_symbolica_substitutions() {
    let base = CoefficientContext::new(["d", "mass_squared"]);
    let context = IndexedCoefficientContext::try_new(&base, "fixed-execution-order", 4).unwrap();
    let base_count = base.variables().len();
    let mut polynomial = context.one().raw.numerator.zero();
    // Two symbolic base parameters, four index variables, sparse/dense mixed
    // terms, and colliding monomials exercise collection and cancellation.
    for seed in 0..96_usize {
        let mut exponents = vec![0_u16; context.variables.len()];
        exponents[0] = (seed % 3) as u16;
        exponents[1] = ((seed / 3) % 2) as u16;
        for index in 0..4 {
            exponents[base_count + index] = ((seed / (index + 1) + index) % 4) as u16;
        }
        polynomial.append_monomial(Integer::from((seed % 11) as i64 - 5), &exponents);
    }
    let mut absent = polynomial.clone();
    absent = absent.replace(base_count + 1, &Integer::from(1));
    let polynomials = [
        polynomial,
        absent,
        context.one().raw.numerator,
        context.zero().raw.numerator,
    ];
    for polynomial in polynomials {
        let source = IndexedPolynomial {
            raw: polynomial,
            context: context.fingerprint.clone(),
        };
        for mask in 0..16 {
            for values in [[0, -2, 1, 3], [2, 0, -1, 1], [-1, 1, 0, -2]] {
                let fixed: Vec<_> = (0..4)
                    .filter(|index| mask & (1 << index) != 0)
                    .map(|index| (index, values[index]))
                    .collect();
                let mut expected = source.raw.clone();
                for &(index, value) in &fixed {
                    expected = expected.replace(base_count + index, &Integer::from(value));
                }
                let actual = context
                    .specialize_fixed_polynomial(&source, &fixed, Default::default())
                    .unwrap();
                assert_eq!(actual.raw, expected, "mask={mask}, values={values:?}");
                assert_eq!(actual.raw.variables(), &context.variables);
            }
        }
    }
}

#[test]
fn fixed_zero_annihilation_respects_tiny_integer_budget_in_both_orders() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "fixed-zero-first", 2).unwrap();
    let base_count = base.variables().len();
    for zero_index in 0..2 {
        let power_index = 1 - zero_index;
        let mut polynomial = context.one().raw.numerator.zero();
        let mut exponents = vec![0_u16; context.variables.len()];
        exponents[base_count + zero_index] = 1;
        exponents[base_count + power_index] = u16::MAX;
        polynomial.append_monomial(Integer::from(1), &exponents);
        let source = IndexedPolynomial {
            raw: polynomial,
            context: context.fingerprint.clone(),
        };
        let mut fixed = [(zero_index, 0), (power_index, i64::MAX)];
        fixed.sort_unstable_by_key(|(index, _)| *index);
        let limits = IndexedAlgebraLimits {
            max_specialization_integer_bits: 0,
            ..Default::default()
        };
        let result = context
            .specialize_fixed_polynomial(&source, &fixed, limits)
            .unwrap();
        assert!(result.raw.is_zero());
        assert_eq!(result.raw.variables(), &context.variables);
    }
}

#[test]
fn fixed_zero_numerator_does_not_hide_a_zero_denominator() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "fixed-zero-guard", 2).unwrap();
    let value = context
        .div(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    assert_eq!(
        context.specialize_fixed_indices(&value, &[(0, 0), (1, 0)], Default::default()),
        Err(IndexedAlgebraError::ZeroDenominator)
    );
}

#[test]
fn fixed_i64_min_uses_the_unsigned_magnitude_preflight() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "fixed-i64-min", 2).unwrap();
    let value = context.index(0).unwrap();
    let exact = IndexedAlgebraLimits {
        max_specialization_integer_bits: 65,
        ..Default::default()
    };
    for sealed in [false, true] {
        let specialize = |limits| {
            if sealed {
                context.specialize_fixed_indices_sealed(&value, &[(0, i64::MIN)], limits)
            } else {
                context.specialize_fixed_indices(&value, &[(0, i64::MIN)], limits)
            }
        };
        let (result, guard) = specialize(exact).unwrap();
        assert_eq!(result, context.integer(i64::MIN));
        assert!(guard.raw.is_one());
        assert_eq!(
            specialize(IndexedAlgebraLimits {
                max_specialization_integer_bits: 64,
                ..exact
            }),
            Err(IndexedAlgebraError::ResourceLimit {
                resource: "fixed-index specialization integer bits",
                requested: 65,
                limit: 64,
            })
        );
    }
}

#[test]
fn absent_fixed_variables_do_not_bypass_prospective_limits() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "fixed-absent-limits", 2).unwrap();
    let source = IndexedPolynomial {
        raw: context.index(0).unwrap().raw.numerator,
        context: context.fingerprint.clone(),
    };
    for value in [0, i64::MIN] {
        for sealed in [false, true] {
            let specialize = |limits| {
                if sealed {
                    context.specialize_fixed_polynomial_sealed(&source, &[(1, value)], limits)
                } else {
                    context.specialize_fixed_polynomial(&source, &[(1, value)], limits)
                }
            };
            assert_eq!(specialize(Default::default()).unwrap(), source);
            assert_eq!(
                specialize(IndexedAlgebraLimits {
                    max_specialization_power_operations: 0,
                    ..Default::default()
                }),
                Err(IndexedAlgebraError::ResourceLimit {
                    resource: "fixed-index specialization power operations",
                    requested: 1,
                    limit: 0,
                })
            );
            assert_eq!(
                specialize(IndexedAlgebraLimits {
                    max_specialization_integer_bits: 0,
                    ..Default::default()
                }),
                Err(IndexedAlgebraError::ResourceLimit {
                    resource: "fixed-index specialization integer bits",
                    requested: 1,
                    limit: 0,
                })
            );
        }
    }
}
