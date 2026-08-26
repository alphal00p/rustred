//! Retained clone-on-stage Symbolica sparse reduction over `K(n)`.
//!
//! The committed reducer is immutable during a stage. Every trial forks the
//! complete Symbolica `SparseRowReducer`, inserts columns in Symbolica's old
//! coordinate convention, and submits one candidate. Only an independent
//! trial is returned as an owning successor. Dependent, empty, rejected, and
//! failed trials are discarded, so retries and sibling trials observe the
//! same committed native state.

use std::sync::Arc;

use symbolica::domains::SelfRing;
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use super::{
    CheckedParametricField, ParametricCoefficientContext, ParametricCoefficientWorkLedgerLimits,
    ParametricCoefficientWorkStats, SymbolicaParametricSparseError,
    SymbolicaParametricSparseInputRow, SymbolicaParametricSparseReduction,
    SymbolicaParametricSparseRow, call_native, check_limit, copy_input_row, copy_native_row,
    decode_reductions, native_row_len, validate_row,
};

/// Per-stage resource envelope for one retained reducer fork.
///
/// These counters describe a clone-on-stage transaction, not a reconstruction:
/// prior native entries are forked once and no historical input rows are
/// replayed. Coefficient work includes candidate ingress, native field
/// callbacks, and returned-row copies only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SymbolicaPersistentSparseLimits {
    pub(crate) coefficient_work: ParametricCoefficientWorkLedgerLimits,
    pub(crate) max_new_columns: usize,
    pub(crate) max_physical_columns_after: usize,
    pub(crate) max_independent_rows_after: usize,
    pub(crate) max_candidate_input_entries: usize,
    pub(crate) max_retained_native_entries_before_clone: usize,
    /// Conservative `U+L` envelope after the candidate, admitted before the
    /// native fork is mutated.
    pub(crate) max_prospective_native_output_entries: usize,
    /// Exact observed `U+L` fill admitted after native execution.
    pub(crate) max_observed_native_output_entries: usize,
    pub(crate) max_returned_trace_entries: usize,
}

impl Default for SymbolicaPersistentSparseLimits {
    fn default() -> Self {
        Self {
            coefficient_work: ParametricCoefficientWorkLedgerLimits::default(),
            max_new_columns: 16_000_000,
            max_physical_columns_after: 16_000_000,
            max_independent_rows_after: 16_000_000,
            max_candidate_input_entries: 1_000_000_000,
            max_retained_native_entries_before_clone: 1_000_000_000,
            max_prospective_native_output_entries: 1_000_000_000,
            max_observed_native_output_entries: 1_000_000_000,
            max_returned_trace_entries: 16_000_000,
        }
    }
}

/// Exact census of one retained clone-on-stage transaction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SymbolicaPersistentSparseStats {
    physical_columns_before: usize,
    physical_columns_after: usize,
    independent_rows_before: usize,
    independent_rows_after: usize,
    inserted_columns: usize,
    candidate_input_entries: usize,
    retained_native_entries_before_clone: usize,
    prospective_native_output_entries: usize,
    observed_native_output_entries: usize,
    /// Trial U fill. It belongs to the successor only in the independent arm.
    trial_native_u_entries_after: usize,
    /// Trial L fill. A dependent arm includes its discarded factor row.
    trial_native_l_entries_after: usize,
    returned_trace_entries: usize,
    coefficient_work: ParametricCoefficientWorkStats,
}

impl SymbolicaPersistentSparseStats {
    pub(crate) const fn physical_columns_before(self) -> usize {
        self.physical_columns_before
    }

    pub(crate) const fn physical_columns_after(self) -> usize {
        self.physical_columns_after
    }

    pub(crate) const fn independent_rows_before(self) -> usize {
        self.independent_rows_before
    }

    pub(crate) const fn independent_rows_after(self) -> usize {
        self.independent_rows_after
    }

    pub(crate) const fn inserted_columns(self) -> usize {
        self.inserted_columns
    }

    pub(crate) const fn candidate_input_entries(self) -> usize {
        self.candidate_input_entries
    }

    pub(crate) const fn retained_native_entries_before_clone(self) -> usize {
        self.retained_native_entries_before_clone
    }

    pub(crate) const fn prospective_native_output_entries(self) -> usize {
        self.prospective_native_output_entries
    }

    pub(crate) const fn observed_native_output_entries(self) -> usize {
        self.observed_native_output_entries
    }

    pub(crate) const fn trial_native_u_entries_after(self) -> usize {
        self.trial_native_u_entries_after
    }

    pub(crate) const fn trial_native_l_entries_after(self) -> usize {
        self.trial_native_l_entries_after
    }

    pub(crate) const fn returned_trace_entries(self) -> usize {
        self.returned_trace_entries
    }

    pub(crate) const fn coefficient_work(self) -> ParametricCoefficientWorkStats {
        self.coefficient_work
    }
}

/// Result of one forked stage. Only the independent arm owns a successor.
#[derive(Debug)]
pub(crate) enum SymbolicaPersistentSparseOutcome {
    Dependent {
        reductions: Vec<SymbolicaParametricSparseReduction>,
        canonical_zero_input: bool,
        stats: SymbolicaPersistentSparseStats,
    },
    Independent {
        successor: SymbolicaPersistentSparseReducer,
        pivot_column: usize,
        normalized_row: SymbolicaParametricSparseRow,
        reductions: Vec<SymbolicaParametricSparseReduction>,
        normalization_divisor: super::ParametricCoefficient,
        stats: SymbolicaPersistentSparseStats,
    },
}

impl SymbolicaPersistentSparseOutcome {
    pub(crate) fn reductions(&self) -> &[SymbolicaParametricSparseReduction] {
        match self {
            Self::Dependent { reductions, .. } | Self::Independent { reductions, .. } => reductions,
        }
    }

    pub(crate) const fn stats(&self) -> SymbolicaPersistentSparseStats {
        match self {
            Self::Dependent { stats, .. } | Self::Independent { stats, .. } => *stats,
        }
    }
}

/// One committed Symbolica forward reducer with a permanently final sentinel.
///
/// The type intentionally does not implement `Clone`; all forks pass through
/// [`Self::try_stage_row`] so structural and coefficient-work limits remain
/// observable.
#[derive(Debug)]
pub(crate) struct SymbolicaPersistentSparseReducer {
    native: SparseRowReducer<CheckedParametricField>,
    physical_columns: usize,
}

