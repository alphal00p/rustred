//! Transport only: native finite master reduction and lazy count inspection.
use super::args::{ArgError, Command, next_utf8_value, next_value, parse_positive_integer};
use super::error::CliError;
use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MasterArgs {
    pub command: PathBuf,
    pub finite_directory: Option<PathBuf>,
    pub finite_policy: Option<PathBuf>,
    pub artifact: Option<PathBuf>,
    pub partial_refinement: Option<PathBuf>,
    pub operation: crate::MasterReductionOperation,
    pub checkpoint: PathBuf,
    pub directory: PathBuf,
    pub previous_artifact: Option<PathBuf>,
    pub collection_artifacts: Vec<PathBuf>,
    pub events: Option<PathBuf>,
    pub stop_file: Option<PathBuf>,
    pub resume: bool,
    pub threads: usize,
    pub deep_verification: Option<bool>,
    pub seed_depth: u32,
    pub containing_sector_depth: u32,
    pub saved_rule_assistance: bool,
    pub circuit_symmetry_assistance: bool,
    pub finite_feedback: Option<bool>,
    pub normalization_profile: Option<crate::MasterNormalizationProfile>,
    pub max_relation_columns: Option<usize>,
    pub checkpoint_interval_seconds: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MasterInspectArgs {
    pub artifact: PathBuf,
    pub finite_policy: Option<PathBuf>,
    pub format: String,
}

pub(super) fn parse(args: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    parse_source(args, false)
}
fn parse_source(
    mut args: impl Iterator<Item = OsString>,
    finite_allowed: bool,
) -> Result<Command, ArgError> {
    let mut result = MasterArgs {
        command: PathBuf::new(),
        finite_directory: None,
        finite_policy: None,
        artifact: None,
        partial_refinement: None,
        operation: crate::MasterReductionOperation::Refine,
        checkpoint: PathBuf::new(),
        directory: PathBuf::new(),
        previous_artifact: None,
        collection_artifacts: Vec::new(),
        events: None,
        stop_file: None,
        resume: false,
        threads: 1,
        deep_verification: None,
        seed_depth: 0,
        containing_sector_depth: 0,
        saved_rule_assistance: false,
        circuit_symmetry_assistance: false,
        finite_feedback: None,
        normalization_profile: None,
        max_relation_columns: None,
        checkpoint_interval_seconds: 3600,
    };
    let mut seen = BTreeSet::new();
    while let Some(option) = args.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match option.as_str() {
            "--command" => "--command",
            "--finite-directory" if finite_allowed => "--finite-directory",
            "--finite-policy" => "--finite-policy",
            "--artifact" => "--artifact",
            "--checkpoint" => "--checkpoint",
            "--directory" => "--directory",
            "--previous-artifact" => "--previous-artifact",
            "--collection-artifact" => "--collection-artifact",
            "--events" => "--events",
            "--stop-file" => "--stop-file",
            "--resume" => "--resume",
            "--threads" => "--threads",
            "--deep-verification" => "--deep-verification",
            "--no-deep-verification" => "--no-deep-verification",
            "--seed-depth" => "--seed-depth",
            "--containing-sector-depth" => "--containing-sector-depth",
            "--saved-rule-assistance" => "--saved-rule-assistance",
            "--circuit-symmetry-assistance" => "--circuit-symmetry-assistance",
            "--finite-feedback" => "--finite-feedback",
            "--no-finite-feedback" => "--no-finite-feedback",
            "--normalization-profile" => "--normalization-profile",
            "--max-relation-columns" => "--max-relation-columns",
            "--checkpoint-interval-seconds" => "--checkpoint-interval-seconds",
            _ => return Err(ArgError::UnknownOption(option)),
        };
        if !seen.insert(option) && option != "--collection-artifact" {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--resume" => result.resume = true,
            "--deep-verification" | "--no-deep-verification" => {
                if result.deep_verification.is_some() {
                    return Err(ArgError::InvalidCombination(
                        "deep verification flags are mutually exclusive",
                    ));
                }
                result.deep_verification = Some(option == "--deep-verification");
            }
            "--saved-rule-assistance" => result.saved_rule_assistance = true,
            "--circuit-symmetry-assistance" => result.circuit_symmetry_assistance = true,
            "--finite-feedback" | "--no-finite-feedback" => {
                if result.finite_feedback.is_some() {
                    return Err(ArgError::InvalidCombination(
                        "finite feedback flags are mutually exclusive",
                    ));
                }
                result.finite_feedback = Some(option == "--finite-feedback");
            }
            "--normalization-profile" => {
                let value = next_utf8_value(&mut args, option)?;
                result.normalization_profile = Some(match value.as_str() {
                    "conservative" => crate::MasterNormalizationProfile::ConservativeV1,
                    "standard" => crate::MasterNormalizationProfile::StandardV1,
                    _ => {
                        return Err(ArgError::InvalidValue {
                            option,
                            value,
                            expected: "conservative or standard",
                        });
                    }
                });
            }
            "--threads" => {
                result.threads =
                    parse_positive_integer(option, next_utf8_value(&mut args, option)?)?
            }
            "--max-relation-columns" => {
                let value = next_utf8_value(&mut args, option)?;
                let limit: usize = value.parse().map_err(|_| ArgError::InvalidValue {
                    option,
                    value,
                    expected: "a nonnegative integer (0 means unlimited)",
                })?;
                result.max_relation_columns = Some(limit);
            }
            "--seed-depth" => {
                let value = next_utf8_value(&mut args, option)?;
                result.seed_depth = value.parse().map_err(|_| ArgError::InvalidValue {
                    option,
                    value,
                    expected: "a nonnegative integer",
                })?;
            }
            "--containing-sector-depth" => {
                let value = next_utf8_value(&mut args, option)?;
                result.containing_sector_depth =
                    value.parse().map_err(|_| ArgError::InvalidValue {
                        option,
                        value,
                        expected: "a nonnegative integer",
                    })?;
            }
            "--checkpoint-interval-seconds" => {
                let value = next_utf8_value(&mut args, option)?;
                result.checkpoint_interval_seconds =
                    value
                        .parse()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or(ArgError::InvalidValue {
                            option,
                            value,
                            expected: "a positive integer",
                        })?;
            }
            _ => {
                let path = PathBuf::from(next_value(&mut args, option)?);
                if path.as_os_str().is_empty() {
                    return Err(ArgError::EmptyPath);
                }
                match option {
                    "--command" => result.command = path,
                    "--finite-directory" => result.finite_directory = Some(path),
                    "--finite-policy" => result.finite_policy = Some(path),
                    "--artifact" => result.artifact = Some(path),
                    "--checkpoint" => result.checkpoint = path,
                    "--directory" => result.directory = path,
                    "--previous-artifact" => result.previous_artifact = Some(path),
                    "--collection-artifact" => result.collection_artifacts.push(path),
                    "--events" => result.events = Some(path),
                    "--stop-file" => result.stop_file = Some(path),
                    _ => unreachable!(),
                }
            }
        }
    }
    let has_command = !result.command.as_os_str().is_empty();
    let has_checkpoint = !result.checkpoint.as_os_str().is_empty();
    let finite = result.finite_directory.is_some();
    if finite
        && (result.finite_directory.is_none()
            || result.finite_policy.is_none()
            || !has_command
            || has_checkpoint
            || result.artifact.is_some()
            || result.directory.as_os_str().is_empty())
    {
        return Err(ArgError::InvalidCombination(
            "finite publication requires --command, --finite-directory, --finite-policy and --directory, with no CP6 --checkpoint or --artifact",
        ));
    }
    if !finite
        && (result.directory.as_os_str().is_empty()
            || (result.artifact.is_some() && (has_command || has_checkpoint))
            || (result.artifact.is_none() && (!has_command || !has_checkpoint)))
    {
        return Err(ArgError::InvalidCombination(
            "requires --directory and either --artifact or both --command and --checkpoint",
        ));
    }
    if result.artifact.is_some() && result.previous_artifact.is_some() {
        return Err(ArgError::InvalidCombination(
            "--artifact already supplies the previous state",
        ));
    }
    if !result.collection_artifacts.is_empty() && result.artifact.is_none() {
        return Err(ArgError::InvalidCombination(
            "--collection-artifact requires a published --artifact source",
        ));
    }
    if result.artifact.is_some() && result.deep_verification.is_some() {
        return Err(ArgError::InvalidCombination(
            "deep verification applies to publication from a saved checkpoint, not artifact refinement",
        ));
    }
    if result.resume && result.previous_artifact.is_some() {
        return Err(ArgError::InvalidCombination(
            "--resume and --previous-artifact cannot be combined",
        ));
    }
    Ok(Command::WalkMasterReduce(result))
}

