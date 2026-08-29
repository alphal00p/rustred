use crate::family::IntegralFamilyLimits;
use crate::foundry::parametric::ParametricRuleLimits;
use crate::identity::ParametricIbpConfig;

/// Resource policy applied before durable payload allocations or native
/// Symbolica algebra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtifactLoadLimits {
    pub max_artifact_bytes: usize,
    pub max_string_bytes: usize,
    pub max_coefficient_bytes: usize,
    pub max_total_coefficient_bytes: usize,
    /// Aggregate opaque source/rule semantic-witness bytes. Witnesses are
    /// compared byte-for-byte and never decoded into native algebra.
    pub max_total_witness_bytes: usize,
    pub max_collection_entries: usize,
    pub max_index_arity: usize,
    /// Exact family reconstruction policy, including the coefficient limits
    /// used while admitting the sparse binary coefficient payloads.
    pub family: IntegralFamilyLimits,
    /// Explicit context/relation policy for independently regenerating the
    /// source derivation plan recorded by the artifact.
    pub source_generation: ParametricIbpConfig,
    /// Explicit policy for deriving and exactly replaying stored rule plans.
    pub rule_derivation: ParametricRuleLimits,
}

/// Resource policy for deterministic durable encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtifactEncodingLimits {
    pub max_artifact_bytes: usize,
    pub max_string_bytes: usize,
    pub max_coefficient_bytes: usize,
    /// Aggregate sparse coefficient payload bytes across the complete
    /// artifact, shared by every nested section and semantic snapshot.
    pub max_total_coefficient_bytes: usize,
    /// Aggregate source/rule semantic-witness bytes across nested plans.
    pub max_total_witness_bytes: usize,
    pub max_collection_entries: usize,
}

impl Default for ArtifactEncodingLimits {
    fn default() -> Self {
        Self {
            max_artifact_bytes: 256 * 1024 * 1024,
            max_string_bytes: 1024 * 1024,
            max_coefficient_bytes: 16 * 1024 * 1024,
            max_total_coefficient_bytes: 128 * 1024 * 1024,
            max_total_witness_bytes: 128 * 1024 * 1024,
            max_collection_entries: 1_000_000,
        }
    }
}

impl Default for ArtifactLoadLimits {
    fn default() -> Self {
        Self {
            max_artifact_bytes: 256 * 1024 * 1024,
            max_string_bytes: 1024 * 1024,
            max_coefficient_bytes: 16 * 1024 * 1024,
            max_total_coefficient_bytes: 128 * 1024 * 1024,
            max_total_witness_bytes: 128 * 1024 * 1024,
            max_collection_entries: 1_000_000,
            max_index_arity: 4_096,
            family: IntegralFamilyLimits::default(),
            source_generation: ParametricIbpConfig::default(),
            rule_derivation: ParametricRuleLimits::default(),
        }
    }
}

impl ArtifactLoadLimits {
    pub(super) fn replay_encoding(self) -> ArtifactEncodingLimits {
        ArtifactEncodingLimits {
            max_artifact_bytes: self.max_artifact_bytes,
            max_string_bytes: self.max_string_bytes,
            max_coefficient_bytes: self.max_coefficient_bytes,
            max_total_coefficient_bytes: self.max_total_coefficient_bytes,
            max_total_witness_bytes: self.max_total_witness_bytes,
            max_collection_entries: self.max_collection_entries,
        }
    }
}
