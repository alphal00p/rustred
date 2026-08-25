use rustred::symmetry::SymmetryGuardOrigin;
use rustred::{
    AFFINE_FAMILY_MAP_V2_SCHEMA, AffineDenominator, Coefficient, CoefficientContext,
    DenominatorRowAction, ExactAlgebraError, ExactAlgebraLimits, ExactMatrix, GuardOrigin,
    IntegralFamily, JacobianWitness, MomentumMap, ScalarProductCoordinate,
    SymmetryVerificationError, SymmetryVerificationLimits, VerifiedAffineFamilyMap,
    verify_affine_family_map,
};

fn affine(
    constant: Coefficient,
    coefficients: impl IntoIterator<Item = Coefficient>,
) -> AffineDenominator {
    AffineDenominator::new(constant, coefficients.into_iter().collect())
}

fn vacuum_two_loop_family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let m2 = coefficients.parameter("m2").unwrap();
    IntegralFamily::new(
        "symmetry-two-loop-vacuum",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                m2.clone(),
                [coefficients.one(), coefficients.zero(), coefficients.zero()],
            ),
            affine(
                m2.clone(),
                [coefficients.zero(), coefficients.zero(), coefficients.one()],
            ),
            affine(
                m2,
                [
                    coefficients.one(),
                    coefficients.integer(-2),
                    coefficients.one(),
                ],
            ),
        ],
        Vec::new(),
        vec![coefficients.zero(); 3],
    )
    .unwrap()
}

fn vacuum_identity_family(
    name: &str,
    context: &CoefficientContext,
    loops: usize,
) -> IntegralFamily {
    let scalar_products = loops * (loops + 1) / 2;
    let denominators = (0..scalar_products)
        .map(|row| {
            affine(
                context.zero(),
                (0..scalar_products).map(|column| {
                    if row == column {
                        context.one()
                    } else {
                        context.zero()
                    }
                }),
            )
        })
        .collect::<Vec<_>>();
    IntegralFamily::new(
        name,
        (0..loops)
            .map(|loop_index| format!("k{loop_index}"))
            .collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![context.zero(); scalar_products],
    )
    .unwrap()
}

fn one_loop_external_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "s"]);
    let s = coefficients.parameter("s").unwrap();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                coefficients.zero(),
                [coefficients.one(), coefficients.zero()],
            ),
            affine(s.clone(), [coefficients.one(), coefficients.integer(2)]),
        ],
        vec![vec![s]],
        vec![coefficients.zero(); 2],
    )
    .unwrap()
}

fn singular_gram_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "x"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                coefficients.zero(),
                [coefficients.one(), coefficients.zero()],
            ),
            affine(
                coefficients.zero(),
                [coefficients.zero(), coefficients.one()],
            ),
        ],
        vec![vec![coefficients.zero()]],
        vec![coefficients.zero(); 2],
    )
    .unwrap()
}

fn singular_gram_map(family: &IntegralFamily, external_scale: Coefficient) -> MomentumMap {
    let context = family.coefficient_context();
    MomentumMap::new(
        ExactMatrix::try_new(1, 1, [context.one()]).unwrap(),
        ExactMatrix::try_new(1, 1, [context.zero()]).unwrap(),
        ExactMatrix::try_new(1, 1, [external_scale]).unwrap(),
    )
}

fn dense_two_loop_external_family(
    name: &str,
    context: &CoefficientContext,
    gram: [[i64; 2]; 2],
    constants: [i64; 7],
) -> IntegralFamily {
    let mut denominators = Vec::new();
    for (coordinate, constant) in constants.into_iter().enumerate() {
        let coefficients: Vec<_> = (0..7)
            .map(|candidate| {
                if candidate == coordinate {
                    context.one()
                } else {
                    context.zero()
                }
            })
            .collect();
        denominators.push(affine(context.integer(constant), coefficients));
    }
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into()],
        vec!["p0".into(), "p1".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        gram.map(|row| row.map(|value| context.integer(value)).to_vec())
            .to_vec(),
        vec![context.zero(); 7],
    )
    .unwrap()
}

