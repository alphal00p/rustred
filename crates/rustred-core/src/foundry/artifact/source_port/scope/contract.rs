//! Requested entry predicates, distinct from a proposed inductive envelope.
//!
//! These objects carry no source replay, rule, guard, descent or coverage
//! authority. In particular, constructing one cannot produce a ClosedArtifact.

mod envelope;
pub(in crate::foundry::artifact) use envelope::ProposedProofEnvelope;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::family::IntegralFamily;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::completion::LatticeBox;
use crate::sector::Mask;

/// Exact degree convention for starting integer indices, not tensor rank.
/// A finite terminal basis need not be minimal. Neither bound asserts that
/// reduction preserves this degree: successors need an independent envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryDegreeBound {
    /// `sum_i max(-n_i, 0) <= D`. Positive propagator powers are unbounded.
    MaxNegativeIndexDegree(u64),
    /// `sum_i (max(n_i - 1, 0) + max(-n_i, 0)) <= D`.
    /// This bounds dots as well as negative indices. It is a separate, finite
    /// entry set, not an alternative spelling of numerator-rank certification.
    MaxTotalExcessDegree(u64),
}

impl EntryDegreeBound {
    pub fn limit(self) -> u64 {
        match self {
            Self::MaxNegativeIndexDegree(limit) | Self::MaxTotalExcessDegree(limit) => limit,
        }
    }

    fn counts_axis(self, active: bool) -> bool {
        matches!(self, Self::MaxTotalExcessDegree(_)) || !active
    }

    /// Exact intersection test for a local rectangular cell, independent of
    /// root admission. The lower corner minimizes either nonnegative degree.
    pub(in crate::foundry::artifact) fn intersects_local_box(
        self,
        sector: &[bool],
        cell: &LatticeBox,
    ) -> Result<bool, ArtifactError> {
        if sector.len() != cell.arity() {
            return Err(ArtifactError::WrongArity {
                expected: sector.len(),
                actual: cell.arity(),
            });
        }
        let mut degree = 0_u128;
        for (&active, &lower) in sector.iter().zip(cell.lower()) {
            if self.counts_axis(active) {
                degree = degree
                    .checked_add(u128::from(lower))
                    .ok_or_else(degree_overflow)?;
            }
        }
        Ok(degree <= u128::from(self.limit()))
    }
}

/// Immutable family-bound starting domain, including every subsector of root.
///
/// The condition is on the integer labels themselves. Interpreting twice the
/// negative degree as a momentum-numerator degree additionally requires an
/// admitted quadratic, unshifted vacuum family. No such interpretation is
/// inferred by this generic scope contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryScope {
    family_fingerprint: Arc<String>,
    root: Mask,
    bound: EntryDegreeBound,
}

impl EntryScope {
    pub fn try_new(
        family: &IntegralFamily,
        root: &Mask,
        bound: EntryDegreeBound,
    ) -> Result<Self, ArtifactError> {
        if root.arity() != family.denominator_count() {
            return Err(ArtifactError::WrongArity {
                expected: family.denominator_count(),
                actual: root.arity(),
            });
        }
        Ok(Self {
            family_fingerprint: family.fingerprint_owner(),
            root: root.clone(),
            bound,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub fn root(&self) -> &Mask {
        &self.root
    }

    pub fn bound(&self) -> EntryDegreeBound {
        self.bound
    }

    /// Bind once when installing or cold-loading an owner. Exact per-entry
    /// membership does not repeatedly rebuild or hash a family fingerprint.
    pub fn validate_binding(
        &self,
        family: &IntegralFamily,
        root: &Mask,
    ) -> Result<(), ArtifactError> {
        if self.family_fingerprint() != family.fingerprint() {
            return Err(ArtifactError::WrongFamily);
        }
        if &self.root != root {
            return Err(invalid("entry scope is bound to a different root sector"));
        }
        Ok(())
    }

    /// Exact entry admission, suitable for enforcement before a cache lookup.
    /// It does not apply to descendants unless their own proof envelope says so.
    pub fn contains(&self, powers: &[i64]) -> Result<bool, ArtifactError> {
        match self.bound {
            EntryDegreeBound::MaxNegativeIndexDegree(limit) => {
                super::rank::entry_contains(self.root.active_bits(), powers, limit)
            }
            EntryDegreeBound::MaxTotalExcessDegree(limit) => {
                self.check_arity(powers.len())?;
                let mut degree = 0_u128;
                for (&allowed_positive, &power) in self.root.active_bits().iter().zip(powers) {
                    if power > 0 && !allowed_positive {
                        return Ok(false);
                    }
                    let local = if power > 0 {
                        (power - 1) as u64
                    } else {
                        power.unsigned_abs()
                    };
                    degree = degree
                        .checked_add(u128::from(local))
                        .ok_or_else(degree_overflow)?;
                }
                Ok(degree <= u128::from(limit))
            }
        }
    }

    /// Whether any mathematical integer in this local rectangular box meets
    /// the entry bound. The lower corner realizes the minimum degree exactly;
    /// unbounded positive rays are never replaced by machine-sized endpoints.
    pub(in crate::foundry::artifact) fn intersects_local_box(
        &self,
        sector: &[bool],
        cell: &LatticeBox,
    ) -> Result<bool, ArtifactError> {
        self.validate_sector(sector)?;
        self.check_arity(cell.arity())?;
        self.bound.intersects_local_box(sector, cell)
    }

    pub(in crate::foundry::artifact) fn validate_sector(
        &self,
        sector: &[bool],
    ) -> Result<(), ArtifactError> {
        self.check_arity(sector.len())?;
        if sector
            .iter()
            .zip(self.root.active_bits())
            .any(|(&active, &allowed)| active && !allowed)
        {
            return Err(invalid("proof-envelope sector lies outside the entry root"));
        }
        Ok(())
    }

    fn check_arity(&self, actual: usize) -> Result<(), ArtifactError> {
        if actual != self.root.arity() {
            return Err(ArtifactError::WrongArity {
                expected: self.root.arity(),
                actual,
            });
        }
        Ok(())
    }
}

fn invalid(detail: &'static str) -> ArtifactError {
    ArtifactError::InvalidRuleShape { detail }
}

fn degree_overflow() -> ArtifactError {
    ArtifactError::ResourceCountOverflow {
        resource: "entry-scope degree",
    }
}
