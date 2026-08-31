use std::sync::Arc;

use crate::algebra::{
    BaseCoefficientSystem, IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficientContext,
    IndexedPolynomial,
};
use crate::foundry::completion::stratum::{GuardBranch, GuardBranchIdentity};

use super::{CoefficientIdealGuardError, CoefficientIdealGuardLimits};

/// Content-bound identity of one conservatively normalized simultaneous
/// coefficient ideal. Equal values prove equal retained generator sets; unequal
/// values do not prove different radicals or varieties.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CoefficientIdealGuardAtomId(Arc<[GuardBranchIdentity]>);

impl CoefficientIdealGuardAtomId {
    pub(crate) fn generators(&self) -> &[GuardBranchIdentity] {
        &self.0
    }
}

/// Canonical exact payload for evaluating one semantic coefficient-ideal
/// predicate at a concrete index point.
///
/// The representative guard is a primitive associate in target coordinates.
/// Different representatives can encode the same retained coefficient ideal;
/// the decision compiler chooses the least exact identity deterministically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CoefficientIdealGuardPredicate {
    context_fingerprint: Arc<String>,
    representative_guard: Arc<IndexedPolynomial>,
    representative_identity: GuardBranchIdentity,
    id: CoefficientIdealGuardAtomId,
}

impl CoefficientIdealGuardPredicate {
    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) const fn id(&self) -> &CoefficientIdealGuardAtomId {
        &self.id
    }

    pub(crate) const fn representative_identity(&self) -> &GuardBranchIdentity {
        &self.representative_identity
    }

    pub(crate) fn representative_guard(&self) -> &IndexedPolynomial {
        self.representative_guard.as_ref()
    }

    pub(crate) fn input_terms(&self) -> usize {
        self.representative_guard.raw().nterms()
    }

    pub(crate) fn try_branch_at(
        &self,
        context: &IndexedCoefficientContext,
        assignment: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<GuardBranch, IndexedAlgebraError> {
        let specialized = context.specialize_polynomial_sealed(
            self.representative_guard.as_ref(),
            assignment,
            limits,
        )?;
        Ok(if specialized.is_zero() {
            GuardBranch::Zero
        } else {
            GuardBranch::NonZero
        })
    }
}

/// One exact guard together with its Symbolica-derived coefficient system and
/// a deterministic primitive-associate generator identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CoefficientIdealGuardAtom {
    predicate: CoefficientIdealGuardPredicate,
    coefficient_system: BaseCoefficientSystem,
    has_literal_unit_generator: bool,
}

impl CoefficientIdealGuardAtom {
    pub(crate) fn context_fingerprint(&self) -> &str {
        self.predicate.context_fingerprint()
    }

    pub(crate) const fn coefficient_system(&self) -> &BaseCoefficientSystem {
        &self.coefficient_system
    }

    pub(crate) const fn id(&self) -> &CoefficientIdealGuardAtomId {
        self.predicate.id()
    }

    pub(crate) const fn predicate(&self) -> &CoefficientIdealGuardPredicate {
        &self.predicate
    }

    pub(crate) const fn has_literal_unit_generator(&self) -> bool {
        self.has_literal_unit_generator
    }

    pub(crate) fn same_retained_ideal(&self, other: &Self) -> bool {
        self.id() == other.id()
    }

    pub(crate) fn try_verify(
        &self,
        context: &IndexedCoefficientContext,
        limits: CoefficientIdealGuardLimits,
    ) -> Result<bool, CoefficientIdealGuardError> {
        Ok(Self::try_from_pulled_back(
            context,
            self.predicate.representative_guard.as_ref().clone(),
            limits,
        )? == *self)
    }

    pub(super) fn from_parts(
        context_fingerprint: Arc<String>,
        representative_guard: IndexedPolynomial,
        representative_identity: GuardBranchIdentity,
        coefficient_system: BaseCoefficientSystem,
        generators: Vec<GuardBranchIdentity>,
        has_literal_unit_generator: bool,
    ) -> Self {
        Self {
            predicate: CoefficientIdealGuardPredicate {
                context_fingerprint,
                representative_guard: Arc::new(representative_guard),
                representative_identity,
                id: CoefficientIdealGuardAtomId(Arc::from(generators)),
            },
            coefficient_system,
            has_literal_unit_generator,
        }
    }
}
