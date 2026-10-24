//! Terminal-lane adapter over the sector-owned projector-graph discovery.
//!
//! [`crate::sector::symmetry::discovery`] owns the canonical graph, proposal
//! recovery and exact verification gates. This adapter only translates this
//! lane's limits, cooperative control and typed errors.

use std::sync::Arc;

use crate::family::IntegralFamily;
use crate::sector::Mask;
use crate::sector::symmetry::VerifiedMap;
use crate::sector::symmetry::discovery::{self as native, DiscoveryError};

use super::super::{ProductSkipReason, TerminalAliasError};
use super::{
    Error, TerminalSectorSymmetryControl, TerminalSectorSymmetryLimits,
    TerminalSectorSymmetrySkipReason,
};

pub(super) use native::{CanonicalSupportKey, PreparedSupport};

impl From<DiscoveryError> for Error {
    fn from(error: DiscoveryError) -> Self {
        match error {
            DiscoveryError::Cancelled => {
                Error::InvalidWitness("sector symmetry preparation cancelled")
            }
            DiscoveryError::ForeignCoefficient => {
                TerminalAliasError::InvalidCoefficientContext.into()
            }
            DiscoveryError::ExactAlgebra(error) => Error::ExactAlgebra(error),
            DiscoveryError::Invalid(reason) => Error::InvalidWitness(reason),
        }
    }
}

fn cancelled<'a>(control: &'a TerminalSectorSymmetryControl<'_>) -> impl Fn() -> bool + 'a {
    move || control.cancelled.is_some_and(|cancelled| cancelled())
}

/// Shared line factors and prepared supports for one family.
pub(super) struct DiscoveryCache(native::DiscoveryCache);

impl DiscoveryCache {
    pub(super) fn new(family: Arc<IntegralFamily>) -> Self {
        Self(native::DiscoveryCache::new(family))
    }

    pub(super) fn prepare_support(
        &mut self,
        root: &Mask,
        limits: TerminalSectorSymmetryLimits,
        control: &TerminalSectorSymmetryControl<'_>,
    ) -> Result<Option<Arc<PreparedSupport>>, Error> {
        Ok(self
            .0
            .prepare_support(root, limits.verification, &cancelled(control))?)
    }

    pub(super) fn skip_reason(&self, root: &Mask) -> Option<TerminalSectorSymmetrySkipReason> {
        self.0.skip_reason(root).map(|reason| match reason {
            native::SupportSkipReason::RankDeficient => {
                TerminalSectorSymmetrySkipReason::RankDeficientSupport
            }
            native::SupportSkipReason::LineGeometry(reason) => {
                TerminalSectorSymmetrySkipReason::MomentumGeometry(ProductSkipReason::from(reason))
            }
        })
    }
}

pub(super) fn verify_finite_root(
    family: &Arc<IntegralFamily>,
    root: &Mask,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<bool, Error> {
    Ok(native::verify_finite_root(
        family,
        root,
        limits.verification,
        &cancelled(control),
    )?)
}

pub(super) fn prepare_support(
    family: &Arc<IntegralFamily>,
    root: &Mask,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Option<Arc<PreparedSupport>>, Error> {
    Ok(native::prepare_support(
        family,
        root,
        limits.verification,
        &cancelled(control),
    )?)
}

pub(super) fn discover_prepared(
    support: &PreparedSupport,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Vec<Arc<VerifiedMap>>, Error> {
    Ok(native::discover_prepared(
        support,
        limits.verification,
        &cancelled(control),
    )?)
}

pub(super) fn match_supports(
    source: &PreparedSupport,
    target: &PreparedSupport,
    limits: TerminalSectorSymmetryLimits,
    control: &TerminalSectorSymmetryControl<'_>,
) -> Result<Option<Arc<VerifiedMap>>, Error> {
    Ok(native::match_supports(
        source,
        target,
        limits.verification,
        &cancelled(control),
    )?)
}