#[test]
fn verifies_nontrivial_two_loop_denominator_permutation() {
    let family = vacuum_two_loop_family();
    let context = family.coefficient_context();
    // k0_source = k1_target, k1_source = k0_target.  This exchanges D0/D1
    // and fixes D2, with det(A)=-1.
    let momentum = MomentumMap::new(
        ExactMatrix::try_new(
            2,
            2,
            [context.zero(), context.one(), context.one(), context.zero()],
        )
        .unwrap(),
        ExactMatrix::try_new(2, 0, []).unwrap(),
        ExactMatrix::try_new(0, 0, []).unwrap(),
    );
    let verified = verify_affine_family_map(
        &family,
        &family,
        momentum,
        SymmetryVerificationLimits::default(),
    )
    .unwrap();
    assert_eq!(
        verified.jacobian(),
        &JacobianWitness::Unit {
            determinant_sign: -1
        }
    );
    assert_eq!(
        verified.row_actions(),
        &[
            DenominatorRowAction::Monomial {
                target: 1,
                scale: context.one(),
            },
            DenominatorRowAction::Monomial {
                target: 0,
                scale: context.one(),
            },
            DenominatorRowAction::Monomial {
                target: 2,
                scale: context.one(),
            },
        ]
    );
    assert_eq!(verified.source_domain(), family.domain());
    assert_eq!(verified.target_domain(), family.domain());
    verified
        .replay(&family, &family, SymmetryVerificationLimits::default())
        .unwrap();
}

#[test]
fn derives_external_shift_and_affine_denominator_action() {
    let source = one_loop_external_family("source-bubble");
    let target = one_loop_external_family("target-bubble");
    let context = source.coefficient_context();
    // k_source = k_target + p_target, p_source = p_target.
    let momentum = MomentumMap::new(
        ExactMatrix::try_new(1, 1, [context.one()]).unwrap(),
        ExactMatrix::try_new(1, 1, [context.one()]).unwrap(),
        ExactMatrix::try_new(1, 1, [context.one()]).unwrap(),
    );
    let verified = verify_affine_family_map(
        &source,
        &target,
        momentum,
        SymmetryVerificationLimits::default(),
    )
    .unwrap();

    // In coordinate order [k.k, k.p]: k_s.k_s = k.k + 2 k.p + s,
    // k_s.p_s = k.p + s.
    let scalar = verified.scalar_products();
    assert_eq!(scalar.constant()[0], context.parameter("s").unwrap());
    assert_eq!(scalar.linear().get(0, 0), Some(&context.one()));
    assert_eq!(scalar.linear().get(0, 1), Some(&context.integer(2)));
    assert_eq!(scalar.constant()[1], context.parameter("s").unwrap());
    assert_eq!(scalar.linear().get(1, 0), Some(&context.zero()));
    assert_eq!(scalar.linear().get(1, 1), Some(&context.one()));

    // D0_source maps to D1_target.  D1_source maps to an affine combination
    // for this deliberately non-translation-invariant two-row basis.
    assert_eq!(
        verified.row_actions()[0],
        DenominatorRowAction::Monomial {
            target: 1,
            scale: context.one(),
        }
    );
    assert!(matches!(
        verified.row_actions()[1],
        DenominatorRowAction::Affine
    ));
}

