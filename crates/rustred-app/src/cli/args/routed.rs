use super::{ArgError, Command, next_utf8_value, parse_positive_integer};
use std::{collections::BTreeSet, ffi::OsString, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RoutedCampaignArgs {
    pub manifest: PathBuf,
    pub targets: PathBuf,
    pub entry_domains: Option<PathBuf>,
    pub output: PathBuf,
    pub events: Option<PathBuf>,
    pub owner_base: PathBuf,
    pub stop_file: Option<PathBuf>,
    pub expansion_limits: Option<PathBuf>,
    pub workers: usize,
    pub nodes: usize,
    pub input_targets: usize,
    pub transport_operations: usize,
    pub transport_endpoints: usize,
    pub coalescing_additions: usize,
    pub rule_applications: usize,
    pub no_progress: bool,
}

pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut result = RoutedCampaignArgs {
        manifest: PathBuf::new(),
        targets: PathBuf::new(),
        entry_domains: None,
        output: PathBuf::new(),
        events: None,
        owner_base: PathBuf::from("."),
        stop_file: None,
        expansion_limits: None,
        workers: 1,
        nodes: 1_000_000,
        input_targets: 100_000,
        transport_operations: 64_000_000,
        transport_endpoints: 4_000_000,
        coalescing_additions: 16_000_000,
        rule_applications: 1_000_000,
        no_progress: false,
    };
    let mut seen = BTreeSet::new();
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        let name = match option.as_str() {
            "--manifest" => "--manifest",
            "--targets" => "--targets",
            "--entry-domains" => "--entry-domains",
            "--output" => "--output",
            "--events" => "--events",
            "--owner-base" => "--owner-base",
            "--stop-file" => "--stop-file",
            "--expansion-limits" => "--expansion-limits",
            "--workers" => "--workers",
            "--max-nodes" => "--max-nodes",
            "--max-input-targets" => "--max-input-targets",
            "--max-transport-operations" => "--max-transport-operations",
            "--max-transport-endpoints" => "--max-transport-endpoints",
            "--max-coalescing-additions" => "--max-coalescing-additions",
            "--max-rule-applications" => "--max-rule-applications",
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
            "--manifest" | "--targets" | "--output" | "--events" | "--owner-base"
            | "--stop-file" | "--expansion-limits" | "--entry-domains" => {
                if value.is_empty() || value == "-" {
                    return Err(ArgError::InvalidValue {
                        option: name,
                        value,
                        expected: "a filesystem path, not stdin/stdout",
                    });
                }
                let path = PathBuf::from(value);
                match name {
                    "--manifest" => result.manifest = path,
                    "--targets" => result.targets = path,
                    "--entry-domains" => result.entry_domains = Some(path),
                    "--output" => result.output = path,
                    "--events" => result.events = Some(path),
                    "--owner-base" => result.owner_base = path,
                    "--expansion-limits" => result.expansion_limits = Some(path),
                    _ => result.stop_file = Some(path),
                }
            }
            _ => {
                let number = parse_positive_integer(name, value)?;
                match name {
                    "--workers" => result.workers = number,
                    "--max-nodes" => result.nodes = number,
                    "--max-input-targets" => result.input_targets = number,
                    "--max-transport-operations" => result.transport_operations = number,
                    "--max-transport-endpoints" => result.transport_endpoints = number,
                    "--max-coalescing-additions" => result.coalescing_additions = number,
                    _ => result.rule_applications = number,
                }
            }
        }
    }
    for option in ["--manifest", "--targets", "--output"] {
        if !seen.contains(option) {
            return Err(ArgError::MissingRequiredOption(option));
        }
    }
    if result.workers > 50 {
        return Err(ArgError::InvalidCombination("at most 50 outer workers"));
    }
    Ok(Command::RoutedCampaign(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse_words(text: &str) -> Result<Command, ArgError> {
        parse(text.split_whitespace().map(OsString::from))
    }
    #[test]
    fn strict_routed_arguments_have_no_inherited_timeout() {
        let Command::RoutedCampaign(args) =
            parse_words("--manifest m --targets t --output o --workers 50").unwrap()
        else {
            panic!()
        };
        assert_eq!(args.workers, 50);
        assert_eq!(args.expansion_limits, None);
        assert_eq!(args.entry_domains, None);
        for suffix in [
            "--workers 0",
            "--workers 51",
            "--max-nodes -1",
            "--workers 2 --workers 3",
            "--timeout 1800",
            "--expansion-limits -",
            "--expansion-limits a --expansion-limits b",
            "--entry-domains -",
            "--entry-domains a --entry-domains b",
        ] {
            assert!(parse_words(&format!("--manifest m --targets t --output o {suffix}")).is_err());
        }
    }

    #[test]
    fn explicit_per_call_limits_are_a_path_not_an_aggregate_override() {
        let Command::RoutedCampaign(args) = parse_words(
            "--manifest m --targets t --output o --expansion-limits limits.json --max-transport-operations 9",
        ).unwrap() else { panic!() };
        assert_eq!(args.expansion_limits, Some(PathBuf::from("limits.json")));
        assert_eq!(args.transport_operations, 9);
    }

    #[test]
    fn explicit_starting_domain_is_opt_in_and_separate_from_targets() {
        let Command::RoutedCampaign(args) =
            parse_words("--manifest m --targets t --output o --entry-domains roots.json").unwrap()
        else {
            panic!()
        };
        assert_eq!(args.entry_domains, Some(PathBuf::from("roots.json")));
        assert_eq!(args.targets, PathBuf::from("t"));
    }
}
