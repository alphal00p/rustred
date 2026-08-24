//! Replayable append-only reference database for parametric elimination.
//!
//! LiteRed's ordinary `SolvejSector` database retains triangular pivots while
//! successive equation batches are submitted inside one residual case group.
//! This module fixes that batch/cursor contract before the elimination kernel
//! itself is made incremental.  It intentionally rebuilds
//! [`crate::ParametricElimination`] over every consumed source prefix, then
//! proves that all previously committed pivots are an exact prefix of the new
//! result.  The implementation is therefore a replay oracle, not yet the fast
//! production kernel.
//!
//! Inputs to this low-level slice are only structurally prevalidated. Callers
//! are responsible for supplying already authenticated generated rows.
//! Production integration must regenerate them from a shared
//! `GeneratedSymbolicRowSpanCertificate`; this database does not turn an
//! arbitrary relation into an IBP identity.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::{
    PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA, ParametricCoefficientContext, ParametricElimination,
    ParametricEliminationError, ParametricEliminationLimits, ParametricEliminationOrdering,
    ParametricPivotEquation, ParametricRelation, ParametricRelationError,
};

pub const PERSISTENT_PARAMETRIC_ELIMINATION_REFERENCE_V1_SCHEMA: &str =
    "rustred-persistent-parametric-elimination-reference-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistentParametricEliminationLimits {
    pub elimination: ParametricEliminationLimits,
    pub max_submitted_batches: usize,
    pub max_rows_per_batch: usize,
    pub max_retained_source_rows: usize,
    /// Legacy field name: this counts sparse `(IndexShift, coefficient)`
    /// integral slots, not coefficient-polynomial monomials or bytes.
    pub max_retained_source_terms: usize,
    pub max_retained_source_manifest_bytes: usize,
    pub max_batch_label_bytes: usize,
    pub max_retained_batch_label_bytes: usize,
    pub max_cumulative_prefix_rebuild_rows: usize,
    /// Legacy field name: cumulative sparse integral slots revisited across
    /// prefix rebuilds, not total Symbolica coefficient storage.
    pub max_cumulative_prefix_rebuild_terms: usize,
    pub max_cumulative_prefix_rebuild_manifest_bytes: usize,
    pub max_cumulative_construction_reductions: usize,
    pub max_cumulative_construction_updates: usize,
    pub max_cumulative_replay_reductions: usize,
    pub max_cumulative_replay_updates: usize,
    pub max_replay_batch_clone_rows: usize,
    pub max_replay_batch_clone_integral_slots: usize,
    pub max_replay_batch_clone_manifest_bytes: usize,
    pub max_replay_source_clone_rows: usize,
    pub max_replay_source_clone_integral_slots: usize,
    pub max_replay_source_clone_manifest_bytes: usize,
    /// Peak bytes in simultaneously live `ParametricElimination` source
    /// manifests.  A rebuild retains the previous manifest while the new
    /// manifest, its independent replay copy, and the largest bounded row-
    /// manifest encoder temporary coexist; certificate replay additionally
    /// retains the certified elimination manifest.
    pub max_coexisting_elimination_source_manifest_bytes: usize,
}

