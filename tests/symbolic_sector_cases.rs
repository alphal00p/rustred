use rustred::{
    CoefficientContext, ParametricArithmeticLimits, ParametricCoefficient,
    ParametricCoefficientContext, ParametricPolynomial, SectorMask, SectorOrthantSide,
    SymbolicPolynomialPredicate, SymbolicPolynomialPredicateKind, SymbolicSectorCaseError,
    SymbolicSectorCaseId, SymbolicSectorCaseLimits, SymbolicSectorCasePartitionBuilder,
};
use std::sync::Arc;

fn make_context(scope: &str) -> ParametricCoefficientContext {
    ParametricCoefficientContext::try_new(&CoefficientContext::new(Vec::<String>::new()), scope, 2)
        .unwrap()
}

fn affine_index_coefficient(
    context: &ParametricCoefficientContext,
    index: usize,
    constant: i64,
) -> ParametricCoefficient {
    context
        .add(&context.index(index).unwrap(), &context.integer(constant))
        .unwrap()
}

fn polynomial(
    context: &ParametricCoefficientContext,
    index: usize,
    constant: i64,
) -> ParametricPolynomial {
    context
        .numerator_condition(&affine_index_coefficient(context, index, constant))
        .unwrap()
}

fn predicate_holds(
    context: &ParametricCoefficientContext,
    predicate: &SymbolicPolynomialPredicate,
    assignment: &[i64],
) -> bool {
    let specialized = context
        .specialize_polynomial(
            predicate.polynomial(),
            assignment,
            ParametricArithmeticLimits::default(),
        )
        .unwrap();
    match predicate.kind() {
        SymbolicPolynomialPredicateKind::EqualZero => specialized.is_zero(),
        // Base parameters are formal elements of K=Q(theta).  A specialized
        // nonzero polynomial such as theta is nonzero in K even though it is
        // not an integer constant in the ambient polynomial storage.
        SymbolicPolynomialPredicateKind::NonZero => !specialized.is_zero(),
    }
}

fn four_leaf_partition(
    context: &ParametricCoefficientContext,
    sector: SectorMask,
) -> rustred::SymbolicSectorCasePartitionCertificate {
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        context,
        sector,
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let first = builder
        .split_on_bad_polynomial(context, builder.root_case(), polynomial(context, 0, -2))
        .unwrap();
    builder
        .split_on_bad_polynomial(context, first.equal_zero_case(), polynomial(context, 1, 1))
        .unwrap();
    builder
        .split_on_bad_polynomial(context, first.nonzero_case(), polynomial(context, 1, 1))
        .unwrap();
    builder.finish(context).unwrap()
}

#[test]
fn source_identity_is_exact_shared_and_budgeted_once_at_freeze() {
    let context = make_context("symbolic-sector-source-identity");
    let sector = SectorMask::try_new([true, false]).unwrap();

    let build = |limits: SymbolicSectorCaseLimits| {
        let mut builder =
            SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), limits).unwrap();
        builder
            .split_on_bad_polynomial(&context, builder.root_case(), polynomial(&context, 0, -2))
            .unwrap();
        builder.finish(&context)
    };

    let certificate = build(SymbolicSectorCaseLimits::default()).unwrap();
    let independently_rebuilt = build(SymbolicSectorCaseLimits::default()).unwrap();
    assert_eq!(
        certificate.source_identity().as_ref(),
        independently_rebuilt.source_identity().as_ref()
    );
    let cloned = certificate.clone();
    assert!(Arc::ptr_eq(
        certificate.source_identity(),
        cloned.source_identity()
    ));

    let identity_bytes = certificate.source_identity().len();
    assert!(identity_bytes > 0);
    let mut exact = SymbolicSectorCaseLimits::default();
    exact.max_source_identity_bytes = identity_bytes;
    assert_eq!(
        build(exact).unwrap().source_identity().len(),
        identity_bytes
    );

    let mut one_short = exact;
    one_short.max_source_identity_bytes = identity_bytes - 1;
    assert!(matches!(
        build(one_short),
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "symbolic partition source identity bytes",
            ..
        })
    ));

    let other_sector = four_leaf_partition(&context, SectorMask::try_new([false, false]).unwrap());
    assert_ne!(
        certificate.source_identity().as_ref(),
        other_sector.source_identity().as_ref()
    );
}

