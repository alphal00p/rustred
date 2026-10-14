//! Native-program delegation for the shared application cache session. This
//! module owns no reduction, coefficient arithmetic, or terminal conventions.
use super::session::{guarded_error, reduction_error};
use crate::{AppError, AppErrorKind};
use rustred::{
    algebra::CoefficientContext,
    family::IntegralKey,
    persistence::application_cache::{ApplicationCacheBinding, ApplicationCacheRecord},
    reduction::{
        ClosedUnitMassProgram, CompletedCachePolicy, GuardedCacheBacking,
        GuardedIntermediateCapture, GuardedRootCache, GuardedRootExportError,
        GuardedRootExportStatistics, GuardedRootResult, HomogeneousMasterDecomposition,
        ReductionError, ReductionLimits,
    },
    solver::{
        CandidateDecomposition, CandidateGuardedCacheError, CandidateReductionError,
        CandidateUnitMassProgram,
    },
};

mod sealed {
    pub trait Sealed {}
}

/// Sealed implementation boundary: callers select existing native programs,
/// not alternative mathematical authorities. The associated decomposition
/// preserves the distinction between closed masters and candidate terminals.
pub trait NativeCacheProgram: sealed::Sealed {
    type Decomposition;
    fn kind(&self) -> &'static str;
    fn binding(&self) -> &ApplicationCacheBinding;
    fn context(&self) -> &CoefficientContext;
    fn new_cache(&self, limits: ReductionLimits, policy: CompletedCachePolicy) -> GuardedRootCache;
    fn validate_entry(&self, key: &IntegralKey, limits: ReductionLimits) -> Result<(), AppError>;
    fn validate_record(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<(), AppError>;
    fn reduce(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<GuardedRootResult, AppError>;
    fn reduce_with_capture(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
        capture: &mut dyn GuardedIntermediateCapture,
    ) -> Result<GuardedRootResult, AppError>;
    fn reduce_decomposition(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<(GuardedRootResult, Self::Decomposition), AppError>;
    fn decomposition(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<Self::Decomposition, AppError>;
    fn export(
        &self,
        cache: &GuardedRootCache,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<GuardedRootExportStatistics, GuardedRootExportError>;
}

impl sealed::Sealed for ClosedUnitMassProgram<'_> {}
impl NativeCacheProgram for ClosedUnitMassProgram<'_> {
    type Decomposition = HomogeneousMasterDecomposition;
    fn kind(&self) -> &'static str {
        "native-closed-guarded-root"
    }
    fn binding(&self) -> &ApplicationCacheBinding {
        self.binding()
    }
    fn context(&self) -> &CoefficientContext {
        self.context()
    }
    fn new_cache(&self, limits: ReductionLimits, policy: CompletedCachePolicy) -> GuardedRootCache {
        GuardedRootCache::with_policy(self, limits, policy)
    }
    fn validate_entry(&self, key: &IntegralKey, _: ReductionLimits) -> Result<(), AppError> {
        self.validate_entry(key).map_err(reduction_error)
    }
    fn validate_record(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<(), AppError> {
        self.validate_completed_record(record, limits)
            .map_err(guarded_error)
    }
    fn reduce(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<GuardedRootResult, AppError> {
        self.reduce_guarded_root(target, cache, limits, backing)
            .map_err(guarded_error)
    }
    fn reduce_with_capture(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
        capture: &mut dyn GuardedIntermediateCapture,
    ) -> Result<GuardedRootResult, AppError> {
        self.reduce_guarded_root_with_capture(target, cache, limits, backing, capture)
            .map_err(guarded_error)
    }
    fn reduce_decomposition(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<(GuardedRootResult, Self::Decomposition), AppError> {
        self.reduce_guarded_root_with_homogeneity(target, cache, limits, backing)
            .map_err(guarded_error)
    }
    fn decomposition(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<Self::Decomposition, AppError> {
        self.homogeneous_decomposition(record, limits)
            .map_err(guarded_error)
    }
    fn export(
        &self,
        cache: &GuardedRootCache,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<GuardedRootExportStatistics, GuardedRootExportError> {
        self.export_completed(cache, backing)
    }
}

impl<const N: usize> sealed::Sealed for CandidateUnitMassProgram<N> {}
impl<const N: usize> NativeCacheProgram for CandidateUnitMassProgram<N> {
    type Decomposition = CandidateDecomposition;
    fn kind(&self) -> &'static str {
        "native-candidate-guarded-root"
    }
    fn binding(&self) -> &ApplicationCacheBinding {
        self.binding()
    }
    fn context(&self) -> &CoefficientContext {
        self.context()
    }
    fn new_cache(&self, limits: ReductionLimits, policy: CompletedCachePolicy) -> GuardedRootCache {
        self.new_cache(limits, policy)
    }
    fn validate_entry(&self, key: &IntegralKey, limits: ReductionLimits) -> Result<(), AppError> {
        self.validate_entry(key, limits).map_err(candidate_error)
    }
    fn validate_record(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<(), AppError> {
        self.validate_completed_record(record, limits)
            .map_err(candidate_guarded_error)
    }
    fn reduce(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<GuardedRootResult, AppError> {
        self.reduce_guarded_root(target, cache, limits, backing)
            .map_err(candidate_guarded_error)
    }
    fn reduce_with_capture(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
        capture: &mut dyn GuardedIntermediateCapture,
    ) -> Result<GuardedRootResult, AppError> {
        self.reduce_guarded_root_with_capture(target, cache, limits, backing, capture)
            .map_err(candidate_guarded_error)
    }
    fn reduce_decomposition(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<(GuardedRootResult, Self::Decomposition), AppError> {
        self.reduce_guarded_root_with_decomposition(target, cache, limits, backing)
            .map_err(candidate_guarded_error)
    }
    fn decomposition(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<Self::Decomposition, AppError> {
        self.decomposition(record, limits)
            .map_err(candidate_guarded_error)
    }
    fn export(
        &self,
        cache: &GuardedRootCache,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<GuardedRootExportStatistics, GuardedRootExportError> {
        self.export_completed(cache, backing)
    }
}

fn candidate_guarded_error(error: CandidateGuardedCacheError) -> AppError {
    match error {
        CandidateGuardedCacheError::Candidate(error) => candidate_error(error),
        CandidateGuardedCacheError::Cache(error) => guarded_error(error),
    }
}
fn candidate_error(error: CandidateReductionError) -> AppError {
    match error {
        CandidateReductionError::Application(error) => reduction_error(error),
        CandidateReductionError::Algebra(error) => {
            reduction_error(ReductionError::IndexedAlgebra(error))
        }
        CandidateReductionError::TraceInputLimit { .. }
        | CandidateReductionError::TraceIntegralLimit { .. }
        | CandidateReductionError::IndexOverflow { .. } => AppError::limit(error.to_string()),
        CandidateReductionError::Uncovered { .. }
        | CandidateReductionError::NonDescending { .. } => {
            AppError::new(AppErrorKind::Execution, error.to_string())
        }
        _ => AppError::input(error.to_string()),
    }
}
