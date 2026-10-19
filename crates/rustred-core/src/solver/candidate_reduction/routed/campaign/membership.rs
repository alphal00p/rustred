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

pub(in super::super) struct Membership<const N: usize, H = RandomState> {
    shards: [Mutex<HashMap<IntegralKey, SeenPhases<N>, H>>; SHARDS],
    hasher: H,
}

pub(super) struct Shard<'a, const N: usize, H> {
    entries: MutexGuard<'a, HashMap<IntegralKey, SeenPhases<N>, H>>,
}

impl<const N: usize> Membership<N> {
    /// Consume only the new completed membership, moving physical keys except
    /// when both Route and Apply require separate wire keys. Reserve and charge
    /// the full exported vector plus both key payloads before beginning moves.
    pub(in super::super) fn into_generated_work(
        self,
        expected: usize,
        limits: super::super::generated::CandidateRoutedDeltaExportLimits,
        check: &mut dyn FnMut() -> Result<
            (),
            crate::solver::candidate_reduction::routed::CandidateRoutedError,
        >,
        progress: Option<&crate::persistence::finite_work::GeneratedWorkProgress>,
    ) -> Result<
        Vec<crate::persistence::finite_work::GeneratedWorkKey>,
        crate::solver::candidate_reduction::routed::CandidateRoutedError,
    > {
        use crate::persistence::finite_work::GeneratedWorkKey as Key;
        use crate::persistence::finite_work::GeneratedWorkProgress as Progress;
        use crate::solver::candidate_reduction::routed::CandidateRoutedError as Error;
        let quota = |resource, requested, limit| Error::ResourceLimit {
            resource,
            requested,
            limit,
        };
        if expected > limits.max_records {
            return Err(quota(
                "completed delta Work export",
                expected,
                limits.max_records,
            ));
        }
        let mut count = 0usize;
        let mut export_bytes = expected
            .checked_mul(std::mem::size_of::<Key>())
            .ok_or_else(|| {
                quota(
                    "completed delta export payload",
                    usize::MAX,
                    limits.max_live_payload_bytes,
                )
            })?;
        let mut stage = Progress::begin(progress, "measure_membership", None, "records");
        let mut measured = 0usize;
        let membership_bytes = self.payload_bytes(&mut || {
            check()?;
            measured += 1;
            stage.advance(measured);
            Ok(())
        })?;
        stage.finish(measured);
        let mut stage = Progress::begin(progress, "account_export", Some(expected), "records");
        for shard in &self.shards {
            let shard = shard
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (key, phases) in shard.iter() {
                check()?;
                let copies = usize::from(phases.route) + usize::from(phases.apply_owner.is_some());
                count = count.checked_add(copies).ok_or_else(|| {
                    quota(
                        "completed delta Work export",
                        usize::MAX,
                        limits.max_records,
                    )
                })?;
                let bytes = key
                    .powers()
                    .len()
                    .checked_mul(std::mem::size_of::<i64>())
                    .and_then(|n| n.checked_mul(copies))
                    .and_then(|n| {
                        n.checked_add(if phases.apply_owner.is_some() {
                            key.powers().len()
                        } else {
                            0
                        })
                    })
                    .ok_or_else(|| {
                        quota(
                            "completed delta export payload",
                            usize::MAX,
                            limits.max_live_payload_bytes,
                        )
                    })?;
                export_bytes = export_bytes.checked_add(bytes).ok_or_else(|| {
                    quota(
                        "completed delta export payload",
                        usize::MAX,
                        limits.max_live_payload_bytes,
                    )
                })?;
                stage.advance(count);
            }
        }
        stage.finish(count);
        if count != expected {
            return Err(Error::InvalidInput(
                "completed delta phase count differs from scheduled Work".into(),
            ));
        }
        let live = export_bytes.checked_add(membership_bytes).ok_or_else(|| {
            quota(
                "completed delta export live payload",
                usize::MAX,
                limits.max_live_payload_bytes,
            )
        })?;
        if live > limits.max_live_payload_bytes {
            return Err(quota(
                "completed delta export live payload",
                live,
                limits.max_live_payload_bytes,
            ));
        }
        let mut result = Vec::new();
        result.try_reserve_exact(expected).map_err(|error| {
            Error::InvalidInput(format!("cannot reserve completed Work export: {error}"))
        })?;
        let mut stage = Progress::begin(progress, "export_keys", Some(expected), "records");
        for shard in self.shards {
            for (key, phases) in shard
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
            {
                check()?;
                let powers = key.into_powers();
                if let Some(owner) = phases.apply_owner {
                    if phases.route {
                        result.push(Key::Route(powers.clone()));
                    }
                    result.push(Key::Apply {
                        owner: owner[..powers.len()].to_vec(),
                        powers,
                    });
                } else if phases.route {
                    result.push(Key::Route(powers));
                }
                stage.advance(result.len());
            }
        }
        stage.finish(result.len());
        // In-place unstable sort has no second O(n) scratch vector. Keys are
        // unique by phase/owner, so stability carries no semantic information.
        check()?;
        if progress.is_some() {
            let mut comparisons = 0usize;
            let mut stage = Progress::begin(progress, "sort_keys", None, "comparisons");
            result.sort_unstable_by(|a, b| {
                comparisons = comparisons.saturating_add(1);
                stage.advance(comparisons);
                a.cmp(b)
            });
            stage.finish(comparisons);
        } else {
            result.sort_unstable();
        }
        check()?;
        let mut stage = Progress::begin(
            progress,
            "validate_sorted_keys",
            Some(result.len().saturating_sub(1)),
            "pairs",
        );
        let mut invalid = result.len() != expected;
        for (i, pair) in result.windows(2).enumerate() {
            if pair[0] >= pair[1] {
                invalid = true;
                break;
            }
            stage.advance(i + 1);
        }
        if invalid {
            return Err(Error::InvalidInput(
                "completed Work export is not unique".into(),
            ));
        }
        stage.finish(result.len().saturating_sub(1));
        Ok(result)
    }

