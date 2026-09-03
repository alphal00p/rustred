//! Atomic exact-support Janet cancellation over one frozen divisor epoch.
//!
//! Selection is coefficient-free and is repeated from the complete admitted
//! support on every step.  Coefficient circuits, source derivations, guards,
//! and the resulting exact-support row are then built in one exact-lazy
//! transaction.  A cursor is changed only after that transaction commits.

use crate::algebra::IndexedCoefficientContext;
use crate::sector::ShiftComplexityKey;

use super::super::super::divisor_index::JanetDivisorScratch;
use super::super::super::limits::InvolutiveWorkCensus;
use super::super::super::selection::{JanetReductionSelection, try_select_janet_reduction};
use super::super::super::{
    EpochId, ForwardShift, InvolutiveError, OreActionIdentity, OreOrderingAdapter,
};
use super::error::{check_limit, checked_add, try_vec};
use super::{
    ExactLazyCompletionLedger, ExactLazyCompletionLedgerId, ExactLazyConsequence, ExactLazyError,
    ExactLazyFrozenJanetEpoch, ExactLazyLimits, ExactLazyOwner, ExactLazyPayloadCensus,
    ExactLazyProbeSchedule, ExactLazySession, ExactLazySupportBudget, ExactLazyTransaction,
    ImportedGuardLineage, ImportedSourceDerivation, LazyCoeff, PendingLazyOreTerm,
    StructuralZeroProof, UnclassifiedLazyOreRow, try_classify_support,
};

const TRACE_STEPS: &str = "exact-lazy Janet cancellation trace steps";
const TRACE_BYTES: &str = "exact-lazy Janet cancellation trace bytes";
const AXPY_INPUT_TERMS: &str = "exact-lazy Janet cancellation AXPY input terms";
const OUTPUT_TERMS: &str = "exact-lazy Janet cancellation output terms";
const PROVENANCE_TERMS: &str = "exact-lazy Janet cancellation provenance terms";
const GUARD_DESCRIPTORS: &str = "exact-lazy Janet cancellation guard descriptors";

/// Observable result of one independently selected cancellation attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExactLazyCancellationOutcome {
    Irreducible,
    Reduced,
}

/// One committed cancellation witness.  It contains no coefficient payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExactLazyReductionStep {
    divisor_ordinal: usize,
    target_shift: ForwardShift,
    operator_shift: ForwardShift,
}

impl ExactLazyReductionStep {
    pub(super) const fn divisor_ordinal(&self) -> usize {
        self.divisor_ordinal
    }

    pub(super) fn target_shift(&self) -> &ForwardShift {
        &self.target_shift
    }

    pub(super) fn operator_shift(&self) -> &ForwardShift {
        &self.operator_shift
    }
}

/// Mutable normal-form state for one exact-lazy consequence.
///
/// The cursor stores the epoch identity rather than a caller-supplied target.
/// Every call independently chooses the greatest reducible supported term.
#[derive(Debug)]
pub(super) struct ExactLazyJanetCursor {
    epoch: EpochId,
    excluded_divisor: Option<usize>,
    ledger: ExactLazyCompletionLedgerId,
    subject: ExactLazyConsequence,
    previous_target: Option<ShiftComplexityKey>,
    divisor_scratch: JanetDivisorScratch,
    divisor_visits: usize,
    trace: Vec<ExactLazyReductionStep>,
    trace_bytes: usize,
    irreducible: bool,
    limits: ExactLazyLimits,
}

/// Opaque common payload of one finalized normal-form authority.
///
/// This is deliberately not exported: only the statically distinct full and
/// self-excluding wrappers below can cross the finalization seam.
#[derive(Debug)]
struct ExactLazyJanetNormalFormAuthority {
    owner: ExactLazyOwner,
    action: OreActionIdentity,
    epoch: EpochId,
    ledger: ExactLazyCompletionLedgerId,
    remainder: ExactLazyConsequence,
    steps: Box<[ExactLazyReductionStep]>,
    divisor_visits: usize,
    trace_bytes: usize,
    support_census: super::ExactLazySupportCensus,
    work_census: InvolutiveWorkCensus,
}

/// Complete normal form against every divisor in one immutable epoch.
///
/// This is not basis-admission authority.  It only records that the retained
/// consequence was irreducible without an excluded divisor under the sealed
/// owner/action/epoch/ledger binding.
#[derive(Debug)]
pub(super) struct ExactLazyFullJanetNormalForm {
    authority: ExactLazyJanetNormalFormAuthority,
}

