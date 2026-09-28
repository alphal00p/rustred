use super::{
    args::{OwnerDomainMatchArgs, StreamPath},
    error::CliError,
    io::{preflight_output_destination, read_bounded, read_input},
    progress::RoutedProgress,
};
use crate::{OwnerDomainMatchRequest, OwnerDomainMatchResult, owner_domain_match_with_progress};
use crate::{
    OwnerDomainWalkRecords, OwnerDomainWalkRequest, OwnerDomainWalkResult,
    OwnerDomainWalkSchedulingPolicy, owner_domain_walk_with_progress,
};
use serde_json::{Value, json};
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
    let request = match_request(&args)?;
    let events: Box<dyn Write + Send> = match args.events.clone() {
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
        args.stop_file.clone(),
    );
    let walking = args.follow_successors;
    let operation = if walking {
        "owner_domain_walk"
    } else {
        "owner_domain_match"
    };
    let result = if walking {
        let walk = walk_request(request, &args);
        owner_domain_walk_with_progress(walk, &cancellation, |mut event| {
            event["operation"] = json!(operation);
            monitor.observe(event);
        })
        .map(|result| {
            (
                result.document,
                result.records,
                result.all_scheduled_domains_resolved,
            )
        })
    } else {
        owner_domain_match_with_progress(request, &cancellation, |mut event| {
            event["operation"] = json!(operation);
            monitor.observe(event)
        })
        .map(|result| {
            (
                result.document,
                OwnerDomainWalkRecords::default(),
                result.classification_complete,
            )
        })
    };
    // A checkpointed walk's records stay in its sidecar until they are
    // streamed into the report below, one record at a time.
    let (mut document, records, outcome) = match result {
        Ok((document, records, completed)) => {
            (document, records, classification_outcome(completed))
        }
        Err(error) => {
            let document = json!({"schema":"rustred.owner-domain-match.json.v2", "event":"finished",
                "status":"preparation_error", "classification_complete":false,
                "all_queries_locally_applicable":false, "family_closure_claim":false,
                "ibp_generation":false, "rhs_successors_expanded":false,
                "error_kind":error.kind().as_str(), "error":error.to_string()});
            (
                document,
                OwnerDomainWalkRecords::default(),
                Err(CliError::from(error)),
            )
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
        document["requested_apply_cell_refinement_max_cardinality"] = json!(
            args.apply_cell_refinement_max_cardinality
                .map(std::num::NonZeroUsize::get)
        );
        // A requested partition is not evidence that any pool was started.
        document["requested_inspection_workers"] = json!(args.inspection_workers);
        if args.reuse_initial_d_bands {
            // Preserve the requested opt-in even on preparation-error receipts.
            document["reuse_initial_d_bands"] = json!(true);
        }
        annotate_walk_policy(
            &mut document,
            args.publication_policy,
            args.transfer_unreserved_lookahead,
        );
        monitor.observe(OwnerDomainWalkResult::completion_progress(&document));
    } else {
        monitor.observe(OwnerDomainMatchResult::completion_progress(&document));
    }
    let presentation = monitor.finish();
    crate::application::atomic_file::write_file_atomically_with(&args.output, false, |file| {
        let mut writer = io::BufWriter::new(file);
        records.write_json(&document, &mut writer)?;
        writer.flush().map_err(|e| e.to_string())
    })
    .map_err(CliError::OutputIo)?;
    presentation.map_err(|e| CliError::OutputIo(format!("event journal: {e}")))?;
    outcome
}

/// The matching request the options describe; reads the queries and the
/// selection manifest, nothing else.
fn match_request(args: &OwnerDomainMatchArgs) -> Result<OwnerDomainMatchRequest, CliError> {
    let query_file = File::open(&args.queries)
        .map_err(|e| CliError::InputIo(format!("{}: {e}", args.queries.display())))?;
    let query_bytes = read_bounded(query_file, "owner-domain queries", args.max_query_bytes)?;
    let queries = String::from_utf8(query_bytes)
        .map_err(|_| CliError::Input("owner-domain queries must be UTF-8".into()))?;
    let mut request = OwnerDomainMatchRequest::new(
        read_input(&StreamPath::File(args.manifest.clone()))?,
        queries,
    );
    request.owner_base = args.owner_base.clone();
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
    Ok(request)
}

/// The walk `--follow-successors` runs over `request`.
fn walk_request(
    request: OwnerDomainMatchRequest,
    args: &OwnerDomainMatchArgs,
) -> OwnerDomainWalkRequest {
    let mut walk = OwnerDomainWalkRequest::new(request);
    walk.workers = args.workers;
    walk.inspection_workers = args.inspection_workers;
    walk.publication_policy = args.publication_policy;
    walk.max_domains = args.max_domains;
    walk.max_frontiers = args.max_frontiers;
    walk.frontier_policy = args.frontier_policy;
    walk.max_events = args.max_successor_events;
    walk.max_containment_checks = args.max_containment_checks;
    walk.reuse_initial_d_bands = args.reuse_initial_d_bands;
    walk.g2_residual_anchors = args.g2_residual_anchors;
    walk.g2_activate_on_resume = args.g2_activate_on_resume;
    if let Some(lookahead) = args.transfer_unreserved_lookahead {
        walk.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved { lookahead };
    }
    walk.route_domain_overcover = args.route_domain_overcover;
    walk.route_joint_source_support_pruning = args.route_joint_source_support_pruning;
    walk.max_route_masks = args.max_route_masks;
    walk.applied_limits.max_boundary_cells = args.max_rhs_cells;
    walk.applied_limits.max_term_visits = args.max_term_visits;
    walk.applied_limits.max_native_operations = args.max_native_operations;
    walk.applied_limits.max_events = args.max_rhs_events;
    walk.applied_limits.max_shift_groups = args.max_shift_groups;
    walk.applied_limits.max_sign_splits = args.max_sign_splits;
    walk.checkpoint = args.checkpoint.clone();
    walk.apply_subdivision = args.apply_subdivision;
    walk.applied_limits.cell_refinement = args.apply_cell_refinement_max_cardinality.map_or(
        rustred::solver::OwnerAppliedCellRefinement::Off,
        |max_cardinality| rustred::solver::OwnerAppliedCellRefinement::SingleFiniteAxis {
            max_cardinality,
        },
    );
    if args.unbounded_work {
        walk.disable_work_limits();
    }
    walk
}

