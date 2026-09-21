//! Shared finite-target closure: one FIFO and one identity set across all
//! entries. Algebra is local to a worker; only scheduling/accounting is locked.
mod model;
mod scheduler;
#[cfg(test)]
mod tests;
mod worker;

use super::RoutedCandidateReducer;
use crate::family::IntegralKey;
pub use model::*;
use scheduler::{Failure, Shared};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

impl<const N: usize> RoutedCandidateReducer<N> {
    /// Trace every entry through one shared canonical worklist. All limits are
    /// campaign-aggregate, except the existing per-formula native algebra caps.
    /// `workers` must be in 1..=64; one uses exactly the same scheduler.
    ///
    /// The observer runs on the calling thread at roughly 250 ms intervals,
    /// including while workers execute native algebra. Cancellation is checked
    /// between native operations: an individual Symbolica call is not preempted.
    /// All workers are joined before return. Native inner-pool configuration is
    /// the caller's responsibility; this function does not mutate environment.
    ///
    /// Only initial entries are rank-admitted. Intermediate keys above that
    /// rank are retained. A successful finite trace may still contain frontiers
    /// and is never a parametric coverage or independent-terminal certificate.
    ///
    /// Coalescing uses a conservative per-owner duplicate-shift reservation,
    /// refunded to actual local usage. A genuinely insufficient conservative
    /// allowance fails explicitly, even when specialization might use less.
    /// Temporary reservations from other workers are waited out, not errors.
    pub fn trace_targets_parallel_with_observer(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
        workers: usize,
        cancellation: &AtomicBool,
        mut observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
        let shared = Shared::new(self, workers, cancellation);
        let preparation = shared.prepare(self, targets);
        if let Err(error) = preparation {
            shared.fail(error);
            let result = shared.into_result();
            observer(
                result
                    .as_ref()
                    .err()
                    .expect("preparation failed")
                    .snapshot(),
            );
            return result;
        }
        // Exact output keys are n + shift; equal shifts are the only possible
        // collisions. Skipped zeros and cancellation can only lower this bound.
        // This scans borrowed metadata once, not formula copies per worker.
        let coalescing_bounds: BTreeMap<_, _> = self
            .programs
            .owners
            .iter()
            .map(|(mask, owner)| {
                let bound = owner
                    .rules
                    .iter()
                    .map(|rule| {
                        let unique: BTreeSet<_> = rule.rhs.iter().map(|term| &term.shift).collect();
                        rule.rhs.len() - unique.len()
                    })
                    .max()
                    .unwrap_or(0);
                (*mask, bound)
            })
            .collect();
        observer(&shared.snapshot());
        std::thread::scope(|scope| {
            for _ in 0..workers {
                let shared = &shared;
                let bounds = &coalescing_bounds;
                if let Err(error) = std::thread::Builder::new()
                    .spawn_scoped(scope, move || worker::run(self, shared, bounds))
                {
                    shared.fail(Failure::Trace(super::CandidateRoutedError::InvalidInput(
                        format!("cannot start campaign worker: {error}"),
                    )));
                    break;
                }
            }
            // Do not invoke user code under the scheduler lock. If it panics,
            // signal the workers before unwinding/joining the scoped threads.
            let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                loop {
                    let (snapshot, done) = shared.wait_snapshot(Duration::from_millis(250));
                    observer(&snapshot);
                    if done {
                        break;
                    }
                }
            }));
            if let Err(panic) = observed {
                shared.fail(CandidateRoutedCampaignFailure::Cancelled);
                std::panic::resume_unwind(panic);
            }
        });
        shared.into_result()
    }
}
