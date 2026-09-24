use super::{
    args::{OwnerDomainMatchArgs, StreamPath},
    error::CliError,
    io::{preflight_output_destination, read_bounded, read_input},
    progress::RoutedProgress,
};
use crate::{OwnerDomainMatchRequest, OwnerDomainMatchResult, owner_domain_match_with_progress};
use crate::{
    OwnerDomainWalkRequest, OwnerDomainWalkResult, OwnerDomainWalkSchedulingPolicy,
    owner_domain_walk_with_progress,
};
use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, atomic::AtomicBool};
#[cfg(test)]
mod checkpoint_path_tests;
#[cfg(test)]
mod query_admission_tests;

pub(super) fn run(args: OwnerDomainMatchArgs) -> Result<(), CliError> {
    OwnerDomainMatchRequest::validate_query_allowances(args.max_queries, args.max_query_bytes)
        .map_err(|message| CliError::Input(message.into()))?;
    super::routed::preflight_inner_pools()?;
    run_admitted(args)
}

fn run_admitted(args: OwnerDomainMatchArgs) -> Result<(), CliError> {
    preflight_checkpoint_paths(&args)?;
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
    let query_file = File::open(&args.queries)
        .map_err(|e| CliError::InputIo(format!("{}: {e}", args.queries.display())))?;
    let query_bytes = read_bounded(query_file, "owner-domain queries", args.max_query_bytes)?;
    let queries = String::from_utf8(query_bytes)
        .map_err(|_| CliError::Input("owner-domain queries must be UTF-8".into()))?;
    let mut request =
        OwnerDomainMatchRequest::new(read_input(&StreamPath::File(args.manifest))?, queries);
    request.owner_base = args.owner_base;
    request.max_queries = args.max_queries;
    request.max_query_bytes = args.max_query_bytes;
    request.max_total_pieces = args.max_total_pieces;
    request.match_limits.max_rules = args.max_rules;
    request.match_limits.max_terminal_checks = args.max_terminal_checks;
    request.match_limits.max_predicates = args.max_predicates;
    request.match_limits.max_pieces = args.max_pieces;
    request.match_limits.max_cells = args.max_cells;
    request.match_limits.max_split_operations = args.max_split_operations;
    request.match_limits.max_coordinate_cells = args.max_coordinate_cells;
    request.match_limits.max_bounded_refinement_cells = args.max_bounded_refinement_cells;
    request.match_limits.refinement_axes = args.refinement_axes;
    request.match_limits.guard_algebra.max_univariate_degree = args.max_guard_univariate_degree;
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
    let walking = args.follow_successors;
    let operation = if walking {
        "owner_domain_walk"
    } else {
        "owner_domain_match"
    };
    let result = if walking {
        let mut walk = OwnerDomainWalkRequest::new(request);
        walk.workers = args.workers;
        walk.inspection_workers = args.inspection_workers;
        walk.publication_policy = args.publication_policy;
        walk.max_domains = args.max_domains;
        walk.max_frontiers = args.max_frontiers;
        walk.max_events = args.max_successor_events;
        walk.max_containment_checks = args.max_containment_checks;
        walk.reuse_initial_d_bands = args.reuse_initial_d_bands;
        if let Some(lookahead) = args.transfer_unreserved_lookahead {
            walk.scheduling_policy =
                OwnerDomainWalkSchedulingPolicy::TransferUnreserved { lookahead };
        }
        walk.route_domain_overcover = args.route_domain_overcover;
        walk.max_route_masks = args.max_route_masks;
        walk.applied_limits.max_boundary_cells = args.max_rhs_cells;
        walk.applied_limits.max_term_visits = args.max_term_visits;
        walk.applied_limits.max_native_operations = args.max_native_operations;
        walk.applied_limits.max_events = args.max_rhs_events;
        walk.applied_limits.max_shift_groups = args.max_shift_groups;
        walk.applied_limits.max_sign_splits = args.max_sign_splits;
        walk.checkpoint = args.checkpoint.clone();
        walk.apply_subdivision = args.apply_subdivision;
        if args.unbounded_work {
            walk.disable_work_limits();
        }
        owner_domain_walk_with_progress(walk, &cancellation, |mut event| {
            event["operation"] = json!(operation);
            monitor.observe(event);
        })
        .map(|result| (result.document, result.all_scheduled_domains_resolved))
    } else {
        owner_domain_match_with_progress(request, &cancellation, |mut event| {
            event["operation"] = json!(operation);
            monitor.observe(event)
        })
        .map(|result| (result.document, result.classification_complete))
    };
    let (mut document, outcome) = match result {
        Ok((document, completed)) => (document, classification_outcome(completed)),
        Err(error) => {
            let document = json!({"schema":"rustred.owner-domain-match.json.v2", "event":"finished",
                "status":"preparation_error", "classification_complete":false,
                "all_queries_locally_applicable":false, "family_closure_claim":false,
                "ibp_generation":false, "rhs_successors_expanded":false,
                "error_kind":error.kind().as_str(), "error":error.to_string()});
            (document, Err(CliError::from(error)))
        }
    };
    document["operation"] = json!(operation);
    document["requested_max_queries"] = json!(args.max_queries);
    document["requested_max_query_bytes"] = json!(args.max_query_bytes);
    document["requested_unbounded_work"] = json!(args.unbounded_work);
    // Also retain the requested local policy on preparation-error receipts.
    document["bounded_refinement_axes"] = json!(match args.refinement_axes {
        rustred::solver::OwnerDomainRefinementAxes::InactiveOnly => "inactive-only",
        rustred::solver::OwnerDomainRefinementAxes::FiniteAxes => "finite-axes",
    });
    document["max_bounded_refinement_cells"] = json!(if walking && args.unbounded_work {
        usize::MAX
    } else {
        args.max_bounded_refinement_cells
    });
    if walking {
        // A requested partition is not evidence that any pool was started.
        document["requested_inspection_workers"] = json!(args.inspection_workers);
        let owner_batched_report = document["schema"] == "rustred.owner-domain-walk.json.v4";
        if args.reuse_initial_d_bands {
            // Preserve the requested opt-in even on preparation-error receipts.
            document["reuse_initial_d_bands"] = json!(true);
        }
        if let Some(lookahead) = args.transfer_unreserved_lookahead {
            document["schema"] = json!("rustred.owner-domain-walk.json.v3");
            document["scheduling_policy"] =
                json!({"kind":"transfer_unreserved", "lookahead":lookahead.get()});
        } else {
            document["schema"] = json!("rustred.owner-domain-walk.json.v2");
        }
        if args.publication_policy == crate::OwnerDomainWalkPublicationPolicy::OwnerBatched {
            // Preserve actual composite identities. A failure before the new
            // traversal starts retains its original schema/initial handles.
            if owner_batched_report {
                document["schema"] = json!("rustred.owner-domain-walk.json.v4");
            }
            document["requested_publication_policy"] = json!("owner_batched");
        }
        monitor.observe(OwnerDomainWalkResult::completion_progress(&document));
    } else {
        monitor.observe(OwnerDomainMatchResult::completion_progress(&document));
    }
    let presentation = monitor.finish();
    crate::application::atomic_file::write_file_atomically_with(&args.output, false, |file| {
        let mut writer = io::BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &document).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())
    })
    .map_err(CliError::OutputIo)?;
    presentation.map_err(|e| CliError::OutputIo(format!("event journal: {e}")))?;
    outcome
}

