//! Read-only inventory of a saved walk. Reinspection recovers identities that
//! CP6 deliberately does not retain; none of this launches or resumes a walk.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};

use super::args::{
    ArgError, Command, StreamPath, next_utf8_value, next_value, parse_positive_integer,
};
use super::error::CliError;
use super::io::{preflight_output_destination, read_bounded, write_output};
use crate::{OwnerDomainWalkInventoryOptions, owner_domain_walk_inventory};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WalkInventoryArgs {
    command: Option<PathBuf>,
    campaign_directory: Option<PathBuf>,
    checkpoint: Option<PathBuf>,
    output: StreamPath,
    threads: usize,
    normalize_terminals: bool,
    rules_start: usize,
    terminals_start: usize,
    normalized_terminals_start: usize,
    page_size: usize,
    force: bool,
}

pub(crate) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut args = WalkInventoryArgs {
        command: None,
        campaign_directory: None,
        checkpoint: None,
        output: StreamPath::Stdio,
        threads: 1,
        normalize_terminals: false,
        rules_start: 0,
        terminals_start: 0,
        normalized_terminals_start: 0,
        page_size: 25,
        force: false,
    };
    let mut seen = BTreeSet::new();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match option.as_str() {
            "--command" => "--command",
            "--campaign-directory" => "--campaign-directory",
            "--checkpoint" => "--checkpoint",
            "--output" => "--output",
            "--threads" => "--threads",
            "--normalize-terminals" => "--normalize-terminals",
            "--rules-start" => "--rules-start",
            "--terminals-start" => "--terminals-start",
            "--normalized-terminals-start" => "--normalized-terminals-start",
            "--page-size" => "--page-size",
            "--force" => "--force",
            _ => return Err(ArgError::UnknownOption(option)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--command" => args.command = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--campaign-directory" => {
                args.campaign_directory = Some(PathBuf::from(next_value(&mut arguments, option)?));
            }
            "--checkpoint" => {
                args.checkpoint = Some(PathBuf::from(next_value(&mut arguments, option)?));
            }
            "--output" => {
                let value = next_value(&mut arguments, option)?;
                if value.is_empty() {
                    return Err(ArgError::EmptyPath);
                }
                args.output = if value == "-" {
                    StreamPath::Stdio
                } else {
                    StreamPath::File(PathBuf::from(value))
                };
            }
            "--threads" => {
                args.threads =
                    parse_positive_integer(option, next_utf8_value(&mut arguments, option)?)?;
            }
            "--rules-start" | "--terminals-start" | "--normalized-terminals-start" => {
                let value = next_utf8_value(&mut arguments, option)?;
                let start = value.parse().map_err(|_| ArgError::InvalidValue {
                    option,
                    value,
                    expected: "a nonnegative integer",
                })?;
                match option {
                    "--rules-start" => args.rules_start = start,
                    "--terminals-start" => args.terminals_start = start,
                    _ => args.normalized_terminals_start = start,
                }
            }
            "--page-size" => {
                let value = next_utf8_value(&mut arguments, option)?;
                args.page_size = value
                    .parse()
                    .ok()
                    .filter(|n| (1..=1000).contains(n))
                    .ok_or(ArgError::InvalidValue {
                        option,
                        value,
                        expected: "an integer in 1..=1000",
                    })?;
            }
            "--normalize-terminals" => args.normalize_terminals = true,
            "--force" => args.force = true,
            _ => unreachable!("admitted option"),
        }
    }
    if args.command.is_some() == args.campaign_directory.is_some() {
        return Err(ArgError::InvalidCombination(
            "walk-inventory requires exactly one of --command and --campaign-directory",
        ));
    }
    if seen.contains("--normalized-terminals-start") && !args.normalize_terminals {
        return Err(ArgError::InvalidCombination(
            "--normalized-terminals-start requires --normalize-terminals",
        ));
    }
    Ok(Command::WalkInventory(args))
}

fn read_json(path: &Path) -> Result<Value, CliError> {
    let file = std::fs::File::open(path)
        .map_err(|e| CliError::InputIo(format!("{}: {e}", path.display())))?;
    let bytes = read_bounded(file, "inventory command metadata", crate::MAX_INPUT_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|e| CliError::Input(format!("{}: {e}", path.display())))
}

