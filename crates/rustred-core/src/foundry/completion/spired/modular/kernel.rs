use std::cmp::Ordering;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::identity::TranslatedSourceRequest;

use super::limits::{
    FORBIDDEN_COLUMNS, NEW_COLUMN_POSITIONS, REDUCER_DENSE_SCAN_WORK, REDUCER_ENTRIES,
    REDUCER_SCRATCH_CELLS, REQUEST_SHIFT_COMPONENTS, RETAINED_NONZEROS, ROW_COLUMNS,
    ROW_STRUCTURAL_TERMS, ROW_VALUES, ROWS, STRUCTURAL_TERMS, SUPPORT_REQUESTS, TRACE_EDGES,
    TRACE_NODES, TRACE_SHIFT_COMPONENTS, TRACE_WORKSPACE, check_limit, checked_add, checked_u32,
    maximum_native_row_growth, try_reserve, try_vec,
};
use super::{
    SpiredForbiddenTerm, SpiredModularError, SpiredModularHit, SpiredModularLimits,
    SpiredModularRow,
};

#[derive(Debug)]
struct TraceNode {
    source: TranslatedSourceRequest,
    direct_dependencies: Box<[usize]>,
}

/// A Symbolica prime field validated before native sparse-reducer allocation.
#[derive(Debug)]
pub(crate) struct SpiredValidatedPrime {
    field: Zp64,
}

impl SpiredValidatedPrime {
    pub(crate) fn try_new(modulus: u64) -> Result<Self, SpiredModularError> {
        validate_prime(modulus)?;
        let field = catch_unwind(AssertUnwindSafe(|| Zp64::new(modulus))).map_err(|_| {
            SpiredModularError::NativePanic {
                operation: "constructing a validated finite field",
            }
        })?;
        Ok(Self { field })
    }

    pub(crate) const fn field(&self) -> &Zp64 {
        &self.field
    }
}

/// Incremental two-rank probe over one prime and point.
///
/// Forbidden identities are kept in canonical `Column` order. A column first
/// encountered after prior rows is safe to insert because the input contract
/// reports every structurally present forbidden term: its prior absence is a
/// proof of historical structural zero. A previously reported zero column is
/// already registered and is never inserted again when a later value becomes
/// nonzero.
#[derive(Debug)]
pub(crate) struct SpiredModularKernel<Column> {
    modulus: u64,
    field: Zp64,
    limits: SpiredModularLimits,
    forbidden_columns: Vec<Column>,
    /// Live native state is taken and dropped as soon as the lane poisons.
    /// This prevents both over-limit fill and split reducer state from
    /// remaining observable through the monitoring accessors.
    forbidden: Option<SparseRowReducer<Zp64>>,
    augmented: Option<SparseRowReducer<Zp64>>,
    trace_nodes: Vec<TraceNode>,
    rows_consumed: usize,
    structural_terms: usize,
    retained_nonzeros: usize,
    reducer_dense_scan_work: usize,
    trace_edges: usize,
    retained_request_shift_components: usize,
    poisoned: bool,
    hit: bool,
}

impl<Column: Ord + Clone> SpiredModularKernel<Column> {
    pub(crate) fn try_new(
        modulus: u64,
        limits: SpiredModularLimits,
    ) -> Result<Self, SpiredModularError> {
        let prime = SpiredValidatedPrime::try_new(modulus)?;
        Self::try_new_with_validated_prime(prime, limits)
    }

    /// Consume one already-validated field and allocate the two native
    /// reducers without repeating primality testing or field construction.
    pub(crate) fn try_new_with_validated_prime(
        prime: SpiredValidatedPrime,
        limits: SpiredModularLimits,
    ) -> Result<Self, SpiredModularError> {
        let field = prime.field;
        let modulus = field.get_prime();
        let (forbidden, augmented) = catch_unwind(AssertUnwindSafe(|| {
            let forbidden = SparseRowReducer::new(0, field.clone(), LuLMode::Pattern);
            // The target exists from construction and is always the last
            // logical column of this reducer.
            let augmented = SparseRowReducer::new(1, field.clone(), LuLMode::Pattern);
            (forbidden, augmented)
        }))
        .map_err(|_| SpiredModularError::NativePanic {
            operation: "constructing streaming sparse reducers",
        })?;
        Ok(Self {
            modulus,
            field,
            limits,
            forbidden_columns: Vec::new(),
            forbidden: Some(forbidden),
            augmented: Some(augmented),
            trace_nodes: Vec::new(),
            rows_consumed: 0,
            structural_terms: 0,
            retained_nonzeros: 0,
            reducer_dense_scan_work: 0,
            trace_edges: 0,
            retained_request_shift_components: 0,
            poisoned: false,
            hit: false,
        })
    }

