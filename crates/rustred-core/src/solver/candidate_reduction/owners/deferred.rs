//! Runtime-only, once-per-owner preparation over an immutable admitted manifest.
//! Campaign/trace collections retain their ordinary eager representation.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use super::model::{CandidateOwnerContext, CandidateOwnerPrograms, PreparedOwner};
use crate::family::IntegralKey;
use crate::sector::OrderingPolicy;
use crate::solver::candidate_reduction::CandidateReductionError;

/// Structural owner identity admitted before any deferred formula is used.
#[derive(Clone, Debug)]
pub struct DeferredCandidateOwnerDescriptor<const N: usize> {
    pub sector: [bool; N],
    pub saved_root: [bool; N],
    pub ordering: OrderingPolicy,
    /// Baseline terminal authority, including preferred-program deferral.
    pub terminals: BTreeSet<IntegralKey>,
}

/// Loader of exactly one immutable selected owner. Implementations must retain
/// authenticated owned source data, not reread mutable files on first use.
/// Existing owner constructors and overlay admission must remain authoritative.
pub trait DeferredCandidateOwnerLoader<const N: usize>: fmt::Debug + Send + Sync {
    fn prepare(
        &self,
        sector: [bool; N],
        context: Arc<CandidateOwnerContext<N>>,
    ) -> Result<CandidateOwnerPrograms<N>, CandidateReductionError>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeferredCandidateOwnerStatistics {
    pub declared_owners: usize,
    pub prepared_owners: usize,
    pub failed_owners: usize,
    pub preparation_attempts: usize,
    pub preparation_time: Duration,
}

#[derive(Debug)]
struct Slot<const N: usize> {
    descriptor: DeferredCandidateOwnerDescriptor<N>,
    prepared: OnceLock<Result<Arc<PreparedOwner<N>>, CandidateReductionError>>,
}

/// Immutable owner presence with independently synchronized, cached preparation.
/// Loading failure is retained as well as success. Coefficient-cache clearing
/// never clears these owner cells or changes their authenticated identity.
#[derive(Debug)]
pub struct DeferredCandidateOwnerPrograms<const N: usize> {
    context: Arc<CandidateOwnerContext<N>>,
    owners: BTreeMap<[bool; N], Slot<N>>,
    loader: Arc<dyn DeferredCandidateOwnerLoader<N>>,
    attempts: AtomicUsize,
    preparation_nanoseconds: AtomicU64,
}

impl<const N: usize> DeferredCandidateOwnerPrograms<N> {
    pub fn try_new(
        context: Arc<CandidateOwnerContext<N>>,
        descriptors: impl IntoIterator<Item = DeferredCandidateOwnerDescriptor<N>>,
        loader: Arc<dyn DeferredCandidateOwnerLoader<N>>,
    ) -> Result<Self, CandidateReductionError> {
        let arity = context.family().denominator_count();
        let mut owners = BTreeMap::new();
        for descriptor in descriptors {
            if descriptor
                .sector
                .iter()
                .zip(descriptor.saved_root)
                .any(|(&active, root)| active && !root)
                || descriptor.sector.iter().skip(arity).any(|&bit| bit)
                || descriptor.saved_root.iter().skip(arity).any(|&bit| bit)
            {
                return Err(invalid("deferred owner/root support is inconsistent"));
            }
            descriptor
                .ordering
                .compare(&vec![0; arity], &vec![0; arity])
                .map_err(crate::reduction::ReductionError::Ordering)?;
            if context.shared.zero_sectors.contains(&descriptor.sector) {
                return Err(invalid("deferred owner also has zero-sector evidence"));
            }
            for key in &descriptor.terminals {
                if key.powers().len() != arity
                    || key
                        .powers()
                        .iter()
                        .zip(descriptor.sector)
                        .any(|(&power, active)| (power > 0) != active)
                {
                    return Err(invalid(
                        "deferred terminal differs from its declared owner/arity",
                    ));
                }
            }
            let sector = descriptor.sector;
            if owners
                .insert(
                    sector,
                    Slot {
                        descriptor,
                        prepared: OnceLock::new(),
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate deferred owner sector"));
            }
        }
        if owners.is_empty() {
            return Err(invalid("deferred owner collection is empty"));
        }
        Ok(Self {
            context,
            owners,
            loader,
            attempts: AtomicUsize::new(0),
            preparation_nanoseconds: AtomicU64::new(0),
        })
    }

    pub fn context(&self) -> &Arc<CandidateOwnerContext<N>> {
        &self.context
    }
    pub fn owner_count(&self) -> usize {
        self.owners.len()
    }
    pub fn terminal_count(&self) -> usize {
        self.owners
            .values()
            .map(|slot| slot.descriptor.terminals.len())
            .sum()
    }
    pub fn owner_sectors(&self) -> impl Iterator<Item = &[bool; N]> {
        self.owners.keys()
    }
    pub fn contains_owner(&self, sector: &[bool; N]) -> bool {
        self.owners.contains_key(sector)
    }
    /// Authenticated baseline terminal inventory, without preparing formulas.
    /// First owner preparation verifies exactly the same terminal set.
    pub(in crate::solver::candidate_reduction) fn contains_terminal(
        &self,
        key: &IntegralKey,
    ) -> bool {
        let support = std::array::from_fn(|i| key.powers().get(i).copied().unwrap_or(0) > 0);
        self.owners
            .get(&support)
            .is_some_and(|slot| slot.descriptor.terminals.contains(key))
    }
    pub fn statistics(&self) -> DeferredCandidateOwnerStatistics {
        DeferredCandidateOwnerStatistics {
            declared_owners: self.owners.len(),
            prepared_owners: self
                .owners
                .values()
                .filter(|slot| matches!(slot.prepared.get(), Some(Ok(_))))
                .count(),
            failed_owners: self
                .owners
                .values()
                .filter(|slot| matches!(slot.prepared.get(), Some(Err(_))))
                .count(),
            preparation_attempts: self.attempts.load(Ordering::Relaxed),
            preparation_time: Duration::from_nanos(
                self.preparation_nanoseconds.load(Ordering::Relaxed),
            ),
        }
    }

    pub(in crate::solver::candidate_reduction) fn owner(
        &self,
        sector: &[bool; N],
    ) -> Result<Option<Arc<PreparedOwner<N>>>, CandidateReductionError> {
        let Some(slot) = self.owners.get(sector) else {
            return Ok(None);
        };
        let result = slot.prepared.get_or_init(|| {
            self.attempts.fetch_add(1, Ordering::Relaxed);
            let started = Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut programs = self.loader.prepare(*sector, self.context.clone())?;
                if !Arc::ptr_eq(programs.context(), &self.context) || programs.owner_count() != 1 {
                    return Err(invalid("deferred loader changed the exact shared context or owner selection"));
                }
                let owner = programs.owners.remove(sector).ok_or_else(|| invalid("deferred loader returned another owner"))?;
                if owner.root != slot.descriptor.saved_root || owner.ordering != slot.descriptor.ordering
                    || owner.batches.first().is_none_or(|batch| batch.terminals != slot.descriptor.terminals)
                {
                    return Err(invalid("deferred loader changed saved root, ordering or baseline terminal authority"));
                }
                Ok(owner)
            })).unwrap_or_else(|_| Err(invalid("deferred owner preparation panicked")));
            let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let _ = self.preparation_nanoseconds.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |before| Some(before.saturating_add(elapsed)));
            result
        });
        result
            .as_ref()
            .map(|owner| Some(owner.clone()))
            .map_err(Clone::clone)
    }
}

