//! Thin closed-program steering. Native admission, guarded algebra, RAM
//! accounting, eviction and mass homogeneity remain in rustred-core.
use std::path::Path;

use rustred::{
    algebra::CoefficientContext,
    family::IntegralKey,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    },
    reduction::{
        ClosedUnitMassProgram, CompletedCachePolicy, GuardedCacheBacking, GuardedCacheBackingError,
        GuardedCacheBackingErrorKind, GuardedCacheError, GuardedRootCache,
        GuardedRootCacheStatistics, GuardedRootExportError, GuardedRootExportStatistics,
        GuardedRootResult, HomogeneousMasterDecomposition, ReductionError, ReductionLimits,
    },
};
use serde_json::{Value, json};

use super::{CachePreparationBackend, CachePreparationResolvedOptions, CachePreparationTarget};
use crate::{
    AppError, AppErrorKind, SavedApplicationCacheBundle, SavedApplicationCachePack,
    SavedApplicationCachePackLimits, SavedApplicationCachePackStatistics,
};

struct ReadOnlyBacking(SavedApplicationCachePack);
impl GuardedCacheBacking for ReadOnlyBacking {
    fn binding(&self) -> &ApplicationCacheBinding {
        self.0.binding()
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        self.0.lookup(key, context).map_err(backing_error)
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
pub struct ClosedApplicationCache<'a> {
    program: ClosedUnitMassProgram<'a>,
    cache: GuardedRootCache,
    limits: ReductionLimits,
    backing: Option<ReadOnlyBacking>,
}
impl std::fmt::Debug for ClosedApplicationCache<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosedApplicationCache")
            .field("binding", self.program.binding())
            .field("limits", &self.limits)
            .field("cache", &self.cache.statistics())
            .field("has_read_only_backing", &self.backing.is_some())
            .finish()
    }
}
impl<'a> ClosedApplicationCache<'a> {
    pub fn new(
        program: ClosedUnitMassProgram<'a>,
        limits: ReductionLimits,
        policy: CompletedCachePolicy,
    ) -> Self {
        let cache = GuardedRootCache::with_policy(&program, limits, policy);
        Self {
            program,
            cache,
            limits,
            backing: None,
        }
    }
    pub fn program(&self) -> &ClosedUnitMassProgram<'a> {
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
        self.backing.as_ref().map(|backing| backing.0.statistics())
    }
    /// Attach a trusted-generated pack for exactly this program without
    /// changing its already configured RAM or native arithmetic/work policy.
    pub fn attach_pack(&mut self, pack: SavedApplicationCachePack) -> Result<(), AppError> {
        if pack.binding() != self.program.binding() {
            return Err(AppError::input(
                "closed cache pack differs from active program",
            ));
        }
        self.backing = Some(ReadOnlyBacking(pack));
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
            backing.0.refresh()?;
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
            .reduce_guarded_root(target, &mut self.cache, self.limits, backing)
            .map_err(guarded_error)
    }
    pub fn homogeneous_decomposition(
        &self,
        record: &ApplicationCacheRecord,
    ) -> Result<HomogeneousMasterDecomposition, AppError> {
        self.program
            .homogeneous_decomposition(record, self.limits)
            .map_err(guarded_error)
    }
    /// Native combined entry preserves the validated-result boundary, without
    /// reinterpreting the returned record as arbitrary external input.
    pub fn reduce_with_common_mass_homogeneity(
        &mut self,
        target: &IntegralKey,
    ) -> Result<HomogeneousMasterDecomposition, AppError> {
        let backing = self
            .backing
            .as_mut()
            .map(|backing| backing as &mut dyn GuardedCacheBacking);
        self.program
            .reduce_guarded_root_with_homogeneity(target, &mut self.cache, self.limits, backing)
            .map(|(_, homogeneous)| homogeneous)
            .map_err(guarded_error)
    }
    pub fn clear_ram(&mut self) -> Result<(), AppError> {
        self.cache.clear().map_err(reduction_error)
    }
    pub(super) fn export_to(
        &self,
        backing: &mut dyn GuardedCacheBacking,
    ) -> Result<GuardedRootExportStatistics, GuardedRootExportError> {
        self.program.export_completed(&self.cache, backing)
    }
}