/// Complete self-excluding autoreduction normal form over one immutable
/// epoch.  It cannot be converted to, or consumed where an unrestricted full
/// normal form is required.
#[derive(Debug)]
pub(super) struct ExactLazySelfExcludedJanetNormalForm {
    authority: ExactLazyJanetNormalFormAuthority,
    excluded_divisor: usize,
}

impl ExactLazyJanetNormalFormAuthority {
    #[allow(clippy::too_many_arguments)]
    fn require_binding(
        &self,
        session: &ExactLazySession<'_>,
        expected_epoch: &EpochId,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        ledger: &ExactLazyCompletionLedger,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        session.require_binding(ordering, context, limits)?;
        if !self.owner.belongs_to(session.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if !self.action.belongs_to(ordering.identity()) {
            return Err(ExactLazyError::WrongOreAction);
        }
        if &self.epoch != expected_epoch {
            return Err(InvolutiveError::StaleEpoch {
                expected: expected_epoch.clone(),
                actual: self.epoch.clone(),
            }
            .into());
        }
        ledger.require_binding(&self.ledger, session, ordering, context, limits)?;
        self.remainder.try_validate_live(session)
    }

    fn replace_remainder(
        &mut self,
        remainder: ExactLazyConsequence,
        ledger: &ExactLazyCompletionLedger,
    ) {
        self.remainder = remainder;
        self.support_census = ledger.support_census();
        self.work_census = ledger.work_census();
    }
}

macro_rules! normal_form_accessors {
    ($normal_form:ident) => {
        impl $normal_form {
            pub(super) fn remainder(&self) -> &ExactLazyConsequence {
                &self.authority.remainder
            }

            pub(super) fn steps(&self) -> &[ExactLazyReductionStep] {
                &self.authority.steps
            }

            pub(super) const fn divisor_visits(&self) -> usize {
                self.authority.divisor_visits
            }

            pub(super) const fn trace_bytes(&self) -> usize {
                self.authority.trace_bytes
            }

            pub(super) const fn support_census(&self) -> super::ExactLazySupportCensus {
                self.authority.support_census
            }

            pub(super) const fn work_census(&self) -> InvolutiveWorkCensus {
                self.authority.work_census
            }

            pub(super) fn epoch(&self) -> &EpochId {
                &self.authority.epoch
            }

            pub(super) fn ledger_id(&self) -> &ExactLazyCompletionLedgerId {
                &self.authority.ledger
            }

            #[allow(clippy::too_many_arguments)]
            pub(super) fn require_binding(
                &self,
                session: &ExactLazySession<'_>,
                expected_epoch: &EpochId,
                ordering: &OreOrderingAdapter,
                context: &IndexedCoefficientContext,
                ledger: &ExactLazyCompletionLedger,
                limits: ExactLazyLimits,
            ) -> Result<(), ExactLazyError> {
                self.authority.require_binding(
                    session,
                    expected_epoch,
                    ordering,
                    context,
                    ledger,
                    limits,
                )
            }
        }
    };
}

normal_form_accessors!(ExactLazyFullJanetNormalForm);
normal_form_accessors!(ExactLazySelfExcludedJanetNormalForm);

impl ExactLazyFullJanetNormalForm {
    pub(super) fn replace_with_normalized_remainder(
        &mut self,
        replacement: super::normalization::AuthenticatedMonicReplacement,
        ledger: &ExactLazyCompletionLedger,
    ) {
        self.authority
            .replace_remainder(replacement.into_consequence(), ledger);
    }
}

impl ExactLazySelfExcludedJanetNormalForm {
    pub(super) const fn excluded_divisor(&self) -> usize {
        self.excluded_divisor
    }

    pub(super) fn replace_with_normalized_remainder(
        &mut self,
        replacement: super::normalization::AuthenticatedMonicReplacement,
        ledger: &ExactLazyCompletionLedger,
    ) {
        self.authority
            .replace_remainder(replacement.into_consequence(), ledger);
    }
}

// Keep normal-form payload access confined to the typed wrappers. In
// particular there is intentionally no `into_remainder`: future insertion
// must consume a separate admission proof rather than discard this seal.

/// Compute one complete unrestricted exact-support Janet normal form using a
/// caller-owned cumulative campaign ledger.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_exact_lazy_full_janet_normal_form(
    session: &mut ExactLazySession<'_>,
    frozen: &ExactLazyFrozenJanetEpoch<'_>,
    subject: ExactLazyConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    schedule: &ExactLazyProbeSchedule,
    ledger: &mut ExactLazyCompletionLedger,
    limits: ExactLazyLimits,
) -> Result<ExactLazyFullJanetNormalForm, ExactLazyError> {
    let mut cursor = ExactLazyJanetCursor::try_new(
        session, frozen, subject, None, ordering, context, ledger, limits,
    )?;
    cursor.try_reduce_to_irreducible(session, frozen, ordering, context, schedule, ledger)?;
    cursor.try_into_full_normal_form(session, frozen, ordering, context, ledger)
}

