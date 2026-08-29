//! Public presentation values and the authenticated owner.

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::IntegralFamily;

use super::limits::FamilyPresentationLimits;

/// Sign of one term in the affine propagator convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AlgebraicSign {
    Positive,
    Negative,
}

/// Metric convention used by the scalar products stored in the family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MetricConvention {
    Euclidean,
    MinkowskiMostlyPlus,
    MinkowskiMostlyMinus,
}

/// Global spelling of every physical denominator.
///
/// A physical row is authenticated as
/// `momentum_squared_sign * q^2 + mass_squared_sign * mass_squared`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropagatorConvention {
    momentum_squared_sign: AlgebraicSign,
    mass_squared_sign: AlgebraicSign,
}

impl PropagatorConvention {
    pub const MOMENTUM_SQUARED_PLUS_MASS_SQUARED: Self =
        Self::new(AlgebraicSign::Positive, AlgebraicSign::Positive);
    pub const MOMENTUM_SQUARED_MINUS_MASS_SQUARED: Self =
        Self::new(AlgebraicSign::Positive, AlgebraicSign::Negative);

    pub const fn new(
        momentum_squared_sign: AlgebraicSign,
        mass_squared_sign: AlgebraicSign,
    ) -> Self {
        Self {
            momentum_squared_sign,
            mass_squared_sign,
        }
    }

    pub const fn momentum_squared_sign(self) -> AlgebraicSign {
        self.momentum_squared_sign
    }

    pub const fn mass_squared_sign(self) -> AlgebraicSign {
        self.mass_squared_sign
    }
}

/// Exact conventions shared by all physical rows in a presentation.
///
/// Loop-measure normalization is deliberately not represented here: it is a
/// caller/Vakint normalization concern outside tensor projection and affine
/// IBP-family semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FamilyConventions {
    metric: MetricConvention,
    propagator: PropagatorConvention,
}

impl FamilyConventions {
    pub const fn new(metric: MetricConvention, propagator: PropagatorConvention) -> Self {
        Self { metric, propagator }
    }

    pub const fn metric(self) -> MetricConvention {
        self.metric
    }

    pub const fn propagator(self) -> PropagatorConvention {
        self.propagator
    }
}

/// One routed momentum `sum_i a_i k_i + sum_a b_a p_a`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MomentumCombination {
    loop_coefficients: Vec<Coefficient>,
    external_shift: Vec<Coefficient>,
}

impl MomentumCombination {
    pub fn new(loop_coefficients: Vec<Coefficient>, external_shift: Vec<Coefficient>) -> Self {
        Self {
            loop_coefficients,
            external_shift,
        }
    }

    pub fn loop_coefficients(&self) -> &[Coefficient] {
        &self.loop_coefficients
    }

    pub fn external_shift(&self) -> &[Coefficient] {
        &self.external_shift
    }
}

/// Metadata for one physical propagator in denominator order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalPropagator {
    id: String,
    momentum: MomentumCombination,
    mass_squared: Coefficient,
}

impl PhysicalPropagator {
    /// Retain a caller-owned ID and exact momentum data.
    pub fn new(id: String, momentum: MomentumCombination, mass_squared: Coefficient) -> Self {
        Self {
            id,
            momentum,
            mass_squared,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn momentum(&self) -> &MomentumCombination {
        &self.momentum
    }

    pub const fn mass_squared(&self) -> &Coefficient {
        &self.mass_squared
    }
}

/// Metadata for one auxiliary denominator/ISP in denominator order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuxiliaryDenominator {
    id: String,
}

impl AuxiliaryDenominator {
    /// Retain a caller-owned ID without a hidden conversion allocation.
    pub fn new(id: String) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Semantic role of one row in the complete affine denominator basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DenominatorRole {
    Physical(PhysicalPropagator),
    Auxiliary(AuxiliaryDenominator),
}

impl DenominatorRole {
    pub fn id(&self) -> &str {
        match self {
            Self::Physical(propagator) => propagator.id(),
            Self::Auxiliary(auxiliary) => auxiliary.id(),
        }
    }

