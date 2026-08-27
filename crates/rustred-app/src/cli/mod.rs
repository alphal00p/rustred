pub(crate) mod args;
mod backend;
pub(crate) mod campaign;
pub(crate) mod campaign_preflight;
pub(crate) mod error;
mod input;
mod io;
mod model;
mod output;

use std::ffi::OsString;
use std::io::Write;

use crate::{
    CampaignPlanRequest, CampaignPreflightRequest, DeriveRequest, campaign_plan,
    campaign_preflight, derive as derive_application,
};
use args::{CampaignPlanArgs, CampaignPreflightArgs, Command, DeriveArgs, HELP, parse_args};
use error::AppError;
use io::{read_input, write_output};

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
            if matches!(error, AppError::Usage(_)) {
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "rustred: usage: run `rustred --help` for the command contract"
                );
            }
            code
        }
    }
}

pub(crate) fn derive_request(request: DeriveRequest) -> Result<crate::DeriveResult, AppError> {
    let prepared = input::prepare_input(&request.source, request.input_format)?;
    let lowered = backend::lower_project(prepared)?;
    let output = output::build_output(
        lowered,
        request.input_format,
        request.relations,
        request.n_cores,
    )?;
    let serialized = output::serialize_output(&output)?;
    Ok(crate::DeriveResult::new(
        output::OUTPUT_SCHEMA,
        "ok",
        serialized,
    ))
}

fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), AppError> {
    match parse_args(arguments)? {
        Command::Help => write_informational_output(HELP),
        Command::Version => {
            write_informational_output(concat!("RustRed ", env!("CARGO_PKG_VERSION"), "\n"))
        }
        Command::Derive(arguments) => derive_cli(arguments),
        Command::CampaignPlan(arguments) => plan_campaign(arguments),
        Command::CampaignPreflight(arguments) => preflight_campaign(arguments),
    }
}

fn derive_cli(arguments: DeriveArgs) -> Result<(), AppError> {
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

fn plan_campaign(arguments: CampaignPlanArgs) -> Result<(), AppError> {
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

fn preflight_campaign(arguments: CampaignPreflightArgs) -> Result<(), AppError> {
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

fn write_informational_output(contents: &str) -> Result<(), AppError> {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(contents.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| AppError::OutputIo(format!("cannot write standard output: {error}")))
}
