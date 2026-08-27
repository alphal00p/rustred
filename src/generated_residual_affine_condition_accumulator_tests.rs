//! Source-neutral tests for generated affine-condition accumulation.
//!
//! These fixtures intentionally name no loop count, topology, propagator, or
//! recurrence.  Concrete polynomials are used only to exercise the generic
//! `K(n)` condition protocol and its resource accounting.

use crate::generated_residual_affine_condition_accumulator::{
    GeneratedResidualAffineCanonicalConditionRow,
    GeneratedResidualAffineConditionAccumulatorCertificate,
    GeneratedResidualAffineConditionAccumulatorError,
    GeneratedResidualAffineConditionAccumulatorLimits, GeneratedResidualAffineConditionInput,
    GeneratedResidualAffineConditionInputClass, GeneratedResidualAffineConditionRelationTerm,
    GeneratedResidualAffineConditionScope, GeneratedResidualAffineConditionSourceLocator,
    accumulate_generated_residual_affine_conditions,
};
use crate::{
    CoefficientContext, IndexShift, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricPolynomial,
};

const SECRET_POSITIVE_SHIFT: i64 = 991;
const SECRET_NEGATIVE_SHIFT: i64 = -997;
const SECRET_POLYNOMIAL_MARKER: &str = "secret_polynomial_marker_991";

struct SourceNeutralFixture {
    context: ParametricCoefficientContext,
    discharged_constant: ParametricPolynomial,
    candidate_zero: ParametricPolynomial,
    base_condition: ParametricPolynomial,
    first_index_representative: ParametricPolynomial,
    distinct_index_condition: ParametricPolynomial,
    associated_index_condition: ParametricPolynomial,
    nonfree_index_condition: ParametricPolynomial,
    pivot_shift: IndexShift,
    first_rhs_shift: IndexShift,
    second_rhs_shift: IndexShift,
}

impl SourceNeutralFixture {
    fn new(scope: &str) -> Self {
        let base = CoefficientContext::new(["theta", SECRET_POLYNOMIAL_MARKER]);
        let context = ParametricCoefficientContext::try_new(&base, scope, 2).unwrap();

        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let secret_parameter = context
            .lift(&base.parameter(SECRET_POLYNOMIAL_MARKER).unwrap())
            .unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let first_index_value = context.add(&n0, &context.one()).unwrap();
        let distinct_index_value = context.add(&n0, &context.integer(2)).unwrap();

        Self {
            discharged_constant: context.numerator_condition(&context.integer(7)).unwrap(),
            candidate_zero: context.numerator_condition(&context.zero()).unwrap(),
            base_condition: context
                .numerator_condition(&context.add(&secret_parameter, &context.one()).unwrap())
                .unwrap(),
            first_index_representative: context.numerator_condition(&first_index_value).unwrap(),
            distinct_index_condition: context.numerator_condition(&distinct_index_value).unwrap(),
            associated_index_condition: context
                .numerator_condition(&context.mul(&theta, &first_index_value).unwrap())
                .unwrap(),
            nonfree_index_condition: context
                .numerator_condition(&context.add(&n1, &context.one()).unwrap())
                .unwrap(),
            pivot_shift: IndexShift::try_new([SECRET_POSITIVE_SHIFT, SECRET_NEGATIVE_SHIFT], 2)
                .unwrap(),
            first_rhs_shift: IndexShift::try_new([SECRET_NEGATIVE_SHIFT, SECRET_POSITIVE_SHIFT], 2)
                .unwrap(),
            second_rhs_shift: IndexShift::try_new(
                [SECRET_POSITIVE_SHIFT, SECRET_POSITIVE_SHIFT],
                2,
            )
            .unwrap(),
            context,
        }
    }

    /// Original authenticated order:
    ///
    /// 0. inherited target guard, base-only;
    /// 1. candidate relation guard, first index representative;
    /// 2. candidate RHS denominator, a `Q(theta)` associate of input 1;
    /// 3. inherited target guard, exact duplicate of input 1 (promotion);
    /// 4. candidate pivot denominator, distinct index locus;
    /// 5. discharged candidate integer constant;
    /// 6. identically-zero candidate;
    /// 7. candidate RHS denominator, exact duplicate of input 4.
    fn inputs(&self) -> Vec<GeneratedResidualAffineConditionInput<'_>> {
        vec![
            GeneratedResidualAffineConditionInput::new(
                &self.base_condition,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                    entry_ordinal: 0,
                    structural_locus_ordinal: 0,
                },
                None,
            ),
            GeneratedResidualAffineConditionInput::new(
                &self.first_index_representative,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                    guard_ordinal: 0,
                },
                None,
            ),
            GeneratedResidualAffineConditionInput::new(
                &self.associated_index_condition,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                    term: GeneratedResidualAffineConditionRelationTerm::Rhs { rhs_ordinal: 0 },
                },
                Some(&self.first_rhs_shift),
            ),
            GeneratedResidualAffineConditionInput::new(
                &self.first_index_representative,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                    entry_ordinal: 1,
                    structural_locus_ordinal: 0,
                },
                None,
            ),
            GeneratedResidualAffineConditionInput::new(
                &self.distinct_index_condition,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                    term: GeneratedResidualAffineConditionRelationTerm::Pivot,
                },
                Some(&self.pivot_shift),
            ),
            GeneratedResidualAffineConditionInput::new(
                &self.discharged_constant,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                    guard_ordinal: 1,
                },
                None,
            ),
            GeneratedResidualAffineConditionInput::new(
                &self.candidate_zero,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                    guard_ordinal: 2,
                },
                None,
            ),
            GeneratedResidualAffineConditionInput::new(
                &self.distinct_index_condition,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                    term: GeneratedResidualAffineConditionRelationTerm::Rhs { rhs_ordinal: 1 },
                },
                Some(&self.second_rhs_shift),
            ),
        ]
    }

    fn accumulate(
        &self,
        limits: GeneratedResidualAffineConditionAccumulatorLimits,
    ) -> Result<
        GeneratedResidualAffineConditionAccumulatorCertificate,
        GeneratedResidualAffineConditionAccumulatorError,
    > {
        accumulate_generated_residual_affine_conditions(&self.context, &[0], self.inputs(), limits)
    }

    fn accumulate_single_candidate(
        &self,
        limits: GeneratedResidualAffineConditionAccumulatorLimits,
    ) -> Result<
        GeneratedResidualAffineConditionAccumulatorCertificate,
        GeneratedResidualAffineConditionAccumulatorError,
    > {
        accumulate_generated_residual_affine_conditions(
            &self.context,
            &[0],
            [candidate_guard(&self.first_index_representative, 0)],
            limits,
        )
    }

    fn accumulate_single_inherited(
        &self,
        limits: GeneratedResidualAffineConditionAccumulatorLimits,
    ) -> Result<
        GeneratedResidualAffineConditionAccumulatorCertificate,
        GeneratedResidualAffineConditionAccumulatorError,
    > {
        accumulate_generated_residual_affine_conditions(
            &self.context,
            &[0],
            [GeneratedResidualAffineConditionInput::new(
                &self.first_index_representative,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                    entry_ordinal: 0,
                    structural_locus_ordinal: 0,
                },
                None,
            )],
            limits,
        )
    }

    fn accumulate_single_denominator(
        &self,
        limits: GeneratedResidualAffineConditionAccumulatorLimits,
    ) -> Result<
        GeneratedResidualAffineConditionAccumulatorCertificate,
        GeneratedResidualAffineConditionAccumulatorError,
    > {
        accumulate_generated_residual_affine_conditions(
            &self.context,
            &[0],
            [GeneratedResidualAffineConditionInput::new(
                &self.first_index_representative,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                    term: GeneratedResidualAffineConditionRelationTerm::Pivot,
                },
                Some(&self.pivot_shift),
            )],
            limits,
        )
    }

    fn accumulate_distinct_pair(
        &self,
        limits: GeneratedResidualAffineConditionAccumulatorLimits,
    ) -> Result<
        GeneratedResidualAffineConditionAccumulatorCertificate,
        GeneratedResidualAffineConditionAccumulatorError,
    > {
        accumulate_generated_residual_affine_conditions(
            &self.context,
            &[0],
            [
                candidate_guard(&self.first_index_representative, 0),
                candidate_guard(&self.distinct_index_condition, 1),
            ],
            limits,
        )
    }

    fn accumulate_associate_pair(
        &self,
        limits: GeneratedResidualAffineConditionAccumulatorLimits,
    ) -> Result<
        GeneratedResidualAffineConditionAccumulatorCertificate,
        GeneratedResidualAffineConditionAccumulatorError,
    > {
        accumulate_generated_residual_affine_conditions(
            &self.context,
            &[0],
            [
                candidate_guard(&self.first_index_representative, 0),
                candidate_guard(&self.associated_index_condition, 1),
            ],
            limits,
        )
    }
}

