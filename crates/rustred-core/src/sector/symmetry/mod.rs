//! Exact, topology-neutral verification of momentum and family maps.
//!
//! A caller supplies the exact momentum substitution
//!
//! ```text
//! l_source = A l_target + B p_target,
//! p_source = C p_target,
//! ```
//!
//! and [`verify()`] derives and independently replays the induced scalar-product
//! and affine-denominator maps. Candidate generation is deliberately outside
//! this proof boundary: graph signatures, numerical samples, denominator
//! counts, and topology names are never accepted as proof.
//!
//! For vacuum families, the sector-owned projector-graph `discovery` service
//! PROPOSES such maps from exact mass-colored canonical graphs, and
//! [`SectorSymmetryCensus`] turns the verified proposals into a public
//! deterministic sector partition, verified routes to representatives,
//! representative automorphisms, exact relation rows and recursive
//! normalization onto representative sectors. Every proposal still passes
//! [`verify()`] and [`integral_transport::compile_with_affine_constants`]
//! before use.

pub mod canonical;
mod census;
mod condition;
pub(crate) mod discovery;
mod error;
pub mod integral_transport;
mod limits;
mod model;
pub mod permutation;
mod verify;

pub use canonical::{
    Canonicalization, CanonicalizationLimits, Canonicalizer, DescendingCanonicalization,
    Error as CanonicalizationError, ExactOrbit, NoHarderWitness, OrbitImage, RoutingCoefficient,
    RoutingWitness,
};
#[cfg(test)]
pub(crate) use canonical::{CoordinatePriorityActionLimits, CoordinatePriorityQuotient};
pub use census::{
    DEFAULT_MAX_CENSUS_GROUP_ORDER, DEFAULT_MAX_NORMAL_FORM_TERMS,
    DEFAULT_MAX_NORMAL_FORM_TRANSPORTS, MAX_SUBSECTOR_CENSUS_ACTIVE_LINES, NormalizationLimits,
    RelationEndpoints, RepresentativePolicy, SectorSymmetryCensus, SectorSymmetryCensusControl,
    SectorSymmetryCensusError, SectorSymmetryCensusOptions, SectorSymmetryCensusProgress,
    SectorSymmetryCensusStatistics, SectorSymmetryClass, SectorSymmetryMember,
    SectorSymmetryNormalForm, SectorSymmetryRelation, SectorSymmetrySkip, SectorSymmetrySkipReason,
    SectorSymmetrySkipStage, ZeroSectorKind, sector_id_cmp,
};
pub use condition::{ConditionSource, NonZeroCondition};
pub use discovery::{CanonicalSupportKey, DiscoveryError, LineGeometrySkip, SupportSkipReason};
pub use error::Error;
pub use limits::{DEFAULT_MAX_MATRIX_ENTRIES, Limits, Stats};
pub use model::{
    CoefficientMatrix, DenominatorAction, DenominatorMap, Jacobian, MomentumMap, ScalarProductMap,
    VerifiedMap,
};
pub use verify::verify;
