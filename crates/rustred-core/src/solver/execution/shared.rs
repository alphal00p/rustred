//! Global admission for a shared Rayon pool. Permits include queued jobs, so
//! a user limit bounds live sector solves even with nested Rayon callbacks.
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct SharedSectorSnapshot {
    pub workers: usize,
    pub max_active_sectors: Option<usize>,
    pub admitted: usize,
    pub running: usize,
    pub waiting_for_worker: usize,
    pub completed: u64,
    pub peak_running: usize,
}

#[derive(Default)]
struct State {
    admitted: usize,
    running: usize,
    completed: u64,
    peak: usize,
}

pub(super) struct Admission {
    workers: usize,
    limit: Option<usize>,
    state: Mutex<State>,
    changed: Condvar,
}

impl Admission {
    pub fn new(workers: usize, limit: Option<usize>) -> Self {
        Self {
            workers,
            limit,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }

    pub fn acquire(self: &Arc<Self>, cancel: Option<&AtomicBool>) -> Option<Permit> {
        let mut state = self.state.lock().unwrap();
        loop {
            if cancel.is_some_and(|cancel| cancel.load(Ordering::Acquire)) {
                return None;
            }
            if self.limit.is_none_or(|limit| state.admitted < limit) {
                state.admitted = state
                    .admitted
                    .checked_add(1)
                    .expect("sector admission overflow");
                return Some(Permit(Arc::clone(self)));
            }
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(100))
                .unwrap()
                .0;
        }
    }

    pub fn running(self: &Arc<Self>) -> Running {
        let mut state = self.state.lock().unwrap();
        state.running += 1;
        state.peak = state.peak.max(state.running);
        Running(Arc::clone(self))
    }

    pub fn snapshot(&self) -> SharedSectorSnapshot {
        let state = self.state.lock().unwrap();
        SharedSectorSnapshot {
            workers: self.workers,
            max_active_sectors: self.limit,
            admitted: state.admitted,
            running: state.running,
            waiting_for_worker: state.admitted.saturating_sub(state.running),
            completed: state.completed,
            peak_running: state.peak,
        }
    }
}

pub(super) struct Permit(Arc<Admission>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().admitted -= 1;
        self.0.changed.notify_all();
    }
}

pub(super) struct Running(Arc<Admission>);
impl Drop for Running {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.running -= 1;
        state.completed = state.completed.saturating_add(1);
    }
}
