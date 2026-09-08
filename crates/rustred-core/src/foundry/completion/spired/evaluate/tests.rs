use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::{Field, Ring};

use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSource,
    TranslatedSourceLimits, TranslatedSourceRequest,
};

use super::backend::evaluate_polynomial;
use super::{
    DirectShiftedSourceError, DirectShiftedSourceEvaluator, DirectShiftedSourceLimits,
    ShiftedModularSourceBuffer,
};

const PRIME: u64 = 101;

fn unit_tadpole(name: &str) -> IntegralFamily {
    let base = CoefficientContext::try_new(["d"]).unwrap();
    let one = base.one();
    let minus_one = base.try_neg(&one, Default::default()).unwrap();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(minus_one, vec![one])],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap()
}

fn guarded_tadpole(name: &str) -> IntegralFamily {
    let base = CoefficientContext::try_new(["d", "x"]).unwrap();
    let reciprocal = base
        .try_div(
            &base.one(),
            &base.parameter("x").unwrap(),
            Default::default(),
        )
        .unwrap();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![reciprocal],
    )
    .unwrap()
}

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let base = CoefficientContext::try_new(["d", "s"]).unwrap();
    let zero = base.zero();
    let one = base.one();
    let minus_s = base
        .try_neg(&base.parameter("s").unwrap(), Default::default())
        .unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_s.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_s.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_s, vec![one.clone(), base.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn request(
    source_ordinal: usize,
    offset: impl IntoIterator<Item = i64>,
) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(source_ordinal, IntegralShift::try_new(offset).unwrap())
}

fn direct_image(buffer: &ShiftedModularSourceBuffer) -> Vec<(Vec<i64>, u64)> {
    buffer
        .terms()
        .map(|term| (term.structural_shift().to_vec(), term.residue()))
        .collect()
}

fn exact_image(
    source: &TranslatedSource,
    base_residues: &[u64],
    index_residues: &[u64],
    modulus: u64,
) -> Vec<(Vec<i64>, u64)> {
    let field = Zp64::new(modulus);
    let point = base_residues
        .iter()
        .chain(index_residues)
        .map(|&residue| field.to_element(residue))
        .collect::<Vec<_>>();
    source
        .terms()
        .iter()
        .map(|(shift, coefficient)| {
            let numerator = evaluate_polynomial(&coefficient.raw().numerator, &point, &field);
            let denominator = evaluate_polynomial(&coefficient.raw().denominator, &point, &field);
            assert!(!field.is_zero(&denominator));
            (
                shift.values().to_vec(),
                field.from_element(&field.div(&numerator, &denominator)),
            )
        })
        .collect()
}

fn corpus_work(completed: &CompletedIbpSourceRows) -> (usize, usize, usize, usize) {
    completed.relations().iter().fold(
        (0, 0, 0, 0),
        |(total_scalars, total_polynomials, max_scalars, max_polynomials), source| {
            let scalars = source.nonzero_conditions().len() + 2 * source.terms().len();
            let condition_terms = source
                .nonzero_conditions()
                .iter()
                .map(|condition| condition.polynomial().raw().nterms())
                .sum::<usize>();
            let coefficient_terms = source
                .terms()
                .values()
                .map(|coefficient| {
                    coefficient.raw().numerator.nterms() + coefficient.raw().denominator.nterms()
                })
                .sum::<usize>();
            let polynomials = condition_terms + coefficient_terms;
            (
                total_scalars + scalars,
                total_polynomials + polynomials,
                max_scalars.max(scalars),
                max_polynomials.max(polynomials),
            )
        },
    )
}

