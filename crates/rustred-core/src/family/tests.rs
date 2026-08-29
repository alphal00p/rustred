//! Integral-family unit tests.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};

use super::build::retain_family_name;
use super::exact::{invert_symbolic_matrix, verify_inverse};
use super::*;

fn identity_denominators(context: &CoefficientContext, size: usize) -> Vec<AffineDenominator> {
    (0..size)
        .map(|row| {
            AffineDenominator::new(
                context.zero(),
                (0..size)
                    .map(|column| {
                        if row == column {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect(),
            )
        })
        .collect()
}

fn one_loop_family_from_basis(
    context: &CoefficientContext,
    name: &str,
    basis: Vec<Vec<Coefficient>>,
) -> Result<IntegralFamily, IntegralFamilyError> {
    let size = basis.len();
    assert!(size > 0);
    assert!(basis.iter().all(|row| row.len() == size));
    let external_count = size - 1;
    let external_momenta = (0..external_count)
        .map(|external| format!("p{external}"))
        .collect::<Vec<_>>();
    let external_gram = (0..external_count)
        .map(|row| {
            (0..external_count)
                .map(|column| {
                    if row == column {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let denominators = basis
        .into_iter()
        .map(|row| AffineDenominator::new(context.zero(), row))
        .collect::<Vec<_>>();

    IntegralFamily::new(
        name.to_owned(),
        vec!["k".to_owned()],
        external_momenta,
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        external_gram,
        vec![context.zero(); size],
    )
}

fn upper_bidiagonal_basis(context: &CoefficientContext, size: usize) -> Vec<Vec<Coefficient>> {
    let x = context.parameter("x").unwrap();
    (0..size)
        .map(|row| {
            (0..size)
                .map(|column| {
                    if row == column {
                        context.integer(i64::try_from(row + 2).unwrap())
                    } else if column == row + 1 {
                        x.clone()
                    } else {
                        context.zero()
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn family_name_retention_moves_owned_buffers_and_fallibly_copies_borrowed_names() {
    let owned = String::from("owned-family-name");
    let owned_pointer = owned.as_ptr();
    let retained_owned = retain_family_name(Cow::Owned(owned)).unwrap();
    assert_eq!(retained_owned.as_ptr(), owned_pointer);

    let borrowed = "borrowed-family-name";
    let retained_borrowed = retain_family_name(Cow::Borrowed(borrowed)).unwrap();
    assert_eq!(retained_borrowed, borrowed);
    assert_ne!(retained_borrowed.as_ptr(), borrowed.as_ptr());
}

#[test]
fn proportional_family_limits_and_identity_strings_precede_label_sets() {
    let context = CoefficientContext::new(["d"]);
    let scalar_limit_first = IntegralFamily::new_with_limits(
        "duplicate-loop-labels",
        vec!["k".into(), "k".into()],
        Vec::new(),
        context.clone(),
        context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        IntegralFamilyLimits {
            max_scalar_products: 0,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        scalar_limit_first,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family scalar products",
            requested: 3,
            limit: 0,
        })
    ));

    let oversized_borrowed_name = "borrowed-family-name";
    let name_limit_first = IntegralFamily::new_with_limits(
        oversized_borrowed_name,
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        IntegralFamilyLimits {
            max_fingerprint_bytes: 4,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        name_limit_first,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family fingerprint bytes",
            requested,
            limit: 4,
        }) if requested == oversized_borrowed_name.len()
    ));

    let oversized_label = "loop-label-too-long";
    let label_limit_first = IntegralFamily::new_with_limits(
        "x",
        vec![oversized_label.into()],
        Vec::new(),
        context.clone(),
        context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        IntegralFamilyLimits {
            max_fingerprint_bytes: 4,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        label_limit_first,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family fingerprint string bytes",
            requested,
            limit: 4,
        }) if requested == oversized_label.len()
    ));

    // Symbolica parameter labels are identifiers.  Keep this fixture
    // oversized for the RustRed fingerprint limit without failing in the
    // coefficient-context parser before that boundary is reached.
    let oversized_parameter = "parameter_label_too_long";
    let parameter_context = CoefficientContext::new([oversized_parameter]);
    let parameter_limit_first = IntegralFamily::new_with_limits(
        "x",
        vec!["k".into()],
        Vec::new(),
        parameter_context.clone(),
        parameter_context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        IntegralFamilyLimits {
            max_fingerprint_bytes: 4,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        parameter_limit_first,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family fingerprint string bytes",
            requested,
            limit: 4,
        }) if requested == oversized_parameter.len()
    ));
}

#[test]
fn coordinates_are_loop_loop_then_loop_external() {
    let context = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "two-loop-two-leg",
        vec!["k1".into(), "k2".into()],
        vec!["p1".into(), "p2".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        identity_denominators(&context, 7),
        vec![
            vec![context.one(), context.zero()],
            vec![context.zero(), context.one()],
        ],
        vec![context.zero(); 7],
    )
    .unwrap();

    assert_eq!(
        family.coordinates(),
        &[
            ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
            ScalarProductCoordinate::LoopLoop { left: 0, right: 1 },
            ScalarProductCoordinate::LoopLoop { left: 1, right: 1 },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 0,
                external_index: 0,
            },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 0,
                external_index: 1,
            },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 1,
                external_index: 0,
            },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 1,
                external_index: 1,
            },
        ]
    );
    assert_eq!(
        family.contraction_momenta(),
        &[
            ContractionMomentum::Loop(0),
            ContractionMomentum::Loop(1),
            ContractionMomentum::External(0),
            ContractionMomentum::External(1),
        ]
    );
    assert_eq!(
        family
            .coordinate_index(ScalarProductCoordinate::LoopExternal {
                loop_index: 1,
                external_index: 0,
            })
            .unwrap(),
        5
    );
    family.verify_exact_replay().unwrap();
}

#[test]
fn symbolic_nonsymmetric_basis_has_domain_conditioned_exact_inverse() {
    let context = CoefficientContext::new(["d", "a", "b", "s"]);
    let d = context.parameter("d").unwrap();
    let a_over_s = context.coefficient_fixture("a/s");
    let b = context.parameter("b").unwrap();
    let c0 = context.coefficient_fixture("a+1");
    let c1 = context.coefficient_fixture("b-2");
    let family = IntegralFamily::new(
        "symbolic",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        d,
        vec![
            AffineDenominator::new(c0, vec![a_over_s, context.one()]),
            AffineDenominator::new(c1, vec![b, context.integer(2)]),
        ],
        vec![vec![context.coefficient_fixture("s")]],
        vec![context.coefficient_fixture("a/3"), context.zero()],
    )
    .unwrap();

    assert_eq!(
        family.domain().basis_determinant(),
        &context.coefficient_fixture("(2*a-b*s)/s")
    );
    assert_eq!(
        family.domain().determinant_nonzero().polynomial(),
        &context.coefficient_fixture("2*a-b*s").numerator
    );
    assert!(family.domain().input_denominators().iter().any(|guard| {
        guard
            .sources()
            .contains(&CoefficientLocation::DenominatorCoefficient {
                denominator: 0,
                coordinate: 0,
            })
            && guard.polynomial() == &context.coefficient_fixture("s").numerator
    }));
    assert!(family.domain().input_denominators().iter().any(|guard| {
        guard
            .sources()
            .contains(&CoefficientLocation::PowerShift { denominator: 0 })
            && guard.polynomial() == &context.integer(3).numerator
    }));
    family.verify_exact_replay().unwrap();
}

#[test]
fn symbolica_matrix_backend_preserves_generic_sizes_orientation_and_replay() {
    let context = CoefficientContext::new(["d", "x"]);

    for size in 1..=6 {
        let family = one_loop_family_from_basis(
            &context,
            &format!("upper-bidiagonal-{size}"),
            upper_bidiagonal_basis(&context, size),
        )
        .unwrap();
        let determinant = (2..=size + 1).product::<usize>();
        assert_eq!(
            family.domain().basis_determinant(),
            &context.integer(i64::try_from(determinant).unwrap())
        );
        assert!(
            family
                .inverse_basis()
                .iter()
                .flatten()
                .all(|entry| context.contains(entry))
        );
        if size == 2 {
            assert_eq!(
                family.inverse_basis()[0][1],
                context.coefficient_fixture("-x/6")
            );
            assert_eq!(family.inverse_basis()[1][0], context.zero());
            assert_eq!(
                family.inverse_basis()[0][0],
                context.coefficient_fixture("1/2")
            );
            assert_eq!(
                family.inverse_basis()[1][1],
                context.coefficient_fixture("1/3")
            );
        }
        family.verify_exact_replay().unwrap();
    }
}

#[test]
fn exact_replay_detects_retained_determinant_and_inverse_tampering() {
    let context = CoefficientContext::new(["d", "x"]);
    let family = one_loop_family_from_basis(
        &context,
        "replay-tamper-seam",
        upper_bidiagonal_basis(&context, 4),
    )
    .unwrap();

    let mut determinant_tamper = family.clone();
    determinant_tamper.domain.basis_determinant = context.integer(1);
    assert!(matches!(
        determinant_tamper.verify_exact_replay(),
        Err(IntegralFamilyError::InternalVerificationFailure { detail })
            if detail.contains("native determinant replay")
    ));

    let mut inverse_tamper = family;
    inverse_tamper.inverse_basis[0][0] = context.zero();
    assert!(matches!(
        inverse_tamper.verify_exact_replay(),
        Err(IntegralFamilyError::InternalVerificationFailure { detail })
            if detail.contains("differs from identity")
    ));
}

#[test]
fn symbolica_matrix_backend_rejects_singular_size_one_and_larger_matrices() {
    let context = CoefficientContext::new(["d", "x"]);
    // Cover both of Symbolica's specialized inverse branches (2x2 and
    // 3x3) as well as the augmented-matrix branch used at size 1 and at
    // sizes four and above.
    for size in [1, 2, 3, 4, 6] {
        let mut basis = upper_bidiagonal_basis(&context, size);
        if size == 1 {
            basis[0][0] = context.zero();
        } else {
            basis[size - 1] = basis[size - 2].clone();
        }
        assert!(matches!(
            one_loop_family_from_basis(&context, &format!("singular-{size}"), basis),
            Err(IntegralFamilyError::SingularDenominatorBasis)
        ));
    }
}

#[test]
fn symbolic_size_four_tracks_pivot_sign_rational_determinant_and_sources() {
    let context = CoefficientContext::new(["d", "x", "s", "t"]);
    let zero = context.zero();
    let one = context.one();
    let basis = vec![
        vec![
            zero.clone(),
            context.coefficient_fixture("x/s"),
            zero.clone(),
            zero.clone(),
        ],
        vec![one.clone(), zero.clone(), zero.clone(), zero.clone()],
        vec![
            zero.clone(),
            zero.clone(),
            context.coefficient_fixture("(x+1)/t"),
            one,
        ],
        vec![zero.clone(), zero.clone(), zero, context.integer(2)],
    ];
    let family = one_loop_family_from_basis(&context, "symbolic-size-four", basis).unwrap();
    let determinant = context.coefficient_fixture("-2*x*(x+1)/(s*t)");

    assert_eq!(family.domain().basis_determinant(), &determinant);
    assert_eq!(
        family.domain().determinant_nonzero().polynomial(),
        &determinant.numerator
    );
    assert_eq!(
        family.domain().determinant_nonzero().sources(),
        &BTreeSet::from([CoefficientLocation::BasisDeterminantNumerator])
    );
    for (source, parameter) in [
        (
            CoefficientLocation::DenominatorCoefficient {
                denominator: 0,
                coordinate: 1,
            },
            "s",
        ),
        (
            CoefficientLocation::DenominatorCoefficient {
                denominator: 2,
                coordinate: 2,
            },
            "t",
        ),
    ] {
        let guard = family
            .domain()
            .input_denominators()
            .iter()
            .find(|guard| guard.sources().contains(&source))
            .unwrap();
        assert_eq!(
            guard.polynomial(),
            &context.parameter(parameter).unwrap().numerator
        );
        assert!(guard.sources().contains(&source));
    }
    assert!(
        family
            .inverse_basis()
            .iter()
            .flatten()
            .all(|entry| context.contains(entry))
    );
    family.verify_exact_replay().unwrap();
}

#[test]
fn matrix_boundary_preserves_gmp_coefficients_and_rejects_foreign_maps() {
    let context = CoefficientContext::new(["x"]);
    let mut huge = context.one();
    huge.numerator.coefficients[0] = format!("1{}", "0".repeat(1_500))
        .parse::<Integer>()
        .unwrap();
    let matrix = vec![
        vec![huge.clone(), context.parameter("x").unwrap()],
        vec![context.zero(), context.one()],
    ];
    let (inverse, determinant) =
        invert_symbolic_matrix(&context, &matrix, IntegralFamilyLimits::default()).unwrap();
    assert_eq!(determinant, huge);
    assert!(
        inverse
            .iter()
            .flatten()
            .all(|entry| context.contains(entry))
    );
    verify_inverse(&context, &matrix, &inverse, IntegralFamilyLimits::default()).unwrap();

    let foreign = CoefficientContext::new(["foreign"]);
    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![foreign.one()]],
            IntegralFamilyLimits::default(),
        ),
        Err(IntegralFamilyError::ExactAlgebra(
            ExactAlgebraError::VariableMapMismatch { .. }
        ))
    ));
}

