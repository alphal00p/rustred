//! Optional accounting for completed result buffers, including channel-owned
//! results. This is an admission limit, not a bound on running CAS allocations.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
pub(super) struct Counter {
    enabled: AtomicBool,
    bytes: AtomicUsize,
    count: AtomicUsize,
    peak_bytes: AtomicUsize,
    peak_count: AtomicUsize,
}

#[derive(Clone, Copy, Default, Debug, serde::Serialize)]
pub(in super::super::super) struct ResultMemory {
    pub bytes: usize,
    pub count: usize,
    pub peak_bytes: usize,
    pub peak_count: usize,
}

#[derive(Default, serde::Serialize)]
pub(in super::super::super) struct EscrowDiagnostics {
    pub enabled: bool,
    pub version: u32,
    pub base_window: usize,
    pub total_logical_limit: usize,
    pub extra_job_allowance: usize,
    pub result_byte_admission_limit: usize,
    pub extra_jobs_dispatched: u64,
    pub peak_logical_reserved: usize,
    pub returned_results: ResultMemory,
    pub result_byte_overshoot: usize,
    pub physical_queued: usize,
    pub physical_computing: usize,
    pub physical_returned: usize,
    pub logical_reserved: usize,
    pub coordinator_completed: usize,
}

impl Counter {
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Release);
    }

    pub fn snapshot(&self) -> ResultMemory {
        ResultMemory {
            bytes: self.bytes.load(Ordering::Acquire),
            count: self.count.load(Ordering::Acquire),
            peak_bytes: self.peak_bytes.load(Ordering::Acquire),
            peak_count: self.peak_count.load(Ordering::Acquire),
        }
    }

    pub fn wrap(self: &Arc<Self>, bytes: Vec<u8>) -> ReturnedBytes {
        let charge = if self.enabled.load(Ordering::Acquire) {
            let capacity = bytes.capacity();
            // All charged buffers are simultaneously allocated: their total
            // cannot exceed the address space. No estimated future sizes.
            let total = self.bytes.fetch_add(capacity, Ordering::AcqRel) + capacity;
            let count = self.count.fetch_add(1, Ordering::AcqRel) + 1;
            self.peak_bytes.fetch_max(total, Ordering::Relaxed);
            self.peak_count.fetch_max(count, Ordering::Relaxed);
            Some(Arc::clone(self))
        } else {
            None
        };
        ReturnedBytes { bytes, charge }
    }
}

/// The charge follows ownership through the worker channel and coordinator
/// map. `into_vec` marks handover to P1 decoding, not unmerged result retention.
pub(in super::super::super) struct ReturnedBytes {
    bytes: Vec<u8>,
    charge: Option<Arc<Counter>>,
}

impl ReturnedBytes {
    pub fn uncharged(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            charge: None,
        }
    }

    pub fn into_vec(mut self) -> Vec<u8> {
        self.release();
        std::mem::take(&mut self.bytes)
    }

    fn release(&mut self) {
        if let Some(counter) = self.charge.take() {
            counter
                .bytes
                .fetch_sub(self.bytes.capacity(), Ordering::AcqRel);
            counter.count.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl Drop for ReturnedBytes {
    fn drop(&mut self) {
        self.release();
    }
}

impl std::fmt::Debug for ReturnedBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.bytes.fmt(f)
    }
}

impl PartialEq<Vec<u8>> for ReturnedBytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        self.bytes == *other
    }
}

impl<const N: usize> PartialEq<[u8; N]> for ReturnedBytes {
    fn eq(&self, other: &[u8; N]) -> bool {
        self.bytes == *other
    }
}

impl std::ops::Deref for ReturnedBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_charge_follows_owned_result_and_releases_on_p1_or_drop() {
        let memory = Arc::new(Counter::default());
        let uncharged = memory.wrap(Vec::with_capacity(4096));
        assert_eq!(memory.snapshot().count, 0);
        drop(uncharged);
        memory.enable();
        let first = memory.wrap(Vec::with_capacity(1024));
        let second = memory.wrap(Vec::with_capacity(2048));
        assert_eq!(memory.snapshot().bytes, 3072);
        assert_eq!(memory.snapshot().count, 2);
        let raw = first.into_vec();
        assert_eq!(memory.snapshot().bytes, 2048);
        assert_eq!(raw.capacity(), 1024);
        drop(second);
        assert_eq!(memory.snapshot().bytes, 0);
        assert_eq!(memory.snapshot().count, 0);
        assert_eq!(memory.snapshot().peak_bytes, 3072);
    }
}
