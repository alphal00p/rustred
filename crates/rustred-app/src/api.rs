use crate::cli;
use crate::cli::error::AppError;

pub use crate::cli::args::{InputFormat, RelationSelection};

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

pub fn derive(request: DeriveRequest) -> Result<DeriveResult, AppError> {
    validate_ingress("derive input", &request.source)?;
    if request.n_cores == 0 {
        return Err(AppError::Input(
            "derive n_cores must be a positive integer".to_owned(),
        ));
    }
    cli::derive_request(request)
}

pub fn campaign_plan(request: CampaignPlanRequest) -> Result<CampaignPlanResult, AppError> {
    validate_ingress("campaign input", &request.source)?;
    cli::campaign::plan_request(request)
}

pub fn campaign_preflight(
    request: CampaignPreflightRequest,
) -> Result<CampaignPreflightResult, AppError> {
    validate_ingress("campaign execution resource profile", &request.profile)?;
    if request.n_cores == 0 {
        return Err(AppError::Input(
            "campaign preflight n_cores must be a positive integer".to_owned(),
        ));
    }
    if request.max_memory_bytes == 0 {
        return Err(AppError::Input(
            "campaign preflight max_memory_bytes must be positive".to_owned(),
        ));
    }
    cli::campaign_preflight::preflight_request(request)
}

fn validate_ingress(label: &str, source: &str) -> Result<(), AppError> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(AppError::Input(format!(
            "{label} has {} bytes, exceeding the {MAX_INPUT_BYTES}-byte application limit",
            source.len()
        )));
    }
    Ok(())
}
