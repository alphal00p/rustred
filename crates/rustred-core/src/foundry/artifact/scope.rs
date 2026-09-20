//! Exact public-entry admission, separate from descendant proof envelopes.
//!
//! Internal bounded owners require actual-cell coverage and successor checks.
//! Durable bounded owners replay the same exact cell and successor obligations.

use std::collections::BTreeMap;

use crate::sector::Mask;

use super::source_port::scope::EntryScope;
use super::{ArtifactError, ClosedArtifact};

#[derive(Debug)]
pub(super) enum ArtifactProofScope {
    Unrestricted,
    TotalExcess(TotalExcessProofScope),
}

/// Read-only entry domain and successor envelopes of a sealed bounded owner.
/// Constructed only after exact source, cell-cover and successor checks.
#[derive(Debug)]
pub struct TotalExcessProofScope {
    entry: EntryScope,
    // This immutable map is intentionally not the public-entry bound. Runtime
    // descendants rely on the checked envelope, never on entry D.
    successor_degrees: BTreeMap<Mask, u64>,
}

impl TotalExcessProofScope {
    /// Bound on starting roots: dots and negative powers both contribute.
    pub fn max_entry_total_excess_degree(&self) -> u64 {
        self.entry.bound().limit()
    }

    /// Sector component of the entry domain: all subsectors are included,
    /// but their starting powers must still satisfy the entry degree bound.
    pub fn root_sector(&self) -> &Mask {
        self.entry.root()
    }

    /// Immutable proved bounds for nonzero descendants, not entry limits.
    pub fn successor_degrees(&self) -> &BTreeMap<Mask, u64> {
        &self.successor_degrees
    }
}

/// Shared boundary errors mapped into each existing public frontend error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RootDomainError {
    WrongArity {
        expected: usize,
        actual: usize,
    },
    OutsideBounds {
        position: usize,
        value: i64,
        lower: i64,
        upper: i64,
    },
    OutsideTotalExcess {
        maximum: u64,
    },
    Scope(ArtifactError),
}

impl ArtifactProofScope {
    pub(super) fn from_verified_total_excess(
        verified: super::install::VerifiedTotalExcessScope,
    ) -> Self {
        let (entry, successor_degrees) = verified.into_parts();
        Self::TotalExcess(TotalExcessProofScope {
            entry,
            successor_degrees,
        })
    }
    pub(super) fn is_unrestricted(&self) -> bool {
        matches!(self, Self::Unrestricted)
    }

    fn admit_entry(&self, powers: &[i64]) -> Result<(), RootDomainError> {
        match self {
            Self::Unrestricted => Ok(()),
            Self::TotalExcess(scope) => {
                if scope
                    .entry
                    .contains(powers)
                    .map_err(RootDomainError::Scope)?
                {
                    Ok(())
                } else {
                    Err(RootDomainError::OutsideTotalExcess {
                        maximum: scope.entry.bound().limit(),
                    })
                }
            }
        }
    }
}

impl ClosedArtifact {
    /// Inspect a sealed total-excess promise. `None` means no extra degree
    /// bound; the declared rectangular root domain still applies. The map
    /// cannot be mutated into a new authority.
    pub fn total_excess_scope(&self) -> Option<&TotalExcessProofScope> {
        match &self.proof_scope {
            ArtifactProofScope::Unrestricted => None,
            ArtifactProofScope::TotalExcess(scope) => Some(scope),
        }
    }

    /// Validate starting powers before canonicalization, cache lookup, or any
    /// reducer mutation. The rectangular carrier retains its old semantics and
    /// error priority; exact total excess is an additional entry-only check.
    pub(crate) fn validate_root_powers(&self, powers: &[i64]) -> Result<(), RootDomainError> {
        if powers.len() != self.arity() {
            return Err(RootDomainError::WrongArity {
                expected: self.arity(),
                actual: powers.len(),
            });
        }
        for (position, (&value, bounds)) in powers
            .iter()
            .zip(self.supported_root_power_bounds())
            .enumerate()
        {
            if !bounds.contains(value) {
                return Err(RootDomainError::OutsideBounds {
                    position,
                    value,
                    lower: bounds.lower(),
                    upper: bounds.upper(),
                });
            }
        }
        self.proof_scope.admit_entry(powers)
    }

    /// Test-only admission fixture over already verified unrestricted rules.
    /// The supplied successor map is NOT a closure proof. Production callers
    /// cannot access this helper; cold replay still validates its full claim.
    #[cfg(test)]
    pub(crate) fn with_total_excess_scope_for_test(
        mut self,
        entry_degree: u64,
        successor_degrees: BTreeMap<Mask, u64>,
    ) -> Result<Self, ArtifactError> {
        use super::source_port::scope::{EntryDegreeBound, root_from_bounds, sector_count};
        let root = root_from_bounds(self.supported_root_power_bounds(), self.arity())?;
        let entry = EntryScope::try_new(
            self.family(),
            &root,
            EntryDegreeBound::MaxTotalExcessDegree(entry_degree),
        )?;
        let mut scoped_zeros = std::collections::BTreeSet::new();
        for zero in self.zero_sectors() {
            if zero.sector().is_subsector_of(&root)? {
                scoped_zeros.insert(zero.sector().clone());
            }
        }
        for (sector, &degree) in &successor_degrees {
            entry.validate_sector(sector.active_bits())?;
            if scoped_zeros.contains(sector) {
                return Err(ArtifactError::InvalidZeroTerminal);
            }
            if degree < entry_degree {
                return Err(ArtifactError::InvalidRuleShape {
                    detail: "test successor degree does not contain entry degree",
                });
            }
        }
        if successor_degrees.len().checked_add(scoped_zeros.len()) != Some(sector_count(&root)?) {
            return Err(ArtifactError::UnsupportedClosureShape);
        }
        self.proof_scope = ArtifactProofScope::TotalExcess(TotalExcessProofScope {
            entry,
            successor_degrees,
        });
        Ok(self)
    }
}

#[cfg(test)]
mod tests;
