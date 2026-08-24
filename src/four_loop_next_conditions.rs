//! Complete local exceptional-condition inventory for the frozen four-loop
//! next-shell elimination certificate.
//!
//! This layer scans every polynomial part whose vanishing can invalidate the
//! already-constructed exact matrix, parent normalization, exact pivot
//! normalization, or retained rules.  It factors over `Z[d]` with Symbolica,
//! normalizes factors structurally (never by formatted strings), proves exact
//! multiply-back and irreducibility, and retains every typed use site.
//!
//! The scope is deliberately narrow.  This is complete for the authenticated
//! fixed projected matrix and its local parent-row normalization.  It is not a
//! complete exceptional inventory for the upstream native four-loop IBP
//! construction.  In addition, all dimension-specialization results are
//! conditional on the independently stated scale assumption `m2 != 0`.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use symbolica::prelude::*;

use crate::coefficient::{
    CoefficientContext, CoefficientPolynomialPart, CoefficientProjectionError,
};
use crate::four_loop_corner_shell::FourLoopCornerColumnId;
use crate::four_loop_next_elimination::{
    FOUR_LOOP_NEXT_ELIMINATION_CHECKSUM, FOUR_LOOP_NEXT_ELIMINATION_COLUMNS,
    FOUR_LOOP_NEXT_ELIMINATION_INPUT_ENTRIES, FOUR_LOOP_NEXT_ELIMINATION_PIVOT_RULES,
    FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_RHS_ENTRIES, FOUR_LOOP_NEXT_ELIMINATION_RANK,
    FOUR_LOOP_NEXT_ELIMINATION_SOURCE_ROWS, FOUR_LOOP_NEXT_ELIMINATION_TRACE_REDUCTIONS,
    FourLoopNextElimination, FourLoopNextEliminationError, FourLoopNextEliminationStatus,
};
use crate::four_loop_next_manifest::FourLoopNextRawRowId;

const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;
const FOUR_LOOP_NEXT_CONDITIONS_SCHEMA: &str =
    "rustred-equal-mass-euclidean-four-loop-next-local-conditions-v1";

/// Denominators of all 22,424 projected source coefficients.
pub const FOUR_LOOP_NEXT_CONDITION_SOURCE_DENOMINATOR_OCCURRENCES: usize = 22_424;
/// Numerator and denominator of all 1,968 parent row scales.
pub const FOUR_LOOP_NEXT_CONDITION_PARENT_SCALE_PART_OCCURRENCES: usize = 3_936;
/// Numerator and denominator of all 1,588 exact pivot divisors.
pub const FOUR_LOOP_NEXT_CONDITION_PIVOT_DIVISOR_PART_OCCURRENCES: usize = 3_176;
/// Denominators of all 3,646 recursive trace factors.
pub const FOUR_LOOP_NEXT_CONDITION_TRACE_FACTOR_DENOMINATOR_OCCURRENCES: usize = 3_646;
/// Denominators of all 15,461 retained right-hand-side coefficients.
pub const FOUR_LOOP_NEXT_CONDITION_RULE_RHS_DENOMINATOR_OCCURRENCES: usize = 15_461;
/// Exact polynomial-part census for this local certificate.
pub const FOUR_LOOP_NEXT_CONDITION_OCCURRENCES: usize = 48_643;
/// Conditions introduced by source domains and actual inversions.
pub const FOUR_LOOP_NEXT_CONDITION_FUNDAMENTAL_OCCURRENCES: usize = 29_536;
/// Retained denominators which must factor through the fundamental set.
pub const FOUR_LOOP_NEXT_CONDITION_DERIVED_OCCURRENCES: usize = 19_107;

/// Polynomial representation used by the condition inventory.
pub type FourLoopNextConditionPolynomial = MultivariatePolynomial<IntegerRing, u16>;

/// Whether one portion of the exceptional-condition scope is complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextConditionCompleteness {
    Complete,
    Incomplete,
}

/// Explicit honesty boundary of the inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextConditionScope {
    fixed_projected_matrix: FourLoopNextConditionCompleteness,
    local_parent_normalization: FourLoopNextConditionCompleteness,
    upstream_native_ibp_pipeline: FourLoopNextConditionCompleteness,
    requires_nonzero_mass_scale: bool,
}

impl FourLoopNextConditionScope {
    pub const fn fixed_projected_matrix(self) -> FourLoopNextConditionCompleteness {
        self.fixed_projected_matrix
    }

    pub const fn local_parent_normalization(self) -> FourLoopNextConditionCompleteness {
        self.local_parent_normalization
    }

    pub const fn upstream_native_ibp_pipeline(self) -> FourLoopNextConditionCompleteness {
        self.upstream_native_ibp_pipeline
    }

