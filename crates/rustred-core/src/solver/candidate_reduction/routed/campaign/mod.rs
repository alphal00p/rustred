//! Shared finite-target closure: one FIFO and one identity set across all
//! entries. Algebra is local to a worker; only scheduling/accounting is locked.
mod census;
mod completed;
mod generated;
mod inline;
pub use census::{
    CandidateRoutedSelectionIdentity, CandidateRoutedTraceCensus, CandidateRoutedTraceCensusLimits,
};
pub use completed::{
    CandidateRoutedCompletedTrace, CandidateRoutedCompletedTraceError,
    CandidateRoutedCompletedTraceLimits,
};
pub use generated::{
    CandidateRoutedCompletedDelta, CandidateRoutedDeltaExportLimits,
    CandidateRoutedGeneratedBaseline,
};
mod model;
mod pool;
pub use inline::CandidateRoutedWorkBudget;
pub use pool::CandidateRoutedWorkerPool;
mod scheduler;
#[cfg(test)]
mod tests;
mod worker;

use super::{CandidateEntryAdmission, RoutedCandidateReducer};
use crate::family::IntegralKey;
pub use model::*;
#[cfg(not(target_arch = "wasm32"))]
use scheduler::Failure;
use scheduler::Shared;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

impl<const N: usize> RoutedCandidateReducer<N> {
    /// Trace every entry through one shared canonical worklist. All limits are
    /// campaign-aggregate, except the existing per-formula native algebra caps.
    /// `workers` must be positive; one uses exactly the same scheduler. Native
    /// worker counts are caller-controlled, with no fixed library ceiling.
    /// The request must fit the calling process's available CPU affinity and
    /// Symbolica license; preflight occurs before admitting any starting Work.
    /// WebAssembly requires exactly one inline worker.
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
        observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
        self.trace_targets_parallel_with_entry_admission_and_observer(
            targets,
            CandidateEntryAdmission::SavedGenerationScope,
            workers,
            cancellation,
            observer,
        )
    }

    /// Shared trace with an entry-only policy, replacing the saved rank gate
    /// when explicitly requested. Source validation still precedes admission.
    /// Every root is checked before any worker starts. On admission failure the
    /// observer receives the failed preflight snapshot with no scheduled work.
    pub fn trace_targets_parallel_with_entry_admission_and_observer(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
        workers: usize,
        cancellation: &AtomicBool,
        observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
        self.trace_parallel(
            targets,
            admission,
            workers,
            None,
            None,
            None,
            cancellation,
            observer,
        )
        .map(|(report, _)| report)
    }

    /// Same shared native kernel with bounded exact selected-rule census.
    /// Census multiplicities count successful deduplicated Work, not inputs.
    pub fn trace_targets_parallel_with_entry_admission_and_census_observer(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
        workers: usize,
        budget: CandidateRoutedWorkBudget,
        census: CandidateRoutedTraceCensusLimits,
        cancellation: &AtomicBool,
        observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
        self.trace_parallel(
            targets,
            admission,
            workers,
            Some(budget),
            Some(census),
            None,
            cancellation,
            observer,
        )
        .map(|(report, _)| report)
    }

    fn trace_parallel(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
        workers: usize,
        budget: Option<CandidateRoutedWorkBudget>,
        census: Option<CandidateRoutedTraceCensusLimits>,
        baseline: Option<scheduler::CompletedBaseline<'_, N>>,
        cancellation: &AtomicBool,
        observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<
        (
            CandidateRoutedCampaignReport<N>,
            scheduler::membership::Membership<N>,
        ),
        CandidateRoutedCampaignError<N>,
    > {
        self.trace_with_pool(
            targets,
            admission,
            workers,
            budget,
            census,
            baseline,
            None,
            cancellation,
            observer,
        )
    }

    fn trace_with_pool(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
        workers: usize,
        budget: Option<CandidateRoutedWorkBudget>,
        census: Option<CandidateRoutedTraceCensusLimits>,
        baseline: Option<scheduler::CompletedBaseline<'_, N>>,
        pool: Option<&CandidateRoutedWorkerPool>,
        cancellation: &AtomicBool,
        mut observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<
        (
            CandidateRoutedCampaignReport<N>,
            scheduler::membership::Membership<N>,
        ),
        CandidateRoutedCampaignError<N>,
    > {
        let mut shared = Shared::new(self, workers, cancellation);
        if let Some(budget) = budget {
            shared.apply_budget(budget);
        }
        if let Some(census) = census {
            shared.enable_census(census);
        }
        shared.set_completed_baseline(baseline);
        // Acquire before admission/observers, not only worker dispatch: a
        // callback recursively invoking this pool receives a typed busy error.
        let (_pool_lease, preparation) = match pool.map(|pool| pool.acquire(workers)).transpose() {
            Ok(lease) => (
                lease,
                shared.prepare_with_entry_admission(self, targets, admission),
            ),
            Err(error) => (None, Err(CandidateRoutedCampaignFailure::from(error))),
        };
        if let Err(error) = preparation {
            shared.fail(error);
            let result = shared.into_result_with_membership();
            observer(
                result
                    .as_ref()
                    .err()
                    .expect("preparation failed")
                    .snapshot(),
            );
            return result;
        }
        let initial = shared.snapshot();
        observer(&initial);
        // An all-baseline-hit batch needs no worker tasks. Entry validation and
        // the final cancellation/completion checks still run on the same path.
        if pool.is_some() && initial.finished {
            return shared.into_result_with_membership();
        }
        #[cfg(target_arch = "wasm32")]
        {
            worker::run_with_progress(self, &shared, || observer(&shared.snapshot()));
            observer(&shared.snapshot());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(pool) = pool {
            pool.run(self, &shared, &mut observer);
        } else {
            std::thread::scope(|scope| {
                for _ in 0..workers {
                    let shared = &shared;
                    if let Err(error) = std::thread::Builder::new()
                        .spawn_scoped(scope, move || worker::run(self, shared))
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
        }
        shared.into_result_with_membership()
    }
}
