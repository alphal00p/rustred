//! Deterministic automatic completion of an independent propagator set by
//! irreducible scalar products (ISPs).
//!
//! LiteRed's `NewDsBasis[..., Append -> True]` first checks that the supplied
//! denominator rows are independent and then applies its private
//! `append[m, IdentityMatrix[Length[sps]]]`: scalar-product unit rows are
//! scanned from left to right and retained exactly when they increase the row
//! rank.  This module implements that algorithm over RustRed's authenticated
//! Symbolica rational-function field using RustRed's documented coordinate
//! order.  Mathematica's `Union` may order the scalar-product expressions
//! differently, so the completed basis can be equivalent without having the
//! same ISP ordinals as one LiteRed session.
//!
//! The result retains the accepted coordinate ordinals and every generic rank
//! in a replayable transcript.  Generated ISP denominators are the scalar
//! products themselves (zero affine constant and one unit coefficient), and
//! their power shifts are exactly zero.  This is the independent-basis path;
//! dependent or overcomplete input belongs to the future partial-fraction
//! denominator-set layer.

use std::borrow::Cow;
use std::fmt::{self, Write as _};

use crate::{
    AffineDenominator, Coefficient, CoefficientContext, ExactAlgebraError, IntegralFamily,
    IntegralFamilyLimits, ScalarProductCoordinate,
};

/// Stable semantic schema for deterministic ISP completion proofs.
pub const AUTOMATIC_ISP_COMPLETION_V1_SCHEMA: &str = "rustred-automatic-isp-completion-v1";

/// Resource policy for the generic-rank completion pass and final family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutomaticIspCompletionLimits {
    pub family: IntegralFamilyLimits,
    /// Largest transient rectangular matrix admitted by one rank test.
    pub max_rank_matrix_entries: usize,
    /// Aggregate numerator-plus-denominator term count admitted in one
    /// borrowed rank matrix before it is cloned for elimination.
    pub max_rank_coefficient_terms: usize,
    /// Aggregate canonical-display bytes admitted in one borrowed rank matrix
    /// before it is cloned for elimination.
    pub max_rank_coefficient_bytes: usize,
    /// Maximum number of exact additions, subtractions, multiplications, and
    /// divisions performed by one construction or replay pass.
    pub max_rank_operations: usize,
    /// Maximum number of initial/candidate rank tests.
    pub max_rank_tests: usize,
}

impl Default for AutomaticIspCompletionLimits {
    fn default() -> Self {
        Self {
            family: IntegralFamilyLimits::default(),
            max_rank_matrix_entries: 16_000_000,
            max_rank_coefficient_terms: 64_000_000,
            max_rank_coefficient_bytes: 2 * 1024 * 1024 * 1024,
            max_rank_operations: 64_000_000,
            max_rank_tests: 65_536,
        }
    }
}

/// Work census retained with a completed family.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutomaticIspCompletionStats {
    rank_tests: usize,
    rank_operations: usize,
    appended_isps: usize,
}

impl AutomaticIspCompletionStats {
    pub const fn rank_tests(self) -> usize {
        self.rank_tests
    }

    pub const fn rank_operations(self) -> usize {
        self.rank_operations
    }

    pub const fn appended_isps(self) -> usize {
        self.appended_isps
    }
}

/// A complete family plus the exact deterministic ISP-completion witness.
#[derive(Clone, Debug)]
pub struct AutomaticIspCompletion {
    family: IntegralFamily,
    input_denominator_count: usize,
    appended_coordinate_ordinals: Box<[usize]>,
    rank_progression: Box<[usize]>,
    limits: AutomaticIspCompletionLimits,
    stats: AutomaticIspCompletionStats,
}

