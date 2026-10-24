use std::fmt;
use symbolica::utils::resources::ResourceLimitError;

/// Stable, transport-neutral classification of an application failure.
///
/// Frontends map these categories to their own presentation: the CLI chooses
/// an exit code and terminal label, while the Python adapter chooses an
/// exception class. Errors carry no frontend-specific terminal, process-exit,
/// Python, or GIL policy; filesystem failures retain their path context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppErrorKind {
    Input,
    Schema,
    Limit,
    Lowering,
    Derivation,
    Execution,
    /// A valid native candidate request reached a point with no applicable rule.
    UnresolvedReduction,
    /// The request exceeds the candidate's recorded starting numerator rank.
    /// Structural root/arity errors and source-condition failures are distinct.
    OutsideReductionScope,
    Cancelled,
    License,
    Serialization,
    OutputLimit,
    InternalInvariant,
}

impl AppErrorKind {
    /// Stable identifier intended for frontend mappings and diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Schema => "schema",
            Self::Limit => "limit",
            Self::Lowering => "lowering",
            Self::Derivation => "derivation",
            Self::Execution => "execution",
            Self::UnresolvedReduction => "unresolved-reduction",
            Self::OutsideReductionScope => "outside-reduction-scope",
            Self::Cancelled => "cancelled",
            Self::License => "license",
            Self::Serialization => "serialization",
            Self::OutputLimit => "output-limit",
            Self::InternalInvariant => "internal-invariant",
        }
    }
}

/// An owned application failure which can cross a thread or FFI boundary.
///
/// The message is deterministic context for the current failure.  Consumers
/// should branch on [`Self::kind`] rather than parsing this text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppError {
    kind: AppErrorKind,
    message: String,
    resource: Option<ResourceLimitError>,
}

impl AppError {
    /// Construct a typed failure at an external native-backend boundary.
    pub fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            resource: None,
        }
    }

    /// Preserve the original process-memory or elapsed-time failure. An
    /// allocation preflight may fail while the current RSS is below its limit,
    /// so callers must not infer this cause by checking the budget again.
    pub fn resource(error: ResourceLimitError) -> Self {
        Self {
            kind: AppErrorKind::Limit,
            message: error.to_string(),
            resource: Some(error),
        }
    }

    pub fn resource_limit_error(&self) -> Option<&ResourceLimitError> {
        self.resource.as_ref()
    }

    /// Keep a transient resource cause when adding an application boundary.
    pub(crate) fn from_native(
        error: impl std::error::Error + 'static,
        fallback: AppErrorKind,
    ) -> Self {
        if let Some(resource) = Self::resource_cause(&error) {
            return Self {
                kind: AppErrorKind::Limit,
                message: error.to_string(),
                resource: Some(resource),
            };
        }
        Self::new(fallback, error.to_string())
    }

    /// Extract the original cause before a native inspection is formatted into
    /// a report. Resource refusal cannot certify an ordinary diagnostic failure.
    pub(crate) fn resource_cause(
        error: &(dyn std::error::Error + 'static),
    ) -> Option<ResourceLimitError> {
        let mut source = Some(error);
        while let Some(error) = source {
            if let Some(resource) = error.downcast_ref::<ResourceLimitError>() {
                return Some(resource.clone());
            }
            source = error
                .downcast_ref::<std::io::Error>()
                .and_then(std::io::Error::get_ref)
                .map(|inner| inner as &(dyn std::error::Error + 'static))
                .or_else(|| error.source());
        }
        None
    }

    pub(crate) fn input(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Input, message)
    }

    pub(crate) fn schema(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Schema, message)
    }

    pub(crate) fn limit(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Limit, message)
    }

    pub(crate) fn lowering(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Lowering, message)
    }

    pub(crate) fn derivation(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Derivation, message)
    }

    pub(crate) fn execution(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Execution, message)
    }

    pub(crate) fn license(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::License, message)
    }

    pub(crate) fn serialization(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Serialization, message)
    }

    pub(crate) fn output_limit(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::OutputLimit, message)
    }

    pub(crate) fn internal_invariant(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::InternalInvariant, message)
    }

    pub const fn kind(&self) -> AppErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_message(self) -> String {
        self.message
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.resource.as_ref().map(|error| error as _)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_wrapped_resource_leaves_keep_the_original_cause() {
        for resource in [
            ResourceLimitError::Memory {
                rss_bytes: 9,
                additional_bytes: 8,
                limit_bytes: 10,
            },
            ResourceLimitError::Elapsed {
                elapsed: std::time::Duration::from_secs(2),
                limit: std::time::Duration::from_secs(1),
            },
        ] {
            let wrapped = std::io::Error::other(std::io::Error::other(resource.clone()));
            let error = AppError::from_native(wrapped, AppErrorKind::Schema);
            assert_eq!(error.kind(), AppErrorKind::Limit);
            assert_eq!(error.resource_limit_error(), Some(&resource));
        }
        let ordinary = AppError::from_native(
            std::io::Error::other("ordinary corruption"),
            AppErrorKind::Schema,
        );
        assert_eq!(ordinary.kind(), AppErrorKind::Schema);
        assert_eq!(ordinary.resource_limit_error(), None);
    }

    #[test]
    fn stable_kinds_do_not_require_message_parsing() {
        let error = AppError::lowering("bad affine family");
        assert_eq!(error.kind(), AppErrorKind::Lowering);
        assert_eq!(error.kind().as_str(), "lowering");
        assert_eq!(error.message(), "bad affine family");
        assert_eq!(error.to_string(), "bad affine family");
    }
}
