//! Finite *starting* index domains, independent of reduction/search limits.
//!
//! Physical power counting supplies these budgets. This module does not infer
//! a gauge, a forest prescription, or physics coverage from a loop count.
mod compositions;
mod plan;
pub use plan::entry_domain_plan;
#[cfg(test)]
mod tests;

use crate::AppError;
use rustred::family::IntegralKey;
use serde::{Deserialize, Serialize};
use symbolica::domains::integer::Integer;

use compositions::Compositions;

/// Bounds on A=sum positive powers and R=sum absolute negative powers.
/// Optional bounds on A-R retain correlations from dimensional counting.
/// These bounds apply ONLY to starting integrals, never to their descendants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryPowerBudget {
    pub max_positive_power: u32,
    pub max_numerator_rank: u32,
    #[serde(default)]
    pub min_power_difference: Option<i32>,
    #[serde(default)]
    pub max_power_difference: Option<i32>,
}

impl EntryPowerBudget {
    /// Conservative initial envelope for a dimensionless marginal two-,
    /// three- or four-point coefficient in Feynman gauge.
    ///
    /// Caller guarantees: ordinary dimension-four renormalizable polynomial
    /// vertices; canonical propagators with tree two-point insertions resummed;
    /// conventional non-oversubtracted proper UV forests without vacuum or
    /// one-point nodes, with vertex-disjoint maximal children and positive-loop
    /// contracted quotients; polynomial scalarization; and no stripped inverse-mass
    /// factors. This constructor does NOT authenticate a graph or model.
    /// L counts still-unintegrated loops (including all factorized components),
    /// not the perturbative order after lower-loop counterterms were integrated.
    ///
    /// At L loops, the forest has at most L positive-loop quotient nodes.
    /// Counting their Taylor derivatives gives A<=5L-1 and R<=3L-1; engineering
    /// dimension with nonnegative explicit mass degree gives A-R>=2L.
    /// These are conservative input bounds, not source/descendant limits.
    pub fn renormalizable_marginal_feynman(loops: u32) -> Result<Self, AppError> {
        if loops == 0 {
            return Err(AppError::input(
                "renormalizable entry profile requires positive loop count",
            ));
        }
        let max_positive_power = loops
            .checked_mul(5)
            .and_then(|x| x.checked_sub(1))
            .ok_or_else(|| AppError::limit("positive-power profile overflow"))?;
        let max_numerator_rank = loops
            .checked_mul(3)
            .and_then(|x| x.checked_sub(1))
            .ok_or_else(|| AppError::limit("numerator profile overflow"))?;
        let min_power_difference = loops
            .checked_mul(2)
            .and_then(|x| i32::try_from(x).ok())
            .ok_or_else(|| AppError::limit("power-difference profile overflow"))?;
        Ok(Self {
            max_positive_power,
            max_numerator_rank,
            min_power_difference: Some(min_power_difference),
            max_power_difference: None,
        })
    }
}

/// A single support with a finite, correlated starting-index envelope.
#[derive(Clone, Debug)]
pub struct FiniteEntryDomain {
    support: Vec<bool>,
    budget: EntryPowerBudget,
    active: usize,
}

impl FiniteEntryDomain {
    pub fn new(support: Vec<bool>, budget: EntryPowerBudget) -> Result<Self, AppError> {
        if support.is_empty() || support.len() > u32::MAX as usize {
            return Err(AppError::input("entry support must have 1..=u32::MAX axes"));
        }
        if let (Some(lower), Some(upper)) =
            (budget.min_power_difference, budget.max_power_difference)
            && lower > upper
        {
            return Err(AppError::input("inverted entry power-difference interval"));
        }
        for bound in [budget.max_positive_power, budget.max_numerator_rank] {
            (bound as usize)
                .checked_add(support.len())
                .ok_or_else(|| AppError::limit("entry composition index space overflows usize"))?;
        }
        let active = support.iter().filter(|&&x| x).count();
        Ok(Self {
            support,
            budget,
            active,
        })
    }

    pub fn support(&self) -> &[bool] {
        &self.support
    }
    pub fn budget(&self) -> EntryPowerBudget {
        self.budget
    }

