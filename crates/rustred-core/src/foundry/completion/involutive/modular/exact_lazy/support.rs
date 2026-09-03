use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::super::super::{ForwardShift, OreOrderingAdapter};
use super::error::try_vec;
use super::{ExactLazyError, ExactLazyOwner, ExactLazyTransaction, LazyCoeff};

/// Opaque proof created only while ingesting an already authenticated exact
/// nonzero coefficient from an [`super::super::super::OreConsequence`].
#[derive(Debug)]
pub(super) struct ExactIngressNonzero {
    owner: ExactLazyOwner,
    root: LazyCoeff,
}

impl ExactIngressNonzero {
    pub(super) fn try_ingress(
        transaction: &mut ExactLazyTransaction<'_, '_>,
        context: &IndexedCoefficientContext,
        exact: Arc<IndexedCoefficient>,
    ) -> Result<(LazyCoeff, Self), ExactLazyError> {
        if exact.is_zero() {
            return Err(ExactLazyError::InvalidProof {
                detail: "an exact zero cannot receive an ingress nonzero proof",
            });
        }
        // Root and proof are minted atomically from this exact Arc. There is
        // no API accepting an independently supplied root, which prevents a
        // nonzero coefficient from authenticating a different DAG value.
        let root = transaction.try_exact_leaf(context, exact)?;
        if transaction.try_is_structural_zero(&root)? {
            return Err(ExactLazyError::InvalidProof {
                detail: "a structural-zero root cannot receive an ingress nonzero proof",
            });
        }
        Ok((
            root.clone(),
            Self {
                owner: transaction.owner().clone(),
                root,
            },
        ))
    }

    fn owns(&self, owner: &ExactLazyOwner, root: &LazyCoeff) -> bool {
        self.owner.belongs_to(owner) && root.owner().belongs_to(owner) && self.root == *root
    }
}

/// Exact one-sided nonzero authority for a retained physical coefficient.
/// Modular and exact-fallback variants are added by the ELC1 classifier; this
/// initial foundation admits only already-authenticated exact ingress.
#[derive(Debug)]
pub(super) enum ExactNonzeroProof {
    ExactIngress(ExactIngressNonzero),
}

impl ExactNonzeroProof {
    pub(super) fn owns(&self, owner: &ExactLazyOwner, root: &LazyCoeff) -> bool {
        match self {
            Self::ExactIngress(proof) => proof.owns(owner, root),
        }
    }
}

/// Opaque proof that a removed coefficient is the distinguished structural
/// zero root. Exact materialized-zero variants are added with classification.
#[derive(Debug)]
pub(super) struct StructuralZeroProof {
    owner: ExactLazyOwner,
    root: LazyCoeff,
}

impl StructuralZeroProof {
    pub(super) fn try_new(
        transaction: &ExactLazyTransaction<'_, '_>,
        root: &LazyCoeff,
    ) -> Result<Self, ExactLazyError> {
        if !transaction.try_is_structural_zero(root)? {
            return Err(ExactLazyError::InvalidProof {
                detail: "a nonzero DAG root cannot receive a structural-zero proof",
            });
        }
        Ok(Self {
            owner: transaction.owner().clone(),
            root: root.clone(),
        })
    }

    pub(super) fn owns(&self, owner: &ExactLazyOwner, root: &LazyCoeff) -> bool {
        self.owner.belongs_to(owner) && root.owner().belongs_to(owner) && self.root == *root
    }

    fn belongs_to_owner(&self, owner: &ExactLazyOwner) -> bool {
        self.owner.belongs_to(owner) && self.root.owner().belongs_to(owner)
    }
}

#[derive(Debug)]
pub(super) enum ExactZeroProof {
    Structural(StructuralZeroProof),
}

/// One strictly supported term in an admitted exact-lazy Ore row.
#[derive(Debug)]
pub(super) struct LazyOreTerm {
    shift: ForwardShift,
    coefficient: LazyCoeff,
    nonzero: ExactNonzeroProof,
}

impl LazyOreTerm {
    pub(super) fn try_new(
        transaction: &ExactLazyTransaction<'_, '_>,
        shift: ForwardShift,
        coefficient: LazyCoeff,
        nonzero: ExactNonzeroProof,
    ) -> Result<Self, ExactLazyError> {
        if shift.arity() != transaction.owner().arity() {
            return Err(ExactLazyError::WrongArity {
                object: "exact-lazy Ore term",
                expected: transaction.owner().arity(),
                actual: shift.arity(),
            });
        }
        transaction.require_lazy_coefficient(&coefficient)?;
        if !nonzero.owns(transaction.owner(), &coefficient) {
            return Err(ExactLazyError::InvalidProof {
                detail: "an Ore term proof does not authenticate its coefficient root",
            });
        }
        Ok(Self {
            shift,
            coefficient,
            nonzero,
        })
    }

    pub(super) fn shift(&self) -> &ForwardShift {
        &self.shift
    }

    pub(super) fn coefficient(&self) -> &LazyCoeff {
        &self.coefficient
    }

    pub(super) fn nonzero_proof(&self) -> &ExactNonzeroProof {
        &self.nonzero
    }
}

