mod campaign {
    pub(super) mod plan;
    pub(super) mod preflight;
}
mod closing;
mod derive;
mod error;
mod input;
mod lowering;
pub(crate) mod memory;
mod model;
mod options;
mod producer;

pub use error::{AppError, AppErrorKind};
pub use options::{
    ClosingFamilySelector, InputFormat, ParseClosingFamilySelectorError, ParseInputFormatError,
    ParseRelationSelectionError, RelationSelection,
};

/// Maximum UTF-8 source payload accepted by every in-process application API.
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum canonical TOML payload returned by every application API.
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;
/// Application ceiling for one closing-artifact reduction request.
pub const MAX_CLOSING_RULE_APPLICATIONS: usize = 1_000_000;
/// Early ingress ceiling aligned with core durable-artifact load defaults.
pub const MAX_CLOSING_ARTIFACT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeriveRequest {
    pub source: String,
    pub input_format: InputFormat,
    pub relations: RelationSelection,
    pub n_cores: usize,
}

impl DeriveRequest {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            input_format: InputFormat::Auto,
            relations: RelationSelection::All,
            n_cores: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignPlanRequest {
    pub source: String,
    pub input_format: InputFormat,
    pub root_id: Option<String>,
}

impl CampaignPlanRequest {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            input_format: InputFormat::Auto,
            root_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignPreflightRequest {
    pub profile: String,
    pub n_cores: usize,
    pub max_memory_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosingArtifactGenerateRequest {
    pub family: ClosingFamilySelector,
}

impl ClosingArtifactGenerateRequest {
    pub const fn new(family: ClosingFamilySelector) -> Self {
        Self { family }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosingArtifactInspectRequest {
    pub artifact: Vec<u8>,
}

impl ClosingArtifactInspectRequest {
    pub fn new(artifact: impl Into<Vec<u8>>) -> Self {
        Self {
            artifact: artifact.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosingArtifactReduceRequest {
    pub artifact: Vec<u8>,
    pub target_powers: Vec<i64>,
    pub max_rule_applications: usize,
}

impl ClosingArtifactReduceRequest {
    pub fn new(artifact: impl Into<Vec<u8>>, target_powers: impl Into<Vec<i64>>) -> Self {
        Self {
            artifact: artifact.into(),
            target_powers: target_powers.into(),
            max_rule_applications: MAX_CLOSING_RULE_APPLICATIONS,
        }
    }
}

impl CampaignPreflightRequest {
    pub fn new(profile: impl Into<String>, n_cores: usize, max_memory_bytes: u64) -> Self {
        Self {
            profile: profile.into(),
            n_cores,
            max_memory_bytes,
        }
    }
}

macro_rules! canonical_result {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            schema: &'static str,
            status: &'static str,
            canonical_toml: String,
        }

        impl $name {
            pub(crate) fn new(
                schema: &'static str,
                status: &'static str,
                canonical_toml: String,
            ) -> Self {
                Self {
                    schema,
                    status,
                    canonical_toml,
                }
            }

            pub const fn schema(&self) -> &'static str {
                self.schema
            }

            pub const fn status(&self) -> &'static str {
                self.status
            }

            /// Return the canonical, newline-terminated TOML document.
            pub fn to_toml(&self) -> &str {
                &self.canonical_toml
            }

            pub fn into_toml(self) -> String {
                self.canonical_toml
            }
        }
    };
}

canonical_result!(DeriveResult);
canonical_result!(CampaignPlanResult);
canonical_result!(CampaignPreflightResult);
canonical_result!(ClosingArtifactInspectResult);

/// Deterministic durable bytes plus the canonical generation metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosingArtifactGenerateResult {
    schema: &'static str,
    status: &'static str,
    canonical_toml: String,
    artifact: Vec<u8>,
}

impl ClosingArtifactGenerateResult {
    pub(crate) fn new(
        schema: &'static str,
        status: &'static str,
        canonical_toml: String,
        artifact: Vec<u8>,
    ) -> Self {
        Self {
            schema,
            status,
            canonical_toml,
            artifact,
        }
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn status(&self) -> &'static str {
        self.status
    }

    /// Canonical metadata describing the generated artifact.
    pub fn to_toml(&self) -> &str {
        &self.canonical_toml
    }

    /// Deterministic immutable closing-artifact encoding.
    pub fn artifact(&self) -> &[u8] {
        &self.artifact
    }

    pub fn into_artifact(self) -> Vec<u8> {
        self.artifact
    }
}

/// One exact coefficient of a typed master integral, with common-scale
/// restoration kept as a separate dimensional-homogeneity monomial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactMasterCoefficient {
    master_powers: Vec<i64>,
    unit_mass_coefficient: String,
    common_mass_squared_power: i128,
}

impl ExactMasterCoefficient {
    pub fn master_powers(&self) -> &[i64] {
        &self.master_powers
    }

