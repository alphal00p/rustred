//! Allocation-sealed session for exact generated-affine group solving.
//!
//! This topology-neutral owner is the only production seam that pairs the
//! persistent exact row database with the unresolved targets derived from the
//! same solve-plan allocation.  A staged transaction retains both the
//! database's consume-once row token and the exact target-state `Arc`; neither
//! component is exposed separately.  Likewise, exact recentering receives one
//! borrowed, jointly authenticated view rather than caller-supplied database,
//! staged-row, or target-state parts.
//!
//! V1 exposes one typed dependent-row commit and no raw successor-state,
//! recentering, rule-publication, or master-inference transition outside this
//! module. A private unconsumed-commit kernel proves the atomic
//! database/target-state transition; crate callers reach its dependent branch
//! only through a sealed owning classification. Future no-target,
//! equality-refinement, and rejected-`WhenBad` paths must add equally typed
//! authorities. Dropping an otherwise unconsumed staged transaction leaves
//! both retained owners unchanged.

use std::fmt;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::generated_affine_residual_group_exact_database::{
    GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView,
    GeneratedAffineResidualGroupExactDatabase, GeneratedAffineResidualGroupExactDatabaseError,
    GeneratedAffineResidualGroupExactDatabaseLimits,
    GeneratedAffineResidualGroupExactReductionStep, GeneratedAffineResidualGroupExactRowOutcome,
    GeneratedAffineResidualGroupStagedExactRow,
};
use crate::generated_affine_residual_group_exact_physical_row::GeneratedAffineResidualGroupExactPhysicalRow;
use crate::generated_affine_residual_group_exact_targets::{
    GeneratedAffineResidualGroupExactTargetCatalog,
    GeneratedAffineResidualGroupExactTargetCatalogLimits,
    GeneratedAffineResidualGroupExactTargetError, GeneratedAffineResidualGroupExactTargetState,
    GeneratedAffineResidualGroupExactTargetStateLimits,
    GeneratedAffineResidualGroupExactTargetStateView,
    GeneratedAffineResidualGroupRetainedExactTarget,
};
use crate::generated_affine_residual_group_physical_key::{
    GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKey,
};
use crate::generated_affine_residual_group_solve_plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolveTargetLocator,
};
use crate::{
    IntegralFamily, ParametricCoefficient, ParametricCoefficientContext, ParametricNonZeroCondition,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-v1";

/// Unforgeable safe-Rust capability for the session-only exact-database API.
///
/// The type is visible to the sibling database module solely so protected
/// methods can name it in their signatures. Its seal and constructor remain
/// private here, it is neither `Clone` nor `Default`, and the owning session
/// never returns a borrow. Consequently another production sibling may name
/// the type but cannot produce a value with which to stage, authenticate, or
/// commit a database transition.
pub(crate) struct GeneratedAffineResidualGroupExactSessionDatabaseCapability {
    _seal: GeneratedAffineResidualGroupExactSessionDatabaseCapabilitySeal,
}

struct GeneratedAffineResidualGroupExactSessionDatabaseCapabilitySeal;

impl GeneratedAffineResidualGroupExactSessionDatabaseCapability {
    fn mint() -> Self {
        Self {
            _seal: GeneratedAffineResidualGroupExactSessionDatabaseCapabilitySeal,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionDatabaseCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionDatabaseCapability")
            .field("private_seal", &"<redacted>")
            .finish()
    }
}

/// Complete child limits for construction and replay of one exact session.
///
/// Each child owns its own arithmetic, replay, allocation, and retained-byte
/// accounting.  The session adds no unbounded collection of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactSessionLimits {
    pub(crate) database: GeneratedAffineResidualGroupExactDatabaseLimits,
    pub(crate) target_catalog: GeneratedAffineResidualGroupExactTargetCatalogLimits,
    pub(crate) target_state: GeneratedAffineResidualGroupExactTargetStateLimits,
}

impl Default for GeneratedAffineResidualGroupExactSessionLimits {
    fn default() -> Self {
        Self {
            database: GeneratedAffineResidualGroupExactDatabaseLimits::default(),
            target_catalog: GeneratedAffineResidualGroupExactTargetCatalogLimits::default(),
            target_state: GeneratedAffineResidualGroupExactTargetStateLimits::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactSessionError {
    Database(GeneratedAffineResidualGroupExactDatabaseError),
    Target(GeneratedAffineResidualGroupExactTargetError),
    WrongTargetStateAllocation,
    GeometryAuthentication,
    GeometryCountOverflow,
    MalformedGeometry,
    ReplayMismatch,
    SymbolicaPanic,
}

impl GeneratedAffineResidualGroupExactSessionError {
    const fn kind(self) -> &'static str {
        match self {
            Self::Database(_) => "Database",
            Self::Target(_) => "Target",
            Self::WrongTargetStateAllocation => "WrongTargetStateAllocation",
            Self::GeometryAuthentication => "GeometryAuthentication",
            Self::GeometryCountOverflow => "GeometryCountOverflow",
            Self::MalformedGeometry => "MalformedGeometry",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionError")
            .field("kind", &self.kind())
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Database(_) => "exact session database operation failed",
            Self::Target(_) => "exact session target operation failed",
            Self::WrongTargetStateAllocation => {
                "exact session transaction belongs to another target-state allocation"
            }
            Self::GeometryAuthentication => "exact session affine geometry authentication failed",
            Self::GeometryCountOverflow => "exact session affine geometry size overflowed",
            Self::MalformedGeometry => "exact session affine geometry is malformed",
            Self::ReplayMismatch => "exact session retained allocation replay mismatch",
            Self::SymbolicaPanic => "Symbolica panicked inside the exact session boundary",
        })
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionError {}

impl From<GeneratedAffineResidualGroupExactDatabaseError>
    for GeneratedAffineResidualGroupExactSessionError
{
    fn from(error: GeneratedAffineResidualGroupExactDatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<GeneratedAffineResidualGroupExactTargetError>
    for GeneratedAffineResidualGroupExactSessionError
{
    fn from(error: GeneratedAffineResidualGroupExactTargetError) -> Self {
        Self::Target(error)
    }
}

/// Failure of an unconsumed session transition.
///
/// Every error before database commit returns the complete sealed transaction,
/// so a caller may drop, inspect through a future policy layer, or retry it
/// without reconstructing authority. `PostPreflightCommitInvariant` is the
/// sole exception: the existing database API consumes its staged token when
/// called. That branch is unreachable while the database's documented
/// preflight/commit contract holds, because this session has already run the
/// same staged-token authentication under an exclusive `&mut self` borrow.
enum GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {
    Preflight {
        error: GeneratedAffineResidualGroupExactSessionError,
        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    },
    PostPreflightCommitInvariant {
        error: GeneratedAffineResidualGroupExactDatabaseError,
    },
}

impl GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {
    const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        match self {
            Self::Preflight { error, .. } => *error,
            Self::PostPreflightCommitInvariant { error } => {
                GeneratedAffineResidualGroupExactSessionError::Database(*error)
            }
        }
    }

    fn into_transaction(
        self,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedTransaction,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        match self {
            Self::Preflight { transaction, .. } => Ok(transaction),
            Self::PostPreflightCommitInvariant { error } => Err(
                GeneratedAffineResidualGroupExactSessionError::Database(error),
            ),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure")
            .field(
                "phase",
                &match self {
                    Self::Preflight { .. } => "preflight",
                    Self::PostPreflightCommitInvariant { .. } => "post-preflight commit invariant",
                },
            )
            .field("error", &self.error())
            .field("private_transaction", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Preflight { .. } => "exact unconsumed session transition failed before commit",
            Self::PostPreflightCommitInvariant { .. } => {
                "exact database rejected a completely preflighted unconsumed transition"
            }
        })
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {}

/// Recoverable failure to seal a staged transaction as dependent.
///
/// Classification is read-only. Every failure therefore returns the exact
/// consume-once transaction supplied by the caller; no database or target
/// state has changed.
pub(crate) struct GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
    error: GeneratedAffineResidualGroupExactSessionError,
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
}

impl GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        self.error
    }

    pub(crate) fn into_transaction(
        self,
    ) -> GeneratedAffineResidualGroupExactSessionStagedTransaction {
        self.transaction
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionDependentClassificationFailure")
            .field("error", &self.error)
            .field("private_transaction", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact session dependent-row classification failed")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {}

/// Non-forgeable, non-`Clone` owning proof that one live session transaction
/// has a dependent database payload.
///
/// Its raw database stage and retained target-state allocation remain private.
/// The scalar coordinates are retained only to check the supposedly
/// infallible typed publication tail after the private common commit kernel.
pub(crate) struct GeneratedAffineResidualGroupExactSessionClassifiedDependent {
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    source_ordinal: usize,
    reduction_count: usize,
}

impl GeneratedAffineResidualGroupExactSessionClassifiedDependent {
    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn reduction_count(&self) -> usize {
        self.reduction_count
    }

    pub(crate) fn into_transaction(
        self,
    ) -> GeneratedAffineResidualGroupExactSessionStagedTransaction {
        self.transaction
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionClassifiedDependent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionClassifiedDependent")
            .field("source_ordinal", &self.source_ordinal)
            .field("reduction_count", &self.reduction_count)
            .field("private_transaction", &"<redacted>")
            .finish()
    }
}

/// Typed successful result of committing a sealed dependent row.
pub(crate) struct GeneratedAffineResidualGroupExactSessionCommittedDependent {
    source_ordinal: usize,
    reductions: Vec<GeneratedAffineResidualGroupExactReductionStep>,
}

impl GeneratedAffineResidualGroupExactSessionCommittedDependent {
    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) fn reductions(&self) -> &[GeneratedAffineResidualGroupExactReductionStep] {
        &self.reductions
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommittedDependent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommittedDependent")
            .field("source_ordinal", &self.source_ordinal)
            .field("reduction_count", &self.reductions.len())
            .field("private_reductions", &"<redacted>")
            .finish()
    }
}

/// Failure of the typed dependent commit path.
///
/// Every ordinary preflight error retains the complete sealed classification,
/// including its consume-once transaction. Only violation of the database's
/// already-preflighted commit invariant lacks a recoverable token.
pub(crate) enum GeneratedAffineResidualGroupExactSessionCommitDependentFailure {
    Preflight {
        error: GeneratedAffineResidualGroupExactSessionError,
        classified: GeneratedAffineResidualGroupExactSessionClassifiedDependent,
    },
    PostPreflightCommitInvariant {
        error: GeneratedAffineResidualGroupExactSessionError,
    },
}

impl GeneratedAffineResidualGroupExactSessionCommitDependentFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        match self {
            Self::Preflight { error, .. } | Self::PostPreflightCommitInvariant { error } => *error,
        }
    }

    pub(crate) fn into_classified(
        self,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionClassifiedDependent,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        match self {
            Self::Preflight { classified, .. } => Ok(classified),
            Self::PostPreflightCommitInvariant { error } => Err(error),
        }
    }

    pub(crate) fn into_transaction(
        self,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedTransaction,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.into_classified()
            .map(GeneratedAffineResidualGroupExactSessionClassifiedDependent::into_transaction)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommitDependentFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommitDependentFailure")
            .field(
                "phase",
                &match self {
                    Self::Preflight { .. } => "preflight",
                    Self::PostPreflightCommitInvariant { .. } => "post-preflight commit invariant",
                },
            )
            .field("error", &self.error())
            .field("private_classification", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionCommitDependentFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Preflight { .. } => "exact dependent session transition failed before commit",
            Self::PostPreflightCommitInvariant { .. } => {
                "exact dependent session transition violated a post-preflight invariant"
            }
        })
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionCommitDependentFailure {}

/// One allocation-bound exact solve session.
///
/// Construction is the unique V1 minting path for the initial target state:
/// the database first creates an opaque, non-`Clone` binding, which is consumed
/// by the state owner and immediately authenticated back against that same
/// database allocation.
pub(crate) struct GeneratedAffineResidualGroupExactSession {
    schema: &'static str,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    database_capability: GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    database: GeneratedAffineResidualGroupExactDatabase,
    catalog: Arc<GeneratedAffineResidualGroupExactTargetCatalog>,
    target_state: Arc<GeneratedAffineResidualGroupExactTargetState>,
    limits: GeneratedAffineResidualGroupExactSessionLimits,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSession")
            .field("schema", &self.schema)
            .field("database_epoch", &self.database.database_epoch())
            .field("group_ordinal", &self.database.group_ordinal())
            .field("state_version", &self.database.state_version())
            .field("pivot_count", &self.database.pivot_count())
            .field("target_count", &self.catalog.len())
            .field("private_plan", &"<redacted>")
            .field("private_database_capability", &"<redacted>")
            .field("private_database", &"<redacted>")
            .field("private_catalog", &"<redacted>")
            .field("private_target_state", &"<redacted>")
            .field("publishes_rule", &false)
            .field("infers_master", &false)
            .finish()
    }
}

impl GeneratedAffineResidualGroupExactSession {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        database_epoch: usize,
        limits: GeneratedAffineResidualGroupExactSessionLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupExactSessionError> {
        catch_unwind(AssertUnwindSafe(|| {
            let database_capability =
                GeneratedAffineResidualGroupExactSessionDatabaseCapability::mint();
            let database = GeneratedAffineResidualGroupExactDatabase::try_new(
                family,
                context,
                Arc::clone(&plan),
                Arc::clone(plan.physical_frame()),
                database_epoch,
                limits.database,
            )?;
            let catalog = Arc::new(plan.compile_exact_target_catalog(
                family,
                context,
                limits.target_catalog,
            )?);
            let binding =
                database.initial_target_state_binding_for_session(&database_capability)?;
            let target_state = GeneratedAffineResidualGroupExactTargetState::try_new(
                family,
                context,
                Arc::clone(&catalog),
                binding,
                limits.target_state,
            )?;
            database.authenticate_target_state_binding(target_state.binding())?;
            Ok(Self {
                schema: GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA,
                plan,
                database_capability,
                database,
                catalog,
                target_state,
                limits,
            })
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic)?
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualGroupExactSessionLimits {
        self.limits
    }

    pub(crate) const fn database_epoch(&self) -> usize {
        self.database.database_epoch()
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.database.group_ordinal()
    }

    pub(crate) const fn state_version(&self) -> usize {
        self.database.state_version()
    }

    pub(crate) fn target_count(&self) -> usize {
        self.catalog.len()
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    /// Replay every retained child and the opaque database/state handshake.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA
                || self.database.group_ordinal() != self.plan.group_ordinal()
                || self.database.database_epoch() != self.target_state.database_epoch()
                || self.database.state_version() != self.target_state.state_version()
                || self.catalog.group_ordinal() != self.plan.group_ordinal()
                || !self.catalog.same_plan_allocation(&self.plan)
            {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }
            self.database
                .authenticate_target_state_binding(self.target_state.binding())?;
            self.catalog.replay(family, context, &self.plan)?;
            self.target_state.replay(
                family,
                context,
                &self.plan,
                self.database.group_ordinal(),
                self.database.database_epoch(),
                self.database.state_version(),
            )?;
            Ok(())
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic)?
    }

    /// Stage one authenticated production row without mutating either owner.
    ///
    /// The returned token retains the exact current target-state allocation;
    /// callers cannot replace it or extract the raw database stage.
    pub(crate) fn stage_replayed_row(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedTransaction,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.replay(family, context)?;
        let staged = self.database.stage_replayed_row_for_session(
            &self.database_capability,
            family,
            context,
            &self.plan,
            self.plan.physical_frame(),
            self.database.database_epoch(),
            source,
        )?;
        Ok(GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&self.target_state),
        })
    }

    /// Jointly authenticate one staged new pivot and its exact unresolved
    /// target state.  This is the sole V1 recentering ingress.
    pub(crate) fn authenticate_staged_new_pivot<'a>(
        &'a self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        transaction: &'a GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedNewPivotView<'a>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.authenticate_target_state_allocation(&transaction.target_state)?;
        self.database
            .authenticate_target_state_binding(transaction.target_state.binding())?;
        let staged_pivot = self.database.authenticate_staged_new_pivot_for_session(
            &self.database_capability,
            &transaction.staged,
        )?;
        let targets = transaction
            .target_state
            .authenticated_view(family, context)?;
        if !targets.authenticates_state_allocation(&self.target_state)
            || !Arc::ptr_eq(
                staged_pivot.plan_for_session(&self.database_capability),
                &self.plan,
            )
            || !Arc::ptr_eq(staged_pivot.frame(), self.plan.physical_frame())
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }

        // Borrow the compact affine geometry from the retained, authenticated
        // plan authority only after the database and target-state transaction
        // have been jointly authenticated. The authority/plan owners never
        // cross this API; recentering receives only the two shape scalars and
        // borrowed row-major matrix it actually needs.
        let plan = staged_pivot.plan_for_session(&self.database_capability);
        let group = plan
            .authority()
            .authenticated_group_view(context)
            .map_err(|_| GeneratedAffineResidualGroupExactSessionError::GeometryAuthentication)?;
        let ambient_arity = group.ambient_arity();
        let free_positions = group.free_positions();
        let matrix_entries = ambient_arity
            .checked_mul(free_positions.len())
            .ok_or(GeneratedAffineResidualGroupExactSessionError::GeometryCountOverflow)?;
        let compact_affine_matrix = group.compact_linear_coefficients();
        if group.ordinal() != staged_pivot.group_ordinal()
            || ambient_arity != context.index_count()
            || ambient_arity != staged_pivot.frame().arity()
            || free_positions != plan.free_positions()
            || free_positions
                .iter()
                .any(|&position| position >= ambient_arity)
            || compact_affine_matrix.len() != matrix_entries
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::MalformedGeometry);
        }
        let staged_live_prospective_retained_bytes =
            staged_pivot.staged_live_prospective_retained_bytes();
        let staged_live_observed_retained_bytes =
            staged_pivot.staged_live_observed_retained_bytes();
        let target_state_combined_retained_byte_envelope = transaction
            .target_state
            .stats()
            .combined_retained_byte_envelope();
        let anchor_case_ordinal = plan.anchor_case_ordinal();
        let free_positions = plan.free_positions();
        let target_locators = plan.targets();
        Ok(GeneratedAffineResidualGroupExactSessionStagedNewPivotView {
            staged_pivot,
            targets,
            anchor_case_ordinal,
            free_positions,
            target_locators,
            ambient_arity,
            compact_affine_matrix,
            staged_live_prospective_retained_bytes,
            staged_live_observed_retained_bytes,
            target_state_combined_retained_byte_envelope,
        })
    }

    /// Consume one raw staged transaction into a non-forgeable dependent
    /// classification. Classification authenticates both retained owners and
    /// the database payload, mutates nothing, and returns the intact
    /// transaction on every rejection (including new-pivot, stale, and
    /// foreign cases).
    pub(crate) fn classify_dependent(
        &self,
        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionClassifiedDependent,
        GeneratedAffineResidualGroupExactSessionDependentClassificationFailure,
    > {
        let classification = (|| {
            self.authenticate_target_state_allocation(&transaction.target_state)?;
            self.database
                .authenticate_target_state_binding(transaction.target_state.binding())?;
            let dependent = self.database.authenticate_staged_dependent_for_session(
                &self.database_capability,
                &transaction.staged,
            )?;
            Ok((dependent.source_ordinal(), dependent.reductions().len()))
        })();
        match classification {
            Ok((source_ordinal, reduction_count)) => Ok(
                GeneratedAffineResidualGroupExactSessionClassifiedDependent {
                    transaction,
                    source_ordinal,
                    reduction_count,
                },
            ),
            Err(error) => Err(
                GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
                    error,
                    transaction,
                },
            ),
        }
    }

    /// Commit one sealed dependent row without consuming a solve target.
    ///
    /// This is the only crate-visible route from a dependent session
    /// transaction to the private common transition kernel. A failure before
    /// database commit returns the complete classified transaction.
    pub(crate) fn commit_dependent(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        classified: GeneratedAffineResidualGroupExactSessionClassifiedDependent,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionCommittedDependent,
        GeneratedAffineResidualGroupExactSessionCommitDependentFailure,
    > {
        let GeneratedAffineResidualGroupExactSessionClassifiedDependent {
            transaction,
            source_ordinal: classified_source_ordinal,
            reduction_count: classified_reduction_count,
        } = classified;
        match self.commit_unconsumed(family, context, transaction) {
            Ok(GeneratedAffineResidualGroupExactRowOutcome::Dependent {
                source_ordinal,
                reductions,
            }) if source_ordinal == classified_source_ordinal
                && reductions.len() == classified_reduction_count =>
            {
                Ok(
                    GeneratedAffineResidualGroupExactSessionCommittedDependent {
                        source_ordinal,
                        reductions,
                    },
                )
            }
            Ok(GeneratedAffineResidualGroupExactRowOutcome::Dependent { .. })
            | Ok(GeneratedAffineResidualGroupExactRowOutcome::NewPivot { .. }) => Err(
                GeneratedAffineResidualGroupExactSessionCommitDependentFailure::PostPreflightCommitInvariant {
                    error: GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                },
            ),
            Err(GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure::Preflight {
                error,
                transaction,
            }) => Err(
                GeneratedAffineResidualGroupExactSessionCommitDependentFailure::Preflight {
                    error,
                    classified:
                        GeneratedAffineResidualGroupExactSessionClassifiedDependent {
                            transaction,
                            source_ordinal: classified_source_ordinal,
                            reduction_count: classified_reduction_count,
                        },
                },
            ),
            Err(
                GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure::PostPreflightCommitInvariant {
                    error,
                },
            ) => Err(
                GeneratedAffineResidualGroupExactSessionCommitDependentFailure::PostPreflightCommitInvariant {
                    error: GeneratedAffineResidualGroupExactSessionError::Database(error),
                },
            ),
        }
    }

    /// Commit a staged row without consuming any solve target.
    ///
    /// This is the common transition for dependent rows and for new pivots
    /// that produce no target, require affine-equality refinement, or are
    /// rejected/unsupported by a later `WhenBad` policy. It never publishes a
    /// rule or infers a master. The complete successor target state is built
    /// fallibly before database mutation; after database commit, installing
    /// the prebuilt `Arc` is an allocation-free move. This untyped kernel is
    /// intentionally module-private: exposing it directly would let a caller
    /// skip recentering/`WhenBad` and advance an arbitrary new pivot. Future
    /// The crate-visible dependent wrapper below already requires its sealed
    /// classification; future wrappers must require the corresponding sealed
    /// recenter/`WhenBad` outcome.
    fn commit_unconsumed(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure,
    > {
        let successor = match self.prepare_unconsumed_successor(family, context, &transaction) {
            Ok(successor) => successor,
            Err(error) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure::Preflight {
                        error,
                        transaction,
                    },
                );
            }
        };

        let GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: transaction_target_state,
        } = transaction;
        let outcome = match self
            .database
            .commit_staged_row_for_session(&self.database_capability, staged)
        {
            Ok(outcome) => outcome,
            Err(error) => {
                return Err(GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure::PostPreflightCommitInvariant {
                    error,
                });
            }
        };

        // Infallible, allocation-free publication tail. The old target state
        // stays live through `transaction_target_state` until both retained
        // owners have advanced coherently.
        let prior_target_state = std::mem::replace(&mut self.target_state, successor);
        debug_assert_eq!(
            self.database.state_version(),
            self.target_state.state_version()
        );
        debug_assert!(
            self.database
                .authenticate_target_state_binding(self.target_state.binding())
                .is_ok()
        );
        drop(transaction_target_state);
        drop(prior_target_state);
        Ok(outcome)
    }

    fn prepare_unconsumed_successor(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        transaction: &GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        Arc<GeneratedAffineResidualGroupExactTargetState>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.authenticate_target_state_allocation(&transaction.target_state)?;
        self.database
            .authenticate_target_state_binding(transaction.target_state.binding())?;
        let successor_binding = self.database.successor_target_state_binding_for_session(
            &self.database_capability,
            &transaction.staged,
        )?;
        transaction
            .target_state
            .prepare_successor(family, context, successor_binding, None)
            .map_err(GeneratedAffineResidualGroupExactSessionError::from)
    }

    fn authenticate_target_state_allocation(
        &self,
        target_state: &Arc<GeneratedAffineResidualGroupExactTargetState>,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        if !Arc::ptr_eq(target_state, &self.target_state) {
            return Err(GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation);
        }
        Ok(())
    }
}

