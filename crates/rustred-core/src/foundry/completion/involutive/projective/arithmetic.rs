use symbolica::prelude::*;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};

use super::super::{ForwardShift, OreConsequence, OreOrderingAdapter};
use super::error::{ProjectiveError, check_limit, checked_add, try_vec};
use super::limits::{ProjectiveLimits, ProjectiveNormalizationPolicy, ProjectiveWorkBudget};
use super::model::{
    PrimitiveOreConsequence, PrimitiveOreTerm, PrimitiveProvenanceTerm,
    ProjectiveNormalizationState,
};
use super::polynomial::{PolynomialWork, admit_payload, payload_census};

impl PrimitiveOreConsequence {
    /// Clear one authenticated rational Ore consequence into the integer
    /// polynomial ring, then remove only content common to its complete
    /// physical-row plus source-provenance vector.
    pub(super) fn try_from_rational(
        consequence: &OreConsequence,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        budget: &mut ProjectiveWorkBudget,
        limits: ProjectiveLimits,
    ) -> Result<Self, ProjectiveError> {
        budget.require_limits(limits)?;
        consequence.try_validate(ordering, context, limits.involutive)?;
        let row_len = consequence.row().terms().len();
        let provenance_len = consequence.provenance().terms().len();
        admit_structure(row_len, provenance_len, limits)?;
        preflight_guard_candidates(
            checked_add(
                "projective rational-ingress denominator candidates",
                row_len,
                provenance_len,
            )?,
            limits,
        )?;

        let mut work = PolynomialWork::try_new(context, limits, budget)?;
        let mut common_denominator = work.one()?;
        let mut denominator_guards = try_vec(
            "projective rational-ingress denominator guards",
            checked_add(
                "projective rational-ingress coefficients",
                row_len,
                provenance_len,
            )?,
        )?;
        let mut row_parts = try_vec("projective rational-ingress row parts", row_len)?;
        let mut row = try_vec("projective cleared row", row_len)?;
        let mut provenance = try_vec("projective cleared provenance", provenance_len)?;
        let base_localization = consequence
            .localization_witness()
            .try_clone_bounded(limits.involutive)?;
        for term in consequence.row().terms() {
            let numerator = work.numerator(term.coefficient())?;
            let denominator = work.denominator(term.coefficient())?;
            // Avoid cloning an already-large accumulated LCM for the common
            // polynomial case. `PolynomialWork::lcm` must still accept one as
            // a general operand, but rational ingress knows that a unit
            // denominator contributes nothing to this fold.
            if !denominator.raw().is_one() {
                common_denominator = work.lcm(&common_denominator, &denominator)?;
            }
            let retain_guard = !denominator.is_nonzero_constant();
            row_parts.push((term.shift().clone(), numerator, denominator, retain_guard));
        }
        let mut provenance_parts = try_vec(
            "projective rational-ingress provenance parts",
            provenance_len,
        )?;
        for term in consequence.provenance().terms() {
            let numerator = work.numerator(term.left_coefficient())?;
            let denominator = work.denominator(term.left_coefficient())?;
            if !denominator.raw().is_one() {
                common_denominator = work.lcm(&common_denominator, &denominator)?;
            }
            let retain_guard = !denominator.is_nonzero_constant();
            provenance_parts.push((
                term.source_ordinal(),
                term.left_shift().clone(),
                numerator,
                denominator,
                retain_guard,
            ));
        }

        for (shift, numerator, denominator, retain_guard) in row_parts {
            let cofactor = work.exact_div(&common_denominator, &denominator)?;
            let coefficient = work.mul(&cofactor, &numerator)?;
            if retain_guard {
                denominator_guards.push(denominator);
            }
            if !coefficient.is_zero() {
                row.push(PrimitiveOreTerm { shift, coefficient });
            }
        }
        for (source_ordinal, left_shift, numerator, denominator, retain_guard) in provenance_parts {
            let cofactor = work.exact_div(&common_denominator, &denominator)?;
            let left_coefficient = work.mul(&cofactor, &numerator)?;
            if retain_guard {
                denominator_guards.push(denominator);
            }
            if !left_coefficient.is_zero() {
                provenance.push(PrimitiveProvenanceTerm {
                    source_ordinal,
                    left_shift,
                    left_coefficient,
                });
            }
        }
        let row = normalize_row(row, &mut work, limits)?;
        let provenance = normalize_provenance(provenance, &mut work, limits)?;
        let (row, provenance) =
            normalize_augmented_content(row, provenance, ordering, &mut work, limits)?;
        let localization = base_localization.try_merge_polynomials(
            denominator_guards,
            context,
            limits.involutive,
        )?;
        let payload = payload_census(
            row.iter()
                .map(|term| &term.coefficient)
                .chain(provenance.iter().map(|term| &term.left_coefficient)),
        )?;
        admit_payload(payload, limits)?;
        let result = Self {
            action: consequence.row().action().clone(),
            arity: consequence.row().arity(),
            context_fingerprint: context.fingerprint_owner(),
            row: row.into_boxed_slice(),
            provenance: provenance.into_boxed_slice(),
            localization,
            normalization: ProjectiveNormalizationState::FullyNormalized,
            payload,
            work: work.census(),
        };
        result.try_validate(ordering, context, limits)?;
        Ok(result)
    }

