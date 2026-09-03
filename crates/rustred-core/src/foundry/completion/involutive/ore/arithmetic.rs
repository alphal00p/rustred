use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};

use crate::foundry::completion::involutive::error::{
    check_limit, checked_add, checked_mul, reserve_additional, try_vec,
};
use crate::foundry::completion::involutive::limits::InvolutiveWorkBudget;
use crate::foundry::completion::involutive::{
    ForwardShift, InvolutiveError, InvolutiveLimits, OreActionIdentity, OreOrderingAdapter,
};

use super::census::coefficient_payload_census;
use super::guard_domain::{LocalizationDomainBudget, try_require_principal_open_coverage};
use super::guards::try_canonical_nonzero_guard;
use super::model::{ConsequenceProvenance, OreConsequence, OreProvenanceTerm, OreRow, OreTerm};

impl OreConsequence {
    /// Replace replay's syntactic guard set by an independently authenticated
    /// lazy witness only after a bounded exact principal-open proof.
    ///
    /// The proof obligation is deliberately one-way: every replay-required
    /// irreducible factor must occur in `authenticated_lazy`. Additional
    /// historic lazy guards are conservative restrictions and remain part of
    /// the returned authority. No unchecked witness-replacement seam exists.
    pub(in crate::foundry::completion::involutive) fn try_restrict_to_authenticated_localization(
        self,
        authenticated_lazy: super::guards::LocalizationWitness,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        budget: &mut LocalizationDomainBudget,
    ) -> Result<Self, InvolutiveError> {
        self.localization.try_validate(context, limits)?;
        authenticated_lazy.try_validate(context, limits)?;
        try_require_principal_open_coverage(
            context,
            authenticated_lazy.guards(),
            self.localization.guards(),
            limits.indexed_algebra.exact_algebra,
            budget,
        )?;
        Ok(Self {
            row: self.row,
            provenance: self.provenance,
            localization: authenticated_lazy,
            coefficient_census: self.coefficient_census,
        })
    }