/// Consume-once staged database row inseparably paired with its target state.
///
/// This type is intentionally neither `Clone` nor decomposable outside this
/// module. The private atomic kernel may consume it exactly once; every
/// crate-visible transition additionally requires a sealed classification or
/// policy outcome.
pub(crate) struct GeneratedAffineResidualGroupExactSessionStagedTransaction {
    staged: GeneratedAffineResidualGroupStagedExactRow,
    target_state: Arc<GeneratedAffineResidualGroupExactTargetState>,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionStagedTransaction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionStagedTransaction")
            .field("database_epoch", &self.target_state.database_epoch())
            .field("group_ordinal", &self.target_state.group_ordinal())
            .field("state_version", &self.target_state.state_version())
            .field("private_database_stage", &"<redacted>")
            .field("private_target_state", &"<redacted>")
            .finish()
    }
}

/// Sealed simultaneous borrow of a database-authenticated new pivot and the
/// exact unresolved targets belonging to the same live session state.
pub(crate) struct GeneratedAffineResidualGroupExactSessionStagedNewPivotView<'a> {
    staged_pivot: GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'a>,
    targets: GeneratedAffineResidualGroupExactTargetStateView<'a>,
    anchor_case_ordinal: usize,
    free_positions: &'a [usize],
    target_locators: &'a [GeneratedAffineResidualGroupSolveTargetLocator],
    ambient_arity: usize,
    compact_affine_matrix: &'a [Integer],
    staged_live_prospective_retained_bytes: usize,
    staged_live_observed_retained_bytes: usize,
    target_state_combined_retained_byte_envelope: usize,
}

