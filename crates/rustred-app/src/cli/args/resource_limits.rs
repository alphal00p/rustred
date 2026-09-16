use std::ffi::OsString;

use crate::{ArtifactLoadLimits, SourcePortLimits};

use super::{ArgError, next_utf8_value, parse_nonnegative_integer, set_once};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResourceLimitsArgs {
    endpoint_cells: Option<usize>,
    consistency_work: Option<usize>,
}

impl ResourceLimitsArgs {
    pub(super) fn parse_option(
        &mut self,
        option: &str,
        arguments: &mut impl Iterator<Item = OsString>,
    ) -> Result<bool, ArgError> {
        let (name, slot) = match option {
            "--max-domain-bound-endpoint-cells" => (
                "--max-domain-bound-endpoint-cells",
                &mut self.endpoint_cells,
            ),
            "--max-predicate-consistency-work" => (
                "--max-predicate-consistency-work",
                &mut self.consistency_work,
            ),
            _ => return Ok(false),
        };
        let value = parse_nonnegative_integer(name, next_utf8_value(arguments, name)?)?;
        set_once(slot, name, value)?;
        Ok(true)
    }

    pub(crate) fn publication_limits(&self) -> SourcePortLimits {
        let mut limits = SourcePortLimits::default();
        if let Some(value) = self.endpoint_cells {
            limits.rule_derivation.max_domain_bound_endpoint_cells = value;
        }
        if let Some(value) = self.consistency_work {
            limits.max_predicate_consistency_work = value;
        }
        limits
    }

    pub(crate) fn load_limits(&self) -> ArtifactLoadLimits {
        let mut limits = ArtifactLoadLimits::default();
        if let Some(value) = self.endpoint_cells {
            limits.rule_derivation.max_domain_bound_endpoint_cells = value;
        }
        if let Some(value) = self.consistency_work {
            limits.max_predicate_consistency_work = value;
        }
        limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::{Command, parse_args};

    fn parse(command: &[&str], flags: &[&str]) -> Result<Command, ArgError> {
        parse_args(
            std::iter::once("rustred")
                .chain(command.iter().copied())
                .chain(flags.iter().copied())
                .map(OsString::from),
        )
    }

    fn resources(command: Command) -> ResourceLimitsArgs {
        match command {
            Command::FamilyClose(args) => args.resources,
            Command::CampaignInspect(args) => args.resources,
            Command::CampaignReduce(args) => args.resources,
            _ => panic!("expected a publication or artifact-load command"),
        }
    }

    #[test]
    fn resource_flags_share_defaults_and_accept_zero_on_every_entry_point() {
        for command in [
            &["family-close"][..],
            &["campaign", "inspect", "--artifact", "-"],
            &["campaign", "reduce", "--artifact", "-", "--powers", "1"],
        ] {
            let defaults = resources(parse(command, &[]).unwrap());
            assert_eq!(defaults.publication_limits(), SourcePortLimits::default());
            assert_eq!(defaults.load_limits(), ArtifactLoadLimits::default());
            let chosen = resources(
                parse(
                    command,
                    &[
                        "--max-domain-bound-endpoint-cells",
                        "0",
                        "--max-predicate-consistency-work",
                        "17",
                    ],
                )
                .unwrap(),
            );
            assert_eq!(
                chosen
                    .publication_limits()
                    .rule_derivation
                    .max_domain_bound_endpoint_cells,
                0
            );
            assert_eq!(
                chosen
                    .load_limits()
                    .rule_derivation
                    .max_domain_bound_endpoint_cells,
                0
            );
            assert_eq!(
                chosen.publication_limits().max_predicate_consistency_work,
                17
            );
            assert_eq!(chosen.load_limits().max_predicate_consistency_work, 17);
            let maximum = usize::MAX.to_string();
            let chosen =
                resources(parse(command, &["--max-predicate-consistency-work", &maximum]).unwrap());
            assert_eq!(
                chosen.load_limits().max_predicate_consistency_work,
                usize::MAX
            );
        }
    }

    #[test]
    fn resource_flags_reject_negative_overflow_missing_and_duplicate_values() {
        for command in [
            &["family-close"][..],
            &["campaign", "inspect", "--artifact", "-"],
            &["campaign", "reduce", "--artifact", "-", "--powers", "1"],
        ] {
            for flag in [
                "--max-domain-bound-endpoint-cells",
                "--max-predicate-consistency-work",
            ] {
                for value in ["-1", "340282366920938463463374607431768211456", "1.5"] {
                    assert!(matches!(
                        parse(command, &[flag, value]),
                        Err(ArgError::InvalidValue { .. })
                    ));
                }
                assert!(matches!(
                    parse(command, &[flag]),
                    Err(ArgError::MissingValue(_))
                ));
                assert!(matches!(
                    parse(command, &[flag, "0", flag, "1"]),
                    Err(ArgError::DuplicateOption(_))
                ));
            }
        }
    }
}
