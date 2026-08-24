//! Black-box ownership and strict-binding tests for the reusable generated
//! IBP/LI row-span proof.  The one-loop family is only a compact fixture.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedSymbolicRowSpanCompiler, GeneratedWhenBadError,
    IntegralFamily, IntegralOrderingPolicy, ParametricElimination, ParametricEliminationLimits,
    ParametricEliminationOrdering, ParametricIbpGenerator, ParametricReductionRuleCandidate,
    ParametricRuleLimits, ParametricSectorCoverageCompiler, ParametricSectorCoverageError,
    ParametricSectorCoverageLimits, SectorMask,
};

fn family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "shared-row-span-one-loop",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn candidate(
    context: &rustred::ParametricCoefficientContext,
    rows: &[rustred::ParametricRelation],
    anchor: i64,
) -> ParametricReductionRuleCandidate {
    let elimination = ParametricElimination::build(
        context,
        rows,
        ParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [anchor],
        )
        .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    ParametricReductionRuleCandidate::try_from_elimination_pivot(
        context,
        rows,
        &elimination,
        0,
        SectorMask::try_new([true]).unwrap(),
        ParametricRuleLimits::default(),
    )
    .unwrap()
}

#[test]
fn one_coverage_batch_reuses_one_allocation_and_replays_with_an_equal_fresh_proof() {
    let family = family();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let limits = ParametricSectorCoverageLimits::default();
    let shared = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            limits.generated_when_bad.ibp,
            limits.generated_when_bad.row_span,
        )
        .unwrap(),
    );
    let candidates = [
        candidate(&context, shared.rows(), 2),
        candidate(&context, shared.rows(), 3),
    ];
    let certificate = ParametricSectorCoverageCompiler::compile_with_row_span(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        &candidates,
        shared.clone(),
        limits,
    )
    .unwrap();

    assert!(Arc::ptr_eq(certificate.row_span_arc(), &shared));
    assert_eq!(certificate.stats().shared_row_span_certificates(), 1);
    assert_eq!(certificate.stats().shared_row_span_candidate_reuses(), 2);
    for attempt in certificate.candidate_attempts() {
        assert!(Arc::ptr_eq(
            &shared,
            attempt.compilation().source_authentication().row_span_arc()
        ));
    }

    // Persistence may reconstruct an equal proof in another allocation.
    // Public replay checks the complete payload and normalizes the candidate
    // batch onto the supplied allocation; pointer identity is only an
    // in-memory ownership invariant.
    let fresh_equal = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            limits.generated_when_bad.ibp,
            limits.generated_when_bad.row_span,
        )
        .unwrap(),
    );
    assert!(!Arc::ptr_eq(&shared, &fresh_equal));
    certificate
        .replay_with_row_span(&family, &context, fresh_equal)
        .unwrap();
}

#[test]
fn shared_row_span_configuration_is_strictly_bound_before_candidate_use() {
    let family = family();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let original_limits = ParametricSectorCoverageLimits::default();
    let shared = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            original_limits.generated_when_bad.ibp,
            original_limits.generated_when_bad.row_span,
        )
        .unwrap(),
    );
    let candidate = candidate(&context, shared.rows(), 2);
    let mut mismatched_limits = original_limits;
    mismatched_limits
        .generated_when_bad
        .row_span
        .limits
        .max_augmented_rows -= 1;

    assert!(matches!(
        ParametricSectorCoverageCompiler::compile_with_row_span(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            &[candidate],
            shared,
            mismatched_limits,
        ),
        Err(ParametricSectorCoverageError::GeneratedWhenBad(
            GeneratedWhenBadError::SharedRowSpanConfigMismatch
        ))
    ));
}