/// Compute one complete self-excluding autoreduction normal form using the
/// same caller-owned cumulative campaign ledger.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_exact_lazy_self_excluded_janet_normal_form(
    session: &mut ExactLazySession<'_>,
    frozen: &ExactLazyFrozenJanetEpoch<'_>,
    subject: ExactLazyConsequence,
    excluded_divisor: usize,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    schedule: &ExactLazyProbeSchedule,
    ledger: &mut ExactLazyCompletionLedger,
    limits: ExactLazyLimits,
) -> Result<ExactLazySelfExcludedJanetNormalForm, ExactLazyError> {
    let mut cursor = ExactLazyJanetCursor::try_new(
        session,
        frozen,
        subject,
        Some(excluded_divisor),
        ordering,
        context,
        ledger,
        limits,
    )?;
    cursor.try_reduce_to_irreducible(session, frozen, ordering, context, schedule, ledger)?;
    cursor.try_into_self_excluded_normal_form(session, frozen, ordering, context, ledger)
}

// No self-allocating production convenience exists. Tests that want an
// isolated calculation must construct exactly one local ledger explicitly.

impl ExactLazyJanetCursor {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_new(
        session: &ExactLazySession<'_>,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        subject: ExactLazyConsequence,
        excluded_divisor: Option<usize>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        ledger: &ExactLazyCompletionLedger,
        limits: ExactLazyLimits,
    ) -> Result<Self, ExactLazyError> {
        session.require_binding(ordering, context, limits)?;
        frozen.division().require_ordering(ordering)?;
        require_same_owner(session, frozen, &subject)?;
        require_exclusion(excluded_divisor, frozen.len())?;
        ledger.require_binding(ledger.id(), session, ordering, context, limits)?;
        let divisor_scratch = frozen.division().try_divisor_scratch(limits.exact)?;
        Ok(Self {
            epoch: frozen.epoch().clone(),
            excluded_divisor,
            ledger: ledger.id().clone(),
            subject,
            previous_target: None,
            divisor_scratch,
            divisor_visits: 0,
            trace: Vec::new(),
            trace_bytes: 0,
            irreducible: false,
            limits,
        })
    }

    pub(super) fn subject(&self) -> &ExactLazyConsequence {
        &self.subject
    }

    pub(super) fn trace(&self) -> &[ExactLazyReductionStep] {
        &self.trace
    }

    pub(super) const fn trace_bytes(&self) -> usize {
        self.trace_bytes
    }

    pub(super) const fn divisor_visits(&self) -> usize {
        self.divisor_visits
    }

    pub(super) fn support_census(
        &self,
        ledger: &ExactLazyCompletionLedger,
    ) -> Result<super::ExactLazySupportCensus, ExactLazyError> {
        ledger.require_identity(&self.ledger)?;
        Ok(ledger.support_census())
    }

    pub(super) fn work_census(
        &self,
        ledger: &ExactLazyCompletionLedger,
    ) -> Result<InvolutiveWorkCensus, ExactLazyError> {
        ledger.require_identity(&self.ledger)?;
        Ok(ledger.work_census())
    }

    /// Cancel the greatest currently reducible term, if one exists.
    ///
    /// Selector and trace work are charged before coefficient mutation.  Any
    /// later error rolls the exact-lazy arenas back but deliberately does not
    /// refund the cursor-owned work or support-classification budgets.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_cancel_once(
        &mut self,
        session: &mut ExactLazySession<'_>,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        schedule: &ExactLazyProbeSchedule,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<ExactLazyCancellationOutcome, ExactLazyError> {
        self.require_request(session, frozen, ordering, context, schedule, ledger)?;
        let subject_terms = self.subject.row().try_terms_live(session)?;
        let selection = {
            let work = ledger.try_work_budget(&self.ledger)?;
            try_select_janet_reduction(
                frozen.division(),
                subject_terms.iter().map(|term| term.shift()),
                self.excluded_divisor,
                ordering,
                self.limits.exact,
                &mut self.divisor_visits,
                &mut self.divisor_scratch,
                work,
            )?
        };
        let Some(selection) = selection else {
            self.irreducible = true;
            return Ok(ExactLazyCancellationOutcome::Irreducible);
        };
        self.require_selection(frozen, &selection)?;

