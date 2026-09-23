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
