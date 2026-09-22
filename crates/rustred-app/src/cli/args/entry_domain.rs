use super::{ArgError, Command, StreamPath, next_value, set_once};
use std::ffi::OsString;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntryDomainPlanArgs {
    pub input: StreamPath,
    pub output: StreamPath,
    pub force: bool,
}

pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut input = None;
    let mut output = None;
    let mut force = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--input" => set_once(
                &mut input,
                "--input",
                StreamPath::parse(next_value(&mut arguments, "--input")?)?,
            )?,
            "--output" => set_once(
                &mut output,
                "--output",
                StreamPath::parse(next_value(&mut arguments, "--output")?)?,
            )?,
            "--force" => {
                if force {
                    return Err(ArgError::DuplicateOption("--force"));
                }
                force = true;
            }
            "--help" | "-h" => {
                if help {
                    return Err(ArgError::DuplicateOption("--help"));
                }
                help = true;
            }
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::EntryDomainPlan(EntryDomainPlanArgs {
        input: input.ok_or(ArgError::MissingRequiredOption("--input"))?,
        output: output.ok_or(ArgError::MissingRequiredOption("--output"))?,
        force,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(s: &str) -> Result<Command, ArgError> {
        super::parse(s.split_whitespace().map(OsString::from))
    }
    #[test]
    fn entry_plan_cli_stdio_force_and_dispatch() {
        assert_eq!(
            parse("--input - --output - --force"),
            Ok(Command::EntryDomainPlan(EntryDomainPlanArgs {
                input: StreamPath::Stdio,
                output: StreamPath::Stdio,
                force: true,
            }))
        );
        assert!(matches!(
            super::super::parse_args(
                [
                    "rustred",
                    "entry-domain-plan",
                    "--input",
                    "in.json",
                    "--output",
                    "out.json"
                ]
                .map(OsString::from)
            ),
            Ok(Command::EntryDomainPlan(_))
        ));
        assert_eq!(parse("--help"), Ok(Command::Help));
    }
    #[test]
    fn entry_plan_cli_rejects_missing_duplicate_and_solver_options() {
        for args in [
            "",
            "--input in",
            "--input in --output out --input other",
            "--input in --output out --force --force",
            "--input in --output out --workers 50",
            "--input in --output out --max-preview-targets 10",
        ] {
            assert!(parse(args).is_err(), "{args}");
        }
    }
}
