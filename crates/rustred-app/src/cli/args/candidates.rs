use std::ffi::OsString;

use super::{
    ArgError, Command, ResourceLimitsArgs, StreamPath, next_utf8_value, next_value,
    parse_nonnegative_integer, parse_positive_integer, set_once,
};
use crate::{CandidateExactBackend, InputFormat};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FamilyCandidatesArgs {
    pub input: StreamPath,
    pub output: StreamPath,
    pub report_output: Option<StreamPath>,
    pub input_format: InputFormat,
    pub n_cores: usize,
    pub exact_backend: CandidateExactBackend,
    pub progress: bool,
    pub permutation: Option<Vec<usize>>,
    pub nonpositive_indices: Vec<usize>,
    pub force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CertifyCandidatesArgs {
    pub input: StreamPath,
    pub output: StreamPath,
    pub report_output: Option<StreamPath>,
    pub resources: ResourceLimitsArgs,
    pub max_negative_index_degree: Option<usize>,
    pub force: bool,
}

pub(super) fn parse_generation(
    arguments: impl Iterator<Item = OsString>,
) -> Result<Command, ArgError> {
    parse(arguments, false)
}

pub(super) fn parse_certification(
    arguments: impl Iterator<Item = OsString>,
) -> Result<Command, ArgError> {
    parse(arguments, true)
}

fn parse(
    mut arguments: impl Iterator<Item = OsString>,
    certification: bool,
) -> Result<Command, ArgError> {
    let mut input = None;
    let mut output = None;
    let mut report_output = None;
    let mut input_format = None;
    let mut n_cores = None;
    let mut exact_backend = None;
    let mut progress = false;
    let mut permutation = None;
    let mut nonpositive_indices = None;
    let mut resources = ResourceLimitsArgs::default();
    let mut max_negative_index_degree = None;
    let mut force = false;
    let mut help = false;
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--progress" if !certification => {
                if progress {
                    return Err(ArgError::DuplicateOption("--progress"));
                }
                progress = true;
            }
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
            "--report-output" => set_once(
                &mut report_output,
                "--report-output",
                StreamPath::parse(next_value(&mut arguments, "--report-output")?)?,
            )?,
            "--input-format" if !certification => {
                let value = next_utf8_value(&mut arguments, "--input-format")?;
                let parsed = value.parse().map_err(|_| ArgError::InvalidValue {
                    option: "--input-format",
                    value,
                    expected: InputFormat::EXPECTED_VALUES,
                })?;
                set_once(&mut input_format, "--input-format", parsed)?;
            }
            "--n-cores" if !certification => {
                let value = next_utf8_value(&mut arguments, "--n-cores")?;
                set_once(
                    &mut n_cores,
                    "--n-cores",
                    parse_positive_integer("--n-cores", value)?,
                )?;
            }
            "--exact-backend" if !certification => {
                let value = next_utf8_value(&mut arguments, "--exact-backend")?;
                let parsed = value.parse().map_err(|_| ArgError::InvalidValue {
                    option: "--exact-backend",
                    value,
                    expected: CandidateExactBackend::EXPECTED_VALUES,
                })?;
                set_once(&mut exact_backend, "--exact-backend", parsed)?;
            }
            "--permutation" if !certification => {
                let value = next_utf8_value(&mut arguments, "--permutation")?;
                set_once(
                    &mut permutation,
                    "--permutation",
                    parse_indices("--permutation", value)?,
                )?;
            }
            "--nonpositive-indices" if !certification => {
                let value = next_utf8_value(&mut arguments, "--nonpositive-indices")?;
                set_once(
                    &mut nonpositive_indices,
                    "--nonpositive-indices",
                    parse_indices("--nonpositive-indices", value)?,
                )?;
            }
            _ if certification && resources.parse_option(&option, &mut arguments)? => {}
            "--max-negative-index-degree" if certification => set_once(
                &mut max_negative_index_degree,
                "--max-negative-index-degree",
                parse_nonnegative_integer(
                    "--max-negative-index-degree",
                    next_utf8_value(&mut arguments, "--max-negative-index-degree")?,
                )?,
            )?,
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    let input = input.unwrap_or(StreamPath::Stdio);
    let output = output.unwrap_or(StreamPath::Stdio);
    if super::campaign::same_file(&input, &output) {
        return Err(ArgError::InvalidCombination(
            "--input and --output must not name the same file",
        ));
    }
    if let Some(report) = &report_output {
        if super::campaign::same_stream_or_file(&output, report)
            || super::campaign::same_file(&input, report)
        {
            return Err(ArgError::InvalidCombination(
                "--report-output must differ from the data output stream/path and input file",
            ));
        }
    }
    if certification {
        Ok(Command::CertifyCandidates(CertifyCandidatesArgs {
            input,
            output,
            report_output,
            resources,
            max_negative_index_degree,
            force,
        }))
    } else {
        Ok(Command::FamilyCandidates(FamilyCandidatesArgs {
            input,
            output,
            report_output,
            input_format: input_format.unwrap_or(InputFormat::Auto),
            n_cores: n_cores.unwrap_or(1),
            exact_backend: exact_backend.unwrap_or_default(),
            progress,
            permutation,
            nonpositive_indices: nonpositive_indices.unwrap_or_default(),
            force,
        }))
    }
}

