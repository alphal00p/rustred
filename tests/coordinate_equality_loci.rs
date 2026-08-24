use rustred::{
    CoefficientContext, CoordinateEqualityEmptyReason, CoordinateEqualityLeafStatus,
    CoordinateEqualityLocusError, CoordinateEqualityLocusExtractor, CoordinateEqualityLocusLimits,
    ParametricCoefficient, ParametricCoefficientContext, ParametricPolynomial, SectorMask,
    SectorOrthantSide, SymbolicPolynomialPredicateKind, SymbolicSectorCaseError,
    SymbolicSectorCaseId, SymbolicSectorCaseLimits, SymbolicSectorCasePartitionBuilder,
    SymbolicSectorCasePartitionCertificate,
};

fn make_context(scope: &str) -> (CoefficientContext, ParametricCoefficientContext) {
    let base = CoefficientContext::new(["theta"]);
    let context = ParametricCoefficientContext::try_new(&base, scope, 2).unwrap();
    (base, context)
}

fn coordinate_factor(
    context: &ParametricCoefficientContext,
    index: usize,
    value: i64,
) -> ParametricCoefficient {
    context
        .sub(&context.index(index).unwrap(), &context.integer(value))
        .unwrap()
}

fn coordinate_polynomial(
    context: &ParametricCoefficientContext,
    index: usize,
    value: i64,
    factor: &ParametricCoefficient,
) -> ParametricPolynomial {
    let expanded = context
        .mul(factor, &coordinate_factor(context, index, value))
        .unwrap();
    context.numerator_condition(&expanded).unwrap()
}

fn branch_partition(
    context: &ParametricCoefficientContext,
    sector: SectorMask,
    predicates: Vec<(ParametricPolynomial, SymbolicPolynomialPredicateKind)>,
) -> (SymbolicSectorCasePartitionCertificate, SymbolicSectorCaseId) {
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        context,
        sector,
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let mut case = builder.root_case();
    for (polynomial, kind) in predicates {
        let children = builder
            .split_on_bad_polynomial(context, case, polynomial)
            .unwrap();
        case = match kind {
            SymbolicPolynomialPredicateKind::EqualZero => children.equal_zero_case(),
            SymbolicPolynomialPredicateKind::NonZero => children.nonzero_case(),
        };
    }
    (builder.finish(context).unwrap(), case)
}

#[test]
fn base_units_signs_scales_and_duplicate_loci_produce_one_canonical_assignment() {
    let (base, context) = make_context("coordinate-locus-base-units");
    let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
    let theta_plus_one = context.add(&theta, &context.one()).unwrap();
    let negative_scaled = context.mul(&context.integer(-7), &theta_plus_one).unwrap();
    let positive_scaled = context.mul(&context.integer(11), &theta).unwrap();
    let first = coordinate_polynomial(&context, 0, 2, &negative_scaled);
    let duplicate_associate = coordinate_polynomial(&context, 0, 2, &positive_scaled);
    assert_ne!(first, duplicate_associate);

    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        vec![
            (first, SymbolicPolynomialPredicateKind::EqualZero),
            (
                duplicate_associate,
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
        ],
    );
    let certificate = CoordinateEqualityLocusExtractor::extract(
        &context,
        &partition,
        leaf,
        CoordinateEqualityLocusLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.assignment().entries(), &[(0, 2)]);
    assert_eq!(certificate.assignment_witnesses().len(), 1);
    assert_eq!(
        certificate.assignment_witnesses()[0].equality_predicate_ordinals(),
        &[0, 1]
    );
    assert_eq!(certificate.recognized_predicates().len(), 2);
    assert!(certificate.unresolved_predicates().is_empty());
    assert_eq!(
        certificate.status(),
        &CoordinateEqualityLeafStatus::NotProvedEmpty
    );
    assert_eq!(certificate.stats().equality_predicates(), 2);
    certificate.replay(&context).unwrap();
}

