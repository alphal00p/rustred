use std::sync::Arc;

use rustred::{
    CoefficientContext, CoordinateEqualityLocusCertificate, CoordinateEqualityLocusExtractor,
    CoordinateEqualityLocusLimits, ParametricCoefficient, ParametricCoefficientContext,
    ResidualUnitAffineIndexMapCertificate, ResidualUnitAffineIndexMapError,
    ResidualUnitAffineIndexMapLimits, ResidualUnitAffineIndexMapUnsupported, SectorMask,
    SymbolicPolynomialPredicateKind, SymbolicSectorCaseLimits, SymbolicSectorCasePartitionBuilder,
};
use symbolica::prelude::Integer;

fn context(scope: &str) -> ParametricCoefficientContext {
    ParametricCoefficientContext::try_new(&CoefficientContext::new(["d"]), scope, 3).unwrap()
}

fn affine_with_base_factor(context: &ParametricCoefficientContext) -> ParametricCoefficient {
    let n0_plus_n1 = context
        .add(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    let affine = context.sub(&n0_plus_n1, &context.integer(3)).unwrap();
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let factor = context.add(&d, &context.one()).unwrap();
    context.mul(&factor, &affine).unwrap()
}

fn source_with_predicate(
    context: &ParametricCoefficientContext,
    predicate: &ParametricCoefficient,
    predicate_is_equality: bool,
    include_literal_n2: bool,
) -> (Arc<CoordinateEqualityLocusCertificate>, usize) {
    source_with_predicate_in_sector(
        context,
        predicate,
        predicate_is_equality,
        include_literal_n2,
        SectorMask::try_new([true, true, true]).unwrap(),
    )
}

fn source_with_predicate_in_sector(
    context: &ParametricCoefficientContext,
    predicate: &ParametricCoefficient,
    predicate_is_equality: bool,
    include_literal_n2: bool,
    sector: SectorMask,
) -> (Arc<CoordinateEqualityLocusCertificate>, usize) {
    let literals = if include_literal_n2 {
        &[(2, 2)][..]
    } else {
        &[][..]
    };
    source_with_literals_in_sector(context, predicate, predicate_is_equality, literals, sector)
}

fn source_with_literals_in_sector(
    context: &ParametricCoefficientContext,
    predicate: &ParametricCoefficient,
    predicate_is_equality: bool,
    literals: &[(usize, i64)],
    sector: SectorMask,
) -> (Arc<CoordinateEqualityLocusCertificate>, usize) {
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        context,
        sector,
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let mut leaf = builder.root_case();
    for &(position, value) in literals {
        let literal = context
            .sub(&context.index(position).unwrap(), &context.integer(value))
            .unwrap();
        leaf = builder
            .split_on_bad_polynomial(
                context,
                leaf,
                context.numerator_condition(&literal).unwrap(),
            )
            .unwrap()
            .equal_zero_case();
    }
    let children = builder
        .split_on_bad_polynomial(
            context,
            leaf,
            context.numerator_condition(predicate).unwrap(),
        )
        .unwrap();
    leaf = if predicate_is_equality {
        children.equal_zero_case()
    } else {
        children.nonzero_case()
    };
    let partition = builder.finish(context).unwrap();
    let source = Arc::new(
        CoordinateEqualityLocusExtractor::extract(
            context,
            &partition,
            leaf,
            CoordinateEqualityLocusLimits::default(),
        )
        .unwrap(),
    );
    let predicate_ordinal = source
        .unresolved_predicates()
        .iter()
        .find(|predicate| {
            predicate.kind()
                == if predicate_is_equality {
                    SymbolicPolynomialPredicateKind::EqualZero
                } else {
                    SymbolicPolynomialPredicateKind::NonZero
                }
        })
        .expect("the dependent predicate must remain outside literal-coordinate extraction")
        .predicate_ordinal();
    (source, predicate_ordinal)
}

fn evaluate_map(
    certificate: &ResidualUnitAffineIndexMapCertificate,
    free_coordinates: &[Integer],
) -> Vec<Integer> {
    assert_eq!(free_coordinates.len(), certificate.free_positions().len());
    (0..certificate.ambient_arity())
        .map(|position| {
            let mut value = certificate.constant(position).unwrap().clone();
            for (free_ordinal, coordinate) in free_coordinates.iter().enumerate() {
                value += certificate
                    .linear_coefficient(position, free_ordinal)
                    .unwrap()
                    * coordinate;
            }
            value
        })
        .collect()
}

#[test]
fn stable_manifest_binds_the_complete_source_partition_and_orthant() {
    let context = context("unit-affine-source-bound-manifest");
    let predicate = affine_with_base_factor(&context);
    let (active_source, active_ordinal) = source_with_predicate_in_sector(
        &context,
        &predicate,
        true,
        false,
        SectorMask::try_new([true, true, true]).unwrap(),
    );
    let (mixed_source, mixed_ordinal) = source_with_predicate_in_sector(
        &context,
        &predicate,
        true,
        false,
        SectorMask::try_new([false, true, true]).unwrap(),
    );
    let active = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        active_source,
        active_ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();
    let mixed = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        mixed_source,
        mixed_ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();

    assert_ne!(
        active.source_partition_identity(),
        mixed.source_partition_identity()
    );
    assert_eq!(active.local_manifest(), mixed.local_manifest());
}

#[test]
fn extracts_replays_and_orients_n0_equals_three_minus_n1_with_literal_n2() {
    let context = context("unit-affine-oriented");
    let (source, predicate_ordinal) =
        source_with_predicate(&context, &affine_with_base_factor(&context), true, true);
    let certificate = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source.clone(),
        predicate_ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();
    certificate.replay(&context).unwrap();

    assert_eq!(certificate.source(), &source);
    assert_eq!(certificate.free_positions(), &[1]);
    assert_eq!(certificate.literal_positions(), &[2]);
    assert_eq!(certificate.constant(0), Some(&Integer::from(3)));
    assert_eq!(certificate.constant(1), Some(&Integer::from(0)));
    assert_eq!(certificate.constant(2), Some(&Integer::from(2)));
    assert_eq!(
        certificate.linear_coefficient(0, 0),
        Some(&Integer::from(-1))
    );
    assert_eq!(
        certificate.linear_coefficient(1, 0),
        Some(&Integer::from(1))
    );
    assert_eq!(
        certificate.linear_coefficient(2, 0),
        Some(&Integer::from(0))
    );
    assert_eq!(certificate.linear_coefficient(0, 1), None);
    assert_eq!(certificate.linear_coefficient(3, 0), None);
    assert!(certificate.local_manifest().contains("|b=3,0,2|A=-1,1,0"));
    assert_eq!(certificate.stats().ambient_arity(), 3);
    assert_eq!(
        certificate.stats().source_identity_bytes_referenced(),
        certificate.source_partition_identity().len()
    );
    assert_eq!(certificate.stats().free_positions(), 1);
    assert_eq!(certificate.stats().literal_positions(), 1);
    assert_eq!(certificate.stats().matrix_entries(), 3);
    assert_eq!(
        certificate.stats().manifest_bytes(),
        certificate.local_manifest().len()
    );
}

#[test]
fn can_orient_the_same_equality_on_the_other_unit_pivot() {
    let context = context("unit-affine-other-pivot");
    let (source, predicate_ordinal) =
        source_with_predicate(&context, &affine_with_base_factor(&context), true, false);
    let certificate = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source,
        predicate_ordinal,
        1,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.free_positions(), &[0, 2]);
    assert_eq!(certificate.constant(1), Some(&Integer::from(3)));
    assert_eq!(
        certificate.linear_coefficient(1, 0),
        Some(&Integer::from(-1))
    );
    assert_eq!(
        certificate.linear_coefficient(1, 1),
        Some(&Integer::from(0))
    );
    assert_eq!(
        certificate.linear_coefficient(0, 0),
        Some(&Integer::from(1))
    );
    assert_eq!(
        certificate.linear_coefficient(2, 1),
        Some(&Integer::from(1))
    );
}