    /// Perform one exact GCD-scaled pseudo-reduction
    /// `u F - v E^delta G` over the complete augmented vector.
    ///
    /// The divisor leader and every translated tail are authenticated against
    /// the frozen Ore order.  The result is still only a projective replay
    /// proposal; it cannot enter a Janet epoch or an artifact.
    pub(super) fn try_pseudo_reduce(
        &self,
        target: &ForwardShift,
        operator_shift: &ForwardShift,
        divisor: &Self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        normalization_policy: ProjectiveNormalizationPolicy,
        budget: &mut ProjectiveWorkBudget,
        limits: ProjectiveLimits,
    ) -> Result<Self, ProjectiveError> {
        budget.require_limits(limits)?;
        self.try_validate(ordering, context, limits)?;
        divisor.try_validate(ordering, context, limits)?;
        let result = self.try_pseudo_reduce_sealed(
            target,
            operator_shift,
            divisor,
            ordering,
            context,
            normalization_policy,
            budget,
            limits,
        )?;
        result.try_validate(ordering, context, limits)?;
        Ok(result)
    }

    /// Inner E2 replay seam for operands already authenticated at an
    /// immutable boundary.  The caller must retain that validation proof for
    /// the frozen divisor and may only feed back results constructed by this
    /// method as subsequent subjects.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_pseudo_reduce_sealed(
        &self,
        target: &ForwardShift,
        operator_shift: &ForwardShift,
        divisor: &Self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        normalization_policy: ProjectiveNormalizationPolicy,
        budget: &mut ProjectiveWorkBudget,
        limits: ProjectiveLimits,
    ) -> Result<Self, ProjectiveError> {
        budget.require_limits(limits)?;
        ordering.require_action(&self.action)?;
        self.require_context(context)?;
        divisor.require_context(context)?;
        if !self.action.belongs_to(&divisor.action) {
            return Err(ProjectiveError::ForeignAction);
        }
        ordering.require_arity("projective reduction target", target.arity())?;
        ordering.require_arity("projective reduction operator", operator_shift.arity())?;
        let subject_coefficient = self
            .coefficient(target)
            .ok_or(ProjectiveError::MissingSubjectTarget)?;
        let divisor_leader = divisor
            .try_leading_term(ordering)?
            .ok_or(ProjectiveError::ZeroDivisor)?;
        let translated_leader_shift =
            operator_shift.try_checked_add(divisor_leader.shift(), limits.involutive)?;
        if &translated_leader_shift != target {
            return Err(ProjectiveError::ReductionTargetMismatch);
        }

        let target_key = ordering.try_key(target)?;
        for term in divisor.row() {
            if term.shift() == divisor_leader.shift() {
                continue;
            }
            let shifted = operator_shift.try_checked_add(term.shift(), limits.involutive)?;
            if ordering.try_key(&shifted)? >= target_key {
                return Err(ProjectiveError::NonDescendingDivisorTail);
            }
        }

        let row_capacity = checked_add(
            "projective pseudo-reduction row inputs",
            self.row.len(),
            divisor.row.len(),
        )?;
        let provenance_capacity = checked_add(
            "projective pseudo-reduction provenance inputs",
            self.provenance.len(),
            divisor.provenance.len(),
        )?;
        check_limit(
            "projective pseudo-reduction augmented inputs",
            checked_add(
                "projective pseudo-reduction augmented inputs",
                row_capacity,
                provenance_capacity,
            )?,
            limits.max_augmented_entries,
        )?;
        let incoming_guard_count = checked_add(
            "projective pseudo-reduction localization candidates",
            divisor.required_nonzero_guards().len(),
            1,
        )?;
        preflight_guard_candidates(incoming_guard_count, limits)?;
        let mut row = try_vec("projective pseudo-reduction row inputs", row_capacity)?;
        let mut provenance = try_vec(
            "projective pseudo-reduction provenance inputs",
            provenance_capacity,
        )?;
        let mut incoming_guards = try_vec(
            "projective translated localization guards",
            incoming_guard_count,
        )?;
        let subject_localization = self.localization.try_clone_bounded(limits.involutive)?;
        let physical_translation = ordering.try_physical_translation(operator_shift)?;

        let mut work = PolynomialWork::try_new(context, limits, budget)?;
        let effective_divisor_leader =
            work.translate(divisor_leader.coefficient(), &physical_translation)?;
        let gcd = work.gcd(subject_coefficient, &effective_divisor_leader)?;
        let subject_multiplier = work.exact_div(&effective_divisor_leader, &gcd)?;
        let divisor_multiplier = work.exact_div(subject_coefficient, &gcd)?;

        for term in &self.row {
            let coefficient = work.mul(&subject_multiplier, &term.coefficient)?;
            if !coefficient.is_zero() {
                row.push(PrimitiveOreTerm {
                    shift: term.shift.clone(),
                    coefficient,
                });
            }
        }
        for term in &divisor.row {
            let shifted = operator_shift.try_checked_add(&term.shift, limits.involutive)?;
            let translated = work.translate(&term.coefficient, &physical_translation)?;
            let coefficient = work.mul(&divisor_multiplier, &translated)?;
            let coefficient = work.neg(&coefficient)?;
            if !coefficient.is_zero() {
                row.push(PrimitiveOreTerm {
                    shift: shifted,
                    coefficient,
                });
            }
        }

        for term in &self.provenance {
            let left_coefficient = work.mul(&subject_multiplier, &term.left_coefficient)?;
            if !left_coefficient.is_zero() {
                provenance.push(PrimitiveProvenanceTerm {
                    source_ordinal: term.source_ordinal,
                    left_shift: term.left_shift.clone(),
                    left_coefficient,
                });
            }
        }
        for term in &divisor.provenance {
            let left_shift = operator_shift.try_checked_add(&term.left_shift, limits.involutive)?;
            let translated = work.translate(&term.left_coefficient, &physical_translation)?;
            let left_coefficient = work.mul(&divisor_multiplier, &translated)?;
            let left_coefficient = work.neg(&left_coefficient)?;
            if !left_coefficient.is_zero() {
                provenance.push(PrimitiveProvenanceTerm {
                    source_ordinal: term.source_ordinal,
                    left_shift,
                    left_coefficient,
                });
            }
        }

        let row = normalize_row(row, &mut work, limits)?;
        if row.binary_search_by(|term| term.shift.cmp(target)).is_ok() {
            return Err(ProjectiveError::Invariant {
                detail: "GCD-scaled pseudo-reduction did not cancel its selected target",
            });
        }
        let provenance = normalize_provenance(provenance, &mut work, limits)?;
        let augmented_count = checked_add(
            "projective pseudo-reduction augmented result",
            row.len(),
            provenance.len(),
        )?;
        let (row, provenance, normalization) = if augmented_count == 0
            || normalization_policy.normalize_after_cancellation(augmented_count)
        {
            let (row, provenance) =
                normalize_augmented_content(row, provenance, ordering, &mut work, limits)?;
            (
                row,
                provenance,
                ProjectiveNormalizationState::FullyNormalized,
            )
        } else {
            (row, provenance, ProjectiveNormalizationState::Deferred)
        };

        for guard in divisor.required_nonzero_guards() {
            incoming_guards.push(work.translate(guard, &physical_translation)?);
        }
        // A projective cancellation scales the subject by B/g. Retaining the
        // effective divisor leader reproduces the conservative localization
        // of the existing monic divisor path on every exceptional fibre.
        incoming_guards.push(effective_divisor_leader);
        let localization = subject_localization.try_merge_polynomials(
            incoming_guards,
            context,
            limits.involutive,
        )?;
        let payload = payload_census(
            row.iter()
                .map(|term| &term.coefficient)
                .chain(provenance.iter().map(|term| &term.left_coefficient)),
        )?;
        admit_payload(payload, limits)?;
        let result = Self {
            action: self.action.clone(),
            arity: self.arity,
            context_fingerprint: self.context_fingerprint.clone(),
            row: row.into_boxed_slice(),
            provenance: provenance.into_boxed_slice(),
            localization,
            normalization,
            payload,
            work: work.census(),
        };
        Ok(result)
    }

    /// Force a complete row-plus-provenance primitive-content checkpoint.
    ///
    /// This remains a proposal-only normalization boundary.  It does not
    /// authenticate a Janet-basis insertion or permit artifact publication.
    pub(super) fn try_full_normalize_for_admission(
        self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        budget: &mut ProjectiveWorkBudget,
        limits: ProjectiveLimits,
    ) -> Result<Self, ProjectiveError> {
        budget.require_limits(limits)?;
        self.try_validate(ordering, context, limits)?;
        if self.is_fully_normalized() {
            return Ok(self);
        }
        let Self {
            action,
            arity,
            context_fingerprint,
            row,
            provenance,
            localization,
            normalization: _,
            payload: _,
            work: _,
        } = self;
        let mut work = PolynomialWork::try_new(context, limits, budget)?;
        let (row, provenance) = normalize_augmented_content(
            row.into_vec(),
            provenance.into_vec(),
            ordering,
            &mut work,
            limits,
        )?;
        let payload = payload_census(
            row.iter()
                .map(|term| &term.coefficient)
                .chain(provenance.iter().map(|term| &term.left_coefficient)),
        )?;
        admit_payload(payload, limits)?;
        let result = Self {
            action,
            arity,
            context_fingerprint,
            row: row.into_boxed_slice(),
            provenance: provenance.into_boxed_slice(),
            localization,
            normalization: ProjectiveNormalizationState::FullyNormalized,
            payload,
            work: work.census(),
        };
        result.try_validate(ordering, context, limits)?;
        Ok(result)
    }
}

