//! Black-box tests for the candidate-to-normalized-source one-pass seam.
//!
//! These tests deliberately exercise the crate-private owner API from outside
//! its implementation module.  The fixture is a generated affine family; no
//! authored reduction relation or topology-specific production path enters
//! the candidate batch.

use std::sync::Arc;

use crate::generated_when_bad::{
    replayed_row_span_authentication_calls, reset_replayed_row_span_authentication_calls,
};
use crate::parametric_sector_coverage::ParametricSectorCoverageError;
use crate::parametric_sector_normalized_source::{
    ParametricSectorNormalizedCoverageSourceCompiler,
    ParametricSectorNormalizedCoverageSourceError, ParametricSectorNormalizedCoverageSourceLimits,
};
use crate::{
    AffineDenominator, GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanCompiler,
    GeneratedWhenBadCompiler, GeneratedWhenBadError, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricElimination, ParametricEliminationLimits,
    ParametricEliminationOrdering, ParametricIbpGenerator, ParametricReductionRuleCandidate,
    ParametricRuleLimits, SectorMask, algebra::CoefficientContext,
};

const CANDIDATE_COUNT: usize = 3;
const ORDERING: IntegralOrderingPolicy = IntegralOrderingPolicy::RustRedUnshiftedV1;

struct GeneratedBatchFixture {
    family: IntegralFamily,
    context: ParametricCoefficientContext,
    sector: SectorMask,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    candidates: Vec<ParametricReductionRuleCandidate>,
}

fn generated_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
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

fn generated_batch(name: &str) -> GeneratedBatchFixture {
    let family = generated_family(name);
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let sector = SectorMask::try_new([true]).unwrap();
    let limits = ParametricSectorNormalizedCoverageSourceLimits::default();
    let row_span = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            limits.coverage.generated_when_bad.ibp,
            limits.coverage.generated_when_bad.row_span,
        )
        .unwrap(),
    );
    let candidates = (0..CANDIDATE_COUNT)
        .map(|ordinal| {
            let anchor = i64::try_from(ordinal).unwrap().checked_add(2).unwrap();
            generated_candidate(&context, &row_span, sector.clone(), anchor)
        })
        .collect();
    GeneratedBatchFixture {
        family,
        context,
        sector,
        row_span,
        candidates,
    }
}

fn generated_candidate(
    context: &ParametricCoefficientContext,
    row_span: &GeneratedSymbolicRowSpanCertificate,
    sector: SectorMask,
    anchor: i64,
) -> ParametricReductionRuleCandidate {
    let elimination = ParametricElimination::build(
        context,
        row_span.rows(),
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
        row_span.rows(),
        &elimination,
        0,
        sector,
        ParametricRuleLimits::default(),
    )
    .unwrap()
}

#[test]
fn one_pass_authenticates_each_candidate_once_and_preserves_arc_and_order() {
    let fixture = generated_batch("one-pass-exact-authentication-count");
    let expected_anchors = fixture
        .candidates
        .iter()
        .map(|candidate| candidate.discovery_anchor().to_vec())
        .collect::<Vec<_>>();
    assert!(expected_anchors.windows(2).all(|pair| pair[0] != pair[1]));

    reset_replayed_row_span_authentication_calls();
    let source =
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector.clone(),
            ORDERING,
            fixture.candidates,
            Arc::clone(&fixture.row_span),
            ParametricSectorNormalizedCoverageSourceLimits::default(),
        )
        .unwrap();

    assert_eq!(replayed_row_span_authentication_calls(), CANDIDATE_COUNT);
    assert!(Arc::ptr_eq(source.row_span_arc(), &fixture.row_span));
    assert_eq!(source.attempts().len(), CANDIDATE_COUNT);
    assert_eq!(
        source
            .attempts()
            .iter()
            .map(|attempt| attempt.ordinal())
            .collect::<Vec<_>>(),
        (0..CANDIDATE_COUNT).collect::<Vec<_>>()
    );
    assert_eq!(
        source
            .attempts()
            .iter()
            .map(|attempt| attempt
                .compilation()
                .candidate()
                .discovery_anchor()
                .to_vec())
            .collect::<Vec<_>>(),
        expected_anchors,
    );
    assert!(source.attempts().iter().all(|attempt| Arc::ptr_eq(
        attempt.compilation().source_authentication().row_span_arc(),
        &fixture.row_span,
    )));
    assert_eq!(source.stats().coverage().candidates(), CANDIDATE_COUNT);
    assert_eq!(source.stats().normalization().attempts(), CANDIDATE_COUNT);

    reset_replayed_row_span_authentication_calls();
    source.replay(&fixture.family, &fixture.context).unwrap();
    assert_eq!(replayed_row_span_authentication_calls(), CANDIDATE_COUNT);
}