fn row_ordinal(class: GeneratedResidualAffineConditionInputClass) -> usize {
    match class {
        GeneratedResidualAffineConditionInputClass::BaseAssumption { row_ordinal }
        | GeneratedResidualAffineConditionInputClass::IndexDependent { row_ordinal } => row_ordinal,
        other => panic!("condition input does not refer to a row: {other:?}"),
    }
}

fn assert_first_representative(
    row: &GeneratedResidualAffineCanonicalConditionRow,
    expected: &ParametricPolynomial,
) {
    assert_eq!(row.polynomial(), expected);
}

fn candidate_guard(
    polynomial: &ParametricPolynomial,
    guard_ordinal: usize,
) -> GeneratedResidualAffineConditionInput<'_> {
    GeneratedResidualAffineConditionInput::new(
        polynomial,
        GeneratedResidualAffineConditionScope::CandidateRequired,
        GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard { guard_ordinal },
        None,
    )
}

#[test]
fn ordered_transcript_preserves_typed_sources_private_shifts_and_all_input_classes() {
    let fixture = SourceNeutralFixture::new("condition-transcript-secret-997");
    let certificate = fixture
        .accumulate(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();

    assert_eq!(
        certificate.context_fingerprint(),
        fixture.context.fingerprint()
    );
    assert_eq!(certificate.free_positions(), &[0]);
    assert_eq!(certificate.inputs().len(), 8);
    assert_eq!(certificate.rows().len(), 3);
    assert!(certificate.candidate_is_identically_bad());

    for (ordinal, input) in certificate.inputs().iter().enumerate() {
        assert_eq!(input.ordinal(), ordinal);
        assert_eq!(certificate.input_view(ordinal).unwrap(), input.view());
    }
    assert!(certificate.input_view(8).is_none());
    assert!(certificate.row_view(3).is_none());

    assert_eq!(
        certificate.input_view(0).unwrap().source(),
        GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
            entry_ordinal: 0,
            structural_locus_ordinal: 0,
        }
    );
    assert_eq!(
        certificate.input_view(1).unwrap().source(),
        GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard { guard_ordinal: 0 }
    );
    assert_eq!(
        certificate.input_view(2).unwrap().source(),
        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
            term: GeneratedResidualAffineConditionRelationTerm::Rhs { rhs_ordinal: 0 },
        }
    );
    assert_eq!(
        certificate.input_view(3).unwrap().source(),
        GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
            entry_ordinal: 1,
            structural_locus_ordinal: 0,
        }
    );
    assert_eq!(
        certificate.input_view(4).unwrap().source(),
        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
            term: GeneratedResidualAffineConditionRelationTerm::Pivot,
        }
    );
    assert_eq!(
        certificate.input_view(5).unwrap().source(),
        GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard { guard_ordinal: 1 }
    );
    assert_eq!(
        certificate.input_view(6).unwrap().source(),
        GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard { guard_ordinal: 2 }
    );
    assert_eq!(
        certificate.input_view(7).unwrap().source(),
        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
            term: GeneratedResidualAffineConditionRelationTerm::Rhs { rhs_ordinal: 1 },
        }
    );

    assert_eq!(
        certificate.inputs()[2]
            .source()
            .private_shift()
            .unwrap()
            .values(),
        &[SECRET_NEGATIVE_SHIFT, SECRET_POSITIVE_SHIFT]
    );
    assert_eq!(
        certificate.inputs()[4]
            .source()
            .private_shift()
            .unwrap()
            .values(),
        &[SECRET_POSITIVE_SHIFT, SECRET_NEGATIVE_SHIFT]
    );
    assert_eq!(
        certificate.inputs()[7]
            .source()
            .private_shift()
            .unwrap()
            .values(),
        &[SECRET_POSITIVE_SHIFT, SECRET_POSITIVE_SHIFT]
    );
    for ordinal in [0, 1, 3, 5, 6] {
        assert!(
            certificate.inputs()[ordinal]
                .source()
                .private_shift()
                .is_none()
        );
    }

    let base_row = row_ordinal(certificate.inputs()[0].class());
    let promoted_row = row_ordinal(certificate.inputs()[1].class());
    let candidate_row = row_ordinal(certificate.inputs()[4].class());
    assert!(matches!(
        certificate.inputs()[0].class(),
        GeneratedResidualAffineConditionInputClass::BaseAssumption { .. }
    ));
    for ordinal in [1, 2, 3, 4, 7] {
        assert!(matches!(
            certificate.inputs()[ordinal].class(),
            GeneratedResidualAffineConditionInputClass::IndexDependent { .. }
        ));
    }
    assert_eq!(
        certificate.inputs()[5].class(),
        GeneratedResidualAffineConditionInputClass::DischargedNonzeroIntegerConstant
    );
    assert_eq!(
        certificate.inputs()[6].class(),
        GeneratedResidualAffineConditionInputClass::IdenticallyZeroCandidate
    );

    assert!(!certificate.rows()[base_row].is_index_dependent());
    assert_eq!(
        certificate.rows()[base_row].scope(),
        GeneratedResidualAffineConditionScope::InheritedTargetPremise
    );
    assert_eq!(certificate.rows()[base_row].source_input_ordinals(), &[0]);

    assert_eq!(row_ordinal(certificate.inputs()[2].class()), promoted_row);
    assert_eq!(row_ordinal(certificate.inputs()[3].class()), promoted_row);
    assert_first_representative(
        &certificate.rows()[promoted_row],
        &fixture.first_index_representative,
    );
    assert_eq!(
        certificate.rows()[promoted_row].scope(),
        GeneratedResidualAffineConditionScope::InheritedTargetPremise
    );
    assert_eq!(
        certificate.rows()[promoted_row].source_input_ordinals(),
        &[1, 2, 3]
    );

    assert_eq!(row_ordinal(certificate.inputs()[7].class()), candidate_row);
    assert_first_representative(
        &certificate.rows()[candidate_row],
        &fixture.distinct_index_condition,
    );
    assert_eq!(
        certificate.rows()[candidate_row].scope(),
        GeneratedResidualAffineConditionScope::CandidateRequired
    );
    assert_eq!(
        certificate.rows()[candidate_row].source_input_ordinals(),
        &[4, 7]
    );

    let stats = certificate.stats();
    assert_eq!(stats.condition_inputs(), 8);
    assert_eq!(stats.source_inputs(), 8);
    assert_eq!(stats.inherited_inputs(), 2);
    assert_eq!(stats.candidate_inputs(), 6);
    assert_eq!(stats.condition_sources(), 6);
    assert_eq!(stats.discharged_nonzero_constants(), 1);
    assert_eq!(stats.identically_zero_candidate_inputs(), 1);
    assert_eq!(stats.unique_rows(), 3);
    assert_eq!(stats.unique_inherited_rows(), 2);
    assert_eq!(stats.unique_candidate_rows(), 1);
    assert_eq!(stats.unique_base_rows(), 1);
    assert_eq!(stats.unique_index_dependent_rows(), 2);
    assert_eq!(stats.source_shift_components(), 6);
}

