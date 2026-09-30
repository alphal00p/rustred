//! Finite discovery schedules, never an integral order or proof of descent.
//!
//! Plans retain original basis ordinals. Persistence owners must bind them to
//! their deterministic source/preconditioning recipe and sector configuration;
//! a permutation or feature inventory is not an authenticated basis digest.

use super::{PolynomialRow, SolverError};

/// A complete finite permutation. Construction validates before solver work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceVisitOrder(Vec<usize>);

/// A finite sector-job permutation, independent of canonical output ordinals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorVisitOrder(Vec<usize>);

impl SectorVisitOrder {
    pub fn new(ordinals: Vec<usize>, sector_count: usize) -> Result<Self, SolverError> {
        validate_visit_order(&ordinals, sector_count)?;
        Ok(Self(ordinals))
    }

    pub fn ordinals(&self) -> &[usize] {
        &self.0
    }

    /// Only this finite preparation is generic over the closure/key. The
    /// Rayon/GPLU engine consumes a common materialized plan representation.
    pub fn by_key<const N: usize, K: Ord>(
        sectors: &[[bool; N]],
        mut key: impl FnMut(usize, &[bool; N]) -> K,
    ) -> Self {
        let mut keyed: Vec<_> = sectors
            .iter()
            .enumerate()
            .map(|(i, sector)| (key(i, sector), i))
            .collect();
        keyed.sort_unstable();
        Self(keyed.into_iter().map(|(_, ordinal)| ordinal).collect())
    }
}

impl SourceVisitOrder {
    pub fn new(ordinals: Vec<usize>, row_count: usize) -> Result<Self, SolverError> {
        validate_visit_order(&ordinals, row_count)?;
        Ok(Self(ordinals))
    }

    pub fn ordinals(&self) -> &[usize] {
        &self.0
    }

    /// Materialize an opaque callback exactly once per original row. Only the
    /// finite result is needed subsequently; equal keys use original ordinals.
    pub fn by_key<const N: usize, K: Ord>(
        basis: &[PolynomialRow<N>],
        mut key: impl FnMut(usize, &SourceRowFeatures<N>) -> K,
    ) -> Result<Self, SolverError> {
        let mut keyed = basis
            .iter()
            .enumerate()
            .map(|(ordinal, row)| {
                SourceRowFeatures::read(row).map(|features| (key(ordinal, &features), ordinal))
            })
            .collect::<Result<Vec<_>, _>>()?;
        keyed.sort_unstable();
        Ok(Self(
            keyed.into_iter().map(|(_, ordinal)| ordinal).collect(),
        ))
    }
}

pub(super) fn validate_visit_order(order: &[usize], row_count: usize) -> Result<(), SolverError> {
    if order.len() != row_count {
        return Err(invalid(
            "source visit order must contain every stored basis row exactly once",
        ));
    }
    let mut seen = vec![false; row_count];
    for &ordinal in order {
        let visited = seen
            .get_mut(ordinal)
            .ok_or_else(|| invalid("source visit order contains an out-of-range basis row"))?;
        if std::mem::replace(visited, true) {
            return Err(invalid("source visit order contains a repeated basis row"));
        }
    }
    Ok(())
}

/// Cheap immutable features: no evaluation, factoring, GCD, or string conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRowFeatures<const N: usize> {
    pub terms: u64,
    pub coefficient_monomials: u64,
    /// Sums over symbolic coordinates of all terms, not a maximum or a clipped
    /// search domain. Fixed numeric powers (including removed cuts) are not
    /// displacements and contribute zero.
    pub positive_shifts: [u64; N],
    pub negative_shifts: [u64; N],
}