impl Default for PersistentParametricEliminationLimits {
    fn default() -> Self {
        Self {
            elimination: ParametricEliminationLimits::default(),
            max_submitted_batches: 1_000_000,
            max_rows_per_batch: 100_000,
            max_retained_source_rows: 100_000,
            max_retained_source_terms: 16_000_000,
            max_retained_source_manifest_bytes: 512 * 1024 * 1024,
            max_batch_label_bytes: 1024 * 1024,
            max_retained_batch_label_bytes: 64 * 1024 * 1024,
            max_cumulative_prefix_rebuild_rows: 1_000_000_000,
            max_cumulative_prefix_rebuild_terms: 10_000_000_000,
            max_cumulative_prefix_rebuild_manifest_bytes: 20_000_000_000,
            max_cumulative_construction_reductions: 1_000_000_000,
            max_cumulative_construction_updates: 10_000_000_000,
            max_cumulative_replay_reductions: 2_000_000_000,
            max_cumulative_replay_updates: 20_000_000_000,
            max_replay_batch_clone_rows: 100_000,
            max_replay_batch_clone_integral_slots: 16_000_000,
            max_replay_batch_clone_manifest_bytes: 512 * 1024 * 1024,
            max_replay_source_clone_rows: 200_000,
            max_replay_source_clone_integral_slots: 32_000_000,
            max_replay_source_clone_manifest_bytes: 1024 * 1024 * 1024,
            max_coexisting_elimination_source_manifest_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PersistentParametricEliminationStats {
    submitted_batches: usize,
    submitted_rows: usize,
    retained_source_integral_slots: usize,
    retained_source_manifest_bytes: usize,
    retained_batch_label_bytes: usize,
    retained_input_guards: usize,
    retained_input_guard_origins: usize,
    retained_input_columns: usize,
    consumed_rows: usize,
    dependent_rows: usize,
    pivot_rows: usize,
    rebuilds: usize,
    cumulative_prefix_rebuild_rows: usize,
    cumulative_prefix_rebuild_integral_slots: usize,
    cumulative_prefix_rebuild_manifest_bytes: usize,
    cumulative_construction_reductions: usize,
    cumulative_construction_updates: usize,
    cumulative_replay_reductions: usize,
    cumulative_replay_updates: usize,
}

impl PersistentParametricEliminationStats {
    pub const fn submitted_batches(self) -> usize {
        self.submitted_batches
    }

    pub const fn submitted_rows(self) -> usize {
        self.submitted_rows
    }

    pub const fn retained_source_terms(self) -> usize {
        self.retained_source_integral_slots
    }

    pub const fn retained_source_integral_slots(self) -> usize {
        self.retained_source_integral_slots
    }

    pub const fn retained_source_manifest_bytes(self) -> usize {
        self.retained_source_manifest_bytes
    }

    pub const fn retained_batch_label_bytes(self) -> usize {
        self.retained_batch_label_bytes
    }

    pub const fn retained_input_guards(self) -> usize {
        self.retained_input_guards
    }

    pub const fn retained_input_guard_origins(self) -> usize {
        self.retained_input_guard_origins
    }

    pub const fn retained_input_columns(self) -> usize {
        self.retained_input_columns
    }

    pub const fn consumed_rows(self) -> usize {
        self.consumed_rows
    }

    pub const fn dependent_rows(self) -> usize {
        self.dependent_rows
    }

    pub const fn pivot_rows(self) -> usize {
        self.pivot_rows
    }

    pub const fn rebuilds(self) -> usize {
        self.rebuilds
    }

    pub const fn cumulative_prefix_rebuild_rows(self) -> usize {
        self.cumulative_prefix_rebuild_rows
    }

    pub const fn cumulative_prefix_rebuild_terms(self) -> usize {
        self.cumulative_prefix_rebuild_integral_slots
    }

    pub const fn cumulative_prefix_rebuild_integral_slots(self) -> usize {
        self.cumulative_prefix_rebuild_integral_slots
    }

    pub const fn cumulative_prefix_rebuild_manifest_bytes(self) -> usize {
        self.cumulative_prefix_rebuild_manifest_bytes
    }

    pub const fn cumulative_construction_reductions(self) -> usize {
        self.cumulative_construction_reductions
    }

    pub const fn cumulative_construction_updates(self) -> usize {
        self.cumulative_construction_updates
    }

    pub const fn cumulative_replay_reductions(self) -> usize {
        self.cumulative_replay_reductions
    }

    pub const fn cumulative_replay_updates(self) -> usize {
        self.cumulative_replay_updates
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistentParametricEliminationBatch {
    ordinal: usize,
    label: Arc<str>,
    first_source_ordinal: usize,
    source_row_count: usize,
}

impl PersistentParametricEliminationBatch {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn first_source_ordinal(&self) -> usize {
        self.first_source_ordinal
    }

    pub const fn source_row_count(&self) -> usize {
        self.source_row_count
    }

    pub fn source_range(&self) -> std::ops::Range<usize> {
        self.try_source_range()
            .expect("a constructed persistent batch has a checked source range")
    }

    pub fn try_source_range(
        &self,
    ) -> Result<std::ops::Range<usize>, PersistentParametricEliminationError> {
        let end = checked_add(
            self.first_source_ordinal,
            self.source_row_count,
            "persistent batch source range",
        )?;
        Ok(self.first_source_ordinal..end)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistentParametricEliminationRowOutcome {
    Dependent,
    Pivot { pivot_ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistentParametricEliminationEvent {
    event_ordinal: usize,
    batch_ordinal: usize,
    source_ordinal: usize,
    within_batch_ordinal: usize,
    outcome: PersistentParametricEliminationRowOutcome,
}

impl PersistentParametricEliminationEvent {
    pub const fn event_ordinal(self) -> usize {
        self.event_ordinal
    }

    pub const fn batch_ordinal(self) -> usize {
        self.batch_ordinal
    }

    pub const fn source_ordinal(self) -> usize {
        self.source_ordinal
    }

    pub const fn within_batch_ordinal(self) -> usize {
        self.within_batch_ordinal
    }

    pub const fn outcome(self) -> PersistentParametricEliminationRowOutcome {
        self.outcome
    }
}

/// Mutable reference implementation for one LiteRed residual-case-group
/// database. Construct a new value at the `clean` boundary; pivots must not be
/// carried into another case group.
pub struct PersistentParametricEliminationDatabase<'context> {
    context: &'context ParametricCoefficientContext,
    family_fingerprint: Arc<str>,
    ordering: ParametricEliminationOrdering,
    limits: PersistentParametricEliminationLimits,
    source_rows: Vec<ParametricRelation>,
    source_integral_slot_prefixes: Vec<usize>,
    source_manifest_lengths: Vec<usize>,
    source_manifest_component_prefixes: Vec<usize>,
    source_columns: BTreeSet<crate::IndexShift>,
    batches: Vec<PersistentParametricEliminationBatch>,
    events: Vec<PersistentParametricEliminationEvent>,
    elimination: Option<ParametricElimination>,
    stats: PersistentParametricEliminationStats,
    interrupted: bool,
}

impl<'context> PersistentParametricEliminationDatabase<'context> {
    pub fn try_new(
        context: &'context ParametricCoefficientContext,
        family_fingerprint: impl Into<Arc<str>>,
        ordering: ParametricEliminationOrdering,
        limits: PersistentParametricEliminationLimits,
    ) -> Result<Self, PersistentParametricEliminationError> {
        if ordering.anchor().len() != context.index_count() {
            return Err(PersistentParametricEliminationError::WrongArity {
                expected: context.index_count(),
                actual: ordering.anchor().len(),
            });
        }
        Ok(Self {
            context,
            family_fingerprint: family_fingerprint.into(),
            ordering,
            limits,
            source_rows: Vec::new(),
            source_integral_slot_prefixes: Vec::new(),
            source_manifest_lengths: Vec::new(),
            source_manifest_component_prefixes: Vec::new(),
            source_columns: BTreeSet::new(),
            batches: Vec::new(),
            events: Vec::new(),
            elimination: None,
            stats: PersistentParametricEliminationStats::default(),
            interrupted: false,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn context(&self) -> &ParametricCoefficientContext {
        self.context
    }

    pub const fn ordering(&self) -> &ParametricEliminationOrdering {
        &self.ordering
    }

    pub const fn limits(&self) -> PersistentParametricEliminationLimits {
        self.limits
    }

    pub const fn stats(&self) -> PersistentParametricEliminationStats {
        self.stats
    }

    pub fn batches(&self) -> &[PersistentParametricEliminationBatch] {
        &self.batches
    }

    pub fn events(&self) -> &[PersistentParametricEliminationEvent] {
        &self.events
    }

    pub const fn elimination(&self) -> Option<&ParametricElimination> {
        self.elimination.as_ref()
    }

    pub fn pending_rows(&self) -> usize {
        self.try_pending_rows().unwrap_or(0)
    }

    /// Fallible cursor query for future decoded/checkpointed state.  The
    /// compatibility `pending_rows` observer saturates corrupt state at zero;
    /// all mutating protocol paths use this typed variant instead.
    pub fn try_pending_rows(&self) -> Result<usize, PersistentParametricEliminationError> {
        self.source_rows
            .len()
            .checked_sub(self.stats.consumed_rows)
            .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent consumed-row cursor exceeds retained rows".to_owned(),
            })
    }

    /// Replace the pending batch. As in LiteRed, submitting while an old
    /// suffix remains is a protocol error rather than an implicit union.
    pub fn submit_prevalidated_rows(
        &mut self,
        label: impl Into<Arc<str>>,
        rows: Vec<ParametricRelation>,
    ) -> Result<(), PersistentParametricEliminationError> {
        self.ensure_active()?;
        let pending = self.try_pending_rows()?;
        if pending != 0 {
            return Err(PersistentParametricEliminationError::PendingBatchNotConsumed { pending });
        }
        if rows.is_empty() {
            return Err(PersistentParametricEliminationError::EmptyBatch);
        }
        check_limit(
            "persistent rows in one submitted batch",
            rows.len(),
            self.limits.max_rows_per_batch,
        )?;
        let label = label.into();
        check_limit(
            "persistent batch label bytes",
            label.len(),
            self.limits.max_batch_label_bytes,
        )?;
        let requested_label_bytes = checked_add(
            self.stats.retained_batch_label_bytes,
            label.len(),
            "persistent retained batch label bytes",
        )?;
        check_limit(
            "persistent retained batch label bytes",
            requested_label_bytes,
            self.limits.max_retained_batch_label_bytes,
        )?;
        check_limit(
            "persistent submitted batches",
            checked_add(self.batches.len(), 1, "persistent submitted batches")?,
            self.limits.max_submitted_batches,
        )?;

        let requested_rows = checked_add(
            self.source_rows.len(),
            rows.len(),
            "persistent retained source rows",
        )?;
        check_limit(
            "persistent retained source rows",
            requested_rows,
            self.limits.max_retained_source_rows,
        )?;
        check_limit(
            "persistent retained source rows",
            requested_rows,
            self.limits.elimination.max_source_rows,
        )?;
        check_limit(
            "persistent replay batch clone rows",
            rows.len(),
            self.limits.max_replay_batch_clone_rows,
        )?;

        let mut added_integral_slots = 0usize;
        let mut added_guards = 0usize;
        let mut added_guard_origins = 0usize;
        let mut added_manifest_bytes = 0usize;
        let mut added_integral_slot_prefixes = Vec::with_capacity(rows.len());
        let mut added_manifest_lengths = Vec::with_capacity(rows.len());
        let mut added_manifest_component_prefixes = Vec::with_capacity(rows.len());
        let mut integral_slot_prefix = self
            .source_integral_slot_prefixes
            .last()
            .copied()
            .unwrap_or(0);
        let mut manifest_component_prefix = self
            .source_manifest_component_prefixes
            .last()
            .copied()
            .unwrap_or(0);
        let mut staged_new_columns = BTreeSet::new();
        for (row, relation) in rows.iter().enumerate() {
            if relation.family_fingerprint() != self.family_fingerprint.as_ref() {
                return Err(PersistentParametricEliminationError::WrongFamily { row });
            }
            if relation.context_fingerprint() != self.context.fingerprint() {
                return Err(PersistentParametricEliminationError::WrongContext { row });
            }
            if relation.arity() != self.context.index_count() {
                return Err(PersistentParametricEliminationError::WrongArity {
                    expected: self.context.index_count(),
                    actual: relation.arity(),
                });
            }
            added_integral_slots = checked_add(
                added_integral_slots,
                relation.terms().len(),
                "persistent retained source integral slots",
            )?;
            added_guards = checked_add(
                added_guards,
                relation.guarded_nonzero_conditions().len(),
                "persistent input relation guards",
            )?;
            for (condition_index, condition) in
                relation.guarded_nonzero_conditions().iter().enumerate()
            {
                if !self.context.contains_nonzero_condition(condition)
                    || condition.polynomial().is_zero()
                    || condition.origins().is_empty()
                {
                    return Err(PersistentParametricEliminationError::Elimination(
                        ParametricEliminationError::InvalidSourceGuard {
                            row,
                            condition: condition_index,
                        },
                    ));
                }
                self.context
                    .validate_polynomial_with_limits(
                        condition.polynomial(),
                        self.limits.elimination.arithmetic.exact_algebra,
                    )
                    .map_err(ParametricEliminationError::from)?;
                check_limit(
                    "persistent origins in one input relation guard",
                    condition.origins().len(),
                    self.limits.elimination.arithmetic.max_guard_origins,
                )?;
                added_guard_origins = checked_add(
                    added_guard_origins,
                    condition.origins().len(),
                    "persistent input relation guard origins",
                )?;
            }
            for (shift, coefficient) in relation.terms() {
                self.context
                    .validate_with_limits(
                        coefficient,
                        self.limits.elimination.arithmetic.exact_algebra,
                    )
                    .map_err(ParametricEliminationError::from)?;
                if !self.source_columns.contains(shift) && !staged_new_columns.contains(shift) {
                    let requested_columns = checked_add(
                        checked_add(
                            self.source_columns.len(),
                            staged_new_columns.len(),
                            "persistent input relation columns",
                        )?,
                        1,
                        "persistent input relation columns",
                    )?;
                    check_limit(
                        "persistent input relation columns",
                        requested_columns,
                        self.limits.elimination.max_columns,
                    )?;
                    let indices = self.ordering.shifted_indices(shift)?;
                    // Force the same fallible injective-key validation as the
                    // elimination's column decoration before retaining rows.
                    self.ordering
                        .policy()
                        .complexity_key(&indices)
                        .map_err(ParametricEliminationError::Sector)?;
                    staged_new_columns.insert(shift.clone());
                }
            }
            integral_slot_prefix = checked_add(
                integral_slot_prefix,
                relation.terms().len(),
                "persistent source integral-slot prefix",
            )?;
            added_integral_slot_prefixes.push(integral_slot_prefix);
            let retained_before_row = checked_add(
                self.stats.retained_source_manifest_bytes,
                added_manifest_bytes,
                "persistent retained source manifest bytes",
            )?;
            let remaining_manifest_bytes = self
                .limits
                .max_retained_source_manifest_bytes
                .checked_sub(retained_before_row)
                .ok_or(PersistentParametricEliminationError::ResourceLimit {
                    resource: "persistent retained source manifest bytes",
                    requested: retained_before_row,
                    limit: self.limits.max_retained_source_manifest_bytes,
                })?;
            let manifest_bytes = match relation.stable_manifest_with_limit(remaining_manifest_bytes)
            {
                Ok(manifest) => manifest.len(),
                Err(ParametricRelationError::ResourceLimit { requested, .. }) => {
                    return Err(PersistentParametricEliminationError::ResourceLimit {
                        resource: "persistent retained source manifest bytes",
                        requested: checked_add(
                            retained_before_row,
                            requested,
                            "persistent retained source manifest bytes",
                        )?,
                        limit: self.limits.max_retained_source_manifest_bytes,
                    });
                }
                Err(ParametricRelationError::ResourceCountOverflow { .. }) => {
                    return Err(
                        PersistentParametricEliminationError::ResourceCountOverflow {
                            resource: "persistent retained source manifest bytes",
                        },
                    );
                }
                Err(error) => return Err(error.into()),
            };
            added_manifest_bytes = checked_add(
                added_manifest_bytes,
                manifest_bytes,
                "persistent retained source manifest bytes",
            )?;
            added_manifest_lengths.push(manifest_bytes);
            let manifest_component_bytes = checked_add(
                checked_add(
                    checked_add(1, manifest_bytes.to_string().len(), "source manifest bytes")?,
                    1,
                    "source manifest bytes",
                )?,
                manifest_bytes,
                "source manifest bytes",
            )?;
            manifest_component_prefix = checked_add(
                manifest_component_prefix,
                manifest_component_bytes,
                "persistent source manifest component prefix",
            )?;
            added_manifest_component_prefixes.push(manifest_component_prefix);
        }
        let requested_integral_slots = checked_add(
            self.stats.retained_source_integral_slots,
            added_integral_slots,
            "persistent retained source integral slots",
        )?;
        check_limit(
            "persistent retained source integral slots",
            requested_integral_slots,
            self.limits.max_retained_source_terms,
        )?;
        check_limit(
            "persistent input relation integral slots",
            requested_integral_slots,
            self.limits.elimination.max_input_terms,
        )?;
        let requested_guards = checked_add(
            self.stats.retained_input_guards,
            added_guards,
            "persistent input relation guards",
        )?;
        check_limit(
            "persistent input relation guards",
            requested_guards,
            self.limits.elimination.max_input_guards,
        )?;
        let requested_guard_origins = checked_add(
            self.stats.retained_input_guard_origins,
            added_guard_origins,
            "persistent input relation guard origins",
        )?;
        check_limit(
            "persistent input relation guard origins",
            requested_guard_origins,
            self.limits.elimination.max_input_guard_origins,
        )?;
        check_limit(
            "persistent input relation columns",
            checked_add(
                self.source_columns.len(),
                staged_new_columns.len(),
                "persistent input relation columns",
            )?,
            self.limits.elimination.max_columns,
        )?;
        let requested_manifest_bytes = checked_add(
            self.stats.retained_source_manifest_bytes,
            added_manifest_bytes,
            "persistent retained source manifest bytes",
        )?;
        check_limit(
            "persistent retained source manifest bytes",
            requested_manifest_bytes,
            self.limits.max_retained_source_manifest_bytes,
        )?;
        check_limit(
            "persistent replay batch clone integral slots",
            added_integral_slots,
            self.limits.max_replay_batch_clone_integral_slots,
        )?;
        check_limit(
            "persistent replay batch clone manifest bytes",
            added_manifest_bytes,
            self.limits.max_replay_batch_clone_manifest_bytes,
        )?;
        check_limit(
            "persistent replay coexisting source clone rows",
            checked_mul(
                requested_rows,
                2,
                "persistent replay coexisting source clone rows",
            )?,
            self.limits.max_replay_source_clone_rows,
        )?;
        check_limit(
            "persistent replay coexisting source clone integral slots",
            checked_mul(
                requested_integral_slots,
                2,
                "persistent replay coexisting source clone integral slots",
            )?,
            self.limits.max_replay_source_clone_integral_slots,
        )?;
        check_limit(
            "persistent replay coexisting source clone manifest bytes",
            checked_mul(
                requested_manifest_bytes,
                2,
                "persistent replay coexisting source clone manifest bytes",
            )?,
            self.limits.max_replay_source_clone_manifest_bytes,
        )?;
        let requested_source_manifest_bytes =
            prefix_source_manifest_bytes(requested_rows, manifest_component_prefix)?;
        check_limit(
            "persistent retained elimination source manifest bytes",
            requested_source_manifest_bytes,
            self.limits.elimination.max_source_manifest_bytes,
        )?;

        let first_source_ordinal = self.source_rows.len();
        let ordinal = self.batches.len();
        let source_row_count = rows.len();
        self.source_rows.extend(rows);
        self.source_integral_slot_prefixes
            .extend(added_integral_slot_prefixes);
        self.source_manifest_lengths.extend(added_manifest_lengths);
        self.source_manifest_component_prefixes
            .extend(added_manifest_component_prefixes);
        self.source_columns.extend(staged_new_columns);
        self.batches.push(PersistentParametricEliminationBatch {
            ordinal,
            label,
            first_source_ordinal,
            source_row_count,
        });
        self.stats.submitted_batches = self.batches.len();
        self.stats.submitted_rows = self.source_rows.len();
        self.stats.retained_source_integral_slots = requested_integral_slots;
        self.stats.retained_source_manifest_bytes = requested_manifest_bytes;
        self.stats.retained_batch_label_bytes = requested_label_bytes;
        self.stats.retained_input_guards = requested_guards;
        self.stats.retained_input_guard_origins = requested_guard_origins;
        self.stats.retained_input_columns = self.source_columns.len();
        Ok(())
    }

    /// Consume and commit one pending equation. The returned event order is
    /// the candidate-priority foundation; dependent rows remain explicit.
    pub fn consume_next(
        &mut self,
    ) -> Result<Option<PersistentParametricEliminationEvent>, PersistentParametricEliminationError>
    {
        self.consume_next_with_outer_elimination_manifest_bytes(0)
    }

    fn consume_next_with_outer_elimination_manifest_bytes(
        &mut self,
        outer_elimination_manifest_bytes: usize,
    ) -> Result<Option<PersistentParametricEliminationEvent>, PersistentParametricEliminationError>
    {
        self.ensure_active()?;
        let result = self.consume_next_active(outer_elimination_manifest_bytes);
        if result.is_err() {
            self.interrupted = true;
        }
        result
    }

    fn consume_next_active(
        &mut self,
        outer_elimination_manifest_bytes: usize,
    ) -> Result<Option<PersistentParametricEliminationEvent>, PersistentParametricEliminationError>
    {
        if self.try_pending_rows()? == 0 {
            return Ok(None);
        }
        let source_ordinal = self.stats.consumed_rows;
        let prefix_len = checked_add(source_ordinal, 1, "persistent consumed source rows")?;
        let prefix_integral_slots = *self
            .source_integral_slot_prefixes
            .get(source_ordinal)
            .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                detail: "consumed source row has no retained term prefix".to_owned(),
            })?;
        let prefix_manifest_component_bytes = *self
            .source_manifest_component_prefixes
            .get(source_ordinal)
            .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                detail: "consumed source row has no retained manifest prefix".to_owned(),
            })?;
        let prefix_manifest_bytes =
            prefix_source_manifest_bytes(prefix_len, prefix_manifest_component_bytes)?;
        check_limit(
            "persistent source manifest bytes in one prefix rebuild",
            prefix_manifest_bytes,
            self.limits.elimination.max_source_manifest_bytes,
        )?;
        let previous_manifest_bytes = self
            .elimination
            .as_ref()
            .map_or(0, |elimination| elimination.source_manifest().len());
        let largest_prefix_row_manifest_bytes = self
            .source_manifest_lengths
            .get(..prefix_len)
            .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                detail: "consumed source prefix exceeds retained manifest metadata".to_owned(),
            })?
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        let coexisting_elimination_manifest_bytes = checked_add(
            checked_add(
                outer_elimination_manifest_bytes,
                previous_manifest_bytes,
                "persistent coexisting elimination source manifest bytes",
            )?,
            checked_add(
                checked_mul(
                    prefix_manifest_bytes,
                    2,
                    "persistent coexisting elimination source manifest bytes",
                )?,
                largest_prefix_row_manifest_bytes,
                "persistent coexisting elimination source manifest bytes",
            )?,
            "persistent coexisting elimination source manifest bytes",
        )?;
        check_limit(
            "persistent coexisting elimination source manifest bytes",
            coexisting_elimination_manifest_bytes,
            self.limits.max_coexisting_elimination_source_manifest_bytes,
        )?;
        // `ParametricElimination::build` materializes the ordered source
        // manifest once during construction and once during exact replay.
        let prefix_manifest_work = checked_mul(
            prefix_manifest_bytes,
            2,
            "persistent cumulative prefix rebuild manifest bytes",
        )?;
        let cumulative_prefix_rebuild_rows = checked_add(
            self.stats.cumulative_prefix_rebuild_rows,
            prefix_len,
            "persistent cumulative prefix rebuild rows",
        )?;
        let cumulative_prefix_rebuild_integral_slots = checked_add(
            self.stats.cumulative_prefix_rebuild_integral_slots,
            prefix_integral_slots,
            "persistent cumulative prefix rebuild integral slots",
        )?;
        let cumulative_prefix_rebuild_manifest_bytes = checked_add(
            self.stats.cumulative_prefix_rebuild_manifest_bytes,
            prefix_manifest_work,
            "persistent cumulative prefix rebuild manifest bytes",
        )?;
        check_limit(
            "persistent cumulative prefix rebuild rows",
            cumulative_prefix_rebuild_rows,
            self.limits.max_cumulative_prefix_rebuild_rows,
        )?;
        check_limit(
            "persistent cumulative prefix rebuild integral slots",
            cumulative_prefix_rebuild_integral_slots,
            self.limits.max_cumulative_prefix_rebuild_terms,
        )?;
        check_limit(
            "persistent cumulative prefix rebuild manifest bytes",
            cumulative_prefix_rebuild_manifest_bytes,
            self.limits.max_cumulative_prefix_rebuild_manifest_bytes,
        )?;
        let mut build_limits = self.remaining_build_limits()?;
        build_limits.max_source_rows = build_limits.max_source_rows.min(prefix_len);
        let rebuilt = match ParametricElimination::build(
            self.context,
            &self.source_rows[..prefix_len],
            self.ordering.clone(),
            build_limits,
        ) {
            Ok(elimination) => elimination,
            Err(error) => {
                self.interrupted = true;
                return Err(error.into());
            }
        };

        let previous_pivots = self
            .elimination
            .as_ref()
            .map_or(&[][..], |elimination| elimination.pivots());
        if rebuilt.pivots().len() < previous_pivots.len()
            || rebuilt.pivots().len() > previous_pivots.len() + 1
            || !pivot_prefix_eq(previous_pivots, rebuilt.pivots())
        {
            self.interrupted = true;
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "a rebuilt source prefix changed an already committed pivot".to_owned(),
            });
        }
        let outcome = if rebuilt.pivots().len() == previous_pivots.len() {
            PersistentParametricEliminationRowOutcome::Dependent
        } else {
            PersistentParametricEliminationRowOutcome::Pivot {
                pivot_ordinal: previous_pivots.len(),
            }
        };
        let batch = self
            .batches
            .iter()
            .find(|batch| {
                batch
                    .try_source_range()
                    .is_ok_and(|range| range.contains(&source_ordinal))
            })
            .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                detail: "consumed source row has no submitted batch".to_owned(),
            })?;
        let event = PersistentParametricEliminationEvent {
            event_ordinal: self.events.len(),
            batch_ordinal: batch.ordinal,
            source_ordinal,
            within_batch_ordinal: source_ordinal - batch.first_source_ordinal,
            outcome,
        };
        let elimination_stats = rebuilt.stats();
        let cumulative_construction_reductions = checked_add(
            self.stats.cumulative_construction_reductions,
            elimination_stats.construction_reductions(),
            "persistent cumulative construction reductions",
        )?;
        let cumulative_construction_updates = checked_add(
            self.stats.cumulative_construction_updates,
            elimination_stats.construction_updates(),
            "persistent cumulative construction updates",
        )?;
        let cumulative_replay_reductions = checked_add(
            self.stats.cumulative_replay_reductions,
            elimination_stats.replay_reductions(),
            "persistent cumulative replay reductions",
        )?;
        let cumulative_replay_updates = checked_add(
            self.stats.cumulative_replay_updates,
            elimination_stats.replay_updates(),
            "persistent cumulative replay updates",
        )?;
        check_limit(
            "persistent cumulative construction reductions",
            cumulative_construction_reductions,
            self.limits.max_cumulative_construction_reductions,
        )?;
        check_limit(
            "persistent cumulative construction updates",
            cumulative_construction_updates,
            self.limits.max_cumulative_construction_updates,
        )?;
        check_limit(
            "persistent cumulative replay reductions",
            cumulative_replay_reductions,
            self.limits.max_cumulative_replay_reductions,
        )?;
        check_limit(
            "persistent cumulative replay updates",
            cumulative_replay_updates,
            self.limits.max_cumulative_replay_updates,
        )?;

