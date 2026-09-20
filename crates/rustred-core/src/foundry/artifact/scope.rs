//! Exact public-entry admission, separate from descendant proof envelopes.
//!
//! No production bounded constructor exists yet. Final actual-cell coverage
//! and successor checks must seal that path before a bounded owner can ship.

use std::collections::BTreeMap;

use crate::sector::Mask;

use super::source_port::scope::EntryScope;
use super::{ArtifactError, ClosedArtifact};

#[derive(Debug)]
pub(super) enum ArtifactProofScope {
    Unrestricted,
    #[allow(dead_code)] // Only test fixtures construct this until final admission exists.
    TotalExcess(TotalExcessProofScope),
}

#[derive(Debug)]
pub(super) struct TotalExcessProofScope {
    entry: EntryScope,
    // This immutable map is intentionally not the public-entry bound. Runtime
    // descendants rely on the eventual checked envelope, never on entry D.
    #[allow(dead_code)] // Consumed by the subsequent actual-cell/persistence bridge.
    successor_degrees: BTreeMap<Mask, u64>,
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
    /// The supplied successor map is NOT a closure proof and cannot be encoded.
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
