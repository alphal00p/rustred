use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::{FamilyDomain, IntegralFamily};

use super::limits::{DEFAULT_MAX_MATRIX_ENTRIES, check_limit};
use super::{Error, Limits, NonZeroCondition, Stats, verify};

/// A checked row-major coefficient matrix.
///
/// Empty dimensions are supported because a vacuum family has `B: L x 0` and
/// `C: 0 x 0`. This type owns only shape-checked candidate data; Symbolica
/// remains the matrix-algebra authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoefficientMatrix {
    pub(super) rows: usize,
    pub(super) columns: usize,
    pub(super) row_major: Box<[Coefficient]>,
}

impl CoefficientMatrix {
    pub fn try_new(
        rows: usize,
        columns: usize,
        row_major: impl IntoIterator<Item = Coefficient>,
    ) -> Result<Self, Error> {
        Self::try_new_with_max_entries(rows, columns, row_major, DEFAULT_MAX_MATRIX_ENTRIES)
    }

    /// Construct a matrix without consuming more than the declared payload
    /// plus one sentinel entry.
    pub fn try_new_with_max_entries(
        rows: usize,
        columns: usize,
        row_major: impl IntoIterator<Item = Coefficient>,
        max_entries: usize,
    ) -> Result<Self, Error> {
        let expected = rows
            .checked_mul(columns)
            .ok_or(Error::ResourceCountOverflow {
                resource: "exact matrix entries",
            })?;
        check_limit("exact matrix entries", expected, max_entries)?;

        let mut iterator = row_major.into_iter();
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(expected)
            .map_err(|_| Error::AllocationFailure {
                resource: "exact matrix entries",
                requested: expected,
            })?;
        for actual in 0..expected {
            let Some(entry) = iterator.next() else {
                return Err(Error::MatrixPayloadSize {
                    rows,
                    columns,
                    expected,
                    actual,
                });
            };
            retained.push(entry);
        }
        if iterator.next().is_some() {
            return Err(Error::MatrixPayloadTooLarge {
                rows,
                columns,
                expected,
            });
        }
        Ok(Self {
            rows,
            columns,
            row_major: retained.into_boxed_slice(),
        })
    }

    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub const fn columns(&self) -> usize {
        self.columns
    }

    pub fn entries(&self) -> &[Coefficient] {
        &self.row_major
    }

    pub fn get(&self, row: usize, column: usize) -> Option<&Coefficient> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.row_major.get(row * self.columns + column)
    }

    pub(super) fn at(&self, row: usize, column: usize) -> &Coefficient {
        &self.row_major[row * self.columns + column]
    }
}

/// Exact source-to-target momentum substitution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MomentumMap {
    pub(super) loop_linear: CoefficientMatrix,
    pub(super) loop_external: CoefficientMatrix,
    pub(super) external_linear: CoefficientMatrix,
}

impl MomentumMap {
    pub fn new(
        loop_linear: CoefficientMatrix,
        loop_external: CoefficientMatrix,
        external_linear: CoefficientMatrix,
    ) -> Self {
        Self {
            loop_linear,
            loop_external,
            external_linear,
        }
    }

    pub const fn loop_linear(&self) -> &CoefficientMatrix {
        &self.loop_linear
    }

    pub const fn loop_external(&self) -> &CoefficientMatrix {
        &self.loop_external
    }

    pub const fn external_linear(&self) -> &CoefficientMatrix {
        &self.external_linear
    }
}

/// `S_source = constant + linear * S_target` in each family's declared
/// scalar-product coordinate order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarProductMap {
    pub(super) constant: Box<[Coefficient]>,
    pub(super) linear: CoefficientMatrix,
}

impl ScalarProductMap {
    pub fn constant(&self) -> &[Coefficient] {
        &self.constant
    }

    pub const fn linear(&self) -> &CoefficientMatrix {
        &self.linear
    }
}

/// `D_source = constant + linear * D_target`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenominatorMap {
    pub(super) constant: Box<[Coefficient]>,
    pub(super) linear: CoefficientMatrix,
}

impl DenominatorMap {
    pub fn constant(&self) -> &[Coefficient] {
        &self.constant
    }

    pub const fn linear(&self) -> &CoefficientMatrix {
        &self.linear
    }
}

