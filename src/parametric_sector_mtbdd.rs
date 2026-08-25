//! Baseline reduced decision DAG for normalized sector-coverage formulas.
//!
//! This module is deliberately ordinal-only.  It cannot construct, multiply,
//! factor, or specialize a Symbolica polynomial.  The caller first authenticates
//! one immutable base table of structural loci for every candidate; the V4
//! backend may later build a private derived product-locus extension, while the
//! baseline MTBDD always compiles the original factor-zero disjunction.

use crate::coverage_decision_dag::{
    CoverageDecisionAtomId, CoverageDecisionBooleanTerminals, CoverageDecisionDag,
    CoverageDecisionDagError, CoverageDecisionDagLimits, CoverageDecisionDagRetainedStats,
    CoverageDecisionDagRootedView, CoverageDecisionDagWorkStats, CoverageDecisionPersistedRef,
    CoverageDecisionRef, CoverageDecisionTerminalPayload, CoverageDecisionTerminalPayloadCensus,
};
use crate::direct_bad_formula::DirectBadFormulaClause;
use crate::parametric_sector_formula_ir::{
    NormalizedBadClauseRole, NormalizedBadFormulaBody, NormalizedBadLiteral,
    NormalizedBadLiteralPolarity, NormalizedCoverageAttempt, NormalizedCoverageIr,
    ParametricSectorFormulaIrError,
};
use std::fmt;
use std::sync::Arc;

pub(crate) const PARAMETRIC_SECTOR_MTBDD_ATOM_ORDER_V1: &str =
    "rustred-coverage-polynomial-zero-by-base-structural-locus-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ParametricSectorMtbddAtom {
    structural_locus_ordinal: usize,
}

