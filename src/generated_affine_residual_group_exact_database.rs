//! Sealed LiteRed-style top reduction for one exact affine group.
//!
//! This owner is deliberately narrower than public rule discovery. It binds
//! one exact solve-plan/frame allocation and keeps algebraic unit pivots for
//! the lifetime of that group. For every submitted raw row it inspects only
//! the current hardest physical key. A known hardest key is substituted; the
//! first unknown hardest key is normalized and stored immediately. In
//! particular, known easier keys in that new pivot's tail are not rewritten.
//!
//! Recentring, target matching, `WhenBad`, rule publication, master inference,
//! and adaptive scheduling are intentionally outside this V1 algebraic seam.
//! Exact GMP key-comparison bit-work and `Vec` move-work metering remain the
//! next isolated resource slice; the present limits bound their cardinalities
//! but do not yet charge operand bit length or insertion distance.
//! Coefficient-work limits bound the algebraic staging work. There is
//! intentionally no byte-valued native-temporary claim in V1: the current
//! coefficient ledger exposes no sound pre-Symbolica peak-memory preflight.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

use crate::generated_affine_residual_group_exact_physical_row::{
    GeneratedAffineResidualGroupExactPhysicalRow,
    GeneratedAffineResidualGroupReplayedExactPhysicalRow,
};
use crate::generated_affine_residual_group_physical_key::{
    GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKey,
};
use crate::generated_affine_residual_group_solve_plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanReplayLimits,
};
use crate::parametric_coefficient::insert_parametric_condition;
use crate::parametric_elimination::{
    ParametricCoefficientWorkLedger, ParametricCoefficientWorkLedgerLimits,
    ParametricCoefficientWorkPhase,
};
use crate::{
    GuardOrigin, IntegralFamily, ParametricCoefficient, ParametricCoefficientContext,
    ParametricNonZeroCondition,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_DATABASE_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-database-v1";

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_LOOKUP_REPLACEMENT_ALLOCATION: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
fn fail_next_lookup_replacement_allocation_for_test() {
    FAIL_NEXT_LOOKUP_REPLACEMENT_ALLOCATION.with(|fail| fail.set(true));
}

#[cfg(test)]
fn take_fail_next_lookup_replacement_allocation_for_test() -> bool {
    FAIL_NEXT_LOOKUP_REPLACEMENT_ALLOCATION.with(|fail| fail.replace(false))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactDatabaseLimits {
    pub(crate) coefficient_work: ParametricCoefficientWorkLedgerLimits,
    /// Caller-owned budget for authenticating the retained solve plan before
    /// this database accepts its allocation identity. Keeping this inside the
    /// database limits makes the replay authority persistent rather than
    /// silently substituting a library default at construction time.
    pub(crate) solve_plan_replay: GeneratedAffineResidualGroupSolvePlanReplayLimits,
    pub(crate) max_pivots: usize,
    pub(crate) max_terms_per_row: usize,
    pub(crate) max_guards_per_row: usize,
    pub(crate) max_guard_origins: usize,
    pub(crate) max_reductions_per_row: usize,
    /// Allocation-free borrowed-input census admitted before the database
    /// allocates its ingress term/guard buffers or deep-copies coefficients and
    /// guards. This is a visible Rust-owned staging bound, not a claim about
    /// Symbolica's internal arithmetic workspace.
    pub(crate) max_ingress_retained_bytes: usize,
    /// Pre-commit retained payload admitted for one pivot that is about to
    /// become persistent database state. This deliberately does not claim to
    /// bound the earlier top-reduction scratch peak.
    pub(crate) max_candidate_retained_bytes: usize,
    /// Cumulative charged retained payload of this database owner.
    pub(crate) max_database_retained_bytes: usize,
}

impl Default for GeneratedAffineResidualGroupExactDatabaseLimits {
    fn default() -> Self {
        const LARGE_BYTES: usize = 256 * 1024 * 1024 * 1024;
        Self {
            coefficient_work: ParametricCoefficientWorkLedgerLimits::default(),
            solve_plan_replay: GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
            max_pivots: 16_000_000,
            max_terms_per_row: 16_000_000,
            max_guards_per_row: 16_000_000,
            max_guard_origins: 64_000_000,
            max_reductions_per_row: 16_000_000,
            max_ingress_retained_bytes: LARGE_BYTES,
            max_candidate_retained_bytes: LARGE_BYTES,
            max_database_retained_bytes: 2 * LARGE_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactDatabaseStats {
    retained_database_bytes: usize,
    last_ingress_prospective_retained_bytes: usize,
    last_ingress_observed_retained_bytes: usize,
    peak_ingress_retained_bytes: usize,
    last_candidate_prospective_retained_bytes: usize,
    last_candidate_observed_retained_bytes: usize,
    peak_candidate_retained_bytes: usize,
}

impl GeneratedAffineResidualGroupExactDatabaseStats {
    pub(crate) const fn retained_database_bytes(self) -> usize {
        self.retained_database_bytes
    }

    pub(crate) const fn last_ingress_prospective_retained_bytes(self) -> usize {
        self.last_ingress_prospective_retained_bytes
    }

    pub(crate) const fn last_ingress_observed_retained_bytes(self) -> usize {
        self.last_ingress_observed_retained_bytes
    }

    pub(crate) const fn peak_ingress_retained_bytes(self) -> usize {
        self.peak_ingress_retained_bytes
    }

    pub(crate) const fn last_candidate_prospective_retained_bytes(self) -> usize {
        self.last_candidate_prospective_retained_bytes
    }

    pub(crate) const fn last_candidate_observed_retained_bytes(self) -> usize {
        self.last_candidate_observed_retained_bytes
    }

    pub(crate) const fn peak_candidate_retained_bytes(self) -> usize {
        self.peak_candidate_retained_bytes
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactDatabaseError {
    WrongPlanAllocation,
    WrongFrameAllocation,
    WrongDatabaseEpoch,
    WrongGroup,
    PlanReplay,
    RowReplay,
    PhysicalKey,
    CoefficientWork,
    InvalidTermOrder,
    InvalidUnitPivot,
    SourceOrderOverflow,
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
    },
    SymbolicaPanic,
}

impl GeneratedAffineResidualGroupExactDatabaseError {
    const fn kind(self) -> &'static str {
        match self {
            Self::WrongPlanAllocation => "WrongPlanAllocation",
            Self::WrongFrameAllocation => "WrongFrameAllocation",
            Self::WrongDatabaseEpoch => "WrongDatabaseEpoch",
            Self::WrongGroup => "WrongGroup",
            Self::PlanReplay => "PlanReplay",
            Self::RowReplay => "RowReplay",
            Self::PhysicalKey => "PhysicalKey",
            Self::CoefficientWork => "CoefficientWork",
            Self::InvalidTermOrder => "InvalidTermOrder",
            Self::InvalidUnitPivot => "InvalidUnitPivot",
            Self::SourceOrderOverflow => "SourceOrderOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactDatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactDatabaseError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactDatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine exact-group database {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactDatabaseError {}

#[derive(Clone, PartialEq, Eq)]
struct ExactDatabaseTerm {
    key: GeneratedAffineResidualGroupPhysicalKey,
    coefficient: ParametricCoefficient,
}

/// Allocation-free deep-payload census for one borrowed ingress row.
///
/// The scalar payload is combined first with logical vector lengths and then,
/// immediately after fallible reservation, with the actual retained
/// capacities. Physical-key payload is charged conservatively even though the
/// ingress clone shares its `Arc`s with the authenticated source row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BorrowedIngressRetainedCensus {
    terms: usize,
    guards: usize,
    deep_payload_bytes: usize,
    prospective_retained_bytes: usize,
}

impl BorrowedIngressRetainedCensus {
    fn observed_retained_bytes(
        self,
        term_capacity: usize,
        guard_capacity: usize,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        if term_capacity < self.terms || guard_capacity < self.guards {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "exact-group borrowed ingress buffers",
                },
            );
        }
        ingress_retained_bytes(term_capacity, guard_capacity, self.deep_payload_bytes)
    }
}

impl fmt::Debug for ExactDatabaseTerm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactDatabaseTerm")
            .field("private_key", &"<redacted>")
            .field("private_coefficient", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactReductionStep {
    pivot_ordinal: usize,
    factor: ParametricCoefficient,
}

impl GeneratedAffineResidualGroupExactReductionStep {
    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) const fn factor(&self) -> &ParametricCoefficient {
        &self.factor
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactReductionStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactReductionStep")
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("private_factor", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ExactUnitPivot {
    ordinal: usize,
    source_ordinal: usize,
    terms: Vec<ExactDatabaseTerm>,
    guards: Vec<ParametricNonZeroCondition>,
    reductions: Vec<GeneratedAffineResidualGroupExactReductionStep>,
    normalization_divisor: ParametricCoefficient,
}

impl fmt::Debug for ExactUnitPivot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactUnitPivot")
            .field("ordinal", &self.ordinal)
            .field("source_ordinal", &self.source_ordinal)
            .field("term_count", &self.terms.len())
            .field("guard_count", &self.guards.len())
            .field("reduction_count", &self.reductions.len())
            .field("private_normalization_divisor", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ExactPivotLookupEntry {
    key: GeneratedAffineResidualGroupPhysicalKey,
    pivot_ordinal: usize,
}

/// Borrowed, read-only view of one chronologically retained algebraic pivot.
pub(crate) struct GeneratedAffineResidualGroupExactUnitPivotView<'a> {
    pivot: &'a ExactUnitPivot,
}

impl<'a> GeneratedAffineResidualGroupExactUnitPivotView<'a> {
    pub(crate) const fn ordinal(&self) -> usize {
        self.pivot.ordinal
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.pivot.source_ordinal
    }

    pub(crate) fn key(&self) -> &'a GeneratedAffineResidualGroupPhysicalKey {
        &self
            .pivot
            .terms
            .last()
            .expect("an authenticated unit pivot is nonempty")
            .key
    }

    pub(crate) fn terms(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'a GeneratedAffineResidualGroupPhysicalKey,
            &'a ParametricCoefficient,
        ),
    > + DoubleEndedIterator
    + 'a {
        self.pivot
            .terms
            .iter()
            .map(|term| (&term.key, &term.coefficient))
    }

    pub(crate) fn guards(&self) -> &'a [ParametricNonZeroCondition] {
        &self.pivot.guards
    }

    pub(crate) fn reductions(&self) -> &'a [GeneratedAffineResidualGroupExactReductionStep] {
        &self.pivot.reductions
    }

    /// Exact pre-normalization leader retained for future event replay.
    pub(crate) const fn normalization_divisor(&self) -> &'a ParametricCoefficient {
        &self.pivot.normalization_divisor
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactRowOutcome {
    Dependent {
        source_ordinal: usize,
        reductions: Vec<GeneratedAffineResidualGroupExactReductionStep>,
    },
    NewPivot {
        source_ordinal: usize,
        pivot_ordinal: usize,
    },
}

/// Persistent algebraic database for one exact solve-plan allocation.
pub(crate) struct GeneratedAffineResidualGroupExactDatabase {
    schema: &'static str,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    database_epoch: usize,
    group_ordinal: usize,
    next_source_ordinal: usize,
    pivots: Vec<ExactUnitPivot>,
    lookup: Vec<ExactPivotLookupEntry>,
    limits: GeneratedAffineResidualGroupExactDatabaseLimits,
    stats: GeneratedAffineResidualGroupExactDatabaseStats,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactDatabase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactDatabase")
            .field("schema", &self.schema)
            .field("database_epoch", &self.database_epoch)
            .field("group_ordinal", &self.group_ordinal)
            .field("next_source_ordinal", &self.next_source_ordinal)
            .field("pivot_count", &self.pivots.len())
            .field("stats", &self.stats)
            .field("private_plan", &"<redacted>")
            .field("private_frame", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .field("publishes_rule", &false)
            .field("infers_master", &false)
            .finish()
    }
}

impl GeneratedAffineResidualGroupExactDatabase {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
        limits: GeneratedAffineResidualGroupExactDatabaseLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupExactDatabaseError> {
        catch_unwind(AssertUnwindSafe(|| {
            check_limit(
                "exact-group database retained bytes",
                size_of::<Self>(),
                limits.max_database_retained_bytes,
            )?;
            if !Arc::ptr_eq(plan.physical_frame(), &frame) {
                return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongFrameAllocation);
            }
            plan.replay(
                family,
                context,
                plan.inventory(),
                plan.authority(),
                &frame,
                limits.solve_plan_replay,
            )
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::PlanReplay)?;
            Ok(Self {
                schema: GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_DATABASE_V1_SCHEMA,
                group_ordinal: plan.group_ordinal(),
                plan,
                frame,
                database_epoch,
                next_source_ordinal: 0,
                pivots: Vec::new(),
                lookup: Vec::new(),
                limits,
                stats: GeneratedAffineResidualGroupExactDatabaseStats {
                    retained_database_bytes: size_of::<Self>(),
                    ..GeneratedAffineResidualGroupExactDatabaseStats::default()
                },
            })
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic)?
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn database_epoch(&self) -> usize {
        self.database_epoch
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }

    pub(crate) fn pivot_count(&self) -> usize {
        self.pivots.len()
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactDatabaseStats {
        self.stats
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    pub(crate) fn pivot(
        &self,
        ordinal: usize,
    ) -> Option<GeneratedAffineResidualGroupExactUnitPivotView<'_>> {
        self.pivots
            .get(ordinal)
            .map(|pivot| GeneratedAffineResidualGroupExactUnitPivotView { pivot })
    }

    #[cfg(test)]
    fn lookup_pivot(
        &self,
        key: &GeneratedAffineResidualGroupPhysicalKey,
    ) -> Option<GeneratedAffineResidualGroupExactUnitPivotView<'_>> {
        let position = self
            .lookup
            .binary_search_by(|entry| entry.key.cmp(key))
            .ok()?;
        self.pivot(self.lookup[position].pivot_ordinal)
    }

    fn authenticate_binding(
        &self,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
    ) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
        if !Arc::ptr_eq(&self.plan, plan) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongPlanAllocation);
        }
        if !Arc::ptr_eq(&self.frame, frame) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongFrameAllocation);
        }
        if self.database_epoch != database_epoch {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongDatabaseEpoch);
        }
        Ok(())
    }

    /// Authenticate and consume one raw physical row in submission order.
    pub(crate) fn ingest_replayed_row(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            self.ingest_replayed_row_inner(family, context, plan, frame, database_epoch, source)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic)?
    }

    fn ingest_replayed_row_inner(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_binding(plan, frame, database_epoch)?;
        let next_source_ordinal = self.preflight_next_source_ordinal()?;
        let view = source
            .replay_for_database(family, context, frame)
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::RowReplay)?;
        if view.group_ordinal() != self.group_ordinal {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongGroup);
        }
        self.ingest_view(context, view, next_source_ordinal)
    }

    fn ingest_view(
        &mut self,
        context: &ParametricCoefficientContext,
        view: GeneratedAffineResidualGroupReplayedExactPhysicalRow<'_>,
        next_source_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let mut ledger = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            self.limits.coefficient_work,
        );
        check_limit(
            "terms in one exact top-reduction row",
            view.term_count(),
            self.limits.max_terms_per_row,
        )?;
        let ingress = preflight_borrowed_ingress(
            context,
            &self.frame,
            view.terms(),
            view.term_count(),
            view.guards(),
            self.limits,
        )?;
        check_limit(
            "exact-group borrowed ingress prospective retained bytes",
            ingress.prospective_retained_bytes,
            self.limits.max_ingress_retained_bytes,
        )?;
        let mut terms = try_terms_with_capacity(view.term_count())?;
        let mut guards = try_guards_with_capacity(view.guard_count())?;
        let observed_ingress_retained_bytes =
            ingress.observed_retained_bytes(terms.capacity(), guards.capacity())?;
        check_limit(
            "exact-group borrowed ingress observed retained bytes",
            observed_ingress_retained_bytes,
            self.limits.max_ingress_retained_bytes,
        )?;
        for (key, coefficient) in view.terms() {
            terms.push(ExactDatabaseTerm {
                key: key.clone(),
                coefficient: ledger
                    .try_copy_authenticated(coefficient)
                    .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?,
            });
        }
        guards.extend(view.guards().iter().cloned());
        self.finish_ingest(
            context,
            terms,
            guards,
            ledger,
            next_source_ordinal,
            ingress.prospective_retained_bytes,
            observed_ingress_retained_bytes,
        )
    }

    fn finish_ingest(
        &mut self,
        context: &ParametricCoefficientContext,
        mut terms: Vec<ExactDatabaseTerm>,
        mut guards: Vec<ParametricNonZeroCondition>,
        mut ledger: ParametricCoefficientWorkLedger,
        next_source_ordinal: usize,
        ingress_prospective_retained_bytes: usize,
        ingress_observed_retained_bytes: usize,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let source_ordinal = self.next_source_ordinal;
        debug_assert_eq!(source_ordinal.checked_add(1), Some(next_source_ordinal));
        let mut reductions = Vec::new();

        loop {
            let Some(hardest) = terms.last() else {
                self.stats = self.stats_with_ingress(
                    ingress_prospective_retained_bytes,
                    ingress_observed_retained_bytes,
                );
                self.commit_source_advance(next_source_ordinal);
                return Ok(GeneratedAffineResidualGroupExactRowOutcome::Dependent {
                    source_ordinal,
                    reductions,
                });
            };
            let Some(pivot_ordinal) = self.lookup_ordinal(&hardest.key) else {
                break;
            };
            let requested = reductions.len().checked_add(1).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: "top-reduction steps in one row",
                },
            )?;
            check_limit(
                "top-reduction steps in one row",
                requested,
                self.limits.max_reductions_per_row,
            )?;
            reductions.try_reserve_exact(1).map_err(|_| {
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "top-reduction steps",
                }
            })?;
            let factor = ledger
                .try_copy_authenticated(&hardest.coefficient)
                .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
            let pivot = &self.pivots[pivot_ordinal];
            if pivot.terms.last().map(|term| &term.key) != Some(&hardest.key) {
                return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot);
            }
            terms.pop();
            merge_guards(
                context,
                &mut guards,
                &pivot.guards,
                self.limits.max_guards_per_row,
                self.limits.max_guard_origins,
                self.limits.coefficient_work.arithmetic.max_guard_origins,
            )?;
            for (term_ordinal, pivot_term) in pivot
                .terms
                .iter()
                .take(pivot.terms.len().saturating_sub(1))
                .enumerate()
            {
                let scaled = ledger
                    .try_mul(context, &factor, &pivot_term.coefficient)
                    .and_then(|value| ledger.try_neg(context, &value))
                    .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
                add_sorted_term(
                    context,
                    &mut ledger,
                    &mut terms,
                    pivot_term.key.clone(),
                    scaled,
                    self.limits.max_terms_per_row,
                )?;
                if let Ok(position) = terms.binary_search_by(|term| term.key.cmp(&pivot_term.key)) {
                    let origin =
                        GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                            solve_group_ordinal: self.group_ordinal,
                            database_epoch: self.database_epoch,
                            event_ordinal: source_ordinal,
                            operation_ordinal: reductions.len(),
                            term_ordinal,
                            pivot_normalization: false,
                        };
                    insert_denominator_guard(
                        context,
                        &mut ledger,
                        &mut guards,
                        &terms[position].coefficient,
                        origin,
                        self.limits,
                    )?;
                }
            }
            reductions.push(GeneratedAffineResidualGroupExactReductionStep {
                pivot_ordinal,
                factor,
            });
        }

        let pivot_ordinal = self.pivots.len();
        let requested = pivot_ordinal.checked_add(1).ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "exact group pivots",
            },
        )?;
        check_limit("exact group pivots", requested, self.limits.max_pivots)?;
        let normalization_divisor = normalize_unknown_leader(
            context,
            &mut ledger,
            &mut terms,
            &mut guards,
            self.group_ordinal,
            self.database_epoch,
            source_ordinal,
            self.limits,
        )?;
        let pivot_key = terms
            .last()
            .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot)?
            .key
            .clone();
        let insertion = self
            .lookup
            .binary_search_by(|entry| entry.key.cmp(&pivot_key))
            .map_or_else(|position| position, |position| position);
        if self
            .lookup
            .get(insertion)
            .is_some_and(|entry| entry.key == pivot_key)
        {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot);
        }
        let prospective_retained_bytes =
            pivot_retained_bytes(&terms, &guards, &reductions, &normalization_divisor, false)?;
        check_limit(
            "exact-group pivot prospective retained bytes",
            prospective_retained_bytes,
            self.limits.max_candidate_retained_bytes,
        )?;
        let pivot = ExactUnitPivot {
            ordinal: pivot_ordinal,
            source_ordinal,
            terms,
            guards,
            reductions,
            normalization_divisor,
        };
        let observed_retained_bytes = exact_unit_pivot_retained_bytes(&pivot)?;
        check_limit(
            "exact-group pivot observed retained bytes",
            observed_retained_bytes,
            self.limits.max_candidate_retained_bytes,
        )?;
        let prospective_database_retained_bytes = database_retained_bytes_with_candidate(
            &self.pivots,
            requested,
            requested,
            Some(&pivot),
        )?;
        check_limit(
            "exact-group database retained bytes",
            prospective_database_retained_bytes,
            self.limits.max_database_retained_bytes,
        )?;

        // Allocate both complete replacement buffers before touching either
        // retained index. If the second allocation fails, the first remains a
        // local and is dropped while the database stays byte-for-byte intact.
        let mut committed_pivots = try_pivot_replacement_with_capacity(requested)?;
        #[cfg(test)]
        if take_fail_next_lookup_replacement_allocation_for_test() {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "sorted exact-pivot lookup replacement",
                },
            );
        }
        let mut committed_lookup = try_lookup_replacement_with_capacity(requested)?;
        let retained_database_bytes = database_retained_bytes_with_candidate(
            &self.pivots,
            committed_pivots.capacity(),
            committed_lookup.capacity(),
            Some(&pivot),
        )?;
        check_limit(
            "exact-group database retained bytes",
            retained_database_bytes,
            self.limits.max_database_retained_bytes,
        )?;
        let ingress_stats = self.stats_with_ingress(
            ingress_prospective_retained_bytes,
            ingress_observed_retained_bytes,
        );
        let committed_stats = GeneratedAffineResidualGroupExactDatabaseStats {
            retained_database_bytes,
            last_candidate_prospective_retained_bytes: prospective_retained_bytes,
            last_candidate_observed_retained_bytes: observed_retained_bytes,
            peak_candidate_retained_bytes: ingress_stats
                .peak_candidate_retained_bytes
                .max(observed_retained_bytes),
            ..ingress_stats
        };

        // Every operation from here through assignment is a capacity-admitted
        // move of already constructed values; no user code or Symbolica call
        // remains. `append`, `insert`, and `push` cannot grow these buffers.
        let mut prior_pivots = std::mem::take(&mut self.pivots);
        let mut prior_lookup = std::mem::take(&mut self.lookup);
        committed_pivots.append(&mut prior_pivots);
        committed_lookup.append(&mut prior_lookup);
        committed_lookup.insert(
            insertion,
            ExactPivotLookupEntry {
                key: pivot_key,
                pivot_ordinal,
            },
        );
        committed_pivots.push(pivot);
        self.pivots = committed_pivots;
        self.lookup = committed_lookup;
        self.stats = committed_stats;
        self.commit_source_advance(next_source_ordinal);
        Ok(GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
            source_ordinal,
            pivot_ordinal,
        })
    }

    fn lookup_ordinal(&self, key: &GeneratedAffineResidualGroupPhysicalKey) -> Option<usize> {
        let position = self
            .lookup
            .binary_search_by(|entry| entry.key.cmp(key))
            .ok()?;
        Some(self.lookup[position].pivot_ordinal)
    }

    fn commit_source_advance(&mut self, next_source_ordinal: usize) {
        debug_assert_eq!(
            self.next_source_ordinal.checked_add(1),
            Some(next_source_ordinal)
        );
        self.next_source_ordinal = next_source_ordinal;
    }

    fn stats_with_ingress(
        &self,
        prospective_retained_bytes: usize,
        observed_retained_bytes: usize,
    ) -> GeneratedAffineResidualGroupExactDatabaseStats {
        GeneratedAffineResidualGroupExactDatabaseStats {
            last_ingress_prospective_retained_bytes: prospective_retained_bytes,
            last_ingress_observed_retained_bytes: observed_retained_bytes,
            peak_ingress_retained_bytes: self
                .stats
                .peak_ingress_retained_bytes
                .max(observed_retained_bytes),
            ..self.stats
        }
    }

    fn preflight_next_source_ordinal(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        self.next_source_ordinal
            .checked_add(1)
            .ok_or(GeneratedAffineResidualGroupExactDatabaseError::SourceOrderOverflow)
    }

    #[cfg(test)]
    fn ingest_test_terms(
        &mut self,
        context: &ParametricCoefficientContext,
        terms: Vec<(
            GeneratedAffineResidualGroupPhysicalKey,
            ParametricCoefficient,
        )>,
        guards: Vec<ParametricNonZeroCondition>,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let next_source_ordinal = self.preflight_next_source_ordinal()?;
        check_limit(
            "terms in one exact top-reduction row",
            terms.len(),
            self.limits.max_terms_per_row,
        )?;
        let mut ledger = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            self.limits.coefficient_work,
        );
        let ingress = preflight_borrowed_ingress(
            context,
            &self.frame,
            terms.iter().map(|(key, coefficient)| (key, coefficient)),
            terms.len(),
            &guards,
            self.limits,
        )?;
        check_limit(
            "exact-group borrowed ingress prospective retained bytes",
            ingress.prospective_retained_bytes,
            self.limits.max_ingress_retained_bytes,
        )?;
        let mut retained = try_terms_with_capacity(terms.len())?;
        let mut retained_guards = try_guards_with_capacity(guards.len())?;
        let observed_ingress_retained_bytes =
            ingress.observed_retained_bytes(retained.capacity(), retained_guards.capacity())?;
        check_limit(
            "exact-group borrowed ingress observed retained bytes",
            observed_ingress_retained_bytes,
            self.limits.max_ingress_retained_bytes,
        )?;
        for (key, coefficient) in terms {
            retained.push(ExactDatabaseTerm {
                key,
                coefficient: ledger
                    .try_copy_authenticated(&coefficient)
                    .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?,
            });
        }
        retained_guards.extend(guards.iter().cloned());
        self.finish_ingest(
            context,
            retained,
            retained_guards,
            ledger,
            next_source_ordinal,
            ingress.prospective_retained_bytes,
            observed_ingress_retained_bytes,
        )
    }
}

