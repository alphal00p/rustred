//! Look ahead over existing deterministic prefix windows without adopting a
//! cursor. All proposed windows share one native traversal and durable commit.
use super::super::windows::FiniteWindow;
use super::*;
use rustred::family::IntegralKey;
use std::collections::BTreeSet;

pub(super) struct PlannedWindow<const N: usize> {
    pub row: usize,
    pub ordinal: u64,
    pub window: FiniteWindow<N>,
}

pub(super) struct WindowGroup<const N: usize> {
    pub windows: Vec<PlannedWindow<N>>,
    pub inputs: usize,
    pub distinct_inputs: usize,
}

impl<const N: usize> WindowGroup<N> {
    pub fn plan(
        sources: &[PrefixWindows<N>],
        cursors: &[PrefixCursor],
        first: usize,
        policy: Policy,
        cancel: &AtomicBool,
    ) -> Result<Self, AppError> {
        let target_grant =
            checked_group_grant(policy.windows.max_seed_points, policy.max_windows_per_batch)?;
        // Per-window enumeration independently enforces its byte allowance.
        // The checked product also bounds retained group seed payload without
        // introducing a new fixed cap or altering the saved window partition.
        checked_group_grant(policy.windows.max_seed_bytes, policy.max_windows_per_batch)?;
        let mut windows = Vec::new();
        let mut inputs = 0usize;
        let mut row = first;
        let mut cursor = cursors[row].clone();
        while windows.len() < policy.max_windows_per_batch && row < sources.len() {
            check(cancel)?;
            if cursor.is_exhausted() {
                row += 1;
                if row == sources.len() {
                    break;
                }
                cursor = cursors[row].clone();
                continue;
            }
            let window = sources[row]
                .next_window(&cursor, cancel)
                .map_err(window_error)?
                .ok_or_else(|| {
                    AppError::internal_invariant("unfinished finite cursor has no window")
                })?;
            inputs = inputs
                .checked_add(window.targets().len())
                .ok_or_else(|| AppError::limit("finite group input count overflow"))?;
            if inputs > target_grant {
                return Err(AppError::internal_invariant(
                    "finite group exceeds admitted seed grant",
                ));
            }
            windows
                .try_reserve(1)
                .map_err(|_| AppError::limit("finite window group allocation"))?;
            let ordinal = cursor.windows();
            cursor = window.successor().clone();
            windows.push(PlannedWindow {
                row,
                ordinal,
                window,
            });
        }
        if windows.is_empty() {
            return Err(AppError::internal_invariant(
                "empty unfinished finite window group",
            ));
        }
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
            "distinct_inputs":self.distinct_inputs})
    }
}

pub(super) fn checked_group_grant(per_window: usize, windows: usize) -> Result<usize, AppError> {
    per_window
        .checked_mul(windows)
        .ok_or_else(|| AppError::limit("finite grouped seed allowance overflow"))
}