#[test]
fn complementary_splits_are_a_replayable_disjoint_cover_of_the_integer_orthant() {
    let context = make_context("symbolic-sector-case-cover");
    let sector = SectorMask::try_new([true, false]).unwrap();
    let certificate = four_leaf_partition(&context, sector.clone());

    certificate.replay(&context, &sector).unwrap();
    assert_eq!(certificate.cases().len(), 4);
    assert_eq!(certificate.splits().len(), 3);
    assert_eq!(certificate.stats().split_count(), 3);
    assert_eq!(certificate.stats().leaf_count(), 4);
    assert_eq!(certificate.stats().max_depth(), 2);
    assert_eq!(certificate.stats().total_leaf_predicates(), 8);
    assert_eq!(certificate.stats().retained_polynomial_terms(), 6);
    assert!(certificate.stats().retained_polynomial_bytes() > 0);

    let constraints = certificate.orthant().constraints();
    assert_eq!(constraints.len(), 2);
    assert_eq!(constraints[0].index(), 0);
    assert_eq!(constraints[0].side(), SectorOrthantSide::AtLeastOne);
    assert_eq!(constraints[1].index(), 1);
    assert_eq!(constraints[1].side(), SectorOrthantSide::AtMostZero);
    assert!(
        certificate
            .orthant()
            .contains_integer_point(&[1, 0])
            .unwrap()
    );
    assert!(
        !certificate
            .orthant()
            .contains_integer_point(&[0, 0])
            .unwrap()
    );
    assert!(
        !certificate
            .orthant()
            .contains_integer_point(&[1, 1])
            .unwrap()
    );

    // With an empty base-parameter field, specializing the predicates makes
    // each branch decidable.  Every sampled integer in the orthant belongs to
    // exactly one final conjunction.
    for n0 in 1..=4 {
        for n1 in -3..=0 {
            let assignment = [n0, n1];
            let matching = certificate
                .cases()
                .iter()
                .filter(|case| {
                    case.predicates()
                        .iter()
                        .all(|predicate| predicate_holds(&context, predicate, &assignment))
                })
                .count();
            assert_eq!(matching, 1, "assignment {assignment:?}");
        }
    }

    // Allocation, branch order, IDs, predicates, and proof transcript are
    // deterministic for the same K(n) context and split sequence.
    assert_eq!(certificate, four_leaf_partition(&context, sector.clone()));
    assert_eq!(
        certificate.splits()[0].children().equal_zero_case().value(),
        1
    );
    assert_eq!(certificate.splits()[0].children().nonzero_case().value(), 2);
}

#[test]
fn split_polynomial_payloads_are_shared_by_transcript_and_descendant_leaves() {
    let context = make_context("symbolic-sector-shared-polynomial-payload");
    let sector = SectorMask::try_new([true, false]).unwrap();
    let certificate = four_leaf_partition(&context, sector);

    let root_polynomial = certificate.splits()[0].bad_polynomial();
    for case in certificate.cases() {
        let retained = case
            .predicates()
            .iter()
            .find(|predicate| predicate.polynomial() == root_polynomial)
            .expect("every final leaf descends from the root split");
        assert!(std::ptr::eq(retained.polynomial(), root_polynomial));
    }

    for split in &certificate.splits()[1..] {
        for child in [
            split.children().equal_zero_case(),
            split.children().nonzero_case(),
        ] {
            let case = certificate.case(child).unwrap();
            let retained = case
                .predicates()
                .iter()
                .find(|predicate| predicate.polynomial() == split.bad_polynomial())
                .unwrap();
            assert!(std::ptr::eq(retained.polynomial(), split.bad_polynomial()));
        }
    }

    let cloned = certificate.clone();
    assert!(std::ptr::eq(
        certificate.splits()[0].bad_polynomial(),
        cloned.splits()[0].bad_polynomial(),
    ));
}