/// Canonical exact support. Only this type exposes terms to the scheduler.
#[derive(Debug)]
pub(super) struct ClassifiedLazyOreRow {
    owner: ExactLazyOwner,
    terms: Box<[LazyOreTerm]>,
}

impl ClassifiedLazyOreRow {
    pub(super) fn try_from_exact_ingress(
        transaction: &ExactLazyTransaction<'_, '_>,
        terms: Vec<LazyOreTerm>,
    ) -> Result<Self, ExactLazyError> {
        let owner = transaction.owner();
        let mut previous: Option<&ForwardShift> = None;
        for term in &terms {
            if term.shift.arity() != owner.arity() {
                return Err(ExactLazyError::WrongArity {
                    object: "classified exact-lazy Ore term",
                    expected: owner.arity(),
                    actual: term.shift.arity(),
                });
            }
            if previous.is_some_and(|previous| previous >= &term.shift) {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "classified Ore support is not strictly shift sorted",
                });
            }
            if !term.nonzero.owns(owner, &term.coefficient) {
                return Err(ExactLazyError::InvalidProof {
                    detail: "classified Ore term has a foreign or root-mismatched proof",
                });
            }
            if transaction.try_is_structural_zero(&term.coefficient)? {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "classified Ore support retained structural zero",
                });
            }
            previous = Some(&term.shift);
        }
        Ok(Self {
            owner: owner.clone(),
            terms: terms.into_boxed_slice(),
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn terms(&self) -> &[LazyOreTerm] {
        &self.terms
    }

    pub(super) fn try_leading_term<'row>(
        &'row self,
        ordering: &OreOrderingAdapter,
        owner: &ExactLazyOwner,
    ) -> Result<Option<&'row LazyOreTerm>, ExactLazyError> {
        if !self.owner.belongs_to(owner) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        owner.require_ordering(ordering)?;
        let mut leading = None;
        let mut leading_key = None;
        for term in &self.terms {
            let key = ordering.try_key(&term.shift)?;
            if leading_key.as_ref().is_none_or(|current| key > *current) {
                leading = Some(term);
                leading_key = Some(key);
            }
        }
        Ok(leading)
    }
}

/// A term whose exact support status must be resolved before scheduling.
#[derive(Debug)]
pub(super) struct PendingLazyOreTerm {
    shift: ForwardShift,
    coefficient: LazyCoeff,
    prior_proof: Option<ExactNonzeroProof>,
}

impl PendingLazyOreTerm {
    pub(super) fn from_changed(shift: ForwardShift, coefficient: LazyCoeff) -> Self {
        Self {
            shift,
            coefficient,
            prior_proof: None,
        }
    }

    pub(super) fn from_unchanged(
        shift: ForwardShift,
        coefficient: LazyCoeff,
        prior_proof: ExactNonzeroProof,
    ) -> Self {
        Self {
            shift,
            coefficient,
            prior_proof: Some(prior_proof),
        }
    }
}

/// Post-AXPY row that deliberately exposes no support or leading-term API.
#[derive(Debug)]
pub(super) struct UnclassifiedLazyOreRow {
    terms: Box<[PendingLazyOreTerm]>,
    structural_zero_elisions: Box<[StructuralZeroProof]>,
}

impl UnclassifiedLazyOreRow {
    pub(super) fn try_new(
        transaction: &ExactLazyTransaction<'_, '_>,
        terms: impl IntoIterator<Item = PendingLazyOreTerm>,
        structural_zero_elisions: impl IntoIterator<Item = StructuralZeroProof>,
    ) -> Result<Self, ExactLazyError> {
        let owner = transaction.owner();
        let terms = terms.into_iter();
        let mut retained = try_vec("unclassified exact-lazy Ore terms", terms.size_hint().0)?;
        for term in terms {
            if term.shift.arity() != owner.arity() {
                return Err(ExactLazyError::WrongArity {
                    object: "unclassified exact-lazy Ore term",
                    expected: owner.arity(),
                    actual: term.shift.arity(),
                });
            }
            transaction.require_lazy_coefficient(&term.coefficient)?;
            if term
                .prior_proof
                .as_ref()
                .is_some_and(|proof| !proof.owns(owner, &term.coefficient))
            {
                return Err(ExactLazyError::InvalidProof {
                    detail: "an unchanged pending term has a foreign or root-mismatched proof",
                });
            }
            retained.push(term);
        }
        let zeros = structural_zero_elisions.into_iter();
        let mut retained_zeros = try_vec(
            "unclassified exact-lazy structural-zero proofs",
            zeros.size_hint().0,
        )?;
        for proof in zeros {
            if !proof.belongs_to_owner(owner) {
                return Err(ExactLazyError::InvalidProof {
                    detail: "an unclassified zero elision belongs to another session",
                });
            }
            retained_zeros.push(proof);
        }
        Ok(Self {
            terms: retained.into_boxed_slice(),
            structural_zero_elisions: retained_zeros.into_boxed_slice(),
        })
    }

    pub(super) const fn pending_term_count(&self) -> usize {
        self.terms.len()
    }

    pub(super) const fn structural_zero_elision_count(&self) -> usize {
        self.structural_zero_elisions.len()
    }
}
