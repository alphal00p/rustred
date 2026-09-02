pub(crate) mod args;
pub(crate) mod error;
mod io;
mod progress;

use std::ffi::OsString;
use std::io::{IsTerminal, Write};

use crate::{
    CampaignPlanRequest, CampaignPreflightRequest, ClosingArtifactGenerateRequest,
    ClosingArtifactInspectRequest, ClosingArtifactReduceRequest, DeriveRequest,
    FoundryCampaignRunRequest, FoundryWaveCampaignRunRequest, campaign_plan, campaign_preflight,
    closing_artifact_generate, closing_artifact_inspect, closing_artifact_reduce,
    derive as derive_application, foundry_campaign_run_with_progress,
    foundry_wave_campaign_run_with_progress,
};
use args::{
    CampaignGenerateArgs, CampaignInspectArgs, CampaignPlanArgs, CampaignPreflightArgs,
    CampaignReduceArgs, Command, DeriveArgs, FoundryCampaignRunArgs, FoundryWaveCampaignRunArgs,
    HELP, StreamPath, parse_args,
};
use error::CliError;
use io::{preflight_output_destination, read_artifact, read_input, write_output};
use progress::{CampaignProgressMonitor, ProgressPresentation};

pub(crate) fn main_entry() -> i32 {
    match run(std::env::args_os()) {
        Ok(()) => 0,
        Err(error) => {
            let code = error.exit_code();
            let _ = writeln!(
                std::io::stderr().lock(),
                "rustred: {}: {error}",
                error.category()
            );
            if matches!(error, CliError::Usage(_)) {
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "rustred: usage: run `rustred --help` for the command contract"
                );
            }
            code
        }
    }
}

fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), CliError> {
    match parse_args(arguments)? {
        Command::Help => write_informational_output(HELP),
        Command::Version => {
            write_informational_output(concat!("RustRed ", env!("CARGO_PKG_VERSION"), "\n"))
        }
        Command::Derive(arguments) => derive_cli(arguments),
        Command::CampaignPlan(arguments) => plan_campaign(arguments),
        Command::CampaignPreflight(arguments) => preflight_campaign(arguments),
        Command::FoundryCampaignRun(arguments) => run_foundry_campaign_cli(arguments),
        Command::FoundryWaveCampaignRun(arguments) => run_foundry_wave_campaign_cli(arguments),
        Command::CampaignGenerate(arguments) => generate_campaign_artifact(arguments),
        Command::CampaignInspect(arguments) => inspect_campaign_artifact(arguments),
        Command::CampaignReduce(arguments) => reduce_campaign_target(arguments),
    }
}

fn run_foundry_wave_campaign_cli(arguments: FoundryWaveCampaignRunArgs) -> Result<(), CliError> {
    let config = read_input(&arguments.config)?;
    preflight_output_destination(&arguments.output, arguments.force)?;
    if let Some(destination) = &arguments.measurements_output {
        preflight_output_destination(destination, arguments.force)?;
    }
    if let Some(destination) = &arguments.artifact_output {
        preflight_output_destination(destination, arguments.force)?;
    }
    let presentation = ProgressPresentation::resolve(
        arguments.no_progress,
        arguments.color,
        std::io::stderr().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
    );
    let mut monitor = CampaignProgressMonitor::new(std::io::stderr(), presentation);
    monitor.start();
    let result = match foundry_wave_campaign_run_with_progress(
        FoundryWaveCampaignRunRequest {
            config,
            sibling_worker_count: arguments.n_cores,
        },
        |progress| monitor.observe_wave(progress),
    ) {
        Ok(result) => result,
        Err(error) => {
            monitor.finish_failed();
            return Err(error.into());
        }
    };
    monitor.finish_wave();
    let report = result.to_toml().as_bytes();
    let measurements = result.measurements_to_toml().as_bytes();
    let artifact = result.artifact_bytes().map(<[u8]>::to_vec);
    // Install the authenticated durable payload before exposing any report
    // which advertises its publication. File destinations are atomic, and an
    // artifact sent to stdout is complete before a later companion-file error
    // can be reported; neither ordering can leave a success report referring
    // to an artifact whose destination was never installed.
    if let (Some(destination), Some(artifact)) = (&arguments.artifact_output, artifact.as_deref()) {
        write_output(destination, artifact, arguments.force)?;
    }
    match &arguments.measurements_output {
        None => write_output(&arguments.output, report, arguments.force)?,
        Some(measurements_output) => match (&arguments.output, measurements_output) {
            (StreamPath::Stdio, StreamPath::File(_)) => {
                write_output(measurements_output, measurements, arguments.force)?;
                write_output(&arguments.output, report, arguments.force)?;
            }
            _ => {
                write_output(&arguments.output, report, arguments.force)?;
                write_output(measurements_output, measurements, arguments.force)?;
            }
        },
    }
    Ok(())
}

