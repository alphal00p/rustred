//! Integer-only generated completed-Work pages and exhaustive exact lookup.
//! Decoding authenticates bytes/structure, not historical mathematical closure.
//! Trusted-generated baseline binding is explicit in the routed campaign API.
mod codec;
mod directory;
mod key;
mod resident;
#[cfg(test)]
mod tests;

pub use codec::{
    FRAME_LIMIT, GeneratedWorkPageRecord, PAGE_MAGIC, ValidatedWorkPage, decode_metadata_frame,
    encode_metadata_frame, validate_page_record, work_payload_bytes,
};
pub use directory::{
    GeneratedDirectoryLimits, GeneratedDirectoryStatistics, GeneratedLookup, GeneratedPageProvider,
    GeneratedPageReadUsage, TrustedGeneratedWorkDirectory, validate_directory,
};
pub use key::GeneratedWorkKey;
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
}
impl GeneratedWorkError {
    pub fn new(kind: GeneratedWorkErrorKind, detail: impl ToString) -> Self {
        Self {
            kind,
            detail: detail.to_string(),
        }
    }
}
impl fmt::Display for GeneratedWorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "generated Work {:?}: {}", self.kind, self.detail)
    }
}
impl std::error::Error for GeneratedWorkError {}
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