pub(super) fn parse_publish(args: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let Command::WalkMasterReduce(mut args) = parse_source(args, true)? else {
        unreachable!()
    };
    if args.artifact.is_some() {
        return Err(ArgError::InvalidCombination(
            "walk-publish requires a saved campaign --command and --checkpoint",
        ));
    }
    if args.saved_rule_assistance
        || args.circuit_symmetry_assistance
        || args.finite_feedback.is_some()
        || args.containing_sector_depth != 0
        || !args.collection_artifacts.is_empty()
        || args.max_relation_columns.is_some()
    {
        return Err(ArgError::InvalidCombination(
            "additional terminal-search strategies require refinement and cannot be used with walk-publish",
        ));
    }
    args.operation = crate::MasterReductionOperation::Publish;
    Ok(Command::WalkMasterReduce(args))
}

pub(super) fn parse_finalize(
    mut args: impl Iterator<Item = OsString>,
) -> Result<Command, ArgError> {
    // Keep export controls separate from the search recipe: this command may
    // materialize proved maps, but must never start another finite search.
    let mut forwarded = Vec::new();
    while let Some(option) = args.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--resume" => forwarded.push(OsString::from(option)),
            "--refinement"
            | "--finite-policy"
            | "--directory"
            | "--threads"
            | "--events"
            | "--stop-file"
            | "--checkpoint-interval-seconds" => {
                let value = next_value(
                    &mut args,
                    match option.as_str() {
                        "--refinement" => "--refinement",
                        "--finite-policy" => "--finite-policy",
                        "--directory" => "--directory",
                        "--threads" => "--threads",
                        "--events" => "--events",
                        "--stop-file" => "--stop-file",
                        _ => "--checkpoint-interval-seconds",
                    },
                )?;
                forwarded.push(OsString::from(if option == "--refinement" {
                    "--artifact"
                } else {
                    &option
                }));
                forwarded.push(value);
            }
            _ => return Err(ArgError::UnknownOption(option)),
        }
    }
    let Command::WalkMasterReduce(mut args) = parse(forwarded.into_iter())? else {
        unreachable!()
    };
    args.partial_refinement = args.artifact.take();
    args.operation = crate::MasterReductionOperation::Publish;
    Ok(Command::WalkMasterReduce(args))
}