#[test]
fn exact_match_scans_every_row_before_skipping_all_associate_calls() {
    let fixture = SourceNeutralFixture::new("condition-exact-before-associate");

    let prefix = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [
            candidate_guard(&fixture.first_index_representative, 0),
            candidate_guard(&fixture.distinct_index_condition, 1),
        ],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap();
    let complete = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [
            candidate_guard(&fixture.first_index_representative, 0),
            candidate_guard(&fixture.distinct_index_condition, 1),
            candidate_guard(&fixture.first_index_representative, 2),
        ],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap();

    assert_eq!(prefix.rows().len(), 2);
    assert_eq!(complete.rows().len(), 2);
    assert_eq!(
        complete.stats().equality_comparisons(),
        prefix.stats().equality_comparisons() + 2,
        "the exact duplicate must still be compared with both retained rows"
    );
    assert_eq!(
        complete.stats().associate_checks(),
        prefix.stats().associate_checks(),
        "finding an exact match anywhere must suppress the entire associate pass"
    );
    assert_eq!(
        complete.rows()[0].source_input_ordinals(),
        &[0, 2],
        "the first exact representative retains both provenance entries"
    );
}

#[test]
fn q_theta_associates_merge_without_replacing_the_first_representative() {
    let fixture = SourceNeutralFixture::new("condition-q-theta-associate");
    let certificate = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [
            GeneratedResidualAffineConditionInput::new(
                &fixture.first_index_representative,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                    guard_ordinal: 0,
                },
                None,
            ),
            GeneratedResidualAffineConditionInput::new(
                &fixture.associated_index_condition,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                    guard_ordinal: 1,
                },
                None,
            ),
        ],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.rows().len(), 1);
    assert_first_representative(&certificate.rows()[0], &fixture.first_index_representative);
    assert_eq!(certificate.rows()[0].source_input_ordinals(), &[0, 1]);
    assert_eq!(certificate.stats().associate_checks(), 1);
    assert!(certificate.stats().associate_native_cross_term_pairs() > 0);
    assert!(
        certificate
            .stats()
            .associate_native_metadata_integer_entry_inspection_bound()
            > 0
    );
}

#[test]
fn base_parameter_loci_remain_distinct_while_rational_units_merge() {
    let context = ParametricCoefficientContext::try_new(
        &CoefficientContext::new(["theta"]),
        "condition-base-rational-associates",
        2,
    )
    .unwrap();
    let theta = context
        .lift(&context.base().parameter("theta").unwrap())
        .unwrap();
    let theta_plus_one = context.add(&theta, &context.one()).unwrap();
    let negative_two_theta = context
        .neg(&context.mul(&context.integer(2), &theta).unwrap())
        .unwrap();
    let theta = context.numerator_condition(&theta).unwrap();
    let theta_plus_one = context.numerator_condition(&theta_plus_one).unwrap();
    let negative_two_theta = context.numerator_condition(&negative_two_theta).unwrap();

    let certificate = accumulate_generated_residual_affine_conditions(
        &context,
        &[0],
        [
            candidate_guard(&theta, 0),
            candidate_guard(&theta_plus_one, 1),
            candidate_guard(&negative_two_theta, 2),
        ],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.rows().len(), 2);
    assert_eq!(row_ordinal(certificate.inputs()[0].class()), 0);
    assert_eq!(row_ordinal(certificate.inputs()[1].class()), 1);
    assert_eq!(row_ordinal(certificate.inputs()[2].class()), 0);
    assert!(certificate.inputs().iter().all(|input| matches!(
        input.class(),
        GeneratedResidualAffineConditionInputClass::BaseAssumption { .. }
    )));
    assert_eq!(certificate.rows()[0].source_input_ordinals(), &[0, 2]);
    assert_eq!(certificate.rows()[1].source_input_ordinals(), &[1]);
    assert_eq!(certificate.stats().associate_checks(), 2);
    assert!(certificate.stats().base_associate_native_scale_calls() > 0);
    assert!(
        certificate
            .stats()
            .base_associate_native_integer_multiplication_bit_work_bound()
            > 0
    );
    assert_eq!(
        certificate.stats().associate_projection_exponent_entries(),
        0
    );
    assert_eq!(certificate.stats().associate_index_groups(), 0);
}

#[test]
fn candidate_first_row_is_promoted_by_inherited_provenance_and_candidate_only_stays_candidate() {
    let fixture = SourceNeutralFixture::new("condition-scope-dominance");
    let certificate = fixture
        .accumulate(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();

    let promoted = row_ordinal(certificate.inputs()[1].class());
    assert_eq!(
        certificate.inputs()[1].scope(),
        GeneratedResidualAffineConditionScope::CandidateRequired
    );
    assert_eq!(
        certificate.inputs()[3].scope(),
        GeneratedResidualAffineConditionScope::InheritedTargetPremise
    );
    assert_eq!(
        certificate.rows()[promoted].source_input_ordinals(),
        &[1, 2, 3]
    );
    assert_eq!(
        certificate.rows()[promoted].scope(),
        GeneratedResidualAffineConditionScope::InheritedTargetPremise
    );

    let candidate_only = row_ordinal(certificate.inputs()[4].class());
    assert_eq!(
        certificate.rows()[candidate_only].source_input_ordinals(),
        &[4, 7]
    );
    assert_eq!(
        certificate.rows()[candidate_only].scope(),
        GeneratedResidualAffineConditionScope::CandidateRequired
    );
}

#[test]
fn candidate_to_inherited_promotion_allows_zero_final_candidate_row_budget() {
    let fixture = SourceNeutralFixture::new("condition-zero-final-candidate-budget");
    let mut limits = GeneratedResidualAffineConditionAccumulatorLimits::default();
    limits.max_unique_candidate_rows = 0;
    let certificate = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [
            candidate_guard(&fixture.first_index_representative, 0),
            GeneratedResidualAffineConditionInput::new(
                &fixture.first_index_representative,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                    entry_ordinal: 4,
                    structural_locus_ordinal: 7,
                },
                None,
            ),
        ],
        limits,
    )
    .unwrap();

    assert_eq!(certificate.rows().len(), 1);
    assert_eq!(certificate.stats().unique_candidate_rows(), 0);
    assert_eq!(certificate.stats().unique_inherited_rows(), 1);
    assert_eq!(certificate.rows()[0].source_input_ordinals(), &[0, 1]);
    assert_eq!(
        certificate.rows()[0].scope(),
        GeneratedResidualAffineConditionScope::InheritedTargetPremise
    );
}