fn invalid(message: &str) -> CandidateReductionError {
    CandidateReductionError::InvalidInput(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::candidate_reduction::owner_test_support::{context, input};

    #[derive(Debug)]
    struct Loader {
        calls: Arc<AtomicUsize>,
        fail: bool,
        wrong_context: bool,
    }
    impl DeferredCandidateOwnerLoader<3> for Loader {
        fn prepare(
            &self,
            sector: [bool; 3],
            shared: Arc<CandidateOwnerContext<3>>,
        ) -> Result<CandidateOwnerPrograms<3>, CandidateReductionError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.fail {
                return Err(invalid("fixture native admission failed"));
            }
            let shared = if self.wrong_context {
                context(shared.family_owner().clone(), None, Default::default())
            } else {
                shared
            };
            let terminal = sector.map(i16::from);
            CandidateOwnerPrograms::try_new(shared, [input(sector, None, vec![], &[terminal])])
        }
    }

    fn fixture(
        fail: bool,
        wrong_context: bool,
    ) -> (Arc<DeferredCandidateOwnerPrograms<3>>, Arc<AtomicUsize>) {
        let family = Arc::new(crate::solver::tests::sunset());
        let shared = context(family, None, Default::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let descriptors = [[false, true, true], [true, false, true]].map(|sector| {
            DeferredCandidateOwnerDescriptor {
                sector,
                saved_root: [true; 3],
                ordering: OrderingPolicy::SpiredUncutV1,
                terminals: BTreeSet::from([IntegralKey::try_new(sector.map(i64::from)).unwrap()]),
            }
        });
        (
            Arc::new(
                DeferredCandidateOwnerPrograms::try_new(
                    shared,
                    descriptors,
                    Arc::new(Loader {
                        calls: calls.clone(),
                        fail,
                        wrong_context,
                    }),
                )
                .unwrap(),
            ),
            calls,
        )
    }

    #[test]
    fn presence_is_eager_but_each_requested_owner_is_prepared_once() {
        let (catalogue, calls) = fixture(false, false);
        assert_eq!(catalogue.owner_count(), 2);
        assert!(catalogue.contains_owner(&[false, true, true]));
        assert_eq!(catalogue.statistics().prepared_owners, 0);
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        let first = catalogue.owner(&[false, true, true]).unwrap().unwrap();
        let repeated = catalogue.owner(&[false, true, true]).unwrap().unwrap();
        assert!(Arc::ptr_eq(&first, &repeated));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(catalogue.statistics().prepared_owners, 1);
        assert!(catalogue.owner(&[true, true, true]).unwrap().is_none());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        catalogue.owner(&[true, false, true]).unwrap().unwrap();
        assert_eq!(catalogue.statistics().prepared_owners, 2);
    }

    #[test]
    fn concurrent_first_use_and_errors_share_one_result() {
        let (catalogue, calls) = fixture(false, false);
        std::thread::scope(|scope| {
            let mut workers = Vec::new();
            for _ in 0..4 {
                let catalogue = &catalogue;
                workers.push(
                    scope.spawn(move || catalogue.owner(&[false, true, true]).unwrap().unwrap()),
                );
            }
            let values = workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>();
            assert!(values.iter().all(|owner| Arc::ptr_eq(&values[0], owner)));
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        for (fail, wrong_context) in [(true, false), (false, true)] {
            let (catalogue, calls) = fixture(fail, wrong_context);
            let first = catalogue.owner(&[false, true, true]).unwrap_err();
            let repeated = catalogue.owner(&[false, true, true]).unwrap_err();
            assert_eq!(first, repeated);
            assert_eq!(calls.load(Ordering::Relaxed), 1);
            assert_eq!(catalogue.statistics().failed_owners, 1);
            assert_eq!(catalogue.statistics().prepared_owners, 0);
        }
    }
}
