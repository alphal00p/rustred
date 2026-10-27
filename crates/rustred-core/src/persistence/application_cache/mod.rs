//! Native generated cache payloads, without reduction or admission authority.
//!
//! Records contain only completed exact expansions and their ordered guards.
//! They contain no eviction metadata, work counters, pins, or partial stacks.
//! A caller must authenticate the binding, validate its current entry policy,
//! and admit decoded payloads to its current retained-memory budget before use.
mod codec;
#[cfg(test)]
mod tests;
mod writer;

use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::coefficient_cache_weight;

use super::{BinaryIoError, BinaryIoLimits};

/// Exact immutable artifact binding. The digest covers the complete runtime
/// package (including scope, routes, zero authority and terminal substitutions),
/// not merely the family or loop count. `layer` separates solver conventions.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct ApplicationCacheBinding {
    pub artifact_digest: [u8; 32],
    pub family_fingerprint: String,
    pub physical_arity: usize,
    pub layer: String,
}

impl ApplicationCacheBinding {
    pub fn validate(&self, limits: ApplicationCacheIoLimits) -> Result<(), BinaryIoError> {
        super::resource_checkpoint()?;
        limits.check_count("cache arity", self.physical_arity, limits.max_arity)?;
        if self.physical_arity == 0 || self.family_fingerprint.is_empty() || self.layer.is_empty() {
            return Err(BinaryIoError::Invalid("empty cache binding field"));
        }
        limits.check_count(
            "cache family fingerprint",
            self.family_fingerprint.len(),
            limits.max_identity_bytes,
        )?;
        limits.check_count("cache layer", self.layer.len(), limits.max_identity_bytes)
    }
}

/// Guarded single-solver roots and routed operational nodes are distinct keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, bincode::Encode, bincode::Decode)]
pub enum ApplicationCacheStage {
    GuardedRoot,
    Route,
    Apply,
}

/// Stable exact key; hashes may index records but never replace key equality.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, bincode::Encode, bincode::Decode)]
pub struct ApplicationCacheKey {
    stage: ApplicationCacheStage,
    owner: Vec<bool>,
    powers: Vec<i64>,
}

