use super::*;

fn args(extra: &str) -> Result<OwnerDomainMatchArgs, ArgError> {
    let text = format!("--manifest m --queries q --output o {extra}");
    match parse(text.split_whitespace().map(OsString::from))? {
        Command::OwnerDomainMatch(args) => Ok(args),
        _ => panic!("owner-domain-match"),
    }
}

#[test]
fn reduction_aggregate_defaults_and_explicit_zero_are_independent_of_finite_policy() {
    let a = args("").unwrap();
    let d = rustred::reduction::ReductionLimits::default();
    assert_eq!(a.reduction_max_rule_applications, d.max_rule_applications);
    assert_eq!(a.reduction_max_pending_frames, d.max_pending_frames);
    assert_eq!(
        a.reduction_max_coalescing_additions,
        d.max_coalescing_additions
    );
    for (option, read) in [
        (
            "--reduction-max-rule-applications",
            (|a: &OwnerDomainMatchArgs| a.reduction_max_rule_applications)
                as fn(&OwnerDomainMatchArgs) -> usize,
        ),
        (
            "--reduction-max-pending-frames",
            (|a: &OwnerDomainMatchArgs| a.reduction_max_pending_frames)
                as fn(&OwnerDomainMatchArgs) -> usize,
        ),
        (
            "--reduction-max-coalescing-additions",
            (|a: &OwnerDomainMatchArgs| a.reduction_max_coalescing_additions)
                as fn(&OwnerDomainMatchArgs) -> usize,
        ),
    ] {
        assert_eq!(read(&args(&format!("{option} 0")).unwrap()), 0);
        for tail in ["-1", "unlimited"] {
            assert!(args(&format!("{option} {tail}")).is_err());
        }
        assert!(args(&format!("{option} 1 {option} 2")).is_err());
    }
}

#[test]
fn reduction_aggregate_explicit_h1_limits_survive_unbounded_work() {
    let a = args("--follow-successors --publication-policy epoch --transfer-unreserved-lookahead 16 --unbounded-work --reduction-max-rule-applications 16000000 --reduction-max-pending-frames 16000000 --reduction-max-coalescing-additions 256000000").unwrap();
    assert_eq!(a.reduction_max_rule_applications, 16_000_000);
    assert_eq!(a.reduction_max_pending_frames, 16_000_000);
    assert_eq!(a.reduction_max_coalescing_additions, 256_000_000);
    assert!(a.finite_replay.is_none());
}