        let divisor = frozen.divisor(selection.divisor_ordinal())?;
        let prepared = prepare_cancellation(
            session,
            &self.subject,
            divisor,
            frozen,
            &selection,
            ordering,
            context,
            self.limits,
        )?;

        // The accounting ledger is monotone. Charge the admitted attempt
        // before any arena mutation so a later rollback cannot erase work.
        let next_trace_bytes = checked_add(TRACE_BYTES, self.trace_bytes, prepared.trace_bytes)?;
        {
            let work = ledger.try_work_budget(&self.ledger)?;
            work.charge_normal_form_step(self.limits.exact)?;
            work.charge_trace_bytes(prepared.trace_bytes, self.limits.exact)?;
        }
        check_limit(
            TRACE_BYTES,
            next_trace_bytes,
            self.limits.exact.max_normal_form_trace_bytes,
        )?;
        let next_steps = checked_add(TRACE_STEPS, self.trace.len(), 1)?;
        check_limit(
            TRACE_STEPS,
            next_steps,
            self.limits.exact.max_normal_form_steps,
        )?;
        self.trace
            .try_reserve(1)
            .map_err(|_| ExactLazyError::AllocationFailure {
                resource: TRACE_STEPS,
                requested: next_steps,
            })?;

        let support = ledger.try_support_budget(&self.ledger)?;
        let mut transaction = session.try_begin_transaction()?;
        let candidate = try_build_cancelled_consequence(
            &mut transaction,
            &self.subject,
            divisor,
            &prepared,
            ordering,
            context,
            schedule,
            support,
            self.limits,
        );
        let candidate = match candidate {
            Ok(candidate) => candidate,
            Err(error) => {
                transaction.try_abort()?;
                return Err(error);
            }
        };
        transaction.try_commit()?;

