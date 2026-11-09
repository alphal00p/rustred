//! Typed failures of the p library (owner: M2-I0 rr-iface). Every gap is one
//! of these, never a silent zero (plan M2 section 2 rule 6).
use std::fmt;

use crate::family::IntegralKey;
use crate::reduction::ReductionError;
use crate::sector::symmetry::integral_transport;

use super::OwnerClassId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PError {
    /// The named M2 task has not landed in this build.
    NotImplemented(&'static str),
    Unsupported(String),
    MissingOwner {
        class: OwnerClassId,
        ledger: u32,
    },
    Uncovered {
        class: OwnerClassId,
        key: IntegralKey,
    },
    /// One request mixes loop counts.
    MixedDimension,
    /// A Gamma tag outside `|m| <= P_GAMMA_TAG_MAX_M`, `|e| <= 127`.
    TagOutOfRange,
    /// A coefficient context other than the library's shared `{d}`.
    ContextMismatch,
    VersionMismatch {
        name: String,
        found: String,
        expected: String,
    },
    MalformedLibrary(String),
    /// A presentation or edge map that breaks its lane contract.
    Contract(String),
    Transport(integral_transport::Error),
    Generation(String),
    Engine(ReductionError),
    /// A generation job ran out of its budget (M3-I1, plan M3 D-M3-4):
    /// `resource` is `"time"` or `"rss"`; nothing of the job is kept.
    BudgetExceeded {
        resource: &'static str,
        detail: String,
    },
}

impl fmt::Display for PError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented(task) => {
                write!(formatter, "not implemented in this build ({task})")
            }
            Self::Unsupported(detail) => write!(formatter, "unsupported: {detail}"),
            Self::MissingOwner { class, ledger } => write!(
                formatter,
                "owner class {:016x} has no program (ledger entry {ledger})",
                class.0
            ),
            Self::Uncovered { class, key } => write!(
                formatter,
                "owner class {:016x} does not cover {:?}",
                class.0,
                key.powers()
            ),
            Self::MixedDimension => {
                formatter.write_str("one p reduction request mixes loop counts")
            }
            Self::TagOutOfRange => {
                formatter.write_str("Gamma tag out of range (|m| <= 8, |e| <= 127, 16 dense slots)")
            }
            Self::ContextMismatch => {
                formatter.write_str("the p library requires one coefficient context {d}")
            }
            Self::VersionMismatch {
                name,
                found,
                expected,
            } => write!(
                formatter,
                "p library generated with {name}={found}, this RustRed has {expected}; \
                 regenerate with rustred p-package"
            ),
            Self::MalformedLibrary(detail) => write!(formatter, "malformed p library: {detail}"),
            Self::Contract(detail) => write!(formatter, "lane contract violated: {detail}"),
            Self::Transport(error) => write!(formatter, "transport: {error}"),
            Self::Generation(detail) => write!(formatter, "generation: {detail}"),
            Self::Engine(error) => write!(formatter, "engine: {error}"),
            Self::BudgetExceeded { resource, detail } => {
                write!(formatter, "{resource} budget exceeded: {detail}")
            }
        }
    }
}

impl std::error::Error for PError {}

impl From<ReductionError> for PError {
    fn from(error: ReductionError) -> Self {
        Self::Engine(error)
    }
}

impl From<integral_transport::Error> for PError {
    fn from(error: integral_transport::Error) -> Self {
        Self::Transport(error)
    }
}
