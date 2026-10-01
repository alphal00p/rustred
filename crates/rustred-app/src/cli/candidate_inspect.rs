use super::args::{CandidateInspectArgs, StreamPath};
use super::error::CliError;
use super::io::{preflight_output_destination, read_artifact, read_input, write_output};
use crate::{
    CandidateBundleLimits, CandidateProgramInspectionOptions, inspect_generated_candidate_program,
};

pub(super) fn run(arguments: CandidateInspectArgs) -> Result<(), CliError> {
    if arguments.input == StreamPath::Stdio && arguments.options == Some(StreamPath::Stdio) {
        return Err(CliError::Input(
            "candidate payload and inspection options cannot both read stdin".into(),
        ));
    }
    preflight_output_destination(&arguments.output, arguments.force)?;
    let options: CandidateProgramInspectionOptions = arguments
        .options
        .map(|path| {
            serde_json::from_str(&read_input(&path)?).map_err(|e| CliError::Input(e.to_string()))
        })
        .transpose()?
        .unwrap_or_default();
    let bytes = read_artifact(&arguments.input)?;
    let report =
        inspect_generated_candidate_program(&bytes, CandidateBundleLimits::default(), options)?;
    write_output(
        &arguments.output,
        report.to_json()?.as_bytes(),
        arguments.force,
    )
}
