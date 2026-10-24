//! Complete signed-integral symmetry equations from whole-polynomial orbits.
//!
//! The map convention is `k_source = A k_target + B p_target`. Active
//! denominators are matched exactly, including masses, while their positive
//! powers may permute. The shared native transporter expands every inactive
//! numerator factor with its affine constant before collecting `I - T(I)`.
//! This finite symmetry span supplies equations to the existing refiner; it
//! does not certify IBP completeness or independence of residual columns.
//! Automatic preparation uses declared input supports as its routing universe.
//! Equivalent supports route to a stable representative, whose self generators
//! act on each WHOLE coalesced polynomial image. Their exact Reynolds average
//! supplies one row per original integral. Image monomials remain auxiliary
//! columns; no additional source or pinch orbit is inferred from those terms.
//! Default preparation also retains the established weak equations once on
//! the original input cohort: finite saved-IBP projections need not commute
//! with the Reynolds projector. Both cohorts share one immutable provider.

mod closure;
mod codec;
mod discovery;
mod model;
mod transport;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::terminal_relations::TerminalEquation;
use crate::sector::Mask;
use crate::sector::symmetry::integral_transport;
use crate::sector::symmetry::{self, MomentumMap, VerifiedMap};

use super::{TerminalAliasError, TerminalNormalizationError as Error};

pub use model::{
    TerminalSectorSymmetryControl, TerminalSectorSymmetryEquations,
    TerminalSectorSymmetryOrbitWitness, TerminalSectorSymmetryPolicy,
    TerminalSectorSymmetryProgress, TerminalSectorSymmetryRoundTrip, TerminalSectorSymmetryRoute,
    TerminalSectorSymmetrySkipReason, TerminalSectorSymmetryStatistics,
    TerminalSectorSymmetryWitness,
};

/// The stronger lane shares the existing exact verification/expansion policy.
pub type TerminalSectorSymmetryLimits = super::TerminalNumeratorSymmetryLimits;

/// The sector-owned discovery gate, with this lane's error mapping (an
/// ineligible family is an `InvalidWitness` with the same reason).
pub(super) fn eligible(family: &IntegralFamily) -> Result<(), Error> {
    Ok(crate::sector::symmetry::discovery::eligible(family)?)
}

pub(super) fn validate_key(family: &IntegralFamily, key: &IntegralKey) -> Result<(), Error> {
    if key.powers().len() != family.denominator_count() {
        return Err(TerminalAliasError::WrongArity {
            expected: family.denominator_count(),
            actual: key.powers().len(),
        }
        .into());
    }
    Ok(())
}

pub(super) fn algebra(error: impl std::fmt::Display) -> Error {
    Error::ExactAlgebra(error.to_string())
}
