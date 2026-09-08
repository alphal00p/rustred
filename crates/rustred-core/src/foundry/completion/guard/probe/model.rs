use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::stratum::{DecoratedStratum, DecoratedStratumId};

use super::super::CoefficientIdealGuardAtom;
use super::ExactGuardProbeError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExactGuardProbeValue {
    IdenticallyZero,
    NonZero(Integer),
}

/// Immutable exact guard payloads in canonical predicate-identity order.
///
/// Entries retain the compiled coefficient ideals because the same catalog is
/// also suitable for later exceptional-locus geometry. Lookup for probe
/// admission nevertheless uses the full representative predicate identity,
/// never the coarser ideal identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactGuardPredicateCatalog {
    pub(super) context_fingerprint: Arc<String>,
    pub(super) entries: Arc<[CoefficientIdealGuardAtom]>,
}

impl ExactGuardPredicateCatalog {
    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Modulus-independent proof that one exact integer/base point lies on every
/// branch of one decorated stratum.
///
/// A nonzero branch first means nonzero as a polynomial in the generic base
/// parameters after exact index specialization. Its retained integer also
/// proves that this particular base sample avoids an accidental numeric root.
/// The integer is kept so a prime portfolio can reject divisors without
/// repeating exact specialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactGuardProbeWitness {
    pub(super) context_fingerprint: Arc<String>,
    pub(super) stratum_id: DecoratedStratumId,
    pub(super) base_parameters: Arc<[i64]>,
    pub(super) index_anchor: Arc<[i64]>,
    pub(super) values: Arc<[ExactGuardProbeValue]>,
}

impl ExactGuardProbeWitness {
    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) const fn stratum_id(&self) -> &DecoratedStratumId {
        &self.stratum_id
    }

    pub(crate) fn base_parameters(&self) -> &[i64] {
        &self.base_parameters
    }

    pub(crate) fn index_anchor(&self) -> &[i64] {
        &self.index_anchor
    }

    pub(crate) fn guard_count(&self) -> usize {
        self.values.len()
    }

    pub(crate) fn try_validate_scope(
        &self,
        context: &IndexedCoefficientContext,
        stratum: &DecoratedStratum,
        base_parameters: &[i64],
        index_anchor: &[i64],
    ) -> Result<(), ExactGuardProbeError> {
        if self.context_fingerprint() != context.fingerprint() {
            return Err(ExactGuardProbeError::WitnessContextMismatch);
        }
        if self.stratum_id() != stratum.id() || self.values.len() != stratum.guards().len() {
            return Err(ExactGuardProbeError::WitnessStratumMismatch);
        }
        if self.base_parameters() != base_parameters {
            return Err(ExactGuardProbeError::WitnessBasePointMismatch);
        }
        if self.index_anchor() != index_anchor {
            return Err(ExactGuardProbeError::WitnessIndexPointMismatch);
        }
        Ok(())
    }
}
