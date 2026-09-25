//! Launch policy is explicit and cannot silently override diagnostic budgets.
use super::*;

fn parse_suffix(suffix: &str) -> Result<OwnerDomainMatchArgs, ArgError> {
    let Command::OwnerDomainMatch(args) = super::parse(
        format!("--manifest m --queries q --output o {suffix}")
            .split_whitespace()
            .map(OsString::from),
    )?
    else {
        panic!("owner-domain command")
    };
    Ok(args)
}

#[test]
fn joint_source_support_is_off_by_default_and_requires_route_walk() {
    assert!(!parse_suffix("").unwrap().route_joint_source_support_pruning);
    let args = parse_suffix("--follow-successors --route-domain-overcover --route-joint-source-support-pruning --unbounded-work").unwrap();
    assert!(args.route_joint_source_support_pruning);
    for suffix in [
        "--route-joint-source-support-pruning",
        "--follow-successors --route-joint-source-support-pruning",
        "--follow-successors --route-domain-overcover --route-joint-source-support-pruning --route-joint-source-support-pruning",
    ] {
        assert!(parse_suffix(suffix).is_err(), "{suffix}");
    }
}

#[test]
fn application_cell_refinement_is_positive_opt_in_and_not_a_work_cap() {
    assert!(
        parse_suffix("")
            .unwrap()
            .apply_cell_refinement_max_cardinality
            .is_none()
    );
    for suffix in [
        "--follow-successors --apply-cell-refinement-max-cardinality 2",
        "--apply-cell-refinement-max-cardinality 2 --follow-successors --unbounded-work",
        "--follow-successors --apply-cell-refinement-max-cardinality 2 --publication-policy ready --transfer-unreserved-lookahead 3",
        "--follow-successors --apply-cell-refinement-max-cardinality 2 --publication-policy owner-batched",
    ] {
        assert_eq!(
            parse_suffix(suffix)
                .unwrap()
                .apply_cell_refinement_max_cardinality
                .unwrap()
                .get(),
            2
        );
    }
    for suffix in [
        "--apply-cell-refinement-max-cardinality 2",
        "--follow-successors --apply-cell-refinement-max-cardinality 0",
        "--follow-successors --apply-cell-refinement-max-cardinality -1",
        "--follow-successors --apply-cell-refinement-max-cardinality 2.5",
        "--follow-successors --apply-cell-refinement-max-cardinality 18446744073709551616",
        "--follow-successors --apply-cell-refinement-max-cardinality 2 --apply-cell-refinement-max-cardinality 3",
    ] {
        assert!(parse_suffix(suffix).is_err(), "{suffix}");
    }
}

#[test]
fn checkpoint_flags_are_explicit_and_order_independent() {
    assert!(parse_suffix("").unwrap().checkpoint.is_none());
    for flag in ["--checkpoint", "--resume"] {
        for text in [
            format!("--follow-successors {flag} saved --checkpoint-interval-seconds 61"),
            format!("--checkpoint-interval-seconds 61 {flag} saved --follow-successors"),
        ] {
            let checkpoint = parse_suffix(&text).unwrap().checkpoint.unwrap();
            assert_eq!(checkpoint.directory, PathBuf::from("saved"));
            assert_eq!(checkpoint.resume, flag == "--resume");
            assert_eq!(checkpoint.interval_seconds, 61);
        }
    }
    assert_eq!(
        parse_suffix("--follow-successors --checkpoint saved")
            .unwrap()
            .checkpoint
            .unwrap()
            .interval_seconds,
        3600
    );
}

#[test]
fn checkpoint_rejects_ambiguous_or_unsupported_launches() {
    for text in [
        "--checkpoint saved",
        "--resume saved",
        "--follow-successors --checkpoint a --resume b",
        "--follow-successors --checkpoint a --checkpoint b",
        "--follow-successors --checkpoint-interval-seconds 60",
        "--follow-successors --checkpoint a --checkpoint-interval-seconds 0",
        "--follow-successors --resume a --checkpoint-interval-seconds -1",
        "--follow-successors --checkpoint a --publication-policy owner-batched",
        "--follow-successors --resume -",
    ] {
        assert!(parse_suffix(text).is_err(), "{text}");
    }
}

#[test]
fn unbounded_work_is_opt_in_and_composes_with_memory_independent_controls() {
    assert!(!parse_suffix("--follow-successors").unwrap().unbounded_work);
    let args = parse_suffix(
        "--follow-successors --unbounded-work --workers 50 \
        --transfer-unreserved-lookahead 256 --reuse-initial-d-bands \
        --max-query-bytes 32000000 --max-queries 70000 \
        --bounded-refinement-axes finite-axes --max-guard-univariate-degree 64 \
        --checkpoint saved",
    )
    .unwrap();
    assert!(args.unbounded_work);
    assert_eq!(args.max_query_bytes, 32000000);
    assert_eq!(args.max_guard_univariate_degree, 64);
    assert!(args.checkpoint.is_some());
    assert!(parse_suffix("--unbounded-work").is_err());
    assert!(parse_suffix("--follow-successors --unbounded-work --unbounded-work").is_err());
}

#[test]
fn unbounded_work_never_silently_discards_an_explicit_work_limit() {
    for cap in [
        "--max-domains",
        "--max-frontiers",
        "--max-successor-events",
        "--max-rules-per-query",
        "--max-terminal-checks-per-query",
        "--max-predicates-per-query",
        "--max-pieces-per-query",
        "--max-cells-per-query",
        "--max-split-operations-per-query",
        "--max-coordinate-cells-per-query",
        "--max-bounded-refinement-cells-per-query",
        "--max-containment-checks",
        "--max-route-masks-per-query",
        "--max-rhs-cells-per-query",
        "--max-term-visits-per-query",
        "--max-native-operations-per-query",
        "--max-rhs-events-per-query",
        "--max-shift-groups-per-query",
        "--max-sign-splits-per-query",
    ] {
        for suffix in [
            format!("--unbounded-work {cap} 20"),
            format!("{cap} 20 --unbounded-work"),
        ] {
            assert!(
                parse_suffix(&format!("--follow-successors {suffix}")).is_err(),
                "{suffix}"
            );
        }
    }
}

#[test]
fn subdivision_is_an_explicit_ordered_pair_and_accepts_zero() {
    assert!(
        parse_suffix("--follow-successors")
            .unwrap()
            .apply_subdivision
            .is_none()
    );
    for text in [
        "--follow-successors --apply-subdivision-axis 0 --apply-subdivision-cut 0",
        "--apply-subdivision-cut 0 --apply-subdivision-axis 0 --follow-successors",
    ] {
        assert_eq!(
            parse_suffix(text).unwrap().apply_subdivision,
            Some(crate::OwnerDomainWalkApplySubdivision { axis: 0, cut: 0 })
        );
    }
    for text in [
        "--apply-subdivision-axis 0 --apply-subdivision-cut 0",
        "--follow-successors --apply-subdivision-axis 0",
        "--follow-successors --apply-subdivision-cut 0",
        "--follow-successors --apply-subdivision-axis -1 --apply-subdivision-cut 0",
        "--follow-successors --apply-subdivision-axis 0 --apply-subdivision-cut -1",
        "--follow-successors --apply-subdivision-axis 0 --apply-subdivision-cut 0 --publication-policy owner-batched",
    ] {
        assert!(parse_suffix(text).is_err(), "{text}");
    }
}
