use std::fmt;

use crate::cli::args::ArgError;

#[derive(Debug)]
pub enum AppError {
    Usage(ArgError),
    InputIo(String),
    Input(String),
    Derivation(String),
    Serialization(String),
    OutputIo(String),
    Execution(String),
}

impl AppError {
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::InputIo(_) => 3,
            Self::Input(_) => 4,
            Self::Derivation(_) => 5,
            Self::Serialization(_) => 6,
            Self::OutputIo(_) => 7,
            Self::Execution(_) => 8,
        }
    }

    pub const fn category(&self) -> &'static str {
        match self {
            Self::Usage(_) => "usage",
            Self::InputIo(_) => "input-io",
            Self::Input(_) => "input",
            Self::Derivation(_) => "derivation",
            Self::Serialization(_) => "serialization",
            Self::OutputIo(_) => "output-io",
            Self::Execution(_) => "execution",
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(error) => error.fmt(formatter),
            Self::InputIo(message)
            | Self::Input(message)
            | Self::Derivation(message)
            | Self::Serialization(message)
            | Self::OutputIo(message)
            | Self::Execution(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AppError {}

impl From<ArgError> for AppError {
    fn from(value: ArgError) -> Self {
        Self::Usage(value)
    }
}