    pub const fn requires_nonzero_mass_scale(self) -> bool {
        self.requires_nonzero_mass_scale
    }
}

pub const FOUR_LOOP_NEXT_CONDITION_SCOPE: FourLoopNextConditionScope =
    FourLoopNextConditionScope {
        fixed_projected_matrix: FourLoopNextConditionCompleteness::Complete,
        local_parent_normalization: FourLoopNextConditionCompleteness::Complete,
        upstream_native_ibp_pipeline: FourLoopNextConditionCompleteness::Incomplete,
        requires_nonzero_mass_scale: true,
    };

/// Algebraic role of one scanned polynomial part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FourLoopNextConditionRole {
    /// A rational coefficient must first be defined.
    FundamentalDomain,
    /// This polynomial is inverted by a normalization step.
    FundamentalInverse,
    /// A retained denominator derived by exact field arithmetic.
    DerivedRetainedUse,
}

/// Stable identifier assigned after sorting normalized factors structurally.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourLoopNextConditionFactorId(u32);

impl FourLoopNextConditionFactorId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Stable scan-order identifier for one polynomial-part occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourLoopNextConditionOccurrenceId(u32);

impl FourLoopNextConditionOccurrenceId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Typed provenance for each condition occurrence.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FourLoopNextConditionUse {
    ProjectedSourceDenominator {
        row_index: usize,
        raw_id: FourLoopNextRawRowId,
        column_index: usize,
        column: FourLoopCornerColumnId,
    },
    ParentRowNormalization {
        row_index: usize,
        raw_id: FourLoopNextRawRowId,
        part: CoefficientPolynomialPart,
    },
    ExactPivotNormalization {
        pivot_ordinal: usize,
        pivot_column_index: usize,
        pivot: FourLoopCornerColumnId,
        source_row_index: usize,
        raw_id: FourLoopNextRawRowId,
        part: CoefficientPolynomialPart,
    },
    TraceReductionFactorDenominator {
        pivot_ordinal: usize,
        pivot: FourLoopCornerColumnId,
        reduction_index: usize,
        prior_pivot_ordinal: usize,
        prior_pivot: FourLoopCornerColumnId,
    },
    RuleRhsDenominator {
        pivot_ordinal: usize,
        pivot: FourLoopCornerColumnId,
        column_index: usize,
        column: FourLoopCornerColumnId,
    },
}

impl FourLoopNextConditionUse {
    pub const fn role(&self) -> FourLoopNextConditionRole {
        match self {
            Self::ProjectedSourceDenominator { .. } => {
                FourLoopNextConditionRole::FundamentalDomain
            }
            Self::ParentRowNormalization { part, .. }
            | Self::ExactPivotNormalization { part, .. } => match part {
                CoefficientPolynomialPart::Numerator => {
                    FourLoopNextConditionRole::FundamentalInverse
                }
                CoefficientPolynomialPart::Denominator => {
                    FourLoopNextConditionRole::FundamentalDomain
                }
            },
            Self::TraceReductionFactorDenominator { .. }
            | Self::RuleRhsDenominator { .. } => {
                FourLoopNextConditionRole::DerivedRetainedUse
            }
        }
    }
}

/// One factor and multiplicity in an occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextConditionFactorMultiplicity {
    factor_id: FourLoopNextConditionFactorId,
    multiplicity: usize,
}

impl FourLoopNextConditionFactorMultiplicity {
    pub const fn factor_id(self) -> FourLoopNextConditionFactorId {
        self.factor_id
    }

    pub const fn multiplicity(self) -> usize {
        self.multiplicity
    }
}

/// One exact scanned polynomial and its typed use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextConditionOccurrence {
    id: FourLoopNextConditionOccurrenceId,
    use_site: FourLoopNextConditionUse,
    polynomial: FourLoopNextConditionPolynomial,
    integer_unit: Integer,
    factors: Vec<FourLoopNextConditionFactorMultiplicity>,
    application_count: usize,
}

impl FourLoopNextConditionOccurrence {
    pub const fn id(&self) -> FourLoopNextConditionOccurrenceId {
        self.id
    }

    pub const fn use_site(&self) -> &FourLoopNextConditionUse {
        &self.use_site
    }

    pub const fn role(&self) -> FourLoopNextConditionRole {
        self.use_site.role()
    }

    pub const fn polynomial(&self) -> &FourLoopNextConditionPolynomial {
        &self.polynomial
    }

    pub const fn integer_unit(&self) -> &Integer {
        &self.integer_unit
    }

    pub fn factors(&self) -> &[FourLoopNextConditionFactorMultiplicity] {
        &self.factors
    }

    /// Number of entries affected by the corresponding normalization/use.
    pub const fn application_count(&self) -> usize {
        self.application_count
    }
}