fn assert_matches_exact(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    evaluator: &mut DirectShiftedSourceEvaluator<'_, '_>,
    translated_request: TranslatedSourceRequest,
    base_residues: &[u64],
    index_residues: &[u64],
    output: &mut ShiftedModularSourceBuffer,
) {
    let exact = generator
        .translate_selected_completed_source_rows(
            completed,
            [translated_request.clone()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    evaluator
        .try_evaluate_request(&translated_request, output)
        .unwrap();
    assert_eq!(
        direct_image(output),
        exact_image(
            &exact.sources()[0],
            base_residues,
            index_residues,
            evaluator.modulus(),
        )
    );
}

#[test]
fn direct_shifted_image_matches_exact_translation_and_retains_zero_terms() {
    let family = unit_tadpole("direct-shifted-differential");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let mut evaluator = DirectShiftedSourceEvaluator::try_new(
        generator.context(),
        &completed,
        PRIME,
        &[6],
        &[1],
        DirectShiftedSourceLimits::default(),
    )
    .unwrap();
    let mut output = ShiftedModularSourceBuffer::default();
    assert_matches_exact(
        &generator,
        &completed,
        &mut evaluator,
        request(0, [2]),
        &[6],
        &[1],
        &mut output,
    );
    assert_eq!(output.arity(), 1);
    assert!(output.terms().any(|term| term.residue() == 0));
}

#[test]
fn guards_reject_before_term_sized_structural_work() {
    let family = guarded_tadpole("direct-shifted-guard");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let mut evaluator = DirectShiftedSourceEvaluator::try_new(
        generator.context(),
        &completed,
        PRIME,
        &[4, 0],
        &[1],
        DirectShiftedSourceLimits::default(),
    )
    .unwrap();
    let mut output = ShiftedModularSourceBuffer::default();
    let error = evaluator
        .try_evaluate_request(&request(0, [i64::MAX]), &mut output)
        .unwrap_err();
    assert!(matches!(
        error,
        DirectShiftedSourceError::ConditionZero {
            source_ordinal: 0,
            ..
        }
    ));
    assert!(output.is_empty());
    assert_eq!(output.arity(), 0);
}

#[test]
fn denominator_and_overflow_failures_leave_the_single_output_buffer_reusable() {
    let family = unit_tadpole("direct-shifted-reuse");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let mut evaluator = DirectShiftedSourceEvaluator::try_new(
        generator.context(),
        &completed,
        PRIME,
        &[7],
        &[3],
        DirectShiftedSourceLimits::default(),
    )
    .unwrap();
    let mut output = ShiftedModularSourceBuffer::default();
    evaluator
        .try_evaluate_request(&request(0, [0]), &mut output)
        .unwrap();
    let capacities = output.capacities_for_test();

    evaluator.force_zero_denominator_for_test(0);
    assert!(matches!(
        evaluator.try_evaluate_request(&request(0, [0]), &mut output),
        Err(DirectShiftedSourceError::TermDenominatorZero {
            source_ordinal: 0,
            term_ordinal: 0,
        })
    ));
    assert!(output.is_empty());
    assert_eq!(output.capacities_for_test(), capacities);

    evaluator
        .try_evaluate_request(&request(0, [0]), &mut output)
        .unwrap();
    assert!(!output.is_empty());
    assert!(matches!(
        evaluator.try_evaluate_request(&request(0, [i64::MAX]), &mut output),
        Err(DirectShiftedSourceError::StructuralShiftOverflow { position: 0, .. })
    ));
    assert!(output.is_empty());
    evaluator
        .try_evaluate_request(&request(0, [0]), &mut output)
        .unwrap();
    assert_eq!(output.capacities_for_test(), capacities);
}

#[test]
fn near_machine_limit_offsets_are_reduced_exactly_in_a_large_field() {
    const LARGE_PRIME: u64 = 2_305_843_009_213_693_951;
    let family = unit_tadpole("direct-shifted-large-offset");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let mut evaluator = DirectShiftedSourceEvaluator::try_new(
        generator.context(),
        &completed,
        LARGE_PRIME,
        &[37],
        &[19],
        DirectShiftedSourceLimits::default(),
    )
    .unwrap();
    let mut output = ShiftedModularSourceBuffer::default();
    for offset in [i64::MAX - 1, i64::MIN + 1] {
        assert_matches_exact(
            &generator,
            &completed,
            &mut evaluator,
            request(0, [offset]),
            &[37],
            &[19],
            &mut output,
        );
    }
}

#[test]
fn multidimensional_sources_match_exact_translation_across_rows_and_offsets() {
    let family = equal_mass_sunset("direct-shifted-sunset");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let base_residues = [17, 3];
    let index_residues = [2, 5, 7];
    let mut evaluator = DirectShiftedSourceEvaluator::try_new(
        generator.context(),
        &completed,
        PRIME,
        &base_residues,
        &index_residues,
        DirectShiftedSourceLimits::default(),
    )
    .unwrap();
    let mut output = ShiftedModularSourceBuffer::default();
    let offsets = [[0, 0, 0], [1, -2, 3], [-4, 2, 1], [3, 0, -2]];
    for source_ordinal in 0..completed.source_row_count() {
        for offset in offsets {
            assert_matches_exact(
                &generator,
                &completed,
                &mut evaluator,
                request(source_ordinal, offset),
                &base_residues,
                &index_residues,
                &mut output,
            );
        }
    }
}

#[test]
fn constructor_rejects_bad_fields_points_and_contexts() {
    let family = unit_tadpole("direct-shifted-construction");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = DirectShiftedSourceLimits::default();
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            9,
            &[1],
            &[1],
            limits,
        ),
        Err(DirectShiftedSourceError::NonPrimeModulus { modulus: 9 })
    ));
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[PRIME],
            &[1],
            limits,
        ),
        Err(DirectShiftedSourceError::NonCanonicalPointResidue { coordinate: 0, .. })
    ));

    let foreign = crate::algebra::IndexedCoefficientContext::try_new(
        generator.context().base(),
        "direct-shifted-foreign",
        1,
    )
    .unwrap();
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(&foreign, &completed, PRIME, &[1], &[1], limits,),
        Err(DirectShiftedSourceError::CompletedSourceContextMismatch)
    ));
}