fn canonical(path: &Path) -> Result<PathBuf, CliError> {
    path.canonicalize()
        .map_err(|e| CliError::InputIo(format!("{}: {e}", path.display())))
}

/// Resolve only a published run in this campaign. Do not execute the supervisor
/// command in active-run.json, and do not silently choose the newest directory.
fn campaign_command(campaign: &Path) -> Result<PathBuf, CliError> {
    let campaign = canonical(campaign)?;
    let active = read_json(&campaign.join("active-run.json"))?;
    let recorded = active["run_directory"].as_str().ok_or_else(|| {
        CliError::Input("active-run.json has no run_directory; use --command explicitly".into())
    })?;
    let name = Path::new(recorded)
        .file_name()
        .ok_or_else(|| CliError::Input("active-run.json has an invalid run_directory".into()))?;
    let runs = canonical(&campaign.join("runs"))?;
    let command = canonical(&runs.join(name).join("request.json"))?;
    if !command.starts_with(&runs) {
        return Err(CliError::Input(
            "saved command escapes the campaign runs directory".into(),
        ));
    }
    Ok(command)
}

/// Even --force must never replace checkpoint state or frozen input programs.
pub(super) fn protect_output(output: &StreamPath, protected: &[PathBuf]) -> Result<(), CliError> {
    let StreamPath::File(path) = output else {
        return Ok(());
    };
    let resolved = if path.exists() {
        canonical(path)?
    } else {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        canonical(parent)?.join(
            path.file_name()
                .ok_or_else(|| CliError::OutputIo("output needs a file name".into()))?,
        )
    };
    for protected in protected {
        let is_directory = protected.is_dir();
        let location = if protected.exists() {
            canonical(protected)?
        } else {
            // A stop file is normally absent: creating it would mutate the
            // producer's control state just as surely as replacing a file.
            let parent = protected
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let Some(name) = protected.file_name() else {
                continue;
            };
            if !parent.exists() {
                continue;
            }
            canonical(parent)?.join(name)
        };
        if resolved == location || is_directory && resolved.starts_with(&location) {
            return Err(CliError::OutputIo(format!(
                "inventory output would overwrite protected campaign data: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Preserve every explicit saved-command stream, including documents stored
/// outside owner_base. Reuse the native parser rather than a second flag grammar.
pub(super) fn protected_command_paths(argv: &[OsString]) -> Result<Vec<PathBuf>, CliError> {
    let Command::OwnerDomainMatch(args) = super::args::parse_args(argv.iter().cloned())? else {
        return Err(CliError::Input("not an owner-domain-match command".into()));
    };
    let mut paths = vec![args.manifest, args.queries, args.output, args.owner_base];
    paths.extend(args.events);
    paths.extend(args.stop_file);
    paths.extend(args.amend_queries);
    paths.extend(args.checkpoint.map(|checkpoint| checkpoint.directory));
    if let Some(program) = argv.first() {
        paths.push(PathBuf::from(program));
    }
    if let Ok(program) = std::env::current_exe() {
        paths.push(program);
    }
    Ok(paths)
}

struct Signals(Vec<signal_hook::SigId>);
impl Signals {
    fn register(cancel: &Arc<AtomicBool>) -> Result<Self, CliError> {
        let mut signals = Self(Vec::new());
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            signals.0.push(
                signal_hook::flag::register(signal, Arc::clone(cancel)).map_err(|e| {
                    CliError::InputIo(format!("install inventory cancellation handler: {e}"))
                })?,
            );
        }
        Ok(signals)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        for id in &self.0 {
            signal_hook::low_level::unregister(*id);
        }
    }
}

pub(super) fn run(args: WalkInventoryArgs) -> Result<(), CliError> {
    preflight_output_destination(&args.output, args.force)?;
    let report = collect_report(&args, true)?;
    let mut bytes =
        serde_json::to_vec_pretty(&report).map_err(|e| CliError::OutputIo(e.to_string()))?;
    bytes.push(b'\n');
    write_output(&args.output, &bytes, args.force)?;
    inventory_verdict(&report)
}

/// Read-only fallback for old campaigns which have a CP6 but no published
/// package yet. Avoid constructing individual rule/coefficient pages.
pub(super) fn campaign_summary(campaign: &Path, threads: usize) -> Result<Value, CliError> {
    collect_report(
        &WalkInventoryArgs {
            command: None,
            campaign_directory: Some(campaign.to_path_buf()),
            checkpoint: None,
            output: StreamPath::Stdio,
            threads,
            normalize_terminals: true,
            rules_start: 0,
            terminals_start: 0,
            normalized_terminals_start: 0,
            page_size: 1,
            force: false,
        },
        false,
    )
}

fn collect_report(args: &WalkInventoryArgs, include_pages: bool) -> Result<Value, CliError> {
    super::routed::preflight_inner_pools()?;
    let command = match (&args.command, &args.campaign_directory) {
        (Some(command), None) => canonical(command)?,
        (None, Some(campaign)) => campaign_command(campaign)?,
        _ => return Err(CliError::Input("ambiguous inventory source".into())),
    };
    let document = read_json(&command)?;
    let argv: Vec<OsString> = document
        .get("command")
        .unwrap_or(&document)
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .map(|v| v.as_str().map(OsString::from))
                .collect()
        })
        .ok_or_else(|| CliError::Input("walk command must be a JSON list of strings".into()))?;
    let protected_inputs = protected_command_paths(&argv)?;
    let request = super::owner_match::walk_request_from_argv(argv).map_err(CliError::Input)?;
    let checkpoint = args
        .checkpoint
        .clone()
        .or_else(|| {
            args.campaign_directory
                .as_ref()
                .map(|p| p.join("checkpoints/main"))
        })
        .or_else(|| request.checkpoint.as_ref().map(|c| c.directory.clone()))
        .ok_or_else(|| CliError::Input("no checkpoint: pass --checkpoint".into()))?;
    let checkpoint = canonical(&checkpoint)?;
    let mut protected = vec![
        checkpoint.clone(),
        request.matching.owner_base.clone(),
        command.clone(),
    ];
    protected.extend(protected_inputs);
    if let Some(directory) = command.parent() {
        protected.push(directory.to_path_buf());
    }
    if let Some(campaign) = &args.campaign_directory {
        for child in ["inputs", "bin", "steering", "active-run.json"] {
            protected.push(campaign.join(child));
        }
    }
    protect_output(&args.output, &protected)?;
    let mut options = OwnerDomainWalkInventoryOptions::new(checkpoint);
    options.verification.threads = args.threads;
    let cancel = Arc::new(AtomicBool::new(false));
    let _signals = Signals::register(&cancel)?;
    let inventory = owner_domain_walk_inventory(&request, &options, &cancel, |event| {
        let _ = writeln!(std::io::stderr().lock(), "{event}");
    })?;
    let mut report = inventory.summary();
    if include_pages {
        report["rule_page"] = json!(inventory.rules(args.rules_start, args.page_size)?);
        report["terminal_page"] = json!(inventory.terminals(args.terminals_start, args.page_size)?);
    }
    report["normalization_requested"] = json!(args.normalize_terminals);
    if args.normalize_terminals && report["complete"] == true {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::AppError::new(
                crate::AppErrorKind::Cancelled,
                "inventory normalization cancelled",
            )
            .into());
        }
        let normalized = inventory.normalize_terminals(Default::default())?;
        report["normalization"] = normalized.metadata()?;
        if include_pages {
            report["normalized_terminal_page"] =
                json!(normalized.terminals(args.normalized_terminals_start, args.page_size)?);
        }
    } else if args.normalize_terminals {
        report["normalization"] = json!({
            "status":"not-run",
            "reason":"normalization requires a complete validated inventory"
        });
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::AppError::new(
            crate::AppErrorKind::Cancelled,
            "inventory cancelled; saved campaign is unchanged",
        )
        .into());
    }
    Ok(report)
}

fn inventory_verdict(report: &Value) -> Result<(), CliError> {
    if report["complete"] == true && report["verification"]["verdict"] == "PASS" {
        return Ok(());
    }
    Err(CliError::Verdict {
        incomplete: report["verification"]["verdict"] != "FAIL",
        message: format!(
            "inventory is not complete and validated: {} (campaign unchanged)",
            report["verification"]["verdict"]
        ),
    })
}

#[cfg(test)]
mod tests;
