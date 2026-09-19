use std::fmt;

/// A structural, resource, or native-decoder error at the binary I/O boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryIoError {
    Invalid(&'static str),
    Limit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Allocation {
        resource: &'static str,
        requested: usize,
    },
    Native(String),
}

impl fmt::Display for BinaryIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(detail) => write!(f, "invalid RustRed binary program: {detail}"),
            Self::Limit {
                resource,
                requested,
                limit,
            } => {
                write!(
                    f,
                    "binary {resource} requires {requested}; limit is {limit}"
                )
            }
            Self::Allocation {
                resource,
                requested,
            } => {
                write!(
                    f,
                    "cannot allocate {requested} bytes/entries for binary {resource}"
                )
            }
            Self::Native(detail) => write!(f, "native Symbolica binary I/O: {detail}"),
        }
    }
}

impl std::error::Error for BinaryIoError {}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), BinaryIoError> {
    if requested > limit {
        Err(BinaryIoError::Limit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
