use crate::algebra::IndexedCoefficientContext;

use super::super::CompletionGeometryLimits;
use super::error::checked_add;
use super::initial::try_preprocess_initial_basis_with_budget;
use super::limits::{InvolutiveWorkBudget, InvolutiveWorkCensus};
use super::normal_form::{
    try_copy_basis_consequences, try_janet_normal_form_excluding, try_janet_normal_form_with_budget,
};
use super::{
    BlindDomainSchedule, InvolutiveError, InvolutiveLimits, JanetBasisEpoch,
    JanetInitialReductionCensus, LocalizationWitness, OreConsequence, OreOrderingAdapter,
};

/// Bounded telemetry from one deterministic Janet autoreduction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetAutoreductionCensus {
    passes: usize,
    normal_form_steps: usize,
    dropped_rows: usize,
}

impl JanetAutoreductionCensus {
    pub(crate) const fn passes(self) -> usize {
        self.passes
    }

    pub(crate) const fn normal_form_steps(self) -> usize {
        self.normal_form_steps
    }

    pub(crate) const fn dropped_rows(self) -> usize {
        self.dropped_rows
    }

    fn try_accumulate(&mut self, right: Self) -> Result<(), InvolutiveError> {
        self.passes = checked_add("Janet autoreduction census", self.passes, right.passes)?;
        self.normal_form_steps = checked_add(
            "Janet autoreduction census",
            self.normal_form_steps,
            right.normal_form_steps,
        )?;
        self.dropped_rows = checked_add(
            "Janet autoreduction census",
            self.dropped_rows,
            right.dropped_rows,
        )?;
        Ok(())
    }
}

/// An immutable autoreduced epoch and the work used to obtain it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct JanetAutoreduction {
    epoch: JanetBasisEpoch,
    census: JanetAutoreductionCensus,
    localization: LocalizationWitness,
    work: InvolutiveWorkCensus,
}

impl JanetAutoreduction {
    pub(crate) fn epoch(&self) -> &JanetBasisEpoch {
        &self.epoch
    }

    pub(crate) const fn census(&self) -> JanetAutoreductionCensus {
        self.census
    }

    pub(crate) fn localization_witness(&self) -> &LocalizationWitness {
        &self.localization
    }

    pub(crate) const fn work_census(&self) -> InvolutiveWorkCensus {
        self.work
    }

    pub(crate) fn into_epoch(self) -> JanetBasisEpoch {
        self.epoch
    }

    fn into_parts(
        self,
    ) -> (
        JanetBasisEpoch,
        JanetAutoreductionCensus,
        LocalizationWitness,
    ) {
        (self.epoch, self.census, self.localization)
    }
}

/// Bounded telemetry from an involutive proposal calculation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetCompletionCensus {
    initial_reduction: JanetInitialReductionCensus,
    attempted_prolongations: usize,
    zero_remainders: usize,
    inserted_remainders: usize,
    truncated_blind_priority_epochs: usize,
    autoreduction: JanetAutoreductionCensus,
}

impl JanetCompletionCensus {
    pub(crate) const fn initial_reduction(self) -> JanetInitialReductionCensus {
        self.initial_reduction
    }

    pub(crate) const fn attempted_prolongations(self) -> usize {
        self.attempted_prolongations
    }

    pub(crate) const fn zero_remainders(self) -> usize {
        self.zero_remainders
    }

    pub(crate) const fn inserted_remainders(self) -> usize {
        self.inserted_remainders
    }

    pub(crate) const fn truncated_blind_priority_epochs(self) -> usize {
        self.truncated_blind_priority_epochs
    }

    pub(crate) const fn autoreduction(self) -> JanetAutoreductionCensus {
        self.autoreduction
    }
}

/// Proposal-only fixed point of bounded Janet prolongation and reduction.
///
/// Exhausting this queue proves only localized involutive completion of the
/// lifted forward Ore module under the frozen action and the returned
/// [`LocalizationWitness`]. It is never an unconditional certificate and does
/// not admit a reduction rule, authenticate a regenerated ordinary source, or
/// publish an artifact.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct JanetCompletionProposal {
    epoch: JanetBasisEpoch,
    census: JanetCompletionCensus,
    localization: LocalizationWitness,
    work: InvolutiveWorkCensus,
}

impl JanetCompletionProposal {
    pub(crate) fn epoch(&self) -> &JanetBasisEpoch {
        &self.epoch
    }

    pub(crate) const fn census(&self) -> JanetCompletionCensus {
        self.census
    }

    /// Canonical conjunction under which every retained row and every discarded
    /// zero normal-form proof in this proposal is valid.
    pub(crate) fn localization_witness(&self) -> &LocalizationWitness {
        &self.localization
    }

