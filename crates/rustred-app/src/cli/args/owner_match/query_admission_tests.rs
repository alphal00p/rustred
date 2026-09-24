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
fn query_admission_defaults_and_large_explicit_caps_are_independent() {
    let default = parse_suffix("").unwrap();
    assert_eq!(
        (default.max_queries, default.max_query_bytes),
        (256, 1024 * 1024)
    );
    for mode in ["", "--follow-successors"] {
        for query_count in [10_001, 70_000, usize::MAX] {
            let args = parse_suffix(&format!(
                "{mode} --max-queries {query_count} --max-query-bytes 33554432"
            ))
            .unwrap();
            assert_eq!(
                (args.max_queries, args.max_query_bytes),
                (query_count, 32 * 1024 * 1024)
            );
            assert_eq!(args.max_domains, default.max_domains);
            assert_eq!(args.max_successor_events, default.max_successor_events);
            assert_eq!(args.max_total_pieces, default.max_total_pieces);
        }
    }
    assert_eq!(
        parse_suffix(&format!("--max-query-bytes {}", usize::MAX))
            .unwrap()
            .max_query_bytes,
        usize::MAX
    );
}

#[test]
fn query_admission_rejects_zero_overflow_missing_and_duplicate_flags() {
    for option in ["--max-queries", "--max-query-bytes"] {
        for value in ["0", "-1", "+1", "1.5", "184467440737095516160"] {
            assert!(parse_suffix(&format!("{option} {value}")).is_err());
        }
        assert!(parse_suffix(option).is_err());
        assert!(parse_suffix(&format!("{option} 1 {option} 2")).is_err());
    }
}