#[test]
fn rejects_singular_foreign_and_gram_violating_candidates() {
    let family = vacuum_two_loop_family();
    let context = family.coefficient_context();
    let singular = MomentumMap::new(
        ExactMatrix::try_new(
            2,
            2,
            [context.one(), context.zero(), context.one(), context.zero()],
        )
        .unwrap(),
        ExactMatrix::try_new(2, 0, []).unwrap(),
        ExactMatrix::try_new(0, 0, []).unwrap(),
    );
    assert_eq!(
        verify_affine_family_map(
            &family,
            &family,
            singular,
            SymmetryVerificationLimits::default()
        )
        .unwrap_err(),
        SymmetryVerificationError::SingularLoopMap
    );

    let bubble = one_loop_external_family("gram-bubble");
    let bubble_context = bubble.coefficient_context();
    let wrong_external = MomentumMap::new(
        ExactMatrix::try_new(1, 1, [bubble_context.one()]).unwrap(),
        ExactMatrix::try_new(1, 1, [bubble_context.zero()]).unwrap(),
        ExactMatrix::try_new(1, 1, [bubble_context.integer(2)]).unwrap(),
    );
    assert!(matches!(
        verify_affine_family_map(
            &bubble,
            &bubble,
            wrong_external,
            SymmetryVerificationLimits::default()
        ),
        Err(SymmetryVerificationError::ExternalGramMismatch { .. })
    ));

    let foreign = CoefficientContext::new(["x"]);
    let foreign_map = MomentumMap::new(
        ExactMatrix::try_new(
            2,
            2,
            [foreign.one(), foreign.zero(), foreign.zero(), foreign.one()],
        )
        .unwrap(),
        ExactMatrix::try_new(2, 0, []).unwrap(),
        ExactMatrix::try_new(0, 0, []).unwrap(),
    );
    assert!(matches!(
        verify_affine_family_map(
            &family,
            &family,
            foreign_map,
            SymmetryVerificationLimits::default()
        ),
        Err(SymmetryVerificationError::ForeignMapCoefficient { .. })
    ));
}

#[test]
fn resource_limits_are_checked_before_retaining_derived_matrices() {
    let family = vacuum_two_loop_family();
    let context = family.coefficient_context();
    let momentum = MomentumMap::new(
        ExactMatrix::try_new(
            2,
            2,
            [context.one(), context.zero(), context.zero(), context.one()],
        )
        .unwrap(),
        ExactMatrix::try_new(2, 0, []).unwrap(),
        ExactMatrix::try_new(0, 0, []).unwrap(),
    );
    let mut strict = SymmetryVerificationLimits::default();
    strict.max_matrix_entries = 3;
    assert!(matches!(
        verify_affine_family_map(&family, &family, momentum, strict),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "retained matrix entries",
            ..
        })
    ));
}

#[test]
fn coordinate_order_used_by_external_fixture_is_explicit() {
    let family = one_loop_external_family("coordinate-check");
    assert_eq!(
        family.coordinates(),
        &[
            ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 0,
                external_index: 0,
            },
        ]
    );
}

#[test]
fn exact_matrix_collection_is_bounded_before_and_during_iteration() {
    let error = ExactMatrix::<i32>::try_new_with_max_entries(
        2,
        2,
        std::iter::from_fn(|| panic!("preflight must not consume an oversized payload")),
        3,
    )
    .unwrap_err();
    assert_eq!(
        error,
        SymmetryVerificationError::ResourceLimit {
            resource: "exact matrix entries",
            requested: 4,
            limit: 3,
        }
    );

    let matrix = ExactMatrix::try_new_with_max_entries(2, 2, 0..4, 4).unwrap();
    assert_eq!(matrix.entries(), &[0, 1, 2, 3]);
    assert_eq!(
        ExactMatrix::try_new_with_max_entries(1, 1, std::iter::repeat(0), 1).unwrap_err(),
        SymmetryVerificationError::MatrixPayloadTooLarge {
            rows: 1,
            columns: 1,
            expected: 1,
        }
    );
}

