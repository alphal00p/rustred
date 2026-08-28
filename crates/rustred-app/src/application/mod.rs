mod campaign {
    pub(super) mod plan;
    pub(super) mod preflight;
}
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
    InputFormat, ParseInputFormatError, ParseRelationSelectionError, RelationSelection,
};

/// Maximum UTF-8 source payload accepted by every in-process application API.
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum canonical TOML payload returned by every application API.
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;

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

fn validate_ingress(label: &str, source: &str) -> Result<(), AppError> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(AppError::limit(format!(
            "{label} has {} bytes, exceeding the {MAX_INPUT_BYTES}-byte application limit",
            source.len()
        )));
    }
    Ok(())
}