        // These operations cannot allocate (`trace` was reserved above) and
        // happen only after every dependent arena has committed.
        self.subject = candidate;
        self.previous_target = Some(prepared.target_key);
        self.trace_bytes = next_trace_bytes;
        self.trace.push(prepared.step);
        self.irreducible = false;
        Ok(ExactLazyCancellationOutcome::Reduced)
    }

    /// Drive this cursor to the first authenticated irreducible remainder.
    ///
    /// The cursor's opaque ledger identity forces every iteration, including
    /// a retry after failure, through the same caller-owned cumulative ledger.
    pub(super) fn try_reduce_to_irreducible(
        &mut self,
        session: &mut ExactLazySession<'_>,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        schedule: &ExactLazyProbeSchedule,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<(), ExactLazyError> {
        loop {
            match self.try_cancel_once(session, frozen, ordering, context, schedule, ledger)? {
                ExactLazyCancellationOutcome::Reduced => {}
                ExactLazyCancellationOutcome::Irreducible => return Ok(()),
            }
        }
    }

    /// Seal only an unrestricted cursor after its complete-support selector
    /// has certified that no Janet cancellation remains.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_into_full_normal_form(
        self,
        session: &ExactLazySession<'_>,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<ExactLazyFullJanetNormalForm, ExactLazyError> {
        self.require_finalization(session, frozen, ordering, context, ledger)?;
        if self.excluded_divisor.is_some() {
            return Err(ExactLazyError::WrongNormalFormMode {
                expected: "full",
                actual: "self-excluding",
            });
        }
        Ok(ExactLazyFullJanetNormalForm {
            authority: self.into_normal_form_authority(session, ordering, ledger),
        })
    }

    /// Seal only a self-excluding cursor.  The excluded ordinal remains an
    /// inseparable part of the returned authority.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_into_self_excluded_normal_form(
        self,
        session: &ExactLazySession<'_>,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<ExactLazySelfExcludedJanetNormalForm, ExactLazyError> {
        self.require_finalization(session, frozen, ordering, context, ledger)?;
        let excluded_divisor =
            self.excluded_divisor
                .ok_or(ExactLazyError::WrongNormalFormMode {
                    expected: "self-excluding",
                    actual: "full",
                })?;
        Ok(ExactLazySelfExcludedJanetNormalForm {
            authority: self.into_normal_form_authority(session, ordering, ledger),
            excluded_divisor,
        })
    }

    fn require_finalization(
        &self,
        session: &ExactLazySession<'_>,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        ledger: &ExactLazyCompletionLedger,
    ) -> Result<(), ExactLazyError> {
        self.require_environment(session, frozen, ordering, context, ledger)?;
        if !self.irreducible {
            return Err(ExactLazyError::InvalidSupport {
                detail: "an exact-lazy Janet cursor was finalized before irreducibility",
            });
        }
        Ok(())
    }

    fn into_normal_form_authority(
        self,
        session: &ExactLazySession<'_>,
        ordering: &OreOrderingAdapter,
        ledger: &ExactLazyCompletionLedger,
    ) -> ExactLazyJanetNormalFormAuthority {
        ExactLazyJanetNormalFormAuthority {
            owner: session.owner().clone(),
            action: ordering.identity().clone(),
            epoch: self.epoch,
            ledger: self.ledger,
            remainder: self.subject,
            steps: self.trace.into_boxed_slice(),
            divisor_visits: self.divisor_visits,
            trace_bytes: self.trace_bytes,
            support_census: ledger.support_census(),
            work_census: ledger.work_census(),
        }
    }

    fn require_request(
        &self,
        session: &ExactLazySession<'_>,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        schedule: &ExactLazyProbeSchedule,
        ledger: &ExactLazyCompletionLedger,
    ) -> Result<(), ExactLazyError> {
        self.require_environment(session, frozen, ordering, context, ledger)?;
        schedule.require_owner(session.owner())
    }

    fn require_environment(
        &self,
        session: &ExactLazySession<'_>,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        ledger: &ExactLazyCompletionLedger,
    ) -> Result<(), ExactLazyError> {
        session.require_binding(ordering, context, self.limits)?;
        frozen.division().require_ordering(ordering)?;
        require_same_owner(session, frozen, &self.subject)?;
        ledger.require_binding(&self.ledger, session, ordering, context, self.limits)?;
        require_exclusion(self.excluded_divisor, frozen.len())?;
        if &self.epoch != frozen.epoch() {
            return Err(InvolutiveError::StaleEpoch {
                expected: frozen.epoch().clone(),
                actual: self.epoch.clone(),
            }
            .into());
        }
        Ok(())
    }

    fn require_selection(
        &self,
        frozen: &ExactLazyFrozenJanetEpoch<'_>,
        selection: &JanetReductionSelection,
    ) -> Result<(), ExactLazyError> {
        if selection.epoch() != frozen.epoch() {
            return Err(InvolutiveError::StaleEpoch {
                expected: frozen.epoch().clone(),
                actual: selection.epoch().clone(),
            }
            .into());
        }
        if self
            .previous_target
            .as_ref()
            .is_some_and(|previous| selection.target_key() >= previous)
        {
            return Err(InvolutiveError::Invariant {
                detail: "exact-lazy Janet cancellation target did not strictly decrease",
            }
            .into());
        }
        Ok(())
    }
}

fn require_same_owner(
    session: &ExactLazySession<'_>,
    frozen: &ExactLazyFrozenJanetEpoch<'_>,
    subject: &ExactLazyConsequence,
) -> Result<(), ExactLazyError> {
    if !session.owner().belongs_to(frozen.owner()) || !session.owner().belongs_to(subject.owner()) {
        return Err(ExactLazyError::WrongSessionOwner);
    }
    subject.try_validate_live(session)
}

fn require_exclusion(excluded: Option<usize>, divisor_count: usize) -> Result<(), ExactLazyError> {
    if excluded.is_some_and(|ordinal| ordinal >= divisor_count) {
        return Err(InvolutiveError::InvalidProlongation {
            detail: "excluded exact-lazy Janet divisor is outside the frozen epoch",
        }
        .into());
    }
    Ok(())
}

struct PreparedCancellation {
    step: ExactLazyReductionStep,
    target_key: ShiftComplexityKey,
    translated_shifts: Box<[ForwardShift]>,
    trace_bytes: usize,
}