#[test]
fn conflicting_coordinate_values_are_an_exact_empty_leaf_proof() {
    let (_, context) = make_context("coordinate-locus-conflict");
    let one = context.one();
    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        vec![
            (
                coordinate_polynomial(&context, 0, 2, &one),
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
            (
                coordinate_polynomial(&context, 0, 3, &one),
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
        ],
    );
    let certificate = CoordinateEqualityLocusExtractor::extract(
        &context,
        &partition,
        leaf,
        CoordinateEqualityLocusLimits::default(),
    )
    .unwrap();

    // The contradictory coordinate is not handed to conditional elimination.
    assert!(certificate.assignment().is_empty());
    assert_eq!(
        certificate.status(),
        &CoordinateEqualityLeafStatus::ProvedEmpty(
            CoordinateEqualityEmptyReason::ConflictingFixedValues {
                index: 0,
                first_value: 2,
                first_equality_predicate_ordinals: vec![0].into_boxed_slice(),
                second_value: 3,
                second_equality_predicate_ordinals: vec![1].into_boxed_slice(),
            }
        )
    );
    certificate.replay(&context).unwrap();
}

#[test]
fn equality_and_associate_nonzero_predicate_prove_the_leaf_empty() {
    let (base, context) = make_context("coordinate-locus-equality-nonzero");
    let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
    let associate = context.add(&theta, &context.one()).unwrap();
    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        vec![
            (
                coordinate_polynomial(&context, 0, 4, &context.integer(-3)),
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
            (
                coordinate_polynomial(&context, 0, 4, &associate),
                SymbolicPolynomialPredicateKind::NonZero,
            ),
        ],
    );
    let certificate = CoordinateEqualityLocusExtractor::extract(
        &context,
        &partition,
        leaf,
        CoordinateEqualityLocusLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.assignment().entries(), &[(0, 4)]);
    assert_eq!(
        certificate.status(),
        &CoordinateEqualityLeafStatus::ProvedEmpty(
            CoordinateEqualityEmptyReason::EqualityNonzeroContradiction {
                index: 0,
                value: 4,
                equality_predicate_ordinals: vec![0].into_boxed_slice(),
                nonzero_predicate_ordinals: vec![1].into_boxed_slice(),
            }
        )
    );
}

#[test]
fn orthant_violating_fixed_value_proves_the_leaf_empty() {
    let (_, context) = make_context("coordinate-locus-orthant");
    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        vec![(
            coordinate_polynomial(&context, 1, 1, &context.one()),
            SymbolicPolynomialPredicateKind::EqualZero,
        )],
    );
    let certificate = CoordinateEqualityLocusExtractor::extract(
        &context,
        &partition,
        leaf,
        CoordinateEqualityLocusLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.assignment().entries(), &[(1, 1)]);
    assert_eq!(
        certificate.status(),
        &CoordinateEqualityLeafStatus::ProvedEmpty(
            CoordinateEqualityEmptyReason::OrthantViolation {
                index: 1,
                value: 1,
                equality_predicate_ordinals: vec![0].into_boxed_slice(),
                side: SectorOrthantSide::AtMostZero,
            }
        )
    );
}

#[test]
fn multivariate_nonlinear_and_nonintegral_loci_remain_unresolved() {
    let (_, context) = make_context("coordinate-locus-unresolved");
    let n0_minus_two = coordinate_factor(&context, 0, 2);
    let n1_plus_one = coordinate_factor(&context, 1, -1);
    let multivariate = context.mul(&n0_minus_two, &n1_plus_one).unwrap();
    let nonlinear = context.mul(&n0_minus_two, &n0_minus_two).unwrap();
    let twice_n0 = context
        .mul(&context.integer(2), &context.index(0).unwrap())
        .unwrap();
    let nonintegral = context.sub(&twice_n0, &context.one()).unwrap();
    let unresolved = [multivariate, nonlinear, nonintegral]
        .into_iter()
        .map(|coefficient| context.numerator_condition(&coefficient).unwrap())
        .collect::<Vec<_>>();

    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        unresolved
            .iter()
            .cloned()
            .map(|polynomial| (polynomial, SymbolicPolynomialPredicateKind::EqualZero))
            .collect(),
    );
    let certificate = CoordinateEqualityLocusExtractor::extract(
        &context,
        &partition,
        leaf,
        CoordinateEqualityLocusLimits::default(),
    )
    .unwrap();

    assert!(certificate.assignment().is_empty());
    assert!(certificate.recognized_predicates().is_empty());
    assert_eq!(certificate.unresolved_predicates().len(), 3);
    assert_eq!(
        certificate
            .unresolved_predicates()
            .iter()
            .map(|predicate| predicate.polynomial().clone())
            .collect::<Vec<_>>(),
        unresolved
    );
    assert_eq!(
        certificate.status(),
        &CoordinateEqualityLeafStatus::NotProvedEmpty
    );
    certificate.replay(&context).unwrap();
}