    pub(in super::super) fn new() -> Self {
        // Independent selectors/table seeds avoid pinning the same hash
        // bucket bits in every key that lands in one membership shard.
        Self::with_hashers(RandomState::new(), RandomState::new)
    }

    /// Reserve before a completed-generation commit. No key or phase is moved
    /// until every shard has passed validation and fallible allocation.
    pub(in super::super) fn reserve_completed(
        &mut self,
        delta: &Self,
        check: &mut dyn FnMut() -> Result<
            (),
            crate::solver::candidate_reduction::routed::CandidateRoutedError,
        >,
    ) -> Result<(), crate::solver::candidate_reduction::routed::CandidateRoutedError> {
        use crate::solver::candidate_reduction::routed::CandidateRoutedError;
        let mut additions = [0usize; SHARDS];
        // Delta and baseline selectors have different seeds. Determine the
        // destination from the baseline selector, never the delta shard index.
        for source in &delta.shards {
            let source = source
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (key, phases) in source.iter() {
                check()?;
                let index = self.hasher.hash_one(key) as usize & (SHARDS - 1);
                let target = self.shards[index]
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(old) = target.get(key) {
                    if (old.route && phases.route)
                        || old.apply_owner.zip(phases.apply_owner).is_some()
                    {
                        return Err(CandidateRoutedError::InvalidInput(
                            "completed Work delta repeats or conflicts with a baseline phase"
                                .into(),
                        ));
                    }
                } else {
                    additions[index] = additions[index].checked_add(1).ok_or_else(|| {
                        CandidateRoutedError::InvalidInput(
                            "completed membership reservation overflow".into(),
                        )
                    })?;
                }
            }
        }
        for (shard, extra) in self.shards.iter_mut().zip(additions) {
            check()?;
            shard
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .try_reserve(extra)
                .map_err(|error| {
                    CandidateRoutedError::InvalidInput(format!(
                        "cannot reserve completed membership: {error}"
                    ))
                })?;
        }
        check()?;
        Ok(())
    }

    /// Infallible logical commit after reserve_completed; moves physical keys
    /// rather than materializing a second full operational-key census.
    pub(in super::super) fn append_completed(&mut self, delta: Self) {
        for shard in delta.shards {
            for (key, phases) in shard
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
            {
                let index = self.hasher.hash_one(&key) as usize & (SHARDS - 1);
                let target = self.shards[index]
                    .get_mut()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(old) = target.get_mut(&key) {
                    old.route |= phases.route;
                    if let Some(owner) = phases.apply_owner {
                        old.apply_owner = Some(owner);
                    }
                } else {
                    target.insert(key, phases);
                }
            }
        }
    }

