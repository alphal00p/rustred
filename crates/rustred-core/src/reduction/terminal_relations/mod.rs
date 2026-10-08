//! Resumable finite ordinary-IBP search among an explicit terminal inventory.
//!
//! Symbolica owns exact specialization, rational arithmetic and sparse
//! elimination. Unresolved auxiliary integrals remain columns, never zeroes or
//! additional declared masters. Exhausting this finite worklist is not a proof
//! of minimality, unrestricted closure, or absence of further relations.

mod assistance;
pub mod collection;
mod codec;
mod elimination;
mod sources;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use super::terminal_normalization::{TerminalNormalizationLimits, TerminalNormalizationPlan};
use crate::algebra::{Coefficient, IndexedAlgebraLimits};
use crate::family::{IntegralFamily, IntegralKey};
use crate::persistence::BinaryIoLimits;
use crate::sector::OrderingPolicy;

pub type TerminalRelationRow = BTreeMap<IntegralKey, Coefficient>;

/// An exact homogeneous equation, together with the generic-parameter domain
/// on which its provider authorizes it. A declared terminal is not an equation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalEquation {
    pub terms: TerminalRelationRow,
    pub nonzero_conditions: Vec<Coefficient>,
}
type NativeReducer = SparseRowReducer<RationalPolynomialField<IntegerRing, u16>>;

#[derive(Clone, Copy, Debug)]
pub struct TerminalRelationLimits {
    pub max_seeds: usize,
    pub max_columns: usize,
    pub max_rows: usize,
    pub max_nonzeros: usize,
    pub normalization: TerminalNormalizationLimits,
    pub algebra: IndexedAlgebraLimits,
}
impl Default for TerminalRelationLimits {
    fn default() -> Self {
        Self {
            max_seeds: 1_000_000,
            max_columns: 1_000_000,
            max_rows: 1_000_000,
            max_nonzeros: 16_000_000,
            normalization: Default::default(),
            algebra: Default::default(),
        }
    }
}

/// Cheap, native progress. Counts describe finite search, not independence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalRelationStats {
    pub raw_terminals: usize,
    pub normalized_terminals: usize,
    pub remaining_terminals: usize,
    pub terminal_relations: usize,
    pub seeds: usize,
    pub completed_seeds: usize,
    pub source_rows_per_seed: usize,
    pub completed_source_rows: u64,
    pub total_source_rows: u64,
    pub independent_rows: usize,
    pub columns: usize,
    pub auxiliary_columns: usize,
    pub nonzeros: usize,
    pub pending_rebuild_rows: usize,
    pub completed_rebuild_rows: u64,
    pub completed_assistance_keys: usize,
    pub pending_assistance_keys: usize,
    pub completed_assistance_rows: u64,
    pub pending_assistance_rows: usize,
    pub seed_depth: u32,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRelationError {
    InvalidInput(String),
    Algebra(String),
    Binary(String),
    Limit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}
