//! Parser-only controls; no owner import, native solve or affinity preflight.
use super::*;
use crate::OwnerDomainWalkPublicationPolicy;

const REQUIRED: &str = "--manifest m --queries q --output o";

fn parse_suffix(suffix: &str) -> Result<OwnerDomainMatchArgs, ArgError> {
    let command = super::parse(
        format!("{REQUIRED} {suffix}")
            .split_whitespace()
            .map(OsString::from),
    )?;
    let Command::OwnerDomainMatch(args) = command else {
        panic!("expected owner-domain-match command");
    };
    Ok(args)
}

#[test]
fn publication_policy_default_is_ordered_without_enabling_successor_work() {
    for suffix in ["", "--follow-successors"] {
        let args = parse_suffix(suffix).unwrap();
        assert_eq!(
            args.publication_policy,
            OwnerDomainWalkPublicationPolicy::Ordered
        );
        assert_eq!(args.follow_successors, !suffix.is_empty());
        assert_eq!(args.workers, 1);
        assert_eq!(args.transfer_unreserved_lookahead, None);
        assert!(!args.reuse_initial_d_bands);
    }
}

#[test]
fn ready_publication_requires_explicit_responsibility_policy_and_supports_resume() {
    assert!(matches!(
        parse_suffix("--follow-successors --publication-policy ready").unwrap_err(),
        ArgError::InvalidCombination(_)
    ));
    for workers in [1, 6, 50] {
        let common =
            format!("--follow-successors --workers {workers} --transfer-unreserved-lookahead 256");
        let mut expected = parse_suffix(&common).unwrap();
        expected.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
        assert_eq!(
            parse_suffix(&format!("{common} --publication-policy ready")).unwrap(),
            expected
        );
        for checkpoint in ["--checkpoint new", "--resume existing"] {
            let args =
                parse_suffix(&format!("{common} --publication-policy ready {checkpoint}")).unwrap();
            assert_eq!(
                args.publication_policy,
                OwnerDomainWalkPublicationPolicy::Ready
            );
            assert!(args.checkpoint.is_some());
        }
    }
    for invalid in [
        "--apply-subdivision-axis 0 --apply-subdivision-cut 1",
        "--max-containment-checks 7",
    ] {
        assert!(parse_suffix(&format!(
            "--follow-successors --publication-policy ready --transfer-unreserved-lookahead 256 {invalid}"
        )).is_err());
    }
}

#[test]
fn publication_policy_choices_preserve_all_other_options_at_one_six_fifty_workers() {
    for (name, policy) in [
        ("ordered", OwnerDomainWalkPublicationPolicy::Ordered),
        (
            "owner-batched",
            OwnerDomainWalkPublicationPolicy::OwnerBatched,
        ),
    ] {
        for workers in [1, 6, 50] {
            let common = format!("--follow-successors --workers {workers}");
            let mut expected = parse_suffix(&common).unwrap();
            expected.publication_policy = policy;
            let actual = parse_suffix(&format!("{common} --publication-policy {name}")).unwrap();
            assert_eq!(actual, expected, "{name}, workers={workers}");
        }
    }
}

#[test]
fn publication_policy_rejects_unknown_spellings_and_missing_values() {
    for value in [
        "automatic",
        "OwnerBatched",
        "owner_batched",
        "unordered",
        "1",
    ] {
        let error =
            parse_suffix(&format!("--follow-successors --publication-policy {value}")).unwrap_err();
        assert!(
            matches!(
                error,
                ArgError::InvalidValue {
                    option: "--publication-policy",
                    ..
                }
            ),
            "{value}: {error}"
        );
    }
    assert_eq!(
        parse_suffix("--follow-successors --publication-policy").unwrap_err(),
        ArgError::MissingValue("--publication-policy")
    );
}

#[test]
fn publication_policy_rejects_duplicates_even_when_values_agree() {
    for first in ["ordered", "owner-batched"] {
        for second in ["ordered", "owner-batched"] {
            assert_eq!(
                parse_suffix(&format!(
                    "--follow-successors --publication-policy {first} --publication-policy {second}"
                ))
                .unwrap_err(),
                ArgError::DuplicateOption("--publication-policy")
            );
        }
    }
}

#[test]
fn publication_policy_requires_explicit_successor_mode_even_for_ordered() {
    for name in ["ordered", "owner-batched"] {
        let error = parse_suffix(&format!("--publication-policy {name}")).unwrap_err();
        assert!(matches!(error, ArgError::InvalidCombination(_)), "{error}");
        assert!(error.to_string().contains("--follow-successors"), "{error}");
        // Scope does not depend on where the enabling flag appears.
        assert!(parse_suffix(&format!("--publication-policy {name} --follow-successors")).is_ok());
    }
}

#[test]
fn publication_policy_composes_with_transfer_initial_anchors_and_native_controls() {
    for name in ["ordered", "owner-batched"] {
        let common = "--follow-successors --workers 6 --transfer-unreserved-lookahead 256 \
            --reuse-initial-d-bands --max-containment-checks unlimited --route-domain-overcover \
            --max-route-masks-per-query 31 --bounded-refinement-axes finite-axes \
            --max-bounded-refinement-cells-per-query 64 --max-successor-events 701 \
            --max-rhs-events-per-query 303";
        let mut expected = parse_suffix(common).unwrap();
        expected.publication_policy = if name == "ordered" {
            OwnerDomainWalkPublicationPolicy::Ordered
        } else {
            OwnerDomainWalkPublicationPolicy::OwnerBatched
        };
        let actual = parse_suffix(&format!("--publication-policy {name} {common}")).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.transfer_unreserved_lookahead.unwrap().get(), 256);
        assert!(actual.reuse_initial_d_bands);
        assert_eq!(actual.max_containment_checks, None);
        assert_eq!(actual.max_successor_events, 701);
        assert_eq!(actual.max_rhs_events, 303);
    }
}

#[test]
fn publication_policy_does_not_bypass_existing_scheduling_validation() {
    for name in ["ordered", "owner-batched"] {
        for invalid in [
            "--reuse-initial-d-bands",
            "--transfer-unreserved-lookahead 256 --max-containment-checks 10",
            "--transfer-unreserved-lookahead 256 --reuse-initial-d-bands --max-containment-checks 10",
            "--workers 0",
            "--workers 257",
        ] {
            assert!(
                parse_suffix(&format!(
                    "--follow-successors --publication-policy {name} {invalid}"
                ))
                .is_err(),
                "{name}: {invalid}"
            );
        }
    }
}