    pub const fn physical(&self) -> Option<&PhysicalPropagator> {
        match self {
            Self::Physical(propagator) => Some(propagator),
            Self::Auxiliary(_) => None,
        }
    }
}

/// Caller-attested exact source-to-family momentum map retained after
/// topology matching.
///
/// In row-major notation this records
/// `l_source = A l_family + B p_family` and
/// `p_source = C p_family`, together with the source momentum order. RustRed
/// validates coefficient domains, shapes, loop unimodularity, and external
/// invertibility. It cannot replay this claim without the source expression;
/// that proof remains owned by the topology matcher that constructs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MomentumRouting {
    source_loop_order: Vec<String>,
    source_external_order: Vec<String>,
    loop_linear: Vec<Vec<Coefficient>>,
    loop_external: Vec<Vec<Coefficient>>,
    external_linear: Vec<Vec<Coefficient>>,
}

impl MomentumRouting {
    pub fn new(
        source_loop_order: Vec<String>,
        source_external_order: Vec<String>,
        loop_linear: Vec<Vec<Coefficient>>,
        loop_external: Vec<Vec<Coefficient>>,
        external_linear: Vec<Vec<Coefficient>>,
    ) -> Self {
        Self {
            source_loop_order,
            source_external_order,
            loop_linear,
            loop_external,
            external_linear,
        }
    }

    pub fn source_loop_order(&self) -> &[String] {
        &self.source_loop_order
    }

    pub fn source_external_order(&self) -> &[String] {
        &self.source_external_order
    }

    pub fn loop_linear(&self) -> &[Vec<Coefficient>] {
        &self.loop_linear
    }

    pub fn loop_external(&self) -> &[Vec<Coefficient>] {
        &self.loop_external
    }

    pub fn external_linear(&self) -> &[Vec<Coefficient>] {
        &self.external_linear
    }
}

/// Claimed common nonzero physical mass scale, authenticated on construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonMassScale {
    scale_squared: Coefficient,
}

impl CommonMassScale {
    pub fn new(scale_squared: Coefficient) -> Self {
        Self { scale_squared }
    }

    pub const fn scale_squared(&self) -> &Coefficient {
        &self.scale_squared
    }
}

/// Source of one presentation-specific generic-domain condition.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PresentationConditionSource {
    CoefficientDenominator(super::error::PresentationCoefficientLocation),
    ExternalRoutingDeterminantNumerator,
    CommonMassScaleNumerator,
}

/// One exact nonzero condition added by presentation metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationNonZeroCondition {
    pub(super) polynomial: CoefficientPolynomial,
    pub(super) sources: Vec<PresentationConditionSource>,
}

impl PresentationNonZeroCondition {
    pub const fn polynomial(&self) -> &CoefficientPolynomial {
        &self.polynomial
    }

    pub fn sources(&self) -> &[PresentationConditionSource] {
        &self.sources
    }
}

/// Generic-domain conditions contributed by presentation-only coefficients.
///
/// The family's own conditions remain available through
/// [`IntegralFamily::domain`]; this supplements rather than copies them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationDomain {
    pub(super) conditions: Vec<PresentationNonZeroCondition>,
}

impl PresentationDomain {
    pub fn conditions(&self) -> impl Iterator<Item = &PresentationNonZeroCondition> {
        self.conditions.iter()
    }
}

/// An exact family plus admitted presentation metadata.
///
/// [`IntegralFamily::fingerprint`] authenticates the affine-family portion
/// only.  Until a versioned presentation fingerprint is introduced, callers
/// must not use it alone as a cache key for this richer value; compare or
/// retain the complete presentation metadata instead. Physical-propagator and
/// common-scale claims are exactly replayed; routing and metric conventions
/// are caller-attested metadata with structural checks, not source-side
/// topology-match proofs.
#[derive(Debug)]
pub struct FamilyPresentation {
    pub(super) family: IntegralFamily,
    pub(super) denominator_roles: Vec<DenominatorRole>,
    pub(super) routing: MomentumRouting,
    pub(super) conventions: FamilyConventions,
    pub(super) common_mass_scale: Option<CommonMassScale>,
    pub(super) domain: PresentationDomain,
    pub(super) limits: FamilyPresentationLimits,
}

impl FamilyPresentation {
    pub const fn family(&self) -> &IntegralFamily {
        &self.family
    }

    pub fn denominator_roles(&self) -> &[DenominatorRole] {
        &self.denominator_roles
    }

    pub const fn routing(&self) -> &MomentumRouting {
        &self.routing
    }

    pub const fn conventions(&self) -> FamilyConventions {
        self.conventions
    }

    pub const fn common_mass_scale(&self) -> Option<&CommonMassScale> {
        self.common_mass_scale.as_ref()
    }

    pub const fn domain(&self) -> &PresentationDomain {
        &self.domain
    }

    pub const fn limits(&self) -> FamilyPresentationLimits {
        self.limits
    }

    pub fn into_family(self) -> IntegralFamily {
        self.family
    }
}
