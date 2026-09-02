use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use crate::application::memory::parse_memory_bytes;
use crate::{ClosingFamilySelector, InputFormat};

use super::{
    ArgError, CampaignGenerateArgs, CampaignInspectArgs, CampaignPlanArgs, CampaignPreflightArgs,
    CampaignReduceArgs, ColorPolicy, Command, FoundryCampaignRunArgs, FoundryWaveCampaignRunArgs,
    StreamPath, next_utf8_value, next_value, parse_nonnegative_integer, parse_positive_integer,
    reject_trailing, set_once,
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
        "run" => parse_run(arguments),
        "run-waves" => parse_run_waves(arguments),
        "generate" => parse_generate(arguments),
        "inspect" => parse_inspect(arguments),
        "reduce" => parse_reduce(arguments),
        _ => Err(ArgError::UnknownSubcommand {
            command: "campaign",
            subcommand,
        }),
    }
}

fn parse_run_waves(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut config = None;
    let mut output = None;
    let mut measurements_output = None;
    let mut artifact_output = None;
    let mut n_cores = None;
    let mut color = None;
    let mut no_progress = false;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => set_flag(&mut help, "--help")?,
            "--force" => set_flag(&mut force, "--force")?,
            "--no-progress" => set_flag(&mut no_progress, "--no-progress")?,
            "--color" => {
                let value = next_utf8_value(&mut arguments, "--color")?;
                set_once(&mut color, "--color", parse_color_policy(value)?)?;
            }
            "--config" => set_once(
                &mut config,
                "--config",
                StreamPath::parse(next_value(&mut arguments, "--config")?)?,
            )?,
            "--output" => set_once(
                &mut output,
                "--output",
                StreamPath::parse(next_value(&mut arguments, "--output")?)?,
            )?,
            "--measurements-output" => set_once(
                &mut measurements_output,
                "--measurements-output",
                StreamPath::parse(next_value(&mut arguments, "--measurements-output")?)?,
            )?,
            "--artifact-output" => set_once(
                &mut artifact_output,
                "--artifact-output",
                StreamPath::parse(next_value(&mut arguments, "--artifact-output")?)?,
            )?,
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
    let config = config.ok_or(ArgError::MissingRequiredOption("--config"))?;
    let output = output.ok_or(ArgError::MissingRequiredOption("--output"))?;
    validate_run_paths(&config, &output, measurements_output.as_ref())?;
    if let Some(artifact) = artifact_output.as_ref() {
        if same_stream_or_file(artifact, &output) || same_file(&config, artifact) {
            return Err(ArgError::InvalidCombination(
                "--artifact-output must differ from --config and --output",
            ));
        }
        if measurements_output
            .as_ref()
            .is_some_and(|measurements| same_stream_or_file(measurements, artifact))
        {
            return Err(ArgError::InvalidCombination(
                "--artifact-output and --measurements-output must differ",
            ));
        }
    }
    Ok(Command::FoundryWaveCampaignRun(
        FoundryWaveCampaignRunArgs {
            config,
            output,
            measurements_output,
            artifact_output,
            n_cores: n_cores.unwrap_or(1),
            no_progress,
            color: color.unwrap_or_default(),
            force,
        },
    ))
}

fn parse_run(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut config = None;
    let mut output = None;
    let mut measurements_output = None;
    let mut color = None;
    let mut no_progress = false;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => set_flag(&mut help, "--help")?,
            "--force" => set_flag(&mut force, "--force")?,
            "--no-progress" => set_flag(&mut no_progress, "--no-progress")?,
            "--color" => {
                let value = next_utf8_value(&mut arguments, "--color")?;
                set_once(&mut color, "--color", parse_color_policy(value)?)?;
            }
            "--config" => set_once(
                &mut config,
                "--config",
                StreamPath::parse(next_value(&mut arguments, "--config")?)?,
            )?,
            "--output" => set_once(
                &mut output,
                "--output",
                StreamPath::parse(next_value(&mut arguments, "--output")?)?,
            )?,
            "--measurements-output" => set_once(
                &mut measurements_output,
                "--measurements-output",
                StreamPath::parse(next_value(&mut arguments, "--measurements-output")?)?,
            )?,
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    let config = config.ok_or(ArgError::MissingRequiredOption("--config"))?;
    let output = output.ok_or(ArgError::MissingRequiredOption("--output"))?;
    validate_run_paths(&config, &output, measurements_output.as_ref())?;
    Ok(Command::FoundryCampaignRun(FoundryCampaignRunArgs {
        config,
        output,
        measurements_output,
        no_progress,
        color: color.unwrap_or_default(),
        force,
    }))
}

