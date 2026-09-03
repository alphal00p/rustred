use super::super::super::ForwardShift;
use super::{
    ClassifiedLazyOreRow, ExactLazyError, ExactLazyOwner, ExactLazyTransaction, ExactNonzeroProof,
    LazyCoeff,
};

/// One exact source-module term retained without expanding future AXPYs.
#[derive(Debug)]
pub(super) struct ImportedSourceTerm {
    source_ordinal: usize,
    left_shift: ForwardShift,
    left_coefficient: LazyCoeff,
    nonzero: ExactNonzeroProof,
}

impl ImportedSourceTerm {
    pub(super) fn try_new(
        transaction: &ExactLazyTransaction<'_, '_>,
        source_ordinal: usize,
        left_shift: ForwardShift,
        left_coefficient: LazyCoeff,
        nonzero: ExactNonzeroProof,
    ) -> Result<Self, ExactLazyError> {
        transaction.require_source_ordinal(source_ordinal)?;
        if left_shift.arity() != transaction.owner().arity() {
            return Err(ExactLazyError::WrongArity {
                object: "imported exact-lazy source shift",
                expected: transaction.owner().arity(),
                actual: left_shift.arity(),
            });
        }
        transaction.require_lazy_coefficient(&left_coefficient)?;
        if !nonzero.owns(transaction.owner(), &left_coefficient) {
            return Err(ExactLazyError::InvalidProof {
                detail: "an imported source proof does not authenticate its coefficient root",
            });
        }
        Ok(Self {
            source_ordinal,
            left_shift,
            left_coefficient,
            nonzero,
        })
    }

    pub(super) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(super) fn left_shift(&self) -> &ForwardShift {
        &self.left_shift
    }

    pub(super) fn left_coefficient(&self) -> &LazyCoeff {
        &self.left_coefficient
    }

    pub(super) fn nonzero_proof(&self) -> &ExactNonzeroProof {
        &self.nonzero
    }
}

/// ELC1's imported root node of the whole-consequence derivation DAG.
#[derive(Debug)]
pub(super) struct ImportedSourceDerivation {
    owner: ExactLazyOwner,
    terms: Box<[ImportedSourceTerm]>,
}

