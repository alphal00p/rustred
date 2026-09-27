//! `rustred walk-verify-closure`: offline closure oracle over a saved walk
//! generation (see `owner_domain_walk_verify_closure`). The walk is named by
//! its own `owner-domain-match --follow-successors` argv, so the verifier
//! reads exactly the request the checkpoint is bound to.
use super::args::{
    ArgError, Command, StreamPath, next_utf8_value, next_value, parse_positive_integer,
};
use super::error::CliError;
use super::io::{preflight_output_destination, write_output};
use crate::{
    OwnerDomainWalkVerifyMutation, OwnerDomainWalkVerifyOptions,
    OwnerDomainWalkVerifyReferenceLevers, OwnerDomainWalkVerifyReinspect,
    owner_domain_walk_verify_closure,
};
use serde_json::Value;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WalkVerifyClosureArgs {
    pub command: PathBuf,
    pub checkpoint: Option<PathBuf>,
    pub output: StreamPath,
    pub threads: usize,
    pub reinspect: OwnerDomainWalkVerifyReinspect,
    pub brute_force_max_points: u64,
    pub brute_force_point_budget: u64,
    pub require_closure: bool,
    pub reference_levers: OwnerDomainWalkVerifyReferenceLevers,
    pub mutation: Option<OwnerDomainWalkVerifyMutation>,
    pub helper_pattern: String,
    pub max_violations: usize,
    pub force: bool,
    /// None: the command file's sibling result.json when it exists.
    pub result: Option<PathBuf>,
    pub no_result: bool,
}

pub(crate) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut command = None;
    let mut checkpoint = None;
    let mut output = StreamPath::Stdio;
    let defaults = OwnerDomainWalkVerifyOptions::new("");
    let mut args = WalkVerifyClosureArgs {
        command: PathBuf::new(),
        checkpoint: None,
        output: StreamPath::Stdio,
        threads: 1,
        reinspect: OwnerDomainWalkVerifyReinspect::All,
        brute_force_max_points: defaults.brute_force_max_points,
        brute_force_point_budget: defaults.brute_force_point_budget,
        require_closure: false,
        reference_levers: defaults.reference_levers,
        mutation: None,
        helper_pattern: defaults.helper_pattern,
        max_violations: defaults.max_violations,
        force: false,
        result: None,
        no_result: false,
    };
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--command" => command = Some(PathBuf::from(next_value(&mut arguments, "--command")?)),
            "--checkpoint" => {
                checkpoint = Some(PathBuf::from(next_value(&mut arguments, "--checkpoint")?))
            }
            "--output" => {
                output = StreamPath::File(PathBuf::from(next_value(&mut arguments, "--output")?))
            }
            "--threads" => {
                args.threads = parse_positive_integer(
                    "--threads",
                    next_utf8_value(&mut arguments, "--threads")?,
                )?
            }
            "--reinspect" => {
                let value = next_utf8_value(&mut arguments, "--reinspect")?;
                args.reinspect = parse_reinspect(&value).ok_or(ArgError::InvalidValue {
                    option: "--reinspect",
                    value,
                    expected: "all, none or sample:N[:SEED]",
                })?;
            }
            "--brute-force-max-points" => {
                let value = next_utf8_value(&mut arguments, "--brute-force-max-points")?;
                args.brute_force_max_points =
                    value.parse().map_err(|_| ArgError::InvalidValue {
                        option: "--brute-force-max-points",
                        value,
                        expected: "a nonnegative integer (0 disables)",
                    })?;
            }
            "--brute-force-point-budget" => {
                let value = next_utf8_value(&mut arguments, "--brute-force-point-budget")?;
                args.brute_force_point_budget =
                    value.parse().map_err(|_| ArgError::InvalidValue {
                        option: "--brute-force-point-budget",
                        value,
                        expected: "a nonnegative integer",
                    })?;
            }
            "--require-closure" => args.require_closure = true,
            "--reference-levers" => {
                let value = next_utf8_value(&mut arguments, "--reference-levers")?;
                args.reference_levers = match value.as_str() {
                    "off" => OwnerDomainWalkVerifyReferenceLevers::Off,
                    "as-run" => OwnerDomainWalkVerifyReferenceLevers::AsRun,
                    _ => {
                        return Err(ArgError::InvalidValue {
                            option: "--reference-levers",
                            value,
                            expected: "off (default) or as-run",
                        });
                    }
                };
            }
            "--mutate" => {
                let value = next_utf8_value(&mut arguments, "--mutate")?;
                args.mutation = Some(OwnerDomainWalkVerifyMutation::parse(&value).ok_or(
                    ArgError::InvalidValue {
                        option: "--mutate",
                        value,
                        expected: "one of dropped-edge, retargeted-alias, dropped-frontier-record, seal-with-frontier, seal-with-error, injected-false-hit, hidden-frontier, hidden-error, miscounted-events, miscounted-successors, retargeted-anchor, remapped-query, foreign-request, foreign-owners, mismatched-result, alias-chain-detour",
                    },
                )?);
            }
            "--helper-pattern" => {
                args.helper_pattern = next_utf8_value(&mut arguments, "--helper-pattern")?
            }
            "--max-violations" => {
                args.max_violations = parse_positive_integer(
                    "--max-violations",
                    next_utf8_value(&mut arguments, "--max-violations")?,
                )?
            }
            "--force" => args.force = true,
            "--result" => {
                args.result = Some(PathBuf::from(next_value(&mut arguments, "--result")?))
            }
            "--no-result" => args.no_result = true,
            _ => return Err(ArgError::UnknownOption(option)),
        }
    }
    args.command = command.ok_or(ArgError::MissingRequiredOption("--command"))?;
    if args.no_result && args.result.is_some() {
        return Err(ArgError::InvalidValue {
            option: "--no-result",
            value: "--result".into(),
            expected: "at most one of --result and --no-result",
        });
    }
    args.checkpoint = checkpoint;
    args.output = output;
    Ok(Command::WalkVerifyClosure(args))
}

