use std::fmt;

/// Typed failure of support-only requested-domain ingress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RequestedDomainSupportError {
    EmptyProposalBatch,
    EmptyParentSupport,
    EmptyIdentity {
        object: &'static str,
    },
    WrongArity {
        object: &'static str,
        expected: usize,
        actual: usize,
    },
    Noncanonical {
        object: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for RequestedDomainSupportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProposalBatch => {
                formatter.write_str("requested-domain support proposal batch is empty")
            }
            Self::EmptyParentSupport => {
                formatter.write_str("requested-domain parent support is empty")
            }
            Self::EmptyIdentity { object } => {
                write!(formatter, "requested-domain support {object} is empty")
            }
            Self::WrongArity {
                object,
                expected,
                actual,
            } => write!(
                formatter,
                "requested-domain support {object} has arity {actual}, expected {expected}"
            ),
            Self::Noncanonical { object } => write!(
                formatter,
                "requested-domain support {object} is not strictly ordered and duplicate-free"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "requested-domain support {resource} overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "requested-domain support {resource} needs {requested}, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for requested-domain support {resource}"
            ),
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "requested-domain support invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for RequestedDomainSupportError {}
