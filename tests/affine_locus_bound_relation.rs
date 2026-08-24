use std::sync::Arc;

use rustred::{
    AffineLocusBoundRelationCompilation, AffineLocusBoundRelationCompiler,
    AffineLocusBoundRelationError, AffineLocusBoundRelationLimits,
    AffineLocusConcreteSpecializationLimits, AffineLocusUnavailableReason, CoefficientContext,
    CoordinateEqualityLocusCertificate, CoordinateEqualityLocusExtractor,
    CoordinateEqualityLocusLimits, IndexShift, ParametricCoefficientContext, ParametricRelation,
    ParametricRowId, ResidualUnitAffineIndexMapCertificate, ResidualUnitAffineIndexMapLimits,
    SectorMask, SymbolicPolynomialPredicateKind, SymbolicSectorCaseLimits,
    SymbolicSectorCasePartitionBuilder,
};

fn context(scope: &str) -> ParametricCoefficientContext {
    ParametricCoefficientContext::try_new(&CoefficientContext::new(["d"]), scope, 3).unwrap()
}

fn affine_polynomial(context: &ParametricCoefficientContext) -> rustred::ParametricCoefficient {
    let sum = context
        .add(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    context.sub(&sum, &context.integer(3)).unwrap()
}

fn affine_map(
    context: &ParametricCoefficientContext,
) -> Arc<ResidualUnitAffineIndexMapCertificate> {
    affine_map_with_n1_exclusion(context, false)
}

fn affine_map_with_n1_exclusion(
    context: &ParametricCoefficientContext,
    exclude_n1_equals_one: bool,
) -> Arc<ResidualUnitAffineIndexMapCertificate> {
    let affine = affine_polynomial(context);
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let base_factor = context.add(&d, &context.one()).unwrap();
    let predicate = context.mul(&base_factor, &affine).unwrap();
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        context,
        SectorMask::try_new([true, true, true]).unwrap(),
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let mut leaf = builder.root_case();
    if exclude_n1_equals_one {
        let excluded = context
            .sub(&context.index(1).unwrap(), &context.integer(1))
            .unwrap();
        leaf = builder
            .split_on_bad_polynomial(
                context,
                leaf,
                context.numerator_condition(&excluded).unwrap(),
            )
            .unwrap()
            .nonzero_case();
    }
    let leaf = builder
        .split_on_bad_polynomial(
            context,
            leaf,
            context.numerator_condition(&predicate).unwrap(),
        )
        .unwrap()
        .equal_zero_case();
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
    let predicate_ordinal = dependent_equality_ordinal(&source);
    Arc::new(
        ResidualUnitAffineIndexMapCertificate::compile(
            context,
            source,
            predicate_ordinal,
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        )
        .unwrap(),
    )
}

fn dependent_equality_ordinal(source: &CoordinateEqualityLocusCertificate) -> usize {
    source
        .unresolved_predicates()
        .iter()
        .find(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
        .unwrap()
        .predicate_ordinal()
}

fn source_row(context: &ParametricCoefficientContext, label: &str) -> ParametricRelation {
    ParametricRelation::new(
        "affine-locus-test-family",
        ParametricRowId::Derived {
            label: Arc::from(label),
        },
        context,
    )
}

fn derived(label: &str) -> ParametricRowId {
    ParametricRowId::Derived {
        label: Arc::from(label),
    }
}

#[test]
fn compiler_enforces_translate_then_compose_and_replays() {
    let context = context("affine-locus-operation-order");
    let map = affine_map(&context);
    let mut source = source_row(&context, "source-n0");
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            context.index(0).unwrap(),
        )
        .unwrap();
    let source = Arc::new(source);
    let translation = IndexShift::try_new([1, 1, 0], 3).unwrap();
    let compilation = AffineLocusBoundRelationCompiler::compile(
        &context,
        source.clone(),
        translation.clone(),
        derived("target-order"),
        map,
        AffineLocusBoundRelationLimits::default(),
    )
    .unwrap();
    let AffineLocusBoundRelationCompilation::Retained(bound) = compilation else {
        panic!("the translated row must be available on the affine locus");
    };
    bound.replay(&context).unwrap();

    let specialized = bound
        .specialize_at_free_values(
            &context,
            &[1, 1],
            AffineLocusConcreteSpecializationLimits::default(),
        )
        .unwrap();
    let direct = source
        .specialize(
            &context,
            &[3, 2, 1],
            AffineLocusConcreteSpecializationLimits::default().arithmetic,
        )
        .unwrap();
    assert_eq!(
        specialized.family_fingerprint(),
        direct.family_fingerprint()
    );
    assert_eq!(specialized.terms(), direct.terms());
    assert_eq!(
        specialized.nonzero_conditions(),
        direct.nonzero_conditions()
    );
    assert_eq!(specialized.terms().len(), 1);
}

#[test]
fn mapped_zero_source_guard_is_a_typed_unavailable_row() {
    let context = context("affine-locus-zero-guard");
    let map = affine_map(&context);
    let mut source = source_row(&context, "source-zero-guard");
    source
        .add_nonzero_condition(
            &context,
            context
                .numerator_condition(&affine_polynomial(&context))
                .unwrap(),
        )
        .unwrap();
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            context.one(),
        )
        .unwrap();
    let compilation = AffineLocusBoundRelationCompiler::compile(
        &context,
        Arc::new(source),
        IndexShift::try_new([0, 0, 0], 3).unwrap(),
        derived("target-zero-guard"),
        map,
        AffineLocusBoundRelationLimits::default(),
    )
    .unwrap();
    let AffineLocusBoundRelationCompilation::Unavailable(unavailable) = compilation else {
        panic!("a guard equal to the affine equality must make only the row unavailable");
    };
    assert_eq!(
        unavailable.reason(),
        &AffineLocusUnavailableReason::SourceGuardComposesToZero { guard_ordinal: 0 }
    );
    assert_eq!(unavailable.stats().polynomial_compositions(), 1);
    unavailable.replay(&context).unwrap();
}