fn normalize_unknown_leader(
    context: &ParametricCoefficientContext,
    ledger: &mut ParametricCoefficientWorkLedger,
    terms: &mut [ExactDatabaseTerm],
    guards: &mut Vec<ParametricNonZeroCondition>,
    group_ordinal: usize,
    database_epoch: usize,
    source_ordinal: usize,
    limits: GeneratedAffineResidualGroupExactDatabaseLimits,
) -> Result<ParametricCoefficient, GeneratedAffineResidualGroupExactDatabaseError> {
    let divisor = ledger
        .try_copy_authenticated(
            &terms
                .last()
                .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot)?
                .coefficient,
        )
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    let one = ledger
        .try_one(context)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    let pending = ledger
        .try_guarded_division_pending(context, &one, &divisor)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    let inverse = ledger
        .try_finish_guarded_division(context, pending)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    merge_guards(
        context,
        guards,
        &inverse.nonzero,
        limits.max_guards_per_row,
        limits.max_guard_origins,
        limits.coefficient_work.arithmetic.max_guard_origins,
    )?;
    let leader_ordinal = terms
        .len()
        .checked_sub(1)
        .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot)?;
    for (term_ordinal, term) in terms.iter_mut().enumerate() {
        term.coefficient = ledger
            .try_mul(context, &term.coefficient, &inverse.value)
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
        insert_denominator_guard(
            context,
            ledger,
            guards,
            &term.coefficient,
            GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                solve_group_ordinal: group_ordinal,
                database_epoch,
                event_ordinal: source_ordinal,
                operation_ordinal: 0,
                term_ordinal,
                pivot_normalization: term_ordinal == leader_ordinal,
            },
            limits,
        )?;
    }
    if terms.last().map(|term| &term.coefficient) != Some(&one) {
        return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot);
    }
    let pivot = &terms
        .last()
        .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot)?
        .key;
    if terms[..terms.len().saturating_sub(1)]
        .iter()
        .any(|term| term.key >= *pivot)
    {
        return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot);
    }
    Ok(divisor)
}

