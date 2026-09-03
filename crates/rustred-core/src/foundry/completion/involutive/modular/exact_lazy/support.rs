use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::super::super::{ForwardShift, OreOrderingAdapter};
use super::super::{CertifiedNonzero, ExactMaterialization, ModularCoefficientDag};
use super::arena::GuardedStructuralOneProof;
use super::classify::SupportClassificationSeal;
use super::error::try_vec;
use super::import::ExactIngressRowSeal;
use super::{ExactLazyError, ExactLazyOwner, ExactLazySession, ExactLazyTransaction, LazyCoeff};

/// Opaque identity of one immutable classified row.
///
/// A leader-inversion authority is bound to this identity rather than to a
/// caller-supplied coefficient root. Moving the row does not change the
/// identity, while independently classified equal-looking rows remain
/// distinct.
#[derive(Clone, Debug)]
pub(super) struct ExactLazyRowIdentity(Arc<()>);

impl ExactLazyRowIdentity {
    fn fresh() -> Self {
        Self(Arc::new(()))
    }

    pub(super) fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Opaque proof created only while ingesting an already authenticated exact
/// nonzero coefficient from an [`super::super::super::OreConsequence`].
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub(super) enum ExactNonzeroProof {
    ExactIngress(ExactIngressNonzero),
    Modular(ModularNonzeroProof),
    ExactFallback(ExactFallbackNonzeroProof),
    /// Structural one installed only through a proof-bound leader inverse.
    GuardedStructuralOne(GuardedStructuralOneProof),
}

impl ExactNonzeroProof {
    /// Authenticate both the logical root binding and the current append-only
    /// arena liveness boundary. In particular, a modular proof whose shared
    /// batch once mentioned a subsequently rolled-back root cannot re-enter a
    /// classified row merely because this term's own root stayed live.
    pub(super) fn owns_live(
        &self,
        transaction: &ExactLazyTransaction<'_, '_>,
        root: &LazyCoeff,
    ) -> bool {
        self.owns_live_dag(transaction.owner(), transaction.coefficient_dag(), root)
    }

    fn owns_live_dag(
        &self,
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        root: &LazyCoeff,
    ) -> bool {
        if !owner.belongs_to(root.owner()) || dag.raw(root.root()).is_err() {
            return false;
        }
        match self {
            Self::ExactIngress(proof) => proof.owns(owner, root),
            Self::Modular(proof) => proof.owns_live(owner, dag, root),
            Self::ExactFallback(proof) => proof.owns(owner, root),
            Self::GuardedStructuralOne(proof) => proof.owns_live(owner, dag, root),
        }
    }
}

/// Exact one-sided nonzero authority obtained from a valid finite-field image.
#[derive(Clone, Debug)]
pub(super) struct ModularNonzeroProof {
    owner: ExactLazyOwner,
    root: LazyCoeff,
    certificate: CertifiedNonzero,
}

impl ModularNonzeroProof {
    pub(super) fn try_new(
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        root: LazyCoeff,
        certificate: CertifiedNonzero,
    ) -> Result<Self, ExactLazyError> {
        if !owner.belongs_to(root.owner()) || !owner.owns_dag(dag.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if !certificate.owns(dag, context, root.root()) {
            return Err(ExactLazyError::InvalidProof {
                detail: "a modular certificate does not authenticate its exact-lazy root",
            });
        }
        Ok(Self {
            owner: owner.clone(),
            root,
            certificate,
        })
    }

    fn owns_live(
        &self,
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        root: &LazyCoeff,
    ) -> bool {
        self.owner.belongs_to(owner)
            && self.root == *root
            && self.root.owner().belongs_to(owner)
            && self.certificate.owns_live(dag, root.root())
    }

    pub(super) fn certificate(&self) -> &CertifiedNonzero {
        &self.certificate
    }
}

/// Exact support proof produced by Symbolica materialization after all
/// scheduled modular images remained zero.
#[derive(Clone, Debug)]
pub(super) struct ExactFallbackNonzeroProof {
    seal: ExactResultSeal,
}

impl ExactFallbackNonzeroProof {
    pub(super) fn try_new(
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        root: LazyCoeff,
        materialization: ExactMaterialization,
    ) -> Result<Self, ExactLazyError> {
        if !owner.belongs_to(root.owner()) || !owner.owns_dag(dag.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if !materialization.owns(dag, context, root.root()) || materialization.value().is_zero() {
            return Err(ExactLazyError::InvalidProof {
                detail: "an exact nonzero fallback does not authenticate its exact-lazy root",
            });
        }
        // Classification authenticates the Symbolica result once. Retaining
        // that potentially enormous exact coefficient would recreate the K6
        // payload blow-up in the hot path, so keep only this compact seal.
        drop(materialization);
        Ok(Self {
            seal: ExactResultSeal {
                owner: owner.clone(),
                root,
            },
        })
    }

    fn owns(&self, owner: &ExactLazyOwner, root: &LazyCoeff) -> bool {
        self.seal.owns(owner, root)
    }
}

/// Opaque proof that a removed coefficient is the distinguished structural
/// zero root. Exact materialized-zero variants are added with classification.
#[derive(Debug)]
pub(super) struct StructuralZeroProof {
    owner: ExactLazyOwner,
    shift: ForwardShift,
    root: LazyCoeff,
}

impl StructuralZeroProof {
    pub(super) fn try_new(
        transaction: &ExactLazyTransaction<'_, '_>,
        shift: ForwardShift,
        root: &LazyCoeff,
    ) -> Result<Self, ExactLazyError> {
        if shift.arity() != transaction.owner().arity() {
            return Err(ExactLazyError::WrongArity {
                object: "structural-zero elision shift",
                expected: transaction.owner().arity(),
                actual: shift.arity(),
            });
        }
        if !transaction.try_is_structural_zero(root)? {
            return Err(ExactLazyError::InvalidProof {
                detail: "a nonzero DAG root cannot receive a structural-zero proof",
            });
        }
        Ok(Self {
            owner: transaction.owner().clone(),
            shift,
            root: root.clone(),
        })
    }

