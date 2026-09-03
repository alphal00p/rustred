use std::sync::Arc;

use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};

use super::super::{ForwardShift, LocalizationWitness, OreActionIdentity, OreOrderingAdapter};
use super::error::{ProjectiveError, check_limit, checked_add};
use super::limits::{ProjectiveLimits, ProjectiveWorkCensus};

/// Whether the complete augmented vector is known to have primitive content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProjectiveNormalizationState {
    FullyNormalized,
    Deferred,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PrimitiveOreTerm {
    pub(super) shift: ForwardShift,
    pub(super) coefficient: IndexedPolynomial,
}

impl PrimitiveOreTerm {
    pub(super) fn shift(&self) -> &ForwardShift {
        &self.shift
    }

    pub(super) fn coefficient(&self) -> &IndexedPolynomial {
        &self.coefficient
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PrimitiveProvenanceTerm {
    pub(super) source_ordinal: usize,
    pub(super) left_shift: ForwardShift,
    pub(super) left_coefficient: IndexedPolynomial,
}

impl PrimitiveProvenanceTerm {
    pub(super) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(super) fn left_shift(&self) -> &ForwardShift {
        &self.left_shift
    }

    pub(super) fn left_coefficient(&self) -> &IndexedPolynomial {
        &self.left_coefficient
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ProjectivePayloadCensus {
    pub(super) polynomial_terms: usize,
    pub(super) exponent_cells: usize,
    pub(super) retained_bytes: usize,
}

/// One exact sparse augmented Ore vector over `Z[parameters,n]`.
///
/// This type intentionally cannot enter a Janet epoch.  Its source-module
/// coefficients are retained alongside the physical row so every projective
/// scaling and content normalization remains an exact augmented identity.
#[derive(Debug)]
pub(super) struct PrimitiveOreConsequence {
    pub(super) action: OreActionIdentity,
    pub(super) arity: usize,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) row: Box<[PrimitiveOreTerm]>,
    pub(super) provenance: Box<[PrimitiveProvenanceTerm]>,
    pub(super) localization: LocalizationWitness,
    pub(super) normalization: ProjectiveNormalizationState,
    pub(super) payload: ProjectivePayloadCensus,
    pub(super) work: ProjectiveWorkCensus,
}

impl PrimitiveOreConsequence {
    pub(super) fn row(&self) -> &[PrimitiveOreTerm] {
        &self.row
    }

    pub(super) fn provenance(&self) -> &[PrimitiveProvenanceTerm] {
        &self.provenance
    }

    pub(super) fn required_nonzero_guards(&self) -> &[std::sync::Arc<IndexedPolynomial>] {
        self.localization.guards()
    }

    pub(super) const fn work_census(&self) -> ProjectiveWorkCensus {
        self.work
    }

    pub(super) const fn payload_census(&self) -> ProjectivePayloadCensus {
        self.payload
    }

    pub(super) const fn normalization_state(&self) -> ProjectiveNormalizationState {
        self.normalization
    }

    pub(super) const fn is_fully_normalized(&self) -> bool {
        matches!(
            self.normalization,
            ProjectiveNormalizationState::FullyNormalized
        )
    }

    pub(super) fn is_zero(&self) -> bool {
        self.row.is_empty()
    }

    pub(super) fn coefficient(&self, shift: &ForwardShift) -> Option<&IndexedPolynomial> {
        self.row
            .binary_search_by(|term| term.shift.cmp(shift))
            .ok()
            .map(|position| &self.row[position].coefficient)
    }

    pub(super) fn try_leading_term<'row>(
        &'row self,
        ordering: &OreOrderingAdapter,
    ) -> Result<Option<&'row PrimitiveOreTerm>, ProjectiveError> {
        ordering.require_action(&self.action)?;
        let mut leading = None;
        for term in &self.row {
            let key = ordering.try_key(&term.shift)?;
            if leading.as_ref().is_none_or(|(_, current)| key > *current) {
                leading = Some((term, key));
            }
        }
        Ok(leading.map(|(term, _)| term))
    }

    pub(super) fn try_validate(
        &self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ProjectiveLimits,
    ) -> Result<(), ProjectiveError> {
        self.require_context(context)?;
        ordering.require_action(&self.action)?;
        ordering.require_arity("projective Ore consequence", self.arity)?;
        check_limit("projective row terms", self.row.len(), limits.max_row_terms)?;
        check_limit(
            "projective provenance terms",
            self.provenance.len(),
            limits.max_provenance_terms,
        )?;
        check_limit(
            "projective augmented entries",
            checked_add(
                "projective augmented entries",
                self.row.len(),
                self.provenance.len(),
            )?,
            limits.max_augmented_entries,
        )?;
        let exact = limits.involutive.indexed_algebra.exact_algebra;
        let mut previous = None;
        for term in &self.row {
            ordering.require_arity("projective row shift", term.shift.arity())?;
            context.validate_polynomial_with_limits(&term.coefficient, exact)?;
            if term.coefficient.is_zero() {
                return Err(ProjectiveError::Invariant {
                    detail: "a projective row retained a zero coefficient",
                });
            }
            if previous.is_some_and(|previous: &ForwardShift| previous >= &term.shift) {
                return Err(ProjectiveError::Invariant {
                    detail: "projective row shifts are not strictly ordered",
                });
            }
            previous = Some(&term.shift);
        }
        let mut previous = None;
        for term in &self.provenance {
            ordering.require_source_ordinal(term.source_ordinal)?;
            ordering.require_arity("projective provenance shift", term.left_shift.arity())?;
            context.validate_polynomial_with_limits(&term.left_coefficient, exact)?;
            if term.left_coefficient.is_zero() {
                return Err(ProjectiveError::Invariant {
                    detail: "projective provenance retained a zero coefficient",
                });
            }
            let key = (term.source_ordinal, &term.left_shift);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(ProjectiveError::Invariant {
                    detail: "projective provenance is not strictly ordered",
                });
            }
            previous = Some(key);
        }
        self.localization.try_validate(context, limits.involutive)?;
        let payload = super::polynomial::payload_census(
            self.row
                .iter()
                .map(|term| &term.coefficient)
                .chain(self.provenance.iter().map(|term| &term.left_coefficient)),
        )?;
        super::polynomial::admit_payload(payload, limits)?;
        if payload != self.payload {
            return Err(ProjectiveError::Invariant {
                detail: "projective payload census disagrees with its augmented vector",
            });
        }
        Ok(())
    }

