//! Shared closed/candidate program steering. Native admission, guarded
//! algebra, RAM accounting, eviction and homogeneity remain in rustred-core.
use std::path::Path;

use rustred::{
    algebra::CoefficientContext,
    family::IntegralKey,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    },
    reduction::{
        ClosedUnitMassProgram, CompletedCachePolicy, GuardedCacheBacking, GuardedCacheBackingError,
        GuardedCacheBackingErrorKind, GuardedCacheError, GuardedIntermediateCapture,
        GuardedRootCache, GuardedRootCacheStatistics, GuardedRootExportError,
        GuardedRootExportStatistics, GuardedRootResult, HomogeneousMasterDecomposition,
        ReductionError, ReductionLimits,
    },
    solver::CandidateUnitMassProgram,
};
use serde_json::{Value, json};

use super::{
    CachePreparationBackend, CachePreparationResolvedOptions, CachePreparationTarget,
    program::NativeCacheProgram,
};
use crate::{
    AppError, AppErrorKind, SavedApplicationCacheBundle, SavedApplicationCacheLookupOutcome,
    SavedApplicationCachePack, SavedApplicationCachePackLimits,
    SavedApplicationCachePackStatistics,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CacheLookupBudgetPolicy {
    Strict,
    #[default]
    ColdOnLookupWorkExhaustion,
}

struct ReadOnlyBacking {
    pack: SavedApplicationCachePack,
    lookup_policy: CacheLookupBudgetPolicy,
    budget_fallbacks: usize,
}
impl GuardedCacheBacking for ReadOnlyBacking {
    fn binding(&self) -> &ApplicationCacheBinding {
        self.pack.binding()
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        if self.lookup_policy == CacheLookupBudgetPolicy::Strict {
            return self.pack.lookup(key, context).map_err(backing_error);
        }
        match self
            .pack
            .lookup_outcome(key, context)
            .map_err(backing_error)?
        {
            SavedApplicationCacheLookupOutcome::Hit(record) => Ok(Some(record)),
            SavedApplicationCacheLookupOutcome::Absent => Ok(None),
            SavedApplicationCacheLookupOutcome::WorkBudgetExhausted => {
                self.budget_fallbacks = self.budget_fallbacks.saturating_add(1);
                Ok(None)
            }
        }
    }
    fn store_completed(
        &mut self,
        _: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        // Explicitly read-only attachment: misses may populate RAM, never
        // mutate the user's pack during ordinary evaluation.
        Ok(())
    }
}

/// One borrowed immutable program, explicit native RAM cache and optional
/// read-only pack. Creating the session performs no disk or network I/O.
pub struct NativeApplicationCache<P: NativeCacheProgram> {
    program: P,
    cache: GuardedRootCache,
    limits: ReductionLimits,
    backing: Option<ReadOnlyBacking>,
    lookup_policy: CacheLookupBudgetPolicy,
}
pub type ClosedApplicationCache<'a> = NativeApplicationCache<ClosedUnitMassProgram<'a>>;
pub type CandidateApplicationCache<const N: usize> =
    NativeApplicationCache<CandidateUnitMassProgram<N>>;

impl<P: NativeCacheProgram> std::fmt::Debug for NativeApplicationCache<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeApplicationCache")
            .field("binding", self.program.binding())
            .field("limits", &self.limits)
            .field("cache", &self.cache.statistics())
            .field("has_read_only_backing", &self.backing.is_some())
            .finish()
    }
}
impl<P: NativeCacheProgram> NativeApplicationCache<P> {
    pub fn new(program: P, limits: ReductionLimits, policy: CompletedCachePolicy) -> Self {
        let cache = program.new_cache(limits, policy);
        Self {
            program,
            cache,
            limits,
            backing: None,
            lookup_policy: CacheLookupBudgetPolicy::default(),
        }
    }
    pub fn program(&self) -> &P {
        &self.program
    }
    pub fn limits(&self) -> ReductionLimits {
        self.limits
    }
    pub fn cache_policy(&self) -> CompletedCachePolicy {
        self.cache.policy()
    }
    pub fn cache_statistics(&self) -> GuardedRootCacheStatistics {
        self.cache.statistics()
    }
    pub fn io_statistics(&self) -> Option<SavedApplicationCachePackStatistics> {
        self.backing
            .as_ref()
            .map(|backing| backing.pack.statistics())
    }
    pub fn set_lookup_budget_policy(&mut self, policy: CacheLookupBudgetPolicy) {
        self.lookup_policy = policy;
        if let Some(backing) = self.backing.as_mut() {
            backing.lookup_policy = policy;
        }
    }
    pub fn lookup_budget_policy(&self) -> CacheLookupBudgetPolicy {
        self.lookup_policy
    }
    pub fn backing_budget_fallbacks(&self) -> usize {
        self.backing
            .as_ref()
            .map_or(0, |backing| backing.budget_fallbacks)
    }
    /// Attach a trusted-generated pack for exactly this program without
    /// changing its already configured RAM or native arithmetic/work policy.
    pub fn attach_pack(&mut self, pack: SavedApplicationCachePack) -> Result<(), AppError> {
        if pack.binding() != self.program.binding() {
            return Err(AppError::input(
                "cache pack differs from active native program",
            ));
        }
        self.backing = Some(ReadOnlyBacking {
            pack,
            lookup_policy: self.lookup_policy,
            budget_fallbacks: 0,
        });
        Ok(())
    }
    /// One-program convenience. Multi-program callers open their exact bundle
    /// once and pass each explicitly bound child reader to attach_pack.
    pub fn attach_bundle(
        &mut self,
        path: &Path,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<(), AppError> {
        let bundle = SavedApplicationCacheBundle::open_generated(
            path,
            std::slice::from_ref(self.program.binding()),
            limits,
        )?;
        self.attach_pack(bundle.open_reader(self.program.binding())?)
    }
    fn refresh_backing(&mut self) -> Result<(), AppError> {
        if let Some(backing) = self.backing.as_mut() {
            backing.pack.refresh()?;
        }
        Ok(())
    }
    pub fn reduce_guarded_root(
        &mut self,
        target: &IntegralKey,
    ) -> Result<GuardedRootResult, AppError> {
        let backing = self
            .backing
            .as_mut()
            .map(|backing| backing as &mut dyn GuardedCacheBacking);
        self.program
            .reduce(target, &mut self.cache, self.limits, backing)
    }
    /// Explicit preparation-only capture; ordinary read-only application does
    /// not construct intermediate provenance or write through to disk.
    pub fn reduce_guarded_root_with_capture(
        &mut self,
        target: &IntegralKey,
        capture: &mut dyn GuardedIntermediateCapture,
    ) -> Result<GuardedRootResult, AppError> {
        let backing = self
            .backing
            .as_mut()
            .map(|backing| backing as &mut dyn GuardedCacheBacking);
        self.program
            .reduce_with_capture(target, &mut self.cache, self.limits, backing, capture)
    }
    pub fn decomposition(
        &self,
        record: &ApplicationCacheRecord,
    ) -> Result<P::Decomposition, AppError> {
        self.program.decomposition(record, self.limits)
    }
    /// Native combined entry preserves the validated-result boundary, without
    /// reinterpreting the returned record as arbitrary external input.
    pub fn reduce_decomposition(
        &mut self,
        target: &IntegralKey,
    ) -> Result<P::Decomposition, AppError> {
        self.reduce_guarded_root_with_decomposition(target)
            .map(|(_, decomposition)| decomposition)
    }
    pub fn reduce_guarded_root_with_decomposition(
        &mut self,
        target: &IntegralKey,
    ) -> Result<(GuardedRootResult, P::Decomposition), AppError> {
        let backing = self
            .backing
            .as_mut()
            .map(|backing| backing as &mut dyn GuardedCacheBacking);
        self.program
            .reduce_decomposition(target, &mut self.cache, self.limits, backing)
    }
    pub fn clear_ram(&mut self) -> Result<(), AppError> {
        self.cache.clear().map_err(reduction_error)
    }
    pub(super) fn export_to(
        &self,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<GuardedRootExportStatistics, GuardedRootExportError> {
        self.program.export(&self.cache, backing)
    }
    pub(super) fn backing_pack(&self) -> Option<&SavedApplicationCachePack> {
        self.backing.as_ref().map(|backing| &backing.pack)
    }
}

impl ClosedApplicationCache<'_> {
    pub fn homogeneous_decomposition(
        &self,
        record: &ApplicationCacheRecord,
    ) -> Result<HomogeneousMasterDecomposition, AppError> {
        self.decomposition(record)
    }
    pub fn reduce_with_common_mass_homogeneity(
        &mut self,
        target: &IntegralKey,
    ) -> Result<HomogeneousMasterDecomposition, AppError> {
        self.reduce_decomposition(target)
    }
}

/// Native preparation implementation for one or more already-authenticated
/// native programs. The caller selects finite roots; no rules are generated.
/// One aggregate coefficient-byte allowance is partitioned across namespaces;
/// the other explicitly configured native work ceilings apply per namespace.
pub struct NativeCachePreparationBackend<P: NativeCacheProgram> {
    bindings: Vec<ApplicationCacheBinding>,
    sessions: Vec<NativeApplicationCache<P>>,
    native_limits: ReductionLimits,
}
impl<P: NativeCacheProgram> std::fmt::Debug for NativeCachePreparationBackend<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeCachePreparationBackend")
            .field("sessions", &self.sessions)
            .field("native_limits", &self.native_limits)
            .finish()
    }
}
pub type ClosedCachePreparationBackend<'a> =
    NativeCachePreparationBackend<ClosedUnitMassProgram<'a>>;
