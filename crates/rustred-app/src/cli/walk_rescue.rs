//! `rustred walk-rescue-plan`: classify the frontiers of a saved walk and
//! write the next digest-chained rescue amendment (see
//! `owner_domain_walk_rescue_plan`). The walk is named by its own
//! `owner-domain-match --follow-successors` argv (with every recorded
//! `--amend-queries`), exactly like `walk-verify-closure`.
use super::args::{
    ArgError, Command, StreamPath, next_utf8_value, next_value, parse_positive_integer,
};
use super::error::CliError;
use super::io::{preflight_output_destination, read_bounded, write_output};
use crate::{
    OwnerDomainWalkRescuePlanOptions, OwnerDomainWalkRescueScope, owner_domain_walk_rescue_plan,
};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WalkRescuePlanArgs {
    pub command: PathBuf,
    pub checkpoint: Option<PathBuf>,
    pub output: StreamPath,
    pub amendment_output: Option<PathBuf>,
    pub helper_id_prefix: String,
    pub rescue_helpers: Option<PathBuf>,
    pub max_repeats: usize,
    pub scope: OwnerDomainWalkRescueScope,
    pub force: bool,
}

pub(crate) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let defaults = OwnerDomainWalkRescuePlanOptions::new("");
    let mut command = None;
    let mut args = WalkRescuePlanArgs {
        command: PathBuf::new(),
        checkpoint: None,
        output: StreamPath::Stdio,
        amendment_output: None,
        helper_id_prefix: defaults.helper_id_prefix,
        rescue_helpers: None,
        max_repeats: defaults.max_repeats,
        scope: defaults.scope,
        force: false,
    };
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--command" => command = Some(PathBuf::from(next_value(&mut arguments, "--command")?)),
            "--checkpoint" => {
                args.checkpoint = Some(PathBuf::from(next_value(&mut arguments, "--checkpoint")?))
            }
            "--output" => {
                args.output =
                    StreamPath::File(PathBuf::from(next_value(&mut arguments, "--output")?))
            }
            "--amendment-output" => {
                args.amendment_output = Some(PathBuf::from(next_value(
                    &mut arguments,
                    "--amendment-output",
                )?))
            }
            "--helper-id-prefix" => {
                args.helper_id_prefix = next_utf8_value(&mut arguments, "--helper-id-prefix")?
            }
            "--rescue-helpers" => {
                args.rescue_helpers = Some(PathBuf::from(next_value(
                    &mut arguments,
                    "--rescue-helpers",
                )?))
            }
            "--max-repeats" => {
                args.max_repeats = parse_positive_integer(
                    "--max-repeats",
                    next_utf8_value(&mut arguments, "--max-repeats")?,
                )?
            }
            "--rescue-scope" => {
                let value = next_utf8_value(&mut arguments, "--rescue-scope")?;
                args.scope = match value.as_str() {
                    "class" => OwnerDomainWalkRescueScope::Class,
                    "tainted" => OwnerDomainWalkRescueScope::Tainted,
                    _ => {
                        return Err(ArgError::InvalidValue {
                            option: "--rescue-scope",
                            value,
                            expected: "class (default) or tainted",
                        });
                    }
                };
            }
            "--force" => args.force = true,
            _ => return Err(ArgError::UnknownOption(option)),
        }
    }
    args.command = command.ok_or(ArgError::MissingRequiredOption("--command"))?;
    if args.helper_id_prefix.is_empty() {
        return Err(ArgError::InvalidValue {
            option: "--helper-id-prefix",
            value: String::new(),
            expected: "a nonempty cosmetic prefix for newly appended auxiliary query IDs",
        });
    }
    Ok(Command::WalkRescuePlan(args))
}

pub(super) fn run(args: WalkRescuePlanArgs) -> Result<(), CliError> {
    preflight_output_destination(&args.output, args.force)?;
    if let Some(path) = &args.amendment_output {
        preflight_output_destination(&StreamPath::File(path.clone()), args.force)?;
    }
    let argv = super::walk_verify::walk_argv(&args.command)?;
    let request = super::owner_match::walk_request_from_argv(argv).map_err(CliError::Input)?;
    let checkpoint = args
        .checkpoint
        .clone()
        .or_else(|| request.checkpoint.as_ref().map(|c| c.directory.clone()))
        .ok_or_else(|| {
            CliError::Input(
                "no checkpoint: pass --checkpoint or a checkpointed walk command".into(),
            )
        })?;
    let mut options = OwnerDomainWalkRescuePlanOptions::new(checkpoint);
    options.helper_id_prefix = args.helper_id_prefix.clone();
    options.max_repeats = args.max_repeats;
    options.scope = args.scope;
    if let Some(path) = &args.rescue_helpers {
        let file = std::fs::File::open(path)
            .map_err(|e| CliError::InputIo(format!("{}: {e}", path.display())))?;
        let bytes = read_bounded(file, "rescue helpers", 64 << 20)?;
        options.rescue_helpers_json = Some(
            String::from_utf8(bytes)
                .map_err(|_| CliError::Input("rescue helpers must be UTF-8".into()))?,
        );
    }
    let result = owner_domain_walk_rescue_plan(&request, &options)?;
    let mut plan = result.plan;
    if let Some(text) = &result.amendment {
        match &args.amendment_output {
            Some(path) => {
                write_output(&StreamPath::File(path.clone()), text.as_bytes(), args.force)?;
                plan["amendment"]["path"] = serde_json::json!(path);
            }
            None => {
                plan["amendment"]["document"] =
                    serde_json::from_str(text).map_err(|e| CliError::OutputIo(e.to_string()))?;
            }
        }
    }
    let mut text =
        serde_json::to_vec_pretty(&plan).map_err(|e| CliError::OutputIo(e.to_string()))?;
    text.push(b'\n');
    write_output(&args.output, &text, args.force)?;
    match plan["verdict"].as_str() {
        Some("rescue" | "no_amendment_needed" | "no_frontier") => Ok(()),
        _ => Err(CliError::Verdict {
            incomplete: false,
            message: format!(
                "frontier rescue refused ({}): {}",
                plan["verdict"], plan["reason"]
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_requires_a_command_and_a_nonempty_pattern() {
        let words = |text: &str| {
            text.split_whitespace()
                .map(OsString::from)
                .collect::<Vec<_>>()
        };
        assert!(matches!(
            parse(words("--output plan.json").into_iter()),
            Err(ArgError::MissingRequiredOption("--command"))
        ));
        let Command::WalkRescuePlan(args) = parse(
            words("--command c.json --helper-id-prefix owner-anchor- --max-repeats 2 --amendment-output a.json")
                .into_iter(),
        )
        .unwrap() else {
            panic!("rescue plan command")
        };
        assert_eq!(args.helper_id_prefix, "owner-anchor-");
        assert_eq!(args.max_repeats, 2);
        assert_eq!(args.amendment_output, Some(PathBuf::from("a.json")));
        assert!(parse(words("--command c.json --max-repeats 0").into_iter()).is_err());
    }
}
