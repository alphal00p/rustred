//! Explicit guarded completed-result reuse for immutable candidate programs.
//! This does not certify candidate formulas or replace their native evaluator.
use super::{CandidateDecomposition, CandidateReducer, CandidateReductionError};
use crate::algebra::CoefficientContext;
use crate::family::IntegralKey;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord, ApplicationCacheStage,
};
use crate::reduction::guarded_cache::{RootUse, export_bound, lookup_bound};
use crate::reduction::{
    CompletedCachePolicy, GuardedCacheBacking, GuardedCacheError, GuardedIntermediateCapture,
    GuardedRootCache, GuardedRootExportError, GuardedRootExportStatistics, GuardedRootResult,
    GuardedRootSource, ReductionError, ReductionLimits, ReductionStatistics,
    coefficient_cache_weight,
};
use std::sync::Arc;

/// Candidate authority and native cache/backing errors remain distinct. A
/// missing candidate rule is never a cache miss promoted to a terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateGuardedCacheError {
    Candidate(CandidateReductionError),
    Cache(GuardedCacheError),
}
impl From<CandidateReductionError> for CandidateGuardedCacheError {
    fn from(error: CandidateReductionError) -> Self {
        Self::Candidate(error)
    }
}
impl From<GuardedCacheError> for CandidateGuardedCacheError {
    fn from(error: GuardedCacheError) -> Self {
        Self::Cache(error)
    }
}
impl From<ReductionError> for CandidateGuardedCacheError {
    fn from(error: ReductionError) -> Self {
        Self::Cache(error.into())
    }
}
impl std::fmt::Display for CandidateGuardedCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Candidate(error) => error.fmt(f),
            Self::Cache(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CandidateGuardedCacheError {}

/// Immutable snapshot sharing the exact native rule program and installed
/// terminal convention. Each cold request gets an isolated empty native
/// session, with no global reducer lock/clear and no rule-polynomial clone.
///
/// The explicit outer cache stores completed guarded starting roots, which
/// also supply native descendants with the same artifact and convention.
/// LRU/LFU applies to those records; the transient candidate inner DAG retains
/// its existing coefficient-only FailOnLimit behavior. A fresh root retains
/// its complete native guard union once, including imported subtree guards,
/// under the same shared allowance. Guardless inner memo entries are never
/// exported as completed guarded records.
#[derive(Clone)]
pub struct CandidateUnitMassProgram<const N: usize> {
    snapshot: Arc<CandidateReducer<N>>,
    binding: ApplicationCacheBinding,
}

impl<const N: usize> CandidateUnitMassProgram<N> {
    /// `program_and_convention_digest` MUST bind the exact source bytes used
    /// to load `source`, including its installed weighted normalization or
    /// alias convention, using unambiguous framing. Pair them at the loader
    /// boundary, not by rereading a mutable path. The digest supplies identity,
    /// not a proof of candidate formulas. Existing source cache/work is ignored.
    pub fn new(source: &CandidateReducer<N>, program_and_convention_digest: [u8; 32]) -> Self {
        let mut snapshot = source.fork_empty();
        // Distributed guarded records always promise complete reductions.
        snapshot.unresolved_policy = super::UnresolvedPolicy::Reject;
        Self {
            snapshot: Arc::new(snapshot),
            binding: ApplicationCacheBinding {
                artifact_digest: program_and_convention_digest,
                family_fingerprint: source.family_fingerprint().into(),
                physical_arity: source.physical_arity(),
                layer: "candidate-guarded-root-canonical-unit-mass-v1".into(),
            },
        }
    }
    pub fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    pub fn context(&self) -> &CoefficientContext {
        self.snapshot.program.context.base()
    }
    pub fn canonical_terminals(&self) -> &std::collections::BTreeSet<IntegralKey> {
        self.snapshot.canonical_terminals()
    }
    fn identity(&self) -> usize {
        Arc::as_ptr(&self.snapshot) as usize
    }

    /// Limits are explicit and independent of the ordinary source reducer's
    /// operational defaults. One allowance covers outer roots plus the cold
    /// native session, including all retained guards; it is not an RSS bound.
    pub fn new_cache(
        &self,
        limits: ReductionLimits,
        policy: CompletedCachePolicy,
    ) -> GuardedRootCache {
        GuardedRootCache::bound(self.binding.clone(), self.identity(), limits, policy)
    }

    /// Native source-condition, root-sector, arity and entry-rank checks run
    /// before every RAM/backing hit under the current request allowances.
    pub fn validate_entry(
        &self,
        target: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<(), CandidateReductionError> {
        self.snapshot.validate_entry_with_limits(target, limits)
    }

    /// Validate a trusted native record at its load boundary. This checks
    /// current admission/context/limits, output ownership, source obligations
    /// and sealed zero/terminal rows; it does not replay a general candidate
    /// identity. Backing records remain explicitly trusted-generated inputs.
    pub fn validate_completed_record(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<(), CandidateGuardedCacheError> {
        if record.key.stage() != ApplicationCacheStage::GuardedRoot
            || !record.key.owner().is_empty()
        {
            return Err(GuardedCacheError::InvalidRecord("not a guarded root key").into());
        }
        let target = IntegralKey::try_new(record.key.powers().iter().copied())
            .map_err(ReductionError::from)?;
        self.validate_entry(&target, limits)?;
        if self.snapshot.is_zero(&target) {
            if !record.terms.is_empty() {
                return Err(GuardedCacheError::InvalidRecord(
                    "nonzero result for a native zero sector",
                )
                .into());
            }
        } else if self.snapshot.terminals().contains(&target) {
            if record.terms != self.snapshot.terminal_terms(&target)? {
                return Err(GuardedCacheError::InvalidRecord(
                    "result contradicts the installed native terminal row",
                )
                .into());
            }
            // Match the ordinary guarded terminal path, including weighted
            // row poles before any subsequent master cancellation.
            for coefficient in record.terms.values() {
                if !coefficient.denominator.is_constant()
                    && !record
                        .nonzero_conditions
                        .contains(&coefficient.denominator.clone().into())
                {
                    return Err(GuardedCacheError::InvalidRecord(
                        "missing native terminal-row denominator guard",
                    )
                    .into());
                }
            }
        }
        let required = self
            .snapshot
            .target_conditions_with_limits(&target, limits)?;
        if required
            .iter()
            .any(|guard| !record.nonzero_conditions.contains(guard))
        {
            return Err(
                GuardedCacheError::InvalidRecord("missing inherited native source guard").into(),
            );
        }
        for (terminal, coefficient) in &record.terms {
            if !self.canonical_terminals().contains(terminal) || coefficient.is_zero() {
                return Err(GuardedCacheError::InvalidRecord(
                    "foreign canonical terminal or explicit zero term",
                )
                .into());
            }
            self.context()
                .validate_with_limits(coefficient, limits.exact_algebra)
                .map_err(ReductionError::from)?;
        }
        for guard in &record.nonzero_conditions {
            self.context()
                .validate_with_limits(guard, limits.exact_algebra)
                .map_err(ReductionError::from)?;
            if guard.is_zero() {
                return Err(GuardedCacheError::InvalidRecord("zero nonzero-condition").into());
            }
        }
        let weight =
            coefficient_cache_weight(record.terms.values().chain(&record.nonzero_conditions))?;
        crate::reduction::cache_policy::check_entry(weight, limits)?;
        Ok(())
    }

    pub fn reduce_guarded_root(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<GuardedRootResult, CandidateGuardedCacheError> {
        self.reduce_guarded_root_impl(target, cache, limits, backing, None)
    }

    /// Opt-in streaming of newly computed admitted descendants, each with its
    /// own exact guard provenance. No subtree guard vectors are retained in
    /// the coefficient memo. A bounded provenance allowance can disable further
    /// optional capture without interrupting the native requested reduction.
    pub fn reduce_guarded_root_with_capture(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
        capture: &mut dyn GuardedIntermediateCapture,
    ) -> Result<GuardedRootResult, CandidateGuardedCacheError> {
        self.reduce_guarded_root_impl(target, cache, limits, backing, Some(capture))
    }

    fn reduce_guarded_root_impl(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        mut backing: Option<&mut dyn GuardedCacheBacking>,
        capture: Option<&mut dyn GuardedIntermediateCapture>,
    ) -> Result<GuardedRootResult, CandidateGuardedCacheError> {
        let limits = limits.bounded_by(cache.limits());
        if let Some((record, source)) =
            self.lookup_completed(target, cache, limits, &mut backing, RootUse::Entry)?
        {
            let mut work = ReductionStatistics::default();
            work.record_cache_hit();
            return Ok(GuardedRootResult {
                record: record.as_ref().clone(),
                work,
                source,
            });
        }
        let shared = cache.shared_budget();
        let (unit, nonzero_conditions, work) = {
            let mut lookup = |descendant: &IntegralKey| {
                self.lookup_completed(descendant, cache, limits, &mut backing, RootUse::Descendant)
                    .map(|found| found.map(|(record, _)| record))
            };
            let mut session = self
                .snapshot
                .fork_empty_with_shared_budget(limits, Arc::clone(&shared));
            if let Some(sink) = capture {
                let mut capture =
                    super::capture::CandidateCapture::new(self, target, sink, shared, limits);
                let result = session.reduce_fresh_unit_mass_guarded_with_capture(
                    target,
                    limits,
                    &mut lookup,
                    &mut capture,
                );
                capture.report();
                result?
            } else {
                session.reduce_fresh_unit_mass_guarded_with_lookup(target, limits, &mut lookup)?
            }
        }; // Drop releases only this native session's retained share.
        cache.completed_reduction(work);
        let record = Arc::new(ApplicationCacheRecord {
            key: ApplicationCacheKey::guarded_root(target),
            terms: unit.into_terms(),
            nonzero_conditions,
        });
        cache.validate_payload(|| self.validate_completed_record(&record, limits))?;
        if let Some(store) = backing {
            store
                .store_completed(&record, self.context())
                .map_err(GuardedCacheError::from)?;
        }
        cache.insert_shared(target.clone(), Arc::clone(&record), limits)?;
        Ok(GuardedRootResult {
            record: record.as_ref().clone(),
            work,
            source: GuardedRootSource::Reduced,
        })
    }

    pub(super) fn lookup_completed(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: &mut Option<&mut dyn GuardedCacheBacking>,
        usage: RootUse,
    ) -> Result<Option<(Arc<ApplicationCacheRecord>, GuardedRootSource)>, CandidateGuardedCacheError>
    {
        if let Err(error) = self.validate_entry(target, limits) {
            // The serialized namespace contains admitted starting roots.
            // Exceeding its entry rank does not clip native descendants or
            // extend the authority of those stored records.
            if usage == RootUse::Descendant
                && matches!(error, CandidateReductionError::OutsideNumeratorRank { .. })
            {
                return Ok(None);
            }
            return Err(error.into());
        }
        lookup_bound(
            &self.binding,
            self.identity(),
            self.context(),
            target,
            cache,
            limits,
            backing,
            usage,
            |record| self.validate_completed_record(record, limits),
        )
    }

    fn decomposition_validated(
        &self,
        target: IntegralKey,
        record: &ApplicationCacheRecord,
    ) -> CandidateDecomposition {
        CandidateDecomposition {
            family_fingerprint: Arc::clone(&self.snapshot.program.family_fingerprint),
            target,
            terms: record.terms.clone(),
            unresolved: Default::default(),
        }
    }

    /// Arbitrary external records are validated. The returned native
    /// CandidateDecomposition remains the common-mass exponent authority;
    /// this API does not relabel a candidate result as a certified master.
    pub fn decomposition(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<CandidateDecomposition, CandidateGuardedCacheError> {
        self.validate_completed_record(record, limits)?;
        let target = IntegralKey::try_new(record.key.powers().iter().copied())
            .map_err(ReductionError::from)?;
        Ok(self.decomposition_validated(target, record))
    }

    /// Materialize the just-validated native result without a second warm
    /// payload traversal. Use CandidateDecomposition::common_mass_squared_power
    /// for mass restoration; cached/persisted coefficients stay unit mass.
    pub fn reduce_guarded_root_with_decomposition(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<(GuardedRootResult, CandidateDecomposition), CandidateGuardedCacheError> {
        let result = self.reduce_guarded_root(target, cache, limits, backing)?;
        let unit = self.decomposition_validated(target.clone(), &result.record);
        Ok((result, unit))
    }

    /// Export resident completed roots one Arc at a time, with callbacks
    /// unlocked. Usage/aging/work is not serialized. Counts denote callback
    /// acceptance, not a durability checkpoint; the backing owns bounded IO.
    pub fn export_completed(
        &self,
        cache: &GuardedRootCache,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<GuardedRootExportStatistics, GuardedRootExportError> {
        export_bound(
            &self.binding,
            self.identity(),
            self.context(),
            cache,
            backing,
        )
    }
}
