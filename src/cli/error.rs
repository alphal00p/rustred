use std::fmt;

use crate::cli::args::ArgError;

#[derive(Debug)]
pub(crate) enum CliError {
    Usage(ArgError),
    InputIo(String),
    Input(String),
    Derivation(String),
    Serialization(String),
    OutputIo(String),
}

impl CliError {
    pub(crate) const fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::InputIo(_) => 3,
            Self::Input(_) => 4,
            Self::Derivation(_) => 5,
            Self::Serialization(_) => 6,
            Self::OutputIo(_) => 7,
        }
    }

    pub(crate) const fn category(&self) -> &'static str {
        match self {
            Self::Usage(_) => "usage",
            Self::InputIo(_) => "input-io",
            Self::Input(_) => "input",
            Self::Derivation(_) => "derivation",
            Self::Serialization(_) => "serialization",
            Self::OutputIo(_) => "output-io",
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(error) => error.fmt(formatter),
            Self::InputIo(message)
            | Self::Input(message)
            | Self::Derivation(message)
            | Self::Serialization(message)
            | Self::OutputIo(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CliError {}

impl From<ArgError> for CliError {
    fn from(value: ArgError) -> Self {
        Self::Usage(value)
    }
}
