//! Cold, independently authenticated lowering of an exact-lazy consequence.
//!
//! This is deliberately not a hot-path conversion or an artifact boundary.
//! It expands the compact source derivation under a separate cumulative
//! budget, asks the existing ELC0 Symbolica materializer for one complete
//! physical/provenance/guard batch, and rebuilds the answer from the sealed
//! ordinary-source chronology.  No unchecked `OreConsequence` constructor is
//! involved.

use std::collections::HashMap;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::super::super::limits::InvolutiveWorkBudget;
use super::super::super::{
    ForwardShift, LocalizationDomainBudget, LocalizationDomainCensus, LocalizationDomainLimits,
    LocalizationWitness, OrdinaryChartLiftLimits, OreConsequence, OreOrderingAdapter, OreRow,
    try_lift_completed_ordinary_sources,
};
use super::super::{
    ExactMaterializationBudget, ExactMaterializationCensus, ExactMaterializerLimits,
    try_materialize_exact_batch,
};
use super::error::{check_limit, checked_add, try_vec};
use super::provenance::{SourceDerivationNodeView, SourceDerivationRef};
use super::{
    ExactLazyConsequence, ExactLazyError, ExactLazyOwner, ExactLazySession, ExactLazyTransaction,
    GuardProbeRequirement, LazyCoeff,
};

const LOWERING_ATTEMPTS: &str = "exact-lazy cold-lowering attempts";
const SUCCESSFUL_LOWERINGS: &str = "exact-lazy successful cold lowerings";
const DERIVATION_VISITS: &str = "exact-lazy lowering derivation visits";
const DERIVATION_FRAME_PUSHES: &str = "exact-lazy lowering derivation frame pushes";
const LIVE_DERIVATION_FRAMES: &str = "exact-lazy lowering live derivation frames";
const PROVENANCE_MERGES: &str = "exact-lazy lowering provenance merges";
const PROVENANCE_ENTRIES: &str = "exact-lazy lowering provenance entries";
const PROVENANCE_COORDINATE_CELLS: &str = "exact-lazy lowering provenance coordinate cells";
const GUARD_REQUIREMENTS: &str = "exact-lazy lowering guard requirements";
const MATERIALIZATION_ROOTS: &str = "exact-lazy lowering materialization roots";
const NESTED_MATERIALIZATION_ATTEMPTS: &str = "exact-lazy lowering nested materializer attempts";
const NESTED_MATERIALIZATION_BATCH_ROOTS: &str =
    "exact-lazy lowering nested materializer batch roots";
const NESTED_MATERIALIZATION_OUTPUTS: &str =
    "exact-lazy lowering nested materializer output values";
const NESTED_MATERIALIZATION_DELTAS: &str =
    "exact-lazy lowering nested materializer initial deltas";
const NESTED_MATERIALIZATION_DELTA_CELLS: &str =
    "exact-lazy lowering nested materializer initial delta cells";

/// Independent resource policy for the deliberately cold exact boundary.
///
/// The nested chart-lift policy must carry the exact same involutive limits as
/// the owning exact-lazy session.  Materialization limits are separate from
/// support fallback so a final differential cannot silently spend the hot
/// classifier's budget or reset it after a failed attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExactLazyLoweringLimits {
    pub(super) materialization: ExactMaterializerLimits,
    pub(super) chart_lift: OrdinaryChartLiftLimits,
    pub(super) max_attempts: usize,
    pub(super) max_successful_lowerings: usize,
    pub(super) max_derivation_visits: usize,
    pub(super) max_derivation_frame_pushes: usize,
    pub(super) max_live_derivation_frames: usize,
    pub(super) max_provenance_merges: usize,
    pub(super) max_provenance_entries: usize,
    pub(super) max_provenance_coordinate_cells: usize,
    pub(super) max_guard_requirements: usize,
    pub(super) max_materialization_roots: usize,
    pub(super) localization_domain: LocalizationDomainLimits,
}

impl ExactLazyLoweringLimits {
    pub(super) fn for_session(session: &ExactLazySession<'_>) -> Self {
        let mut chart_lift = OrdinaryChartLiftLimits::default();
        chart_lift.involutive = session.limits().exact;
        Self {
            materialization: ExactMaterializerLimits::default(),
            chart_lift,
            max_attempts: 1_000_000,
            max_successful_lowerings: 1_000_000,
            max_derivation_visits: 100_000_000,
            max_derivation_frame_pushes: 200_000_000,
            max_live_derivation_frames: 16_000_000,
            max_provenance_merges: 100_000_000,
            max_provenance_entries: 16_000_000,
            max_provenance_coordinate_cells: 1_000_000_000,
            max_guard_requirements: 16_000_000,
            max_materialization_roots: 32_000_000,
            localization_domain: LocalizationDomainLimits::default(),
        }
    }
}