#[test]
fn singular_gram_still_requires_an_invertible_external_map() {
    let family = singular_gram_family("singular-gram-external-map");
    let context = family.coefficient_context();
    assert_eq!(
        verify_affine_family_map(
            &family,
            &family,
            singular_gram_map(&family, context.zero()),
            SymmetryVerificationLimits::default(),
        )
        .unwrap_err(),
        SymmetryVerificationError::SingularExternalMap
    );

    let x = context.parameter("x").unwrap();
    let verified = verify_affine_family_map(
        &family,
        &family,
        singular_gram_map(&family, x.clone()),
        SymmetryVerificationLimits::default(),
    )
    .unwrap();
    assert_eq!(verified.external_determinant(), &x);
    assert_eq!(
        verified.row_actions()[1],
        DenominatorRowAction::Monomial {
            target: 1,
            scale: x.clone(),
        }
    );
    let x_guard = verified
        .replay_guards()
        .iter()
        .find(|condition| condition.polynomial() == &x.numerator)
        .expect("det(C) and the monomial scale require x != 0");
    assert!(
        x_guard
            .origins()
            .contains(&SymmetryGuardOrigin::ExternalMapDeterminantNumerator)
    );
    assert!(
        x_guard
            .origins()
            .contains(&SymmetryGuardOrigin::DenominatorScaleNumerator {
                source_denominator: 1,
                target_denominator: 1,
            })
    );
}

#[test]
fn rational_map_guards_retain_candidate_scale_and_family_provenance() {
    let family = singular_gram_family("rational-guard-map");
    let context = family.coefficient_context();
    let x = context.parameter("x").unwrap();
    let x_plus_one = context
        .try_add(&x, &context.one(), ExactAlgebraLimits::default())
        .unwrap();
    let scale = context
        .try_div(&x, &x_plus_one, ExactAlgebraLimits::default())
        .unwrap();
    let verified = verify_affine_family_map(
        &family,
        &family,
        singular_gram_map(&family, scale.clone()),
        SymmetryVerificationLimits::default(),
    )
    .unwrap();

    assert_eq!(
        verified.candidate_denominator_guards(),
        &[x_plus_one.numerator.clone()]
    );
    let denominator_guard = verified
        .replay_guards()
        .iter()
        .find(|condition| condition.polynomial() == &x_plus_one.numerator)
        .unwrap();
    assert!(
        denominator_guard
            .origins()
            .contains(&SymmetryGuardOrigin::MomentumMapDenominator {
                matrix: "C",
                row: 0,
                column: 0,
            })
    );

    let x_guard = verified
        .replay_guards()
        .iter()
        .find(|condition| condition.polynomial() == &x.numerator)
        .unwrap();
    assert!(
        x_guard
            .origins()
            .contains(&SymmetryGuardOrigin::ExternalMapDeterminantNumerator)
    );
    assert!(
        x_guard
            .origins()
            .contains(&SymmetryGuardOrigin::DenominatorScaleNumerator {
                source_denominator: 1,
                target_denominator: 1,
            })
    );

    let unit_guard = verified
        .replay_guards()
        .iter()
        .find(|condition| condition.polynomial().is_one())
        .unwrap();
    assert!(
        unit_guard
            .origins()
            .contains(&SymmetryGuardOrigin::SourceFamily(
                GuardOrigin::FamilyBasisDeterminantNumerator,
            ))
    );
    assert!(
        unit_guard
            .origins()
            .contains(&SymmetryGuardOrigin::TargetFamily(
                GuardOrigin::FamilyBasisDeterminantNumerator,
            ))
    );
}

