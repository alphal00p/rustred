//! Independent differential fixtures: the expected identities below come
//! directly from contracting derivatives, not from the cut-preparation code.

use std::collections::BTreeMap;

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::family::AffineDenominator;
use crate::solver::PolynomialRow;

use super::*;

#[derive(Clone, Copy)]
struct Toy<'a> {
    scale: &'a str,
    offset: &'a str,
    self_gram: &'a str,
    cross_gram: &'a str,
    mixed_cut: bool,
    shifted_axis: Option<usize>,
}

impl Default for Toy<'_> {
    fn default() -> Self {
        Self {
            scale: "1",
            offset: "0",
            self_gram: "1",
            cross_gram: "gamma",
            mixed_cut: false,
            shifted_axis: None,
        }
    }
}

/// Coordinates are [k², k.u, k.v], with D0=c k.u, D1=k.v, D2=-k².
fn toy(spec: Toy<'_>) -> IntegralFamily {
    let base = CoefficientContext::new(["d", "c", "g", "gamma", "nu"]);
    // Use the context's declared symbol directly: unqualified `gamma` is
    // also a native built-in function name in the general Atom parser.
    let value = |expression: &str| {
        base.parameter(expression)
            .unwrap_or_else(|| base.integer(expression.parse::<i64>().unwrap()))
    };
    let mut shifts = vec![base.zero(); 3];
    if let Some(axis) = spec.shifted_axis {
        shifts[axis] = value("nu");
    }
    IntegralFamily::new(
        "linear-cut-direct-derivative-fixture",
        vec!["k".into()],
        vec!["u".into(), "v".into()],
        base.clone(),
        value("d"),
        vec![
            AffineDenominator::new(
                value(spec.offset),
                vec![
                    base.zero(),
                    value(spec.scale),
                    if spec.mixed_cut {
                        base.one()
                    } else {
                        base.zero()
                    },
                ],
            ),
            AffineDenominator::new(base.zero(), vec![base.zero(), base.zero(), base.one()]),
            AffineDenominator::new(base.zero(), vec![value("-1"), base.zero(), base.zero()]),
        ],
        vec![
            vec![value(spec.self_gram), value(spec.cross_gram)],
            vec![value(spec.cross_gram), base.one()],
        ],
        shifts,
    )
    .unwrap()
}

fn variable(template: &CoefficientPolynomial, position: usize) -> Coefficient {
    template
        .variable(&template.variables()[position])
        .unwrap()
        .into()
}

fn integer(template: &CoefficientPolynomial, value: i64) -> Coefficient {
    template.constant(Integer::from(value)).into()
}

fn fixed_integral(first: i16, shifts: [i16; 2]) -> Integral<3> {
    Integral::new([
        Power::new(false, first).unwrap(),
        Power::new(true, shifts[0]).unwrap(),
        Power::new(true, shifts[1]).unwrap(),
    ])
}

/// Source clearing may multiply an entire identity by a parameter polynomial.
/// Compare its exact native-rational coefficients modulo that one common
/// nonzero scale, without numerical evaluation or a homemade CAS normalizer.
fn proportional(actual: &PolynomialRow<3>, expected: &BTreeMap<Integral<3>, Coefficient>) -> bool {
    let actual: BTreeMap<_, Coefficient> = actual
        .iter()
        .map(|term| (term.integral, term.coefficient.clone().into()))
        .collect();
    if actual.keys().ne(expected.keys()) || actual.is_empty() {
        return false;
    }
    let scale = actual.first_key_value().unwrap().1 / expected.first_key_value().unwrap().1;
    !scale.is_zero()
        && actual
            .iter()
            .all(|(key, value)| value == &(&expected[key] * &scale))
}