#[test]
fn one_pass_empty_batch_authenticates_zero_and_preserves_explicit_arc() {
    let fixture = generated_batch("one-pass-empty-batch");
    let mut limits = ParametricSectorNormalizedCoverageSourceLimits::default();
    limits.coverage.max_candidates = 0;
    limits.normalization = limits.normalization.with_max_attempts(0);

    reset_replayed_row_span_authentication_calls();
    let source =
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector,
            ORDERING,
            Vec::new(),
            Arc::clone(&fixture.row_span),
            limits,
        )
        .unwrap();

    assert_eq!(replayed_row_span_authentication_calls(), 0);
    assert!(Arc::ptr_eq(source.row_span_arc(), &fixture.row_span));
    assert!(source.attempts().is_empty());
    assert!(source.normalized().ir().attempts().is_empty());
    assert_eq!(source.stats().coverage().candidates(), 0);
    assert_eq!(source.stats().normalization().attempts(), 0);
}

#[test]
fn one_pass_preflights_late_candidate_metadata_before_any_authentication() {
    let mut fixture = generated_batch("one-pass-late-candidate-scope");
    let mut foreign = generated_batch("one-pass-late-candidate-scope-foreign");
    fixture.candidates.truncate(2);
    fixture.candidates.push(foreign.candidates.remove(0));

    reset_replayed_row_span_authentication_calls();
    assert!(matches!(
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector,
            ORDERING,
            fixture.candidates,
            fixture.row_span,
            ParametricSectorNormalizedCoverageSourceLimits::default(),
        ),
        Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
            ParametricSectorCoverageError::CandidateWrongFamily { ordinal: 2 }
        ))
    ));
    assert_eq!(replayed_row_span_authentication_calls(), 0);

    let mut sector_fixture = generated_batch("one-pass-late-candidate-sector");
    sector_fixture.candidates.truncate(2);
    sector_fixture.candidates.push(generated_candidate(
        &sector_fixture.context,
        &sector_fixture.row_span,
        SectorMask::try_new([false]).unwrap(),
        9,
    ));
    reset_replayed_row_span_authentication_calls();
    assert!(matches!(
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &sector_fixture.family,
            &sector_fixture.context,
            sector_fixture.sector,
            ORDERING,
            sector_fixture.candidates,
            sector_fixture.row_span,
            ParametricSectorNormalizedCoverageSourceLimits::default(),
        ),
        Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
            ParametricSectorCoverageError::CandidateWrongSector { ordinal: 2 }
        ))
    ));
    assert_eq!(replayed_row_span_authentication_calls(), 0);
}

#[derive(Clone, Copy)]
struct PredictableAggregateSourceCensus {
    canonical_rows: usize,
    canonical_terms: usize,
    retained_rows: usize,
    retained_terms: usize,
    minimum_match_attempts: usize,
}