#[test]
fn shared_polynomial_payload_term_limit_is_exact_and_transactional() {
    let context = make_context("symbolic-sector-shared-polynomial-term-limit");
    let sector = SectorMask::try_new([true, false]).unwrap();
    let first_polynomial = polynomial(&context, 0, -2);
    let second_polynomial = polynomial(&context, 1, 1);
    let exact_terms = first_polynomial
        .term_count()
        .checked_add(second_polynomial.term_count())
        .unwrap();

    let mut exact_limits = SymbolicSectorCaseLimits::default();
    exact_limits.max_retained_polynomial_terms = exact_terms;
    let mut exact =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), exact_limits)
            .unwrap();
    let exact_first = exact
        .split_on_bad_polynomial(&context, exact.root_case(), first_polynomial.clone())
        .unwrap();
    exact
        .split_on_bad_polynomial(
            &context,
            exact_first.equal_zero_case(),
            second_polynomial.clone(),
        )
        .unwrap();
    assert_eq!(exact.stats().retained_polynomial_terms(), exact_terms);

    let mut one_below_limits = exact_limits;
    one_below_limits.max_retained_polynomial_terms = exact_terms - 1;
    let mut one_below =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector, one_below_limits).unwrap();
    let first = one_below
        .split_on_bad_polynomial(&context, one_below.root_case(), first_polynomial)
        .unwrap();
    let stats_before = one_below.stats();
    let cases_before = one_below.live_cases().cloned().collect::<Vec<_>>();
    assert_eq!(
        one_below.split_on_bad_polynomial(&context, first.equal_zero_case(), second_polynomial,),
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "retained symbolic predicate terms",
            requested: exact_terms,
            limit: exact_terms - 1,
        })
    );
    assert_eq!(one_below.stats(), stats_before);
    assert_eq!(
        one_below.live_cases().cloned().collect::<Vec<_>>(),
        cases_before
    );
}

