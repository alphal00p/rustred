//! The lane-program factory shared by assembly and the loader (signature:
//! M2-I0 rr-iface; body: M2-T8a rr-assemble).
//!
//! [`lane_program`] rebuilds the program of one owner from what a library
//! persists: lane, roles, edge maps and, for a Custom owner, its payload and
//! meta. Nothing is guessed: a lane without a program, a payload on a rule lane
//! or a Custom owner without one is [`PError::MalformedLibrary`], and the false
//! knock-out guard is re-checked before a Custom payload is read.
use std::fmt;
use std::sync::Arc;

use super::carpet::CarpetStep;
use super::descent::check_custom_guard;
use super::error::PError;
use super::insertion::InsertionStep;
use super::lane::OwnerLane;
use super::library::{OwnerProgram, PLibrary, POwnerSpec};
use super::series_parallel::SeriesParallelStep;
use super::step::PLaneStep;
use super::triangle::TriangleStep;

/// The bytes of a Custom owner program, in memory or on first use.
#[derive(Clone)]
pub enum CustomPayloadRef {
    Bytes(Arc<[u8]>),
    Lazy(Arc<dyn Fn() -> Result<Arc<[u8]>, PError> + Send + Sync>),
}

impl CustomPayloadRef {
    pub fn bytes(&self) -> Result<Arc<[u8]>, PError> {
        match self {
            Self::Bytes(bytes) => Ok(bytes.clone()),
            Self::Lazy(load) => load(),
        }
    }
}

impl fmt::Debug for CustomPayloadRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bytes(bytes) => write!(formatter, "Bytes({} bytes)", bytes.len()),
            Self::Lazy(_) => formatter.write_str("Lazy"),
        }
    }
}

/// The program of `owner` in `library`: the step of its lane, built from the
/// owner's roles and edge maps (Insertion, SeriesParallel, Carpet: M2-T4;
/// Triangle: M2-T5), or the Custom step of its payload (M2-T7c). The trivial
/// node is no owner: steps reach it as the terminal `Unit`.
///
/// A rule lane carries no payload and no Custom meta; a Custom owner carries
/// both and passes the false knock-out guard. `Zero`, `Diamond` (M3) and
/// `Product` own no program in M2.
pub fn lane_program(library: &PLibrary, owner: &POwnerSpec) -> Result<OwnerProgram, PError> {
    let malformed = |detail: String| {
        PError::MalformedLibrary(format!(
            "owner {:016x} on presentation {}: {detail}",
            owner.class.0, owner.presentation.0
        ))
    };
    if owner.lane != OwnerLane::Custom && (owner.payload.is_some() || owner.custom.is_some()) {
        return Err(malformed(format!(
            "the lane {:?} carries a Custom payload",
            owner.lane
        )));
    }
    let step: Arc<dyn PLaneStep> = match owner.lane {
        OwnerLane::SeriesParallel => Arc::new(SeriesParallelStep::from_owner(library, owner)?),
        OwnerLane::Insertion => Arc::new(InsertionStep::from_owner(library, owner)?),
        OwnerLane::Carpet => Arc::new(CarpetStep::from_owner(library, owner)?),
        OwnerLane::Triangle => Arc::new(TriangleStep::from_owner(library, owner)?),
        OwnerLane::Custom => custom_step(library, owner)?,
        lane @ (OwnerLane::Zero | OwnerLane::Diamond | OwnerLane::Product) => {
            return Err(malformed(format!("the lane {lane:?} has no program")));
        }
    };
    if step.lane() != owner.lane {
        return Err(malformed(format!(
            "the program of lane {:?} answers {:?}",
            owner.lane,
            step.lane()
        )));
    }
    Ok(OwnerProgram::Lane(step))
}

/// The Custom step of `owner`: guard first, then the payload.
fn custom_step(library: &PLibrary, owner: &POwnerSpec) -> Result<Arc<dyn PLaneStep>, PError> {
    let presentation = library
        .presentations()
        .get(owner.presentation.0 as usize)
        .ok_or_else(|| {
            PError::MalformedLibrary(format!(
                "a Custom owner on the unknown presentation {}",
                owner.presentation.0
            ))
        })?;
    check_custom_guard(library, owner)?;
    let (Some(payload), Some(_meta)) = (&owner.payload, &owner.custom) else {
        return Err(PError::MalformedLibrary(format!(
            "the Custom owner {:016x} has no payload",
            owner.class.0
        )));
    };
    let _bytes = payload.bytes()?;
    // `custom::build_custom_step(presentation, &bytes, meta)` arrives with
    // M2-T7c; until then a Custom owner fails closed.
    let _ = presentation;
    Err(PError::NotImplemented("M2-T7c"))
}