#[test]
fn rejects_nonunit_nonaffine_and_non_equality_predicates_without_sampling() {
    let context = context("unit-affine-unsupported");
    let two_n0 = context
        .mul(&context.integer(2), &context.index(0).unwrap())
        .unwrap();
    let nonunit = context.add(&two_n0, &context.index(1).unwrap()).unwrap();
    let (source, ordinal) = source_with_predicate(&context, &nonunit, true, false);
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &context,
            source,
            ordinal,
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::Unsupported {
            reason: ResidualUnitAffineIndexMapUnsupported::NonIntegralAffineCoefficient { .. },
            ..
        })
    ));

    let square = context
        .mul(&context.index(1).unwrap(), &context.index(1).unwrap())
        .unwrap();
    let nonlinear = context.sub(&context.index(0).unwrap(), &square).unwrap();
    let (source, ordinal) = source_with_predicate(&context, &nonlinear, true, false);
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &context,
            source,
            ordinal,
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::Unsupported {
            reason: ResidualUnitAffineIndexMapUnsupported::NonAffineIndexEquality { .. },
            ..
        })
    ));

    let (source, ordinal) =
        source_with_predicate(&context, &affine_with_base_factor(&context), false, false);
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &context,
            source,
            ordinal,
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::Unsupported {
            reason: ResidualUnitAffineIndexMapUnsupported::PredicateIsNotEquality,
            ..
        })
    ));
}

