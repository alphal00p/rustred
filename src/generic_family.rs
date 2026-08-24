//! Loop-count-independent integral-family algebra.
//!
//! This module is the first production slice of the LiteRed-compatible core.
//! It deliberately stops before sectors and rule discovery: its job is to
//! authenticate one complete affine denominator basis and to cache the exact
//! contractions from which parametric IBPs are generated.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::fmt::Write as _;
use std::sync::Arc;

use symbolica::prelude::*;

use crate::coefficient::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
pub use crate::guards::CoefficientLocation;
use crate::guards::GuardOrigin;

/// A polynomial over the authenticated base-field variables.
pub type BasePolynomial = MultivariatePolynomial<IntegerRing, u16>;

/// Resource policy for constructing and replaying one complete affine family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntegralFamilyLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_scalar_products: usize,
    pub max_matrix_entries: usize,
    pub max_derivative_contractions: usize,
    /// Exact byte length of the stable, typed family identity.
    pub max_fingerprint_bytes: usize,
    /// Total bytes, sparse terms, exponent entries, and GMP magnitude bits
    /// inspected while constructing the stable family identity.
    pub max_fingerprint_encoding_work: usize,
    pub max_fingerprint_polynomial_terms: usize,
    pub max_fingerprint_exponent_entries: usize,
    pub max_fingerprint_integer_bits: usize,
}

impl Default for IntegralFamilyLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_scalar_products: 4_096,
            max_matrix_entries: 16_000_000,
            max_derivative_contractions: 16_000_000,
            max_fingerprint_bytes: 1024 * 1024 * 1024,
            max_fingerprint_encoding_work: 4_000_000_000_000_000,
            max_fingerprint_polynomial_terms: 256_000_000,
            max_fingerprint_exponent_entries: 16_000_000_000,
            max_fingerprint_integer_bits: 4_000_000_000_000_000,
        }
    }
}

/// Exact census of the stable family-identity construction phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IntegralFamilyFingerprintStats {
    encoded_bytes: usize,
    encoding_work: usize,
    polynomial_terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
}

impl IntegralFamilyFingerprintStats {
    pub const fn encoded_bytes(self) -> usize {
        self.encoded_bytes
    }

    pub const fn encoding_work(self) -> usize {
        self.encoding_work
    }

    pub const fn polynomial_terms(self) -> usize {
        self.polynomial_terms
    }

    pub const fn exponent_entries(self) -> usize {
        self.exponent_entries
    }

    pub const fn integer_bits(self) -> usize {
        self.integer_bits
    }
}

/// Deterministic coordinates for scalar products involving a loop momentum.
///
/// Coordinates are ordered as all upper-triangular loop-loop products,
/// followed by loop-external products in loop-major order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScalarProductCoordinate {
    LoopLoop {
        left: usize,
        right: usize,
    },
    LoopExternal {
        loop_index: usize,
        external_index: usize,
    },
}

/// A momentum used to contract a loop derivative.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContractionMomentum {
    Loop(usize),
    External(usize),
}

/// One denominator `constant + sum_s coefficients[s] S_s`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineDenominator {
    constant: Coefficient,
    coefficients: Vec<Coefficient>,
}

impl AffineDenominator {
    pub fn new(constant: Coefficient, coefficients: Vec<Coefficient>) -> Self {
        Self {
            constant,
            coefficients,
        }
    }

    pub fn constant(&self) -> &Coefficient {
        &self.constant
    }

    pub fn coefficients(&self) -> &[Coefficient] {
        &self.coefficients
    }
}

/// An affine form in the ordered denominator basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenominatorExpansion {
    constant: Coefficient,
    denominator_coefficients: Vec<Coefficient>,
}

impl DenominatorExpansion {
    pub fn constant(&self) -> &Coefficient {
        &self.constant
    }

    pub fn denominator_coefficients(&self) -> &[Coefficient] {
        &self.denominator_coefficients
    }
}

/// A polynomial condition that defines the generic domain of a family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseNonZeroCondition {
    source: CoefficientLocation,
    polynomial: BasePolynomial,
    origins: BTreeSet<GuardOrigin>,
}

impl BaseNonZeroCondition {
    pub fn source(&self) -> &CoefficientLocation {
        &self.source
    }

    pub fn polynomial(&self) -> &BasePolynomial {
        &self.polynomial
    }

    /// Every family datum that contributed this exact polynomial condition.
    /// Origins are sorted independently of construction order.
    pub fn origins(&self) -> &BTreeSet<GuardOrigin> {
        &self.origins
    }
}

/// The exact domain on which the denominator-coordinate map is valid.
///
/// `input_denominators` are retained even if factors cancel in the determinant
/// or inverse.  The determinant numerator is a separate condition; a family
/// specialization is valid only when every listed polynomial is nonzero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyDomain {
    input_denominators: Vec<BaseNonZeroCondition>,
    basis_determinant: Coefficient,
    determinant_nonzero: BaseNonZeroCondition,
}

impl FamilyDomain {
    pub fn input_denominators(&self) -> &[BaseNonZeroCondition] {
        &self.input_denominators
    }

    pub fn basis_determinant(&self) -> &Coefficient {
        &self.basis_determinant
    }

    pub fn determinant_nonzero(&self) -> &BaseNonZeroCondition {
        &self.determinant_nonzero
    }

    pub fn conditions(&self) -> impl Iterator<Item = &BaseNonZeroCondition> {
        self.input_denominators
            .iter()
            .filter(|condition| condition.polynomial != self.determinant_nonzero.polynomial)
            .chain(std::iter::once(&self.determinant_nonzero))
    }
}

/// Typed construction and lookup failures for [`IntegralFamily`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenericFamilyError {
    NoLoopMomenta,
    ScalarProductCountOverflow {
        loops: usize,
        externals: usize,
    },
    EmptyMomentumLabel {
        role: &'static str,
        index: usize,
    },
    DuplicateMomentumLabel {
        role: &'static str,
        label: String,
    },
    MomentumLabelOverlap {
        label: String,
    },
    WrongDenominatorCount {
        expected: usize,
        actual: usize,
    },
    WrongDenominatorRowSize {
        denominator: usize,
        expected: usize,
        actual: usize,
    },
    WrongPowerShiftCount {
        expected: usize,
        actual: usize,
    },
    WrongExternalGramRowCount {
        expected: usize,
        actual: usize,
    },
    WrongExternalGramColumnCount {
        row: usize,
        expected: usize,
        actual: usize,
    },
    AsymmetricExternalGram {
        row: usize,
        column: usize,
    },
    ForeignCoefficientContext {
        location: CoefficientLocation,
    },
    InvalidCoefficient {
        location: CoefficientLocation,
        error: ExactAlgebraError,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ExactAlgebra(ExactAlgebraError),
    MatrixDimensionOverflow {
        size: usize,
    },
    SingularDenominatorBasis,
    LoopMomentumOutOfRange {
        index: usize,
        loops: usize,
    },
    ExternalMomentumOutOfRange {
        index: usize,
        externals: usize,
    },
    ScalarProductOutOfRange {
        index: usize,
        scalar_products: usize,
    },
    DenominatorOutOfRange {
        index: usize,
        denominators: usize,
    },
    InternalVerificationFailure {
        detail: String,
    },
}

impl fmt::Display for GenericFamilyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLoopMomenta => {
                formatter.write_str("an integral family needs at least one loop momentum")
            }
            Self::ScalarProductCountOverflow { loops, externals } => write!(
                formatter,
                "the scalar-product count for {loops} loops and {externals} external momenta does not fit in usize"
            ),
            Self::EmptyMomentumLabel { role, index } => {
                write!(formatter, "{role} momentum {index} has an empty label")
            }
            Self::DuplicateMomentumLabel { role, label } => {
                write!(formatter, "{role} momentum label {label:?} is repeated")
            }
            Self::MomentumLabelOverlap { label } => write!(
                formatter,
                "momentum label {label:?} is used for both a loop and an external momentum"
            ),
            Self::WrongDenominatorCount { expected, actual } => write!(
                formatter,
                "a complete affine basis needs {expected} denominators, received {actual}"
            ),
            Self::WrongDenominatorRowSize {
                denominator,
                expected,
                actual,
            } => write!(
                formatter,
                "denominator {denominator} has {actual} scalar-product coefficients, expected {expected}"
            ),
            Self::WrongPowerShiftCount { expected, actual } => write!(
                formatter,
                "received {actual} power shifts for a basis of size {expected}"
            ),
            Self::WrongExternalGramRowCount { expected, actual } => write!(
                formatter,
                "external Gram matrix has {actual} rows, expected {expected}"
            ),
            Self::WrongExternalGramColumnCount {
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "external Gram row {row} has {actual} entries, expected {expected}"
            ),
            Self::AsymmetricExternalGram { row, column } => write!(
                formatter,
                "external Gram entries ({row},{column}) and ({column},{row}) differ"
            ),
            Self::ForeignCoefficientContext { location } => write!(
                formatter,
                "coefficient at {location:?} does not use the family's exact Symbolica variable map"
            ),
            Self::InvalidCoefficient { location, error } => {
                write!(formatter, "invalid coefficient at {location:?}: {error}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded units for {resource}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::MatrixDimensionOverflow { size } => write!(
                formatter,
                "an augmented denominator matrix of size {size} cannot be represented"
            ),
            Self::SingularDenominatorBasis => formatter
                .write_str("the affine denominator coefficient matrix is identically singular"),
            Self::LoopMomentumOutOfRange { index, loops } => write!(
                formatter,
                "loop momentum {index} is outside a family with {loops} loops"
            ),
            Self::ExternalMomentumOutOfRange { index, externals } => write!(
                formatter,
                "external momentum {index} is outside a family with {externals} external momenta"
            ),
            Self::ScalarProductOutOfRange {
                index,
                scalar_products,
            } => write!(
                formatter,
                "scalar-product coordinate {index} is outside a basis of size {scalar_products}"
            ),
            Self::DenominatorOutOfRange {
                index,
                denominators,
            } => write!(
                formatter,
                "denominator {index} is outside a basis of size {denominators}"
            ),
            Self::InternalVerificationFailure { detail } => {
                write!(formatter, "exact family replay failed: {detail}")
            }
        }
    }
}