    fn belongs_to_owner(&self, owner: &ExactLazyOwner) -> bool {
        self.owner.belongs_to(owner) && self.root.owner().belongs_to(owner)
    }

    pub(super) fn shift(&self) -> &ForwardShift {
        &self.shift
    }

    pub(super) fn root(&self) -> &LazyCoeff {
        &self.root
    }
}

#[derive(Debug)]
pub(super) enum ExactZeroProof {
    Structural(StructuralZeroProof),
    ExactFallback(ExactFallbackZeroProof),
}

impl ExactZeroProof {
    fn owns_live(&self, transaction: &ExactLazyTransaction<'_, '_>) -> bool {
        let owner = transaction.owner();
        let owner_matches = match self {
            Self::Structural(proof) => proof.belongs_to_owner(owner),
            Self::ExactFallback(proof) => proof.belongs_to_owner(owner),
        };
        owner_matches && transaction.require_lazy_coefficient(self.root()).is_ok()
    }

    pub(super) fn shift(&self) -> &ForwardShift {
        match self {
            Self::Structural(proof) => proof.shift(),
            Self::ExactFallback(proof) => proof.shift(),
        }
    }

    pub(super) fn root(&self) -> &LazyCoeff {
        match self {
            Self::Structural(proof) => proof.root(),
            Self::ExactFallback(proof) => proof.root(),
        }
    }
}

/// Exact Symbolica zero authority for a removed nonsyntactic coefficient.
#[derive(Debug)]
pub(super) struct ExactFallbackZeroProof {
    shift: ForwardShift,
    authority: ExactFallbackZeroAuthority,
}

#[derive(Clone, Debug)]
pub(super) struct ExactFallbackZeroAuthority {
    seal: ExactResultSeal,
}

impl ExactFallbackZeroAuthority {
    pub(super) fn try_new(
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        root: LazyCoeff,
        materialization: ExactMaterialization,
    ) -> Result<Self, ExactLazyError> {
        if !owner.belongs_to(root.owner()) || !owner.owns_dag(dag.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if !materialization.owns(dag, context, root.root()) || !materialization.value().is_zero() {
            return Err(ExactLazyError::InvalidProof {
                detail: "an exact zero fallback does not authenticate its exact-lazy root",
            });
        }
        drop(materialization);
        Ok(Self {
            seal: ExactResultSeal {
                owner: owner.clone(),
                root,
            },
        })
    }

    pub(super) fn bind_shift(&self, shift: ForwardShift) -> ExactFallbackZeroProof {
        ExactFallbackZeroProof {
            shift,
            authority: self.clone(),
        }
    }
}

impl ExactFallbackZeroProof {
    pub(super) fn shift(&self) -> &ForwardShift {
        &self.shift
    }