fn run_foundry_campaign_cli(arguments: FoundryCampaignRunArgs) -> Result<(), CliError> {
    let config = read_input(&arguments.config)?;
    preflight_output_destination(&arguments.output, arguments.force)?;
    if let Some(destination) = &arguments.measurements_output {
        preflight_output_destination(destination, arguments.force)?;
    }
    let presentation = ProgressPresentation::resolve(
        arguments.no_progress,
        arguments.color,
        std::io::stderr().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
    );
    let mut monitor = CampaignProgressMonitor::new(std::io::stderr(), presentation);
    monitor.start();
    let result =
        match foundry_campaign_run_with_progress(FoundryCampaignRunRequest { config }, |progress| {
            monitor.observe(progress)
        }) {
            Ok(result) => result,
            Err(error) => {
                // Keep the application failure authoritative. The monitor makes
                // a best effort to terminate its in-place row before main_entry
                // prints the stable error message.
                monitor.finish_failed();
                return Err(error.into());
            }
        };
    monitor.finish(
        result.stop(),
        result.snapshot(),
        result.census(),
        result.maximum_dimension(),
        result.task_report_ceiling(),
    );
    let report = result.to_toml().as_bytes();
    let measurements = result.measurements_to_toml().as_bytes();
    match &arguments.measurements_output {
        None => write_output(&arguments.output, report, arguments.force),
        Some(measurements_output) => {
            // At most one destination is stdout (enforced by the parser). Put
            // durable file output first so stdout never advertises success
            // before its companion file has been installed.
            match (&arguments.output, measurements_output) {
                (StreamPath::Stdio, StreamPath::File(_)) => {
                    write_output(measurements_output, measurements, arguments.force)?;
                    write_output(&arguments.output, report, arguments.force)
                }
                _ => {
                    write_output(&arguments.output, report, arguments.force)?;
                    write_output(measurements_output, measurements, arguments.force)
                }
            }
        }
    }
}

fn generate_campaign_artifact(arguments: CampaignGenerateArgs) -> Result<(), CliError> {
    let result = closing_artifact_generate(ClosingArtifactGenerateRequest {
        family: arguments.family,
    })?;
    write_output(&arguments.output, result.artifact(), arguments.force)
}

fn inspect_campaign_artifact(arguments: CampaignInspectArgs) -> Result<(), CliError> {
    let artifact = read_artifact(&arguments.artifact)?;
    let result = closing_artifact_inspect(ClosingArtifactInspectRequest { artifact })?;
    write_output(
        &arguments.output,
        result.to_toml().as_bytes(),
        arguments.force,
    )
}

fn reduce_campaign_target(arguments: CampaignReduceArgs) -> Result<(), CliError> {
    let artifact = read_artifact(&arguments.artifact)?;
    let result = closing_artifact_reduce(ClosingArtifactReduceRequest {
        artifact,
        target_powers: arguments.target_powers,
        max_rule_applications: arguments.max_rule_applications,
    })?;
    write_output(
        &arguments.output,
        result.to_toml().as_bytes(),
        arguments.force,
    )
}

fn derive_cli(arguments: DeriveArgs) -> Result<(), CliError> {
    // Every fallible stage completes before `write_output` sees one byte. In
    // particular, stdout is never left with a truncated TOML document.
    let source = read_input(&arguments.input)?;
    let result = derive_application(DeriveRequest {
        source,
        input_format: arguments.input_format,
        relations: arguments.relations,
        n_cores: arguments.n_cores,
    })?;
    write_output(
        &arguments.output,
        result.to_toml().as_bytes(),
        arguments.force,
    )
}

fn plan_campaign(arguments: CampaignPlanArgs) -> Result<(), CliError> {
    let source = read_input(&arguments.input)?;
    let result = campaign_plan(CampaignPlanRequest {
        source,
        input_format: arguments.input_format,
        root_id: arguments.root_id,
    })?;
    write_output(
        &arguments.output,
        result.to_toml().as_bytes(),
        arguments.force,
    )
}

fn preflight_campaign(arguments: CampaignPreflightArgs) -> Result<(), CliError> {
    let profile = read_input(&arguments.profile)?;
    let result = campaign_preflight(CampaignPreflightRequest {
        profile,
        n_cores: arguments.n_cores,
        max_memory_bytes: arguments.max_memory_bytes,
    })?;
    write_output(
        &arguments.output,
        result.to_toml().as_bytes(),
        arguments.force,
    )
}

fn write_informational_output(contents: &str) -> Result<(), CliError> {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(contents.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| CliError::OutputIo(format!("cannot write standard output: {error}")))
}