impl AutomaticIspCompletion {
    /// Complete an independent, possibly short denominator list with the
    /// default checked resource policy.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new<'name>(
        name: impl Into<Cow<'name, str>>,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        coefficients: CoefficientContext,
        dimension: Coefficient,
        denominators: Vec<AffineDenominator>,
        external_gram: Vec<Vec<Coefficient>>,
        power_shifts: Vec<Coefficient>,
    ) -> Result<Self, AutomaticIspCompletionError> {
        Self::try_new_with_limits(
            name,
            loop_momenta,
            external_momenta,
            coefficients,
            dimension,
            denominators,
            external_gram,
            power_shifts,
            AutomaticIspCompletionLimits::default(),
        )
    }

    /// Complete an independent denominator list under explicit rank and
    /// family-construction budgets.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new_with_limits<'name>(
        name: impl Into<Cow<'name, str>>,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        coefficients: CoefficientContext,
        dimension: Coefficient,
        mut denominators: Vec<AffineDenominator>,
        external_gram: Vec<Vec<Coefficient>>,
        mut power_shifts: Vec<Coefficient>,
        limits: AutomaticIspCompletionLimits,
    ) -> Result<Self, AutomaticIspCompletionError> {
        let loops = loop_momenta.len();
        if loops == 0 {
            return Err(AutomaticIspCompletionError::NoLoopMomenta);
        }
        let scalar_products = checked_scalar_product_count(loops, external_momenta.len())?;
        check_limit(
            "family scalar products",
            scalar_products,
            limits.family.max_scalar_products,
        )?;
        if denominators.is_empty() {
            return Err(AutomaticIspCompletionError::NoInputDenominators);
        }
        if denominators.len() > scalar_products {
            return Err(AutomaticIspCompletionError::TooManyInputDenominators {
                maximum: scalar_products,
                actual: denominators.len(),
            });
        }
        if power_shifts.len() != denominators.len() {
            return Err(AutomaticIspCompletionError::WrongInputPowerShiftCount {
                expected: denominators.len(),
                actual: power_shifts.len(),
            });
        }
        authenticate_input_rows(&coefficients, &denominators, scalar_products, limits.family)?;

        // `checked_row_rank` preflights its own work matrix, but assembling
        // `rows` below already clones the complete supplied matrix. Bound that
        // allocation before it occurs.
        preflight_rank_matrix(
            denominators.len(),
            scalar_products,
            limits.max_rank_matrix_entries,
        )?;
        preflight_rank_coefficients(
            denominators
                .iter()
                .flat_map(|denominator| denominator.coefficients()),
            limits,
        )?;

        let input_denominator_count = denominators.len();
        let mut rows = denominators
            .iter()
            .map(|denominator| denominator.coefficients().to_vec())
            .collect::<Vec<_>>();
        let mut budget = RankBudget::new(limits);
        let input_rank = checked_row_rank(&coefficients, &rows, &mut budget)?;
        if input_rank != input_denominator_count {
            return Err(AutomaticIspCompletionError::DependentInputDenominators {
                denominators: input_denominator_count,
                generic_rank: input_rank,
            });
        }

        let mut appended_coordinate_ordinals =
            Vec::with_capacity(scalar_products.saturating_sub(input_denominator_count));
        let mut rank_progression = vec![input_rank];
        let mut rank = input_rank;
        for coordinate in 0..scalar_products {
            if rank == scalar_products {
                break;
            }
            let candidate_rows = rows.len().checked_add(1).ok_or(
                AutomaticIspCompletionError::ResourceCountOverflow {
                    resource: "automatic ISP rank matrix rows",
                },
            )?;
            preflight_rank_matrix(
                candidate_rows,
                scalar_products,
                limits.max_rank_matrix_entries,
            )?;
            let zero = coefficients.zero();
            let one = coefficients.one();
            preflight_rank_coefficients(
                rows.iter().flatten().chain(
                    (0..scalar_products)
                        .map(|candidate| if candidate == coordinate { &one } else { &zero }),
                ),
                limits,
            )?;
            let mut candidate = vec![zero; scalar_products];
            candidate[coordinate] = one;
            rows.push(candidate.clone());
            let candidate_rank = checked_row_rank(&coefficients, &rows, &mut budget)?;
            if candidate_rank == rank + 1 {
                appended_coordinate_ordinals.push(coordinate);
                rank = candidate_rank;
                rank_progression.push(rank);
                denominators.push(AffineDenominator::new(coefficients.zero(), candidate));
                power_shifts.push(coefficients.zero());
            } else if candidate_rank == rank {
                rows.pop();
            } else {
                return Err(AutomaticIspCompletionError::InternalVerificationFailure {
                    detail: format!(
                        "appending scalar-product unit row {coordinate} changed rank from {rank} to {candidate_rank}"
                    ),
                });
            }
        }
        if rank != scalar_products || denominators.len() != scalar_products {
            return Err(AutomaticIspCompletionError::InternalVerificationFailure {
                detail: format!("canonical unit rows stopped at rank {rank} of {scalar_products}"),
            });
        }
        debug_assert_eq!(
            appended_coordinate_ordinals.len(),
            scalar_products - input_denominator_count
        );

        let family = IntegralFamily::new_with_limits(
            name,
            loop_momenta,
            external_momenta,
            coefficients,
            dimension,
            denominators,
            external_gram,
            power_shifts,
            limits.family,
        )?;
        let stats = AutomaticIspCompletionStats {
            rank_tests: budget.tests,
            rank_operations: budget.operations,
            appended_isps: appended_coordinate_ordinals.len(),
        };
        let completion = Self {
            family,
            input_denominator_count,
            appended_coordinate_ordinals: appended_coordinate_ordinals.into_boxed_slice(),
            rank_progression: rank_progression.into_boxed_slice(),
            limits,
            stats,
        };
        completion.replay()?;
        Ok(completion)
    }

    pub const fn schema(&self) -> &'static str {
        AUTOMATIC_ISP_COMPLETION_V1_SCHEMA
    }

    pub fn family(&self) -> &IntegralFamily {
        &self.family
    }

    pub fn into_family(self) -> IntegralFamily {
        self.family
    }

    pub const fn input_denominator_count(&self) -> usize {
        self.input_denominator_count
    }

    pub fn appended_coordinate_ordinals(&self) -> &[usize] {
        &self.appended_coordinate_ordinals
    }

    pub fn appended_coordinates(&self) -> impl Iterator<Item = ScalarProductCoordinate> + '_ {
        self.appended_coordinate_ordinals
            .iter()
            .map(|&ordinal| self.family.coordinates()[ordinal])
    }

    /// Initial generic rank followed by the rank after every accepted ISP.
    pub fn rank_progression(&self) -> &[usize] {
        &self.rank_progression
    }

    pub const fn limits(&self) -> AutomaticIspCompletionLimits {
        self.limits
    }

    pub const fn stats(&self) -> AutomaticIspCompletionStats {
        self.stats
    }

    /// Recompute LiteRed's rank-increasing identity-row algorithm, scanning
    /// RustRed's persisted coordinate order, from the retained input prefix;
    /// compare the full family, rank transcript, and zero shifts.
    pub fn replay(&self) -> Result<(), AutomaticIspCompletionError> {
        self.family.verify_exact_replay()?;
        let scalar_products = self.family.coordinates().len();
        if self.input_denominator_count == 0
            || self.input_denominator_count > scalar_products
            || self.family.denominators().len() != scalar_products
            || self.family.power_shifts().len() != scalar_products
        {
            return Err(AutomaticIspCompletionError::InternalVerificationFailure {
                detail: "retained ISP completion dimensions differ".to_owned(),
            });
        }
        let context = self.family.coefficient_context();
        preflight_rank_matrix(
            self.input_denominator_count,
            scalar_products,
            self.limits.max_rank_matrix_entries,
        )?;
        preflight_rank_coefficients(
            self.family.denominators()[..self.input_denominator_count]
                .iter()
                .flat_map(|denominator| denominator.coefficients()),
            self.limits,
        )?;
        let mut rows = self.family.denominators()[..self.input_denominator_count]
            .iter()
            .map(|denominator| denominator.coefficients().to_vec())
            .collect::<Vec<_>>();
        let mut budget = RankBudget::new(self.limits);
        let mut rank = checked_row_rank(context, &rows, &mut budget)?;
        if rank != self.input_denominator_count {
            return Err(AutomaticIspCompletionError::InternalVerificationFailure {
                detail: "retained input denominator prefix is dependent".to_owned(),
            });
        }
        let mut replayed_ordinals = Vec::new();
        let mut replayed_ranks = vec![rank];
        for coordinate in 0..scalar_products {
            if rank == scalar_products {
                break;
            }
            let candidate_rows = rows.len().checked_add(1).ok_or(
                AutomaticIspCompletionError::ResourceCountOverflow {
                    resource: "automatic ISP rank matrix rows",
                },
            )?;
            preflight_rank_matrix(
                candidate_rows,
                scalar_products,
                self.limits.max_rank_matrix_entries,
            )?;
            let zero = context.zero();
            let one = context.one();
            preflight_rank_coefficients(
                rows.iter().flatten().chain(
                    (0..scalar_products)
                        .map(|candidate| if candidate == coordinate { &one } else { &zero }),
                ),
                self.limits,
            )?;
            let mut candidate = vec![zero; scalar_products];
            candidate[coordinate] = one;
            rows.push(candidate.clone());
            let candidate_rank = checked_row_rank(context, &rows, &mut budget)?;
            if candidate_rank == rank + 1 {
                let denominator_position = self.input_denominator_count + replayed_ordinals.len();
                let denominator = &self.family.denominators()[denominator_position];
                if !denominator.constant().is_zero()
                    || denominator.coefficients() != candidate
                    || !self.family.power_shifts()[denominator_position].is_zero()
                {
                    return Err(AutomaticIspCompletionError::InternalVerificationFailure {
                        detail: format!(
                            "retained generated ISP {denominator_position} is not canonical coordinate {coordinate} with zero shift"
                        ),
                    });
                }
                replayed_ordinals.push(coordinate);
                rank = candidate_rank;
                replayed_ranks.push(rank);
            } else if candidate_rank == rank {
                rows.pop();
            } else {
                return Err(AutomaticIspCompletionError::InternalVerificationFailure {
                    detail: "rank replay changed by more than one".to_owned(),
                });
            }
        }
        let replayed_stats = AutomaticIspCompletionStats {
            rank_tests: budget.tests,
            rank_operations: budget.operations,
            appended_isps: replayed_ordinals.len(),
        };
        if replayed_ordinals.as_slice() != self.appended_coordinate_ordinals.as_ref()
            || replayed_ranks.as_slice() != self.rank_progression.as_ref()
            || self.stats != replayed_stats
        {
            return Err(AutomaticIspCompletionError::InternalVerificationFailure {
                detail: "retained ISP completion transcript differs on replay".to_owned(),
            });
        }
        Ok(())
    }
}

