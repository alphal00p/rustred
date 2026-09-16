use std::ffi::OsString;

use super::{
    ArgError, Command, FamilyCloseArgs, ResourceLimitsArgs, StreamPath, next_utf8_value,
    next_value, parse_nonnegative_integer, parse_positive_integer, set_once,
};
use crate::InputFormat;

pub(super) fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Command, ArgError> {
    let mut input = None;
    let mut output = None;
    let mut input_format = None;
    let mut permutation = None;
    let mut nonpositive_indices = None;
    let mut resources = ResourceLimitsArgs::default();
    let mut n_cores = None;
    let mut force = false;
    let mut progress = false;
    let mut help = false;
    let mut arguments = arguments.peekable();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--help" | "-h" => {
                if help {
                    return Err(ArgError::DuplicateOption("--help"));
                }
                help = true;
            }
            "--force" => {
                if force {
                    return Err(ArgError::DuplicateOption("--force"));
                }
                force = true;
            }
            "--progress" => {
                if progress {
                    return Err(ArgError::DuplicateOption("--progress"));
                }
                progress = true;
            }
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
            "--input-format" => {
                let value = next_utf8_value(&mut arguments, "--input-format")?;
                let parsed = value.parse().map_err(|_| ArgError::InvalidValue {
                    option: "--input-format",
                    value,
                    expected: InputFormat::EXPECTED_VALUES,
                })?;
                set_once(&mut input_format, "--input-format", parsed)?;
            }
            "--permutation" => {
                let value = next_utf8_value(&mut arguments, "--permutation")?;
                let coordinates = value
                    .split(',')
                    .map(|token| parse_nonnegative_integer("--permutation", token.to_owned()))
                    .collect::<Result<Vec<_>, _>>()?;
                set_once(&mut permutation, "--permutation", coordinates)?;
            }
            "--nonpositive-indices" => {
                let value = next_utf8_value(&mut arguments, "--nonpositive-indices")?;
                let coordinates = value
                    .split(',')
                    .map(|token| {
                        parse_nonnegative_integer("--nonpositive-indices", token.to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                set_once(
                    &mut nonpositive_indices,
                    "--nonpositive-indices",
                    coordinates,
                )?;
            }
            "--n-cores" => {
                let value = next_utf8_value(&mut arguments, "--n-cores")?;
                set_once(
                    &mut n_cores,
                    "--n-cores",
                    parse_positive_integer("--n-cores", value)?,
                )?;
            }
            _ if resources.parse_option(&option, &mut arguments)? => {}
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    Ok(Command::FamilyClose(FamilyCloseArgs {
        input: input.unwrap_or(StreamPath::Stdio),
        output: output.unwrap_or(StreamPath::Stdio),
        input_format: input_format.unwrap_or(InputFormat::Auto),
        permutation,
        nonpositive_indices: nonpositive_indices.unwrap_or_default(),
        resources,
        n_cores: n_cores.unwrap_or(1),
        progress,
        force,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_generic_family_close_options() {
        let command = parse(
            [
                "--input",
                "family.toml",
                "--output",
                "family.rr",
                "--permutation",
                "2,1,0",
                "--n-cores",
                "3",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap();
        let Command::FamilyClose(arguments) = command else {
            panic!("expected family-close command");
        };
        assert_eq!(arguments.permutation, Some(vec![2, 1, 0]));
        assert_eq!(arguments.n_cores, 3);
        assert!(!arguments.force);
        assert!(!arguments.progress);
        assert!(arguments.nonpositive_indices.is_empty());
    }

    #[test]
    fn progress_flag_is_opt_in_and_cannot_be_repeated() {
        let command = parse(["--progress"].into_iter().map(OsString::from)).unwrap();
        let Command::FamilyClose(arguments) = command else {
            panic!("expected family-close");
        };
        assert!(arguments.progress);
        assert!(matches!(
            parse(["--progress", "--progress"].into_iter().map(OsString::from)),
            Err(ArgError::DuplicateOption("--progress"))
        ));
    }

    #[test]
    fn nonpositive_indices_are_explicit_and_duplicate_options_fail() {
        let command = parse(
            ["--nonpositive-indices", "8,9"]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        let Command::FamilyClose(arguments) = command else {
            panic!("expected family-close");
        };
        assert_eq!(arguments.nonpositive_indices, [8, 9]);
        assert!(
            parse(
                ["--nonpositive-indices", ""]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_err()
        );
        assert!(matches!(
            parse(
                ["--nonpositive-indices", "0", "--nonpositive-indices", "1"]
                    .into_iter()
                    .map(OsString::from)
            ),
            Err(ArgError::DuplicateOption("--nonpositive-indices"))
        ));
    }

    #[test]
    fn rejects_empty_and_duplicate_permutation_options() {
        assert!(parse(["--permutation", ""].into_iter().map(OsString::from)).is_err());
        assert!(matches!(
            parse(
                ["--permutation", "0", "--permutation", "0"]
                    .into_iter()
                    .map(OsString::from)
            ),
            Err(ArgError::DuplicateOption("--permutation"))
        ));
    }
}