#[test]
fn matrix_boundary_propagates_typed_exact_algebra_limits() {
    let context = CoefficientContext::new(["x"]);
    let x_plus_one = context.coefficient_fixture("x+1");
    let matrix = vec![
        vec![x_plus_one.clone(), context.one()],
        vec![context.one(), x_plus_one],
    ];
    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![context.one()]],
            IntegralFamilyLimits {
                max_matrix_exact_operations: 7,
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ResourceLimit {
            resource: "Symbolica coefficient matrix exact operations",
            requested,
            limit: 7,
        }) if requested > 7
    ));

    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![context.one()]],
            IntegralFamilyLimits {
                max_matrix_input_retained_bytes: 0,
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ResourceLimit {
            resource: "coefficient matrix input retained bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));
    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![context.one()]],
            IntegralFamilyLimits {
                max_matrix_output_retained_bytes: 0,
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &matrix,
            IntegralFamilyLimits {
                exact_algebra: ExactAlgebraLimits {
                    max_term_operations: 1,
                    ..ExactAlgebraLimits::default()
                },
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit { .. }
        ))
    ));

    assert!(matches!(
        invert_symbolic_matrix(
            &context,
            &[vec![context.parameter("x").unwrap()]],
            IntegralFamilyLimits {
                exact_algebra: ExactAlgebraLimits {
                    max_exponent: 0,
                    ..ExactAlgebraLimits::default()
                },
                ..IntegralFamilyLimits::default()
            },
        ),
        Err(IntegralFamilyError::ExactAlgebra(
            ExactAlgebraError::ExponentLimit { .. }
        ))
    ));
}

