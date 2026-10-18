//! Look ahead over existing deterministic prefix windows without adopting a
//! cursor. All proposed windows share one native traversal and durable commit.
use super::super::windows::{FiniteWindow, PlannedFiniteWindow};
use super::*;
use rustred::family::IntegralKey;
use rustred::solver::CandidateRoutedWorkerPool;
use std::collections::BTreeSet;
use std::time::Instant;

struct PendingWindow<const N: usize> {
    row: usize,
    ordinal: u64,
    plan: PlannedFiniteWindow<N>,
}

pub(super) struct PlannedWindow<const N: usize> {
    pub row: usize,
    pub ordinal: u64,
    pub window: FiniteWindow<N>,
}

pub(super) struct WindowGroup<const N: usize> {
    pub windows: Vec<PlannedWindow<N>>,
    pub inputs: usize,
    pub distinct_inputs: usize,
    pub planning_seconds: f64,
    pub materialization_seconds: f64,
}

impl<const N: usize> WindowGroup<N> {
    pub fn plan(
        sources: &[PrefixWindows<N>],
        cursors: &[PrefixCursor],
        first: usize,
        policy: Policy,
        pool: Option<&CandidateRoutedWorkerPool>,
        observer: &impl Fn(Value),
        cancel: &AtomicBool,
    ) -> Result<Self, AppError> {
        let target_grant =
            checked_group_grant(policy.windows.max_seed_points, policy.max_windows_per_batch)?;
        // Per-window enumeration independently enforces its byte allowance.
        // The checked product also bounds retained group seed payload without
        // introducing a new fixed cap or altering the saved window partition.
        checked_group_grant(policy.windows.max_seed_bytes, policy.max_windows_per_batch)?;
        let started = Instant::now();
        let mut pending = Vec::new();
        let mut inputs = 0usize;
        let mut row = first;
        let mut cursor = cursors[row].clone();
        while pending.len() < policy.max_windows_per_batch && row < sources.len() {
            check(cancel)?;
            if cursor.is_exhausted() {
                row += 1;
                if row == sources.len() {
                    break;
                }
                cursor = cursors[row].clone();
                continue;
            }
            let plan = sources[row]
                .plan_window(&cursor, cancel)
                .map_err(window_error)?
                .ok_or_else(|| {
                    AppError::internal_invariant("unfinished finite cursor has no window")
                })?;
            inputs = inputs
                .checked_add(plan.target_count())
                .ok_or_else(|| AppError::limit("finite group input count overflow"))?;
            if inputs > target_grant {
                return Err(AppError::internal_invariant(
                    "finite group exceeds admitted seed grant",
                ));
            }
            pending
                .try_reserve(1)
                .map_err(|_| AppError::limit("finite window group allocation"))?;
            let ordinal = cursor.windows();
            cursor = plan.successor().clone();
            pending.push(PendingWindow { row, ordinal, plan });
        }
        if pending.is_empty() {
            return Err(AppError::internal_invariant(
                "empty unfinished finite window group",
            ));
        }
        let planning_seconds = started.elapsed().as_secs_f64();
        let parallel = pool.is_some() && pending.len() > 1;
        observer(
            json!({"event":"finite_backend_group_materializing","row":first,
            "window":pending[0].ordinal,"windows_count":pending.len(),
            "inputs":inputs,"parallel":parallel,"planning_seconds":planning_seconds}),
        );
        check(cancel)?;
        let materialization_started = Instant::now();
        let materialize = |pending: PendingWindow<N>| {
            sources[pending.row]
                .materialize_window(pending.plan, cancel)
                .map(|window| PlannedWindow {
                    row: pending.row,
                    ordinal: pending.ordinal,
                    window,
                })
                .map_err(window_error)
        };
        let mut windows = Vec::new();
        windows
            .try_reserve_exact(pending.len())
            .map_err(|_| AppError::limit("finite prepared window descriptors allocation"))?;
        if let Some(pool) = pool.filter(|_| parallel) {
            // Results retain exact source order, including the lowest-ordinal
            // failure. No completed worker result can advance a saved cursor.
            // There are two O(group-size) descriptor buffers while collecting
            // results, but each target Vec is moved, never copied. The native
            // RSS guard covers total allocation beyond the seed payload grant.
            let results = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pool.map_ordered(pending, materialize)
            }))
            .map_err(|_| AppError::execution("finite parallel window preparation panicked"))?
            .map_err(|error| AppError::execution(error.to_string()))?
            .into_iter();
            for result in results {
                windows.push(result?);
            }
        } else {
            for pending in pending {
                windows.push(materialize(pending)?);
            }
        }
        check(cancel)?;
        let materialization_seconds = materialization_started.elapsed().as_secs_f64();
        // Distinct starts and input occurrences differ for overlapping rows.
        // Borrow keys: never duplicate the complete resident Work index or
        // allocate another copy of the seed polynomials/coordinates.
        // Prefix windows within one row are disjoint by construction.
        let distinct_inputs = if windows.first().unwrap().row == windows.last().unwrap().row {
            inputs
        } else {
            windows
                .iter()
                .flat_map(|w| w.window.targets())
                .collect::<BTreeSet<&IntegralKey>>()
                .len()
        };
        Ok(Self {
            windows,
            inputs,
            distinct_inputs,
            planning_seconds,
            materialization_seconds,
        })
    }

    pub fn first(&self) -> &PlannedWindow<N> {
        &self.windows[0]
    }
    pub fn last(&self) -> &PlannedWindow<N> {
        self.windows.last().unwrap()
    }
    pub fn metadata(&self) -> Value {
        json!({"windows_count":self.windows.len(),"last_row":self.last().row,
            "last_window":self.last().ordinal,"inputs":self.inputs,
            "distinct_inputs":self.distinct_inputs,"planning_seconds":self.planning_seconds,
            "materialization_seconds":self.materialization_seconds})
    }
}

pub(super) fn checked_group_grant(per_window: usize, windows: usize) -> Result<usize, AppError> {
    per_window
        .checked_mul(windows)
        .ok_or_else(|| AppError::limit("finite grouped seed allowance overflow"))
}