/// Typed failures from exact automatic ISP completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutomaticIspCompletionError {
    NoLoopMomenta,
    NoInputDenominators,
    ScalarProductCountOverflow {
        loops: usize,
        externals: usize,
    },
    TooManyInputDenominators {
        maximum: usize,
        actual: usize,
    },
    WrongInputPowerShiftCount {
        expected: usize,
        actual: usize,
    },
    WrongDenominatorRowSize {
        denominator: usize,
        expected: usize,
        actual: usize,
    },
    InvalidInputCoefficient {
        denominator: usize,
        coordinate: Option<usize>,
        error: ExactAlgebraError,
    },
    DependentInputDenominators {
        denominators: usize,
        generic_rank: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ExactAlgebra(ExactAlgebraError),
    Family(crate::GenericFamilyError),
    InternalVerificationFailure {
        detail: String,
    },
}

impl fmt::Display for AutomaticIspCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLoopMomenta => {
                formatter.write_str("automatic ISP completion needs at least one loop momentum")
            }
            Self::NoInputDenominators => {
                formatter.write_str("automatic ISP completion needs at least one input denominator")
            }
            Self::ScalarProductCountOverflow { loops, externals } => write!(
                formatter,
                "the scalar-product count for {loops} loops and {externals} external momenta overflowed usize"
            ),
            Self::TooManyInputDenominators { maximum, actual } => write!(
                formatter,
                "an independent basis has at most {maximum} input denominators, received {actual}"
            ),
            Self::WrongInputPowerShiftCount { expected, actual } => write!(
                formatter,
                "received {actual} input power shifts for {expected} supplied denominators"
            ),
            Self::WrongDenominatorRowSize {
                denominator,
                expected,
                actual,
            } => write!(
                formatter,
                "input denominator {denominator} has {actual} scalar-product coefficients, expected {expected}"
            ),
            Self::InvalidInputCoefficient {
                denominator,
                coordinate,
                error,
            } => match coordinate {
                Some(coordinate) => write!(
                    formatter,
                    "invalid coefficient {coordinate} of input denominator {denominator}: {error}"
                ),
                None => write!(
                    formatter,
                    "invalid constant of input denominator {denominator}: {error}"
                ),
            },
            Self::DependentInputDenominators {
                denominators,
                generic_rank,
            } => write!(
                formatter,
                "the {denominators} supplied denominators have generic rank {generic_rank}; dependent sets require partial fractioning"
            ),
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
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
            Self::InternalVerificationFailure { detail } => {
                write!(
                    formatter,
                    "automatic ISP completion replay failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for AutomaticIspCompletionError {}

impl From<ExactAlgebraError> for AutomaticIspCompletionError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<crate::GenericFamilyError> for AutomaticIspCompletionError {
    fn from(value: crate::GenericFamilyError) -> Self {
        Self::Family(value)
    }
}

fn checked_scalar_product_count(
    loops: usize,
    externals: usize,
) -> Result<usize, AutomaticIspCompletionError> {
    // Divide the even factor before multiplying. `L*(L+1)` can overflow even
    // when the triangular scalar-product count itself is representable.
    let successor = loops
        .checked_add(1)
        .ok_or(AutomaticIspCompletionError::ScalarProductCountOverflow { loops, externals })?;
    let (left, right) = if loops % 2 == 0 {
        (loops / 2, successor)
    } else {
        (loops, successor / 2)
    };
    let loop_loop = left
        .checked_mul(right)
        .ok_or(AutomaticIspCompletionError::ScalarProductCountOverflow { loops, externals })?;
    loop_loop
        .checked_add(
            loops.checked_mul(externals).ok_or(
                AutomaticIspCompletionError::ScalarProductCountOverflow { loops, externals },
            )?,
        )
        .ok_or(AutomaticIspCompletionError::ScalarProductCountOverflow { loops, externals })
}

fn preflight_rank_matrix(
    rows: usize,
    columns: usize,
    limit: usize,
) -> Result<(), AutomaticIspCompletionError> {
    let entries =
        rows.checked_mul(columns)
            .ok_or(AutomaticIspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank matrix entries",
            })?;
    check_limit("automatic ISP rank matrix entries", entries, limit)
}

fn authenticate_input_rows(
    context: &CoefficientContext,
    denominators: &[AffineDenominator],
    scalar_products: usize,
    family_limits: IntegralFamilyLimits,
) -> Result<(), AutomaticIspCompletionError> {
    for (denominator, affine) in denominators.iter().enumerate() {
        if affine.coefficients().len() != scalar_products {
            return Err(AutomaticIspCompletionError::WrongDenominatorRowSize {
                denominator,
                expected: scalar_products,
                actual: affine.coefficients().len(),
            });
        }
        context
            .validate_with_limits(affine.constant(), family_limits.exact_algebra)
            .map_err(
                |error| AutomaticIspCompletionError::InvalidInputCoefficient {
                    denominator,
                    coordinate: None,
                    error,
                },
            )?;
        for (coordinate, coefficient) in affine.coefficients().iter().enumerate() {
            context
                .validate_with_limits(coefficient, family_limits.exact_algebra)
                .map_err(
                    |error| AutomaticIspCompletionError::InvalidInputCoefficient {
                        denominator,
                        coordinate: Some(coordinate),
                        error,
                    },
                )?;
        }
    }
    Ok(())
}

struct RankBudget {
    limits: AutomaticIspCompletionLimits,
    tests: usize,
    operations: usize,
}

impl RankBudget {
    const fn new(limits: AutomaticIspCompletionLimits) -> Self {
        Self {
            limits,
            tests: 0,
            operations: 0,
        }
    }

    fn start_test(
        &mut self,
        matrix: &[Vec<Coefficient>],
    ) -> Result<(), AutomaticIspCompletionError> {
        self.tests = self.tests.checked_add(1).ok_or(
            AutomaticIspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank tests",
            },
        )?;
        check_limit(
            "automatic ISP rank tests",
            self.tests,
            self.limits.max_rank_tests,
        )?;
        let columns = matrix.first().map_or(0, Vec::len);
        preflight_rank_matrix(matrix.len(), columns, self.limits.max_rank_matrix_entries)?;
        preflight_rank_coefficients(matrix.iter().flatten(), self.limits)
    }

    fn operation(&mut self) -> Result<(), AutomaticIspCompletionError> {
        self.operations = self.operations.checked_add(1).ok_or(
            AutomaticIspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank operations",
            },
        )?;
        check_limit(
            "automatic ISP rank operations",
            self.operations,
            self.limits.max_rank_operations,
        )
    }
}

