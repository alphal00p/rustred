//! Saved-publication facade over native completed Route/Apply persistence.
use super::*;
use rustred::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use rustred::reduction::{GuardedCacheBacking, GuardedIntermediateCapture};
use rustred::solver::CandidateRoutedApplicationObserver;

#[cfg(test)]
#[path = "selected_cache_tests.rs"]
mod selected_cache_tests;

impl<const N: usize> PreparedSavedApplication<N> {
    pub(super) fn enforces_recorded_entry_coverage(&self) -> bool {
        self.options.entry_coverage_policy
            == SavedApplicationEntryCoveragePolicy::FollowUnresolvedPolicy
            && self.options.unresolved_policy == UnresolvedPolicy::Reject
    }

    pub(super) fn entry_admission(
        domain: &RequestedEntryDomain<N>,
        policy: SavedApplicationEntryCoveragePolicy,
    ) -> rustred::solver::CandidateEntryAdmission<'_, N> {
        match policy {
            SavedApplicationEntryCoveragePolicy::FollowUnresolvedPolicy => {
                entry::admission(Some(domain))
            }
            SavedApplicationEntryCoveragePolicy::Advisory => {
                rustred::solver::CandidateEntryAdmission::Advisory
            }
        }
    }

    /// Exact saved coverage, independently of operational application policy.
    pub fn entry_is_admitted(&self, key: &IntegralKey) -> Result<bool, AppError> {
        self.entry_domain.is_admitted(key)
    }

    /// Native structural/source validation and the current entry policy. Cache
    /// preparation uses this before resume lookups, not a coverage certificate.
    pub fn validate_entry_for_reduction(&self, key: &IntegralKey) -> Result<(), AppError> {
        self.application
            .validate_entry_for_reduction(
                key,
                Self::entry_admission(&self.entry_domain, self.options.entry_coverage_policy),
            )
            .map_err(native_failure)
    }

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
            .map_err(native_failure)
    }

    /// Encode an explicit subset of an immutable native pack in the existing
    /// single-file format, with one shared coefficient table. The caller must
    /// authenticate `binding.artifact_digest` against its complete campaign and
    /// catalogue; family, arity, solver layer and the backing must agree here.
    ///
    /// Duplicate requested keys are exported once. Missing keys, unsupported
    /// stages, unresolved terms and corrupt selected payloads are errors. This
    /// performs no reduction and does not export unrelated resident entries.
    /// Exact terminal coefficients and ordered nonzero conditions are retained.
    /// The result is not published until the caller atomically writes the
    /// fully encoded bytes; an error never supplies a partial cache file.
    pub fn encode_selected_completed_cache(
        &self,
        binding: &ApplicationCacheBinding,
        backing: &mut SavedApplicationCachePack,
        keys: &[ApplicationCacheKey],
        limits: SavedApplicationCacheFileLimits,
    ) -> Result<(Vec<u8>, SavedApplicationCacheFileIdentity), AppError> {
        if backing.binding() != binding || self.cache_binding(binding.artifact_digest) != *binding {
            return Err(AppError::input("selected cache export binding mismatch"));
        }
        let mut index_bytes = keys
            .len()
            .checked_mul(std::mem::size_of::<ApplicationCacheRecord>())
            .ok_or_else(|| AppError::limit("selected cache record allocation overflow"))?;
        if index_bytes > limits.chunk.max_index_owned_bytes {
            return Err(AppError::limit(
                "selected cache record index exceeds allowance",
            ));
        }
        cache_pack::resource_allocation(index_bytes)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(keys.len())
            .map_err(|error| AppError::limit(error.to_string()))?;
        let mut coefficient_terms = 0usize;
        let mut coefficient_bytes = 0usize;
        let context = self.family().coefficient_context();
        backing.visit_selected_completed_records(keys, context, |record| {
            self.validate_completed_cache_record(binding, &record)?;
            let (terms, bytes) = record
                .coefficient_census()
                .map_err(cache_pack::binary_error)?;
            coefficient_terms = coefficient_terms.checked_add(terms).ok_or_else(|| {
                AppError::limit("selected cache coefficient term census overflow")
            })?;
            coefficient_bytes = coefficient_bytes.checked_add(bytes).ok_or_else(|| {
                AppError::limit("selected cache coefficient byte census overflow")
            })?;
            index_bytes = index_bytes
                .checked_add(record.key.owned_heap_bytes())
                .ok_or_else(|| AppError::limit("selected cache key census overflow"))?;
            if (limits.chunk.diagnostics_enabled
                && (records.len().saturating_add(1) > limits.chunk.max_records
                    || coefficient_terms > limits.chunk.max_decoded_coefficient_terms))
                || coefficient_bytes > limits.chunk.max_decoded_coefficient_bytes
                || index_bytes > limits.chunk.max_index_owned_bytes
            {
                return Err(AppError::limit(
                    "selected cache record collection exceeds allowance",
                ));
            }
            records.push(record);
            Ok(())
        })?;
        records.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        SavedApplicationCacheFile::encode_generated(binding, &records, context, limits)
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
                Self::entry_admission(&self.entry_domain, self.options.entry_coverage_policy),
                binding,
                backing,
                capture,
            )
            .map_err(native_failure)?;
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
                Self::entry_admission(&self.entry_domain, self.options.entry_coverage_policy),
                binding,
                backing,
                capture,
            )
            .map_err(native_failure)?;
        self.apply_terminal_substitutions(reduced)
    }

    /// Observe the same native cache/capture request and saved substitutions.
    /// A native success precedes saved terminal substitution; only the saved
    /// final event completes this request. Telemetry faults preserve its result.
    pub fn reduce_linear_combination_with_cache_and_observation<'a>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        binding: &ApplicationCacheBinding,
        backing: Option<&'a mut dyn GuardedCacheBacking>,
        capture: Option<&'a mut dyn GuardedIntermediateCapture>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&AtomicBool>,
    ) -> Result<SavedApplicationReduction, AppError> {
        progress::begin_observation(observer, self.family().fingerprint());
        let reduced = self
            .application
            .reduce_linear_combination_with_cache_and_observation(
                terms,
                Self::entry_admission(&self.entry_domain, self.options.entry_coverage_policy),
                binding,
                backing,
                capture,
                observer,
                cancel,
            )
            .map_err(native_failure);
        self.finish_observed_reduction(reduced, observer, cancel)
    }

    /// Verified initial routing, optional persistent cache/capture and saved
    /// substitutions share the ordinary exact kernels and this one observer.
    pub fn reduce_routed_linear_combination_with_cache_and_observation<'a>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        binding: &ApplicationCacheBinding,
        backing: Option<&'a mut dyn GuardedCacheBacking>,
        capture: Option<&'a mut dyn GuardedIntermediateCapture>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&AtomicBool>,
    ) -> Result<SavedApplicationReduction, AppError> {
        progress::begin_observation(observer, self.family().fingerprint());
        let reduced = self
            .application
            .reduce_routed_linear_combination_with_cache_and_observation(
                terms,
                Self::entry_admission(&self.entry_domain, self.options.entry_coverage_policy),
                binding,
                backing,
                capture,
                observer,
                cancel,
            )
            .map_err(native_failure);
        self.finish_observed_reduction(reduced, observer, cancel)
    }

    pub fn completed_cache_record(
        &self,
        binding: &ApplicationCacheBinding,
        key: &ApplicationCacheKey,
    ) -> Result<Option<ApplicationCacheRecord>, AppError> {
        self.application
            .completed_cache_record(binding, key)
            .map_err(native_failure)
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
            .map_err(native_failure)
    }
    pub fn export_completed_with_statistics(
        &self,
        binding: &ApplicationCacheBinding,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<rustred::solver::CandidateRoutedCacheExportStatistics, AppError> {
        self.application
            .export_completed_with_statistics(binding, backing)
            .map_err(native_failure)
    }
}