impl ApplicationCacheKey {
    pub fn guarded_root(key: &IntegralKey) -> Self {
        Self {
            stage: ApplicationCacheStage::GuardedRoot,
            owner: Vec::new(),
            powers: key.powers().to_vec(),
        }
    }
    pub fn route(key: &IntegralKey) -> Self {
        Self {
            stage: ApplicationCacheStage::Route,
            owner: Vec::new(),
            powers: key.powers().to_vec(),
        }
    }
    pub fn apply(owner: Vec<bool>, key: &IntegralKey) -> Self {
        Self {
            stage: ApplicationCacheStage::Apply,
            owner,
            powers: key.powers().to_vec(),
        }
    }
    pub fn stage(&self) -> ApplicationCacheStage {
        self.stage
    }
    pub fn owner(&self) -> &[bool] {
        &self.owner
    }
    pub fn powers(&self) -> &[i64] {
        &self.powers
    }
    /// Clone-owned key storage, separate from the coefficient census.
    pub fn owned_heap_bytes(&self) -> usize {
        self.powers
            .capacity()
            .saturating_mul(std::mem::size_of::<i64>())
            .saturating_add(
                self.owner
                    .capacity()
                    .saturating_mul(std::mem::size_of::<bool>()),
            )
    }
    pub fn integral(&self) -> Result<IntegralKey, BinaryIoError> {
        IntegralKey::try_new(self.powers.iter().copied())
            .map_err(|e| BinaryIoError::Native(e.to_string()))
    }
    pub fn validate(&self, arity: usize) -> Result<(), BinaryIoError> {
        super::resource_checkpoint()?;
        if self.powers.len() != arity || arity == 0 {
            return Err(BinaryIoError::Invalid(
                "cache key arity differs from binding",
            ));
        }
        match self.stage {
            ApplicationCacheStage::Apply => {
                if self.owner.len() != arity {
                    return Err(BinaryIoError::Invalid(
                        "cache Apply key differs from its owner support",
                    ));
                }
                for (&active, &power) in self.owner.iter().zip(&self.powers) {
                    super::resource_checkpoint()?;
                    if active != (power > 0) {
                        return Err(BinaryIoError::Invalid(
                            "cache Apply key differs from its owner support",
                        ));
                    }
                }
            }
            _ if !self.owner.is_empty() => {
                return Err(BinaryIoError::Invalid(
                    "non-Apply cache key carries an owner",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationCacheRecord {
    pub key: ApplicationCacheKey,
    pub terms: BTreeMap<IntegralKey, Coefficient>,
    pub nonzero_conditions: Vec<Coefficient>,
}

impl ApplicationCacheRecord {
    /// Clone transport payload under every ambient RAM/time scope. Known key,
    /// coefficient and guard buffers are checked before allocation; the map's
    /// private node storage is observed by RSS checkpoints after each insertion.
    /// This grants no binding, entry-admission or mathematical authority.
    pub fn try_clone_with_resources(&self) -> Result<Self, BinaryIoError> {
        use crate::algebra::coefficient_clone_owned_retained_byte_bound;

        let overflow = || BinaryIoError::Invalid("cache clone byte count overflow");
        super::resource_checkpoint()?;
        let key_bytes = self
            .key
            .powers
            .len()
            .checked_mul(std::mem::size_of::<i64>())
            .and_then(|bytes| bytes.checked_add(self.key.owner.len()))
            .ok_or_else(overflow)?;
        super::resource_allocation(key_bytes)?;
        let key = self.key.clone();
        super::resource_checkpoint()?;
        let mut terms = BTreeMap::new();
        for (target, value) in &self.terms {
            super::resource_checkpoint()?;
            let coefficient_bytes =
                coefficient_clone_owned_retained_byte_bound(value).ok_or_else(overflow)?;
            let clone_bytes = target
                .powers()
                .len()
                .checked_mul(std::mem::size_of::<i64>())
                .and_then(|bytes| bytes.checked_add(coefficient_bytes))
                .ok_or_else(overflow)?;
            super::resource_allocation(clone_bytes)?;
            terms.insert(target.clone(), value.clone());
            super::resource_checkpoint()?;
        }
        super::resource_array::<Coefficient>(self.nonzero_conditions.len())?;
        let mut nonzero_conditions = Vec::new();
        nonzero_conditions
            .try_reserve_exact(self.nonzero_conditions.len())
            .map_err(|_| BinaryIoError::Allocation {
                resource: "cache clone guards",
                requested: self.nonzero_conditions.len(),
            })?;
        super::resource_checkpoint()?;
        for value in &self.nonzero_conditions {
            let coefficient_heap_bytes = coefficient_clone_owned_retained_byte_bound(value)
                .and_then(|bytes| bytes.checked_sub(std::mem::size_of::<Coefficient>()))
                .ok_or_else(overflow)?;
            super::resource_allocation(coefficient_heap_bytes)?;
            nonzero_conditions.push(value.clone());
            super::resource_checkpoint()?;
        }
        Ok(Self {
            key,
            terms,
            nonzero_conditions,
        })
    }

    /// Same clone-owned coefficient census used by native reduction caches.
    /// Key/index storage and native scratch are separately bounded by I/O limits.
    pub fn coefficient_census(&self) -> Result<(usize, usize), BinaryIoError> {
        super::resource_checkpoint()?;
        let weight = coefficient_cache_weight(self.terms.values().chain(&self.nonzero_conditions))
            .map_err(reduction_error)?;
        super::resource_checkpoint()?;
        Ok((weight.coefficient_terms, weight.coefficient_bytes))
    }
}

/// Owned-byte allowances apply independently of the ambient RSS/time scopes.
/// Finite count/identity templates are opt-in diagnostics and do not constrain
/// normal execution. Index bytes exclude coefficient payloads; coefficient
/// bytes exclude map/key bookkeeping and native scratch, which remain subject
/// to the ambient resource budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationCacheIoLimits {
    pub binary: BinaryIoLimits,
    pub diagnostics_enabled: bool,
    pub max_arity: usize,
    pub max_identity_bytes: usize,
    pub max_records: usize,
    /// Retained row/key and coefficient-frame index storage, excluding payloads.
    pub max_index_owned_bytes: usize,
    pub max_terms_per_record: usize,
    pub max_guards_per_record: usize,
    pub max_decoded_coefficient_terms: usize,
    pub max_decoded_coefficient_bytes: usize,
}
impl Default for ApplicationCacheIoLimits {
    fn default() -> Self {
        Self {
            binary: BinaryIoLimits::default(),
            diagnostics_enabled: false,
            max_arity: 64,
            max_identity_bytes: 1_000_000,
            max_records: 4096,
            max_index_owned_bytes: usize::MAX,
            max_terms_per_record: 1_000_000,
            max_guards_per_record: 1_000_000,
            max_decoded_coefficient_terms: 16_000_000,
            max_decoded_coefficient_bytes: usize::MAX,
        }
    }
}

impl ApplicationCacheIoLimits {
    fn check_count(
        self,
        resource: &'static str,
        requested: usize,
        limit: usize,
    ) -> Result<(), BinaryIoError> {
        super::resource_checkpoint()?;
        if self.diagnostics_enabled {
            super::error::check_limit(resource, requested, limit)?;
        }
        Ok(())
    }
}

pub use codec::{ApplicationCacheChunk, OwnedApplicationCacheChunk};
pub use writer::{
    ApplicationCacheChunkWriteStatistics, ApplicationCacheChunkWriter,
    encode_application_cache_chunk,
};

fn reduction_error(error: crate::reduction::ReductionError) -> BinaryIoError {
    match error {
        crate::reduction::ReductionError::Resources(error) => BinaryIoError::Resource(error),
        error => BinaryIoError::Native(error.to_string()),
    }
}

fn validate_coefficient(
    value: &Coefficient,
    context: &CoefficientContext,
    limits: ApplicationCacheIoLimits,
) -> Result<(), BinaryIoError> {
    super::resource_checkpoint()?;
    context
        .validate_with_limits(value, limits.binary.exact_algebra)
        .map_err(|e| BinaryIoError::Native(e.to_string()))
}
