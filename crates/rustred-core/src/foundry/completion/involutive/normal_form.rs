use std::sync::Arc;

use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::sector::ShiftComplexityKey;

use super::error::{check_limit, checked_add, checked_mul, try_push_bounded};
use super::janet::JanetDivisionEpoch;
use super::limits::InvolutiveWorkBudget;
use super::{
    ForwardShift, InvolutiveError, InvolutiveLimits, JanetBasisElement, JanetBasisEpoch,
    OreConsequence, OreOrderingAdapter,
};

/// One exact left-Ore cancellation in a deterministic Janet normal form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JanetReductionStep {
    divisor_ordinal: usize,
    target_shift: ForwardShift,
    operator_shift: ForwardShift,
    required_nonzero: Option<Arc<IndexedPolynomial>>,
}

impl JanetReductionStep {
    pub(crate) fn divisor_ordinal(&self) -> usize {
        self.divisor_ordinal
    }

    pub(crate) fn target_shift(&self) -> &ForwardShift {
        &self.target_shift
    }

    pub(crate) fn operator_shift(&self) -> &ForwardShift {
        &self.operator_shift
    }

    pub(crate) fn required_nonzero(&self) -> Option<&IndexedPolynomial> {
        self.required_nonzero.as_deref()
    }
}

/// Exact Janet remainder together with bounded proposal telemetry.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct JanetNormalForm {
    remainder: OreConsequence,
    steps: Box<[JanetReductionStep]>,
    divisor_visits: usize,
    trace_bytes: usize,
}

impl JanetNormalForm {
    pub(crate) fn remainder(&self) -> &OreConsequence {
        &self.remainder
    }

    pub(crate) fn steps(&self) -> &[JanetReductionStep] {
        &self.steps
    }

    pub(crate) fn divisor_visits(&self) -> usize {
        self.divisor_visits
    }

    pub(crate) fn trace_bytes(&self) -> usize {
        self.trace_bytes
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.remainder.is_zero()
    }

    pub(crate) fn into_remainder(self) -> OreConsequence {
        self.remainder
    }

    pub(super) fn into_parts(self) -> (OreConsequence, usize) {
        (self.remainder, self.steps.len())
    }
}

/// Autoreduction-specific ownership result from one frozen-epoch normal form.
///
/// An irreducible sealed row keeps its existing allocation. A row is copied
/// through the exact Ore boundary only after the borrowed indexed scan has
/// selected its first real cancellation.
pub(super) enum JanetAutoreductionNormalForm {
    Shared(Arc<OreConsequence>),
    Materialized(JanetNormalForm),
}

