//! Bounded, process-local preparation telemetry. Never checkpoint authority.
//! Producers update fixed-size scalars; consumers may poll independently while
//! a commit is adopting its already reserved resident index. No external user
//! callback, serialization or fallible IO runs at that atomic boundary.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default)]
pub struct GeneratedWorkProgress(Arc<Mutex<Option<State>>>);

#[derive(Clone, Debug)]
struct State {
    phase: &'static str,
    unit: &'static str,
    completed: usize,
    total: Option<usize>,
    started: Instant,
    updated: Instant,
    finished: bool,
}

#[derive(Clone, Debug)]
pub struct GeneratedWorkProgressSnapshot {
    pub phase: &'static str,
    pub unit: &'static str,
    pub completed: usize,
    pub total: Option<usize>,
    pub elapsed: Duration,
    pub update_age: Duration,
    pub finished: bool,
}

pub struct GeneratedWorkProgressReporter {
    handle: Option<GeneratedWorkProgress>,
    phase: &'static str,
    last: Instant,
    started: Instant,
}

impl GeneratedWorkProgress {
    pub fn clear(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    pub fn snapshot(&self) -> Option<GeneratedWorkProgressSnapshot> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let state = state.as_ref()?;
        Some(GeneratedWorkProgressSnapshot {
            phase: state.phase,
            unit: state.unit,
            completed: state.completed,
            total: state.total,
            elapsed: if state.finished {
                state.updated.duration_since(state.started)
            } else {
                state.started.elapsed()
            },
            update_age: state.updated.elapsed(),
            finished: state.finished,
        })
    }

    pub fn begin(
        handle: Option<&Self>,
        phase: &'static str,
        total: Option<usize>,
        unit: &'static str,
    ) -> GeneratedWorkProgressReporter {
        let now = Instant::now();
        if let Some(handle) = handle {
            *handle.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(State {
                phase,
                unit,
                completed: 0,
                total,
                started: now,
                updated: now,
                finished: false,
            });
        }
        GeneratedWorkProgressReporter {
            handle: handle.cloned(),
            phase,
            last: now,
            started: now,
        }
    }
}

impl GeneratedWorkProgressReporter {
    /// Reporting cadence only, not a workload limit. Avoid clocks/locks on the
    /// overwhelming majority of key comparisons and per-record operations.
    pub fn advance(&mut self, completed: usize) {
        if completed & 0x3fff == 0 {
            self.poll(completed);
        }
    }

    /// For coarse IO/page operations where checking the clock is inexpensive.
    pub fn poll(&mut self, completed: usize) {
        if self.handle.is_some() && self.last.elapsed() >= Duration::from_millis(500) {
            self.publish(completed, false);
        }
    }

    pub fn finish(&mut self, completed: usize) {
        self.publish(completed, true);
    }

    fn publish(&mut self, completed: usize, finished: bool) {
        if let Some(handle) = &self.handle {
            let mut state = handle.0.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = state
                .as_mut()
                .filter(|state| state.phase == self.phase && state.started == self.started)
            {
                state.completed = completed;
                state.updated = Instant::now();
                state.finished = finished;
                self.last = state.updated;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_and_indeterminate_sort_counts_are_distinct_from_completion() {
        let handle = GeneratedWorkProgress::default();
        let mut records =
            GeneratedWorkProgress::begin(Some(&handle), "export_keys", Some(20), "records");
        records.finish(20);
        assert!(handle.snapshot().unwrap().finished);
        let mut sort =
            GeneratedWorkProgress::begin(Some(&handle), "sort_keys", None, "comparisons");
        sort.finish(99);
        let snapshot = handle.snapshot().unwrap();
        assert_eq!(
            (snapshot.completed, snapshot.total, snapshot.unit),
            (99, None, "comparisons")
        );
        handle.clear();
        assert!(handle.snapshot().is_none());
    }

    #[test]
    fn stale_reporter_cannot_overwrite_a_new_phase() {
        let handle = GeneratedWorkProgress::default();
        let mut old = GeneratedWorkProgress::begin(Some(&handle), "old", Some(2), "records");
        let mut new = GeneratedWorkProgress::begin(Some(&handle), "new", Some(3), "records");
        old.finish(2);
        assert_eq!(handle.snapshot().unwrap().completed, 0);
        new.finish(3);
        assert_eq!(handle.snapshot().unwrap().completed, 3);
    }
}
