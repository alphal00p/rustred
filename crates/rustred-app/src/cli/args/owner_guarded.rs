use super::{ArgError, Command, next_utf8_value, parse_positive_integer};
use std::{collections::BTreeSet, ffi::OsString, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OwnerGuardedApplyArgs {
    pub manifest: PathBuf,
    pub queries: PathBuf,
    pub output: PathBuf,
    pub owner_base: PathBuf,
    pub work_limits: Option<PathBuf>,
    pub events: Option<PathBuf>,
    pub stop_file: Option<PathBuf>,
    pub max_queries: usize,
    pub max_report_events: usize,
    pub max_report_bytes: usize,
    pub max_expression_bytes: usize,
    pub no_progress: bool,
}
pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let defaults = crate::OwnerGuardedApplyRequest::new(String::new(), String::new());
    let mut a = OwnerGuardedApplyArgs {
        manifest: PathBuf::new(),
        queries: PathBuf::new(),
        output: PathBuf::new(),
        owner_base: PathBuf::from("."),
        work_limits: None,
        events: None,
        stop_file: None,
        max_queries: defaults.max_queries,
        max_report_events: defaults.max_report_events,
        max_report_bytes: defaults.max_report_bytes,
        max_expression_bytes: defaults.max_expression_bytes,
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
            "--work-limits" => "--work-limits",
            "--events" => "--events",
            "--stop-file" => "--stop-file",
            "--max-queries" => "--max-queries",
            "--max-report-events" => "--max-report-events",
            "--max-report-bytes" => "--max-report-bytes",
            "--max-expression-bytes" => "--max-expression-bytes",
            "--no-progress" => "--no-progress",
            "--help" | "-h" => return Ok(Command::Help),
            _ => return Err(ArgError::UnknownOption(option)),
        };
        if !seen.insert(name) {
            return Err(ArgError::DuplicateOption(name));
        }
        if name == "--no-progress" {
            a.no_progress = true;
            continue;
        }
        let value = next_utf8_value(&mut arguments, name)?;
        if matches!(
            name,
            "--manifest"
                | "--queries"
                | "--output"
                | "--owner-base"
                | "--work-limits"
                | "--events"
                | "--stop-file"
        ) {
            if value.is_empty() || value == "-" {
                return Err(ArgError::InvalidValue {
                    option: name,
                    value,
                    expected: "a filesystem path",
                });
            }
            let path = PathBuf::from(value);
            match name {
                "--manifest" => a.manifest = path,
                "--queries" => a.queries = path,
                "--output" => a.output = path,
                "--owner-base" => a.owner_base = path,
                "--work-limits" => a.work_limits = Some(path),
                "--events" => a.events = Some(path),
                _ => a.stop_file = Some(path),
            }
        } else {
            let n = parse_positive_integer(name, value)?;
            match name {
                "--max-queries" => a.max_queries = n,
                "--max-report-events" => a.max_report_events = n,
                "--max-report-bytes" => a.max_report_bytes = n,
                _ => a.max_expression_bytes = n,
            }
        }
    }
    for name in ["--manifest", "--queries", "--output"] {
        if !seen.contains(name) {
            return Err(ArgError::MissingRequiredOption(name));
        }
    }
    if a.max_queries > 10_000
        || a.max_report_events > 1_000_000
        || !(131_072..=268_435_456).contains(&a.max_report_bytes)
        || a.max_expression_bytes > 16_777_216
    {
        return Err(ArgError::InvalidCombination(
            "guarded diagnostic report allowances exceed supported bounds",
        ));
    }
    Ok(Command::OwnerGuardedApply(a))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guarded_cli_is_explicit_and_rejects_unrelated_modes() {
        let parse = |s: &str| super::parse(s.split_whitespace().map(OsString::from));
        assert!(matches!(
            parse("--manifest m --queries q --output o --work-limits l"),
            Ok(Command::OwnerGuardedApply(_))
        ));
        for suffix in [
            "--workers 6",
            "--follow-successors",
            "--timeout 60",
            "--guards x",
            "--max-queries 0",
            "--max-report-bytes 65535",
            "--max-report-events 1000001",
            "--queries second",
        ] {
            assert!(
                parse(&format!("--manifest m --queries q --output o {suffix}")).is_err(),
                "{suffix}"
            );
        }
        assert!(parse("--manifest m --output o").is_err());
    }
}
