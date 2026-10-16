//! Transport for the separate finite-demand engine; no CP6 checkpoint parsing.
use super::{
    args::{self, ArgError, Command, StreamPath, next_utf8_value, next_value},
    error::CliError,
    io::{preflight_output_destination, read_bounded},
    progress::RoutedProgress,
};
use crate::{FiniteDemandMode, FiniteDemandPolicy, FiniteDemandRunRequest};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, IsTerminal, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Run,
    Inspect,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FiniteArgs {
    action: Action,
    directory: PathBuf,
    command: Option<PathBuf>,
    policy: Option<PathBuf>,
    output: Option<PathBuf>,
    events: Option<PathBuf>,
    stop_file: Option<PathBuf>,
    resume: bool,
    repair_preparation: bool,
    no_progress: bool,
    format: String,
}
pub(super) fn parse(mut argv: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let action = match argv.next().and_then(|s| s.into_string().ok()).as_deref() {
        Some("run") => Action::Run,
        Some("inspect") => Action::Inspect,
        _ => {
            return Err(ArgError::InvalidCombination(
                "finite-demand needs run or inspect",
            ));
        }
    };
    let mut result = FiniteArgs {
        action,
        directory: PathBuf::new(),
        command: None,
        policy: None,
        output: None,
        events: None,
        stop_file: None,
        resume: false,
        repair_preparation: false,
        no_progress: false,
        format: "auto".into(),
    };
    let mut seen = BTreeSet::new();
    while let Some(value) = argv.next() {
        let value = value.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match value.as_str() {
            "--directory" => "--directory",
            "--source-command" => "--source-command",
            "--policy" => "--policy",
            "--output" => "--output",
            "--events" => "--events",
            "--stop-file" => "--stop-file",
            "--resume" => "--resume",
            "--repair-preparation" => "--repair-preparation",
            "--no-progress" => "--no-progress",
            "--format" => "--format",
            _ => return Err(ArgError::UnknownOption(value)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--resume" => result.resume = true,
            "--repair-preparation" => result.repair_preparation = true,
            "--no-progress" => result.no_progress = true,
            "--format" => {
                result.format = next_utf8_value(&mut argv, option)?;
                if !["auto", "json", "table"].contains(&result.format.as_str()) {
                    return Err(ArgError::InvalidValue {
                        option,
                        value: result.format,
                        expected: "auto, json or table",
                    });
                }
            }
            _ => {
                let path = PathBuf::from(next_value(&mut argv, option)?);
                if path.as_os_str().is_empty() {
                    return Err(ArgError::EmptyPath);
                }
                match option {
                    "--directory" => result.directory = path,
                    "--source-command" => result.command = Some(path),
                    "--policy" => result.policy = Some(path),
                    "--output" => result.output = Some(path),
                    "--events" => result.events = Some(path),
                    "--stop-file" => result.stop_file = Some(path),
                    _ => unreachable!(),
                }
            }
        }
    }
    if result.directory.as_os_str().is_empty() {
        return Err(ArgError::InvalidCombination(
            "finite-demand requires --directory",
        ));
    }
    match action {
        Action::Run
            if result.command.is_none()
                || result.policy.is_none()
                || result.output.is_none()
                || seen.contains("--format") =>
        {
            return Err(ArgError::InvalidCombination(
                "finite-demand run needs --source-command, --policy and --output; no --format",
            ));
        }
        Action::Inspect
            if seen
                .iter()
                .any(|s| !["--directory", "--format", "--policy"].contains(s)) =>
        {
            return Err(ArgError::InvalidCombination(
                "finite-demand inspect accepts only --directory, --format and --policy",
            ));
        }
        _ => {}
    }
    Ok(Command::FiniteDemand(result))
}

#[cfg(test)]
pub(super) fn read_source(
    path: &Path,
) -> Result<(crate::OwnerDomainWalkRequest, Vec<PathBuf>, Vec<PathBuf>), CliError> {
    read_source_with_limit(path, crate::MAX_INPUT_BYTES)
}
pub(super) fn read_source_with_policy(
    path: &Path,
    policy: &FiniteDemandPolicy,
) -> Result<(crate::OwnerDomainWalkRequest, Vec<PathBuf>, Vec<PathBuf>), CliError> {
    read_source_with_limit(path, policy.max_source_bytes)
}
fn read_source_with_limit(
    path: &Path,
    max_source_bytes: usize,
) -> Result<(crate::OwnerDomainWalkRequest, Vec<PathBuf>, Vec<PathBuf>), CliError> {
    let path = path
        .canonicalize()
        .map_err(|e| CliError::InputIo(e.to_string()))?;
    let bytes = read_bounded(
        File::open(&path).map_err(|e| CliError::InputIo(e.to_string()))?,
        "finite source carrier",
        max_source_bytes,
    )?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|e| CliError::Input(e.to_string()))?;
    let argv: Vec<OsString> = document
        .get("command")
        .unwrap_or(&document)
        .as_array()
        .and_then(|a| a.iter().map(|v| v.as_str().map(OsString::from)).collect())
        .ok_or_else(|| CliError::Input("finite source command must be a list of strings".into()))?;
    let (request, mut protected, inert) = super::owner_match::finite_source_request_from_argv(
        argv,
        path.parent().unwrap(),
        max_source_bytes,
    )
    .map_err(CliError::Input)?;
    protected.push(path);
    Ok((request, protected, inert))
}
pub(super) fn read_policy(path: &Path) -> Result<FiniteDemandPolicy, CliError> {
    let bytes = read_bounded(
        File::open(path).map_err(|e| CliError::InputIo(e.to_string()))?,
        "finite policy",
        usize::MAX,
    )?;
    FiniteDemandPolicy::from_json(&bytes).map_err(Into::into)
}