#[test]
fn equal_family_input_denominators_merge_all_sources() {
    let context = CoefficientContext::new(["d", "m", "a", "nu", "s"]);
    let family = IntegralFamily::new(
        "merged-input-denominators",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.coefficient_fixture("d/s"),
        vec![AffineDenominator::new(
            context.coefficient_fixture("m/s"),
            vec![context.coefficient_fixture("a/s")],
        )],
        Vec::new(),
        vec![context.coefficient_fixture("nu/s")],
    )
    .unwrap();

    assert_eq!(family.domain().input_denominators().len(), 1);
    let condition = &family.domain().input_denominators()[0];
    assert_eq!(
        condition.polynomial(),
        &context.parameter("s").unwrap().numerator
    );
    let expected_sources = BTreeSet::from([
        CoefficientLocation::Dimension,
        CoefficientLocation::DenominatorConstant { denominator: 0 },
        CoefficientLocation::DenominatorCoefficient {
            denominator: 0,
            coordinate: 0,
        },
        CoefficientLocation::PowerShift { denominator: 0 },
    ]);
    assert_eq!(condition.sources(), &expected_sources);
}

#[test]
fn external_derivative_contractions_include_gram_constants() {
    let context = CoefficientContext::new(["d", "m2", "c", "s", "nu"]);
    let m2 = context.parameter("m2").unwrap();
    let c = context.parameter("c").unwrap();
    let s = context.parameter("s").unwrap();
    let family = IntegralFamily::new(
        "one-loop-one-leg",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(m2.clone(), vec![context.one(), context.zero()]),
            AffineDenominator::new(c.clone(), vec![context.zero(), context.one()]),
        ],
        vec![vec![s.clone()]],
        vec![context.parameter("nu").unwrap(), context.zero()],
    )
    .unwrap();

    let k_d0 = family
        .derivative_contraction(0, 0, ContractionMomentum::Loop(0))
        .unwrap();
    assert_eq!(k_d0.constant(), &(-(&context.integer(2) * &m2)));
    assert_eq!(
        k_d0.denominator_coefficients(),
        &[context.integer(2), context.zero()]
    );

    let p_d0 = family
        .derivative_contraction(0, 0, ContractionMomentum::External(0))
        .unwrap();
    assert_eq!(p_d0.constant(), &(-(&context.integer(2) * &c)));
    assert_eq!(
        p_d0.denominator_coefficients(),
        &[context.zero(), context.integer(2)]
    );

    let p_d1 = family
        .derivative_contraction(1, 0, ContractionMomentum::External(0))
        .unwrap();
    assert_eq!(p_d1.constant(), &s);
    assert_eq!(
        p_d1.denominator_coefficients(),
        &[context.zero(), context.zero()]
    );
    family.verify_exact_replay().unwrap();
}

