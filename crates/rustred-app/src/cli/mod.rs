pub(crate) mod args;
pub(crate) mod error;
mod io;

use std::ffi::OsString;
use std::io::Write;

use crate::{
    CampaignPlanRequest, CampaignPreflightRequest, ClosingArtifactGenerateRequest,
    ClosingArtifactInspectRequest, ClosingArtifactReduceRequest, DeriveRequest, campaign_plan,
    campaign_preflight, closing_artifact_generate, closing_artifact_inspect,
    closing_artifact_reduce, derive as derive_application,
};
use args::{
    CampaignGenerateArgs, CampaignInspectArgs, CampaignPlanArgs, CampaignPreflightArgs,
    CampaignReduceArgs, Command, DeriveArgs, HELP, parse_args,
};
use error::CliError;
use io::{read_artifact, read_input, write_output};

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
        Command::CampaignGenerate(arguments) => generate_campaign_artifact(arguments),
        Command::CampaignInspect(arguments) => inspect_campaign_artifact(arguments),
        Command::CampaignReduce(arguments) => reduce_campaign_target(arguments),
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