fn parse_color_policy(value: String) -> Result<ColorPolicy, ArgError> {
    match value.as_str() {
        "auto" => Ok(ColorPolicy::Auto),
        "always" => Ok(ColorPolicy::Always),
        "never" => Ok(ColorPolicy::Never),
        _ => Err(ArgError::InvalidValue {
            option: "--color",
            value,
            expected: "auto, always, or never",
        }),
    }
}

fn validate_run_paths(
    config: &StreamPath,
    output: &StreamPath,
    measurements_output: Option<&StreamPath>,
) -> Result<(), ArgError> {
    if same_file(config, output) {
        return Err(ArgError::InvalidCombination(
            "--config and --output must not name the same stream or path",
        ));
    }
    let Some(measurements_output) = measurements_output else {
        return Ok(());
    };
    if same_stream_or_file(output, measurements_output) {
        return Err(ArgError::InvalidCombination(
            "--output and --measurements-output must not name the same stream or path",
        ));
    }
    if same_file(config, measurements_output) {
        return Err(ArgError::InvalidCombination(
            "--config and --measurements-output must not name the same stream or path",
        ));
    }
    Ok(())
}

fn same_file(left: &StreamPath, right: &StreamPath) -> bool {
    matches!(
        (left, right),
        (StreamPath::File(left), StreamPath::File(right))
            if resolve_path_without_requiring_destination(left)
                == resolve_path_without_requiring_destination(right)
    )
}

fn same_stream_or_file(left: &StreamPath, right: &StreamPath) -> bool {
    matches!((left, right), (StreamPath::Stdio, StreamPath::Stdio)) || same_file(left, right)
}

/// Resolve every existing prefix through the filesystem, then normalize any
/// missing suffix lexically. This mirrors `Path.resolve(strict=False)` for the
/// output paths accepted by the CLI: destinations need not exist, while two
/// spellings through `.`/`..` or symlinked parent directories still compare as
/// the same eventual directory entry.
fn resolve_path_without_requiring_destination(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut existing_prefix = absolute.clone();
    let mut missing_suffix = Vec::new();
    loop {
        if let Ok(resolved_prefix) = existing_prefix.canonicalize() {
            let mut resolved = resolved_prefix;
            for component in missing_suffix.iter().rev() {
                resolved.push(component);
            }
            return normalize_path_lexically(&resolved);
        }
        let Some(component) = existing_prefix.components().next_back() else {
            return normalize_path_lexically(&absolute);
        };
        let is_root = matches!(component, Component::Prefix(_) | Component::RootDir);
        let component = component.as_os_str().to_os_string();
        if is_root || !existing_prefix.pop() {
            return normalize_path_lexically(&absolute);
        }
        missing_suffix.push(component);
    }
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
        }
    }
    normalized
}

fn parse_generate(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut family = None;
    let mut output = None;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => set_flag(&mut help, "--help")?,
            "--force" => set_flag(&mut force, "--force")?,
            "--family" => {
                let value = next_utf8_value(&mut arguments, "--family")?;
                let parsed =
                    value
                        .parse::<ClosingFamilySelector>()
                        .map_err(|_| ArgError::InvalidValue {
                            option: "--family",
                            value,
                            expected: ClosingFamilySelector::EXPECTED_VALUES,
                        })?;
                set_once(&mut family, "--family", parsed)?;
            }
            "--output" => set_once(
                &mut output,
                "--output",
                StreamPath::parse(next_value(&mut arguments, "--output")?)?,
            )?,
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::CampaignGenerate(CampaignGenerateArgs {
        family: family.ok_or(ArgError::MissingRequiredOption("--family"))?,
        output: output.unwrap_or(StreamPath::Stdio),
        force,
    }))
}

