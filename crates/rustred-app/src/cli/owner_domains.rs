use super::{
    args::{OwnerDomainScanArgs, StreamPath},
    error::CliError,
    io::{preflight_output_destination, read_input, write_output},
    progress::RoutedProgress,
};
use crate::{OwnerDomainScanRequest, OwnerDomainScanResult, owner_domain_scan_with_progress};
use serde_json::json;
use std::fs::OpenOptions;
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, atomic::AtomicBool};

pub(super) fn run(args: OwnerDomainScanArgs) -> Result<(), CliError> {
    super::routed::preflight_inner_pools()?;
    let output = StreamPath::File(args.output.clone());
    preflight_output_destination(&output, false)?;
    if let Some(path) = &args.events {
        preflight_output_destination(&StreamPath::File(path.clone()), false)?;
    }
    if args.events.as_ref() == Some(&args.output)
        || args.stop_file.as_ref() == Some(&args.output)
        || args.events.is_some() && args.events == args.stop_file
    {
        return Err(CliError::Input(
            "event, result and stop paths must differ".into(),
        ));
    }
    let mut request = OwnerDomainScanRequest::new(
        read_input(&StreamPath::File(args.manifest))?,
        Some(args.rank),
    );
    request.owner_base = args.owner_base;
    request.scan_limits.max_rules = args.max_rules;
    request.scan_limits.max_terms = args.max_terms;
    request.scan_limits.max_regions = args.max_regions;
    request.max_total_regions = args.max_total_regions;
    request.max_summary_groups = args.max_summary_groups;
    let events: Box<dyn Write + Send> = match args.events {
        Some(path) => Box::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| CliError::OutputIo(format!("{}: {e}", path.display())))?,
        ),
        None => Box::new(io::stdout()),
    };
    let cancellation = Arc::new(AtomicBool::new(
        args.stop_file.as_ref().is_some_and(|p| p.exists()),
    ));
    let monitor = RoutedProgress::start(
        events,
        io::stderr().is_terminal() && !args.no_progress,
        Arc::clone(&cancellation),
        args.stop_file,
    );
    let result = owner_domain_scan_with_progress(request, &cancellation, |mut event| {
        event["operation"] = json!("owner_domain_scan");
        monitor.observe(event)
    });
    let mut document = match &result {
        Ok(result) => result.document.clone(),
        Err(error) => json!({"schema":"rustred.owner-domain-scan.json.v1", "event":"finished",
            "status":"preparation_error", "scan_complete":false, "family_closure_claim":false,
            "ibp_generation":false, "error_kind":error.kind().as_str(), "error":error.to_string()}),
    };
    document["operation"] = json!("owner_domain_scan");
    monitor.observe(OwnerDomainScanResult::completion_progress(&document));
    let presentation = monitor.finish();
    let bytes =
        serde_json::to_vec_pretty(&document).map_err(|e| CliError::OutputIo(e.to_string()))?;
    write_output(&output, &bytes, false)?;
    presentation.map_err(|e| CliError::OutputIo(format!("event journal: {e}")))?;
    if !result?.scan_complete {
        return Err(CliError::Input(
            "owner-domain scan incomplete; see saved result".into(),
        ));
    }
    Ok(())
}
