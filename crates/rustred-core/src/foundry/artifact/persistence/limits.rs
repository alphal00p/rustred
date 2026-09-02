use crate::family::IntegralFamilyLimits;
use crate::foundry::artifact::{K6_ARITY, K6_MASTER_TERMINAL_COUNT, MAX_PUBLISHED_K6_RULE_CELLS};
use crate::foundry::cell::RuleCellLimits;
use crate::foundry::completion::CompletionGeometryLimits;
use crate::foundry::parametric::ParametricRuleLimits;
use crate::identity::{ParametricIbpConfig, TranslatedSourceLimits};

const K6_BOX_ENDPOINT_CELLS: usize = K6_ARITY * 2;

/// Bounded exact box-cover policy used while authenticating a persisted
/// closing artifact.
///
/// These limits are public because durable input is untrusted and callers may
/// need a tighter policy.  They are separate from campaign search geometry:
/// ordinary artifact loading only subtracts persisted rule/master boxes from
/// one finite certified root at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtifactCoverReplayLimits {
    pub max_arity: usize,
    pub max_requested_boxes: usize,
    pub max_requested_box_coordinate_cells: usize,
    pub max_uncovered_boxes: usize,
    pub max_uncovered_box_coordinate_cells: usize,
    pub max_split_operations: usize,
}

impl Default for ArtifactCoverReplayLimits {
    fn default() -> Self {
        // One rule cell can contribute one six-dimensional box with two
        // endpoints per coordinate.  Keep every related default coherent with
        // the shared one-million-cell publication/load ceiling rather than
        // retaining the generic geometry helper's historical 65,536-box cap.
        // The verifier filters by registered sector before allocating, but
        // retain a conservative policy for the complete published cell plus
        // master census.  All arithmetic is checked even though these are
        // compile-time-owned constants.
        let requested_boxes = MAX_PUBLISHED_K6_RULE_CELLS
            .checked_add(K6_MASTER_TERMINAL_COUNT)
            .expect("registered K6 cover-box ceiling fits usize");
        let coordinate_cells = requested_boxes
            .checked_mul(K6_BOX_ENDPOINT_CELLS)
            .expect("registered K6 cover-coordinate ceiling fits usize");
        Self {
            max_arity: 4_096,
            max_requested_boxes: requested_boxes,
            max_requested_box_coordinate_cells: coordinate_cells,
            max_uncovered_boxes: MAX_PUBLISHED_K6_RULE_CELLS,
            max_uncovered_box_coordinate_cells: coordinate_cells,
            max_split_operations: coordinate_cells,
        }
    }
}

impl ArtifactCoverReplayLimits {
    pub(crate) fn geometry(self) -> CompletionGeometryLimits {
        // BoxCover never consults the leading-ideal generator fields.  Keep
        // them closed instead of importing unrelated hidden defaults into the
        // public durable-load policy.
        CompletionGeometryLimits {
            max_arity: self.max_arity,
            max_requested_generators: 0,
            max_requested_generator_coordinate_cells: 0,
            max_minimal_generators: 0,
            max_requested_boxes: self.max_requested_boxes,
            max_requested_box_coordinate_cells: self.max_requested_box_coordinate_cells,
            max_uncovered_boxes: self.max_uncovered_boxes,
            max_uncovered_box_coordinate_cells: self.max_uncovered_box_coordinate_cells,
            max_split_operations: self.max_split_operations,
        }
    }
}

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
    /// Aggregate policy for replaying each stored translated-source request
    /// batch before native elimination.
    pub translated_sources: TranslatedSourceLimits,
    /// Explicit policy for deriving and exactly replaying stored rule plans.
    pub rule_derivation: ParametricRuleLimits,
    /// Policy for residual projection, guard refinement, and retained cell
    /// payloads reconstructed from each stored rule plan.
    pub rule_cells: RuleCellLimits,
    /// Exact persisted rule/master cover replay policy.  This is applied
    /// before allocating the per-sector input box collection.
    pub cover_replay: ArtifactCoverReplayLimits,
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
            translated_sources: TranslatedSourceLimits::default(),
            rule_derivation: ParametricRuleLimits::default(),
            rule_cells: RuleCellLimits::default(),
            cover_replay: ArtifactCoverReplayLimits::default(),
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