fn parse_inspect(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut artifact = None;
    let mut output = None;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => set_flag(&mut help, "--help")?,
            "--force" => set_flag(&mut force, "--force")?,
            "--artifact" => {
                set_once(
                    &mut artifact,
                    "--artifact",
                    StreamPath::parse(next_value(&mut arguments, "--artifact")?)?,
                )?;
            }
            "--output" => set_once(
                &mut output,
                "--output",
                StreamPath::parse(next_value(&mut arguments, "--output")?)?,
            )?,
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::CampaignInspect(CampaignInspectArgs {
        artifact: artifact.ok_or(ArgError::MissingRequiredOption("--artifact"))?,
        output: output.unwrap_or(StreamPath::Stdio),
        force,
    }))
}

fn parse_reduce(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut artifact = None;
    let mut powers = None;
    let mut max_rule_applications = None;
    let mut output = None;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => set_flag(&mut help, "--help")?,
            "--force" => set_flag(&mut force, "--force")?,
            "--artifact" => {
                set_once(
                    &mut artifact,
                    "--artifact",
                    StreamPath::parse(next_value(&mut arguments, "--artifact")?)?,
                )?;
            }
            "--powers" => {
                let value = next_utf8_value(&mut arguments, "--powers")?;
                let parsed = parse_powers(value.clone()).ok_or(ArgError::InvalidValue {
                    option: "--powers",
                    value,
                    expected: "a nonempty comma-separated list of signed 64-bit integers",
                })?;
                set_once(&mut powers, "--powers", parsed)?;
            }
            "--max-rule-applications" => {
                let value = next_utf8_value(&mut arguments, "--max-rule-applications")?;
                let parsed = parse_nonnegative_integer("--max-rule-applications", value)?;
                set_once(
                    &mut max_rule_applications,
                    "--max-rule-applications",
                    parsed,
                )?;
            }
            "--output" => set_once(
                &mut output,
                "--output",
                StreamPath::parse(next_value(&mut arguments, "--output")?)?,
            )?,
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::CampaignReduce(CampaignReduceArgs {
        artifact: artifact.ok_or(ArgError::MissingRequiredOption("--artifact"))?,
        target_powers: powers.ok_or(ArgError::MissingRequiredOption("--powers"))?,
        max_rule_applications: max_rule_applications
            .unwrap_or(crate::MAX_CLOSING_RULE_APPLICATIONS),
        output: output.unwrap_or(StreamPath::Stdio),
        force,
    }))
}

fn parse_powers(value: String) -> Option<Vec<i64>> {
    if value.is_empty() {
        return None;
    }
    value
        .split(',')
        .map(str::trim)
        .map(|component| {
            if component.is_empty() {
                None
            } else {
                component.parse::<i64>().ok()
            }
        })
        .collect()
}