pub(super) fn parse_inspect(mut args: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut result = MasterInspectArgs {
        artifact: PathBuf::new(),
        finite_policy: None,
        format: "auto".into(),
    };
    let mut seen = BTreeSet::new();
    while let Some(option) = args.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match option.as_str() {
            "--artifact" => "--artifact",
            "--finite-policy" => "--finite-policy",
            "--format" => "--format",
            _ => return Err(ArgError::UnknownOption(option)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        if option == "--artifact" {
            result.artifact = PathBuf::from(next_value(&mut args, option)?);
        } else if option == "--finite-policy" {
            result.finite_policy = Some(PathBuf::from(next_value(&mut args, option)?));
        } else {
            let value = next_utf8_value(&mut args, option)?;
            if !["auto", "table", "json"].contains(&value.as_str()) {
                return Err(ArgError::InvalidValue {
                    option,
                    value,
                    expected: "auto, table or json",
                });
            }
            result.format = value;
        }
    }
    if result.artifact.as_os_str().is_empty() {
        return Err(ArgError::InvalidCombination(
            "master-inspect requires --artifact",
        ));
    }
    Ok(Command::MasterInspect(result))
}

pub(super) fn inspect(args: MasterInspectArgs) -> Result<(), CliError> {
    let policy = args
        .finite_policy
        .as_ref()
        .map(|p| super::finite_demand::read_policy(p))
        .transpose()?;
    let report = if let Some(policy) = &policy {
        crate::master_reduction_inspect_with_policy(&args.artifact, policy)?
    } else {
        crate::master_reduction_inspect(&args.artifact)?
    };
    let terminal = std::io::stdout().is_terminal();
    if args.format == "json" || args.format == "auto" && !terminal {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| CliError::OutputIo(e.to_string()))?
        );
    } else {
        let width = crossterm::terminal::size().map_or(110, |(w, _)| w as usize);
        let color = terminal && std::env::var_os("NO_COLOR").is_none();
        println!(
            "{}",
            super::master_table::render_master_table(&report, color, width)
        );
    }
    Ok(())
}