    fn belongs_to_owner(&self, owner: &ExactLazyOwner) -> bool {
        self.authority.seal.belongs_to_owner(owner)
    }

    fn root(&self) -> &LazyCoeff {
        &self.authority.seal.root
    }
}

/// Compact result of an exact zero/nonzero decision. The materialized
/// Symbolica payload is deliberately absent.
#[derive(Clone, Debug)]
struct ExactResultSeal {
    owner: ExactLazyOwner,
    root: LazyCoeff,
}

impl ExactResultSeal {
    fn owns(&self, owner: &ExactLazyOwner, root: &LazyCoeff) -> bool {
        self.belongs_to_owner(owner) && self.root == *root
    }

    fn belongs_to_owner(&self, owner: &ExactLazyOwner) -> bool {
        self.owner.belongs_to(owner) && self.root.owner().belongs_to(owner)
    }
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
        if !nonzero.owns_live(transaction, &coefficient) {
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
    identity: ExactLazyRowIdentity,
    terms: Box<[LazyOreTerm]>,
    exact_zero_elisions: Box<[ExactZeroProof]>,
}

enum RowAuthority {
    Ingress(ExactIngressRowSeal),
    Classification(SupportClassificationSeal),
}

impl ClassifiedLazyOreRow {
    pub(super) fn try_from_exact_ingress(
        transaction: &ExactLazyTransaction<'_, '_>,
        terms: Vec<LazyOreTerm>,
        seal: ExactIngressRowSeal,
    ) -> Result<Self, ExactLazyError> {
        Self::try_from_sealed_parts(transaction, terms, Vec::new(), RowAuthority::Ingress(seal))
    }

    pub(super) fn try_from_classification(
        transaction: &ExactLazyTransaction<'_, '_>,
        terms: Vec<LazyOreTerm>,
        exact_zero_elisions: Vec<ExactZeroProof>,
        seal: SupportClassificationSeal,
    ) -> Result<Self, ExactLazyError> {
        Self::try_from_sealed_parts(
            transaction,
            terms,
            exact_zero_elisions,
            RowAuthority::Classification(seal),
        )
    }

    fn try_from_sealed_parts(
        transaction: &ExactLazyTransaction<'_, '_>,
        terms: Vec<LazyOreTerm>,
        exact_zero_elisions: Vec<ExactZeroProof>,
        authority: RowAuthority,
    ) -> Result<Self, ExactLazyError> {
        let _ = authority;
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
            if !term.nonzero.owns_live(transaction, &term.coefficient) {
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
        for proof in &exact_zero_elisions {
            if proof.shift().arity() != owner.arity() {
                return Err(ExactLazyError::WrongArity {
                    object: "classified exact-zero elision shift",
                    expected: owner.arity(),
                    actual: proof.shift().arity(),
                });
            }
            if !proof.owns_live(transaction) {
                return Err(ExactLazyError::InvalidProof {
                    detail: "a classified zero elision belongs to another session",
                });
            }
        }
        Ok(Self {
            owner: owner.clone(),
            identity: ExactLazyRowIdentity::fresh(),
            terms: terms.into_boxed_slice(),
            exact_zero_elisions: exact_zero_elisions.into_boxed_slice(),
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn identity(&self) -> &ExactLazyRowIdentity {
        &self.identity
    }

    /// Scheduler-visible support, released only after every retained proof
    /// and every exact-zero elision is rechecked against the current committed
    /// arena. This rejects rows that escaped a transaction later aborted.
    pub(super) fn try_terms_live<'row>(
        &'row self,
        session: &ExactLazySession<'_>,
    ) -> Result<&'row [LazyOreTerm], ExactLazyError> {
        self.try_validate_live(session.owner(), session.coefficient_dag(), |coefficient| {
            session.require_lazy_coefficient(coefficient)
        })?;
        Ok(&self.terms)
    }

    pub(super) fn try_terms_in_transaction<'row>(
        &'row self,
        transaction: &ExactLazyTransaction<'_, '_>,
    ) -> Result<&'row [LazyOreTerm], ExactLazyError> {
        self.try_validate_live(
            transaction.owner(),
            transaction.coefficient_dag(),
            |coefficient| transaction.require_lazy_coefficient(coefficient),
        )?;
        Ok(&self.terms)
    }