/// The walk request an `owner-domain-match --follow-successors` argv (with
/// its program name) describes, built by the same parser, admission checks
/// and request construction as the command, without opening any output,
/// event or stop file. Used by the restore-at-scale test and the offline
/// closure verifier.
pub(crate) fn walk_request_from_argv(
    argv: Vec<std::ffi::OsString>,
) -> Result<OwnerDomainWalkRequest, String> {
    let super::args::Command::OwnerDomainMatch(args) =
        super::args::parse_args(argv).map_err(|e| e.to_string())?
    else {
        return Err("not an owner-domain-match command".into());
    };
    if !args.follow_successors {
        return Err("owner-domain-match argv does not walk (--follow-successors)".into());
    }
    OwnerDomainMatchRequest::validate_query_allowances(args.max_queries, args.max_query_bytes)
        .map_err(str::to_owned)?;
    preflight_checkpoint_paths(&args).map_err(|e| e.to_string())?;
    let request = match_request(&args).map_err(|e| e.to_string())?;
    Ok(walk_request(request, &args))
}

/// Requested steering must not rebrand Ready's flat-ID v5, its compact paused
/// receipt, or a preparation error as an older walk's result schema.
fn annotate_walk_policy(
    document: &mut Value,
    policy: crate::OwnerDomainWalkPublicationPolicy,
    lookahead: Option<std::num::NonZeroUsize>,
) {
    use crate::OwnerDomainWalkPublicationPolicy;
    let owner_batched_report = document["schema"] == "rustred.owner-domain-walk.json.v4";
    if let Some(lookahead) = lookahead {
        document["scheduling_policy"] =
            json!({"kind":"transfer_unreserved", "lookahead":lookahead.get()});
    }
    if policy == OwnerDomainWalkPublicationPolicy::Ready {
        document["requested_publication_policy"] = json!("ready");
        return;
    }
    document["schema"] = json!(if lookahead.is_some() {
        "rustred.owner-domain-walk.json.v3"
    } else {
        "rustred.owner-domain-walk.json.v2"
    });
    if policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
        if owner_batched_report {
            document["schema"] = json!("rustred.owner-domain-walk.json.v4");
        }
        document["requested_publication_policy"] = json!("owner_batched");
    }
}

#[cfg(test)]
mod publication_receipt_tests {
    use super::*;

    #[test]
    fn ready_preserves_actual_final_paused_and_preparation_schemas() {
        for schema in [
            "rustred.owner-domain-walk.json.v5",
            "rustred.owner-domain-walk.paused.json.v1",
            "rustred.owner-domain-match.json.v2",
        ] {
            let mut document = json!({"schema": schema});
            annotate_walk_policy(
                &mut document,
                crate::OwnerDomainWalkPublicationPolicy::Ready,
                std::num::NonZeroUsize::new(256),
            );
            assert_eq!(document["schema"], schema);
            assert_eq!(document["requested_publication_policy"], "ready");
            assert_eq!(document["scheduling_policy"]["lookahead"], 256);
            assert!(document.get("all_scheduled_domains_resolved").is_none());
        }
    }

    #[test]
    fn existing_ordered_and_composite_owner_batched_receipts_are_unchanged() {
        for (policy, source, horizon, expected) in [
            (
                crate::OwnerDomainWalkPublicationPolicy::Ordered,
                "rustred.owner-domain-walk.json.v2",
                None,
                "rustred.owner-domain-walk.json.v2",
            ),
            (
                crate::OwnerDomainWalkPublicationPolicy::Ordered,
                "rustred.owner-domain-walk.json.v2",
                Some(8),
                "rustred.owner-domain-walk.json.v3",
            ),
            (
                crate::OwnerDomainWalkPublicationPolicy::OwnerBatched,
                "rustred.owner-domain-walk.json.v4",
                Some(8),
                "rustred.owner-domain-walk.json.v4",
            ),
            (
                crate::OwnerDomainWalkPublicationPolicy::OwnerBatched,
                "rustred.owner-domain-match.json.v2",
                Some(8),
                "rustred.owner-domain-walk.json.v3",
            ),
        ] {
            let mut document = json!({"schema": source});
            annotate_walk_policy(
                &mut document,
                policy,
                horizon.and_then(std::num::NonZeroUsize::new),
            );
            assert_eq!(document["schema"], expected);
        }
    }
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
