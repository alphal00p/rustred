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
    pub checkpoint: PathBuf,
    pub directory: PathBuf,
    pub previous_artifact: Option<PathBuf>,
    pub events: Option<PathBuf>,
    pub stop_file: Option<PathBuf>,
    pub resume: bool,
    pub threads: usize,
    pub seed_depth: u32,
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
        checkpoint: PathBuf::new(),
        directory: PathBuf::new(),
        previous_artifact: None,
        events: None,
        stop_file: None,
        resume: false,
        threads: 1,
        seed_depth: 0,
        checkpoint_interval_seconds: 3600,
    };
    let mut seen = BTreeSet::new();
    while let Some(option) = args.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match option.as_str() {
            "--command" => "--command",
            "--checkpoint" => "--checkpoint",
            "--directory" => "--directory",
            "--previous-artifact" => "--previous-artifact",
            "--events" => "--events",
            "--stop-file" => "--stop-file",
            "--resume" => "--resume",
            "--threads" => "--threads",
            "--seed-depth" => "--seed-depth",
            "--checkpoint-interval-seconds" => "--checkpoint-interval-seconds",
            _ => return Err(ArgError::UnknownOption(option)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--resume" => result.resume = true,
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
                    "--checkpoint" => result.checkpoint = path,
                    "--directory" => result.directory = path,
                    "--previous-artifact" => result.previous_artifact = Some(path),
                    "--events" => result.events = Some(path),
                    "--stop-file" => result.stop_file = Some(path),
                    _ => unreachable!(),
                }
            }
        }
    }
    if result.command.as_os_str().is_empty()
        || result.checkpoint.as_os_str().is_empty()
        || result.directory.as_os_str().is_empty()
    {
        return Err(ArgError::InvalidCombination(
            "walk-master-reduce requires --command, --checkpoint and --directory",
        ));
    }
    if result.resume && result.previous_artifact.is_some() {
        return Err(ArgError::InvalidCombination(
            "--resume and --previous-artifact cannot be combined",
        ));
    }
    Ok(Command::WalkMasterReduce(result))
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
    let file = std::fs::File::open(&args.command).map_err(|e| CliError::InputIo(e.to_string()))?;
    let bytes = super::io::read_bounded(file, "saved walk request", crate::MAX_INPUT_BYTES)?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|e| CliError::Input(e.to_string()))?;
    let argv: Vec<OsString> = document
        .get("command")
        .unwrap_or(&document)
        .as_array()
        .and_then(|a| a.iter().map(|v| v.as_str().map(OsString::from)).collect())
        .ok_or_else(|| CliError::Input("saved command must be a list of strings".into()))?;
    let mut protected = super::walk_inventory::protected_command_paths(&argv)?;
    protected.extend([args.command.clone(), args.checkpoint.clone()]);
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
    let request = super::owner_match::walk_request_from_argv(argv).map_err(CliError::Input)?;
    // The phase directory must never be an original input/checkpoint or its
    // ancestor. Explicit paths avoid accidental writes to production state.
    std::fs::create_dir_all(&args.directory).map_err(|e| CliError::OutputIo(e.to_string()))?;
    let directory = args
        .directory
        .canonicalize()
        .map_err(|e| CliError::InputIo(e.to_string()))?;
    for source in [
        &request.matching.owner_base,
        &args.checkpoint,
        &args.command,
    ] {
        let source = source
            .canonicalize()
            .map_err(|e| CliError::InputIo(e.to_string()))?;
        if source.starts_with(&directory) || directory.starts_with(&source) {
            return Err(CliError::Input(
                "phase directory overlaps saved campaign inputs/checkpoint".into(),
            ));
        }
    }
    let mut options = crate::MasterReductionOptions::new(&args.checkpoint, &directory);
    options.resume = args.resume;
    options.previous_artifact = args.previous_artifact;
    options.threads = args.threads;
    options.seed_depth = args.seed_depth;
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
        if path.starts_with(&args.checkpoint) || path == args.command {
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
        let result = crate::master_reduce_saved_campaign(&request, &options, &cancel, |event| {
            if let Some(events) = &events {
                let _ = writeln!(events.lock().expect("events writer"), "{event}");
            } else {
                let _ = writeln!(std::io::stderr().lock(), "{event}");
            }
        });
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
}
