use super::{
    args::{RoutedCampaignArgs, StreamPath},
    error::CliError,
    io::{preflight_output_destination, read_input, write_output},
    progress::RoutedProgress,
};
use crate::{RoutedCampaignRequest, routed_campaign_with_progress};
use serde_json::json;
use std::fs::OpenOptions;
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, atomic::AtomicBool};

pub(super) fn run(args: RoutedCampaignArgs) -> Result<(), CliError> {
    // Outer scheduling owns the worker budget; configure native pools before
    // launching the process, never by mutating the environment after threads.
    for name in [
        "RAYON_NUM_THREADS",
        "OMP_NUM_THREADS",
        "OMP_THREAD_LIMIT",
        "OPENBLAS_NUM_THREADS",
        "MKL_NUM_THREADS",
        "BLIS_NUM_THREADS",
        "SYMBOLICA_HIDE_BANNER",
    ] {
        if std::env::var(name).as_deref() != Ok("1") {
            return Err(CliError::Input(format!(
                "set {name}=1 before launch (the Python steering example does this)"
            )));
        }
    }
    let output = StreamPath::File(args.output.clone());
    preflight_output_destination(&output, false)?;
    if let Some(path) = &args.events {
        preflight_output_destination(&StreamPath::File(path.clone()), false)?;
    }
    // No overwrite option: input/event/stop aliases cannot destroy saved owners.
    if args.events.as_ref() == Some(&args.output)
        || args.stop_file.as_ref() == Some(&args.output)
        || args.events.is_some() && args.events == args.stop_file
    {
        return Err(CliError::Input(
            "event, result and stop paths must differ".into(),
        ));
    }
    let mut request = RoutedCampaignRequest::new(
        read_input(&StreamPath::File(args.manifest))?,
        read_input(&StreamPath::File(args.targets))?,
    );
    request.owner_base = args.owner_base;
    request.workers = args.workers;
    request.trace_limits.max_unique_nodes = args.nodes;
    request.trace_limits.max_input_targets = args.input_targets;
    request.trace_limits.max_transport_calls = args.nodes;
    request.trace_limits.max_transport_operations = args.transport_operations;
    request.trace_limits.max_transport_endpoints = args.transport_endpoints;
    request.reduction_limits.max_rule_applications = args.rule_applications;
    request.reduction_limits.max_pending_frames = args.nodes;
    request.reduction_limits.max_coalescing_additions = args.coalescing_additions;
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
    let result =
        routed_campaign_with_progress(request, &cancellation, |event| monitor.observe(event));
    let document = match &result {
        Ok(result) => result.document.clone(),
        Err(error) => {
            json!({"schema":"rustred.routed-campaign.json.v1","event":"finished","status":"preparation_error",
            "completed_finite_trace":false,"family_closure_claim":false,"ibp_generation":false,"work_checkpoint":false,
            "error_kind":error.kind().as_str(),"error":error.to_string()})
        }
    };
    monitor.observe(document.clone());
    let presentation = monitor.finish();
    let bytes =
        serde_json::to_vec_pretty(&document).map_err(|e| CliError::OutputIo(e.to_string()))?;
    write_output(&output, &bytes, false)?;
    presentation.map_err(|e| CliError::OutputIo(format!("event journal: {e}")))?;
    let result = result?;
    if !result.completed_finite_trace {
        return Err(CliError::Input(
            "shared finite trace incomplete; see saved result (not a closure claim)".into(),
        ));
    }
    Ok(())
}
