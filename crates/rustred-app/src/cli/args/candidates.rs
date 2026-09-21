use std::ffi::OsString;
use std::path::PathBuf;

use super::{
    ArgError, Command, ResourceLimitsArgs, StreamPath, next_utf8_value, next_value,
    parse_nonnegative_integer, parse_positive_integer, set_once,
};
use crate::{
    CandidateBundleLimits, CandidateCheckpointOptions, CandidateExactBackend, FiniteCaseLimits,
    FiniteCasePolicy, InputFormat, MAX_CANDIDATE_BUNDLE_BYTES,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FamilyCandidatesArgs {
    pub input: StreamPath,
    pub output: StreamPath,
    pub report_output: Option<StreamPath>,
    pub input_format: InputFormat,
    pub n_cores: usize,
    pub exact_backend: CandidateExactBackend,
    pub numerical_depth: u32,
    pub max_numerator_rank: Option<u32>,
    pub finite_case_policy: FiniteCasePolicy,
    pub finite_case_limits: FiniteCaseLimits,
    pub bundle_limits: CandidateBundleLimits,
    pub checkpoint: Option<CandidateCheckpointOptions>,
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
    pub max_total_excess_degree: Option<u64>,
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
    let mut numerical_depth = None;
    let mut max_numerator_rank = None;
    let mut finite_case_policy = None;
    let mut finite_max_visited_points = None;
    let mut finite_max_retained_terminals = None;
    let mut bundle_max_bytes = None;
    let mut bundle_max_entries = None;
    let mut bundle_max_coefficient_bytes = None;
    let mut bundle_max_total_coefficient_bytes = None;
    let mut checkpoint_dir = None;
    let mut checkpoint_max_bytes = None;
    let mut resume = false;
    let mut progress = false;
    let mut permutation = None;
    let mut nonpositive_indices = None;
    let mut resources = ResourceLimitsArgs::default();
    let mut max_negative_index_degree = None;
    let mut max_total_excess_degree = None;
    let mut force = false;
    let mut help = false;
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--bundle-max-bytes"
            | "--bundle-max-entries"
            | "--bundle-max-coefficient-bytes"
            | "--bundle-max-total-coefficient-bytes"
                if !certification =>
            {
                let (name, slot) = match option.as_str() {
                    "--bundle-max-bytes" => ("--bundle-max-bytes", &mut bundle_max_bytes),
                    "--bundle-max-entries" => ("--bundle-max-entries", &mut bundle_max_entries),
                    "--bundle-max-coefficient-bytes" => (
                        "--bundle-max-coefficient-bytes",
                        &mut bundle_max_coefficient_bytes,
                    ),
                    _ => (
                        "--bundle-max-total-coefficient-bytes",
                        &mut bundle_max_total_coefficient_bytes,
                    ),
                };
                let value = next_utf8_value(&mut arguments, name)?;
                set_once(slot, name, parse_positive_integer(name, value)?)?;
            }
            "--checkpoint-dir" if !certification => {
                let path = next_value(&mut arguments, "--checkpoint-dir")?;
                if path.is_empty() {
                    return Err(ArgError::InvalidCombination(
                        "--checkpoint-dir must not be empty",
                    ));
                }
                set_once(&mut checkpoint_dir, "--checkpoint-dir", PathBuf::from(path))?;
            }
            "--checkpoint-max-bytes" if !certification => {
                let value = next_utf8_value(&mut arguments, "--checkpoint-max-bytes")?;
                set_once(
                    &mut checkpoint_max_bytes,
                    "--checkpoint-max-bytes",
                    parse_positive_integer("--checkpoint-max-bytes", value)?,
                )?;
            }
            "--resume" if !certification => {
                if resume {
                    return Err(ArgError::DuplicateOption("--resume"));
                }
                resume = true;
            }
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
            "--numerical-depth" if !certification => {
                let value = next_utf8_value(&mut arguments, "--numerical-depth")?;
                let parsed = value.parse::<u32>().map_err(|_| ArgError::InvalidValue {
                    option: "--numerical-depth",
                    value,
                    expected: "an integer from 0 to 4294967295",
                })?;
                set_once(&mut numerical_depth, "--numerical-depth", parsed)?;
            }
            "--max-numerator-rank" if !certification => {
                let value = next_utf8_value(&mut arguments, "--max-numerator-rank")?;
                let parsed = value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit())
                    .then(|| value.parse::<u32>().ok())
                    .flatten()
                    .ok_or(ArgError::InvalidValue {
                        option: "--max-numerator-rank",
                        value,
                        expected: "a decimal integer from 0 to 4294967295",
                    })?;
                set_once(&mut max_numerator_rank, "--max-numerator-rank", parsed)?;
            }
            "--finite-case-policy" if !certification => {
                let value = next_utf8_value(&mut arguments, "--finite-case-policy")?;
                let parsed = value.parse().map_err(|_| ArgError::InvalidValue {
                    option: "--finite-case-policy",
                    value,
                    expected: FiniteCasePolicy::EXPECTED_VALUES,
                })?;
                set_once(&mut finite_case_policy, "--finite-case-policy", parsed)?;
            }
            "--finite-max-visited-points" if !certification => {
                let value = next_utf8_value(&mut arguments, "--finite-max-visited-points")?;
                set_once(
                    &mut finite_max_visited_points,
                    "--finite-max-visited-points",
                    parse_positive_integer("--finite-max-visited-points", value)?,
                )?;
            }
            "--finite-max-retained-terminals" if !certification => {
                let value = next_utf8_value(&mut arguments, "--finite-max-retained-terminals")?;
                set_once(
                    &mut finite_max_retained_terminals,
                    "--finite-max-retained-terminals",
                    parse_positive_integer("--finite-max-retained-terminals", value)?,
                )?;
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
            "--max-total-excess-degree" if certification => {
                let value = next_utf8_value(&mut arguments, "--max-total-excess-degree")?;
                let parsed = value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit())
                    .then(|| value.parse::<u64>().ok())
                    .flatten()
                    .ok_or(ArgError::InvalidValue {
                        option: "--max-total-excess-degree",
                        value,
                        expected: "a decimal integer from 0 to 18446744073709551615",
                    })?;
                set_once(
                    &mut max_total_excess_degree,
                    "--max-total-excess-degree",
                    parsed,
                )?;
            }
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option)),
            _ => return Err(ArgError::UnexpectedArgument(option)),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    let finite_case_policy = finite_case_policy.unwrap_or_default();
    if finite_case_policy == FiniteCasePolicy::RetainRankFinite && max_numerator_rank.is_none() {
        return Err(ArgError::InvalidCombination(
            "--finite-case-policy retain-rank-finite requires --max-numerator-rank",
        ));
    }
    if finite_case_policy != FiniteCasePolicy::RetainRankFinite
        && (finite_max_visited_points.is_some() || finite_max_retained_terminals.is_some())
    {
        return Err(ArgError::InvalidCombination(
            "finite retention limits require --finite-case-policy retain-rank-finite",
        ));
    }
    let default_finite_limits = FiniteCaseLimits::default();
    let finite_case_limits = FiniteCaseLimits {
        max_visited_points: finite_max_visited_points
            .unwrap_or(default_finite_limits.max_visited_points),
        max_retained_terminals: finite_max_retained_terminals
            .unwrap_or(default_finite_limits.max_retained_terminals),
    };
    if bundle_max_bytes.is_some_and(|bytes| bytes > MAX_CANDIDATE_BUNDLE_BYTES) {
        return Err(ArgError::InvalidCombination(
            "--bundle-max-bytes exceeds the hard 1073741824-byte (1 GiB) candidate limit",
        ));
    }
    let defaults = CandidateBundleLimits::default();
    let bundle_limits = CandidateBundleLimits {
        max_bundle_bytes: bundle_max_bytes.unwrap_or(defaults.max_bundle_bytes),
        max_collection_entries: bundle_max_entries.unwrap_or(defaults.max_collection_entries),
        max_coefficient_bytes: bundle_max_coefficient_bytes
            .unwrap_or(defaults.max_coefficient_bytes),
        max_total_coefficient_bytes: bundle_max_total_coefficient_bytes
            .unwrap_or(defaults.max_total_coefficient_bytes),
        ..defaults
    };
    if max_negative_index_degree.is_some() && max_total_excess_degree.is_some() {
        return Err(ArgError::InvalidCombination(
            "--max-negative-index-degree and --max-total-excess-degree are mutually exclusive",
        ));
    }
    if checkpoint_dir.is_none() && (resume || checkpoint_max_bytes.is_some()) {
        return Err(ArgError::InvalidCombination(
            "--resume and --checkpoint-max-bytes require --checkpoint-dir",
        ));
    }
    let checkpoint = checkpoint_dir.map(|directory| {
        let mut options = CandidateCheckpointOptions::new(directory);
        options.resume = resume;
        if let Some(bytes) = checkpoint_max_bytes {
            options.max_total_bytes = bytes;
        }
        options
    });
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
            max_total_excess_degree,
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
            numerical_depth: numerical_depth
                .unwrap_or_else(|| rustred::solver::SectorSolveOptions::default().numerical_depth),
            max_numerator_rank,
            finite_case_policy,
            finite_case_limits,
            bundle_limits,
            checkpoint,
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
    fn bundle_transport_limits_are_positive_optional_and_generation_only() {
        let parse_args = |args: &[&str]| parse_generation(args.iter().map(OsString::from));
        let Command::FamilyCandidates(defaults) = parse_args(&[]).unwrap() else {
            panic!("generation expected")
        };
        assert_eq!(defaults.bundle_limits, CandidateBundleLimits::default());
        for option in [
            "--bundle-max-bytes",
            "--bundle-max-entries",
            "--bundle-max-coefficient-bytes",
            "--bundle-max-total-coefficient-bytes",
        ] {
            for value in ["0", "-1", "+1", "true", "0.5", "", "184467440737095516160"] {
                assert!(parse_args(&[option, value]).is_err(), "{option} {value}");
            }
            assert!(parse_args(&[option, "1", option, "1"]).is_err());
            assert!(parse_certification([option, "1"].into_iter().map(OsString::from)).is_err());
        }
        assert!(parse_args(&["--bundle-max-bytes", "1073741825"]).is_err());
        let Command::FamilyCandidates(parsed) = parse_args(&[
            "--bundle-max-bytes",
            "1073741824",
            "--bundle-max-entries",
            "32000000",
            "--bundle-max-coefficient-bytes",
            "33554432",
            "--bundle-max-total-coefficient-bytes",
            "536870912",
        ])
        .unwrap() else {
            panic!("generation expected")
        };
        assert_eq!(
            parsed.bundle_limits.max_bundle_bytes,
            MAX_CANDIDATE_BUNDLE_BYTES
        );
        assert_eq!(parsed.bundle_limits.max_collection_entries, 32_000_000);
        assert_eq!(parsed.bundle_limits.max_coefficient_bytes, 32 * 1024 * 1024);
        assert_eq!(
            parsed.bundle_limits.max_total_coefficient_bytes,
            512 * 1024 * 1024
        );
    }

    #[test]
    fn finite_retention_requires_rank_and_owns_only_positive_work_limits() {
        let parse_args = |args: &[&str]| parse_generation(args.iter().map(OsString::from));
        for args in [
            vec!["--finite-case-policy", "retain-rank-finite"],
            vec![
                "--finite-case-policy",
                "unknown",
                "--max-numerator-rank",
                "10",
            ],
            vec!["--finite-max-visited-points", "1"],
            vec!["--finite-max-retained-terminals", "1"],
            vec![
                "--finite-case-policy",
                "retain-rank-finite",
                "--max-numerator-rank",
                "10",
                "--finite-max-visited-points",
                "0",
            ],
            vec![
                "--finite-case-policy",
                "retain-rank-finite",
                "--max-numerator-rank",
                "10",
                "--finite-max-retained-terminals",
                "0",
            ],
            vec![
                "--finite-case-policy",
                "search",
                "--finite-case-policy",
                "search",
            ],
        ] {
            assert!(parse_args(&args).is_err(), "{args:?}");
        }
        let Command::FamilyCandidates(args) = parse_args(&[
            "--finite-case-policy",
            "retain-rank-finite",
            "--max-numerator-rank",
            "0",
            "--finite-max-visited-points",
            "123",
            "--finite-max-retained-terminals",
            "45",
        ])
        .unwrap() else {
            panic!("generation expected")
        };
        assert_eq!(args.finite_case_policy, FiniteCasePolicy::RetainRankFinite);
        assert_eq!(
            args.finite_case_limits,
            FiniteCaseLimits {
                max_visited_points: 123,
                max_retained_terminals: 45
            }
        );
        assert!(
            parse_certification(
                ["--finite-case-policy", "search"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_err()
        );
    }

    #[test]
    fn numerator_rank_is_optional_strict_u32_and_generation_only() {
        let Command::FamilyCandidates(default) = parse_generation(std::iter::empty()).unwrap()
        else {
            panic!("generation expected")
        };
        assert_eq!(default.max_numerator_rank, None);
        for rank in [0, 10, 20, u32::MAX] {
            let value = rank.to_string();
            let Command::FamilyCandidates(args) = parse_generation(
                ["--max-numerator-rank", &value]
                    .into_iter()
                    .map(OsString::from),
            )
            .unwrap() else {
                panic!("generation expected")
            };
            assert_eq!(args.max_numerator_rank, Some(rank));
            assert_eq!(args.numerical_depth, 2);
        }
        for value in ["", "-1", "+1", "1.0", " 1", "true", "4294967296"] {
            assert!(
                parse_generation(
                    ["--max-numerator-rank", value]
                        .into_iter()
                        .map(OsString::from)
                )
                .is_err()
            );
        }
        assert!(
            parse_generation(["--max-numerator-rank"].into_iter().map(OsString::from)).is_err()
        );
        assert!(
            parse_generation(
                ["--max-numerator-rank", "1", "--max-numerator-rank", "1"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_err()
        );
        assert!(
            parse_certification(
                ["--max-numerator-rank", "1"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_err()
        );
    }

    #[test]
    fn total_excess_scope_is_optional_u64_and_certification_only() {
        let Command::CertifyCandidates(default) = parse_certification(std::iter::empty()).unwrap()
        else {
            panic!("certification expected")
        };
        assert_eq!(default.max_total_excess_degree, None);
        for degree in [0, 2, 30, 31, u64::MAX] {
            let value = degree.to_string();
            let Command::CertifyCandidates(arguments) = parse_certification(
                ["--max-total-excess-degree", value.as_str()]
                    .into_iter()
                    .map(OsString::from),
            )
            .unwrap() else {
                panic!("certification expected")
            };
            assert_eq!(arguments.max_total_excess_degree, Some(degree));
            assert_eq!(arguments.max_negative_index_degree, None);
        }
        for value in [
            "",
            "-1",
            "+1",
            "1.0",
            " 1",
            "1e2",
            "0x1",
            "18446744073709551616",
        ] {
            assert!(
                parse_certification(
                    ["--max-total-excess-degree", value]
                        .into_iter()
                        .map(OsString::from)
                )
                .is_err(),
                "accepted {value:?}"
            );
        }
        for options in [
            vec!["--max-total-excess-degree"],
            vec![
                "--max-total-excess-degree",
                "2",
                "--max-total-excess-degree",
                "2",
            ],
            vec![
                "--max-total-excess-degree",
                "2",
                "--max-negative-index-degree",
                "1",
            ],
            vec![
                "--max-negative-index-degree",
                "1",
                "--max-total-excess-degree",
                "2",
            ],
        ] {
            assert!(parse_certification(options.into_iter().map(OsString::from)).is_err());
        }
        assert!(
            parse_generation(
                ["--max-total-excess-degree", "2"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_err()
        );
    }

    #[test]
    fn checkpoint_options_are_explicit_generation_only_and_require_a_directory() {
        let Command::FamilyCandidates(default) = parse_generation(std::iter::empty()).unwrap()
        else {
            panic!("generation expected")
        };
        assert_eq!(default.checkpoint, None);
        let Command::FamilyCandidates(args) = parse_generation(
            [
                "--checkpoint-dir",
                "saved-sectors",
                "--resume",
                "--checkpoint-max-bytes",
                "8192",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap() else {
            panic!("generation expected")
        };
        let options = args.checkpoint.unwrap();
        assert_eq!(options.directory, PathBuf::from("saved-sectors"));
        assert!(options.resume);
        assert_eq!(options.max_total_bytes, 8192);
        for args in [
            vec!["--resume"],
            vec!["--checkpoint-max-bytes", "1"],
            vec!["--checkpoint-dir", ""],
            vec!["--checkpoint-dir", "x", "--checkpoint-dir", "y"],
            vec!["--checkpoint-dir", "x", "--resume", "--resume"],
            vec![
                "--checkpoint-dir",
                "x",
                "--checkpoint-max-bytes",
                "1",
                "--checkpoint-max-bytes",
                "2",
            ],
        ] {
            assert!(parse_generation(args.into_iter().map(OsString::from)).is_err());
        }
        for value in ["0", "-1", "true", "18446744073709551616"] {
            assert!(
                parse_generation(
                    ["--checkpoint-dir", "x", "--checkpoint-max-bytes", value]
                        .into_iter()
                        .map(OsString::from)
                )
                .is_err()
            );
        }
        for args in [
            vec!["--checkpoint-dir", "x"],
            vec!["--resume"],
            vec!["--checkpoint-max-bytes", "1"],
        ] {
            assert!(parse_certification(args.into_iter().map(OsString::from)).is_err());
        }
    }

    #[test]
    fn explicit_candidate_backend_is_separate_from_certification() {
        for backend in [
            CandidateExactBackend::Sparse,
            CandidateExactBackend::SparseFactorized,
            CandidateExactBackend::SparseTargetOnlyFactorized,
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