#[test]
fn a_term_denominator_that_maps_to_zero_is_not_misread_as_a_zero_integral() {
    let context = context("affine-locus-zero-term-denominator");
    let map = affine_map(&context);
    let singular = context
        .checked_div(&context.one(), &affine_polynomial(&context))
        .unwrap();
    let mut source = source_row(&context, "source-zero-term-denominator");
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            singular,
        )
        .unwrap();
    let compilation = AffineLocusBoundRelationCompiler::compile(
        &context,
        Arc::new(source),
        IndexShift::try_new([0, 0, 0], 3).unwrap(),
        derived("target-zero-term-denominator"),
        map,
        AffineLocusBoundRelationLimits::default(),
    )
    .unwrap();
    let AffineLocusBoundRelationCompilation::Unavailable(unavailable) = compilation else {
        panic!("a singular source term must make the row unavailable");
    };
    // Complete-row translation deliberately maps guards before coefficients,
    // so the retained source denominator is the first exact witness.
    assert_eq!(
        unavailable.reason(),
        &AffineLocusUnavailableReason::SourceGuardComposesToZero { guard_ordinal: 0 }
    );
    unavailable.replay(&context).unwrap();
}

#[test]
fn base_assumptions_are_retained_outside_the_private_index_guard_set() {
    let context = context("affine-locus-base-assumption");
    let map = affine_map(&context);
    let d = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let condition = context.add(&affine_polynomial(&context), &d).unwrap();
    let mut source = source_row(&context, "source-base-assumption");
    source
        .add_nonzero_condition(&context, context.numerator_condition(&condition).unwrap())
        .unwrap();
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            context.one(),
        )
        .unwrap();
    let compilation = AffineLocusBoundRelationCompiler::compile(
        &context,
        Arc::new(source),
        IndexShift::try_new([0, 0, 0], 3).unwrap(),
        derived("target-base-assumption"),
        map,
        AffineLocusBoundRelationLimits::default(),
    )
    .unwrap();
    let AffineLocusBoundRelationCompilation::Retained(bound) = compilation else {
        panic!("d != 0 is a formal-base assumption, not an unavailable row");
    };
    assert_eq!(bound.base_assumptions().len(), 1);
    assert!(
        !context
            .polynomial_depends_on_indices(bound.base_assumptions()[0].condition().polynomial())
            .unwrap()
    );
    let concrete = bound
        .specialize_at_free_values(
            &context,
            &[1, 1],
            AffineLocusConcreteSpecializationLimits::default(),
        )
        .unwrap();
    assert_eq!(concrete.nonzero_conditions().len(), 1);
    assert_eq!(
        concrete.nonzero_conditions()[0].to_expression(),
        context.base().parameter("d").unwrap().to_expression()
    );
}

#[test]
fn public_specialization_checks_the_full_source_orthant() {
    let context = context("affine-locus-source-case-boundary");
    let map = affine_map(&context);
    let mut source = source_row(&context, "source-boundary");
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            context.one(),
        )
        .unwrap();
    let AffineLocusBoundRelationCompilation::Retained(bound) =
        AffineLocusBoundRelationCompiler::compile(
            &context,
            Arc::new(source),
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            derived("target-boundary"),
            map,
            AffineLocusBoundRelationLimits::default(),
        )
        .unwrap()
    else {
        panic!("the symbolic row must compile");
    };
    assert!(matches!(
        bound.specialize_at_free_values(
            &context,
            &[3, 2],
            AffineLocusConcreteSpecializationLimits::default(),
        ),
        Err(AffineLocusBoundRelationError::ConcretePointOutsideSourceOrthant)
    ));
}