impl ImportedSourceDerivation {
    pub(super) fn try_new(
        transaction: &ExactLazyTransaction<'_, '_>,
        terms: Vec<ImportedSourceTerm>,
    ) -> Result<Self, ExactLazyError> {
        let mut previous: Option<(usize, &ForwardShift)> = None;
        for term in &terms {
            let key = (term.source_ordinal, &term.left_shift);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "imported source chronology is not strictly canonical",
                });
            }
            transaction.require_source_ordinal(term.source_ordinal)?;
            transaction.require_lazy_coefficient(&term.left_coefficient)?;
            if !term
                .nonzero
                .owns(transaction.owner(), &term.left_coefficient)
            {
                return Err(ExactLazyError::InvalidProof {
                    detail: "imported source term has a foreign or root-mismatched proof",
                });
            }
            previous = Some(key);
        }
        Ok(Self {
            owner: transaction.owner().clone(),
            terms: terms.into_boxed_slice(),
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn terms(&self) -> &[ImportedSourceTerm] {
        &self.terms
    }
}

/// Exact guard descriptor. ELC1 ingress currently needs only authenticated
/// denominator-one polynomial guards; denominator-of DAG roots are introduced
/// by cancellation.
#[derive(Debug)]
pub(super) enum ExactGuardDescriptor {
    Polynomial {
        coefficient: LazyCoeff,
        nonzero: ExactNonzeroProof,
    },
}

impl ExactGuardDescriptor {
    pub(super) fn try_polynomial(
        transaction: &ExactLazyTransaction<'_, '_>,
        coefficient: LazyCoeff,
        nonzero: ExactNonzeroProof,
    ) -> Result<Self, ExactLazyError> {
        transaction.require_lazy_coefficient(&coefficient)?;
        if !nonzero.owns(transaction.owner(), &coefficient) {
            return Err(ExactLazyError::InvalidProof {
                detail: "an exact guard proof does not authenticate its polynomial root",
            });
        }
        Ok(Self::Polynomial {
            coefficient,
            nonzero,
        })
    }

    pub(super) fn coefficient(&self) -> &LazyCoeff {
        match self {
            Self::Polynomial { coefficient, .. } => coefficient,
        }
    }

    pub(super) fn nonzero_proof(&self) -> &ExactNonzeroProof {
        match self {
            Self::Polynomial { nonzero, .. } => nonzero,
        }
    }
}

/// ELC1's imported root node of the exact guard-lineage DAG.
#[derive(Debug)]
pub(super) struct ImportedGuardLineage {
    owner: ExactLazyOwner,
    descriptors: Box<[ExactGuardDescriptor]>,
}

impl ImportedGuardLineage {
    pub(super) fn try_new(
        transaction: &ExactLazyTransaction<'_, '_>,
        descriptors: Vec<ExactGuardDescriptor>,
    ) -> Result<Self, ExactLazyError> {
        for descriptor in &descriptors {
            transaction.require_lazy_coefficient(descriptor.coefficient())?;
            if !descriptor
                .nonzero_proof()
                .owns(transaction.owner(), descriptor.coefficient())
            {
                return Err(ExactLazyError::InvalidProof {
                    detail: "an imported guard has a foreign or root-mismatched proof",
                });
            }
        }
        Ok(Self {
            owner: transaction.owner().clone(),
            descriptors: descriptors.into_boxed_slice(),
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn descriptors(&self) -> &[ExactGuardDescriptor] {
        &self.descriptors
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ExactLazyPayloadCensus {
    physical_terms: usize,
    provenance_terms: usize,
    guard_descriptors: usize,
}

impl ExactLazyPayloadCensus {
    pub(super) const fn new(
        physical_terms: usize,
        provenance_terms: usize,
        guard_descriptors: usize,
    ) -> Self {
        Self {
            physical_terms,
            provenance_terms,
            guard_descriptors,
        }
    }

    pub(super) const fn physical_terms(self) -> usize {
        self.physical_terms
    }

    pub(super) const fn provenance_terms(self) -> usize {
        self.provenance_terms
    }

    pub(super) const fn guard_descriptors(self) -> usize {
        self.guard_descriptors
    }
}

/// One exact-support consequence in an uncompacted ELC1 session.
#[derive(Debug)]
pub(super) struct ExactLazyConsequence {
    owner: ExactLazyOwner,
    row: ClassifiedLazyOreRow,
    derivation: ImportedSourceDerivation,
    guards: ImportedGuardLineage,
    census: ExactLazyPayloadCensus,
}

impl ExactLazyConsequence {
    pub(super) fn try_new(
        transaction: &ExactLazyTransaction<'_, '_>,
        row: ClassifiedLazyOreRow,
        derivation: ImportedSourceDerivation,
        guards: ImportedGuardLineage,
        census: ExactLazyPayloadCensus,
    ) -> Result<Self, ExactLazyError> {
        let owner = transaction.owner();
        if !row.owner().belongs_to(owner)
            || !derivation.owner().belongs_to(owner)
            || !guards.owner().belongs_to(owner)
        {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if row.terms().len() != census.physical_terms
            || derivation.terms().len() != census.provenance_terms
            || guards.descriptors().len() != census.guard_descriptors
        {
            return Err(ExactLazyError::InvalidSupport {
                detail: "exact-lazy payload census disagrees with imported payload",
            });
        }
        for term in row.terms() {
            if term.shift().arity() != owner.arity() {
                return Err(ExactLazyError::WrongArity {
                    object: "admitted exact-lazy Ore term",
                    expected: owner.arity(),
                    actual: term.shift().arity(),
                });
            }
            transaction.require_lazy_coefficient(term.coefficient())?;
            if !term.nonzero_proof().owns(owner, term.coefficient()) {
                return Err(ExactLazyError::InvalidProof {
                    detail: "admitted Ore term has a foreign or root-mismatched proof",
                });
            }
        }
        for term in derivation.terms() {
            transaction.require_source_ordinal(term.source_ordinal())?;
            if term.left_shift().arity() != owner.arity() {
                return Err(ExactLazyError::WrongArity {
                    object: "admitted exact-lazy provenance shift",
                    expected: owner.arity(),
                    actual: term.left_shift().arity(),
                });
            }
            transaction.require_lazy_coefficient(term.left_coefficient())?;
            if !term.nonzero_proof().owns(owner, term.left_coefficient()) {
                return Err(ExactLazyError::InvalidProof {
                    detail: "admitted provenance term has a foreign or root-mismatched proof",
                });
            }
        }
        for descriptor in guards.descriptors() {
            transaction.require_lazy_coefficient(descriptor.coefficient())?;
            if !descriptor
                .nonzero_proof()
                .owns(owner, descriptor.coefficient())
            {
                return Err(ExactLazyError::InvalidProof {
                    detail: "admitted guard has a foreign or root-mismatched proof",
                });
            }
        }
        Ok(Self {
            owner: owner.clone(),
            row,
            derivation,
            guards,
            census,
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn row(&self) -> &ClassifiedLazyOreRow {
        &self.row
    }

    pub(super) fn derivation(&self) -> &ImportedSourceDerivation {
        &self.derivation
    }

    pub(super) fn guards(&self) -> &ImportedGuardLineage {
        &self.guards
    }

    pub(super) const fn census(&self) -> ExactLazyPayloadCensus {
        self.census
    }
}