#[allow(clippy::too_many_arguments)]
fn prepare_cancellation(
    session: &ExactLazySession<'_>,
    subject: &ExactLazyConsequence,
    divisor: &ExactLazyConsequence,
    frozen: &ExactLazyFrozenJanetEpoch<'_>,
    selection: &JanetReductionSelection,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> Result<PreparedCancellation, ExactLazyError> {
    let exact_element = frozen
        .division()
        .elements()
        .get(selection.divisor_ordinal())
        .ok_or(InvolutiveError::Invariant {
            detail: "selected Janet divisor disappeared from its frozen epoch",
        })?;
    if exact_element.ordinal() != selection.divisor_ordinal()
        || exact_element.leading_shift() != selection.divisor_leading_shift()
    {
        return Err(InvolutiveError::Invariant {
            detail: "selected Janet divisor geometry changed inside its frozen epoch",
        }
        .into());
    }
    let exact_leader = exact_element
        .consequence()
        .row()
        .coefficient(exact_element.leading_shift())
        .ok_or(InvolutiveError::Invariant {
            detail: "a frozen Janet divisor is missing its exact leader",
        })?;
    if exact_leader != &context.one() {
        return Err(InvolutiveError::Invariant {
            detail: "a frozen Janet divisor has a non-monic exact leader",
        }
        .into());
    }

    let subject_terms = subject.row().try_terms_live(session)?;
    let divisor_terms = divisor.row().try_terms_live(session)?;
    let lazy_leading = divisor.row().try_leading_term(session, ordering)?.ok_or(
        ExactLazyError::InvalidSupport {
            detail: "a frozen exact-lazy Janet divisor has empty support",
        },
    )?;
    if lazy_leading.shift() != exact_element.leading_shift() {
        return Err(ExactLazyError::InvalidSupport {
            detail: "exact and exact-lazy frozen Janet leaders disagree",
        });
    }
    let operator_shift = selection
        .target_shift()
        .try_checked_sub(exact_element.leading_shift(), limits.exact)?;

    let input_terms = checked_add(AXPY_INPUT_TERMS, subject_terms.len(), divisor_terms.len())?;
    check_limit(
        AXPY_INPUT_TERMS,
        input_terms,
        limits.exact.max_axpy_input_terms,
    )?;
    let mut translated = try_vec(
        "exact-lazy translated Janet divisor support",
        divisor_terms.len(),
    )?;
    let mut previous_shift: Option<&ForwardShift> = None;
    let mut leader_count = 0usize;
    for term in divisor_terms {
        let shift = term
            .shift()
            .try_checked_add(&operator_shift, limits.exact)?;
        if previous_shift.is_some_and(|previous| previous >= &shift) {
            return Err(ExactLazyError::InvalidSupport {
                detail: "translated frozen divisor support is not strictly shift sorted",
            });
        }
        if term.shift() == exact_element.leading_shift() {
            leader_count = checked_add("exact-lazy frozen divisor leaders", leader_count, 1)?;
            if &shift != selection.target_shift() {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "translated frozen divisor leader does not equal selected target",
                });
            }
        } else if ordering.try_key(&shift)? >= *selection.target_key() {
            return Err(ExactLazyError::InvalidSupport {
                detail: "translated frozen divisor tail does not strictly descend from target",
            });
        }
        translated.push(shift);
        previous_shift = translated.last();
    }
    if leader_count != 1 {
        return Err(ExactLazyError::InvalidSupport {
            detail: "frozen exact-lazy Janet divisor does not have one unique leader",
        });
    }
    if subject_terms
        .binary_search_by(|term| term.shift().cmp(selection.target_shift()))
        .is_err()
    {
        return Err(ExactLazyError::InvalidSupport {
            detail: "selected exact-lazy cancellation target disappeared before mutation",
        });
    }

    let step = ExactLazyReductionStep {
        divisor_ordinal: selection.divisor_ordinal(),
        target_shift: selection.target_shift().clone(),
        operator_shift,
    };
    let trace_bytes = step_retained_bytes(&step)?;
    Ok(PreparedCancellation {
        step,
        target_key: selection.target_key().clone(),
        translated_shifts: translated.into_boxed_slice(),
        trace_bytes,
    })
}