pub type CandidateCachePreparationBackend<const N: usize> =
    NativeCachePreparationBackend<CandidateUnitMassProgram<N>>;

impl<P: NativeCacheProgram> NativeCachePreparationBackend<P> {
    pub fn new(programs: Vec<P>, native_limits: ReductionLimits) -> Result<Self, AppError> {
        Self::with_policy(
            programs,
            native_limits,
            CompletedCachePolicy::EvictLeastRecentlyUsed,
        )
    }
    pub fn with_policy(
        programs: Vec<P>,
        native_limits: ReductionLimits,
        policy: CompletedCachePolicy,
    ) -> Result<Self, AppError> {
        if programs.is_empty() {
            return Err(AppError::input("native cache preparation needs a program"));
        }
        let per_namespace = native_limits.max_cached_coefficient_bytes / programs.len();
        if per_namespace == 0 {
            return Err(AppError::limit(
                "RAM budget cannot support every cache namespace",
            ));
        }
        let bindings = programs
            .iter()
            .map(|program| program.binding().clone())
            .collect();
        let sessions = programs
            .into_iter()
            .map(|program| {
                NativeApplicationCache::new(
                    program,
                    ReductionLimits {
                        max_cached_coefficient_bytes: per_namespace,
                        ..native_limits
                    },
                    policy,
                )
            })
            .collect();
        Ok(Self {
            bindings,
            sessions,
            native_limits,
        })
    }
    pub fn session(&self, namespace: usize) -> Result<&NativeApplicationCache<P>, AppError> {
        self.sessions
            .get(namespace)
            .ok_or_else(|| AppError::input("unknown native cache namespace"))
    }
    pub fn session_mut(
        &mut self,
        namespace: usize,
    ) -> Result<&mut NativeApplicationCache<P>, AppError> {
        self.sessions
            .get_mut(namespace)
            .ok_or_else(|| AppError::input("unknown native cache namespace"))
    }
    pub fn sessions(&self) -> &[NativeApplicationCache<P>] {
        &self.sessions
    }
    pub fn native_limits(&self) -> ReductionLimits {
        self.native_limits
    }
    pub fn cache_policy(&self) -> CompletedCachePolicy {
        self.sessions[0].cache_policy()
    }
    pub fn set_lookup_budget_policy(&mut self, policy: CacheLookupBudgetPolicy) {
        for session in &mut self.sessions {
            session.set_lookup_budget_policy(policy);
        }
    }
    pub fn lookup_budget_policy(&self) -> CacheLookupBudgetPolicy {
        self.sessions[0].lookup_budget_policy()
    }
    pub fn clear_ram(&mut self) -> Result<(), AppError> {
        for session in &mut self.sessions {
            session.clear_ram()?;
        }
        Ok(())
    }
    /// Exact all-namespace attachment. Binding failures occur before any
    /// session replacement; existing RAM and native policy are not changed.
    pub fn attach_bundle(
        &mut self,
        path: &Path,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<(), AppError> {
        let bundle = SavedApplicationCacheBundle::open_generated(path, &self.bindings, limits)?;
        let readers = self
            .bindings
            .iter()
            .map(|binding| bundle.open_reader(binding))
            .collect::<Result<Vec<_>, _>>()?;
        for (session, reader) in self.sessions.iter_mut().zip(readers) {
            session.attach_pack(reader)?;
        }
        Ok(())
    }
}
impl<P: NativeCacheProgram> CachePreparationBackend for NativeCachePreparationBackend<P> {
    fn bindings(&self) -> &[ApplicationCacheBinding] {
        &self.bindings
    }
    fn context(&self, namespace: usize) -> Result<&CoefficientContext, AppError> {
        Ok(self.session(namespace)?.program.context())
    }
    fn attach_completed_cache(
        &mut self,
        namespace: usize,
        reader: SavedApplicationCachePack,
    ) -> Result<(), AppError> {
        self.sessions
            .get_mut(namespace)
            .ok_or_else(|| AppError::input("unknown native cache namespace"))?
            .attach_pack(reader)
    }
    fn refresh_completed_caches(&mut self) -> Result<(), AppError> {
        for session in &mut self.sessions {
            session.refresh_backing()?;
        }
        Ok(())
    }
    fn completed_cache_directory_bytes(&self) -> Result<usize, AppError> {
        self.sessions
            .iter()
            .filter_map(|session| session.io_statistics())
            .try_fold(0usize, |bytes, statistics| {
                bytes.checked_add(statistics.directory_owned_bytes)
            })
            .ok_or_else(|| AppError::limit("native backing directory byte count overflow"))
    }
    fn configure(&mut self, options: &CachePreparationResolvedOptions) -> Result<(), AppError> {
        // A conservative documented partition bounds the aggregate across
        // independently owned native root caches. No hidden per-family copy
        // of the full aggregate RAM allowance is created.
        let per_namespace = options.retained_coefficient_bytes / self.sessions.len();
        if per_namespace == 0 {
            return Err(AppError::limit(
                "RAM budget cannot support every cache namespace",
            ));
        }
        for session in &mut self.sessions {
            session.limits = ReductionLimits {
                max_cached_coefficient_bytes: per_namespace,
                ..self.native_limits
            };
            session.cache = session
                .program
                .new_cache(session.limits, options.cache_policy);
        }
        self.native_limits.max_cached_coefficient_bytes = options.retained_coefficient_bytes;
        Ok(())
    }
    fn validate_entry(&self, target: &CachePreparationTarget) -> Result<(), AppError> {
        let session = self.session(target.namespace())?;
        session
            .program
            .validate_entry(target.integral(), session.limits)
    }
    fn validate_completed_record(
        &self,
        namespace: usize,
        record: &ApplicationCacheRecord,
    ) -> Result<(), AppError> {
        let session = self.session(namespace)?;
        session.program.validate_record(record, session.limits)
    }
    fn reduce_guarded_root(
        &mut self,
        target: &CachePreparationTarget,
    ) -> Result<ApplicationCacheRecord, AppError> {
        self.sessions
            .get_mut(target.namespace())
            .ok_or_else(|| AppError::input("unknown native cache namespace"))?
            .reduce_guarded_root(target.integral())
            .map(|result| result.record)
    }
    fn reduce_guarded_root_with_capture(
        &mut self,
        target: &CachePreparationTarget,
        capture: &mut dyn GuardedIntermediateCapture,
    ) -> Result<ApplicationCacheRecord, AppError> {
        self.session_mut(target.namespace())?
            .reduce_guarded_root_with_capture(target.integral(), capture)
            .map(|result| result.record)
    }
    fn operational_profile(&self) -> Value {
        json!({"backend":self.sessions[0].program.kind(), "ram_partition":"equal-per-namespace",
            "native_limits":self.sessions.iter().map(|session| format!("{:?}", session.limits)).collect::<Vec<_>>()})
    }
    fn statistics(&self) -> Value {
        json!(self.sessions.iter().map(|session| {
            let stats = session.cache_statistics();
            json!({"ram_hits":stats.ram_hits, "backing_hits":stats.backing_hits,
                "descendant_ram_hits":stats.descendant_ram_hits, "descendant_backing_hits":stats.descendant_backing_hits,
                "payload_validations":stats.payload_validations,
                "completed_reductions":stats.completed_reductions, "evicted_entries":stats.evicted_entries,
                "cached_integrals":stats.cached_integrals, "cached_coefficient_terms":stats.cached_coefficient_terms,
                "cached_coefficient_bytes":stats.cached_coefficient_bytes,
                "backing_io":session.io_statistics().map(|io| json!({
                    "directory_owned_bytes":io.directory_owned_bytes,
                    "chunks_read":io.chunks_read, "encoded_bytes_read":io.chunk_bytes_read,
                    "decoded_bytes":io.decoded_chunk_bytes, "hits":io.hits, "misses":io.misses,
                    "lookup_limit_refusals":io.lookup_limit_refusals,
                    "key_index_skips":io.key_index_skips})),
                "backing_budget_fallbacks":session.backing_budget_fallbacks(),
                "rule_applications":stats.work.rule_applications(), "coalescing_additions":stats.work.coalescing_additions()})
        }).collect::<Vec<_>>())
    }
}

pub(super) fn reduction_error(error: ReductionError) -> AppError {
    if let ReductionError::Resources(error) = error {
        return AppError::resource(error);
    }
    crate::application::closing::map_reduction_error("guarded native cache", error)
}
pub(super) fn backing_error(error: AppError) -> GuardedCacheBackingError {
    if let Some(resource) = error.resource_limit_error() {
        return GuardedCacheBackingError::resource(resource.clone());
    }
    GuardedCacheBackingError {
        kind: match error.kind() {
            AppErrorKind::Limit | AppErrorKind::OutputLimit => GuardedCacheBackingErrorKind::Limit,
            AppErrorKind::Input | AppErrorKind::Schema | AppErrorKind::Serialization => {
                GuardedCacheBackingErrorKind::Invalid
            }
            _ => GuardedCacheBackingErrorKind::Io,
        },
        message: error.to_string(),
    }
}
pub(super) fn guarded_error(error: GuardedCacheError) -> AppError {
    match error {
        GuardedCacheError::Reduction(error) => reduction_error(error),
        GuardedCacheError::BindingMismatch | GuardedCacheError::InvalidRecord(_) => {
            AppError::input(error.to_string())
        }
        GuardedCacheError::Backing(error) => {
            if let GuardedCacheBackingErrorKind::Resource(resource) = &error.kind {
                return AppError::resource(resource.clone());
            }
            AppError::new(
                match &error.kind {
                    GuardedCacheBackingErrorKind::Invalid => AppErrorKind::Input,
                    GuardedCacheBackingErrorKind::Limit => AppErrorKind::Limit,
                    GuardedCacheBackingErrorKind::Io => AppErrorKind::Execution,
                    GuardedCacheBackingErrorKind::Resource(_) => unreachable!(),
                },
                error.to_string(),
            )
        }
    }
}
