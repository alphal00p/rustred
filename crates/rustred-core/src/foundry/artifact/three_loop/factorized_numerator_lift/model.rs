use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::foundry::artifact::FactorizationRule;
use crate::foundry::artifact::factorized_numerator_lift::RoutedAffineDenominator;
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits};

use super::error::ProbeError;
use super::limits::ProbeLimits;
use super::{ARITY, LOOP_COUNT};

#[derive(Clone, Debug)]
pub(super) struct CornerAngularForm {
    pub(super) constant: Coefficient,
    pub(super) cross_coefficients: [Coefficient; 3],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct AffineMomentKey {
    pub(super) remaining_powers: [u64; ARITY],
    pub(super) cross_powers: [u64; 3],
}

pub(super) struct CornerMomentEvaluator<'a> {
    pub(super) context: &'a CoefficientContext,
    pub(super) forms: [CornerAngularForm; ARITY],
    pub(super) denominator_priority: CoordinatePriority,
    pub(super) vector_priority: CoordinatePriority,
    pub(super) affine_cache: BTreeMap<AffineMomentKey, Coefficient>,
    pub(super) angular_cache: BTreeMap<[u64; 3], Coefficient>,
    pub(super) affine_transition_count: usize,
    pub(super) angular_transition_count: usize,
    /// Ranks whose generic-domain denominator `d+r-2` was used. A future
    /// production owner must persist these as explicit exceptional guards;
    /// retaining the census here does not discharge that obligation.
    pub(super) angular_guard_ranks: BTreeSet<u64>,
    pub(super) limits: ProbeLimits,
}

impl<'a> CornerMomentEvaluator<'a> {
    pub(super) fn try_new(
        family: &'a crate::family::IntegralFamily,
        rule: &FactorizationRule,
        transformed: &[RoutedAffineDenominator],
        rank_by_denominator: &[usize; ARITY],
        limits: ProbeLimits,
    ) -> Result<Self, ProbeError> {
        let limits = limits.validate()?;
        validate_k1_cubed_corner(rule)?;
        if transformed.len() != ARITY {
            return Err(ProbeError::WrongTransformedFormCount {
                expected: ARITY,
                actual: transformed.len(),
            });
        }
        let denominator_priority = CoordinatePriority::try_new(
            ARITY,
            rank_by_denominator,
            CoordinatePriorityLimits::default(),
        )?;
        let stable = denominator_priority.try_stable_id(CoordinatePriorityLimits::default())?;
        let replayed =
            CoordinatePriority::try_from_stable_id(&stable, CoordinatePriorityLimits::default())?;
        if replayed != denominator_priority {
            return Err(ProbeError::Invariant {
                detail: "the persisted denominator priority did not replay",
            });
        }
        let forms: [CornerAngularForm; ARITY] = transformed
            .iter()
            .map(|form| super::derive::corner_angular_form(family, form))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| ProbeError::Invariant {
                detail: "the admitted transformed forms lost their arity",
            })?;
        Ok(Self {
            context: family.coefficient_context(),
            forms,
            denominator_priority,
            vector_priority: CoordinatePriority::try_natural(
                LOOP_COUNT,
                CoordinatePriorityLimits::default(),
            )?,
            affine_cache: BTreeMap::new(),
            angular_cache: BTreeMap::new(),
            affine_transition_count: 0,
            angular_transition_count: 0,
            angular_guard_ranks: BTreeSet::new(),
            limits,
        })
    }
}

/// Admit only a product of three one-coordinate, one-loop factors whose
/// parent positions exactly cover the factorization sector. This structural
/// check prevents the K3 x K1 rule from entering the `q_i^2 = 1` spherical
/// fixture; it does not confer production authority on the accepted rule.
fn validate_k1_cubed_corner(rule: &FactorizationRule) -> Result<(), ProbeError> {
    if rule.factors().len() != LOOP_COUNT {
        return Err(ProbeError::UnsupportedCornerFactorization {
            detail: "expected exactly three independent factors",
        });
    }
    let active = rule.application_domain().sector().active_bits();
    if active.len() != ARITY || active.iter().filter(|&&entry| entry).count() != LOOP_COUNT {
        return Err(ProbeError::UnsupportedCornerFactorization {
            detail: "expected exactly three active parent denominators",
        });
    }
    let mut parent_positions = BTreeSet::new();
    let mut transformed_loops = BTreeSet::new();
    for factor in rule.factors() {
        if factor.parent_positions().len() != 1 || factor.transformed_loop_positions().len() != 1 {
            return Err(ProbeError::UnsupportedCornerFactorization {
                detail: "every factor must own one denominator and one transformed loop",
            });
        }
        let parent = factor.parent_positions()[0];
        let transformed = factor.transformed_loop_positions()[0];
        if parent >= ARITY || !active[parent] || transformed >= LOOP_COUNT {
            return Err(ProbeError::UnsupportedCornerFactorization {
                detail: "factor positions do not belong to the admitted parent/loop coordinates",
            });
        }
        parent_positions.insert(parent);
        transformed_loops.insert(transformed);
    }
    if parent_positions.len() != LOOP_COUNT || transformed_loops.len() != LOOP_COUNT {
        return Err(ProbeError::UnsupportedCornerFactorization {
            detail: "factor positions are not disjoint complete covers",
        });
    }
    Ok(())
}