impl fmt::Display for TerminalRelationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(s) => write!(f, "terminal relations input: {s}"),
            Self::Algebra(s) => write!(f, "terminal relations algebra: {s}"),
            Self::Binary(s) => write!(f, "terminal relations binary: {s}"),
            Self::Limit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "terminal relations {resource}: {requested} exceeds {limit}"
            ),
        }
    }
}
impl std::error::Error for TerminalRelationError {}
fn check(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TerminalRelationError> {
    if requested > limit {
        Err(TerminalRelationError::Limit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
fn algebra(error: impl fmt::Display) -> TerminalRelationError {
    TerminalRelationError::Algebra(error.to_string())
}
fn invalid(error: impl fmt::Display) -> TerminalRelationError {
    TerminalRelationError::InvalidInput(error.to_string())
}

/// Safe checkpoints exist immediately before/after each [`Self::step`]. A
/// Symbolica row operation is indivisible and is never serialized half-done.
pub struct TerminalRelationSession {
    family: Arc<IntegralFamily>,
    raw: BTreeSet<IntegralKey>,
    normalization: TerminalNormalizationPlan,
    sources: sources::Sources,
    seeds: Vec<IntegralKey>,
    seed_cursor: usize,
    source_cursor: usize,
    seed_depth: u32,
    columns: Vec<IntegralKey>,
    // Distance from the end stays invariant when auxiliary columns prepend.
    column_offsets: BTreeMap<IntegralKey, u32>,
    terminals: BTreeSet<IntegralKey>,
    alias_keys: BTreeSet<IntegralKey>,
    aliases: BTreeMap<IntegralKey, IntegralKey>,
    column_normalized: bool,
    terminal_relations: usize,
    reducer: NativeReducer,
    conditions: Vec<Coefficient>,
    rebuild: Vec<TerminalRelationRow>,
    rebuild_cursor: usize,
    completed_rebuild_rows: u64,
    assistance: Option<assistance::Assistance>,
    limits: TerminalRelationLimits,
}

impl TerminalRelationSession {
    pub fn new(
        family: Arc<IntegralFamily>,
        raw: BTreeSet<IntegralKey>,
        seed_depth: u32,
        limits: TerminalRelationLimits,
    ) -> Result<Self, TerminalRelationError> {
        Self::validate_keys(&family, &raw)?;
        let normalization = TerminalNormalizationPlan::vacuum_quadratic_numerators(
            &family,
            &raw,
            OrderingPolicy::SpiredUncutV1,
            limits.normalization,
        )
        .map_err(algebra)?;
        let sources = sources::Sources::new(&family)?;
        let seeds = sources::seeds(
            normalization.canonical_terminals(),
            seed_depth,
            limits.max_seeds,
        )?;
        let mut columns: Vec<_> = normalization
            .canonical_terminals()
            .iter()
            .cloned()
            .collect();
        elimination::sort_keys(&mut columns);
        check("columns", columns.len(), limits.max_columns)?;
        let width = u32::try_from(columns.len()).map_err(invalid)?;
        let terminals = normalization.canonical_terminals().clone();
        let empty = raw.is_empty();
        Ok(Self {
            family,
            raw,
            normalization,
            sources,
            seeds,
            seed_cursor: 0,
            source_cursor: 0,
            seed_depth,
            terminals,
            alias_keys: BTreeSet::new(),
            aliases: BTreeMap::new(),
            column_normalized: empty,
            terminal_relations: 0,
            column_offsets: elimination::column_offsets(&columns),
            columns,
            reducer: SparseRowReducer::new(width, RationalPolynomialField::new(Z), LuLMode::None),
            conditions: Vec::new(),
            rebuild: Vec::new(),
            rebuild_cursor: 0,
            completed_rebuild_rows: 0,
            assistance: None,
            limits,
        })
    }

    fn validate_keys(
        family: &IntegralFamily,
        keys: &BTreeSet<IntegralKey>,
    ) -> Result<(), TerminalRelationError> {
        if keys
            .iter()
            .any(|k| k.powers().len() != family.denominator_count())
        {
            return Err(invalid("terminal arity differs from family"));
        }
        Ok(())
    }

    pub fn family_owner(&self) -> &Arc<IntegralFamily> {
        &self.family
    }
    pub fn raw_terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.raw
    }
    pub fn normalization(&self) -> &TerminalNormalizationPlan {
        &self.normalization
    }
    /// Inherited source and specialization conditions. Elimination takes place
    /// over the exact rational-function field: this list is not a certificate
    /// of pivot regularity at every exceptional numerical dimension. Exported
    /// rule coefficients retain their symbolic denominators.
    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        &self.conditions
    }
    pub fn is_complete(&self) -> bool {
        self.column_normalized
            && self.rebuild_cursor == self.rebuild.len()
            && self.seed_cursor == self.seeds.len()
            && self.assistance.as_ref().is_none_or(|a| a.is_complete())
    }
    pub fn statistics(&self) -> TerminalRelationStats {
        TerminalRelationStats {
            raw_terminals: self.raw.len(),
            normalized_terminals: self.terminals.len(),
            remaining_terminals: self.terminals.len() - self.terminal_relations,
            terminal_relations: self.terminal_relations,
            seeds: self.seeds.len(),
            completed_seeds: self.seed_cursor,
            source_rows_per_seed: self.sources.len(),
            completed_source_rows: self.seed_cursor as u64 * self.sources.len() as u64
                + self.source_cursor as u64,
            total_source_rows: self.seeds.len() as u64 * self.sources.len() as u64,
            independent_rows: self.reducer.u().nrows() as usize,
            columns: self.columns.len(),
            auxiliary_columns: self.columns.len() - self.terminals.len(),
            nonzeros: self.reducer.u().nvalues(),
            pending_rebuild_rows: self.rebuild.len() - self.rebuild_cursor,
            completed_rebuild_rows: self.completed_rebuild_rows,
            completed_assistance_keys: self.assistance.as_ref().map_or(0, |a| a.queried.len()),
            pending_assistance_keys: self.assistance.as_ref().map_or(0, |a| a.pending.len()),
            completed_assistance_rows: self.assistance.as_ref().map_or(0, |a| a.completed_rows),
            pending_assistance_rows: self
                .assistance
                .as_ref()
                .map_or(0, |a| a.equations.len() - a.equation_cursor),
            seed_depth: self.seed_depth,
            complete: self.is_complete(),
        }
    }

    /// Execute one complete native source/elimination row. Cancellation checked
    /// before work leaves the session unchanged and safe to save.
    pub fn step(
        &mut self,
        cancel: &AtomicBool,
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        if self.assistance.is_some() {
            return Err(invalid("assisted sessions require step_with_provider"));
        }
        self.step_ordinary(cancel)
    }

    /// Replay at most one already-proved basis row after an inventory extension.
    /// This never consumes ordinary sources, queries an equation provider, or
    /// marks pending assistance as queried. It is safe for publication-only
    /// reuse of assisted sessions without preparing their original providers.
    pub fn step_rebuild_only(
        &mut self,
        cancel: &AtomicBool,
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        if self.rebuild_cursor == self.rebuild.len() {
            return Ok(self.statistics());
        }
        self.step_ordinary(cancel)
    }

    fn step_ordinary(
        &mut self,
        cancel: &AtomicBool,
    ) -> Result<TerminalRelationStats, TerminalRelationError> {
        if cancel.load(Ordering::Relaxed) || self.is_complete() {
            return Ok(self.statistics());
        }
        check(
            "stored nonzeros",
            self.reducer.u().nvalues(),
            self.limits.max_nonzeros,
        )?;
        if self.rebuild_cursor < self.rebuild.len() {
            let row = self.normalized_row(&self.rebuild[self.rebuild_cursor])?;
            self.add_row(row)?;
            self.rebuild_cursor += 1;
            self.completed_rebuild_rows += 1;
            if self.rebuild_cursor == self.rebuild.len() {
                self.rebuild.clear();
                self.rebuild_cursor = 0;
            }
        } else if self.seed_cursor == self.seeds.len() {
            self.normalize_generated()?;
        } else {
            let (row, conditions) = self.sources.row(
                &self.seeds[self.seed_cursor],
                self.source_cursor,
                self.limits.algebra,
            )?;
            let normalized = self.normalized_row(&row)?;
            // Only ordinary-source support can request more saved equations.
            // Added saved-equation children never grow this finite worklist.
            let support = self.assistance_support(row.keys().chain(normalized.keys()))?;
            self.add_row(normalized)?;
            if let Some(assistance) = &mut self.assistance {
                assistance.pending.extend(support);
            }
            for guard in conditions {
                if !self.conditions.contains(&guard) {
                    self.conditions.push(guard);
                }
            }
            self.source_cursor += 1;
            if self.source_cursor == self.sources.len() {
                self.source_cursor = 0;
                self.seed_cursor += 1;
            }
        }
        Ok(self.statistics())
    }

    /// Add a higher-scope terminal inventory or a deeper finite seed shell.
    /// Completed sources remain completed. If representatives change, the old
    /// independent row space is reindexed by resumable native row operations.
    pub fn extend(
        &mut self,
        additional: &BTreeSet<IntegralKey>,
        seed_depth: u32,
    ) -> Result<(), TerminalRelationError> {
        if self.rebuild_cursor < self.rebuild.len() {
            return Err(invalid(
                "finish pending row-space rebuild before extending again",
            ));
        }
        if seed_depth < self.seed_depth {
            return Err(invalid("seed depth cannot decrease"));
        }
        Self::validate_keys(&self.family, additional)?;
        let raw: BTreeSet<_> = self.raw.union(additional).cloned().collect();
        let normalization = TerminalNormalizationPlan::vacuum_quadratic_numerators(
            &self.family,
            &raw,
            OrderingPolicy::SpiredUncutV1,
            self.limits.normalization,
        )
        .map_err(algebra)?;
        check(
            "columns",
            normalization.canonical_terminals().len(),
            self.limits.max_columns,
        )?;
        u32::try_from(normalization.canonical_terminals().len()).map_err(invalid)?;
        let proposed = sources::seeds(
            normalization.canonical_terminals(),
            seed_depth,
            self.limits.max_seeds,
        )?;
        let existing: BTreeSet<_> = self.seeds.iter().cloned().collect();
        let new_seeds: Vec<_> = proposed
            .into_iter()
            .filter(|k| !existing.contains(k))
            .collect();
        check(
            "seeds",
            self.seeds.len().saturating_add(new_seeds.len()),
            self.limits.max_seeds,
        )?;
        let changed = raw != self.raw;
        let support = self.assistance_support(
            raw.iter()
                .chain(normalization.canonical_terminals())
                .chain(new_seeds.iter()),
        )?;
        if changed {
            self.rebuild = self.basis_rows();
            self.rebuild_cursor = 0;
            self.columns = normalization
                .canonical_terminals()
                .iter()
                .cloned()
                .collect();
            elimination::sort_keys(&mut self.columns);
            self.column_offsets = elimination::column_offsets(&self.columns);
            self.reducer = SparseRowReducer::new(
                u32::try_from(self.columns.len()).map_err(invalid)?,
                RationalPolynomialField::new(Z),
                LuLMode::None,
            );
            self.terminal_relations = 0;
        }
        if changed {
            self.terminals = normalization.canonical_terminals().clone();
        }
        self.raw = raw;
        self.normalization = normalization;
        self.seeds.extend(new_seeds);
        self.seed_depth = seed_depth;
        if changed {
            self.aliases.clear();
            self.alias_keys.clear();
        }
        self.column_normalized = false;
        if let Some(assistance) = &mut self.assistance {
            assistance.pending.extend(support);
        }
        Ok(())
    }

    /// Append finite ordinary-IBP source points without promoting any integral
    /// into the requested terminal block or changing the signed-L1 seed depth.
    /// Existing source/rebuild cursors and proved rows remain valid. Appending
    /// during a rebuild is safe: it changes no column roles, and ordinary work
    /// still drains the existing rebuild before processing the new sources.
    ///
    /// The complete ordered seed inventory already belongs to the native
    /// checkpoint. Repeated additions are no-ops, including after resume.
    pub fn extend_source_seeds(
        &mut self,
        additional: &BTreeSet<IntegralKey>,
    ) -> Result<usize, TerminalRelationError> {
        Self::validate_keys(&self.family, additional)?;
        let existing: BTreeSet<_> = self.seeds.iter().collect();
        let new: Vec<_> = additional
            .iter()
            .filter(|key| !existing.contains(key))
            .cloned()
            .collect();
        check(
            "seeds",
            self.seeds.len().saturating_add(new.len()),
            self.limits.max_seeds,
        )?;
        let support = self.assistance_support(new.iter())?;
        let added = new.len();
        if added == 0 {
            return Ok(0);
        }
        self.seeds.extend(new);
        self.column_normalized = false;
        if let Some(assistance) = &mut self.assistance {
            assistance.pending.extend(support);
        }
        Ok(added)
    }

    /// Add containing-sector ordinary sources around the raw terminal keys.
    /// Each selected nonpositive index is set directly to +1; every other
    /// index is retained. All combinations of one through `max_promoted_axes`
    /// axes are considered, with deterministic deduplication and seed limits.
    /// Zero selects no additional sources; values above the family arity fail.
    ///
    /// These are same-family ordinary IBPs, including when an inactive slot
    /// represents a numerator. No physical-propagator, mass, or factorization
    /// assumption is used. This is distinct from a signed-L1 neighborhood:
    /// promoting a negative index can jump over several integer powers.
    pub fn extend_containing_sector_seeds(
        &mut self,
        max_promoted_axes: usize,
    ) -> Result<usize, TerminalRelationError> {
        if max_promoted_axes > self.family.denominator_count() {
            return Err(invalid("containing-sector promotions exceed family arity"));
        }
        let additional =
            sources::containing_sector_seeds(&self.raw, max_promoted_axes, self.limits.max_seeds)?;
        self.extend_source_seeds(&additional)
    }

    pub fn to_native_bytes(
        &self,
        limits: BinaryIoLimits,
    ) -> Result<Vec<u8>, TerminalRelationError> {
        codec::encode(self, limits)
    }
    pub fn from_native_bytes(
        bytes: &[u8],
        limits: TerminalRelationLimits,
        io: BinaryIoLimits,
    ) -> Result<Self, TerminalRelationError> {
        codec::decode(bytes, limits, io)
    }
}