impl<'a> GeneratedAffineResidualGroupExactSessionStagedNewPivotView<'a> {
    pub(crate) fn key(&self) -> &'a GeneratedAffineResidualGroupPhysicalKey {
        self.staged_pivot.key()
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
        self.staged_pivot.terms()
    }

    pub(crate) fn guards(&self) -> &'a [ParametricNonZeroCondition] {
        self.staged_pivot.guards()
    }

    pub(crate) fn reductions(&self) -> &'a [GeneratedAffineResidualGroupExactReductionStep] {
        self.staged_pivot.reductions()
    }

    pub(crate) const fn normalization_divisor(&self) -> &'a ParametricCoefficient {
        self.staged_pivot.normalization_divisor()
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.staged_pivot.source_ordinal()
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.staged_pivot.pivot_ordinal()
    }

    pub(crate) fn production_source(
        &self,
    ) -> Option<&'a Arc<GeneratedAffineResidualGroupExactPhysicalRow>> {
        self.staged_pivot.production_source()
    }

    pub(crate) fn target_ordinals(&self) -> Range<usize> {
        self.targets.iter()
    }

    pub(crate) fn is_target_unresolved(
        &self,
        solve_ordinal: usize,
    ) -> Result<bool, GeneratedAffineResidualGroupExactSessionError> {
        self.targets
            .is_unresolved(solve_ordinal)
            .map_err(GeneratedAffineResidualGroupExactSessionError::from)
    }

    pub(crate) fn retain_target(
        &self,
        solve_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualGroupRetainedExactTarget,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.targets
            .retain_target(solve_ordinal)
            .map_err(GeneratedAffineResidualGroupExactSessionError::from)
    }

    pub(crate) fn physical_frame(&self) -> &'a Arc<GeneratedAffineResidualGroupPhysicalFrame> {
        self.staged_pivot.frame()
    }

    pub(crate) const fn database_epoch(&self) -> usize {
        self.staged_pivot.database_epoch()
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.staged_pivot.group_ordinal()
    }

    pub(crate) fn anchor_case_ordinal(&self) -> usize {
        self.anchor_case_ordinal
    }

    pub(crate) fn free_positions(&self) -> &[usize] {
        self.free_positions
    }

    pub(crate) fn target_locators(&self) -> &[GeneratedAffineResidualGroupSolveTargetLocator] {
        self.target_locators
    }

    pub(crate) const fn ambient_arity(&self) -> usize {
        self.ambient_arity
    }

    /// Borrowed row-major `ambient_arity * free_positions().len()` compact
    /// affine matrix authenticated from the retained plan authority.
    pub(crate) const fn compact_affine_matrix(&self) -> &'a [Integer] {
        self.compact_affine_matrix
    }

    pub(crate) const fn staged_live_prospective_retained_bytes(&self) -> usize {
        self.staged_live_prospective_retained_bytes
    }

    pub(crate) const fn staged_live_observed_retained_bytes(&self) -> usize {
        self.staged_live_observed_retained_bytes
    }

    pub(crate) const fn target_state_combined_retained_byte_envelope(&self) -> usize {
        self.target_state_combined_retained_byte_envelope
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionStagedNewPivotView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionStagedNewPivotView")
            .field("database_epoch", &self.database_epoch())
            .field("group_ordinal", &self.group_ordinal())
            .field("state_version", &self.staged_pivot.state_version())
            .field("source_ordinal", &self.staged_pivot.source_ordinal())
            .field("pivot_ordinal", &self.staged_pivot.pivot_ordinal())
            .field("ambient_arity", &self.ambient_arity)
            .field(
                "compact_affine_matrix_entries",
                &self.compact_affine_matrix.len(),
            )
            .field("target_count", &self.target_locators().len())
            .field(
                "staged_live_prospective_retained_bytes",
                &self.staged_live_prospective_retained_bytes,
            )
            .field(
                "staged_live_observed_retained_bytes",
                &self.staged_live_observed_retained_bytes,
            )
            .field(
                "target_state_combined_retained_byte_envelope",
                &self.target_state_combined_retained_byte_envelope,
            )
            .field("private_staged_pivot", &"<redacted>")
            .field("private_compact_affine_matrix", &"<redacted>")
            .field("private_target_state", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
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
    use crate::generated_affine_residual_group_physical_key::{
        GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKeyLimits,
    };
    use crate::generated_affine_residual_group_solve_plan::GeneratedAffineResidualGroupSolvePlanLimits;
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        SectorMask,
    };

    fn test_family(name: &str) -> IntegralFamily {
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

    fn plan_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) {
        let family = test_family(name);
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
                frame,
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        (family, context, plan)
    }

    fn production_row(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> Arc<GeneratedAffineResidualGroupExactPhysicalRow> {
        let frame = plan.physical_frame();
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
        panic!("the generated-affine fixture produced no authenticated physical row")
    }

    #[test]
    fn construction_owns_one_database_bound_catalog_and_initial_state() {
        let (family, context, plan) = plan_fixture("exact-session-construction-private");
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            53,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();

        assert_eq!(
            session.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA
        );
        assert_eq!(session.database_epoch(), 53);
        assert_eq!(session.group_ordinal(), plan.group_ordinal());
        assert_eq!(session.state_version(), 0);
        assert_eq!(session.target_count(), plan.targets().len());
        assert_eq!(
            session.limits(),
            GeneratedAffineResidualGroupExactSessionLimits::default()
        );
        assert!(!session.publishes_rule());
        assert!(!session.infers_master());
        session.replay(&family, &context).unwrap();
        session
            .database
            .authenticate_target_state_binding(session.target_state.binding())
            .unwrap();
    }

    #[test]
    fn production_database_transition_surface_is_session_capability_gated() {
        let database_source = include_str!("generated_affine_residual_group_exact_database.rs");
        let session_source = include_str!("generated_affine_residual_group_exact_session.rs");
        let capability = "GeneratedAffineResidualGroupExactSessionDatabaseCapability";

        // Every production entry capable of minting, classifying, or
        // consuming database transition authority names the unforgeable
        // session capability in its signature.
        for method in [
            "initial_target_state_binding_for_session",
            "successor_target_state_binding_for_session",
            "stage_replayed_row_for_session",
            "authenticate_staged_new_pivot_for_session",
            "authenticate_staged_dependent_for_session",
            "commit_staged_row_for_session",
            "plan_for_session",
        ] {
            let marker = format!("fn {method}");
            let start = database_source
                .find(&marker)
                .unwrap_or_else(|| panic!("missing capability-gated method {method}"));
            let signature_end = database_source[start..]
                .find(" {")
                .map(|offset| start + offset)
                .unwrap_or_else(|| panic!("unterminated signature for {method}"));
            assert!(
                database_source[start..signature_end].contains(capability),
                "production database method {method} lacks the session capability"
            );
        }

        // Legacy direct database names no longer exist. Explicit `_for_test`
        // adapters complement Rust's compile-time private-field seal and are
        // absent from a normal library build.
        for method in [
            "initial_target_state_binding",
            "successor_target_state_binding",
            "stage_replayed_row",
            "ingest_replayed_row",
            "authenticate_staged_new_pivot",
            "commit_staged_row",
            "plan",
        ] {
            assert!(
                !database_source.contains(&format!("fn {method}(")),
                "legacy direct database API {method} remains nameable"
            );
        }
        for method in [
            "initial_target_state_binding_for_test",
            "successor_target_state_binding_for_test",
            "stage_replayed_row_for_test",
            "ingest_replayed_row_for_test",
            "commit_staged_row_for_test",
        ] {
            let marker = format!("    pub(crate) fn {method}(");
            let occurrences = database_source.match_indices(&marker).collect::<Vec<_>>();
            assert_eq!(
                occurrences.len(),
                1,
                "test transition adapter {method} must have exactly one definition"
            );
            let prefix = &database_source[..occurrences[0].0];
            assert!(
                prefix.ends_with("    #[cfg(test)]\n"),
                "test transition adapter {method} is not cfg(test)-sealed"
            );
        }
        assert!(!database_source.contains("pub(crate) fn authenticate_staged_dependent("));
        assert!(
            database_source.contains("#[cfg(test)]\n    fn authenticate_staged_new_pivot_for_test")
        );
        assert!(database_source.contains("#[cfg(test)]\n    fn plan_for_test("));

        // The capability itself has a private seal and a module-private mint;
        // the session stores it privately and exposes no capability accessor.
        assert!(
            session_source
                .contains("struct GeneratedAffineResidualGroupExactSessionDatabaseCapabilitySeal;")
        );
        assert!(session_source.contains("    fn mint() -> Self"));
        let crate_visible_mint = ["pub(crate)", " fn mint() -> Self"].concat();
        let public_mint = ["pub", " fn mint() -> Self"].concat();
        let capability_accessor = ["fn database_", "capability(&self)"].concat();
        assert!(!session_source.contains(&crate_visible_mint));
        assert!(!session_source.contains(&public_mint));
        assert!(!session_source.contains(&capability_accessor));
    }

    #[test]
    fn joint_view_rejects_foreign_state_despite_equal_visible_coordinates() {
        let (family, context, plan) = plan_fixture("exact-session-foreign-state-private");
        let first = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            59,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let second = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            59,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        assert_eq!(first.database_epoch(), second.database_epoch());
        assert_eq!(first.group_ordinal(), second.group_ordinal());
        assert_eq!(first.state_version(), second.state_version());
        assert_eq!(first.target_count(), second.target_count());
        assert!(!first.target_state.same_allocation(&second.target_state));
        assert_eq!(
            first.authenticate_target_state_allocation(&second.target_state),
            Err(GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation)
        );
        assert!(
            first
                .database
                .authenticate_target_state_binding(second.target_state.binding())
                .is_err()
        );
        assert!(
            second
                .database
                .authenticate_target_state_binding(first.target_state.binding())
                .is_err()
        );

        let source = production_row(&family, &context, &plan);
        let transaction = first
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let expected_group = plan.authority().authenticated_group_view(&context).unwrap();
        let expected_ambient_arity = expected_group.ambient_arity();
        let expected_matrix = expected_group.compact_linear_coefficients();
        let expected_staged_live_prospective =
            transaction.staged.staged_live_prospective_retained_bytes();
        let expected_staged_live_observed =
            transaction.staged.staged_live_observed_retained_bytes();
        let expected_target_envelope = transaction
            .target_state
            .stats()
            .combined_retained_byte_envelope();
        let joint = first
            .authenticate_staged_new_pivot(&family, &context, &transaction)
            .unwrap();
        assert_eq!(joint.database_epoch(), 59);
        assert_eq!(joint.group_ordinal(), plan.group_ordinal());
        assert_eq!(joint.anchor_case_ordinal(), plan.anchor_case_ordinal());
        assert_eq!(joint.free_positions(), plan.free_positions());
        assert_eq!(joint.target_locators(), plan.targets());
        assert_eq!(joint.ambient_arity(), expected_ambient_arity);
        assert_eq!(joint.ambient_arity(), context.index_count());
        assert_eq!(
            joint.compact_affine_matrix().len(),
            joint.ambient_arity() * joint.free_positions().len()
        );
        assert!(std::ptr::eq(joint.compact_affine_matrix(), expected_matrix));
        assert_eq!(
            joint.staged_live_prospective_retained_bytes(),
            expected_staged_live_prospective
        );
        assert_eq!(
            joint.staged_live_observed_retained_bytes(),
            expected_staged_live_observed
        );
        assert_eq!(
            joint.target_state_combined_retained_byte_envelope(),
            expected_target_envelope
        );
        assert!(
            joint.staged_live_prospective_retained_bytes()
                <= joint.staged_live_observed_retained_bytes()
        );
        assert!(Arc::ptr_eq(joint.physical_frame(), plan.physical_frame()));
        assert_eq!(joint.target_ordinals().len(), plan.targets().len());
        assert!(
            joint
                .target_ordinals()
                .all(|ordinal| joint.is_target_unresolved(ordinal) == Ok(true))
        );
        assert_eq!(joint.pivot_ordinal(), 0);
        assert_eq!(joint.source_ordinal(), 0);
        assert!(joint.production_source().is_some());
        assert!(joint.terms().len() > 0);
        assert_eq!(joint.key(), joint.terms().next_back().unwrap().0);
        assert!(joint.guards().len() <= source.guard_count());
        assert!(joint.reductions().is_empty());
        assert!(!joint.normalization_divisor().is_zero());
        let first_target = joint.target_ordinals().next().unwrap();
        let retained_target = joint.retain_target(first_target).unwrap();
        assert_eq!(retained_target.solve_ordinal(), first_target);
        let joint_debug = format!("{joint:?}");
        assert!(joint_debug.contains("private_staged_pivot: \"<redacted>\""));
        assert!(joint_debug.contains("private_compact_affine_matrix: \"<redacted>\""));
        assert!(joint_debug.contains("private_target_state: \"<redacted>\""));
        assert!(!joint_debug.contains(plan.stable_manifest()));
        drop(retained_target);
        drop(joint);
        drop(transaction);

        let transaction = first
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionStagedTransaction { staged, .. } = transaction;
        let forged = GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&second.target_state),
        };
        assert!(matches!(
            first.authenticate_staged_new_pivot(&family, &context, &forged),
            Err(GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation)
        ));
        assert!(matches!(
            second.authenticate_staged_new_pivot(&family, &context, &forged),
            Err(GeneratedAffineResidualGroupExactSessionError::Database(_))
        ));
    }

    #[test]
    fn typed_dependent_commit_advances_database_and_targets_atomically() {
        let (family, context, plan) = plan_fixture("exact-session-unconsumed-commit-private");
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            61,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let source = production_row(&family, &context, &plan);
        let initial_state = Arc::clone(&session.target_state);
        let initial_target_stats = session.target_state.stats();

        // Both transactions bind the exact same visible version and exact
        // initial state allocation. Committing either must stale the other.
        let accepted = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let competing = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let outcome = session
            .commit_unconsumed(&family, &context, accepted)
            .unwrap();
        assert_eq!(
            outcome,
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0,
            }
        );
        assert_eq!(session.database.state_version(), 1);
        assert_eq!(session.target_state.state_version(), 1);
        assert_eq!(session.database.pivot_count(), 1);
        assert!(!session.target_state.same_allocation(&initial_state));
        assert_eq!(
            session.target_state.stats().dispositions(),
            initial_target_stats.dispositions()
        );
        assert_eq!(
            session.target_state.stats().unresolved(),
            initial_target_stats.unresolved()
        );
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();

        let failure = session
            .commit_unconsumed(&family, &context, competing)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        let competing = failure.into_transaction().unwrap();
        assert_eq!(session.database.state_version(), 1);
        assert_eq!(session.target_state.state_version(), 1);
        assert_eq!(session.database.pivot_count(), 1);
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();

        // Even if an internal adversarial caller replaces only the retained
        // state Arc, the independently sealed database stage remains stale and
        // the recoverable preflight failure mutates neither owner.
        let GeneratedAffineResidualGroupExactSessionStagedTransaction { staged, .. } = competing;
        let forged = GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&session.target_state),
        };
        let failure = session
            .commit_unconsumed(&family, &context, forged)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::StaleStagedRow
            )
        );
        drop(failure.into_transaction().unwrap());
        assert_eq!(session.database.state_version(), 1);
        assert_eq!(session.target_state.state_version(), 1);
        assert_eq!(session.database.pivot_count(), 1);
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();

        // Replaying the identical production row now closes against pivot 0,
        // but the unconsumed transition still advances both state versions.
        let dependent = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let classified = session.classify_dependent(dependent).unwrap();
        assert_eq!(classified.source_ordinal(), 1);
        assert_eq!(classified.reduction_count(), 1);
        let committed = session
            .commit_dependent(&family, &context, classified)
            .unwrap();
        assert_eq!(committed.source_ordinal(), 1);
        assert_eq!(committed.reductions().len(), 1);
        assert_eq!(session.database.state_version(), 2);
        assert_eq!(session.target_state.state_version(), 2);
        assert_eq!(session.database.pivot_count(), 1);
        assert_eq!(
            session.target_state.stats().unresolved(),
            initial_target_stats.unresolved()
        );
        assert_eq!(session.target_state.stats().consumed(), 0);
        assert!(!session.publishes_rule());
        assert!(!session.infers_master());
        session.replay(&family, &context).unwrap();

        // A target-successor resource failure occurs after the database has
        // authenticated/minted the future binding but before either owner is
        // mutated, and therefore returns the complete transaction.
        let mut limits = GeneratedAffineResidualGroupExactSessionLimits::default();
        limits.target_state.max_disposition_copies = 0;
        let mut limited = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            67,
            limits,
        )
        .unwrap();
        assert!(limited.target_count() > 0);
        let transaction = limited
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let failure = limited
            .commit_unconsumed(&family, &context, transaction)
            .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Target(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target disposition copies",
                    requested,
                    limit: 0,
                }
            ) if requested == limited.target_count()
        ));
        let recovered = failure.into_transaction().unwrap();
        let recovered_view = limited
            .authenticate_staged_new_pivot(&family, &context, &recovered)
            .unwrap();
        assert_eq!(recovered_view.source_ordinal(), 0);
        drop(recovered_view);
        drop(recovered);
        assert_eq!(limited.database.state_version(), 0);
        assert_eq!(limited.target_state.state_version(), 0);
        assert_eq!(limited.database.pivot_count(), 0);
        assert_eq!(limited.target_state.stats().consumed(), 0);
        limited.replay(&family, &context).unwrap();
    }

    #[test]
    fn dependent_classifier_rejects_new_pivot_and_returns_intact_transaction() {
        let (family, context, plan) = plan_fixture("exact-session-dependent-reject-new-pivot");
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            71,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let source = production_row(&family, &context, &plan);
        let transaction = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let failure = session.classify_dependent(transaction).unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::NewPivotStagedRow
            )
        );
        let failure_debug = format!("{failure:?}");
        assert!(failure_debug.contains("private_transaction: \"<redacted>\""));
        let recovered = failure.into_transaction();
        let joint = session
            .authenticate_staged_new_pivot(&family, &context, &recovered)
            .unwrap();
        assert_eq!(joint.source_ordinal(), 0);
        assert_eq!(joint.pivot_ordinal(), 0);
        drop(joint);
        drop(recovered);

        assert_eq!(session.database.state_version(), 0);
        assert_eq!(session.target_state.state_version(), 0);
        assert_eq!(session.database.pivot_count(), 0);
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();
    }

    #[test]
    fn dependent_commit_stale_and_foreign_failures_preserve_transaction_authority() {
        let (family, context, plan) = plan_fixture("exact-session-dependent-stale-foreign");
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            73,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let foreign = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            73,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let source = production_row(&family, &context, &plan);

        // Seed exactly one pivot through the still-private new-pivot test
        // kernel, then classify two competing dependent stages at version 1.
        let first_pivot = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        assert!(matches!(
            session
                .commit_unconsumed(&family, &context, first_pivot)
                .unwrap(),
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0
            }
        ));
        let accepted = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let competing = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let accepted = session.classify_dependent(accepted).unwrap();
        let competing = session.classify_dependent(competing).unwrap();
        assert_eq!(accepted.source_ordinal(), competing.source_ordinal());
        assert_eq!(accepted.reduction_count(), competing.reduction_count());

        session
            .commit_dependent(&family, &context, accepted)
            .unwrap();
        let failure = session
            .commit_dependent(&family, &context, competing)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        let failure_debug = format!("{failure:?}");
        assert!(failure_debug.contains("private_classification: \"<redacted>\""));
        let recovered_classified = failure.into_classified().unwrap();
        assert_eq!(recovered_classified.source_ordinal(), 1);
        assert_eq!(recovered_classified.reduction_count(), 1);
        let recovered = recovered_classified.into_transaction();
        assert_eq!(session.database.state_version(), 2);
        assert_eq!(session.target_state.state_version(), 2);
        assert_eq!(session.database.pivot_count(), 1);
        session.replay(&family, &context).unwrap();

        // The original target-state allocation is stale and is returned
        // intact by classification. Replacing only that internal Arc proves
        // the independently sealed database transition is stale as well.
        let failure = session.classify_dependent(recovered).unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        let recovered = failure.into_transaction();
        let GeneratedAffineResidualGroupExactSessionStagedTransaction { staged, .. } = recovered;
        let forged_live_target = GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&session.target_state),
        };
        let failure = session.classify_dependent(forged_live_target).unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::StaleStagedRow
            )
        );
        let recovered = failure.into_transaction();

        // Finally pair the same untouched database stage with the foreign
        // session's live target allocation. Target authentication succeeds in
        // that owner, but the hidden database allocation rejects the stage.
        let GeneratedAffineResidualGroupExactSessionStagedTransaction { staged, .. } = recovered;
        let forged_foreign = GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&foreign.target_state),
        };
        let failure = foreign.classify_dependent(forged_foreign).unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::WrongDatabaseAllocation
            )
        );
        drop(failure.into_transaction());
        assert_eq!(foreign.database.state_version(), 0);
        assert_eq!(foreign.target_state.state_version(), 0);
        assert_eq!(foreign.database.pivot_count(), 0);
        foreign.replay(&family, &context).unwrap();
    }
}
