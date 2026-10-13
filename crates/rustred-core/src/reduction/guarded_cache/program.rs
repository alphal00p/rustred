use super::{
    GuardedCacheBacking, GuardedCacheError, GuardedRootCache, GuardedRootResult, GuardedRootSource,
    RootUse,
};
use crate::algebra::CoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::{ClosedArtifact, CommonMassHomogeneityProof};
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord, ApplicationCacheStage,
};
use crate::reduction::{
    HomogeneousMasterCoefficient, HomogeneousMasterDecomposition, MasterDecomposition, Reducer,
    ReductionError, ReductionLimits, ReductionStatistics, coefficient_cache_weight,
};
use std::sync::Arc;

const LAYER: &str = "closed-guarded-root-raw-unit-mass-v1";

/// Immutable borrowed program plus exact persisted-byte identity. Construct
/// once from the artifact decoded by the loader; no deep artifact clone.
pub struct ClosedUnitMassProgram<'a> {
    artifact: &'a ClosedArtifact,
    binding: ApplicationCacheBinding,
}

impl<'a> ClosedUnitMassProgram<'a> {
    /// `artifact_digest` MUST hash the exact authenticated source bytes used
    /// to decode `artifact`, including its terminal convention. The caller
    /// supplies this pair at the decode boundary, not by rereading a mutable
    /// path later. A digest is an identity, not new mathematical authority.
    pub fn new(artifact: &'a ClosedArtifact, artifact_digest: [u8; 32]) -> Self {
        Self {
            artifact,
            binding: ApplicationCacheBinding {
                artifact_digest,
                family_fingerprint: artifact.family_fingerprint().into(),
                physical_arity: artifact.arity(),
                layer: LAYER.into(),
            },
        }
    }
    pub fn artifact(&self) -> &'a ClosedArtifact {
        self.artifact
    }
    pub fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    pub fn context(&self) -> &CoefficientContext {
        self.artifact.coefficient_context()
    }
    pub(super) fn artifact_identity(&self) -> usize {
        self.artifact as *const ClosedArtifact as usize
    }
    pub fn validate_entry(&self, target: &IntegralKey) -> Result<(), ReductionError> {
        self.artifact
            .validate_root_powers(target.powers())
            .map_err(Into::into)
    }

    /// Validate a trusted-generated record's shape, current entry domain,
    /// terminal ownership, coefficient context and current payload allowance.
    /// This does not prove a supplied identity: backing stores remain explicit
    /// trusted-native inputs, just like the native record codec.
    pub fn validate_completed_record(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<(), GuardedCacheError> {
        if record.key.stage() != ApplicationCacheStage::GuardedRoot
            || !record.key.owner().is_empty()
        {
            return Err(GuardedCacheError::InvalidRecord("not a guarded root key"));
        }
        let target = IntegralKey::try_new(record.key.powers().iter().copied())
            .map_err(ReductionError::from)?;
        self.validate_entry(&target)?;
        if self.artifact.is_zero_terminal(&target) && !record.terms.is_empty() {
            return Err(GuardedCacheError::InvalidRecord(
                "nonzero result for a sealed zero terminal",
            ));
        }
        if self.artifact.masters().contains(&target)
            && (record.terms.len() != 1 || record.terms.get(&target) != Some(&self.context().one()))
        {
            return Err(GuardedCacheError::InvalidRecord(
                "non-identity result for a sealed master terminal",
            ));
        }
        for (master, coefficient) in &record.terms {
            if !self.artifact.masters().contains(master) || coefficient.is_zero() {
                return Err(GuardedCacheError::InvalidRecord(
                    "foreign master or explicit zero term",
                ));
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
                return Err(GuardedCacheError::InvalidRecord("zero nonzero-condition"));
            }
        }
        let weight =
            coefficient_cache_weight(record.terms.values().chain(&record.nonzero_conditions))?;
        super::super::cache_policy::check_entry(weight, limits)?;
        Ok(())
    }

    /// Admission precedes every RAM/backing hit. Only complete guarded roots
    /// enter either cache. Cold work uses the unchanged native guarded reducer
    /// with one shared budget for retained roots and transient native payloads.
    /// Pressure evicts an eligible outer root and retries the same insertion,
    /// never restarting the solve or resetting a work grant.
    /// Completed prepared roots may also supply admitted same-artifact
    /// descendants. Their guard vectors are merged in stored order using the
    /// existing first-occurrence union; this deterministic sequence can differ
    /// from an entirely cold DAG traversal while preserving every obligation.
    pub fn reduce_guarded_root(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        mut backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<GuardedRootResult, GuardedCacheError> {
        let limits = limits.bounded_by(cache.limits);
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
            Reducer::reduce_artifact_unit_mass_guarded_with_lookup(
                self.artifact,
                target,
                limits,
                shared,
                &mut lookup,
            )?
        };
        cache.completed_reduction(work);
        let record = Arc::new(ApplicationCacheRecord {
            key: ApplicationCacheKey::guarded_root(target),
            terms: unit.into_terms(),
            nonzero_conditions,
        });
        cache.validate_payload(|| self.validate_completed_record(&record, limits))?;
        if let Some(store) = backing {
            store.store_completed(&record, self.context())?;
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
    ) -> Result<Option<(Arc<ApplicationCacheRecord>, GuardedRootSource)>, GuardedCacheError> {
        if let Err(error) = self.validate_entry(target) {
            // A root-only admission boundary is not a descendant boundary.
            // Such nodes stay inside the existing certified native descent;
            // they cannot consult or populate the external guarded-root cache.
            if usage == RootUse::Descendant
                && matches!(
                    error,
                    ReductionError::OutsideCertifiedRootDomain { .. }
                        | ReductionError::OutsideCertifiedTotalExcessDomain { .. }
                )
            {
                return Ok(None);
            }
            return Err(error.into());
        }
        super::lookup_bound(
            &self.binding,
            self.artifact_identity(),
            self.context(),
            target,
            cache,
            limits,
            backing,
            usage,
            |record| self.validate_completed_record(record, limits),
        )
    }

    /// Reduce and restore homogeneity within one native call. The conversion
    /// consumes the just-validated result before any mutable external exposure,
    /// so a warm hit does not traverse its coefficient payload a second time.
    /// Arbitrary external records must use `homogeneous_decomposition` instead.
    pub fn reduce_guarded_root_with_homogeneity(
        &self,
        target: &IntegralKey,
        cache: &mut GuardedRootCache,
        limits: ReductionLimits,
        backing: Option<&mut dyn GuardedCacheBacking>,
    ) -> Result<(GuardedRootResult, HomogeneousMasterDecomposition), GuardedCacheError> {
        self.require_homogeneity()?;
        let result = self.reduce_guarded_root(target, cache, limits, backing)?;
        let unit = MasterDecomposition::new(
            self.artifact.family_fingerprint_owner(),
            target.clone(),
            result.record.terms.clone(),
        );
        let homogeneous = self.homogeneous_from_unit(unit)?;
        Ok((result, homogeneous))
    }

    pub fn decomposition(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<MasterDecomposition, GuardedCacheError> {
        self.validate_completed_record(record, limits)?;
        let target = IntegralKey::try_new(record.key.powers().iter().copied())
            .map_err(ReductionError::from)?;
        Ok(MasterDecomposition::new(
            self.artifact.family_fingerprint_owner(),
            target,
            record.terms.clone(),
        ))
    }

    /// Reuse the native common-mass exponent helper; callers materialize their
    /// own mass symbol. The persisted/cache payload remains raw unit mass.
    pub fn homogeneous_decomposition(
        &self,
        record: &ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<HomogeneousMasterDecomposition, GuardedCacheError> {
        self.require_homogeneity()?;
        self.homogeneous_from_unit(self.decomposition(record, limits)?)
    }

    fn require_homogeneity(&self) -> Result<(), ReductionError> {
        if self.artifact.common_mass_homogeneity()
            != Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
        {
            return Err(ReductionError::MissingCommonMassHomogeneityProof);
        }
        Ok(())
    }

    fn homogeneous_from_unit(
        &self,
        unit: MasterDecomposition,
    ) -> Result<HomogeneousMasterDecomposition, GuardedCacheError> {
        let target = unit.target().clone();
        let mut terms = std::collections::BTreeMap::new();
        for (master, coefficient) in unit.into_terms() {
            let exponent = super::super::reducer::homogeneity_exponent(&target, &master)?;
            terms.insert(
                master,
                HomogeneousMasterCoefficient::new(coefficient, exponent),
            );
        }
        Ok(HomogeneousMasterDecomposition::new(
            self.artifact.family_fingerprint_owner(),
            target,
            terms,
        ))
    }
}