impl std::error::Error for GenericFamilyError {}

impl From<ExactAlgebraError> for GenericFamilyError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

/// A complete, loop-count-independent affine integral family.
#[derive(Clone, Debug)]
pub struct IntegralFamily {
    name: String,
    // `Arc<String>` moves the already fallibly allocated user-sized buffer;
    // cloning a family shares it. Only the fixed-size Arc header allocation is
    // infallible, unlike `String -> Arc<str>`, which may copy proportionally.
    fingerprint: Arc<String>,
    fingerprint_stats: IntegralFamilyFingerprintStats,
    loop_momenta: Vec<String>,
    external_momenta: Vec<String>,
    coefficients: CoefficientContext,
    dimension: Coefficient,
    coordinates: Vec<ScalarProductCoordinate>,
    contractions: Vec<ContractionMomentum>,
    denominators: Vec<AffineDenominator>,
    external_gram: Vec<Vec<Coefficient>>,
    power_shifts: Vec<Coefficient>,
    limits: IntegralFamilyLimits,
    inverse_basis: Vec<Vec<Coefficient>>,
    domain: FamilyDomain,
    // denominator -> differentiated loop -> contraction momentum
    derivative_contractions: Vec<Vec<Vec<DenominatorExpansion>>>,
}

/// Compatibility name for callers that prefer to make genericity explicit.
pub type GenericFamily = IntegralFamily;

