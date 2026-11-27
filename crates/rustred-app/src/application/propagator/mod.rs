//! The p library on disk and in vakint: assembly, format, package, loader and
//! custom-owner generation (plan M2 section 4; owner of this file: M2-I0
//! rr-iface). Each submodule names its owner task.
use std::collections::BTreeMap;
use std::path::PathBuf;

pub mod assemble;
pub mod content;
pub mod eps_census;
pub mod format;
pub mod generate;
pub mod loader;
pub mod ordering;
pub mod package;
pub mod seed;

pub use assemble::{AssembleOptions, AssembledLibrary, assemble_library};
pub use content::{P_CONTENT_CHECK_ENV, P_CONTENT_FILE, PContentCheck, PContentManifest};
pub use generate::{CustomOwnerOptions, CustomOwnerPayload, generate_custom_owner};
pub use loader::{load_p_library, load_p_library_checked};
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
        /// vk5l WP6: Custom nodes declared NOT CLOSED, by graph key: (node
        /// name, status). Their owner has no payload (`OwnerProgram::Missing`)
        /// and the package records them in `library.json` `open_nodes`; a key
        /// reaching one is `PError::NodeNotClosed`. A node both imported and
        /// open is refused.
        open: BTreeMap<Vec<u8>, (String, String)>,
    },
}

/// One imported Custom node: the presentation its payload is bound to (its id
/// and node reference are set by assembly), the payload and its record. The
/// payload may stay on disk (`CustomPayloadRef::File`, p5l-library session 2):
/// assembly then reads it once for its residuals, the package writer copies
/// it, and its program is built on use.
#[derive(Clone, Debug)]
pub struct ImportedCustomNode {
    pub presentation: rustred::propagator::presentation::Presentation,
    pub payload: rustred::propagator::programs::CustomPayloadRef,
    pub meta: rustred::propagator::library::CustomOwnerMeta,
}