#[test]
fn validates_labels_gram_arities_and_contexts() {
    let context = CoefficientContext::new(["d"]);
    let result = IntegralFamily::new(
        "none",
        Vec::new(),
        Vec::new(),
        context.clone(),
        context.one(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert!(matches!(result, Err(IntegralFamilyError::NoLoopMomenta)));

    let result = IntegralFamily::new(
        "overlap",
        vec!["q".into()],
        vec!["q".into()],
        context.clone(),
        context.one(),
        identity_denominators(&context, 2),
        vec![vec![context.one()]],
        vec![context.zero(); 2],
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::MomentumLabelOverlap { .. })
    ));

    let result = IntegralFamily::new(
        "wrong-denominator-count",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.one(),
        identity_denominators(&context, 1),
        vec![vec![context.one()]],
        vec![context.zero(); 2],
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::WrongDenominatorCount {
            expected: 2,
            actual: 1
        })
    ));

    let result = IntegralFamily::new(
        "wrong-power-shift-count",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.one(),
        identity_denominators(&context, 1),
        Vec::new(),
        Vec::new(),
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::WrongPowerShiftCount {
            expected: 1,
            actual: 0
        })
    ));

    let result = IntegralFamily::new(
        "bad-gram",
        vec!["k".into()],
        vec!["p".into(), "q".into()],
        context.clone(),
        context.one(),
        identity_denominators(&context, 3),
        vec![
            vec![context.one(), context.one()],
            vec![context.zero(), context.one()],
        ],
        vec![context.zero(); 3],
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::AsymmetricExternalGram { row: 0, column: 1 })
    ));

    let foreign = CoefficientContext::new(["x"]);
    let result = IntegralFamily::new(
        "foreign",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        foreign.one(),
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
    );
    assert!(matches!(
        result,
        Err(IntegralFamilyError::ForeignCoefficientContext {
            location: CoefficientLocation::Dimension
        })
    ));
}