    pub(super) fn require_context(
        &self,
        context: &IndexedCoefficientContext,
    ) -> Result<(), ProjectiveError> {
        if context.index_count() != self.arity {
            return Err(ProjectiveError::ContextIndexArityMismatch {
                consequence_arity: self.arity,
                context_index_count: context.index_count(),
            });
        }
        if self.context_fingerprint.as_str() != context.fingerprint() {
            return Err(ProjectiveError::ContextFingerprintMismatch);
        }
        Ok(())
    }
}

impl PartialEq for PrimitiveOreConsequence {
    fn eq(&self, other: &Self) -> bool {
        self.action.belongs_to(&other.action)
            && self.arity == other.arity
            && self.context_fingerprint == other.context_fingerprint
            && self.row == other.row
            && self.provenance == other.provenance
            && self.localization == other.localization
            && self.normalization == other.normalization
            && self.payload == other.payload
    }
}

impl Eq for PrimitiveOreConsequence {}

/// Immutable proof that one projective consequence crossed its complete
/// structural, coefficient-context, localization, and payload boundary.
///
/// The wrapper carries no completion or publication authority.  It only lets
/// an E2 replay reuse validation of a frozen divisor rather than rescanning
/// its complete augmented payload at every cancellation.
pub(super) struct ValidatedProjectiveConsequence<'consequence> {
    consequence: &'consequence PrimitiveOreConsequence,
    limits: ProjectiveLimits,
}

impl<'consequence> ValidatedProjectiveConsequence<'consequence> {
    pub(super) fn try_new(
        consequence: &'consequence PrimitiveOreConsequence,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ProjectiveLimits,
    ) -> Result<Self, ProjectiveError> {
        consequence.try_validate(ordering, context, limits)?;
        Ok(Self {
            consequence,
            limits,
        })
    }

    pub(super) const fn consequence(&self) -> &'consequence PrimitiveOreConsequence {
        self.consequence
    }

    pub(super) fn require_limits(&self, limits: ProjectiveLimits) -> Result<(), ProjectiveError> {
        if self.limits == limits {
            Ok(())
        } else {
            Err(ProjectiveError::ValidatedDivisorLimitsMismatch)
        }
    }
}
