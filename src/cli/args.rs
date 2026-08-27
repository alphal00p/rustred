use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputFormat {
    Auto,
    Toml,
    Symbolica,
}

impl InputFormat {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Toml => "toml",
            Self::Symbolica => "symbolica",
        }
    }

    fn parse(value: &str) -> Result<Self, ArgError> {
        match value {
            "auto" => Ok(Self::Auto),
            "toml" => Ok(Self::Toml),
            "symbolica" => Ok(Self::Symbolica),
            _ => Err(ArgError::InvalidValue {
                option: "--input-format",
                value: value.to_owned(),
                expected: "auto, toml, or symbolica",
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelationSelection {
    All,
    Ordinary,
    LorentzInvariance,
}

impl RelationSelection {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Ordinary => "ordinary",
            Self::LorentzInvariance => "li",
        }
    }

    fn parse(value: &str) -> Result<Self, ArgError> {
        match value {
            "all" => Ok(Self::All),
            "ordinary" => Ok(Self::Ordinary),
            "li" => Ok(Self::LorentzInvariance),
            _ => Err(ArgError::InvalidValue {
                option: "--relations",
                value: value.to_owned(),
                expected: "all, ordinary, or li",
            }),
        }
    }

    pub(crate) const fn includes_ordinary(self) -> bool {
        matches!(self, Self::All | Self::Ordinary)
    }

    pub(crate) const fn includes_li(self) -> bool {
        matches!(self, Self::All | Self::LorentzInvariance)
    }
}

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
pub(crate) enum ArgError {
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
            Self::MissingSubcommand(command) => {
                write!(
                    formatter,
                    "missing {command} subcommand; expected `plan` or `preflight`"
                )
            }
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
            return Ok(Command::Help);
        }
        "--version" | "-V" => {
            reject_trailing(arguments)?;
            return Ok(Command::Version);
        }
        "derive" => return parse_derive(arguments),
        "campaign" => return parse_campaign(arguments),
        _ => return Err(ArgError::UnknownCommand(command)),
    }
}

fn parse_derive(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut input = None;
    let mut output = None;
    let mut input_format = None;
    let mut relations = None;
    let mut n_cores = None;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => {
                if help {
                    return Err(ArgError::DuplicateOption("--help"));
                }
                help = true;
            }
            "--force" => {
                if force {
                    return Err(ArgError::DuplicateOption("--force"));
                }
                force = true;
            }
            "--input" => {
                set_once(
                    &mut input,
                    "--input",
                    StreamPath::parse(next_value(&mut arguments, "--input")?)?,
                )?;
            }
            "--output" => {
                set_once(
                    &mut output,
                    "--output",
                    StreamPath::parse(next_value(&mut arguments, "--output")?)?,
                )?;
            }
            "--input-format" => {
                let value = next_utf8_value(&mut arguments, "--input-format")?;
                set_once(
                    &mut input_format,
                    "--input-format",
                    InputFormat::parse(&value)?,
                )?;
            }
            "--relations" => {
                let value = next_utf8_value(&mut arguments, "--relations")?;
                set_once(
                    &mut relations,
                    "--relations",
                    RelationSelection::parse(&value)?,
                )?;
            }
            "--n-cores" => {
                let value = next_utf8_value(&mut arguments, "--n-cores")?;
                let parsed = parse_positive_integer("--n-cores", value)?;
                set_once(&mut n_cores, "--n-cores", parsed)?;
            }
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::Derive(DeriveArgs {
        input: input.unwrap_or(StreamPath::Stdio),
        output: output.unwrap_or(StreamPath::Stdio),
        input_format: input_format.unwrap_or(InputFormat::Auto),
        relations: relations.unwrap_or(RelationSelection::All),
        n_cores: n_cores.unwrap_or(1),
        force,
    }))
}

fn parse_campaign(mut arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let Some(subcommand) = arguments.next() else {
        return Err(ArgError::MissingSubcommand("campaign"));
    };
    let subcommand = subcommand.into_string().map_err(ArgError::NonUtf8Option)?;
    match subcommand.as_str() {
        "--help" | "-h" | "help" => {
            reject_trailing(arguments)?;
            Ok(Command::Help)
        }
        "plan" => parse_campaign_plan(arguments),
        "preflight" => parse_campaign_preflight(arguments),
        _ => Err(ArgError::UnknownSubcommand {
            command: "campaign",
            subcommand,
        }),
    }
}

