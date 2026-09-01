//! Immutable outputs of exact factorized-numerator routing compilation.

use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::sector::SectorInteriorDomain;

use super::error::FactorizedNumeratorLiftError;

#[derive(Clone, Debug)]
pub(crate) struct RoutedAffineDenominator {
    pub(super) constant: Coefficient,
    pub(super) scalar_coefficients: Box<[Coefficient]>,
}

impl RoutedAffineDenominator {
    pub(crate) fn constant(&self) -> &Coefficient {
        &self.constant
    }

    pub(crate) fn scalar_coefficients(&self) -> &[Coefficient] {
        &self.scalar_coefficients
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CanonicalDenominatorRelation {
    pub(super) constant: Coefficient,
    pub(super) denominator_coefficients: Box<[Coefficient]>,
}

impl CanonicalDenominatorRelation {
    pub(crate) fn constant(&self) -> &Coefficient {
        &self.constant
    }

    pub(crate) fn denominator_coefficients(&self) -> &[Coefficient] {
        &self.denominator_coefficients
    }
}

/// Exact routing proof retained after deterministic row-sign gauge selection.
#[derive(Debug)]
pub(crate) struct CompiledFactorizationRouting {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) application_domain: SectorInteriorDomain,
    pub(super) signed_loop_basis: Box<[i64]>,
    pub(super) loop_basis_determinant: Coefficient,
    pub(super) transformed_denominators: Box<[RoutedAffineDenominator]>,
    pub(super) relations: Box<[CanonicalDenominatorRelation]>,
    pub(super) unit_images: Box<[Option<usize>]>,
}

impl CompiledFactorizationRouting {
    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(crate) fn application_domain(&self) -> &SectorInteriorDomain {
        &self.application_domain
    }

    pub(crate) fn signed_loop_basis(&self) -> &[i64] {
        &self.signed_loop_basis
    }

    pub(crate) fn loop_basis_determinant(&self) -> &Coefficient {
        &self.loop_basis_determinant
    }

    pub(crate) fn transformed_denominators(&self) -> &[RoutedAffineDenominator] {
        &self.transformed_denominators
    }

    pub(crate) fn relations(&self) -> &[CanonicalDenominatorRelation] {
        &self.relations
    }

    pub(crate) fn unit_images(&self) -> &[Option<usize>] {
        &self.unit_images
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FactorizedNumeratorLiftUnsupportedReason {
    MultipleAffineSourceRows { count: usize },
    AffineSourceIsActive { source: usize },
}

/// Honest structural disposition of one authenticated factorization routing.
#[derive(Debug)]
pub(crate) enum FactorizedNumeratorLiftCompilation {
    Action(FactorizedNumeratorLiftAction),
    /// Every transformed denominator is already a canonical unit image.
    /// [`CompiledFactorizationRouting::try_route_key`] owns the exact pure
    /// routed-key operation, but reducer dispatch remains unintegrated and
    /// this disposition does not claim terminal closure.
    NoAffineLiftRequired(CompiledFactorizationRouting),
    Unsupported {
        routing: CompiledFactorizationRouting,
        reason: FactorizedNumeratorLiftUnsupportedReason,
    },
}

impl FactorizedNumeratorLiftCompilation {
    pub(crate) fn routing(&self) -> &CompiledFactorizationRouting {
        match self {
            Self::Action(action) => action.routing(),
            Self::NoAffineLiftRequired(routing) | Self::Unsupported { routing, .. } => routing,
        }
    }
}

/// One immutable affine numerator-lift action.
///
/// Its domain is the complete factorized sector: active powers are at least
/// one and inactive powers are at most zero.  A target with zero power at the
/// distinguished affine source is admitted but needs no auxiliary step.
#[derive(Debug)]
pub(crate) struct FactorizedNumeratorLiftAction {
    pub(super) routing: CompiledFactorizationRouting,
    pub(super) affine_source: usize,
    pub(super) branch_width: usize,
    pub(super) identity: Arc<()>,
}

impl FactorizedNumeratorLiftAction {
    pub(crate) fn routing(&self) -> &CompiledFactorizationRouting {
        &self.routing
    }

    pub(crate) fn application_domain(&self) -> &SectorInteriorDomain {
        self.routing.application_domain()
    }

    pub(crate) fn affine_source(&self) -> usize {
        self.affine_source
    }

    pub(crate) fn affine_relation(&self) -> &CanonicalDenominatorRelation {
        &self.routing.relations[self.affine_source]
    }

    /// Exact number of children emitted by every nonempty auxiliary step.
    pub(crate) fn branch_width(&self) -> usize {
        self.branch_width
    }
}

/// Explicit well-founded measure for the auxiliary recurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FactorizedNumeratorLiftMeasure(u64);

impl FactorizedNumeratorLiftMeasure {
    pub(crate) fn remaining_power(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FactorizedNumeratorLiftState {
    pub(super) identity: Arc<()>,
    pub(super) remaining_power: u64,
    pub(super) routed_powers: Box<[i64]>,
}

#[derive(Clone, Debug)]
pub(crate) enum FactorizedNumeratorLiftStart {
    /// Exact pure unit-image routing; no affine power remains.
    Routed(crate::family::IntegralKey),
    Auxiliary(FactorizedNumeratorLiftState),
}

impl FactorizedNumeratorLiftState {
    pub(crate) fn measure(&self) -> FactorizedNumeratorLiftMeasure {
        FactorizedNumeratorLiftMeasure(self.remaining_power)
    }

    pub(crate) fn routed_powers(&self) -> &[i64] {
        &self.routed_powers
    }

    pub(crate) fn try_integral_key(
        &self,
    ) -> Result<Option<crate::family::IntegralKey>, FactorizedNumeratorLiftError> {
        if self.remaining_power == 0 {
            Ok(Some(crate::family::IntegralKey::try_new(
                self.routed_powers.iter().copied(),
            )?))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FactorizedNumeratorLiftChild {
    pub(super) coefficient: Coefficient,
    pub(super) state: FactorizedNumeratorLiftState,
}

impl FactorizedNumeratorLiftChild {
    pub(crate) fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub(crate) fn state(&self) -> &FactorizedNumeratorLiftState {
        &self.state
    }
}