impl IntegralFamily {
    /// Construct and exactly authenticate a complete affine denominator basis.
    #[allow(clippy::too_many_arguments)]
    pub fn new<'name>(
        name: impl Into<Cow<'name, str>>,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        coefficients: CoefficientContext,
        dimension: Coefficient,
        denominators: Vec<AffineDenominator>,
        external_gram: Vec<Vec<Coefficient>>,
        power_shifts: Vec<Coefficient>,
    ) -> Result<Self, GenericFamilyError> {
        Self::new_with_limits(
            name,
            loop_momenta,
            external_momenta,
            coefficients,
            dimension,
            denominators,
            external_gram,
            power_shifts,
            IntegralFamilyLimits::default(),
        )
    }

    /// Construct a family under explicit exact-algebra and allocation limits.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_limits<'name>(
        name: impl Into<Cow<'name, str>>,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        coefficients: CoefficientContext,
        dimension: Coefficient,
        denominators: Vec<AffineDenominator>,
        external_gram: Vec<Vec<Coefficient>>,
        power_shifts: Vec<Coefficient>,
        limits: IntegralFamilyLimits,
    ) -> Result<Self, GenericFamilyError> {
        let name = name.into();
        check_family_limit(
            "family fingerprint bytes",
            name.len(),
            limits.max_fingerprint_bytes,
        )?;
        let loops = loop_momenta.len();
        let externals = external_momenta.len();
        let scalar_products = checked_scalar_product_count(loops, externals)?;
        check_family_limit(
            "family scalar products",
            scalar_products,
            limits.max_scalar_products,
        )?;
        let augmented_columns =
            scalar_products
                .checked_mul(2)
                .ok_or(GenericFamilyError::ResourceCountOverflow {
                    resource: "augmented family matrix entries",
                })?;
        let matrix_entries = scalar_products.checked_mul(augmented_columns).ok_or(
            GenericFamilyError::ResourceCountOverflow {
                resource: "augmented family matrix entries",
            },
        )?;
        check_family_limit(
            "augmented family matrix entries",
            matrix_entries,
            limits.max_matrix_entries,
        )?;
        let derivative_entries = scalar_products
            .checked_mul(loops)
            .and_then(|count| count.checked_mul(loops.checked_add(externals)?))
            .ok_or(GenericFamilyError::ResourceCountOverflow {
                resource: "family derivative contractions",
            })?;
        check_family_limit(
            "family derivative contractions",
            derivative_entries,
            limits.max_derivative_contractions,
        )?;
        if loops == 0 {
            return Err(GenericFamilyError::NoLoopMomenta);
        }
        preflight_family_identity_strings(
            name.as_ref(),
            &loop_momenta,
            &external_momenta,
            coefficients.parameter_names(),
            limits.max_fingerprint_bytes,
        )?;
        let name = retain_family_name(name)?;
        validate_momentum_labels(&loop_momenta, &external_momenta)?;
        let coordinates = build_coordinates(loops, externals, scalar_products);
        let contractions = (0..loops)
            .map(ContractionMomentum::Loop)
            .chain((0..externals).map(ContractionMomentum::External))
            .collect::<Vec<_>>();

        if denominators.len() != scalar_products {
            return Err(GenericFamilyError::WrongDenominatorCount {
                expected: scalar_products,
                actual: denominators.len(),
            });
        }
        if power_shifts.len() != scalar_products {
            return Err(GenericFamilyError::WrongPowerShiftCount {
                expected: scalar_products,
                actual: power_shifts.len(),
            });
        }
        if external_gram.len() != externals {
            return Err(GenericFamilyError::WrongExternalGramRowCount {
                expected: externals,
                actual: external_gram.len(),
            });
        }
        for (row, values) in external_gram.iter().enumerate() {
            if values.len() != externals {
                return Err(GenericFamilyError::WrongExternalGramColumnCount {
                    row,
                    expected: externals,
                    actual: values.len(),
                });
            }
        }
        for (denominator, affine) in denominators.iter().enumerate() {
            if affine.coefficients.len() != scalar_products {
                return Err(GenericFamilyError::WrongDenominatorRowSize {
                    denominator,
                    expected: scalar_products,
                    actual: affine.coefficients.len(),
                });
            }
        }

        let mut input_denominators = Vec::new();
        validate_and_retain_input_denominator(
            &coefficients,
            &dimension,
            CoefficientLocation::Dimension,
            limits.exact_algebra,
            &mut input_denominators,
        )?;
        for (denominator, affine) in denominators.iter().enumerate() {
            validate_and_retain_input_denominator(
                &coefficients,
                &affine.constant,
                CoefficientLocation::DenominatorConstant { denominator },
                limits.exact_algebra,
                &mut input_denominators,
            )?;
            for (coordinate, coefficient) in affine.coefficients.iter().enumerate() {
                validate_and_retain_input_denominator(
                    &coefficients,
                    coefficient,
                    CoefficientLocation::DenominatorCoefficient {
                        denominator,
                        coordinate,
                    },
                    limits.exact_algebra,
                    &mut input_denominators,
                )?;
            }
        }
        for (row, values) in external_gram.iter().enumerate() {
            for (column, coefficient) in values.iter().enumerate() {
                validate_and_retain_input_denominator(
                    &coefficients,
                    coefficient,
                    CoefficientLocation::ExternalGram { row, column },
                    limits.exact_algebra,
                    &mut input_denominators,
                )?;
            }
        }
        for (denominator, power_shift) in power_shifts.iter().enumerate() {
            validate_and_retain_input_denominator(
                &coefficients,
                power_shift,
                CoefficientLocation::PowerShift { denominator },
                limits.exact_algebra,
                &mut input_denominators,
            )?;
        }
        for row in 0..externals {
            for column in row + 1..externals {
                if !coefficients_are_equal(
                    &coefficients,
                    &external_gram[row][column],
                    &external_gram[column][row],
                    limits.exact_algebra,
                )? {
                    return Err(GenericFamilyError::AsymmetricExternalGram { row, column });
                }
            }
        }

        // Every coefficient is now authenticated on the exact ordered base
        // map. Census the complete typed identity before any GMP formatting or
        // user-sized fingerprint allocation is attempted.
        let (fingerprint, fingerprint_stats) = build_family_fingerprint(
            &name,
            &loop_momenta,
            &external_momenta,
            &coefficients,
            &dimension,
            &denominators,
            &external_gram,
            &power_shifts,
            limits,
        )?;

        let basis = denominators
            .iter()
            .map(|denominator| denominator.coefficients.clone())
            .collect::<Vec<_>>();
        let (inverse_basis, basis_determinant) =
            invert_symbolic_matrix(&coefficients, &basis, limits.exact_algebra)?;
        verify_inverse(&coefficients, &basis, &inverse_basis, limits.exact_algebra)?;
        let determinant_nonzero = make_family_nonzero_condition(
            CoefficientLocation::BasisDeterminantNumerator,
            basis_determinant.numerator.clone(),
            GuardOrigin::FamilyBasisDeterminantNumerator,
        );
        // If the determinant numerator is already an input-denominator
        // condition, retain one polynomial with the union of both reasons.
        // The dedicated determinant getter remains available as a compatible
        // view of that same merged condition.
        let determinant_nonzero = if let Some(existing) = input_denominators
            .iter_mut()
            .find(|condition| condition.polynomial == determinant_nonzero.polynomial)
        {
            merge_family_condition(existing, &determinant_nonzero);
            existing.clone()
        } else {
            determinant_nonzero
        };
        let domain = FamilyDomain {
            input_denominators,
            basis_determinant,
            determinant_nonzero,
        };

        let mut family = Self {
            name,
            fingerprint: Arc::new(fingerprint),
            fingerprint_stats,
            loop_momenta,
            external_momenta,
            coefficients,
            dimension,
            coordinates,
            contractions,
            denominators,
            external_gram,
            power_shifts,
            limits,
            inverse_basis,
            domain,
            derivative_contractions: Vec::new(),
        };
        family.derivative_contractions = family.build_derivative_contractions()?;
        family.verify_exact_replay()?;
        Ok(family)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Stable typed semantic identity used to scope parametric indices and
    /// cached relations. Symbolica's process-local symbol ids and expression
    /// printers are deliberately absent: coefficients are serialized from
    /// their authenticated sparse integer-polynomial payload.
    pub fn fingerprint(&self) -> String {
        self.fingerprint.as_str().to_owned()
    }

    /// Borrow the semantic identity cached once during authenticated family
    /// construction. Proof-bearing replay paths should prefer this view when
    /// they only need comparison or a separately fallible retained copy.
    pub fn fingerprint_ref(&self) -> &str {
        self.fingerprint.as_str()
    }

    pub const fn fingerprint_stats(&self) -> IntegralFamilyFingerprintStats {
        self.fingerprint_stats
    }

    pub fn loop_count(&self) -> usize {
        self.loop_momenta.len()
    }

    pub fn external_count(&self) -> usize {
        self.external_momenta.len()
    }

    pub fn denominator_count(&self) -> usize {
        self.denominators.len()
    }

    pub fn loop_momenta(&self) -> &[String] {
        &self.loop_momenta
    }

    pub fn external_momenta(&self) -> &[String] {
        &self.external_momenta
    }

    pub fn coefficient_context(&self) -> &CoefficientContext {
        &self.coefficients
    }

    pub fn dimension(&self) -> &Coefficient {
        &self.dimension
    }

    pub fn coordinates(&self) -> &[ScalarProductCoordinate] {
        &self.coordinates
    }

    pub fn contraction_momenta(&self) -> &[ContractionMomentum] {
        &self.contractions
    }

    pub fn denominators(&self) -> &[AffineDenominator] {
        &self.denominators
    }

    pub fn external_gram(&self) -> &[Vec<Coefficient>] {
        &self.external_gram
    }

    pub fn power_shifts(&self) -> &[Coefficient] {
        &self.power_shifts
    }

    pub fn limits(&self) -> IntegralFamilyLimits {
        self.limits
    }

    /// Matrix `A^-1` in `S = A^-1 (D-c)` orientation.
    pub fn inverse_basis(&self) -> &[Vec<Coefficient>] {
        &self.inverse_basis
    }

    pub fn domain(&self) -> &FamilyDomain {
        &self.domain
    }

    /// Return the deterministic position of a typed scalar product.
    pub fn coordinate_index(
        &self,
        coordinate: ScalarProductCoordinate,
    ) -> Result<usize, GenericFamilyError> {
        match coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                for index in [left, right] {
                    if index >= self.loop_count() {
                        return Err(GenericFamilyError::LoopMomentumOutOfRange {
                            index,
                            loops: self.loop_count(),
                        });
                    }
                }
                Ok(self.loop_loop_coordinate_index(left, right))
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                if loop_index >= self.loop_count() {
                    return Err(GenericFamilyError::LoopMomentumOutOfRange {
                        index: loop_index,
                        loops: self.loop_count(),
                    });
                }
                if external_index >= self.external_count() {
                    return Err(GenericFamilyError::ExternalMomentumOutOfRange {
                        index: external_index,
                        externals: self.external_count(),
                    });
                }
                Ok(self.loop_external_coordinate_index(loop_index, external_index))
            }
        }
    }

    /// Express one scalar-product coordinate in the denominator basis.
    pub fn scalar_product_expansion(
        &self,
        coordinate: usize,
    ) -> Result<DenominatorExpansion, GenericFamilyError> {
        let Some(denominator_coefficients) = self.inverse_basis.get(coordinate).cloned() else {
            return Err(GenericFamilyError::ScalarProductOutOfRange {
                index: coordinate,
                scalar_products: self.coordinates.len(),
            });
        };
        let mut constant = self.coefficients.zero();
        for (coefficient, denominator) in denominator_coefficients.iter().zip(&self.denominators) {
            let contribution = self.coefficients.try_mul(
                coefficient,
                &denominator.constant,
                self.limits.exact_algebra,
            )?;
            constant =
                self.coefficients
                    .try_sub(&constant, &contribution, self.limits.exact_algebra)?;
        }
        Ok(DenominatorExpansion {
            constant,
            denominator_coefficients,
        })
    }

    /// Return the cached affine image of `q . d D_r / d k_i`.
    pub fn derivative_contraction(
        &self,
        denominator: usize,
        differentiated_loop: usize,
        contraction: ContractionMomentum,
    ) -> Result<&DenominatorExpansion, GenericFamilyError> {
        if denominator >= self.denominator_count() {
            return Err(GenericFamilyError::DenominatorOutOfRange {
                index: denominator,
                denominators: self.denominator_count(),
            });
        }
        if differentiated_loop >= self.loop_count() {
            return Err(GenericFamilyError::LoopMomentumOutOfRange {
                index: differentiated_loop,
                loops: self.loop_count(),
            });
        }
        let contraction_index = match contraction {
            ContractionMomentum::Loop(index) => {
                if index >= self.loop_count() {
                    return Err(GenericFamilyError::LoopMomentumOutOfRange {
                        index,
                        loops: self.loop_count(),
                    });
                }
                index
            }
            ContractionMomentum::External(index) => {
                if index >= self.external_count() {
                    return Err(GenericFamilyError::ExternalMomentumOutOfRange {
                        index,
                        externals: self.external_count(),
                    });
                }
                self.loop_count() + index
            }
        };
        Ok(&self.derivative_contractions[denominator][differentiated_loop][contraction_index])
    }

    /// Recheck the inverse, every scalar-product round trip, and every cached
    /// derivative contraction in the free scalar-product module over `K`.
    pub fn verify_exact_replay(&self) -> Result<(), GenericFamilyError> {
        let basis = self
            .denominators
            .iter()
            .map(|denominator| denominator.coefficients.clone())
            .collect::<Vec<_>>();
        verify_inverse(
            &self.coefficients,
            &basis,
            &self.inverse_basis,
            self.limits.exact_algebra,
        )?;

        for coordinate in 0..self.coordinates.len() {
            let expansion = self.scalar_product_expansion(coordinate)?;
            let (constant, scalar_coefficients) = self.replay_denominator_expansion(&expansion)?;
            if !constant.is_zero() {
                return Err(GenericFamilyError::InternalVerificationFailure {
                    detail: format!(
                        "scalar-product coordinate {coordinate} has nonzero replay constant"
                    ),
                });
            }
            for (candidate, coefficient) in scalar_coefficients.iter().enumerate() {
                let expected = if candidate == coordinate {
                    self.coefficients.one()
                } else {
                    self.coefficients.zero()
                };
                if !coefficients_are_equal(
                    &self.coefficients,
                    coefficient,
                    &expected,
                    self.limits.exact_algebra,
                )? {
                    return Err(GenericFamilyError::InternalVerificationFailure {
                        detail: format!(
                            "scalar-product coordinate {coordinate} replays incorrectly at coordinate {candidate}"
                        ),
                    });
                }
            }
        }

        for denominator in 0..self.denominator_count() {
            for differentiated_loop in 0..self.loop_count() {
                for (contraction_index, &contraction) in self.contractions.iter().enumerate() {
                    let direct =
                        self.direct_derivative(denominator, differentiated_loop, contraction)?;
                    let cached = &self.derivative_contractions[denominator][differentiated_loop]
                        [contraction_index];
                    let replayed = self.replay_denominator_expansion(cached)?;
                    if !affine_forms_are_equal(
                        &self.coefficients,
                        &direct,
                        &replayed,
                        self.limits.exact_algebra,
                    )? {
                        return Err(GenericFamilyError::InternalVerificationFailure {
                            detail: format!(
                                "derivative contraction D_{denominator}, k_{differentiated_loop}, {contraction:?} does not replay"
                            ),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn build_derivative_contractions(
        &self,
    ) -> Result<Vec<Vec<Vec<DenominatorExpansion>>>, GenericFamilyError> {
        (0..self.denominator_count())
            .map(|denominator| {
                (0..self.loop_count())
                    .map(|differentiated_loop| {
                        self.contractions
                            .iter()
                            .map(|&contraction| {
                                let (constant, scalar_coefficients) = self.direct_derivative(
                                    denominator,
                                    differentiated_loop,
                                    contraction,
                                )?;
                                self.rewrite_scalar_affine(constant, &scalar_coefficients)
                            })
                            .collect::<Result<Vec<_>, GenericFamilyError>>()
                    })
                    .collect::<Result<Vec<_>, GenericFamilyError>>()
            })
            .collect::<Result<Vec<_>, GenericFamilyError>>()
    }

    fn direct_derivative(
        &self,
        denominator: usize,
        differentiated_loop: usize,
        contraction: ContractionMomentum,
    ) -> Result<(Coefficient, Vec<Coefficient>), GenericFamilyError> {
        let mut constant = self.coefficients.zero();
        let mut scalar_coefficients = vec![self.coefficients.zero(); self.coordinates.len()];
        for (coordinate_index, coordinate) in self.coordinates.iter().copied().enumerate() {
            let coefficient = &self.denominators[denominator].coefficients[coordinate_index];
            if coefficient.is_zero() {
                continue;
            }
            match coordinate {
                ScalarProductCoordinate::LoopLoop { left, right } => {
                    if differentiated_loop == left {
                        self.add_dot_with_loop(
                            &mut scalar_coefficients,
                            contraction,
                            right,
                            coefficient,
                        )?;
                    }
                    if differentiated_loop == right {
                        self.add_dot_with_loop(
                            &mut scalar_coefficients,
                            contraction,
                            left,
                            coefficient,
                        )?;
                    }
                }
                ScalarProductCoordinate::LoopExternal {
                    loop_index,
                    external_index,
                } => {
                    if differentiated_loop == loop_index {
                        self.add_dot_with_external(
                            &mut constant,
                            &mut scalar_coefficients,
                            contraction,
                            external_index,
                            coefficient,
                        )?;
                    }
                }
            }
        }
        Ok((constant, scalar_coefficients))
    }

    fn add_dot_with_loop(
        &self,
        scalar_coefficients: &mut [Coefficient],
        contraction: ContractionMomentum,
        loop_index: usize,
        coefficient: &Coefficient,
    ) -> Result<(), GenericFamilyError> {
        let coordinate = match contraction {
            ContractionMomentum::Loop(other) => self.loop_loop_coordinate_index(loop_index, other),
            ContractionMomentum::External(external_index) => {
                self.loop_external_coordinate_index(loop_index, external_index)
            }
        };
        scalar_coefficients[coordinate] = self.coefficients.try_add(
            &scalar_coefficients[coordinate],
            coefficient,
            self.limits.exact_algebra,
        )?;
        Ok(())
    }

    fn add_dot_with_external(
        &self,
        constant: &mut Coefficient,
        scalar_coefficients: &mut [Coefficient],
        contraction: ContractionMomentum,
        external_index: usize,
        coefficient: &Coefficient,
    ) -> Result<(), GenericFamilyError> {
        match contraction {
            ContractionMomentum::Loop(loop_index) => {
                let coordinate = self.loop_external_coordinate_index(loop_index, external_index);
                scalar_coefficients[coordinate] = self.coefficients.try_add(
                    &scalar_coefficients[coordinate],
                    coefficient,
                    self.limits.exact_algebra,
                )?;
            }
            ContractionMomentum::External(other) => {
                let contribution = self.coefficients.try_mul(
                    coefficient,
                    &self.external_gram[other][external_index],
                    self.limits.exact_algebra,
                )?;
                *constant = self.coefficients.try_add(
                    constant,
                    &contribution,
                    self.limits.exact_algebra,
                )?;
            }
        }
        Ok(())
    }

    fn rewrite_scalar_affine(
        &self,
        direct_constant: Coefficient,
        scalar_coefficients: &[Coefficient],
    ) -> Result<DenominatorExpansion, GenericFamilyError> {
        let mut denominator_coefficients = vec![self.coefficients.zero(); self.denominator_count()];
        for (scalar_product, scalar_coefficient) in scalar_coefficients.iter().enumerate() {
            if scalar_coefficient.is_zero() {
                continue;
            }
            for (target, inverse_coefficient) in
                self.inverse_basis[scalar_product].iter().enumerate()
            {
                let contribution = self.coefficients.try_mul(
                    scalar_coefficient,
                    inverse_coefficient,
                    self.limits.exact_algebra,
                )?;
                denominator_coefficients[target] = self.coefficients.try_add(
                    &denominator_coefficients[target],
                    &contribution,
                    self.limits.exact_algebra,
                )?;
            }
        }
        let mut constant = direct_constant;
        for (coefficient, denominator) in denominator_coefficients.iter().zip(&self.denominators) {
            let contribution = self.coefficients.try_mul(
                coefficient,
                &denominator.constant,
                self.limits.exact_algebra,
            )?;
            constant =
                self.coefficients
                    .try_sub(&constant, &contribution, self.limits.exact_algebra)?;
        }
        Ok(DenominatorExpansion {
            constant,
            denominator_coefficients,
        })
    }

    fn replay_denominator_expansion(
        &self,
        expansion: &DenominatorExpansion,
    ) -> Result<(Coefficient, Vec<Coefficient>), GenericFamilyError> {
        let mut constant = expansion.constant.clone();
        let mut scalar_coefficients = vec![self.coefficients.zero(); self.coordinates.len()];
        for (denominator_coefficient, denominator) in expansion
            .denominator_coefficients
            .iter()
            .zip(&self.denominators)
        {
            let contribution = self.coefficients.try_mul(
                denominator_coefficient,
                &denominator.constant,
                self.limits.exact_algebra,
            )?;
            constant =
                self.coefficients
                    .try_add(&constant, &contribution, self.limits.exact_algebra)?;
            for (coordinate, basis_coefficient) in denominator.coefficients.iter().enumerate() {
                let contribution = self.coefficients.try_mul(
                    denominator_coefficient,
                    basis_coefficient,
                    self.limits.exact_algebra,
                )?;
                scalar_coefficients[coordinate] = self.coefficients.try_add(
                    &scalar_coefficients[coordinate],
                    &contribution,
                    self.limits.exact_algebra,
                )?;
            }
        }
        Ok((constant, scalar_coefficients))
    }

    fn loop_loop_coordinate_index(&self, left: usize, right: usize) -> usize {
        let (left, right) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        let total = triangular(self.loop_count())
            .expect("the family constructor proved the loop-loop count representable");
        let remaining = triangular(self.loop_count() - left)
            .expect("a smaller triangular count is representable");
        total - remaining + (right - left)
    }

    fn loop_external_coordinate_index(&self, loop_index: usize, external_index: usize) -> usize {
        let loop_loop = triangular(self.loop_count())
            .expect("the family constructor proved the loop-loop count representable");
        loop_loop + loop_index * self.external_count() + external_index
    }
}

fn preflight_family_identity_strings(
    name: &str,
    loop_momenta: &[String],
    external_momenta: &[String],
    parameters: &[String],
    limit: usize,
) -> Result<(), GenericFamilyError> {
    check_family_limit("family fingerprint bytes", name.len(), limit)?;
    for value in loop_momenta
        .iter()
        .chain(external_momenta)
        .chain(parameters)
    {
        check_family_limit("family fingerprint string bytes", value.len(), limit)?;
    }
    Ok(())
}

fn retain_family_name(name: Cow<'_, str>) -> Result<String, GenericFamilyError> {
    match name {
        Cow::Owned(name) => Ok(name),
        Cow::Borrowed(name) => try_copy_family_string(name, "family name"),
    }
}

fn try_copy_family_string(
    source: &str,
    resource: &'static str,
) -> Result<String, GenericFamilyError> {
    let mut target = String::new();
    target
        .try_reserve_exact(source.len())
        .map_err(|_| GenericFamilyError::AllocationFailure {
            resource,
            requested: source.len(),
        })?;
    target.push_str(source);
    Ok(target)
}

fn validate_momentum_labels(
    loop_momenta: &[String],
    external_momenta: &[String],
) -> Result<(), GenericFamilyError> {
    if loop_momenta.is_empty() {
        return Err(GenericFamilyError::NoLoopMomenta);
    }
    let mut loops = HashSet::new();
    loops
        .try_reserve(loop_momenta.len())
        .map_err(|_| GenericFamilyError::AllocationFailure {
            resource: "loop momentum label set",
            requested: loop_momenta.len(),
        })?;
    for (index, label) in loop_momenta.iter().enumerate() {
        if label.trim().is_empty() {
            return Err(GenericFamilyError::EmptyMomentumLabel {
                role: "loop",
                index,
            });
        }
        if !loops.insert(label.as_str()) {
            return Err(GenericFamilyError::DuplicateMomentumLabel {
                role: "loop",
                label: try_copy_family_string(label, "duplicate loop momentum label")?,
            });
        }
    }
    let mut externals = HashSet::new();
    externals.try_reserve(external_momenta.len()).map_err(|_| {
        GenericFamilyError::AllocationFailure {
            resource: "external momentum label set",
            requested: external_momenta.len(),
        }
    })?;
    for (index, label) in external_momenta.iter().enumerate() {
        if label.trim().is_empty() {
            return Err(GenericFamilyError::EmptyMomentumLabel {
                role: "external",
                index,
            });
        }
        if loops.contains(label.as_str()) {
            return Err(GenericFamilyError::MomentumLabelOverlap {
                label: try_copy_family_string(label, "overlapping momentum label")?,
            });
        }
        if !externals.insert(label.as_str()) {
            return Err(GenericFamilyError::DuplicateMomentumLabel {
                role: "external",
                label: try_copy_family_string(label, "duplicate external momentum label")?,
            });
        }
    }
    Ok(())
}

const INTEGRAL_FAMILY_FINGERPRINT_V2_SCHEMA: &str = "rustred-integral-family-v2;";

#[allow(clippy::too_many_arguments)]
fn build_family_fingerprint(
    name: &str,
    loop_momenta: &[String],
    external_momenta: &[String],
    coefficients: &CoefficientContext,
    dimension: &Coefficient,
    denominators: &[AffineDenominator],
    external_gram: &[Vec<Coefficient>],
    power_shifts: &[Coefficient],
    limits: IntegralFamilyLimits,
) -> Result<(String, IntegralFamilyFingerprintStats), GenericFamilyError> {
    let mut census = FamilyFingerprintCensus::new(limits);
    encode_family_fingerprint(
        &mut census,
        name,
        loop_momenta,
        external_momenta,
        coefficients,
        dimension,
        denominators,
        external_gram,
        power_shifts,
    )?;
    let stats = census.finish();

    let mut writer = FamilyFingerprintWriter::try_new(stats.encoded_bytes)?;
    encode_family_fingerprint(
        &mut writer,
        name,
        loop_momenta,
        external_momenta,
        coefficients,
        dimension,
        denominators,
        external_gram,
        power_shifts,
    )?;
    let fingerprint = writer.finish()?;
    Ok((fingerprint, stats))
}

/// Typed V2 grammar. All variable-size strings are byte-length delimited and
/// all collection shapes precede their payload. A rational coefficient is the
/// ordered pair of its authenticated numerator and denominator sparse
/// polynomials; every Integer is an explicit sign plus uppercase hexadecimal
/// magnitude, independent of Symbolica expression printers and symbol ids.
#[allow(clippy::too_many_arguments)]
fn encode_family_fingerprint(
    sink: &mut impl FamilyFingerprintSink,
    name: &str,
    loop_momenta: &[String],
    external_momenta: &[String],
    coefficients: &CoefficientContext,
    dimension: &Coefficient,
    denominators: &[AffineDenominator],
    external_gram: &[Vec<Coefficient>],
    power_shifts: &[Coefficient],
) -> Result<(), GenericFamilyError> {
    sink.literal(INTEGRAL_FAMILY_FINGERPRINT_V2_SCHEMA)?;
    sink.literal("N")?;
    encode_fingerprint_string(sink, name)?;
    encode_fingerprint_string_list(sink, "L", loop_momenta)?;
    encode_fingerprint_string_list(sink, "E", external_momenta)?;
    encode_fingerprint_string_list(sink, "P", coefficients.parameter_names())?;

    sink.literal("Q")?;
    sink.usize_value(loop_momenta.len())?;
    sink.literal(",")?;
    sink.usize_value(external_momenta.len())?;
    sink.literal(",")?;
    sink.usize_value(denominators.len())?;
    sink.literal(";")?;

    sink.literal("M;")?;
    encode_fingerprint_coefficient(sink, dimension)?;
    sink.literal("D")?;
    sink.usize_value(denominators.len())?;
    sink.literal(";")?;
    for denominator in denominators {
        encode_fingerprint_coefficient(sink, denominator.constant())?;
        for coefficient in denominator.coefficients() {
            encode_fingerprint_coefficient(sink, coefficient)?;
        }
    }

    let gram_entries = external_gram
        .iter()
        .try_fold(0usize, |count, row| count.checked_add(row.len()))
        .ok_or(GenericFamilyError::ResourceCountOverflow {
            resource: "family fingerprint external Gram entries",
        })?;
    sink.literal("G")?;
    sink.usize_value(gram_entries)?;
    sink.literal(";")?;
    for coefficient in external_gram.iter().flatten() {
        encode_fingerprint_coefficient(sink, coefficient)?;
    }

    sink.literal("U")?;
    sink.usize_value(power_shifts.len())?;
    sink.literal(";")?;
    for shift in power_shifts {
        encode_fingerprint_coefficient(sink, shift)?;
    }
    Ok(())
}

fn encode_fingerprint_string_list(
    sink: &mut impl FamilyFingerprintSink,
    marker: &'static str,
    values: &[String],
) -> Result<(), GenericFamilyError> {
    sink.literal(marker)?;
    sink.usize_value(values.len())?;
    sink.literal(";")?;
    for value in values {
        encode_fingerprint_string(sink, value)?;
    }
    Ok(())
}

fn encode_fingerprint_string(
    sink: &mut impl FamilyFingerprintSink,
    value: &str,
) -> Result<(), GenericFamilyError> {
    sink.usize_value(value.len())?;
    sink.literal(":")?;
    sink.literal(value)?;
    sink.literal(";")
}

fn encode_fingerprint_coefficient(
    sink: &mut impl FamilyFingerprintSink,
    coefficient: &Coefficient,
) -> Result<(), GenericFamilyError> {
    sink.literal("R")?;
    encode_fingerprint_polynomial(sink, &coefficient.numerator)?;
    encode_fingerprint_polynomial(sink, &coefficient.denominator)
}

fn encode_fingerprint_polynomial(
    sink: &mut impl FamilyFingerprintSink,
    polynomial: &BasePolynomial,
) -> Result<(), GenericFamilyError> {
    let variables = polynomial.variables.len();
    sink.literal("Y")?;
    sink.usize_value(variables)?;
    sink.literal(",")?;
    sink.usize_value(polynomial.coefficients.len())?;
    sink.literal(";")?;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        sink.polynomial_term()?;
        sink.integer_value(coefficient)?;
        sink.literal("X")?;
        let start =
            term.checked_mul(variables)
                .ok_or(GenericFamilyError::ResourceCountOverflow {
                    resource: "family fingerprint polynomial exponent offset",
                })?;
        let end =
            start
                .checked_add(variables)
                .ok_or(GenericFamilyError::ResourceCountOverflow {
                    resource: "family fingerprint polynomial exponent offset",
                })?;
        let exponents = polynomial.exponents.get(start..end).ok_or_else(|| {
            GenericFamilyError::InternalVerificationFailure {
                detail: "authenticated fingerprint polynomial has a malformed exponent layout"
                    .to_owned(),
            }
        })?;
        for (position, &exponent) in exponents.iter().enumerate() {
            if position != 0 {
                sink.literal(",")?;
            }
            sink.exponent_value(exponent)?;
        }
        sink.literal(";")?;
    }
    Ok(())
}

trait FamilyFingerprintSink {
    fn literal(&mut self, value: &str) -> Result<(), GenericFamilyError>;
    fn usize_value(&mut self, value: usize) -> Result<(), GenericFamilyError>;
    fn polynomial_term(&mut self) -> Result<(), GenericFamilyError>;
    fn exponent_value(&mut self, value: u16) -> Result<(), GenericFamilyError>;
    fn integer_value(&mut self, value: &Integer) -> Result<(), GenericFamilyError>;
}

struct FamilyFingerprintCensus {
    limits: IntegralFamilyLimits,
    stats: IntegralFamilyFingerprintStats,
}

impl FamilyFingerprintCensus {
    const fn new(limits: IntegralFamilyLimits) -> Self {
        Self {
            limits,
            stats: IntegralFamilyFingerprintStats {
                encoded_bytes: 0,
                encoding_work: 0,
                polynomial_terms: 0,
                exponent_entries: 0,
                integer_bits: 0,
            },
        }
    }

    const fn finish(self) -> IntegralFamilyFingerprintStats {
        self.stats
    }

    fn add_bytes(&mut self, additional: usize) -> Result<(), GenericFamilyError> {
        self.stats.encoded_bytes = checked_bounded_fingerprint_add(
            "family fingerprint bytes",
            self.stats.encoded_bytes,
            additional,
            self.limits.max_fingerprint_bytes,
        )?;
        self.add_work(additional)
    }

    fn add_work(&mut self, additional: usize) -> Result<(), GenericFamilyError> {
        self.stats.encoding_work = checked_bounded_fingerprint_add(
            "family fingerprint encoding work",
            self.stats.encoding_work,
            additional,
            self.limits.max_fingerprint_encoding_work,
        )?;
        Ok(())
    }
}

impl FamilyFingerprintSink for FamilyFingerprintCensus {
    fn literal(&mut self, value: &str) -> Result<(), GenericFamilyError> {
        self.add_bytes(value.len())
    }

    fn usize_value(&mut self, value: usize) -> Result<(), GenericFamilyError> {
        self.add_bytes(decimal_digits_usize(value))
    }

    fn polynomial_term(&mut self) -> Result<(), GenericFamilyError> {
        self.stats.polynomial_terms = checked_bounded_fingerprint_add(
            "family fingerprint polynomial terms",
            self.stats.polynomial_terms,
            1,
            self.limits.max_fingerprint_polynomial_terms,
        )?;
        self.add_work(1)
    }

    fn exponent_value(&mut self, value: u16) -> Result<(), GenericFamilyError> {
        self.stats.exponent_entries = checked_bounded_fingerprint_add(
            "family fingerprint exponent entries",
            self.stats.exponent_entries,
            1,
            self.limits.max_fingerprint_exponent_entries,
        )?;
        self.add_work(1)?;
        self.add_bytes(decimal_digits_usize(usize::from(value)))
    }

    fn integer_value(&mut self, value: &Integer) -> Result<(), GenericFamilyError> {
        let bits = family_fingerprint_integer_bits(value)?;
        self.stats.integer_bits = checked_bounded_fingerprint_add(
            "family fingerprint integer bits",
            self.stats.integer_bits,
            bits,
            self.limits.max_fingerprint_integer_bits,
        )?;
        self.add_work(bits)?;
        let hexadecimal_digits = if bits == 0 {
            1
        } else {
            bits.checked_add(3)
                .ok_or(GenericFamilyError::ResourceCountOverflow {
                    resource: "family fingerprint hexadecimal digits",
                })?
                / 4
        };
        // `I`, explicit sign, magnitude, and `;`.
        self.add_bytes(hexadecimal_digits.checked_add(3).ok_or(
            GenericFamilyError::ResourceCountOverflow {
                resource: "family fingerprint integer bytes",
            },
        )?)
    }
}

struct FamilyFingerprintWriter {
    output: String,
    expected_bytes: usize,
}

impl FamilyFingerprintWriter {
    fn try_new(expected_bytes: usize) -> Result<Self, GenericFamilyError> {
        let mut output = String::new();
        output.try_reserve_exact(expected_bytes).map_err(|_| {
            GenericFamilyError::AllocationFailure {
                resource: "family fingerprint",
                requested: expected_bytes,
            }
        })?;
        Ok(Self {
            output,
            expected_bytes,
        })
    }

    fn finish(self) -> Result<String, GenericFamilyError> {
        if self.output.len() != self.expected_bytes {
            return Err(GenericFamilyError::InternalVerificationFailure {
                detail: "family fingerprint census differs from encoded byte length".to_owned(),
            });
        }
        Ok(self.output)
    }

    fn formatting_failure() -> GenericFamilyError {
        GenericFamilyError::InternalVerificationFailure {
            detail: "family fingerprint exceeded its authenticated byte census".to_owned(),
        }
    }
}

impl fmt::Write for FamilyFingerprintWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let requested = self
            .output
            .len()
            .checked_add(value.len())
            .ok_or(fmt::Error)?;
        if requested > self.expected_bytes {
            return Err(fmt::Error);
        }
        self.output.push_str(value);
        Ok(())
    }
}

impl FamilyFingerprintSink for FamilyFingerprintWriter {
    fn literal(&mut self, value: &str) -> Result<(), GenericFamilyError> {
        self.write_str(value)
            .map_err(|_| Self::formatting_failure())
    }

    fn usize_value(&mut self, value: usize) -> Result<(), GenericFamilyError> {
        write!(self, "{value}").map_err(|_| Self::formatting_failure())
    }

    fn polynomial_term(&mut self) -> Result<(), GenericFamilyError> {
        Ok(())
    }

    fn exponent_value(&mut self, value: u16) -> Result<(), GenericFamilyError> {
        write!(self, "{value}").map_err(|_| Self::formatting_failure())
    }

    fn integer_value(&mut self, value: &Integer) -> Result<(), GenericFamilyError> {
        let result = match value {
            Integer::Single(value) => {
                let sign = if *value < 0 { '-' } else { '+' };
                write!(self, "I{sign}{:X};", value.unsigned_abs())
            }
            Integer::Double(value) => {
                let sign = if *value < 0 { '-' } else { '+' };
                write!(self, "I{sign}{:X};", value.unsigned_abs())
            }
            // Rug's hexadecimal formatter emits a leading minus followed by
            // the magnitude, so no proportional GMP clone is needed here.
            Integer::Large(value) if value.is_negative() => write!(self, "I{value:X};"),
            Integer::Large(value) => write!(self, "I+{value:X};"),
        };
        result.map_err(|_| Self::formatting_failure())
    }
}

const fn decimal_digits_usize(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        value.ilog10() as usize + 1
    }
}

fn family_fingerprint_integer_bits(value: &Integer) -> Result<usize, GenericFamilyError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| GenericFamilyError::ResourceCountOverflow {
        resource: "family fingerprint integer bits",
    })
}

