//! Coefficient-free deterministic Janet reduction selection.
//!
//! Exact expanded rows and exact-support circuit rows deliberately share this
//! boundary.  Selection depends only on authenticated support and immutable
//! Janet geometry; coefficient representation must not be able to change the
//! chosen target or divisor.

use crate::sector::ShiftComplexityKey;

use super::divisor_index::JanetDivisorScratch;
use super::error::checked_add;
use super::janet::JanetDivisionGeometry;
use super::limits::InvolutiveWorkBudget;
use super::{EpochId, ForwardShift, InvolutiveError, InvolutiveLimits, OreOrderingAdapter};

/// One independently selected reduction witness in a frozen Janet epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct JanetReductionSelection {
    epoch: EpochId,
    target_shift: ForwardShift,
    target_key: ShiftComplexityKey,
    divisor_ordinal: usize,
    divisor_leading_shift: ForwardShift,
}

impl JanetReductionSelection {
    pub(super) fn epoch(&self) -> &EpochId {
        &self.epoch
    }

    pub(super) fn target_shift(&self) -> &ForwardShift {
        &self.target_shift
    }

    pub(super) fn target_key(&self) -> &ShiftComplexityKey {
        &self.target_key
    }

    pub(super) const fn divisor_ordinal(&self) -> usize {
        self.divisor_ordinal
    }

    pub(super) fn divisor_leading_shift(&self) -> &ForwardShift {
        &self.divisor_leading_shift
    }
}

/// Select the greatest exactly supported term having a Janet divisor.
///
/// The divisor index preserves the historical lowest-ordinal divisor rule.
/// `divisor_visits` preserves the old logical flat-scan census even though the
/// physical lookup uses coordinate postings.  Both are intentionally shared
/// by expanded and lazy normal forms.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_select_janet_reduction<'support>(
    basis: &(impl JanetDivisionGeometry + ?Sized),
    support: impl IntoIterator<Item = &'support ForwardShift>,
    excluded_divisor: Option<usize>,
    ordering: &OreOrderingAdapter,
    limits: InvolutiveLimits,
    divisor_visits: &mut usize,
    divisor_scratch: &mut JanetDivisorScratch,
    work: &mut InvolutiveWorkBudget,
) -> Result<Option<JanetReductionSelection>, InvolutiveError> {
    basis.require_geometry_ordering(ordering)?;
    basis.require_geometry_query_environment(excluded_divisor, divisor_scratch)?;
    let mut selected: Option<JanetReductionSelection> = None;
    for shift in support {
        let divisor = basis.try_geometry_janet_divisor_with_scratch(
            shift,
            excluded_divisor,
            divisor_scratch,
            limits,
            work,
        )?;
        // A hit at ordinal `o` visited `o + 1` rows in the historical flat
        // scan. A miss visited the whole epoch, including an excluded row.
        let logical_visits = if let Some(ordinal) = divisor {
            checked_add("Janet normal-form divisor visits", ordinal, 1)?
        } else {
            basis.geometry_element_count()
        };
        *divisor_visits = checked_add(
            "Janet normal-form divisor visits",
            *divisor_visits,
            logical_visits,
        )?;
        work.charge_divisor_visits(logical_visits, limits)?;

        let Some(divisor_ordinal) = divisor else {
            continue;
        };
        let divisor =
            basis
                .geometry_monomial(divisor_ordinal)
                .ok_or(InvolutiveError::Invariant {
                    detail: "Janet divisor ordinal disappeared from its immutable geometry",
                })?;
        let target_key = ordering.try_key(shift)?;
        if selected
            .as_ref()
            .is_none_or(|current| target_key > current.target_key)
        {
            selected = Some(JanetReductionSelection {
                epoch: basis.geometry_epoch().clone(),
                target_shift: shift.clone(),
                target_key,
                divisor_ordinal,
                divisor_leading_shift: divisor.leading_shift().clone(),
            });
        }
    }
    Ok(selected)
}
