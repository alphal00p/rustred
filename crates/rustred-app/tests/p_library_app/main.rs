//! The p-library tests of rustred-app: one binary, one module per M2 task
//! (each module file names its owner).
mod assemble;
mod basis;
mod cli;
mod custom;
mod eps;
mod iface4;
mod package;
mod perf;
mod twosided;

/// The app side of the frozen interfaces (plan M2 section 4), public path
/// `rustred_app::propagator`.
#[test]
fn the_frozen_signatures_hold() {
    use std::path::Path;
    use std::sync::Arc;

    use rustred::propagator::error::PError;
    use rustred::propagator::families::PFamiliesFile;
    use rustred::propagator::library::PLibrary;
    use rustred::propagator::presentation::Presentation;
    use rustred_app::propagator::{
        AssembleOptions, AssembledLibrary, CustomOwnerOptions, CustomOwnerPayload, CustomPayloads,
        PLibraryOptions, PLibrarySource, assemble_library, equivalent_p_libraries,
        generate_custom_owner, load_p_library, write_p_library,
    };

    let _: fn(
        &PFamiliesFile,
        u8,
        CustomPayloads,
        &AssembleOptions,
    ) -> Result<AssembledLibrary, PError> = assemble_library;
    let _: fn(&Presentation, &CustomOwnerOptions) -> Result<CustomOwnerPayload, PError> =
        generate_custom_owner;
    let _: fn(&AssembledLibrary, &Path) -> Result<(), PError> = write_p_library;
    let _: fn(PLibrarySource, &PLibraryOptions) -> Result<Arc<PLibrary>, PError> = load_p_library;
    let _: fn(&PLibrary, &PLibrary) -> Result<bool, PError> = equivalent_p_libraries;
}
