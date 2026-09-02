use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::sector::ShiftComplexityKey;

use crate::foundry::completion::involutive::error::{check_limit, try_push_bounded, try_vec};
use crate::foundry::completion::involutive::limits::InvolutiveWorkBudget;
use crate::foundry::completion::involutive::{
    ForwardShift, InvolutiveError, InvolutiveLimits, OreActionIdentity, OreOrderingAdapter,
};

use super::arithmetic::{normalize_row_terms, require_context_arity, require_shift_arity};
use super::census::{CoefficientPayloadCensus, coefficient_payload_census};
use super::guards::LocalizationWitness;

/// One nonzero sparse Ore-row entry.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OreTerm {
    pub(super) shift: ForwardShift,
    pub(super) coefficient: IndexedCoefficient,
}

impl OreTerm {
    pub(crate) fn shift(&self) -> &ForwardShift {
        &self.shift
    }

    pub(crate) fn coefficient(&self) -> &IndexedCoefficient {
        &self.coefficient
    }
}

/// Canonical sparse linear difference operator over `K(n)[E]`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OreRow {
    pub(super) action: OreActionIdentity,
    pub(super) arity: usize,
    pub(super) terms: Box<[OreTerm]>,
}

impl OreRow {
    pub(crate) fn try_new(
        ordering: &OreOrderingAdapter,
        terms: impl IntoIterator<Item = (ForwardShift, IndexedCoefficient)>,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let arity = ordering.arity();
        require_context_arity(arity, context, limits)?;
        let mut retained = Vec::new();
        for (shift, coefficient) in terms {
            require_shift_arity("Ore row term", arity, &shift)?;
            context.validate_with_limits(&coefficient, limits.indexed_algebra.exact_algebra)?;
            try_push_bounded(
                &mut retained,
                OreTerm { shift, coefficient },
                "Ore input terms",
                limits.max_axpy_input_terms,
            )?;
        }
        let mut work = InvolutiveWorkBudget::default();
        normalize_row_terms(
            ordering.identity(),
            arity,
            retained,
            context,
            limits,
            &mut work,
        )
    }

    pub(crate) fn arity(&self) -> usize {
        self.arity
    }

    pub(crate) fn action(&self) -> &OreActionIdentity {
        &self.action
    }

    pub(crate) fn terms(&self) -> &[OreTerm] {
        &self.terms
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub(crate) fn coefficient(&self, shift: &ForwardShift) -> Option<&IndexedCoefficient> {
        self.terms
            .binary_search_by(|term| term.shift.cmp(shift))
            .ok()
            .map(|position| &self.terms[position].coefficient)
    }

    pub(crate) fn try_leading_term<'row>(
        &'row self,
        ordering: &OreOrderingAdapter,
    ) -> Result<Option<(&'row OreTerm, ShiftComplexityKey)>, InvolutiveError> {
        ordering.require_action(&self.action)?;
        ordering.require_arity("Ore row", self.arity)?;
        let mut leading: Option<(&OreTerm, ShiftComplexityKey)> = None;
        for term in &self.terms {
            let key = ordering.try_key(&term.shift)?;
            if leading.as_ref().is_none_or(|(_, current)| key > *current) {
                leading = Some((term, key));
            }
        }
        Ok(leading)
    }

    fn try_validate(
        &self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        ordering.require_action(&self.action)?;
        ordering.require_arity("Ore row", self.arity)?;
        require_context_arity(self.arity, context, limits)?;
        check_limit("Ore row terms", self.terms.len(), limits.max_row_terms)?;
        let mut previous = None;
        for term in &self.terms {
            require_shift_arity("Ore row term", self.arity, &term.shift)?;
            context
                .validate_with_limits(&term.coefficient, limits.indexed_algebra.exact_algebra)?;
            if term.coefficient.is_zero() {
                return Err(InvolutiveError::Invariant {
                    detail: "a canonical Ore row retained a zero term",
                });
            }
            if previous.is_some_and(|previous: &ForwardShift| previous >= &term.shift) {
                return Err(InvolutiveError::Invariant {
                    detail: "canonical Ore terms are not strictly shift ordered",
                });
            }
            previous = Some(&term.shift);
        }
        Ok(())
    }
}

/// One source-row contribution `a(n) E^alpha P_source`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OreProvenanceTerm {
    pub(super) source_ordinal: usize,
    pub(super) left_shift: ForwardShift,
    pub(super) left_coefficient: IndexedCoefficient,
}

impl OreProvenanceTerm {
    pub(crate) fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) fn left_shift(&self) -> &ForwardShift {
        &self.left_shift
    }

    pub(crate) fn left_coefficient(&self) -> &IndexedCoefficient {
        &self.left_coefficient
    }
}

/// Exact sparse source-module provenance for one derived consequence.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ConsequenceProvenance {
    pub(super) action: OreActionIdentity,
    pub(super) arity: usize,
    pub(super) terms: Box<[OreProvenanceTerm]>,
}

