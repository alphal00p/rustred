//! Parser-only partition controls; no preparation or native computation.
use super::*;

fn parse_suffix(suffix: &str) -> Result<OwnerDomainMatchArgs, ArgError> {
    let Command::OwnerDomainMatch(args) = super::parse(
        format!("--manifest m --queries q --output o {suffix}")
            .split_whitespace()
            .map(OsString::from),
    )?
    else {
        panic!("owner match")
    };
    Ok(args)
}

#[test]
fn inspection_partition_is_additive_and_order_independent() {
    for workers in [1usize, 2, 4, 6, 50] {
        for policy in ["ordered", "owner-batched"] {
            let common =
                format!("--follow-successors --workers {workers} --publication-policy {policy}");
            let default = parse_suffix(&common).unwrap();
            assert_eq!(default.inspection_workers, None);
            for inspectors in 1..=workers.saturating_sub(1).max(1) {
                let mut expected = default.clone();
                expected.inspection_workers = Some(inspectors);
                let option = format!("--inspection-workers {inspectors}");
                assert_eq!(
                    parse_suffix(&format!("{common} {option}")).unwrap(),
                    expected
                );
                assert_eq!(
                    parse_suffix(&format!("{option} {common}")).unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn inspection_partition_rejects_invalid_scope_counts_and_duplicates() {
    for suffix in [
        "--inspection-workers 1",
        "--follow-successors --inspection-workers 2",
        "--follow-successors --workers 2 --inspection-workers 2",
        "--follow-successors --workers 50 --inspection-workers 50",
        "--follow-successors --workers 50 --inspection-workers 0",
        "--follow-successors --workers 50 --inspection-workers -1",
        "--follow-successors --workers 50 --inspection-workers 40 --max-containment-checks 7",
    ] {
        assert!(parse_suffix(suffix).is_err(), "{suffix}");
    }
    assert_eq!(
        parse_suffix("--follow-successors --inspection-workers").unwrap_err(),
        ArgError::MissingValue("--inspection-workers")
    );
    assert_eq!(
        parse_suffix("--follow-successors --inspection-workers 1 --inspection-workers 1")
            .unwrap_err(),
        ArgError::DuplicateOption("--inspection-workers")
    );
    for (workers, inspectors) in [(1, 1), (2, 1), (6, 5), (50, 49)] {
        assert!(parse_suffix(&format!("--follow-successors --workers {workers} --inspection-workers {inspectors} --max-containment-checks 7")).is_ok());
    }
}

#[test]
fn epoch_preparation_options_are_explicit_bounded_and_partitioned() {
    let prefix = "--follow-successors --publication-policy epoch --transfer-unreserved-lookahead 256 --workers 50";
    let defaults = parse_suffix(prefix).unwrap();
    assert_eq!(defaults.epoch_preparation_workers, None);
    assert_eq!(defaults.epoch_preparation_max_obligations, None);
    for helpers in [0, 1, 2, 7, 48] {
        let flags = format!(
            "--epoch-preparation-workers {helpers} --epoch-preparation-max-obligations 123 --epoch-preparation-max-retirements 321"
        );
        let parsed = parse_suffix(&format!("{prefix} {flags}")).unwrap();
        assert_eq!(parsed.epoch_preparation_workers, Some(helpers));
        assert_eq!(parsed.epoch_preparation_max_obligations, Some(123));
        assert_eq!(parsed.epoch_preparation_max_retirements, Some(321));
        assert_eq!(parse_suffix(&format!("{flags} {prefix}")).unwrap(), parsed);
        assert!(
            parse_suffix(&format!(
                "{prefix} {flags} --inspection-workers {}",
                49 - helpers
            ))
            .is_ok()
        );
    }
    for flags in [
        "--epoch-preparation-workers 49",
        "--epoch-preparation-workers -1",
        "--epoch-preparation-workers 7 --inspection-workers 49",
        "--epoch-preparation-max-obligations 0",
        "--epoch-preparation-max-retirements 4294967296",
        "--epoch-preparation-workers 0 --epoch-preparation-workers 0",
        "--epoch-preparation-max-obligations 1 --epoch-preparation-max-obligations 2",
    ] {
        assert!(
            parse_suffix(&format!("{prefix} {flags}")).is_err(),
            "{flags}"
        );
    }
    for flags in [
        "--epoch-preparation-workers 0",
        "--epoch-preparation-max-obligations 12",
        "--epoch-preparation-max-retirements 12",
    ] {
        assert!(parse_suffix(&format!("--follow-successors --workers 50 {flags}")).is_err());
    }
    assert!(parse_suffix("--follow-successors --publication-policy epoch --transfer-unreserved-lookahead 256 --workers 1 --epoch-preparation-workers 0").is_ok());
}
