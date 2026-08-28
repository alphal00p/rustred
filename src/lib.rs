//! RustRed: a pure-Rust, Symbolica-backed port of LiteRed-style parametric IBP
//! derivation and reduction.
//!
//! The generic production path is loop-count and topology independent:
//! [`IntegralFamily`] authenticates a complete affine scalar-product basis and
//! [`ParametricIbpGenerator`] derives reusable ordinary and Lorentz-invariance
//! identities over the exact field `K(n)`. Loop/topology-authored recurrences
//! are not part of the generic production crate and are not sources of generic
//! parametric identities or future discovered rules.

pub mod algebra;
pub mod campaign;
pub mod family;
pub mod identity;
pub mod parametric_ibp;
pub mod sectors;
pub mod symbolica_affine_denominator;
pub mod symbolica_integral_input;
pub mod symmetry;
pub mod symmetry_discovery;
pub mod zero_sectors;

pub use algebra::{
    CoefficientPolynomial, IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficient,
    IndexedCoefficientContext, IndexedPolynomial,
};
pub use campaign::{ParallelExecution, ParallelExecutionError};
pub use family::isp::{
    ISP_COMPLETION_V2_SCHEMA, IspCompletion, IspCompletionError, IspCompletionLimits,
    IspCompletionStats,
};
pub use family::symanzik::{
    FeynmanPolynomial, FeynmanPolynomialContext, FeynmanPolynomialError, FeynmanPolynomialLimits,
    RawFeynmanPolynomial, SymanzikPolynomials,
};
pub use family::{
    AffineDenominator, CoefficientLocation, ContractionMomentum, DenominatorExpansion,
    FamilyDomain, FamilyNonZeroCondition, IntegralFamily, IntegralFamilyError,
    IntegralFamilyFingerprintStats, IntegralFamilyLimits, IntegralKey, IntegralKeyError,
    ScalarProductCoordinate,
};
pub use parametric_ibp::{
    ParametricIbpConfig, ParametricIbpError, ParametricIbpGenerator, ParametricIbpRelations,
};
pub use sectors::{
    CutConstraint, IntegralComplexityComponent, IntegralComplexityKey, IntegralOrderingPolicy,
    RUSTRED_UNSHIFTED_ORDER_V1_ID, RUSTRED_UNSHIFTED_ORDER_V1_SCHEMA, SectorAnalysisStatus,
    SectorExclusion, SectorFoundationError, SectorMask, SectorPattern, SectorPatternMismatch,
    SectorPatternSlot, SectorRestrictions, StrictDescentWitness,
};
pub use symbolica_affine_denominator::{
    CompiledSymbolicaAffineDenominator, SYMBOLICA_AFFINE_DENOMINATOR_V1_SCHEMA,
    SymbolicaAffineDenominatorCompiler, SymbolicaAffineDenominatorError,
    SymbolicaAffineDenominatorLimits, SymbolicaAffineDenominatorStats,
};
pub use symbolica_integral_input::{
    ExternalGramInputV1, LoweredSymbolicaDenominatorV1, LoweredSymbolicaProjectV1,
    NormalizedProjectInputV1, NormalizedProjectPartsV1, NormalizedProjectSourceV1,
    NormalizedPropagatorV1, NormalizedTargetV1, ParameterSourceV1, PropagatorInputV1,
    RUSTRED_LOWERED_SYMBOLICA_PROJECT_V1_SCHEMA, RUSTRED_PROJECT_TOML_V1_SCHEMA,
    RUSTRED_SYMBOLICA_INTEGRAL_V1_SCHEMA, SymbolicaIntegralInputCompiler,
    SymbolicaIntegralInputError, SymbolicaIntegralInputLimits, SymbolicaIntegralInputStats,
    SymbolicaProjectLoweringError, SymbolicaProjectLoweringLimits, TextExternalGramInputV1,
    TextProjectPartsV1, TextPropagatorInputV1,
};
pub use symmetry::{
    AFFINE_FAMILY_MAP_V2_SCHEMA, AffineDenominatorMap, AffineScalarProductMap,
    DenominatorRowAction, ExactMatrix, JacobianWitness, MomentumMap, SymmetryVerificationError,
    SymmetryVerificationLimits, SymmetryVerificationStats, VerifiedAffineFamilyMap,
    verify_affine_family_map,
};
pub use symmetry_discovery::{
    INTERNAL_FAMILY_PERMUTATION_SYMMETRY_V1_SCHEMA, InternalSymmetryCompatibilityError,
    InternalSymmetryKeyTransportError, InternalSymmetryReplayError,
    VerifiedInternalFamilyPermutationSymmetry, compile_internal_family_permutation_symmetry,
};
pub use zero_sectors::{
    FullColumnRankWitness, PowerShiftPolicy, ZERO_SECTOR_CERTIFICATE_SCHEMA, ZeroSectorAnalyzer,
    ZeroSectorCertificate, ZeroSectorConditionSource, ZeroSectorDecision, ZeroSectorDomain,
    ZeroSectorDomainCondition, ZeroSectorError, ZeroSectorLimits, ZeroSectorResource,
};