fn parse_indices(option: &'static str, value: String) -> Result<Vec<usize>, ArgError> {
    value
        .split(',')
        .map(|v| parse_nonnegative_integer(option, v.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_candidate_backend_is_separate_from_certification() {
        for backend in [
            CandidateExactBackend::Sparse,
            CandidateExactBackend::SparseFactorized,
            CandidateExactBackend::SemiNumerical,
        ] {
            let Command::FamilyCandidates(args) = parse_generation(
                ["--exact-backend", backend.as_str()]
                    .into_iter()
                    .map(OsString::from),
            )
            .unwrap() else {
                panic!("generation expected")
            };
            assert_eq!(args.exact_backend, backend);
            assert!(
                parse_certification(
                    ["--exact-backend", backend.as_str()]
                        .into_iter()
                        .map(OsString::from),
                )
                .is_err()
            );
        }
        let Command::FamilyCandidates(default) = parse_generation(std::iter::empty()).unwrap()
        else {
            panic!("generation expected")
        };
        assert_eq!(default.exact_backend, CandidateExactBackend::Sparse);
        for args in [
            vec!["--exact-backend", "invalid"],
            vec!["--exact-backend", "sparse_factorized"],
            vec![
                "--exact-backend",
                "sparse",
                "--exact-backend",
                "semi-numerical",
            ],
            vec![
                "--exact-backend",
                "sparse-factorized",
                "--exact-backend",
                "sparse-factorized",
            ],
        ] {
            assert!(parse_generation(args.into_iter().map(OsString::from)).is_err());
        }
        assert!(
            parse_certification(
                ["--exact-backend", "sparse"]
                    .into_iter()
                    .map(OsString::from),
            )
            .is_err()
        );
    }

    #[test]
    fn candidate_generation_and_certification_have_separate_controls() {
        let Command::FamilyCandidates(arguments) = parse_generation(
            [
                "--n-cores",
                "2",
                "--permutation",
                "2,1,0",
                "--nonpositive-indices",
                "2",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap() else {
            panic!("generation expected")
        };
        assert_eq!(arguments.n_cores, 2);
        assert_eq!(arguments.permutation, Some(vec![2, 1, 0]));
        assert_eq!(arguments.nonpositive_indices, [2]);
        let Command::CertifyCandidates(arguments) = parse_certification(
            [
                "--max-predicate-atoms",
                "64",
                "--max-domain-bound-endpoint-cells",
                "0",
                "--max-negative-index-degree",
                "30",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap() else {
            panic!("certification expected")
        };
        assert_eq!(
            arguments.resources.publication_limits().max_predicate_atoms,
            64
        );
        assert_eq!(
            arguments
                .resources
                .publication_limits()
                .rule_derivation
                .max_domain_bound_endpoint_cells,
            0
        );
        assert_eq!(arguments.max_negative_index_degree, Some(30));
        assert!(
            parse_generation(
                ["--max-predicate-atoms", "64"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_err()
        );
        assert!(parse_certification(["--n-cores", "2"].into_iter().map(OsString::from)).is_err());
    }

    #[test]
    fn candidate_commands_reject_duplicate_invalid_and_unsupported_values() {
        for options in [
            vec!["--force", "--force"],
            vec!["--input", "a", "--input", "b"],
        ] {
            assert!(parse_generation(options.iter().copied().map(OsString::from)).is_err());
            assert!(parse_certification(options.iter().copied().map(OsString::from)).is_err());
        }
        for options in [
            vec!["--n-cores", "0"],
            vec!["--permutation", ""],
            vec!["--nonpositive-indices", "-1"],
        ] {
            assert!(parse_generation(options.iter().copied().map(OsString::from)).is_err());
        }
        for value in ["257", "-1", "18446744073709551616"] {
            assert!(
                parse_certification(
                    ["--max-predicate-atoms", value]
                        .into_iter()
                        .map(OsString::from)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn candidate_report_must_have_a_distinct_output_and_not_overwrite_input() {
        for options in [
            vec!["--report-output", "-"],
            vec!["--input", "family", "--output", "family"],
            vec!["--output", "result", "--report-output", "./result"],
            vec!["--input", "family", "--report-output", "./family"],
            vec!["--report-output", "a", "--report-output", "b"],
        ] {
            assert!(parse_generation(options.iter().copied().map(OsString::from)).is_err());
            assert!(parse_certification(options.iter().copied().map(OsString::from)).is_err());
        }
        let Command::FamilyCandidates(args) = parse_generation(
            ["--report-output", "timing.toml"]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap() else {
            panic!("candidate generation")
        };
        assert!(matches!(args.report_output, Some(StreamPath::File(_))));
    }
}