#[test]
fn descendant_reference_and_shared_payload_byte_limits_are_exact_and_transactional() {
    let context = make_context("symbolic-sector-descendant-reference-limits");
    let sector = SectorMask::try_new([true, false]).unwrap();
    let first_polynomial = polynomial(&context, 0, -2);
    let second_polynomial = polynomial(&context, 1, 1);

    // A root split retains two predicate references. Splitting one depth-one
    // leaf replaces its one-reference path by two two-reference paths, for an
    // exact cumulative census of five references.
    let mut exact_reference_limits = SymbolicSectorCaseLimits::default();
    exact_reference_limits.max_total_leaf_predicates = 5;
    let mut exact_references = SymbolicSectorCasePartitionBuilder::try_new(
        &context,
        sector.clone(),
        exact_reference_limits,
    )
    .unwrap();
    let first = exact_references
        .split_on_bad_polynomial(
            &context,
            exact_references.root_case(),
            first_polynomial.clone(),
        )
        .unwrap();
    exact_references
        .split_on_bad_polynomial(&context, first.equal_zero_case(), second_polynomial.clone())
        .unwrap();
    assert_eq!(exact_references.stats().total_leaf_predicates(), 5);

    let mut one_below_reference_limits = exact_reference_limits;
    one_below_reference_limits.max_total_leaf_predicates = 4;
    let mut limited_references = SymbolicSectorCasePartitionBuilder::try_new(
        &context,
        sector.clone(),
        one_below_reference_limits,
    )
    .unwrap();
    let first = limited_references
        .split_on_bad_polynomial(
            &context,
            limited_references.root_case(),
            first_polynomial.clone(),
        )
        .unwrap();
    let stats_before = limited_references.stats();
    let cases_before = limited_references.live_cases().cloned().collect::<Vec<_>>();
    assert_eq!(
        limited_references.split_on_bad_polynomial(
            &context,
            first.equal_zero_case(),
            second_polynomial.clone(),
        ),
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "total symbolic leaf predicates",
            requested: 5,
            limit: 4,
        })
    );
    assert_eq!(limited_references.stats(), stats_before);
    assert_eq!(
        limited_references.live_cases().cloned().collect::<Vec<_>>(),
        cases_before
    );

    // Check the canonical-display byte census independently at depth two.
    // Only the two transcript payloads are charged; descendant Arc references
    // are not.
    let expected_bytes = first_polynomial
        .raw()
        .to_string()
        .len()
        .checked_add(second_polynomial.raw().to_string().len())
        .unwrap();
    let mut calibrated = SymbolicSectorCasePartitionBuilder::try_new(
        &context,
        sector.clone(),
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let first = calibrated
        .split_on_bad_polynomial(&context, calibrated.root_case(), first_polynomial.clone())
        .unwrap();
    let first_bytes = calibrated.stats().retained_polynomial_bytes();
    calibrated
        .split_on_bad_polynomial(&context, first.equal_zero_case(), second_polynomial.clone())
        .unwrap();
    let exact_bytes = calibrated.stats().retained_polynomial_bytes();
    assert_eq!(exact_bytes, expected_bytes);
    assert!(exact_bytes > first_bytes);

    let mut exact_byte_limits = SymbolicSectorCaseLimits::default();
    exact_byte_limits.max_retained_polynomial_bytes = exact_bytes;
    let mut exact_bytes_builder =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), exact_byte_limits)
            .unwrap();
    let first = exact_bytes_builder
        .split_on_bad_polynomial(
            &context,
            exact_bytes_builder.root_case(),
            first_polynomial.clone(),
        )
        .unwrap();
    exact_bytes_builder
        .split_on_bad_polynomial(&context, first.equal_zero_case(), second_polynomial.clone())
        .unwrap();
    assert_eq!(
        exact_bytes_builder.stats().retained_polynomial_bytes(),
        exact_bytes
    );

    let mut one_below_byte_limits = exact_byte_limits;
    one_below_byte_limits.max_retained_polynomial_bytes = exact_bytes - 1;
    let mut limited_bytes =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector, one_below_byte_limits)
            .unwrap();
    let first = limited_bytes
        .split_on_bad_polynomial(&context, limited_bytes.root_case(), first_polynomial)
        .unwrap();
    let stats_before = limited_bytes.stats();
    let cases_before = limited_bytes.live_cases().cloned().collect::<Vec<_>>();
    assert_eq!(
        limited_bytes
            .split_on_bad_polynomial(&context, first.equal_zero_case(), second_polynomial,),
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "retained symbolic predicate bytes",
            requested: exact_bytes,
            limit: exact_bytes - 1,
        })
    );
    assert_eq!(limited_bytes.stats(), stats_before);
    assert_eq!(
        limited_bytes.live_cases().cloned().collect::<Vec<_>>(),
        cases_before
    );
}