/// Monotone telemetry for one caller-owned cold-lowering campaign.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ExactLazyLoweringCensus {
    attempts: usize,
    successful_lowerings: usize,
    derivation_visits: usize,
    derivation_frame_pushes: usize,
    peak_live_derivation_frames: usize,
    provenance_merges: usize,
    peak_provenance_entries: usize,
    provenance_coordinate_cells: usize,
    guard_requirements: usize,
    materialization_roots: usize,
}

impl ExactLazyLoweringCensus {
    pub(super) const fn attempts(self) -> usize {
        self.attempts
    }

    pub(super) const fn successful_lowerings(self) -> usize {
        self.successful_lowerings
    }

    pub(super) const fn derivation_visits(self) -> usize {
        self.derivation_visits
    }

    pub(super) const fn derivation_frame_pushes(self) -> usize {
        self.derivation_frame_pushes
    }

    pub(super) const fn peak_live_derivation_frames(self) -> usize {
        self.peak_live_derivation_frames
    }

    pub(super) const fn provenance_merges(self) -> usize {
        self.provenance_merges
    }

    pub(super) const fn peak_provenance_entries(self) -> usize {
        self.peak_provenance_entries
    }

    pub(super) const fn provenance_coordinate_cells(self) -> usize {
        self.provenance_coordinate_cells
    }

    pub(super) const fn guard_requirements(self) -> usize {
        self.guard_requirements
    }

    pub(super) const fn materialization_roots(self) -> usize {
        self.materialization_roots
    }
}

/// Caller-owned, owner-bound accounting for all exact lowering attempts.
#[derive(Debug)]
pub(super) struct ExactLazyLoweringBudget {
    owner: ExactLazyOwner,
    limits: ExactLazyLoweringLimits,
    census: ExactLazyLoweringCensus,
    materialization: ExactMaterializationBudget,
    replay_work: InvolutiveWorkBudget,
    localization_domain: LocalizationDomainBudget,
}

impl ExactLazyLoweringBudget {
    pub(super) fn try_new(
        session: &ExactLazySession<'_>,
        limits: ExactLazyLoweringLimits,
    ) -> Result<Self, ExactLazyError> {
        if limits.chart_lift.involutive != session.limits().exact {
            return Err(ExactLazyError::WrongLimitsContract);
        }
        Ok(Self {
            owner: session.owner().clone(),
            limits,
            census: ExactLazyLoweringCensus::default(),
            materialization: ExactMaterializationBudget::new(limits.materialization),
            replay_work: InvolutiveWorkBudget::default(),
            localization_domain: LocalizationDomainBudget::new(limits.localization_domain),
        })
    }

    pub(super) const fn census(&self) -> ExactLazyLoweringCensus {
        self.census
    }

    pub(super) const fn materialization_census(&self) -> ExactMaterializationCensus {
        self.materialization.census()
    }

    pub(super) fn replay_work_census(&self) -> super::super::super::InvolutiveWorkCensus {
        self.replay_work.census()
    }

    pub(super) const fn localization_domain_census(&self) -> LocalizationDomainCensus {
        self.localization_domain.census()
    }

    fn require_owner(&self, owner: &ExactLazyOwner) -> Result<(), ExactLazyError> {
        if self.owner.belongs_to(owner) && self.limits.chart_lift.involutive == owner.limits().exact
        {
            Ok(())
        } else {
            Err(ExactLazyError::WrongLimitsContract)
        }
    }

    fn try_start_attempt(&mut self, owner: &ExactLazyOwner) -> Result<(), ExactLazyError> {
        self.require_owner(owner)?;
        charge(
            LOWERING_ATTEMPTS,
            &mut self.census.attempts,
            1,
            self.limits.max_attempts,
        )
    }

    fn try_preflight_success(&self) -> Result<usize, ExactLazyError> {
        let successes = checked_add(SUCCESSFUL_LOWERINGS, self.census.successful_lowerings, 1)?;
        check_limit(
            SUCCESSFUL_LOWERINGS,
            successes,
            self.limits.max_successful_lowerings,
        )?;
        Ok(successes)
    }

