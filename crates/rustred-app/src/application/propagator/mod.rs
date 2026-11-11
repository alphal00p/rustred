//! The p library on disk and in vakint: assembly, format, package, loader and
//! custom-owner generation (plan M2 section 4; owner of this file: M2-I0
//! rr-iface). Each submodule names its owner task.
use std::collections::BTreeMap;
use std::path::PathBuf;

pub mod assemble;
pub mod format;
pub mod generate;
pub mod loader;
pub mod ordering;
pub mod package;

pub use assemble::{AssembleOptions, AssembledLibrary, assemble_library};
pub use generate::{CustomOwnerOptions, CustomOwnerPayload, generate_custom_owner};
pub use loader::load_p_library;
pub use package::{equivalent_p_libraries, write_p_library};

/// Where a written library lives: a directory, or embedded `(name, bytes)` files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PLibrarySource {
    Directory(PathBuf),
    Memory(&'static [(&'static str, &'static [u8])]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PLibraryOptions {
    pub verify_descent: bool,
}

/// The Custom owners of an assembly: generated, or reused verbatim by class label.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // one value per assembly
pub enum CustomPayloads {
    Generate(CustomOwnerOptions),
    Provided(BTreeMap<String, CustomOwnerPayload>),
    /// `Provided` payloads by class label, plus imported Custom nodes by the
    /// graph key of their DAG node (p5l-library): a Custom node with an import
    /// is presented in the import's own presentation (kind `Caller`, the
    /// table's family, `p-import-table`) instead of `present_node`'s, and its
    /// owner takes the import's payload.
    Imported {
        provided: BTreeMap<String, CustomOwnerPayload>,
        imported: BTreeMap<Vec<u8>, ImportedCustomNode>,
    },
}

/// One imported Custom node: the presentation its payload is bound to (its id
/// and node reference are set by assembly) and the payload.
#[derive(Clone, Debug)]
pub struct ImportedCustomNode {
    pub presentation: rustred::propagator::presentation::Presentation,
    pub payload: CustomOwnerPayload,
}