#[test]
fn foreign_context_and_unknown_leaf_are_typed_failures() {
    let (_, context) = make_context("coordinate-locus-binding-a");
    let (_, foreign) = make_context("coordinate-locus-binding-b");
    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        vec![(
            coordinate_polynomial(&context, 0, 2, &context.one()),
            SymbolicPolynomialPredicateKind::EqualZero,
        )],
    );
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(
            &foreign,
            &partition,
            leaf,
            CoordinateEqualityLocusLimits::default(),
        ),
        Err(CoordinateEqualityLocusError::SourcePartition(
            SymbolicSectorCaseError::ContextMismatch
        ))
    );
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(
            &context,
            &partition,
            SymbolicSectorCaseId::ROOT,
            CoordinateEqualityLocusLimits::default(),
        ),
        Err(CoordinateEqualityLocusError::CaseNotFound {
            case: SymbolicSectorCaseId::ROOT,
        })
    );
}

#[test]
fn aggregate_limits_fail_before_unbounded_retention() {
    let (_, context) = make_context("coordinate-locus-limits");
    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        vec![(
            coordinate_polynomial(&context, 0, 2, &context.one()),
            SymbolicPolynomialPredicateKind::EqualZero,
        )],
    );

    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_predicates = 0;
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(&context, &partition, leaf, limits),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus predicates",
            requested: 1,
            limit: 0,
        })
    );

    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_assignments = 0;
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(&context, &partition, leaf, limits),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus assignments",
            requested: 1,
            limit: 0,
        })
    );

    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_total_witness_ordinals = 1;
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(&context, &partition, leaf, limits),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus witness ordinals",
            requested: 2,
            limit: 1,
        })
    );

    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_retained_polynomial_terms = partition.stats().retained_polynomial_terms() - 1;
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(&context, &partition, leaf, limits),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus retained polynomial terms",
            requested: partition.stats().retained_polynomial_terms(),
            limit: partition.stats().retained_polynomial_terms() - 1,
        })
    );

    // For an unresolved predicate, reject the polynomial-term clone before
    // attempting canonical display formatting for the byte census.
    let nonlinear_factor = coordinate_factor(&context, 0, 2);
    let nonlinear = context.mul(&nonlinear_factor, &nonlinear_factor).unwrap();
    let nonlinear = context.numerator_condition(&nonlinear).unwrap();
    let (unresolved_partition, unresolved_leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        vec![(
            nonlinear.clone(),
            SymbolicPolynomialPredicateKind::EqualZero,
        )],
    );
    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_retained_polynomial_terms = unresolved_partition.stats().retained_polynomial_terms();
    limits.max_retained_polynomial_bytes = unresolved_partition.stats().retained_polynomial_bytes();
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(
            &context,
            &unresolved_partition,
            unresolved_leaf,
            limits,
        ),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus retained polynomial terms",
            requested: unresolved_partition.stats().retained_polynomial_terms()
                + nonlinear.term_count(),
            limit: unresolved_partition.stats().retained_polynomial_terms(),
        })
    );
}

