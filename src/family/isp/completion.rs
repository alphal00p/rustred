//! Deterministic completion of an independent propagator set by
//! irreducible scalar products (ISPs).
//!
//! LiteRed's `NewDsBasis[..., Append -> True]` first checks that the supplied
//! denominator rows are independent and then applies its private
//! `append[m, IdentityMatrix[Length[sps]]]`: scalar-product unit rows are
//! scanned from left to right and retained exactly when they increase the row
//! rank.  This module implements that algorithm using Symbolica's native exact
//! matrix rank over RustRed's authenticated rational-function field and
//! RustRed's documented coordinate order.  Mathematica's `Union` may order the
//! scalar-product expressions differently, so the completed basis can be
//! equivalent without having the same ISP ordinals as one LiteRed session.
//!
//! The result retains the accepted coordinate ordinals and every generic rank
//! in a deterministic witness. Generated ISP denominators are the scalar
//! products themselves (zero affine constant and one unit coefficient), and
//! their power shifts are exactly zero.  This is the independent-basis path;
//! dependent or overcomplete input belongs to the future partial-fraction
//! denominator-set layer.

use std::borrow::Cow;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily, ScalarProductCoordinate};

use super::error::IspCompletionError;
use super::model::{ISP_COMPLETION_V2_SCHEMA, IspCompletionLimits, IspCompletionStats};
use super::rank::{
    RankBudget, authenticate_input_rows, check_limit, checked_row_rank,
    checked_scalar_product_count, preflight_rank_coefficients, preflight_rank_matrix,
};

/// A complete family plus the exact deterministic ISP-completion witness.
#[derive(Clone, Debug)]
pub struct IspCompletion {
    family: IntegralFamily,
    input_denominator_count: usize,
    appended_coordinate_ordinals: Box<[usize]>,
    rank_progression: Box<[usize]>,
    limits: IspCompletionLimits,
    stats: IspCompletionStats,
}

impl IspCompletion {
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
    ) -> Result<Self, IspCompletionError> {
        Self::try_new_with_limits(
            name,
            loop_momenta,
            external_momenta,
            coefficients,
            dimension,
            denominators,
            external_gram,
            power_shifts,
            IspCompletionLimits::default(),
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
        limits: IspCompletionLimits,
    ) -> Result<Self, IspCompletionError> {
        let loops = loop_momenta.len();
        if loops == 0 {
            return Err(IspCompletionError::NoLoopMomenta);
        }
        let scalar_products = checked_scalar_product_count(loops, external_momenta.len())?;
        check_limit(
            "family scalar products",
            scalar_products,
            limits.family.max_scalar_products,
        )?;
        if denominators.is_empty() {
            return Err(IspCompletionError::NoInputDenominators);
        }
        if denominators.len() > scalar_products {
            return Err(IspCompletionError::TooManyInputDenominators {
                maximum: scalar_products,
                actual: denominators.len(),
            });
        }
        if power_shifts.len() != denominators.len() {
            return Err(IspCompletionError::WrongInputPowerShiftCount {
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
            return Err(IspCompletionError::DependentInputDenominators {
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
            let candidate_rows =
                rows.len()
                    .checked_add(1)
                    .ok_or(IspCompletionError::ResourceCountOverflow {
                        resource: "automatic ISP rank matrix rows",
                    })?;
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
                return Err(IspCompletionError::InternalVerificationFailure {
                    detail: format!(
                        "appending scalar-product unit row {coordinate} changed rank from {rank} to {candidate_rank}"
                    ),
                });
            }
        }
        if rank != scalar_products || denominators.len() != scalar_products {
            return Err(IspCompletionError::InternalVerificationFailure {
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
        let stats = IspCompletionStats {
            rank_tests: budget.tests,
            rank_operations: budget.operations,
            appended_isps: appended_coordinate_ordinals.len(),
        };
        Ok(Self {
            family,
            input_denominator_count,
            appended_coordinate_ordinals: appended_coordinate_ordinals.into_boxed_slice(),
            rank_progression: rank_progression.into_boxed_slice(),
            limits,
            stats,
        })
    }

    pub const fn schema(&self) -> &'static str {
        ISP_COMPLETION_V2_SCHEMA
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

    pub const fn limits(&self) -> IspCompletionLimits {
        self.limits
    }

    pub const fn stats(&self) -> IspCompletionStats {
        self.stats
    }
}
