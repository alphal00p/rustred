use super::super::super::ForwardShift;
use super::arena::ExactLazyCommitReceipt;
use super::{
    ClassifiedLazyOreRow, ExactLazyError, ExactLazyOwner, ExactLazyTransaction, ExactNonzeroProof,
    GuardLineageRef, LazyCoeff, SourceDerivationRef,
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
        if !nonzero.owns_live(transaction, &left_coefficient) {
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
    root: SourceDerivationRef,
    source_term_count: usize,
}

impl ImportedSourceDerivation {
    pub(super) fn try_new(
        transaction: &mut ExactLazyTransaction<'_, '_>,
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
            if !term.nonzero.owns_live(transaction, &term.left_coefficient) {
                return Err(ExactLazyError::InvalidProof {
                    detail: "imported source term has a foreign or root-mismatched proof",
                });
            }
            previous = Some(key);
        }
        let mut root = transaction.zero_derivation();
        for term in terms {
            // Exactly encode c E^delta P_source: the shift applies to the
            // source row once, while c remains the left AXPY multiplier.
            let source = transaction.try_source_derivation(term.source_ordinal)?;
            root = transaction.try_left_axpy_derivation(
                &root,
                &term.left_coefficient,
                &term.left_shift,
                &source,
            )?;
        }
        Self::try_from_lineage(transaction, root)
    }

    pub(super) fn try_from_lineage(
        transaction: &ExactLazyTransaction<'_, '_>,
        root: SourceDerivationRef,
    ) -> Result<Self, ExactLazyError> {
        transaction.require_derivation(&root)?;
        let source_term_count = root.logical_source_terms();
        Ok(Self {
            owner: transaction.owner().clone(),
            root,
            source_term_count,
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn root(&self) -> &SourceDerivationRef {
        &self.root
    }

    pub(super) const fn source_term_count(&self) -> usize {
        self.source_term_count
    }
}

/// ELC1's imported root node of the exact guard-lineage DAG.
#[derive(Debug)]
pub(super) struct ImportedGuardLineage {
    owner: ExactLazyOwner,
    root: GuardLineageRef,
    descriptor_count: usize,
}

impl ImportedGuardLineage {
    pub(super) fn try_new(
        transaction: &mut ExactLazyTransaction<'_, '_>,
        descriptors: Vec<GuardLineageRef>,
    ) -> Result<Self, ExactLazyError> {
        let mut root = transaction.empty_guards();
        for descriptor in descriptors {
            root = transaction.try_union_guards(&root, &descriptor)?;
        }
        Self::try_from_lineage(transaction, root)
    }

    pub(super) fn try_from_lineage(
        transaction: &ExactLazyTransaction<'_, '_>,
        root: GuardLineageRef,
    ) -> Result<Self, ExactLazyError> {
        transaction.require_guard_lineage(&root)?;
        let descriptor_count = root.logical_descriptors();
        Ok(Self {
            owner: transaction.owner().clone(),
            root,
            descriptor_count,
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn root(&self) -> &GuardLineageRef {
        &self.root
    }

    pub(super) const fn descriptor_count(&self) -> usize {
        self.descriptor_count
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
    commit_receipt: ExactLazyCommitReceipt,
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
        let live_terms = row.try_terms_in_transaction(transaction)?;
        if live_terms.len() != census.physical_terms
            || derivation.source_term_count() != census.provenance_terms
            || guards.descriptor_count() != census.guard_descriptors
        {
            return Err(ExactLazyError::InvalidSupport {
                detail: "exact-lazy payload census disagrees with imported payload",
            });
        }
        for term in live_terms {
            if term.shift().arity() != owner.arity() {
                return Err(ExactLazyError::WrongArity {
                    object: "admitted exact-lazy Ore term",
                    expected: owner.arity(),
                    actual: term.shift().arity(),
                });
            }
            transaction.require_lazy_coefficient(term.coefficient())?;
            if !term
                .nonzero_proof()
                .owns_live(transaction, term.coefficient())
            {
                return Err(ExactLazyError::InvalidProof {
                    detail: "admitted Ore term has a foreign or root-mismatched proof",
                });
            }
        }
        transaction.require_derivation(derivation.root())?;
        transaction.require_guard_lineage(guards.root())?;
        Ok(Self {
            owner: owner.clone(),
            commit_receipt: transaction.pending_commit_receipt(),
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

    /// Recheck every retained arena root before this consequence enters a new
    /// mutation boundary. This catches a complete value which escaped an
    /// aborted transaction even when some of its coefficient roots happen to
    /// lie below the old committed floor.
    pub(super) fn try_validate_live(
        &self,
        session: &super::ExactLazySession<'_>,
    ) -> Result<(), ExactLazyError> {
        if !self.owner.belongs_to(session.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if !self.commit_receipt.owns_committed(session.owner()) {
            return Err(ExactLazyError::InvalidProof {
                detail: "exact-lazy consequence did not cross its transaction commit boundary",
            });
        }
        let terms = self.row.try_terms_live(session)?;
        session.require_derivation(self.derivation.root())?;
        session.require_guard_lineage(self.guards.root())?;
        if terms.len() != self.census.physical_terms()
            || self.derivation.source_term_count() != self.census.provenance_terms()
            || self.guards.descriptor_count() != self.census.guard_descriptors()
        {
            return Err(ExactLazyError::InvalidSupport {
                detail: "live exact-lazy payload disagrees with its minted census",
            });
        }
        Ok(())
    }
}