/// One occurrence at which a distinct normalized factor is required.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextConditionFactorUse {
    occurrence_id: FourLoopNextConditionOccurrenceId,
    multiplicity: usize,
}

impl FourLoopNextConditionFactorUse {
    pub const fn occurrence_id(self) -> FourLoopNextConditionOccurrenceId {
        self.occurrence_id
    }

    pub const fn multiplicity(self) -> usize {
        self.multiplicity
    }
}

/// One primitive, positive-leading, irreducible factor in `Z[d]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextExceptionalFactor {
    id: FourLoopNextConditionFactorId,
    polynomial: FourLoopNextConditionPolynomial,
    uses: Vec<FourLoopNextConditionFactorUse>,
}

impl FourLoopNextExceptionalFactor {
    pub const fn id(&self) -> FourLoopNextConditionFactorId {
        self.id
    }

    pub const fn polynomial(&self) -> &FourLoopNextConditionPolynomial {
        &self.polynomial
    }

    pub fn uses(&self) -> &[FourLoopNextConditionFactorUse] {
        &self.uses
    }
}

/// Independent, explicit resource envelopes for scanning and factoring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextConditionsConfig {
    pub max_occurrences: usize,
    pub max_scanned_polynomial_terms: usize,
    pub max_scanned_polynomial_bytes: usize,
    pub max_polynomial_terms: usize,
    pub max_factor_degree: usize,
    pub max_integer_bits: usize,
    pub max_distinct_factorization_inputs: usize,
    pub max_factorization_calls: usize,
    pub max_factors_per_input: usize,
    pub max_total_factor_multiplicity: usize,
    pub max_distinct_factors: usize,
    pub max_factor_uses: usize,
    pub max_uses_per_factor: usize,
    pub max_retained_factor_terms: usize,
    pub max_retained_factor_bytes: usize,
    pub max_reconstruction_pair_products: u128,
}

impl Default for FourLoopNextConditionsConfig {
    fn default() -> Self {
        Self {
            max_occurrences: FOUR_LOOP_NEXT_CONDITION_OCCURRENCES,
            max_scanned_polynomial_terms: 2_000_000,
            max_scanned_polynomial_bytes: 64 * 1024 * 1024,
            max_polynomial_terms: 65_536,
            max_factor_degree: u16::MAX as usize,
            max_integer_bits: 1_048_576,
            max_distinct_factorization_inputs: FOUR_LOOP_NEXT_CONDITION_OCCURRENCES,
            max_factorization_calls: FOUR_LOOP_NEXT_CONDITION_OCCURRENCES,
            max_factors_per_input: 65_536,
            max_total_factor_multiplicity: 2_000_000,
            max_distinct_factors: FOUR_LOOP_NEXT_CONDITION_OCCURRENCES,
            max_factor_uses: 2_000_000,
            max_uses_per_factor: FOUR_LOOP_NEXT_CONDITION_OCCURRENCES,
            max_retained_factor_terms: 2_000_000,
            max_retained_factor_bytes: 64 * 1024 * 1024,
            max_reconstruction_pair_products: 1_000_000_000,
        }
    }
}

/// Auditable counters retained in the certificate checksum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopNextConditionsStats {
    source_denominator_occurrences: usize,
    parent_scale_part_occurrences: usize,
    pivot_divisor_part_occurrences: usize,
    trace_factor_denominator_occurrences: usize,
    rule_rhs_denominator_occurrences: usize,
    fundamental_occurrences: usize,
    derived_occurrences: usize,
    nonconstant_occurrences: usize,
    scanned_polynomial_terms: usize,
    scanned_polynomial_bytes: usize,
    factorization_calls: usize,
    factorization_cache_hits: usize,
    total_factor_multiplicity: usize,
    distinct_factors: usize,
    factor_uses: usize,
    retained_factor_terms: usize,
    retained_factor_bytes: usize,
    reconstruction_pair_products: u128,
}

macro_rules! stats_getter {
    ($name:ident, $field:ident) => {
        pub const fn $name(self) -> usize {
            self.$field
        }
    };
}

impl FourLoopNextConditionsStats {
    stats_getter!(source_denominator_occurrences, source_denominator_occurrences);
    stats_getter!(parent_scale_part_occurrences, parent_scale_part_occurrences);
    stats_getter!(pivot_divisor_part_occurrences, pivot_divisor_part_occurrences);
    stats_getter!(trace_factor_denominator_occurrences, trace_factor_denominator_occurrences);
    stats_getter!(rule_rhs_denominator_occurrences, rule_rhs_denominator_occurrences);
    stats_getter!(fundamental_occurrences, fundamental_occurrences);
    stats_getter!(derived_occurrences, derived_occurrences);
    stats_getter!(nonconstant_occurrences, nonconstant_occurrences);
    stats_getter!(scanned_polynomial_terms, scanned_polynomial_terms);
    stats_getter!(scanned_polynomial_bytes, scanned_polynomial_bytes);
    stats_getter!(factorization_calls, factorization_calls);
    stats_getter!(factorization_cache_hits, factorization_cache_hits);
    stats_getter!(total_factor_multiplicity, total_factor_multiplicity);
    stats_getter!(distinct_factors, distinct_factors);
    stats_getter!(factor_uses, factor_uses);
    stats_getter!(retained_factor_terms, retained_factor_terms);
    stats_getter!(retained_factor_bytes, retained_factor_bytes);