fn checked_row_rank(
    context: &CoefficientContext,
    matrix: &[Vec<Coefficient>],
    budget: &mut RankBudget,
) -> Result<usize, AutomaticIspCompletionError> {
    let columns = matrix.first().map_or(0, Vec::len);
    if matrix.iter().any(|row| row.len() != columns) {
        return Err(AutomaticIspCompletionError::InternalVerificationFailure {
            detail: "rank matrix is not rectangular".to_owned(),
        });
    }
    // Census the borrowed input before `to_vec` duplicates every Symbolica
    // coefficient for the destructive elimination workspace.
    budget.start_test(matrix)?;
    let mut work = matrix.to_vec();
    let mut pivot_row = 0;
    for column in 0..columns {
        let Some(found) = (pivot_row..work.len()).find(|&row| !work[row][column].is_zero()) else {
            continue;
        };
        work.swap(pivot_row, found);
        let pivot = work[pivot_row][column].clone();
        for entry in column..columns {
            budget.operation()?;
            work[pivot_row][entry] = context.try_div(
                &work[pivot_row][entry],
                &pivot,
                budget.limits.family.exact_algebra,
            )?;
        }
        if pivot_row + 1 < work.len() {
            // The normalized row coexists with `work` during elimination.
            // Bound the complete row payload before cloning it.
            preflight_rank_coefficients(work[pivot_row].iter(), budget.limits)?;
            let normalized = work[pivot_row].clone();
            for row in pivot_row + 1..work.len() {
                let factor = work[row][column].clone();
                if factor.is_zero() {
                    continue;
                }
                for entry in column..columns {
                    budget.operation()?;
                    let contribution = context.try_mul(
                        &factor,
                        &normalized[entry],
                        budget.limits.family.exact_algebra,
                    )?;
                    budget.operation()?;
                    work[row][entry] = context.try_sub(
                        &work[row][entry],
                        &contribution,
                        budget.limits.family.exact_algebra,
                    )?;
                }
            }
        }
        pivot_row += 1;
        if pivot_row == work.len() {
            break;
        }
    }
    Ok(pivot_row)
}

