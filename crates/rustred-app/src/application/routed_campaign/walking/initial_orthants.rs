//! Optional immutable proof of scheduling reuse, never completed coverage.
//! Built once from actual initial admissions in this execution's queue; later
//! admissions and other program epochs can neither mutate nor seed this index.
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use super::queue::{Domain, Phase};

pub(super) const MAX_BUCKETS: usize = 4096;
pub(super) const MAX_ENTRY_BYTES: usize = 2 * 1024 * 1024;

pub(super) struct InitialOrthants<const N: usize> {
    ranks: HashMap<(Phase, [bool; N]), Option<u32>>,
}
impl<const N: usize> InitialOrthants<N> {
    pub fn empty() -> Self {
        Self {
            ranks: HashMap::new(),
        }
    }
    pub fn from_initial(domains: &[Arc<Domain<N>>], cancellation: &AtomicBool) -> Self {
        Self::with_limits(domains, cancellation, MAX_BUCKETS, MAX_ENTRY_BYTES)
    }
    fn with_limits(
        domains: &[Arc<Domain<N>>],
        cancellation: &AtomicBool,
        max_buckets: usize,
        max_bytes: usize,
    ) -> Self {
        let mut snapshot = Self::empty();
        let max_buckets =
            max_buckets.min(max_bytes / size_of::<((Phase, [bool; N]), Option<u32>)>());
        if max_buckets == 0 {
            return snapshot;
        }
        for domain in domains {
            if cancellation.load(Ordering::Acquire) {
                break;
            }
            if !domain.is_full_orthant() {
                continue;
            }
            let key = (domain.phase, domain.owner);
            if let Some(rank) = snapshot.ranks.get_mut(&key) {
                if domain.rank.is_none_or(|r| rank.is_some_and(|old| old <= r)) {
                    *rank = domain.rank;
                }
            } else {
                // An incomplete optional index is safe: misses use the normal
                // cache/queue path. Do not fail a campaign for this optimization.
                if snapshot.ranks.len() == max_buckets || snapshot.ranks.try_reserve(1).is_err() {
                    break;
                }
                snapshot.ranks.insert(key, domain.rank);
            }
        }
        snapshot
    }
    /// Native callers already validated the endpoint's local coordinates and
    /// actual rank. A full orthant contains every valid box of this phase/mask
    /// with no larger rank scope; finite u32::MAX is NOT an unbounded rank.
    pub fn contains(&self, phase: Phase, owner: &[bool; N], rank: Option<u32>) -> bool {
        self.ranks
            .get(&(phase, *owner))
            .is_some_and(|bound| bound.is_none_or(|r| rank.is_some_and(|s| s <= r)))
    }
}

// MAX_ENTRY_BYTES bounds logical key/value payload only, not HashMap spare
// capacity/allocator overhead, native buffers, or process RSS. No coordinates,
// native coefficients, query outputs or program handles are copied here.
#[cfg(test)]
mod tests;
