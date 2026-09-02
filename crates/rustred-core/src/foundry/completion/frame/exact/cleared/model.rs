//! Fraction-free proof values and resource policy.

use std::fmt;

use crate::algebra::{
    ExactAlgebraLimits, IndexedAlgebraError, IndexedAlgebraLimits, IndexedPolynomial,
};
use crate::foundry::completion::frame::SourceInstanceId;

use super::super::{ExactCircuitGuardOrigin, ExactTargetCircuit, ExactTargetCircuitIdentity};

/// Explicit bounds for fraction-free reconstruction at promotion.
///
/// Symbolica's native multivariate GCD has no scratch-memory callback. The
/// pair-work cap is therefore a conservative ingress envelope. Every native
/// result crosses the authenticated indexed-algebra boundary; reconstructed
/// polynomial outputs retained by this proof are charged cumulatively, while
/// operation-local algebra limits and the structural row/column caps bound
/// transient values and source-owned inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClearedCircuitLimits {
    pub(super) exact_algebra: ExactAlgebraLimits,
    pub(super) fixed_index_specialization: IndexedAlgebraLimits,
    pub(super) max_source_contributions: usize,
    pub(super) max_source_terms: usize,
    pub(super) max_physical_columns: usize,
    pub(super) max_polynomial_operations: usize,
    pub(super) max_gcd_term_pairs: usize,
    pub(super) max_retained_polynomial_terms: usize,
    pub(super) max_guards: usize,
    pub(super) max_guard_origins: usize,
    pub(super) max_condition_source_entries: usize,
    pub(super) max_guard_serialization_bytes: usize,
}

impl Default for ClearedCircuitLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            fixed_index_specialization: IndexedAlgebraLimits::default(),
            max_source_contributions: 65_536,
            max_source_terms: 16_000_000,
            max_physical_columns: 4_000_000,
            max_polynomial_operations: 100_000_000,
            max_gcd_term_pairs: 64_000_000,
            max_retained_polynomial_terms: 64_000_000,
            max_guards: 1_000_000,
            max_guard_origins: 4_000_000,
            max_condition_source_entries: 4_000_000,
            max_guard_serialization_bytes: 64 * 1024 * 1024,
        }
    }
}

impl ClearedCircuitLimits {
    pub(crate) const fn with_max_polynomial_operations(mut self, limit: usize) -> Self {
        self.max_polynomial_operations = limit;
        self
    }
}

/// Typed failure at the fraction-free reconstruction boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ClearedCircuitError {
    WrongContext,
    InvalidCircuitBinding {
        detail: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    NativePanic {
        operation: &'static str,
    },
    NonExactPolynomialDivision,
    RationalCoefficientSurvivedClearing,
    ZeroSourceOrFamilyGuard,
    ZeroFinalTargetCoefficient,
    ReplayMismatch {
        physical_column: usize,
        detail: &'static str,
    },
    IndexedAlgebra(IndexedAlgebraError),
}

impl fmt::Display for ClearedCircuitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str(
                "the exact circuit, physical frame, and indexed context are not identical",
            ),
            Self::InvalidCircuitBinding { detail } => {
                write!(formatter, "invalid cleared-circuit binding: {detail}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
            }
            Self::NonExactPolynomialDivision => {
                formatter.write_str("a claimed exact polynomial division retained a denominator")
            }
            Self::RationalCoefficientSurvivedClearing => formatter
                .write_str("fraction-free source replay retained a rational physical coefficient"),
            Self::ZeroSourceOrFamilyGuard => formatter.write_str(
                "a source or family condition is identically zero on the circuit's fixed stratum",
            ),
            Self::ZeroFinalTargetCoefficient => {
                formatter.write_str("the cleared final target coefficient is zero")
            }
            Self::ReplayMismatch {
                physical_column,
                detail,
            } => write!(
                formatter,
                "cleared source replay failed at physical column {physical_column}: {detail}"
            ),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ClearedCircuitError {}

impl From<IndexedAlgebraError> for ClearedCircuitError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}

/// Provenance retained after elimination-path guards have been separated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ClearedSemanticGuardOrigin {
    SourceOrFamily(ExactCircuitGuardOrigin),
    FinalTargetCoefficient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClearedSemanticGuard {
    pub(super) polynomial: IndexedPolynomial,
    pub(super) origins: Box<[ClearedSemanticGuardOrigin]>,
}

impl ClearedSemanticGuard {
    pub(crate) const fn polynomial(&self) -> &IndexedPolynomial {
        &self.polynomial
    }

