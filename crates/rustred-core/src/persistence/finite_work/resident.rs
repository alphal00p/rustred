//! Optional exact index of one complete trusted-generated predecessor.
//! This is an optimization of the same trust boundary, not a closure proof.
use super::*;
use std::{
    mem::size_of,
    sync::atomic::{AtomicBool, Ordering},
};
mod packed;
use packed::{PackedWork, Slot};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidentWorkStatistics {
    pub records: usize,
    pub capacity: usize,
    pub retained_payload_estimate: usize,
    pub reserved_coexisting_bytes: usize,
    pub max_live_payload_bytes: usize,
}

/// Cannot be built from caller-provided membership answers or partial pages.
/// Reads are immutable and lock-free throughout a native window. Phase and
/// complete powers participate in hash AND equality. Validated Apply owners
/// equal positive support, so the full literal owner is exactly recoverable.
pub struct TrustedGeneratedResidentWork {
    execution: GeneratedExecutionBinding,
    generation: GeneratedWorkDigest,
    work: PackedWork,
    reserved: usize,
    maximum: usize,
}

/// Owns the previous index plus a separately charged native-sealed delta (or
/// no new Work for a metadata-only append).
/// Dropping this value aborts the optimization; only the persistence adapter
/// may call `after_durable_commit`, after CURRENT has committed successfully.
pub struct PreparedResidentWork {
    previous: TrustedGeneratedResidentWork,
    delta: Vec<Slot>,
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
            original_delta_payload: 0,
        })
    }

    /// Hydrate exactly once from every validated page of the captured directory.
    /// `reserved_coexisting_bytes` charges the caller's page cache, decoder,
    /// actual directory payload and explicit transient reservations that may
    /// coexist, never a directory's permission cap. Updates separately charge
    /// the original native delta's actual vector/key capacities.
    ///
    /// Accounting includes chunk capacity/slack and a conservative HashTable
    /// bucket/control estimate, including old/new tables during reservation.
    /// The allocator does not promise exact usable-byte layouts: this grant is a
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
        let work = PackedWork::new(directory.execution().physical_arity as usize)?;
        let planned = sum(&[reserved_coexisting_bytes, work.growth_payload(records)?])?;
        if planned > max_live_payload_bytes {
            check(cancel)?;
            return Ok(None);
        }
        let mut result = Self {
            execution: directory.execution().clone(),
            generation: directory.generation(),
            work,
            reserved: reserved_coexisting_bytes,
            maximum: max_live_payload_bytes,
        };
        if !result.work.reserve(records, cancel)? {
            check(cancel)?;
            return Ok(None);
        }
        // Check the reported capacity too; the preflight estimate is expressly
        // conservative rather than a promise about std's allocation internals.
        if sum(&[result.reserved, result.retained_payload_estimate()?])? > result.maximum {
            check(cancel)?;
            return Ok(None);
        }
        for expected in directory.runs.iter().flatten() {
            check(cancel)?;
            let mut visited = false;
            let mut failure = None;
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
                        if failure.is_some() {
                            break;
                        }
                        let inserted = (|| {
                            check(cancel)?;
                            key.validate(result.execution.physical_arity as usize)?;
                            if !result.work.hydrate_key(key) {
                                return Err(corrupt("resident Work repeats across pages/runs"));
                            }
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

    /// Allocation-free exact native membership, with the same full phase,
    /// physical arity, owner support and zero-padding validation as conversion
    /// through GeneratedWorkKey::from_native followed by contains().
    pub fn contains_native<const N: usize>(
        &self,
        work: &crate::solver::CandidateRoutedWork<N>,
    ) -> GeneratedWorkResult<bool> {
        use crate::solver::CandidateRoutedWork;
        let powers = work.target().powers();
        let arity = self.execution.physical_arity as usize;
        if arity == 0 || powers.len() != arity || !crate::fits_storage(arity, N) {
            return Err(corrupt(
                "native resident Work has invalid physical/storage arity",
            ));
        }
        let apply = match work {
            CandidateRoutedWork::Route(_) => false,
            CandidateRoutedWork::Apply { owner_sector, .. } => {
                if owner_sector[arity..].iter().any(|&active| active)
                    || owner_sector[..arity]
                        .iter()
                        .zip(powers)
                        .any(|(&active, &power)| active != (power > 0))
                {
                    return Err(corrupt(
                        "native resident Apply owner differs from physical support",
                    ));
                }
                true
            }
        };
        Ok(self.work.contains_parts(apply, powers))
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
    /// expose staged keys. Only new powers are copied into shared chunks;
    /// occasional hash growth moves copyable slots, never heap-owned keys.
    pub fn prepare_delta<const N: usize>(
        self,
        delta: &crate::solver::CandidateRoutedCompletedDelta<'_, N>,
        cancel: &AtomicBool,
    ) -> GeneratedWorkResult<Option<PreparedResidentWork>> {
        self.prepare_delta_with_progress(delta, cancel, None)
    }

    pub fn prepare_delta_with_progress<const N: usize>(
        mut self,
        delta: &crate::solver::CandidateRoutedCompletedDelta<'_, N>,
        cancel: &AtomicBool,
        progress: Option<&GeneratedWorkProgress>,
    ) -> GeneratedWorkResult<Option<PreparedResidentWork>> {
        check(cancel)?;
        if delta.execution() != &self.execution || delta.predecessor() != self.generation {
            return Err(corrupt(
                "resident delta differs from captured execution/generation",
            ));
        }
        let mut stage = GeneratedWorkProgress::begin(
            progress,
            "validate_resident_delta",
            Some(delta.work().len()),
            "records",
        );
        for (i, key) in delta.work().iter().enumerate() {
            check(cancel)?;
            key.validate(self.execution.physical_arity as usize)?;
            if self.work.contains(key) {
                return Err(corrupt("native resident delta repeats completed Work"));
            }
            stage.advance(i + 1);
        }
        stage.finish(delta.work().len());
        let vector = delta
            .work()
            .len()
            .checked_mul(size_of::<Slot>())
            .ok_or_else(|| resource("resident delta vector overflow"))?;
        let growth = self.work.growth_payload(delta.work().len())?;
        let original_delta = delta.work_payload_bytes()?;
        if sum(&[
            self.reserved,
            original_delta,
            self.retained_payload_estimate()?,
            growth,
            vector,
        ])? > self.maximum
        {
            check(cancel)?;
            return Ok(None);
        }
        let mut stage =
            GeneratedWorkProgress::begin(progress, "reserve_resident_index", None, "allocations");
        if !self.work.reserve(delta.work().len(), cancel)? {
            check(cancel)?;
            return Ok(None);
        }
        stage.finish(1);
        let mut staged = Vec::new();
        if staged.try_reserve_exact(delta.work().len()).is_err() {
            check(cancel)?;
            return Ok(None);
        }
        let mut stage = GeneratedWorkProgress::begin(
            progress,
            "stage_resident_delta",
            Some(delta.work().len()),
            "records",
        );
        for (i, key) in delta.work().iter().enumerate() {
            check(cancel)?;
            staged.push(self.work.stage(key));
            stage.advance(i + 1);
        }
        stage.finish(delta.work().len());
        let staged_payload = slot_payload(staged.capacity())?;
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
        check(cancel)?;
        Ok(Some(PreparedResidentWork {
            previous: self,
            delta: staged,
            original_delta_payload: original_delta,
        }))
    }

    fn retained_payload_estimate(&self) -> GeneratedWorkResult<usize> {
        self.work.retained_payload()
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
            slot_payload(self.delta.capacity())?,
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
        self,
        generation: GeneratedWorkDigest,
    ) -> TrustedGeneratedResidentWork {
        self.after_durable_commit_with_progress(generation, None)
    }

    /// Only bounded process-local scalar telemetry is updated; no external
    /// callback or fallible operation enters the post-CURRENT adoption path.
    pub fn after_durable_commit_with_progress(
        mut self,
        generation: GeneratedWorkDigest,
        progress: Option<&GeneratedWorkProgress>,
    ) -> TrustedGeneratedResidentWork {
        let count = self.delta.len();
        let mut stage = GeneratedWorkProgress::begin(
            progress,
            "install_resident_delta",
            Some(count),
            "records",
        );
        for (i, slot) in self.delta.into_iter().enumerate() {
            self.previous.work.insert_reserved(slot);
            stage.advance(i + 1);
        }
        stage.finish(count);
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
fn slot_payload(capacity: usize) -> GeneratedWorkResult<usize> {
    capacity
        .checked_mul(size_of::<Slot>())
        .ok_or_else(|| resource("resident delta slot payload overflow"))
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
