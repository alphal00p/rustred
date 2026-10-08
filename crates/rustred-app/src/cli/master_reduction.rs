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
    pub artifact: Option<PathBuf>,
    pub operation: crate::MasterReductionOperation,
    pub checkpoint: PathBuf,
    pub directory: PathBuf,
    pub previous_artifact: Option<PathBuf>,
    pub collection_artifacts: Vec<PathBuf>,
    pub events: Option<PathBuf>,
    pub stop_file: Option<PathBuf>,
    pub resume: bool,
    pub threads: usize,
    pub seed_depth: u32,
    pub containing_sector_depth: u32,
    pub saved_rule_assistance: bool,
    pub circuit_symmetry_assistance: bool,
    pub finite_feedback: Option<bool>,
    pub normalization_profile: Option<crate::MasterNormalizationProfile>,
    pub checkpoint_interval_seconds: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MasterInspectArgs {
    pub artifact: PathBuf,
    pub format: String,
}

pub(super) fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut result = MasterArgs {
        command: PathBuf::new(),
        artifact: None,
        operation: crate::MasterReductionOperation::Refine,
        checkpoint: PathBuf::new(),
        directory: PathBuf::new(),
        previous_artifact: None,
        collection_artifacts: Vec::new(),
        events: None,
        stop_file: None,
        resume: false,
        threads: 1,
        seed_depth: 0,
        containing_sector_depth: 0,
        saved_rule_assistance: false,
        circuit_symmetry_assistance: false,
        finite_feedback: None,
        normalization_profile: None,
        checkpoint_interval_seconds: 3600,
    };
    let mut seen = BTreeSet::new();
    while let Some(option) = args.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match option.as_str() {
            "--command" => "--command",
            "--artifact" => "--artifact",
            "--checkpoint" => "--checkpoint",
            "--directory" => "--directory",
            "--previous-artifact" => "--previous-artifact",
            "--collection-artifact" => "--collection-artifact",
            "--events" => "--events",
            "--stop-file" => "--stop-file",
            "--resume" => "--resume",
            "--threads" => "--threads",
            "--seed-depth" => "--seed-depth",
            "--containing-sector-depth" => "--containing-sector-depth",
            "--saved-rule-assistance" => "--saved-rule-assistance",
            "--circuit-symmetry-assistance" => "--circuit-symmetry-assistance",
            "--finite-feedback" => "--finite-feedback",
            "--no-finite-feedback" => "--no-finite-feedback",
            "--normalization-profile" => "--normalization-profile",
            "--checkpoint-interval-seconds" => "--checkpoint-interval-seconds",
            _ => return Err(ArgError::UnknownOption(option)),
        };
        if !seen.insert(option) && option != "--collection-artifact" {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--resume" => result.resume = true,
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
    if result.directory.as_os_str().is_empty()
        || (result.artifact.is_some() && (has_command || has_checkpoint))
        || (result.artifact.is_none() && (!has_command || !has_checkpoint))
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
    if result.resume && result.previous_artifact.is_some() {
        return Err(ArgError::InvalidCombination(
            "--resume and --previous-artifact cannot be combined",
        ));
    }
    Ok(Command::WalkMasterReduce(result))
}

pub(super) fn parse_publish(args: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let Command::WalkMasterReduce(mut args) = parse(args)? else {
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
    {
        return Err(ArgError::InvalidCombination(
            "additional terminal-search strategies require refinement and cannot be used with walk-publish",
        ));
    }
    args.operation = crate::MasterReductionOperation::Publish;
    Ok(Command::WalkMasterReduce(args))
}

pub(super) fn parse_inspect(mut args: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut result = MasterInspectArgs {
        artifact: PathBuf::new(),
        format: "auto".into(),
    };
    let mut seen = BTreeSet::new();
    while let Some(option) = args.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match option.as_str() {
            "--artifact" => "--artifact",
            "--format" => "--format",
            _ => return Err(ArgError::UnknownOption(option)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        if option == "--artifact" {
            result.artifact = PathBuf::from(next_value(&mut args, option)?);
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
    let report = crate::master_reduction_inspect(&args.artifact)?;
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
    let (request, mut protected) = if let Some(source) = &args.artifact {
        (None, vec![artifact_directory(source)?])
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
    if args.artifact.is_none() {
        protected.extend([args.command.clone(), args.checkpoint.clone()]);
    }
    if let Some(previous) = &args.previous_artifact {
        protected.push(artifact_directory(previous)?);
    }
    for peer in &args.collection_artifacts {
        if peer.exists() {
            protected.push(artifact_directory(peer)?);
        }
    }
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
    // The phase directory must never be an original input/checkpoint or its
    // ancestor. Explicit paths avoid accidental writes to production state.
    std::fs::create_dir_all(&args.directory).map_err(|e| CliError::OutputIo(e.to_string()))?;
    let directory = args
        .directory
        .canonicalize()
        .map_err(|e| CliError::InputIo(e.to_string()))?;
    let mut options = crate::MasterReductionOptions::new(&args.checkpoint, &directory);
    options.operation = args.operation;
    options.resume = args.resume;
    options.previous_artifact = args.previous_artifact;
    options.collection_artifacts = args.collection_artifacts;
    options.threads = args.threads;
    options.seed_depth = args.seed_depth;
    options.containing_sector_depth = args.containing_sector_depth;
    options.saved_rule_assistance = args.saved_rule_assistance;
    options.circuit_symmetry_assistance = args.circuit_symmetry_assistance;
    options.finite_feedback = args.finite_feedback;
    options.normalization_profile = args.normalization_profile;
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
        let result = if let Some(source) = &args.artifact {
            crate::master_refine_published_artifact(source, &options, &cancel, observe)
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