    pub(crate) const fn work_census(&self) -> InvolutiveWorkCensus {
        self.work
    }

    pub(crate) fn into_epoch(self) -> JanetBasisEpoch {
        self.epoch
    }
}

/// Deterministically autoreduce every row against the other rows of the same
/// frozen epoch, rebuilding masks and obligations after every changed pass.
///
/// Each pass is synchronous: all remainders are computed against one
/// immutable epoch. That makes results independent of allocation order and
/// prevents a partially rebuilt basis from influencing later rows.
pub(crate) fn try_autoreduce_epoch(
    epoch: JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    geometry_limits: CompletionGeometryLimits,
) -> Result<JanetAutoreduction, InvolutiveError> {
    let mut work = InvolutiveWorkBudget::default();
    let mut result = try_autoreduce_epoch_with_budget(
        epoch,
        ordering,
        context,
        limits,
        geometry_limits,
        &mut work,
    )?;
    result.work = work.census();
    Ok(result)
}

fn try_autoreduce_epoch_with_budget(
    mut epoch: JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    geometry_limits: CompletionGeometryLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetAutoreduction, InvolutiveError> {
    epoch.require_ordering(ordering)?;
    let mut census = JanetAutoreductionCensus::default();
    let mut localization = epoch_localization_witness(&epoch, limits)?;
    loop {
        let requested_passes = checked_add("Janet autoreduction passes", census.passes, 1)?;
        #[cfg(test)]
        super::diagnostics::record_autoreduction_pass(&epoch, requested_passes, work.census());
        work.charge_autoreduction_pass(limits)?;

        let copied = try_copy_basis_consequences(&epoch, ordering, context, limits, work)?;
        let mut replacements = Vec::new();
        replacements.try_reserve_exact(copied.len()).map_err(|_| {
            InvolutiveError::AllocationFailure {
                resource: "Janet autoreduction output rows",
                requested: copied.len(),
            }
        })?;
        let mut changed = false;
        let mut pass_steps = 0usize;
        let mut pass_dropped = 0usize;
        for (ordinal, original) in copied.into_iter().enumerate() {
            let normal_form = try_janet_normal_form_excluding(
                original,
                &epoch,
                Some(ordinal),
                ordering,
                context,
                limits,
                work,
            )?;
            let (remainder, steps) = normal_form.into_parts();
            localization = localization.try_union(remainder.localization_witness(), limits)?;
            pass_steps = checked_add("Janet autoreduction census", pass_steps, steps)?;
            if remainder.is_zero() {
                pass_dropped = checked_add("Janet autoreduction census", pass_dropped, 1)?;
                changed = true;
                continue;
            }
            if &remainder != epoch.elements()[ordinal].consequence() {
                changed = true;
            }
            replacements.push(remainder);
        }

        census.passes = requested_passes;
        census.normal_form_steps = checked_add(
            "Janet autoreduction census",
            census.normal_form_steps,
            pass_steps,
        )?;
        census.dropped_rows = checked_add(
            "Janet autoreduction census",
            census.dropped_rows,
            pass_dropped,
        )?;
        if !changed {
            return Ok(JanetAutoreduction {
                epoch,
                census,
                localization,
                work: InvolutiveWorkCensus::default(),
            });
        }
        epoch = epoch.try_replacement_successor(
            replacements,
            ordering,
            context,
            limits,
            geometry_limits,
            work,
        )?;
    }
}

/// Run a bounded, deterministic prolongation-to-remainder fixed point.
///
/// Every epoch obtains a fresh exact blind-domain schedule. Truncating the
/// retained diagnostic boxes can only change priority: the returned ordinal
/// permutation still visits every mandatory Janet obligation. A nonzero
/// remainder enters a new immutable epoch, is autoreduced, and invalidates the
/// entire previous queue before work resumes.
pub(crate) fn try_complete_janet_proposal(
    initial: JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    geometry_limits: CompletionGeometryLimits,
) -> Result<JanetCompletionProposal, InvolutiveError> {
    let mut work = InvolutiveWorkBudget::default();
    try_complete_janet_proposal_with_budget(
        initial,
        LocalizationWitness::default(),
        JanetInitialReductionCensus::default(),
        ordering,
        context,
        limits,
        geometry_limits,
        &mut work,
    )
}

/// Deterministically row-reduce coincident initial heads, then autoreduce and
/// complete the resulting Janet basis under one cumulative work ledger.
pub(crate) fn try_complete_janet_proposal_from_consequences(
    consequences: Vec<OreConsequence>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    geometry_limits: CompletionGeometryLimits,
) -> Result<JanetCompletionProposal, InvolutiveError> {
    let mut work = InvolutiveWorkBudget::default();
    let initial = try_preprocess_initial_basis_with_budget(
        consequences,
        ordering,
        context,
        limits,
        geometry_limits,
        &mut work,
    )?;
    let (epoch, localization, census) = initial.into_parts();
    #[cfg(test)]
    super::diagnostics::record_initial_basis(census, &epoch, work.census());
    try_complete_janet_proposal_with_budget(
        epoch,
        localization,
        census,
        ordering,
        context,
        limits,
        geometry_limits,
        &mut work,
    )
}

fn try_complete_janet_proposal_with_budget(
    initial: JanetBasisEpoch,
    initial_localization: LocalizationWitness,
    initial_reduction: JanetInitialReductionCensus,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    geometry_limits: CompletionGeometryLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetCompletionProposal, InvolutiveError> {
    let autoreduced = try_autoreduce_epoch_with_budget(
        initial,
        ordering,
        context,
        limits,
        geometry_limits,
        work,
    )?;
    let (mut epoch, initial_autoreduction, autoreduction_localization) = autoreduced.into_parts();
    let mut localization = initial_localization.try_union(&autoreduction_localization, limits)?;
    let mut census = JanetCompletionCensus {
        initial_reduction,
        autoreduction: initial_autoreduction,
        ..JanetCompletionCensus::default()
    };

    #[cfg(test)]
    super::diagnostics::record_completion_epoch(&epoch, work.census());

    loop {
        epoch.require_ordering(ordering)?;
        let blind =
            BlindDomainSchedule::try_from_partition(epoch.uncovered_partition(), ordering, limits)?;
        if blind.is_truncated() {
            census.truncated_blind_priority_epochs = checked_add(
                "Janet completion census",
                census.truncated_blind_priority_epochs,
                1,
            )?;
        }
        let priority = blind.try_rank_prolongation_ordinals(&epoch, ordering, limits)?;
        let mut inserted = None;
        for ordinal in priority.into_vec() {
            let requested_iterations = checked_add(
                "Janet completion iterations",
                census.attempted_prolongations,
                1,
            )?;
            work.charge_completion_iteration(limits)?;
            let prolongation =
                epoch
                    .prolongations()
                    .get(ordinal)
                    .ok_or(InvolutiveError::Invariant {
                        detail: "blind-domain priority returned an invalid obligation ordinal",
                    })?;
            let subject = epoch.try_apply_prolongation_with_budget(
                prolongation,
                ordering,
                context,
                limits,
                work,
            )?;
            let normal_form = try_janet_normal_form_with_budget(
                subject, &epoch, ordering, context, limits, work,
            )?;
            census.attempted_prolongations = requested_iterations;
            if normal_form.is_zero() {
                localization = localization
                    .try_union(normal_form.remainder().localization_witness(), limits)?;
                census.zero_remainders =
                    checked_add("Janet completion census", census.zero_remainders, 1)?;
                continue;
            }
            census.inserted_remainders =
                checked_add("Janet completion census", census.inserted_remainders, 1)?;
            inserted = Some(normal_form.into_remainder());
            break;
        }

        let Some(remainder) = inserted else {
            localization =
                localization.try_union(&epoch_localization_witness(&epoch, limits)?, limits)?;
            return Ok(JanetCompletionProposal {
                epoch,
                census,
                localization,
                work: work.census(),
            });
        };
        let successor = epoch.try_successor_with_budget(
            [remainder],
            ordering,
            context,
            limits,
            geometry_limits,
            work,
        )?;
        #[cfg(test)]
        super::diagnostics::record_completion_autoreduction(&successor, work.census());
        let autoreduced = try_autoreduce_epoch_with_budget(
            successor,
            ordering,
            context,
            limits,
            geometry_limits,
            work,
        )?;
        let (next, autoreduction, autoreduction_localization) = autoreduced.into_parts();
        localization = localization.try_union(&autoreduction_localization, limits)?;
        census.autoreduction.try_accumulate(autoreduction)?;
        epoch = next;
        #[cfg(test)]
        super::diagnostics::record_completion_epoch(&epoch, work.census());
    }
}

fn epoch_localization_witness(
    epoch: &JanetBasisEpoch,
    limits: InvolutiveLimits,
) -> Result<LocalizationWitness, InvolutiveError> {
    let mut localization = LocalizationWitness::default();
    for element in epoch.elements() {
        localization =
            localization.try_union(element.consequence().localization_witness(), limits)?;
    }
    Ok(localization)
}