    pub(crate) fn origins(&self) -> &[ClearedSemanticGuardOrigin] {
        &self.origins
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ClearedGuardTelemetry {
    pub(super) before_unique: usize,
    pub(super) before_source_or_family_only: usize,
    pub(super) before_intermediate_only: usize,
    pub(super) before_mixed: usize,
    pub(super) after_unique: usize,
    pub(super) after_source_or_family: usize,
    pub(super) final_target_guard_retained: bool,
}

impl ClearedGuardTelemetry {
    pub(crate) const fn before_unique(self) -> usize {
        self.before_unique
    }

    pub(crate) const fn before_source_or_family_only(self) -> usize {
        self.before_source_or_family_only
    }

    pub(crate) const fn before_intermediate_only(self) -> usize {
        self.before_intermediate_only
    }

    pub(crate) const fn before_mixed(self) -> usize {
        self.before_mixed
    }

    pub(crate) const fn after_unique(self) -> usize {
        self.after_unique
    }

    pub(crate) const fn after_source_or_family(self) -> usize {
        self.after_source_or_family
    }

    pub(crate) const fn final_target_guard_retained(self) -> bool {
        self.final_target_guard_retained
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClearedSourceCofactor {
    pub(super) frame_row_ordinal: usize,
    pub(super) source_instance: SourceInstanceId,
    pub(super) row_denominator: IndexedPolynomial,
    pub(super) cofactor: IndexedPolynomial,
}

impl ClearedSourceCofactor {
    pub(crate) const fn frame_row_ordinal(&self) -> usize {
        self.frame_row_ordinal
    }

    pub(crate) const fn source_instance(&self) -> &SourceInstanceId {
        &self.source_instance
    }

    pub(crate) const fn row_denominator(&self) -> &IndexedPolynomial {
        &self.row_denominator
    }

    pub(crate) const fn cofactor(&self) -> &IndexedPolynomial {
        &self.cofactor
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClearedPhysicalTerm {
    pub(super) physical_column: usize,
    pub(super) coefficient: IndexedPolynomial,
}

impl ClearedPhysicalTerm {
    pub(crate) const fn physical_column(&self) -> usize {
        self.physical_column
    }

    pub(crate) const fn coefficient(&self) -> &IndexedPolynomial {
        &self.coefficient
    }
}

/// A replayed polynomial consequence, with no ownership authority.
#[derive(Clone, Debug)]
pub(crate) struct ClearedExactCircuit {
    pub(super) circuit_identity: ExactTargetCircuitIdentity,
    pub(super) target_column: usize,
    pub(super) target_coefficient: IndexedPolynomial,
    pub(super) source_cofactors: Box<[ClearedSourceCofactor]>,
    pub(super) physical_terms: Box<[ClearedPhysicalTerm]>,
    pub(super) semantic_guards: Box<[ClearedSemanticGuard]>,
    pub(super) guard_telemetry: ClearedGuardTelemetry,
    pub(super) exact_operations: usize,
    pub(super) gcd_term_pairs: usize,
    pub(super) retained_polynomial_terms: usize,
}

// The identity token is authority, not mathematical content.  As for
// `ExactTargetCircuit`, deterministic reconstruction compares the complete
// structural certificate while binding is checked separately through
// `is_bound_to`.
impl PartialEq for ClearedExactCircuit {
    fn eq(&self, other: &Self) -> bool {
        self.target_column == other.target_column
            && self.target_coefficient == other.target_coefficient
            && self.source_cofactors == other.source_cofactors
            && self.physical_terms == other.physical_terms
            && self.semantic_guards == other.semantic_guards
            && self.guard_telemetry == other.guard_telemetry
            && self.exact_operations == other.exact_operations
            && self.gcd_term_pairs == other.gcd_term_pairs
            && self.retained_polynomial_terms == other.retained_polynomial_terms
    }
}

impl Eq for ClearedExactCircuit {}

impl ClearedExactCircuit {
    pub(crate) fn is_bound_to(&self, circuit: &ExactTargetCircuit) -> bool {
        self.circuit_identity.belongs_to(circuit)
    }

    pub(crate) const fn target_column(&self) -> usize {
        self.target_column
    }

    pub(crate) const fn target_coefficient(&self) -> &IndexedPolynomial {
        &self.target_coefficient
    }

    pub(crate) fn source_cofactors(&self) -> &[ClearedSourceCofactor] {
        &self.source_cofactors
    }

    pub(crate) fn physical_terms(&self) -> &[ClearedPhysicalTerm] {
        &self.physical_terms
    }

    pub(crate) fn semantic_guards(&self) -> &[ClearedSemanticGuard] {
        &self.semantic_guards
    }

    pub(crate) const fn guard_telemetry(&self) -> ClearedGuardTelemetry {
        self.guard_telemetry
    }

    pub(crate) const fn exact_operations(&self) -> usize {
        self.exact_operations
    }

    pub(crate) const fn gcd_term_pairs(&self) -> usize {
        self.gcd_term_pairs
    }

    pub(crate) const fn retained_polynomial_terms(&self) -> usize {
        self.retained_polynomial_terms
    }
}