fn parse_campaign_preflight(
    arguments: impl Iterator<Item = OsString>,
) -> Result<Command, ArgError> {
    let mut profile = None;
    let mut output = None;
    let mut n_cores = None;
    let mut max_memory_bytes = None;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => {
                if help {
                    return Err(ArgError::DuplicateOption("--help"));
                }
                help = true;
            }
            "--force" => {
                if force {
                    return Err(ArgError::DuplicateOption("--force"));
                }
                force = true;
            }
            "--profile" => {
                set_once(
                    &mut profile,
                    "--profile",
                    StreamPath::parse(next_value(&mut arguments, "--profile")?)?,
                )?;
            }
            "--output" => {
                set_once(
                    &mut output,
                    "--output",
                    StreamPath::parse(next_value(&mut arguments, "--output")?)?,
                )?;
            }
            "--n-cores" => {
                let value = next_utf8_value(&mut arguments, "--n-cores")?;
                let parsed = parse_positive_integer("--n-cores", value)?;
                set_once(&mut n_cores, "--n-cores", parsed)?;
            }
            "--max-memory" => {
                let value = next_utf8_value(&mut arguments, "--max-memory")?;
                let parsed = parse_memory_bytes(&value)
                    .filter(|bytes| *bytes > 0)
                    .ok_or(ArgError::InvalidValue {
                        option: "--max-memory",
                        value,
                        expected: "a positive integer followed by B, KiB, MiB, GiB, or TiB",
                    })?;
                set_once(&mut max_memory_bytes, "--max-memory", parsed)?;
            }
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::CampaignPreflight(CampaignPreflightArgs {
        profile: profile.ok_or(ArgError::MissingRequiredOption("--profile"))?,
        output: output.unwrap_or(StreamPath::Stdio),
        n_cores: n_cores.unwrap_or(1),
        max_memory_bytes: max_memory_bytes
            .ok_or(ArgError::MissingRequiredOption("--max-memory"))?,
        force,
    }))
}

fn parse_campaign_plan(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut input = None;
    let mut output = None;
    let mut input_format = None;
    let mut root_id = None;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => {
                if help {
                    return Err(ArgError::DuplicateOption("--help"));
                }
                help = true;
            }
            "--force" => {
                if force {
                    return Err(ArgError::DuplicateOption("--force"));
                }
                force = true;
            }
            "--input" => {
                set_once(
                    &mut input,
                    "--input",
                    StreamPath::parse(next_value(&mut arguments, "--input")?)?,
                )?;
            }
            "--output" => {
                set_once(
                    &mut output,
                    "--output",
                    StreamPath::parse(next_value(&mut arguments, "--output")?)?,
                )?;
            }
            "--input-format" => {
                let value = next_utf8_value(&mut arguments, "--input-format")?;
                set_once(
                    &mut input_format,
                    "--input-format",
                    InputFormat::parse(&value)?,
                )?;
            }
            "--root-id" => {
                let value = next_utf8_value(&mut arguments, "--root-id")?;
                if value.is_empty() {
                    return Err(ArgError::InvalidValue {
                        option: "--root-id",
                        value,
                        expected: "a nonempty UTF-8 identifier",
                    });
                }
                set_once(&mut root_id, "--root-id", value)?;
            }
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::CampaignPlan(CampaignPlanArgs {
        input: input.unwrap_or(StreamPath::Stdio),
        output: output.unwrap_or(StreamPath::Stdio),
        input_format: input_format.unwrap_or(InputFormat::Auto),
        root_id,
        force,
    }))
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