    pub(super) fn try_leading_term<'row>(
        &'row self,
        session: &ExactLazySession<'_>,
        ordering: &OreOrderingAdapter,
    ) -> Result<Option<&'row LazyOreTerm>, ExactLazyError> {
        let terms = self.try_terms_live(session)?;
        session.owner().require_ordering(ordering)?;
        try_select_leading(terms, ordering)
    }

    pub(super) fn try_leading_term_in_transaction<'row>(
        &'row self,
        transaction: &ExactLazyTransaction<'_, '_>,
        ordering: &OreOrderingAdapter,
    ) -> Result<Option<&'row LazyOreTerm>, ExactLazyError> {
        let terms = self.try_terms_in_transaction(transaction)?;
        transaction.owner().require_ordering(ordering)?;
        try_select_leading(terms, ordering)
    }

    pub(super) const fn physical_term_count(&self) -> usize {
        self.terms.len()
    }

    #[cfg(test)]
    pub(super) fn try_exact_zero_elisions_live<'row>(
        &'row self,
        session: &ExactLazySession<'_>,
    ) -> Result<&'row [ExactZeroProof], ExactLazyError> {
        self.try_terms_live(session)?;
        Ok(&self.exact_zero_elisions)
    }

    #[cfg(test)]
    pub(super) fn try_exact_zero_elisions_in_transaction<'row>(
        &'row self,
        transaction: &ExactLazyTransaction<'_, '_>,
    ) -> Result<&'row [ExactZeroProof], ExactLazyError> {
        self.try_terms_in_transaction(transaction)?;
        Ok(&self.exact_zero_elisions)
    }

    fn try_validate_live(
        &self,
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        mut require_root: impl FnMut(&LazyCoeff) -> Result<(), ExactLazyError>,
    ) -> Result<(), ExactLazyError> {
        if !self.owner.belongs_to(owner) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        for term in &self.terms {
            require_root(&term.coefficient)?;
            if !term.nonzero.owns_live_dag(owner, dag, &term.coefficient) {
                return Err(ExactLazyError::InvalidProof {
                    detail: "classified Ore term proof is no longer live",
                });
            }
        }
        for proof in &self.exact_zero_elisions {
            require_root(proof.root())?;
            let owner_matches = match proof {
                ExactZeroProof::Structural(proof) => proof.belongs_to_owner(owner),
                ExactZeroProof::ExactFallback(proof) => proof.belongs_to_owner(owner),
            };
            if !owner_matches {
                return Err(ExactLazyError::InvalidProof {
                    detail: "classified exact-zero elision is no longer live",
                });
            }
        }
        Ok(())
    }
}

fn try_select_leading<'row>(
    terms: &'row [LazyOreTerm],
    ordering: &OreOrderingAdapter,
) -> Result<Option<&'row LazyOreTerm>, ExactLazyError> {
    let mut leading = None;
    let mut leading_key = None;
    for term in terms {
        let key = ordering.try_key(&term.shift)?;
        if leading_key.as_ref().is_none_or(|current| key > *current) {
            leading = Some(term);
            leading_key = Some(key);
        }
    }
    Ok(leading)
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

    pub(super) fn shift(&self) -> &ForwardShift {
        &self.shift
    }

    pub(super) fn coefficient(&self) -> &LazyCoeff {
        &self.coefficient
    }

    pub(super) fn has_prior_proof(&self) -> bool {
        self.prior_proof.is_some()
    }

    pub(super) fn into_parts(self) -> (ForwardShift, LazyCoeff, Option<ExactNonzeroProof>) {
        (self.shift, self.coefficient, self.prior_proof)
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
                .is_some_and(|proof| !proof.owns_live(transaction, &term.coefficient))
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
            if proof.shift().arity() != owner.arity() {
                return Err(ExactLazyError::WrongArity {
                    object: "unclassified structural-zero elision shift",
                    expected: owner.arity(),
                    actual: proof.shift().arity(),
                });
            }
            transaction.require_lazy_coefficient(proof.root())?;
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

    pub(super) fn into_parts(self) -> (Box<[PendingLazyOreTerm]>, Box<[StructuralZeroProof]>) {
        (self.terms, self.structural_zero_elisions)
    }
}
