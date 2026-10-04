//! Bounded, read-only observation at the native applied-successor seam.
//! No snapshot resolution, successor traversal, rule generation, or coefficient parsing.
mod applied_observer {
    pub mod input;
    pub mod record;
    #[cfg(test)]
    mod tests;
}
use applied_observer::{input::*, record::*};
use rustred::solver::OwnerFeedbackPolicy;
use rustred_app::{
    FiniteCasePolicy, RoutedCampaignRequest, RoutedFeedbackOptions, RoutedFeedbackSession,
};
use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

fn observe<const N: usize>(
    request: &Request,
    selection: &str,
    targets: &str,
    queries: &Queries,
    output: &Path,
    cancel: &AtomicBool,
) -> Result<bool, String> {
    let started = Instant::now();
    let limits = request.native_limits.build()?;
    let mut recorder = Recorder::new(output, request.recorder_limits.clone())?;
    // These policies concern only hypothetical NEW feedback searches. No round is called.
    let options = RoutedFeedbackOptions::new(
        OwnerFeedbackPolicy::default(),
        FiniteCasePolicy::SearchFinite,
    );
    let mut native_request = RoutedCampaignRequest::new(selection.to_owned(), targets.to_owned());
    native_request.owner_base = request.owner_base.clone();
    native_request.workers = request.workers;
    let preparation_started = Instant::now();
    let prepared = RoutedFeedbackSession::<N>::prepare(native_request, options, cancel, |_| {});
    let preparation_seconds = preparation_started.elapsed().as_secs_f64();
    let session = match prepared {
        Ok(Some(session)) => session,
        Ok(None) => return recorder.finish(json!({"preparation_seconds":preparation_seconds,
            "preparation_cancelled":true,"queries_requested":queries.queries.len(),"queries_observed":0,
            "visitor_seconds":0.0,"total_seconds_before_encoding":started.elapsed().as_secs_f64()}), false),
        Err(error) => return recorder.finish(json!({"preparation_seconds":preparation_seconds,
            "preparation_error":bounded_debug(&error, request.recorder_limits.max_error_bytes),
            "queries_requested":queries.queries.len(),"queries_observed":0,
            "visitor_seconds":0.0,"total_seconds_before_encoding":started.elapsed().as_secs_f64()}), false),
    };
    let mut complete = true;
    let mut visitor_seconds = 0.0;
    let mut observed = 0;
    for (ordinal, query) in queries.queries.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            complete = false;
            break;
        }
        recorder.begin_query(ordinal, query)?;
        let owner: [bool; N] = query
            .owner
            .bytes()
            .map(|b| b == b'1')
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "query owner arity")?;
        let phase = Instant::now();
        let result = session
            .programs()
            .visit_power_bounded_owner_applied_successors(
                owner,
                &query.lower,
                &query.upper,
                query.max_numerator_rank,
                query.power_bounds.native(),
                limits,
                cancel,
                |event| recorder.event(event),
            );
        let seconds = phase.elapsed().as_secs_f64();
        visitor_seconds += seconds;
        observed += 1;
        let outcome = recorder.end_query(result, seconds)?;
        complete &= outcome;
        if recorder.stopped() {
            break;
        }
    }
    complete &= observed == queries.queries.len() && !cancel.load(Ordering::Relaxed);
    recorder.finish(json!({"preparation_seconds":preparation_seconds,
        "visitor_seconds":visitor_seconds,"visitor_seconds_scope":"sum of serial native calls including visitor recording",
        "queries_requested":queries.queries.len(),"queries_observed":observed,
        "total_seconds_before_encoding":started.elapsed().as_secs_f64()}), complete)
}

fn run() -> Result<bool, String> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: applied-observer REQUEST.json FRESH_OUTPUT_DIRECTORY".into());
    }
    let request_text = read_bounded(Path::new(&args[1]), 1 << 20)?;
    let request: Request = serde_json::from_str(&request_text).map_err(|e| e.to_string())?;
    request.validate()?;
    let selection = read_bounded(&request.selection, request.recorder_limits.max_input_bytes)?;
    let query_text = read_bounded(&request.queries, request.recorder_limits.max_input_bytes)?;
    let targets = read_bounded(
        &request.preparation_targets_csv,
        request.recorder_limits.max_input_bytes,
    )?;
    let queries: Queries = serde_json::from_str(&query_text).map_err(|e| e.to_string())?;
    let n = queries.validate(request.recorder_limits.max_queries)?;
    let output = Path::new(&args[2]);
    fs::create_dir(output).map_err(|e| e.to_string())?;
    // Bind literal inputs, not a reserialized approximation. Normal native loading still
    // authenticates all saved artifacts. The outer frozen plan additionally pins file bytes.
    write_new_json(
        &output.join("binding.json"),
        &json!({"schema":"rustred.owner-applied-observer.binding.v1",
        "request":request,"request_blake3":blake3::hash(request_text.as_bytes()).to_hex().as_str(),
        "selection_blake3":blake3::hash(selection.as_bytes()).to_hex().as_str(),
        "queries_blake3":blake3::hash(query_text.as_bytes()).to_hex().as_str(),
        "preparation_targets_blake3":blake3::hash(targets.as_bytes()).to_hex().as_str(),
        "trace_and_reduction_limits":"cached native defaults; trace/feedback methods never called",
        "coordinate_convention":"active n=1+coordinate; inactive n=-coordinate",
        "coefficient_coordinates":"original source indices after native fixed-coordinate restriction",
        "guard_binding":"immutable selection/payloads plus classified owner,batch,rule; no standalone proof",
        "target_semantics":"box intersect translated rank and A/D predicates; membership is not applicability",
        "no_snapshot_resolution":true,"no_traversal":true}),
        request.recorder_limits.max_record_bytes,
    )?;
    let cancel = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, cancel.clone()).map_err(|e| e.to_string())?;
    }
    macro_rules! dispatch { ($($n:literal),*) => {match n {
        $($n => observe::<$n>(&request, &selection, &targets, &queries, output, &cancel),)*
        _ => Err("unsupported arity".into()),
    }}; }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn main() {
    let started = Instant::now();
    match run() {
        Ok(complete) => {
            println!(
                "{}",
                json!({"complete_outcome":complete,"seconds":started.elapsed().as_secs_f64()})
            );
            if !complete {
                std::process::exit(4);
            }
        }
        Err(error) => {
            eprintln!(
                "{}",
                json!({"complete_outcome":false,"adapter_error":error,"seconds":started.elapsed().as_secs_f64()})
            );
            std::process::exit(2);
        }
    }
}