    fn finish_success(&mut self, admitted_successes: usize) {
        debug_assert_eq!(
            admitted_successes,
            self.census.successful_lowerings.saturating_add(1)
        );
        self.census.successful_lowerings = admitted_successes;
    }

    fn try_charge_visit(&mut self) -> Result<(), ExactLazyError> {
        charge(
            DERIVATION_VISITS,
            &mut self.census.derivation_visits,
            1,
            self.limits.max_derivation_visits,
        )
    }

    fn try_charge_frame_pushes(&mut self, amount: usize) -> Result<(), ExactLazyError> {
        charge(
            DERIVATION_FRAME_PUSHES,
            &mut self.census.derivation_frame_pushes,
            amount,
            self.limits.max_derivation_frame_pushes,
        )
    }

    fn try_observe_live_frames(&mut self, value: usize) -> Result<(), ExactLazyError> {
        self.census.peak_live_derivation_frames =
            self.census.peak_live_derivation_frames.max(value);
        check_limit(
            LIVE_DERIVATION_FRAMES,
            value,
            self.limits.max_live_derivation_frames,
        )
    }

    fn try_charge_provenance_merge(&mut self) -> Result<(), ExactLazyError> {
        charge(
            PROVENANCE_MERGES,
            &mut self.census.provenance_merges,
            1,
            self.limits.max_provenance_merges,
        )
    }

    fn try_observe_provenance_entries(&mut self, value: usize) -> Result<(), ExactLazyError> {
        self.census.peak_provenance_entries = self.census.peak_provenance_entries.max(value);
        check_limit(
            PROVENANCE_ENTRIES,
            value,
            self.limits.max_provenance_entries,
        )
    }

    fn try_charge_provenance_cells(&mut self, amount: usize) -> Result<(), ExactLazyError> {
        charge(
            PROVENANCE_COORDINATE_CELLS,
            &mut self.census.provenance_coordinate_cells,
            amount,
            self.limits.max_provenance_coordinate_cells,
        )
    }

    fn try_charge_guard_requirements(&mut self, amount: usize) -> Result<(), ExactLazyError> {
        charge(
            GUARD_REQUIREMENTS,
            &mut self.census.guard_requirements,
            amount,
            self.limits.max_guard_requirements,
        )
    }

    fn try_preflight_guard_requirements(&self, amount: usize) -> Result<(), ExactLazyError> {
        let requested = checked_add(GUARD_REQUIREMENTS, self.census.guard_requirements, amount)?;
        check_limit(
            GUARD_REQUIREMENTS,
            requested,
            self.limits.max_guard_requirements,
        )
    }

    fn try_charge_materialization_roots(&mut self, amount: usize) -> Result<(), ExactLazyError> {
        charge(
            MATERIALIZATION_ROOTS,
            &mut self.census.materialization_roots,
            amount,
            self.limits.max_materialization_roots,
        )
    }

    fn try_preflight_materialization_roots(&self, amount: usize) -> Result<(), ExactLazyError> {
        let requested = checked_add(
            MATERIALIZATION_ROOTS,
            self.census.materialization_roots,
            amount,
        )?;
        check_limit(
            MATERIALIZATION_ROOTS,
            requested,
            self.limits.max_materialization_roots,
        )
    }

    /// Reject predictable nested-materializer cap failures before lineage
    /// expansion allocates any coefficient nodes or traversal buffers.
    fn try_preflight_nested_materializer_start(
        &self,
        index_count: usize,
        minimum_roots: usize,
    ) -> Result<(), ExactLazyError> {
        let limits = self.limits.materialization;
        let attempts = checked_add(
            NESTED_MATERIALIZATION_ATTEMPTS,
            self.materialization.attempts(),
            1,
        )?;
        check_limit(
            NESTED_MATERIALIZATION_ATTEMPTS,
            attempts,
            limits.max_attempts,
        )?;
        check_limit(
            NESTED_MATERIALIZATION_BATCH_ROOTS,
            minimum_roots,
            limits.max_batch_roots,
        )?;
        let outputs = checked_add(
            NESTED_MATERIALIZATION_OUTPUTS,
            self.materialization.census().output_values(),
            minimum_roots,
        )?;
        check_limit(
            NESTED_MATERIALIZATION_OUTPUTS,
            outputs,
            limits.max_output_values,
        )?;
        check_limit(
            NESTED_MATERIALIZATION_DELTAS,
            1,
            limits.max_accumulated_deltas,
        )?;
        check_limit(
            NESTED_MATERIALIZATION_DELTA_CELLS,
            index_count,
            limits.max_accumulated_delta_coordinate_cells,
        )
    }

