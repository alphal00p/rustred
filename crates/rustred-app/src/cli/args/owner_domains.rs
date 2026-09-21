use super::{ArgError, Command, next_utf8_value, parse_positive_integer};
use std::{collections::BTreeSet, ffi::OsString, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OwnerDomainScanArgs {
    pub manifest: PathBuf,
    pub output: PathBuf,
    pub owner_base: PathBuf,
    pub events: Option<PathBuf>,
    pub stop_file: Option<PathBuf>,
    pub rank: u32,
    pub max_rules: usize,
    pub max_terms: usize,
    pub max_regions: usize,
    pub max_total_regions: usize,
    pub max_summary_groups: usize,
    pub no_progress: bool,
}

pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let defaults = rustred::solver::OwnerSuccessorLimits::default();
    let mut result = OwnerDomainScanArgs {
        manifest: PathBuf::new(),
        output: PathBuf::new(),
        owner_base: PathBuf::from("."),
        events: None,
        stop_file: None,
        rank: 0,
        no_progress: false,
        max_rules: defaults.max_rules,
        max_terms: defaults.max_terms,
        max_regions: defaults.max_regions,
        max_total_regions: 20_000_000,
        max_summary_groups: 16_384,
    };
    let mut seen = BTreeSet::new();
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let name = match option.as_str() {
            "--manifest" => "--manifest",
            "--output" => "--output",
            "--owner-base" => "--owner-base",
            "--events" => "--events",
            "--stop-file" => "--stop-file",
            "--max-numerator-rank" => "--max-numerator-rank",
            "--max-rules-per-owner" => "--max-rules-per-owner",
            "--max-terms-per-owner" => "--max-terms-per-owner",
            "--max-regions-per-owner" => "--max-regions-per-owner",
            "--max-total-regions" => "--max-total-regions",
            "--max-summary-groups" => "--max-summary-groups",
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
            "--manifest" | "--output" | "--owner-base" | "--events" | "--stop-file" => {
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
                    "--output" => result.output = path,
                    "--owner-base" => result.owner_base = path,
                    "--events" => result.events = Some(path),
                    _ => result.stop_file = Some(path),
                }
            }
            "--max-numerator-rank" => {
                result.rank = value.parse().map_err(|_| ArgError::InvalidValue {
                    option: name,
                    value,
                    expected: "an unsigned 32-bit integer (zero is allowed)",
                })?;
            }
            _ => {
                let number = parse_positive_integer(name, value)?;
                match name {
                    "--max-rules-per-owner" => result.max_rules = number,
                    "--max-terms-per-owner" => result.max_terms = number,
                    "--max-regions-per-owner" => result.max_regions = number,
                    "--max-total-regions" => result.max_total_regions = number,
                    _ => result.max_summary_groups = number,
                }
            }
        }
    }
    for name in ["--manifest", "--output", "--max-numerator-rank"] {
        if !seen.contains(name) {
            return Err(ArgError::MissingRequiredOption(name));
        }
    }
    if result.max_summary_groups > 1_000_000 {
        return Err(ArgError::InvalidCombination(
            "at most 1000000 retained summary groups",
        ));
    }
    Ok(Command::OwnerDomainScan(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_domain_scan_requires_explicit_scope_not_concrete_targets() {
        let parse = |s: &str| super::parse(s.split_whitespace().map(OsString::from));
        assert!(matches!(
            parse("--manifest m --output o --max-numerator-rank 0"),
            Ok(Command::OwnerDomainScan(OwnerDomainScanArgs {
                rank: 0,
                ..
            }))
        ));
        for suffix in [
            "",
            "--max-numerator-rank -1",
            "--max-numerator-rank 4294967296",
            "--max-numerator-rank 10 --targets t",
            "--max-numerator-rank 10 --workers 50",
            "--max-numerator-rank 10 --max-total-regions 0",
            "--max-numerator-rank 10 --max-summary-groups 1000001",
            "--max-numerator-rank 10 --max-numerator-rank 20",
            "--max-numerator-rank 10 --timeout 1800",
        ] {
            assert!(
                parse(&format!("--manifest m --output o {suffix}")).is_err(),
                "{suffix}"
            );
        }
    }

    #[test]
    fn owner_domain_scan_explicit_group_ceiling_increases_without_default_change() {
        let parse = |s: &str| super::parse(s.split_whitespace().map(OsString::from));
        for maximum in [100_001, 1_000_000] {
            let Command::OwnerDomainScan(args) = parse(&format!(
                "--manifest m --output o --max-numerator-rank 10 --max-summary-groups {maximum}"
            ))
            .unwrap() else {
                panic!("scan command");
            };
            assert_eq!(args.max_summary_groups, maximum);
        }
        let Command::OwnerDomainScan(args) =
            parse("--manifest m --output o --max-numerator-rank 10").unwrap()
        else {
            panic!("scan command");
        };
        assert_eq!(args.max_summary_groups, 16_384);
    }
}