fn preflight_rank_coefficients<'coefficient>(
    coefficients: impl IntoIterator<Item = &'coefficient Coefficient>,
    limits: AutomaticIspCompletionLimits,
) -> Result<(), AutomaticIspCompletionError> {
    let mut coefficient_terms = 0usize;
    let mut coefficient_bytes = 0usize;
    for coefficient in coefficients {
        let terms = coefficient
            .numerator
            .nterms()
            .checked_add(coefficient.denominator.nterms())
            .ok_or(AutomaticIspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank coefficient terms",
            })?;
        coefficient_terms = coefficient_terms.checked_add(terms).ok_or(
            AutomaticIspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank coefficient terms",
            },
        )?;
        check_limit(
            "automatic ISP rank coefficient terms",
            coefficient_terms,
            limits.max_rank_coefficient_terms,
        )?;
        coefficient_bytes = checked_coefficient_display_bytes(
            coefficient_bytes,
            coefficient,
            limits.max_rank_coefficient_bytes,
        )?;
    }
    Ok(())
}

fn checked_coefficient_display_bytes(
    retained: usize,
    coefficient: &Coefficient,
    limit: usize,
) -> Result<usize, AutomaticIspCompletionError> {
    let remaining = limit.saturating_sub(retained);
    let mut writer = BoundedByteCounter {
        bytes: 0,
        limit: remaining,
    };
    if write!(&mut writer, "{coefficient}").is_err() {
        return Err(AutomaticIspCompletionError::ResourceLimit {
            resource: "automatic ISP rank coefficient bytes",
            requested: limit.saturating_add(1),
            limit,
        });
    }
    let requested = retained.checked_add(writer.bytes).ok_or(
        AutomaticIspCompletionError::ResourceCountOverflow {
            resource: "automatic ISP rank coefficient bytes",
        },
    )?;
    check_limit("automatic ISP rank coefficient bytes", requested, limit)?;
    Ok(requested)
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
}

