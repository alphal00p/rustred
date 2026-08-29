//! Exact, topology-neutral verification of momentum and family maps.
//!
//! A caller supplies the exact momentum substitution
//!
//! ```text
//! l_source = A l_target + B p_target,
//! p_source = C p_target,
//! ```
//!
//! and [`verify`] derives and independently replays the induced scalar-product
//! and affine-denominator maps. Candidate generation is deliberately outside
//! this proof boundary: graph signatures, numerical samples, denominator
//! counts, and topology names are never accepted as proof.

mod condition;
mod error;
mod limits;
mod model;
pub mod permutation;
mod verify;

pub use condition::{ConditionSource, NonZeroCondition};
pub use error::Error;
pub use limits::{DEFAULT_MAX_MATRIX_ENTRIES, Limits, Stats};
pub use model::{
    CoefficientMatrix, DenominatorAction, DenominatorMap, Jacobian, MomentumMap, ScalarProductMap,
    VerifiedMap,
};
pub use verify::verify;
