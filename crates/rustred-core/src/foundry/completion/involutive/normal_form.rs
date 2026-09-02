use std::sync::Arc;

use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::sector::ShiftComplexityKey;

use super::error::{check_limit, checked_add, checked_mul, try_push_bounded, try_vec};
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
    mut subject: OreConsequence,
    basis: &JanetBasisEpoch,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetNormalForm, InvolutiveError> {
    basis.require_ordering(ordering)?;
    subject.try_validate(ordering, context, limits)?;
    if excluded_divisor.is_some_and(|ordinal| ordinal >= basis.elements().len()) {
        return Err(InvolutiveError::InvalidProlongation {
            detail: "excluded Janet divisor is outside the current epoch",
        });
    }

    let mut steps = Vec::new();
    let mut divisor_visits = 0usize;
    let mut trace_bytes = 0usize;
    let mut previous_target: Option<ShiftComplexityKey> = None;

    loop {
        let Some(selected) = try_select_reduction(
            &subject,
            basis,
            excluded_divisor,
            ordering,
            limits,
            &mut divisor_visits,
            work,
        )?
        else {
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
        subject = subject.try_left_axpy_sealed(
            &multiplier,
            &operator_shift,
            divisor,
            ordering,
            context,
            limits,
            work,
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
    basis: &'a JanetBasisEpoch,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    limits: InvolutiveLimits,
    divisor_visits: &mut usize,
    work: &mut InvolutiveWorkBudget,
) -> Result<Option<SelectedReduction<'a>>, InvolutiveError> {
    let mut selected: Option<SelectedReduction<'a>> = None;
    for term in subject.row().terms() {
        let mut divisor = None;
        for element in basis.elements() {
            *divisor_visits = checked_add("Janet normal-form divisor visits", *divisor_visits, 1)?;
            work.charge_divisor_visit(limits)?;
            check_limit(
                "Janet normal-form divisor visits",
                *divisor_visits,
                limits.max_normal_form_divisor_visits,
            )?;
            if excluded_divisor == Some(element.ordinal()) {
                continue;
            }
            if element
                .multiplicative()
                .janet_divides(element.leading_shift(), term.shift())
            {
                divisor = Some(element);
                break;
            }
        }
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

/// Copy every exact row/provenance/guard payload through the bounded Ore
/// arithmetic boundary before attempting autoreduction.
pub(super) fn try_copy_basis_consequences(
    basis: &JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<Vec<OreConsequence>, InvolutiveError> {
    basis.require_ordering(ordering)?;
    let mut copied = try_vec("Janet autoreduction input rows", basis.elements().len())?;
    for element in basis.elements() {
        copied.push(
            element
                .consequence()
                .try_copy_sealed(ordering, context, limits, work)?,
        );
    }
    Ok(copied)
}
