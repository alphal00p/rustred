//! Core value types and authenticated integral-family state.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::algebra::matrix::{
    DEFAULT_MAX_EXACT_OPERATIONS, DEFAULT_MAX_INPUT_RETAINED_BYTES,
    DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
};
use crate::algebra::{Coefficient, CoefficientContext, CoefficientPolynomial, ExactAlgebraLimits};

/// One coefficient-valued family datum that can contribute a generic-domain
/// nonzero condition.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CoefficientLocation {
    Dimension,
    DenominatorConstant {
        denominator: usize,
    },
    DenominatorCoefficient {
        denominator: usize,
        coordinate: usize,
    },
    ExternalGram {
        row: usize,
        column: usize,
    },
    PowerShift {
        denominator: usize,
    },
    BasisDeterminantNumerator,
}

impl CoefficientLocation {
    /// Version-stable identity used in user-facing output and proof payloads.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing coefficient-location provenance to String cannot fail");
        output
    }

    pub(crate) fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Dimension => writer.write_str("dimension"),
            Self::DenominatorConstant { denominator } => {
                write!(writer, "denominator-constant:{denominator}")
            }
            Self::DenominatorCoefficient {
                denominator,
                coordinate,
            } => write!(writer, "denominator-coefficient:{denominator}:{coordinate}"),
            Self::ExternalGram { row, column } => {
                write!(writer, "external-gram:{row}:{column}")
            }
            Self::PowerShift { denominator } => write!(writer, "power-shift:{denominator}"),
            Self::BasisDeterminantNumerator => writer.write_str("basis-determinant-numerator"),
        }
    }
}

/// Resource policy for constructing and replaying one complete affine family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntegralFamilyLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_scalar_products: usize,
    /// Aggregate checked scalar operations admitted for one native Symbolica
    /// determinant/inverse/verification session.
    pub max_matrix_exact_operations: usize,
    /// Aggregate clone-owned retained bytes admitted for authenticated matrix
    /// inputs in one native Symbolica session.
    pub max_matrix_input_retained_bytes: usize,
    /// Aggregate clone-owned retained bytes admitted for native determinant,
    /// inverse, and verification outputs in one Symbolica session.
    pub max_matrix_output_retained_bytes: usize,
    pub max_matrix_entries: usize,
    /// Number of cached `(denominator, differentiated loop, contraction)`
    /// affine expansions.
    pub max_derivative_contractions: usize,
    /// Total [`Coefficient`] cells retained by those cached expansions,
    /// including each affine constant and every dense denominator coefficient.
    ///
    /// This is a structural cell bound, not an RSS bound: Symbolica's public
    /// coefficient API does not expose the heap capacity of every exact
    /// polynomial owned by a cell.
    pub max_derivative_contraction_coefficient_cells: usize,
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
            max_matrix_exact_operations: DEFAULT_MAX_EXACT_OPERATIONS,
            max_matrix_input_retained_bytes: DEFAULT_MAX_INPUT_RETAINED_BYTES,
            max_matrix_output_retained_bytes: DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
            max_matrix_entries: 16_000_000,
            max_derivative_contractions: 16_000_000,
            max_derivative_contraction_coefficient_cells: 16_000_000,
            max_fingerprint_bytes: 1024 * 1024 * 1024,
            max_fingerprint_encoding_work: 4_000_000_000_000_000,
            max_fingerprint_polynomial_terms: 256_000_000,
            max_fingerprint_exponent_entries: 16_000_000_000,
            max_fingerprint_integer_bits: 4_000_000_000_000_000,
        }
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
    pub(super) constant: Coefficient,
    pub(super) coefficients: Vec<Coefficient>,
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

impl AsRef<[Coefficient]> for AffineDenominator {
    fn as_ref(&self) -> &[Coefficient] {
        &self.coefficients
    }
}

/// An affine form in the ordered denominator basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenominatorExpansion {
    pub(super) constant: Coefficient,
    pub(super) denominator_coefficients: Vec<Coefficient>,
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
pub struct FamilyNonZeroCondition {
    pub(super) polynomial: CoefficientPolynomial,
    pub(super) sources: BTreeSet<CoefficientLocation>,
}

impl FamilyNonZeroCondition {
    pub fn polynomial(&self) -> &CoefficientPolynomial {
        &self.polynomial
    }

    /// Every family datum that contributed this exact polynomial condition.
    /// Sources are sorted independently of construction order.
    pub fn sources(&self) -> &BTreeSet<CoefficientLocation> {
        &self.sources
    }
}

/// The exact domain on which the denominator-coordinate map is valid.
///
/// Input-denominator guards are retained even if factors cancel in the
/// determinant or inverse. The determinant numerator is merged into the same
/// canonical condition list; a specialization is valid only when every
/// listed polynomial is nonzero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyDomain {
    pub(super) conditions: Vec<FamilyNonZeroCondition>,
    pub(super) basis_determinant: Coefficient,
}

impl FamilyDomain {
    pub fn basis_determinant(&self) -> &Coefficient {
        &self.basis_determinant
    }

    pub fn conditions(&self) -> impl Iterator<Item = &FamilyNonZeroCondition> {
        self.conditions.iter()
    }
}

/// A complete, loop-count-independent affine integral family.
#[derive(Debug)]
pub struct IntegralFamily {
    pub(super) name: String,
    // Retain the already fallibly built String allocation. Converting it to
    // Arc<str> would allocate and copy the complete caller-sized identity a
    // second time through an infallible standard-library conversion.
    pub(super) fingerprint: Arc<String>,
    pub(super) loop_momenta: Vec<String>,
    pub(super) external_momenta: Vec<String>,
    pub(super) coefficients: CoefficientContext,
    pub(super) dimension: Coefficient,
    pub(super) coordinates: Vec<ScalarProductCoordinate>,
    pub(super) contractions: Vec<ContractionMomentum>,
    pub(super) denominators: Vec<AffineDenominator>,
    pub(super) external_gram: Vec<Vec<Coefficient>>,
    pub(super) power_shifts: Vec<Coefficient>,
    pub(super) limits: IntegralFamilyLimits,
    pub(super) inverse_basis: Vec<Vec<Coefficient>>,
    pub(super) domain: FamilyDomain,
    // denominator -> differentiated loop -> contraction momentum
    pub(super) derivative_contractions: Vec<Vec<Vec<DenominatorExpansion>>>,
}

impl IntegralFamily {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Stable typed semantic identity cached during authenticated family
    /// construction.
    pub fn fingerprint(&self) -> &str {
        self.fingerprint.as_str()
    }

    pub(crate) fn fingerprint_owner(&self) -> Arc<String> {
        self.fingerprint.clone()
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

    /// Matrix `A^-1` in `S = A^-1 (D-c)` orientation.
    pub fn inverse_basis(&self) -> &[Vec<Coefficient>] {
        &self.inverse_basis
    }

    pub fn domain(&self) -> &FamilyDomain {
        &self.domain
    }
}
