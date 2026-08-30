//! Exact physical-column roles on one decorated family stratum.
//!
//! This boundary separates discovery from proof. A raw physical column may
//! enter a target query's allowed RHS block only after the named ordering
//! proves same-sector descent and every exact proper-subsector image is
//! covered by a terminalizing owner frozen from an authenticated artifact.
//! Ordinary RuleCells do not become closure owners merely by existing.
//!
//! The registry remains an `i64`-carrier experiment. It does not turn machine
//! endpoints into mathematical infinity and does not prove coefficient guard
//! predicates; [`DecoratedStratum`] only binds to guard proofs owned by later
//! exact foundry layers.

mod error;
mod identity;
mod limits;
mod model;
mod owners;
mod partition;

pub(crate) use error::StratumRegistryError;
pub(crate) use limits::StratumRegistryLimits;
pub(crate) use model::{
    DecoratedStratum, DecoratedStratumId, GuardBranch, GuardBranchIdentity, GuardPredicateAuthority,
};
pub(crate) use owners::{
    ImmutableOwnerKind, ImmutableOwnerSnapshot, ImmutableOwnerSnapshotId, ImmutableOwnerWitness,
};
pub(crate) use partition::{ForbiddenColumnReason, ProperSubsectorOwner, TargetColumnPartition};

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, StratumRegistryError> {
    left.checked_add(right)
        .ok_or(StratumRegistryError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, StratumRegistryError> {
    left.checked_mul(right)
        .ok_or(StratumRegistryError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), StratumRegistryError> {
    if requested > limit {
        Err(StratumRegistryError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), StratumRegistryError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| StratumRegistryError::AllocationFailure {
            resource,
            requested,
        })
}

#[cfg(test)]
mod tests;