impl SymbolicaPersistentSparseReducer {
    /// Construct an empty retained reducer over one already admitted context
    /// allocation. `physical_columns` excludes the internal final sentinel.
    pub(crate) fn try_new(
        context: Arc<ParametricCoefficientContext>,
        physical_columns: usize,
        limits: SymbolicaPersistentSparseLimits,
    ) -> Result<Self, SymbolicaParametricSparseError> {
        check_limit(
            "persistent Symbolica sparse physical columns",
            physical_columns,
            limits.max_physical_columns_after,
        )?;
        let native_columns = physical_columns.checked_add(1).ok_or(
            SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "persistent Symbolica sparse columns including sentinel",
            },
        )?;
        let native_columns = u32::try_from(native_columns)
            .map_err(|_| SymbolicaParametricSparseError::DimensionOverflow)?;
        let native = call_native("persistent reducer construction", || {
            let field = CheckedParametricField::new(context);
            SparseRowReducer::new(native_columns, field, LuLMode::Full)
        })?;
        Ok(Self {
            native,
            physical_columns,
        })
    }

    pub(crate) const fn physical_columns(&self) -> usize {
        self.physical_columns
    }

    pub(crate) fn independent_rows(&self) -> usize {
        self.native.u().nrows() as usize
    }

    pub(crate) fn native_u_entries(&self) -> usize {
        self.native.u().nvalues()
    }

    pub(crate) fn native_l_entries(&self) -> usize {
        self.native.l().nvalues()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.native.u().field().context.fingerprint()
    }

    /// Fork the committed reducer, insert physical columns at ordered old
    /// coordinates, and forward-reduce one candidate expressed in the new
    /// coordinates. Equal insertion positions are valid and preserve their
    /// supplied order. Position `physical_columns` inserts immediately before
    /// the sentinel.
    pub(crate) fn try_stage_row(
        &self,
        old_coordinate_insertions: &[usize],
        candidate: &SymbolicaParametricSparseInputRow<'_>,
        limits: SymbolicaPersistentSparseLimits,
    ) -> Result<SymbolicaPersistentSparseOutcome, SymbolicaParametricSparseError> {
        self.validate_committed_state()?;
        check_limit(
            "persistent Symbolica sparse inserted columns",
            old_coordinate_insertions.len(),
            limits.max_new_columns,
        )?;
        let physical_columns_after = self
            .physical_columns
            .checked_add(old_coordinate_insertions.len())
            .ok_or(SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "persistent Symbolica sparse physical columns after insertion",
            })?;
        check_limit(
            "persistent Symbolica sparse physical columns after insertion",
            physical_columns_after,
            limits.max_physical_columns_after,
        )?;
        let native_columns_after = physical_columns_after.checked_add(1).ok_or(
            SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "persistent Symbolica sparse columns including sentinel after insertion",
            },
        )?;
        u32::try_from(native_columns_after)
            .map_err(|_| SymbolicaParametricSparseError::DimensionOverflow)?;

        let mut insertion_columns = Vec::new();
        insertion_columns
            .try_reserve_exact(old_coordinate_insertions.len())
            .map_err(|_| SymbolicaParametricSparseError::AllocationFailure {
                resource: "persistent Symbolica sparse insertion coordinates",
            })?;
        let mut previous = None;
        for (ordinal, &position) in old_coordinate_insertions.iter().enumerate() {
            if position > self.physical_columns {
                return Err(SymbolicaParametricSparseError::ColumnInsertionOutOfRange {
                    ordinal,
                    position,
                    physical_columns: self.physical_columns,
                });
            }
            if let Some(previous) = previous {
                if position < previous {
                    return Err(SymbolicaParametricSparseError::DecreasingColumnInsertions {
                        previous,
                        current: position,
                    });
                }
            }
            insertion_columns.push(
                u32::try_from(position)
                    .map_err(|_| SymbolicaParametricSparseError::DimensionOverflow)?,
            );
            previous = Some(position);
        }

        check_limit(
            "persistent Symbolica sparse candidate input entries",
            candidate.entries.len(),
            limits.max_candidate_input_entries,
        )?;
        validate_row(
            self.native.u().field().context.as_ref(),
            physical_columns_after,
            self.independent_rows(),
            candidate,
            limits.coefficient_work.arithmetic.exact_algebra,
        )?;
        // New-column coverage is part of the staged transcript contract, not
        // a resource-admission decision. Authenticate it before any
        // prospective-fill limit can mask malformed final coordinates.
        let has_inserted_columns =
            validate_inserted_column_coverage(candidate, old_coordinate_insertions)?;

        let independent_rows_before = self.independent_rows();
        let potential_independent_rows_after = independent_rows_before.checked_add(1).ok_or(
            SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "persistent Symbolica sparse independent rows",
            },
        )?;
        u32::try_from(potential_independent_rows_after)
            .map_err(|_| SymbolicaParametricSparseError::DimensionOverflow)?;
        // Admit the committed rank now. A dependent or empty trial does not
        // consume another independent row. The potential `+1` is checked only
        // after Symbolica has classified an independent trial, which is then
        // discarded if it exceeds this exact post-stage limit.
        check_limit(
            "retained persistent Symbolica sparse independent rows before stage",
            independent_rows_before,
            limits.max_independent_rows_after,
        )?;

        let retained_native_entries_before_clone = self
            .native
            .u()
            .nvalues()
            .checked_add(self.native.l().nvalues())
            .ok_or(SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "retained persistent Symbolica sparse native entries",
            })?;
        check_limit(
            "retained persistent Symbolica sparse native entries before clone",
            retained_native_entries_before_clone,
            limits.max_retained_native_entries_before_clone,
        )?;
        let maximum_candidate_native_entries = if candidate.is_empty() {
            0
        } else {
            physical_columns_after
                .checked_add(independent_rows_before)
                .and_then(|value| value.checked_add(1))
                .ok_or(SymbolicaParametricSparseError::ResourceCountOverflow {
                    resource: "prospective persistent Symbolica sparse candidate native entries",
                })?
        };
        let prospective_native_output_entries = retained_native_entries_before_clone
            .checked_add(maximum_candidate_native_entries)
            .ok_or(SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "prospective persistent Symbolica sparse native output entries",
            })?;
        check_limit(
            "prospective persistent Symbolica sparse native output entries",
            prospective_native_output_entries,
            limits.max_prospective_native_output_entries,
        )?;

        // Serialize the whole forked stage. The native clone shares this field
        // controller, but never shares mutable U/L/pivot/scratch storage.
        // Symbolica exposes only infallible Clone/add_cols APIs: allocator OOM
        // may abort the process and is deliberately not claimed as a typed,
        // recoverable failure by this adapter.
        let field = self.native.u().field().clone();
        let field_stage = field.begin_stage(limits.coefficient_work);
        let mut trial = call_native("persistent reducer clone", || self.native.clone())?;
        if !insertion_columns.is_empty() {
            call_native("persistent column insertion", || {
                trial.add_cols(&insertion_columns)
            })?;
        }
        if !native_state_matches_column_insertions(
            &self.native,
            &trial,
            self.physical_columns,
            old_coordinate_insertions,
        ) {
            return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "persistent column insertion and sentinel preservation",
            });
        }

        let u_rows_before = trial.u().nrows() as usize;
        let l_rows_before = trial.l().nrows() as usize;
        let (candidate_values, candidate_columns) =
            call_native("persistent candidate input copy", || {
                copy_input_row(&field, candidate)
            })??;
        let pivot_column = call_native("persistent candidate forward reduction", || {
            trial.add_row(&candidate_values, &candidate_columns)
        })?;

        if trial
            .pivots()
            .get(physical_columns_after)
            .copied()
            .flatten()
            .is_some()
        {
            return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "persistent sentinel-column preservation",
            });
        }
        let observed_native_output_entries =
            trial.u().nvalues().checked_add(trial.l().nvalues()).ok_or(
                SymbolicaParametricSparseError::ResourceCountOverflow {
                    resource: "observed persistent Symbolica sparse native output entries",
                },
            )?;
        validate_observed_within_prospective(
            observed_native_output_entries,
            prospective_native_output_entries,
        )?;
        if !native_history_matches_candidate_delta(
            &self.native,
            &trial,
            self.physical_columns,
            old_coordinate_insertions,
            pivot_column.map(|column| column as usize),
        ) {
            return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "persistent candidate historical native prefix",
            });
        }

        if candidate.is_empty() {
            if pivot_column.is_some()
                || trial.u().nrows() as usize != u_rows_before
                || trial.l().nrows() as usize != l_rows_before
            {
                return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                    operation: "persistent canonical-zero candidate",
                });
            }
            check_limit(
                "observed persistent Symbolica sparse native output entries",
                observed_native_output_entries,
                limits.max_observed_native_output_entries,
            )?;
            let stats = persistent_stats(
                &field,
                self.physical_columns,
                physical_columns_after,
                independent_rows_before,
                independent_rows_before,
                old_coordinate_insertions.len(),
                candidate.entries.len(),
                retained_native_entries_before_clone,
                prospective_native_output_entries,
                observed_native_output_entries,
                &trial,
                0,
            );
            drop(field_stage);
            return Ok(SymbolicaPersistentSparseOutcome::Dependent {
                reductions: Vec::new(),
                canonical_zero_input: true,
                stats,
            });
        }

        if trial.l().nrows() as usize != l_rows_before + 1 {
            return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "persistent candidate L row",
            });
        }
        match pivot_column {
            None => {
                if has_inserted_columns {
                    return Err(SymbolicaParametricSparseError::NewColumnDependentCandidate);
                }
                if trial.u().nrows() as usize != u_rows_before {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent dependent candidate U dimensions",
                    });
                }
                if trial.l().ncols() as usize != independent_rows_before
                    || !native_pivots_match_u(&trial, physical_columns_after)
                {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent dependent candidate pivots and L dimensions",
                    });
                }
                check_limit(
                    "observed persistent Symbolica sparse native output entries",
                    observed_native_output_entries,
                    limits.max_observed_native_output_entries,
                )?;
                let returned_trace_entries = native_row_len(trial.l(), l_rows_before)?;
                check_limit(
                    "persistent Symbolica sparse returned trace entries",
                    returned_trace_entries,
                    limits.max_returned_trace_entries,
                )?;
                let l_row = call_native("persistent candidate L extraction", || {
                    copy_native_row(&field, trial.l(), l_rows_before)
                })??;
                let reductions = decode_reductions(l_row, u_rows_before)?;
                if reductions.len() != returned_trace_entries {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent dependent returned trace length",
                    });
                }
                let stats = persistent_stats(
                    &field,
                    self.physical_columns,
                    physical_columns_after,
                    independent_rows_before,
                    independent_rows_before,
                    old_coordinate_insertions.len(),
                    candidate.entries.len(),
                    retained_native_entries_before_clone,
                    prospective_native_output_entries,
                    observed_native_output_entries,
                    &trial,
                    returned_trace_entries,
                );
                drop(field_stage);
                Ok(SymbolicaPersistentSparseOutcome::Dependent {
                    reductions,
                    canonical_zero_input: false,
                    stats,
                })
            }
            Some(pivot_column) => {
                let pivot_column = pivot_column as usize;
                if pivot_column >= physical_columns_after
                    || trial.u().nrows() as usize != u_rows_before + 1
                    || trial.pivots().get(pivot_column).copied().flatten()
                        != Some(u_rows_before as u32)
                {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent independent candidate pivot",
                    });
                }
                check_limit(
                    "persistent Symbolica sparse independent rows after successful stage",
                    potential_independent_rows_after,
                    limits.max_independent_rows_after,
                )?;
                check_limit(
                    "observed persistent Symbolica sparse native output entries",
                    observed_native_output_entries,
                    limits.max_observed_native_output_entries,
                )?;
                let l_trace_entries = native_row_len(trial.l(), l_rows_before)?;
                let u_trace_entries = native_row_len(trial.u(), u_rows_before)?;
                let returned_trace_entries = l_trace_entries.checked_add(u_trace_entries).ok_or(
                    SymbolicaParametricSparseError::ResourceCountOverflow {
                        resource: "persistent Symbolica sparse returned trace entries",
                    },
                )?;
                check_limit(
                    "persistent Symbolica sparse returned trace entries",
                    returned_trace_entries,
                    limits.max_returned_trace_entries,
                )?;
                if trial.l().ncols() as usize != potential_independent_rows_after
                    || !native_pivots_match_u(&trial, physical_columns_after)
                {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent independent candidate pivots and L dimensions",
                    });
                }
                let mut l_row = call_native("persistent candidate L extraction", || {
                    copy_native_row(&field, trial.l(), l_rows_before)
                })??;
                let Some(diagonal) = l_row.pop() else {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent independent candidate L diagonal",
                    });
                };
                if diagonal.column != u_rows_before || diagonal.coefficient.is_zero() {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent independent candidate L diagonal",
                    });
                }
                let normalization_divisor = diagonal.coefficient;
                let reductions = decode_reductions(l_row, u_rows_before)?;
                let normalized_row = SymbolicaParametricSparseRow::new(call_native(
                    "persistent candidate U extraction",
                    || copy_native_row(&field, trial.u(), u_rows_before),
                )??);
                if normalized_row.entries.first().map(|entry| entry.column) != Some(pivot_column)
                    || normalized_row
                        .entries
                        .first()
                        .is_none_or(|entry| !entry.coefficient.raw.is_one())
                    || normalized_row
                        .entries
                        .iter()
                        .any(|entry| entry.column >= physical_columns_after)
                {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent independent normalized U row",
                    });
                }
                if reductions
                    .len()
                    .checked_add(normalized_row.entries.len())
                    .and_then(|value| value.checked_add(1))
                    != Some(returned_trace_entries)
                {
                    return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                        operation: "persistent independent returned trace length",
                    });
                }
                let stats = persistent_stats(
                    &field,
                    self.physical_columns,
                    physical_columns_after,
                    independent_rows_before,
                    independent_rows_before + 1,
                    old_coordinate_insertions.len(),
                    candidate.entries.len(),
                    retained_native_entries_before_clone,
                    prospective_native_output_entries,
                    observed_native_output_entries,
                    &trial,
                    returned_trace_entries,
                );
                drop(field_stage);
                Ok(SymbolicaPersistentSparseOutcome::Independent {
                    successor: Self {
                        native: trial,
                        physical_columns: physical_columns_after,
                    },
                    pivot_column,
                    normalized_row,
                    reductions,
                    normalization_divisor,
                    stats,
                })
            }
        }
    }

    fn validate_committed_state(&self) -> Result<(), SymbolicaParametricSparseError> {
        let native_columns = self.physical_columns.checked_add(1).ok_or(
            SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "committed persistent Symbolica sparse columns including sentinel",
            },
        )?;
        let independent_rows = self.native.u().nrows() as usize;
        if self.native.u().ncols() as usize != native_columns
            || self.native.pivots().len() != native_columns
            || independent_rows > self.physical_columns
            || self.native.l().nrows() as usize != independent_rows
            || self.native.l().ncols() as usize != independent_rows
            || !native_pivots_match_u(&self.native, self.physical_columns)
            || self
                .native
                .u()
                .col_idcs()
                .iter()
                .any(|&column| column as usize == self.physical_columns)
            || self
                .native
                .pivots()
                .get(self.physical_columns)
                .copied()
                .flatten()
                .is_some()
        {
            return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "committed persistent reducer state",
            });
        }
        Ok(())
    }
}

