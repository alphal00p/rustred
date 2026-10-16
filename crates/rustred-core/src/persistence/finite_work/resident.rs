//! Optional exact index of one complete trusted-generated predecessor.
//! This is an optimization of the same trust boundary, not a closure proof.
use super::*;
use std::{
    collections::HashSet,
    mem::size_of,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidentWorkStatistics {
    pub records: usize,
    pub capacity: usize,
    pub retained_payload_estimate: usize,
    pub reserved_coexisting_bytes: usize,
    pub max_live_payload_bytes: usize,
}

/// Cannot be built from caller-provided membership answers or partial pages.
/// Reads are immutable and lock-free throughout a native window. Full keys,
/// including phase and literal Apply owner, participate in hash AND equality.
pub struct TrustedGeneratedResidentWork {
    execution: GeneratedExecutionBinding,
    generation: GeneratedWorkDigest,
    work: HashSet<GeneratedWorkKey>,
    heap_bytes: usize,
    reserved: usize,
    maximum: usize,
}

/// Owns the previous index plus a separately charged native-sealed delta (or
/// no new Work for a metadata-only append).
/// Dropping this value aborts the optimization; only the persistence adapter
/// may call `after_durable_commit`, after CURRENT has committed successfully.
pub struct PreparedResidentWork {
    previous: TrustedGeneratedResidentWork,
    delta: Vec<GeneratedWorkKey>,
    delta_heap: usize,
    original_delta_payload: usize,
}

impl TrustedGeneratedResidentWork {
    /// Prepare a generation rebind for the persistence adapter's metadata-only
    /// append. It must commit an EMPTY Work delta against this predecessor;
    /// the staged index has exactly the same members and allocates nothing.
    pub fn prepare_metadata_only(
        self,
        predecessor: GeneratedWorkDigest,
    ) -> GeneratedWorkResult<PreparedResidentWork> {
        if self.generation != predecessor {
            return Err(corrupt("resident metadata predecessor differs"));
        }
        Ok(PreparedResidentWork {
            previous: self,
            delta: Vec::new(),
            delta_heap: 0,
            original_delta_payload: 0,
        })
    }

    /// Hydrate exactly once from every validated page of the captured directory.
    /// `reserved_coexisting_bytes` charges the caller's page cache, decoder,
    /// actual directory payload and explicit transient reservations that may
    /// coexist, never a directory's permission cap. Updates separately charge
    /// the original native delta's actual vector/key capacities.
    ///
    /// Accounting includes key Vec capacities and a conservative HashSet
    /// bucket/control estimate, including old/new tables during reservation.
    /// std does not promise allocator-exact table layouts: this grant is a
    /// payload estimate, not a hard RSS cap. The process RSS guard remains
    /// authoritative. Insufficient grant/allocation yields None; integrity,
    /// provider, accounting and cancellation failures are never a decline.
    pub fn hydrate(
        directory: &TrustedGeneratedWorkDirectory,
        provider: &dyn GeneratedPageProvider,
        max_live_payload_bytes: usize,
        reserved_coexisting_bytes: usize,
        cancel: &AtomicBool,
    ) -> GeneratedWorkResult<Option<Self>> {
        check(cancel)?;
        if max_live_payload_bytes == 0 {
            return Ok(None);
        }
        let records = usize::try_from(directory.statistics().records)
            .map_err(|_| resource("resident Work record count overflow"))?;
        let heap_bound = records
            .checked_mul(directory.execution().physical_arity as usize)
            .and_then(|n| n.checked_mul(size_of::<i64>() + size_of::<bool>()))
            .ok_or_else(|| resource("resident Work key estimate overflow"))?;
        let planned = sum(&[
            reserved_coexisting_bytes,
            table_estimate(records)?,
            heap_bound,
        ])?;
        if planned > max_live_payload_bytes {
            check(cancel)?;
            return Ok(None);
        }
        let mut result = Self {
            execution: directory.execution().clone(),
            generation: directory.generation(),
            work: HashSet::new(),
            heap_bytes: 0,
            reserved: reserved_coexisting_bytes,
            maximum: max_live_payload_bytes,
        };
        if result.work.try_reserve(records).is_err() {
            check(cancel)?;
            return Ok(None);
        }
        // Check the reported capacity too; the preflight estimate is expressly
        // conservative rather than a promise about std's allocation internals.
        if sum(&[
            result.reserved,
            retained_table_estimate(result.work.capacity())?,
            heap_bound,
        ])? > result.maximum
        {
            check(cancel)?;
            return Ok(None);
        }
        for expected in directory.runs.iter().flatten() {
            check(cancel)?;
            let mut visited = false;
            let mut failure = None;
            let mut allocation_declined = false;
            let usage =
                provider.visit_validated_page(expected, expected.object.bytes, &mut |page| {
                    if visited {
                        failure = Some(corrupt("resident provider visited more than once"));
                        return;
                    }
                    visited = true;
                    if !page.matches(expected, directory.execution()) {
                        failure = Some(corrupt("resident provider substituted page/execution"));
                        return;
                    }
                    for key in &page.records {
                        if failure.is_some() || allocation_declined {
                            break;
                        }
                        let inserted = (|| {
                            check(cancel)?;
                            let owned = match clone_key(key) {
                                Ok(key) => key,
                                Err(_) => {
                                    allocation_declined = true;
                                    return Ok(());
                                }
                            };
                            let bytes = owned.heap_bytes()?;
                            if sum(&[result.reserved, result.retained_payload_estimate()?, bytes])?
                                > result.maximum
                            {
                                allocation_declined = true;
                                return Ok(());
                            }
                            if !result.work.insert(owned) {
                                return Err(corrupt("resident Work repeats across pages/runs"));
                            }
                            result.heap_bytes = sum(&[result.heap_bytes, bytes])?;
                            Ok(())
                        })();
                        if let Err(error) = inserted {
                            failure = Some(error);
                        }
                    }
                });
            if let Some(error) = failure {
                return Err(error);
            }
            let usage = usage?;
            if !visited
                || (usage.cache_hit && usage.disk_bytes != 0)
                || (!usage.cache_hit && usage.disk_bytes != expected.object.bytes)
            {
                return Err(corrupt("resident provider visit/read accounting mismatch"));
            }
            if allocation_declined {
                check(cancel)?;
                return Ok(None);
            }
        }
        check(cancel)?;
        if result.work.len() != records {
            return Err(corrupt("resident Work cardinality differs from directory"));
        }
        if sum(&[result.retained_payload_estimate()?, result.reserved])? > result.maximum {
            check(cancel)?;
            return Ok(None);
        }
        Ok(Some(result))
    }

    pub fn validate_directory(
        &self,
        directory: &TrustedGeneratedWorkDirectory,
    ) -> GeneratedWorkResult<()> {
        if self.execution != *directory.execution()
            || self.generation != directory.generation()
            || self.work.len() as u64 != directory.statistics().records
        {
            return Err(corrupt(
                "resident Work differs from captured execution/generation/count",
            ));
        }
        Ok(())
    }

    pub fn contains(&self, key: &GeneratedWorkKey) -> GeneratedWorkResult<bool> {
        key.validate(self.execution.physical_arity as usize)?;
        Ok(self.work.contains(key))
    }

    pub fn statistics(&self) -> GeneratedWorkResult<ResidentWorkStatistics> {
        Ok(ResidentWorkStatistics {
            records: self.work.len(),
            capacity: self.work.capacity(),
            retained_payload_estimate: self.retained_payload_estimate()?,
            reserved_coexisting_bytes: self.reserved,
            max_live_payload_bytes: self.maximum,
        })
    }

    /// Consume the index so an abandoned/failed commit cannot accidentally
    /// expose staged keys. Only the delta is cloned; existing keys are moved
    /// by occasional std HashSet growth, never cloned/rebuilt each window.
    pub fn prepare_delta<const N: usize>(
        mut self,
        delta: &crate::solver::CandidateRoutedCompletedDelta<'_, N>,
        cancel: &AtomicBool,
    ) -> GeneratedWorkResult<Option<PreparedResidentWork>> {
        check(cancel)?;
        if delta.execution() != &self.execution || delta.predecessor() != self.generation {
            return Err(corrupt(
                "resident delta differs from captured execution/generation",
            ));
        }
        let required = self
            .work
            .len()
            .checked_add(delta.work().len())
            .ok_or_else(|| resource("resident delta count overflow"))?;
        let mut heap = 0usize;
        for key in delta.work() {
            check(cancel)?;
            key.validate(self.execution.physical_arity as usize)?;
            if self.work.contains(key) {
                return Err(corrupt("native resident delta repeats completed Work"));
            }
            heap = sum(&[heap, clone_heap_bytes(key)?])?;
        }
        let vector = delta
            .work()
            .len()
            .checked_mul(size_of::<GeneratedWorkKey>())
            .ok_or_else(|| resource("resident delta vector overflow"))?;
        let growth = if required > self.work.capacity() {
            table_estimate(required)?
        } else {
            0
        };
        let original_delta = delta.work_payload_bytes()?;
        if sum(&[
            self.reserved,
            original_delta,
            self.retained_payload_estimate()?,
            growth,
            vector,
            heap,
        ])? > self.maximum
        {
            check(cancel)?;
            return Ok(None);
        }
        if self.work.try_reserve(delta.work().len()).is_err() {
            check(cancel)?;
            return Ok(None);
        }
        let mut staged = Vec::new();
        if staged.try_reserve_exact(delta.work().len()).is_err() {
            check(cancel)?;
            return Ok(None);
        }
        for key in delta.work() {
            check(cancel)?;
            match clone_key(key) {
                Ok(key) => staged.push(key),
                Err(_) => {
                    check(cancel)?;
                    return Ok(None);
                }
            }
        }
        let staged_payload = work_payload_bytes(&staged, staged.capacity())?;
        if sum(&[
            self.reserved,
            original_delta,
            self.retained_payload_estimate()?,
            staged_payload,
        ])? > self.maximum
        {
            check(cancel)?;
            return Ok(None);
        }
        let delta_heap = staged
            .iter()
            .try_fold(0usize, |total, key| sum(&[total, key.heap_bytes()?]))?;
        // Establish that the infallible postcommit addition cannot overflow.
        sum(&[self.heap_bytes, delta_heap])?;
        check(cancel)?;
        Ok(Some(PreparedResidentWork {
            previous: self,
            delta: staged,
            delta_heap,
            original_delta_payload: original_delta,
        }))
    }

    fn retained_payload_estimate(&self) -> GeneratedWorkResult<usize> {
        sum(&[
            retained_table_estimate(self.work.capacity())?,
            self.heap_bytes,
        ])
    }
}

impl PreparedResidentWork {
    /// Refresh the adapter's actual coexistence accounting before CURRENT.
    /// A commit may hold predecessor and successor directory allocations at
    /// once; only the successor's charge remains after durable adoption.
    /// Insufficient grant drops the optional index without refusing the commit.
    pub fn refresh_coexisting_bytes(
        mut self,
        during_commit: usize,
        after_commit: usize,
    ) -> GeneratedWorkResult<Option<Self>> {
        if after_commit > during_commit {
            return Err(corrupt(
                "resident postcommit reservation exceeds coexistence",
            ));
        }
        if sum(&[
            during_commit,
            self.original_delta_payload,
            self.previous.retained_payload_estimate()?,
            work_payload_bytes(&self.delta, self.delta.capacity())?,
        ])? > self.previous.maximum
        {
            return Ok(None);
        }
        self.previous.reserved = after_commit;
        Ok(Some(self))
    }

    /// Trusted persistence integration boundary, called only after successful
    /// durable native-sealed or unchanged-Work metadata commit. All reservation was done before
    /// CURRENT; moving the staged keys requires no new allocation or I/O.
    pub fn after_durable_commit(
        mut self,
        generation: GeneratedWorkDigest,
    ) -> TrustedGeneratedResidentWork {
        self.previous.work.extend(self.delta);
        self.previous.heap_bytes += self.delta_heap;
        self.previous.generation = generation;
        self.previous
    }
}

fn sum(parts: &[usize]) -> GeneratedWorkResult<usize> {
    parts
        .iter()
        .try_fold(0usize, |total, &n| total.checked_add(n))
        .ok_or_else(|| resource("resident Work payload estimate overflow"))
}
// Reserve estimate: at least two buckets per requested record, rounded up,
// with key slots + control bytes and a full extra control group. This is an
// explicit conservative estimate, not a dependency on std's private layout.
fn table_estimate(records: usize) -> GeneratedWorkResult<usize> {
    if records == 0 {
        return Ok(0);
    }
    let buckets = records
        .checked_mul(2)
        .and_then(usize::checked_next_power_of_two)
        .ok_or_else(|| resource("resident Work table estimate overflow"))?
        .max(4);
    sum(&[
        buckets
            .checked_mul(size_of::<GeneratedWorkKey>() + 1)
            .ok_or_else(|| resource("resident Work slot estimate overflow"))?,
        64,
    ])
}
fn retained_table_estimate(capacity: usize) -> GeneratedWorkResult<usize> {
    if capacity == 0 {
        return Ok(0);
    }
    let buckets = capacity
        .checked_add(1)
        .and_then(usize::checked_next_power_of_two)
        .ok_or_else(|| resource("resident Work capacity estimate overflow"))?;
    sum(&[
        buckets
            .checked_mul(size_of::<GeneratedWorkKey>() + 1)
            .ok_or_else(|| resource("resident Work capacity payload overflow"))?,
        64,
    ])
}
fn clone_heap_bytes(key: &GeneratedWorkKey) -> GeneratedWorkResult<usize> {
    key.powers()
        .len()
        .checked_mul(size_of::<i64>() + usize::from(matches!(key, GeneratedWorkKey::Apply { .. })))
        .ok_or_else(|| resource("resident Work clone payload overflow"))
}
fn clone_vec<T: Copy>(values: &[T]) -> Result<Vec<T>, std::collections::TryReserveError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(values.len())?;
    copy.extend_from_slice(values);
    Ok(copy)
}
fn clone_key(
    key: &GeneratedWorkKey,
) -> Result<GeneratedWorkKey, std::collections::TryReserveError> {
    Ok(match key {
        GeneratedWorkKey::Route(powers) => GeneratedWorkKey::Route(clone_vec(powers)?),
        GeneratedWorkKey::Apply { owner, powers } => GeneratedWorkKey::Apply {
            owner: clone_vec(owner)?,
            powers: clone_vec(powers)?,
        },
    })
}
fn check(cancel: &AtomicBool) -> GeneratedWorkResult<()> {
    if cancel.load(Ordering::Acquire) {
        Err(GeneratedWorkError::new(
            GeneratedWorkErrorKind::Cancelled,
            "resident Work hydration/update cancelled",
        ))
    } else {
        Ok(())
    }
}