fn predictable_aggregate_source_census(
    fixture: &GeneratedBatchFixture,
) -> PredictableAggregateSourceCensus {
    let candidate_count = fixture.candidates.len();
    let canonical_rows = fixture
        .row_span
        .stats()
        .canonical_rows()
        .checked_mul(candidate_count)
        .unwrap();
    let canonical_terms = fixture
        .row_span
        .stats()
        .canonical_terms()
        .checked_mul(candidate_count)
        .unwrap();
    let retained_rows = fixture
        .candidates
        .iter()
        .map(|candidate| candidate.derivation().source_rows().len())
        .sum();
    let retained_terms = fixture
        .candidates
        .iter()
        .flat_map(|candidate| candidate.derivation().source_rows())
        .map(|row| row.terms().len())
        .sum();
    PredictableAggregateSourceCensus {
        canonical_rows,
        canonical_terms,
        retained_rows,
        retained_terms,
        minimum_match_attempts: retained_rows,
    }
}

fn with_exact_aggregate_source_limits(
    mut limits: ParametricSectorNormalizedCoverageSourceLimits,
    census: PredictableAggregateSourceCensus,
) -> ParametricSectorNormalizedCoverageSourceLimits {
    limits.coverage.max_total_canonical_rows = census.canonical_rows;
    limits.coverage.max_total_canonical_terms = census.canonical_terms;
    limits.coverage.max_total_retained_source_rows = census.retained_rows;
    limits.coverage.max_total_retained_source_terms = census.retained_terms;
    limits.coverage.max_total_source_match_attempts = census.minimum_match_attempts;
    limits
}

#[test]
fn one_pass_aggregate_source_preflights_accept_exact_and_reject_one_below_before_authentication() {
    let fixture = generated_batch("one-pass-aggregate-source-preflights");
    let census = predictable_aggregate_source_census(&fixture);
    let exact = with_exact_aggregate_source_limits(
        ParametricSectorNormalizedCoverageSourceLimits::default(),
        census,
    );

    reset_replayed_row_span_authentication_calls();
    let exact_source =
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector.clone(),
            ORDERING,
            fixture.candidates.clone(),
            Arc::clone(&fixture.row_span),
            exact,
        )
        .unwrap();
    assert_eq!(replayed_row_span_authentication_calls(), CANDIDATE_COUNT);
    let exact_stats = exact_source.stats().coverage();
    assert_eq!(exact_stats.canonical_rows(), census.canonical_rows);
    assert_eq!(exact_stats.canonical_terms(), census.canonical_terms);
    assert_eq!(exact_stats.retained_source_rows(), census.retained_rows);
    assert_eq!(exact_stats.retained_source_terms(), census.retained_terms);
    assert_eq!(
        exact_stats.source_match_attempts(),
        census.minimum_match_attempts
    );

    type SetLimit = fn(&mut ParametricSectorNormalizedCoverageSourceLimits, usize);
    let one_below_cases: [(&str, usize, SetLimit); 5] = [
        (
            "sector-coverage canonical rows",
            census.canonical_rows,
            |limits, value| limits.coverage.max_total_canonical_rows = value,
        ),
        (
            "sector-coverage canonical terms",
            census.canonical_terms,
            |limits, value| limits.coverage.max_total_canonical_terms = value,
        ),
        (
            "sector-coverage retained source rows",
            census.retained_rows,
            |limits, value| limits.coverage.max_total_retained_source_rows = value,
        ),
        (
            "sector-coverage retained source terms",
            census.retained_terms,
            |limits, value| limits.coverage.max_total_retained_source_terms = value,
        ),
        (
            "sector-coverage source match attempts",
            census.minimum_match_attempts,
            |limits, value| limits.coverage.max_total_source_match_attempts = value,
        ),
    ];
    for (resource, requested, set_limit) in one_below_cases {
        assert!(requested > 0);
        let mut one_below = exact;
        set_limit(&mut one_below, requested - 1);
        reset_replayed_row_span_authentication_calls();
        match ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector.clone(),
            ORDERING,
            fixture.candidates.clone(),
            Arc::clone(&fixture.row_span),
            one_below,
        ) {
            Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: actual_resource,
                    requested: actual_requested,
                    limit,
                },
            )) => {
                assert_eq!(actual_resource, resource);
                assert_eq!(actual_requested, requested);
                assert_eq!(limit, requested - 1);
            }
            other => panic!("expected exact one-below failure for {resource}, got {other:?}"),
        }
        assert_eq!(
            replayed_row_span_authentication_calls(),
            0,
            "{resource} must fail before the first candidate authentication",
        );
    }
}

