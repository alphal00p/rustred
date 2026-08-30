use std::collections::HashMap;

use super::super::GuardDecisionDagError;

pub(super) const CANDIDATES: &str = "semantic guard DAG candidates";
pub(super) const UNIQUE_ATOMS: &str = "semantic guard DAG unique atoms";
pub(super) const CANDIDATE_ATOM_REFERENCES: &str = "semantic guard DAG candidate atom references";
pub(super) const ATOM_IDENTITY_BYTES: &str = "semantic guard DAG atom identity bytes";
pub(super) const MEMO_STATES: &str = "semantic guard DAG memo states";
pub(super) const MEMO_STATE_WORDS: &str = "semantic guard DAG memo state words";
pub(super) const CANDIDATE_SCANS: &str = "semantic guard DAG candidate scans";
pub(super) const NODES: &str = "semantic guard DAG nodes";
pub(super) const EDGES: &str = "semantic guard DAG edges";
pub(super) const PENDING_WORK: &str = "semantic guard DAG pending work items";

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GuardDecisionDagError> {
    if requested > limit {
        Err(GuardDecisionDagError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GuardDecisionDagError> {
    left.checked_add(right)
        .ok_or(GuardDecisionDagError::ResourceCountOverflow { resource })
}

pub(super) fn try_vec<T>(
    capacity: usize,
    resource: &'static str,
) -> Result<Vec<T>, GuardDecisionDagError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| GuardDecisionDagError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}

pub(super) fn try_clone_vec<T: Clone>(
    values: &[T],
    resource: &'static str,
) -> Result<Vec<T>, GuardDecisionDagError> {
    let mut result = try_vec(values.len(), resource)?;
    result.extend_from_slice(values);
    Ok(result)
}

pub(super) fn try_clone_boxed<T: Clone>(
    values: &[T],
    resource: &'static str,
) -> Result<Box<[T]>, GuardDecisionDagError> {
    Ok(try_clone_vec(values, resource)?.into_boxed_slice())
}

pub(super) fn try_hash_map<K: Eq + std::hash::Hash, V>(
    capacity: usize,
    resource: &'static str,
) -> Result<HashMap<K, V>, GuardDecisionDagError> {
    let mut values = HashMap::new();
    values
        .try_reserve(capacity)
        .map_err(|_| GuardDecisionDagError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
