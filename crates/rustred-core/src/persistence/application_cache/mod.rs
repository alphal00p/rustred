//! Native generated cache payloads, without reduction or admission authority.
//!
//! Records contain only completed exact expansions and their ordered guards.
//! They contain no eviction metadata, work counters, pins, or partial stacks.
//! A caller must authenticate the binding, validate its current entry policy,
//! and admit decoded payloads to its current retained-memory budget before use.
mod codec;
#[cfg(test)]
mod tests;

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
        super::error::check_limit("cache arity", self.physical_arity, limits.max_arity)?;
        if self.physical_arity == 0 || self.family_fingerprint.is_empty() || self.layer.is_empty() {
            return Err(BinaryIoError::Invalid("empty cache binding field"));
        }
        super::error::check_limit(
            "cache family fingerprint",
            self.family_fingerprint.len(),
            limits.max_identity_bytes,
        )?;
        super::error::check_limit("cache layer", self.layer.len(), limits.max_identity_bytes)
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
        if self.powers.len() != arity || arity == 0 {
            return Err(BinaryIoError::Invalid(
                "cache key arity differs from binding",
            ));
        }
        match self.stage {
            ApplicationCacheStage::Apply => {
                if self.owner.len() != arity
                    || self
                        .owner
                        .iter()
                        .zip(&self.powers)
                        .any(|(&active, &power)| active != (power > 0))
                {
                    return Err(BinaryIoError::Invalid(
                        "cache Apply key differs from its owner support",
                    ));
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
    /// Same clone-owned coefficient census used by native reduction caches.
    /// Key/index storage and native scratch are separately bounded by I/O limits.
    pub fn coefficient_census(&self) -> Result<(usize, usize), BinaryIoError> {
        let weight = coefficient_cache_weight(self.terms.values().chain(&self.nonzero_conditions))
            .map_err(|e| BinaryIoError::Native(e.to_string()))?;
        Ok((weight.coefficient_terms, weight.coefficient_bytes))
    }
}

/// Per-chunk/per-record bounds, not a process-wide RSS guarantee. The decoder
/// retains at most one chunk and one requested record; native Symbolica scratch
/// and its process-global symbol registry retain their existing contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationCacheIoLimits {
    pub binary: BinaryIoLimits,
    pub max_arity: usize,
    pub max_identity_bytes: usize,
    pub max_records: usize,
    pub max_terms_per_record: usize,
    pub max_guards_per_record: usize,
    pub max_decoded_coefficient_terms: usize,
    pub max_decoded_coefficient_bytes: usize,
}
impl Default for ApplicationCacheIoLimits {
    fn default() -> Self {
        Self {
            binary: BinaryIoLimits {
                max_program_bytes: 64 * 1024 * 1024,
                max_total_atom_bytes: 64 * 1024 * 1024,
                ..Default::default()
            },
            max_arity: 64,
            max_identity_bytes: 1_000_000,
            max_records: 4096,
            max_terms_per_record: 1_000_000,
            max_guards_per_record: 1_000_000,
            max_decoded_coefficient_terms: 16_000_000,
            max_decoded_coefficient_bytes: 1024 * 1024 * 1024,
        }
    }
}

pub use codec::{ApplicationCacheChunk, encode_application_cache_chunk};

fn validate_coefficient(
    value: &Coefficient,
    context: &CoefficientContext,
    limits: ApplicationCacheIoLimits,
) -> Result<(), BinaryIoError> {
    context
        .validate_with_limits(value, limits.binary.exact_algebra)
        .map_err(|e| BinaryIoError::Native(e.to_string()))
}