    /// Independent exact membership; deliberately does not inspect any owner
    /// scope or intermediate/search policy.
    pub fn contains(&self, target: &IntegralKey) -> bool {
        if target.powers().len() != self.support.len() {
            return false;
        }
        let mut positive = 0u64;
        let mut rank = 0u64;
        for (&power, &active) in target.powers().iter().zip(&self.support) {
            if (power > 0) != active {
                return false;
            }
            if active {
                let Some(next) = positive.checked_add(power as u64) else {
                    return false;
                };
                positive = next;
            } else {
                let Some(next) = rank.checked_add(power.unsigned_abs()) else {
                    return false;
                };
                rank = next;
            }
            if positive > u64::from(self.budget.max_positive_power)
                || rank > u64::from(self.budget.max_numerator_rank)
            {
                return false;
            }
        }
        let difference = positive as i64 - rank as i64;
        self.budget
            .min_power_difference
            .is_none_or(|x| difference >= i64::from(x))
            && self
                .budget
                .max_power_difference
                .is_none_or(|x| difference <= i64::from(x))
    }

    fn positive_totals(&self) -> std::ops::RangeInclusive<u32> {
        let rank_cap = if self.active == self.support.len() {
            0
        } else {
            i64::from(self.budget.max_numerator_rank)
        };
        let mut minimum = self.active as i64;
        let mut maximum = if self.active == 0 {
            0
        } else {
            i64::from(self.budget.max_positive_power)
        };
        // Existentially eliminate R before iterating shells. An empty domain
        // or a high exact A-R band must not scan billions of impossible layers
        // inside a single uninterruptible Iterator::next().
        if let Some(difference) = self.budget.min_power_difference {
            minimum = minimum.max(i64::from(difference));
        }
        if let Some(difference) = self.budget.max_power_difference {
            maximum = maximum.min(rank_cap + i64::from(difference));
        }
        if minimum > maximum {
            return 1..=0;
        }
        minimum as u32..=maximum as u32
    }

    fn ranks(&self, positive: u32) -> std::ops::RangeInclusive<u32> {
        let minimum = self
            .budget
            .max_power_difference
            .map_or(0, |x| (i64::from(positive) - i64::from(x)).max(0));
        let maximum = self
            .budget
            .min_power_difference
            .map_or(i64::from(self.budget.max_numerator_rank), |x| {
                (i64::from(positive) - i64::from(x)).min(i64::from(self.budget.max_numerator_rank))
            });
        let maximum = if self.active == self.support.len() {
            maximum.min(0)
        } else {
            maximum
        };
        if maximum < minimum {
            return 1..=0;
        }
        minimum as u32..=maximum as u32
    }

    /// Exact total without enumerating individual keys. `max_positive_layers`
    /// is an explicit preflight-work allowance, not a coverage cutoff: refusal
    /// returns an error, never a truncated count. Symbolica owns binomials and
    /// arbitrary-precision integer arithmetic.
    pub fn target_count(&self, max_positive_layers: usize) -> Result<Integer, AppError> {
        let mut count = Integer::from(0);
        let inactive = self.support.len() - self.active;
        for (layer, positive) in self.positive_totals().enumerate() {
            if layer >= max_positive_layers {
                return Err(AppError::limit(
                    "entry count exceeds positive-layer allowance",
                ));
            }
            let ranks = self.ranks(positive);
            if ranks.is_empty() {
                continue;
            }
            let positive_count = if self.active == 0 {
                Integer::from(1)
            } else {
                Integer::binom(i64::from(positive) - 1, self.active as i64 - 1)
            };
            let rank_count = if inactive == 0 {
                Integer::from(1)
            } else {
                let cumulative = |r: i64| {
                    if r < 0 {
                        Integer::from(0)
                    } else {
                        Integer::binom(r + inactive as i64, inactive as i64)
                    }
                };
                cumulative(i64::from(*ranks.end())) - cumulative(i64::from(*ranks.start()) - 1)
            };
            count += positive_count * rank_count;
        }
        Ok(count)
    }

    /// Deterministic A/R shells followed by Symbolica's composition order.
    /// The iterator retains O(arity) state, not the full target collection.
    /// Consumers must batch/stream admission rather than eagerly collect a
    /// large envelope before starting the shared scheduler.
    pub fn targets(&self) -> impl Iterator<Item = Result<IntegralKey, AppError>> + '_ {
        self.positive_totals().flat_map(move |positive| {
            self.ranks(positive).flat_map(move |rank| {
                Compositions::new(self.active, positive - self.active as u32).flat_map(
                    move |dots| {
                        Compositions::new(self.support.len() - self.active, rank).map(
                            move |numerators| {
                                let mut active = dots.iter();
                                let mut inactive = numerators.iter();
                                IntegralKey::try_new(self.support.iter().map(|&present| {
                                    if present {
                                        1 + i64::from(*active.next().expect("active composition"))
                                    } else {
                                        -i64::from(*inactive.next().expect("inactive composition"))
                                    }
                                }))
                                .map_err(|e| AppError::limit(e.to_string()))
                            },
                        )
                    },
                )
            })
        })
    }
}