#[test]
fn cheap_caller_validation_precedes_the_source_corpus_scan() {
    let family = unit_tadpole("direct-shifted-validation-order");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let scan_rejecting_limits = DirectShiftedSourceLimits {
        max_terms_per_source: 0,
        ..DirectShiftedSourceLimits::default()
    };

    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            9,
            &[1],
            &[1],
            scan_rejecting_limits,
        ),
        Err(DirectShiftedSourceError::NonPrimeModulus { modulus: 9 })
    ));
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[],
            &[1],
            scan_rejecting_limits,
        ),
        Err(DirectShiftedSourceError::WrongBaseParameterArity {
            expected: 1,
            actual: 0,
        })
    ));
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[1],
            &[],
            scan_rejecting_limits,
        ),
        Err(DirectShiftedSourceError::WrongIndexPointArity {
            expected: 1,
            actual: 0,
        })
    ));
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[PRIME],
            &[1],
            scan_rejecting_limits,
        ),
        Err(DirectShiftedSourceError::NonCanonicalPointResidue { coordinate: 0, .. })
    ));
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[1],
            &[1],
            scan_rejecting_limits,
        ),
        Err(DirectShiftedSourceError::ResourceLimit {
            resource: "source terms",
            ..
        })
    ));
}

#[test]
fn aggregate_source_corpus_work_limits_have_exact_boundaries() {
    let family = equal_mass_sunset("direct-shifted-corpus-limits");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let (scalar_outputs, polynomial_terms, max_source_scalars, max_source_polynomials) =
        corpus_work(&completed);
    assert!(scalar_outputs > max_source_scalars);
    assert!(polynomial_terms > max_source_polynomials);

    let scalar_limits = DirectShiftedSourceLimits {
        max_scalar_outputs_per_source: max_source_scalars,
        max_polynomial_terms_per_source: max_source_polynomials,
        max_corpus_scalar_outputs: scalar_outputs - 1,
        max_corpus_polynomial_terms: polynomial_terms,
        ..DirectShiftedSourceLimits::default()
    };
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[17, 3],
            &[2, 5, 7],
            scalar_limits,
        ),
        Err(DirectShiftedSourceError::ResourceLimit {
            resource: "source-corpus scalar outputs",
            requested,
            limit,
        }) if requested == scalar_outputs && limit + 1 == scalar_outputs
    ));

    let polynomial_limits = DirectShiftedSourceLimits {
        max_scalar_outputs_per_source: max_source_scalars,
        max_polynomial_terms_per_source: max_source_polynomials,
        max_corpus_scalar_outputs: scalar_outputs,
        max_corpus_polynomial_terms: polynomial_terms - 1,
        ..DirectShiftedSourceLimits::default()
    };
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[17, 3],
            &[2, 5, 7],
            polynomial_limits,
        ),
        Err(DirectShiftedSourceError::ResourceLimit {
            resource: "source-corpus polynomial terms",
            requested,
            limit,
        }) if requested == polynomial_terms && limit + 1 == polynomial_terms
    ));

    let exact_limits = DirectShiftedSourceLimits {
        max_scalar_outputs_per_source: max_source_scalars,
        max_polynomial_terms_per_source: max_source_polynomials,
        max_corpus_scalar_outputs: scalar_outputs,
        max_corpus_polynomial_terms: polynomial_terms,
        ..DirectShiftedSourceLimits::default()
    };
    DirectShiftedSourceEvaluator::try_new(
        generator.context(),
        &completed,
        PRIME,
        &[17, 3],
        &[2, 5, 7],
        exact_limits,
    )
    .unwrap();
}

