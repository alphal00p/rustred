use super::{ArgError, Command, next_utf8_value, parse_positive_integer};
use std::{collections::BTreeSet, ffi::OsString, path::PathBuf};

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
    pub no_progress: bool,
}

pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let limits = rustred::solver::OwnerDomainMatchLimits::default();
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
        no_progress: false,
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
            "--no-progress" => "--no-progress",
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
        let value = next_utf8_value(&mut arguments, name)?;
        match name {
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
                    "--max-queries" => result.max_queries = value,
                    "--max-total-pieces" => result.max_total_pieces = value,
                    "--max-rules-per-query" => result.max_rules = value,
                    "--max-terminal-checks-per-query" => result.max_terminal_checks = value,
                    "--max-predicates-per-query" => result.max_predicates = value,
                    "--max-pieces-per-query" => result.max_pieces = value,
                    "--max-cells-per-query" => result.max_cells = value,
                    "--max-split-operations-per-query" => result.max_split_operations = value,
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
        assert_eq!(args.max_queries, 256);
        assert_eq!(args.max_total_pieces, 100_000);
        assert_eq!(args.max_rules, limits.max_rules);
        assert_eq!(args.max_terminal_checks, limits.max_terminal_checks);
        assert_eq!(args.max_predicates, limits.max_predicates);
        assert_eq!(args.max_pieces, limits.max_pieces);
        assert_eq!(args.max_cells, limits.max_cells);
        assert_eq!(args.max_split_operations, limits.max_split_operations);
        assert_eq!(args.max_coordinate_cells, limits.max_coordinate_cells);
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
}