#[test]
fn base_assumptions_and_free_index_conditions_are_classified_separately() {
    let fixture = SourceNeutralFixture::new("condition-base-versus-index");
    let certificate = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [
            GeneratedResidualAffineConditionInput::new(
                &fixture.base_condition,
                GeneratedResidualAffineConditionScope::InheritedTargetPremise,
                GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                    entry_ordinal: 0,
                    structural_locus_ordinal: 0,
                },
                None,
            ),
            GeneratedResidualAffineConditionInput::new(
                &fixture.first_index_representative,
                GeneratedResidualAffineConditionScope::CandidateRequired,
                GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                    guard_ordinal: 0,
                },
                None,
            ),
        ],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        certificate.inputs()[0].class(),
        GeneratedResidualAffineConditionInputClass::BaseAssumption { .. }
    ));
    assert!(matches!(
        certificate.inputs()[1].class(),
        GeneratedResidualAffineConditionInputClass::IndexDependent { .. }
    ));
    assert_eq!(certificate.stats().unique_base_rows(), 1);
    assert_eq!(certificate.stats().unique_index_dependent_rows(), 1);
}

#[test]
fn inherited_zero_and_nonfree_index_support_are_hard_errors() {
    let fixture = SourceNeutralFixture::new("condition-hard-errors");
    let inherited_zero = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [GeneratedResidualAffineConditionInput::new(
            &fixture.candidate_zero,
            GeneratedResidualAffineConditionScope::InheritedTargetPremise,
            GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                entry_ordinal: 0,
                structural_locus_ordinal: 0,
            },
            None,
        )],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        inherited_zero,
        GeneratedResidualAffineConditionAccumulatorError::InheritedConditionIsIdenticallyZero {
            input_ordinal: 0,
        }
    );

    let nonfree = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [GeneratedResidualAffineConditionInput::new(
            &fixture.nonfree_index_condition,
            GeneratedResidualAffineConditionScope::CandidateRequired,
            GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                guard_ordinal: 0,
            },
            None,
        )],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        nonfree,
        GeneratedResidualAffineConditionAccumulatorError::NonfreePrivateIndexSupport {
            input_ordinal: 0,
            position: 1,
        }
    );
}

#[test]
fn free_position_source_scope_and_private_shift_schema_are_typed_errors() {
    let fixture = SourceNeutralFixture::new("condition-input-schema");
    let defaults = GeneratedResidualAffineConditionAccumulatorLimits::default();

    let no_inputs = Vec::<GeneratedResidualAffineConditionInput<'_>>::new();
    assert_eq!(
        accumulate_generated_residual_affine_conditions(
            &fixture.context,
            &[2],
            no_inputs,
            defaults,
        )
        .unwrap_err(),
        GeneratedResidualAffineConditionAccumulatorError::FreePositionOutOfRange {
            position: 2,
            index_count: 2,
        }
    );
    assert_eq!(
        accumulate_generated_residual_affine_conditions(
            &fixture.context,
            &[0, 0],
            Vec::<GeneratedResidualAffineConditionInput<'_>>::new(),
            defaults,
        )
        .unwrap_err(),
        GeneratedResidualAffineConditionAccumulatorError::NonIncreasingFreePositions {
            previous: 0,
            current: 0,
        }
    );

    let candidate_target = GeneratedResidualAffineConditionInput::new(
        &fixture.first_index_representative,
        GeneratedResidualAffineConditionScope::CandidateRequired,
        GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
            entry_ordinal: 0,
            structural_locus_ordinal: 0,
        },
        None,
    );
    assert_eq!(
        accumulate_generated_residual_affine_conditions(
            &fixture.context,
            &[0],
            [candidate_target],
            defaults,
        )
        .unwrap_err(),
        GeneratedResidualAffineConditionAccumulatorError::SourceScopeMismatch { input_ordinal: 0 }
    );

    let inherited_relation = GeneratedResidualAffineConditionInput::new(
        &fixture.first_index_representative,
        GeneratedResidualAffineConditionScope::InheritedTargetPremise,
        GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard { guard_ordinal: 0 },
        None,
    );
    assert_eq!(
        accumulate_generated_residual_affine_conditions(
            &fixture.context,
            &[0],
            [inherited_relation],
            defaults,
        )
        .unwrap_err(),
        GeneratedResidualAffineConditionAccumulatorError::SourceScopeMismatch { input_ordinal: 0 }
    );

    let missing_shift = GeneratedResidualAffineConditionInput::new(
        &fixture.first_index_representative,
        GeneratedResidualAffineConditionScope::CandidateRequired,
        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
            term: GeneratedResidualAffineConditionRelationTerm::Pivot,
        },
        None,
    );
    assert_eq!(
        accumulate_generated_residual_affine_conditions(
            &fixture.context,
            &[0],
            [missing_shift],
            defaults,
        )
        .unwrap_err(),
        GeneratedResidualAffineConditionAccumulatorError::MissingPrivateShift { input_ordinal: 0 }
    );

    let unexpected_shift = GeneratedResidualAffineConditionInput::new(
        &fixture.first_index_representative,
        GeneratedResidualAffineConditionScope::CandidateRequired,
        GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard { guard_ordinal: 0 },
        Some(&fixture.pivot_shift),
    );
    assert_eq!(
        accumulate_generated_residual_affine_conditions(
            &fixture.context,
            &[0],
            [unexpected_shift],
            defaults,
        )
        .unwrap_err(),
        GeneratedResidualAffineConditionAccumulatorError::UnexpectedPrivateShift {
            input_ordinal: 0,
        }
    );

    let wrong_arity_shift = IndexShift::try_new([SECRET_POSITIVE_SHIFT], 1).unwrap();
    let wrong_arity = GeneratedResidualAffineConditionInput::new(
        &fixture.first_index_representative,
        GeneratedResidualAffineConditionScope::CandidateRequired,
        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
            term: GeneratedResidualAffineConditionRelationTerm::Rhs { rhs_ordinal: 0 },
        },
        Some(&wrong_arity_shift),
    );
    assert_eq!(
        accumulate_generated_residual_affine_conditions(
            &fixture.context,
            &[0],
            [wrong_arity],
            defaults,
        )
        .unwrap_err(),
        GeneratedResidualAffineConditionAccumulatorError::WrongPrivateShiftArity {
            input_ordinal: 0,
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn every_debug_and_view_projection_redacts_polynomials_context_and_private_shifts() {
    let fixture = SourceNeutralFixture::new("secret-context-scope-997");
    let raw_input = fixture.inputs().remove(2);
    let input_debug = format!("{raw_input:?}");
    let certificate = fixture
        .accumulate(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();

    let mut rendered = vec![
        input_debug,
        format!("{certificate:?}"),
        format!("{:?}", certificate.inputs()),
        format!("{:?}", certificate.rows()),
        format!("{:?}", certificate.input_view(2).unwrap()),
        format!("{:?}", certificate.row_view(0).unwrap()),
    ];
    rendered.push(format!("{:?}", certificate.inputs()[2].source()));

    for debug in rendered {
        assert!(debug.contains("redacted") || !debug.contains("polynomial"));
        assert!(
            !debug.contains(SECRET_POLYNOMIAL_MARKER),
            "leaked polynomial marker in {debug}"
        );
        assert!(
            !debug.contains("secret-context-scope"),
            "leaked context fingerprint in {debug}"
        );
        assert!(
            !debug.contains(&SECRET_POSITIVE_SHIFT.to_string()),
            "leaked positive shift in {debug}"
        );
        assert!(
            !debug.contains(&SECRET_NEGATIVE_SHIFT.to_string()),
            "leaked negative shift in {debug}"
        );
    }
}

#[test]
fn wrong_context_and_typed_input_errors_have_private_debug_output() {
    const FOREIGN_MARKER: &str = "foreign_polynomial_marker_1234567";
    const FOREIGN_SCOPE: &str = "foreign_context_scope_1234567";

    let fixture = SourceNeutralFixture::new("private-error-context-991");
    let foreign_base = CoefficientContext::new([FOREIGN_MARKER]);
    let foreign_context =
        ParametricCoefficientContext::try_new(&foreign_base, FOREIGN_SCOPE, 2).unwrap();
    let foreign_parameter = foreign_context
        .lift(&foreign_base.parameter(FOREIGN_MARKER).unwrap())
        .unwrap();
    let foreign_index = foreign_context.index(0).unwrap();
    let foreign_polynomial = foreign_context
        .numerator_condition(
            &foreign_context
                .add(&foreign_parameter, &foreign_index)
                .unwrap(),
        )
        .unwrap();

    let wrong_context = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [candidate_guard(&foreign_polynomial, 0)],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        wrong_context,
        GeneratedResidualAffineConditionAccumulatorError::ParametricCoefficient(
            ParametricCoefficientError::WrongContext,
        )
    );

    let missing_shift = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [GeneratedResidualAffineConditionInput::new(
            &fixture.base_condition,
            GeneratedResidualAffineConditionScope::CandidateRequired,
            GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                term: GeneratedResidualAffineConditionRelationTerm::Pivot,
            },
            None,
        )],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap_err();
    let unexpected_shift = accumulate_generated_residual_affine_conditions(
        &fixture.context,
        &[0],
        [GeneratedResidualAffineConditionInput::new(
            &fixture.base_condition,
            GeneratedResidualAffineConditionScope::CandidateRequired,
            GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                guard_ordinal: 0,
            },
            Some(&fixture.pivot_shift),
        )],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap_err();

    let accumulate_private_associate_pair =
        |limits: GeneratedResidualAffineConditionAccumulatorLimits| {
            accumulate_generated_residual_affine_conditions(
                &fixture.context,
                &[0],
                [
                    GeneratedResidualAffineConditionInput::new(
                        &fixture.first_index_representative,
                        GeneratedResidualAffineConditionScope::CandidateRequired,
                        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                            term: GeneratedResidualAffineConditionRelationTerm::Pivot,
                        },
                        Some(&fixture.pivot_shift),
                    ),
                    GeneratedResidualAffineConditionInput::new(
                        &fixture.associated_index_condition,
                        GeneratedResidualAffineConditionScope::CandidateRequired,
                        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                            term: GeneratedResidualAffineConditionRelationTerm::Rhs {
                                rhs_ordinal: 0,
                            },
                        },
                        Some(&fixture.first_rhs_shift),
                    ),
                ],
                limits,
            )
        };
    let associate_baseline = accumulate_private_associate_pair(
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap();
    let associate_index_groups = associate_baseline.stats().associate_index_groups();
    assert!(associate_index_groups > 0);
    let mut one_below_associate = GeneratedResidualAffineConditionAccumulatorLimits::default();
    one_below_associate.max_associate_index_groups = associate_index_groups - 1;
    let wrapped_associate_limit =
        accumulate_private_associate_pair(one_below_associate).unwrap_err();
    assert_eq!(
        wrapped_associate_limit,
        GeneratedResidualAffineConditionAccumulatorError::ParametricCoefficient(
            ParametricCoefficientError::ResourceLimit {
                resource: "polynomial-associate index groups",
                requested: associate_index_groups,
                limit: associate_index_groups - 1,
            },
        )
    );

    for error in [
        wrong_context,
        missing_shift,
        unexpected_shift,
        wrapped_associate_limit,
    ] {
        for rendered in [format!("{error:?}"), error.to_string()] {
            for secret in [
                SECRET_POLYNOMIAL_MARKER,
                "private-error-context",
                FOREIGN_MARKER,
                FOREIGN_SCOPE,
            ] {
                assert!(
                    !rendered.contains(secret),
                    "error leaked {secret}: {rendered}"
                );
            }
            assert!(!rendered.contains(&SECRET_POSITIVE_SHIFT.to_string()));
            assert!(!rendered.contains(&SECRET_NEGATIVE_SHIFT.to_string()));
        }
    }
}

