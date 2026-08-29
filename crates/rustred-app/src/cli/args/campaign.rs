use std::ffi::OsString;

use crate::InputFormat;
use crate::application::memory::parse_memory_bytes;

use super::{
    ArgError, CampaignPlanArgs, CampaignPreflightArgs, Command, StreamPath, next_utf8_value,
    next_value, parse_positive_integer, reject_trailing, set_once,
};

pub(super) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let Some(subcommand) = arguments.next() else {
        return Err(ArgError::MissingSubcommand("campaign"));
    };
    let subcommand = subcommand.into_string().map_err(ArgError::NonUtf8Option)?;
    match subcommand.as_str() {
        "--help" | "-h" | "help" => {
            reject_trailing(arguments)?;
            Ok(Command::Help)
        }
        "plan" => parse_plan(arguments),
        "preflight" => parse_preflight(arguments),
        _ => Err(ArgError::UnknownSubcommand {
            command: "campaign",
            subcommand,
        }),
    }
}

fn parse_preflight(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
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

fn parse_plan(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
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
                let parsed = value.parse().map_err(|_| ArgError::InvalidValue {
                    option: "--input-format",
                    value,
                    expected: InputFormat::EXPECTED_VALUES,
                })?;
                set_once(&mut input_format, "--input-format", parsed)?;
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
