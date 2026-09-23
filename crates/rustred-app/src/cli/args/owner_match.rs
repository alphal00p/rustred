use super::{
    ArgError, Command, next_utf8_value, parse_nonnegative_integer, parse_positive_integer,
};
use std::{collections::BTreeSet, ffi::OsString, num::NonZeroUsize, path::PathBuf};

#[cfg(test)]
mod publication_tests;
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
    pub workers: usize,
    pub inspection_workers: Option<usize>,
    pub publication_policy: crate::OwnerDomainWalkPublicationPolicy,
    pub route_domain_overcover: bool,
    pub max_route_masks: usize,
    pub max_rhs_cells: usize,
    pub max_term_visits: usize,
    pub max_native_operations: usize,
    pub max_rhs_events: usize,
    pub max_shift_groups: usize,
    pub max_sign_splits: usize,
    pub max_domains: usize,
    pub max_frontiers: usize,
    pub max_successor_events: usize,
    pub max_containment_checks: Option<usize>,
    pub transfer_unreserved_lookahead: Option<NonZeroUsize>,
    pub reuse_initial_d_bands: bool,
}

pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let limits = rustred::solver::OwnerDomainMatchLimits::default();
    let applied = rustred::solver::OwnerAppliedLimits::default();
    let mut result = OwnerDomainMatchArgs {
        manifest: PathBuf::new(),
        queries: PathBuf::new(),
        output: PathBuf::new(),
        owner_base: PathBuf::from("."),
        events: None,
        stop_file: None,
        max_queries: 256,
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
        workers: 1,
        inspection_workers: None,
        publication_policy: crate::OwnerDomainWalkPublicationPolicy::Ordered,
        route_domain_overcover: false,
        max_route_masks: 100_000,
        max_rhs_cells: applied.max_boundary_cells,
        max_term_visits: applied.max_term_visits,
        max_native_operations: applied.max_native_operations,
        max_rhs_events: applied.max_events,
        max_shift_groups: applied.max_shift_groups,
        max_sign_splits: applied.max_sign_splits,
        max_domains: 100_000,
        max_frontiers: 100_000,
        max_successor_events: 1_000_000,
        max_containment_checks: None,
        transfer_unreserved_lookahead: None,
        reuse_initial_d_bands: false,
    };
    let mut seen = BTreeSet::new();
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let name = match option.as_str() {
            "--manifest" => "--manifest",
            "--queries" => "--queries",
            "--output" => "--output",
            "--owner-base" => "--owner-base",
            "--events" => "--events",
            "--stop-file" => "--stop-file",
            "--max-queries" => "--max-queries",
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
            "--workers" => "--workers",
            "--publication-policy" => "--publication-policy",
            "--inspection-workers" => "--inspection-workers",
            "--route-domain-overcover" => "--route-domain-overcover",
            "--max-route-masks-per-query" => "--max-route-masks-per-query",
            "--max-rhs-cells-per-query" => "--max-rhs-cells-per-query",
            "--max-term-visits-per-query" => "--max-term-visits-per-query",
            "--max-native-operations-per-query" => "--max-native-operations-per-query",
            "--max-rhs-events-per-query" => "--max-rhs-events-per-query",
            "--max-shift-groups-per-query" => "--max-shift-groups-per-query",
            "--max-sign-splits-per-query" => "--max-sign-splits-per-query",
            "--max-domains" => "--max-domains",
            "--max-frontiers" => "--max-frontiers",
            "--max-successor-events" => "--max-successor-events",
            "--max-containment-checks" => "--max-containment-checks",
            "--transfer-unreserved-lookahead" => "--transfer-unreserved-lookahead",
            "--reuse-initial-d-bands" => "--reuse-initial-d-bands",
            "--help" | "-h" => return Ok(Command::Help),
            _ => return Err(ArgError::UnknownOption(option)),
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
        if name == "--route-domain-overcover" {
            result.route_domain_overcover = true;
            continue;
        }
        if name == "--reuse-initial-d-bands" {
            result.reuse_initial_d_bands = true;
            continue;
        }
        let value = next_utf8_value(&mut arguments, name)?;
        match name {
            "--inspection-workers" => {
                result.inspection_workers = Some(parse_positive_integer(name, value)?);
            }
            "--publication-policy" => {
                result.publication_policy = match value.as_str() {
                    "ordered" => crate::OwnerDomainWalkPublicationPolicy::Ordered,
                    "owner-batched" => crate::OwnerDomainWalkPublicationPolicy::OwnerBatched,
                    _ => {
                        return Err(ArgError::InvalidValue {
                            option: name,
                            value,
                            expected: "ordered or owner-batched",
                        });
                    }
                };
            }
            "--transfer-unreserved-lookahead" => {
                result.transfer_unreserved_lookahead =
                    NonZeroUsize::new(parse_positive_integer(name, value)?);
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
            "--manifest" | "--queries" | "--output" | "--owner-base" | "--events"
            | "--stop-file" => {
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
                    _ => result.stop_file = Some(path),
                }
            }
            _ => {
                let value = parse_positive_integer(name, value)?;
                match name {
                    "--workers" => result.workers = value,
                    "--max-queries" => result.max_queries = value,
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
    if result.max_queries > 10_000 || result.max_total_pieces > 1_000_000 {
        return Err(ArgError::InvalidCombination(
            "at most 10000 queries /1000000 retained pieces",
        ));
    }
    if result.max_frontiers > 1_000_000 {
        return Err(ArgError::InvalidCombination(
            "at most 1000000 retained frontiers",
        ));
    }
    if result.workers > 64 {
        return Err(ArgError::InvalidCombination("at most 64 symbolic workers"));
    }
    if !result.follow_successors
        && [
            "--workers",
            "--inspection-workers",
            "--publication-policy",
            "--max-domains",
            "--max-frontiers",
            "--max-successor-events",
            "--max-containment-checks",
            "--transfer-unreserved-lookahead",
            "--reuse-initial-d-bands",
            "--route-domain-overcover",
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
    Ok(Command::OwnerDomainMatch(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(text: &str) -> Result<Command, ArgError> {
        super::parse(text.split_whitespace().map(OsString::from))
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
            "--max-queries 10001",
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
            "--follow-successors --workers 65",
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