fn add_sorted_term(
    context: &ParametricCoefficientContext,
    ledger: &mut ParametricCoefficientWorkLedger,
    terms: &mut Vec<ExactDatabaseTerm>,
    key: GeneratedAffineResidualGroupPhysicalKey,
    coefficient: ParametricCoefficient,
    max_terms: usize,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    match terms.binary_search_by(|term| term.key.cmp(&key)) {
        Ok(position) => {
            let sum = ledger
                .try_add(context, &terms[position].coefficient, &coefficient)
                .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
            if sum.is_zero() {
                terms.remove(position);
            } else {
                terms[position].coefficient = sum;
            }
        }
        Err(position) => {
            let requested = terms.len().checked_add(1).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: "terms in one exact top-reduction row",
                },
            )?;
            check_limit("terms in one exact top-reduction row", requested, max_terms)?;
            terms.try_reserve_exact(1).map_err(|_| {
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "exact top-reduction row terms",
                }
            })?;
            terms.insert(position, ExactDatabaseTerm { key, coefficient });
        }
    }
    Ok(())
}

fn preflight_borrowed_ingress<'a>(
    context: &ParametricCoefficientContext,
    frame: &GeneratedAffineResidualGroupPhysicalFrame,
    terms: impl IntoIterator<
        Item = (
            &'a GeneratedAffineResidualGroupPhysicalKey,
            &'a ParametricCoefficient,
        ),
    >,
    expected_terms: usize,
    guards: &[ParametricNonZeroCondition],
    limits: GeneratedAffineResidualGroupExactDatabaseLimits,
) -> Result<BorrowedIngressRetainedCensus, GeneratedAffineResidualGroupExactDatabaseError> {
    check_limit(
        "terms in one exact top-reduction row",
        expected_terms,
        limits.max_terms_per_row,
    )?;
    check_limit(
        "guards in one exact top-reduction row",
        guards.len(),
        limits.max_guards_per_row,
    )?;

    let resource = "exact-group borrowed ingress retained bytes";
    let mut deep_payload_bytes = 0usize;
    let mut observed_terms = 0usize;
    let mut previous_key: Option<&GeneratedAffineResidualGroupPhysicalKey> = None;
    for (key, coefficient) in terms {
        observed_terms = checked_add(resource, observed_terms, 1)?;
        if coefficient.is_zero() || previous_key.is_some_and(|previous| previous >= key) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidTermOrder);
        }
        context
            .validate_with_limits(
                coefficient,
                limits.coefficient_work.arithmetic.exact_algebra,
            )
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
        frame
            .replay_key(key)
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::PhysicalKey)?;
        deep_payload_bytes = checked_add(resource, deep_payload_bytes, key.retained_bytes())?;
        deep_payload_bytes = checked_add(
            resource,
            deep_payload_bytes,
            coefficient.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource },
            )?,
        )?;
        previous_key = Some(key);
    }
    if observed_terms != expected_terms || observed_terms == 0 {
        return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidTermOrder);
    }

    let mut aggregate_origins = 0usize;
    for guard in guards {
        if guard.origins().is_empty() {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork);
        }
        context
            .validate_polynomial_with_limits(
                guard.polynomial(),
                limits.coefficient_work.arithmetic.exact_algebra,
            )
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
        check_limit(
            "guard origins in one exact top-reduction condition",
            guard.origins().len(),
            limits.coefficient_work.arithmetic.max_guard_origins,
        )?;
        aggregate_origins = checked_add(
            "guard origins in one exact top-reduction row",
            aggregate_origins,
            guard.origins().len(),
        )?;
        deep_payload_bytes = checked_add(
            resource,
            deep_payload_bytes,
            guard.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource },
            )?,
        )?;
    }
    check_limit(
        "guard origins in one exact top-reduction row",
        aggregate_origins,
        limits.max_guard_origins,
    )?;
    let prospective_retained_bytes =
        ingress_retained_bytes(expected_terms, guards.len(), deep_payload_bytes)?;
    Ok(BorrowedIngressRetainedCensus {
        terms: expected_terms,
        guards: guards.len(),
        deep_payload_bytes,
        prospective_retained_bytes,
    })
}