    pub fn unit_mass_coefficient(&self) -> &str {
        &self.unit_mass_coefficient
    }

    pub const fn common_mass_squared_power(&self) -> i128 {
        self.common_mass_squared_power
    }

    pub(crate) fn new(
        master_powers: Vec<i64>,
        unit_mass_coefficient: String,
        common_mass_squared_power: i128,
    ) -> Self {
        Self {
            master_powers,
            unit_mass_coefficient,
            common_mass_squared_power,
        }
    }
}

/// Canonical reduction document together with its transport-neutral exact
/// master decomposition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosingArtifactReduceResult {
    schema: &'static str,
    status: &'static str,
    canonical_toml: String,
    family_fingerprint: String,
    target_powers: Vec<i64>,
    terms: Vec<ExactMasterCoefficient>,
}

impl ClosingArtifactReduceResult {
    pub(crate) fn new(
        schema: &'static str,
        status: &'static str,
        canonical_toml: String,
        family_fingerprint: String,
        target_powers: Vec<i64>,
        terms: Vec<ExactMasterCoefficient>,
    ) -> Self {
        Self {
            schema,
            status,
            canonical_toml,
            family_fingerprint,
            target_powers,
            terms,
        }
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn status(&self) -> &'static str {
        self.status
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn target_powers(&self) -> &[i64] {
        &self.target_powers
    }

    pub fn terms(&self) -> &[ExactMasterCoefficient] {
        &self.terms
    }

    pub fn to_toml(&self) -> &str {
        &self.canonical_toml
    }

    pub fn into_toml(self) -> String {
        self.canonical_toml
    }
}

/// Derive canonical, fully parametric IBP/LI relations for one owned request.
///
/// # Panics
///
/// Expected failures are returned as [`AppError`]. RustRed/Symbolica invariant
/// panics are deliberately not caught here. If an outer FFI or coordinator
/// boundary catches an unwind, it must treat the shared runtime as poisoned
/// and reject subsequent work rather than reuse potentially mutated state.
pub fn derive(request: DeriveRequest) -> Result<DeriveResult, AppError> {
    validate_ingress("derive input", &request.source)?;
    if request.n_cores == 0 {
        return Err(AppError::input("derive n_cores must be a positive integer"));
    }
    derive::derive_request(request)
}

/// Authenticate and deduplicate the declared roots of one campaign.
///
/// # Panics
///
/// This function follows the panic contract documented on [`derive()`].
pub fn campaign_plan(request: CampaignPlanRequest) -> Result<CampaignPlanResult, AppError> {
    validate_ingress("campaign input", &request.source)?;
    campaign::plan::plan_request(request)
}

/// Compute the RAM-aware execution-width preflight without starting workers.
///
/// # Panics
///
/// This function follows the panic contract documented on [`derive()`].
pub fn campaign_preflight(
    request: CampaignPreflightRequest,
) -> Result<CampaignPreflightResult, AppError> {
    validate_ingress("campaign execution resource profile", &request.profile)?;
    if request.n_cores == 0 {
        return Err(AppError::input(
            "campaign preflight n_cores must be a positive integer",
        ));
    }
    if request.max_memory_bytes == 0 {
        return Err(AppError::input(
            "campaign preflight max_memory_bytes must be positive",
        ));
    }
    campaign::preflight::preflight_request(request)
}

/// Generate and seal one selected closing artifact for this operation.
///
/// The result owns deterministic durable bytes as well as canonical metadata.
pub fn closing_artifact_generate(
    request: ClosingArtifactGenerateRequest,
) -> Result<ClosingArtifactGenerateResult, AppError> {
    closing::generate_request(request)
}

/// Decode, authenticate once, and inspect supplied durable artifact bytes.
pub fn closing_artifact_inspect(
    request: ClosingArtifactInspectRequest,
) -> Result<ClosingArtifactInspectResult, AppError> {
    closing::inspect_request(request)
}

/// Decode once and apply a supplied closing artifact to an exact target.
pub fn closing_artifact_reduce(
    request: ClosingArtifactReduceRequest,
) -> Result<ClosingArtifactReduceResult, AppError> {
    closing::reduce_request(request)
}

fn validate_ingress(label: &str, source: &str) -> Result<(), AppError> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(AppError::limit(format!(
            "{label} has {} bytes, exceeding the {MAX_INPUT_BYTES}-byte application limit",
            source.len()
        )));
    }
    Ok(())
}