    /// Return a projectively normalized copy whose current Ore leader is one.
    ///
    /// Janet epochs retain this invariant at their single construction
    /// boundary.  Scaling is performed by the ordinary exact left-AXPY path,
    /// so the row and its source-module provenance receive the identical
    /// rational-function multiplier.  The denominator of that multiplier is
    /// precisely the numerator of the old leading coefficient; the AXPY
    /// localization path therefore records exactly the additional nonzero
    /// condition needed by this division and preserves every existing guard.
    ///
    /// `None` is the allocation- and arithmetic-free stable path for an
    /// already-monic sealed consequence.
    pub(in crate::foundry::completion::involutive) fn try_monic_copy_sealed(
        &self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Option<Self>, InvolutiveError> {
        ordering.require_action(&self.row.action)?;
        ordering.require_action(&self.provenance.action)?;
        ordering.require_arity("Ore monic normalization row", self.row.arity)?;
        ordering.require_arity("Ore monic normalization provenance", self.provenance.arity)?;
        let (leading, leading_key) = self
            .row
            .try_leading_term(ordering)?
            .ok_or(InvolutiveError::ZeroBasisRow)?;
        let original_leading_shift = leading.shift.clone();
        let original_leading_key = leading_key;
        let one = context.one();
        if leading.coefficient == one {
            return Ok(None);
        }

        work.charge_exact_coefficient_operations(1, limits)?;
        let inverse = context.div_bound_with_limits(
            context.bind_sealed(&one)?,
            context.bind_sealed(&leading.coefficient)?,
            limits.indexed_algebra.exact_algebra,
        )?;
        let operator_zero = ForwardShift::try_zero(self.row.arity, limits)?;
        let normalized = super::super::with_coefficient_diagnostic_site!(
            MonicNormalization,
            Self::try_zero(ordering, context, limits)?.try_left_axpy_sealed(
                &inverse,
                &operator_zero,
                self,
                ordering,
                context,
                limits,
                work,
            )
        )?;
        let (normalized_leading, normalized_leading_key) = normalized
            .row
            .try_leading_term(ordering)?
            .ok_or(InvolutiveError::Invariant {
                detail: "monic normalization produced a zero Ore row",
            })?;
        if normalized_leading.shift != original_leading_shift
            || normalized_leading_key != original_leading_key
        {
            return Err(InvolutiveError::Invariant {
                detail: "exact Ore projective normalization changed the leading shift or key",
            });
        }
        if normalized_leading.coefficient != one {
            return Err(InvolutiveError::Invariant {
                detail: "exact Ore projective normalization did not produce a unit leader",
            });
        }
        Ok(Some(normalized))
    }

    /// Consume `self` and add `multiplier * E^operator_shift * source`.
    ///
    /// The coefficient mutation is the exact Ore action
    /// `E^a c(n) = c(n + signed_sector(a)) E^a`. The same action composes
    /// every source-module provenance coefficient, so derived consequences
    /// never lose their independently replayable origin.
    pub(crate) fn try_left_axpy(
        self,
        multiplier: &IndexedCoefficient,
        operator_shift: &ForwardShift,
        source: &Self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let mut work = InvolutiveWorkBudget::default();
        self.try_validate(ordering, context, limits)?;
        source.try_validate(ordering, context, limits)?;
        context.validate_with_limits(multiplier, limits.indexed_algebra.exact_algebra)?;
        super::super::with_coefficient_diagnostic_site!(
            DirectAxpy,
            self.try_left_axpy_sealed(
                multiplier,
                operator_shift,
                source,
                ordering,
                context,
                limits,
                &mut work,
            )
        )
    }

    pub(in crate::foundry::completion::involutive) fn try_left_axpy_with_budget(
        self,
        multiplier: &IndexedCoefficient,
        operator_shift: &ForwardShift,
        source: &Self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        self.try_validate(ordering, context, limits)?;
        source.try_validate(ordering, context, limits)?;
        context.validate_with_limits(multiplier, limits.indexed_algebra.exact_algebra)?;
        self.try_left_axpy_sealed(
            multiplier,
            operator_shift,
            source,
            ordering,
            context,
            limits,
            work,
        )
    }

    /// Trusted inner AXPY after complete consequence and multiplier ingress
    /// validation. Every native result remains authenticated by the indexed
    /// algebra boundary.
    pub(in crate::foundry::completion::involutive) fn try_left_axpy_sealed(
        self,
        multiplier: &IndexedCoefficient,
        operator_shift: &ForwardShift,
        source: &Self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        ordering.require_action(&self.row.action)?;
        ordering.require_action(&source.row.action)?;
        ordering.require_action(&self.provenance.action)?;
        ordering.require_action(&source.provenance.action)?;
        ordering.require_arity("Ore AXPY accumulator", self.row.arity)?;
        ordering.require_arity("Ore AXPY source", source.row.arity)?;
        require_shift_arity("Ore AXPY operator", self.row.arity, operator_shift)?;

        let input_terms = checked_add(
            "Ore AXPY input terms",
            checked_add(
                "Ore AXPY input terms",
                self.row.terms.len(),
                source.row.terms.len(),
            )?,
            checked_add(
                "Ore AXPY input terms",
                self.provenance.terms.len(),
                source.provenance.terms.len(),
            )?,
        )?;
        check_limit(
            "Ore AXPY input terms",
            input_terms,
            limits.max_axpy_input_terms,
        )?;
        if multiplier.is_zero() {
            return Ok(self);
        }

        let transformed_coefficient_operations = checked_mul(
            "Janet exact coefficient operations",
            checked_add(
                "Janet exact coefficient operations",
                source.row.terms.len(),
                source.provenance.terms.len(),
            )?,
            2,
        )?;
        work.charge_exact_coefficient_operations(transformed_coefficient_operations, limits)?;

        let physical_translation = ordering.try_physical_translation(operator_shift)?;
        let multiplier = context.bind_sealed(multiplier)?;
        let multiplier_denominator = context.denominator_condition_from_bound(multiplier)?;
        let mut transformed_row = try_vec("Ore transformed row terms", source.row.terms.len())?;
        for term in &source.row.terms {
            let coefficient = context.translate_sealed(
                &term.coefficient,
                &physical_translation,
                limits.indexed_algebra,
            )?;
            let coefficient = context.mul_bound_with_limits(
                multiplier,
                context.bind_sealed(&coefficient)?,
                limits.indexed_algebra.exact_algebra,
            )?;
            if coefficient.is_zero() {
                continue;
            }
            transformed_row.push(OreTerm {
                shift: operator_shift.try_checked_add(&term.shift, limits)?,
                coefficient,
            });
        }

        let mut transformed_provenance = try_vec(
            "Ore transformed provenance terms",
            source.provenance.terms.len(),
        )?;
        for term in &source.provenance.terms {
            let coefficient = context.translate_sealed(
                &term.left_coefficient,
                &physical_translation,
                limits.indexed_algebra,
            )?;
            let coefficient = context.mul_bound_with_limits(
                multiplier,
                context.bind_sealed(&coefficient)?,
                limits.indexed_algebra.exact_algebra,
            )?;
            if coefficient.is_zero() {
                continue;
            }
            transformed_provenance.push(OreProvenanceTerm {
                source_ordinal: term.source_ordinal,
                left_shift: operator_shift.try_checked_add(&term.left_shift, limits)?,
                left_coefficient: coefficient,
            });
        }

        let mut row_terms = self.row.terms.into_vec();
        reserve_additional(&mut row_terms, transformed_row.len(), "Ore AXPY row terms")?;
        row_terms.extend(transformed_row);
        let row = normalize_row_terms(
            &self.row.action,
            self.row.arity,
            row_terms,
            context,
            limits,
            work,
        )?;

        let mut provenance_terms = self.provenance.terms.into_vec();
        reserve_additional(
            &mut provenance_terms,
            transformed_provenance.len(),
            "Ore AXPY provenance terms",
        )?;
        provenance_terms.extend(transformed_provenance);
        let provenance = normalize_provenance_terms(
            &self.provenance.action,
            self.provenance.arity,
            provenance_terms,
            context,
            limits,
            work,
        )?;

        let mut incoming_guards = try_vec(
            "Ore translated localization guards",
            checked_add(
                "Ore localization guards",
                source.localization.guards().len(),
                1,
            )?,
        )?;
        for guard in source.localization.guards() {
            let translated = context.translate_polynomial_sealed(
                guard,
                &physical_translation,
                limits.indexed_algebra,
            )?;
            incoming_guards.push(translated);
        }
        incoming_guards.push(multiplier_denominator);
        let localization =
            self.localization
                .try_merge_polynomials(incoming_guards, context, limits)?;
        let coefficient_census = coefficient_payload_census(&row, &provenance, limits)?;
        Ok(Self {
            row,
            provenance,
            localization,
            coefficient_census,
        })
    }

    pub(in crate::foundry::completion::involutive) fn try_require_nonzero_guard(
        self,
        guard: IndexedPolynomial,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<(Self, Option<Arc<IndexedPolynomial>>), InvolutiveError> {
        let Some(guard) = try_canonical_nonzero_guard(context, &guard, limits)? else {
            return Ok((self, None));
        };
        let guard = Arc::new(guard);
        let localization = self
            .localization
            .try_merge_canonical_arcs([Arc::clone(&guard)], limits)?;
        Ok((
            Self {
                row: self.row,
                provenance: self.provenance,
                localization,
                coefficient_census: self.coefficient_census,
            },
            Some(guard),
        ))
    }

    /// Attach a translated guard batch with one exact cardinality preflight
    /// and at most one growth of the retained guard vector.
    ///
    /// This boundary is intended for ordinary-source chart lifting, where a
    /// relation may carry many nonzero conditions. It avoids repeatedly
    /// converting and reallocating a boxed slice for every condition while
    /// preserving the same canonical primitive associates and resource
    /// census as the scalar helper.
    pub(in crate::foundry::completion::involutive) fn try_require_nonzero_guards(
        self,
        guards: Vec<IndexedPolynomial>,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let localization = self
            .localization
            .try_merge_polynomials(guards, context, limits)?;
        Ok(Self {
            row: self.row,
            provenance: self.provenance,
            localization,
            coefficient_census: self.coefficient_census,
        })
    }

    pub(in crate::foundry::completion::involutive) fn try_copy_sealed(
        &self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        let zero = ForwardShift::try_zero(ordering.arity(), limits)?;
        super::super::with_coefficient_diagnostic_site!(
            AutoreductionMaterialization,
            Self::try_zero(ordering, context, limits)?.try_left_axpy_sealed(
                &context.one(),
                &zero,
                self,
                ordering,
                context,
                limits,
                work,
            )
        )
    }
}

pub(super) fn normalize_row_terms(
    action: &OreActionIdentity,
    arity: usize,
    mut terms: Vec<OreTerm>,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<OreRow, InvolutiveError> {
    terms.sort_unstable_by(|left, right| left.shift.cmp(&right.shift));
    let mut normalized: Vec<OreTerm> = try_vec("canonical Ore row terms", terms.len())?;
    for term in terms {
        if term.coefficient.is_zero() {
            continue;
        }
        if let Some(previous) = normalized.last_mut()
            && previous.shift == term.shift
        {
            work.charge_exact_coefficient_operations(1, limits)?;
            let sum = context.add_bound_with_limits(
                context.bind_sealed(&previous.coefficient)?,
                context.bind_sealed(&term.coefficient)?,
                limits.indexed_algebra.exact_algebra,
            )?;
            if sum.is_zero() {
                normalized.pop();
            } else {
                previous.coefficient = sum;
            }
        } else {
            normalized.push(term);
        }
    }
    check_limit("Ore row terms", normalized.len(), limits.max_row_terms)?;
    Ok(OreRow {
        action: action.clone(),
        arity,
        terms: normalized.into_boxed_slice(),
    })
}

fn normalize_provenance_terms(
    action: &OreActionIdentity,
    arity: usize,
    mut terms: Vec<OreProvenanceTerm>,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<ConsequenceProvenance, InvolutiveError> {
    terms.sort_unstable_by(|left, right| {
        left.source_ordinal
            .cmp(&right.source_ordinal)
            .then_with(|| left.left_shift.cmp(&right.left_shift))
    });
    let mut normalized: Vec<OreProvenanceTerm> =
        try_vec("canonical Ore provenance terms", terms.len())?;
    for term in terms {
        if term.left_coefficient.is_zero() {
            continue;
        }
        if let Some(previous) = normalized.last_mut()
            && previous.source_ordinal == term.source_ordinal
            && previous.left_shift == term.left_shift
        {
            work.charge_exact_coefficient_operations(1, limits)?;
            let sum = context.add_bound_with_limits(
                context.bind_sealed(&previous.left_coefficient)?,
                context.bind_sealed(&term.left_coefficient)?,
                limits.indexed_algebra.exact_algebra,
            )?;
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
        "Ore provenance terms",
        normalized.len(),
        limits.max_provenance_terms,
    )?;
    Ok(ConsequenceProvenance {
        action: action.clone(),
        arity,
        terms: normalized.into_boxed_slice(),
    })
}

pub(super) fn require_context_arity(
    arity: usize,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    if arity == 0 {
        return Err(InvolutiveError::EmptyCoordinateSpace);
    }
    check_limit("Ore arity", arity, limits.max_arity)?;
    if context.index_count() == arity {
        Ok(())
    } else {
        Err(InvolutiveError::WrongArity {
            object: "indexed coefficient context",
            expected: arity,
            actual: context.index_count(),
        })
    }
}

pub(super) fn require_shift_arity(
    object: &'static str,
    arity: usize,
    shift: &ForwardShift,
) -> Result<(), InvolutiveError> {
    if shift.arity() == arity {
        Ok(())
    } else {
        Err(InvolutiveError::WrongArity {
            object,
            expected: arity,
            actual: shift.arity(),
        })
    }
}
