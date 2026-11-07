//! Test budgets that stop a request at a chosen resource checkpoint, the way
//! a cancelled request stops: a cancellation installed as a Symbolica
//! resource budget (Vakint's) refuses every checkpoint once it is set.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

/// Message of the refusal.
pub(crate) const MESSAGE: &str = "request cancelled";

/// A budget that counts the checkpoints it sees while armed and refuses from
/// the `stop_at`-th armed checkpoint on (never without `stop_at`).
pub(crate) struct Stopping {
    armed: Arc<AtomicBool>,
    seen: Arc<AtomicUsize>,
    budget: ResourceBudget,
}

impl Stopping {
    /// Armed from the start.
    pub(crate) fn armed(stop_at: Option<usize>) -> Self {
        let stopping = Self::disarmed(stop_at);
        stopping.arm();
        stopping
    }

    /// Armed by [`Self::arm`] (e.g. when a request reaches a stage).
    pub(crate) fn disarmed(stop_at: Option<usize>) -> Self {
        let armed = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(AtomicUsize::new(0));
        let (monitor_armed, monitor_seen) = (Arc::clone(&armed), Arc::clone(&seen));
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(u64::MAX),
                max_elapsed: None,
            },
            move || {
                if !monitor_armed.load(Ordering::SeqCst) {
                    return Ok(0);
                }
                let seen = monitor_seen.fetch_add(1, Ordering::SeqCst) + 1;
                if stop_at.is_some_and(|stop_at| seen >= stop_at) {
                    Err(MESSAGE.to_owned())
                } else {
                    Ok(0)
                }
            },
        );
        Self {
            armed,
            seen,
            budget,
        }
    }

    pub(crate) fn arm(&self) {
        self.armed.store(true, Ordering::SeqCst);
    }

    /// A handle that arms this budget (for kernels and observers).
    pub(crate) fn trigger(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.armed)
    }

    /// Checkpoints seen while armed.
    pub(crate) fn seen(&self) -> usize {
        self.seen.load(Ordering::SeqCst)
    }

    pub(crate) fn run<R>(&self, operation: impl FnOnce() -> R) -> R {
        self.budget.run(operation)
    }
}

/// Whether `error` (or a cause of it) is the refusal of a [`Stopping`] budget.
pub(crate) fn is_stop(error: &(dyn std::error::Error + 'static)) -> bool {
    matches!(
        crate::reduction::resource_limit_error(error),
        Some(ResourceLimitError::MonitorUnavailable { message, .. }) if message == MESSAGE
    )
}
