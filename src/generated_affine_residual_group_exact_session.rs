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
//! V1 deliberately exposes no successor-state, commit, recentering, rule
//! publication, or master-inference transition outside this module. A private
//! unconsumed-commit kernel proves the atomic database/target-state transition,
//! but future crate callers must reach it only through typed dependent,
//! no-target, equality-refinement, or rejected-`WhenBad` authorities. Dropping
//! an otherwise unconsumed staged transaction leaves both retained owners
//! unchanged.

use std::fmt;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

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
    ReplayMismatch,
    SymbolicaPanic,
}

impl GeneratedAffineResidualGroupExactSessionError {
    const fn kind(self) -> &'static str {
        match self {
            Self::Database(_) => "Database",
            Self::Target(_) => "Target",
            Self::WrongTargetStateAllocation => "WrongTargetStateAllocation",
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

/// One allocation-bound exact solve session.
///
/// Construction is the unique V1 minting path for the initial target state:
/// the database first creates an opaque, non-`Clone` binding, which is consumed
/// by the state owner and immediately authenticated back against that same
/// database allocation.
pub(crate) struct GeneratedAffineResidualGroupExactSession {
    schema: &'static str,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
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
            let binding = database.initial_target_state_binding()?;
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
        let staged = self.database.stage_replayed_row(
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
        let staged_pivot = self
            .database
            .authenticate_staged_new_pivot(&transaction.staged)?;
        let targets = transaction
            .target_state
            .authenticated_view(family, context)?;
        if !targets.authenticates_state_allocation(&self.target_state)
            || !Arc::ptr_eq(staged_pivot.plan(), &self.plan)
            || !Arc::ptr_eq(staged_pivot.frame(), self.plan.physical_frame())
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        Ok(GeneratedAffineResidualGroupExactSessionStagedNewPivotView {
            staged_pivot,
            targets,
        })
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
    /// public(crate) wrappers must require the corresponding sealed outcome.
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
        let outcome = match self.database.commit_staged_row(staged) {
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
        let successor_binding = self
            .database
            .successor_target_state_binding(&transaction.staged)?;
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
/// module. The private atomic kernel may consume it exactly once; a future
/// crate-visible transition must additionally require a sealed policy outcome.
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
        self.staged_pivot.plan().anchor_case_ordinal()
    }

    pub(crate) fn free_positions(&self) -> &[usize] {
        self.staged_pivot.plan().free_positions()
    }

    pub(crate) fn target_locators(&self) -> &[GeneratedAffineResidualGroupSolveTargetLocator] {
        self.staged_pivot.plan().targets()
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
            .field("target_count", &self.target_locators().len())
            .field("private_staged_pivot", &"<redacted>")
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
        let joint = first
            .authenticate_staged_new_pivot(&family, &context, &transaction)
            .unwrap();
        assert_eq!(joint.database_epoch(), 59);
        assert_eq!(joint.group_ordinal(), plan.group_ordinal());
        assert_eq!(joint.anchor_case_ordinal(), plan.anchor_case_ordinal());
        assert_eq!(joint.free_positions(), plan.free_positions());
        assert_eq!(joint.target_locators(), plan.targets());
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
    fn unconsumed_commit_advances_database_and_targets_atomically() {
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
        let outcome = session
            .commit_unconsumed(&family, &context, dependent)
            .unwrap();
        let GeneratedAffineResidualGroupExactRowOutcome::Dependent {
            source_ordinal,
            reductions,
        } = outcome
        else {
            panic!("the identical row must close against its retained exact pivot")
        };
        assert_eq!(source_ordinal, 1);
        assert_eq!(reductions.len(), 1);
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
}
