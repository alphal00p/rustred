//! Honest all-36 six-loop/K=21 stress input shared by backend tests.
//!
//! This module is test-only.  It constructs the generic coordinate family,
//! generated row span, adaptive candidates, and the backend-neutral normalized
//! source through the safe one-pass candidate seam without selecting a
//! decision backend.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::adaptive_rules::{AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits};
use crate::parametric_sector_normalized_source::{
    ParametricSectorNormalizedCoverageSource, ParametricSectorNormalizedCoverageSourceCompiler,
    ParametricSectorNormalizedCoverageSourceLimits,
};
use crate::{
    AffineDenominator, ConcreteIntegralKey, GeneratedSymbolicRowSpanCompiler, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpGenerator, SectorMask,
    algebra::CoefficientContext,
};

pub(crate) const SIX_LOOP_K21_ARITY: usize = 21;
pub(crate) const SIX_LOOP_K21_ROW_COUNT: usize = 36;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SixLoopK21BuildTimings {
    pub(crate) family_and_context: Duration,
    pub(crate) row_span: Duration,
    pub(crate) adaptive_candidates: Duration,
    pub(crate) candidate_to_normalized_source: Duration,
}

pub(crate) struct SixLoopK21NormalizedFixture {
    pub(crate) family: IntegralFamily,
    pub(crate) context: ParametricCoefficientContext,
    pub(crate) source: Arc<ParametricSectorNormalizedCoverageSource>,
    pub(crate) timings: SixLoopK21BuildTimings,
}

fn six_loop_unit_mass_coordinate_basis(name: &str) -> IntegralFamily {
    const LOOPS: usize = 6;
    let coefficients = CoefficientContext::new(["d"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let denominators = (0..SIX_LOOP_K21_ARITY)
        .map(|row| {
            AffineDenominator::new(
                coefficients.integer(-1),
                (0..SIX_LOOP_K21_ARITY)
                    .map(|column| {
                        if row == column {
                            one.clone()
                        } else {
                            zero.clone()
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    IntegralFamily::new(
        name,
        (0..LOOPS).map(|loop_| format!("k{}", loop_ + 1)).collect(),
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![zero; SIX_LOOP_K21_ARITY],
    )
    .unwrap()
}

pub(crate) fn compile_six_loop_k21_normalized_fixture(name: &str) -> SixLoopK21NormalizedFixture {
    let started = Instant::now();
    let family = six_loop_unit_mass_coordinate_basis(name);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let family_and_context = started.elapsed();

    let source_limits = ParametricSectorNormalizedCoverageSourceLimits::default();
    let started = Instant::now();
    let row_span = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            source_limits.coverage.generated_when_bad.ibp,
            source_limits.coverage.generated_when_bad.row_span,
        )
        .unwrap(),
    );
    assert_eq!(row_span.rows().len(), SIX_LOOP_K21_ROW_COUNT);
    let row_span_time = started.elapsed();

    let started = Instant::now();
    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 0;
    let mut adaptive = AdaptiveParametricRuleProvider::try_new(
        &context,
        row_span.rows(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        adaptive_limits,
    )
    .unwrap();
    let corner = ConcreteIntegralKey::try_new(vec![0; SIX_LOOP_K21_ARITY]).unwrap();
    let candidates = adaptive.candidates_for_quotient(&corner).unwrap();
    assert_eq!(candidates.len(), SIX_LOOP_K21_ROW_COUNT);
    let adaptive_candidates = started.elapsed();

    let started = Instant::now();
    let source = Arc::new(
        ParametricSectorNormalizedCoverageSourceCompiler::compile_candidates_with_row_span(
            &family,
            &context,
            SectorMask::try_new([false; SIX_LOOP_K21_ARITY]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            candidates,
            Arc::clone(&row_span),
            source_limits,
        )
        .unwrap(),
    );
    let candidate_to_normalized_source = started.elapsed();

    SixLoopK21NormalizedFixture {
        family,
        context,
        source,
        timings: SixLoopK21BuildTimings {
            family_and_context,
            row_span: row_span_time,
            adaptive_candidates,
            candidate_to_normalized_source,
        },
    }
}
