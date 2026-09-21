//! Monotonic full-key/phase membership, separate from FIFO admission.
//!
//! A probe holds only one shard and releases it before the scheduler lock is
//! acquired. Mutation requires the scheduler lock FIRST, then one shard; the
//! caller commits the existing global budgets, count and queue in that scope.
//! No pending records, eviction, queue partitioning or hash-only identities.

use super::{IntegralKey, Work};
use std::collections::{HashMap, hash_map::RandomState};
use std::hash::BuildHasher;
use std::sync::{Mutex, MutexGuard};

const SHARDS: usize = 64;

#[derive(Default)]
struct SeenPhases<const N: usize> {
    route: bool,
    apply_owner: Option<[bool; N]>,
}

impl<const N: usize> SeenPhases<N> {
    fn contains(&self, node: &Work<N>) -> bool {
        match node {
            Work::Route(_) => self.route,
            Work::Apply { owner_sector, .. } => self.apply_owner.as_ref() == Some(owner_sector),
        }
    }

    fn insert(&mut self, node: &Work<N>) {
        match node {
            Work::Route(_) => self.route = true,
            Work::Apply { owner_sector, .. } => self.apply_owner = Some(*owner_sector),
        }
    }
}

pub(super) struct Membership<const N: usize, H = RandomState> {
    shards: [Mutex<HashMap<IntegralKey, SeenPhases<N>, H>>; SHARDS],
    hasher: H,
}

pub(super) struct Shard<'a, const N: usize, H> {
    entries: MutexGuard<'a, HashMap<IntegralKey, SeenPhases<N>, H>>,
}

impl<const N: usize> Membership<N> {
    pub(super) fn new() -> Self {
        // Independent selectors/table seeds avoid pinning the same hash
        // bucket bits in every key that lands in one membership shard.
        Self::with_hashers(RandomState::new(), RandomState::new)
    }
}

impl<const N: usize, H: BuildHasher> Membership<N, H> {
    // The generic hasher also permits forced-collision tests of full equality.
    fn with_hashers(hasher: H, mut table_hasher: impl FnMut() -> H) -> Self {
        Self {
            shards: std::array::from_fn(|_| Mutex::new(HashMap::with_hasher(table_hasher()))),
            hasher,
        }
    }

    pub(super) fn shard_index(&self, node: &Work<N>) -> usize {
        self.hasher.hash_one(node.target()) as usize & (SHARDS - 1)
    }

    /// A false result is only a hint: another publisher may commit afterwards.
    /// A true result stays true for this immutable campaign's lifetime.
    pub(super) fn probe(&self, node: &Work<N>) -> (usize, bool) {
        let index = self.shard_index(node);
        let present = self.lock_shard(index).contains(node);
        (index, present)
    }

    pub(super) fn lock_shard(&self, index: usize) -> Shard<'_, N, H> {
        Shard {
            entries: self.shards[index]
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        }
    }
}

impl<const N: usize, H: BuildHasher> Shard<'_, N, H> {
    pub(super) fn contains(&self, node: &Work<N>) -> bool {
        self.entries
            .get(node.target())
            .is_some_and(|phases| phases.contains(node))
    }

    /// Called only after a missing-phase recheck and global budget admission.
    /// Returns whether this is a new physical key, not merely a new phase.
    pub(super) fn insert(&mut self, node: &Work<N>) -> bool {
        if let Some(phases) = self.entries.get_mut(node.target()) {
            phases.insert(node);
            false
        } else {
            let mut phases = SeenPhases::default();
            phases.insert(node);
            self.entries.insert(node.target().clone(), phases);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{BuildHasherDefault, Hasher};

    #[derive(Default)]
    struct CollisionHasher;
    impl Hasher for CollisionHasher {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, _: &[u8]) {}
    }

    #[test]
    fn membership_preprobe_full_hash_collisions_keep_keys_and_phases_distinct() {
        let membership = Membership::<2, _>::with_hashers(
            BuildHasherDefault::<CollisionHasher>::default(),
            BuildHasherDefault::<CollisionHasher>::default,
        );
        let a = Work::Route(IntegralKey::try_new([1, 0]).unwrap());
        let b = Work::Route(IntegralKey::try_new([2, 0]).unwrap());
        let apply = Work::Apply {
            owner_sector: [true, false],
            target: a.target().clone(),
        };
        assert_eq!(membership.probe(&a), (0, false));
        assert_eq!(membership.probe(&b), (0, false));
        {
            let mut shard = membership.lock_shard(0);
            assert!(shard.insert(&a));
            assert!(!shard.contains(&b));
            assert!(!shard.contains(&apply));
            assert!(shard.insert(&b));
            assert!(!shard.insert(&apply));
        }
        assert!(membership.probe(&a).1);
        assert!(membership.probe(&b).1);
        assert!(membership.probe(&apply).1);
        assert!(
            !membership
                .probe(&Work::Apply {
                    owner_sector: [false, true],
                    target: a.target().clone(),
                })
                .1
        );
    }

    #[test]
    fn membership_preprobe_table_hasher_factory_is_independent_of_selector() {
        struct SeededBuilder(u64);
        struct SeededHasher(u64);
        impl Hasher for SeededHasher {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, _: &[u8]) {}
        }
        impl BuildHasher for SeededBuilder {
            type Hasher = SeededHasher;
            fn build_hasher(&self) -> SeededHasher {
                SeededHasher(self.0)
            }
        }
        let mut next_seed = 0;
        let membership = Membership::<1, _>::with_hashers(SeededBuilder(100), || {
            next_seed += 1;
            SeededBuilder(next_seed)
        });
        assert_eq!(next_seed, SHARDS as u64);
        let node = Work::Route(IntegralKey::try_new([1]).unwrap());
        assert_eq!(membership.shard_index(&node), 100 & (SHARDS - 1));
        for (index, shard) in membership.shards.iter().enumerate() {
            assert_eq!(
                shard.lock().unwrap().hasher().hash_one(node.target()),
                index as u64 + 1
            );
        }
    }
}