/// Exact action of one source denominator row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DenominatorAction {
    Monomial { target: usize, scale: Coefficient },
    Affine,
}

/// Loop-measure witness. The first parametric rule compiler accepts only
/// `Unit`; a formal determinant power must never be silently discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Jacobian {
    Unit { determinant_sign: i8 },
    FormalDeterminantPower { determinant: Coefficient },
}

/// Newly derived proof object. All fields are private so callers cannot
/// construct a certificate without verification.
#[derive(Clone, Debug)]
pub struct VerifiedMap {
    pub(super) source_family_fingerprint: String,
    pub(super) target_family_fingerprint: String,
    pub(super) momentum: MomentumMap,
    pub(super) scalar_products: ScalarProductMap,
    pub(super) denominators: DenominatorMap,
    pub(super) row_actions: Box<[DenominatorAction]>,
    pub(super) loop_determinant: Coefficient,
    pub(super) external_determinant: Coefficient,
    pub(super) jacobian: Jacobian,
    pub(super) source_domain: FamilyDomain,
    pub(super) target_domain: FamilyDomain,
    pub(super) candidate_denominator_conditions: Box<[CoefficientPolynomial]>,
    pub(super) nonzero_conditions: Box<[NonZeroCondition]>,
    pub(super) stats: Stats,
}

impl VerifiedMap {
    pub fn source_family_fingerprint(&self) -> &str {
        &self.source_family_fingerprint
    }

    pub fn target_family_fingerprint(&self) -> &str {
        &self.target_family_fingerprint
    }

    pub const fn momentum(&self) -> &MomentumMap {
        &self.momentum
    }

    pub const fn scalar_products(&self) -> &ScalarProductMap {
        &self.scalar_products
    }

    pub const fn denominators(&self) -> &DenominatorMap {
        &self.denominators
    }

    pub fn row_actions(&self) -> &[DenominatorAction] {
        &self.row_actions
    }

    pub const fn loop_determinant(&self) -> &Coefficient {
        &self.loop_determinant
    }

    pub const fn external_determinant(&self) -> &Coefficient {
        &self.external_determinant
    }

    pub const fn jacobian(&self) -> &Jacobian {
        &self.jacobian
    }

    pub const fn source_domain(&self) -> &FamilyDomain {
        &self.source_domain
    }

    pub const fn target_domain(&self) -> &FamilyDomain {
        &self.target_domain
    }

    /// Denominators of caller-supplied rational map entries, before any
    /// cancellation in derived matrices.
    pub fn candidate_denominator_conditions(&self) -> &[CoefficientPolynomial] {
        &self.candidate_denominator_conditions
    }

    /// The exact numerator of `det(A)`, required to be nonzero.
    pub const fn loop_determinant_nonzero_condition(&self) -> &CoefficientPolynomial {
        &self.loop_determinant.numerator
    }

    /// Complete merged nonzero domain for replay, including both family
    /// domains, all candidate denominators, both determinant numerators, and
    /// every monomial denominator scale numerator.
    pub fn nonzero_conditions(&self) -> &[NonZeroCondition] {
        &self.nonzero_conditions
    }

    pub const fn stats(&self) -> Stats {
        self.stats
    }

    /// Replay a retained map from its momentum witness. Derived data is
    /// compared structurally, including the complete pre-cancellation domain.
    pub fn replay(
        &self,
        source: &IntegralFamily,
        target: &IntegralFamily,
        limits: Limits,
    ) -> Result<(), Error> {
        let replayed = verify(source, target, self.momentum.clone(), limits)?;
        if replayed.source_family_fingerprint != self.source_family_fingerprint
            || replayed.target_family_fingerprint != self.target_family_fingerprint
            || replayed.scalar_products != self.scalar_products
            || replayed.denominators != self.denominators
            || replayed.row_actions != self.row_actions
            || replayed.loop_determinant != self.loop_determinant
            || replayed.external_determinant != self.external_determinant
            || replayed.jacobian != self.jacobian
            || replayed.source_domain != self.source_domain
            || replayed.target_domain != self.target_domain
            || replayed.candidate_denominator_conditions != self.candidate_denominator_conditions
            || replayed.nonzero_conditions != self.nonzero_conditions
        {
            return Err(Error::CertificateReplayMismatch);
        }
        Ok(())
    }
}
