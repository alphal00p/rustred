//! Invocation-local attempt accounting and one cancellable finite-case permit.
//! Completed CP6 records, not this scheduler state, carry durable authority.
use super::{InitialBindings, Outcome};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Attempts {
    seen: BTreeSet<u32>,
    active: Option<u32>,
    waiting: usize,
    last: Option<u32>,
    closed: usize,
    declined: usize,
    failed: usize,
}

pub(in crate::application::routed_campaign::walking) struct Account<const N: usize> {
    legacy_reserved: AtomicBool,
    work: Mutex<Value>,
    bindings: Option<Arc<InitialBindings<N>>>,
    initial_queries: bool,
    attempts: Mutex<Attempts>,
    ready: Condvar,
}
impl<const N: usize> Default for Account<N> {
    fn default() -> Self {
        Self {
            legacy_reserved: AtomicBool::new(false),
            work: Mutex::new(json!({"status":"not_attempted"})),
            bindings: None,
            initial_queries: false,
            attempts: Mutex::new(Attempts::default()),
            ready: Condvar::new(),
        }
    }
}
impl<const N: usize> Account<N> {
    pub fn await_initial_queries(&mut self) {
        self.initial_queries = true;
    }
    pub fn install_bindings(&mut self, bindings: Arc<InitialBindings<N>>) -> Result<(), String> {
        if self.bindings.is_some() || self.legacy_reserved.load(Ordering::Acquire) {
            return Err("finite initial-query account was already initialized".into());
        }
        self.bindings = Some(bindings);
        self.initial_queries = true;
        Ok(())
    }
    pub fn bindings(&self) -> Option<&InitialBindings<N>> {
        self.bindings.as_deref()
    }
    pub fn reserve(&self) -> bool {
        self.bindings.is_none() && !self.legacy_reserved.swap(true, Ordering::AcqRel)
    }
    /// Busy eligible cases wait; they do not silently become symbolic work.
    /// Cancellation is checked while waiting and between native operations.
    pub fn acquire(&self, id: u32, cancel: &AtomicBool) -> Result<Option<Permit<'_, N>>, ()> {
        let mut state = self
            .attempts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.seen.contains(&id) {
            return Ok(None);
        }
        state.waiting += 1;
        loop {
            if cancel.load(Ordering::Acquire) {
                state.waiting -= 1;
                return Err(());
            }
            if state.active.is_none() {
                state.waiting -= 1;
                if !state.seen.insert(id) {
                    return Ok(None);
                }
                state.active = Some(id);
                state.last = Some(id);
                return Ok(Some(Permit { account: self, id }));
            }
            state = self
                .ready
                .wait_timeout(state, Duration::from_millis(50))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
            if state.seen.contains(&id) {
                state.waiting -= 1;
                return Ok(None);
            }
        }
    }
    pub fn record(&self, work: Value) {
        *self
            .work
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = work;
    }
    pub fn completed_attempt(&self, outcome: &Outcome) {
        if self.bindings.is_none() {
            return;
        }
        let mut state = self
            .attempts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match outcome {
            Outcome::Closed { .. } => state.closed += 1,
            Outcome::Declined { .. } => state.declined += 1,
            Outcome::Failed { .. } => state.failed += 1,
        }
    }
    pub fn report(&self) -> Value {
        if let Some(bindings) = &self.bindings {
            let state = self
                .attempts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            json!({"attempts":state.seen.len(),"max_attempts":bindings.len(),
                "active_domain":state.active,"waiting_cases":state.waiting,"last_domain":state.last,
                "completed_cases":state.closed,"declined_cases":state.declined,"failed_cases":state.failed,
                "work":self.work.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
                "scope":"invocation-local authenticated whole initial queries; clean stop saves merged cases, unfinished case restarts; hard crash preserves only last checkpoint; not closure"})
        } else if self.initial_queries {
            json!({"status":"awaiting_authenticated_input_admission","attempts":0,"max_attempts":null,
                "work":self.work.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
                "scope":"finite initial-query mode; no completed input binding or native finite attempt in this invocation; not closure"})
        } else {
            json!({"attempts":usize::from(self.legacy_reserved.load(Ordering::Acquire)),
                "max_attempts":1,"work":self.work.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
                "scope":"fresh whole initial ID0 only; includes declined or cancelled trace work; not proof"})
        }
    }
}
pub(in crate::application::routed_campaign::walking) struct Permit<'a, const N: usize> {
    account: &'a Account<N>,
    id: u32,
}
impl<const N: usize> Drop for Permit<'_, N> {
    fn drop(&mut self) {
        let mut state = self
            .account
            .attempts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        debug_assert_eq!(state.active, Some(self.id));
        state.active = None;
        self.account.ready.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_query_unbound_report_does_not_claim_legacy_id0_scope() {
        let mut account = Account::<1>::default();
        account.await_initial_queries();
        let report = account.report();
        assert_eq!(report["status"], "awaiting_authenticated_input_admission");
        assert_eq!(report["attempts"], 0);
        assert!(report["max_attempts"].is_null());
        assert!(!report["scope"].as_str().unwrap().contains("ID0"));
    }
    #[test]
    fn finite_query_busy_permit_waits_and_cancels_without_symbolic_fallback() {
        let mut account = Account::<1>::default();
        account
            .install_bindings(Arc::new(InitialBindings::from_domains([])))
            .unwrap();
        let cancel = AtomicBool::new(false);
        let held = account.acquire(0, &cancel).unwrap().unwrap();
        std::thread::scope(|scope| {
            let waiting =
                scope.spawn(|| account.acquire(1, &cancel).map(|permit| permit.is_some()));
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while account.report()["waiting_cases"] != 1 {
                if std::time::Instant::now() >= deadline {
                    cancel.store(true, Ordering::Release);
                    panic!("finite waiter did not enter within test deadline");
                }
                std::thread::yield_now();
            }
            cancel.store(true, Ordering::Release);
            assert_eq!(waiting.join().unwrap(), Err(()));
        });
        assert_eq!(account.report()["attempts"], 1);
        drop(held);
        cancel.store(false, Ordering::Release);
        assert!(account.acquire(0, &cancel).unwrap().is_none());
        let next = account.acquire(1, &cancel).unwrap().unwrap();
        assert_eq!(account.report()["active_domain"], 1);
        drop(next);
        assert!(account.report()["active_domain"].is_null());
    }
}