        let (dependent_rows, pivot_rows) = match outcome {
            PersistentParametricEliminationRowOutcome::Dependent => (
                checked_add(self.stats.dependent_rows, 1, "persistent dependent rows")?,
                self.stats.pivot_rows,
            ),
            PersistentParametricEliminationRowOutcome::Pivot { .. } => (
                self.stats.dependent_rows,
                checked_add(self.stats.pivot_rows, 1, "persistent pivot rows")?,
            ),
        };
        let rebuilds = checked_add(self.stats.rebuilds, 1, "persistent rebuilds")?;

        self.elimination = Some(rebuilt);
        self.events.push(event);
        self.stats.consumed_rows = prefix_len;
        self.stats.dependent_rows = dependent_rows;
        self.stats.pivot_rows = pivot_rows;
        self.stats.rebuilds = rebuilds;
        self.stats.cumulative_prefix_rebuild_rows = cumulative_prefix_rebuild_rows;
        self.stats.cumulative_prefix_rebuild_integral_slots =
            cumulative_prefix_rebuild_integral_slots;
        self.stats.cumulative_prefix_rebuild_manifest_bytes =
            cumulative_prefix_rebuild_manifest_bytes;
        self.stats.cumulative_construction_reductions = cumulative_construction_reductions;
        self.stats.cumulative_construction_updates = cumulative_construction_updates;
        self.stats.cumulative_replay_reductions = cumulative_replay_reductions;
        self.stats.cumulative_replay_updates = cumulative_replay_updates;
        Ok(Some(event))
    }

    /// Consume rows until a newly committed pivot satisfies `selector`.
    /// Skipped pivots remain committed, exactly like LiteRed pivots whose
    /// left sides do not match the current `solveeqs` pattern.
    pub fn solve_until(
        &mut self,
        mut selector: impl FnMut(&ParametricPivotEquation) -> bool,
    ) -> Result<Option<PersistentParametricEliminationEvent>, PersistentParametricEliminationError>
    {
        self.ensure_active()?;
        let result = self.solve_until_active(&mut selector);
        if result.is_err() {
            self.interrupted = true;
        }
        result
    }

    fn solve_until_active(
        &mut self,
        selector: &mut impl FnMut(&ParametricPivotEquation) -> bool,
    ) -> Result<Option<PersistentParametricEliminationEvent>, PersistentParametricEliminationError>
    {
        while let Some(event) = self.consume_next_active(0)? {
            let PersistentParametricEliminationRowOutcome::Pivot { pivot_ordinal } = event.outcome
            else {
                continue;
            };
            let pivot = self
                .elimination
                .as_ref()
                .and_then(|elimination| elimination.pivots().get(pivot_ordinal))
                .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                    detail: "committed pivot event has no pivot equation".to_owned(),
                })?;
            if selector(pivot) {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    /// Freeze the current case-group database. A pending suffix is retained
    /// explicitly; it is not silently treated as solved.
    pub fn finish(self) -> PersistentParametricEliminationCertificate {
        PersistentParametricEliminationCertificate {
            schema: PERSISTENT_PARAMETRIC_ELIMINATION_REFERENCE_V1_SCHEMA,
            family_fingerprint: self.family_fingerprint,
            context_fingerprint: self.context.fingerprint().into(),
            ordering: self.ordering,
            limits: self.limits,
            source_rows: self.source_rows.into_boxed_slice(),
            source_manifest_lengths: self.source_manifest_lengths.into_boxed_slice(),
            batches: self.batches.into_boxed_slice(),
            events: self.events.into_boxed_slice(),
            elimination: self.elimination,
            stats: self.stats,
            interrupted: self.interrupted,
        }
    }

    fn ensure_active(&self) -> Result<(), PersistentParametricEliminationError> {
        if self.interrupted {
            Err(PersistentParametricEliminationError::DatabaseInterrupted)
        } else {
            Ok(())
        }
    }

    fn remaining_build_limits(
        &self,
    ) -> Result<ParametricEliminationLimits, PersistentParametricEliminationError> {
        let mut limits = self.limits.elimination;
        limits.max_reductions = limits.max_reductions.min(
            self.limits
                .max_cumulative_construction_reductions
                .checked_sub(self.stats.cumulative_construction_reductions)
                .ok_or(PersistentParametricEliminationError::ResourceLimit {
                    resource: "persistent cumulative construction reductions",
                    requested: self.stats.cumulative_construction_reductions,
                    limit: self.limits.max_cumulative_construction_reductions,
                })?,
        );
        limits.max_sparse_updates = limits.max_sparse_updates.min(
            self.limits
                .max_cumulative_construction_updates
                .checked_sub(self.stats.cumulative_construction_updates)
                .ok_or(PersistentParametricEliminationError::ResourceLimit {
                    resource: "persistent cumulative construction updates",
                    requested: self.stats.cumulative_construction_updates,
                    limit: self.limits.max_cumulative_construction_updates,
                })?,
        );
        limits.max_replay_reductions = limits.max_replay_reductions.min(
            self.limits
                .max_cumulative_replay_reductions
                .checked_sub(self.stats.cumulative_replay_reductions)
                .ok_or(PersistentParametricEliminationError::ResourceLimit {
                    resource: "persistent cumulative replay reductions",
                    requested: self.stats.cumulative_replay_reductions,
                    limit: self.limits.max_cumulative_replay_reductions,
                })?,
        );
        limits.max_replay_updates = limits.max_replay_updates.min(
            self.limits
                .max_cumulative_replay_updates
                .checked_sub(self.stats.cumulative_replay_updates)
                .ok_or(PersistentParametricEliminationError::ResourceLimit {
                    resource: "persistent cumulative replay updates",
                    requested: self.stats.cumulative_replay_updates,
                    limit: self.limits.max_cumulative_replay_updates,
                })?,
        );
        Ok(limits)
    }
}

