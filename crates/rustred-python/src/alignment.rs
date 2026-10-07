//! CPython's wasm32 object allocator provides eight-byte alignment. Rust-owned
//! over-aligned payloads must live behind a Box or Arc, not inline in a pyclass.
//! Keep this census in sync with `register_rustred_module` and its submodules.

macro_rules! check_classes {
    ($($class:ty),+ $(,)?) => {
        $(const _: () = assert!(
            std::mem::align_of::<$class>() <= 8,
            concat!(stringify!($class), " exceeds CPython wasm32 object alignment"),
        );)+
    };
}

check_classes!(
    crate::PyDeriveResult,
    crate::PyCampaignPlanResult,
    crate::PyCampaignPreflightResult,
    crate::PyClosingArtifactInspectionResult,
    crate::PyFoundryCampaignRunResult,
    crate::PyFoundryWaveCampaignRunResult,
    crate::PyClosingArtifactGenerationResult,
    crate::PyExactMasterCoefficient,
    crate::PyClosingArtifactReductionResult,
    crate::candidates::PyCandidateBundleResult,
    crate::candidates::PyCandidateGenerationRequest,
    crate::streaming::PyCandidateGenerationSession,
    crate::streaming::PyCandidateArtifact,
    crate::normalization::PyTerminalNormalization,
);