impl<const N: usize> SourceRowFeatures<N> {
    pub fn read(row: &PolynomialRow<N>) -> Result<Self, SolverError> {
        let mut result = Self {
            terms: u64::try_from(row.len()).map_err(|_| invalid("source term count overflow"))?,
            coefficient_monomials: 0,
            positive_shifts: [0; N],
            negative_shifts: [0; N],
        };
        for term in row {
            result.coefficient_monomials = result
                .coefficient_monomials
                .checked_add(
                    u64::try_from(term.coefficient.nterms())
                        .map_err(|_| invalid("coefficient term count overflow"))?,
                )
                .ok_or_else(|| invalid("coefficient term count overflow"))?;
            for (axis, power) in term.integral.powers().iter().enumerate() {
                if !power.is_symbolic() {
                    continue;
                }
                let value = i32::from(power.value());
                let (totals, amount) = if value >= 0 {
                    (&mut result.positive_shifts, value as u64)
                } else {
                    (&mut result.negative_shifts, (-value) as u64)
                };
                totals[axis] = totals[axis]
                    .checked_add(amount)
                    .ok_or_else(|| invalid("source shift feature overflow"))?;
            }
        }
        Ok(result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceRowFeature {
    Terms,
    CoefficientMonomials,
    AbsoluteShifts(Vec<u32>),
    PositiveShifts(Vec<u32>),
    NegativeShifts(Vec<u32>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRowPriority {
    pub feature: SourceRowFeature,
    pub descending: bool,
}

/// A runtime recipe over a finite row inventory. It does not change integral
/// comparison, seed enumeration, exact replay, or source IDs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceDiscoveryStrategy {
    /// No feature extraction, key allocation, or per-row callback.
    #[default]
    InputOrder,
    Features(Vec<SourceRowPriority>),
    Materialized(SourceVisitOrder),
}

impl SourceDiscoveryStrategy {
    pub const MAX_PRIORITIES: usize = 8;
    pub const MAX_WEIGHT: u32 = 1_000_000;

    pub fn validate(&self, arity: usize) -> Result<(), SolverError> {
        if let Self::Features(priorities) = self {
            if priorities.is_empty() || priorities.len() > Self::MAX_PRIORITIES {
                return Err(invalid(
                    "source strategy requires one through eight priorities",
                ));
            }
            for priority in priorities {
                match &priority.feature {
                    SourceRowFeature::Terms | SourceRowFeature::CoefficientMonomials => (),
                    SourceRowFeature::AbsoluteShifts(weights)
                    | SourceRowFeature::PositiveShifts(weights)
                    | SourceRowFeature::NegativeShifts(weights) => {
                        if weights.len() != arity
                            || weights.iter().all(|&w| w == 0)
                            || weights.iter().any(|&w| w > Self::MAX_WEIGHT)
                        {
                            return Err(invalid(
                                "source strategy weights require exact arity, a nonzero weight, and weights at most 1000000",
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn materialize<const N: usize>(
        &self,
        basis: &[PolynomialRow<N>],
    ) -> Result<Option<SourceVisitOrder>, SolverError> {
        self.validate(N)?;
        match self {
            Self::InputOrder => Ok(None),
            Self::Materialized(plan) => {
                validate_visit_order(plan.ordinals(), basis.len())?;
                Ok(Some(plan.clone()))
            }
            Self::Features(priorities) => {
                let mut keyed = Vec::with_capacity(basis.len());
                for (ordinal, row) in basis.iter().enumerate() {
                    let features = SourceRowFeatures::read(row)?;
                    let mut scores = [0u128; Self::MAX_PRIORITIES];
                    for (score, priority) in scores.iter_mut().zip(priorities) {
                        *score = priority.score(&features)?;
                        if priority.descending {
                            *score = u128::MAX - *score;
                        }
                    }
                    keyed.push((scores, ordinal));
                }
                keyed.sort_unstable();
                Ok(Some(SourceVisitOrder(
                    keyed.into_iter().map(|(_, ordinal)| ordinal).collect(),
                )))
            }
        }
    }
}

impl SourceRowPriority {
    fn score<const N: usize>(&self, features: &SourceRowFeatures<N>) -> Result<u128, SolverError> {
        let weights = match &self.feature {
            SourceRowFeature::Terms => return Ok(features.terms.into()),
            SourceRowFeature::CoefficientMonomials => {
                return Ok(features.coefficient_monomials.into());
            }
            SourceRowFeature::AbsoluteShifts(w)
            | SourceRowFeature::PositiveShifts(w)
            | SourceRowFeature::NegativeShifts(w) => w,
        };
        let mut score = 0u128;
        for (axis, &weight) in weights.iter().enumerate() {
            let value = match &self.feature {
                SourceRowFeature::PositiveShifts(_) => u128::from(features.positive_shifts[axis]),
                SourceRowFeature::NegativeShifts(_) => u128::from(features.negative_shifts[axis]),
                _ => {
                    u128::from(features.positive_shifts[axis])
                        + u128::from(features.negative_shifts[axis])
                }
            };
            score = score
                .checked_add(
                    value
                        .checked_mul(weight.into())
                        .ok_or_else(|| invalid("source priority overflow"))?,
                )
                .ok_or_else(|| invalid("source priority overflow"))?;
        }
        Ok(score)
    }
}

fn invalid(message: &str) -> SolverError {
    SolverError::InvalidInput(message.into())
}

#[cfg(test)]
mod tests;