impl ConsequenceProvenance {
    pub(crate) fn terms(&self) -> &[OreProvenanceTerm] {
        &self.terms
    }

    fn try_validate(
        &self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        ordering.require_action(&self.action)?;
        ordering.require_arity("Ore provenance", self.arity)?;
        require_context_arity(self.arity, context, limits)?;
        check_limit(
            "Ore provenance terms",
            self.terms.len(),
            limits.max_provenance_terms,
        )?;
        let mut previous: Option<(usize, &ForwardShift)> = None;
        for term in &self.terms {
            require_shift_arity("Ore provenance term", self.arity, &term.left_shift)?;
            context.validate_with_limits(
                &term.left_coefficient,
                limits.indexed_algebra.exact_algebra,
            )?;
            if term.left_coefficient.is_zero() {
                return Err(InvolutiveError::Invariant {
                    detail: "canonical Ore provenance retained a zero term",
                });
            }
            let key = (term.source_ordinal, &term.left_shift);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(InvolutiveError::Invariant {
                    detail: "canonical Ore provenance is not strictly ordered",
                });
            }
            previous = Some(key);
        }
        Ok(())
    }
}

/// One exact difference consequence and its source-module witness.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OreConsequence {
    pub(super) row: OreRow,
    pub(super) provenance: ConsequenceProvenance,
    pub(super) localization: LocalizationWitness,
    pub(super) coefficient_census: CoefficientPayloadCensus,
}

impl OreConsequence {
    #[cfg(test)]
    pub(crate) fn try_from_source(
        source_ordinal: usize,
        row: OreRow,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let left_shift = ForwardShift::try_zero(row.arity, limits)?;
        Self::try_from_left_shifted_source(
            source_ordinal,
            left_shift,
            row,
            ordering,
            context,
            limits,
        )
    }

    /// Bind a row already regenerated as `E^left_shift P_source` to its exact
    /// one-term source-module witness.
    pub(in crate::foundry::completion::involutive) fn try_from_left_shifted_source(
        source_ordinal: usize,
        left_shift: ForwardShift,
        row: OreRow,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        ordering.require_source_ordinal(source_ordinal)?;
        row.try_validate(ordering, context, limits)?;
        if row.is_zero() {
            return Err(InvolutiveError::ZeroBasisRow);
        }
        require_shift_arity("left-shifted source witness", row.arity, &left_shift)?;
        check_limit("Ore provenance terms", 1, limits.max_provenance_terms)?;
        let mut terms = try_vec("source Ore provenance terms", 1)?;
        terms.push(OreProvenanceTerm {
            source_ordinal,
            left_shift,
            left_coefficient: context.one(),
        });
        let provenance = ConsequenceProvenance {
            action: row.action.clone(),
            arity: row.arity,
            terms: terms.into_boxed_slice(),
        };
        let coefficient_census = coefficient_payload_census(&row, &provenance, limits)?;
        Ok(Self {
            row,
            provenance,
            localization: LocalizationWitness::default(),
            coefficient_census,
        })
    }

    pub(crate) fn try_zero(
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let arity = ordering.arity();
        require_context_arity(arity, context, limits)?;
        let row = OreRow {
            action: ordering.identity().clone(),
            arity,
            terms: Box::new([]),
        };
        let provenance = ConsequenceProvenance {
            action: ordering.identity().clone(),
            arity,
            terms: Box::new([]),
        };
        Ok(Self {
            row,
            provenance,
            localization: LocalizationWitness::default(),
            coefficient_census: CoefficientPayloadCensus::default(),
        })
    }

    pub(crate) fn row(&self) -> &OreRow {
        &self.row
    }

    pub(crate) fn provenance(&self) -> &ConsequenceProvenance {
        &self.provenance
    }

    pub(crate) fn required_nonzero_guards(&self) -> &[Arc<IndexedPolynomial>] {
        self.localization.guards()
    }

    pub(crate) fn localization_witness(&self) -> &LocalizationWitness {
        &self.localization
    }

    pub(crate) const fn coefficient_census(&self) -> CoefficientPayloadCensus {
        self.coefficient_census
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.row.is_zero()
    }

    pub(crate) fn try_validate(
        &self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        self.row.try_validate(ordering, context, limits)?;
        self.provenance.try_validate(ordering, context, limits)?;
        if !self.row.action.belongs_to(&self.provenance.action) {
            return Err(InvolutiveError::ForeignOreAction);
        }
        self.localization.try_validate(context, limits)?;
        let coefficient_census = coefficient_payload_census(&self.row, &self.provenance, limits)?;
        if coefficient_census != self.coefficient_census {
            return Err(InvolutiveError::Invariant {
                detail: "Ore coefficient payload census disagrees with its row and provenance",
            });
        }
        Ok(())
    }
}