#[test]
fn one_pass_preflights_count_config_and_fingerprint_caps_without_authentication() {
    let mut fixture = generated_batch("one-pass-preflight-caps");
    let default = ParametricSectorNormalizedCoverageSourceLimits::default();

    let mut candidate_limited = default;
    candidate_limited.coverage.max_candidates = CANDIDATE_COUNT - 1;
    reset_replayed_row_span_authentication_calls();
    assert!(matches!(
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector.clone(),
            ORDERING,
            fixture.candidates.clone(),
            Arc::clone(&fixture.row_span),
            candidate_limited,
        ),
        Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
            ParametricSectorCoverageError::ResourceLimit {
                resource: "sector-coverage candidates",
                requested: CANDIDATE_COUNT,
                limit,
            }
        )) if limit == CANDIDATE_COUNT - 1
    ));
    assert_eq!(replayed_row_span_authentication_calls(), 0);

    let mut normalization_limited = default;
    normalization_limited.normalization = normalization_limited
        .normalization
        .with_max_attempts(CANDIDATE_COUNT - 1);
    reset_replayed_row_span_authentication_calls();
    assert!(matches!(
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector.clone(),
            ORDERING,
            fixture.candidates.clone(),
            Arc::clone(&fixture.row_span),
            normalization_limited,
        ),
        Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
            ParametricSectorCoverageError::ResourceLimit {
                resource: "formula-normalization attempts",
                requested: CANDIDATE_COUNT,
                limit,
            }
        )) if limit == CANDIDATE_COUNT - 1
    ));
    assert_eq!(replayed_row_span_authentication_calls(), 0);

    let mut family_fingerprint_limited = default;
    family_fingerprint_limited.normalization = family_fingerprint_limited
        .normalization
        .with_max_family_fingerprint_bytes(fixture.family.fingerprint().len() - 1);
    reset_replayed_row_span_authentication_calls();
    assert!(matches!(
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector.clone(),
            ORDERING,
            fixture.candidates.clone(),
            Arc::clone(&fixture.row_span),
            family_fingerprint_limited,
        ),
        Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
            ParametricSectorCoverageError::ResourceLimit {
                resource: "formula-normalization family fingerprint bytes",
                requested,
                limit,
            }
        )) if requested == fixture.family.fingerprint().len() && limit + 1 == requested
    ));
    assert_eq!(replayed_row_span_authentication_calls(), 0);

    let mut context_fingerprint_limited = default;
    context_fingerprint_limited.normalization = context_fingerprint_limited
        .normalization
        .with_max_context_fingerprint_bytes(fixture.context.fingerprint().len() - 1);
    reset_replayed_row_span_authentication_calls();
    assert!(matches!(
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector.clone(),
            ORDERING,
            fixture.candidates.clone(),
            Arc::clone(&fixture.row_span),
            context_fingerprint_limited,
        ),
        Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
            ParametricSectorCoverageError::ResourceLimit {
                resource: "formula-normalization context fingerprint bytes",
                requested,
                limit,
            }
        )) if requested == fixture.context.fingerprint().len() && limit + 1 == requested
    ));
    assert_eq!(replayed_row_span_authentication_calls(), 0);

    let mut wrong_row_span_config = default.coverage.generated_when_bad.row_span;
    wrong_row_span_config.limits.max_aggregate_manifest_bytes += 1;
    fixture.row_span = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &fixture.family,
            &fixture.context,
            default.coverage.generated_when_bad.ibp,
            wrong_row_span_config,
        )
        .unwrap(),
    );
    reset_replayed_row_span_authentication_calls();
    assert!(matches!(
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector,
            ORDERING,
            fixture.candidates,
            fixture.row_span,
            default,
        ),
        Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
            ParametricSectorCoverageError::GeneratedWhenBad(
                GeneratedWhenBadError::SharedRowSpanConfigMismatch
            )
        ))
    ));
    assert_eq!(replayed_row_span_authentication_calls(), 0);
}