/// Keep managed checkpoint generations separate from immutable inputs and
/// human-facing reports, including aliases through existing symlinks.
fn preflight_checkpoint_paths(args: &OwnerDomainMatchArgs) -> Result<(), CliError> {
    let Some(options) = &args.checkpoint else {
        return Ok(());
    };
    let directory = super::candidates::resolved_location(&options.directory)?;
    for path in [
        &args.manifest,
        &args.queries,
        &args.output,
        &args.owner_base,
    ]
    .into_iter()
    .chain(args.events.iter())
    .chain(args.stop_file.iter())
    {
        if super::candidates::resolved_location(path)?.starts_with(&directory) {
            return Err(CliError::Input(
                "inputs, owner base, result, events and stop file must be outside the dedicated checkpoint directory".into(),
            ));
        }
    }
    Ok(())
}

/// Exact gaps and invalid source conditions are successful classifications, not
/// successful reductions. Unknown/resource/cancelled classifications are not.
fn classification_outcome(complete: bool) -> Result<(), CliError> {
    if complete {
        Ok(())
    } else {
        Err(CliError::Input(
            "owner-domain classification incomplete; see saved result".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_domain_match_exit_status_means_classification_not_applicability() {
        assert!(classification_outcome(true).is_ok());
        assert_eq!(classification_outcome(false).unwrap_err().exit_code(), 4);
    }
}