/// Compute a complete term-wise Janet normal form over the exact indexed
/// rational-function field.
///
/// Terms that have no Janet divisor remain in the row. At every iteration the
/// greatest *reducible* term is cancelled; admissibility of the frozen order
/// then makes those selected terms strictly decrease even when a larger
/// irreducible term remains in the eventual remainder.
pub(crate) fn try_janet_normal_form(
    subject: OreConsequence,
    basis: &JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> Result<JanetNormalForm, InvolutiveError> {
    let mut work = InvolutiveWorkBudget::default();
    try_janet_normal_form_with_budget(subject, basis, ordering, context, limits, &mut work)
}

pub(super) fn try_janet_normal_form_with_budget(
    subject: OreConsequence,
    basis: &JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetNormalForm, InvolutiveError> {
    try_janet_normal_form_excluding(subject, basis, None, ordering, context, limits, work)
}

pub(super) fn try_janet_normal_form_excluding(
    subject: OreConsequence,
    basis: &JanetBasisEpoch,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetNormalForm, InvolutiveError> {
    try_janet_normal_form_on_division_excluding(
        subject,
        basis.division(),
        excluded_divisor,
        ordering,
        context,
        limits,
        work,
    )
}

fn try_janet_normal_form_on_division_excluding(
    subject: OreConsequence,
    basis: &JanetDivisionEpoch,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetNormalForm, InvolutiveError> {
    validate_normal_form_request(&subject, basis, excluded_divisor, ordering, context, limits)?;
    let divisor_scratch = basis.try_divisor_scratch(limits)?;
    try_reduce_owned_normal_form(
        subject,
        basis,
        excluded_divisor,
        ordering,
        context,
        limits,
        work,
        None,
        divisor_scratch,
        0,
    )
}

/// Scan one sealed epoch row by reference and materialize it only if an exact
/// cancellation is actually available after applying the requested exclusion.
///
/// The first selection, its historical logical divisor visits, and the index
/// scratch state are passed directly into the ordinary reduction loop. They
/// are therefore neither queried nor charged a second time.
pub(super) fn try_janet_autoreduction_normal_form_on_division_excluding(
    subject: &Arc<OreConsequence>,
    basis: &JanetDivisionEpoch,
    excluded_divisor: usize,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetAutoreductionNormalForm, InvolutiveError> {
    let excluded_divisor = Some(excluded_divisor);
    validate_normal_form_request(
        subject.as_ref(),
        basis,
        excluded_divisor,
        ordering,
        context,
        limits,
    )?;
    let mut divisor_visits = 0usize;
    let mut divisor_scratch = basis.try_divisor_scratch(limits)?;
    let first = try_select_reduction(
        subject.as_ref(),
        basis,
        excluded_divisor,
        ordering,
        limits,
        &mut divisor_visits,
        &mut divisor_scratch,
        work,
    )?;
    let Some(first) = first else {
        work.charge_autoreduction_shared_row(limits)?;
        return Ok(JanetAutoreductionNormalForm::Shared(Arc::clone(subject)));
    };

    // Admit materialization before allocating or applying the identity AXPY,
    // so a tight cumulative cap cannot leave an unpublished partial row.
    work.charge_autoreduction_materialized_row(limits)?;
    let owned = subject.try_copy_sealed(ordering, context, limits, work)?;
    let normal_form = try_reduce_owned_normal_form(
        owned,
        basis,
        excluded_divisor,
        ordering,
        context,
        limits,
        work,
        Some(first),
        divisor_scratch,
        divisor_visits,
    )?;
    Ok(JanetAutoreductionNormalForm::Materialized(normal_form))
}

#[cfg(test)]
pub(super) fn try_janet_autoreduction_normal_form_excluding(
    subject: &Arc<OreConsequence>,
    basis: &JanetBasisEpoch,
    excluded_divisor: usize,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetAutoreductionNormalForm, InvolutiveError> {
    try_janet_autoreduction_normal_form_on_division_excluding(
        subject,
        basis.division(),
        excluded_divisor,
        ordering,
        context,
        limits,
        work,
    )
}

fn validate_normal_form_request(
    subject: &OreConsequence,
    basis: &JanetDivisionEpoch,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    basis.require_ordering(ordering)?;
    subject.try_validate(ordering, context, limits)?;
    if excluded_divisor.is_some_and(|ordinal| ordinal >= basis.elements().len()) {
        return Err(InvolutiveError::InvalidProlongation {
            detail: "excluded Janet divisor is outside the current epoch",
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn try_reduce_owned_normal_form<'basis>(
    mut subject: OreConsequence,
    basis: &'basis JanetDivisionEpoch,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
    mut first_selection: Option<SelectedReduction<'basis>>,
    mut divisor_scratch: super::divisor_index::JanetDivisorScratch,
    mut divisor_visits: usize,
) -> Result<JanetNormalForm, InvolutiveError> {
    let mut steps = Vec::new();
    let mut trace_bytes = 0usize;
    let mut previous_target: Option<ShiftComplexityKey> = None;

    loop {
        let selected = if let Some(selected) = first_selection.take() {
            Some(selected)
        } else {
            try_select_reduction(
                &subject,
                basis,
                excluded_divisor,
                ordering,
                limits,
                &mut divisor_visits,
                &mut divisor_scratch,
                work,
            )?
        };
        let Some(selected) = selected else {
            break;
        };
        if previous_target
            .as_ref()
            .is_some_and(|previous| selected.target_key >= *previous)
        {
            return Err(InvolutiveError::Invariant {
                detail: "Janet normal-form reduction target did not strictly decrease",
            });
        }

        let operator_shift = selected
            .target_shift
            .try_checked_sub(selected.divisor.leading_shift(), limits)?;
        work.charge_exact_coefficient_operations(1, limits)?;
        let divisor_coefficient = selected
            .divisor
            .consequence()
            .row()
            .coefficient(selected.divisor.leading_shift())
            .ok_or(InvolutiveError::Invariant {
                detail: "a Janet basis leader is absent from its own canonical row",
            })?;
        if divisor_coefficient != &context.one() {
            return Err(InvolutiveError::Invariant {
                detail: "a Janet basis retained a non-monic Ore leader",
            });
        }
        let multiplier = context.neg_bound_with_limits(
            context.bind_sealed(subject.row().coefficient(&selected.target_shift).ok_or(
                InvolutiveError::Invariant {
                    detail: "a selected Janet reduction term disappeared before cancellation",
                },
            )?)?,
            limits.indexed_algebra.exact_algebra,
        )?;

        let target_shift = selected.target_shift;
        let target_key = selected.target_key.clone();
        let divisor_ordinal = selected.divisor.ordinal();
        let divisor = selected.divisor.consequence();
        subject = super::with_coefficient_diagnostic_site!(
            NormalFormCancellation,
            subject.try_left_axpy_sealed(
                &multiplier,
                &operator_shift,
                divisor,
                ordering,
                context,
                limits,
                work,
            )
        )?;
        let step = JanetReductionStep {
            divisor_ordinal,
            target_shift,
            operator_shift,
            required_nonzero: None,
        };
        let step_bytes = step_retained_bytes(&step)?;
        trace_bytes = checked_add("Janet normal-form trace bytes", trace_bytes, step_bytes)?;
        work.charge_trace_bytes(step_bytes, limits)?;
        work.charge_normal_form_step(limits)?;
        check_limit(
            "Janet normal-form trace bytes",
            trace_bytes,
            limits.max_normal_form_trace_bytes,
        )?;
        try_push_bounded(
            &mut steps,
            step,
            "Janet normal-form steps",
            limits.max_normal_form_steps,
        )?;
        previous_target = Some(target_key);
    }

    subject.try_validate(ordering, context, limits)?;
    Ok(JanetNormalForm {
        remainder: subject,
        steps: steps.into_boxed_slice(),
        divisor_visits,
        trace_bytes,
    })
}

struct SelectedReduction<'a> {
    target_shift: ForwardShift,
    divisor: &'a JanetBasisElement,
    target_key: ShiftComplexityKey,
}

fn try_select_reduction<'a>(
    subject: &OreConsequence,
    basis: &'a JanetDivisionEpoch,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    limits: InvolutiveLimits,
    divisor_visits: &mut usize,
    divisor_scratch: &mut super::divisor_index::JanetDivisorScratch,
    work: &mut InvolutiveWorkBudget,
) -> Result<Option<SelectedReduction<'a>>, InvolutiveError> {
    let mut selected: Option<SelectedReduction<'a>> = None;
    for term in subject.row().terms() {
        let divisor = basis.try_janet_divisor_with_scratch(
            term.shift(),
            excluded_divisor,
            divisor_scratch,
            limits,
            work,
        )?;
        // Preserve the historical flat-scan work contract without performing
        // that scan: a hit at ordinal `o` visited `o + 1` rows, while a miss
        // visited the complete epoch (including an excluded row).
        let logical_visits = if let Some(ordinal) = divisor {
            checked_add("Janet normal-form divisor visits", ordinal, 1)?
        } else {
            basis.elements().len()
        };
        *divisor_visits = checked_add(
            "Janet normal-form divisor visits",
            *divisor_visits,
            logical_visits,
        )?;
        work.charge_divisor_visits(logical_visits, limits)?;
        check_limit(
            "Janet normal-form divisor visits",
            *divisor_visits,
            limits.max_normal_form_divisor_visits,
        )?;
        let divisor = if let Some(ordinal) = divisor {
            Some(
                basis
                    .elements()
                    .get(ordinal)
                    .ok_or(InvolutiveError::Invariant {
                        detail: "Janet divisor ordinal disappeared from its immutable epoch",
                    })?,
            )
        } else {
            None
        };
        let Some(divisor) = divisor else {
            continue;
        };
        let target_key = ordering.try_key(term.shift())?;
        if selected
            .as_ref()
            .is_none_or(|current| target_key > current.target_key)
        {
            selected = Some(SelectedReduction {
                target_shift: term.shift().clone(),
                divisor,
                target_key,
            });
        }
    }
    Ok(selected)
}

fn step_retained_bytes(step: &JanetReductionStep) -> Result<usize, InvolutiveError> {
    let shift_cells = checked_add(
        "Janet normal-form trace bytes",
        step.target_shift.arity(),
        step.operator_shift.arity(),
    )?;
    let shift_bytes = checked_mul(
        "Janet normal-form trace bytes",
        shift_cells,
        std::mem::size_of::<u64>(),
    )?;
    checked_add(
        "Janet normal-form trace bytes",
        std::mem::size_of::<JanetReductionStep>(),
        shift_bytes,
    )
}
