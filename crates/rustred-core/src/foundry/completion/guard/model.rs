use std::sync::Arc;

use crate::algebra::{BaseCoefficientSystem, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::stratum::GuardBranchIdentity;

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

/// One exact guard together with its Symbolica-derived coefficient system and
/// a deterministic primitive-associate generator identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CoefficientIdealGuardAtom {
    context_fingerprint: Arc<String>,
    pulled_back_guard: IndexedPolynomial,
    coefficient_system: BaseCoefficientSystem,
    id: CoefficientIdealGuardAtomId,
    has_literal_unit_generator: bool,
}

impl CoefficientIdealGuardAtom {
    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) const fn coefficient_system(&self) -> &BaseCoefficientSystem {
        &self.coefficient_system
    }

    pub(crate) const fn id(&self) -> &CoefficientIdealGuardAtomId {
        &self.id
    }

    pub(crate) const fn has_literal_unit_generator(&self) -> bool {
        self.has_literal_unit_generator
    }

    pub(crate) fn same_retained_ideal(&self, other: &Self) -> bool {
        self.id == other.id
    }

    pub(crate) fn try_verify(
        &self,
        context: &IndexedCoefficientContext,
        limits: CoefficientIdealGuardLimits,
    ) -> Result<bool, CoefficientIdealGuardError> {
        Ok(Self::try_from_pulled_back(context, self.pulled_back_guard.clone(), limits)? == *self)
    }

    pub(super) fn from_parts(
        context_fingerprint: Arc<String>,
        pulled_back_guard: IndexedPolynomial,
        coefficient_system: BaseCoefficientSystem,
        generators: Vec<GuardBranchIdentity>,
        has_literal_unit_generator: bool,
    ) -> Self {
        Self {
            context_fingerprint,
            pulled_back_guard,
            coefficient_system,
            id: CoefficientIdealGuardAtomId(Arc::from(generators)),
            has_literal_unit_generator,
        }
    }
}
