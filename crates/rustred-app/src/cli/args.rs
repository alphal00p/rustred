mod campaign;
mod derive;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use crate::{ClosingFamilySelector, InputFormat, RelationSelection};

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
pub(crate) struct CampaignGenerateArgs {
    pub(crate) family: ClosingFamilySelector,
    pub(crate) output: StreamPath,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignInspectArgs {
    pub(crate) artifact: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignReduceArgs {
    pub(crate) artifact: StreamPath,
    pub(crate) target_powers: Vec<i64>,
    pub(crate) max_rule_applications: usize,
    pub(crate) output: StreamPath,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FoundryCampaignRunArgs {
    pub(crate) config: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) measurements_output: Option<StreamPath>,
    pub(crate) no_progress: bool,
    pub(crate) color: ColorPolicy,
    pub(crate) force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FoundryWaveCampaignRunArgs {
    pub(crate) config: StreamPath,
    pub(crate) output: StreamPath,
    pub(crate) measurements_output: Option<StreamPath>,
    pub(crate) artifact_output: Option<StreamPath>,
    pub(crate) n_cores: usize,
    pub(crate) no_progress: bool,
    pub(crate) color: ColorPolicy,
    pub(crate) force: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ColorPolicy {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Derive(DeriveArgs),
    CampaignPlan(CampaignPlanArgs),
    CampaignPreflight(CampaignPreflightArgs),
    CampaignGenerate(CampaignGenerateArgs),
    CampaignInspect(CampaignInspectArgs),
    CampaignReduce(CampaignReduceArgs),
    FoundryCampaignRun(FoundryCampaignRunArgs),
    FoundryWaveCampaignRun(FoundryWaveCampaignRunArgs),
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
    InvalidCombination(&'static str),
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
                "missing {command} subcommand; expected `plan`, `preflight`, `run`, `run-waves`, `generate`, `inspect`, or `reduce`"
            ),
            Self::UnknownCommand(command) => write!(formatter, "unknown command {command:?}"),
            Self::UnknownSubcommand {
                command,
                subcommand,
            } => write!(
                formatter,
                "unknown {command} subcommand {subcommand:?}; expected `plan`, `preflight`, `run`, `run-waves`, `generate`, `inspect`, or `reduce`"
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
            Self::InvalidCombination(detail) => formatter.write_str(detail),
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

fn parse_nonnegative_integer(option: &'static str, value: String) -> Result<usize, ArgError> {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| value.parse::<usize>().ok())
        .flatten()
        .ok_or(ArgError::InvalidValue {
            option,
            value,
            expected: "a nonnegative integer fitting this platform",
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
    rustred campaign run [OPTIONS]
    rustred campaign run-waves [OPTIONS]
    rustred campaign generate [OPTIONS]
    rustred campaign inspect [OPTIONS]
    rustred campaign reduce [OPTIONS]

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

FOUNDRY CAMPAIGN RUN OPTIONS:
    --config <PATH|->            Read strict versioned campaign TOML from PATH or stdin
    --output <PATH|->            Write deterministic diagnostic TOML to PATH or stdout
    --measurements-output <PATH|->
                                 Optionally write nonsemantic timing TOML separately
    --no-progress                Disable the interactive stderr dashboard
    --color <WHEN>               auto, always, or never [default: auto]
    --force                      Atomically replace existing output files

FOUNDRY WAVE CAMPAIGN RUN OPTIONS:
    --config <PATH|->            Read strict versioned campaign TOML from PATH or stdin
    --output <PATH|->            Write deterministic diagnostic TOML to PATH or stdout
    --measurements-output <PATH|->
                                 Optionally write nonsemantic timing TOML separately
    --artifact-output <PATH|->   Write canonical durable K6 bytes after successful closure
    --n-cores <COUNT>            Sibling workers within one atomic wave [default: 1]
    --no-progress                Disable the interactive stderr dashboard
    --color <WHEN>               auto, always, or never [default: auto]
    --force                      Atomically replace existing output files

CAMPAIGN GENERATE OPTIONS:
    --family <SELECTOR>          unit-mass-vacuum-k1 or unit-mass-vacuum-k3
    --output <PATH|->            Write durable artifact bytes to PATH or stdout [default: -]
    --force                      Atomically replace an existing output file

CAMPAIGN INSPECT OPTIONS:
    --artifact <PATH|->          Read durable artifact bytes from PATH or standard input
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
    --force                      Atomically replace an existing output file

CAMPAIGN REDUCE OPTIONS:
    --artifact <PATH|->          Read durable artifact bytes from PATH or standard input
    --powers <N,...>             Signed integer target powers in denominator order
    --max-rule-applications <N>  Per-request recurrence ceiling [default: 1000000]
    --output <PATH|->            Write TOML to PATH, or standard output with - [default: -]
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

`campaign run` executes one bounded foundry diagnostic. Its deterministic
report is not a closing artifact; optional timings are emitted only through the
separate measurement sidecar. On a terminal, its stderr dashboard is refreshed
in place; `cap ETA` estimates only time to the configured task ceiling, never
time to mathematical closure. Redirected stderr is quiet by default. The V2
configuration uses disjoint `autonomous` and `external-hints-only` modes;
autonomous requests cannot carry caller-authored search hints.

`campaign run-waves` executes the `full-rank-atomic-waves` itinerary. Its
successful result can still be incomplete; same-rank siblings publish only as
a complete atomic wave. After complete publication, `--artifact-output`
writes bytes only after exact installation and one successful cold reload. Its
terminal-only dashboard aggregates detached sibling telemetry and is quiet
when stderr is redirected.

`campaign generate` writes a deterministic durable artifact encoding.
`campaign inspect` loads and authenticates durable artifact bytes once, then
writes their canonical metadata as TOML.

`campaign reduce` loads and applies the supplied artifact and emits exact
typed-master coefficients at unit mass together with the separate power of
`mass_squared` required by dimensional homogeneity.
";