#[test]
fn rejects_a_literal_bound_and_preflights_manifest_budget() {
    let context = context("unit-affine-limits");
    let (source, ordinal) =
        source_with_predicate(&context, &affine_with_base_factor(&context), true, true);
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &context,
            source.clone(),
            ordinal,
            2,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::Unsupported {
            reason: ResidualUnitAffineIndexMapUnsupported::BoundPositionAlreadyLiteral {
                position: 2
            },
            ..
        })
    ));

    let complete = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source.clone(),
        ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();
    let mut limits = ResidualUnitAffineIndexMapLimits::default();
    limits.max_manifest_bytes = complete.local_manifest().len() - 1;
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(&context, source, ordinal, 0, limits),
        Err(ResidualUnitAffineIndexMapError::ResourceLimit {
            resource: "local manifest",
            ..
        })
    ));
}

#[test]
fn preflights_predicate_scan_and_retained_reference_budgets() {
    let context = context("unit-affine-preflight-budgets");
    let (source, ordinal) =
        source_with_predicate(&context, &affine_with_base_factor(&context), true, false);

    let mut limits = ResidualUnitAffineIndexMapLimits::default();
    limits.max_unresolved_predicates_scanned = 0;
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &context,
            source.clone(),
            ordinal,
            0,
            limits,
        ),
        Err(ResidualUnitAffineIndexMapError::ResourceLimit {
            resource: "unresolved predicates scanned",
            ..
        })
    ));

    limits = ResidualUnitAffineIndexMapLimits::default();
    limits.max_retained_term_references = 0;
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(&context, source, ordinal, 0, limits),
        Err(ResidualUnitAffineIndexMapError::ResourceLimit {
            resource: "retained term references",
            ..
        })
    ));
}

#[test]
fn refuses_to_claim_full_start_parity_with_an_unconsumed_second_equality() {
    let context = context("unit-affine-two-dependent-equalities");
    let first = affine_with_base_factor(&context);
    let second = context
        .sub(
            &context
                .add(&context.index(1).unwrap(), &context.index(2).unwrap())
                .unwrap(),
            &context.integer(4),
        )
        .unwrap();
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        &context,
        SectorMask::try_new([true, true, true]).unwrap(),
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let first_leaf = builder
        .split_on_bad_polynomial(
            &context,
            builder.root_case(),
            context.numerator_condition(&first).unwrap(),
        )
        .unwrap()
        .equal_zero_case();
    let leaf = builder
        .split_on_bad_polynomial(
            &context,
            first_leaf,
            context.numerator_condition(&second).unwrap(),
        )
        .unwrap()
        .equal_zero_case();
    let partition = builder.finish(&context).unwrap();
    let source = Arc::new(
        CoordinateEqualityLocusExtractor::extract(
            &context,
            &partition,
            leaf,
            CoordinateEqualityLocusLimits::default(),
        )
        .unwrap(),
    );
    let selected = source
        .unresolved_predicates()
        .iter()
        .find(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
        .unwrap()
        .predicate_ordinal();

    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &context,
            source,
            selected,
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::Unsupported {
            reason: ResidualUnitAffineIndexMapUnsupported::UnconsumedEqualityPredicates {
                additional: 1
            },
            ..
        })
    ));
}

