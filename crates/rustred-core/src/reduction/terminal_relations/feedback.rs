//! Narrow orchestration hooks for finite, caller-authorized refinement rounds.
//!
//! The native U is exact saved row-space authority, not an independent source
//! certificate. These hooks do not authorize equations, invent master keys, or
//! turn a selected worklist's exhaustion into algebraic saturation.

use super::*;
use crate::sector::Mask;

impl TerminalRelationSession {
    /// Ordered finite ordinary-source inventory retained by the native
    /// checkpoint. Exposing it read-only lets a round-manifest validator check
    /// the exact append-only schedule, not merely its aggregate row count.
    pub fn source_seeds(&self) -> &[IntegralKey] {
        &self.seeds
    }

    /// Exact externally authorized queue and its committed prefix cursor.
    /// `None` means no external queue remains pending; it does not certify that
    /// a caller's optional refinement manifest has completed. Rows keep their
    /// guards, including for a consumed prefix retained during a pending batch.
    /// This is operational state for cold receipt validation, not source
    /// authority and not an independently replayed proof of the native U.
    pub fn externally_queued_equations(&self) -> Option<(&[TerminalEquation], usize)> {
        let assistance = self.assistance.as_ref()?;
        assistance
            .externally_queued
            .then_some((assistance.equations.as_slice(), assistance.equation_cursor))
    }

    /// Auxiliary columns connected to the requested terminal block through
    /// the current exact U's row/column incidence graph.
    ///
    /// This includes reverse connections: an auxiliary pivot can contain a
    /// terminal in its tail. Forward traversal from terminal pivots would miss
    /// precisely those unresolved bridges. No coefficients or endpoint
    /// monomials are copied, and unrelated auxiliary components are excluded.
    /// The caller freezes any chosen finite proposal universe separately.
    pub fn terminal_relevant_auxiliary_keys(
        &self,
    ) -> Result<BTreeSet<IntegralKey>, TerminalRelationError> {
        self.terminal_relevant_auxiliary_keys_with_control(&AtomicBool::new(false))
    }

    pub fn terminal_relevant_auxiliary_keys_with_control(
        &self,
        cancel: &AtomicBool,
    ) -> Result<BTreeSet<IntegralKey>, TerminalRelationError> {
        let u = self.reducer.u();
        let mut components = Components::new(self.columns.len());
        for span in u.row_ptrs().windows(2) {
            cancelled(cancel)?;
            let ids = &u.col_idcs()[span[0]..span[1]];
            if let Some((&first, tail)) = ids.split_first() {
                for &column in tail {
                    components.join(first as usize, column as usize);
                }
            }
        }
        let terminal_components: BTreeSet<_> = self
            .columns
            .iter()
            .enumerate()
            .filter(|(_, key)| self.terminals.contains(*key))
            .map(|(column, _)| components.root(column))
            .collect();
        let mut result = BTreeSet::new();
        for (column, key) in self.columns.iter().enumerate() {
            cancelled(cancel)?;
            if !self.terminals.contains(key)
                && terminal_components.contains(&components.root(column))
            {
                result.insert(key.clone());
            }
        }
        Ok(result)
    }