pub(crate) fn parse_memory_bytes(value: &str) -> Option<u64> {
    let (digits, multiplier) = [
        ("TiB", 1_u64 << 40),
        ("GiB", 1_u64 << 30),
        ("MiB", 1_u64 << 20),
        ("KiB", 1_u64 << 10),
        ("B", 1_u64),
    ]
    .into_iter()
    .find_map(|(suffix, multiplier)| {
        value
            .strip_suffix(suffix)
            .map(|digits| (digits, multiplier))
    })?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u64>().ok()?.checked_mul(multiplier)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Command, ArgError> {
        parse_args(arguments.iter().map(OsString::from))
    }

    #[test]
    fn defaults_to_stdio_auto_and_all_relations() {
        assert_eq!(
            parse(&["rustred", "derive"]).unwrap(),
            Command::Derive(DeriveArgs {
                input: StreamPath::Stdio,
                output: StreamPath::Stdio,
                input_format: InputFormat::Auto,
                relations: RelationSelection::All,
                n_cores: 1,
                force: false,
            })
        );
    }

    #[test]
    fn parses_the_complete_derive_surface() {
        assert_eq!(
            parse(&[
                "rustred",
                "derive",
                "--input",
                "family.symbolica",
                "--output",
                "relations.toml",
                "--input-format",
                "symbolica",
                "--relations",
                "ordinary",
                "--n-cores",
                "4",
                "--force",
            ])
            .unwrap(),
            Command::Derive(DeriveArgs {
                input: StreamPath::File("family.symbolica".into()),
                output: StreamPath::File("relations.toml".into()),
                input_format: InputFormat::Symbolica,
                relations: RelationSelection::Ordinary,
                n_cores: 4,
                force: true,
            })
        );
    }

    #[test]
    fn parses_the_campaign_plan_surface_without_execution_budgets() {
        assert_eq!(
            parse(&[
                "rustred",
                "campaign",
                "plan",
                "--input",
                "campaign.symbolica",
                "--output",
                "plan.toml",
                "--input-format",
                "symbolica",
                "--root-id",
                "raw-root",
                "--force",
            ])
            .unwrap(),
            Command::CampaignPlan(CampaignPlanArgs {
                input: StreamPath::File("campaign.symbolica".into()),
                output: StreamPath::File("plan.toml".into()),
                input_format: InputFormat::Symbolica,
                root_id: Some("raw-root".to_owned()),
                force: true,
            })
        );
        assert_eq!(
            parse(&["rustred", "campaign", "plan", "--n-cores", "2"]),
            Err(ArgError::UnknownOption("--n-cores".to_owned()))
        );
        assert_eq!(
            parse(&["rustred", "campaign", "plan", "--max-memory", "1GiB"]),
            Err(ArgError::UnknownOption("--max-memory".to_owned()))
        );
    }

    #[test]
    fn parses_campaign_preflight_with_a_default_inline_ceiling() {
        assert_eq!(
            parse(&[
                "rustred",
                "campaign",
                "preflight",
                "--profile",
                "epyc.toml",
                "--max-memory",
                "900GiB",
            ])
            .unwrap(),
            Command::CampaignPreflight(CampaignPreflightArgs {
                profile: StreamPath::File("epyc.toml".into()),
                output: StreamPath::Stdio,
                n_cores: 1,
                max_memory_bytes: 900 * (1_u64 << 30),
                force: false,
            })
        );
        assert_eq!(
            parse(&[
                "rustred",
                "campaign",
                "preflight",
                "--profile",
                "-",
                "--output",
                "width.toml",
                "--n-cores",
                "100",
                "--max-memory",
                "1TiB",
                "--force",
            ])
            .unwrap(),
            Command::CampaignPreflight(CampaignPreflightArgs {
                profile: StreamPath::Stdio,
                output: StreamPath::File("width.toml".into()),
                n_cores: 100,
                max_memory_bytes: 1_u64 << 40,
                force: true,
            })
        );
    }

    #[test]
    fn campaign_preflight_requires_profile_and_memory_and_parses_strict_units() {
        assert_eq!(
            parse(&["rustred", "campaign", "preflight", "--max-memory", "1GiB",]),
            Err(ArgError::MissingRequiredOption("--profile"))
        );
        assert_eq!(
            parse(&["rustred", "campaign", "preflight", "--profile", "host.toml",]),
            Err(ArgError::MissingRequiredOption("--max-memory"))
        );
        for (input, expected) in [
            ("1B", 1),
            ("2KiB", 2 * (1_u64 << 10)),
            ("3MiB", 3 * (1_u64 << 20)),
            ("4GiB", 4 * (1_u64 << 30)),
            ("5TiB", 5 * (1_u64 << 40)),
        ] {
            assert_eq!(parse_memory_bytes(input), Some(expected));
        }
        for invalid in [
            "0",
            "1KB",
            "1GB",
            "1.5GiB",
            "+1GiB",
            " 1GiB",
            "1GiB ",
            "16777216TiB",
        ] {
            assert_eq!(parse_memory_bytes(invalid), None, "accepted {invalid:?}");
        }
        for invalid in ["0B", "1GB", "1.5GiB", "16777216TiB"] {
            assert!(matches!(
                parse(&[
                    "rustred",
                    "campaign",
                    "preflight",
                    "--profile",
                    "host.toml",
                    "--max-memory",
                    invalid,
                ]),
                Err(ArgError::InvalidValue {
                    option: "--max-memory",
                    ..
                })
            ));
        }
    }

    #[test]
    fn rejects_duplicate_and_unknown_options() {
        assert_eq!(
            parse(&["rustred", "derive", "--force", "--force"]),
            Err(ArgError::DuplicateOption("--force"))
        );
        assert_eq!(
            parse(&["rustred", "derive", "--mystery"]),
            Err(ArgError::UnknownOption("--mystery".to_owned()))
        );
        assert!(matches!(
            parse(&["rustred", "derive", "--n-cores", "0"]),
            Err(ArgError::InvalidValue {
                option: "--n-cores",
                ..
            })
        ));
        assert_eq!(
            parse(&["rustred", "derive", "--n-cores", "2", "--n-cores", "3",]),
            Err(ArgError::DuplicateOption("--n-cores"))
        );
        for invalid in ["-2", "+2", "184467440737095516160"] {
            assert!(matches!(
                parse(&["rustred", "derive", "--n-cores", invalid]),
                Err(ArgError::InvalidValue {
                    option: "--n-cores",
                    ..
                })
            ));
        }
    }
}
