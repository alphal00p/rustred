//! Browser-safe, synchronous execution. This is not a background job queue.

use super::{CoordinatorError, panic_message};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_arch = "wasm32")]
static COORDINATOR: Coordinator = Coordinator::new();

pub(crate) struct Coordinator {
    active: AtomicBool,
    poisoned: AtomicBool,
}

impl Coordinator {
    const fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            poisoned: AtomicBool::new(false),
        }
    }

    /// Run the whole operation before returning. In Pyodide the embedding
    /// application can put its interpreter in a Web Worker to keep its UI live.
    pub(crate) fn submit<F>(&self, operation: F) -> Result<(), CoordinatorError>
    where
        F: FnOnce() + Send + 'static,
    {
        self.execute(operation)
    }

    pub(crate) fn execute<T, F>(&self, operation: F) -> Result<T, CoordinatorError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(CoordinatorError::Poisoned);
        }
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| CoordinatorError::Busy)?;
        // A callback must never recursively enter Symbolica through the
        // coordinator. No blocking mutex/channel is used on this path.
        let _active = ActiveGuard(&self.active);
        if self.poisoned.load(Ordering::Acquire) {
            return Err(CoordinatorError::Poisoned);
        }
        match catch_unwind(AssertUnwindSafe(operation)) {
            Ok(value) => Ok(value),
            Err(payload) => {
                self.poisoned.store(true, Ordering::Release);
                Err(CoordinatorError::Panicked(panic_message(payload.as_ref())))
            }
        }
    }
}

struct ActiveGuard<'a>(&'a AtomicBool);

impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn process_coordinator() -> Result<&'static Coordinator, String> {
    Ok(&COORDINATOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn inline_submission_finishes_before_returning_on_the_calling_thread() {
        let coordinator = Coordinator::new();
        let finished = Arc::new(AtomicBool::new(false));
        let work = finished.clone();
        let caller = std::thread::current().id();
        coordinator
            .submit(move || {
                assert_eq!(std::thread::current().id(), caller);
                work.store(true, Ordering::Release);
            })
            .unwrap();
        assert!(finished.load(Ordering::Acquire));
        assert_eq!(coordinator.execute(|| 7), Ok(7));
    }

    #[test]
    fn inline_reentry_fails_without_blocking_and_later_work_still_runs() {
        let coordinator = Arc::new(Coordinator::new());
        let nested = coordinator.clone();
        assert_eq!(
            coordinator.execute(move || nested.execute(|| 2)),
            Ok(Err(CoordinatorError::Busy))
        );
        assert_eq!(coordinator.execute(|| 3), Ok(3));
    }

    #[test]
    fn inline_panic_permanently_poisons_the_coordinator() {
        let coordinator = Coordinator::new();
        assert_eq!(
            coordinator.execute(|| panic!("inline test panic")),
            Err(CoordinatorError::Panicked("inline test panic".into()))
        );
        assert_eq!(coordinator.execute(|| 3), Err(CoordinatorError::Poisoned));
    }
}
