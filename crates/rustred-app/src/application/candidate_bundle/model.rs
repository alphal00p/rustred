use rustred::algebra::ExactAlgebraLimits;
use rustred::foundry::artifact::{ArtifactLoadLimits, SourcePortLimits};
use rustred::solver::SymbolicExactBackend;

use crate::application::InputFormat;

pub const CANDIDATE_BUNDLE_SCHEMA: &str = "rustred.generated-candidates.binary.v1";
pub const FAMILY_CANDIDATES_SCHEMA: &str = "rustred.family-candidates-output.toml.v1";
pub const CANDIDATE_CERTIFICATION_SCHEMA: &str = "rustred.candidate-certification-output.toml.v1";
/// Hard ceiling for explicitly enlarged candidate ingress/output policies.
/// Candidates retain discovery source traces and may exceed closed-artifact
/// sizes. Defaults remain unchanged; callers must opt into the larger budget.
pub const MAX_CANDIDATE_BUNDLE_BYTES: usize = 1024 * 1024 * 1024;
/// Maximum semantic rank bound accepted by the (currently fail-closed)
/// bounded-certification front end.  Keeping this cap explicit prevents a
/// caller from accidentally turning a bounded request into an unbounded
/// resource campaign while the scoped artifact contract is completed.
pub const MAX_RANK_SCOPED_CERTIFICATION_DEGREE: usize = 30;
pub(super) const STATUS: &str = "uncertified-candidates";
pub(super) const SOLVER_POLICY: &str = "ordinary-source-port-default-v1";

/// Structural metadata only: counts do not establish valid rules or closure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateBundleInspection {
    pub schema: String,
    pub status: String,
    pub family_fingerprint: String,
    pub arity: usize,
    pub solved_sectors: usize,
    pub generated_rules: usize,
    pub finite_residuals: usize,
    pub unique_coefficients: usize,
    pub symbolica_state_bytes: usize,
    pub coefficient_table_bytes: usize,
}

/// Exact materialization policy for candidate generation. All choices use
/// the same source discovery and guards; none certifies family closure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CandidateExactBackend {
    #[default]
    Sparse,
    SemiNumerical,
    /// Native factorized denominators throughout symbolic sparse elimination.
    /// Rule extraction and candidate bundles retain ordinary coefficients.
    SparseFactorized,
}

impl CandidateExactBackend {
    pub const EXPECTED_VALUES: &str = "sparse, sparse-factorized, or semi-numerical";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sparse => "sparse",
            Self::SemiNumerical => "semi-numerical",
            Self::SparseFactorized => "sparse-factorized",
        }
    }

    pub(super) fn solver_backend(self) -> SymbolicExactBackend {
        match self {
            Self::Sparse => SymbolicExactBackend::Sparse,
            Self::SparseFactorized => SymbolicExactBackend::SparseFactorized,
            Self::SemiNumerical => SymbolicExactBackend::SemiNumerical {
                max_degree: 128,
                max_probes: 200_000,
                max_attempts: 4,
                max_primes: 8,
            },
        }
    }
}

impl std::str::FromStr for CandidateExactBackend {
    type Err = crate::AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "sparse" => Ok(Self::Sparse),
            "semi-numerical" => Ok(Self::SemiNumerical),
            "sparse-factorized" => Ok(Self::SparseFactorized),
            _ => Err(crate::AppError::input(format!(
                "invalid candidate exact backend {value:?}; expected {}",
                Self::EXPECTED_VALUES
            ))),
        }
    }
}

/// Caller-owned ingress/output policy, not data read from a candidate bundle.
/// Byte limits bound native generated-program frames, not peak RSS or every
/// allocation inside Symbolica. Native payloads require trusted provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateBundleLimits {
    pub max_bundle_bytes: usize,
    pub max_collection_entries: usize,
    pub max_coefficient_bytes: usize,
    pub max_total_coefficient_bytes: usize,
    pub exact_algebra: ExactAlgebraLimits,
}

impl CandidateBundleLimits {
    pub(super) fn bundle_byte_limit(self) -> usize {
        self.max_bundle_bytes.min(MAX_CANDIDATE_BUNDLE_BYTES)
    }

    pub(super) fn binary_limits(self) -> rustred::persistence::BinaryIoLimits {
        rustred::persistence::BinaryIoLimits {
            max_program_bytes: self.bundle_byte_limit(),
            max_collection_entries: self.max_collection_entries,
            max_atom_bytes: self.max_coefficient_bytes,
            max_total_atom_bytes: self.max_total_coefficient_bytes,
            exact_algebra: self.exact_algebra,
            ..Default::default()
        }
    }

    pub(super) fn family_limits(self) -> rustred::family::IntegralFamilyLimits {
        rustred::family::IntegralFamilyLimits {
            exact_algebra: self.exact_algebra,
            ..Default::default()
        }
    }
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
    pub exact_backend: CandidateExactBackend,
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
            exact_backend: CandidateExactBackend::default(),
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

#[derive(Clone)]
pub(super) struct Bundle {
    pub records: ProgramRecord,
    pub family: rustred::persistence::NativeFamilyRecord,
    pub coefficients: std::sync::Arc<rustred::persistence::DecodedCoefficientTable>,
}

impl std::fmt::Debug for Bundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bundle")
            .field("records", &self.records)
            .field("family", &self.family)
            .field("coefficient_count", &self.coefficients.len())
            .finish()
    }
}

impl std::ops::Deref for Bundle {
    type Target = ProgramRecord;
    fn deref(&self) -> &Self::Target {
        &self.records
    }
}

impl std::ops::DerefMut for Bundle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.records
    }
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct ProgramRecord {
    pub schema: String,
    pub status: String,
    pub solver_policy: String,
    pub family_source: String,
    pub input_format: String,
    pub family_fingerprint: String,
    pub root_sector: Vec<bool>,
    pub permutation: Option<Vec<usize>>,
    pub sectors: Vec<SectorRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct SectorRecord {
    pub sector: Vec<bool>,
    pub rules: Vec<RuleRecord>,
    pub finite_residuals: Vec<IntegralRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct RuleRecord {
    pub case: CaseRecord,
    pub target: IntegralRecord,
    pub rhs: Vec<TermRecord>,
    pub sources: Vec<SeedRecord>,
    pub exclusions: Vec<Vec<u32>>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct CaseRecord {
    pub kind: String,
    pub fixed_axes: Vec<usize>,
    pub fixed_values: Vec<i16>,
    pub equations: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct IntegralRecord {
    pub symbolic: Vec<bool>,
    pub values: Vec<i16>,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct TermRecord {
    pub integral: IntegralRecord,
    pub coefficient: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct SeedRecord {
    pub basis_row: usize,
    pub integral: IntegralRecord,
    pub shifts: Vec<i16>,
}