    fn try_preflight_nested_materializer_batch(
        &self,
        exact_roots: usize,
    ) -> Result<(), ExactLazyError> {
        let limits = self.limits.materialization;
        check_limit(
            NESTED_MATERIALIZATION_BATCH_ROOTS,
            exact_roots,
            limits.max_batch_roots,
        )?;
        let outputs = checked_add(
            NESTED_MATERIALIZATION_OUTPUTS,
            self.materialization.census().output_values(),
            exact_roots,
        )?;
        check_limit(
            NESTED_MATERIALIZATION_OUTPUTS,
            outputs,
            limits.max_output_values,
        )
    }
}

/// Exact result that crossed both the lazy materialization and sealed-source
/// replay checks.  It confers no Janet basis, completion, or artifact
/// authority.
#[derive(Debug)]
pub(super) struct AuthenticatedLoweredConsequence {
    owner: ExactLazyOwner,
    exact: OreConsequence,
}

impl AuthenticatedLoweredConsequence {
    pub(super) fn consequence(&self) -> &OreConsequence {
        &self.exact
    }

    pub(super) fn into_consequence(self) -> OreConsequence {
        self.exact
    }

    pub(super) fn belongs_to(&self, owner: &ExactLazyOwner) -> bool {
        self.owner.belongs_to(owner)
    }
}