impl fmt::Write for BoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > self.limit {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), AutomaticIspCompletionError> {
    if requested > limit {
        Err(AutomaticIspCompletionError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod coefficient_limit_tests {
    use super::*;

    fn one_loop_completion(
        coefficient: Coefficient,
        limits: AutomaticIspCompletionLimits,
    ) -> Result<AutomaticIspCompletion, AutomaticIspCompletionError> {
        let context = CoefficientContext::new(["d"]);
        AutomaticIspCompletion::try_new_with_limits(
            "automatic-isp-coefficient-census",
            vec!["k".to_owned()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(context.zero(), vec![coefficient])],
            Vec::new(),
            vec![context.zero()],
            limits,
        )
    }

    #[test]
    fn coefficient_term_census_accepts_exact_boundary_and_rejects_one_below() {
        let context = CoefficientContext::new(["d"]);
        // Every rational polynomial owns a numerator and a denominator term.
        let exact = context.one().numerator.nterms() + context.one().denominator.nterms();
        assert_eq!(exact, 2);

        let mut limits = AutomaticIspCompletionLimits::default();
        limits.max_rank_coefficient_terms = exact;
        one_loop_completion(context.one(), limits).unwrap();

        limits.max_rank_coefficient_terms = exact - 1;
        assert!(matches!(
            one_loop_completion(context.one(), limits),
            Err(AutomaticIspCompletionError::ResourceLimit {
                resource: "automatic ISP rank coefficient terms",
                requested,
                limit,
            }) if requested == exact && limit == exact - 1
        ));
    }

    #[test]
    fn coefficient_byte_census_bounds_a_large_integer_before_rank_clone() {
        let context = CoefficientContext::new(["d"]);
        let coefficient = context
            .parse("123456789012345678901234567890123456789")
            .unwrap();
        let exact = coefficient.to_string().len();
        assert!(exact > 32);

        let mut limits = AutomaticIspCompletionLimits::default();
        limits.max_rank_coefficient_bytes = exact;
        one_loop_completion(coefficient.clone(), limits).unwrap();

        limits.max_rank_coefficient_bytes = exact - 1;
        assert!(matches!(
            one_loop_completion(coefficient, limits),
            Err(AutomaticIspCompletionError::ResourceLimit {
                resource: "automatic ISP rank coefficient bytes",
                requested,
                limit,
            }) if requested > limit && limit == exact - 1
        ));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn triangular_count_avoids_a_false_intermediate_product_overflow() {
        let loops = 6_000_000_000usize;
        assert_eq!(
            checked_scalar_product_count(loops, 0).unwrap(),
            18_000_000_003_000_000_000usize
        );
    }

    #[test]
    fn replay_rejects_ordinal_rank_and_input_prefix_transcript_tampering() {
        let context = CoefficientContext::new(["d", "s"]);
        let make = || {
            AutomaticIspCompletion::try_new(
                "automatic-isp-transcript-tamper",
                vec!["k".to_owned()],
                vec!["p".to_owned()],
                context.clone(),
                context.parameter("d").unwrap(),
                vec![AffineDenominator::new(
                    context.zero(),
                    vec![context.one(), context.zero()],
                )],
                vec![vec![context.parameter("s").unwrap()]],
                vec![context.zero()],
            )
            .unwrap()
        };

        let mut ordinal = make();
        ordinal.appended_coordinate_ordinals[0] = 0;
        assert!(matches!(
            ordinal.replay(),
            Err(AutomaticIspCompletionError::InternalVerificationFailure { .. })
        ));

        let mut ranks = make();
        ranks.rank_progression[1] = 1;
        assert!(matches!(
            ranks.replay(),
            Err(AutomaticIspCompletionError::InternalVerificationFailure { .. })
        ));

        let mut prefix = make();
        prefix.input_denominator_count = 2;
        assert!(matches!(
            prefix.replay(),
            Err(AutomaticIspCompletionError::InternalVerificationFailure { .. })
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_binds_the_complete_rank_work_census() {
        let context = CoefficientContext::new(["d", "m"]);
        let mut completion = AutomaticIspCompletion::try_new(
            "automatic-isp-stats-tamper",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.parse("-m").unwrap(),
                vec![context.one()],
            )],
            Vec::new(),
            vec![context.zero()],
        )
        .unwrap();
        completion.stats.rank_tests += 1;
        assert!(matches!(
            completion.replay(),
            Err(AutomaticIspCompletionError::InternalVerificationFailure { .. })
        ));

        let mut completion = AutomaticIspCompletion::try_new(
            "automatic-isp-operation-stats-tamper",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.parse("-m").unwrap(),
                vec![context.one()],
            )],
            Vec::new(),
            vec![context.zero()],
        )
        .unwrap();
        completion.stats.rank_operations += 1;
        assert!(matches!(
            completion.replay(),
            Err(AutomaticIspCompletionError::InternalVerificationFailure { .. })
        ));
    }
}