fn checked_bounded_fingerprint_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, GenericFamilyError> {
    let requested = current
        .checked_add(additional)
        .ok_or(GenericFamilyError::ResourceCountOverflow { resource })?;
    check_family_limit(resource, requested, limit)?;
    Ok(requested)
}

fn triangular(value: usize) -> Option<usize> {
    let successor = value.checked_add(1)?;
    let (left, right) = if value % 2 == 0 {
        (value / 2, successor)
    } else {
        (value, successor / 2)
    };
    left.checked_mul(right)
}

fn checked_scalar_product_count(
    loops: usize,
    externals: usize,
) -> Result<usize, GenericFamilyError> {
    if loops == 0 {
        return Err(GenericFamilyError::NoLoopMomenta);
    }
    let loop_loop = triangular(loops)
        .ok_or(GenericFamilyError::ScalarProductCountOverflow { loops, externals })?;
    let loop_external = loops
        .checked_mul(externals)
        .ok_or(GenericFamilyError::ScalarProductCountOverflow { loops, externals })?;
    loop_loop
        .checked_add(loop_external)
        .ok_or(GenericFamilyError::ScalarProductCountOverflow { loops, externals })
}

fn check_family_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GenericFamilyError> {
    if requested > limit {
        Err(GenericFamilyError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn build_coordinates(
    loops: usize,
    externals: usize,
    capacity: usize,
) -> Vec<ScalarProductCoordinate> {
    let mut coordinates = Vec::with_capacity(capacity);
    for left in 0..loops {
        for right in left..loops {
            coordinates.push(ScalarProductCoordinate::LoopLoop { left, right });
        }
    }
    for loop_index in 0..loops {
        for external_index in 0..externals {
            coordinates.push(ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            });
        }
    }
    coordinates
}

fn validate_and_retain_input_denominator(
    context: &CoefficientContext,
    coefficient: &Coefficient,
    location: CoefficientLocation,
    limits: ExactAlgebraLimits,
    conditions: &mut Vec<BaseNonZeroCondition>,
) -> Result<(), GenericFamilyError> {
    if let Err(error) = context.validate_with_limits(coefficient, limits) {
        if matches!(error, ExactAlgebraError::VariableMapMismatch { .. }) {
            return Err(GenericFamilyError::ForeignCoefficientContext { location });
        }
        return Err(GenericFamilyError::InvalidCoefficient { location, error });
    }
    if !coefficient.denominator.is_one() {
        let condition = make_family_nonzero_condition(
            location.clone(),
            coefficient.denominator.clone(),
            GuardOrigin::FamilyInputCoefficientDenominator { location },
        );
        if let Some(existing) = conditions
            .iter_mut()
            .find(|existing| existing.polynomial == condition.polynomial)
        {
            merge_family_condition(existing, &condition);
        } else {
            conditions.push(condition);
        }
    }
    Ok(())
}

fn make_family_nonzero_condition(
    source: CoefficientLocation,
    polynomial: BasePolynomial,
    origin: GuardOrigin,
) -> BaseNonZeroCondition {
    BaseNonZeroCondition {
        source,
        polynomial,
        origins: BTreeSet::from([origin]),
    }
}

fn merge_family_condition(target: &mut BaseNonZeroCondition, source: &BaseNonZeroCondition) {
    debug_assert_eq!(target.polynomial, source.polynomial);
    if source.source < target.source {
        target.source = source.source.clone();
    }
    target.origins.extend(source.origins.iter().cloned());
}

fn coefficients_are_equal(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
    limits: ExactAlgebraLimits,
) -> Result<bool, GenericFamilyError> {
    if left == right {
        return Ok(true);
    }
    Ok(context.try_sub(left, right, limits)?.is_zero())
}

fn affine_forms_are_equal(
    context: &CoefficientContext,
    left: &(Coefficient, Vec<Coefficient>),
    right: &(Coefficient, Vec<Coefficient>),
    limits: ExactAlgebraLimits,
) -> Result<bool, GenericFamilyError> {
    if left.1.len() != right.1.len() || !coefficients_are_equal(context, &left.0, &right.0, limits)?
    {
        return Ok(false);
    }
    for (left, right) in left.1.iter().zip(&right.1) {
        if !coefficients_are_equal(context, left, right, limits)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn invert_symbolic_matrix(
    context: &CoefficientContext,
    matrix: &[Vec<Coefficient>],
    limits: ExactAlgebraLimits,
) -> Result<(Vec<Vec<Coefficient>>, Coefficient), GenericFamilyError> {
    let size = matrix.len();
    if size == 0 || matrix.iter().any(|row| row.len() != size) {
        return Err(GenericFamilyError::SingularDenominatorBasis);
    }
    let augmented_columns = size
        .checked_mul(2)
        .ok_or(GenericFamilyError::MatrixDimensionOverflow { size })?;
    let _entry_count = size
        .checked_mul(augmented_columns)
        .ok_or(GenericFamilyError::MatrixDimensionOverflow { size })?;
    let mut augmented = vec![vec![context.zero(); augmented_columns]; size];
    for row in 0..size {
        for column in 0..size {
            augmented[row][column] = matrix[row][column].clone();
        }
        augmented[row][size + row] = context.one();
    }

    let mut determinant = context.one();
    for column in 0..size {
        let pivot = (column..size)
            .find(|&row| !augmented[row][column].is_zero())
            .ok_or(GenericFamilyError::SingularDenominatorBasis)?;
        if pivot != column {
            augmented.swap(pivot, column);
            determinant = context.try_neg(&determinant, limits)?;
        }
        let pivot_value = augmented[column][column].clone();
        determinant = context.try_mul(&determinant, &pivot_value, limits)?;
        for entry in &mut augmented[column] {
            *entry = context.try_div(entry, &pivot_value, limits)?;
        }
        for row in 0..size {
            if row == column {
                continue;
            }
            let factor = augmented[row][column].clone();
            if factor.is_zero() {
                continue;
            }
            for entry in 0..augmented_columns {
                let contribution = context.try_mul(&factor, &augmented[column][entry], limits)?;
                augmented[row][entry] =
                    context.try_sub(&augmented[row][entry], &contribution, limits)?;
            }
        }
    }
    if determinant.is_zero() || context.validate_with_limits(&determinant, limits).is_err() {
        return Err(GenericFamilyError::SingularDenominatorBasis);
    }
    let inverse = augmented
        .into_iter()
        .map(|row| row[size..].to_vec())
        .collect::<Vec<_>>();
    if inverse
        .iter()
        .flatten()
        .any(|coefficient| context.validate_with_limits(coefficient, limits).is_err())
    {
        return Err(GenericFamilyError::InternalVerificationFailure {
            detail: "matrix inversion changed the authenticated coefficient map".to_owned(),
        });
    }
    Ok((inverse, determinant))
}

fn verify_inverse(
    context: &CoefficientContext,
    matrix: &[Vec<Coefficient>],
    inverse: &[Vec<Coefficient>],
    limits: ExactAlgebraLimits,
) -> Result<(), GenericFamilyError> {
    let left = multiply_symbolic_matrices(context, matrix, inverse, limits)?;
    let right = multiply_symbolic_matrices(context, inverse, matrix, limits)?;
    for (side, product) in [("A A^-1", left), ("A^-1 A", right)] {
        for (row, values) in product.iter().enumerate() {
            for (column, coefficient) in values.iter().enumerate() {
                let expected = if row == column {
                    context.one()
                } else {
                    context.zero()
                };
                if !coefficients_are_equal(context, coefficient, &expected, limits)? {
                    return Err(GenericFamilyError::InternalVerificationFailure {
                        detail: format!("{side} differs from identity at ({row},{column})"),
                    });
                }
            }
        }
    }
    Ok(())
}

fn multiply_symbolic_matrices(
    context: &CoefficientContext,
    left: &[Vec<Coefficient>],
    right: &[Vec<Coefficient>],
    limits: ExactAlgebraLimits,
) -> Result<Vec<Vec<Coefficient>>, GenericFamilyError> {
    if left.is_empty()
        || right.is_empty()
        || left.iter().any(|row| row.len() != right.len())
        || right.iter().any(|row| row.len() != right[0].len())
    {
        return Err(GenericFamilyError::InternalVerificationFailure {
            detail: "matrix replay received incompatible dimensions".to_owned(),
        });
    }
    let mut product = vec![vec![context.zero(); right[0].len()]; left.len()];
    for row in 0..left.len() {
        for column in 0..right[0].len() {
            for inner in 0..right.len() {
                let contribution =
                    context.try_mul(&left[row][inner], &right[inner][column], limits)?;
                product[row][column] =
                    context.try_add(&product[row][column], &contribution, limits)?;
            }
            if context
                .validate_with_limits(&product[row][column], limits)
                .is_err()
            {
                return Err(GenericFamilyError::InternalVerificationFailure {
                    detail: "matrix replay changed the authenticated coefficient map".to_owned(),
                });
            }
        }
    }
    Ok(product)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_denominators(context: &CoefficientContext, size: usize) -> Vec<AffineDenominator> {
        (0..size)
            .map(|row| {
                AffineDenominator::new(
                    context.zero(),
                    (0..size)
                        .map(|column| {
                            if row == column {
                                context.one()
                            } else {
                                context.zero()
                            }
                        })
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn family_name_retention_moves_owned_buffers_and_fallibly_copies_borrowed_names() {
        let owned = String::from("owned-family-name");
        let owned_pointer = owned.as_ptr();
        let retained_owned = retain_family_name(Cow::Owned(owned)).unwrap();
        assert_eq!(retained_owned.as_ptr(), owned_pointer);

        let borrowed = "borrowed-family-name";
        let retained_borrowed = retain_family_name(Cow::Borrowed(borrowed)).unwrap();
        assert_eq!(retained_borrowed, borrowed);
        assert_ne!(retained_borrowed.as_ptr(), borrowed.as_ptr());
    }

    #[test]
    fn proportional_family_limits_and_identity_strings_precede_label_sets() {
        let context = CoefficientContext::new(["d"]);
        let scalar_limit_first = IntegralFamily::new_with_limits(
            "duplicate-loop-labels",
            vec!["k".into(), "k".into()],
            Vec::new(),
            context.clone(),
            context.one(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            IntegralFamilyLimits {
                max_scalar_products: 0,
                ..IntegralFamilyLimits::default()
            },
        );
        assert!(matches!(
            scalar_limit_first,
            Err(GenericFamilyError::ResourceLimit {
                resource: "family scalar products",
                requested: 3,
                limit: 0,
            })
        ));

        let oversized_borrowed_name = "borrowed-family-name";
        let name_limit_first = IntegralFamily::new_with_limits(
            oversized_borrowed_name,
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.one(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            IntegralFamilyLimits {
                max_fingerprint_bytes: 4,
                ..IntegralFamilyLimits::default()
            },
        );
        assert!(matches!(
            name_limit_first,
            Err(GenericFamilyError::ResourceLimit {
                resource: "family fingerprint bytes",
                requested,
                limit: 4,
            }) if requested == oversized_borrowed_name.len()
        ));

        let oversized_label = "loop-label-too-long";
        let label_limit_first = IntegralFamily::new_with_limits(
            "x",
            vec![oversized_label.into()],
            Vec::new(),
            context.clone(),
            context.one(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            IntegralFamilyLimits {
                max_fingerprint_bytes: 4,
                ..IntegralFamilyLimits::default()
            },
        );
        assert!(matches!(
            label_limit_first,
            Err(GenericFamilyError::ResourceLimit {
                resource: "family fingerprint string bytes",
                requested,
                limit: 4,
            }) if requested == oversized_label.len()
        ));

        // Symbolica parameter labels are identifiers.  Keep this fixture
        // oversized for the RustRed fingerprint limit without failing in the
        // coefficient-context parser before that boundary is reached.
        let oversized_parameter = "parameter_label_too_long";
        let parameter_context = CoefficientContext::new([oversized_parameter]);
        let parameter_limit_first = IntegralFamily::new_with_limits(
            "x",
            vec!["k".into()],
            Vec::new(),
            parameter_context.clone(),
            parameter_context.one(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            IntegralFamilyLimits {
                max_fingerprint_bytes: 4,
                ..IntegralFamilyLimits::default()
            },
        );
        assert!(matches!(
            parameter_limit_first,
            Err(GenericFamilyError::ResourceLimit {
                resource: "family fingerprint string bytes",
                requested,
                limit: 4,
            }) if requested == oversized_parameter.len()
        ));
    }

    #[test]
    fn coordinates_are_loop_loop_then_loop_external() {
        let context = CoefficientContext::new(["d"]);
        let family = IntegralFamily::new(
            "two-loop-two-leg",
            vec!["k1".into(), "k2".into()],
            vec!["p1".into(), "p2".into()],
            context.clone(),
            context.parameter("d").unwrap(),
            identity_denominators(&context, 7),
            vec![
                vec![context.one(), context.zero()],
                vec![context.zero(), context.one()],
            ],
            vec![context.zero(); 7],
        )
        .unwrap();

        assert_eq!(
            family.coordinates(),
            &[
                ScalarProductCoordinate::LoopLoop { left: 0, right: 0 },
                ScalarProductCoordinate::LoopLoop { left: 0, right: 1 },
                ScalarProductCoordinate::LoopLoop { left: 1, right: 1 },
                ScalarProductCoordinate::LoopExternal {
                    loop_index: 0,
                    external_index: 0,
                },
                ScalarProductCoordinate::LoopExternal {
                    loop_index: 0,
                    external_index: 1,
                },
                ScalarProductCoordinate::LoopExternal {
                    loop_index: 1,
                    external_index: 0,
                },
                ScalarProductCoordinate::LoopExternal {
                    loop_index: 1,
                    external_index: 1,
                },
            ]
        );
        assert_eq!(
            family.contraction_momenta(),
            &[
                ContractionMomentum::Loop(0),
                ContractionMomentum::Loop(1),
                ContractionMomentum::External(0),
                ContractionMomentum::External(1),
            ]
        );
        assert_eq!(
            family
                .coordinate_index(ScalarProductCoordinate::LoopExternal {
                    loop_index: 1,
                    external_index: 0,
                })
                .unwrap(),
            5
        );
        family.verify_exact_replay().unwrap();
    }

    #[test]
    fn symbolic_nonsymmetric_basis_has_guarded_exact_inverse() {
        let context = CoefficientContext::new(["d", "a", "b", "s"]);
        let d = context.parameter("d").unwrap();
        let a_over_s = context.parse("a/s").unwrap();
        let b = context.parameter("b").unwrap();
        let c0 = context.parse("a+1").unwrap();
        let c1 = context.parse("b-2").unwrap();
        let family = IntegralFamily::new(
            "symbolic",
            vec!["k".into()],
            vec!["p".into()],
            context.clone(),
            d,
            vec![
                AffineDenominator::new(c0, vec![a_over_s, context.one()]),
                AffineDenominator::new(c1, vec![b, context.integer(2)]),
            ],
            vec![vec![context.parse("s").unwrap()]],
            vec![context.parse("a/3").unwrap(), context.zero()],
        )
        .unwrap();

        assert_eq!(
            family.domain().basis_determinant(),
            &context.parse("(2*a-b*s)/s").unwrap()
        );
        assert_eq!(
            family.domain().determinant_nonzero().polynomial(),
            &context.parse("2*a-b*s").unwrap().numerator
        );
        assert!(family.domain().input_denominators().iter().any(|guard| {
            guard.source()
                == &CoefficientLocation::DenominatorCoefficient {
                    denominator: 0,
                    coordinate: 0,
                }
                && guard.polynomial() == &context.parse("s").unwrap().numerator
        }));
        assert!(family.domain().input_denominators().iter().any(|guard| {
            guard.source() == &CoefficientLocation::PowerShift { denominator: 0 }
                && guard.polynomial() == &context.integer(3).numerator
        }));
        family.verify_exact_replay().unwrap();
    }

    #[test]
    fn equal_family_input_denominators_merge_all_typed_origins() {
        let context = CoefficientContext::new(["d", "m", "a", "nu", "s"]);
        let family = IntegralFamily::new(
            "merged-input-denominators",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.parse("d/s").unwrap(),
            vec![AffineDenominator::new(
                context.parse("m/s").unwrap(),
                vec![context.parse("a/s").unwrap()],
            )],
            Vec::new(),
            vec![context.parse("nu/s").unwrap()],
        )
        .unwrap();

        assert_eq!(family.domain().input_denominators().len(), 1);
        let condition = &family.domain().input_denominators()[0];
        assert_eq!(
            condition.polynomial(),
            &context.parameter("s").unwrap().numerator
        );
        for location in [
            CoefficientLocation::Dimension,
            CoefficientLocation::DenominatorConstant { denominator: 0 },
            CoefficientLocation::DenominatorCoefficient {
                denominator: 0,
                coordinate: 0,
            },
            CoefficientLocation::PowerShift { denominator: 0 },
        ] {
            assert!(
                condition
                    .origins()
                    .contains(&GuardOrigin::FamilyInputCoefficientDenominator { location })
            );
        }
    }

    #[test]
    fn external_derivative_contractions_include_gram_constants() {
        let context = CoefficientContext::new(["d", "m2", "c", "s", "nu"]);
        let m2 = context.parameter("m2").unwrap();
        let c = context.parameter("c").unwrap();
        let s = context.parameter("s").unwrap();
        let family = IntegralFamily::new(
            "one-loop-one-leg",
            vec!["k".into()],
            vec!["p".into()],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(m2.clone(), vec![context.one(), context.zero()]),
                AffineDenominator::new(c.clone(), vec![context.zero(), context.one()]),
            ],
            vec![vec![s.clone()]],
            vec![context.parameter("nu").unwrap(), context.zero()],
        )
        .unwrap();

        let k_d0 = family
            .derivative_contraction(0, 0, ContractionMomentum::Loop(0))
            .unwrap();
        assert_eq!(k_d0.constant(), &(-(&context.integer(2) * &m2)));
        assert_eq!(
            k_d0.denominator_coefficients(),
            &[context.integer(2), context.zero()]
        );

        let p_d0 = family
            .derivative_contraction(0, 0, ContractionMomentum::External(0))
            .unwrap();
        assert_eq!(p_d0.constant(), &(-(&context.integer(2) * &c)));
        assert_eq!(
            p_d0.denominator_coefficients(),
            &[context.zero(), context.integer(2)]
        );

        let p_d1 = family
            .derivative_contraction(1, 0, ContractionMomentum::External(0))
            .unwrap();
        assert_eq!(p_d1.constant(), &s);
        assert_eq!(
            p_d1.denominator_coefficients(),
            &[context.zero(), context.zero()]
        );
        family.verify_exact_replay().unwrap();
    }

    #[test]
    fn validates_labels_gram_arities_and_contexts() {
        let context = CoefficientContext::new(["d"]);
        let result = IntegralFamily::new(
            "none",
            Vec::new(),
            Vec::new(),
            context.clone(),
            context.one(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        assert!(matches!(result, Err(GenericFamilyError::NoLoopMomenta)));

        let result = IntegralFamily::new(
            "overlap",
            vec!["q".into()],
            vec!["q".into()],
            context.clone(),
            context.one(),
            identity_denominators(&context, 2),
            vec![vec![context.one()]],
            vec![context.zero(); 2],
        );
        assert!(matches!(
            result,
            Err(GenericFamilyError::MomentumLabelOverlap { .. })
        ));

        let result = IntegralFamily::new(
            "wrong-denominator-count",
            vec!["k".into()],
            vec!["p".into()],
            context.clone(),
            context.one(),
            identity_denominators(&context, 1),
            vec![vec![context.one()]],
            vec![context.zero(); 2],
        );
        assert!(matches!(
            result,
            Err(GenericFamilyError::WrongDenominatorCount {
                expected: 2,
                actual: 1
            })
        ));

        let result = IntegralFamily::new(
            "wrong-power-shift-count",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.one(),
            identity_denominators(&context, 1),
            Vec::new(),
            Vec::new(),
        );
        assert!(matches!(
            result,
            Err(GenericFamilyError::WrongPowerShiftCount {
                expected: 1,
                actual: 0
            })
        ));

        let result = IntegralFamily::new(
            "bad-gram",
            vec!["k".into()],
            vec!["p".into(), "q".into()],
            context.clone(),
            context.one(),
            identity_denominators(&context, 3),
            vec![
                vec![context.one(), context.one()],
                vec![context.zero(), context.one()],
            ],
            vec![context.zero(); 3],
        );
        assert!(matches!(
            result,
            Err(GenericFamilyError::AsymmetricExternalGram { row: 0, column: 1 })
        ));

        let foreign = CoefficientContext::new(["x"]);
        let result = IntegralFamily::new(
            "foreign",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            foreign.one(),
            identity_denominators(&context, 1),
            Vec::new(),
            vec![context.zero()],
        );
        assert!(matches!(
            result,
            Err(GenericFamilyError::ForeignCoefficientContext {
                location: CoefficientLocation::Dimension
            })
        ));
    }

    #[test]
    fn singular_symbolic_basis_is_rejected_but_singular_external_gram_is_allowed() {
        let context = CoefficientContext::new(["d"]);
        let singular = IntegralFamily::new(
            "singular",
            vec!["k".into()],
            vec!["p".into()],
            context.clone(),
            context.one(),
            vec![
                AffineDenominator::new(context.zero(), vec![context.one(), context.integer(2)]),
                AffineDenominator::new(
                    context.zero(),
                    vec![context.integer(2), context.integer(4)],
                ),
            ],
            vec![vec![context.zero()]],
            vec![context.zero(); 2],
        );
        assert!(matches!(
            singular,
            Err(GenericFamilyError::SingularDenominatorBasis)
        ));

        let valid = IntegralFamily::new(
            "null-external",
            vec!["k".into()],
            vec!["p".into()],
            context.clone(),
            context.one(),
            identity_denominators(&context, 2),
            vec![vec![context.zero()]],
            vec![context.zero(); 2],
        )
        .unwrap();
        valid.verify_exact_replay().unwrap();
    }

    #[test]
    fn rational_base_field_without_parameters_is_supported() {
        let context = CoefficientContext::new(Vec::<String>::new());
        let family = IntegralFamily::new(
            "rational",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.integer(4),
            identity_denominators(&context, 1),
            Vec::new(),
            vec![context.zero()],
        )
        .unwrap();
        assert!(family.coefficient_context().parameter_names().is_empty());
        family.verify_exact_replay().unwrap();
    }

    #[test]
    fn family_authentication_rejects_malformed_coefficients_and_resource_limits() {
        let context = CoefficientContext::new(["x"]);
        let mut malformed_dimension = context.one();
        malformed_dimension.numerator.exponents.push(0);
        let malformed = IntegralFamily::new(
            "malformed",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            malformed_dimension,
            identity_denominators(&context, 1),
            Vec::new(),
            vec![context.zero()],
        );
        assert!(matches!(
            malformed,
            Err(GenericFamilyError::InvalidCoefficient {
                location: CoefficientLocation::Dimension,
                error: ExactAlgebraError::MalformedExponentLayout { .. },
            })
        ));

        let limited = IntegralFamily::new_with_limits(
            "limited",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.one(),
            identity_denominators(&context, 1),
            Vec::new(),
            vec![context.zero()],
            IntegralFamilyLimits {
                max_scalar_products: 0,
                ..IntegralFamilyLimits::default()
            },
        );
        assert!(matches!(
            limited,
            Err(GenericFamilyError::ResourceLimit {
                resource: "family scalar products",
                requested: 1,
                limit: 0,
            })
        ));
    }

    fn huge_gmp_fingerprint_family(
        limits: IntegralFamilyLimits,
    ) -> Result<IntegralFamily, GenericFamilyError> {
        let context = CoefficientContext::new(["x"]);
        let decimal = format!("1{}", "0".repeat(1_500));
        let magnitude = decimal.parse::<Integer>().unwrap();
        let mut dimension = context.parameter("x").unwrap();
        dimension.numerator.coefficients[0] = -magnitude;
        IntegralFamily::new_with_limits(
            "huge-gmp-fingerprint",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            dimension,
            identity_denominators(&context, 1),
            Vec::new(),
            vec![context.zero()],
            limits,
        )
    }

    #[test]
    fn typed_fingerprint_preflights_exact_and_one_below_huge_gmp_payloads() {
        let family = huge_gmp_fingerprint_family(IntegralFamilyLimits::default()).unwrap();
        let stats = family.fingerprint_stats();
        assert_eq!(stats.encoded_bytes(), family.fingerprint_ref().len());
        assert!(stats.integer_bits() > 4_000);
        assert!(family.fingerprint_ref().contains("I-"));
        let cloned = family.clone();
        assert!(Arc::ptr_eq(&family.fingerprint, &cloned.fingerprint));

        let mut exact = IntegralFamilyLimits::default();
        exact.max_fingerprint_bytes = stats.encoded_bytes();
        exact.max_fingerprint_encoding_work = stats.encoding_work();
        exact.max_fingerprint_polynomial_terms = stats.polynomial_terms();
        exact.max_fingerprint_exponent_entries = stats.exponent_entries();
        exact.max_fingerprint_integer_bits = stats.integer_bits();
        let rebuilt = huge_gmp_fingerprint_family(exact).unwrap();
        assert_eq!(rebuilt.fingerprint_ref(), family.fingerprint_ref());
        assert_eq!(rebuilt.fingerprint_stats(), stats);

        macro_rules! one_below {
            ($field:ident, $getter:ident, $resource:literal) => {{
                let requested = stats.$getter();
                assert!(requested > 0, $resource);
                let mut limits = IntegralFamilyLimits::default();
                limits.$field = requested - 1;
                assert!(matches!(
                    huge_gmp_fingerprint_family(limits),
                    Err(GenericFamilyError::ResourceLimit {
                        resource: actual,
                        requested: actual_requested,
                        limit,
                    }) if actual == $resource
                        && actual_requested == requested
                        && limit == requested - 1
                ));
            }};
        }
        one_below!(
            max_fingerprint_bytes,
            encoded_bytes,
            "family fingerprint bytes"
        );
        one_below!(
            max_fingerprint_encoding_work,
            encoding_work,
            "family fingerprint encoding work"
        );
        one_below!(
            max_fingerprint_polynomial_terms,
            polynomial_terms,
            "family fingerprint polynomial terms"
        );
        one_below!(
            max_fingerprint_exponent_entries,
            exponent_entries,
            "family fingerprint exponent entries"
        );
        one_below!(
            max_fingerprint_integer_bits,
            integer_bits,
            "family fingerprint integer bits"
        );
    }
}
