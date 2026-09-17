use rustred::algebra::ExactAlgebraLimits;
use rustred::foundry::artifact::{ArtifactLoadLimits, SourcePortLimits};
use serde::{Deserialize, Serialize};

use crate::application::InputFormat;

pub const CANDIDATE_BUNDLE_SCHEMA: &str = "rustred.uncertified-candidates.toml.v1";
pub const FAMILY_CANDIDATES_SCHEMA: &str = "rustred.family-candidates-output.toml.v1";
pub const CANDIDATE_CERTIFICATION_SCHEMA: &str = "rustred.candidate-certification-output.toml.v1";
/// Maximum semantic rank bound accepted by the (currently fail-closed)
/// bounded-certification front end.  Keeping this cap explicit prevents a
/// caller from accidentally turning a bounded request into an unbounded
/// resource campaign while the scoped artifact contract is completed.
pub const MAX_RANK_SCOPED_CERTIFICATION_DEGREE: usize = 30;
pub(super) const STATUS: &str = "uncertified-candidates";
pub(super) const SOLVER_POLICY: &str = "ordinary-source-port-default-v1";

/// Caller-owned ingress/output policy, not data read from a candidate bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateBundleLimits {
    pub max_bundle_bytes: usize,
    pub max_collection_entries: usize,
    pub max_coefficient_bytes: usize,
    pub max_total_coefficient_bytes: usize,
    pub exact_algebra: ExactAlgebraLimits,
}

impl Default for CandidateBundleLimits {
    fn default() -> Self {
        let core = ArtifactLoadLimits::default();
        Self {
            max_bundle_bytes: core.max_artifact_bytes,
            max_collection_entries: core.max_collection_entries,
            max_coefficient_bytes: core.max_coefficient_bytes,
            max_total_coefficient_bytes: core.max_total_coefficient_bytes,
            exact_algebra: core.family.exact_algebra,
        }
    }
}

/// Solve the explicit root and save formulas without trying to certify closure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyCandidatesRequest {
    pub source: String,
    pub input_format: InputFormat,
    pub n_cores: usize,
    pub permutation: Option<Vec<usize>>,
    pub nonpositive_indices: Vec<usize>,
    pub bundle_limits: CandidateBundleLimits,
}

impl FamilyCandidatesRequest {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            input_format: InputFormat::Auto,
            n_cores: 1,
            permutation: None,
            nonpositive_indices: Vec::new(),
            bundle_limits: CandidateBundleLimits::default(),
        }
    }
}

/// Reconstruct saved formulas and independently replay/install them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateCertificationRequest {
    pub bundle: Vec<u8>,
    pub input_limits: CandidateBundleLimits,
    pub publication_limits: SourcePortLimits,
    /// Optional entry numerator degree requested for a future scoped proof.
    /// This is deliberately not interpreted as a whole-family certificate:
    /// the current durable artifact schema has no persisted successor-closed
    /// scope or runtime entry admission.  Requests are therefore rejected
    /// before decoding until that contract is implemented end to end.
    pub max_negative_index_degree: Option<usize>,
}

impl CandidateCertificationRequest {
    pub fn new(bundle: impl Into<Vec<u8>>) -> Self {
        Self {
            bundle: bundle.into(),
            input_limits: CandidateBundleLimits::default(),
            publication_limits: SourcePortLimits::default(),
            max_negative_index_degree: None,
        }
    }

    pub fn with_max_negative_index_degree(mut self, degree: usize) -> Self {
        self.max_negative_index_degree = Some(degree);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateBundleResult {
    pub(super) bundle: Vec<u8>,
    pub(super) report_toml: String,
}

impl CandidateBundleResult {
    pub fn schema(&self) -> &'static str {
        FAMILY_CANDIDATES_SCHEMA
    }
    pub fn status(&self) -> &'static str {
        STATUS
    }
    pub fn bundle(&self) -> &[u8] {
        &self.bundle
    }
    pub fn into_bundle(self) -> Vec<u8> {
        self.bundle
    }
    pub fn to_toml(&self) -> &str {
        &self.report_toml
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateCertificationResult {
    pub(super) artifact: Vec<u8>,
    pub(super) report_toml: String,
}

impl CandidateCertificationResult {
    pub fn schema(&self) -> &'static str {
        CANDIDATE_CERTIFICATION_SCHEMA
    }
    pub fn status(&self) -> &'static str {
        "generated-durable"
    }
    pub fn artifact(&self) -> &[u8] {
        &self.artifact
    }
    pub fn into_artifact(self) -> Vec<u8> {
        self.artifact
    }
    pub fn to_toml(&self) -> &str {
        &self.report_toml
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bundle {
    pub schema: String,
    pub status: String,
    pub solver_policy: String,
    pub family_source: String,
    pub input_format: String,
    pub family_fingerprint: String,
    pub root_sector: Vec<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permutation: Option<Vec<usize>>,
    pub sectors: Vec<SectorRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SectorRecord {
    pub sector: Vec<bool>,
    pub rules: Vec<RuleRecord>,
    pub finite_residuals: Vec<IntegralRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuleRecord {
    pub case: CaseRecord,
    pub target: IntegralRecord,
    pub rhs: Vec<TermRecord>,
    pub sources: Vec<SeedRecord>,
    pub exclusions: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseRecord {
    pub kind: String,
    pub fixed_axes: Vec<usize>,
    pub fixed_values: Vec<i16>,
    pub equations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IntegralRecord {
    pub symbolic: Vec<bool>,
    pub values: Vec<i16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TermRecord {
    pub integral: IntegralRecord,
    pub coefficient: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeedRecord {
    pub basis_row: usize,
    pub integral: IntegralRecord,
    pub shifts: Vec<i16>,
}