fn parse_reinspect(value: &str) -> Option<OwnerDomainWalkVerifyReinspect> {
    match value {
        "all" => Some(OwnerDomainWalkVerifyReinspect::All),
        "none" => Some(OwnerDomainWalkVerifyReinspect::None),
        _ => {
            let rest = value.strip_prefix("sample:")?;
            let (count, seed) = rest.split_once(':').unwrap_or((rest, "0"));
            Some(OwnerDomainWalkVerifyReinspect::Sample {
                count: count.parse().ok().filter(|&c| c > 0)?,
                seed: seed.parse().ok()?,
            })
        }
    }
}

/// The walk argv: a JSON list of strings (`command.json`) or an object with
/// a `command` list (`request.json`).
fn walk_argv(path: &PathBuf) -> Result<Vec<OsString>, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::InputIo(format!("{}: {e}", path.display())))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| CliError::Input(format!("{}: {e}", path.display())))?;
    let list = value.get("command").unwrap_or(&value);
    list.as_array()
        .and_then(|items| {
            items
                .iter()
                .map(|v| v.as_str().map(OsString::from))
                .collect()
        })
        .ok_or_else(|| CliError::Input("walk command must be a JSON list of strings".into()))
}

pub(super) fn run(args: WalkVerifyClosureArgs) -> Result<(), CliError> {
    super::routed::preflight_inner_pools()?;
    preflight_output_destination(&args.output, args.force)?;
    let argv = walk_argv(&args.command)?;
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
    let mut options = OwnerDomainWalkVerifyOptions::new(checkpoint);
    options.threads = args.threads;
    options.reinspect = args.reinspect;
    options.brute_force_max_points = args.brute_force_max_points;
    options.brute_force_point_budget = args.brute_force_point_budget;
    options.require_closure = args.require_closure;
    options.reference_levers = args.reference_levers;
    options.mutation = args.mutation;
    options.helper_pattern = args.helper_pattern.clone();
    options.max_violations = args.max_violations;
    options.result = if args.no_result {
        None
    } else {
        args.result.clone().or_else(|| {
            let sibling = args.command.parent()?.join("result.json");
            sibling.is_file().then_some(sibling)
        })
    };
    let cancellation = AtomicBool::new(false);
    let report = owner_domain_walk_verify_closure(&request, &options, &cancellation, |event| {
        let _ = writeln!(std::io::stderr().lock(), "{event}");
    })?;
    let mut text =
        serde_json::to_vec_pretty(&report).map_err(|e| CliError::OutputIo(e.to_string()))?;
    text.push(b'\n');
    write_output(&args.output, &text, args.force)?;
    match report["verdict"].as_str() {
        Some("PASS") => Ok(()),
        Some("INCOMPLETE") => Err(CliError::Verdict {
            incomplete: true,
            message: format!(
                "closure verification INCOMPLETE: {}",
                report["verdict_reason"]
            ),
        }),
        _ => Err(CliError::Verdict {
            incomplete: false,
            message: format!(
                "closure verification FAIL: {}",
                report["violations_by_class"]
            ),
        }),
    }
}