/// Materialize, replay, and authenticate one exact-lazy consequence.
///
/// Temporary coefficient roots live in one rollback-only transaction.  Both
/// success and failure restore the prior committed floor; cumulative arena and
/// lowering work remain charged.
pub(super) fn try_lower_for_exact_replay(
    session: &mut ExactLazySession<'_>,
    consequence: &ExactLazyConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    budget: &mut ExactLazyLoweringBudget,
) -> Result<AuthenticatedLoweredConsequence, ExactLazyError> {
    let session_limits = session.limits();
    session.require_binding(ordering, context, session_limits)?;
    if !consequence.owner().belongs_to(session.owner()) {
        return Err(ExactLazyError::WrongSessionOwner);
    }
    consequence.try_validate_live(session)?;
    budget.try_start_attempt(session.owner())?;
    let admitted_successes = budget.try_preflight_success()?;

    let mut transaction = session.try_begin_transaction()?;
    let lowered = try_lower_in_transaction(
        &mut transaction,
        consequence,
        ordering,
        context,
        budget,
        session_limits.exact,
    );
    match lowered {
        Ok(exact) => {
            transaction.try_abort()?;
            budget.finish_success(admitted_successes);
            Ok(AuthenticatedLoweredConsequence {
                owner: consequence.owner().clone(),
                exact,
            })
        }
        Err(error) => {
            transaction.try_abort()?;
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn try_lower_in_transaction(
    transaction: &mut ExactLazyTransaction<'_, '_>,
    consequence: &ExactLazyConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    budget: &mut ExactLazyLoweringBudget,
    exact_limits: super::super::super::InvolutiveLimits,
) -> Result<OreConsequence, ExactLazyError> {
    let physical_terms = consequence.row().try_terms_in_transaction(transaction)?;
    transaction.require_derivation(consequence.derivation().root())?;
    transaction.require_guard_lineage(consequence.guards().root())?;

    // The logical derivation and guard counts are conservative upper bounds
    // on the canonical flattened payload.  Check the complete root envelope
    // before traversal can allocate translated coefficient nodes or guard
    // collection can append anything.  The exact post-dedup count is charged
    // below once known.
    let descriptor_envelope = consequence.guards().descriptor_count();
    budget.try_preflight_guard_requirements(descriptor_envelope)?;
    let root_envelope = checked_add(
        MATERIALIZATION_ROOTS,
        checked_add(
            MATERIALIZATION_ROOTS,
            physical_terms.len(),
            consequence.derivation().source_term_count(),
        )?,
        descriptor_envelope,
    )?;
    budget.try_preflight_materialization_roots(root_envelope)?;
    // Physical roots are an exact lower bound before lineage expansion; the
    // deduplicated provenance/guard contribution is not known yet and must
    // not be replaced by a conservative rejection against a tighter nested
    // batch cap.
    budget.try_preflight_nested_materializer_start(context.index_count(), physical_terms.len())?;

    let mut physical_shifts = try_vec("exact-lazy lowered physical shifts", physical_terms.len())?;
    let mut physical_roots = try_vec(
        "exact-lazy physical coefficient roots",
        physical_terms.len(),
    )?;
    for term in physical_terms {
        physical_shifts.push(term.shift().clone());
        physical_roots.push(term.coefficient().root().clone());
    }
    let physical_count = physical_roots.len();

    let provenance = try_expand_derivation(
        transaction,
        consequence.derivation().root(),
        ordering,
        budget,
        exact_limits,
    )?;
    let guard_requirements =
        transaction.try_collect_guard_probe_requirements(consequence.guards().root(), ordering)?;
    budget.try_charge_guard_requirements(guard_requirements.len())?;
    let root_count = checked_add(
        MATERIALIZATION_ROOTS,
        checked_add(MATERIALIZATION_ROOTS, physical_count, provenance.len())?,
        guard_requirements.len(),
    )?;
    budget.try_preflight_nested_materializer_batch(root_count)?;
    budget.try_charge_materialization_roots(root_count)?;
    let mut roots = try_vec("exact-lazy lowering materialization roots", root_count)?;
    roots.extend(physical_roots);
    for term in &provenance {
        roots.push(term.coefficient.root().clone());
    }
    let provenance_end = roots.len();
    for requirement in &guard_requirements {
        match requirement {
            GuardProbeRequirement::Nonzero(root) | GuardProbeRequirement::Defined(root) => {
                roots.push(root.root().clone());
            }
        }
    }
    debug_assert_eq!(roots.len(), root_count);

    let exact_batch = try_materialize_exact_batch(
        transaction.coefficient_dag(),
        context,
        &roots,
        &mut budget.materialization,
    )?;
    if !exact_batch.owns(transaction.coefficient_dag(), context, &roots) {
        return Err(ExactLazyError::InvalidProof {
            detail: "cold-lowering materialization batch lost its root binding",
        });
    }
    let materialized = exact_batch.into_materializations();

    let mut exact_physical = try_vec("exact-lazy exact physical row", physical_count)?;
    for (shift, value) in physical_shifts
        .into_iter()
        .zip(&materialized[..physical_count])
    {
        exact_physical.push((shift, value.value().clone()));
    }
    let exact_physical = OreRow::try_new(ordering, exact_physical, context, exact_limits)?;
    if exact_physical.terms().len() != physical_count {
        return Err(ExactLazyError::InvalidProof {
            detail: "classified exact-lazy physical support changed during exact lowering",
        });
    }

    let mut exact_provenance = try_vec(
        "exact-lazy exact provenance entries",
        provenance_end - physical_count,
    )?;
    for (term, value) in provenance
        .into_iter()
        .zip(&materialized[physical_count..provenance_end])
    {
        if !value.value().is_zero() {
            exact_provenance.push(ExactProvenanceTerm {
                source_ordinal: term.source_ordinal,
                left_shift: term.left_shift,
                left_coefficient: value.value().clone(),
            });
        }
    }

    let guard_offset = provenance_end;
    let mut exact_guards = try_vec(
        "exact-lazy exact guard polynomials",
        guard_requirements.len(),
    )?;
    for (requirement, value) in guard_requirements.iter().zip(&materialized[guard_offset..]) {
        let guard = match requirement {
            GuardProbeRequirement::Nonzero(_) => context.numerator_condition_with_limits(
                value.value(),
                exact_limits.indexed_algebra.exact_algebra,
            )?,
            GuardProbeRequirement::Defined(_) => context.denominator_condition_with_limits(
                value.value(),
                exact_limits.indexed_algebra.exact_algebra,
            )?,
        };
        exact_guards.push(guard);
    }
    let lazy_localization = LocalizationWitness::default().try_merge_polynomials(
        exact_guards,
        context,
        exact_limits,
    )?;

    let lifted = try_lift_completed_ordinary_sources(
        transaction.completed_sources(),
        ordering,
        context,
        budget.limits.chart_lift,
    )
    .map_err(|_| ExactLazyError::InvalidProof {
        detail: "sealed ordinary sources could not be replayed in the exact Ore chart",
    })?;
    let mut replayed = OreConsequence::try_zero(ordering, context, exact_limits)?;
    for term in &exact_provenance {
        let source =
            lifted
                .sources()
                .get(term.source_ordinal)
                .ok_or(ExactLazyError::InvalidProof {
                    detail: "lowered provenance names a missing sealed source ordinal",
                })?;
        let residual = term
            .left_shift
            .try_checked_sub(source.left_shift(), exact_limits)
            .map_err(|_| ExactLazyError::InvalidProof {
                detail: "lowered provenance lies below its source's minimal chart lift",
            })?;
        replayed = replayed.try_left_axpy_with_budget(
            &term.left_coefficient,
            &residual,
            source.consequence(),
            ordering,
            context,
            exact_limits,
            &mut budget.replay_work,
        )?;
    }

    if replayed.row() != &exact_physical {
        return Err(ExactLazyError::InvalidProof {
            detail: "materialized exact-lazy row disagrees with complete sealed-source replay",
        });
    }
    require_exact_provenance(&exact_provenance, replayed.provenance())?;

    // Replay may reconstruct a reducible product where compact lazy lineage
    // retains its translated factors separately. Prove, without changing the
    // hot witness representation, that every replay-required factor is present
    // in the lazy principal-open radical. The proof-bound Ore seam then keeps
    // the lazy witness itself, including any conservative historic guards.
    replayed = replayed.try_restrict_to_authenticated_localization(
        lazy_localization,
        context,
        exact_limits,
        &mut budget.localization_domain,
    )?;
    replayed.try_validate(ordering, context, exact_limits)?;
    Ok(replayed)
}

#[derive(Debug)]
struct DerivationFrame {
    derivation: SourceDerivationRef,
    outer_coefficient: LazyCoeff,
    outer_shift: ForwardShift,
}

#[derive(Debug)]
struct LazyProvenanceTerm {
    source_ordinal: usize,
    left_shift: ForwardShift,
    coefficient: LazyCoeff,
}

#[derive(Debug)]
struct ExactProvenanceTerm {
    source_ordinal: usize,
    left_shift: ForwardShift,
    left_coefficient: IndexedCoefficient,
}

fn try_expand_derivation(
    transaction: &mut ExactLazyTransaction<'_, '_>,
    root: &SourceDerivationRef,
    ordering: &OreOrderingAdapter,
    budget: &mut ExactLazyLoweringBudget,
    exact_limits: super::super::super::InvolutiveLimits,
) -> Result<Vec<LazyProvenanceTerm>, ExactLazyError> {
    transaction.require_derivation(root)?;
    let zero_shift = ForwardShift::try_zero(transaction.owner().arity(), exact_limits)?;
    budget.try_charge_frame_pushes(1)?;
    budget.try_observe_live_frames(1)?;
    let mut stack = try_vec("exact-lazy lowering derivation frames", 1)?;
    stack.push(DerivationFrame {
        derivation: root.clone(),
        outer_coefficient: transaction.one(),
        outer_shift: zero_shift,
    });
    debug_assert_eq!(stack.len(), 1);

    let mut entries: HashMap<(usize, ForwardShift), LazyCoeff> = HashMap::new();
    while let Some(frame) = stack.pop() {
        budget.try_charge_visit()?;
        if transaction.try_is_structural_zero(&frame.outer_coefficient)? {
            continue;
        }
        match transaction.try_derivation_node_view(&frame.derivation)? {
            SourceDerivationNodeView::Zero => {}
            SourceDerivationNodeView::Source { source_ordinal } => {
                transaction.require_source_ordinal(source_ordinal)?;
                budget.try_charge_provenance_merge()?;
                let key = (source_ordinal, frame.outer_shift);
                if let Some(existing) = entries.get(&key).cloned() {
                    let merged = transaction.try_add(&existing, &frame.outer_coefficient)?;
                    if transaction.try_is_structural_zero(&merged)? {
                        entries.remove(&key);
                    } else {
                        entries.insert(key, merged);
                    }
                } else {
                    let requested = checked_add(PROVENANCE_ENTRIES, entries.len(), 1)?;
                    budget.try_observe_provenance_entries(requested)?;
                    budget.try_charge_provenance_cells(transaction.owner().arity())?;
                    entries
                        .try_reserve(1)
                        .map_err(|_| ExactLazyError::AllocationFailure {
                            resource: PROVENANCE_ENTRIES,
                            requested,
                        })?;
                    entries.insert(key, frame.outer_coefficient);
                }
            }
            SourceDerivationNodeView::Translate { shift, child } => {
                let outer_shift = frame.outer_shift.try_checked_add(&shift, exact_limits)?;
                try_push_derivation_frame(
                    &mut stack,
                    DerivationFrame {
                        derivation: child,
                        outer_coefficient: frame.outer_coefficient,
                        outer_shift,
                    },
                    budget,
                )?;
            }
            SourceDerivationNodeView::Axpy {
                target,
                multiplier,
                source,
            } => {
                let translated_multiplier = transaction.try_translate_by_operator(
                    &multiplier,
                    &frame.outer_shift,
                    ordering,
                )?;
                let source_coefficient =
                    transaction.try_mul(&frame.outer_coefficient, &translated_multiplier)?;
                // Push source first so the target branch is visited first.
                try_push_derivation_frame(
                    &mut stack,
                    DerivationFrame {
                        derivation: source,
                        outer_coefficient: source_coefficient,
                        outer_shift: frame.outer_shift.clone(),
                    },
                    budget,
                )?;
                try_push_derivation_frame(
                    &mut stack,
                    DerivationFrame {
                        derivation: target,
                        outer_coefficient: frame.outer_coefficient,
                        outer_shift: frame.outer_shift,
                    },
                    budget,
                )?;
            }
            SourceDerivationNodeView::LeftAxpy {
                target,
                multiplier,
                operator_shift,
                source,
            } => {
                // c E^alpha (target + m E^delta source)
                // = c E^alpha target
                //   + c sigma_alpha(m) E^(alpha+delta) source.
                let translated_multiplier = transaction.try_translate_by_operator(
                    &multiplier,
                    &frame.outer_shift,
                    ordering,
                )?;
                let source_coefficient =
                    transaction.try_mul(&frame.outer_coefficient, &translated_multiplier)?;
                let source_shift = frame
                    .outer_shift
                    .try_checked_add(&operator_shift, exact_limits)?;
                try_push_derivation_frame(
                    &mut stack,
                    DerivationFrame {
                        derivation: source,
                        outer_coefficient: source_coefficient,
                        outer_shift: source_shift,
                    },
                    budget,
                )?;
                try_push_derivation_frame(
                    &mut stack,
                    DerivationFrame {
                        derivation: target,
                        outer_coefficient: frame.outer_coefficient,
                        outer_shift: frame.outer_shift,
                    },
                    budget,
                )?;
            }
        }
    }

    let mut terms = try_vec("canonical exact-lazy provenance roots", entries.len())?;
    for ((source_ordinal, left_shift), coefficient) in entries {
        terms.push(LazyProvenanceTerm {
            source_ordinal,
            left_shift,
            coefficient,
        });
    }
    terms.sort_unstable_by(|left, right| {
        left.source_ordinal
            .cmp(&right.source_ordinal)
            .then_with(|| left.left_shift.cmp(&right.left_shift))
    });
    Ok(terms)
}

fn try_push_derivation_frame(
    stack: &mut Vec<DerivationFrame>,
    frame: DerivationFrame,
    budget: &mut ExactLazyLoweringBudget,
) -> Result<(), ExactLazyError> {
    budget.try_charge_frame_pushes(1)?;
    let requested = checked_add(LIVE_DERIVATION_FRAMES, stack.len(), 1)?;
    budget.try_observe_live_frames(requested)?;
    stack
        .try_reserve(1)
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource: LIVE_DERIVATION_FRAMES,
            requested,
        })?;
    stack.push(frame);
    Ok(())
}

fn require_exact_provenance(
    expected: &[ExactProvenanceTerm],
    actual: &super::super::super::ConsequenceProvenance,
) -> Result<(), ExactLazyError> {
    if expected.len() != actual.terms().len()
        || expected.iter().zip(actual.terms()).any(|(left, right)| {
            left.source_ordinal != right.source_ordinal()
                || &left.left_shift != right.left_shift()
                || &left.left_coefficient != right.left_coefficient()
        })
    {
        Err(ExactLazyError::InvalidProof {
            detail: "materialized provenance disagrees with complete sealed-source replay",
        })
    } else {
        Ok(())
    }
}

fn charge(
    resource: &'static str,
    value: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), ExactLazyError> {
    *value = checked_add(resource, *value, amount)?;
    check_limit(resource, *value, limit)
}