#[allow(clippy::too_many_arguments)]
fn try_build_cancelled_consequence(
    transaction: &mut ExactLazyTransaction<'_, '_>,
    subject: &ExactLazyConsequence,
    divisor: &ExactLazyConsequence,
    prepared: &PreparedCancellation,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    schedule: &ExactLazyProbeSchedule,
    support_budget: &mut ExactLazySupportBudget,
    limits: ExactLazyLimits,
) -> Result<ExactLazyConsequence, ExactLazyError> {
    let subject_terms = subject.row().try_terms_in_transaction(transaction)?;
    let divisor_terms = divisor.row().try_terms_in_transaction(transaction)?;
    let divisor_leader_shift = prepared
        .step
        .target_shift()
        .try_checked_sub(prepared.step.operator_shift(), limits.exact)?;
    let lazy_leader = divisor_terms
        .iter()
        .find(|term| term.shift() == &divisor_leader_shift)
        .ok_or(ExactLazyError::InvalidSupport {
            detail: "frozen exact-lazy Janet leader disappeared before cancellation",
        })?;
    if lazy_leader.coefficient() != &transaction.one() {
        return Err(ExactLazyError::InvalidSupport {
            detail: "frozen exact-lazy Janet divisor leader is not structural one",
        });
    }

    let target_index = subject_terms
        .binary_search_by(|term| term.shift().cmp(prepared.step.target_shift()))
        .map_err(|_| ExactLazyError::InvalidSupport {
            detail: "selected exact-lazy cancellation target disappeared inside transaction",
        })?;
    let target_coefficient = subject_terms[target_index].coefficient().clone();
    let multiplier = transaction.try_neg(&target_coefficient)?;

    if prepared.translated_shifts.len() != divisor_terms.len() {
        return Err(ExactLazyError::InvalidSupport {
            detail: "prepared translated divisor support changed before cancellation",
        });
    }
    let mut transformed = try_vec(
        "exact-lazy transformed Janet divisor terms",
        divisor_terms.len(),
    )?;
    for (term, shift) in divisor_terms.iter().zip(prepared.translated_shifts.iter()) {
        let translated = transaction.try_translate_by_operator(
            term.coefficient(),
            prepared.step.operator_shift(),
            ordering,
        )?;
        let coefficient = transaction.try_mul(&multiplier, &translated)?;
        transformed.push((shift.clone(), coefficient));
    }

    let (pending, zeros, target_structural_zero) = try_sparse_merge(
        transaction,
        subject_terms,
        transformed,
        prepared.step.target_shift(),
        limits,
    )?;
    if !target_structural_zero {
        return Err(ExactLazyError::InvalidSupport {
            detail: "selected Janet target did not become structural zero",
        });
    }
    let unclassified = UnclassifiedLazyOreRow::try_new(transaction, pending, zeros)?;

    let derivation_root = transaction.try_left_axpy_derivation(
        subject.derivation().root(),
        &multiplier,
        prepared.step.operator_shift(),
        divisor.derivation().root(),
    )?;
    let expected_provenance_count = checked_add(
        PROVENANCE_TERMS,
        subject.derivation().source_term_count(),
        divisor.derivation().source_term_count(),
    )?;
    let derivation = ImportedSourceDerivation::try_from_lineage(transaction, derivation_root)?;
    if derivation.source_term_count() != expected_provenance_count {
        return Err(ExactLazyError::InvalidSupport {
            detail: "left-AXPY derivation lost its logical source occurrence count",
        });
    }

    let translated_guards = transaction
        .try_translate_guards(divisor.guards().root(), prepared.step.operator_shift())?;
    let joined_guards =
        transaction.try_union_guards(subject.guards().root(), &translated_guards)?;
    let multiplier_denominator = transaction.try_denominator_guard(&multiplier)?;
    let guard_root = transaction.try_union_guards(&joined_guards, &multiplier_denominator)?;
    let expected_guard_count = checked_add(
        GUARD_DESCRIPTORS,
        checked_add(
            GUARD_DESCRIPTORS,
            subject.guards().descriptor_count(),
            divisor.guards().descriptor_count(),
        )?,
        1,
    )?;
    let guards = ImportedGuardLineage::try_from_lineage(transaction, guard_root)?;
    if guards.descriptor_count() != expected_guard_count {
        return Err(ExactLazyError::InvalidSupport {
            detail: "guard union lost its logical descriptor occurrence count",
        });
    }
    let guard_requirements =
        transaction.try_collect_guard_probe_requirements(guards.root(), ordering)?;

    let row = try_classify_support(
        transaction,
        context,
        &guard_requirements,
        unclassified,
        schedule,
        support_budget,
    )?;
    let census = ExactLazyPayloadCensus::new(
        row.physical_term_count(),
        expected_provenance_count,
        expected_guard_count,
    );
    ExactLazyConsequence::try_new(transaction, row, derivation, guards, census)
}