impl ParametricSectorMtbddAtom {
    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParametricSectorMtbddDisposition {
    DescendingRule { candidate_ordinal: usize },
    Uncovered,
    Unsupported { candidate_ordinals: Box<[usize]> },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParametricSectorMtbddTerminalPayload {
    BooleanFalse,
    BooleanTrue,
    Disposition(ParametricSectorMtbddDisposition),
}

impl CoverageDecisionTerminalPayload for ParametricSectorMtbddTerminalPayload {
    fn coverage_decision_retained_census(&self) -> CoverageDecisionTerminalPayloadCensus {
        let references = match self {
            Self::Disposition(ParametricSectorMtbddDisposition::DescendingRule { .. }) => 1,
            Self::Disposition(ParametricSectorMtbddDisposition::Unsupported {
                candidate_ordinals,
            }) => candidate_ordinals.len(),
            _ => 0,
        };
        let units = references
            .checked_add(1)
            .expect("validated terminal census fits usize");
        CoverageDecisionTerminalPayloadCensus::new(units, references)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorMtbddLimits {
    pub(crate) dag: CoverageDecisionDagLimits,
    pub(crate) max_base_structural_loci: usize,
    pub(crate) max_attempts: usize,
    pub(crate) max_normalized_clauses: usize,
    pub(crate) max_normalized_literals: usize,
    pub(crate) max_clause_source_references: usize,
    pub(crate) max_factor_lists: usize,
    pub(crate) max_factor_references: usize,
    pub(crate) max_atom_staging_entries: usize,
    pub(crate) max_atom_sort_scratch_entries: usize,
    pub(crate) max_atom_sort_comparisons: usize,
    pub(crate) max_atom_dedup_scans: usize,
    pub(crate) max_atoms: usize,
    pub(crate) max_atom_lookup_comparisons: usize,
    pub(crate) max_formula_compile_steps: usize,
    pub(crate) max_priority_candidate_pairs: usize,
    pub(crate) max_unsupported_references: usize,
    pub(crate) max_typed_root_node_mark_entries: usize,
    pub(crate) max_typed_root_terminal_mark_entries: usize,
    pub(crate) max_typed_root_stack_entries: usize,
    pub(crate) max_typed_root_stack_pushes: usize,
    pub(crate) max_typed_root_visits: usize,
    pub(crate) max_classification_visits: usize,
}

impl Default for ParametricSectorMtbddLimits {
    fn default() -> Self {
        Self {
            dag: CoverageDecisionDagLimits::default(),
            max_base_structural_loci: 16_000_000,
            max_attempts: 1_000_000,
            max_normalized_clauses: 16_000_000,
            max_normalized_literals: 32_000_000,
            max_clause_source_references: 32_000_000,
            max_factor_lists: 1_000_000,
            max_factor_references: 32_000_000,
            max_atom_staging_entries: 32_000_000,
            max_atom_sort_scratch_entries: 32_000_000,
            max_atom_sort_comparisons: 1_000_000_000,
            max_atom_dedup_scans: 32_000_000,
            max_atoms: 16_000_000,
            max_atom_lookup_comparisons: 256_000_000,
            max_formula_compile_steps: 256_000_000,
            max_priority_candidate_pairs: 16_000_000,
            max_unsupported_references: 16_000_000,
            max_typed_root_node_mark_entries: 16_000_000,
            max_typed_root_terminal_mark_entries: 1_000_000,
            max_typed_root_stack_entries: 16_000_001,
            max_typed_root_stack_pushes: 32_000_001,
            max_typed_root_visits: 16_000_000,
            max_classification_visits: 16_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricSectorMtbddStats {
    pub(crate) base_structural_loci: usize,
    pub(crate) attempts: usize,
    pub(crate) certified_attempts: usize,
    pub(crate) unsupported_attempts: usize,
    pub(crate) normalized_clauses: usize,
    pub(crate) normalized_literals: usize,
    pub(crate) clause_source_references: usize,
    pub(crate) factor_lists: usize,
    pub(crate) factor_references: usize,
    pub(crate) atom_staging_entries: usize,
    pub(crate) atom_sort_scratch_entries: usize,
    pub(crate) atom_sort_comparisons: usize,
    pub(crate) atom_dedup_scans: usize,
    pub(crate) atoms: usize,
    pub(crate) atom_lookup_comparisons: usize,
    pub(crate) formula_compile_steps: usize,
    pub(crate) priority_candidate_pairs: usize,
    pub(crate) unsupported_references: usize,
    pub(crate) typed_root_node_mark_entries: usize,
    pub(crate) typed_root_terminal_mark_entries: usize,
    pub(crate) typed_root_stack_entries: usize,
    pub(crate) typed_root_stack_pushes: usize,
    pub(crate) typed_root_visits: usize,
    pub(crate) arena_retained_before_export: CoverageDecisionDagRetainedStats,
    pub(crate) rooted_retained: CoverageDecisionDagRetainedStats,
    pub(crate) dag_work: CoverageDecisionDagWorkStats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorMtbddError {
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    AssignmentArityMismatch {
        expected: usize,
        actual: usize,
    },
    BooleanTerminalReachable,
    DispositionCandidateOutOfRange {
        candidate_ordinal: usize,
        attempt_count: usize,
    },
    DispositionCandidateIsNotCertified {
        candidate_ordinal: usize,
    },
    UnsupportedFallbackMismatch,
    MalformedRootedView,
    FormulaIr(ParametricSectorFormulaIrError),
    Core(CoverageDecisionDagError),
}

impl fmt::Display for ParametricSectorMtbddError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "parametric sector MTBDD error: {self:?}")
    }
}

impl std::error::Error for ParametricSectorMtbddError {}

impl From<CoverageDecisionDagError> for ParametricSectorMtbddError {
    fn from(value: CoverageDecisionDagError) -> Self {
        Self::Core(value)
    }
}

impl From<ParametricSectorFormulaIrError> for ParametricSectorMtbddError {
    fn from(value: ParametricSectorFormulaIrError) -> Self {
        Self::FormulaIr(value)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorMtbddDecisionFunction {
    formula_schema: &'static str,
    order_schema: &'static str,
    base_structural_locus_count: usize,
    atoms: Box<[ParametricSectorMtbddAtom]>,
    rooted: CoverageDecisionDagRootedView<ParametricSectorMtbddTerminalPayload>,
    limits: ParametricSectorMtbddLimits,
    stats: ParametricSectorMtbddStats,
}

impl ParametricSectorMtbddDecisionFunction {
    pub(crate) const fn formula_schema(&self) -> &'static str {
        self.formula_schema
    }

    pub(crate) const fn order_schema(&self) -> &'static str {
        self.order_schema
    }

    pub(crate) fn atoms(&self) -> &[ParametricSectorMtbddAtom] {
        &self.atoms
    }

    pub(crate) const fn base_structural_locus_count(&self) -> usize {
        self.base_structural_locus_count
    }

    pub(crate) fn rooted(
        &self,
    ) -> &CoverageDecisionDagRootedView<ParametricSectorMtbddTerminalPayload> {
        &self.rooted
    }

    pub(crate) const fn limits(&self) -> ParametricSectorMtbddLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ParametricSectorMtbddStats {
        self.stats
    }

    pub(crate) fn classify_assignment(
        &self,
        zero_by_base_structural_locus: &[bool],
    ) -> Result<&ParametricSectorMtbddDisposition, ParametricSectorMtbddError> {
        if zero_by_base_structural_locus.len() != self.base_structural_locus_count {
            return Err(ParametricSectorMtbddError::AssignmentArityMismatch {
                expected: self.base_structural_locus_count,
                actual: zero_by_base_structural_locus.len(),
            });
        }
        let mut reference = *self
            .rooted
            .roots()
            .first()
            .ok_or(ParametricSectorMtbddError::MalformedRootedView)?;
        let mut visits = 0usize;
        loop {
            visits = bounded_add(
                "MTBDD classification visits",
                visits,
                1,
                self.limits.max_classification_visits,
            )?;
            match reference {
                CoverageDecisionPersistedRef::Terminal(id) => {
                    let payload = self
                        .rooted
                        .terminal_payloads()
                        .get(id.ordinal())
                        .ok_or(ParametricSectorMtbddError::MalformedRootedView)?;
                    return match payload.as_ref() {
                        ParametricSectorMtbddTerminalPayload::Disposition(disposition) => {
                            Ok(disposition)
                        }
                        _ => Err(ParametricSectorMtbddError::BooleanTerminalReachable),
                    };
                }
                CoverageDecisionPersistedRef::Node(id) => {
                    let node = self
                        .rooted
                        .nodes()
                        .get(id.ordinal())
                        .copied()
                        .ok_or(ParametricSectorMtbddError::MalformedRootedView)?;
                    let atom = self
                        .atoms
                        .get(node.atom().ordinal())
                        .ok_or(ParametricSectorMtbddError::MalformedRootedView)?;
                    reference = if zero_by_base_structural_locus[atom.structural_locus_ordinal] {
                        node.when_true()
                    } else {
                        node.when_false()
                    };
                }
            }
        }
    }
}

pub(crate) struct ParametricSectorMtbddCompiler;

impl ParametricSectorMtbddCompiler {
    pub(crate) fn compile(
        ir: &NormalizedCoverageIr,
        limits: ParametricSectorMtbddLimits,
    ) -> Result<ParametricSectorMtbddDecisionFunction, ParametricSectorMtbddError> {
        let mut stats = census_ir(ir, limits)?;
        let atoms = build_dense_atoms(ir, stats.normalized_literals, limits, &mut stats)?;
        let mut dag = CoverageDecisionDag::new(atoms.len(), limits.dag)?;
        let mut atom_lookup_comparisons = 0usize;
        let mut formula_compile_steps = 0usize;
        let mut priority_candidate_pairs = 0usize;
        let mut unsupported_references = 0usize;
        let rooted = dag.with_operation(|operation| {
            let boolean_false = operation
                .intern_terminal(Arc::new(ParametricSectorMtbddTerminalPayload::BooleanFalse))?;
            let boolean_true = operation
                .intern_terminal(Arc::new(ParametricSectorMtbddTerminalPayload::BooleanTrue))?;
            let boolean = operation.boolean_terminals(boolean_false, boolean_true)?;
            let mut candidates = Vec::<(CoverageDecisionRef, CoverageDecisionRef)>::new();
            let mut cutoff = false;
            for attempt in ir.attempts() {
                let NormalizedCoverageAttempt::Certified(formula) = attempt else {
                    continue;
                };
                let bad = compile_formula(
                    operation,
                    formula.body(),
                    &atoms,
                    boolean,
                    &mut atom_lookup_comparisons,
                    &mut formula_compile_steps,
                    limits,
                )?;
                if bad == boolean.when_true() {
                    continue;
                }
                charge_core_counter(
                    "MTBDD priority candidate pairs",
                    &mut priority_candidate_pairs,
                    limits.max_priority_candidate_pairs,
                )?;
                try_reserve_core(
                    "compiled MTBDD priority candidate pairs",
                    &mut candidates,
                    1,
                )?;
                let applies = operation.intern_terminal(Arc::new(
                    ParametricSectorMtbddTerminalPayload::Disposition(
                        ParametricSectorMtbddDisposition::DescendingRule {
                            candidate_ordinal: formula.source_attempt_ordinal(),
                        },
                    ),
                ))?;
                candidates.push((bad, applies));
                if bad == boolean.when_false() {
                    cutoff = true;
                    break;
                }
            }
            let fallback = if cutoff {
                boolean.when_false()
            } else {
                if stats.unsupported_attempts > limits.max_unsupported_references {
                    return Err(CoverageDecisionDagError::ResourceLimit {
                        resource: "MTBDD unsupported fallback references",
                        requested: stats.unsupported_attempts,
                        limit: limits.max_unsupported_references,
                    });
                }
                let mut unsupported = Vec::new();
                try_reserve_core_exact(
                    "MTBDD unsupported fallback ordinals",
                    &mut unsupported,
                    stats.unsupported_attempts,
                )?;
                for attempt in ir.attempts() {
                    if let NormalizedCoverageAttempt::Unsupported {
                        source_attempt_ordinal,
                    } = attempt
                    {
                        unsupported.push(*source_attempt_ordinal);
                    }
                }
                unsupported_references = unsupported.len();
                let fallback_disposition = if unsupported.is_empty() {
                    ParametricSectorMtbddDisposition::Uncovered
                } else {
                    ParametricSectorMtbddDisposition::Unsupported {
                        candidate_ordinals: unsupported.into_boxed_slice(),
                    }
                };
                operation.intern_terminal(Arc::new(
                    ParametricSectorMtbddTerminalPayload::Disposition(fallback_disposition),
                ))?
            };
            let root = operation.compose_candidate_priority(&candidates, boolean, fallback)?;
            operation.export_rooted(&[root], boolean)
        })?;
        stats.atom_lookup_comparisons = atom_lookup_comparisons;
        stats.formula_compile_steps = formula_compile_steps;
        stats.priority_candidate_pairs = priority_candidate_pairs;
        stats.unsupported_references = unsupported_references;
        stats.arena_retained_before_export = dag.retained_stats();
        stats.rooted_retained = rooted.retained_stats();
        stats.dag_work = dag.stats().work;
        let typed_root = validate_typed_final_root(&rooted, ir, atoms.len(), limits)?;
        stats.typed_root_node_mark_entries = typed_root.node_mark_entries;
        stats.typed_root_terminal_mark_entries = typed_root.terminal_mark_entries;
        stats.typed_root_stack_entries = typed_root.stack_entries;
        stats.typed_root_stack_pushes = typed_root.stack_pushes;
        stats.typed_root_visits = typed_root.visits;
        Ok(ParametricSectorMtbddDecisionFunction {
            formula_schema: ir.schema(),
            order_schema: PARAMETRIC_SECTOR_MTBDD_ATOM_ORDER_V1,
            base_structural_locus_count: ir.base_structural_locus_count(),
            atoms: atoms.into_boxed_slice(),
            rooted,
            limits,
            stats,
        })
    }
}

fn census_ir(
    ir: &NormalizedCoverageIr,
    limits: ParametricSectorMtbddLimits,
) -> Result<ParametricSectorMtbddStats, ParametricSectorMtbddError> {
    check_limit(
        "base structural loci",
        ir.base_structural_locus_count(),
        limits.max_base_structural_loci,
    )?;
    check_limit(
        "normalized coverage attempts",
        ir.attempts().len(),
        limits.max_attempts,
    )?;
    let mut stats = ParametricSectorMtbddStats {
        base_structural_loci: ir.base_structural_locus_count(),
        attempts: ir.attempts().len(),
        ..ParametricSectorMtbddStats::default()
    };
    for attempt in ir.attempts() {
        match attempt {
            NormalizedCoverageAttempt::Unsupported { .. } => {
                stats.unsupported_attempts = checked_add(
                    "normalized unsupported attempts",
                    stats.unsupported_attempts,
                    1,
                )?;
            }
            NormalizedCoverageAttempt::Certified(formula) => {
                stats.certified_attempts =
                    checked_add("normalized certified attempts", stats.certified_attempts, 1)?;
                match formula.body() {
                    NormalizedBadFormulaBody::False => {}
                    NormalizedBadFormulaBody::True { sources } => {
                        stats.clause_source_references = bounded_add(
                            "normalized clause source references",
                            stats.clause_source_references,
                            sources.len(),
                            limits.max_clause_source_references,
                        )?;
                    }
                    NormalizedBadFormulaBody::Dnf {
                        clauses,
                        atomic_equal_zero_factors,
                    } => {
                        stats.normalized_clauses = bounded_add(
                            "normalized bad clauses",
                            stats.normalized_clauses,
                            clauses.len(),
                            limits.max_normalized_clauses,
                        )?;
                        for clause in clauses.iter() {
                            stats.normalized_literals = bounded_add(
                                "normalized bad literals",
                                stats.normalized_literals,
                                clause.body().atom_count(),
                                limits.max_normalized_literals,
                            )?;
                            stats.clause_source_references = bounded_add(
                                "normalized clause source references",
                                stats.clause_source_references,
                                clause.sources().len(),
                                limits.max_clause_source_references,
                            )?;
                        }
                        if !atomic_equal_zero_factors.is_empty() {
                            stats.factor_lists = bounded_add(
                                "normalized factor lists",
                                stats.factor_lists,
                                1,
                                limits.max_factor_lists,
                            )?;
                        }
                        stats.factor_references = bounded_add(
                            "normalized factor references",
                            stats.factor_references,
                            atomic_equal_zero_factors.len(),
                            limits.max_factor_references,
                        )?;
                    }
                }
            }
        }
    }
    Ok(stats)
}

fn build_dense_atoms(
    ir: &NormalizedCoverageIr,
    literal_count: usize,
    limits: ParametricSectorMtbddLimits,
    stats: &mut ParametricSectorMtbddStats,
) -> Result<Vec<ParametricSectorMtbddAtom>, ParametricSectorMtbddError> {
    check_limit(
        "normalized MTBDD atom staging entries",
        literal_count,
        limits.max_atom_staging_entries,
    )?;
    let mut atoms = Vec::new();
    try_reserve_exact("normalized MTBDD atom staging", &mut atoms, literal_count)?;
    for attempt in ir.attempts() {
        let NormalizedCoverageAttempt::Certified(formula) = attempt else {
            continue;
        };
        if let NormalizedBadFormulaBody::Dnf { clauses, .. } = formula.body() {
            for clause in clauses.iter() {
                match clause.body() {
                    DirectBadFormulaClause::Atom(literal) => {
                        stage_dense_atom(
                            &mut atoms,
                            literal.structural_locus_ordinal(),
                            limits,
                            stats,
                        )?;
                    }
                    DirectBadFormulaClause::Conjunction(left, right) => {
                        stage_dense_atom(
                            &mut atoms,
                            left.structural_locus_ordinal(),
                            limits,
                            stats,
                        )?;
                        stage_dense_atom(
                            &mut atoms,
                            right.structural_locus_ordinal(),
                            limits,
                            stats,
                        )?;
                    }
                }
            }
        }
    }
    bounded_sort_dense_atoms(&mut atoms, limits, stats)?;
    bounded_dedup_dense_atoms(&mut atoms, limits, stats)?;
    check_limit("normalized MTBDD atoms", atoms.len(), limits.max_atoms)?;
    stats.atoms = atoms.len();
    Ok(atoms)
}

fn stage_dense_atom(
    atoms: &mut Vec<ParametricSectorMtbddAtom>,
    structural_locus_ordinal: usize,
    limits: ParametricSectorMtbddLimits,
    stats: &mut ParametricSectorMtbddStats,
) -> Result<(), ParametricSectorMtbddError> {
    let staged = bounded_add(
        "normalized MTBDD atom staging entries",
        stats.atom_staging_entries,
        1,
        limits.max_atom_staging_entries,
    )?;
    atoms.push(ParametricSectorMtbddAtom {
        structural_locus_ordinal,
    });
    stats.atom_staging_entries = staged;
    Ok(())
}

fn bounded_sort_dense_atoms(
    atoms: &mut [ParametricSectorMtbddAtom],
    limits: ParametricSectorMtbddLimits,
    stats: &mut ParametricSectorMtbddStats,
) -> Result<(), ParametricSectorMtbddError> {
    check_limit(
        "normalized MTBDD atom sort scratch entries",
        atoms.len(),
        limits.max_atom_sort_scratch_entries,
    )?;
    let mut scratch = Vec::new();
    try_reserve_exact(
        "normalized MTBDD atom sort scratch",
        &mut scratch,
        atoms.len(),
    )?;
    scratch.extend_from_slice(atoms);
    stats.atom_sort_scratch_entries = scratch.len();

    let mut width = 1usize;
    let mut data_in_atoms = true;
    while width < atoms.len() {
        if data_in_atoms {
            merge_dense_atom_runs(atoms, &mut scratch, width, limits, stats)?;
        } else {
            merge_dense_atom_runs(&scratch, atoms, width, limits, stats)?;
        }
        data_in_atoms = !data_in_atoms;
        width = width.saturating_mul(2).min(atoms.len());
    }
    if !data_in_atoms {
        atoms.copy_from_slice(&scratch);
    }
    Ok(())
}

fn merge_dense_atom_runs(
    source: &[ParametricSectorMtbddAtom],
    destination: &mut [ParametricSectorMtbddAtom],
    width: usize,
    limits: ParametricSectorMtbddLimits,
    stats: &mut ParametricSectorMtbddStats,
) -> Result<(), ParametricSectorMtbddError> {
    let run_width = width.saturating_mul(2);
    let mut start = 0usize;
    while start < source.len() {
        let middle = start.saturating_add(width).min(source.len());
        let end = start.saturating_add(run_width).min(source.len());
        let mut left = start;
        let mut right = middle;
        for output in &mut destination[start..end] {
            let take_left = if left == middle {
                false
            } else if right == end {
                true
            } else {
                stats.atom_sort_comparisons = bounded_add(
                    "normalized MTBDD atom sort comparisons",
                    stats.atom_sort_comparisons,
                    1,
                    limits.max_atom_sort_comparisons,
                )?;
                source[left].structural_locus_ordinal <= source[right].structural_locus_ordinal
            };
            if take_left {
                *output = source[left];
                left += 1;
            } else {
                *output = source[right];
                right += 1;
            }
        }
        start = end;
    }
    Ok(())
}

fn bounded_dedup_dense_atoms(
    atoms: &mut Vec<ParametricSectorMtbddAtom>,
    limits: ParametricSectorMtbddLimits,
    stats: &mut ParametricSectorMtbddStats,
) -> Result<(), ParametricSectorMtbddError> {
    let mut retained = 0usize;
    for scanned in 0..atoms.len() {
        stats.atom_dedup_scans = bounded_add(
            "normalized MTBDD atom dedup scans",
            stats.atom_dedup_scans,
            1,
            limits.max_atom_dedup_scans,
        )?;
        if retained == 0
            || atoms[retained - 1].structural_locus_ordinal
                != atoms[scanned].structural_locus_ordinal
        {
            atoms[retained] = atoms[scanned];
            retained += 1;
        }
    }
    atoms.truncate(retained);
    Ok(())
}

fn compile_formula<T: CoverageDecisionTerminalPayload>(
    operation: &mut crate::coverage_decision_dag::CoverageDecisionDagOperation<'_, T>,
    body: &NormalizedBadFormulaBody,
    atoms: &[ParametricSectorMtbddAtom],
    boolean: CoverageDecisionBooleanTerminals,
    comparisons: &mut usize,
    steps: &mut usize,
    limits: ParametricSectorMtbddLimits,
) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
    charge_core_counter(
        "MTBDD formula compile steps",
        steps,
        limits.max_formula_compile_steps,
    )?;
    match body {
        NormalizedBadFormulaBody::False => Ok(boolean.when_false()),
        NormalizedBadFormulaBody::True { .. } => Ok(boolean.when_true()),
        NormalizedBadFormulaBody::Dnf {
            clauses,
            atomic_equal_zero_factors,
        } => {
            let mut root = boolean.when_false();
            for factor in atomic_equal_zero_factors.iter().rev() {
                charge_core_counter(
                    "MTBDD formula compile steps",
                    steps,
                    limits.max_formula_compile_steps,
                )?;
                let atom = dense_atom_for_locus(
                    atoms,
                    factor.structural_locus_ordinal(),
                    comparisons,
                    limits.max_atom_lookup_comparisons,
                )?;
                root = operation.branch(atom, root, boolean.when_true())?;
            }
            for clause in clauses.iter() {
                if clause.role() == NormalizedBadClauseRole::AtomicEqualZeroFactor {
                    continue;
                }
                charge_core_counter(
                    "MTBDD formula compile steps",
                    steps,
                    limits.max_formula_compile_steps,
                )?;
                let clause_root = match clause.body() {
                    DirectBadFormulaClause::Atom(literal) => compile_literal(
                        operation,
                        literal,
                        atoms,
                        boolean,
                        comparisons,
                        steps,
                        limits,
                    )?,
                    DirectBadFormulaClause::Conjunction(left, right) => {
                        let left = compile_literal(
                            operation,
                            left,
                            atoms,
                            boolean,
                            comparisons,
                            steps,
                            limits,
                        )?;
                        let right = compile_literal(
                            operation,
                            right,
                            atoms,
                            boolean,
                            comparisons,
                            steps,
                            limits,
                        )?;
                        operation.boolean_and(left, right, boolean)?
                    }
                };
                root = operation.boolean_or(root, clause_root, boolean)?;
            }
            Ok(root)
        }
    }
}

fn compile_literal<T: CoverageDecisionTerminalPayload>(
    operation: &mut crate::coverage_decision_dag::CoverageDecisionDagOperation<'_, T>,
    literal: NormalizedBadLiteral,
    atoms: &[ParametricSectorMtbddAtom],
    boolean: CoverageDecisionBooleanTerminals,
    comparisons: &mut usize,
    steps: &mut usize,
    limits: ParametricSectorMtbddLimits,
) -> Result<CoverageDecisionRef, CoverageDecisionDagError> {
    charge_core_counter(
        "MTBDD formula compile steps",
        steps,
        limits.max_formula_compile_steps,
    )?;
    let atom = dense_atom_for_locus(
        atoms,
        literal.structural_locus_ordinal(),
        comparisons,
        limits.max_atom_lookup_comparisons,
    )?;
    match literal.polarity() {
        NormalizedBadLiteralPolarity::EqualZero => {
            operation.branch(atom, boolean.when_false(), boolean.when_true())
        }
        NormalizedBadLiteralPolarity::NonZero => {
            operation.branch(atom, boolean.when_true(), boolean.when_false())
        }
    }
}

fn dense_atom_for_locus(
    atoms: &[ParametricSectorMtbddAtom],
    locus: usize,
    comparisons: &mut usize,
    limit: usize,
) -> Result<CoverageDecisionAtomId, CoverageDecisionDagError> {
    let mut low = 0usize;
    let mut high = atoms.len();
    while low < high {
        charge_core_counter("MTBDD atom lookup comparisons", comparisons, limit)?;
        let middle = low + (high - low) / 2;
        match atoms[middle].structural_locus_ordinal.cmp(&locus) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(CoverageDecisionAtomId::new(middle)),
        }
    }
    Err(CoverageDecisionDagError::InternalVariableOrderMismatch)
}

fn charge_core_counter(
    resource: &'static str,
    counter: &mut usize,
    limit: usize,
) -> Result<(), CoverageDecisionDagError> {
    let requested = counter
        .checked_add(1)
        .ok_or(CoverageDecisionDagError::ResourceCountOverflow { resource })?;
    if requested > limit {
        return Err(CoverageDecisionDagError::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    *counter = requested;
    Ok(())
}

/// Validate the production root's payload type boundary after construction.
///
/// This is deliberately not semantic replay: persisted acceptance must rebuild
/// from authenticated formula IR and compare the complete rooted view.  Merely
/// pointing at a well-typed candidate ordinal cannot prove the encoded Boolean
/// function.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TypedRootValidationStats {
    node_mark_entries: usize,
    terminal_mark_entries: usize,
    stack_entries: usize,
    stack_pushes: usize,
    visits: usize,
}

fn validate_typed_final_root(
    rooted: &CoverageDecisionDagRootedView<ParametricSectorMtbddTerminalPayload>,
    ir: &NormalizedCoverageIr,
    expected_atom_count: usize,
    limits: ParametricSectorMtbddLimits,
) -> Result<TypedRootValidationStats, ParametricSectorMtbddError> {
    if rooted.roots().len() != 1
        || rooted.atom_count() != expected_atom_count
        || rooted.boolean_false() == rooted.boolean_true()
    {
        return Err(ParametricSectorMtbddError::MalformedRootedView);
    }
    let endpoint_payload = |reference: CoverageDecisionPersistedRef| match reference {
        CoverageDecisionPersistedRef::Terminal(id) => rooted
            .terminal_payloads()
            .get(id.ordinal())
            .map(Arc::as_ref),
        CoverageDecisionPersistedRef::Node(_) => None,
    };
    if !matches!(
        endpoint_payload(rooted.boolean_false()),
        Some(ParametricSectorMtbddTerminalPayload::BooleanFalse)
    ) || !matches!(
        endpoint_payload(rooted.boolean_true()),
        Some(ParametricSectorMtbddTerminalPayload::BooleanTrue)
    ) {
        return Err(ParametricSectorMtbddError::MalformedRootedView);
    }
    check_limit(
        "typed-root node-mark entries",
        rooted.nodes().len(),
        limits.max_typed_root_node_mark_entries,
    )?;
    let mut node_seen = Vec::new();
    try_reserve_exact(
        "typed-root node marks",
        &mut node_seen,
        rooted.nodes().len(),
    )?;
    node_seen.resize(rooted.nodes().len(), false);
    check_limit(
        "typed-root terminal-mark entries",
        rooted.terminal_payloads().len(),
        limits.max_typed_root_terminal_mark_entries,
    )?;
    let mut terminal_seen = Vec::new();
    try_reserve_exact(
        "typed-root terminal marks",
        &mut terminal_seen,
        rooted.terminal_payloads().len(),
    )?;
    terminal_seen.resize(rooted.terminal_payloads().len(), false);
    let stack_entries = rooted.nodes().len().checked_add(1).ok_or(
        ParametricSectorMtbddError::ResourceCountOverflow {
            resource: "typed-root traversal stack entries",
        },
    )?;
    check_limit(
        "typed-root traversal stack entries",
        stack_entries,
        limits.max_typed_root_stack_entries,
    )?;
    let mut stack = Vec::new();
    try_reserve_exact("typed-root traversal stack", &mut stack, stack_entries)?;
    let mut stats = TypedRootValidationStats {
        node_mark_entries: node_seen.len(),
        terminal_mark_entries: terminal_seen.len(),
        stack_entries,
        ..TypedRootValidationStats::default()
    };
    push_typed_root_reference(&mut stack, rooted.roots()[0], limits, &mut stats)?;
    let expected_unsupported_count = ir
        .attempts()
        .iter()
        .filter(|attempt| matches!(attempt, NormalizedCoverageAttempt::Unsupported { .. }))
        .count();
    stats.visits = bounded_add(
        "typed final-root visits",
        0,
        2,
        limits.max_typed_root_visits,
    )?;
    while let Some(reference) = stack.pop() {
        match reference {
            CoverageDecisionPersistedRef::Terminal(id) => {
                let seen = terminal_seen
                    .get_mut(id.ordinal())
                    .ok_or(ParametricSectorMtbddError::MalformedRootedView)?;
                if *seen {
                    continue;
                }
                *seen = true;
                stats.visits = bounded_add(
                    "typed final-root visits",
                    stats.visits,
                    1,
                    limits.max_typed_root_visits,
                )?;
                let payload = rooted
                    .terminal_payloads()
                    .get(id.ordinal())
                    .ok_or(ParametricSectorMtbddError::MalformedRootedView)?;
                match payload.as_ref() {
                    ParametricSectorMtbddTerminalPayload::BooleanFalse
                    | ParametricSectorMtbddTerminalPayload::BooleanTrue => {
                        return Err(ParametricSectorMtbddError::BooleanTerminalReachable);
                    }
                    ParametricSectorMtbddTerminalPayload::Disposition(
                        ParametricSectorMtbddDisposition::DescendingRule { candidate_ordinal },
                    ) => {
                        let attempt = ir.attempts().get(*candidate_ordinal).ok_or(
                            ParametricSectorMtbddError::DispositionCandidateOutOfRange {
                                candidate_ordinal: *candidate_ordinal,
                                attempt_count: ir.attempts().len(),
                            },
                        )?;
                        if !matches!(attempt, NormalizedCoverageAttempt::Certified(_)) {
                            return Err(
                                ParametricSectorMtbddError::DispositionCandidateIsNotCertified {
                                    candidate_ordinal: *candidate_ordinal,
                                },
                            );
                        }
                    }
                    ParametricSectorMtbddTerminalPayload::Disposition(
                        ParametricSectorMtbddDisposition::Uncovered,
                    ) => {
                        if expected_unsupported_count != 0 {
                            return Err(ParametricSectorMtbddError::UnsupportedFallbackMismatch);
                        }
                    }
                    ParametricSectorMtbddTerminalPayload::Disposition(
                        ParametricSectorMtbddDisposition::Unsupported { candidate_ordinals },
                    ) => {
                        if expected_unsupported_count == 0
                            || !unsupported_ordinals_match(ir, candidate_ordinals)
                        {
                            return Err(ParametricSectorMtbddError::UnsupportedFallbackMismatch);
                        }
                    }
                }
            }
            CoverageDecisionPersistedRef::Node(id) => {
                let seen = node_seen
                    .get_mut(id.ordinal())
                    .ok_or(ParametricSectorMtbddError::MalformedRootedView)?;
                if *seen {
                    continue;
                }
                *seen = true;
                stats.visits = bounded_add(
                    "typed final-root visits",
                    stats.visits,
                    1,
                    limits.max_typed_root_visits,
                )?;
                let node = rooted
                    .nodes()
                    .get(id.ordinal())
                    .copied()
                    .ok_or(ParametricSectorMtbddError::MalformedRootedView)?;
                push_typed_root_reference(&mut stack, node.when_true(), limits, &mut stats)?;
                push_typed_root_reference(&mut stack, node.when_false(), limits, &mut stats)?;
            }
        }
    }
    Ok(stats)
}

fn push_typed_root_reference(
    stack: &mut Vec<CoverageDecisionPersistedRef>,
    reference: CoverageDecisionPersistedRef,
    limits: ParametricSectorMtbddLimits,
    stats: &mut TypedRootValidationStats,
) -> Result<(), ParametricSectorMtbddError> {
    let requested_entries = checked_add("typed-root traversal stack entries", stack.len(), 1)?;
    check_limit(
        "typed-root traversal stack entries",
        requested_entries,
        limits.max_typed_root_stack_entries,
    )?;
    let requested_pushes = bounded_add(
        "typed-root traversal stack pushes",
        stats.stack_pushes,
        1,
        limits.max_typed_root_stack_pushes,
    )?;
    stack.push(reference);
    stats.stack_pushes = requested_pushes;
    Ok(())
}

fn unsupported_ordinals_match(ir: &NormalizedCoverageIr, actual: &[usize]) -> bool {
    let mut actual = actual.iter().copied();
    for attempt in ir.attempts() {
        if let NormalizedCoverageAttempt::Unsupported {
            source_attempt_ordinal,
        } = attempt
            && actual.next() != Some(*source_attempt_ordinal)
        {
            return false;
        }
    }
    actual.next().is_none()
}

pub(crate) fn reference_disposition_for_assignment(
    ir: &NormalizedCoverageIr,
    zero_by_base_structural_locus: &[bool],
) -> Result<ParametricSectorMtbddDisposition, ParametricSectorMtbddError> {
    if zero_by_base_structural_locus.len() != ir.base_structural_locus_count() {
        return Err(ParametricSectorMtbddError::AssignmentArityMismatch {
            expected: ir.base_structural_locus_count(),
            actual: zero_by_base_structural_locus.len(),
        });
    }
    for attempt in ir.attempts() {
        if let NormalizedCoverageAttempt::Certified(formula) = attempt
            && !evaluate_formula(formula.body(), zero_by_base_structural_locus)
        {
            return Ok(ParametricSectorMtbddDisposition::DescendingRule {
                candidate_ordinal: formula.source_attempt_ordinal(),
            });
        }
    }
    let unsupported_count = ir
        .attempts()
        .iter()
        .filter(|attempt| matches!(attempt, NormalizedCoverageAttempt::Unsupported { .. }))
        .count();
    let mut unsupported = Vec::new();
    try_reserve_exact(
        "reference unsupported ordinals",
        &mut unsupported,
        unsupported_count,
    )?;
    for attempt in ir.attempts() {
        if let NormalizedCoverageAttempt::Unsupported {
            source_attempt_ordinal,
        } = attempt
        {
            unsupported.push(*source_attempt_ordinal);
        }
    }
    Ok(if unsupported.is_empty() {
        ParametricSectorMtbddDisposition::Uncovered
    } else {
        ParametricSectorMtbddDisposition::Unsupported {
            candidate_ordinals: unsupported.into_boxed_slice(),
        }
    })
}

fn evaluate_formula(body: &NormalizedBadFormulaBody, zero: &[bool]) -> bool {
    match body {
        NormalizedBadFormulaBody::False => false,
        NormalizedBadFormulaBody::True { .. } => true,
        NormalizedBadFormulaBody::Dnf { clauses, .. } => clauses.iter().any(|clause| {
            let literal = |literal: NormalizedBadLiteral| match literal.polarity() {
                NormalizedBadLiteralPolarity::EqualZero => zero[literal.structural_locus_ordinal()],
                NormalizedBadLiteralPolarity::NonZero => !zero[literal.structural_locus_ordinal()],
            };
            match clause.body() {
                DirectBadFormulaClause::Atom(atom) => literal(atom),
                DirectBadFormulaClause::Conjunction(left, right) => literal(left) && literal(right),
            }
        }),
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorMtbddError> {
    left.checked_add(right)
        .ok_or(ParametricSectorMtbddError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricSectorMtbddError> {
    if requested > limit {
        Err(ParametricSectorMtbddError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    delta: usize,
    limit: usize,
) -> Result<usize, ParametricSectorMtbddError> {
    let requested = checked_add(resource, current, delta)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), ParametricSectorMtbddError> {
    values.try_reserve_exact(additional).map_err(|_| {
        ParametricSectorMtbddError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}

fn try_reserve_core_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), CoverageDecisionDagError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| CoverageDecisionDagError::AllocationFailure {
            resource,
            requested: additional,
        })
}

fn try_reserve_core<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), CoverageDecisionDagError> {
    values
        .try_reserve(additional)
        .map_err(|_| CoverageDecisionDagError::AllocationFailure {
            resource,
            requested: additional,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage_decision_dag::CoverageDecisionDag;
    use crate::parametric_sector_formula_ir::{
        NormalizedBadClause, NormalizedBadClauseRole, NormalizedBadClauseSource,
        NormalizedCandidateBadFormula, NormalizedFactorZeroSource,
    };

    fn source(ordinal: usize) -> Box<[NormalizedBadClauseSource]> {
        vec![NormalizedBadClauseSource::LeakEvent {
            event_ordinal: ordinal,
        }]
        .into_boxed_slice()
    }

    fn literal(locus: usize, polarity: NormalizedBadLiteralPolarity) -> NormalizedBadLiteral {
        NormalizedBadLiteral::new(locus, polarity)
    }

    fn typed_root_limits(max_typed_root_visits: usize) -> ParametricSectorMtbddLimits {
        ParametricSectorMtbddLimits {
            max_typed_root_visits,
            ..ParametricSectorMtbddLimits::default()
        }
    }

    fn clause(
        body: DirectBadFormulaClause<NormalizedBadLiteral>,
        source_ordinal: usize,
        role: NormalizedBadClauseRole,
    ) -> NormalizedBadClause {
        NormalizedBadClause::new(body, source(source_ordinal), role)
    }

    fn dnf_attempt(
        ordinal: usize,
        clauses: Vec<NormalizedBadClause>,
        factors: Vec<NormalizedFactorZeroSource>,
    ) -> NormalizedCoverageAttempt {
        NormalizedCoverageAttempt::Certified(NormalizedCandidateBadFormula::new(
            ordinal,
            NormalizedBadFormulaBody::Dnf {
                clauses: clauses.into_boxed_slice(),
                atomic_equal_zero_factors: factors.into_boxed_slice(),
            },
        ))
    }

    fn false_attempt(ordinal: usize) -> NormalizedCoverageAttempt {
        NormalizedCoverageAttempt::Certified(NormalizedCandidateBadFormula::new(
            ordinal,
            NormalizedBadFormulaBody::False,
        ))
    }

    fn true_attempt(ordinal: usize) -> NormalizedCoverageAttempt {
        NormalizedCoverageAttempt::Certified(NormalizedCandidateBadFormula::new(
            ordinal,
            NormalizedBadFormulaBody::True {
                sources: source(ordinal),
            },
        ))
    }

    fn unsupported(ordinal: usize) -> NormalizedCoverageAttempt {
        NormalizedCoverageAttempt::Unsupported {
            source_attempt_ordinal: ordinal,
        }
    }

    fn ir(base_count: usize, attempts: Vec<NormalizedCoverageAttempt>) -> NormalizedCoverageIr {
        NormalizedCoverageIr::try_new(base_count, attempts.into_boxed_slice()).unwrap()
    }

    fn compile(ir: &NormalizedCoverageIr) -> ParametricSectorMtbddDecisionFunction {
        ParametricSectorMtbddCompiler::compile(ir, ParametricSectorMtbddLimits::default()).unwrap()
    }

    #[derive(Clone, Copy)]
    enum OracleLiteral {
        EqualZero(usize),
        NonZero(usize),
    }

    #[derive(Clone)]
    enum OracleClause {
        Atom(OracleLiteral),
        Conjunction(OracleLiteral, OracleLiteral),
    }

    #[derive(Clone)]
    enum OracleAttempt {
        Certified {
            ordinal: usize,
            bad: Vec<OracleClause>,
        },
        Unsupported {
            ordinal: usize,
        },
    }

    fn oracle_literal(literal: OracleLiteral, zero: &[bool]) -> bool {
        match literal {
            OracleLiteral::EqualZero(locus) => zero[locus],
            OracleLiteral::NonZero(locus) => !zero[locus],
        }
    }

    fn oracle_disposition(
        attempts: &[OracleAttempt],
        zero: &[bool],
    ) -> ParametricSectorMtbddDisposition {
        for attempt in attempts {
            if let OracleAttempt::Certified { ordinal, bad } = attempt {
                let is_bad = bad.iter().any(|clause| match *clause {
                    OracleClause::Atom(literal) => oracle_literal(literal, zero),
                    OracleClause::Conjunction(left, right) => {
                        oracle_literal(left, zero) && oracle_literal(right, zero)
                    }
                });
                if !is_bad {
                    return ParametricSectorMtbddDisposition::DescendingRule {
                        candidate_ordinal: *ordinal,
                    };
                }
            }
        }
        let unsupported = attempts
            .iter()
            .filter_map(|attempt| match attempt {
                OracleAttempt::Unsupported { ordinal } => Some(*ordinal),
                OracleAttempt::Certified { .. } => None,
            })
            .collect::<Vec<_>>();
        if unsupported.is_empty() {
            ParametricSectorMtbddDisposition::Uncovered
        } else {
            ParametricSectorMtbddDisposition::Unsupported {
                candidate_ordinals: unsupported.into_boxed_slice(),
            }
        }
    }

    #[test]
    fn true_bad_skips_candidate_and_false_bad_cuts_off_without_dropping_suffix_census() {
        let function = compile(&ir(
            0,
            vec![
                true_attempt(0),
                unsupported(1),
                false_attempt(2),
                unsupported(3),
                false_attempt(4),
            ],
        ));
        assert_eq!(function.stats().attempts, 5);
        assert_eq!(function.stats().certified_attempts, 3);
        assert_eq!(function.stats().unsupported_attempts, 2);
        assert_eq!(
            function.classify_assignment(&[]).unwrap(),
            &ParametricSectorMtbddDisposition::DescendingRule {
                candidate_ordinal: 2,
            }
        );

        let all_bad = compile(&ir(
            0,
            vec![true_attempt(0), unsupported(1), true_attempt(2)],
        ));
        assert_eq!(
            all_bad.classify_assignment(&[]).unwrap(),
            &ParametricSectorMtbddDisposition::Unsupported {
                candidate_ordinals: vec![1].into_boxed_slice(),
            }
        );
    }

    #[test]
    fn sparse_loci_compile_densely_and_match_an_independent_exhaustive_oracle() {
        let normalized = ir(
            12,
            vec![
                dnf_attempt(
                    0,
                    vec![
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                2,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            0,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                        clause(
                            DirectBadFormulaClause::Conjunction(
                                literal(5, NormalizedBadLiteralPolarity::NonZero),
                                literal(9, NormalizedBadLiteralPolarity::EqualZero),
                            ),
                            1,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                    ],
                    vec![NormalizedFactorZeroSource::new(2, 0)],
                ),
                false_attempt(1),
            ],
        );
        let function = compile(&normalized);
        assert_eq!(function.base_structural_locus_count(), 12);
        assert_eq!(function.rooted().atom_count(), 3);
        assert_eq!(
            function
                .atoms()
                .iter()
                .map(|atom| atom.structural_locus_ordinal())
                .collect::<Vec<_>>(),
            vec![2, 5, 9]
        );

        let oracle = vec![
            OracleAttempt::Certified {
                ordinal: 0,
                bad: vec![
                    OracleClause::Atom(OracleLiteral::EqualZero(2)),
                    OracleClause::Conjunction(
                        OracleLiteral::NonZero(5),
                        OracleLiteral::EqualZero(9),
                    ),
                ],
            },
            OracleAttempt::Certified {
                ordinal: 1,
                bad: Vec::new(),
            },
        ];
        for mask in 0usize..8 {
            let mut zero = vec![false; 12];
            for (bit, locus) in [2usize, 5, 9].into_iter().enumerate() {
                zero[locus] = mask & (1 << bit) != 0;
            }
            assert_eq!(
                function.classify_assignment(&zero).unwrap(),
                &oracle_disposition(&oracle, &zero),
                "assignment mask {mask:03b}"
            );
        }
        let second = compile(&normalized);
        assert_eq!(function.rooted(), second.rooted());
    }

    #[test]
    fn equality_nonzero_and_boundary_gate_polarities_are_exact() {
        for polarity in [
            NormalizedBadLiteralPolarity::EqualZero,
            NormalizedBadLiteralPolarity::NonZero,
        ] {
            let normalized = ir(
                1,
                vec![
                    dnf_attempt(
                        0,
                        vec![clause(
                            DirectBadFormulaClause::Atom(literal(0, polarity)),
                            0,
                            NormalizedBadClauseRole::Ordinary,
                        )],
                        Vec::new(),
                    ),
                    false_attempt(1),
                ],
            );
            let function = compile(&normalized);
            for zero in [false, true] {
                let is_bad = match polarity {
                    NormalizedBadLiteralPolarity::EqualZero => zero,
                    NormalizedBadLiteralPolarity::NonZero => !zero,
                };
                assert_eq!(
                    function.classify_assignment(&[zero]).unwrap(),
                    &ParametricSectorMtbddDisposition::DescendingRule {
                        candidate_ordinal: usize::from(is_bad),
                    }
                );
            }
        }

        let normalized = ir(
            2,
            vec![
                dnf_attempt(
                    0,
                    vec![clause(
                        DirectBadFormulaClause::Conjunction(
                            literal(0, NormalizedBadLiteralPolarity::EqualZero),
                            literal(1, NormalizedBadLiteralPolarity::NonZero),
                        ),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
                false_attempt(1),
            ],
        );
        let function = compile(&normalized);
        for boundary_zero in [false, true] {
            for gate_zero in [false, true] {
                let bad = boundary_zero && !gate_zero;
                assert_eq!(
                    function
                        .classify_assignment(&[boundary_zero, gate_zero])
                        .unwrap(),
                    &ParametricSectorMtbddDisposition::DescendingRule {
                        candidate_ordinal: usize::from(bad),
                    }
                );
            }
        }
    }

    #[test]
    fn contradictory_conjunction_and_tautological_disjunction_reduce_canonically() {
        let equal = literal(0, NormalizedBadLiteralPolarity::EqualZero);
        let nonzero = literal(0, NormalizedBadLiteralPolarity::NonZero);
        let contradiction = ir(
            1,
            vec![
                dnf_attempt(
                    0,
                    vec![clause(
                        DirectBadFormulaClause::Conjunction(equal, nonzero),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
                false_attempt(1),
            ],
        );
        let function = compile(&contradiction);
        for zero in [false, true] {
            assert_eq!(
                function.classify_assignment(&[zero]).unwrap(),
                &ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: 0
                }
            );
        }

        let tautology = ir(
            1,
            vec![
                dnf_attempt(
                    0,
                    vec![
                        clause(
                            DirectBadFormulaClause::Atom(equal),
                            0,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                        clause(
                            DirectBadFormulaClause::Atom(nonzero),
                            1,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                    ],
                    Vec::new(),
                ),
                false_attempt(1),
            ],
        );
        let function = compile(&tautology);
        for zero in [false, true] {
            assert_eq!(
                function.classify_assignment(&[zero]).unwrap(),
                &ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: 1
                }
            );
        }
    }

    #[test]
    fn factor_or_and_ordinary_equal_zero_share_only_base_atoms() {
        let normalized = ir(
            6,
            vec![
                dnf_attempt(
                    0,
                    vec![
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                1,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            0,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                3,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            1,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                5,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            2,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                    ],
                    vec![
                        NormalizedFactorZeroSource::new(3, 1),
                        NormalizedFactorZeroSource::new(5, 2),
                    ],
                ),
                false_attempt(1),
            ],
        );
        let function = compile(&normalized);
        assert_eq!(
            function
                .atoms()
                .iter()
                .map(|atom| atom.structural_locus_ordinal())
                .collect::<Vec<_>>(),
            vec![1, 3, 5]
        );
        for mask in 0usize..8 {
            let zero = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0];
            let mut assignment = vec![false; 6];
            assignment[1] = zero[0];
            assignment[3] = zero[1];
            assignment[5] = zero[2];
            assert_eq!(
                function.classify_assignment(&assignment).unwrap(),
                &ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: usize::from(zero.into_iter().any(|value| value)),
                }
            );
        }

        let production = include_str!("parametric_sector_mtbdd.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for import in production
            .lines()
            .filter(|line| line.trim_start().starts_with("use "))
        {
            assert!(!import.contains("symbolica"));
            assert!(!import.contains("parametric_coefficient"));
        }
    }

    #[test]
    fn factor_decisions_short_circuit_in_ascending_dense_atom_order() {
        let normalized = ir(
            12,
            vec![
                dnf_attempt(
                    0,
                    vec![
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                2,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            0,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                7,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            1,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                11,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            2,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                    ],
                    vec![
                        NormalizedFactorZeroSource::new(2, 0),
                        NormalizedFactorZeroSource::new(7, 1),
                        NormalizedFactorZeroSource::new(11, 2),
                    ],
                ),
                false_attempt(1),
            ],
        );
        let function = compile(&normalized);
        let rebuilt = CoverageDecisionDag::rebuild_rooted(
            function.rooted(),
            CoverageDecisionDagLimits::default(),
        )
        .unwrap();
        let root = rebuilt.roots()[0];

        let mut queried = Vec::new();
        let payload = rebuilt
            .dag()
            .evaluate(root, |atom| {
                queried.push(atom.ordinal());
                Some(atom.ordinal() == 0)
            })
            .unwrap();
        assert!(matches!(
            payload,
            ParametricSectorMtbddTerminalPayload::Disposition(
                ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: 1
                }
            )
        ));
        assert_eq!(queried, vec![0]);

        queried.clear();
        rebuilt
            .dag()
            .evaluate(root, |atom| {
                queried.push(atom.ordinal());
                Some(false)
            })
            .unwrap();
        assert_eq!(queried, vec![0, 1, 2]);
    }

    #[test]
    fn priority_uncovered_and_exact_unsupported_fallback_are_preserved() {
        let overlapping = ir(
            2,
            vec![
                dnf_attempt(
                    0,
                    vec![clause(
                        DirectBadFormulaClause::Atom(literal(
                            0,
                            NormalizedBadLiteralPolarity::EqualZero,
                        )),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
                dnf_attempt(
                    1,
                    vec![clause(
                        DirectBadFormulaClause::Atom(literal(
                            1,
                            NormalizedBadLiteralPolarity::EqualZero,
                        )),
                        1,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
            ],
        );
        let function = compile(&overlapping);
        assert_eq!(
            function.classify_assignment(&[false, false]).unwrap(),
            &ParametricSectorMtbddDisposition::DescendingRule {
                candidate_ordinal: 0
            }
        );
        assert_eq!(
            function.classify_assignment(&[true, false]).unwrap(),
            &ParametricSectorMtbddDisposition::DescendingRule {
                candidate_ordinal: 1
            }
        );
        assert_eq!(
            function.classify_assignment(&[true, true]).unwrap(),
            &ParametricSectorMtbddDisposition::Uncovered
        );

        let empty = compile(&ir(0, Vec::new()));
        assert_eq!(
            empty.classify_assignment(&[]).unwrap(),
            &ParametricSectorMtbddDisposition::Uncovered
        );
        let fallback = compile(&ir(
            0,
            vec![unsupported(0), true_attempt(1), unsupported(2)],
        ));
        assert_eq!(
            fallback.classify_assignment(&[]).unwrap(),
            &ParametricSectorMtbddDisposition::Unsupported {
                candidate_ordinals: vec![0, 2].into_boxed_slice(),
            }
        );

        let oracle_attempts = vec![
            OracleAttempt::Unsupported { ordinal: 0 },
            OracleAttempt::Certified {
                ordinal: 1,
                bad: vec![OracleClause::Atom(OracleLiteral::EqualZero(0))],
            },
            OracleAttempt::Unsupported { ordinal: 2 },
        ];
        assert_eq!(
            oracle_disposition(&oracle_attempts, &[true]),
            ParametricSectorMtbddDisposition::Unsupported {
                candidate_ordinals: vec![0, 2].into_boxed_slice(),
            }
        );
        assert_eq!(
            oracle_disposition(&oracle_attempts, &[false]),
            ParametricSectorMtbddDisposition::DescendingRule {
                candidate_ordinal: 1,
            }
        );
        assert_eq!(
            oracle_disposition(
                &[OracleAttempt::Certified {
                    ordinal: 0,
                    bad: vec![OracleClause::Atom(OracleLiteral::EqualZero(0))],
                }],
                &[true],
            ),
            ParametricSectorMtbddDisposition::Uncovered
        );
    }

    #[test]
    fn conditional_prefix_survives_false_cutoff_and_eliminates_all_fallbacks() {
        let normalized = ir(
            1,
            vec![
                dnf_attempt(
                    0,
                    vec![clause(
                        DirectBadFormulaClause::Atom(literal(
                            0,
                            NormalizedBadLiteralPolarity::EqualZero,
                        )),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
                unsupported(1),
                false_attempt(2),
                unsupported(3),
            ],
        );
        let mut limits = ParametricSectorMtbddLimits::default();
        limits.max_unsupported_references = 0;
        let function = ParametricSectorMtbddCompiler::compile(&normalized, limits).unwrap();
        assert_eq!(function.stats().unsupported_attempts, 2);
        assert_eq!(function.stats().unsupported_references, 0);
        assert_eq!(function.stats().priority_candidate_pairs, 2);
        assert_eq!(
            function.classify_assignment(&[false]).unwrap(),
            &ParametricSectorMtbddDisposition::DescendingRule {
                candidate_ordinal: 0,
            }
        );
        assert_eq!(
            function.classify_assignment(&[true]).unwrap(),
            &ParametricSectorMtbddDisposition::DescendingRule {
                candidate_ordinal: 2,
            }
        );
        assert!(function.rooted().terminal_payloads().iter().all(|payload| {
            !matches!(
                payload.as_ref(),
                ParametricSectorMtbddTerminalPayload::Disposition(
                    ParametricSectorMtbddDisposition::Unsupported { .. }
                )
            )
        }));
    }

    #[test]
    fn factor_and_ordinary_same_locus_boolean_absorption_is_exact() {
        let factor = |source_ordinal| {
            clause(
                DirectBadFormulaClause::Atom(literal(0, NormalizedBadLiteralPolarity::EqualZero)),
                source_ordinal,
                NormalizedBadClauseRole::AtomicEqualZeroFactor,
            )
        };
        let tautology = ir(
            1,
            vec![
                dnf_attempt(
                    0,
                    vec![
                        factor(0),
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                0,
                                NormalizedBadLiteralPolarity::NonZero,
                            )),
                            1,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                    ],
                    vec![NormalizedFactorZeroSource::new(0, 0)],
                ),
                false_attempt(1),
            ],
        );
        let function = compile(&tautology);
        for zero in [false, true] {
            assert_eq!(
                function.classify_assignment(&[zero]).unwrap(),
                &ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: 1,
                }
            );
        }

        let absorption = ir(
            2,
            vec![
                dnf_attempt(
                    0,
                    vec![
                        factor(0),
                        clause(
                            DirectBadFormulaClause::Conjunction(
                                literal(0, NormalizedBadLiteralPolarity::NonZero),
                                literal(1, NormalizedBadLiteralPolarity::EqualZero),
                            ),
                            1,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                    ],
                    vec![NormalizedFactorZeroSource::new(0, 0)],
                ),
                false_attempt(1),
            ],
        );
        let function = compile(&absorption);
        for x_zero in [false, true] {
            for y_zero in [false, true] {
                assert_eq!(
                    function.classify_assignment(&[x_zero, y_zero]).unwrap(),
                    &ParametricSectorMtbddDisposition::DescendingRule {
                        candidate_ordinal: usize::from(x_zero || y_zero),
                    }
                );
            }
        }
    }

    #[test]
    fn constant_false_cutoff_constructs_no_dead_suffix_or_fallback() {
        let normalized = ir(
            2,
            vec![
                false_attempt(0),
                unsupported(1),
                dnf_attempt(
                    2,
                    vec![clause(
                        DirectBadFormulaClause::Conjunction(
                            literal(0, NormalizedBadLiteralPolarity::EqualZero),
                            literal(1, NormalizedBadLiteralPolarity::NonZero),
                        ),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
            ],
        );
        let mut limits = ParametricSectorMtbddLimits::default();
        limits.max_formula_compile_steps = 1;
        limits.max_priority_candidate_pairs = 1;
        limits.max_unsupported_references = 0;
        limits.dag.max_nodes = 0;
        limits.dag.max_unique_table_entries = 0;
        limits.dag.max_retained_child_references = 0;
        limits.dag.max_terminals = 3;
        limits.dag.max_terminal_index_entries = 3;
        limits.dag.max_retained_terminal_payload_handles = 6;
        let function = ParametricSectorMtbddCompiler::compile(&normalized, limits).unwrap();
        assert_eq!(function.stats().formula_compile_steps, 1);
        assert_eq!(function.stats().priority_candidate_pairs, 1);
        assert_eq!(function.stats().unsupported_attempts, 1);
        assert_eq!(function.stats().unsupported_references, 0);
        assert_eq!(function.rooted().nodes().len(), 0);
        assert_eq!(function.rooted().terminal_payloads().len(), 3);
        assert!(function.rooted().terminal_payloads().iter().all(|payload| {
            !matches!(
                payload.as_ref(),
                ParametricSectorMtbddTerminalPayload::Disposition(
                    ParametricSectorMtbddDisposition::Unsupported { .. }
                )
            )
        }));
        for assignment in [[false, false], [false, true], [true, false], [true, true]] {
            assert_eq!(
                function.classify_assignment(&assignment).unwrap(),
                &ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: 0
                }
            );
        }
    }

    fn manual_rooted(
        atom_count: usize,
        swapped_boolean_roles: bool,
        roots: usize,
        root_payload: ParametricSectorMtbddTerminalPayload,
        root_is_boolean: bool,
    ) -> CoverageDecisionDagRootedView<ParametricSectorMtbddTerminalPayload> {
        let mut dag =
            CoverageDecisionDag::new(atom_count, CoverageDecisionDagLimits::default()).unwrap();
        dag.with_operation(|operation| {
            let false_payload = if swapped_boolean_roles {
                ParametricSectorMtbddTerminalPayload::BooleanTrue
            } else {
                ParametricSectorMtbddTerminalPayload::BooleanFalse
            };
            let true_payload = if swapped_boolean_roles {
                ParametricSectorMtbddTerminalPayload::BooleanFalse
            } else {
                ParametricSectorMtbddTerminalPayload::BooleanTrue
            };
            let when_false = operation.intern_terminal(Arc::new(false_payload))?;
            let when_true = operation.intern_terminal(Arc::new(true_payload))?;
            let boolean = operation.boolean_terminals(when_false, when_true)?;
            let root = if root_is_boolean {
                when_false
            } else {
                operation.intern_terminal(Arc::new(root_payload))?
            };
            let root_list = vec![root; roots];
            operation.export_rooted(&root_list, boolean)
        })
        .unwrap()
    }

    fn manual_malformed_node_rooted()
    -> CoverageDecisionDagRootedView<ParametricSectorMtbddTerminalPayload> {
        let mut dag = CoverageDecisionDag::new(1, CoverageDecisionDagLimits::default()).unwrap();
        dag.with_operation(|operation| {
            let when_false = operation
                .intern_terminal(Arc::new(ParametricSectorMtbddTerminalPayload::BooleanFalse))?;
            let when_true = operation
                .intern_terminal(Arc::new(ParametricSectorMtbddTerminalPayload::BooleanTrue))?;
            let boolean = operation.boolean_terminals(when_false, when_true)?;
            let uncovered = operation.intern_terminal(Arc::new(
                ParametricSectorMtbddTerminalPayload::Disposition(
                    ParametricSectorMtbddDisposition::Uncovered,
                ),
            ))?;
            let out_of_range = operation.intern_terminal(Arc::new(
                ParametricSectorMtbddTerminalPayload::Disposition(
                    ParametricSectorMtbddDisposition::DescendingRule {
                        candidate_ordinal: 1,
                    },
                ),
            ))?;
            let root = operation.branch(CoverageDecisionAtomId::new(0), uncovered, out_of_range)?;
            operation.export_rooted(&[root], boolean)
        })
        .unwrap()
    }

    #[test]
    fn typed_root_validation_rejects_boolean_and_payload_tampering() {
        let empty = ir(0, Vec::new());
        let boolean_root = manual_rooted(
            0,
            false,
            1,
            ParametricSectorMtbddTerminalPayload::Disposition(
                ParametricSectorMtbddDisposition::Uncovered,
            ),
            true,
        );
        assert!(matches!(
            validate_typed_final_root(&boolean_root, &empty, 0, typed_root_limits(16)),
            Err(ParametricSectorMtbddError::BooleanTerminalReachable)
        ));
        let swapped = manual_rooted(
            0,
            true,
            1,
            ParametricSectorMtbddTerminalPayload::Disposition(
                ParametricSectorMtbddDisposition::Uncovered,
            ),
            false,
        );
        assert!(matches!(
            validate_typed_final_root(&swapped, &empty, 0, typed_root_limits(16)),
            Err(ParametricSectorMtbddError::MalformedRootedView)
        ));
        let two_roots = manual_rooted(
            0,
            false,
            2,
            ParametricSectorMtbddTerminalPayload::Disposition(
                ParametricSectorMtbddDisposition::Uncovered,
            ),
            false,
        );
        assert!(matches!(
            validate_typed_final_root(&two_roots, &empty, 0, typed_root_limits(16)),
            Err(ParametricSectorMtbddError::MalformedRootedView)
        ));

        let unsupported_ir = ir(0, vec![unsupported(0)]);
        let bad_descending = manual_rooted(
            0,
            false,
            1,
            ParametricSectorMtbddTerminalPayload::Disposition(
                ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: 0,
                },
            ),
            false,
        );
        assert!(matches!(
            validate_typed_final_root(&bad_descending, &unsupported_ir, 0, typed_root_limits(16),),
            Err(
                ParametricSectorMtbddError::DispositionCandidateIsNotCertified {
                    candidate_ordinal: 0
                }
            )
        ));

        let unsupported_empty = manual_rooted(
            0,
            false,
            1,
            ParametricSectorMtbddTerminalPayload::Disposition(
                ParametricSectorMtbddDisposition::Unsupported {
                    candidate_ordinals: Vec::new().into_boxed_slice(),
                },
            ),
            false,
        );
        assert!(matches!(
            validate_typed_final_root(&unsupported_empty, &empty, 0, typed_root_limits(16)),
            Err(ParametricSectorMtbddError::UnsupportedFallbackMismatch)
        ));

        let wrong_atom_count = manual_rooted(
            1,
            false,
            1,
            ParametricSectorMtbddTerminalPayload::Disposition(
                ParametricSectorMtbddDisposition::Uncovered,
            ),
            false,
        );
        assert!(matches!(
            validate_typed_final_root(&wrong_atom_count, &empty, 0, typed_root_limits(16)),
            Err(ParametricSectorMtbddError::MalformedRootedView)
        ));

        let out_of_range = manual_rooted(
            0,
            false,
            1,
            ParametricSectorMtbddTerminalPayload::Disposition(
                ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: 1,
                },
            ),
            false,
        );
        assert!(matches!(
            validate_typed_final_root(&out_of_range, &empty, 0, typed_root_limits(16)),
            Err(ParametricSectorMtbddError::DispositionCandidateOutOfRange {
                candidate_ordinal: 1,
                attempt_count: 0,
            })
        ));

        let exact_fallback_ir = ir(0, vec![unsupported(0), true_attempt(1), unsupported(2)]);
        for ordinals in [vec![2, 0], vec![0], vec![0, 1, 2]] {
            let malformed = manual_rooted(
                0,
                false,
                1,
                ParametricSectorMtbddTerminalPayload::Disposition(
                    ParametricSectorMtbddDisposition::Unsupported {
                        candidate_ordinals: ordinals.into_boxed_slice(),
                    },
                ),
                false,
            );
            assert!(matches!(
                validate_typed_final_root(&malformed, &exact_fallback_ir, 0, typed_root_limits(16),),
                Err(ParametricSectorMtbddError::UnsupportedFallbackMismatch)
            ));
        }
    }

    #[test]
    fn typed_root_resources_are_preflighted_on_a_malformed_payload_path() {
        let empty = ir(0, Vec::new());
        let rooted = manual_malformed_node_rooted();
        let node_mark_entries = rooted.nodes().len();
        let terminal_mark_entries = rooted.terminal_payloads().len();
        let stack_entries = node_mark_entries + 1;
        let stack_pushes = 3;

        let mut exact = typed_root_limits(16);
        exact.max_typed_root_node_mark_entries = node_mark_entries;
        exact.max_typed_root_terminal_mark_entries = terminal_mark_entries;
        exact.max_typed_root_stack_entries = stack_entries;
        exact.max_typed_root_stack_pushes = stack_pushes;
        assert!(matches!(
            validate_typed_final_root(&rooted, &empty, 1, exact),
            Err(ParametricSectorMtbddError::DispositionCandidateOutOfRange {
                candidate_ordinal: 1,
                attempt_count: 0,
            })
        ));

        macro_rules! one_below {
            ($field:ident, $value:expr, $resource:literal) => {{
                let mut limits = typed_root_limits(16);
                limits.$field = $value - 1;
                assert!(matches!(
                    validate_typed_final_root(&rooted, &empty, 1, limits),
                    Err(ParametricSectorMtbddError::ResourceLimit {
                        resource: actual_resource,
                        requested,
                        limit,
                    }) if actual_resource == $resource
                        && requested == $value
                        && limit == $value - 1
                ));
            }};
        }
        one_below!(
            max_typed_root_node_mark_entries,
            node_mark_entries,
            "typed-root node-mark entries"
        );
        one_below!(
            max_typed_root_terminal_mark_entries,
            terminal_mark_entries,
            "typed-root terminal-mark entries"
        );
        one_below!(
            max_typed_root_stack_entries,
            stack_entries,
            "typed-root traversal stack entries"
        );
        one_below!(
            max_typed_root_stack_pushes,
            stack_pushes,
            "typed-root traversal stack pushes"
        );
    }

    fn resource_fixture() -> NormalizedCoverageIr {
        ir(
            5,
            vec![
                unsupported(0),
                dnf_attempt(
                    1,
                    vec![
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                0,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            0,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                3,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            2,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                4,
                                NormalizedBadLiteralPolarity::EqualZero,
                            )),
                            3,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                        clause(
                            DirectBadFormulaClause::Conjunction(
                                literal(1, NormalizedBadLiteralPolarity::NonZero),
                                literal(2, NormalizedBadLiteralPolarity::EqualZero),
                            ),
                            1,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                    ],
                    vec![
                        NormalizedFactorZeroSource::new(3, 1),
                        NormalizedFactorZeroSource::new(4, 2),
                    ],
                ),
                true_attempt(2),
            ],
        )
    }

    fn assert_outer_limit(
        result: Result<ParametricSectorMtbddDecisionFunction, ParametricSectorMtbddError>,
        resource: &'static str,
        requested: usize,
        limit: usize,
    ) {
        assert!(matches!(
            result,
            Err(ParametricSectorMtbddError::ResourceLimit {
                resource: actual_resource,
                requested: actual_requested,
                limit: actual_limit,
            }) if actual_resource == resource
                && actual_requested == requested
                && actual_limit == limit
        ));
    }

    fn assert_core_limit(
        result: Result<ParametricSectorMtbddDecisionFunction, ParametricSectorMtbddError>,
        resource: &'static str,
        requested: usize,
        limit: usize,
    ) {
        assert!(matches!(
            result,
            Err(ParametricSectorMtbddError::Core(
                CoverageDecisionDagError::ResourceLimit {
                    resource: actual_resource,
                    requested: actual_requested,
                    limit: actual_limit,
                }
            )) if actual_resource == resource
                && actual_requested == requested
                && actual_limit == limit
        ));
    }

    #[test]
    fn every_mtbdd_compilation_resource_limit_has_exact_and_one_below_evidence() {
        let normalized = resource_fixture();
        let baseline = compile(&normalized);
        let stats = baseline.stats();
        assert!(stats.base_structural_loci > 0);
        assert!(stats.attempts > 0);
        assert!(stats.normalized_clauses > 0);
        assert!(stats.normalized_literals > 0);
        assert!(stats.clause_source_references > 0);
        assert!(stats.factor_lists > 0);
        assert!(stats.factor_references > 0);
        assert!(stats.atom_staging_entries > 0);
        assert!(stats.atom_sort_scratch_entries > 0);
        assert!(stats.atom_sort_comparisons > 0);
        assert!(stats.atom_dedup_scans > 0);
        assert!(stats.atoms > 0);
        assert!(stats.atom_lookup_comparisons > 0);
        assert!(stats.formula_compile_steps > 0);
        assert!(stats.priority_candidate_pairs > 0);
        assert!(stats.unsupported_references > 0);
        assert!(stats.typed_root_node_mark_entries > 0);
        assert!(stats.typed_root_terminal_mark_entries > 0);
        assert!(stats.typed_root_stack_entries > 0);
        assert!(stats.typed_root_stack_pushes > 0);
        assert!(stats.typed_root_visits > 0);

        let mut exact = ParametricSectorMtbddLimits::default();
        exact.max_base_structural_loci = stats.base_structural_loci;
        exact.max_attempts = stats.attempts;
        exact.max_normalized_clauses = stats.normalized_clauses;
        exact.max_normalized_literals = stats.normalized_literals;
        exact.max_clause_source_references = stats.clause_source_references;
        exact.max_factor_lists = stats.factor_lists;
        exact.max_factor_references = stats.factor_references;
        exact.max_atom_staging_entries = stats.atom_staging_entries;
        exact.max_atom_sort_scratch_entries = stats.atom_sort_scratch_entries;
        exact.max_atom_sort_comparisons = stats.atom_sort_comparisons;
        exact.max_atom_dedup_scans = stats.atom_dedup_scans;
        exact.max_atoms = stats.atoms;
        exact.max_atom_lookup_comparisons = stats.atom_lookup_comparisons;
        exact.max_formula_compile_steps = stats.formula_compile_steps;
        exact.max_priority_candidate_pairs = stats.priority_candidate_pairs;
        exact.max_unsupported_references = stats.unsupported_references;
        exact.max_typed_root_node_mark_entries = stats.typed_root_node_mark_entries;
        exact.max_typed_root_terminal_mark_entries = stats.typed_root_terminal_mark_entries;
        exact.max_typed_root_stack_entries = stats.typed_root_stack_entries;
        exact.max_typed_root_stack_pushes = stats.typed_root_stack_pushes;
        exact.max_typed_root_visits = stats.typed_root_visits;
        let exact_function = ParametricSectorMtbddCompiler::compile(&normalized, exact).unwrap();
        assert_eq!(exact_function.rooted(), baseline.rooted());

        macro_rules! outer_one_below {
            ($field:ident, $value:expr, $resource:literal) => {{
                let mut limits = ParametricSectorMtbddLimits::default();
                limits.$field = $value - 1;
                assert_outer_limit(
                    ParametricSectorMtbddCompiler::compile(&normalized, limits),
                    $resource,
                    $value,
                    $value - 1,
                );
            }};
        }
        macro_rules! core_one_below {
            ($field:ident, $value:expr, $resource:literal) => {{
                let mut limits = ParametricSectorMtbddLimits::default();
                limits.$field = $value - 1;
                assert_core_limit(
                    ParametricSectorMtbddCompiler::compile(&normalized, limits),
                    $resource,
                    $value,
                    $value - 1,
                );
            }};
        }
        outer_one_below!(
            max_base_structural_loci,
            stats.base_structural_loci,
            "base structural loci"
        );
        outer_one_below!(max_attempts, stats.attempts, "normalized coverage attempts");
        outer_one_below!(
            max_normalized_clauses,
            stats.normalized_clauses,
            "normalized bad clauses"
        );
        outer_one_below!(
            max_normalized_literals,
            stats.normalized_literals,
            "normalized bad literals"
        );
        outer_one_below!(
            max_clause_source_references,
            stats.clause_source_references,
            "normalized clause source references"
        );
        outer_one_below!(
            max_factor_lists,
            stats.factor_lists,
            "normalized factor lists"
        );
        outer_one_below!(
            max_factor_references,
            stats.factor_references,
            "normalized factor references"
        );
        outer_one_below!(
            max_atom_staging_entries,
            stats.atom_staging_entries,
            "normalized MTBDD atom staging entries"
        );
        outer_one_below!(
            max_atom_sort_scratch_entries,
            stats.atom_sort_scratch_entries,
            "normalized MTBDD atom sort scratch entries"
        );
        outer_one_below!(
            max_atom_sort_comparisons,
            stats.atom_sort_comparisons,
            "normalized MTBDD atom sort comparisons"
        );
        outer_one_below!(
            max_atom_dedup_scans,
            stats.atom_dedup_scans,
            "normalized MTBDD atom dedup scans"
        );
        outer_one_below!(max_atoms, stats.atoms, "normalized MTBDD atoms");
        core_one_below!(
            max_atom_lookup_comparisons,
            stats.atom_lookup_comparisons,
            "MTBDD atom lookup comparisons"
        );
        core_one_below!(
            max_formula_compile_steps,
            stats.formula_compile_steps,
            "MTBDD formula compile steps"
        );
        core_one_below!(
            max_priority_candidate_pairs,
            stats.priority_candidate_pairs,
            "MTBDD priority candidate pairs"
        );
        core_one_below!(
            max_unsupported_references,
            stats.unsupported_references,
            "MTBDD unsupported fallback references"
        );
        outer_one_below!(
            max_typed_root_node_mark_entries,
            stats.typed_root_node_mark_entries,
            "typed-root node-mark entries"
        );
        outer_one_below!(
            max_typed_root_terminal_mark_entries,
            stats.typed_root_terminal_mark_entries,
            "typed-root terminal-mark entries"
        );
        outer_one_below!(
            max_typed_root_stack_entries,
            stats.typed_root_stack_entries,
            "typed-root traversal stack entries"
        );
        outer_one_below!(
            max_typed_root_stack_pushes,
            stats.typed_root_stack_pushes,
            "typed-root traversal stack pushes"
        );
        outer_one_below!(
            max_typed_root_visits,
            stats.typed_root_visits,
            "typed final-root visits"
        );
    }

    #[test]
    fn classification_budget_and_rooted_rebuild_are_enforced() {
        let normalized = ir(0, Vec::new());
        let mut exact = ParametricSectorMtbddLimits::default();
        exact.max_classification_visits = 1;
        let function = ParametricSectorMtbddCompiler::compile(&normalized, exact).unwrap();
        assert_eq!(
            function.classify_assignment(&[]).unwrap(),
            &ParametricSectorMtbddDisposition::Uncovered
        );
        let rebuilt = CoverageDecisionDag::rebuild_rooted(
            function.rooted(),
            CoverageDecisionDagLimits::default(),
        )
        .unwrap();
        assert_eq!(rebuilt.roots().len(), 1);
        assert_eq!(
            rebuilt.dag().retained_stats(),
            function.rooted().retained_stats()
        );

        let mut below = ParametricSectorMtbddLimits::default();
        below.max_classification_visits = 0;
        let function = ParametricSectorMtbddCompiler::compile(&normalized, below).unwrap();
        assert!(matches!(
            function.classify_assignment(&[]),
            Err(ParametricSectorMtbddError::ResourceLimit {
                resource: "MTBDD classification visits",
                requested: 1,
                limit: 0,
            })
        ));
        assert!(matches!(
            function.classify_assignment(&[false]),
            Err(ParametricSectorMtbddError::AssignmentArityMismatch {
                expected: 0,
                actual: 1
            })
        ));
    }

    #[test]
    fn multi_node_classification_budget_has_exact_and_one_below_evidence() {
        let normalized = ir(
            2,
            vec![
                dnf_attempt(
                    0,
                    vec![clause(
                        DirectBadFormulaClause::Conjunction(
                            literal(0, NormalizedBadLiteralPolarity::EqualZero),
                            literal(1, NormalizedBadLiteralPolarity::EqualZero),
                        ),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
                false_attempt(1),
            ],
        );
        let mut exact = ParametricSectorMtbddLimits::default();
        exact.max_classification_visits = 3;
        let function = ParametricSectorMtbddCompiler::compile(&normalized, exact).unwrap();
        assert_eq!(
            function.classify_assignment(&[true, true]).unwrap(),
            &ParametricSectorMtbddDisposition::DescendingRule {
                candidate_ordinal: 1,
            }
        );

        let mut below = ParametricSectorMtbddLimits::default();
        below.max_classification_visits = 2;
        let function = ParametricSectorMtbddCompiler::compile(&normalized, below).unwrap();
        assert!(matches!(
            function.classify_assignment(&[true, true]),
            Err(ParametricSectorMtbddError::ResourceLimit {
                resource: "MTBDD classification visits",
                requested: 3,
                limit: 2,
            })
        ));
    }

    #[test]
    fn production_and_independent_reference_agree_for_all_small_assignments() {
        let normalized = resource_fixture();
        let function = compile(&normalized);
        for mask in 0usize..(1usize << normalized.base_structural_locus_count()) {
            let zero = (0..normalized.base_structural_locus_count())
                .map(|bit| mask & (1 << bit) != 0)
                .collect::<Vec<_>>();
            assert_eq!(
                function.classify_assignment(&zero).unwrap(),
                &reference_disposition_for_assignment(&normalized, &zero).unwrap(),
                "assignment mask {mask:05b}"
            );
        }
    }
}