fn validate_inserted_column_coverage(
    candidate: &SymbolicaParametricSparseInputRow<'_>,
    old_coordinate_insertions: &[usize],
) -> Result<bool, SymbolicaParametricSparseError> {
    let mut candidate_offset = 0;
    for (inserted_before, &old_position) in old_coordinate_insertions.iter().enumerate() {
        let new_position = old_position.checked_add(inserted_before).ok_or(
            SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "persistent Symbolica sparse inserted new-coordinate position",
            },
        )?;
        while candidate
            .entries
            .get(candidate_offset)
            .is_some_and(|entry| entry.column < new_position)
        {
            candidate_offset += 1;
        }
        if !candidate
            .entries
            .get(candidate_offset)
            .is_some_and(|entry| entry.column == new_position)
        {
            return Err(
                SymbolicaParametricSparseError::MissingInsertedColumnCandidateEntry {
                    ordinal: inserted_before,
                    final_column: new_position,
                },
            );
        }
    }
    Ok(!old_coordinate_insertions.is_empty())
}

fn native_pivots_match_u(
    reducer: &SparseRowReducer<CheckedParametricField>,
    physical_columns: usize,
) -> bool {
    if reducer.pivots().len() != physical_columns.saturating_add(1)
        || reducer
            .pivots()
            .get(physical_columns)
            .copied()
            .flatten()
            .is_some()
        || reducer
            .pivots()
            .iter()
            .filter(|pivot| pivot.is_some())
            .count()
            != reducer.u().nrows() as usize
    {
        return false;
    }
    reducer
        .u()
        .row_ptrs()
        .windows(2)
        .enumerate()
        .all(|(row_ordinal, row_bounds)| {
            let start = row_bounds[0];
            let end = row_bounds[1];
            let Some(&pivot_column) = reducer.u().col_idcs().get(start) else {
                return false;
            };
            start < end
                && reducer
                    .u()
                    .values()
                    .get(start)
                    .is_some_and(|value| value.0.raw.is_one())
                && reducer.u().col_idcs()[start..end]
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                && reducer.u().col_idcs()[start..end]
                    .iter()
                    .all(|&column| (column as usize) < physical_columns)
                && reducer
                    .pivots()
                    .get(pivot_column as usize)
                    .copied()
                    .flatten()
                    == Some(row_ordinal as u32)
        })
}

fn old_column_after_insertions(
    old_column: usize,
    old_coordinate_insertions: &[usize],
) -> Option<usize> {
    old_column.checked_add(
        old_coordinate_insertions
            .iter()
            .take_while(|&&position| position <= old_column)
            .count(),
    )
}