fn ingress_retained_bytes(
    term_capacity: usize,
    guard_capacity: usize,
    deep_payload_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact-group borrowed ingress retained bytes";
    checked_sum(
        RESOURCE,
        [
            size_of::<Vec<ExactDatabaseTerm>>(),
            size_of::<Vec<ParametricNonZeroCondition>>(),
            checked_mul(RESOURCE, term_capacity, size_of::<ExactDatabaseTerm>())?,
            checked_mul(
                RESOURCE,
                guard_capacity,
                size_of::<ParametricNonZeroCondition>(),
            )?,
            deep_payload_bytes,
        ],
    )
}

fn copy_guards(
    context: &ParametricCoefficientContext,
    source: &[ParametricNonZeroCondition],
    max_guards: usize,
    max_aggregate_origins: usize,
    max_origins_per_condition: usize,
) -> Result<Vec<ParametricNonZeroCondition>, GeneratedAffineResidualGroupExactDatabaseError> {
    check_limit(
        "guards in one exact top-reduction row",
        source.len(),
        max_guards,
    )?;
    let origins = source.iter().try_fold(0usize, |total, guard| {
        if !context.contains_nonzero_condition(guard) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork);
        }
        check_limit(
            "guard origins in one exact top-reduction condition",
            guard.origins().len(),
            max_origins_per_condition,
        )?;
        total.checked_add(guard.origins().len()).ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "guard origins in one exact top-reduction row",
            },
        )
    })?;
    check_limit(
        "guard origins in one exact top-reduction row",
        origins,
        max_aggregate_origins,
    )?;
    // The exact physical-row compiler already authenticated and bounded these
    // payloads. V1 reserves the outer vector fallibly before cloning; a fully
    // fallible inner condition clone remains a later safety hardening seam.
    let mut guards = Vec::new();
    guards.try_reserve_exact(source.len()).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "exact top-reduction row guards",
        }
    })?;
    guards.extend(source.iter().cloned());
    Ok(guards)
}

fn merge_guards(
    context: &ParametricCoefficientContext,
    target: &mut Vec<ParametricNonZeroCondition>,
    source: &[ParametricNonZeroCondition],
    max_guards: usize,
    max_aggregate_origins: usize,
    max_origins_per_condition: usize,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    let mut trial = copy_guards(
        context,
        target,
        max_guards,
        max_aggregate_origins,
        max_origins_per_condition,
    )?;
    trial.try_reserve_exact(source.len()).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "exact top-reduction row guards",
        }
    })?;
    for guard in source {
        if !context.contains_nonzero_condition(guard) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork);
        }
        insert_parametric_condition(&mut trial, guard.clone(), max_origins_per_condition)
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
        check_guard_occurrence_limits(
            &trial,
            max_guards,
            max_aggregate_origins,
            max_origins_per_condition,
        )?;
    }
    *target = trial;
    Ok(())
}