fn set_flag(flag: &mut bool, option: &'static str) -> Result<(), ArgError> {
    if *flag {
        Err(ArgError::DuplicateOption(option))
    } else {
        *flag = true;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> impl Iterator<Item = OsString> {
        values
            .iter()
            .map(|value| OsString::from(*value))
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn run_requires_explicit_config_and_report_destinations() {
        assert_eq!(
            parse(arguments(&["run"])),
            Err(ArgError::MissingRequiredOption("--config"))
        );
        assert_eq!(
            parse(arguments(&["run", "--config", "-"])),
            Err(ArgError::MissingRequiredOption("--output"))
        );
        assert_eq!(
            parse(arguments(&[
                "run", "--config", "-", "--output", "-", "--force"
            ])),
            Ok(Command::FoundryCampaignRun(FoundryCampaignRunArgs {
                config: StreamPath::Stdio,
                output: StreamPath::Stdio,
                measurements_output: None,
                no_progress: false,
                color: ColorPolicy::Auto,
                force: true,
            }))
        );
    }

    #[test]
    fn run_parses_progress_and_color_policy_once() {
        assert_eq!(
            parse(arguments(&[
                "run",
                "--config",
                "config.toml",
                "--output",
                "report.toml",
                "--no-progress",
                "--color",
                "never",
            ])),
            Ok(Command::FoundryCampaignRun(FoundryCampaignRunArgs {
                config: StreamPath::File("config.toml".into()),
                output: StreamPath::File("report.toml".into()),
                measurements_output: None,
                no_progress: true,
                color: ColorPolicy::Never,
                force: false,
            }))
        );
        assert_eq!(
            parse(arguments(&[
                "run",
                "--config",
                "-",
                "--output",
                "report.toml",
                "--color",
                "rainbow",
            ])),
            Err(ArgError::InvalidValue {
                option: "--color",
                value: "rainbow".to_owned(),
                expected: "auto, always, or never",
            })
        );
        assert_eq!(
            parse(arguments(&[
                "run",
                "--config",
                "-",
                "--output",
                "report.toml",
                "--no-progress",
                "--no-progress",
            ])),
            Err(ArgError::DuplicateOption("--no-progress"))
        );
    }

    #[test]
    fn run_waves_parses_the_same_progress_contract() {
        assert_eq!(
            parse(arguments(&[
                "run-waves",
                "--config",
                "config.toml",
                "--output",
                "report.toml",
                "--artifact-output",
                "closed.rribp",
                "--n-cores",
                "3",
                "--no-progress",
                "--color",
                "never",
            ])),
            Ok(Command::FoundryWaveCampaignRun(
                FoundryWaveCampaignRunArgs {
                    config: StreamPath::File("config.toml".into()),
                    output: StreamPath::File("report.toml".into()),
                    measurements_output: None,
                    artifact_output: Some(StreamPath::File("closed.rribp".into())),
                    n_cores: 3,
                    no_progress: true,
                    color: ColorPolicy::Never,
                    force: false,
                }
            ))
        );
        assert_eq!(
            parse(arguments(&[
                "run-waves",
                "--config",
                "-",
                "--output",
                "report.toml",
                "--color",
                "rainbow",
            ])),
            Err(ArgError::InvalidValue {
                option: "--color",
                value: "rainbow".to_owned(),
                expected: "auto, always, or never",
            })
        );
        assert_eq!(
            parse(arguments(&[
                "run-waves",
                "--config",
                "-",
                "--output",
                "report.toml",
                "--no-progress",
                "--no-progress",
            ])),
            Err(ArgError::DuplicateOption("--no-progress"))
        );
    }

    #[test]
    fn run_rejects_ambiguous_measurement_and_file_destinations() {
        assert_eq!(
            parse(arguments(&[
                "run",
                "--config",
                "-",
                "--output",
                "-",
                "--measurements-output",
                "-",
            ])),
            Err(ArgError::InvalidCombination(
                "--output and --measurements-output must not name the same stream or path"
            ))
        );
        assert!(matches!(
            parse(arguments(&[
                "run",
                "--config",
                "campaign.toml",
                "--output",
                "campaign.toml",
            ])),
            Err(ArgError::InvalidCombination(_))
        ));
        assert!(matches!(
            parse(arguments(&[
                "run",
                "--config",
                "campaign.toml",
                "--output",
                "report.toml",
                "--measurements-output",
                "campaign.toml",
            ])),
            Err(ArgError::InvalidCombination(_))
        ));
    }

    #[test]
    fn path_equivalence_does_not_require_destinations_to_exist() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustred-uncreated-campaign-destinations-{}-{nonce}",
            std::process::id(),
        ));
        assert!(!root.exists());
        let direct = StreamPath::File(root.join("report.toml"));
        let lexical_alias = StreamPath::File(root.join("missing").join("..").join("report.toml"));

        assert!(same_file(&direct, &lexical_alias));
    }
}