#[test]
fn singular_symbolic_basis_is_rejected_but_singular_external_gram_is_allowed() {
    let context = CoefficientContext::new(["d"]);
    let singular = IntegralFamily::new(
        "singular",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.one(),
        vec![
            AffineDenominator::new(context.zero(), vec![context.one(), context.integer(2)]),
            AffineDenominator::new(context.zero(), vec![context.integer(2), context.integer(4)]),
        ],
        vec![vec![context.zero()]],
        vec![context.zero(); 2],
    );
    assert!(matches!(
        singular,
        Err(IntegralFamilyError::SingularDenominatorBasis)
    ));

    let valid = IntegralFamily::new(
        "null-external",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.one(),
        identity_denominators(&context, 2),
        vec![vec![context.zero()]],
        vec![context.zero(); 2],
    )
    .unwrap();
    valid.verify_exact_replay().unwrap();
}

#[test]
fn rational_base_field_without_parameters_is_supported() {
    let context = CoefficientContext::new(Vec::<String>::new());
    let family = IntegralFamily::new(
        "rational",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.integer(4),
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap();
    assert!(family.coefficient_context().parameter_names().is_empty());
    family.verify_exact_replay().unwrap();
}

#[test]
fn family_authentication_rejects_malformed_coefficients_and_resource_limits() {
    let context = CoefficientContext::new(["x"]);
    let mut malformed_dimension = context.one();
    malformed_dimension.numerator.exponents.push(0);
    let malformed = IntegralFamily::new(
        "malformed",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        malformed_dimension,
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
    );
    assert!(matches!(
        malformed,
        Err(IntegralFamilyError::InvalidCoefficient {
            location: CoefficientLocation::Dimension,
            error: ExactAlgebraError::MalformedExponentLayout { .. },
        })
    ));

    let limited = IntegralFamily::new_with_limits(
        "limited",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.one(),
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
        IntegralFamilyLimits {
            max_scalar_products: 0,
            ..IntegralFamilyLimits::default()
        },
    );
    assert!(matches!(
        limited,
        Err(IntegralFamilyError::ResourceLimit {
            resource: "family scalar products",
            requested: 1,
            limit: 0,
        })
    ));
}

fn huge_gmp_fingerprint_family(
    limits: IntegralFamilyLimits,
) -> Result<IntegralFamily, IntegralFamilyError> {
    let context = CoefficientContext::new(["x"]);
    let decimal = format!("1{}", "0".repeat(1_500));
    let magnitude = decimal.parse::<Integer>().unwrap();
    let mut dimension = context.parameter("x").unwrap();
    dimension.numerator.coefficients[0] = -magnitude;
    IntegralFamily::new_with_limits(
        "huge-gmp-fingerprint",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        dimension,
        identity_denominators(&context, 1),
        Vec::new(),
        vec![context.zero()],
        limits,
    )
}

#[test]
fn typed_fingerprint_preflights_exact_and_one_below_huge_gmp_payloads() {
    let family = huge_gmp_fingerprint_family(IntegralFamilyLimits::default()).unwrap();
    let stats = family.fingerprint_stats();
    assert_eq!(stats.encoded_bytes(), family.fingerprint_ref().len());
    assert!(stats.integer_bits() > 4_000);
    assert!(family.fingerprint_ref().contains("I-"));
    let cloned = family.clone();
    assert!(Arc::ptr_eq(&family.fingerprint, &cloned.fingerprint));

    let mut exact = IntegralFamilyLimits::default();
    exact.max_fingerprint_bytes = stats.encoded_bytes();
    exact.max_fingerprint_encoding_work = stats.encoding_work();
    exact.max_fingerprint_polynomial_terms = stats.polynomial_terms();
    exact.max_fingerprint_exponent_entries = stats.exponent_entries();
    exact.max_fingerprint_integer_bits = stats.integer_bits();
    let rebuilt = huge_gmp_fingerprint_family(exact).unwrap();
    assert_eq!(rebuilt.fingerprint_ref(), family.fingerprint_ref());
    assert_eq!(rebuilt.fingerprint_stats(), stats);

    macro_rules! one_below {
        ($field:ident, $getter:ident, $resource:literal) => {{
            let requested = stats.$getter();
            assert!(requested > 0, $resource);
            let mut limits = IntegralFamilyLimits::default();
            limits.$field = requested - 1;
            assert!(matches!(
                huge_gmp_fingerprint_family(limits),
                Err(IntegralFamilyError::ResourceLimit {
                    resource: actual,
                    requested: actual_requested,
                    limit,
                }) if actual == $resource
                    && actual_requested == requested
                    && limit == requested - 1
            ));
        }};
    }
    one_below!(
        max_fingerprint_bytes,
        encoded_bytes,
        "family fingerprint bytes"
    );
    one_below!(
        max_fingerprint_encoding_work,
        encoding_work,
        "family fingerprint encoding work"
    );
    one_below!(
        max_fingerprint_polynomial_terms,
        polynomial_terms,
        "family fingerprint polynomial terms"
    );
    one_below!(
        max_fingerprint_exponent_entries,
        exponent_entries,
        "family fingerprint exponent entries"
    );
    one_below!(
        max_fingerprint_integer_bits,
        integer_bits,
        "family fingerprint integer bits"
    );
}
