//! RustRed: pure-Rust, Symbolica-backed parametric IBP and LI derivation.
//!
//! The generic production path is loop-count and topology independent:
//! [`IntegralFamily`] authenticates a complete affine scalar-product basis and
//! [`identity::ParametricIbpGenerator`] derives reusable ordinary and
//! Lorentz-invariance identities over the exact field `K(n)`. Loop/topology-
//! authored recurrences are not part of the generic production crate and are
//! not sources of generic parametric identities or future discovered rules.

pub mod algebra;
pub mod campaign;
pub mod family;
pub mod identity;
pub mod input;
pub mod sector;
pub mod symmetry_discovery;
pub mod zero_sectors;

mod symbolica_affine_denominator;

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