#[test]
fn canonical_cut_rule_matches_independent_derivative_with_scaled_cut_and_gram() {
    for scaled in [false, true] {
        let family = toy(Toy {
            scale: if scaled { "c" } else { "1" },
            self_gram: if scaled { "g" } else { "1" },
            ..Toy::default()
        });
        let preparation = prepare_linear_cuts::<3>(&family, [true, false, false], true).unwrap();
        assert_eq!(preparation.rules.len(), 1);
        let rule = &preparation.rules[0];
        assert_eq!(rule.axis, 0);
        // Reference ordinary-source order is v, u, k, then its LI source.
        assert_eq!(rule.source_ordinal, 1);
        assert_eq!(rule.target, Integral::symbolic([0; 3]).unwrap());
        let template = &rule.rhs[0].coefficient.numerator;
        let indices = preparation.sources.index_variables();
        let [n0, n1, n2] = indices.map(|position| variable(template, position));
        let one = integer(template, 1);
        let two = integer(template, 2);
        let gamma = variable(template, 3);
        let c = if scaled {
            variable(template, 1)
        } else {
            one.clone()
        };
        let g = if scaled {
            variable(template, 2)
        } else {
            one.clone()
        };
        let pole = &n0 - &one;
        let expected = BTreeMap::from([
            (
                Integral::symbolic([-1, 1, 0]).unwrap(),
                -(&(&gamma * &n1) / &(&(&c * &g) * &pole)),
            ),
            (
                Integral::symbolic([-2, 0, 1]).unwrap(),
                &(&two * &n2) / &(&(&(&c * &c) * &g) * &pole),
            ),
        ]);
        let actual: BTreeMap<_, _> = rule
            .rhs
            .iter()
            .map(|term| (term.integral, term.coefficient.clone()))
            .collect();
        assert_eq!(actual, expected);
        for term in &rule.rhs {
            assert!(
                term.coefficient
                    .denominator
                    .replace(indices[0], &Integer::from(1))
                    .is_zero()
            );
            assert!(
                !term
                    .coefficient
                    .denominator
                    .replace(indices[0], &Integer::from(0))
                    .is_zero()
            );
        }
    }
}

#[test]
fn fixed_frame_matches_direct_v_and_loop_identities_and_annuls_u_and_li_rows() {
    for scaled in [false, true] {
        let family = toy(Toy {
            scale: if scaled { "c" } else { "1" },
            self_gram: if scaled { "g" } else { "1" },
            ..Toy::default()
        });
        let ordinary = prepare_linear_cuts::<3>(&family, [true, false, false], false).unwrap();
        let all = prepare_linear_cuts::<3>(&family, [true, false, false], true).unwrap();
        assert_eq!(ordinary.sources.rows(), all.sources.rows());
        assert_eq!(all.sources.rows().len(), 2);
        assert_eq!(all.sources.fixed(), &[Some(1), None, None]);
        let template = &all.sources.rows()[0][0].coefficient;
        let [i0, i1, i2] = *all.sources.index_variables();
        let n1 = variable(template, i1);
        let n2 = variable(template, i2);
        let d = variable(template, 0);
        let gamma = variable(template, 3);
        let one = integer(template, 1);
        let two = integer(template, 2);
        let g = if scaled {
            variable(template, 2)
        } else {
            one.clone()
        };
        let expected_v = BTreeMap::from([
            (
                fixed_integral(1, [1, 0]),
                &(&(&(&gamma * &gamma) / &g) - &one) * &n1,
            ),
            (fixed_integral(1, [-1, 1]), &two * &n2),
        ]);
        let expected_loop = BTreeMap::from([(
            fixed_integral(1, [0, 0]),
            &(&(&d - &one) - &n1) - &(&two * &n2),
        )]);
        assert!(proportional(&all.sources.rows()[0], &expected_v));
        assert!(proportional(&all.sources.rows()[1], &expected_loop));
        for term in all.sources.rows().iter().flatten() {
            assert_eq!(term.integral[0], Power::new(false, 1).unwrap());
            assert!(!term.coefficient.contains(i0));
        }
    }
}

