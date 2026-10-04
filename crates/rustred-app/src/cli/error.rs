use std::fmt;

use crate::{AppError, AppErrorKind};

use super::args::ArgError;

/// Failures owned by the command-line transport.
#[derive(Debug)]
pub(crate) enum CliError {
    Usage(ArgError),
    InputIo(String),
    Input(String),
    OutputIo(String),
    Application(AppError),
    /// An oracle's own verdict (not a failure of the command): FAIL exits 1,
    /// INCOMPLETE (no violation found, check not exhaustive) exits 9.
    Verdict {
        incomplete: bool,
        message: String,
    },
}

impl CliError {
    pub(crate) const fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::InputIo(_) => 3,
            Self::Input(_) => 4,
            Self::Application(error) => match error.kind() {
                AppErrorKind::Input
                | AppErrorKind::Schema
                | AppErrorKind::Limit
                | AppErrorKind::Lowering => 4,
                AppErrorKind::Derivation => 5,
                AppErrorKind::Serialization | AppErrorKind::OutputLimit => 6,
                AppErrorKind::Execution | AppErrorKind::Cancelled | AppErrorKind::License => 8,
                AppErrorKind::InternalInvariant => 70,
            },
            Self::OutputIo(_) => 7,
            Self::Verdict { incomplete, .. } => {
                if *incomplete {
                    9
                } else {
                    1
                }
            }
        }
    }

    pub(crate) const fn category(&self) -> &'static str {
        match self {
            Self::Usage(_) => "usage",
            Self::InputIo(_) => "input-io",
            Self::Input(_) => "input",
            Self::Application(error) => match error.kind() {
                AppErrorKind::Input
                | AppErrorKind::Schema
                | AppErrorKind::Limit
                | AppErrorKind::Lowering => "input",
                AppErrorKind::Derivation => "derivation",
                AppErrorKind::Serialization | AppErrorKind::OutputLimit => "serialization",
                AppErrorKind::Execution | AppErrorKind::Cancelled | AppErrorKind::License => "execution",
                AppErrorKind::InternalInvariant => "internal",
            },
            Self::OutputIo(_) => "output-io",
            Self::Verdict { .. } => "verdict",
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(error) => error.fmt(formatter),
            Self::InputIo(message)
            | Self::Input(message)
            | Self::OutputIo(message)
            | Self::Verdict { message, .. } => formatter.write_str(message),
            Self::Application(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CliError {}

impl From<ArgError> for CliError {
    fn from(value: ArgError) -> Self {
        Self::Usage(value)
    }
}

impl From<AppError> for CliError {
    fn from(value: AppError) -> Self {
        Self::Application(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_categories_keep_the_stable_cli_exit_contract() {
        let cases = [
            (AppErrorKind::Input, "input", "input", 4),
            (AppErrorKind::Schema, "schema", "input", 4),
            (AppErrorKind::Limit, "limit", "input", 4),
            (AppErrorKind::Lowering, "lowering", "input", 4),
            (AppErrorKind::Derivation, "derivation", "derivation", 5),
            (AppErrorKind::Execution, "execution", "execution", 8),
            (AppErrorKind::License, "license", "execution", 8),
            (
                AppErrorKind::Serialization,
                "serialization",
                "serialization",
                6,
            ),
            (
                AppErrorKind::OutputLimit,
                "output-limit",
                "serialization",
                6,
            ),
            (
                AppErrorKind::InternalInvariant,
                "internal-invariant",
                "internal",
                70,
            ),
        ];

        for (kind, stable_kind, category, exit_code) in cases {
            let error = CliError::from(AppError::new(kind, "message"));
            assert_eq!(kind.as_str(), stable_kind);
            assert_eq!(error.category(), category);
            assert_eq!(error.exit_code(), exit_code);
        }
    }
}
