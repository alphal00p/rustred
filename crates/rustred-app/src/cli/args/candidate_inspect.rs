use super::{ArgError, Command, StreamPath, next_value, set_once};
use std::ffi::OsString;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateInspectArgs {
    pub input: StreamPath,
    pub output: StreamPath,
    pub options: Option<StreamPath>,
    pub force: bool,
}

pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let (mut input, mut output, mut options) = (None, None, None);
    let (mut force, mut help) = (false, false);
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
            "--options" => set_once(
                &mut options,
                "--options",
                StreamPath::parse(next_value(&mut arguments, "--options")?)?,
            )?,
            "--force" if !force => force = true,
            "--force" => return Err(ArgError::DuplicateOption("--force")),
            "--help" | "-h" if !help => help = true,
            "--help" | "-h" => return Err(ArgError::DuplicateOption("--help")),
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::CandidateInspect(CandidateInspectArgs {
        input: input.ok_or(ArgError::MissingRequiredOption("--input"))?,
        output: output.unwrap_or(StreamPath::Stdio),
        options,
        force,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_inspect_dispatch_and_strict_options() {
        assert!(matches!(
            super::super::parse_args(
                [
                    "rustred",
                    "candidate-inspect",
                    "--input",
                    "rules.rrbin",
                    "--options",
                    "selection.json"
                ]
                .map(OsString::from)
            ),
            Ok(Command::CandidateInspect(_))
        ));
        for arguments in ["", "--input a --input b", "--input a --workers 1"] {
            assert!(parse(arguments.split_whitespace().map(OsString::from)).is_err());
        }
    }
}