    pub(crate) const fn modulus(&self) -> u64 {
        self.modulus
    }

    pub(crate) const fn rows_consumed(&self) -> usize {
        self.rows_consumed
    }

    pub(crate) fn forbidden_columns(&self) -> &[Column] {
        self.forbidden_columns.as_slice()
    }

    pub(crate) const fn target_logical_column(&self) -> usize {
        self.forbidden_columns.len()
    }

    pub(crate) fn forbidden_rank(&self) -> usize {
        self.forbidden
            .as_ref()
            .map_or(0, |reducer| reducer.u().nrows() as usize)
    }

    pub(crate) fn augmented_rank(&self) -> usize {
        self.augmented
            .as_ref()
            .map_or(0, |reducer| reducer.u().nrows() as usize)
    }

    pub(crate) const fn has_hit(&self) -> bool {
        self.hit
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Pre-register a canonical union of forbidden columns which are known to
    /// have been structurally zero in every row already consumed by this lane.
    ///
    /// `columns` must be strictly increasing. Existing columns may be repeated
    /// across calls; the method inserts only the missing set and returns its
    /// cardinality. All missing columns are inserted into each Symbolica
    /// reducer in one `add_cols` call, so old pivots move together and the
    /// augmented target remains last. Claiming a late column through this API
    /// is sound only when the sealed coordinator has established its complete
    /// historical structural-zero property. Ordinary streamed rows retain the
    /// independent dynamic-registration fallback.
    pub(crate) fn try_preregister_historical_zero_columns(
        &mut self,
        columns: &[Column],
    ) -> Result<usize, SpiredModularError> {
        if self.poisoned {
            return Err(SpiredModularError::Poisoned);
        }
        if self.hit {
            return Err(SpiredModularError::AlreadyHit);
        }
        check_limit(
            FORBIDDEN_COLUMNS,
            columns.len(),
            self.limits.max_forbidden_columns,
        )?;
        for (first_column, pair) in columns.windows(2).enumerate() {
            if pair[0] >= pair[1] {
                return Err(SpiredModularError::NonCanonicalForbiddenColumnBatch {
                    first_column,
                    second_column: first_column + 1,
                });
            }
        }

        // Both registries are strictly sorted. Count the missing union first
        // so the common all-existing case remains allocation- and clone-free.
        let mut existing_ordinal = 0usize;
        let mut added = 0usize;
        for column in columns {
            while self
                .forbidden_columns
                .get(existing_ordinal)
                .is_some_and(|existing| existing < column)
            {
                existing_ordinal += 1;
            }
            if self.forbidden_columns.get(existing_ordinal) == Some(column) {
                existing_ordinal += 1;
            } else {
                added = checked_add(FORBIDDEN_COLUMNS, added, 1)?;
            }
        }
        let next_forbidden_columns =
            checked_add(FORBIDDEN_COLUMNS, self.forbidden_columns.len(), added)?;
        check_limit(
            FORBIDDEN_COLUMNS,
            next_forbidden_columns,
            self.limits.max_forbidden_columns,
        )?;
        checked_u32(FORBIDDEN_COLUMNS, next_forbidden_columns)?;
        let augmented_columns = checked_add(FORBIDDEN_COLUMNS, next_forbidden_columns, 1)?;
        checked_u32(FORBIDDEN_COLUMNS, augmented_columns)?;
        let reducer_width = checked_add(
            REDUCER_SCRATCH_CELLS,
            next_forbidden_columns,
            augmented_columns,
        )?;
        check_limit(
            REDUCER_SCRATCH_CELLS,
            reducer_width,
            self.limits.max_reducer_scratch_cells,
        )?;
        if added == 0 {
            return Ok(0);
        }

        let mut new_columns = try_vec(FORBIDDEN_COLUMNS, added)?;
        let mut insertion_positions = try_vec(NEW_COLUMN_POSITIONS, added)?;
        existing_ordinal = 0;
        for column in columns {
            while self
                .forbidden_columns
                .get(existing_ordinal)
                .is_some_and(|existing| existing < column)
            {
                existing_ordinal += 1;
            }
            if self.forbidden_columns.get(existing_ordinal) == Some(column) {
                existing_ordinal += 1;
            } else {
                new_columns.push(column.clone());
                insertion_positions.push(checked_u32(NEW_COLUMN_POSITIONS, existing_ordinal)?);
            }
        }
        if new_columns.len() != added || insertion_positions.len() != added {
            return Err(SpiredModularError::Invariant {
                detail: "pre-registered column union changed its missing cardinality",
            });
        }
        let prospective_columns =
            try_merge_sorted_columns(&self.forbidden_columns, new_columns, next_forbidden_columns)?;

        let Some(forbidden) = self.forbidden.as_mut() else {
            return Err(SpiredModularError::Poisoned);
        };
        let Some(augmented) = self.augmented.as_mut() else {
            return Err(SpiredModularError::Poisoned);
        };
        let insertion = catch_unwind(AssertUnwindSafe(|| {
            forbidden.add_cols(&insertion_positions);
            augmented.add_cols(&insertion_positions);
        }));
        if insertion.is_err() {
            self.poison();
            return Err(SpiredModularError::NativePanic {
                operation: "pre-registering historical-zero forbidden columns",
            });
        }
        self.forbidden_columns = prospective_columns;
        if let Err(error) = self.check_dimensions() {
            self.poison();
            return Err(error);
        }
        if let Err(error) = self.enforce_native_fill_after_mutation() {
            return Err(error);
        }
        Ok(added)
    }

    /// Admit exactly one classified row to each reducer and stop at the first
    /// checked target-rank gain.
    pub(crate) fn try_push_row(
        &mut self,
        mut row: SpiredModularRow<Column>,
    ) -> Result<Option<SpiredModularHit>, SpiredModularError> {
        if self.poisoned {
            return Err(SpiredModularError::Poisoned);
        }
        if self.hit {
            return Err(SpiredModularError::AlreadyHit);
        }

        let row_ordinal = self.rows_consumed;
        let next_rows = checked_add(ROWS, self.rows_consumed, 1)?;
        check_limit(ROWS, next_rows, self.limits.max_rows)?;
        checked_u32(ROWS, next_rows)?;
        check_limit(
            REQUEST_SHIFT_COMPONENTS,
            row.source.offset().len(),
            self.limits.max_request_shift_components,
        )?;
        check_limit(
            ROW_STRUCTURAL_TERMS,
            row.forbidden_terms.len(),
            self.limits.max_structural_terms_per_row,
        )?;
        let next_structural = checked_add(
            STRUCTURAL_TERMS,
            self.structural_terms,
            row.forbidden_terms.len(),
        )?;
        check_limit(
            STRUCTURAL_TERMS,
            next_structural,
            self.limits.max_structural_terms,
        )?;
        validate_residue(row_ordinal, None, row.target_residue, self.modulus)?;

        let mut indexed_terms = try_vec(ROW_STRUCTURAL_TERMS, row.forbidden_terms.len())?;
        indexed_terms.extend(row.forbidden_terms.drain(..).enumerate());
        indexed_terms.sort_unstable_by(|left, right| {
            left.1
                .column
                .cmp(&right.1.column)
                .then_with(|| left.0.cmp(&right.0))
        });
        for (sorted, pair) in indexed_terms.windows(2).enumerate() {
            if pair[0].1.column == pair[1].1.column {
                return Err(SpiredModularError::DuplicateForbiddenTerm {
                    row: row_ordinal,
                    first_term: pair[0].0,
                    second_term: pair[1].0,
                });
            }
            debug_assert!(
                pair[0].1.column < pair[1].1.column,
                "validated structural terms must be strictly ordered at {sorted}"
            );
        }
        for (original_ordinal, term) in &indexed_terms {
            validate_residue(
                row_ordinal,
                Some(*original_ordinal),
                term.residue,
                self.modulus,
            )?;
        }

        let mut new_columns = try_vec(NEW_COLUMN_POSITIONS, indexed_terms.len())?;
        let mut insertion_positions = try_vec(NEW_COLUMN_POSITIONS, indexed_terms.len())?;
        for (_, term) in &indexed_terms {
            if let Err(position) = self.forbidden_columns.binary_search(&term.column) {
                new_columns.push(term.column.clone());
                insertion_positions.push(checked_u32(NEW_COLUMN_POSITIONS, position)?);
            }
        }
        let next_forbidden_columns = checked_add(
            FORBIDDEN_COLUMNS,
            self.forbidden_columns.len(),
            new_columns.len(),
        )?;
        check_limit(
            FORBIDDEN_COLUMNS,
            next_forbidden_columns,
            self.limits.max_forbidden_columns,
        )?;
        checked_u32(FORBIDDEN_COLUMNS, next_forbidden_columns)?;
        let augmented_columns = checked_add(FORBIDDEN_COLUMNS, next_forbidden_columns, 1)?;
        checked_u32(FORBIDDEN_COLUMNS, augmented_columns)?;

        let reducer_width = checked_add(
            REDUCER_SCRATCH_CELLS,
            next_forbidden_columns,
            augmented_columns,
        )?;
        check_limit(
            REDUCER_SCRATCH_CELLS,
            reducer_width,
            self.limits.max_reducer_scratch_cells,
        )?;
        let next_dense_scan_work = checked_add(
            REDUCER_DENSE_SCAN_WORK,
            self.reducer_dense_scan_work,
            reducer_width,
        )?;
        check_limit(
            REDUCER_DENSE_SCAN_WORK,
            next_dense_scan_work,
            self.limits.max_reducer_dense_scan_work,
        )?;

        let forbidden_row_nonzeros = indexed_terms
            .iter()
            .filter(|(_, term)| term.residue != 0)
            .count();
        let row_nonzeros = checked_add(
            RETAINED_NONZEROS,
            forbidden_row_nonzeros,
            usize::from(row.target_residue != 0),
        )?;
        let next_nonzeros = checked_add(RETAINED_NONZEROS, self.retained_nonzeros, row_nonzeros)?;
        check_limit(
            RETAINED_NONZEROS,
            next_nonzeros,
            self.limits.max_retained_nonzeros,
        )?;

        let forbidden_before = self.forbidden_rank();
        let augmented_before = self.augmented_rank();
        let current_native_entries = self.native_entry_count()?;
        let forbidden_growth = maximum_native_row_growth(
            REDUCER_ENTRIES,
            next_forbidden_columns,
            forbidden_before,
            forbidden_row_nonzeros,
        )?;
        let augmented_growth = maximum_native_row_growth(
            REDUCER_ENTRIES,
            augmented_columns,
            augmented_before,
            row_nonzeros,
        )?;
        let maximum_native_growth =
            checked_add(REDUCER_ENTRIES, forbidden_growth, augmented_growth)?;
        let maximum_native_entries = checked_add(
            REDUCER_ENTRIES,
            current_native_entries,
            maximum_native_growth,
        )?;
        check_limit(
            REDUCER_ENTRIES,
            maximum_native_entries,
            self.limits.max_reducer_entries,
        )?;

        // Finish every fallible Rust-side allocation and index conversion
        // before mutating either native reducer. The common no-new-column path
        // borrows the existing registry and performs no whole-registry clone
        // or sort. New columns are already canonical and are merged linearly.
        let prospective_columns = if new_columns.is_empty() {
            None
        } else {
            Some(try_merge_sorted_columns(
                &self.forbidden_columns,
                new_columns,
                next_forbidden_columns,
            )?)
        };
        let column_registry = prospective_columns
            .as_deref()
            .unwrap_or(self.forbidden_columns.as_slice());

        let mut forbidden_values = try_vec(ROW_VALUES, row_nonzeros)?;
        let mut forbidden_indices = try_vec(ROW_COLUMNS, row_nonzeros)?;
        for (_, term) in &indexed_terms {
            if term.residue == 0 {
                continue;
            }
            let column = column_registry.binary_search(&term.column).map_err(|_| {
                SpiredModularError::Invariant {
                    detail: "a structurally registered forbidden column disappeared",
                }
            })?;
            forbidden_values.push(self.field.to_element(term.residue));
            forbidden_indices.push(checked_u32(ROW_COLUMNS, column)?);
        }
        let mut augmented_values = try_vec(ROW_VALUES, row_nonzeros)?;
        augmented_values.extend_from_slice(&forbidden_values);
        let mut augmented_indices = try_vec(ROW_COLUMNS, row_nonzeros)?;
        augmented_indices.extend_from_slice(&forbidden_indices);
        let target_column = next_forbidden_columns;
        if row.target_residue != 0 {
            augmented_values.push(self.field.to_element(row.target_residue));
            augmented_indices.push(checked_u32(ROW_COLUMNS, target_column)?);
        }

        if !insertion_positions.is_empty() {
            let Some(forbidden) = self.forbidden.as_mut() else {
                return Err(SpiredModularError::Poisoned);
            };
            let Some(augmented) = self.augmented.as_mut() else {
                return Err(SpiredModularError::Poisoned);
            };
            let insertion = catch_unwind(AssertUnwindSafe(|| {
                // Positions are relative to the old forbidden ordering. The
                // same positions insert immediately before the augmented
                // target, including repeated end positions.
                forbidden.add_cols(&insertion_positions);
                augmented.add_cols(&insertion_positions);
            }));
            if insertion.is_err() {
                self.poison();
                return Err(SpiredModularError::NativePanic {
                    operation: "inserting historical-zero forbidden columns",
                });
            }
        }
        if let Some(prospective_columns) = prospective_columns {
            self.forbidden_columns = prospective_columns;
        }
        if let Err(error) = self.check_dimensions() {
            self.poison();
            return Err(error);
        }

        let Some(forbidden) = self.forbidden.as_mut() else {
            return Err(SpiredModularError::Poisoned);
        };
        let Some(augmented) = self.augmented.as_mut() else {
            return Err(SpiredModularError::Poisoned);
        };
        let native = catch_unwind(AssertUnwindSafe(|| {
            let forbidden_pivot = forbidden.add_row(&forbidden_values, &forbidden_indices);
            let augmented_pivot = augmented.add_row(&augmented_values, &augmented_indices);
            (forbidden_pivot, augmented_pivot)
        }));
        let (forbidden_pivot, augmented_pivot) = match native {
            Ok(result) => result,
            Err(_) => {
                self.poison();
                return Err(SpiredModularError::NativePanic {
                    operation: "forward-reducing one streamed row",
                });
            }
        };
        self.rows_consumed = next_rows;
        self.structural_terms = next_structural;
        self.retained_nonzeros = next_nonzeros;
        self.reducer_dense_scan_work = next_dense_scan_work;

        let forbidden_after = self.forbidden_rank();
        let augmented_after = self.augmented_rank();
        if let Err(error) = validate_rank_step(
            forbidden_before,
            augmented_before,
            forbidden_after,
            augmented_after,
            forbidden_pivot,
            augmented_pivot,
            target_column,
        ) {
            self.poison();
            return Err(error);
        }

        let trace_root = if augmented_pivot.is_some() {
            match self.capture_trace_node(row.source, augmented_before) {
                Ok(root) => Some(root),
                Err(error) => {
                    self.poison();
                    return Err(error);
                }
            }
        } else {
            None
        };
        if let Err(error) = self.enforce_native_fill_after_mutation() {
            return Err(error);
        }

        let hit_now = augmented_after == forbidden_after + 1;
        if !hit_now {
            return Ok(None);
        }
        let root = match trace_root {
            Some(root) => root,
            None => {
                self.poison();
                return Err(SpiredModularError::Invariant {
                    detail: "target-rank gain has no accepted augmented trace root",
                });
            }
        };
        let hit = match self.build_hit(root, target_column) {
            Ok(hit) => hit,
            Err(error) => {
                self.poison();
                return Err(error);
            }
        };
        self.hit = true;
        Ok(Some(hit))
    }

    fn check_dimensions(&self) -> Result<(), SpiredModularError> {
        let forbidden = self
            .forbidden
            .as_ref()
            .ok_or(SpiredModularError::Poisoned)?;
        let augmented = self
            .augmented
            .as_ref()
            .ok_or(SpiredModularError::Poisoned)?;
        if forbidden.u().ncols() as usize != self.forbidden_columns.len() {
            return Err(SpiredModularError::Invariant {
                detail: "forbidden reducer width disagrees with its structural registry",
            });
        }
        if augmented.u().ncols() as usize != self.forbidden_columns.len() + 1 {
            return Err(SpiredModularError::Invariant {
                detail: "augmented reducer did not retain the target as its last column",
            });
        }
        Ok(())
    }

    fn capture_trace_node(
        &mut self,
        source: TranslatedSourceRequest,
        prior_augmented_rank: usize,
    ) -> Result<usize, SpiredModularError> {
        if self.trace_nodes.len() != prior_augmented_rank {
            return Err(SpiredModularError::Invariant {
                detail: "augmented basis rank and dependency trace diverged",
            });
        }
        let lower = self
            .augmented
            .as_ref()
            .ok_or(SpiredModularError::Poisoned)?
            .l();
        let row = (lower.nrows() as usize)
            .checked_sub(1)
            .ok_or(SpiredModularError::Invariant {
                detail: "accepted augmented row has no L-pattern row",
            })?;
        let start = *lower
            .row_ptrs()
            .get(row)
            .ok_or(SpiredModularError::Invariant {
                detail: "L-pattern row start is absent",
            })?;
        let end = *lower
            .row_ptrs()
            .get(row + 1)
            .ok_or(SpiredModularError::Invariant {
                detail: "L-pattern row end is absent",
            })?;
        let pattern = lower
            .col_idcs()
            .get(start..end)
            .ok_or(SpiredModularError::Invariant {
                detail: "L-pattern row range is invalid",
            })?;
        let mut dependencies = try_vec(TRACE_EDGES, pattern.len())?;
        let mut diagonal_count = 0usize;
        for &raw in pattern {
            let dependency = raw as usize;
            if dependency < prior_augmented_rank {
                dependencies.push(dependency);
            } else if dependency == prior_augmented_rank {
                diagonal_count = checked_add(TRACE_EDGES, diagonal_count, 1)?;
            } else {
                return Err(SpiredModularError::Invariant {
                    detail: "L-pattern references a future augmented basis row",
                });
            }
        }
        if diagonal_count != 1 {
            return Err(SpiredModularError::Invariant {
                detail: "accepted augmented L-pattern row lacks one diagonal entry",
            });
        }
        dependencies.sort_unstable();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SpiredModularError::Invariant {
                detail: "augmented L-pattern repeats a direct dependency",
            });
        }
        let next_nodes = checked_add(TRACE_NODES, self.trace_nodes.len(), 1)?;
        check_limit(TRACE_NODES, next_nodes, self.limits.max_trace_nodes)?;
        let next_edges = checked_add(TRACE_EDGES, self.trace_edges, dependencies.len())?;
        check_limit(TRACE_EDGES, next_edges, self.limits.max_trace_edges)?;
        let next_shift_components = checked_add(
            TRACE_SHIFT_COMPONENTS,
            self.retained_request_shift_components,
            source.offset().len(),
        )?;
        check_limit(
            TRACE_SHIFT_COMPONENTS,
            next_shift_components,
            self.limits.max_retained_request_shift_components,
        )?;
        try_reserve(&mut self.trace_nodes, 1, TRACE_NODES)?;
        let root = self.trace_nodes.len();
        self.trace_nodes.push(TraceNode {
            source,
            direct_dependencies: dependencies.into_boxed_slice(),
        });
        self.trace_edges = next_edges;
        self.retained_request_shift_components = next_shift_components;
        Ok(root)
    }

    fn build_hit(
        &self,
        root: usize,
        target_logical_column: usize,
    ) -> Result<SpiredModularHit, SpiredModularError> {
        let root_node = self
            .trace_nodes
            .get(root)
            .ok_or(SpiredModularError::Invariant {
                detail: "target trace root is outside the augmented basis",
            })?;
        check_limit(
            SUPPORT_REQUESTS,
            root_node.direct_dependencies.len(),
            self.limits.max_support_requests,
        )?;
        let mut direct = try_vec(SUPPORT_REQUESTS, root_node.direct_dependencies.len())?;
        for &dependency in root_node.direct_dependencies.iter() {
            direct.push(
                self.trace_nodes
                    .get(dependency)
                    .ok_or(SpiredModularError::Invariant {
                        detail: "direct dependency is outside the trace arena",
                    })?
                    .source
                    .clone(),
            );
        }
        direct.sort_unstable();
        direct.dedup();

        let mut visited = try_vec(TRACE_WORKSPACE, self.trace_nodes.len())?;
        visited.resize(self.trace_nodes.len(), false);
        let mut stack = try_vec(TRACE_WORKSPACE, self.trace_nodes.len())?;
        *visited.get_mut(root).ok_or(SpiredModularError::Invariant {
            detail: "target trace root is outside the DFS workspace",
        })? = true;
        stack.push(root);
        let mut support = try_vec(
            SUPPORT_REQUESTS,
            usize::min(self.trace_nodes.len(), self.limits.max_support_requests),
        )?;
        while let Some(node) = stack.pop() {
            let trace = self
                .trace_nodes
                .get(node)
                .ok_or(SpiredModularError::Invariant {
                    detail: "DFS trace node is absent",
                })?;
            let next_support = checked_add(SUPPORT_REQUESTS, support.len(), 1)?;
            check_limit(
                SUPPORT_REQUESTS,
                next_support,
                self.limits.max_support_requests,
            )?;
            support.push(trace.source.clone());
            for &dependency in trace.direct_dependencies.iter().rev() {
                if dependency >= node {
                    return Err(SpiredModularError::Invariant {
                        detail: "dependency trace is not strictly acyclic",
                    });
                }
                let scheduled =
                    visited
                        .get_mut(dependency)
                        .ok_or(SpiredModularError::Invariant {
                            detail: "DFS dependency is outside the trace arena",
                        })?;
                if !*scheduled {
                    *scheduled = true;
                    stack.push(dependency);
                }
            }
        }
        support.sort_unstable();
        support.dedup();
        Ok(SpiredModularHit {
            rows_consumed: self.rows_consumed,
            forbidden_rank: self.forbidden_rank(),
            augmented_rank: self.augmented_rank(),
            target_logical_column,
            direct_dependencies: direct.into_boxed_slice(),
            support: support.into_boxed_slice(),
        })
    }

    fn native_entry_count(&self) -> Result<usize, SpiredModularError> {
        let forbidden = self
            .forbidden
            .as_ref()
            .ok_or(SpiredModularError::Poisoned)?;
        let augmented = self
            .augmented
            .as_ref()
            .ok_or(SpiredModularError::Poisoned)?;
        let entries = [
            forbidden.u().nvalues(),
            forbidden.l().col_idcs().len(),
            augmented.u().nvalues(),
            augmented.l().col_idcs().len(),
        ]
        .into_iter()
        .try_fold(0usize, |total, count| {
            checked_add(REDUCER_ENTRIES, total, count)
        })?;
        Ok(entries)
    }

    fn check_native_fill(&self) -> Result<(), SpiredModularError> {
        let entries = self.native_entry_count()?;
        check_limit(REDUCER_ENTRIES, entries, self.limits.max_reducer_entries)
    }

    fn enforce_native_fill_after_mutation(&mut self) -> Result<(), SpiredModularError> {
        match self.check_native_fill() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.poison();
                Err(error)
            }
        }
    }

    fn poison(&mut self) {
        self.poisoned = true;
        self.hit = false;
        drop(self.forbidden.take());
        drop(self.augmented.take());
        drop(std::mem::take(&mut self.forbidden_columns));
        drop(std::mem::take(&mut self.trace_nodes));
        self.rows_consumed = 0;
        self.structural_terms = 0;
        self.retained_nonzeros = 0;
        self.reducer_dense_scan_work = 0;
        self.trace_edges = 0;
        self.retained_request_shift_components = 0;
    }

    #[cfg(test)]
    fn forbidden_pivots(&self) -> &[Option<u32>] {
        self.forbidden
            .as_ref()
            .expect("test only inspects live modular lanes")
            .pivots()
    }

    #[cfg(test)]
    fn trace_node_count(&self) -> usize {
        self.trace_nodes.len()
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_rank_step(
    forbidden_before: usize,
    augmented_before: usize,
    forbidden_after: usize,
    augmented_after: usize,
    forbidden_pivot: Option<u32>,
    augmented_pivot: Option<u32>,
    target_column: usize,
) -> Result<(), SpiredModularError> {
    let forbidden_gain = usize::from(forbidden_pivot.is_some());
    let augmented_gain = usize::from(augmented_pivot.is_some());
    if forbidden_after != forbidden_before + forbidden_gain
        || augmented_after != augmented_before + augmented_gain
    {
        return Err(SpiredModularError::Invariant {
            detail: "Symbolica pivot result disagrees with the observed rank change",
        });
    }
    if augmented_before != forbidden_before || augmented_after < forbidden_after {
        return Err(SpiredModularError::Invariant {
            detail: "target-rank delta was invalid before its first gain",
        });
    }
    if augmented_after > forbidden_after + 1 {
        return Err(SpiredModularError::Invariant {
            detail: "one target column changed rank by more than one",
        });
    }
    if forbidden_pivot.is_some() && forbidden_pivot != augmented_pivot {
        return Err(SpiredModularError::Invariant {
            detail: "synchronized reducers chose different forbidden pivots",
        });
    }
    if augmented_after == forbidden_after + 1 {
        if forbidden_pivot.is_some()
            || augmented_pivot.map(|pivot| pivot as usize) != Some(target_column)
        {
            return Err(SpiredModularError::Invariant {
                detail: "target-rank gain was not witnessed by the logical target pivot",
            });
        }
    } else if augmented_pivot.map(|pivot| pivot as usize) == Some(target_column) {
        return Err(SpiredModularError::Invariant {
            detail: "logical target pivot did not produce a target-rank gain",
        });
    }
    Ok(())
}

fn validate_prime(modulus: u64) -> Result<(), SpiredModularError> {
    if modulus.is_multiple_of(2) {
        return Err(SpiredModularError::UnsupportedEvenModulus { modulus });
    }
    if modulus == u64::MAX || !Integer::from(modulus).is_prime(0) {
        return Err(SpiredModularError::NonPrimeModulus { modulus });
    }
    Ok(())
}

fn validate_residue(
    row: usize,
    term: Option<usize>,
    residue: u64,
    modulus: u64,
) -> Result<(), SpiredModularError> {
    if residue >= modulus {
        Err(SpiredModularError::NonCanonicalResidue {
            row,
            term,
            residue,
            modulus,
        })
    } else {
        Ok(())
    }
}

fn try_merge_sorted_columns<Column: Ord + Clone>(
    existing: &[Column],
    new_columns: Vec<Column>,
    expected: usize,
) -> Result<Vec<Column>, SpiredModularError> {
    let mut merged = try_vec(FORBIDDEN_COLUMNS, expected)?;
    let mut existing_ordinal = 0usize;
    let mut new_columns = new_columns.into_iter().peekable();
    loop {
        match (existing.get(existing_ordinal), new_columns.peek()) {
            (Some(old), Some(new)) => match old.cmp(new) {
                Ordering::Less => {
                    merged.push(old.clone());
                    existing_ordinal += 1;
                }
                Ordering::Greater => {
                    merged.push(new_columns.next().ok_or(SpiredModularError::Invariant {
                        detail: "new-column merge lost its pending value",
                    })?);
                }
                Ordering::Equal => {
                    return Err(SpiredModularError::Invariant {
                        detail: "new-column merge received an existing column",
                    });
                }
            },
            (Some(old), None) => {
                merged.push(old.clone());
                existing_ordinal += 1;
            }
            (None, Some(_)) => {
                merged.extend(new_columns);
                break;
            }
            (None, None) => break,
        }
    }
    if merged.len() != expected || merged.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SpiredModularError::Invariant {
            detail: "prospective forbidden-column registry is not canonical",
        });
    }
    Ok(merged)
}

#[cfg(test)]
mod tests;