#[test]
fn pivot_parameter_guards_survive_cancellation_but_do_not_exclude_fixed_cut_one() {
    let preparation = prepare_linear_cuts::<3>(
        &toy(Toy {
            scale: "c",
            self_gram: "g",
            ..Toy::default()
        }),
        [true, false, false],
        true,
    )
    .unwrap();
    let conditions = preparation.sources.conditions();
    assert!(!conditions.is_empty());
    // c is needed for the original denominator chart; g is additionally
    // needed to divide the cut pivot, even though it cancels from the loop row.
    for parameter in [1, 2] {
        assert!(
            conditions
                .iter()
                .any(|condition| { condition.replace(parameter, &Integer::from(0)).is_zero() })
        );
    }
    for condition in conditions {
        assert!(!condition.is_zero());
        for index in preparation.sources.index_variables() {
            assert!(!condition.contains(*index));
            assert!(!condition.replace(*index, &Integer::from(1)).is_zero());
        }
    }
}

#[test]
fn independent_cuts_are_removed_once_without_reintroducing_each_other() {
    let preparation = prepare_linear_cuts::<3>(
        &toy(Toy {
            cross_gram: "0",
            ..Toy::default()
        }),
        [true, true, false],
        true,
    )
    .unwrap();
    assert_eq!(preparation.rules.len(), 2);
    for rule in &preparation.rules {
        assert_eq!(rule.rhs.len(), 1);
        for term in &rule.rhs {
            assert!(term.integral[rule.axis].value() < 0);
            assert!(term.integral[0].value() <= 0 && term.integral[1].value() <= 0);
        }
    }
    assert_eq!(preparation.sources.fixed(), &[Some(1), Some(1), None]);
    assert_eq!(preparation.sources.rows().len(), 1);
    let template = &preparation.sources.rows()[0][0].coefficient;
    let n2 = variable(template, preparation.sources.index_variables()[2]);
    let d = variable(template, 0);
    let two = integer(template, 2);
    let integral = Integral::new([
        Power::new(false, 1).unwrap(),
        Power::new(false, 1).unwrap(),
        Power::new(true, 0).unwrap(),
    ]);
    assert!(proportional(
        &preparation.sources.rows()[0],
        &BTreeMap::from([(integral, &(&d - &two) - &(&two * &n2)),])
    ));
}

#[test]
fn unsupported_cut_geometries_fail_explicitly_instead_of_selecting_another_pivot() {
    let examples = [
        (Toy::default(), [true, true, false], 0),
        (
            Toy {
                self_gram: "0",
                ..Toy::default()
            },
            [true, false, false],
            0,
        ),
        (
            Toy {
                offset: "1",
                ..Toy::default()
            },
            [true, false, false],
            0,
        ),
        (
            Toy {
                mixed_cut: true,
                ..Toy::default()
            },
            [true, false, false],
            0,
        ),
        (Toy::default(), [false, false, true], 2),
    ];
    for (spec, removed, expected_axis) in examples {
        assert!(matches!(
            prepare_linear_cuts::<3>(&toy(spec), removed, true),
            Err(LinearCutError::Unsupported { axis, .. }) if axis == expected_axis
        ));
    }
    assert!(matches!(
        prepare_linear_cuts::<2>(&toy(Toy::default()), [false; 2], true),
        Err(LinearCutError::Source(SolverError::InvalidInput(_)))
    ));
}

#[test]
fn shifted_powers_in_cut_mode_are_explicitly_deferred_under_current_reference_policy() {
    for axis in 0..3 {
        assert!(matches!(
            prepare_linear_cuts::<3>(
                &toy(Toy { shifted_axis: Some(axis), ..Toy::default() }),
                [true, false, false],
                true,
            ),
            Err(LinearCutError::Unsupported { reason, .. })
                if reason.contains("noninteger")
        ));
    }
}

#[test]
fn empty_cut_mask_preserves_original_sources_including_noninteger_offsets() {
    for include_lorentz in [false, true] {
        let family = toy(Toy {
            shifted_axis: Some(1),
            ..Toy::default()
        });
        let original =
            SourceSystem::<3>::from_family_with_lorentz(&family, include_lorentz).unwrap();
        let preparation = prepare_linear_cuts::<3>(&family, [false; 3], include_lorentz).unwrap();
        assert!(preparation.rules.is_empty());
        assert_eq!(preparation.sources.rows(), original.rows());
        assert_eq!(preparation.sources.conditions(), original.conditions());
        assert_eq!(preparation.sources.fixed(), &[None; 3]);
    }
}
