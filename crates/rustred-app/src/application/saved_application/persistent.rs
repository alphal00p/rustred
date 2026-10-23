//! Saved-publication facade over native completed Route/Apply persistence.
use super::*;
use rustred::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use rustred::reduction::{GuardedCacheBacking, GuardedIntermediateCapture};

impl<const N: usize> PreparedSavedApplication<N> {
    /// `artifact_digest` is the consumer's authenticated semantic digest of the
    /// complete application and catalogue bindings, independent of cache files
    /// and release labels. Terminal substitutions must be included.
    pub fn cache_binding(&self, artifact_digest: [u8; 32]) -> ApplicationCacheBinding {
        self.application.persistent_cache_binding(artifact_digest)
    }

    pub fn validate_completed_cache_record(
        &self,
        binding: &ApplicationCacheBinding,
        record: &ApplicationCacheRecord,
    ) -> Result<(), AppError> {
        self.application
            .validate_completed_cache_record(binding, record)
            .map_err(failure)
    }

    /// Apply with optional native backing and bounded completed-node capture.
    /// Capture requires strict mode; a read-only backing also supports Preserve.
    pub fn reduce_linear_combination_with_cache<'a>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        binding: &ApplicationCacheBinding,
        backing: Option<&'a mut dyn GuardedCacheBacking>,
        capture: Option<&'a mut dyn GuardedIntermediateCapture>,
    ) -> Result<SavedApplicationReduction, AppError> {
        let reduced = self
            .application
            .reduce_linear_combination_with_cache(
                terms,
                entry::admission(Some(&self.entry_domain)),
                binding,
                backing,
                capture,
            )
            .map_err(failure)?;
        self.apply_terminal_substitutions(reduced)
    }

    /// Verified entry routing and the complete native reduction share the same
    /// bounded request and the same optional persistent cache.
    pub fn reduce_routed_linear_combination_with_cache<'a>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        binding: &ApplicationCacheBinding,
        backing: Option<&'a mut dyn GuardedCacheBacking>,
        capture: Option<&'a mut dyn GuardedIntermediateCapture>,
    ) -> Result<SavedApplicationReduction, AppError> {
        let reduced = self
            .application
            .reduce_routed_linear_combination_with_cache(
                terms,
                entry::admission(Some(&self.entry_domain)),
                binding,
                backing,
                capture,
            )
            .map_err(failure)?;
        self.apply_terminal_substitutions(reduced)
    }

    pub fn completed_cache_record(
        &self,
        binding: &ApplicationCacheBinding,
        key: &ApplicationCacheKey,
    ) -> Result<Option<ApplicationCacheRecord>, AppError> {
        self.application
            .completed_cache_record(binding, key)
            .map_err(failure)
    }

    /// Stream resident completed nodes, filtering unresolved records. This does not copy unloaded
    /// backing records or claim checkpoint durability.
    pub fn export_completed(
        &self,
        binding: &ApplicationCacheBinding,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<usize, AppError> {
        self.application
            .export_completed(binding, backing)
            .map_err(failure)
    }
    pub fn export_completed_with_statistics(
        &self,
        binding: &ApplicationCacheBinding,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<rustred::solver::CandidateRoutedCacheExportStatistics, AppError> {
        self.application
            .export_completed_with_statistics(binding, backing)
            .map_err(failure)
    }
}
