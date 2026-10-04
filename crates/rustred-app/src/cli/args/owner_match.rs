use super::{
    ArgError, Command, next_utf8_value, parse_nonnegative_integer, parse_positive_integer,
};
use crate::application::MAX_WALK_WORKERS;
use std::{collections::BTreeSet, ffi::OsString, num::NonZeroUsize, path::PathBuf};

/// The argument error is a static string; keep it in step with the shared cap.
const MAX_WORKERS_MESSAGE: &str = "at most 256 symbolic workers";
const _: () = assert!(MAX_WALK_WORKERS == 256, "update MAX_WORKERS_MESSAGE");
mod finite_replay_args;
#[cfg(test)]
mod reduction_budget_tests;

#[cfg(test)]
mod campaign_tests;
#[cfg(test)]
mod publication_tests;
#[cfg(test)]
mod query_admission_tests;
#[cfg(test)]
mod worker_budget_tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OwnerDomainMatchArgs {
    pub manifest: PathBuf,
    pub queries: PathBuf,
    pub output: PathBuf,
    pub owner_base: PathBuf,
    pub events: Option<PathBuf>,
    pub stop_file: Option<PathBuf>,
    pub max_queries: usize,
    pub max_query_bytes: usize,
    pub max_total_pieces: usize,
    pub max_rules: usize,
    pub max_terminal_checks: usize,
    pub max_predicates: usize,
    pub max_pieces: usize,
    pub max_cells: usize,
    pub max_split_operations: usize,
    pub max_coordinate_cells: usize,
    pub max_bounded_refinement_cells: usize,
    pub refinement_axes: rustred::solver::OwnerDomainRefinementAxes,
    pub max_guard_univariate_degree: usize,
    pub no_progress: bool,
    pub follow_successors: bool,
    pub finite_replay: Option<crate::OwnerDomainWalkFiniteReplayLimits>,
    pub finite_replay_budget_preflight: bool,
    pub reduction_max_rule_applications: usize,
    pub reduction_max_pending_frames: usize,
    pub reduction_max_coalescing_additions: usize,
    pub workers: usize,
    pub inspection_workers: Option<usize>,
    pub publication_policy: crate::OwnerDomainWalkPublicationPolicy,
    pub epoch_inspector_lookup: Option<crate::OwnerDomainWalkEpochInspectorLookup>,
    pub epoch_rolling: bool,
    pub epoch_dispatch: Option<crate::OwnerDomainWalkEpochDispatchPolicy>,
    pub epoch_publication_order: Option<crate::OwnerDomainWalkEpochPublicationOrder>,
    pub epoch_cut_size: Option<usize>,
    pub epoch_window: Option<usize>,
    pub epoch_result_escrow_jobs: usize,
    pub epoch_result_escrow_bytes: Option<usize>,
    pub epoch_preparation_workers: Option<usize>,
    pub epoch_preparation_max_obligations: Option<usize>,
    pub epoch_preparation_max_retirements: Option<usize>,
    pub route_domain_overcover: bool,
    pub route_joint_source_support_pruning: bool,
    pub max_route_masks: usize,
    pub max_rhs_cells: usize,
    pub max_term_visits: usize,
    pub max_native_operations: usize,
    pub max_rhs_events: usize,
    pub max_shift_groups: usize,
    pub max_sign_splits: usize,
    pub max_domains: usize,
    pub max_frontiers: usize,
    pub frontier_policy: crate::OwnerDomainWalkFrontierPolicy,
    pub max_successor_events: usize,
    pub max_containment_checks: Option<usize>,
    pub transfer_unreserved_lookahead: Option<NonZeroUsize>,
    pub reuse_initial_d_bands: bool,
    pub g2_residual_anchors: crate::OwnerDomainWalkG2ResidualAnchors,
    pub g2_activate_on_resume: bool,
    pub unbounded_work: bool,
    pub checkpoint: Option<crate::OwnerDomainWalkCheckpointOptions>,
    pub apply_subdivision: Option<crate::OwnerDomainWalkApplySubdivision>,
    pub apply_cell_refinement_max_cardinality: Option<NonZeroUsize>,
    /// Resume-time rescue amendments, in chain order (repeatable option).
    pub amend_queries: Vec<PathBuf>,
}

pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let limits = rustred::solver::OwnerDomainMatchLimits::default();
    let applied = rustred::solver::OwnerAppliedLimits::default();
    let reduction = rustred::reduction::ReductionLimits::default();
    let mut result = OwnerDomainMatchArgs {
        manifest: PathBuf::new(),
        queries: PathBuf::new(),
        output: PathBuf::new(),
        owner_base: PathBuf::from("."),
        events: None,
        stop_file: None,
        max_queries: 256,
        max_query_bytes: 1024 * 1024,
        max_total_pieces: 100_000,
        max_rules: limits.max_rules,
        max_terminal_checks: limits.max_terminal_checks,
        max_predicates: limits.max_predicates,
        max_pieces: limits.max_pieces,
        max_cells: limits.max_cells,
        max_split_operations: limits.max_split_operations,
        max_coordinate_cells: limits.max_coordinate_cells,
        max_bounded_refinement_cells: limits.max_bounded_refinement_cells,
        refinement_axes: limits.refinement_axes,
        max_guard_univariate_degree: limits.guard_algebra.max_univariate_degree,
        no_progress: false,
        follow_successors: false,
        finite_replay: None,
        finite_replay_budget_preflight: false,
        reduction_max_rule_applications: reduction.max_rule_applications,
        reduction_max_pending_frames: reduction.max_pending_frames,
        reduction_max_coalescing_additions: reduction.max_coalescing_additions,
        workers: 1,
        inspection_workers: None,
        publication_policy: crate::OwnerDomainWalkPublicationPolicy::Ordered,
        epoch_inspector_lookup: None,
        epoch_rolling: false,
        epoch_dispatch: None,
        epoch_publication_order: None,
        epoch_cut_size: None,
        epoch_window: None,
        epoch_result_escrow_jobs: 0,
        epoch_result_escrow_bytes: None,
        epoch_preparation_workers: None,
        epoch_preparation_max_obligations: None,
        epoch_preparation_max_retirements: None,
        route_domain_overcover: false,
        route_joint_source_support_pruning: false,
        max_route_masks: 100_000,
        max_rhs_cells: applied.max_boundary_cells,
        max_term_visits: applied.max_term_visits,
        max_native_operations: applied.max_native_operations,
        max_rhs_events: applied.max_events,
        max_shift_groups: applied.max_shift_groups,
        max_sign_splits: applied.max_sign_splits,
        max_domains: 100_000,
        max_frontiers: 100_000,
        frontier_policy: crate::OwnerDomainWalkFrontierPolicy::Record,
        max_successor_events: 1_000_000,
        max_containment_checks: None,
        transfer_unreserved_lookahead: None,
        reuse_initial_d_bands: false,
        g2_residual_anchors: crate::OwnerDomainWalkG2ResidualAnchors::Off,
        g2_activate_on_resume: false,
        unbounded_work: false,
        checkpoint: None,
        apply_subdivision: None,
        apply_cell_refinement_max_cardinality: None,
        amend_queries: Vec::new(),
    };
    let mut checkpoint_path = None;
    let mut resume_path = None;
    let mut checkpoint_interval = None;
    let mut subdivision_axis = None;
    let mut subdivision_cut = None;
    let mut seen = BTreeSet::new();
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        // The one repeatable option: every rescue amendment of the chain.
        if option == "--amend-queries" {
            let value = next_utf8_value(&mut arguments, "--amend-queries")?;
            if value.is_empty() || value == "-" {
                return Err(ArgError::InvalidValue {
                    option: "--amend-queries",
                    value,
                    expected: "a filesystem path",
                });
            }
            result.amend_queries.push(PathBuf::from(value));
            seen.insert("--amend-queries");
            continue;
        }
        let name = match option.as_str() {
            "--manifest" => "--manifest",
            "--queries" => "--queries",
            "--output" => "--output",
            "--owner-base" => "--owner-base",
            "--events" => "--events",
            "--stop-file" => "--stop-file",
            "--max-queries" => "--max-queries",
            "--max-query-bytes" => "--max-query-bytes",
            "--max-total-pieces" => "--max-total-pieces",
            "--max-rules-per-query" => "--max-rules-per-query",
            "--max-terminal-checks-per-query" => "--max-terminal-checks-per-query",
            "--max-predicates-per-query" => "--max-predicates-per-query",
            "--max-pieces-per-query" => "--max-pieces-per-query",
            "--max-cells-per-query" => "--max-cells-per-query",
            "--max-split-operations-per-query" => "--max-split-operations-per-query",
            "--max-coordinate-cells-per-query" => "--max-coordinate-cells-per-query",
            "--max-guard-univariate-degree" => "--max-guard-univariate-degree",
            "--bounded-refinement-axes" => "--bounded-refinement-axes",
            "--max-bounded-refinement-cells-per-query" => {
                "--max-bounded-refinement-cells-per-query"
            }
            "--no-progress" => "--no-progress",
            "--follow-successors" => "--follow-successors",
            "--finite-replay-budget-preflight" => "--finite-replay-budget-preflight",
            "--reduction-max-rule-applications" => "--reduction-max-rule-applications",
            "--reduction-max-pending-frames" => "--reduction-max-pending-frames",
            "--reduction-max-coalescing-additions" => "--reduction-max-coalescing-additions",
            "--workers" => "--workers",
            "--publication-policy" => "--publication-policy",
            "--epoch-inspector-lookup" => "--epoch-inspector-lookup",
            "--epoch-rolling" => "--epoch-rolling",
            "--epoch-dispatch" => "--epoch-dispatch",
            "--epoch-publication-order" => "--epoch-publication-order",
            "--epoch-cut-size" => "--epoch-cut-size",
            "--epoch-window" => "--epoch-window",
            "--epoch-result-escrow-jobs" => "--epoch-result-escrow-jobs",
            "--epoch-result-escrow-bytes" => "--epoch-result-escrow-bytes",
            "--epoch-preparation-workers" => "--epoch-preparation-workers",
            "--epoch-preparation-max-obligations" => "--epoch-preparation-max-obligations",
            "--epoch-preparation-max-retirements" => "--epoch-preparation-max-retirements",
            "--inspection-workers" => "--inspection-workers",
            "--route-domain-overcover" => "--route-domain-overcover",
            "--route-joint-source-support-pruning" => "--route-joint-source-support-pruning",
            "--max-route-masks-per-query" => "--max-route-masks-per-query",
            "--max-rhs-cells-per-query" => "--max-rhs-cells-per-query",
            "--max-term-visits-per-query" => "--max-term-visits-per-query",
            "--max-native-operations-per-query" => "--max-native-operations-per-query",
            "--max-rhs-events-per-query" => "--max-rhs-events-per-query",
            "--max-shift-groups-per-query" => "--max-shift-groups-per-query",
            "--max-sign-splits-per-query" => "--max-sign-splits-per-query",
            "--max-domains" => "--max-domains",
            "--max-frontiers" => "--max-frontiers",
            "--frontier-policy" => "--frontier-policy",
            "--max-successor-events" => "--max-successor-events",
            "--max-containment-checks" => "--max-containment-checks",
            "--transfer-unreserved-lookahead" => "--transfer-unreserved-lookahead",
            "--reuse-initial-d-bands" => "--reuse-initial-d-bands",
            "--g2-residual-anchors" => "--g2-residual-anchors",
            "--g2-activate-on-resume" => "--g2-activate-on-resume",
            "--unbounded-work" => "--unbounded-work",
            "--checkpoint" => "--checkpoint",
            "--resume" => "--resume",
            "--checkpoint-interval-seconds" => "--checkpoint-interval-seconds",
            "--apply-subdivision-axis" => "--apply-subdivision-axis",
            "--apply-subdivision-cut" => "--apply-subdivision-cut",
            "--apply-cell-refinement-max-cardinality" => "--apply-cell-refinement-max-cardinality",
            "--help" | "-h" => return Ok(Command::Help),
            _ => finite_replay_args::option_name(&option)
                .ok_or_else(|| ArgError::UnknownOption(option.clone()))?,
        };
        if !seen.insert(name) {
            return Err(ArgError::DuplicateOption(name));
        }
        if name == "--no-progress" {
            result.no_progress = true;
            continue;
        }
        if name == "--follow-successors" {
            result.follow_successors = true;
            continue;
        }
        if name == "--finite-replay-budget-preflight" {
            result.finite_replay_budget_preflight = true;
            continue;
        }
        if name == finite_replay_args::ENABLE {
            result
                .finite_replay
                .get_or_insert_with(finite_replay_args::defaults);
            continue;
        }
        if name == "--epoch-rolling" {
            result.epoch_rolling = true;
            continue;
        }
        if name == "--route-domain-overcover" {
            result.route_domain_overcover = true;
            continue;
        }
        if name == "--route-joint-source-support-pruning" {
            result.route_joint_source_support_pruning = true;
            continue;
        }
        if name == "--reuse-initial-d-bands" {
            result.reuse_initial_d_bands = true;
            continue;
        }
        if name == "--g2-activate-on-resume" {
            result.g2_activate_on_resume = true;
            continue;
        }
        if name == "--unbounded-work" {
            result.unbounded_work = true;
            continue;
        }
        let value = next_utf8_value(&mut arguments, name)?;
        if finite_replay_args::option_name(name).is_some() {
            let cap = parse_nonnegative_integer(name, value)?;
            finite_replay_args::set_cap(
                result
                    .finite_replay
                    .get_or_insert_with(finite_replay_args::defaults),
                name,
                cap,
            );
            continue;
        }
        match name {
            "--apply-cell-refinement-max-cardinality" => {
                result.apply_cell_refinement_max_cardinality =
                    NonZeroUsize::new(parse_positive_integer(name, value)?);
            }
            "--apply-subdivision-axis" => {
                subdivision_axis = Some(parse_nonnegative_integer(name, value)?);
            }
            "--apply-subdivision-cut" => {
                subdivision_cut = Some(parse_nonnegative_integer(name, value)? as u64);
            }
            "--checkpoint-interval-seconds" => {
                checkpoint_interval = Some(parse_positive_integer(name, value)? as u64);
            }
            "--inspection-workers" => {
                result.inspection_workers = Some(parse_positive_integer(name, value)?);
            }
            "--publication-policy" => {
                result.publication_policy = match value.as_str() {
                    "ordered" => crate::OwnerDomainWalkPublicationPolicy::Ordered,
                    "owner-batched" => crate::OwnerDomainWalkPublicationPolicy::OwnerBatched,
                    "ready" => crate::OwnerDomainWalkPublicationPolicy::Ready,
                    "epoch" => crate::OwnerDomainWalkPublicationPolicy::Epoch,
                    _ => {
                        return Err(ArgError::InvalidValue {
                            option: name,
                            value,
                            expected: "ordered, owner-batched, ready, or epoch",
                        });
                    }
                };
            }
            "--transfer-unreserved-lookahead" => {
                result.transfer_unreserved_lookahead =
                    NonZeroUsize::new(parse_positive_integer(name, value)?);
            }
            "--epoch-inspector-lookup" => {
                result.epoch_inspector_lookup = Some(
                    crate::OwnerDomainWalkEpochInspectorLookup::parse(&value).ok_or(
                        ArgError::InvalidValue {
                            option: name,
                            value,
                            expected: "all-miss or snapshot",
                        },
                    )?,
                );
            }
            "--epoch-dispatch" => {
                result.epoch_dispatch = Some(
                    crate::OwnerDomainWalkEpochDispatchPolicy::parse(&value).ok_or(
                        ArgError::InvalidValue {
                            option: name,
                            value,
                            expected: "fifo or adaptive",
                        },
                    )?,
                );
            }
            "--epoch-publication-order" => {
                result.epoch_publication_order = Some(
                    crate::OwnerDomainWalkEpochPublicationOrder::parse(&value).ok_or(
                        ArgError::InvalidValue {
                            option: name,
                            value,
                            expected: "oldest-prefix or oldest-ready",
                        },
                    )?,
                );
            }
            "--epoch-cut-size" => {
                result.epoch_cut_size = Some(parse_positive_integer(name, value)?);
            }
            "--epoch-window" => {
                result.epoch_window = Some(parse_positive_integer(name, value)?);
            }
            "--epoch-result-escrow-jobs" => {
                result.epoch_result_escrow_jobs = parse_nonnegative_integer(name, value)?;
            }
            "--epoch-result-escrow-bytes" => {
                result.epoch_result_escrow_bytes = Some(parse_positive_integer(name, value)?);
            }
            "--epoch-preparation-workers" => {
                result.epoch_preparation_workers = Some(parse_nonnegative_integer(name, value)?);
            }
            "--epoch-preparation-max-obligations" => {
                result.epoch_preparation_max_obligations =
                    Some(parse_positive_integer(name, value)?);
            }
            "--epoch-preparation-max-retirements" => {
                result.epoch_preparation_max_retirements =
                    Some(parse_positive_integer(name, value)?);
            }
            "--frontier-policy" => {
                result.frontier_policy = crate::OwnerDomainWalkFrontierPolicy::parse(&value)
                    .ok_or(ArgError::InvalidValue {
                        option: name,
                        value,
                        expected: "record or stop",
                    })?;
            }
            "--g2-residual-anchors" => {
                result.g2_residual_anchors = crate::OwnerDomainWalkG2ResidualAnchors::parse(&value)
                    .ok_or(ArgError::InvalidValue {
                        option: name,
                        value,
                        expected: "off or union",
                    })?;
            }
            "--bounded-refinement-axes" => {
                result.refinement_axes = match value.as_str() {
                    "inactive-only" => rustred::solver::OwnerDomainRefinementAxes::InactiveOnly,
                    "finite-axes" => rustred::solver::OwnerDomainRefinementAxes::FiniteAxes,
                    _ => {
                        return Err(ArgError::InvalidValue {
                            option: name,
                            value,
                            expected: "inactive-only or finite-axes",
                        });
                    }
                };
            }
            "--max-containment-checks" => {
                result.max_containment_checks = if value == "unlimited" {
                    None
                } else {
                    Some(parse_positive_integer(name, value)?)
                };
            }
            "--max-bounded-refinement-cells-per-query" => {
                result.max_bounded_refinement_cells = parse_nonnegative_integer(name, value)?;
            }
            "--reduction-max-rule-applications" => {
                result.reduction_max_rule_applications = parse_nonnegative_integer(name, value)?;
            }
            "--reduction-max-pending-frames" => {
                result.reduction_max_pending_frames = parse_nonnegative_integer(name, value)?;
            }
            "--reduction-max-coalescing-additions" => {
                result.reduction_max_coalescing_additions = parse_nonnegative_integer(name, value)?;
            }
            "--manifest" | "--queries" | "--output" | "--owner-base" | "--events"
            | "--stop-file" | "--checkpoint" | "--resume" => {
                if value.is_empty() || value == "-" {
                    return Err(ArgError::InvalidValue {
                        option: name,
                        value,
                        expected: "a filesystem path",
                    });
                }
                let path = PathBuf::from(value);
                match name {
                    "--manifest" => result.manifest = path,
                    "--queries" => result.queries = path,
                    "--output" => result.output = path,
                    "--owner-base" => result.owner_base = path,
                    "--events" => result.events = Some(path),
                    "--checkpoint" => checkpoint_path = Some(path),
                    "--resume" => resume_path = Some(path),
                    _ => result.stop_file = Some(path),
                }
            }
            _ => {
                let value = parse_positive_integer(name, value)?;
                match name {
                    "--workers" => result.workers = value,
                    "--max-queries" => result.max_queries = value,
                    "--max-query-bytes" => result.max_query_bytes = value,
                    "--max-total-pieces" => result.max_total_pieces = value,
                    "--max-rules-per-query" => result.max_rules = value,
                    "--max-terminal-checks-per-query" => result.max_terminal_checks = value,
                    "--max-predicates-per-query" => result.max_predicates = value,
                    "--max-pieces-per-query" => result.max_pieces = value,
                    "--max-cells-per-query" => result.max_cells = value,
                    "--max-split-operations-per-query" => result.max_split_operations = value,
                    "--max-domains" => result.max_domains = value,
                    "--max-frontiers" => result.max_frontiers = value,
                    "--max-successor-events" => result.max_successor_events = value,
                    "--max-route-masks-per-query" => result.max_route_masks = value,
                    "--max-rhs-cells-per-query" => result.max_rhs_cells = value,
                    "--max-term-visits-per-query" => result.max_term_visits = value,
                    "--max-native-operations-per-query" => result.max_native_operations = value,
                    "--max-rhs-events-per-query" => result.max_rhs_events = value,
                    "--max-shift-groups-per-query" => result.max_shift_groups = value,
                    "--max-sign-splits-per-query" => result.max_sign_splits = value,
                    "--max-guard-univariate-degree" => result.max_guard_univariate_degree = value,
                    _ => result.max_coordinate_cells = value,
                }
            }
        }
    }
    for name in ["--manifest", "--queries", "--output"] {
        if !seen.contains(name) {
            return Err(ArgError::MissingRequiredOption(name));
        }
    }
    if checkpoint_path.is_some() && resume_path.is_some() {
        return Err(ArgError::InvalidCombination(
            "--checkpoint and --resume are mutually exclusive",
        ));
    }
    if !result.amend_queries.is_empty() && resume_path.is_none() {
        return Err(ArgError::InvalidCombination(
            "--amend-queries requires --resume (a rescue amends a saved walk)",
        ));
    }
    let resume = resume_path.is_some();
    result.apply_subdivision = match (subdivision_axis, subdivision_cut) {
        (Some(axis), Some(cut)) => Some(crate::OwnerDomainWalkApplySubdivision { axis, cut }),
        (None, None) => None,
        _ => {
            return Err(ArgError::InvalidCombination(
                "--apply-subdivision-axis and --apply-subdivision-cut must be supplied together",
            ));
        }
    };
    if let Some(path) = checkpoint_path.or(resume_path) {
        let mut checkpoint = crate::OwnerDomainWalkCheckpointOptions::new(path);
        checkpoint.resume = resume;
        if let Some(seconds) = checkpoint_interval {
            checkpoint.interval_seconds = seconds;
        }
        result.checkpoint = Some(checkpoint);
    } else if checkpoint_interval.is_some() {
        return Err(ArgError::InvalidCombination(
            "--checkpoint-interval-seconds requires --checkpoint or --resume",
        ));
    }
    if result.unbounded_work
        && seen.iter().any(|name| {
            matches!(
                *name,
                "--max-rules-per-query"
                    | "--max-terminal-checks-per-query"
                    | "--max-predicates-per-query"
                    | "--max-pieces-per-query"
                    | "--max-cells-per-query"
                    | "--max-split-operations-per-query"
                    | "--max-coordinate-cells-per-query"
                    | "--max-bounded-refinement-cells-per-query"
                    | "--max-domains"
                    | "--max-frontiers"
                    | "--max-successor-events"
                    | "--max-containment-checks"
                    | "--max-route-masks-per-query"
                    | "--max-rhs-cells-per-query"
                    | "--max-term-visits-per-query"
                    | "--max-native-operations-per-query"
                    | "--max-rhs-events-per-query"
                    | "--max-shift-groups-per-query"
                    | "--max-sign-splits-per-query"
            )
        })
    {
        return Err(ArgError::InvalidCombination(
            "--unbounded-work cannot be combined with explicit diagnostic work caps",
        ));
    }
    let epoch = result.publication_policy == crate::OwnerDomainWalkPublicationPolicy::Epoch;
    if result.epoch_result_escrow_jobs == 0 && result.epoch_result_escrow_bytes.is_some() {
        return Err(ArgError::InvalidCombination(
            "Epoch result escrow bytes require positive escrow jobs",
        ));
    }
    if result.epoch_result_escrow_jobs > 0 {
        if !result.follow_successors
            || !epoch
            || !result.epoch_rolling
            || result.checkpoint.is_none()
            || result.epoch_publication_order.is_some_and(|order| {
                order != crate::OwnerDomainWalkEpochPublicationOrder::OldestPrefix
            })
            || result.epoch_result_escrow_bytes.is_none()
        {
            return Err(ArgError::InvalidCombination(
                "Epoch result escrow requires rolling oldest-prefix CP6 and positive explicit bytes",
            ));
        }
        if result.epoch_result_escrow_jobs >= 4096
            || result.epoch_window.is_some_and(|base| {
                base.checked_add(result.epoch_result_escrow_jobs)
                    .is_none_or(|total| total > 4096)
            })
        {
            return Err(ArgError::InvalidCombination(
                "Epoch base window plus escrow jobs must be at most 4096",
            ));
        }
    }
    if (result.epoch_preparation_workers.is_some()
        || result.epoch_preparation_max_obligations.is_some()
        || result.epoch_preparation_max_retirements.is_some())
        && (!result.follow_successors || !epoch)
    {
        return Err(ArgError::InvalidCombination(
            "Epoch preparation options require --follow-successors and --publication-policy epoch",
        ));
    }
    if result
        .epoch_preparation_max_obligations
        .is_some_and(|n| n > u32::MAX as usize)
        || result
            .epoch_preparation_max_retirements
            .is_some_and(|n| n > u32::MAX as usize)
    {
        return Err(ArgError::InvalidCombination(
            "Epoch preparation counts must be in 1..=u32::MAX",
        ));
    }
    crate::OwnerDomainWalkRequest::validate_epoch_worker_partition(
        result.workers,
        result.inspection_workers,
        result.max_containment_checks,
        result.epoch_preparation_workers,
    )
    .map_err(ArgError::InvalidCombination)?;
    if (result.epoch_publication_order.is_some()
        || result.epoch_cut_size.is_some()
        || result.epoch_window.is_some())
        && (!result.follow_successors
            || !epoch
            || !result.epoch_rolling
            || result.checkpoint.is_none())
    {
        return Err(ArgError::InvalidCombination(
            "Epoch publication order, cut size and window require --follow-successors, --publication-policy epoch, --epoch-rolling and --checkpoint or --resume",
        ));
    }
    if result.epoch_cut_size.is_some_and(|value| value > 4096)
        || result.epoch_window.is_some_and(|value| value > 4096)
    {
        return Err(ArgError::InvalidCombination(
            "Epoch cut size and window must be in 1..=4096",
        ));
    }
    if result.epoch_dispatch.is_some()
        && (!result.follow_successors
            || !epoch
            || result.checkpoint.is_none()
            || (result.epoch_dispatch == Some(crate::OwnerDomainWalkEpochDispatchPolicy::Adaptive)
                && !result.epoch_rolling))
    {
        return Err(ArgError::InvalidCombination(
            "--epoch-dispatch requires --follow-successors, --publication-policy epoch and --checkpoint or --resume; adaptive also requires --epoch-rolling",
        ));
    }
    if result.epoch_rolling && (!result.follow_successors || !epoch || result.checkpoint.is_none())
    {
        return Err(ArgError::InvalidCombination(
            "--epoch-rolling requires --follow-successors, --publication-policy epoch and --checkpoint or --resume",
        ));
    }
    // Epoch: frontier stop is the default (A10) and needs
    // no checkpoint; explicit checkpoint/resume uses CP6.
    if result.epoch_inspector_lookup.is_some()
        && (!result.follow_successors || !epoch || result.checkpoint.is_none())
    {
        return Err(ArgError::InvalidCombination(
            "--epoch-inspector-lookup requires --follow-successors, --publication-policy epoch and --checkpoint or --resume",
        ));
    }
    if epoch && !seen.contains("--frontier-policy") {
        result.frontier_policy = crate::OwnerDomainWalkFrontierPolicy::Stop;
    }
    if result.frontier_policy == crate::OwnerDomainWalkFrontierPolicy::Stop
        && result.checkpoint.is_none()
        && !epoch
    {
        return Err(ArgError::InvalidCombination(
            "--frontier-policy stop requires --checkpoint or --resume",
        ));
    }
    if result.checkpoint.is_some()
        && result.publication_policy == crate::OwnerDomainWalkPublicationPolicy::OwnerBatched
    {
        return Err(ArgError::InvalidCombination(
            "checkpoint/resume requires ordered or ready publication",
        ));
    }
    if result.publication_policy == crate::OwnerDomainWalkPublicationPolicy::Ready
        && result.transfer_unreserved_lookahead.is_none()
    {
        return Err(ArgError::InvalidCombination(
            "ready publication requires --transfer-unreserved-lookahead",
        ));
    }
    if epoch && result.transfer_unreserved_lookahead.is_none() {
        return Err(ArgError::InvalidCombination(
            "epoch publication requires --transfer-unreserved-lookahead (transfers are part of Epoch semantics; the value is not used)",
        ));
    }
    if result.apply_subdivision.is_some()
        && result.publication_policy != crate::OwnerDomainWalkPublicationPolicy::Ordered
    {
        return Err(ArgError::InvalidCombination(
            "physical Apply subdivision requires ordered publication",
        ));
    }
    crate::OwnerDomainMatchRequest::validate_query_allowances(
        result.max_queries,
        result.max_query_bytes,
    )
    .map_err(ArgError::InvalidCombination)?;
    if result.max_total_pieces > 1_000_000 {
        return Err(ArgError::InvalidCombination(
            "at most 1000000 retained pieces",
        ));
    }
    if result.max_frontiers > 1_000_000 {
        return Err(ArgError::InvalidCombination(
            "at most 1000000 retained frontiers",
        ));
    }
    if result.workers > MAX_WALK_WORKERS {
        return Err(ArgError::InvalidCombination(MAX_WORKERS_MESSAGE));
    }
    if !result.follow_successors
        && [
            "--workers",
            "--unbounded-work",
            "--checkpoint",
            "--resume",
            "--checkpoint-interval-seconds",
            "--apply-subdivision-axis",
            "--apply-subdivision-cut",
            "--apply-cell-refinement-max-cardinality",
            "--amend-queries",
            "--inspection-workers",
            "--publication-policy",
            "--epoch-inspector-lookup",
            "--epoch-rolling",
            "--epoch-dispatch",
            "--epoch-publication-order",
            "--epoch-cut-size",
            "--epoch-window",
            "--epoch-preparation-workers",
            "--epoch-preparation-max-obligations",
            "--epoch-preparation-max-retirements",
            "--max-domains",
            "--max-frontiers",
            "--frontier-policy",
            "--max-successor-events",
            "--max-containment-checks",
            "--transfer-unreserved-lookahead",
            "--reuse-initial-d-bands",
            "--g2-residual-anchors",
            "--g2-activate-on-resume",
            "--route-domain-overcover",
            "--route-joint-source-support-pruning",
            "--max-route-masks-per-query",
            "--max-rhs-cells-per-query",
            "--max-term-visits-per-query",
            "--max-native-operations-per-query",
            "--max-rhs-events-per-query",
            "--max-shift-groups-per-query",
            "--max-sign-splits-per-query",
        ]
        .iter()
        .any(|name| seen.contains(name))
    {
        return Err(ArgError::InvalidCombination(
            "successor work allowances require --follow-successors",
        ));
    }
    if !result.route_domain_overcover && seen.contains("--max-route-masks-per-query") {
        return Err(ArgError::InvalidCombination(
            "route mask allowance requires --route-domain-overcover",
        ));
    }
    if result.route_joint_source_support_pruning && !result.route_domain_overcover {
        return Err(ArgError::InvalidCombination(
            "joint source-support pruning requires --route-domain-overcover",
        ));
    }
    crate::OwnerDomainWalkRequest::validate_inspection_workers(
        result.workers,
        result.inspection_workers,
        result.max_containment_checks,
    )
    .map_err(ArgError::InvalidCombination)?;
    if result.transfer_unreserved_lookahead.is_some() && result.max_containment_checks.is_some() {
        return Err(ArgError::InvalidCombination(
            "--transfer-unreserved-lookahead requires unlimited containment checks",
        ));
    }
    if result.reuse_initial_d_bands && result.transfer_unreserved_lookahead.is_none() {
        return Err(ArgError::InvalidCombination(
            "--reuse-initial-d-bands requires --transfer-unreserved-lookahead",
        ));
    }
    if result.g2_residual_anchors != crate::OwnerDomainWalkG2ResidualAnchors::Off {
        if result.transfer_unreserved_lookahead.is_none() {
            return Err(ArgError::InvalidCombination(
                "--g2-residual-anchors union requires --transfer-unreserved-lookahead",
            ));
        }
        if result.apply_subdivision.is_some() {
            return Err(ArgError::InvalidCombination(
                "--g2-residual-anchors union does not support physical Apply subdivision",
            ));
        }
        if matches!(
            result.publication_policy,
            crate::OwnerDomainWalkPublicationPolicy::OwnerBatched
        ) {
            return Err(ArgError::InvalidCombination(
                "--g2-residual-anchors union requires ordered, ready or epoch publication",
            ));
        }
    }
    if result.g2_activate_on_resume
        && (result.g2_residual_anchors == crate::OwnerDomainWalkG2ResidualAnchors::Off
            || !result.checkpoint.as_ref().is_some_and(|c| c.resume))
    {
        return Err(ArgError::InvalidCombination(
            "--g2-activate-on-resume requires --resume and --g2-residual-anchors union",
        ));
    }
    finite_replay_args::validate(&result, &seen)?;
    Ok(Command::OwnerDomainMatch(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(text: &str) -> Result<Command, ArgError> {
        super::parse(text.split_whitespace().map(OsString::from))
    }

    #[test]
    fn result_escrow_requires_paired_bytes_and_rolling_prefix() {
        let base = "--manifest m --queries q --output o --follow-successors --publication-policy epoch --transfer-unreserved-lookahead 16 --checkpoint cp --epoch-rolling";
        let Command::OwnerDomainMatch(args) = parse(&format!("{base} --epoch-window 76 --epoch-result-escrow-jobs 16 --epoch-result-escrow-bytes 1048576")).unwrap() else { panic!("match") };
        assert_eq!(args.epoch_result_escrow_jobs, 16);
        assert_eq!(args.epoch_result_escrow_bytes, Some(1048576));
        for suffix in [
            "--epoch-result-escrow-jobs 1",
            "--epoch-result-escrow-bytes 1",
            "--epoch-result-escrow-jobs 1 --epoch-result-escrow-bytes 0",
            "--epoch-result-escrow-jobs 1 --epoch-result-escrow-bytes 1 --epoch-publication-order oldest-ready",
            "--epoch-result-escrow-jobs 4096 --epoch-result-escrow-bytes 1",
            "--epoch-result-escrow-jobs 1 --epoch-result-escrow-bytes 1 --epoch-window 4096",
            "--epoch-result-escrow-jobs 1 --epoch-result-escrow-bytes 1 --epoch-result-escrow-jobs 1",
        ] {
            assert!(parse(&format!("{base} {suffix}")).is_err(), "{suffix}");
        }
        assert!(parse("--manifest m --queries q --output o --epoch-result-escrow-jobs 1 --epoch-result-escrow-bytes 1").is_err());
        let Command::OwnerDomainMatch(zero) =
            parse("--manifest m --queries q --output o --epoch-result-escrow-jobs 0").unwrap()
        else {
            panic!("match")
        };
        assert_eq!(zero.epoch_result_escrow_jobs, 0);
    }

    #[test]
    fn rolling_batch_controls_are_explicit_bounded_and_require_durable_epoch() {
        let base = "--manifest m --queries q --output o --follow-successors";
        let epoch = format!(
            "{base} --publication-policy epoch --transfer-unreserved-lookahead 16 --checkpoint cp"
        );
        let Command::OwnerDomainMatch(default) = parse(&epoch).unwrap() else {
            panic!("match")
        };
        assert_eq!(default.epoch_publication_order, None);
        assert_eq!(default.epoch_cut_size, None);
        assert_eq!(default.epoch_window, None);
        for mode in ["oldest-prefix", "oldest-ready"] {
            let Command::OwnerDomainMatch(args) = parse(&format!(
                "{epoch} --epoch-rolling --epoch-publication-order {mode} --epoch-cut-size 16 --epoch-window 800"
            )).unwrap() else { panic!("match") };
            assert_eq!(args.epoch_publication_order.unwrap().name(), mode);
            assert_eq!(args.epoch_cut_size, Some(16));
            assert_eq!(args.epoch_window, Some(800));
        }
        for bad in [
            format!("{base} --epoch-window 32"),
            format!("{epoch} --epoch-cut-size 16"),
            format!("{epoch} --epoch-rolling --epoch-cut-size 0"),
            format!("{epoch} --epoch-rolling --epoch-window 4097"),
            format!("{epoch} --epoch-rolling --epoch-cut-size 4097"),
            format!("{epoch} --epoch-rolling --epoch-publication-order invalid"),
            format!("{epoch} --epoch-rolling --epoch-window 32 --epoch-window 64"),
            format!(
                "{epoch} --epoch-rolling --epoch-publication-order oldest-ready --epoch-publication-order oldest-prefix"
            ),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn adaptive_dispatch_requires_rolling_durable_epoch_and_explicit_valid_policy() {
        let base = "--manifest m --queries q --output o --follow-successors";
        let epoch = format!(
            "{base} --publication-policy epoch --transfer-unreserved-lookahead 16 --checkpoint cp"
        );
        for mode in ["fifo", "adaptive"] {
            let Command::OwnerDomainMatch(args) =
                parse(&format!("{epoch} --epoch-rolling --epoch-dispatch {mode}")).unwrap()
            else {
                panic!("match")
            };
            assert_eq!(args.epoch_dispatch.unwrap().name(), mode);
        }
        for bad in [
            format!("{base} --epoch-dispatch fifo"),
            format!("{epoch} --epoch-dispatch adaptive"),
            format!("{epoch} --epoch-rolling --epoch-dispatch unknown"),
            format!("{epoch} --epoch-rolling --epoch-dispatch fifo --epoch-dispatch adaptive"),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn epoch_rolling_is_explicit_checkpoint_only_and_rejects_duplicates() {
        let base = "--manifest m --queries q --output o --follow-successors";
        let epoch = format!("{base} --publication-policy epoch --transfer-unreserved-lookahead 16");
        let Command::OwnerDomainMatch(default) = parse(base).unwrap() else {
            panic!("match")
        };
        assert!(!default.epoch_rolling);
        for checkpoint in ["--checkpoint", "--resume"] {
            let Command::OwnerDomainMatch(args) =
                parse(&format!("{epoch} {checkpoint} cp --epoch-rolling")).unwrap()
            else {
                panic!("match")
            };
            assert!(args.epoch_rolling);
        }
        for bad in [
            format!("{base} --epoch-rolling"),
            format!("{base} --checkpoint cp --epoch-rolling"),
            format!("{epoch} --epoch-rolling"),
            format!("{epoch} --checkpoint cp --epoch-rolling --epoch-rolling"),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn epoch_inspector_lookup_is_explicit_durable_and_rejects_duplicate_or_wrong_scope() {
        let base = "--manifest m --queries q --output o --follow-successors";
        let epoch = format!("{base} --publication-policy epoch --transfer-unreserved-lookahead 16");
        let Command::OwnerDomainMatch(default) = parse(base).unwrap() else {
            panic!("match")
        };
        assert_eq!(default.epoch_inspector_lookup, None);
        for checkpoint in ["--checkpoint", "--resume"] {
            for mode in ["all-miss", "snapshot"] {
                let Command::OwnerDomainMatch(args) = parse(&format!(
                    "{epoch} {checkpoint} cp --epoch-inspector-lookup {mode}"
                ))
                .unwrap() else {
                    panic!("match")
                };
                assert_eq!(args.epoch_inspector_lookup.unwrap().name(), mode);
            }
        }
        for bad in [
            format!("{base} --epoch-inspector-lookup all-miss"),
            format!("{base} --checkpoint cp --epoch-inspector-lookup snapshot"),
            format!("{epoch} --epoch-inspector-lookup snapshot"),
            format!("{epoch} --checkpoint cp --epoch-inspector-lookup maybe"),
            format!(
                "{epoch} --checkpoint cp --epoch-inspector-lookup snapshot --epoch-inspector-lookup all-miss"
            ),
            format!(
                "{} --checkpoint cp --epoch-inspector-lookup snapshot",
                epoch.replace("--follow-successors", "")
            ),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn g2_residual_anchors_parse_and_require_the_ledger() {
        let base = "--manifest m --queries q --output o --follow-successors";
        let Command::OwnerDomainMatch(args) = parse(base).unwrap() else {
            panic!("match command")
        };
        assert_eq!(
            args.g2_residual_anchors,
            crate::OwnerDomainWalkG2ResidualAnchors::Off
        );
        let Command::OwnerDomainMatch(args) = parse(&format!(
            "{base} --transfer-unreserved-lookahead 256 --g2-residual-anchors union"
        ))
        .unwrap() else {
            panic!("match command")
        };
        assert_eq!(
            args.g2_residual_anchors,
            crate::OwnerDomainWalkG2ResidualAnchors::Union
        );
        for bad in [
            format!("{base} --g2-residual-anchors union"),
            format!("{base} --transfer-unreserved-lookahead 256 --g2-residual-anchors maybe"),
            "--manifest m --queries q --output o --g2-residual-anchors union".to_owned(),
            format!(
                "{base} --transfer-unreserved-lookahead 256 --g2-residual-anchors union --publication-policy owner-batched"
            ),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn owner_domain_match_requires_external_queries_and_keeps_native_defaults() {
        let Command::OwnerDomainMatch(args) = parse("--manifest m --queries q --output o").unwrap()
        else {
            panic!("match command")
        };
        let limits = rustred::solver::OwnerDomainMatchLimits::default();
        let applied = rustred::solver::OwnerAppliedLimits::default();
        assert_eq!(args.workers, 1);
        assert_eq!(args.max_containment_checks, None);
        assert_eq!(args.transfer_unreserved_lookahead, None);
        assert!(!args.reuse_initial_d_bands);
        assert_eq!(args.max_frontiers, 100_000);
        assert_eq!(args.max_rhs_events, applied.max_events);
        assert_eq!(args.max_shift_groups, applied.max_shift_groups);
        assert_eq!(args.max_sign_splits, applied.max_sign_splits);
        assert_eq!(args.max_queries, 256);
        assert_eq!(args.max_total_pieces, 100_000);
        assert_eq!(args.max_rules, limits.max_rules);
        assert_eq!(args.max_terminal_checks, limits.max_terminal_checks);
        assert_eq!(args.max_predicates, limits.max_predicates);
        assert_eq!(args.max_pieces, limits.max_pieces);
        assert_eq!(args.max_cells, limits.max_cells);
        assert_eq!(args.max_split_operations, limits.max_split_operations);
        assert_eq!(args.max_coordinate_cells, limits.max_coordinate_cells);
        assert_eq!(args.max_bounded_refinement_cells, 0);
        assert_eq!(
            args.refinement_axes,
            rustred::solver::OwnerDomainRefinementAxes::InactiveOnly
        );
        assert_eq!(
            args.max_guard_univariate_degree,
            limits.guard_algebra.max_univariate_degree
        );
        for text in [
            "--queries q --output o",
            "--manifest m --output o",
            "--manifest m --queries q",
        ] {
            assert!(parse(text).is_err());
        }
    }

    #[test]
    fn owner_domain_match_forwards_all_explicit_allowances_and_presentation_paths() {
        let Command::OwnerDomainMatch(args) = parse("--manifest m --queries q --output o --owner-base b --events e --stop-file s --no-progress --max-queries 9 --max-total-pieces 10 --max-rules-per-query 11 --max-terminal-checks-per-query 12 --max-predicates-per-query 13 --max-pieces-per-query 14 --max-cells-per-query 15 --max-split-operations-per-query 16 --max-coordinate-cells-per-query 17").unwrap() else { panic!("match command") };
        assert_eq!(
            [
                args.max_queries,
                args.max_total_pieces,
                args.max_rules,
                args.max_terminal_checks,
                args.max_predicates,
                args.max_pieces,
                args.max_cells,
                args.max_split_operations,
                args.max_coordinate_cells
            ],
            [9, 10, 11, 12, 13, 14, 15, 16, 17]
        );
        assert_eq!(args.owner_base, PathBuf::from("b"));
        assert_eq!(args.events, Some(PathBuf::from("e")));
        assert_eq!(args.stop_file, Some(PathBuf::from("s")));
        assert!(args.no_progress);
    }

    #[test]
    fn owner_domain_match_rejects_duplicate_invalid_and_scope_overrides() {
        for suffix in [
            "--max-queries 0",
            "--max-total-pieces 1000001",
            "--max-rules-per-query -1",
            "--max-cells-per-query +2",
            "--max-bounded-refinement-cells-per-query -1",
            "--bounded-refinement-axes all",
            "--bounded-refinement-axes FiniteAxes",
            "--bounded-refinement-axes finite-axes --bounded-refinement-axes inactive-only",
            "--max-guard-univariate-degree 0",
            "--max-pieces-per-query 1 --max-pieces-per-query 2",
            "--queries duplicate",
            "--output -",
            "--no-progress --no-progress",
            "--max-numerator-rank 10",
            "--workers 6",
            "--targets t",
            "--timeout 60",
            "--force",
        ] {
            assert!(
                parse(&format!("--manifest m --queries q --output o {suffix}")).is_err(),
                "{suffix}"
            );
        }
    }

    #[test]
    fn bounded_refinement_allowance_is_explicit_and_zero_disables_it() {
        for cells in [0, 10, 64] {
            let Command::OwnerDomainMatch(args) = parse(&format!(
                "--manifest m --queries q --output o --max-bounded-refinement-cells-per-query {cells}"
            )).unwrap() else { panic!("match command") };
            assert_eq!(args.max_bounded_refinement_cells, cells);
        }
    }

    #[test]
    fn bounded_refinement_axes_policy_is_independent_of_faces_and_walk_mode() {
        use rustred::solver::OwnerDomainRefinementAxes;
        for (name, axes) in [
            ("inactive-only", OwnerDomainRefinementAxes::InactiveOnly),
            ("finite-axes", OwnerDomainRefinementAxes::FiniteAxes),
        ] {
            for mode in ["", "--follow-successors"] {
                for cells in [0, 17] {
                    let Command::OwnerDomainMatch(args) = parse(&format!(
                        "--manifest m --queries q --output o {mode} --bounded-refinement-axes {name} --max-bounded-refinement-cells-per-query {cells}"
                    )).unwrap() else { panic!("match command") };
                    assert_eq!(args.refinement_axes, axes);
                    assert_eq!(args.max_bounded_refinement_cells, cells);
                    assert!(!args.route_domain_overcover);
                }
            }
        }
    }

    #[test]
    fn guard_degree_budget_is_not_a_numerator_rank_override() {
        let Command::OwnerDomainMatch(args) =
            parse("--manifest m --queries q --output o --max-guard-univariate-degree 64").unwrap()
        else {
            panic!("match command")
        };
        assert_eq!(args.max_guard_univariate_degree, 64);
        assert_eq!(args.max_bounded_refinement_cells, 0);
        assert!(!args.follow_successors);
    }

    #[test]
    fn symbolic_successor_walk_is_explicit_and_bounded() {
        let Command::OwnerDomainMatch(args) = parse("--manifest m --queries q --output o --follow-successors --max-domains 7 --max-successor-events 31 --max-containment-checks 90").unwrap() else { panic!("match command") };
        assert!(args.follow_successors);
        assert_eq!(
            (
                args.max_domains,
                args.max_successor_events,
                args.max_containment_checks
            ),
            (7, 31, Some(90))
        );
        for suffix in [
            "--max-domains 7",
            "--follow-successors --max-successor-events 0",
            "--follow-successors --max-domains 0",
            "--follow-successors --follow-successors",
        ] {
            assert!(parse(&format!("--manifest m --queries q --output o {suffix}")).is_err());
        }
    }

    #[test]
    fn explicit_domain_budget_accepts_more_than_one_million() {
        for limit in [1_000_001, 10_000_000, usize::MAX] {
            let Command::OwnerDomainMatch(args) = parse(&format!(
                "--manifest m --queries q --output o --follow-successors --max-domains {limit}"
            ))
            .unwrap() else {
                panic!("match command")
            };
            assert_eq!(args.max_domains, limit);
        }
    }

    #[test]
    fn containment_policy_is_unlimited_by_default_or_explicit_literal() {
        for suffix in ["", "--max-containment-checks unlimited"] {
            let Command::OwnerDomainMatch(args) = parse(&format!(
                "--manifest m --queries q --output o --follow-successors {suffix}"
            ))
            .unwrap() else {
                panic!("match command")
            };
            assert_eq!(args.max_containment_checks, None);
        }
        for suffix in [
            "--max-containment-checks unlimited",
            "--follow-successors --max-containment-checks 0",
            "--follow-successors --max-containment-checks Unlimited",
            "--follow-successors --max-containment-checks none",
            "--follow-successors --max-containment-checks unlimited --max-containment-checks 7",
        ] {
            assert!(parse(&format!("--manifest m --queries q --output o {suffix}")).is_err());
        }
    }

    #[test]
    fn unreserved_transfer_is_opt_in_positive_and_requires_unlimited_walk() {
        for horizon in [1, 50, usize::MAX] {
            for cap in ["", "--max-containment-checks unlimited"] {
                let Command::OwnerDomainMatch(args) = parse(&format!(
                    "--manifest m --queries q --output o --follow-successors --transfer-unreserved-lookahead {horizon} {cap}"
                )).unwrap() else { panic!("match command") };
                assert_eq!(args.transfer_unreserved_lookahead.unwrap().get(), horizon);
            }
        }
        for suffix in [
            "--transfer-unreserved-lookahead 50",
            "--follow-successors --transfer-unreserved-lookahead 0",
            "--follow-successors --transfer-unreserved-lookahead +1",
            "--follow-successors --transfer-unreserved-lookahead 50 --max-containment-checks 100",
            "--follow-successors --max-containment-checks 100 --transfer-unreserved-lookahead 50",
            "--follow-successors --transfer-unreserved-lookahead 1 --transfer-unreserved-lookahead 2",
        ] {
            assert!(
                parse(&format!("--manifest m --queries q --output o {suffix}")).is_err(),
                "{suffix}"
            );
        }
    }

    #[test]
    fn initial_d_band_reuse_requires_an_explicit_unlimited_delegating_walk() {
        for cap in ["", "--max-containment-checks unlimited"] {
            let Command::OwnerDomainMatch(args) = parse(&format!(
                "--manifest m --queries q --output o --follow-successors --transfer-unreserved-lookahead 50 --reuse-initial-d-bands {cap}"
            )).unwrap() else { panic!("match command") };
            assert!(args.reuse_initial_d_bands);
            assert_eq!(args.transfer_unreserved_lookahead.unwrap().get(), 50);
            assert_eq!(args.max_containment_checks, None);
        }
        for suffix in [
            "--reuse-initial-d-bands",
            "--follow-successors --reuse-initial-d-bands",
            "--transfer-unreserved-lookahead 50 --reuse-initial-d-bands",
            "--follow-successors --transfer-unreserved-lookahead 50 --reuse-initial-d-bands --max-containment-checks 10",
            "--follow-successors --transfer-unreserved-lookahead 50 --reuse-initial-d-bands --reuse-initial-d-bands",
            "--follow-successors --transfer-unreserved-lookahead 50 --reuse-initial-d-bands false",
        ] {
            assert!(
                parse(&format!("--manifest m --queries q --output o {suffix}")).is_err(),
                "{suffix}"
            );
        }
    }

    #[test]
    fn route_overcover_and_rhs_work_budgets_are_explicit() {
        let Command::OwnerDomainMatch(args) = parse("--manifest m --queries q --output o --follow-successors --route-domain-overcover --max-route-masks-per-query 17 --max-rhs-cells-per-query 101 --max-term-visits-per-query 202 --max-native-operations-per-query 303").unwrap() else { panic!("match command") };
        assert!(args.route_domain_overcover);
        assert_eq!(
            (
                args.max_route_masks,
                args.max_rhs_cells,
                args.max_term_visits,
                args.max_native_operations
            ),
            (17, 101, 202, 303)
        );
        for suffix in [
            "--route-domain-overcover",
            "--max-rhs-cells-per-query 2",
            "--follow-successors --max-route-masks-per-query 3",
            "--follow-successors --route-domain-overcover --max-route-masks-per-query 0",
            "--follow-successors --route-domain-overcover --route-domain-overcover",
        ] {
            assert!(parse(&format!("--manifest m --queries q --output o {suffix}")).is_err());
        }
    }

    #[test]
    fn parallel_walk_and_native_event_allowances_are_distinct_from_aggregate_work() {
        let Command::OwnerDomainMatch(args) = parse("--manifest m --queries q --output o --follow-successors --workers 6 --max-successor-events 701 --max-rhs-events-per-query 303 --max-shift-groups-per-query 202 --max-sign-splits-per-query 101").unwrap() else { panic!("match command") };
        assert_eq!(args.workers, 6);
        assert_eq!(args.max_successor_events, 701);
        assert_eq!(args.max_rhs_events, 303);
        assert_eq!(args.max_shift_groups, 202);
        assert_eq!(args.max_sign_splits, 101);
        for suffix in [
            "--max-rhs-events-per-query 10",
            "--max-shift-groups-per-query 10",
            "--max-sign-splits-per-query 10",
            "--workers 1",
            "--follow-successors --workers 0",
            "--follow-successors --workers 257",
            "--follow-successors --max-rhs-events-per-query 0",
            "--follow-successors --max-shift-groups-per-query 0",
            "--follow-successors --max-sign-splits-per-query 0",
            "--follow-successors --workers 1 --workers 2",
            "--follow-successors --max-rhs-events-per-query 1 --max-rhs-events-per-query 2",
        ] {
            assert!(
                parse(&format!("--manifest m --queries q --output o {suffix}")).is_err(),
                "{suffix}"
            );
        }
    }

    #[test]
    fn streamed_event_allowance_is_independent_of_retained_frontier_memory() {
        let Command::OwnerDomainMatch(args) = parse("--manifest m --queries q --output o --follow-successors --max-successor-events 100000000 --max-frontiers 17").unwrap() else { panic!("match command") };
        assert_eq!(args.max_successor_events, 100_000_000);
        assert_eq!(args.max_frontiers, 17);
        for suffix in [
            "--max-frontiers 17",
            "--follow-successors --max-frontiers 0",
            "--follow-successors --max-frontiers 1000001",
        ] {
            assert!(parse(&format!("--manifest m --queries q --output o {suffix}")).is_err());
        }
    }
}
