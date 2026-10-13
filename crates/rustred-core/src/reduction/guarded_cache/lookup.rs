//! Shared storage mechanics only. Each mathematical program must check its
//! current entry authority before calling this helper.
use super::{GuardedCacheBacking, GuardedCacheError, GuardedRootCache, GuardedRootSource, RootUse};
use crate::algebra::CoefficientContext;
use crate::family::IntegralKey;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use crate::reduction::{ReductionError, ReductionLimits};
use std::sync::Arc;

pub(crate) fn lookup_bound<E>(
    binding: &ApplicationCacheBinding,
    identity: usize,
    context: &CoefficientContext,
    target: &IntegralKey,
    cache: &mut GuardedRootCache,
    limits: ReductionLimits,
    backing: &mut Option<&mut dyn GuardedCacheBacking>,
    usage: RootUse,
    validate: impl Fn(&ApplicationCacheRecord) -> Result<(), E>,
) -> Result<Option<(Arc<ApplicationCacheRecord>, GuardedRootSource)>, E>
where
    E: From<GuardedCacheError> + From<ReductionError>,
{
    if &cache.binding != binding
        || cache.artifact_identity != identity
        || backing
            .as_ref()
            .is_some_and(|store| store.binding() != binding)
    {
        return Err(GuardedCacheError::BindingMismatch.into());
    }
    // Individually oversized hits fail before useful unrelated roots are
    // evicted merely to satisfy a tighter aggregate allowance.
    cache.check_hit_weight(target, limits)?;
    cache.check_limits(limits)?;
    if let Some(record) = cache.hit(target, limits, usage, &validate)? {
        return Ok(Some((record, GuardedRootSource::Ram)));
    }
    let key = ApplicationCacheKey::guarded_root(target);
    if let Some(store) = backing.as_mut() {
        if let Some(record) = store
            .lookup(&key, context)
            .map_err(GuardedCacheError::from)?
        {
            if record.key != key {
                return Err(GuardedCacheError::InvalidRecord(
                    "backing returned a different exact key",
                )
                .into());
            }
            cache.validate_payload(|| validate(&record))?;
            let record = Arc::new(record);
            cache.insert_shared(target.clone(), Arc::clone(&record), limits)?;
            cache.backing_hit(usage);
            return Ok(Some((record, GuardedRootSource::Backing)));
        }
    }
    Ok(None)
}