#[test]
fn one_pass_payload_stats_and_priority_order_match_legacy_two_stage_source() {
    let fixture = generated_batch("one-pass-legacy-equivalence");
    let limits = ParametricSectorNormalizedCoverageSourceLimits::default();
    let expected_anchors = fixture
        .candidates
        .iter()
        .map(|candidate| candidate.discovery_anchor().to_vec())
        .collect::<Vec<_>>();
    assert!(expected_anchors.windows(2).all(|pair| pair[0] != pair[1]));

    // Only the legacy low-level compilation seam requires a caller replay.
    // The one-pass source constructor below owns its independent replay.
    fixture
        .row_span
        .replay(&fixture.family, &fixture.context)
        .unwrap();
    reset_replayed_row_span_authentication_calls();
    let legacy_compilations = fixture
        .candidates
        .iter()
        .map(|candidate| {
            GeneratedWhenBadCompiler::compile_with_replayed_row_span(
                &fixture.family,
                &fixture.context,
                candidate,
                Arc::clone(&fixture.row_span),
                limits.coverage.generated_when_bad,
            )
            .unwrap()
        })
        .collect();
    let legacy =
        ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector.clone(),
            ORDERING,
            legacy_compilations,
            Arc::clone(&fixture.row_span),
            limits,
        )
        .unwrap();
    assert_eq!(
        replayed_row_span_authentication_calls(),
        CANDIDATE_COUNT * 2,
        "legacy candidate compilation and source rebinding each authenticate the full batch",
    );

    reset_replayed_row_span_authentication_calls();
    let one_pass =
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &fixture.family,
            &fixture.context,
            fixture.sector,
            ORDERING,
            fixture.candidates,
            Arc::clone(&fixture.row_span),
            limits,
        )
        .unwrap();
    assert_eq!(replayed_row_span_authentication_calls(), CANDIDATE_COUNT);

    assert!(legacy.payload_eq(&one_pass));
    assert_eq!(legacy.stats(), one_pass.stats());
    assert_eq!(legacy.normalized(), one_pass.normalized());
    assert_eq!(legacy.attempts().len(), one_pass.attempts().len());
    assert_eq!(
        legacy
            .attempts()
            .iter()
            .map(|attempt| attempt
                .compilation()
                .candidate()
                .discovery_anchor()
                .to_vec())
            .collect::<Vec<_>>(),
        expected_anchors,
    );
    assert_eq!(
        one_pass
            .attempts()
            .iter()
            .map(|attempt| attempt
                .compilation()
                .candidate()
                .discovery_anchor()
                .to_vec())
            .collect::<Vec<_>>(),
        expected_anchors,
    );
    for (ordinal, (legacy_attempt, one_pass_attempt)) in legacy
        .attempts()
        .iter()
        .zip(one_pass.attempts())
        .enumerate()
    {
        assert_eq!(legacy_attempt.ordinal(), ordinal);
        assert_eq!(one_pass_attempt.ordinal(), ordinal);
        assert!(legacy_attempt.payload_eq(one_pass_attempt));
        assert!(Arc::ptr_eq(
            one_pass_attempt
                .compilation()
                .source_authentication()
                .row_span_arc(),
            &fixture.row_span,
        ));
    }
}
