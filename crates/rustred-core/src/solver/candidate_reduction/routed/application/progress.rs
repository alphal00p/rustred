//! Optional caller-thread observations of exact application, never CAS work.
use std::time::{Duration, Instant};

use super::CandidateRoutedApplicationStatistics;
use crate::reduction::CacheWeight;

/// Charged completed coefficient payload, including guards. These counts do
/// not include pending frames, output scratch, allocator overhead or process RSS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedCacheCensus {
    pub entries: usize,
    pub coefficient_terms: usize,
    pub coefficient_bytes: usize,
}

impl CandidateRoutedCacheCensus {
    pub(super) fn new(entries: usize, weight: CacheWeight) -> Self {
        Self {
            entries,
            coefficient_terms: weight.coefficient_terms,
            coefficient_bytes: weight.coefficient_bytes,
        }
    }

    pub(super) fn retain(&mut self, weight: CacheWeight) {
        // A subset of the already admitted cache census cannot overflow. The
        // caller updates this only after budget admission or first pinning.
        self.entries += 1;
        self.coefficient_terms += weight.coefficient_terms;
        self.coefficient_bytes += weight.coefficient_bytes;
    }

    pub(super) fn release(&mut self, weight: CacheWeight) {
        self.entries -= 1;
        self.coefficient_terms -= weight.coefficient_terms;
        self.coefficient_bytes -= weight.coefficient_bytes;
    }

    pub(super) fn excluding(self, other: Self) -> Self {
        Self {
            entries: self.entries - other.entries,
            coefficient_terms: self.coefficient_terms - other.coefficient_terms,
            coefficient_bytes: self.coefficient_bytes - other.coefficient_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedApplicationPhase {
    Admission,
    Expansion,
    Combination,
    RootCollection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedApplicationStatus {
    Running,
    Succeeded,
    Failed,
}

/// One coherent safe-boundary observation. Work is local to this request;
/// retained cache counts also include reusable entries from earlier requests.
/// A successful core request precedes any caller-owned terminal substitutions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateRoutedApplicationSnapshot {
    pub sequence: u64,
    pub elapsed: Duration,
    pub phase: CandidateRoutedApplicationPhase,
    pub status: CandidateRoutedApplicationStatus,
    /// Request counters, with `operational_nodes` updated in this snapshot copy
    /// to match the current number of distinct entered nodes.
    pub statistics: CandidateRoutedApplicationStatistics,
    pub unique_nodes: usize,
    pub active_nodes: usize,
    pub pending_frames: usize,
    pub cached: CandidateRoutedCacheCensus,
    /// Evicting policies maintain dependency pins. None under FailOnLimit, not zero.
    pub pinned: Option<CandidateRoutedCacheCensus>,
    pub eligible: Option<CandidateRoutedCacheCensus>,
}

pub(super) struct Progress<'a> {
    pub observer: &'a mut dyn FnMut(&CandidateRoutedApplicationSnapshot),
    pub phase: CandidateRoutedApplicationPhase,
    pub started: Instant,
    pub sequence: u64,
    interval: Duration,
    last: Instant,
    remaining: u16,
}

impl<'a> Progress<'a> {
    pub fn new(
        interval: Duration,
        observer: &'a mut dyn FnMut(&CandidateRoutedApplicationSnapshot),
    ) -> Self {
        let started = Instant::now();
        Self {
            observer,
            phase: CandidateRoutedApplicationPhase::Admission,
            started,
            sequence: 0,
            interval,
            last: started,
            remaining: 256,
        }
    }

    pub fn due(&mut self, force: bool) -> Option<Instant> {
        if !force && !self.interval.is_zero() {
            self.remaining -= 1;
            if self.remaining != 0 {
                return None;
            }
            self.remaining = 256;
        }
        let now = Instant::now();
        if !force && now.duration_since(self.last) < self.interval {
            return None;
        }
        self.last = now;
        self.sequence = self.sequence.saturating_add(1);
        Some(now)
    }
}

#[cfg(test)]
mod tests;