fn preflight_guard_candidates(
    incoming: usize,
    limits: ProjectiveLimits,
) -> Result<(), ProjectiveError> {
    check_limit(
        "projective incoming localization guard candidates",
        incoming,
        limits.max_localization_guard_candidates,
    )
}

fn admit_structure(
    row_terms: usize,
    provenance_terms: usize,
    limits: ProjectiveLimits,
) -> Result<(), ProjectiveError> {
    check_limit("projective row terms", row_terms, limits.max_row_terms)?;
    check_limit(
        "projective provenance terms",
        provenance_terms,
        limits.max_provenance_terms,
    )?;
    check_limit(
        "projective augmented entries",
        checked_add("projective augmented entries", row_terms, provenance_terms)?,
        limits.max_augmented_entries,
    )
}

fn normalize_row(
    mut terms: Vec<PrimitiveOreTerm>,
    work: &mut PolynomialWork<'_, '_>,
    limits: ProjectiveLimits,
) -> Result<Vec<PrimitiveOreTerm>, ProjectiveError> {
    terms.sort_unstable_by(|left, right| left.shift.cmp(&right.shift));
    let mut normalized: Vec<PrimitiveOreTerm> =
        try_vec("canonical projective row terms", terms.len())?;
    for term in terms {
        if term.coefficient.is_zero() {
            continue;
        }
        if let Some(previous) = normalized.last_mut()
            && previous.shift == term.shift
        {
            let sum = work.add(&previous.coefficient, &term.coefficient)?;
            if sum.is_zero() {
                normalized.pop();
            } else {
                previous.coefficient = sum;
            }
        } else {
            normalized.push(term);
        }
    }
    check_limit(
        "projective row terms",
        normalized.len(),
        limits.max_row_terms,
    )?;
    Ok(normalized)
}