fn native_state_matches_column_insertions(
    committed: &SparseRowReducer<CheckedParametricField>,
    trial: &SparseRowReducer<CheckedParametricField>,
    physical_columns_before: usize,
    old_coordinate_insertions: &[usize],
) -> bool {
    let Some(physical_columns_after) =
        physical_columns_before.checked_add(old_coordinate_insertions.len())
    else {
        return false;
    };
    let Some(native_columns_after) = physical_columns_after.checked_add(1) else {
        return false;
    };
    if trial.u().ncols() as usize != native_columns_after
        || trial.pivots().len() != native_columns_after
        || trial.u().nrows() != committed.u().nrows()
        || trial.l().nrows() != committed.l().nrows()
        || trial.u().nvalues() != committed.u().nvalues()
        || trial.l().nvalues() != committed.l().nvalues()
        || trial.u().row_ptrs() != committed.u().row_ptrs()
        || trial.u().values() != committed.u().values()
        || trial.l() != committed.l()
    {
        return false;
    }

    if !committed
        .u()
        .col_idcs()
        .iter()
        .zip(trial.u().col_idcs())
        .all(|(&old_column, &new_column)| {
            old_column_after_insertions(old_column as usize, old_coordinate_insertions)
                == Some(new_column as usize)
        })
    {
        return false;
    }

    for (old_column, old_pivot) in committed.pivots().iter().enumerate() {
        let Some(new_column) = old_column_after_insertions(old_column, old_coordinate_insertions)
        else {
            return false;
        };
        if trial.pivots().get(new_column) != Some(old_pivot) {
            return false;
        }
    }
    for (ordinal, &old_position) in old_coordinate_insertions.iter().enumerate() {
        let Some(new_column) = old_position.checked_add(ordinal) else {
            return false;
        };
        if trial.pivots().get(new_column).copied().flatten().is_some() {
            return false;
        }
    }
    trial
        .pivots()
        .get(physical_columns_after)
        .copied()
        .flatten()
        .is_none()
}

/// Authenticate that a native candidate trial appended at most one new U/L
/// row and did not rewrite any committed algebra. Column insertions remap U
/// coordinates and pivots; historical L columns are row ordinals and remain
/// byte-for-byte stable.
fn native_history_matches_candidate_delta(
    committed: &SparseRowReducer<CheckedParametricField>,
    trial: &SparseRowReducer<CheckedParametricField>,
    physical_columns_before: usize,
    old_coordinate_insertions: &[usize],
    candidate_pivot: Option<usize>,
) -> bool {
    let Some(physical_columns_after) =
        physical_columns_before.checked_add(old_coordinate_insertions.len())
    else {
        return false;
    };
    let Some(native_columns_after) = physical_columns_after.checked_add(1) else {
        return false;
    };
    let historical_rows = committed.u().nrows() as usize;
    let Ok(candidate_pivot_row) = u32::try_from(historical_rows) else {
        return false;
    };
    if trial.u().ncols() as usize != native_columns_after
        || trial.pivots().len() != native_columns_after
        || trial.u().nrows() < committed.u().nrows()
        || trial.l().nrows() < committed.l().nrows()
        || candidate_pivot.is_some_and(|column| column >= physical_columns_after)
        || trial.u().col_idcs().len() < committed.u().col_idcs().len()
        || !trial.u().row_ptrs().starts_with(committed.u().row_ptrs())
        || !trial.u().values().starts_with(committed.u().values())
        || !trial.l().row_ptrs().starts_with(committed.l().row_ptrs())
        || !trial.l().col_idcs().starts_with(committed.l().col_idcs())
        || !trial.l().values().starts_with(committed.l().values())
    {
        return false;
    }

    if !committed
        .u()
        .col_idcs()
        .iter()
        .zip(trial.u().col_idcs())
        .all(|(&old_column, &new_column)| {
            old_column_after_insertions(old_column as usize, old_coordinate_insertions)
                == Some(new_column as usize)
        })
    {
        return false;
    }

    // Remapped old coordinates and inserted coordinates partition the complete
    // final pivot vector. Comparing both sets therefore rejects extra pivots,
    // missing pivots, and any rewrite of a historical row mapping.
    for (old_column, &old_pivot) in committed.pivots().iter().enumerate() {
        let Some(final_column) = old_column_after_insertions(old_column, old_coordinate_insertions)
        else {
            return false;
        };
        let expected = if candidate_pivot == Some(final_column) {
            if old_pivot.is_some() {
                return false;
            }
            Some(candidate_pivot_row)
        } else {
            old_pivot
        };
        if trial.pivots().get(final_column).copied().flatten() != expected {
            return false;
        }
    }
    for (ordinal, &old_position) in old_coordinate_insertions.iter().enumerate() {
        let Some(final_column) = old_position.checked_add(ordinal) else {
            return false;
        };
        let expected = (candidate_pivot == Some(final_column)).then_some(candidate_pivot_row);
        if trial.pivots().get(final_column).copied().flatten() != expected {
            return false;
        }
    }
    true
}