    pub(in super::super) fn payload_bytes(
        &self,
        check: &mut dyn FnMut() -> Result<
            (),
            crate::solver::candidate_reduction::routed::CandidateRoutedError,
        >,
    ) -> Result<usize, crate::solver::candidate_reduction::routed::CandidateRoutedError> {
        let mut bytes = 0usize;
        for shard in &self.shards {
            let shard = shard
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for key in shard.keys() {
                check()?;
                bytes = bytes.checked_add(std::mem::size_of::<(IntegralKey, SeenPhases<N>)>())
                    .and_then(|bytes| key.powers().len().checked_mul(std::mem::size_of::<i64>()).and_then(|key| bytes.checked_add(key)))
                    .ok_or_else(|| crate::solver::candidate_reduction::routed::CandidateRoutedError::ResourceLimit {
                        resource: "completed membership payload", requested: usize::MAX, limit: usize::MAX,
                    })?;
            }
        }
        Ok(bytes)
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
    pub(in super::super) fn probe(&self, node: &Work<N>) -> (usize, bool) {
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

    #[test]
    fn completed_membership_merge_preserves_phases_and_refuses_owner_overwrite() {
        let key = IntegralKey::try_new([1, 0]).unwrap();
        let route = Work::Route(key.clone());
        let apply = Work::Apply {
            owner_sector: [true, false],
            target: key.clone(),
        };
        let conflict = Work::Apply {
            owner_sector: [false, true],
            target: key,
        };
        let mut old = Membership::<2>::new();
        old.lock_shard(old.shard_index(&route)).insert(&route);
        let delta = Membership::new();
        delta.lock_shard(delta.shard_index(&apply)).insert(&apply);
        old.reserve_completed(&delta, &mut || Ok(())).unwrap();
        old.append_completed(delta);
        assert!(old.probe(&route).1 && old.probe(&apply).1);
        assert!(!old.probe(&conflict).1);
        let bad = Membership::new();
        bad.lock_shard(bad.shard_index(&conflict)).insert(&conflict);
        assert!(old.reserve_completed(&bad, &mut || Ok(())).is_err());
        assert!(old.probe(&route).1 && old.probe(&apply).1);
        assert!(!old.probe(&conflict).1);
        let duplicate = Membership::new();
        duplicate
            .lock_shard(duplicate.shard_index(&route))
            .insert(&route);
        assert!(old.reserve_completed(&duplicate, &mut || Ok(())).is_err());
        assert!(old.probe(&route).1 && old.probe(&apply).1);
        let equal_apply = Membership::new();
        equal_apply
            .lock_shard(equal_apply.shard_index(&apply))
            .insert(&apply);
        assert!(old.reserve_completed(&equal_apply, &mut || Ok(())).is_err());
        assert!(old.probe(&route).1 && old.probe(&apply).1);
    }

    #[test]
    fn completed_membership_reservation_check_failure_moves_no_keys() {
        use crate::solver::candidate_reduction::routed::CandidateRoutedError;
        let a = Work::Route(IntegralKey::try_new([1, 0]).unwrap());
        let b = Work::Route(IntegralKey::try_new([2, 0]).unwrap());
        let mut old = Membership::<2>::new();
        old.lock_shard(old.shard_index(&a)).insert(&a);
        let delta = Membership::new();
        delta.lock_shard(delta.shard_index(&b)).insert(&b);
        let mut checks = 0;
        let error = old.reserve_completed(&delta, &mut || {
            checks += 1;
            if checks == 2 {
                Err(CandidateRoutedError::InvalidInput(
                    "injected cancellation".into(),
                ))
            } else {
                Ok(())
            }
        });
        assert!(error.is_err());
        assert_eq!(checks, 2);
        assert!(old.probe(&a).1);
        assert!(!old.probe(&b).1);
        assert!(delta.probe(&b).1);
        old.reserve_completed(&delta, &mut || Ok(())).unwrap();
        old.append_completed(delta);
        assert!(old.probe(&a).1 && old.probe(&b).1);
    }
}