fn normalize_provenance(
    mut terms: Vec<PrimitiveProvenanceTerm>,
    work: &mut PolynomialWork<'_, '_>,
    limits: ProjectiveLimits,
) -> Result<Vec<PrimitiveProvenanceTerm>, ProjectiveError> {
    terms.sort_unstable_by(|left, right| {
        left.source_ordinal
            .cmp(&right.source_ordinal)
            .then_with(|| left.left_shift.cmp(&right.left_shift))
    });
    let mut normalized: Vec<PrimitiveProvenanceTerm> =
        try_vec("canonical projective provenance terms", terms.len())?;
    for term in terms {
        if term.left_coefficient.is_zero() {
            continue;
        }
        if let Some(previous) = normalized.last_mut()
            && previous.source_ordinal == term.source_ordinal
            && previous.left_shift == term.left_shift
        {
            let sum = work.add(&previous.left_coefficient, &term.left_coefficient)?;
            if sum.is_zero() {
                normalized.pop();
            } else {
                previous.left_coefficient = sum;
            }
        } else {
            normalized.push(term);
        }
    }
    check_limit(
        "projective provenance terms",
        normalized.len(),
        limits.max_provenance_terms,
    )?;
    Ok(normalized)
}

fn normalize_augmented_content(
    mut row: Vec<PrimitiveOreTerm>,
    mut provenance: Vec<PrimitiveProvenanceTerm>,
    ordering: &OreOrderingAdapter,
    work: &mut PolynomialWork<'_, '_>,
    limits: ProjectiveLimits,
) -> Result<(Vec<PrimitiveOreTerm>, Vec<PrimitiveProvenanceTerm>), ProjectiveError> {
    work.record_content_normalization()?;
    let augmented_count = checked_add("projective augmented entries", row.len(), provenance.len())?;
    if augmented_count == 0 {
        return Ok((row, provenance));
    }
    check_limit(
        "projective augmented entries",
        augmented_count,
        limits.max_augmented_entries,
    )?;
    let mut entries = try_vec("projective augmented-content inputs", augmented_count)?;
    entries.extend(row.iter().map(|term| &term.coefficient));
    entries.extend(provenance.iter().map(|term| &term.left_coefficient));
    let content = work.gcd_multiple(&entries)?;
    drop(entries);
    if content.is_zero() {
        return Err(ProjectiveError::Invariant {
            detail: "nonzero augmented entries produced zero polynomial content",
        });
    }
    if !content.raw().is_one() {
        for term in &mut row {
            term.coefficient = work.exact_div(&term.coefficient, &content)?;
        }
        for term in &mut provenance {
            term.left_coefficient = work.exact_div(&term.left_coefficient, &content)?;
        }
    }

    orient_augmented_sign(row, provenance, ordering, work)
}