#[test]
fn context_sector_trivial_and_repeated_predicate_attacks_are_typed() {
    let context = make_context("symbolic-sector-case-binding");
    let other = make_context("symbolic-sector-case-other");
    let sector = SectorMask::try_new([true, false]).unwrap();
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        &context,
        sector.clone(),
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let p = polynomial(&context, 0, -2);

    assert_eq!(
        builder.split_on_bad_polynomial(&other, builder.root_case(), p.clone()),
        Err(SymbolicSectorCaseError::ContextMismatch)
    );
    let zero = context.numerator_condition(&context.zero()).unwrap();
    assert_eq!(
        builder.split_on_bad_polynomial(&context, builder.root_case(), zero),
        Err(SymbolicSectorCaseError::IdenticallyZeroSplitPolynomial)
    );
    let one = context.numerator_condition(&context.one()).unwrap();
    assert_eq!(
        builder.split_on_bad_polynomial(&context, builder.root_case(), one),
        Err(SymbolicSectorCaseError::NonzeroConstantSplitPolynomial)
    );

    let children = builder
        .split_on_bad_polynomial(&context, builder.root_case(), p.clone())
        .unwrap();
    assert_eq!(
        builder.split_on_bad_polynomial(&context, children.equal_zero_case(), p),
        Err(SymbolicSectorCaseError::PredicateAlreadyDecided {
            case: children.equal_zero_case(),
            kind: SymbolicPolynomialPredicateKind::EqualZero,
        })
    );
    assert_eq!(
        builder.split_on_bad_polynomial(
            &context,
            SymbolicSectorCaseId::ROOT,
            polynomial(&context, 1, 1),
        ),
        Err(SymbolicSectorCaseError::CaseNotLive {
            case: SymbolicSectorCaseId::ROOT,
        })
    );

    let certificate = builder.finish(&context).unwrap();
    assert_eq!(
        certificate.replay(&other, &sector),
        Err(SymbolicSectorCaseError::ContextMismatch)
    );
    assert_eq!(
        certificate.replay(&context, &SectorMask::try_new([false, false]).unwrap()),
        Err(SymbolicSectorCaseError::SectorMismatch)
    );
    assert_eq!(
        SymbolicSectorCasePartitionBuilder::try_new(
            &context,
            SectorMask::try_new([true]).unwrap(),
            SymbolicSectorCaseLimits::default(),
        )
        .unwrap_err(),
        SymbolicSectorCaseError::WrongIndexArity {
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn base_only_polynomials_are_constants_in_the_kn_index_ring() {
    let base = CoefficientContext::new(["theta"]);
    let context =
        ParametricCoefficientContext::try_new(&base, "symbolic-sector-base-constant", 1).unwrap();
    let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
    let theta_polynomial = context.numerator_condition(&theta).unwrap();
    assert!(
        !context
            .polynomial_depends_on_indices(&theta_polynomial)
            .unwrap()
    );

    let sector = SectorMask::try_new([true]).unwrap();
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        &context,
        sector,
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        builder.split_on_bad_polynomial(&context, builder.root_case(), theta_polynomial,),
        Err(SymbolicSectorCaseError::NonzeroConstantSplitPolynomial)
    );

    let theta_n = context.mul(&theta, &context.index(0).unwrap()).unwrap();
    let theta_n_polynomial = context.numerator_condition(&theta_n).unwrap();
    assert!(
        context
            .polynomial_depends_on_indices(&theta_n_polynomial)
            .unwrap()
    );
    builder
        .split_on_bad_polynomial(&context, builder.root_case(), theta_n_polynomial)
        .unwrap();
}

#[test]
fn pivot_numerator_helper_retains_the_exact_bad_locus() {
    let context = make_context("symbolic-sector-case-pivot");
    let sector = SectorMask::try_new([true, false]).unwrap();
    let numerator = affine_index_coefficient(&context, 0, -2);
    let denominator = affine_index_coefficient(&context, 1, -3);
    let pivot = context.checked_div(&numerator, &denominator).unwrap();
    let expected_bad = context.numerator_condition(&pivot).unwrap();

    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        &context,
        sector,
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let children = builder
        .split_on_pivot_coefficient(&context, builder.root_case(), &pivot)
        .unwrap();
    let bad = builder
        .live_cases()
        .find(|case| case.id() == children.equal_zero_case())
        .unwrap();
    assert_eq!(bad.predicates().len(), 1);
    assert_eq!(
        bad.predicates()[0].kind(),
        SymbolicPolynomialPredicateKind::EqualZero
    );
    assert_eq!(bad.predicates()[0].polynomial(), &expected_bad);
}

#[test]
fn resource_failures_are_preflighted_and_transactional() {
    let context = make_context("symbolic-sector-case-limits");
    let sector = SectorMask::try_new([true, false]).unwrap();

    let mut limits = SymbolicSectorCaseLimits::default();
    limits.max_indices = 1;
    assert_eq!(
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), limits).unwrap_err(),
        SymbolicSectorCaseError::ResourceLimit {
            resource: "symbolic sector indices",
            requested: 2,
            limit: 1,
        }
    );

    let mut limits = SymbolicSectorCaseLimits::default();
    limits.max_context_fingerprint_bytes = context.fingerprint().len() - 1;
    assert_eq!(
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), limits).unwrap_err(),
        SymbolicSectorCaseError::ResourceLimit {
            resource: "symbolic sector context fingerprint bytes",
            requested: context.fingerprint().len(),
            limit: context.fingerprint().len() - 1,
        }
    );

    let mut limits = SymbolicSectorCaseLimits::default();
    limits.max_splits = 0;
    let mut builder =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), limits).unwrap();
    let before = builder.stats();
    assert_eq!(
        builder
            .split_on_bad_polynomial(&context, builder.root_case(), polynomial(&context, 0, -2),),
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "symbolic sector case splits",
            requested: 1,
            limit: 0,
        })
    );
    assert_eq!(builder.stats(), before);
    assert_eq!(builder.live_cases().count(), 1);

    let mut limits = SymbolicSectorCaseLimits::default();
    limits.max_total_leaf_predicates = 1;
    let mut builder =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), limits).unwrap();
    assert_eq!(
        builder
            .split_on_bad_polynomial(&context, builder.root_case(), polynomial(&context, 0, -2),),
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "total symbolic leaf predicates",
            requested: 2,
            limit: 1,
        })
    );

    let retained_terms = polynomial(&context, 0, -2).term_count();
    let mut limits = SymbolicSectorCaseLimits::default();
    limits.max_retained_polynomial_terms = retained_terms - 1;
    let mut builder =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), limits).unwrap();
    let before = builder.stats();
    assert_eq!(
        builder
            .split_on_bad_polynomial(&context, builder.root_case(), polynomial(&context, 0, -2),),
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "retained symbolic predicate terms",
            requested: retained_terms,
            limit: retained_terms - 1,
        })
    );
    assert_eq!(builder.stats(), before);
    assert_eq!(builder.live_cases().count(), 1);

    let mut limits = SymbolicSectorCaseLimits::default();
    limits.max_live_cases = 0;
    assert_eq!(
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector, limits).unwrap_err(),
        SymbolicSectorCaseError::ResourceLimit {
            resource: "live symbolic sector cases",
            requested: 1,
            limit: 0,
        }
    );

    let sector = SectorMask::try_new([true, false]).unwrap();
    let mut calibrated = SymbolicSectorCasePartitionBuilder::try_new(
        &context,
        sector.clone(),
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    calibrated
        .split_on_bad_polynomial(
            &context,
            calibrated.root_case(),
            polynomial(&context, 0, -2),
        )
        .unwrap();
    let exact_bytes = calibrated.stats().retained_polynomial_bytes();
    assert!(exact_bytes > 0);

    let mut limits = SymbolicSectorCaseLimits::default();
    limits.max_retained_polynomial_bytes = exact_bytes;
    let mut exact =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector.clone(), limits).unwrap();
    exact
        .split_on_bad_polynomial(&context, exact.root_case(), polynomial(&context, 0, -2))
        .unwrap();

    limits.max_retained_polynomial_bytes = exact_bytes - 1;
    let mut limited =
        SymbolicSectorCasePartitionBuilder::try_new(&context, sector, limits).unwrap();
    let before = limited.stats();
    assert_eq!(
        limited
            .split_on_bad_polynomial(&context, limited.root_case(), polynomial(&context, 0, -2),),
        Err(SymbolicSectorCaseError::ResourceLimit {
            resource: "retained symbolic predicate bytes",
            requested: exact_bytes,
            limit: exact_bytes - 1,
        })
    );
    assert_eq!(limited.stats(), before);
    assert_eq!(limited.live_cases().count(), 1);
}
