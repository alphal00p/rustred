//! The lane-program factory shared by assembly and the loader (signature:
//! M2-I0 rr-iface; body: M2-T8a rr-assemble).
use std::fmt;
use std::sync::Arc;

use super::error::PError;
use super::library::{OwnerProgram, PLibrary, POwnerSpec};

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

pub fn lane_program(_library: &PLibrary, _owner: &POwnerSpec) -> Result<OwnerProgram, PError> {
    Err(PError::NotImplemented("M2-T8a"))
}