#[test]
fn dense_transport_matches_an_independent_bilinear_oracle() {
    let context = CoefficientContext::new(["d"]);
    let source = dense_two_loop_external_family(
        "dense-source",
        &context,
        [[7, 4], [4, 3]],
        [1, 2, 3, 4, 5, 6, 7],
    );
    let target = dense_two_loop_external_family(
        "dense-target",
        &context,
        [[2, 1], [1, 3]],
        [10, 20, 30, 40, 50, 60, 70],
    );
    let momentum = MomentumMap::new(
        ExactMatrix::try_new(
            2,
            2,
            [
                context.integer(1),
                context.integer(2),
                context.integer(3),
                context.integer(5),
            ],
        )
        .unwrap(),
        ExactMatrix::try_new(
            2,
            2,
            [
                context.integer(1),
                context.integer(-1),
                context.integer(2),
                context.integer(1),
            ],
        )
        .unwrap(),
        ExactMatrix::try_new(
            2,
            2,
            [
                context.integer(1),
                context.integer(1),
                context.integer(0),
                context.integer(1),
            ],
        )
        .unwrap(),
    );
    let verified = verify_affine_family_map(
        &source,
        &target,
        momentum,
        SymmetryVerificationLimits::default(),
    )
    .unwrap();

    let expected_constant = [3, 0, 15, -1, -2, 10, 5];
    let expected_linear = [
        [1, 4, 4, 2, -2, 4, -4],
        [3, 11, 10, 5, -2, 9, -3],
        [9, 30, 25, 12, 6, 20, 10],
        [0, 0, 0, 1, 1, 2, 2],
        [0, 0, 0, 0, 1, 0, 2],
        [0, 0, 0, 3, 3, 5, 5],
        [0, 0, 0, 0, 3, 0, 5],
    ];
    for (row, expected) in expected_constant.into_iter().enumerate() {
        assert_eq!(
            verified.scalar_products().constant()[row],
            context.integer(expected)
        );
        for (column, coefficient) in expected_linear[row].into_iter().enumerate() {
            assert_eq!(
                verified.scalar_products().linear().get(row, column),
                Some(&context.integer(coefficient))
            );
            assert_eq!(
                verified.denominators().linear().get(row, column),
                Some(&context.integer(coefficient))
            );
        }
    }
    for (actual, expected) in verified
        .denominators()
        .constant()
        .iter()
        .zip([-146, -978, -4102, -347, -187, -904, -488])
    {
        assert_eq!(actual, &context.integer(expected));
    }
    assert_eq!(
        verified.jacobian(),
        &JacobianWitness::Unit {
            determinant_sign: -1,
        }
    );
    assert_eq!(VerifiedAffineFamilyMap::SCHEMA, AFFINE_FAMILY_MAP_V2_SCHEMA);
    assert_eq!(verified.stats().symbolica_determinant_calls(), 2);
    assert_eq!(verified.stats().symbolica_product_calls(), 6);
    assert_eq!(verified.stats().symbolica_transpose_calls(), 1);
    assert_eq!(verified.stats().determinant_states(), 0);
}

#[test]
fn native_four_by_four_determinant_has_no_subset_state_gate() {
    let context = CoefficientContext::new(["d", "x"]);
    let family = vacuum_identity_family("native-four-loop-determinant", &context, 4);
    let x = context.parameter("x").unwrap();
    let momentum = MomentumMap::new(
        ExactMatrix::try_new(
            4,
            4,
            [
                x,
                context.one(),
                context.zero(),
                context.zero(),
                context.zero(),
                context.integer(2),
                context.one(),
                context.zero(),
                context.zero(),
                context.zero(),
                context.integer(3),
                context.one(),
                context.zero(),
                context.zero(),
                context.zero(),
                context.integer(4),
            ],
        )
        .unwrap(),
        ExactMatrix::try_new(4, 0, []).unwrap(),
        ExactMatrix::try_new(0, 0, []).unwrap(),
    );
    let mut limits = SymmetryVerificationLimits::default();
    limits.max_determinant_states = 0;
    let verified = verify_affine_family_map(&family, &family, momentum, limits).unwrap();
    assert_eq!(verified.loop_determinant(), &context.parse("24*x").unwrap());
    assert_eq!(verified.external_determinant(), &context.one());
    assert_eq!(verified.stats().determinant_states(), 0);
    assert_eq!(verified.stats().symbolica_determinant_calls(), 1);
    verified.replay(&family, &family, limits).unwrap();
}

#[test]
fn validation_resource_errors_remain_typed() {
    let family = singular_gram_family("typed-map-error");
    let context = family.coefficient_context();
    let x_plus_one = context
        .try_add(
            &context.parameter("x").unwrap(),
            &context.one(),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    let mut limits = SymmetryVerificationLimits::default();
    limits.exact_algebra.max_polynomial_terms = 1;
    assert!(matches!(
        verify_affine_family_map(
            &family,
            &family,
            singular_gram_map(&family, x_plus_one),
            limits,
        ),
        Err(SymmetryVerificationError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit {
                resource: "authenticated polynomial terms",
                requested: 2,
                limit: 1,
            }
        ))
    ));
}