    pub const fn reconstruction_pair_products(self) -> u128 {
        self.reconstruction_pair_products
    }
}

/// Result of evaluating the inventory at one exact rational dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FourLoopNextRationalSpecialization {
    /// No local inventory factor vanishes.  The fixed matrix retains rank
    /// 1,588 under the separately stated assumption `m2 != 0`.
    RegularInDimensionUnderNonzeroMassScale {
        dimension: Rational,
        certified_local_rank: usize,
        upstream_native_ibp_pipeline_complete: bool,
    },
    /// At least one inventory factor vanishes.  This generic certificate is
    /// unsupported there; this does not prove a genuine rank drop.
    UnsupportedByGenericCertificate {
        dimension: Rational,
        vanishing_factors: Vec<FourLoopNextConditionFactorId>,
    },
}

/// Typed failures from local condition construction and replay.
#[derive(Debug)]
pub enum FourLoopNextConditionsError {
    CertificateMismatch { component: &'static str },
    CoefficientProjection {
        row_index: usize,
        column_index: Option<usize>,
        source: CoefficientProjectionError,
    },
    ZeroPolynomial { occurrence: Option<usize> },
    MalformedPolynomial {
        occurrence: Option<usize>,
        reason: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    ArithmeticOverflow { resource: &'static str },
    FactorizationMismatch { occurrence: Option<usize> },
    ReducibleReturnedFactor { occurrence: Option<usize> },
    DerivedFactorNotFundamental { occurrence: usize },
    CensusMismatch {
        resource: &'static str,
        expected: usize,
        actual: usize,
    },
    SpecializationAuditMismatch { occurrence: usize },
    ReplayMismatch { component: &'static str },
    EliminationReplay(FourLoopNextEliminationError),
}

impl fmt::Display for FourLoopNextConditionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CertificateMismatch { component } => {
                write!(formatter, "four-loop condition source mismatch in {component}")
            }
            Self::CoefficientProjection {
                row_index,
                column_index,
                source,
            } => match column_index {
                Some(column_index) => write!(
                    formatter,
                    "could not project condition source row {row_index}, column {column_index}: {source}"
                ),
                None => write!(
                    formatter,
                    "could not project condition source row {row_index} scale: {source}"
                ),
            },
            Self::ZeroPolynomial { occurrence } => {
                write!(formatter, "condition occurrence {occurrence:?} is the zero polynomial")
            }
            Self::MalformedPolynomial { occurrence, reason } => write!(
                formatter,
                "condition occurrence {occurrence:?} has a malformed polynomial: {reason}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "condition {resource} requested {requested}, limit is {limit}"
            ),
            Self::ArithmeticOverflow { resource } => {
                write!(formatter, "arithmetic overflow while counting condition {resource}")
            }
            Self::FactorizationMismatch { occurrence } => write!(
                formatter,
                "Symbolica factorization did not exactly reconstruct condition occurrence {occurrence:?}"
            ),
            Self::ReducibleReturnedFactor { occurrence } => write!(
                formatter,
                "Symbolica returned a reducible condition factor at occurrence {occurrence:?}"
            ),
            Self::DerivedFactorNotFundamental { occurrence } => write!(
                formatter,
                "derived denominator occurrence {occurrence} contains a factor absent from the fundamental inversion/domain set"
            ),
            Self::CensusMismatch {
                resource,
                expected,
                actual,
            } => write!(
                formatter,
                "four-loop condition {resource} mismatch: expected {expected}, found {actual}"
            ),
            Self::SpecializationAuditMismatch { occurrence } => write!(
                formatter,
                "condition factor evaluation and source-polynomial evaluation disagree at occurrence {occurrence}"
            ),
            Self::ReplayMismatch { component } => {
                write!(formatter, "four-loop condition replay mismatch in {component}")
            }
            Self::EliminationReplay(error) => {
                write!(formatter, "four-loop elimination replay failed: {error}")
            }
        }
    }
}

impl Error for FourLoopNextConditionsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CoefficientProjection { source, .. } => Some(source),
            Self::EliminationReplay(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FourLoopNextEliminationError> for FourLoopNextConditionsError {
    fn from(error: FourLoopNextEliminationError) -> Self {
        Self::EliminationReplay(error)
    }
}