fn try_sparse_merge(
    transaction: &mut ExactLazyTransaction<'_, '_>,
    subject_terms: &[super::LazyOreTerm],
    transformed: Vec<(ForwardShift, LazyCoeff)>,
    target: &ForwardShift,
    limits: ExactLazyLimits,
) -> Result<(Vec<PendingLazyOreTerm>, Vec<StructuralZeroProof>, bool), ExactLazyError> {
    let capacity = checked_add(OUTPUT_TERMS, subject_terms.len(), transformed.len())?
        .min(limits.exact.max_row_terms);
    let mut pending = try_vec(OUTPUT_TERMS, capacity)?;
    let zero_capacity = checked_add(OUTPUT_TERMS, subject_terms.len(), transformed.len())?;
    let mut zeros = try_vec(
        "exact-lazy Janet cancellation structural-zero elisions",
        zero_capacity,
    )?;
    let mut left = 0usize;
    let mut right = 0usize;
    let mut target_structural_zero = false;
    while left < subject_terms.len() || right < transformed.len() {
        match (subject_terms.get(left), transformed.get(right)) {
            (Some(subject_term), Some((divisor_shift, divisor_coefficient))) => {
                match subject_term.shift().cmp(divisor_shift) {
                    std::cmp::Ordering::Less => {
                        try_push_pending(
                            &mut pending,
                            PendingLazyOreTerm::from_unchanged(
                                subject_term.shift().clone(),
                                subject_term.coefficient().clone(),
                                subject_term.nonzero_proof().clone(),
                            ),
                            limits,
                        )?;
                        left += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        retain_changed_or_zero(
                            transaction,
                            &mut pending,
                            &mut zeros,
                            divisor_shift.clone(),
                            divisor_coefficient.clone(),
                            limits,
                        )?;
                        right += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        let coefficient =
                            transaction.try_add(subject_term.coefficient(), divisor_coefficient)?;
                        if transaction.try_is_structural_zero(&coefficient)? {
                            zeros.push(StructuralZeroProof::try_new(
                                transaction,
                                subject_term.shift().clone(),
                                &coefficient,
                            )?);
                            if subject_term.shift() == target {
                                target_structural_zero = true;
                            }
                        } else {
                            try_push_pending(
                                &mut pending,
                                PendingLazyOreTerm::from_changed(
                                    subject_term.shift().clone(),
                                    coefficient,
                                ),
                                limits,
                            )?;
                        }
                        left += 1;
                        right += 1;
                    }
                }
            }
            (Some(subject_term), None) => {
                try_push_pending(
                    &mut pending,
                    PendingLazyOreTerm::from_unchanged(
                        subject_term.shift().clone(),
                        subject_term.coefficient().clone(),
                        subject_term.nonzero_proof().clone(),
                    ),
                    limits,
                )?;
                left += 1;
            }
            (None, Some((divisor_shift, divisor_coefficient))) => {
                retain_changed_or_zero(
                    transaction,
                    &mut pending,
                    &mut zeros,
                    divisor_shift.clone(),
                    divisor_coefficient.clone(),
                    limits,
                )?;
                right += 1;
            }
            (None, None) => break,
        }
    }
    Ok((pending, zeros, target_structural_zero))
}

fn retain_changed_or_zero(
    transaction: &ExactLazyTransaction<'_, '_>,
    pending: &mut Vec<PendingLazyOreTerm>,
    zeros: &mut Vec<StructuralZeroProof>,
    shift: ForwardShift,
    coefficient: LazyCoeff,
    limits: ExactLazyLimits,
) -> Result<(), ExactLazyError> {
    if transaction.try_is_structural_zero(&coefficient)? {
        zeros.push(StructuralZeroProof::try_new(
            transaction,
            shift,
            &coefficient,
        )?);
    } else {
        try_push_pending(
            pending,
            PendingLazyOreTerm::from_changed(shift, coefficient),
            limits,
        )?;
    }
    Ok(())
}

fn try_push_pending(
    pending: &mut Vec<PendingLazyOreTerm>,
    term: PendingLazyOreTerm,
    limits: ExactLazyLimits,
) -> Result<(), ExactLazyError> {
    let requested = checked_add(OUTPUT_TERMS, pending.len(), 1)?;
    check_limit(OUTPUT_TERMS, requested, limits.exact.max_row_terms)?;
    pending.push(term);
    Ok(())
}

fn step_retained_bytes(step: &ExactLazyReductionStep) -> Result<usize, ExactLazyError> {
    let shift_cells = checked_add(
        TRACE_BYTES,
        step.target_shift.arity(),
        step.operator_shift.arity(),
    )?;
    let shift_bytes = shift_cells.checked_mul(std::mem::size_of::<u64>()).ok_or(
        ExactLazyError::ResourceCountOverflow {
            resource: TRACE_BYTES,
        },
    )?;
    checked_add(
        TRACE_BYTES,
        std::mem::size_of::<ExactLazyReductionStep>(),
        shift_bytes,
    )
}