#[test]
fn folds_a_nonzero_literal_coefficient_into_the_bound_constant_exactly() {
    let context = context("unit-affine-fold-nonzero-literal-coefficient");
    let two_n2 = context
        .mul(&context.integer(2), &context.index(2).unwrap())
        .unwrap();
    let indices = context
        .add(
            &context
                .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                .unwrap(),
            &two_n2,
        )
        .unwrap();
    let affine = context.sub(&indices, &context.integer(7)).unwrap();
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let predicate = context
        .mul(&context.add(&d, &context.one()).unwrap(), &affine)
        .unwrap();
    let (source, ordinal) = source_with_literals_in_sector(
        &context,
        &predicate,
        true,
        &[(2, 2)],
        SectorMask::try_new([true, true, true]).unwrap(),
    );

    let certificate = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source,
        ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();

    // n0 + n1 + 2*n2 - 7 = 0 with n2 = 2 gives n0 = 3 - n1.
    assert_eq!(certificate.free_positions(), &[1]);
    assert_eq!(certificate.literal_positions(), &[2]);
    assert_eq!(certificate.constant(0), Some(&Integer::from(3)));
    assert_eq!(certificate.constant(2), Some(&Integer::from(2)));
    assert_eq!(
        certificate.linear_coefficient(0, 0),
        Some(&Integer::from(-1))
    );
}

#[test]
fn accepts_a_negative_nonprimitive_associate_without_changing_the_map() {
    let context = context("unit-affine-negative-nonprimitive-associate");
    let scaled = context
        .mul(&context.integer(-6), &affine_with_base_factor(&context))
        .unwrap();
    let (source, ordinal) = source_with_predicate(&context, &scaled, true, false);

    let certificate = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source,
        ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.free_positions(), &[1, 2]);
    assert_eq!(certificate.constant(0), Some(&Integer::from(3)));
    assert_eq!(
        certificate.linear_coefficient(0, 0),
        Some(&Integer::from(-1))
    );
    assert_eq!(
        certificate.linear_coefficient(0, 1),
        Some(&Integer::from(0))
    );
    certificate.replay(&context).unwrap();
}

#[test]
fn rejects_inconsistent_base_blocks_as_not_one_affine_associate() {
    let context = context("unit-affine-inconsistent-base-blocks");
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let first_row = context
        .sub(
            &context
                .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                .unwrap(),
            &context.integer(3),
        )
        .unwrap();
    let twice_n1 = context
        .mul(&context.integer(2), &context.index(1).unwrap())
        .unwrap();
    let second_row = context
        .sub(
            &context.add(&context.index(0).unwrap(), &twice_n1).unwrap(),
            &context.integer(3),
        )
        .unwrap();
    let predicate = context
        .add(&context.mul(&d, &first_row).unwrap(), &second_row)
        .unwrap();
    let (source, ordinal) = source_with_predicate(&context, &predicate, true, false);
    let expected_case = source.source_case();

    let error = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source,
        ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap_err();
    match error {
        ResidualUnitAffineIndexMapError::Unsupported {
            source_case,
            predicate_ordinal,
            reason:
                ResidualUnitAffineIndexMapUnsupported::NotAssociateToSingleIntegerAffineRow { .. },
        } => {
            assert_eq!(source_case, expected_case);
            assert_eq!(predicate_ordinal, ordinal);
        }
        other => panic!("expected a source-located NotAssociate result, got {other:?}"),
    }
}

#[test]
fn emits_a_constant_map_when_every_nonbound_coordinate_is_literal() {
    let context = context("unit-affine-zero-free-coordinates");
    let two_n1 = context
        .mul(&context.integer(2), &context.index(1).unwrap())
        .unwrap();
    let row = context
        .sub(
            &context
                .sub(
                    &context.add(&context.index(0).unwrap(), &two_n1).unwrap(),
                    &context.index(2).unwrap(),
                )
                .unwrap(),
            &context.integer(3),
        )
        .unwrap();
    let (source, ordinal) = source_with_literals_in_sector(
        &context,
        &row,
        true,
        &[(1, 2), (2, 4)],
        SectorMask::try_new([true, true, true]).unwrap(),
    );

    let certificate = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source,
        ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();

    assert!(certificate.free_positions().is_empty());
    assert_eq!(certificate.literal_positions(), &[1, 2]);
    assert_eq!(certificate.stats().matrix_entries(), 0);
    assert_eq!(certificate.constant(0), Some(&Integer::from(3)));
    assert_eq!(certificate.constant(1), Some(&Integer::from(2)));
    assert_eq!(certificate.constant(2), Some(&Integer::from(4)));
    assert_eq!(certificate.linear_coefficient(0, 0), None);
    assert_eq!(
        evaluate_map(&certificate, &[]),
        vec![Integer::from(3), Integer::from(2), Integer::from(4)]
    );
}