    /// Select one actual terminal-linked auxiliary per not-yet-attempted
    /// positive support, ordered by `(A + R, R, A, signed_tuple)` where A is
    /// the positive-power sum and R is the negative-power degree. This never
    /// fabricates a sector corner or enumerates supports absent from native U.
    ///
    /// The caller owns the rank-epoch ledger: mark a support attempted only
    /// after its whole frozen batch commits; clear/reopen that ledger only
    /// when the protected terminal rank strictly decreases. New supports can
    /// therefore be tried without immediate terminal-rank gain. Exhausting
    /// this selected policy does not assert global algebraic saturation.
    pub fn selective_auxiliary_source_seeds(
        &self,
        attempted_supports: &BTreeSet<Mask>,
        cancel: &AtomicBool,
    ) -> Result<BTreeMap<Mask, IntegralKey>, TerminalRelationError> {
        if attempted_supports
            .iter()
            .any(|support| support.arity() != self.family.denominator_count())
        {
            return Err(invalid("selective support arity differs from family"));
        }
        let mut selected: BTreeMap<Mask, IntegralKey> = BTreeMap::new();
        for key in self.terminal_relevant_auxiliary_keys_with_control(cancel)? {
            cancelled(cancel)?;
            let support = Mask::try_from_indices(key.powers()).map_err(invalid)?;
            if attempted_supports.contains(&support) {
                continue;
            }
            match selected.entry(support) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(key);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if structural_cost(&key) < structural_cost(entry.get()) {
                        entry.insert(key);
                    }
                }
            }
        }
        Ok(selected)
    }

    /// Generic-field residual support of a caller-authorized equation against
    /// the frozen native U. This is a selection diagnostic ONLY: callers must
    /// enqueue the original witnessed equation, not infer a new unguarded
    /// coefficient identity from this support. Existing Symbolica elimination
    /// owns all arithmetic; the session, its guards, and its queues are unchanged.
    pub fn authorized_equation_residual_support(
        &self,
        equation: &TerminalEquation,
        cancel: &AtomicBool,
    ) -> Result<BTreeSet<IntegralKey>, TerminalRelationError> {
        cancelled(cancel)?;
        self.validate_equations(std::slice::from_ref(equation))?;
        let normalized = self.normalized_row(&equation.terms)?;
        execution::residual_support(self, normalized, cancel)
    }

    /// Whether every row in an externally authorized finite cohort belongs
    /// to the current generic-field native U span after existing normalization.
    /// One compact reachable-U snapshot is shared for the complete cohort.
    ///
    /// This validates retained-row consistency, not independent source replay
    /// or applicability at exceptional parameter values. The caller must still
    /// authenticate source/parent capsules and their complete guard domains.
    /// No normalized residual coefficient is returned or published.
    pub fn authorized_equations_are_satisfied(
        &self,
        equations: &[TerminalEquation],
        cancel: &AtomicBool,
    ) -> Result<bool, TerminalRelationError> {
        cancelled(cancel)?;
        self.validate_equations(equations)?;
        let mut normalized = Vec::with_capacity(equations.len());
        for equation in equations {
            cancelled(cancel)?;
            normalized.push(self.normalized_row(&equation.terms)?);
        }
        execution::rows_are_satisfied(self, normalized, cancel)
    }

    /// Append a finite, externally authorized round to the existing native
    /// assistance queue without replacing its immutable finite provider map.
    ///
    /// The caller owns and durably authenticates the new source/parent-rowspace
    /// capsule before saving this mutation. As with `step_with_provider`, this
    /// API is NOT a source proof checker. It accepts work only at a fully drained
    /// session boundary, preserving the original binding, queried-key ledger,
    /// native U, declared terminal inventory, and ordinary-source cursor.
    /// Completed equations and the pending row cursor already use the native
    /// checkpoint representation. New equation endpoints remain auxiliaries.
    pub fn enqueue_authorized_equations(
        &mut self,
        equations: &[TerminalEquation],
    ) -> Result<usize, TerminalRelationError> {
        if !self.is_complete() {
            return Err(invalid(
                "authorized feedback rows require a drained native session",
            ));
        }
        if self.assistance.is_none() {
            return Err(invalid(
                "authorized feedback rows require an existing assistance binding",
            ));
        }
        self.validate_equations(equations)?;
        let assistance = self.assistance.as_mut().expect("validated assistance");
        // Native writers normally clear a consumed queue immediately. Accept
        // an otherwise valid historical checkpoint retaining its fully consumed
        // prefix as well: completeness proves that no pending row is discarded.
        assistance.equations.clear();
        assistance.equation_cursor = 0;
        assistance.equations.extend_from_slice(equations);
        assistance.externally_queued = !equations.is_empty();
        Ok(equations.len())
    }
}

fn structural_cost(key: &IntegralKey) -> (u128, u128, u128, &IntegralKey) {
    // An in-memory signed tuple contains at most usize::MAX i64 entries, so
    // both sums and their sum fit u128 on the supported <=64-bit platforms.
    let mut positive = 0u128;
    let mut negative = 0u128;
    for &power in key.powers() {
        if power > 0 {
            positive += power as u128;
        } else {
            negative += power.unsigned_abs() as u128;
        }
    }
    (positive + negative, negative, positive, key)
}

fn cancelled(cancel: &AtomicBool) -> Result<(), TerminalRelationError> {
    if cancel.load(Ordering::Relaxed) {
        Err(invalid("selective refinement inspection cancelled"))
    } else {
        Ok(())
    }
}

/// Ordinary incidence bookkeeping, not polynomial or row-reduction algebra.
struct Components {
    parents: Vec<usize>,
    ranks: Vec<u8>,
}

impl Components {
    fn new(size: usize) -> Self {
        Self {
            parents: (0..size).collect(),
            ranks: vec![0; size],
        }
    }

    fn root(&mut self, mut column: usize) -> usize {
        while self.parents[column] != column {
            self.parents[column] = self.parents[self.parents[column]];
            column = self.parents[column];
        }
        column
    }

    fn join(&mut self, left: usize, right: usize) {
        let mut left = self.root(left);
        let mut right = self.root(right);
        if left == right {
            return;
        }
        if self.ranks[left] < self.ranks[right] {
            std::mem::swap(&mut left, &mut right);
        }
        self.parents[right] = left;
        if self.ranks[left] == self.ranks[right] {
            self.ranks[left] += 1;
        }
    }
}

#[cfg(test)]
mod tests;