fn lexical_absolute(path: &Path) -> Result<PathBuf, CliError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| CliError::InputIo(e.to_string()))?
            .join(path)
    };
    let mut out = PathBuf::new();
    for c in absolute.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            _ => out.push(c.as_os_str()),
        }
    }
    Ok(out)
}
fn prospective_path(path: &Path) -> Result<PathBuf, CliError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => path
            .canonicalize()
            .map_err(|e| CliError::InputIo(e.to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let name = path
                .file_name()
                .ok_or_else(|| CliError::Input("invalid finite write path".into()))?;
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            Ok(prospective_path(parent)?.join(name))
        }
        Err(error) => Err(CliError::InputIo(error.to_string())),
    }
}
pub(super) fn protect(
    path: &Path,
    protected: &[PathBuf],
    inert: &[PathBuf],
) -> Result<(), CliError> {
    let resolved = prospective_path(path)?;
    for input in protected {
        let input = prospective_path(input)?;
        if resolved.starts_with(&input) || input.starts_with(&resolved) {
            return Err(CliError::Input(
                "finite output/stop overlaps a source or engine namespace".into(),
            ));
        }
    }
    let lexical = lexical_absolute(path)?;
    for input in inert {
        let input = lexical_absolute(input)?;
        if lexical.starts_with(&input)
            || input.starts_with(&lexical)
            || resolved.starts_with(&input)
            || input.starts_with(&resolved)
        {
            return Err(CliError::Input(
                "finite write path overlaps an inert legacy carrier path".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
struct Signals(Vec<signal_hook::SigId>);
#[cfg(unix)]
impl Drop for Signals {
    fn drop(&mut self) {
        for id in self.0.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

pub(super) fn run(args: FiniteArgs) -> Result<(), CliError> {
    if args.action == Action::Inspect {
        let policy = args.policy.as_deref().map(read_policy).transpose()?;
        let status = crate::inspect_finite_demand_with_policy(&args.directory, policy.as_ref())?;
        if args.format == "json" || args.format == "auto" && !io::stdout().is_terminal() {
            println!(
                "{}",
                serde_json::to_string_pretty(&status)
                    .map_err(|e| CliError::OutputIo(e.to_string()))?
            );
        } else if status["status"] == "preparing" {
            println!(
                "Initial preparation incomplete: no native checkpoint or closed Work is available.\nRepeat run with --repair-preparation and the original source/policy; no Work will be reused."
            );
        } else {
            println!(
                "Finite-demand committed generation {}\nRequired rows: {}/{}\nInput occurrences: {}\nCompleted Work: {}\nSource metadata: {}\nTrusted generated metadata; publication verification is separate.",
                status["generation"]["number"],
                status["committed"]["exhausted_required_rows"],
                status["committed"]["required_rows"],
                status["committed"]["input_occurrences"],
                status["committed"]["completed_work"],
                status["source"]["status"]
            );
        }
        return Ok(());
    }
    super::routed::preflight_inner_pools()?;
    let policy = read_policy(args.policy.as_ref().unwrap())?;
    let (source, mut protected, inert) =
        read_source_with_policy(args.command.as_ref().unwrap(), &policy)?;
    protected.push(args.policy.clone().unwrap());
    protect(&args.directory, &protected, &inert)?;
    protected.push(args.directory.clone());
    let output = args.output.as_ref().unwrap();
    let destinations = [Some(output), args.events.as_ref(), args.stop_file.as_ref()];
    let mut unique = BTreeSet::new();
    for path in destinations.into_iter().flatten() {
        protect(path, &protected, &inert)?;
        if !unique.insert(lexical_absolute(path)?) {
            return Err(CliError::Input(
                "finite result/events/stop paths must differ".into(),
            ));
        }
    }
    preflight_output_destination(&StreamPath::File(output.clone()), false)?;
    if let Some(events) = &args.events {
        preflight_output_destination(&StreamPath::File(events.clone()), false)?;
    }
    let events: Box<dyn Write + Send> = match &args.events {
        Some(path) => Box::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| CliError::OutputIo(e.to_string()))?,
        ),
        None => Box::new(io::stdout()),
    };
    let cancel = Arc::new(AtomicBool::new(
        args.stop_file.as_ref().is_some_and(|p| p.exists()),
    ));
    #[cfg(unix)]
    let mut signals = Signals(Vec::new());
    #[cfg(unix)]
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signals.0.push(
            signal_hook::flag::register(signal, Arc::clone(&cancel))
                .map_err(|e| CliError::InputIo(e.to_string()))?,
        );
    }
    let monitor = RoutedProgress::start_finite(
        events,
        io::stderr().is_terminal()
            && !args.no_progress
            && (args.events.is_some() || !io::stdout().is_terminal()),
        Arc::clone(&cancel),
        args.stop_file.clone(),
        policy.workers,
        !args.no_progress,
    );
    let result = crate::run_finite_demand_with_progress(
        FiniteDemandRunRequest {
            source,
            policy: policy.clone(),
            directory: args.directory.clone(),
            mode: if args.repair_preparation {
                FiniteDemandMode::RepairPreparation
            } else if args.resume {
                FiniteDemandMode::Resume
            } else {
                FiniteDemandMode::Create
            },
        },
        &cancel,
        |event| monitor.observe(event),
    );
    let document = match &result {
        Ok(result) => result.document.clone(),
        Err(error) => {
            let mut inspection_policy = "supplied";
            let status = crate::inspect_finite_demand_with_policy(&args.directory, Some(&policy))
                .or_else(|_| {
                    inspection_policy = "default_diagnostic";
                    crate::inspect_finite_demand(&args.directory)
                });
            let repair = status
                .as_ref()
                .ok()
                .is_some_and(|v| v["repair_preparation_possible"] == true);
            json!({"schema":"rustred.finite-demand-run.v1","engine":"rustred.finite-demand.v1",
                "status":if error.kind()==crate::AppErrorKind::Cancelled {"cancelled"} else if error.kind()==crate::AppErrorKind::Limit {"limited"} else {"failed"},
                "complete":false,"cancelled":cancel.load(std::sync::atomic::Ordering::Acquire),"policy":policy,
                "error_kind":error.kind().as_str(),"error":error.to_string(),
                "committed":status.as_ref().ok().map(|s|s["committed"].clone()),
                "generation":status.as_ref().ok().map(|s|s["generation"].clone()),
                "repair_preparation_possible":repair,
                "native_repair_command":if repair {repair_command(&args)}else{None},
                "recovery_note":if repair {Some("No native checkpoint/closed Work is available. Explicit repair repeats preparation; use the original supervisor/resource environment with the suggested native command.")}else{None},
                "inspection_error":status.err().map(|e|e.to_string()),"inspection_policy":inspection_policy,
                "publication_claim":false})
        }
    };
    monitor.observe(json!({"event":"finished","operation":"finite_demand","result":document}));
    let monitoring = monitor.finish();
    crate::application::atomic_file::write_file_atomically_with(output, false, |file| {
        serde_json::to_writer_pretty(&mut *file, &document).map_err(|e| e.to_string())?;
        file.write_all(b"\n").map_err(|e| e.to_string())
    })
    .map_err(CliError::OutputIo)?;
    monitoring.map_err(|e| CliError::OutputIo(e.to_string()))?;
    result.map(|_| ()).map_err(Into::into)
}

fn repair_command(args: &FiniteArgs) -> Option<Vec<String>> {
    let output = args.output.as_ref()?;
    let mut proposed = None;
    for attempt in 1..=256 {
        let mut name = output.file_name()?.to_os_string();
        name.push(format!(".repair-{attempt}.json"));
        let path = output.with_file_name(name);
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                proposed = Some(path);
                break;
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    let exe = std::env::current_exe().ok()?;
    let directory = args.directory.canonicalize().ok()?;
    let source = args.command.as_ref()?.canonicalize().ok()?;
    let policy = args.policy.as_ref()?.canonicalize().ok()?;
    let output = lexical_absolute(&proposed?).ok()?;
    Some(vec![
        exe.to_str()?.into(),
        "finite-demand".into(),
        "run".into(),
        "--directory".into(),
        directory.to_str()?.into(),
        "--source-command".into(),
        source.to_str()?.into(),
        "--policy".into(),
        policy.to_str()?.into(),
        "--resume".into(),
        "--repair-preparation".into(),
        "--output".into(),
        output.to_str()?.into(),
    ])
}