#[test]
fn extracted_map_is_idempotent_at_representative_integer_free_points() {
    let context = context("unit-affine-idempotence-points");
    let (source, ordinal) =
        source_with_predicate(&context, &affine_with_base_factor(&context), true, true);
    let certificate = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source,
        ordinal,
        0,
        ResidualUnitAffineIndexMapLimits::default(),
    )
    .unwrap();

    for coordinate in [-9, 0, 1, 17] {
        let first = evaluate_map(&certificate, &[Integer::from(coordinate)]);
        let projected_free = certificate
            .free_positions()
            .iter()
            .map(|&position| first[position].clone())
            .collect::<Vec<_>>();
        let second = evaluate_map(&certificate, &projected_free);
        assert_eq!(
            second, first,
            "idempotence failed at free value {coordinate}"
        );
    }
}

#[test]
fn rejects_wrong_context_out_of_range_empty_source_and_absent_bound() {
    let source_context = context("unit-affine-source-context");
    let (source, ordinal) = source_with_predicate(
        &source_context,
        &affine_with_base_factor(&source_context),
        true,
        false,
    );
    let other_context = context("unit-affine-other-context");
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &other_context,
            source.clone(),
            ordinal,
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::WrongContext)
    ));

    let expected_case = source.source_case();
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &source_context,
            source,
            ordinal,
            3,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::BoundPositionOutOfRange {
            source_case,
            predicate_ordinal,
            position: 3,
            arity: 3,
        }) if source_case == expected_case && predicate_ordinal == ordinal
    ));

    let empty_context = context("unit-affine-proved-empty-source");
    let empty_predicate = affine_with_base_factor(&empty_context);
    let (empty_source, empty_ordinal) = source_with_literals_in_sector(
        &empty_context,
        &empty_predicate,
        true,
        &[(2, 0)],
        SectorMask::try_new([true, true, true]).unwrap(),
    );
    let empty_case = empty_source.source_case();
    assert!(empty_source.is_proved_empty());
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &empty_context,
            empty_source,
            empty_ordinal,
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::Unsupported {
            source_case,
            predicate_ordinal,
            reason: ResidualUnitAffineIndexMapUnsupported::SourceLeafProvedEmpty,
        }) if source_case == empty_case && predicate_ordinal == empty_ordinal
    ));

    let absent_context = context("unit-affine-bound-absent");
    let absent_predicate = absent_context
        .sub(
            &absent_context
                .add(
                    &absent_context.index(1).unwrap(),
                    &absent_context.index(2).unwrap(),
                )
                .unwrap(),
            &absent_context.integer(3),
        )
        .unwrap();
    let (absent_source, absent_ordinal) =
        source_with_predicate(&absent_context, &absent_predicate, true, false);
    let absent_case = absent_source.source_case();
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &absent_context,
            absent_source,
            absent_ordinal,
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::Unsupported {
            source_case,
            predicate_ordinal,
            reason: ResidualUnitAffineIndexMapUnsupported::BoundVariableAbsent { position: 0 },
        }) if source_case == absent_case && predicate_ordinal == absent_ordinal
    ));
}

#[test]
fn source_identity_reference_budget_accepts_the_exact_boundary_only() {
    let context = context("unit-affine-source-identity-budget");
    let (source, ordinal) =
        source_with_predicate(&context, &affine_with_base_factor(&context), true, false);
    let source_identity_bytes = source.source_partition().source_identity().len();
    assert!(source_identity_bytes > 0);

    let mut limits = ResidualUnitAffineIndexMapLimits::default();
    limits.max_source_identity_bytes_referenced = source_identity_bytes;
    let certificate = ResidualUnitAffineIndexMapCertificate::compile(
        &context,
        source.clone(),
        ordinal,
        0,
        limits,
    )
    .unwrap();
    assert_eq!(
        certificate.stats().source_identity_bytes_referenced(),
        source_identity_bytes
    );

    limits.max_source_identity_bytes_referenced = source_identity_bytes - 1;
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &context,
            source,
            ordinal,
            0,
            limits,
        ),
        Err(ResidualUnitAffineIndexMapError::ResourceLimit {
            resource: "source identity bytes referenced",
            requested,
            limit,
        }) if requested == source_identity_bytes && limit + 1 == source_identity_bytes
    ));
}
