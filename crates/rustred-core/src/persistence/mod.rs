//! Topology-independent native binary transport for generated RustRed programs.
//!
//! Candidate and certified payloads share framing, not mathematical authority.
//! Inspecting an envelope never constructs a closing artifact. Native Symbolica
//! import is explicitly a generated-data boundary, not a hostile-input parser.

mod atoms;
mod catalog;
mod compare;
mod envelope;
mod error;
mod family;
mod limits;
mod native;

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
pub use family::NativeFamilyRecord;
pub use limits::BinaryIoLimits;