#[test]
fn aggregate_limits_pass_exactly_at_and_fail_one_below() {
    let family = singular_gram_family("aggregate-boundaries");
    let context = family.coefficient_context();
    let x = context.parameter("x").unwrap();
    let baseline = verify_affine_family_map(
        &family,
        &family,
        singular_gram_map(&family, x),
        SymmetryVerificationLimits::default(),
    )
    .unwrap();
    let stats = baseline.stats();

    let mut exact = SymmetryVerificationLimits::default();
    exact.max_matrix_entries = stats.matrix_entries();
    exact.max_exact_operations = stats.exact_operations();
    exact.max_determinant_states = 0;
    exact.max_symbolica_single_matrix_entries = stats.symbolica_largest_matrix_entries();
    exact.max_symbolica_live_matrix_entries = stats.symbolica_peak_live_matrix_entries();
    exact.max_symbolica_input_retained_bytes = stats.symbolica_input_retained_bytes();
    exact.max_symbolica_output_retained_bytes = stats.symbolica_output_retained_bytes();
    exact.max_guard_polynomials = stats.guard_polynomials();
    exact.max_guard_origins = stats.guard_origins();
    let replayed =
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), exact).unwrap();
    assert_eq!(replayed.stats(), stats);

    let mut one_below = SymmetryVerificationLimits::default();
    one_below.max_matrix_entries = stats.matrix_entries() - 1;
    assert!(matches!(
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), one_below,),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "retained matrix entries",
            ..
        })
    ));

    let mut one_below = SymmetryVerificationLimits::default();
    one_below.max_exact_operations = stats.exact_operations() - 1;
    assert!(matches!(
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), one_below,),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "exact operations",
            ..
        })
    ));

    assert_eq!(stats.determinant_states(), 0);
    assert!(stats.symbolica_exact_operations() > 0);
    assert!(stats.symbolica_admitted_exact_operations() > 0);
    assert_eq!(
        stats.symbolica_exact_operations(),
        stats.symbolica_admitted_exact_operations(),
        "this 1x1-determinant/product fixture pins an actual-operation one-below boundary",
    );
    assert!(stats.symbolica_determinant_calls() > 0);
    assert!(stats.symbolica_product_calls() > 0);

    let mut one_below = SymmetryVerificationLimits::default();
    one_below.max_symbolica_single_matrix_entries = stats.symbolica_largest_matrix_entries() - 1;
    assert!(matches!(
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), one_below,),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "Symbolica single matrix entries",
            ..
        })
    ));

    let mut one_below = SymmetryVerificationLimits::default();
    one_below.max_symbolica_live_matrix_entries = stats.symbolica_peak_live_matrix_entries() - 1;
    assert!(matches!(
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), one_below,),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "Symbolica live matrix entries",
            ..
        })
    ));

    let mut one_below = SymmetryVerificationLimits::default();
    one_below.max_symbolica_input_retained_bytes = stats.symbolica_input_retained_bytes() - 1;
    assert!(matches!(
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), one_below,),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "Symbolica input retained bytes",
            ..
        })
    ));

    let mut one_below = SymmetryVerificationLimits::default();
    one_below.max_symbolica_output_retained_bytes = stats.symbolica_output_retained_bytes() - 1;
    assert!(matches!(
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), one_below,),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "Symbolica output retained bytes",
            ..
        })
    ));

    let mut one_below = SymmetryVerificationLimits::default();
    one_below.max_guard_polynomials = stats.guard_polynomials() - 1;
    assert!(matches!(
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), one_below,),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "guard polynomials",
            ..
        })
    ));

    let mut one_below = SymmetryVerificationLimits::default();
    one_below.max_guard_origins = stats.guard_origins() - 1;
    assert!(matches!(
        verify_affine_family_map(&family, &family, baseline.momentum().clone(), one_below,),
        Err(SymmetryVerificationError::ResourceLimit {
            resource: "guard origins",
            ..
        })
    ));
}
