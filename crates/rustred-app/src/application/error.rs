use std::fmt;

/// Stable, transport-neutral classification of an application failure.
///
/// Frontends map these categories to their own presentation: the CLI chooses
/// an exit code and terminal label, while the future Python adapter will
/// choose an exception class.  The application layer deliberately carries no
/// path, terminal, process-exit, Python, or GIL policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppErrorKind {
    Input,
    Schema,
    Limit,
    Lowering,
    Derivation,
    Execution,
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
}

impl AppError {
    pub(crate) fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
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

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_kinds_do_not_require_message_parsing() {
        let error = AppError::lowering("bad affine family");
        assert_eq!(error.kind(), AppErrorKind::Lowering);
        assert_eq!(error.kind().as_str(), "lowering");
        assert_eq!(error.message(), "bad affine family");
        assert_eq!(error.to_string(), "bad affine family");
    }
}