#[test]
fn public_specialization_checks_every_source_case_predicate() {
    let context = context("affine-locus-source-predicate-boundary");
    let map = affine_map_with_n1_exclusion(&context, true);
    let mut source = source_row(&context, "source-predicate-boundary");
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            context.one(),
        )
        .unwrap();
    let AffineLocusBoundRelationCompilation::Retained(bound) =
        AffineLocusBoundRelationCompiler::compile(
            &context,
            Arc::new(source),
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            derived("target-predicate-boundary"),
            map,
            AffineLocusBoundRelationLimits::default(),
        )
        .unwrap()
    else {
        panic!("the symbolic row must compile");
    };
    assert!(matches!(
        bound.specialize_at_free_values(
            &context,
            &[1, 2],
            AffineLocusConcreteSpecializationLimits::default(),
        ),
        Err(
            AffineLocusBoundRelationError::ConcretePointOutsideSourceCase {
                predicate_ordinal: 0
            }
        )
    ));
}

#[test]
fn row_wide_aggregate_budget_rejects_two_coefficient_halves() {
    let context = context("affine-locus-aggregate-budget");
    let map = affine_map(&context);
    let mut source = source_row(&context, "source-budget");
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            context.index(0).unwrap(),
        )
        .unwrap();
    let mut limits = AffineLocusBoundRelationLimits::default();
    limits.max_total_source_terms = 1;
    assert!(matches!(
        AffineLocusBoundRelationCompiler::compile(
            &context,
            Arc::new(source),
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            derived("target-budget"),
            map,
            limits,
        ),
        Err(AffineLocusBoundRelationError::Composition(_))
    ));
}

#[test]
fn row_wide_integer_budget_charges_the_durable_mapped_denominator_copy() {
    let context = context("affine-locus-durable-denominator-budget");
    let map = affine_map(&context);
    let denominator = context
        .sub(
            &context
                .add(&context.index(0).unwrap(), &context.index(2).unwrap())
                .unwrap(),
            &context.integer(2),
        )
        .unwrap();
    let coefficient = context.checked_div(&context.one(), &denominator).unwrap();
    let mut source = source_row(&context, "source-durable-denominator-budget");
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            coefficient,
        )
        .unwrap();
    let source = Arc::new(source);
    let translation = IndexShift::try_new([0, 0, 0], 3).unwrap();
    let target = derived("target-durable-denominator-budget");
    let AffineLocusBoundRelationCompilation::Retained(unrestricted) =
        AffineLocusBoundRelationCompiler::compile(
            &context,
            source.clone(),
            translation.clone(),
            target.clone(),
            map.clone(),
            AffineLocusBoundRelationLimits::default(),
        )
        .unwrap()
    else {
        panic!("the mapped denominator is nonzero and index dependent");
    };
    assert!(unrestricted.stats().durable_guard_terms() > 0);
    assert!(unrestricted.stats().durable_guard_integer_bit_payload() > 0);

    let mut strict = AffineLocusBoundRelationLimits::default();
    strict.max_total_integer_bit_work = unrestricted.stats().integer_bit_work_bound() - 1;
    assert!(matches!(
        AffineLocusBoundRelationCompiler::compile(
            &context,
            source,
            translation,
            target,
            map,
            strict,
        ),
        Err(AffineLocusBoundRelationError::Composition(_))
            | Err(AffineLocusBoundRelationError::ResourceLimit {
                resource: "integer bit work",
                ..
            })
    ));
}

#[test]
fn row_wide_budget_charges_copied_guard_origin_payloads() {
    let context = context("affine-locus-guard-origin-byte-budget");
    let map = affine_map(&context);
    let guard_polynomial = context
        .sub(
            &context
                .add(&context.index(0).unwrap(), &context.index(2).unwrap())
                .unwrap(),
            &context.integer(2),
        )
        .unwrap();
    let mut source = source_row(&context, "source-guard-origin-byte-budget");
    source
        .add_nonzero_condition(
            &context,
            context.numerator_condition(&guard_polynomial).unwrap(),
        )
        .unwrap();
    source
        .add_term(
            &context,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            context.one(),
        )
        .unwrap();
    let source = Arc::new(source);
    let translation = IndexShift::try_new([0, 0, 0], 3).unwrap();
    let target = derived("target-guard-origin-byte-budget");
    let AffineLocusBoundRelationCompilation::Retained(unrestricted) =
        AffineLocusBoundRelationCompiler::compile(
            &context,
            source.clone(),
            translation.clone(),
            target.clone(),
            map.clone(),
            AffineLocusBoundRelationLimits::default(),
        )
        .unwrap()
    else {
        panic!("the mapped source guard is nonzero on the affine locus");
    };
    assert!(unrestricted.stats().guard_origin_retained_bytes() > 0);

    let mut strict = AffineLocusBoundRelationLimits::default();
    strict.max_total_guard_origin_retained_bytes =
        unrestricted.stats().guard_origin_retained_bytes() - 1;
    assert!(matches!(
        AffineLocusBoundRelationCompiler::compile(
            &context,
            source,
            translation,
            target,
            map,
            strict,
        ),
        Err(AffineLocusBoundRelationError::Composition(_))
            | Err(AffineLocusBoundRelationError::ResourceLimit {
                resource: "guard origin retained bytes",
                ..
            })
    ));
}