#[test]
fn unequal_index_group_counts_remain_distinct_in_both_input_orders() {
    let base = CoefficientContext::new(["theta"]);
    let context =
        ParametricCoefficientContext::try_new(&base, "unequal-index-group-counts", 2).unwrap();
    let n0 = context.index(0).unwrap();
    let n0_squared = context.mul(&n0, &n0).unwrap();
    let p = context
        .numerator_condition(&context.add(&n0, &context.one()).unwrap())
        .unwrap();
    let q = context
        .numerator_condition(
            &context
                .add(&context.add(&n0_squared, &n0).unwrap(), &context.one())
                .unwrap(),
        )
        .unwrap();

    for [first, second] in [[&p, &q], [&q, &p]] {
        let certificate = accumulate_generated_residual_affine_conditions(
            &context,
            &[0],
            [candidate_guard(first, 0), candidate_guard(second, 1)],
            GeneratedResidualAffineConditionAccumulatorLimits::default(),
        )
        .unwrap();

        let first_row = match certificate.inputs()[0].class() {
            GeneratedResidualAffineConditionInputClass::IndexDependent { row_ordinal } => {
                row_ordinal
            }
            other => panic!("first unequal-group input was misclassified: {other:?}"),
        };
        let second_row = match certificate.inputs()[1].class() {
            GeneratedResidualAffineConditionInputClass::IndexDependent { row_ordinal } => {
                row_ordinal
            }
            other => panic!("second unequal-group input was misclassified: {other:?}"),
        };

        assert_eq!((first_row, second_row), (0, 1));
        assert_ne!(first_row, second_row);
        assert_eq!(certificate.rows().len(), 2);
        assert_first_representative(&certificate.rows()[first_row], first);
        assert_first_representative(&certificate.rows()[second_row], second);
        assert_eq!(certificate.rows()[first_row].source_input_ordinals(), &[0]);
        assert_eq!(certificate.rows()[second_row].source_input_ordinals(), &[1]);
        assert_eq!(certificate.stats().associate_checks(), 1);
        assert_eq!(certificate.stats().associate_index_groups(), 5);
    }
}