#[test]
fn source_and_point_resource_caps_fail_closed() {
    let family = unit_tadpole("direct-shifted-limits");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);

    let source_limits = DirectShiftedSourceLimits {
        max_source_rows: 0,
        ..DirectShiftedSourceLimits::default()
    };
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[1],
            &[1],
            source_limits,
        ),
        Err(DirectShiftedSourceError::ResourceLimit {
            resource: "ordinary source rows",
            ..
        })
    ));

    let point_limits = DirectShiftedSourceLimits {
        max_point_coordinates: 1,
        ..DirectShiftedSourceLimits::default()
    };
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[1],
            &[1],
            point_limits,
        ),
        Err(DirectShiftedSourceError::ResourceLimit {
            resource: "modular point coordinates",
            requested: 2,
            limit: 1,
        })
    ));

    let structural_limits = DirectShiftedSourceLimits {
        max_shift_coordinate_cells_per_source: 1,
        ..DirectShiftedSourceLimits::default()
    };
    assert!(matches!(
        DirectShiftedSourceEvaluator::try_new(
            generator.context(),
            &completed,
            PRIME,
            &[1],
            &[1],
            structural_limits,
        ),
        Err(DirectShiftedSourceError::ResourceLimit {
            resource: "shifted structural coordinates",
            ..
        })
    ));
}

#[test]
fn index_polynomials_use_the_shifted_index_point() {
    let family = unit_tadpole("direct-shifted-index-polynomial");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let polynomial = generator
        .context()
        .parse_expression_with_limits(
            "rustred_indexed_coefficient_v1::n0-4",
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert!(polynomial.raw().denominator.is_one());
    let mut evaluator = DirectShiftedSourceEvaluator::try_new(
        generator.context(),
        &completed,
        PRIME,
        &[8],
        &[1],
        DirectShiftedSourceLimits::default(),
    )
    .unwrap();
    let mut output = ShiftedModularSourceBuffer::default();
    evaluator
        .try_evaluate_request(&request(0, [0]), &mut output)
        .unwrap();
    assert_eq!(
        evaluator.evaluate_at_shifted_point_for_test(&polynomial.raw().numerator),
        PRIME - 3
    );
    evaluator
        .try_evaluate_request(&request(0, [3]), &mut output)
        .unwrap();
    assert_eq!(
        evaluator.evaluate_at_shifted_point_for_test(&polynomial.raw().numerator),
        0
    );
}