#[test]
fn exact_i64_boundary_loci_are_recognized_without_narrowing_or_sign_loss() {
    let (_, context) = make_context("coordinate-locus-i64-boundaries");
    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([false, true]).unwrap(),
        vec![
            (
                coordinate_polynomial(&context, 0, i64::MIN, &context.integer(-1)),
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
            (
                coordinate_polynomial(&context, 1, i64::MAX, &context.integer(1)),
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
        ],
    );
    let certificate = CoordinateEqualityLocusExtractor::extract(
        &context,
        &partition,
        leaf,
        CoordinateEqualityLocusLimits::default(),
    )
    .unwrap();

    assert_eq!(
        certificate.assignment().entries(),
        &[(0, i64::MIN), (1, i64::MAX)]
    );
    assert_eq!(
        certificate.status(),
        &CoordinateEqualityLeafStatus::NotProvedEmpty
    );
    certificate.replay(&context).unwrap();
}

#[test]
fn nonzero_coordinate_exclusions_are_retained_but_never_become_assignments() {
    let (base, context) = make_context("coordinate-locus-nonzero-only");
    let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
    let theta_n0 = context.mul(&theta, &context.index(0).unwrap()).unwrap();
    let two_theta_plus_one = context
        .add(
            &context.mul(&context.integer(2), &theta).unwrap(),
            &context.one(),
        )
        .unwrap();
    // theta*n0-(2*theta+1) has the nonintegral, parameter-dependent root
    // 2+1/theta and is deliberately outside the coordinate-assignment API.
    let parameter_dependent_root = context.sub(&theta_n0, &two_theta_plus_one).unwrap();
    let unresolved = context
        .numerator_condition(&parameter_dependent_root)
        .unwrap();
    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, true]).unwrap(),
        vec![
            (
                coordinate_polynomial(&context, 1, 3, &context.one()),
                SymbolicPolynomialPredicateKind::NonZero,
            ),
            (
                unresolved.clone(),
                SymbolicPolynomialPredicateKind::EqualZero,
            ),
        ],
    );
    let certificate = CoordinateEqualityLocusExtractor::extract(
        &context,
        &partition,
        leaf,
        CoordinateEqualityLocusLimits::default(),
    )
    .unwrap();

    assert!(certificate.assignment().is_empty());
    assert_eq!(certificate.recognized_predicates().len(), 1);
    assert_eq!(
        certificate.recognized_predicates()[0].kind(),
        SymbolicPolynomialPredicateKind::NonZero
    );
    assert_eq!(certificate.recognized_predicates()[0].index(), 1);
    assert_eq!(certificate.recognized_predicates()[0].value(), 3);
    assert_eq!(certificate.unresolved_predicates().len(), 1);
    assert_eq!(
        certificate.unresolved_predicates()[0].polynomial(),
        &unresolved
    );
    assert_eq!(
        certificate.status(),
        &CoordinateEqualityLeafStatus::NotProvedEmpty
    );
    certificate.replay(&context).unwrap();
}

#[test]
fn independent_inspection_and_recognition_budgets_are_enforced() {
    let (_, context) = make_context("coordinate-locus-independent-limits");
    let (partition, leaf) = branch_partition(
        &context,
        SectorMask::try_new([true, false]).unwrap(),
        vec![(
            coordinate_polynomial(&context, 0, 2, &context.one()),
            SymbolicPolynomialPredicateKind::EqualZero,
        )],
    );

    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_polynomial_terms_inspected = 1;
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(&context, &partition, leaf, limits),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus polynomial terms inspected",
            requested: 2,
            limit: 1,
        })
    );

    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_exponent_entries_inspected = 5;
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(&context, &partition, leaf, limits),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus exponent entries inspected",
            requested: 6,
            limit: 5,
        })
    );

    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_recognition_operations = 11;
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(&context, &partition, leaf, limits),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus recognition operations",
            requested: 12,
            limit: 11,
        })
    );

    let mut limits = CoordinateEqualityLocusLimits::default();
    limits.max_integer_coefficient_bits = 1;
    assert_eq!(
        CoordinateEqualityLocusExtractor::extract(&context, &partition, leaf, limits),
        Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus integer coefficient bits",
            requested: 2,
            limit: 1,
        })
    );
}
