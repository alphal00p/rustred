use super::super::super::{TerminalAliasError, TerminalAliasStatistics};
use super::super::{VacuumParametricLimits, VerifiedVacuumParameterMap};
use crate::family::{IntegralFamily, IntegralKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

/// A coordinate key remains owned by its exact family; equal index vectors in
/// different families are never identified without a verified parameter map.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VacuumIntegralKey {
    pub(super) family: Arc<String>,
    pub(super) key: IntegralKey,
}
impl VacuumIntegralKey {
    pub(crate) fn from_family(family: &IntegralFamily, key: IntegralKey) -> Self {
        Self {
            family: family.fingerprint_owner(),
            key,
        }
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family
    }
    pub fn integral(&self) -> &IntegralKey {
        &self.key
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VacuumFamilyAliasLimits {
    pub max_families: usize,
    pub max_terminals: usize,
    /// Support and canonicalization counts apply to the entire collection.
    pub parametric: VacuumParametricLimits,
}
impl Default for VacuumFamilyAliasLimits {
    fn default() -> Self {
        Self {
            max_families: 1024,
            max_terminals: 1_000_000,
            parametric: Default::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VacuumFamilyAliasError {
    Alias(TerminalAliasError),
    DuplicateFamily,
    IncompatibleFamilies,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}
impl From<TerminalAliasError> for VacuumFamilyAliasError {
    fn from(error: TerminalAliasError) -> Self {
        Self::Alias(error)
    }
}
impl fmt::Display for VacuumFamilyAliasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(error) => error.fmt(f),
            Self::DuplicateFamily => {
                f.write_str("duplicate family inventory in vacuum alias collection")
            }
            Self::IncompatibleFamilies => f.write_str(
                "vacuum alias families must share exact coefficient map, dimension and loop count",
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "vacuum alias {resource} limit: requested {requested}, limit {limit}"
            ),
        }
    }
}
impl std::error::Error for VacuumFamilyAliasError {}

/// A sealed unit equality, in the same formal loop-measure convention.
/// External family-specific prefactors and source guards are not removed.
#[derive(Clone, Debug)]
pub struct VerifiedFamilyVacuumAlias {
    pub(super) representative: VacuumIntegralKey,
    pub(super) witness: Arc<VerifiedVacuumParameterMap>,
}
impl VerifiedFamilyVacuumAlias {
    pub fn representative(&self) -> &VacuumIntegralKey {
        &self.representative
    }
    pub fn witness(&self) -> &VerifiedVacuumParameterMap {
        &self.witness
    }
}

/// Standalone finite, family-tagged scalar/dotted equalities. Every alias is a
/// direct edge to a declared representative ordered by (fingerprint, key).
/// No oracle, minimality claim, coefficient conversion, or campaign mutation.
#[derive(Clone, Debug)]
pub struct VacuumFamilyAliasPlan {
    pub(super) families: BTreeMap<Arc<String>, Arc<IntegralFamily>>,
    pub(super) raw: BTreeSet<VacuumIntegralKey>,
    pub(super) canonical: BTreeSet<VacuumIntegralKey>,
    pub(super) aliases: BTreeMap<VacuumIntegralKey, VerifiedFamilyVacuumAlias>,
    pub(super) statistics: TerminalAliasStatistics,
}
impl VacuumFamilyAliasPlan {
    pub fn raw_terminals(&self) -> &BTreeSet<VacuumIntegralKey> {
        &self.raw
    }
    pub fn canonical_terminals(&self) -> &BTreeSet<VacuumIntegralKey> {
        &self.canonical
    }
    pub fn aliases(&self) -> &BTreeMap<VacuumIntegralKey, VerifiedFamilyVacuumAlias> {
        &self.aliases
    }
    pub fn statistics(&self) -> &TerminalAliasStatistics {
        &self.statistics
    }
    pub fn representative(
        &self,
        family: &IntegralFamily,
        key: &IntegralKey,
    ) -> Result<&VacuumIntegralKey, TerminalAliasError> {
        let owner = family.fingerprint_owner();
        if !self.families.contains_key(&owner) {
            return Err(TerminalAliasError::WrongFamily);
        }
        if key.powers().len() != family.denominator_count() {
            return Err(TerminalAliasError::WrongArity {
                expected: family.denominator_count(),
                actual: key.powers().len(),
            });
        }
        let tagged = VacuumIntegralKey {
            family: owner,
            key: key.clone(),
        };
        if let Some(alias) = self.aliases.get(&tagged) {
            return Ok(alias.representative());
        }
        self.raw
            .get(&tagged)
            .ok_or(TerminalAliasError::UndeclaredTerminal)
    }
}
