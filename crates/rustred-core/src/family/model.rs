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
            max_matrix_exact_operations: DEFAULT_MAX_EXACT_OPERATIONS,
            max_matrix_input_retained_bytes: DEFAULT_MAX_INPUT_RETAINED_BYTES,
            max_matrix_output_retained_bytes: DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
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
    pub(super) encoded_bytes: usize,
    pub(super) encoding_work: usize,
    pub(super) polynomial_terms: usize,
    pub(super) exponent_entries: usize,
    pub(super) integer_bits: usize,
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
/// `input_denominators` are retained even if factors cancel in the determinant
/// or inverse.  The determinant numerator is a separate condition; a family
/// specialization is valid only when every listed polynomial is nonzero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyDomain {
    pub(super) input_denominators: Vec<FamilyNonZeroCondition>,
    pub(super) basis_determinant: Coefficient,
    pub(super) determinant_nonzero: FamilyNonZeroCondition,
}

impl FamilyDomain {
    pub fn input_denominators(&self) -> &[FamilyNonZeroCondition] {
        &self.input_denominators
    }

    pub fn basis_determinant(&self) -> &Coefficient {
        &self.basis_determinant
    }

    pub fn determinant_nonzero(&self) -> &FamilyNonZeroCondition {
        &self.determinant_nonzero
    }

    pub fn conditions(&self) -> impl Iterator<Item = &FamilyNonZeroCondition> {
        self.input_denominators
            .iter()
            .filter(|condition| condition.polynomial != self.determinant_nonzero.polynomial)
            .chain(std::iter::once(&self.determinant_nonzero))
    }
}

/// A complete, loop-count-independent affine integral family.
#[derive(Clone, Debug)]
pub struct IntegralFamily {
    pub(super) name: String,
    // `Arc<String>` moves the already fallibly allocated user-sized buffer;
    // cloning a family shares it. Only the fixed-size Arc header allocation is
    // infallible, unlike `String -> Arc<str>`, which may copy proportionally.
    pub(super) fingerprint: Arc<String>,
    pub(super) fingerprint_stats: IntegralFamilyFingerprintStats,
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
}