#[test]
fn sparse_high_degree_projective_units_merge_but_same_support_near_miss_does_not() {
    let base = CoefficientContext::new(["theta"]);
    let context =
        ParametricCoefficientContext::try_new(&base, "sparse-projective-adversarial", 1).unwrap();
    let n = context.index(0).unwrap();
    let n2 = context.mul(&n, &n).unwrap();
    let n4 = context.mul(&n2, &n2).unwrap();
    let n8 = context.mul(&n4, &n4).unwrap();
    let n16 = context.mul(&n8, &n8).unwrap();
    let n32 = context.mul(&n16, &n16).unwrap();
    let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
    let theta2 = context.mul(&theta, &theta).unwrap();
    let theta4 = context.mul(&theta2, &theta2).unwrap();
    let theta8 = context.mul(&theta4, &theta4).unwrap();
    let theta16 = context.mul(&theta8, &theta8).unwrap();

    // Three widely separated index monomials force a genuine projective
    // coefficient-vector comparison.  The last input keeps exactly the same
    // index support while perturbing only its middle coefficient.
    let middle = context.mul(&theta, &n8).unwrap();
    let sparse = context
        .add(&context.add(&n32, &middle).unwrap(), &context.one())
        .unwrap();
    let scalar = context.add(&theta16, &context.integer(3)).unwrap();
    let positive_scaled = context.mul(&scalar, &sparse).unwrap();
    let negative_scaled = context.neg(&positive_scaled).unwrap();
    let same_support_near_miss = context.add(&negative_scaled, &n8).unwrap();

    let sparse = context.numerator_condition(&sparse).unwrap();
    let positive_scaled = context.numerator_condition(&positive_scaled).unwrap();
    let negative_scaled = context.numerator_condition(&negative_scaled).unwrap();
    let same_support_near_miss = context
        .numerator_condition(&same_support_near_miss)
        .unwrap();
    let certificate = accumulate_generated_residual_affine_conditions(
        &context,
        &[0],
        [
            candidate_guard(&sparse, 0),
            candidate_guard(&positive_scaled, 1),
            candidate_guard(&negative_scaled, 2),
            candidate_guard(&same_support_near_miss, 3),
        ],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap();

    let projective_row = row_ordinal(certificate.inputs()[0].class());
    assert_eq!(row_ordinal(certificate.inputs()[1].class()), projective_row);
    assert_eq!(row_ordinal(certificate.inputs()[2].class()), projective_row);
    let near_miss_row = row_ordinal(certificate.inputs()[3].class());
    assert_ne!(near_miss_row, projective_row);
    assert_eq!(certificate.rows().len(), 2);
    assert_first_representative(&certificate.rows()[projective_row], &sparse);
    assert_eq!(
        certificate.rows()[projective_row].source_input_ordinals(),
        &[0, 1, 2]
    );
    assert_eq!(
        certificate.rows()[near_miss_row].source_input_ordinals(),
        &[3]
    );
    assert_eq!(certificate.stats().associate_checks(), 3);
    assert!(certificate.stats().associate_index_groups() >= 6);
    assert!(certificate.stats().associate_native_cross_term_pairs() > 0);
    assert!(
        certificate
            .stats()
            .associate_native_metadata_exponent_entry_inspection_bound()
            > 0
    );
    assert!(
        certificate
            .stats()
            .associate_native_integer_multiplication_bit_work_bound()
            > 0
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedLimitLayer {
    Accumulator,
    ParametricCoefficient,
}

fn assert_exact_and_one_below<Run, SetLimit>(
    run: Run,
    observed: usize,
    boundary_name: &str,
    expected_layer: ExpectedLimitLayer,
    expected_resource: &'static str,
    set_limit: SetLimit,
) where
    Run: Fn(
        GeneratedResidualAffineConditionAccumulatorLimits,
    ) -> Result<
        GeneratedResidualAffineConditionAccumulatorCertificate,
        GeneratedResidualAffineConditionAccumulatorError,
    >,
    SetLimit: Fn(&mut GeneratedResidualAffineConditionAccumulatorLimits, usize),
{
    assert!(
        observed > 0,
        "baseline counter {boundary_name} must be nonzero"
    );

    let mut exact = GeneratedResidualAffineConditionAccumulatorLimits::default();
    set_limit(&mut exact, observed);
    run(exact).unwrap_or_else(|error| panic!("exact {boundary_name}={observed} failed: {error:?}"));

    let mut one_below = GeneratedResidualAffineConditionAccumulatorLimits::default();
    set_limit(&mut one_below, observed - 1);
    let error = run(one_below).unwrap_err();
    let (actual_layer, resource, requested, limit) = match error {
        GeneratedResidualAffineConditionAccumulatorError::ResourceLimit {
            resource,
            requested,
            limit,
        } => (ExpectedLimitLayer::Accumulator, resource, requested, limit),
        GeneratedResidualAffineConditionAccumulatorError::ParametricCoefficient(
            ParametricCoefficientError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ) => (
            ExpectedLimitLayer::ParametricCoefficient,
            resource,
            requested,
            limit,
        ),
        other => panic!("one-below {boundary_name} returned a non-limit error: {other:?}"),
    };
    assert_eq!(
        actual_layer, expected_layer,
        "wrong layer for {boundary_name}"
    );
    assert_eq!(
        resource, expected_resource,
        "wrong resource for {boundary_name}"
    );
    assert_eq!(requested, observed, "wrong request for {boundary_name}");
    assert_eq!(limit, observed - 1, "wrong limit for {boundary_name}");
}

macro_rules! boundary {
    ($run:expr, $stats:expr, $getter:ident, $field:ident, $layer:ident, $resource:literal) => {
        assert_exact_and_one_below(
            $run,
            $stats.$getter(),
            stringify!($field),
            ExpectedLimitLayer::$layer,
            $resource,
            |limits, value| limits.$field = value,
        );
    };
}

#[test]
fn aggregate_base_associate_limits_have_strict_exact_and_one_below_boundaries() {
    let context = ParametricCoefficientContext::try_new(
        &CoefficientContext::new(["theta"]),
        "condition-base-associate-boundaries",
        2,
    )
    .unwrap();
    let theta = context
        .lift(&context.base().parameter("theta").unwrap())
        .unwrap();
    let theta_plus_one = context.add(&theta, &context.one()).unwrap();
    let left = context
        .numerator_condition(&context.mul(&context.integer(2), &theta_plus_one).unwrap())
        .unwrap();
    let right = context
        .numerator_condition(&context.neg(&theta_plus_one).unwrap())
        .unwrap();
    let run = |limits| {
        accumulate_generated_residual_affine_conditions(
            &context,
            &[0],
            [candidate_guard(&left, 0), candidate_guard(&right, 1)],
            limits,
        )
    };
    let baseline = run(GeneratedResidualAffineConditionAccumulatorLimits::default()).unwrap();
    let stats = baseline.stats();
    let exact_only = accumulate_generated_residual_affine_conditions(
        &context,
        &[0],
        [candidate_guard(&left, 0), candidate_guard(&left, 1)],
        GeneratedResidualAffineConditionAccumulatorLimits::default(),
    )
    .unwrap();
    assert!(
        stats.context_fingerprint_comparison_bytes()
            > exact_only.stats().context_fingerprint_comparison_bytes(),
        "the common context counter must consume the base-associate child census"
    );
    assert!(
        stats.variable_map_entry_comparisons()
            > exact_only.stats().variable_map_entry_comparisons(),
        "the common variable-map counter must consume the base-associate child census"
    );

    boundary!(
        run,
        stats,
        context_fingerprint_comparison_bytes,
        max_context_fingerprint_comparison_bytes,
        Accumulator,
        "affine condition context fingerprint comparison bytes"
    );
    boundary!(
        run,
        stats,
        variable_map_entry_comparisons,
        max_variable_map_entry_comparisons,
        Accumulator,
        "affine condition variable-map entry comparisons"
    );

    boundary!(
        run,
        stats,
        base_associate_validation_terms,
        max_base_associate_validation_terms,
        ParametricCoefficient,
        "base polynomial-associate validation terms"
    );
    boundary!(
        run,
        stats,
        base_associate_validation_exponent_entries,
        max_base_associate_validation_exponent_entries,
        ParametricCoefficient,
        "base polynomial-associate validation exponent entries"
    );
    boundary!(
        run,
        stats,
        base_associate_validation_integer_bits,
        max_base_associate_validation_integer_bits,
        ParametricCoefficient,
        "base polynomial-associate validation integer bits"
    );
    boundary!(
        run,
        stats,
        base_associate_source_owned_bytes,
        max_base_associate_source_owned_bytes,
        ParametricCoefficient,
        "base polynomial-associate source owned bytes"
    );
    boundary!(
        run,
        stats,
        base_associate_index_exponent_entries,
        max_base_associate_index_exponent_entries,
        ParametricCoefficient,
        "base polynomial-associate index exponent entries"
    );
    boundary!(
        run,
        stats,
        base_associate_native_scale_calls,
        max_base_associate_native_scale_calls,
        ParametricCoefficient,
        "base polynomial-associate native scale calls"
    );
    boundary!(
        run,
        stats,
        base_associate_native_coefficient_multiplications,
        max_base_associate_native_coefficient_multiplications,
        ParametricCoefficient,
        "base polynomial-associate native coefficient multiplications"
    );
    boundary!(
        run,
        stats,
        base_associate_native_integer_multiplication_bit_work_bound,
        max_base_associate_native_integer_multiplication_bit_work_bound,
        ParametricCoefficient,
        "base polynomial-associate native integer multiplication bit-work bound"
    );
    boundary!(
        run,
        stats,
        base_associate_output_terms,
        max_base_associate_output_terms,
        ParametricCoefficient,
        "base polynomial-associate output terms"
    );
    boundary!(
        run,
        stats,
        base_associate_output_exponent_entries,
        max_base_associate_output_exponent_entries,
        ParametricCoefficient,
        "base polynomial-associate output exponent entries"
    );
    boundary!(
        run,
        stats,
        base_associate_output_integer_bit_bound,
        max_base_associate_output_integer_bit_bound,
        ParametricCoefficient,
        "base polynomial-associate output integer bit bound"
    );
    boundary!(
        run,
        stats,
        base_associate_output_retained_byte_bound,
        max_base_associate_output_retained_byte_bound,
        ParametricCoefficient,
        "base polynomial-associate output retained byte bound"
    );
    boundary!(
        run,
        stats,
        base_associate_payload_comparison_terms,
        max_base_associate_payload_comparison_terms,
        ParametricCoefficient,
        "base polynomial-associate payload comparison terms"
    );
    boundary!(
        run,
        stats,
        base_associate_payload_comparison_exponent_entries,
        max_base_associate_payload_comparison_exponent_entries,
        ParametricCoefficient,
        "base polynomial-associate payload comparison exponent entries"
    );
    boundary!(
        run,
        stats,
        base_associate_payload_comparison_integer_bit_bound,
        max_base_associate_payload_comparison_integer_bit_bound,
        ParametricCoefficient,
        "base polynomial-associate payload comparison integer bit bound"
    );
    boundary!(
        run,
        stats,
        base_associate_native_workspace_byte_envelope,
        max_base_associate_native_workspace_byte_envelope,
        ParametricCoefficient,
        "base polynomial-associate native workspace byte envelope"
    );
    boundary!(
        run,
        stats,
        base_associate_rustred_visible_temporary_byte_envelope,
        max_base_associate_rustred_visible_temporary_byte_envelope,
        ParametricCoefficient,
        "base polynomial-associate RustRed-visible temporary byte envelope"
    );
    boundary!(
        run,
        stats,
        base_associate_combined_temporary_byte_envelope,
        max_base_associate_combined_temporary_byte_envelope,
        ParametricCoefficient,
        "base polynomial-associate combined temporary byte envelope"
    );
}

#[test]
fn aggregate_stream_schema_limits_have_strict_exact_and_one_below_boundaries() {
    let fixture = SourceNeutralFixture::new("condition-stream-schema-boundaries");
    let baseline = fixture
        .accumulate_single_candidate(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();
    let stats = baseline.stats();

    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        context_fingerprint_bytes,
        max_context_fingerprint_bytes,
        Accumulator,
        "affine condition context fingerprint bytes"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        ambient_variables,
        max_ambient_variables,
        Accumulator,
        "affine condition ambient variables"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        free_positions,
        max_free_positions,
        Accumulator,
        "affine condition free positions"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        condition_inputs,
        max_condition_inputs,
        Accumulator,
        "affine condition inputs"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        source_inputs,
        max_source_inputs,
        Accumulator,
        "affine condition source inputs"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        condition_sources,
        max_condition_sources,
        Accumulator,
        "affine canonical condition sources"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        unique_rows,
        max_unique_rows,
        Accumulator,
        "unique affine condition rows"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        unique_candidate_rows,
        max_unique_candidate_rows,
        Accumulator,
        "unique candidate affine condition rows"
    );
}

#[test]
fn aggregate_authentication_limits_have_strict_exact_and_one_below_boundaries() {
    let fixture = SourceNeutralFixture::new("condition-authentication-boundaries");
    let baseline = fixture
        .accumulate_single_candidate(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();
    let stats = baseline.stats();

    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        context_fingerprint_comparison_bytes,
        max_context_fingerprint_comparison_bytes,
        Accumulator,
        "affine condition context fingerprint comparison bytes"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        variable_map_entry_comparisons,
        max_variable_map_entry_comparisons,
        Accumulator,
        "affine condition variable-map entry comparisons"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        shared_allocation_identity_comparisons,
        max_shared_allocation_identity_comparisons,
        Accumulator,
        "affine condition shared-allocation identity comparisons"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        input_polynomial_terms,
        max_input_polynomial_terms,
        ParametricCoefficient,
        "parametric polynomial validation source terms"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        input_polynomial_exponent_entries,
        max_input_polynomial_exponent_entries,
        ParametricCoefficient,
        "parametric polynomial validation source exponent entries"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        input_polynomial_integer_bits,
        max_input_polynomial_integer_bits,
        ParametricCoefficient,
        "parametric polynomial validation source integer bits"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        dependency_exponent_entries,
        max_dependency_exponent_entries,
        Accumulator,
        "affine condition dependency exponent entries"
    );
}

#[test]
fn aggregate_scope_shift_and_equality_limits_have_strict_boundaries() {
    let fixture = SourceNeutralFixture::new("condition-scope-equality-boundaries");
    let inherited = fixture
        .accumulate_single_inherited(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();
    boundary!(
        |limits| fixture.accumulate_single_inherited(limits),
        inherited.stats(),
        unique_inherited_rows,
        max_unique_inherited_rows,
        Accumulator,
        "unique inherited affine condition rows"
    );

    let denominator = fixture
        .accumulate_single_denominator(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();
    boundary!(
        |limits| fixture.accumulate_single_denominator(limits),
        denominator.stats(),
        source_shift_components,
        max_source_shift_components,
        Accumulator,
        "affine condition source shift components"
    );

    let distinct = fixture
        .accumulate_distinct_pair(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();
    let stats = distinct.stats();
    boundary!(
        |limits| fixture.accumulate_distinct_pair(limits),
        stats,
        equality_comparisons,
        max_equality_comparisons,
        Accumulator,
        "affine condition equality comparisons"
    );
    boundary!(
        |limits| fixture.accumulate_distinct_pair(limits),
        stats,
        equality_term_units,
        max_equality_term_units,
        Accumulator,
        "affine condition equality term units"
    );
    boundary!(
        |limits| fixture.accumulate_distinct_pair(limits),
        stats,
        equality_exponent_entries,
        max_equality_exponent_entries,
        Accumulator,
        "affine condition equality exponent entries"
    );
    boundary!(
        |limits| fixture.accumulate_distinct_pair(limits),
        stats,
        equality_integer_bits,
        max_equality_integer_bits,
        Accumulator,
        "affine condition equality integer bits"
    );
}

#[test]
fn aggregate_associate_structural_limits_have_strict_boundaries() {
    let fixture = SourceNeutralFixture::new("condition-associate-structural-boundaries");
    let baseline = fixture
        .accumulate_associate_pair(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();
    let stats = baseline.stats();

    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_checks,
        max_associate_checks,
        Accumulator,
        "affine condition associate checks"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_term_units,
        max_associate_term_units,
        Accumulator,
        "affine condition associate term units"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_exponent_entries,
        max_associate_exponent_entries,
        Accumulator,
        "affine condition associate exponent entries"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_integer_bits,
        max_associate_integer_bits,
        Accumulator,
        "affine condition associate integer bits"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_validation_terms,
        max_associate_validation_terms,
        ParametricCoefficient,
        "polynomial-associate validation terms"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_validation_exponent_entries,
        max_associate_validation_exponent_entries,
        ParametricCoefficient,
        "polynomial-associate validation exponent entries"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_validation_integer_bits,
        max_associate_validation_integer_bits,
        ParametricCoefficient,
        "polynomial-associate validation integer bits"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_projection_exponent_entries,
        max_associate_projection_exponent_entries,
        ParametricCoefficient,
        "polynomial-associate projection exponent entries"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_projection_coefficient_capacity_bytes,
        max_associate_projection_coefficient_capacity_bytes,
        ParametricCoefficient,
        "polynomial-associate projection coefficient-capacity bytes"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_projection_group_bound,
        max_associate_projection_group_bound,
        ParametricCoefficient,
        "polynomial-associate projection group bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_projection_variable_mask_comparison_bound,
        max_associate_projection_variable_mask_comparison_bound,
        ParametricCoefficient,
        "polynomial-associate projection variable-mask comparison bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_projection_hash_key_exponent_entry_bound,
        max_associate_projection_hash_key_exponent_entry_bound,
        ParametricCoefficient,
        "polynomial-associate projection hash-key exponent-entry bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_projection_coefficient_append_comparison_bound,
        max_associate_projection_coefficient_append_comparison_bound,
        ParametricCoefficient,
        "polynomial-associate projection coefficient append comparison bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_projection_sorted_insert_comparison_bound,
        max_associate_projection_sorted_insert_comparison_bound,
        ParametricCoefficient,
        "polynomial-associate projection sorted-insert comparison bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_projection_sorted_insert_move_exponent_entry_bound,
        max_associate_projection_sorted_insert_move_exponent_entry_bound,
        ParametricCoefficient,
        "polynomial-associate projection sorted-insert move exponent-entry bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_index_groups,
        max_associate_index_groups,
        ParametricCoefficient,
        "polynomial-associate index groups"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_index_support_comparison_entries,
        max_associate_index_support_comparison_entries,
        ParametricCoefficient,
        "polynomial-associate index support comparison entries"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_anchor_cost_operations,
        max_associate_anchor_cost_operations,
        ParametricCoefficient,
        "polynomial-associate anchor cost operations"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_cross_term_pairs,
        max_associate_native_cross_term_pairs,
        ParametricCoefficient,
        "polynomial-associate native cross term pairs"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_peak_native_cross_term_pairs,
        max_associate_peak_native_cross_term_pairs,
        ParametricCoefficient,
        "polynomial-associate peak native cross term pairs"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_base_exponent_additions,
        max_associate_native_base_exponent_additions,
        ParametricCoefficient,
        "polynomial-associate native base exponent additions"
    );
}

#[test]
fn aggregate_associate_native_work_and_workspace_limits_have_strict_boundaries() {
    let fixture = SourceNeutralFixture::new("condition-associate-integer-boundaries");
    let baseline = fixture
        .accumulate_associate_pair(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();
    let stats = baseline.stats();

    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_metadata_exponent_entry_inspection_bound,
        max_associate_native_metadata_exponent_entry_inspection_bound,
        ParametricCoefficient,
        "polynomial-associate native metadata exponent-entry inspection bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_metadata_integer_entry_inspection_bound,
        max_associate_native_metadata_integer_entry_inspection_bound,
        ParametricCoefficient,
        "polynomial-associate native metadata integer-entry inspection bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_integer_multiplication_bit_work_bound,
        max_associate_native_integer_multiplication_bit_work_bound,
        ParametricCoefficient,
        "polynomial-associate native integer multiplication bit-work bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_integer_collection_bit_work_bound,
        max_associate_native_integer_collection_bit_work_bound,
        ParametricCoefficient,
        "polynomial-associate native integer collection bit-work bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_output_term_bound,
        max_associate_native_output_term_bound,
        ParametricCoefficient,
        "polynomial-associate native output term bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_output_exponent_entry_bound,
        max_associate_native_output_exponent_entry_bound,
        ParametricCoefficient,
        "polynomial-associate native output exponent entry bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_output_integer_bit_bound,
        max_associate_native_output_integer_bit_bound,
        ParametricCoefficient,
        "polynomial-associate native output integer bit bound"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_native_workspace_byte_envelope,
        max_associate_native_workspace_byte_envelope,
        ParametricCoefficient,
        "polynomial-associate native workspace byte envelope"
    );
    boundary!(
        |limits| fixture.accumulate_associate_pair(limits),
        stats,
        associate_rustred_visible_temporary_byte_envelope,
        max_associate_rustred_visible_temporary_byte_envelope,
        ParametricCoefficient,
        "polynomial-associate RustRed-visible temporary byte envelope"
    );
}

#[test]
fn aggregate_retained_and_final_limits_have_strict_boundaries() {
    let fixture = SourceNeutralFixture::new("condition-retained-final-boundaries");
    let baseline = fixture
        .accumulate_single_candidate(GeneratedResidualAffineConditionAccumulatorLimits::default())
        .unwrap();
    let stats = baseline.stats();

    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        retained_polynomial_terms,
        max_retained_polynomial_terms,
        Accumulator,
        "retained affine condition polynomial terms"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        retained_polynomial_exponent_entries,
        max_retained_polynomial_exponent_entries,
        Accumulator,
        "retained affine condition polynomial exponent entries"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        retained_polynomial_integer_bits,
        max_retained_polynomial_integer_bits,
        Accumulator,
        "retained affine condition polynomial integer bits"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        retained_polynomial_display_bytes,
        max_retained_polynomial_display_bytes,
        Accumulator,
        "retained affine condition polynomial display bytes"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        retained_polynomial_owned_byte_envelope,
        max_retained_polynomial_owned_bytes,
        Accumulator,
        "retained affine condition polynomial owned bytes"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        retained_byte_envelope,
        max_retained_bytes,
        Accumulator,
        "affine condition retained bytes"
    );
    boundary!(
        |limits| fixture.accumulate_single_candidate(limits),
        stats,
        final_invariant_entries,
        max_final_invariant_entries,
        Accumulator,
        "affine condition final invariant entries"
    );

    assert!(stats.retained_polynomial_owned_bytes() > 0);
    assert!(
        stats.retained_polynomial_owned_bytes() <= stats.retained_polynomial_owned_byte_envelope()
    );
    assert!(stats.retained_bytes() > 0);
    assert!(stats.retained_bytes() <= stats.retained_byte_envelope());
}
