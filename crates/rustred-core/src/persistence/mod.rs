//! Topology-independent native binary transport for generated RustRed programs.
//!
//! Candidate and certified payloads share framing, not mathematical authority.
//! Inspecting an envelope never constructs a closing artifact. Native Symbolica
//! import is explicitly a generated-data boundary, not a hostile-input parser.

pub mod application_cache;
mod atoms;
mod bytes;
mod catalog;
mod compare;
mod envelope;
mod error;
mod family;
pub mod finite_work;
mod lazy;
mod limits;
mod native;
mod structure;
pub(crate) mod terminal_normalization;

pub use atoms::{
    CoefficientId, CoefficientTableBuilder, DecodedCoefficientTable, EncodedCoefficientTable,
};
pub use catalog::{ExactTerminalCatalog, TerminalCatalogCoverage};
pub use compare::equivalent_generated_programs;
pub(crate) use compare::same_native_coefficient;
pub use envelope::{
    BINARY_PROGRAM_VERSION, BinaryProgramKind, BinarySection, ProgramEnvelope, SectionTag,
    encode_program, inspect_program,
};
pub use error::BinaryIoError;
pub(crate) use error::{
    native_encode_error, native_io_error, resource_allocation, resource_array, resource_checkpoint,
};
pub(crate) use error::{native_error, resource_failure};
pub use family::NativeFamilyRecord;
pub use lazy::LazyDecodedCoefficientTable;
pub use limits::BinaryIoLimits;
pub(crate) use structure::{decode_structure, encode_structure};
