use std::fmt;
use symbolica::utils::resources::ResourceLimitError;

/// A structural, resource, or native-decoder error at the binary I/O boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryIoError {
    Resource(ResourceLimitError),
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
            Self::Resource(error) => error.fmt(f),
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

impl std::error::Error for BinaryIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ResourceLimitError> for BinaryIoError {
    fn from(error: ResourceLimitError) -> Self {
        Self::Resource(error)
    }
}

pub(crate) fn resource_checkpoint() -> Result<(), BinaryIoError> {
    symbolica::utils::resources::checkpoint().map_err(BinaryIoError::Resource)
}

pub(crate) fn resource_allocation(bytes: usize) -> Result<(), BinaryIoError> {
    let bytes =
        u64::try_from(bytes).map_err(|_| BinaryIoError::Invalid("allocation bytes exceed u64"))?;
    symbolica::utils::resources::check_allocation(bytes).map_err(BinaryIoError::Resource)
}

/// Check representable allocation size before any reserve or native decode.
pub(crate) fn resource_array<T>(entries: usize) -> Result<(), BinaryIoError> {
    let bytes = entries
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(BinaryIoError::Invalid("allocation size overflow"))?;
    resource_allocation(bytes)
}

pub(crate) fn native_io_error(error: std::io::Error) -> BinaryIoError {
    if let Some(inner) = error.get_ref() {
        if let Some(error) = inner.downcast_ref::<ResourceLimitError>() {
            return BinaryIoError::Resource(error.clone());
        }
        if let Some(error) = inner.downcast_ref::<BinaryIoError>() {
            return error.clone();
        }
    }
    BinaryIoError::Native(error.to_string())
}

pub(crate) fn native_encode_error(error: bincode::error::EncodeError) -> BinaryIoError {
    match error {
        bincode::error::EncodeError::Io { inner, .. } => native_io_error(inner),
        error => BinaryIoError::Native(error.to_string()),
    }
}

pub(crate) fn native_decode_error(error: bincode::error::DecodeError) -> BinaryIoError {
    match error {
        bincode::error::DecodeError::Io { inner, .. } => native_io_error(inner),
        error => BinaryIoError::Native(error.to_string()),
    }
}

pub(crate) fn resource_failure(error: &dyn std::any::Any) -> Option<ResourceLimitError> {
    if let Some(error) = error.downcast_ref::<ResourceLimitError>() {
        return Some(error.clone());
    }
    if let Some(BinaryIoError::Resource(error)) = error.downcast_ref::<BinaryIoError>() {
        return Some(error.clone());
    }
    if let Some(crate::algebra::ExactAlgebraError::Resource(error)) =
        error.downcast_ref::<crate::algebra::ExactAlgebraError>()
    {
        return Some(error.clone());
    }
    None
}

pub(crate) fn native_error(error: impl fmt::Display + 'static) -> BinaryIoError {
    if let Some(error) = (&error as &dyn std::any::Any).downcast_ref::<BinaryIoError>() {
        return error.clone();
    }
    if let Some(error) = resource_failure(&error) {
        return BinaryIoError::Resource(error);
    }
    BinaryIoError::Native(error.to_string())
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), BinaryIoError> {
    resource_checkpoint()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use symbolica::utils::resources::{ResourceBudget, ResourceLimits};

    #[test]
    fn ambient_ram_failure_is_typed_even_under_nested_unbounded_scope() {
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(5),
                ..Default::default()
            },
            || Ok(4),
        );
        let result = budget.run(|| ResourceBudget::default().run(|| resource_allocation(2)));
        assert!(matches!(
            result,
            Err(BinaryIoError::Resource(ResourceLimitError::Memory {
                additional_bytes: 2,
                ..
            }))
        ));
    }

    #[test]
    fn allocation_size_overflow_is_structural() {
        assert!(matches!(
            resource_array::<u64>(usize::MAX),
            Err(BinaryIoError::Invalid("allocation size overflow"))
        ));
    }
}
