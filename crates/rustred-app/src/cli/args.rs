mod campaign;
mod derive;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use crate::{InputFormat, RelationSelection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamPath {
    Stdio,
    File(PathBuf),
}

impl StreamPath {
    fn parse(value: OsString) -> Result<Self, ArgError> {
        if value == "-" {
            Ok(Self::Stdio)
        } else if value.is_empty() {
            Err(ArgError::EmptyPath)
        } else {
            Ok(Self::File(PathBuf::from(value)))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeriveArgs {
    pub(crate) input: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) input_format: InputFormat,
    pub(crate) relations: RelationSelection,
    pub(crate) n_cores: usize,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignPlanArgs {
    pub(crate) input: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) input_format: InputFormat,
    pub(crate) root_id: Option<String>,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignPreflightArgs {
    pub(crate) profile: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) n_cores: usize,
    pub(crate) max_memory_bytes: u64,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Derive(DeriveArgs),
    CampaignPlan(CampaignPlanArgs),
    CampaignPreflight(CampaignPreflightArgs),
    Help,
    Version,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgError {
    NonUtf8Option(OsString),
    MissingCommand,
    MissingSubcommand(&'static str),
    UnknownCommand(String),
    UnknownSubcommand {
        command: &'static str,
        subcommand: String,
    },
    UnknownOption(String),
    DuplicateOption(&'static str),
    MissingValue(&'static str),
    MissingRequiredOption(&'static str),
    UnexpectedArgument(String),
    InvalidValue {
        option: &'static str,
        value: String,
        expected: &'static str,
    },
    EmptyPath,
}

impl fmt::Display for ArgError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8Option(value) => {
                write!(formatter, "command-line option is not UTF-8: {value:?}")
            }
            Self::MissingCommand => {
                formatter.write_str("missing command; expected `derive` or `campaign`")
            }
            Self::MissingSubcommand(command) => write!(
                formatter,
                "missing {command} subcommand; expected `plan` or `preflight`"
            ),
            Self::UnknownCommand(command) => write!(formatter, "unknown command {command:?}"),
            Self::UnknownSubcommand {
                command,
                subcommand,
            } => write!(
                formatter,
                "unknown {command} subcommand {subcommand:?}; expected `plan` or `preflight`"
            ),
            Self::UnknownOption(option) => write!(formatter, "unknown option {option:?}"),
            Self::DuplicateOption(option) => {
                write!(formatter, "option {option} was supplied twice")
            }
            Self::MissingValue(option) => write!(formatter, "option {option} needs a value"),
            Self::MissingRequiredOption(option) => {
                write!(formatter, "required option {option} was not supplied")
            }
            Self::UnexpectedArgument(argument) => {
                write!(formatter, "unexpected positional argument {argument:?}")
            }
            Self::InvalidValue {
                option,
                value,
                expected,
            } => write!(
                formatter,
                "invalid value {value:?} for {option}; expected {expected}"
            ),
            Self::EmptyPath => formatter.write_str("an input or output path cannot be empty"),
        }
    }
}

impl std::error::Error for ArgError {}

pub(crate) fn parse_args(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Command, ArgError> {
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        return Err(ArgError::MissingCommand);
    };
    let command = command.into_string().map_err(ArgError::NonUtf8Option)?;
    match command.as_str() {
        "--help" | "-h" | "help" => {
            reject_trailing(arguments)?;
            Ok(Command::Help)
        }
        "--version" | "-V" => {
            reject_trailing(arguments)?;
            Ok(Command::Version)
        }
        "derive" => derive::parse(arguments),
        "campaign" => campaign::parse(arguments),
        _ => Err(ArgError::UnknownCommand(command)),
    }
}

fn reject_trailing(arguments: impl IntoIterator<Item = OsString>) -> Result<(), ArgError> {
    if let Some(argument) = arguments.into_iter().next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        Err(ArgError::UnexpectedArgument(argument))
    } else {
        Ok(())
    }
}

fn next_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &'static str,
) -> Result<OsString, ArgError> {
    arguments.next().ok_or(ArgError::MissingValue(option))
}

fn next_utf8_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &'static str,
) -> Result<String, ArgError> {
    next_value(arguments, option)?
        .into_string()
        .map_err(ArgError::NonUtf8Option)
}

fn parse_positive_integer(option: &'static str, value: String) -> Result<usize, ArgError> {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| value.parse::<usize>().ok())
        .flatten()
        .filter(|value| *value > 0)
        .ok_or(ArgError::InvalidValue {
            option,
            value,
            expected: "a positive integer",
        })
}

fn set_once<T>(slot: &mut Option<T>, option: &'static str, value: T) -> Result<(), ArgError> {
    if slot.is_some() {
        Err(ArgError::DuplicateOption(option))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

pub(crate) const HELP: &str = "\
RustRed: pure-Rust parametric IBP/LI derivation with Symbolica

USAGE:
    rustred derive [OPTIONS]
    rustred campaign plan [OPTIONS]
    rustred campaign preflight [OPTIONS]

DERIVE OPTIONS:
    --input <PATH|->             Read from PATH, or standard input with - [default: -]
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --input-format <FORMAT>      auto, toml, or symbolica [default: auto]
    --relations <SELECTION>      all, ordinary, or li [default: all]
    --n-cores <COUNT>            Maximum worker cores for parallel stages [default: 1]
    --force                      Atomically replace an existing output file

CAMPAIGN PLAN OPTIONS:
    --input <PATH|->             Read from PATH, or standard input with - [default: -]
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --input-format <FORMAT>      auto, toml, or symbolica [default: auto]
    --root-id <ID>               Root ID for one raw Symbolica campaign input
    --force                      Atomically replace an existing output file

CAMPAIGN PREFLIGHT OPTIONS:
    --profile <PATH|->           Read a physical resource profile from PATH or -
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --n-cores <COUNT>            Requested execution-width ceiling [default: 1]
    --max-memory <SIZE>          Operational memory limit (B/KiB/MiB/GiB/TiB)
    --force                      Atomically replace an existing output file

GENERAL OPTIONS:
    -h, --help                   Print this help
    -V, --version                Print the RustRed version

`derive` generates fully parametric identities. Any concrete target carried by
the input is validated and reported as not processed; it is never reduced by
this command.

`campaign plan` authenticates and interns only the supplied campaign roots.
It does not discover dependencies, derive relations, prove closure, or publish
rules. It deliberately has no --n-cores or --max-memory option.

`campaign preflight` checks a topology-neutral physical resource profile and
reports a ready width or typed memory-capacity pause. It never starts a frontier
or constructs a worker pool.
";