pub(super) fn run(args: MasterArgs) -> Result<(), CliError> {
    super::routed::preflight_inner_pools()?;
    let finite_policy = args
        .finite_policy
        .as_ref()
        .map(|p| super::finite_demand::read_policy(p))
        .transpose()?;
    let source = args.artifact.as_ref().or(args.partial_refinement.as_ref());
    let mut finite_inert = Vec::new();
    let (request, mut protected) = if let Some(source) = source {
        (None, vec![artifact_directory(source)?])
    } else if args.finite_directory.is_some() {
        let (request, protected, inert) = super::finite_demand::read_source_with_policy(
            &args.command,
            finite_policy
                .as_ref()
                .expect("finite source policy validated"),
        )?;
        finite_inert = inert;
        (Some(request), protected)
    } else {
        let file =
            std::fs::File::open(&args.command).map_err(|e| CliError::InputIo(e.to_string()))?;
        let bytes = super::io::read_bounded(file, "saved walk request", crate::MAX_INPUT_BYTES)?;
        let document: Value =
            serde_json::from_slice(&bytes).map_err(|e| CliError::Input(e.to_string()))?;
        let argv: Vec<OsString> = document
            .get("command")
            .unwrap_or(&document)
            .as_array()
            .and_then(|a| a.iter().map(|v| v.as_str().map(OsString::from)).collect())
            .ok_or_else(|| CliError::Input("saved command must be a list of strings".into()))?;
        let protected = super::walk_inventory::protected_command_paths(&argv)?;
        let request = super::owner_match::walk_request_from_argv(argv).map_err(CliError::Input)?;
        (Some(request), protected)
    };
    if let Some(policy_path) = &args.finite_policy {
        protected.push(policy_path.clone());
    }
    if source.is_none() {
        protected.push(args.command.clone());
        if let Some(finite) = &args.finite_directory {
            protected.push(finite.clone());
            protected.push(args.finite_policy.clone().unwrap());
        } else {
            protected.push(args.checkpoint.clone());
        }
    }
    if let Some(previous) = &args.previous_artifact {
        protected.push(artifact_directory(previous)?);
    }
    for peer in &args.collection_artifacts {
        if peer.exists() {
            protected.push(artifact_directory(peer)?);
        }
    }
    if args.finite_directory.is_some() {
        // These protected paths include inert legacy carrier outputs. Do not
        // open or canonicalize them merely to publish a finite source.
        super::finite_demand::protect(&args.directory, &protected, &finite_inert)?;
        if let Some(events) = &args.events {
            super::finite_demand::protect(events, &protected, &finite_inert)?;
        }
        if let Some(stop) = &args.stop_file {
            super::finite_demand::protect(stop, &protected, &finite_inert)?;
        }
    } else {
        let proposed_directory = prospective_path(&args.directory)?;
        for source in &protected {
            let source = prospective_path(source)?;
            if proposed_directory.starts_with(&source) || source.starts_with(&proposed_directory) {
                return Err(CliError::Input(
                    "phase directory overlaps protected campaign data".into(),
                ));
            }
        }
        super::walk_inventory::protect_output(
            &super::args::StreamPath::File(args.directory.clone()),
            &protected,
        )?;
        if let Some(events) = &args.events {
            super::walk_inventory::protect_output(
                &super::args::StreamPath::File(events.clone()),
                &protected,
            )?;
        }
    }
    // The phase directory must never be an original input/checkpoint or its
    // ancestor. Explicit paths avoid accidental writes to production state.
    let directory = if args.partial_refinement.is_some() || args.finite_directory.is_some() {
        // Native export acquires the source lock before creating its output.
        prospective_path(&args.directory)?
    } else {
        std::fs::create_dir_all(&args.directory).map_err(|e| CliError::OutputIo(e.to_string()))?;
        args.directory
            .canonicalize()
            .map_err(|e| CliError::InputIo(e.to_string()))?
    };
    let checkpoint = args
        .finite_directory
        .as_ref()
        .map(|p| p.join("store"))
        .unwrap_or_else(|| args.checkpoint.clone());
    let mut options = crate::MasterReductionOptions::new(&checkpoint, &directory);
    options.finite_policy = finite_policy.clone();
    options.operation = args.operation;
    options.resume = args.resume;
    options.previous_artifact = args.previous_artifact;
    options.collection_artifacts = args.collection_artifacts;
    options.threads = args.threads;
    options.deep_verification = args.deep_verification;
    options.seed_depth = args.seed_depth;
    options.containing_sector_depth = args.containing_sector_depth;
    options.saved_rule_assistance = args.saved_rule_assistance;
    options.circuit_symmetry_assistance = args.circuit_symmetry_assistance;
    options.finite_feedback = args.finite_feedback;
    options.normalization_profile = args.normalization_profile;
    options.max_relation_columns = args.max_relation_columns;
    options.checkpoint_interval = Duration::from_secs(args.checkpoint_interval_seconds);
    let cancel = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let mut signals = Vec::new();
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signals.push(
            signal_hook::flag::register(signal, Arc::clone(&cancel))
                .map_err(|e| CliError::InputIo(e.to_string()))?,
        );
    }
    let events = if let Some(path) = args.events {
        if (!args.checkpoint.as_os_str().is_empty() && path.starts_with(&args.checkpoint))
            || path == args.command
        {
            return Err(CliError::Input("events path overlaps saved inputs".into()));
        }
        Some(Mutex::new(
            std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(path)
                .map_err(|e| CliError::OutputIo(e.to_string()))?,
        ))
    } else {
        None
    };
    let stop_file = args.stop_file;
    let result = std::thread::scope(|scope| {
        if let Some(path) = stop_file {
            let cancel = &cancel;
            let finished = &finished;
            scope.spawn(move || {
                while !finished.load(Ordering::Relaxed) {
                    if path.exists() {
                        cancel.store(true, Ordering::Relaxed);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        }
        let observe = |event| {
            if let Some(events) = &events {
                let _ = writeln!(events.lock().expect("events writer"), "{event}");
            } else {
                let _ = writeln!(std::io::stderr().lock(), "{event}");
            }
        };
        let result = if let Some(source) = &args.partial_refinement {
            crate::master_publish_partial_refinement(source, &options, &cancel, observe)
        } else if let Some(source) = &args.artifact {
            crate::master_refine_published_artifact(source, &options, &cancel, observe)
        } else if let Some(finite) = &args.finite_directory {
            crate::publish_finite_demand_with_progress(
                crate::FiniteDemandRunRequest {
                    source: request.clone().expect("finite source request"),
                    policy: finite_policy.clone().expect("finite policy"),
                    directory: finite.clone(),
                    mode: crate::FiniteDemandMode::Resume,
                },
                &options,
                &cancel,
                observe,
            )
        } else {
            crate::master_reduce_saved_campaign(
                request.as_ref().expect("source request"),
                &options,
                &cancel,
                observe,
            )
        };
        finished.store(true, Ordering::Relaxed);
        result
    });
    for signal in signals {
        signal_hook::low_level::unregister(signal);
    }
    let report = result?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| CliError::OutputIo(e.to_string()))?
    );
    if report["status"] == "paused" {
        return Err(CliError::Input(format!(
            "Master reduction paused; resume checkpoint {}",
            directory.display()
        )));
    }
    Ok(())
}

fn artifact_directory(path: &Path) -> Result<PathBuf, CliError> {
    let path = path
        .canonicalize()
        .map_err(|e| CliError::InputIo(e.to_string()))?;
    Ok(if path.is_file() {
        path.parent()
            .expect("canonical file has parent")
            .to_path_buf()
    } else {
        path
    })
}

fn prospective_path(path: &Path) -> Result<PathBuf, CliError> {
    if path.exists() {
        return path
            .canonicalize()
            .map_err(|e| CliError::InputIo(e.to_string()));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| CliError::Input("invalid destination path".into()))?;
    Ok(prospective_path(parent)?.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_policy_is_shared_by_artifact_cold_read_and_refinement_commands() {
        let Command::WalkMasterReduce(refinement) = parse(
            [
                "--artifact",
                "source",
                "--directory",
                "out",
                "--finite-policy",
                "policy.json",
            ]
            .map(OsString::from)
            .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(refinement.finite_policy, Some(PathBuf::from("policy.json")));
        let Command::WalkMasterReduce(finalize) = parse_finalize(
            [
                "--refinement",
                "source",
                "--directory",
                "out",
                "--finite-policy",
                "policy.json",
            ]
            .map(OsString::from)
            .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(finalize.finite_policy, refinement.finite_policy);
        let Command::MasterInspect(inspect) = parse_inspect(
            ["--artifact", "source", "--finite-policy", "policy.json"]
                .map(OsString::from)
                .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(inspect.finite_policy, refinement.finite_policy);
        assert!(
            super::super::artifact_inspect::parse(
                ["--artifact", "source", "--finite-policy", "policy.json"]
                    .map(OsString::from)
                    .into_iter(),
            )
            .is_ok()
        );
    }

    #[test]
    fn partial_publication_has_no_search_controls() {
        let Command::WalkMasterReduce(args) = parse_finalize(
            ["--refinement", "stopped", "--directory", "out", "--resume"]
                .map(OsString::from)
                .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(args.operation, crate::MasterReductionOperation::Publish);
        assert_eq!(args.partial_refinement, Some(PathBuf::from("stopped")));
        assert!(args.artifact.is_none());
        assert!(args.resume);
        for option in [
            "--seed-depth",
            "--max-relation-columns",
            "--saved-rule-assistance",
            "--artifact",
        ] {
            assert!(
                parse_finalize(
                    ["--refinement", "stopped", "--directory", "out", option, "1"]
                        .map(OsString::from)
                        .into_iter()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn relation_column_budget_distinguishes_omission_finite_and_explicit_unlimited() {
        let base = ["--artifact", "source", "--directory", "out"];
        let Command::WalkMasterReduce(default) =
            parse(base.map(OsString::from).into_iter()).unwrap()
        else {
            panic!()
        };
        assert_eq!(default.max_relation_columns, None);
        for (limit, expected) in [("0", Some(0)), ("2000000", Some(2_000_000))] {
            let Command::WalkMasterReduce(args) = parse(
                base.into_iter()
                    .chain(["--max-relation-columns", limit])
                    .map(OsString::from),
            )
            .unwrap() else {
                panic!()
            };
            assert_eq!(args.max_relation_columns, expected);
        }
        assert!(
            parse(
                base.into_iter()
                    .chain(["--max-relation-columns", "-1"])
                    .map(OsString::from)
            )
            .is_err()
        );
    }
    #[test]
    fn normalization_profile_is_optional_explicit_and_validated() {
        let base = ["--artifact", "source", "--directory", "out"];
        let Command::WalkMasterReduce(default) =
            parse(base.map(OsString::from).into_iter()).unwrap()
        else {
            panic!()
        };
        assert_eq!(default.normalization_profile, None);
        for (name, expected) in [
            (
                "conservative",
                crate::MasterNormalizationProfile::ConservativeV1,
            ),
            ("standard", crate::MasterNormalizationProfile::StandardV1),
        ] {
            let Command::WalkMasterReduce(parsed) = parse(
                base.into_iter()
                    .chain(["--normalization-profile", name])
                    .map(OsString::from),
            )
            .unwrap() else {
                panic!()
            };
            assert_eq!(parsed.normalization_profile, Some(expected));
        }
        assert!(
            parse(
                base.into_iter()
                    .chain(["--normalization-profile", "unbounded"])
                    .map(OsString::from)
            )
            .is_err()
        );
        assert!(
            parse(
                base.into_iter()
                    .chain([
                        "--normalization-profile",
                        "standard",
                        "--normalization-profile",
                        "standard"
                    ])
                    .map(OsString::from)
            )
            .is_err()
        );
    }
    #[test]
    fn publication_deep_verification_is_optional_and_excludes_artifact_refinement() {
        let base = [
            "--command",
            "request",
            "--checkpoint",
            "cp",
            "--directory",
            "out",
        ];
        let Command::WalkMasterReduce(default) =
            parse_publish(base.map(OsString::from).into_iter()).unwrap()
        else {
            panic!()
        };
        assert_eq!(default.deep_verification, None);
        for (flag, expected) in [
            ("--deep-verification", true),
            ("--no-deep-verification", false),
        ] {
            let Command::WalkMasterReduce(parsed) =
                parse_publish(base.into_iter().chain([flag]).map(OsString::from)).unwrap()
            else {
                panic!()
            };
            assert_eq!(parsed.deep_verification, Some(expected));
            assert!(
                parse(
                    ["--artifact", "source", "--directory", "out", flag]
                        .map(OsString::from)
                        .into_iter()
                )
                .is_err()
            );
        }
        assert!(
            parse_publish(
                base.into_iter()
                    .chain(["--deep-verification", "--no-deep-verification"])
                    .map(OsString::from)
            )
            .is_err()
        );
    }

    #[test]
    fn finite_feedback_is_optional_boolean_and_refine_only() {
        let base = ["--artifact", "source", "--directory", "out"];
        let Command::WalkMasterReduce(default) =
            parse(base.map(OsString::from).into_iter()).unwrap()
        else {
            panic!()
        };
        assert_eq!(default.finite_feedback, None);
        for (flag, expected) in [("--finite-feedback", true), ("--no-finite-feedback", false)] {
            let Command::WalkMasterReduce(parsed) =
                parse(base.into_iter().chain([flag]).map(OsString::from)).unwrap()
            else {
                panic!()
            };
            assert_eq!(parsed.finite_feedback, Some(expected));
            assert!(
                parse_publish(
                    [
                        "--command",
                        "request",
                        "--checkpoint",
                        "cp",
                        "--directory",
                        "out",
                        flag
                    ]
                    .map(OsString::from)
                    .into_iter()
                )
                .is_err()
            );
        }
        assert!(
            parse(
                base.into_iter()
                    .chain(["--finite-feedback", "--no-finite-feedback"])
                    .map(OsString::from)
            )
            .is_err()
        );
    }

    #[test]
    fn circuit_assistance_is_explicit_independent_and_refine_only() {
        let base = ["--artifact", "source", "--directory", "out"];
        let Command::WalkMasterReduce(default) =
            parse(base.map(OsString::from).into_iter()).unwrap()
        else {
            panic!()
        };
        assert!(!default.circuit_symmetry_assistance);
        for saved in [false, true] {
            let mut args: Vec<_> = base
                .into_iter()
                .chain(["--circuit-symmetry-assistance"])
                .map(OsString::from)
                .collect();
            if saved {
                args.push(OsString::from("--saved-rule-assistance"));
            }
            let Command::WalkMasterReduce(parsed) = parse(args.clone().into_iter()).unwrap() else {
                panic!()
            };
            assert!(parsed.circuit_symmetry_assistance);
            assert_eq!(parsed.saved_rule_assistance, saved);
            args.push(OsString::from("--circuit-symmetry-assistance"));
            assert!(matches!(
                parse(args.into_iter()),
                Err(ArgError::DuplicateOption("--circuit-symmetry-assistance"))
            ));
        }
        assert!(
            parse_publish(
                [
                    "--command",
                    "request",
                    "--checkpoint",
                    "cp",
                    "--directory",
                    "out",
                    "--circuit-symmetry-assistance",
                ]
                .map(OsString::from)
                .into_iter()
            )
            .is_err()
        );
    }
    #[test]
    fn containing_sector_search_is_explicit_and_nonnegative() {
        let argv = [
            "--artifact",
            "source",
            "--directory",
            "out",
            "--containing-sector-depth",
            "2",
        ];
        let Command::WalkMasterReduce(args) = parse(argv.map(OsString::from).into_iter()).unwrap()
        else {
            panic!()
        };
        assert_eq!(args.containing_sector_depth, 2);
        assert_eq!(args.seed_depth, 0);
        assert!(!args.saved_rule_assistance);
        assert!(
            parse(
                [
                    "--artifact",
                    "source",
                    "--directory",
                    "out",
                    "--containing-sector-depth",
                    "-1"
                ]
                .map(OsString::from)
                .into_iter()
            )
            .is_err()
        );
        assert!(
            parse_publish(
                [
                    "--command",
                    "request",
                    "--checkpoint",
                    "cp",
                    "--directory",
                    "out",
                    "--containing-sector-depth",
                    "1"
                ]
                .map(OsString::from)
                .into_iter()
            )
            .is_err()
        );
    }
    #[test]
    fn master_inspect_format_contract() {
        assert!(
            parse_inspect(
                ["--artifact", "sample", "--format", "table"]
                    .map(OsString::from)
                    .into_iter()
            )
            .is_ok()
        );
        assert!(
            parse_inspect(
                ["--artifact", "sample", "--format", "xml"]
                    .map(OsString::from)
                    .into_iter()
            )
            .is_err()
        );
    }
    #[test]
    fn master_phase_requires_complete_source() {
        assert!(parse(["--command", "request"].map(OsString::from).into_iter()).is_err());
        let Command::WalkMasterReduce(args) = parse(
            [
                "--command",
                "request",
                "--checkpoint",
                "cp",
                "--directory",
                "out",
            ]
            .map(OsString::from)
            .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(args.seed_depth, 0);
    }

    #[test]
    fn publication_and_refinement_have_explicit_distinct_sources() {
        let Command::WalkMasterReduce(args) = parse_publish(
            [
                "--command",
                "request",
                "--checkpoint",
                "cp",
                "--directory",
                "out",
            ]
            .map(OsString::from)
            .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(args.operation, crate::MasterReductionOperation::Publish);
        let Command::WalkMasterReduce(args) = parse(
            ["--artifact", "source", "--directory", "out"]
                .map(OsString::from)
                .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(args.operation, crate::MasterReductionOperation::Refine);
        for argv in [
            vec![
                "--artifact",
                "source",
                "--command",
                "request",
                "--directory",
                "out",
            ],
            vec![
                "--artifact",
                "source",
                "--previous-artifact",
                "previous",
                "--directory",
                "out",
            ],
        ] {
            assert!(parse(argv.into_iter().map(OsString::from)).is_err());
        }
        assert!(
            parse_publish(
                ["--artifact", "source", "--directory", "out"]
                    .map(OsString::from)
                    .into_iter()
            )
            .is_err()
        );
    }

    #[test]
    fn saved_rule_assistance_is_an_explicit_refinement_only_flag() {
        let Command::WalkMasterReduce(args) = parse(
            [
                "--artifact",
                "source",
                "--directory",
                "out",
                "--saved-rule-assistance",
            ]
            .map(OsString::from)
            .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert!(args.saved_rule_assistance);
        assert_eq!(args.operation, crate::MasterReductionOperation::Refine);
        assert!(
            parse_publish(
                [
                    "--command",
                    "request",
                    "--checkpoint",
                    "cp",
                    "--directory",
                    "out",
                    "--saved-rule-assistance"
                ]
                .map(OsString::from)
                .into_iter(),
            )
            .is_err()
        );
        assert!(matches!(
            parse(
                [
                    "--artifact",
                    "source",
                    "--directory",
                    "out",
                    "--saved-rule-assistance",
                    "--saved-rule-assistance"
                ]
                .map(OsString::from)
                .into_iter(),
            ),
            Err(ArgError::DuplicateOption("--saved-rule-assistance"))
        ));
    }

    #[test]
    fn collection_peers_are_repeatable_only_with_a_published_source() {
        let Command::WalkMasterReduce(args) = parse(
            [
                "--artifact",
                "source",
                "--directory",
                "out",
                "--collection-artifact",
                "peer-a",
                "--collection-artifact",
                "peer-b",
            ]
            .map(OsString::from)
            .into_iter(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            args.collection_artifacts,
            vec![PathBuf::from("peer-a"), PathBuf::from("peer-b")]
        );
        assert!(
            parse(
                [
                    "--command",
                    "request",
                    "--checkpoint",
                    "cp",
                    "--directory",
                    "out",
                    "--collection-artifact",
                    "peer"
                ]
                .map(OsString::from)
                .into_iter()
            )
            .is_err()
        );
    }

    #[test]
    fn artifact_manifest_protects_its_entire_source_directory() {
        let scratch =
            std::env::temp_dir().join(format!("rustred-cli-master-source-{}", std::process::id()));
        std::fs::create_dir(&scratch).unwrap();
        let file = scratch.join("artifact.json");
        std::fs::write(&file, b"{}").unwrap();
        assert_eq!(
            artifact_directory(&file).unwrap(),
            scratch.canonicalize().unwrap()
        );
        assert_eq!(
            artifact_directory(&scratch).unwrap(),
            scratch.canonicalize().unwrap()
        );
        std::fs::remove_dir_all(scratch).unwrap();
    }
}