fn validate_observed_within_prospective(
    observed: usize,
    prospective: usize,
) -> Result<(), SymbolicaParametricSparseError> {
    if observed > prospective {
        return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
            operation: "persistent prospective native output envelope",
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn persistent_stats(
    field: &CheckedParametricField,
    physical_columns_before: usize,
    physical_columns_after: usize,
    independent_rows_before: usize,
    independent_rows_after: usize,
    inserted_columns: usize,
    candidate_input_entries: usize,
    retained_native_entries_before_clone: usize,
    prospective_native_output_entries: usize,
    observed_native_output_entries: usize,
    reducer: &SparseRowReducer<CheckedParametricField>,
    returned_trace_entries: usize,
) -> SymbolicaPersistentSparseStats {
    debug_assert_eq!(reducer.u().nrows() as usize, independent_rows_after);
    debug_assert_eq!(
        reducer.u().nvalues().checked_add(reducer.l().nvalues()),
        Some(observed_native_output_entries)
    );
    SymbolicaPersistentSparseStats {
        physical_columns_before,
        physical_columns_after,
        independent_rows_before,
        independent_rows_after,
        inserted_columns,
        candidate_input_entries,
        retained_native_entries_before_clone,
        prospective_native_output_entries,
        observed_native_output_entries,
        trial_native_u_entries_after: reducer.u().nvalues(),
        trial_native_l_entries_after: reducer.l().nvalues(),
        returned_trace_entries,
        coefficient_work: field.stats(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoefficientContext;
    use crate::parametric_coefficient::symbolica_sparse::{
        SymbolicaParametricSparseEntry, SymbolicaParametricSparseLimits,
        SymbolicaParametricSparseOutcome, SymbolicaParametricSparseRow, forward_reduce_last_row,
    };

    fn context(scope: &str) -> Arc<ParametricCoefficientContext> {
        Arc::new(
            ParametricCoefficientContext::try_new(&CoefficientContext::new(["d"]), scope, 1)
                .unwrap(),
        )
    }

    fn entry(
        column: usize,
        coefficient: super::super::ParametricCoefficient,
    ) -> SymbolicaParametricSparseEntry {
        SymbolicaParametricSparseEntry::new(column, coefficient)
    }

    fn row(entries: Vec<SymbolicaParametricSparseEntry>) -> SymbolicaParametricSparseRow {
        SymbolicaParametricSparseRow::new(entries)
    }

    fn stage(
        reducer: &SymbolicaPersistentSparseReducer,
        insertions: &[usize],
        candidate: &SymbolicaParametricSparseRow,
        limits: SymbolicaPersistentSparseLimits,
    ) -> Result<SymbolicaPersistentSparseOutcome, SymbolicaParametricSparseError> {
        reducer.try_stage_row(insertions, &candidate.try_as_input()?, limits)
    }

    fn independent(
        outcome: SymbolicaPersistentSparseOutcome,
    ) -> (
        SymbolicaPersistentSparseReducer,
        usize,
        SymbolicaParametricSparseRow,
        Vec<SymbolicaParametricSparseReduction>,
        super::super::ParametricCoefficient,
        SymbolicaPersistentSparseStats,
    ) {
        let SymbolicaPersistentSparseOutcome::Independent {
            successor,
            pivot_column,
            normalized_row,
            reductions,
            normalization_divisor,
            stats,
        } = outcome
        else {
            panic!("candidate must be independent")
        };
        (
            successor,
            pivot_column,
            normalized_row,
            reductions,
            normalization_divisor,
            stats,
        )
    }

    struct MatchedIndependentStage {
        successor: SymbolicaPersistentSparseReducer,
        pivot_column: usize,
        normalized_row: SymbolicaParametricSparseRow,
        reduction_rows: Vec<usize>,
        reduction_factors: Vec<super::super::ParametricCoefficient>,
        normalization_divisor: super::super::ParametricCoefficient,
    }

    enum MatchedStage {
        Dependent {
            canonical_zero_input: bool,
            reduction_rows: Vec<usize>,
            reduction_factors: Vec<super::super::ParametricCoefficient>,
        },
        Independent(MatchedIndependentStage),
    }

    fn remap_row_after_insertions(
        source: &SymbolicaParametricSparseRow,
        old_coordinate_insertions: &[usize],
    ) -> SymbolicaParametricSparseRow {
        row(source
            .entries()
            .iter()
            .map(|source_entry| {
                let inserted_before_or_at = old_coordinate_insertions
                    .iter()
                    .take_while(|&&position| position <= source_entry.column())
                    .count();
                entry(
                    source_entry.column() + inserted_before_or_at,
                    source_entry.coefficient().clone(),
                )
            })
            .collect())
    }

    fn stage_and_match_rebuild(
        context: &Arc<ParametricCoefficientContext>,
        retained: &SymbolicaPersistentSparseReducer,
        oracle_prior_rows: &mut Vec<SymbolicaParametricSparseRow>,
        old_coordinate_insertions: &[usize],
        candidate: &SymbolicaParametricSparseRow,
    ) -> MatchedStage {
        let physical_columns_after = retained
            .physical_columns()
            .checked_add(old_coordinate_insertions.len())
            .unwrap();
        let remapped_prior_rows = oracle_prior_rows
            .iter()
            .map(|prior| remap_row_after_insertions(prior, old_coordinate_insertions))
            .collect::<Vec<_>>();
        let prior_inputs = remapped_prior_rows
            .iter()
            .map(SymbolicaParametricSparseRow::try_as_input)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let candidate_input = candidate.try_as_input().unwrap();
        let rebuilt = forward_reduce_last_row(
            context.as_ref(),
            physical_columns_after,
            &prior_inputs,
            &candidate_input,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        let persistent = stage(
            retained,
            old_coordinate_insertions,
            candidate,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();

        match (persistent, rebuilt) {
            (
                SymbolicaPersistentSparseOutcome::Dependent {
                    reductions: persistent_reductions,
                    canonical_zero_input: persistent_zero,
                    ..
                },
                SymbolicaParametricSparseOutcome::Dependent {
                    reductions: rebuilt_reductions,
                    canonical_zero_input: rebuilt_zero,
                    ..
                },
            ) => {
                assert_eq!(persistent_zero, rebuilt_zero);
                assert_eq!(persistent_reductions, rebuilt_reductions);
                assert!(
                    old_coordinate_insertions.is_empty(),
                    "a nonzero inserted column cannot produce a dependent stage"
                );
                MatchedStage::Dependent {
                    canonical_zero_input: persistent_zero,
                    reduction_rows: persistent_reductions
                        .iter()
                        .map(SymbolicaParametricSparseReduction::pivot_row)
                        .collect(),
                    reduction_factors: persistent_reductions
                        .iter()
                        .map(|reduction| reduction.factor().clone())
                        .collect(),
                }
            }
            (
                SymbolicaPersistentSparseOutcome::Independent {
                    successor,
                    pivot_column: persistent_pivot,
                    normalized_row: persistent_row,
                    reductions: persistent_reductions,
                    normalization_divisor: persistent_divisor,
                    ..
                },
                SymbolicaParametricSparseOutcome::Independent {
                    pivot_column: rebuilt_pivot,
                    normalized_row: rebuilt_row,
                    reductions: rebuilt_reductions,
                    normalization_divisor: rebuilt_divisor,
                    ..
                },
            ) => {
                assert_eq!(persistent_pivot, rebuilt_pivot);
                assert_eq!(persistent_row, rebuilt_row);
                assert_eq!(persistent_reductions, rebuilt_reductions);
                assert_eq!(persistent_divisor, rebuilt_divisor);
                let reduction_rows = persistent_reductions
                    .iter()
                    .map(SymbolicaParametricSparseReduction::pivot_row)
                    .collect();
                let reduction_factors = persistent_reductions
                    .iter()
                    .map(|reduction| reduction.factor().clone())
                    .collect();
                *oracle_prior_rows = remapped_prior_rows;
                oracle_prior_rows.push(rebuilt_row);
                MatchedStage::Independent(MatchedIndependentStage {
                    successor,
                    pivot_column: persistent_pivot,
                    normalized_row: persistent_row,
                    reduction_rows,
                    reduction_factors,
                    normalization_divisor: persistent_divisor,
                })
            }
            (persistent, rebuilt) => panic!(
                "persistent and rebuild dispositions differ: persistent={persistent:?}, rebuilt={rebuilt:?}"
            ),
        }
    }

    fn matched_independent(stage: MatchedStage) -> MatchedIndependentStage {
        let MatchedStage::Independent(independent) = stage else {
            panic!("stage must be independent")
        };
        independent
    }

    #[test]
    fn persistent_independent_stages_leave_base_immutable_and_support_siblings() {
        let context = context("persistent-sparse-independent-siblings");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            2,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let first = row(vec![entry(0, context.integer(2)), entry(1, context.one())]);
        let second = row(vec![entry(1, context.integer(3))]);

        let (first_successor, first_pivot, _, _, _, first_stats) = independent(
            stage(
                &base,
                &[],
                &first,
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(first_pivot, 0);
        assert_eq!(base.independent_rows(), 0);
        assert_eq!(base.native_u_entries(), 0);
        assert_eq!(base.native_l_entries(), 0);
        assert_eq!(first_successor.independent_rows(), 1);
        assert_eq!(first_stats.independent_rows_before(), 0);
        assert_eq!(first_stats.independent_rows_after(), 1);

        let (second_successor, second_pivot, _, _, _, _) = independent(
            stage(
                &base,
                &[],
                &second,
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(second_pivot, 1);
        assert_eq!(second_successor.independent_rows(), 1);
        assert_eq!(base.independent_rows(), 0);
        assert_eq!(
            first_successor.context_fingerprint(),
            base.context_fingerprint()
        );
        assert_eq!(
            second_successor.context_fingerprint(),
            base.context_fingerprint()
        );
    }

    #[test]
    fn persistent_dependent_trial_is_discarded_and_retry_is_deterministic() {
        let context = context("persistent-sparse-dependent-retry");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            2,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let first = row(vec![entry(0, context.one()), entry(1, context.one())]);
        let (committed, _, _, _, _, _) = independent(
            stage(
                &base,
                &[],
                &first,
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let dependent = row(vec![
            entry(0, context.integer(2)),
            entry(1, context.integer(2)),
        ]);

        for _ in 0..2 {
            let outcome = stage(
                &committed,
                &[],
                &dependent,
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap();
            let SymbolicaPersistentSparseOutcome::Dependent {
                reductions,
                canonical_zero_input,
                stats,
            } = outcome
            else {
                panic!("candidate must be dependent")
            };
            assert!(!canonical_zero_input);
            assert_eq!(reductions.len(), 1);
            assert_eq!(reductions[0].pivot_row(), 0);
            assert_eq!(reductions[0].factor(), &context.integer(2));
            assert_eq!(stats.independent_rows_before(), 1);
            assert_eq!(stats.independent_rows_after(), 1);
            assert_eq!(committed.independent_rows(), 1);
            assert_eq!(committed.native_l_entries(), 1);
        }

        let sibling = row(vec![entry(1, context.one())]);
        let (successor, pivot, _, _, _, _) = independent(
            stage(
                &committed,
                &[],
                &sibling,
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(pivot, 1);
        assert_eq!(successor.independent_rows(), 2);
        assert_eq!(committed.independent_rows(), 1);
    }

    #[test]
    fn persistent_empty_trial_is_discarded_without_mutating_base() {
        let context = context("persistent-sparse-empty");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context,
            2,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let outcome = stage(
            &base,
            &[],
            &SymbolicaParametricSparseRow::default(),
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let SymbolicaPersistentSparseOutcome::Dependent {
            reductions,
            canonical_zero_input,
            stats,
        } = outcome
        else {
            panic!("empty row must be dependent")
        };
        assert!(canonical_zero_input);
        assert!(reductions.is_empty());
        assert_eq!(stats.physical_columns_before(), 2);
        assert_eq!(stats.physical_columns_after(), 2);
        assert_eq!(stats.inserted_columns(), 0);
        assert_eq!(stats.candidate_input_entries(), 0);
        assert_eq!(stats.retained_native_entries_before_clone(), 0);
        assert_eq!(stats.prospective_native_output_entries(), 0);
        assert_eq!(stats.observed_native_output_entries(), 0);
        assert_eq!(stats.trial_native_u_entries_after(), 0);
        assert_eq!(stats.trial_native_l_entries_after(), 0);
        assert_eq!(stats.returned_trace_entries(), 0);
        assert_eq!(
            stats.coefficient_work(),
            ParametricCoefficientWorkStats::default()
        );
        assert_eq!(base.physical_columns(), 2);
    }

    #[test]
    fn persistent_full_physical_rank_keeps_dependent_sentinel_trace() {
        let context = context("persistent-sparse-full-rank-sentinel");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            2,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let (one, _, _, _, _, _) = independent(
            stage(
                &base,
                &[],
                &row(vec![entry(1, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let (full, _, _, _, _, _) = independent(
            stage(
                &one,
                &[],
                &row(vec![entry(0, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let candidate = row(vec![
            entry(0, context.integer(2)),
            entry(1, context.integer(3)),
        ]);
        let outcome = stage(
            &full,
            &[],
            &candidate,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let SymbolicaPersistentSparseOutcome::Dependent {
            reductions, stats, ..
        } = outcome
        else {
            panic!("full-rank candidate must be dependent")
        };
        assert_eq!(
            reductions
                .iter()
                .map(SymbolicaParametricSparseReduction::pivot_row)
                .collect::<Vec<_>>(),
            vec![1, 0]
        );
        assert_eq!(stats.independent_rows_after(), 2);
        assert_eq!(full.independent_rows(), 2);
        assert_eq!(full.native_l_entries(), 2);
    }

    #[test]
    fn persistent_insertions_accept_front_middle_back_and_duplicate_old_positions() {
        let context = context("persistent-sparse-insertion-layout");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            3,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let (committed, _, _, _, _, _) = independent(
            stage(
                &base,
                &[],
                &row(vec![entry(1, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        // New-coordinate inserted columns are 0, 3, 4, and 6. The old pivot
        // at column 1 moves to column 2; the old sentinel moves to final 7.
        let candidate = row(vec![
            entry(0, context.one()),
            entry(3, context.integer(2)),
            entry(4, context.integer(3)),
            entry(6, context.integer(4)),
        ]);
        let (successor, pivot, normalized, _, _, stats) = independent(
            stage(
                &committed,
                &[0, 2, 2, 3],
                &candidate,
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(pivot, 0);
        assert_eq!(successor.physical_columns(), 7);
        assert_eq!(successor.native.u().ncols(), 8);
        assert_eq!(successor.native.pivots()[2], Some(0));
        assert_eq!(successor.native.pivots()[0], Some(1));
        assert_eq!(successor.native.pivots()[7], None);
        assert_eq!(
            normalized
                .entries()
                .iter()
                .map(SymbolicaParametricSparseEntry::column)
                .collect::<Vec<_>>(),
            vec![0, 3, 4, 6]
        );
        assert_eq!(stats.physical_columns_before(), 3);
        assert_eq!(stats.physical_columns_after(), 7);
        assert_eq!(stats.inserted_columns(), 4);
    }

    #[test]
    fn persistent_insertions_reject_decrease_range_and_native_dimension_overflow() {
        let context = context("persistent-sparse-invalid-insertions");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            3,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let candidate = row(vec![entry(0, context.one())]);
        assert!(matches!(
            stage(
                &base,
                &[2, 1],
                &candidate,
                SymbolicaPersistentSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::DecreasingColumnInsertions {
                previous: 2,
                current: 1,
            })
        ));
        assert!(matches!(
            stage(
                &base,
                &[4],
                &candidate,
                SymbolicaPersistentSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::ColumnInsertionOutOfRange {
                ordinal: 0,
                position: 4,
                physical_columns: 3,
            })
        ));
        assert!(matches!(
            stage(
                &base,
                &[1],
                &candidate,
                SymbolicaPersistentSparseLimits::default(),
            ),
            Err(
                SymbolicaParametricSparseError::MissingInsertedColumnCandidateEntry {
                    ordinal: 0,
                    final_column: 1,
                }
            )
        ));
        let mut masked_by_prospective_limit = SymbolicaPersistentSparseLimits::default();
        masked_by_prospective_limit.max_prospective_native_output_entries = 0;
        assert!(matches!(
            stage(&base, &[1], &candidate, masked_by_prospective_limit,),
            Err(
                SymbolicaParametricSparseError::MissingInsertedColumnCandidateEntry {
                    ordinal: 0,
                    final_column: 1,
                }
            )
        ));
        assert!(matches!(
            stage(
                &base,
                &[0],
                &SymbolicaParametricSparseRow::default(),
                SymbolicaPersistentSparseLimits::default(),
            ),
            Err(
                SymbolicaParametricSparseError::MissingInsertedColumnCandidateEntry {
                    ordinal: 0,
                    final_column: 0,
                }
            )
        ));

        if usize::BITS > u32::BITS {
            let mut limits = SymbolicaPersistentSparseLimits::default();
            limits.max_physical_columns_after = usize::MAX;
            assert!(matches!(
                SymbolicaPersistentSparseReducer::try_new(context, u32::MAX as usize, limits,),
                Err(SymbolicaParametricSparseError::DimensionOverflow)
            ));
        }
        assert_eq!(base.physical_columns(), 3);
        assert_eq!(base.independent_rows(), 0);
    }

    #[test]
    fn persistent_new_column_coverage_requires_every_duplicate_position() {
        let context = context("persistent-sparse-new-column-dependence-guard");
        let one = context.one();
        let insertions = [0, 2, 2, 3];
        for new_column in [0, 3, 4, 6] {
            let candidate = row(vec![entry(new_column, one.clone())]);
            assert!(matches!(
                validate_inserted_column_coverage(&candidate.try_as_input().unwrap(), &insertions,),
                Err(SymbolicaParametricSparseError::MissingInsertedColumnCandidateEntry { .. })
            ));
        }
        let complete = row(vec![
            entry(0, one.clone()),
            entry(3, one.clone()),
            entry(4, one.clone()),
            entry(6, one),
        ]);
        assert!(
            validate_inserted_column_coverage(&complete.try_as_input().unwrap(), &insertions,)
                .unwrap()
        );
    }

    #[test]
    fn persistent_post_native_limit_failure_discards_trial_and_allows_retry() {
        let context = context("persistent-sparse-post-native-limit-retry");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            2,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let candidate = row(vec![entry(0, context.one()), entry(1, context.one())]);
        let pilot = stage(
            &base,
            &[],
            &candidate,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let observed = pilot.stats().observed_native_output_entries();
        assert_eq!(observed, 3);

        let mut rejected = SymbolicaPersistentSparseLimits::default();
        rejected.max_observed_native_output_entries = observed - 1;
        assert!(matches!(
            stage(&base, &[], &candidate, rejected),
            Err(SymbolicaParametricSparseError::ResourceLimit {
                resource: "observed persistent Symbolica sparse native output entries",
                requested: 3,
                limit: 2,
            })
        ));
        assert_eq!(base.independent_rows(), 0);
        assert_eq!(base.native_u_entries(), 0);
        assert_eq!(base.native_l_entries(), 0);

        let retry = stage(
            &base,
            &[],
            &candidate,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let (_, retry_pivot, retry_row, _, retry_divisor, retry_stats) = independent(retry);
        let (_, pilot_pivot, pilot_row, _, pilot_divisor, pilot_stats) = independent(pilot);
        assert_eq!(retry_pivot, pilot_pivot);
        assert_eq!(retry_row, pilot_row);
        assert_eq!(retry_divisor, pilot_divisor);
        assert_eq!(retry_stats, pilot_stats);
    }

    #[test]
    fn persistent_history_authentication_rejects_a_rewritten_native_prefix() {
        let context = context("persistent-sparse-history-prefix-authentication");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            3,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let (committed, _, _, _, _, _) = independent(
            stage(
                &base,
                &[],
                &row(vec![entry(0, context.one()), entry(2, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let (successor, pivot, _, _, _, _) = independent(
            stage(
                &committed,
                &[],
                &row(vec![entry(1, context.one()), entry(2, context.integer(2))]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        assert!(native_history_matches_candidate_delta(
            &committed.native,
            &successor.native,
            committed.physical_columns,
            &[],
            Some(pivot),
        ));

        let mut rewritten = successor.native.clone();
        rewritten.back_substitute();
        assert!(!native_history_matches_candidate_delta(
            &committed.native,
            &rewritten,
            committed.physical_columns,
            &[],
            Some(pivot),
        ));
        assert_eq!(committed.independent_rows(), 1);
    }

    #[test]
    fn persistent_history_authentication_rejects_rewritten_u_coefficients() {
        let context = context("persistent-sparse-history-u-coefficient-authentication");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            3,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let (committed, _, _, _, _, _) = independent(
            stage(
                &base,
                &[],
                &row(vec![entry(0, context.one()), entry(2, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );

        // A separately valid reducer can have identical dimensions, pivots,
        // row pointers, column indices, and historical L while carrying a
        // different normalized historical U coefficient. The prefix check
        // must reject that algebraic rewrite directly, not only through an L
        // or shape mismatch.
        let alternate_base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            3,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let (alternate_history, _, _, _, _, _) = independent(
            stage(
                &alternate_base,
                &[],
                &row(vec![entry(0, context.one()), entry(2, context.integer(2))]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(alternate_history.native.l(), committed.native.l());
        assert_eq!(alternate_history.native.pivots(), committed.native.pivots());
        assert_eq!(
            alternate_history.native.u().row_ptrs(),
            committed.native.u().row_ptrs()
        );
        assert_eq!(
            alternate_history.native.u().col_idcs(),
            committed.native.u().col_idcs()
        );
        assert_ne!(
            alternate_history.native.u().values(),
            committed.native.u().values()
        );
        assert!(!native_history_matches_candidate_delta(
            &committed.native,
            &alternate_history.native,
            committed.physical_columns,
            &[],
            None,
        ));
    }

    #[test]
    fn persistent_impossible_native_fill_precedes_configured_observed_limit() {
        assert!(matches!(
            validate_observed_within_prospective(4, 3),
            Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "persistent prospective native output envelope",
            })
        ));
        assert_eq!(validate_observed_within_prospective(3, 3), Ok(()));
        assert!(matches!(
            check_limit(
                "observed persistent Symbolica sparse native output entries",
                3,
                2,
            ),
            Err(SymbolicaParametricSparseError::ResourceLimit {
                resource: "observed persistent Symbolica sparse native output entries",
                requested: 3,
                limit: 2,
            })
        ));
    }

    #[test]
    fn persistent_rank_limit_admits_dependent_at_boundary_and_discards_independent_overflow() {
        let context = context("persistent-sparse-rank-limit-boundary");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            2,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let (committed, _, _, _, _, _) = independent(
            stage(
                &base,
                &[],
                &row(vec![entry(0, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let mut rank_one = SymbolicaPersistentSparseLimits::default();
        rank_one.max_independent_rows_after = 1;

        assert!(matches!(
            stage(
                &committed,
                &[],
                &row(vec![entry(0, context.integer(2))]),
                rank_one,
            )
            .unwrap(),
            SymbolicaPersistentSparseOutcome::Dependent { .. }
        ));
        let mut rank_and_observed_fill = rank_one;
        rank_and_observed_fill.max_observed_native_output_entries = 0;
        assert!(matches!(
            stage(
                &committed,
                &[],
                &row(vec![entry(1, context.one())]),
                rank_and_observed_fill,
            ),
            Err(SymbolicaParametricSparseError::ResourceLimit {
                resource: "persistent Symbolica sparse independent rows after successful stage",
                requested: 2,
                limit: 1,
            })
        ));
        assert_eq!(committed.independent_rows(), 1);
        assert_eq!(committed.native_l_entries(), 1);
        let (retry, pivot, _, _, _, _) = independent(
            stage(
                &committed,
                &[],
                &row(vec![entry(1, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(pivot, 1);
        assert_eq!(retry.independent_rows(), 2);
    }

    #[test]
    fn persistent_fixed_insertion_sequence_matches_rebuild_oracle_stage_by_stage() {
        let context = context("persistent-sparse-insertion-rebuild-oracle");
        let one = context.one();
        let two = context.integer(2);
        let three = context.integer(3);
        let four = context.integer(4);
        let n = context.index(0).unwrap();
        let d = context.lift(&context.base().parameter_at(0)).unwrap();
        let d_over_n = context.checked_div(&d, &n).unwrap();
        let mut retained = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            2,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let mut oracle_prior_rows = Vec::new();

        // The initial physical catalog is [B, F] easiest-first, hence the
        // native order is [F, B, sentinel].
        let first = matched_independent(stage_and_match_rebuild(
            &context,
            &retained,
            &mut oracle_prior_rows,
            &[],
            &row(vec![entry(0, one.clone()), entry(1, one.clone())]),
        ));
        assert_eq!(first.pivot_column, 0);
        assert!(first.reduction_rows.is_empty());
        assert!(first.reduction_factors.is_empty());
        assert_eq!(first.normalization_divisor, one);
        retained = first.successor;

        // Add hardest G at old native position 0 and middle E at position 1.
        // The merged native order is [G, F, E, B, sentinel].
        let second = matched_independent(stage_and_match_rebuild(
            &context,
            &retained,
            &mut oracle_prior_rows,
            &[0, 1],
            &row(vec![entry(0, one.clone()), entry(2, one.clone())]),
        ));
        assert_eq!(second.pivot_column, 0);
        assert!(second.reduction_rows.is_empty());
        retained = second.successor;

        // 2(G+E) and 3(F+B) cancel in native pivot order. Chronological
        // pivot ordinals are therefore deliberately nonmonotone [1, 0].
        let third_candidate = row(vec![
            entry(0, two.clone()),
            entry(1, three.clone()),
            entry(2, context.add(&n, &two).unwrap()),
            entry(3, context.add(&d, &three).unwrap()),
        ]);
        let third = matched_independent(stage_and_match_rebuild(
            &context,
            &retained,
            &mut oracle_prior_rows,
            &[],
            &third_candidate,
        ));
        assert_eq!(third.pivot_column, 2);
        assert_eq!(third.reduction_rows, vec![1, 0]);
        assert_eq!(third.reduction_factors, vec![two.clone(), three.clone()]);
        assert_eq!(third.normalization_divisor, n);
        assert_eq!(
            third.normalized_row,
            row(vec![entry(2, one.clone()), entry(3, d_over_n.clone())])
        );
        retained = third.successor;

        // Exercise a nonempty dependent row with a rational index-dependent
        // factor. It is exactly 2*row(1) + 3*row(0) + r*row(2).
        let n_plus_one = context.add(&n, &one).unwrap();
        let r = context.checked_div(&n_plus_one, &d).unwrap();
        let r_d_over_n = context.mul(&r, &d_over_n).unwrap();
        let dependent_candidate = row(vec![
            entry(0, two.clone()),
            entry(1, three.clone()),
            entry(2, context.add(&two, &r).unwrap()),
            entry(3, context.add(&three, &r_d_over_n).unwrap()),
        ]);
        let MatchedStage::Dependent {
            canonical_zero_input,
            reduction_rows,
            reduction_factors,
        } = stage_and_match_rebuild(
            &context,
            &retained,
            &mut oracle_prior_rows,
            &[],
            &dependent_candidate,
        )
        else {
            panic!("the explicit row combination must be dependent")
        };
        assert!(!canonical_zero_input);
        assert_eq!(reduction_rows, vec![1, 0, 2]);
        assert_eq!(reduction_factors, vec![two.clone(), three.clone(), r]);
        assert_eq!(retained.independent_rows(), 3);
        assert_eq!(oracle_prior_rows.len(), 3);

        let MatchedStage::Dependent {
            canonical_zero_input,
            reduction_rows,
            reduction_factors,
        } = stage_and_match_rebuild(
            &context,
            &retained,
            &mut oracle_prior_rows,
            &[],
            &SymbolicaParametricSparseRow::default(),
        )
        else {
            panic!("the empty row must be dependent")
        };
        assert!(canonical_zero_input);
        assert!(reduction_rows.is_empty());
        assert!(reduction_factors.is_empty());

        // Extend [B,E,F,G] by front A, same-gap middle C,D, and back H.
        // In native coordinates the hardest-first inserted keys H,D,C,A use
        // Symbolica's old-coordinate positions [0,3,3,4], producing final
        // columns 0,4,5,7 and leaving the sentinel at 8.
        let two_n = context.mul(&two, &n).unwrap();
        let q_numerator = context.sub(&d, &two_n).unwrap();
        let q = context.checked_div(&q_numerator, &n_plus_one).unwrap();
        let insertion_candidate = row(vec![
            entry(0, q.clone()),
            entry(4, context.mul(&two, &q).unwrap()),
            entry(5, context.mul(&three, &q).unwrap()),
            entry(7, context.mul(&four, &q).unwrap()),
        ]);
        let inserted = matched_independent(stage_and_match_rebuild(
            &context,
            &retained,
            &mut oracle_prior_rows,
            &[0, 3, 3, 4],
            &insertion_candidate,
        ));
        assert_eq!(inserted.pivot_column, 0);
        assert!(inserted.reduction_rows.is_empty());
        assert_eq!(inserted.normalization_divisor, q);
        assert_eq!(
            inserted.normalized_row,
            row(vec![
                entry(0, one.clone()),
                entry(4, two.clone()),
                entry(5, three.clone()),
                entry(7, four),
            ])
        );
        retained = inserted.successor;
        assert_eq!(retained.physical_columns(), 8);
        assert_eq!(retained.native.pivots()[8], None);

        // Fill the four remaining physical pivot columns without back
        // substitution; the retained U basis must remain the oracle's forward
        // basis at every stage.
        for column in [4, 5, 6, 7] {
            let filled = matched_independent(stage_and_match_rebuild(
                &context,
                &retained,
                &mut oracle_prior_rows,
                &[],
                &row(vec![entry(column, one.clone())]),
            ));
            assert_eq!(filled.pivot_column, column);
            assert!(filled.reduction_rows.is_empty());
            assert_eq!(filled.normalization_divisor, one);
            retained = filled.successor;
        }
        assert_eq!(retained.independent_rows(), retained.physical_columns());
        assert_eq!(oracle_prior_rows.len(), 8);
        assert_eq!(
            retained.native.pivots(),
            &vec![
                Some(3),
                Some(1),
                Some(0),
                Some(2),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
                None,
            ]
        );

        let full_u = retained.native.u().clone();
        let full_l = retained.native.l().clone();
        let full_pivots = retained.native.pivots().clone();
        let full_rank_candidate = row(vec![
            entry(1, two.clone()),
            entry(2, three.clone()),
            entry(3, two.clone()),
            entry(6, three.clone()),
        ]);
        let MatchedStage::Dependent {
            canonical_zero_input,
            reduction_rows,
            reduction_factors,
        } = stage_and_match_rebuild(
            &context,
            &retained,
            &mut oracle_prior_rows,
            &[],
            &full_rank_candidate,
        )
        else {
            panic!("the full-rank row combination must be dependent")
        };
        assert!(!canonical_zero_input);
        assert_eq!(reduction_rows, vec![1, 0]);
        assert_eq!(reduction_factors, vec![two, three]);
        assert_eq!(retained.native.u(), &full_u);
        assert_eq!(retained.native.l(), &full_l);
        assert_eq!(retained.native.pivots(), &full_pivots);
        assert_eq!(retained.native.pivots()[8], None);

        let MatchedStage::Dependent {
            canonical_zero_input,
            reduction_rows,
            reduction_factors,
        } = stage_and_match_rebuild(
            &context,
            &retained,
            &mut oracle_prior_rows,
            &[],
            &SymbolicaParametricSparseRow::default(),
        )
        else {
            panic!("the full-rank empty row must be dependent")
        };
        assert!(canonical_zero_input);
        assert!(reduction_rows.is_empty());
        assert!(reduction_factors.is_empty());
        assert_eq!(retained.native.u(), &full_u);
        assert_eq!(retained.native.l(), &full_l);
        assert_eq!(retained.native.pivots(), &full_pivots);
    }

    #[test]
    fn persistent_matches_rebuild_oracle_without_column_insertions() {
        let context = context("persistent-sparse-rebuild-oracle");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            4,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let first = row(vec![entry(1, context.one()), entry(3, context.one())]);
        let second = row(vec![entry(0, context.one()), entry(2, context.one())]);
        let candidate = row(vec![
            entry(0, context.integer(2)),
            entry(1, context.integer(3)),
            entry(2, context.index(0).unwrap()),
            entry(3, context.integer(5)),
        ]);
        let (one, _, normalized_first, _, _, _) = independent(
            stage(
                &base,
                &[],
                &first,
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let (two, _, normalized_second, _, _, _) = independent(
            stage(
                &one,
                &[],
                &second,
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let persistent = stage(
            &two,
            &[],
            &candidate,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();

        let normalized_prior_rows = [normalized_first, normalized_second];
        let prior_inputs = normalized_prior_rows
            .iter()
            .map(SymbolicaParametricSparseRow::try_as_input)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let candidate_input = candidate.try_as_input().unwrap();
        let rebuilt = forward_reduce_last_row(
            context.as_ref(),
            4,
            &prior_inputs,
            &candidate_input,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();

        let SymbolicaPersistentSparseOutcome::Independent {
            pivot_column: persistent_pivot,
            normalized_row: persistent_row,
            reductions: persistent_reductions,
            normalization_divisor: persistent_divisor,
            ..
        } = persistent
        else {
            panic!("persistent candidate must be independent")
        };
        let SymbolicaParametricSparseOutcome::Independent {
            pivot_column: rebuilt_pivot,
            normalized_row: rebuilt_row,
            reductions: rebuilt_reductions,
            normalization_divisor: rebuilt_divisor,
            ..
        } = rebuilt
        else {
            panic!("rebuilt candidate must be independent")
        };
        assert_eq!(persistent_pivot, rebuilt_pivot);
        assert_eq!(persistent_row, rebuilt_row);
        assert_eq!(persistent_reductions, rebuilt_reductions);
        assert_eq!(persistent_divisor, rebuilt_divisor);
    }

    #[test]
    fn persistent_back_substitution_mutates_only_a_disposable_native_clone() {
        let context = context("persistent-sparse-disposable-back-substitution");
        let base = SymbolicaPersistentSparseReducer::try_new(
            context.clone(),
            2,
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        let (one, _, _, _, _, _) = independent(
            stage(
                &base,
                &[],
                &row(vec![entry(0, context.one()), entry(1, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let (committed, _, _, _, _, _) = independent(
            stage(
                &one,
                &[],
                &row(vec![entry(1, context.one())]),
                SymbolicaPersistentSparseLimits::default(),
            )
            .unwrap(),
        );
        let committed_u = committed.native.u().clone();
        let committed_l = committed.native.l().clone();
        let field = committed.native.u().field().clone();
        let field_stage = field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
        let mut publication_clone = committed.native.clone();
        call_native("disposable persistent back substitution", || {
            publication_clone.back_substitute()
        })
        .unwrap();
        drop(field_stage);

        assert_eq!(committed.native.u(), &committed_u);
        assert_eq!(committed.native.l(), &committed_l);
        assert_eq!(committed.native.l().nrows(), 2);
        assert_eq!(publication_clone.l().nrows(), 0);
        let retry = stage(
            &committed,
            &[],
            &row(vec![entry(0, context.integer(2))]),
            SymbolicaPersistentSparseLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            retry,
            SymbolicaPersistentSparseOutcome::Dependent { .. }
        ));
    }
}
