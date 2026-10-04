//! Optional finite replay has separate, always-finite work allowances.
use super::{ArgError, BTreeSet, OwnerDomainMatchArgs};
use crate::OwnerDomainWalkFiniteReplayLimits;

pub(super) const ENABLE: &str = "--finite-replay-initial-domain";
const CAPS: [&str; 9] = [
    "--finite-replay-max-nodes",
    "--finite-replay-max-rule-applications",
    "--finite-replay-max-transport-calls",
    "--finite-replay-max-transport-operations",
    "--finite-replay-max-transport-endpoints",
    "--finite-replay-max-coalescing-additions",
    "--finite-replay-max-positive-layers",
    "--finite-replay-max-seed-points",
    "--finite-replay-max-seed-bytes",
];

pub(super) fn option_name(name: &str) -> Option<&'static str> {
    std::iter::once(ENABLE)
        .chain(CAPS)
        .find(|&known| known == name)
}

pub(super) fn defaults() -> OwnerDomainWalkFiniteReplayLimits {
    OwnerDomainWalkFiniteReplayLimits {
        max_nodes: 1_000_000,
        max_rule_applications: 1_000_000,
        max_transport_calls: 1_000_000,
        max_transport_operations: 64_000_000,
        max_transport_endpoints: 4_000_000,
        max_coalescing_additions: 16_000_000,
        max_positive_layers: 64,
        max_seed_points: 1024,
        max_seed_bytes: 1024 * 1024,
    }
}

pub(super) fn set_cap(limits: &mut OwnerDomainWalkFiniteReplayLimits, name: &str, cap: usize) {
    match name {
        "--finite-replay-max-nodes" => limits.max_nodes = cap,
        "--finite-replay-max-rule-applications" => limits.max_rule_applications = cap,
        "--finite-replay-max-transport-calls" => limits.max_transport_calls = cap,
        "--finite-replay-max-transport-operations" => limits.max_transport_operations = cap,
        "--finite-replay-max-transport-endpoints" => limits.max_transport_endpoints = cap,
        "--finite-replay-max-coalescing-additions" => limits.max_coalescing_additions = cap,
        "--finite-replay-max-positive-layers" => limits.max_positive_layers = cap,
        "--finite-replay-max-seed-points" => limits.max_seed_points = cap,
        "--finite-replay-max-seed-bytes" => limits.max_seed_bytes = cap,
        _ => unreachable!("finite replay option was already classified as a cap"),
    }
}

pub(super) fn validate(args: &OwnerDomainMatchArgs, seen: &BTreeSet<&str>) -> Result<(), ArgError> {
    if args.finite_replay_budget_preflight
        && (args.finite_replay.is_none() || args.events.is_some() || args.stop_file.is_some())
    {
        return Err(ArgError::InvalidCombination(
            "finite replay budget preflight requires finite replay and no event/stop paths",
        ));
    }
    if args.finite_replay.is_none() {
        return Ok(());
    }
    if !seen.contains(ENABLE) {
        return Err(ArgError::InvalidCombination(
            "finite replay allowances require --finite-replay-initial-domain",
        ));
    }
    if !args.follow_successors
        || args.publication_policy != crate::OwnerDomainWalkPublicationPolicy::Epoch
        || !args.checkpoint.as_ref().is_some_and(|cp| !cp.resume)
        || !args.amend_queries.is_empty()
        || args.g2_activate_on_resume
    {
        return Err(ArgError::InvalidCombination(
            "finite replay requires --follow-successors, epoch publication and a fresh --checkpoint, without resume or amendments",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{Command, parse};
    use super::*;
    use std::ffi::OsString;

    const BASE: &str = "--manifest m --queries q --output o --follow-successors --publication-policy epoch --transfer-unreserved-lookahead 16 --checkpoint cp";

    fn args(text: &str) -> Result<OwnerDomainMatchArgs, ArgError> {
        match parse(text.split_whitespace().map(OsString::from))? {
            Command::OwnerDomainMatch(args) => Ok(args),
            _ => panic!("expected owner-domain-match"),
        }
    }

    #[test]
    fn finite_replay_is_default_off_and_explicit_caps_are_order_independent() {
        assert!(args(BASE).unwrap().finite_replay.is_none());
        let enabled = args(&format!("{BASE} {ENABLE}")).unwrap();
        assert_eq!(enabled.finite_replay, Some(defaults()));
        for text in [
            format!("{BASE} --finite-replay-max-nodes 7 {ENABLE}"),
            format!("{BASE} {ENABLE} --finite-replay-max-nodes 7"),
        ] {
            let mut expected = defaults();
            expected.max_nodes = 7;
            assert_eq!(args(&text).unwrap().finite_replay, Some(expected));
        }
    }

    #[test]
    fn finite_replay_requires_fresh_cp6_and_explicit_enable() {
        for text in [
            format!("{BASE} --finite-replay-max-nodes 9"),
            format!(
                "{} {ENABLE}",
                BASE.replace("--checkpoint cp", "--resume cp")
            ),
            format!("{} {ENABLE}", BASE.replace("--checkpoint cp", "")),
            format!(
                "{} {ENABLE}",
                BASE.replace("--publication-policy epoch", "--publication-policy ready")
            ),
            format!("{BASE} {ENABLE} --amend-queries repair.json"),
            format!("--manifest m --queries q --output o {ENABLE}"),
            format!("{BASE} {ENABLE} {ENABLE}"),
            format!("{BASE} --finite-replay-initial-singleton"),
        ] {
            assert!(args(&text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn finite_replay_caps_accept_zero_but_not_negative_duplicate_or_unlimited() {
        for cap in CAPS {
            assert!(args(&format!("{BASE} {ENABLE} {cap} 0")).is_ok());
            for suffix in [
                format!("{cap} -1"),
                format!("{cap} unlimited"),
                format!("{cap} 1 {cap} 2"),
            ] {
                assert!(args(&format!("{BASE} {ENABLE} {suffix}")).is_err());
            }
        }
        let bounded = args(&format!("{BASE} {ENABLE} --unbounded-work")).unwrap();
        assert_eq!(bounded.finite_replay, Some(defaults()));
    }

    #[test]
    fn finite_replay_budget_preflight_is_explicit_diagnostic_only() {
        assert!(
            args(&format!("{BASE} {ENABLE} --finite-replay-budget-preflight"))
                .unwrap()
                .finite_replay_budget_preflight
        );
        for suffix in ["", "--events e", "--stop-file stop"] {
            let enable = if suffix.is_empty() { "" } else { ENABLE };
            assert!(
                args(&format!(
                    "{BASE} {enable} --finite-replay-budget-preflight {suffix}"
                ))
                .is_err()
            );
        }
    }
}
