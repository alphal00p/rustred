use super::{
    args::EntryDomainPlanArgs,
    error::CliError,
    io::{preflight_output_destination, read_input, write_output},
};

pub(super) fn run(args: EntryDomainPlanArgs) -> Result<(), CliError> {
    preflight_output_destination(&args.output, args.force)?;
    let source = read_input(&args.input)?;
    let plan = crate::entry_domain_plan(&source)?;
    let mut bytes = serde_json::to_vec_pretty(&plan)
        .map_err(|e| CliError::OutputIo(format!("entry-domain plan serialization: {e}")))?;
    bytes.push(b'\n');
    write_output(&args.output, &bytes, args.force)
}
