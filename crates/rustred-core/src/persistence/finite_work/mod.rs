//! Integer-only generated completed-Work pages and exhaustive exact lookup.
//! Decoding authenticates bytes/structure, not historical mathematical closure.
//! Trusted-generated baseline binding is explicit in the routed campaign API.
mod codec;
mod directory;
mod key;
mod progress;
mod resident;
#[cfg(test)]
mod tests;

pub use codec::{
    FRAME_LIMIT, GeneratedWorkPageRecord, PAGE_MAGIC, ValidatedWorkPage, decode_metadata_frame,
    decode_metadata_frame_with_allocation_limit, encode_metadata_frame,
    encode_metadata_frame_repeatable, metadata_decode_allowance, validate_page_record,
    validate_page_record_with_diagnostics, work_payload_bytes,
};
pub use directory::{
    GeneratedDirectoryLimits, GeneratedDirectoryStatistics, GeneratedLookup, GeneratedPageProvider,
    GeneratedPageReadUsage, TrustedGeneratedWorkDirectory, directory_payload_bytes,
    validate_directory,
};
pub use key::GeneratedWorkKey;
pub use progress::{
    GeneratedWorkProgress, GeneratedWorkProgressReporter, GeneratedWorkProgressSnapshot,
};
pub use resident::{PreparedResidentWork, ResidentWorkStatistics, TrustedGeneratedResidentWork};
use std::fmt;

pub type GeneratedWorkDigest = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedWorkErrorKind {
    Corrupt,
    Resource,
    Io,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedWorkError {
    pub kind: GeneratedWorkErrorKind,
    detail: String,
    resource_limit: Option<symbolica::utils::resources::ResourceLimitError>,
}
impl GeneratedWorkError {
    pub fn new(kind: GeneratedWorkErrorKind, detail: impl ToString) -> Self {
        Self {
            kind,
            detail: detail.to_string(),
            resource_limit: None,
        }
    }
    pub fn resource_limit(&self) -> Option<&symbolica::utils::resources::ResourceLimitError> {
        self.resource_limit.as_ref()
    }
}
impl fmt::Display for GeneratedWorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "generated Work {:?}: {}", self.kind, self.detail)
    }
}
impl std::error::Error for GeneratedWorkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.resource_limit.as_ref().map(|error| error as _)
    }
}
impl From<symbolica::utils::resources::ResourceLimitError> for GeneratedWorkError {
    fn from(error: symbolica::utils::resources::ResourceLimitError) -> Self {
        Self {
            kind: GeneratedWorkErrorKind::Resource,
            detail: error.to_string(),
            resource_limit: Some(error),
        }
    }
}
fn checkpoint() -> GeneratedWorkResult<()> {
    symbolica::utils::resources::checkpoint().map_err(GeneratedWorkError::from)
}
fn allocation(bytes: usize) -> GeneratedWorkResult<()> {
    let bytes = u64::try_from(bytes).map_err(|_| resource("allocation bytes exceed u64"))?;
    symbolica::utils::resources::check_allocation(bytes).map_err(GeneratedWorkError::from)
}
fn binary_resource(error: super::BinaryIoError) -> GeneratedWorkError {
    match error {
        super::BinaryIoError::Resource(error) => error.into(),
        error => resource(error),
    }
}
fn io_resource(error: std::io::Error) -> GeneratedWorkError {
    if let Some(inner) = error
        .get_ref()
        .and_then(|error| error.downcast_ref::<GeneratedWorkError>())
    {
        return inner.clone();
    }
    binary_resource(super::native_io_error(error))
}
fn encode_resource(error: bincode::error::EncodeError) -> GeneratedWorkError {
    match error {
        bincode::error::EncodeError::Io { inner, .. } => io_resource(inner),
        error => resource(error),
    }
}
pub type GeneratedWorkResult<T> = Result<T, GeneratedWorkError>;
fn corrupt(detail: impl ToString) -> GeneratedWorkError {
    GeneratedWorkError::new(GeneratedWorkErrorKind::Corrupt, detail)
}
fn resource(detail: impl ToString) -> GeneratedWorkError {
    GeneratedWorkError::new(GeneratedWorkErrorKind::Resource, detail)
}

/// An authenticated loader must compute this from the exact immutable source
/// payloads/selection/dispatch convention it actually prepares. This data alone
/// does not authenticate a reducer or prove that a prior graph was completed.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct GeneratedExecutionBinding {
    pub physical_arity: u32,
    pub program_identity: GeneratedWorkDigest,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct GeneratedObjectRef {
    pub digest: GeneratedWorkDigest,
    pub bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct GeneratedWorkPageRef {
    pub object: GeneratedObjectRef,
    pub records: u64,
    pub first: GeneratedWorkKey,
    pub last: GeneratedWorkKey,
}