/// Replayable certificate for one isolated residual-case-group database.
pub struct PersistentParametricEliminationCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    ordering: ParametricEliminationOrdering,
    limits: PersistentParametricEliminationLimits,
    source_rows: Box<[ParametricRelation]>,
    source_manifest_lengths: Box<[usize]>,
    batches: Box<[PersistentParametricEliminationBatch]>,
    events: Box<[PersistentParametricEliminationEvent]>,
    elimination: Option<ParametricElimination>,
    stats: PersistentParametricEliminationStats,
    interrupted: bool,
}

impl PersistentParametricEliminationCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub const fn ordering(&self) -> &ParametricEliminationOrdering {
        &self.ordering
    }

    pub const fn limits(&self) -> PersistentParametricEliminationLimits {
        self.limits
    }

    pub fn batches(&self) -> &[PersistentParametricEliminationBatch] {
        &self.batches
    }

    pub fn events(&self) -> &[PersistentParametricEliminationEvent] {
        &self.events
    }

    pub const fn elimination(&self) -> Option<&ParametricElimination> {
        self.elimination.as_ref()
    }

    pub const fn stats(&self) -> PersistentParametricEliminationStats {
        self.stats
    }

    pub fn pending_rows(&self) -> usize {
        self.try_pending_rows().unwrap_or(0)
    }

    /// Fallible cursor query for certificates reconstructed by a future
    /// decoder. Replay calls this invariant out explicitly before allocating.
    pub fn try_pending_rows(&self) -> Result<usize, PersistentParametricEliminationError> {
        self.source_rows
            .len()
            .checked_sub(self.stats.consumed_rows)
            .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent consumed-row cursor exceeds retained rows".to_owned(),
            })
    }

    pub const fn interrupted(&self) -> bool {
        self.interrupted
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), PersistentParametricEliminationError> {
        if self.schema != PERSISTENT_PARAMETRIC_ELIMINATION_REFERENCE_V1_SCHEMA
            || self.context_fingerprint.as_ref() != context.fingerprint()
            || self.interrupted
        {
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent certificate scope or completion state differs".to_owned(),
            });
        }
        if self.source_manifest_lengths.len() != self.source_rows.len() {
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent row-manifest length table differs from retained rows"
                    .to_owned(),
            });
        }
        if self.stats.consumed_rows > self.source_rows.len() {
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent consumed-row cursor exceeds retained rows".to_owned(),
            });
        }

        // Treat every persisted counter as untrusted decoder metadata.  Audit
        // the actual borrowed rows and every fixed elimination constraint
        // before the first replay-owned deep clone is allocated.
        check_limit(
            "persistent retained source rows",
            self.source_rows.len(),
            self.limits.max_retained_source_rows,
        )?;
        check_limit(
            "persistent retained source rows",
            self.source_rows.len(),
            self.limits.elimination.max_source_rows,
        )?;
        let mut actual_source_integral_slots = 0usize;
        let mut actual_source_manifest_bytes = 0usize;
        let mut actual_input_guards = 0usize;
        let mut actual_input_guard_origins = 0usize;
        let mut actual_columns = BTreeSet::new();
        for (row_index, (row, &stored_manifest_bytes)) in self
            .source_rows
            .iter()
            .zip(self.source_manifest_lengths.iter())
            .enumerate()
        {
            if row.family_fingerprint() != self.family_fingerprint.as_ref()
                || row.context_fingerprint() != self.context_fingerprint.as_ref()
                || row.arity() != context.index_count()
            {
                return Err(PersistentParametricEliminationError::ReplayMismatch {
                    detail: format!("persistent source-row scope differs at row {row_index}"),
                });
            }
            actual_source_integral_slots = checked_add(
                actual_source_integral_slots,
                row.terms().len(),
                "persistent retained source integral slots",
            )?;
            check_limit(
                "persistent retained source integral slots",
                actual_source_integral_slots,
                self.limits.max_retained_source_terms,
            )?;
            check_limit(
                "persistent input relation integral slots",
                actual_source_integral_slots,
                self.limits.elimination.max_input_terms,
            )?;
            for (shift, coefficient) in row.terms() {
                context
                    .validate_with_limits(
                        coefficient,
                        self.limits.elimination.arithmetic.exact_algebra,
                    )
                    .map_err(ParametricEliminationError::from)?;
                if !actual_columns.contains(shift) {
                    let requested_columns =
                        checked_add(actual_columns.len(), 1, "persistent input relation columns")?;
                    check_limit(
                        "persistent input relation columns",
                        requested_columns,
                        self.limits.elimination.max_columns,
                    )?;
                    let indices = self.ordering.shifted_indices(shift)?;
                    self.ordering
                        .policy()
                        .complexity_key(&indices)
                        .map_err(ParametricEliminationError::Sector)?;
                    actual_columns.insert(shift.clone());
                }
            }
            for (condition_index, condition) in row.guarded_nonzero_conditions().iter().enumerate()
            {
                if !context.contains_nonzero_condition(condition)
                    || condition.polynomial().is_zero()
                    || condition.origins().is_empty()
                {
                    return Err(PersistentParametricEliminationError::Elimination(
                        ParametricEliminationError::InvalidSourceGuard {
                            row: row_index,
                            condition: condition_index,
                        },
                    ));
                }
                context
                    .validate_polynomial_with_limits(
                        condition.polynomial(),
                        self.limits.elimination.arithmetic.exact_algebra,
                    )
                    .map_err(ParametricEliminationError::from)?;
                check_limit(
                    "persistent origins in one input relation guard",
                    condition.origins().len(),
                    self.limits.elimination.arithmetic.max_guard_origins,
                )?;
                actual_input_guards =
                    checked_add(actual_input_guards, 1, "persistent input relation guards")?;
                check_limit(
                    "persistent input relation guards",
                    actual_input_guards,
                    self.limits.elimination.max_input_guards,
                )?;
                actual_input_guard_origins = checked_add(
                    actual_input_guard_origins,
                    condition.origins().len(),
                    "persistent input relation guard origins",
                )?;
                check_limit(
                    "persistent input relation guard origins",
                    actual_input_guard_origins,
                    self.limits.elimination.max_input_guard_origins,
                )?;
            }
            let remaining = self
                .limits
                .max_retained_source_manifest_bytes
                .checked_sub(actual_source_manifest_bytes)
                .ok_or(PersistentParametricEliminationError::ResourceLimit {
                    resource: "persistent retained source manifest bytes",
                    requested: actual_source_manifest_bytes,
                    limit: self.limits.max_retained_source_manifest_bytes,
                })?;
            let actual_manifest_bytes = row.stable_manifest_with_limit(remaining)?.len();
            if actual_manifest_bytes != stored_manifest_bytes {
                return Err(PersistentParametricEliminationError::ReplayMismatch {
                    detail: format!(
                        "persistent row-manifest length differs at source row {row_index}"
                    ),
                });
            }
            actual_source_manifest_bytes = checked_add(
                actual_source_manifest_bytes,
                actual_manifest_bytes,
                "persistent retained source manifest bytes",
            )?;
        }
        check_limit(
            "persistent retained source manifest bytes",
            actual_source_manifest_bytes,
            self.limits.max_retained_source_manifest_bytes,
        )?;

        // Audit the complete batch table, including clone-local payload bounds,
        // before allocating any `Vec<ParametricRelation>` during replay.
        let mut expected_first_source_ordinal = 0usize;
        let mut actual_label_bytes = 0usize;
        check_limit(
            "persistent submitted batches",
            self.batches.len(),
            self.limits.max_submitted_batches,
        )?;
        for (expected_batch_ordinal, batch) in self.batches.iter().enumerate() {
            if batch.ordinal != expected_batch_ordinal {
                return Err(PersistentParametricEliminationError::ReplayMismatch {
                    detail: "persistent batch ordinals are not contiguous".to_owned(),
                });
            }
            if batch.first_source_ordinal != expected_first_source_ordinal {
                return Err(PersistentParametricEliminationError::ReplayMismatch {
                    detail: "persistent batch source ranges are not contiguous".to_owned(),
                });
            }
            if batch.source_row_count == 0 {
                return Err(PersistentParametricEliminationError::ReplayMismatch {
                    detail: "persistent certificate contains an empty batch".to_owned(),
                });
            }
            check_limit(
                "persistent rows in one submitted batch",
                batch.source_row_count,
                self.limits.max_rows_per_batch,
            )?;
            check_limit(
                "persistent replay batch clone rows",
                batch.source_row_count,
                self.limits.max_replay_batch_clone_rows,
            )?;
            check_limit(
                "persistent batch label bytes",
                batch.label.len(),
                self.limits.max_batch_label_bytes,
            )?;
            actual_label_bytes = checked_add(
                actual_label_bytes,
                batch.label.len(),
                "persistent retained batch label bytes",
            )?;
            check_limit(
                "persistent retained batch label bytes",
                actual_label_bytes,
                self.limits.max_retained_batch_label_bytes,
            )?;
            let range_end = batch
                .first_source_ordinal
                .checked_add(batch.source_row_count)
                .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                    detail: "persistent batch source range overflows usize".to_owned(),
                })?;
            expected_first_source_ordinal = range_end;
            let range = batch.first_source_ordinal..range_end;
            let borrowed_rows = self.source_rows.get(range.clone()).ok_or_else(|| {
                PersistentParametricEliminationError::ReplayMismatch {
                    detail: "persistent batch exceeds retained source rows".to_owned(),
                }
            })?;
            let manifest_lengths = self.source_manifest_lengths.get(range).ok_or_else(|| {
                PersistentParametricEliminationError::ReplayMismatch {
                    detail: "persistent batch exceeds the row-manifest length table".to_owned(),
                }
            })?;
            let clone_integral_slots = borrowed_rows.iter().try_fold(0usize, |total, row| {
                checked_add(
                    total,
                    row.terms().len(),
                    "persistent replay batch clone integral slots",
                )
            })?;
            let clone_manifest_bytes =
                manifest_lengths.iter().try_fold(0usize, |total, &length| {
                    checked_add(
                        total,
                        length,
                        "persistent replay batch clone manifest bytes",
                    )
                })?;
            check_limit(
                "persistent replay batch clone integral slots",
                clone_integral_slots,
                self.limits.max_replay_batch_clone_integral_slots,
            )?;
            check_limit(
                "persistent replay batch clone manifest bytes",
                clone_manifest_bytes,
                self.limits.max_replay_batch_clone_manifest_bytes,
            )?;
        }
        if expected_first_source_ordinal != self.source_rows.len() {
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent batches do not cover every retained source row".to_owned(),
            });
        }
        if self.stats.submitted_batches != self.batches.len()
            || self.stats.submitted_rows != self.source_rows.len()
            || self.stats.retained_source_integral_slots != actual_source_integral_slots
            || self.stats.retained_source_manifest_bytes != actual_source_manifest_bytes
            || self.stats.retained_batch_label_bytes != actual_label_bytes
            || self.stats.retained_input_guards != actual_input_guards
            || self.stats.retained_input_guard_origins != actual_input_guard_origins
            || self.stats.retained_input_columns != actual_columns.len()
        {
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent retained-payload statistics differ from actual rows".to_owned(),
            });
        }
        if self.events.len() != self.stats.consumed_rows {
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent event count differs from the consumed-row cursor".to_owned(),
            });
        }
        let mut audited_dependent_rows = 0usize;
        let mut audited_pivot_rows = 0usize;
        for (event_ordinal, event) in self.events.iter().enumerate() {
            let batch = self.batches.get(event.batch_ordinal).ok_or_else(|| {
                PersistentParametricEliminationError::ReplayMismatch {
                    detail: "persistent event refers to an absent batch".to_owned(),
                }
            })?;
            let batch_range = batch.try_source_range()?;
            if event.event_ordinal != event_ordinal
                || event.source_ordinal != event_ordinal
                || !batch_range.contains(&event.source_ordinal)
                || event.within_batch_ordinal != event.source_ordinal - batch.first_source_ordinal
            {
                return Err(PersistentParametricEliminationError::ReplayMismatch {
                    detail: "persistent event cursor or batch locator is inconsistent".to_owned(),
                });
            }
            match event.outcome {
                PersistentParametricEliminationRowOutcome::Dependent => {
                    audited_dependent_rows = checked_add(
                        audited_dependent_rows,
                        1,
                        "persistent audited dependent rows",
                    )?;
                }
                PersistentParametricEliminationRowOutcome::Pivot { pivot_ordinal } => {
                    if pivot_ordinal != audited_pivot_rows {
                        return Err(PersistentParametricEliminationError::ReplayMismatch {
                            detail: "persistent pivot-event ordinals are not contiguous".to_owned(),
                        });
                    }
                    audited_pivot_rows =
                        checked_add(audited_pivot_rows, 1, "persistent audited pivot rows")?;
                }
            }
        }
        if self.stats.dependent_rows != audited_dependent_rows
            || self.stats.pivot_rows != audited_pivot_rows
            || self.stats.rebuilds != self.stats.consumed_rows
            || self
                .elimination
                .as_ref()
                .map_or(0, |elimination| elimination.pivots().len())
                != audited_pivot_rows
            || (self.stats.consumed_rows == 0) != self.elimination.is_none()
        {
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent consumed-event statistics or elimination differ".to_owned(),
            });
        }
        check_limit(
            "persistent replay coexisting source clone rows",
            checked_mul(
                self.source_rows.len(),
                2,
                "persistent replay coexisting source clone rows",
            )?,
            self.limits.max_replay_source_clone_rows,
        )?;
        check_limit(
            "persistent replay coexisting source clone integral slots",
            checked_mul(
                actual_source_integral_slots,
                2,
                "persistent replay coexisting source clone integral slots",
            )?,
            self.limits.max_replay_source_clone_integral_slots,
        )?;
        check_limit(
            "persistent replay coexisting source clone manifest bytes",
            checked_mul(
                actual_source_manifest_bytes,
                2,
                "persistent replay coexisting source clone manifest bytes",
            )?,
            self.limits.max_replay_source_clone_manifest_bytes,
        )?;

        let mut replay = PersistentParametricEliminationDatabase::try_new(
            context,
            self.family_fingerprint.clone(),
            self.ordering.clone(),
            self.limits,
        )?;
        let certified_elimination_manifest_bytes = self
            .elimination
            .as_ref()
            .map_or(0, |elimination| elimination.source_manifest().len());
        let mut event_cursor = 0usize;
        for batch in self.batches.iter() {
            let borrowed_rows =
                self.source_rows
                    .get(batch.try_source_range()?)
                    .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                        detail: "persistent batch exceeds retained source rows after preflight"
                            .to_owned(),
                    })?;
            // This is the only replay-owned deep source clone.  The complete
            // certificate and batch payload were authenticated above.
            replay.submit_prevalidated_rows(batch.label.clone(), borrowed_rows.to_vec())?;
            while let Some(expected) = self.events.get(event_cursor) {
                if expected.batch_ordinal != batch.ordinal {
                    break;
                }
                let actual = replay
                    .consume_next_with_outer_elimination_manifest_bytes(
                        certified_elimination_manifest_bytes,
                    )?
                    .ok_or_else(|| PersistentParametricEliminationError::ReplayMismatch {
                        detail: "persistent replay ended before its event transcript".to_owned(),
                    })?;
                if &actual != expected {
                    return Err(PersistentParametricEliminationError::ReplayMismatch {
                        detail: "persistent row event differs during replay".to_owned(),
                    });
                }
                event_cursor = checked_add(event_cursor, 1, "persistent replay event cursor")?;
            }
        }
        if event_cursor != self.events.len() {
            return Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent event transcript refers to an absent batch".to_owned(),
            });
        }
        let replayed = replay.finish();
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(PersistentParametricEliminationError::ReplayMismatch {
                detail: "persistent certificate payload differs during replay".to_owned(),
            })
        }
    }

    fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.ordering == other.ordering
            && self.limits == other.limits
            && relations_eq(&self.source_rows, &other.source_rows)
            && self.source_manifest_lengths == other.source_manifest_lengths
            && self.batches == other.batches
            && self.events == other.events
            && optional_elimination_eq(self.elimination.as_ref(), other.elimination.as_ref())
            && self.stats == other.stats
            && self.interrupted == other.interrupted
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistentParametricEliminationError {
    EmptyBatch,
    PendingBatchNotConsumed {
        pending: usize,
    },
    DatabaseInterrupted,
    WrongFamily {
        row: usize,
    },
    WrongContext {
        row: usize,
    },
    WrongArity {
        expected: usize,
        actual: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Elimination(ParametricEliminationError),
    Relation(ParametricRelationError),
    ReplayMismatch {
        detail: String,
    },
}

impl fmt::Display for PersistentParametricEliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBatch => write!(formatter, "persistent elimination batch is empty"),
            Self::PendingBatchNotConsumed { pending } => write!(
                formatter,
                "persistent elimination has {pending} unconsumed pending rows"
            ),
            Self::DatabaseInterrupted => {
                write!(formatter, "persistent elimination database is interrupted")
            }
            Self::WrongFamily { row } => {
                write!(
                    formatter,
                    "persistent source row {row} belongs to another family"
                )
            }
            Self::WrongContext { row } => {
                write!(
                    formatter,
                    "persistent source row {row} belongs to another K(n) context"
                )
            }
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "persistent elimination expected arity {expected}, received {actual}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "persistent elimination resource limit exceeded for {resource}: requested {requested}, limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "persistent elimination resource count overflow for {resource}"
            ),
            Self::Elimination(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::ReplayMismatch { detail } => {
                write!(
                    formatter,
                    "persistent elimination replay mismatch: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for PersistentParametricEliminationError {}

impl From<ParametricEliminationError> for PersistentParametricEliminationError {
    fn from(value: ParametricEliminationError) -> Self {
        Self::Elimination(value)
    }
}

impl From<ParametricRelationError> for PersistentParametricEliminationError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

fn pivot_prefix_eq(left: &[ParametricPivotEquation], right: &[ParametricPivotEquation]) -> bool {
    left.iter().zip(right).all(|(left, right)| {
        left.ordinal() == right.ordinal()
            && left.pivot() == right.pivot()
            && left.trace() == right.trace()
            && left
                .unit_relation()
                .has_identical_guard_provenance(right.unit_relation())
    })
}

fn elimination_eq(left: &ParametricElimination, right: &ParametricElimination) -> bool {
    left.family_fingerprint() == right.family_fingerprint()
        && left.context_fingerprint() == right.context_fingerprint()
        && left.source_manifest() == right.source_manifest()
        && left.ordering() == right.ordering()
        && left.limits() == right.limits()
        && left.columns_easiest_first() == right.columns_easiest_first()
        && left.free_columns() == right.free_columns()
        && left.stats() == right.stats()
        && left.pivots().len() == right.pivots().len()
        && pivot_prefix_eq(left.pivots(), right.pivots())
}

fn optional_elimination_eq(
    left: Option<&ParametricElimination>,
    right: Option<&ParametricElimination>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => elimination_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn relations_eq(left: &[ParametricRelation], right: &[ParametricRelation]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.has_identical_guard_provenance(right))
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, PersistentParametricEliminationError> {
    left.checked_add(right)
        .ok_or(PersistentParametricEliminationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, PersistentParametricEliminationError> {
    left.checked_mul(right)
        .ok_or(PersistentParametricEliminationError::ResourceCountOverflow { resource })
}

fn prefix_source_manifest_bytes(
    source_rows: usize,
    row_component_bytes: usize,
) -> Result<usize, PersistentParametricEliminationError> {
    let header_bytes = checked_add(
        checked_add(
            PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA.len(),
            "|rows=".len(),
            "persistent source manifest bytes",
        )?,
        source_rows.to_string().len(),
        "persistent source manifest bytes",
    )?;
    checked_add(
        header_bytes,
        row_component_bytes,
        "persistent source manifest bytes",
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), PersistentParametricEliminationError> {
    if requested > limit {
        Err(PersistentParametricEliminationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, CoefficientContext, IntegralFamily, IntegralOrderingPolicy,
        ParametricCoefficientContext, ParametricIbpGenerator,
    };

    fn pending_certificate() -> (
        ParametricCoefficientContext,
        PersistentParametricEliminationCertificate,
    ) {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let family = IntegralFamily::new(
            "persistent-private-replay-audit",
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parameter("m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let context = generated.context().clone();
        let row = generated.ibp_li().next().unwrap().clone();
        let mut database = PersistentParametricEliminationDatabase::try_new(
            generated.context(),
            family.fingerprint(),
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap(),
            PersistentParametricEliminationLimits::default(),
        )
        .unwrap();
        database
            .submit_prevalidated_rows("pending", vec![row])
            .unwrap();
        (context, database.finish())
    }

    #[test]
    fn replay_rejects_tampered_row_manifest_metadata() {
        let (context, mut certificate) = pending_certificate();
        certificate.source_manifest_lengths[0] += 1;

        assert!(matches!(
            certificate.replay(&context),
            Err(PersistentParametricEliminationError::ReplayMismatch { detail })
                if detail.contains("row-manifest length differs")
        ));
    }

    #[test]
    fn replay_rejects_tampered_retained_statistics() {
        let (context, mut certificate) = pending_certificate();
        certificate.stats.retained_source_integral_slots = 0;

        assert!(matches!(
            certificate.replay(&context),
            Err(PersistentParametricEliminationError::ReplayMismatch { detail })
                if detail.contains("retained-payload statistics")
        ));
    }

    #[test]
    fn replay_rejects_noncontiguous_batch_ranges() {
        let (context, mut certificate) = pending_certificate();
        certificate.batches[0].first_source_ordinal = 1;

        assert!(matches!(
            certificate.replay(&context),
            Err(PersistentParametricEliminationError::ReplayMismatch { detail })
                if detail.contains("source ranges are not contiguous")
        ));
    }

    #[test]
    fn fallible_pending_cursor_rejects_tampered_metadata() {
        let (_context, mut certificate) = pending_certificate();
        certificate.stats.consumed_rows = certificate.source_rows.len() + 1;

        assert_eq!(certificate.pending_rows(), 0);
        assert!(matches!(
            certificate.try_pending_rows(),
            Err(PersistentParametricEliminationError::ReplayMismatch { detail })
                if detail.contains("cursor exceeds retained rows")
        ));
    }
}