fn insert_denominator_guard(
    context: &ParametricCoefficientContext,
    ledger: &mut ParametricCoefficientWorkLedger,
    guards: &mut Vec<ParametricNonZeroCondition>,
    coefficient: &ParametricCoefficient,
    origin: GuardOrigin,
    limits: GeneratedAffineResidualGroupExactDatabaseLimits,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    let per_condition = limits.coefficient_work.arithmetic.max_guard_origins;
    let mut trial = copy_guards(
        context,
        guards,
        limits.max_guards_per_row,
        limits.max_guard_origins,
        per_condition,
    )?;
    ledger
        .try_insert_denominator_guard(context, &mut trial, coefficient, origin)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    check_guard_occurrence_limits(
        &trial,
        limits.max_guards_per_row,
        limits.max_guard_origins,
        per_condition,
    )?;
    *guards = trial;
    Ok(())
}

fn check_guard_occurrence_limits(
    guards: &[ParametricNonZeroCondition],
    max_guards: usize,
    max_aggregate_origins: usize,
    max_origins_per_condition: usize,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    check_limit(
        "guards in one exact top-reduction row",
        guards.len(),
        max_guards,
    )?;
    let aggregate = guards.iter().try_fold(0usize, |total, guard| {
        check_limit(
            "guard origins in one exact top-reduction condition",
            guard.origins().len(),
            max_origins_per_condition,
        )?;
        total.checked_add(guard.origins().len()).ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "guard origins in one exact top-reduction row",
            },
        )
    })?;
    check_limit(
        "guard origins in one exact top-reduction row",
        aggregate,
        max_aggregate_origins,
    )
}

fn exact_unit_pivot_retained_bytes(
    pivot: &ExactUnitPivot,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    pivot_retained_bytes(
        &pivot.terms,
        &pivot.guards,
        &pivot.reductions,
        &pivot.normalization_divisor,
        true,
    )
}

fn pivot_deep_retained_bytes(
    pivot: &ExactUnitPivot,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    let inline = checked_add(
        "exact-group pivot retained bytes",
        size_of::<ExactUnitPivot>(),
        size_of::<ExactPivotLookupEntry>(),
    )?;
    exact_unit_pivot_retained_bytes(pivot)?
        .checked_sub(inline)
        .ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "exact-group pivot retained bytes",
            },
        )
}

/// Complete persistent database ownership under explicit outer-vector
/// capacities. Deep lookup-key payload is excluded because each lookup key is
/// a shallow clone of the leader key already charged by its pivot terms.
fn database_retained_bytes_with_candidate(
    pivots: &[ExactUnitPivot],
    pivot_capacity: usize,
    lookup_capacity: usize,
    candidate: Option<&ExactUnitPivot>,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact-group database retained bytes";
    let required = checked_add(RESOURCE, pivots.len(), usize::from(candidate.is_some()))?;
    if pivot_capacity < required || lookup_capacity < required {
        return Err(
            GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                resource: "exact-group database replacement vectors",
            },
        );
    }
    let mut bytes = checked_sum(
        RESOURCE,
        [
            size_of::<GeneratedAffineResidualGroupExactDatabase>(),
            checked_mul(RESOURCE, pivot_capacity, size_of::<ExactUnitPivot>())?,
            checked_mul(
                RESOURCE,
                lookup_capacity,
                size_of::<ExactPivotLookupEntry>(),
            )?,
        ],
    )?;
    for pivot in pivots.iter().chain(candidate) {
        bytes = checked_add(RESOURCE, bytes, pivot_deep_retained_bytes(pivot)?)?;
    }
    Ok(bytes)
}

/// Conservative charged ownership of one pivot and its sorted lookup entry.
///
/// The lookup key is a shallow `Arc` clone of the pivot leader. Its inline
/// entry is charged here, while the shared deep key payload is charged exactly
/// once through `terms`. `observed_capacity` selects actual staged vector
/// capacities; the prospective pass uses exact logical lengths.
fn pivot_retained_bytes(
    terms: &Vec<ExactDatabaseTerm>,
    guards: &Vec<ParametricNonZeroCondition>,
    reductions: &Vec<GeneratedAffineResidualGroupExactReductionStep>,
    normalization_divisor: &ParametricCoefficient,
    observed_capacity: bool,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact-group pivot retained bytes";
    let term_slots = if observed_capacity {
        terms.capacity()
    } else {
        terms.len()
    };
    let guard_slots = if observed_capacity {
        guards.capacity()
    } else {
        guards.len()
    };
    let reduction_slots = if observed_capacity {
        reductions.capacity()
    } else {
        reductions.len()
    };
    let mut bytes = checked_sum(
        RESOURCE,
        [
            size_of::<ExactUnitPivot>(),
            size_of::<ExactPivotLookupEntry>(),
            checked_mul(RESOURCE, term_slots, size_of::<ExactDatabaseTerm>())?,
            checked_mul(
                RESOURCE,
                guard_slots,
                size_of::<ParametricNonZeroCondition>(),
            )?,
            checked_mul(
                RESOURCE,
                reduction_slots,
                size_of::<GeneratedAffineResidualGroupExactReductionStep>(),
            )?,
            normalization_divisor.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        ],
    )?;
    for term in terms {
        bytes = checked_add(RESOURCE, bytes, term.key.retained_bytes())?;
        bytes = checked_add(
            RESOURCE,
            bytes,
            term.coefficient.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    for guard in guards {
        bytes = checked_add(
            RESOURCE,
            bytes,
            guard.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    for reduction in reductions {
        bytes = checked_add(
            RESOURCE,
            bytes,
            reduction.factor.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    Ok(bytes)
}

fn try_terms_with_capacity(
    capacity: usize,
) -> Result<Vec<ExactDatabaseTerm>, GeneratedAffineResidualGroupExactDatabaseError> {
    let mut terms = Vec::new();
    terms.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "exact top-reduction row terms",
        }
    })?;
    Ok(terms)
}

fn try_pivot_replacement_with_capacity(
    capacity: usize,
) -> Result<Vec<ExactUnitPivot>, GeneratedAffineResidualGroupExactDatabaseError> {
    let mut pivots = Vec::new();
    pivots.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "chronological exact-pivot replacement",
        }
    })?;
    Ok(pivots)
}

fn try_lookup_replacement_with_capacity(
    capacity: usize,
) -> Result<Vec<ExactPivotLookupEntry>, GeneratedAffineResidualGroupExactDatabaseError> {
    let mut lookup = Vec::new();
    lookup.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "sorted exact-pivot lookup replacement",
        }
    })?;
    Ok(lookup)
}

fn try_guards_with_capacity(
    capacity: usize,
) -> Result<Vec<ParametricNonZeroCondition>, GeneratedAffineResidualGroupExactDatabaseError> {
    let mut guards = Vec::new();
    guards.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "exact top-reduction row guards",
        }
    })?;
    Ok(guards)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource })
}

fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    values
        .into_iter()
        .try_fold(0usize, |sum, value| checked_add(resource, sum, value))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use symbolica::prelude::Integer;

    use super::*;
    use crate::generated_affine_parametric_ordering::{
        GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingLimits,
    };
    use crate::generated_affine_prepare_point_schedule::{
        GeneratedAffinePreparePointScheduleCertificate, GeneratedAffinePreparePointScheduleLimits,
    };
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_inventory::{
        GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_case_reelimination::{
        GeneratedAffineResidualCaseReeliminationCompilation,
        GeneratedAffineResidualCaseReeliminationCompiler,
        GeneratedAffineResidualCaseReeliminationLimits,
    };
    use crate::generated_affine_residual_group_exact_physical_row::{
        GeneratedAffineResidualGroupExactPhysicalRowCompiler,
        GeneratedAffineResidualGroupExactPhysicalRowLimits,
    };
    use crate::generated_affine_residual_group_physical_key::GeneratedAffineResidualGroupPhysicalKeyLimits;
    use crate::generated_affine_residual_group_solve_plan::GeneratedAffineResidualGroupSolvePlanLimits;
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        SectorMask,
    };

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn database_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualGroupSolvePlan>,
        Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        GeneratedAffineResidualGroupExactDatabase,
        Vec<GeneratedAffineResidualGroupPhysicalKey>,
    ) {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("011").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                GeneratedAffineResidualSourceAuthority::initial_global(queue),
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let group_ordinal = (0..inventory.group_count())
            .max_by_key(|&ordinal| {
                inventory
                    .authenticated_group_view(&context, ordinal)
                    .unwrap()
                    .case_ordinals()
                    .len()
            })
            .unwrap();
        let group = inventory
            .authenticated_group_view(&context, group_ordinal)
            .unwrap();
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                group.anchor_case_ordinal(),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new(
                &family,
                &context,
                inventory,
                authority,
                Arc::clone(&frame),
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        let database = GeneratedAffineResidualGroupExactDatabase::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            Arc::clone(&frame),
            29,
            GeneratedAffineResidualGroupExactDatabaseLimits::default(),
        )
        .unwrap();

        let mut keys = Vec::new();
        for first in -4..=4 {
            let mut values = vec![Integer::from(0); frame.arity()];
            values[0] = Integer::from(first);
            keys.push(
                frame
                    .test_key_for_borrowed_physical_values(&values)
                    .unwrap(),
            );
        }
        keys.sort();
        keys.dedup();
        assert!(keys.len() >= 2);
        (family, context, plan, frame, database, keys)
    }

    fn production_exact_physical_row(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> Arc<GeneratedAffineResidualGroupExactPhysicalRow> {
        for &case_ordinal in frame.case_ordinals() {
            let authority = Arc::new(
                GeneratedAffineResidualCaseAuthority::try_new(
                    family,
                    context,
                    Arc::clone(plan.inventory()),
                    case_ordinal,
                    GeneratedAffineResidualCaseAuthorityLimits::default(),
                )
                .unwrap(),
            );
            let premises = match compile_generated_affine_residual_case_premises(
                family,
                context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(value) => Arc::new(value),
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    continue;
                }
            };
            let ordering = Arc::new(
                GeneratedAffineParametricOrderingCertificate::try_new(
                    family,
                    context,
                    Arc::clone(&authority),
                    GeneratedAffineParametricOrderingLimits::default(),
                )
                .unwrap(),
            );
            let schedule = Arc::new(
                GeneratedAffinePreparePointScheduleCertificate::compile(
                    family,
                    context,
                    Arc::clone(&ordering),
                    &authority,
                    0,
                    GeneratedAffinePreparePointScheduleLimits::default(),
                )
                .unwrap(),
            );
            let compilation = GeneratedAffineResidualCaseReeliminationCompiler::compile(
                family,
                context,
                authority,
                premises,
                ordering,
                schedule,
                GeneratedAffineResidualCaseReeliminationLimits::default(),
            )
            .unwrap();
            let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) =
                compilation
            else {
                continue;
            };
            let certificate = Arc::new(certificate);
            let Some(witness_ordinal) = certificate
                .witnesses()
                .iter()
                .position(|witness| witness.outcome().is_retained())
            else {
                continue;
            };
            let retained_row_ordinal = certificate.witnesses()[..witness_ordinal]
                .iter()
                .filter(|witness| witness.outcome().is_retained())
                .count();
            return Arc::new(
                GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
                    family,
                    context,
                    certificate,
                    retained_row_ordinal,
                    witness_ordinal,
                    Arc::clone(frame),
                    GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
                )
                .unwrap(),
            );
        }
        panic!("the generic affine-group fixture produced no authenticated physical row")
    }

    fn indexed_guard(
        context: &ParametricCoefficientContext,
        offset: i64,
    ) -> ParametricNonZeroCondition {
        let polynomial = context
            .numerator_condition(
                &context
                    .add(&context.index(0).unwrap(), &context.integer(offset))
                    .unwrap(),
            )
            .unwrap();
        context
            .nonzero_condition(polynomial, GuardOrigin::ExplicitRelationCondition)
            .unwrap()
    }

    fn exact_solve_plan_replay_limits(
        plan: &GeneratedAffineResidualGroupSolvePlan,
    ) -> GeneratedAffineResidualGroupSolvePlanReplayLimits {
        let stats = plan.stats();
        GeneratedAffineResidualGroupSolvePlanReplayLimits {
            max_parent_allocation_comparisons: stats.retained_parent_references(),
            max_combined_owner_bytes: stats.replay_combined_owner_bytes(),
            max_payload_comparison_units: stats.payload_comparison_units(),
            max_payload_comparison_bytes: stats.payload_comparison_bytes(),
        }
    }

    #[test]
    fn constructor_uses_caller_owned_solve_plan_replay_limits_exactly() {
        let (family, context, plan, frame, database, _keys) =
            database_fixture("exact-db-caller-owned-plan-replay");
        let exact = exact_solve_plan_replay_limits(&plan);
        assert!(exact.max_parent_allocation_comparisons > 0);
        assert!(exact.max_combined_owner_bytes > 0);
        assert!(exact.max_payload_comparison_units > 0);
        assert!(exact.max_payload_comparison_bytes > 0);

        let baseline_database = (
            database.next_source_ordinal,
            database.pivots.len(),
            database.pivots.capacity(),
            database.lookup.len(),
            database.lookup.capacity(),
            database.stats(),
            database.limits,
        );
        let baseline_parent_counts = (
            Arc::strong_count(&plan),
            Arc::strong_count(&frame),
            Arc::strong_count(plan.inventory()),
            Arc::strong_count(plan.authority()),
        );

        let mut limits = GeneratedAffineResidualGroupExactDatabaseLimits::default();
        limits.solve_plan_replay = exact;
        let admitted = GeneratedAffineResidualGroupExactDatabase::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            Arc::clone(&frame),
            31,
            limits,
        )
        .unwrap();
        assert_eq!(admitted.limits.solve_plan_replay, exact);
        assert_eq!(admitted.database_epoch(), 31);
        drop(admitted);
        assert_eq!(
            (
                Arc::strong_count(&plan),
                Arc::strong_count(&frame),
                Arc::strong_count(plan.inventory()),
                Arc::strong_count(plan.authority()),
            ),
            baseline_parent_counts
        );

        let one_below = [
            GeneratedAffineResidualGroupSolvePlanReplayLimits {
                max_parent_allocation_comparisons: exact.max_parent_allocation_comparisons - 1,
                ..exact
            },
            GeneratedAffineResidualGroupSolvePlanReplayLimits {
                max_combined_owner_bytes: exact.max_combined_owner_bytes - 1,
                ..exact
            },
            GeneratedAffineResidualGroupSolvePlanReplayLimits {
                max_payload_comparison_units: exact.max_payload_comparison_units - 1,
                ..exact
            },
            GeneratedAffineResidualGroupSolvePlanReplayLimits {
                max_payload_comparison_bytes: exact.max_payload_comparison_bytes - 1,
                ..exact
            },
        ];
        for rejected in one_below {
            let mut limits = GeneratedAffineResidualGroupExactDatabaseLimits::default();
            limits.solve_plan_replay = rejected;
            assert!(matches!(
                GeneratedAffineResidualGroupExactDatabase::try_new(
                    &family,
                    &context,
                    Arc::clone(&plan),
                    Arc::clone(&frame),
                    31,
                    limits,
                ),
                Err(GeneratedAffineResidualGroupExactDatabaseError::PlanReplay)
            ));
            assert_eq!(
                (
                    Arc::strong_count(&plan),
                    Arc::strong_count(&frame),
                    Arc::strong_count(plan.inventory()),
                    Arc::strong_count(plan.authority()),
                ),
                baseline_parent_counts,
                "failed construction must release every temporary parent reference"
            );
            assert_eq!(
                (
                    database.next_source_ordinal,
                    database.pivots.len(),
                    database.pivots.capacity(),
                    database.lookup.len(),
                    database.lookup.capacity(),
                    database.stats(),
                    database.limits,
                ),
                baseline_database,
                "a failed sibling construction must not mutate retained database state"
            );
        }
    }

    #[test]
    fn production_replayed_row_authenticates_before_exact_ingress() {
        let (family, context, plan, frame, mut database, _keys) =
            database_fixture("exact-db-production-row-ingress");
        let source = production_exact_physical_row(&family, &context, &plan, &frame);
        assert_eq!(source.group_ordinal(), database.group_ordinal());
        let baseline = (
            database.next_source_ordinal,
            database.pivots.len(),
            database.pivots.capacity(),
            database.lookup.len(),
            database.lookup.capacity(),
            database.stats(),
        );

        let foreign_plan = Arc::new(plan.as_ref().clone());
        assert!(matches!(
            database.ingest_replayed_row(
                &family,
                &context,
                &foreign_plan,
                &frame,
                database.database_epoch(),
                &source,
            ),
            Err(GeneratedAffineResidualGroupExactDatabaseError::WrongPlanAllocation)
        ));
        assert_eq!(
            (
                database.next_source_ordinal,
                database.pivots.len(),
                database.pivots.capacity(),
                database.lookup.len(),
                database.lookup.capacity(),
                database.stats(),
            ),
            baseline
        );

        let foreign_frame = Arc::new(frame.as_ref().clone());
        assert!(matches!(
            database.ingest_replayed_row(
                &family,
                &context,
                &plan,
                &foreign_frame,
                database.database_epoch(),
                &source,
            ),
            Err(GeneratedAffineResidualGroupExactDatabaseError::WrongFrameAllocation)
        ));
        assert_eq!(
            (
                database.next_source_ordinal,
                database.pivots.len(),
                database.pivots.capacity(),
                database.lookup.len(),
                database.lookup.capacity(),
                database.stats(),
            ),
            baseline
        );

        assert!(matches!(
            database.ingest_replayed_row(
                &family,
                &context,
                &plan,
                &frame,
                database.database_epoch() + 1,
                &source,
            ),
            Err(GeneratedAffineResidualGroupExactDatabaseError::WrongDatabaseEpoch)
        ));
        assert_eq!(
            (
                database.next_source_ordinal,
                database.pivots.len(),
                database.pivots.capacity(),
                database.lookup.len(),
                database.lookup.capacity(),
                database.stats(),
            ),
            baseline
        );

        let replayed = source
            .replay_for_database(&family, &context, &frame)
            .unwrap();
        let source_term_count = replayed.term_count();
        let source_guard_count = replayed.guard_count();
        let (source_leader, source_divisor) = replayed.terms().next_back().unwrap();
        let outcome = database
            .ingest_replayed_row(
                &family,
                &context,
                &plan,
                &frame,
                database.database_epoch(),
                &source,
            )
            .unwrap();
        assert_eq!(
            outcome,
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0,
            }
        );
        let pivot = database.pivot(0).unwrap();
        assert_eq!(pivot.key(), source_leader);
        assert_eq!(pivot.normalization_divisor(), source_divisor);
        assert_eq!(pivot.terms().len(), source_term_count);
        assert!(pivot.guards().len() >= source_guard_count);
        for source_guard in replayed.guards() {
            assert!(pivot.guards().contains(source_guard));
        }
        assert_eq!(database.next_source_ordinal, 1);
    }

    #[test]
    fn unknown_hardest_b_stops_before_known_easier_a() {
        let (_family, context, _plan, _frame, mut database, keys) =
            database_fixture("exact-db-top-reduced-tail");
        let a = keys[0].clone();
        let b = keys[1].clone();
        assert!(a < b);
        let a_guard = indexed_guard(&context, 1);

        assert!(matches!(
            database
                .ingest_test_terms(
                    &context,
                    vec![(a.clone(), context.one())],
                    vec![a_guard.clone()],
                )
                .unwrap(),
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                pivot_ordinal: 0,
                ..
            }
        ));
        let old_a = database
            .pivot(0)
            .unwrap()
            .terms()
            .map(|(key, coefficient)| (key.clone(), coefficient.clone()))
            .collect::<Vec<_>>();
        assert_eq!(database.pivot(0).unwrap().guards(), &[a_guard.clone()]);

        assert!(matches!(
            database
                .ingest_test_terms(
                    &context,
                    vec![(a.clone(), context.integer(3)), (b.clone(), context.one()),],
                    Vec::new(),
                )
                .unwrap(),
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                pivot_ordinal: 1,
                ..
            }
        ));

        let pivot_b = database.pivot(1).unwrap();
        assert!(pivot_b.reductions().is_empty());
        let terms = pivot_b.terms().collect::<Vec<_>>();
        assert_eq!(terms.len(), 2);
        assert_eq!(terms[0].0, &a);
        assert_eq!(terms[0].1, &context.integer(3));
        assert_eq!(terms[1].0, &b);
        assert_eq!(terms[1].1, &context.one());
        assert!(
            pivot_b.guards().is_empty(),
            "A guards must not enter B's tail"
        );
        assert_eq!(
            database
                .pivot(0)
                .unwrap()
                .terms()
                .map(|(key, coefficient)| (key.clone(), coefficient.clone()))
                .collect::<Vec<_>>(),
            old_a,
            "new pivots must never rewrite chronological old pivots"
        );
        assert_eq!(database.lookup_pivot(&a).unwrap().ordinal(), 0);
        assert_eq!(database.lookup_pivot(&b).unwrap().ordinal(), 1);
        assert!(!database.publishes_rule());
        assert!(!database.infers_master());
    }

    #[test]
    fn known_hardest_chain_recollects_until_dependent() {
        let (_family, context, _plan, _frame, mut database, keys) =
            database_fixture("exact-db-known-hardest-chain");
        let a = keys[0].clone();
        let b = keys[1].clone();
        database
            .ingest_test_terms(&context, vec![(a.clone(), context.one())], Vec::new())
            .unwrap();
        database
            .ingest_test_terms(
                &context,
                vec![(a.clone(), context.integer(3)), (b.clone(), context.one())],
                Vec::new(),
            )
            .unwrap();

        let outcome = database
            .ingest_test_terms(
                &context,
                vec![(a, context.integer(2)), (b, context.one())],
                Vec::new(),
            )
            .unwrap();
        let GeneratedAffineResidualGroupExactRowOutcome::Dependent { reductions, .. } = outcome
        else {
            panic!("known B then known A must eliminate the complete row")
        };
        assert_eq!(
            reductions
                .iter()
                .map(GeneratedAffineResidualGroupExactReductionStep::pivot_ordinal)
                .collect::<Vec<_>>(),
            [1, 0]
        );
        assert_eq!(reductions[0].factor(), &context.one());
        assert_eq!(reductions[1].factor(), &context.integer(-1));
        assert_eq!(database.pivot_count(), 2);
    }

    #[test]
    fn actual_hardest_substitution_imports_both_row_and_pivot_guards() {
        let (_family, context, _plan, _frame, mut database, keys) =
            database_fixture("exact-db-positive-guard-import");
        let lower = keys[0].clone();
        let higher = keys[1].clone();
        let pivot_guard = indexed_guard(&context, 1);
        let row_guard = indexed_guard(&context, 2);

        database
            .ingest_test_terms(
                &context,
                vec![
                    (lower.clone(), context.integer(2)),
                    (higher.clone(), context.one()),
                ],
                vec![pivot_guard.clone()],
            )
            .unwrap();
        let outcome = database
            .ingest_test_terms(
                &context,
                vec![(higher, context.one())],
                vec![row_guard.clone()],
            )
            .unwrap();
        let GeneratedAffineResidualGroupExactRowOutcome::NewPivot { pivot_ordinal, .. } = outcome
        else {
            panic!("the known harder key must expose the unknown lower key")
        };
        let pivot = database.pivot(pivot_ordinal).unwrap();
        assert_eq!(pivot.key(), &lower);
        assert_eq!(pivot.reductions().len(), 1);
        assert_eq!(pivot.reductions()[0].pivot_ordinal(), 0);
        assert_eq!(pivot.reductions()[0].factor(), &context.one());
        assert_eq!(pivot.guards().len(), 2);
        assert!(pivot.guards().contains(&pivot_guard));
        assert!(pivot.guards().contains(&row_guard));
    }

    #[test]
    fn aggregate_guard_origin_limit_is_exact_and_one_below_rolls_back() {
        let (_family, context, _plan, _frame, mut exact, keys) =
            database_fixture("exact-db-aggregate-guard-exact");
        exact.limits.max_guard_origins = 2;
        exact.limits.coefficient_work.arithmetic.max_guard_origins = 1;
        let lower = keys[0].clone();
        let higher = keys[1].clone();
        let denominator = context
            .add(&context.index(0).unwrap(), &context.one())
            .unwrap();
        let reciprocal = context.checked_div(&context.one(), &denominator).unwrap();
        let row_guard = indexed_guard(&context, 2);
        let outcome = exact
            .ingest_test_terms(
                &context,
                vec![
                    (lower.clone(), reciprocal.clone()),
                    (higher.clone(), context.one()),
                ],
                vec![row_guard.clone()],
            )
            .unwrap();
        let GeneratedAffineResidualGroupExactRowOutcome::NewPivot { pivot_ordinal, .. } = outcome
        else {
            panic!("the exact aggregate-origin ceiling must admit the pivot")
        };
        let pivot = exact.pivot(pivot_ordinal).unwrap();
        assert_eq!(pivot.guards().len(), 2);
        assert_eq!(
            pivot
                .guards()
                .iter()
                .map(|guard| guard.origins().len())
                .sum::<usize>(),
            2
        );
        assert!(pivot.guards().contains(&row_guard));
        assert!(pivot.guards().iter().any(|guard| {
            guard.origins().iter().any(|origin| {
                matches!(
                    origin,
                    GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                        pivot_normalization: false,
                        ..
                    }
                )
            })
        }));

        let (_family, context, _plan, _frame, mut one_below, keys) =
            database_fixture("exact-db-aggregate-guard-one-below");
        one_below.limits.max_guard_origins = 1;
        one_below
            .limits
            .coefficient_work
            .arithmetic
            .max_guard_origins = 1;
        let denominator = context
            .add(&context.index(0).unwrap(), &context.one())
            .unwrap();
        let reciprocal = context.checked_div(&context.one(), &denominator).unwrap();
        let before_pivots = one_below.pivots.clone();
        let before_lookup = one_below.lookup.clone();
        let before_source = one_below.next_source_ordinal;
        assert!(matches!(
            one_below.ingest_test_terms(
                &context,
                vec![
                    (keys[0].clone(), reciprocal),
                    (keys[1].clone(), context.one()),
                ],
                vec![indexed_guard(&context, 2)],
            ),
            Err(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceLimit {
                    resource: "guard origins in one exact top-reduction row",
                    requested: 2,
                    limit: 1,
                }
            )
        ));
        assert_eq!(one_below.pivots, before_pivots);
        assert!(one_below.lookup == before_lookup);
        assert_eq!(one_below.next_source_ordinal, before_source);
    }

    #[test]
    fn source_ordinal_overflow_preflight_leaves_database_unchanged() {
        let (_family, context, _plan, _frame, mut database, keys) =
            database_fixture("exact-db-source-overflow");
        database.next_source_ordinal = usize::MAX;
        let before_pivots = database.pivots.clone();
        let before_lookup = database.lookup.clone();
        assert_eq!(
            database.ingest_test_terms(
                &context,
                vec![(keys[0].clone(), context.one())],
                Vec::new(),
            ),
            Err(GeneratedAffineResidualGroupExactDatabaseError::SourceOrderOverflow)
        );
        assert_eq!(database.next_source_ordinal, usize::MAX);
        assert_eq!(database.pivots, before_pivots);
        assert!(database.lookup == before_lookup);
    }

    #[test]
    fn candidate_retained_bytes_exact_and_one_below_are_transactional() {
        let (_family, context, _plan, _frame, mut pilot, keys) =
            database_fixture("exact-db-candidate-bytes-pilot");
        pilot
            .ingest_test_terms(
                &context,
                vec![(keys[0].clone(), context.integer(3))],
                Vec::new(),
            )
            .unwrap();
        let exact_bytes = pilot.stats().last_candidate_observed_retained_bytes();
        assert!(exact_bytes > 0);
        assert_eq!(
            pilot.pivot(0).unwrap().normalization_divisor(),
            &context.integer(3)
        );

        let (_family, context, _plan, _frame, mut exact, keys) =
            database_fixture("exact-db-candidate-bytes-exact");
        exact.limits.max_candidate_retained_bytes = exact_bytes;
        exact
            .ingest_test_terms(
                &context,
                vec![(keys[0].clone(), context.integer(3))],
                Vec::new(),
            )
            .unwrap();
        assert_eq!(
            exact.stats().last_candidate_observed_retained_bytes(),
            exact_bytes
        );
        assert_eq!(
            exact.pivot(0).unwrap().normalization_divisor(),
            &context.integer(3)
        );

        let (_family, context, _plan, _frame, mut one_below, keys) =
            database_fixture("exact-db-candidate-bytes-one-below");
        one_below.limits.max_candidate_retained_bytes = exact_bytes - 1;
        let before_pivots = one_below.pivots.clone();
        let before_lookup = one_below.lookup.clone();
        let before_source = one_below.next_source_ordinal;
        let before_stats = one_below.stats();
        assert!(matches!(
            one_below.ingest_test_terms(
                &context,
                vec![(keys[0].clone(), context.integer(3))],
                Vec::new(),
            ),
            Err(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceLimit {
                    limit,
                    ..
                }
            ) if limit == exact_bytes - 1
        ));
        assert_eq!(one_below.pivots, before_pivots);
        assert!(one_below.lookup == before_lookup);
        assert_eq!(one_below.next_source_ordinal, before_source);
        assert_eq!(one_below.stats(), before_stats);
    }

    #[test]
    fn borrowed_ingress_bytes_are_admitted_before_database_mutation() {
        let (_family, context, _plan, _frame, mut pilot, keys) =
            database_fixture("exact-db-ingress-bytes-pilot");
        pilot
            .ingest_test_terms(
                &context,
                vec![(keys[0].clone(), context.integer(3))],
                Vec::new(),
            )
            .unwrap();
        let exact_bytes = pilot.stats().last_ingress_observed_retained_bytes();
        assert!(exact_bytes > 0);
        assert!(pilot.stats().last_ingress_prospective_retained_bytes() <= exact_bytes);

        let (_family, context, _plan, _frame, mut exact, keys) =
            database_fixture("exact-db-ingress-bytes-exact");
        exact.limits.max_ingress_retained_bytes = exact_bytes;
        exact
            .ingest_test_terms(
                &context,
                vec![(keys[0].clone(), context.integer(3))],
                Vec::new(),
            )
            .unwrap();
        assert_eq!(
            exact.stats().last_ingress_observed_retained_bytes(),
            exact_bytes
        );

        let (_family, context, _plan, _frame, mut one_below, keys) =
            database_fixture("exact-db-ingress-bytes-one-below");
        one_below.limits.max_ingress_retained_bytes = exact_bytes - 1;
        let before_pivots = one_below.pivots.clone();
        let before_lookup = one_below.lookup.clone();
        let before_pivot_capacity = one_below.pivots.capacity();
        let before_lookup_capacity = one_below.lookup.capacity();
        let before_source = one_below.next_source_ordinal;
        let before_stats = one_below.stats();
        assert!(matches!(
            one_below.ingest_test_terms(
                &context,
                vec![(keys[0].clone(), context.integer(3))],
                Vec::new(),
            ),
            Err(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceLimit {
                    resource:
                        "exact-group borrowed ingress prospective retained bytes"
                        | "exact-group borrowed ingress observed retained bytes",
                    limit,
                    ..
                }
            ) if limit == exact_bytes - 1
        ));
        assert_eq!(one_below.pivots, before_pivots);
        assert!(one_below.lookup == before_lookup);
        assert_eq!(one_below.pivots.capacity(), before_pivot_capacity);
        assert_eq!(one_below.lookup.capacity(), before_lookup_capacity);
        assert_eq!(one_below.next_source_ordinal, before_source);
        assert_eq!(one_below.stats(), before_stats);
    }

    #[test]
    fn outer_capacity_accounting_and_replacement_failure_are_transactional() {
        let (_family, context, _plan, _frame, mut database, keys) =
            database_fixture("exact-db-outer-capacity-transaction");
        database
            .ingest_test_terms(&context, vec![(keys[0].clone(), context.one())], Vec::new())
            .unwrap();
        assert_eq!(
            database.stats().retained_database_bytes(),
            database_retained_bytes_with_candidate(
                &database.pivots,
                database.pivots.capacity(),
                database.lookup.capacity(),
                None,
            )
            .unwrap()
        );
        assert!(database.pivots.capacity() >= database.pivots.len());
        assert!(database.lookup.capacity() >= database.lookup.len());

        let before_pivots = database.pivots.clone();
        let before_lookup = database.lookup.clone();
        let before_pivot_capacity = database.pivots.capacity();
        let before_lookup_capacity = database.lookup.capacity();
        let before_source = database.next_source_ordinal;
        let before_stats = database.stats();
        fail_next_lookup_replacement_allocation_for_test();
        assert_eq!(
            database.ingest_test_terms(
                &context,
                vec![(keys[1].clone(), context.one())],
                Vec::new(),
            ),
            Err(
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "sorted exact-pivot lookup replacement",
                }
            )
        );
        assert_eq!(database.pivots, before_pivots);
        assert!(database.lookup == before_lookup);
        assert_eq!(database.pivots.capacity(), before_pivot_capacity);
        assert_eq!(database.lookup.capacity(), before_lookup_capacity);
        assert_eq!(database.next_source_ordinal, before_source);
        assert_eq!(database.stats(), before_stats);
    }

    #[test]
    fn cumulative_retained_bytes_exact_and_one_below_are_transactional() {
        let (_family, context, _plan, _frame, mut pilot, keys) =
            database_fixture("exact-db-cumulative-bytes-pilot");
        pilot
            .ingest_test_terms(&context, vec![(keys[0].clone(), context.one())], Vec::new())
            .unwrap();
        pilot
            .ingest_test_terms(&context, vec![(keys[1].clone(), context.one())], Vec::new())
            .unwrap();
        let exact_bytes = pilot.stats().retained_database_bytes();

        let (_family, context, _plan, _frame, mut exact, keys) =
            database_fixture("exact-db-cumulative-bytes-exact");
        exact.limits.max_database_retained_bytes = exact_bytes;
        exact
            .ingest_test_terms(&context, vec![(keys[0].clone(), context.one())], Vec::new())
            .unwrap();
        exact
            .ingest_test_terms(&context, vec![(keys[1].clone(), context.one())], Vec::new())
            .unwrap();
        assert_eq!(exact.stats().retained_database_bytes(), exact_bytes);

        let (_family, context, _plan, _frame, mut one_below, keys) =
            database_fixture("exact-db-cumulative-bytes-one-below");
        one_below.limits.max_database_retained_bytes = exact_bytes - 1;
        one_below
            .ingest_test_terms(&context, vec![(keys[0].clone(), context.one())], Vec::new())
            .unwrap();
        let before_pivots = one_below.pivots.clone();
        let before_lookup = one_below.lookup.clone();
        let before_source = one_below.next_source_ordinal;
        let before_stats = one_below.stats();
        assert!(matches!(
            one_below.ingest_test_terms(
                &context,
                vec![(keys[1].clone(), context.one())],
                Vec::new(),
            ),
            Err(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceLimit {
                    resource: "exact-group database retained bytes",
                    requested,
                    limit,
                }
            ) if requested == exact_bytes && limit == exact_bytes - 1
        ));
        assert_eq!(one_below.pivots, before_pivots);
        assert!(one_below.lookup == before_lookup);
        assert_eq!(one_below.next_source_ordinal, before_source);
        assert_eq!(one_below.stats(), before_stats);
    }
}