/// Native preparation implementation for one or more already-authenticated
/// sealed programs. The caller selects finite roots; no rules are generated.
pub struct ClosedCachePreparationBackend<'a> {
    bindings: Vec<ApplicationCacheBinding>,
    sessions: Vec<ClosedApplicationCache<'a>>,
    native_limits: ReductionLimits,
}
impl<'a> ClosedCachePreparationBackend<'a> {
    pub fn new(
        programs: Vec<ClosedUnitMassProgram<'a>>,
        native_limits: ReductionLimits,
    ) -> Result<Self, AppError> {
        if programs.is_empty() {
            return Err(AppError::input("closed cache preparation needs a program"));
        }
        let bindings = programs
            .iter()
            .map(|program| program.binding().clone())
            .collect();
        let sessions = programs
            .into_iter()
            .map(|program| {
                ClosedApplicationCache::new(
                    program,
                    native_limits,
                    CompletedCachePolicy::EvictLeastRecentlyUsed,
                )
            })
            .collect();
        Ok(Self {
            bindings,
            sessions,
            native_limits,
        })
    }
    fn session(&self, namespace: usize) -> Result<&ClosedApplicationCache<'a>, AppError> {
        self.sessions
            .get(namespace)
            .ok_or_else(|| AppError::input("unknown closed cache namespace"))
    }
}
impl CachePreparationBackend for ClosedCachePreparationBackend<'_> {
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
            .ok_or_else(|| AppError::input("unknown closed cache namespace"))?
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
            .ok_or_else(|| AppError::limit("closed backing directory byte count overflow"))
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
            session.cache = GuardedRootCache::with_policy(
                &session.program,
                session.limits,
                options.cache_policy,
            );
        }
        Ok(())
    }
    fn validate_entry(&self, target: &CachePreparationTarget) -> Result<(), AppError> {
        self.session(target.namespace())?
            .program
            .validate_entry(target.integral())
            .map_err(reduction_error)
    }
    fn validate_completed_record(
        &self,
        namespace: usize,
        record: &ApplicationCacheRecord,
    ) -> Result<(), AppError> {
        let session = self.session(namespace)?;
        session
            .program
            .validate_completed_record(record, session.limits)
            .map_err(guarded_error)
    }
    fn reduce_guarded_root(
        &mut self,
        target: &CachePreparationTarget,
    ) -> Result<ApplicationCacheRecord, AppError> {
        self.sessions
            .get_mut(target.namespace())
            .ok_or_else(|| AppError::input("unknown closed cache namespace"))?
            .reduce_guarded_root(target.integral())
            .map(|result| result.record)
    }
    fn operational_profile(&self) -> Value {
        json!({"backend":"native-closed-guarded-root", "ram_partition":"equal-per-namespace",
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
                    "lookup_limit_refusals":io.lookup_limit_refusals})),
                "rule_applications":stats.work.rule_applications(), "coalescing_additions":stats.work.coalescing_additions()})
        }).collect::<Vec<_>>())
    }
}

fn reduction_error(error: ReductionError) -> AppError {
    crate::application::closing::map_reduction_error("guarded closed cache", error)
}
pub(super) fn backing_error(error: AppError) -> GuardedCacheBackingError {
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
        GuardedCacheError::Backing(error) => AppError::new(
            match error.kind {
                GuardedCacheBackingErrorKind::Invalid => AppErrorKind::Input,
                GuardedCacheBackingErrorKind::Limit => AppErrorKind::Limit,
                GuardedCacheBackingErrorKind::Io => AppErrorKind::Execution,
            },
            error.to_string(),
        ),
    }
}