fn orient_augmented_sign(
    mut row: Vec<PrimitiveOreTerm>,
    mut provenance: Vec<PrimitiveProvenanceTerm>,
    ordering: &OreOrderingAdapter,
    work: &mut PolynomialWork<'_, '_>,
) -> Result<(Vec<PrimitiveOreTerm>, Vec<PrimitiveProvenanceTerm>), ProjectiveError> {
    let sign_polynomial = if row.is_empty() {
        provenance.first().map(|term| &term.left_coefficient)
    } else {
        let mut leading = None;
        for term in &row {
            let key = ordering.try_key(&term.shift)?;
            if leading.as_ref().is_none_or(|(_, current)| key > *current) {
                leading = Some((term, key));
            }
        }
        leading.map(|(term, _)| &term.coefficient)
    };
    if sign_polynomial.is_some_and(|polynomial| polynomial.raw().lcoeff().is_negative()) {
        for term in &mut row {
            term.coefficient = work.neg(&term.coefficient)?;
        }
        for term in &mut provenance {
            term.left_coefficient = work.neg(&term.left_coefficient)?;
        }
    }
    Ok((row, provenance))
}

#[cfg(test)]
pub(super) fn polynomial_as_coefficient(
    polynomial: &IndexedPolynomial,
    context: &IndexedCoefficientContext,
) -> Result<IndexedCoefficient, ProjectiveError> {
    Ok(context.coefficient_from_polynomial_sealed(polynomial)?)
}
