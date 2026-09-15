//! End-to-end executable-reference sector benchmark with exact parameters.
//!
//! Usage:
//! spired-solve-sector <family> <sector-mask|all> <new-output-directory>
//!   <zero-sectors-file|-> [nonzero-sectors-file|-] [reference-directory|-]
//!   [symbolic-depth|unbounded] [workers] [orderings-file|-] [schedule]
//!
//! Manifest files contain whitespace-separated 0/1 entries, N per sector,
//! exactly as the original SpIRed examples. `all` requires the nonzero manifest.
//! Set RUSTRED_SPIRED_PROGRESS=1 for complete case constraints and coarse
//! search/exact/guard/geometry milestones (diagnostic I/O is not timed fairly
//! against a silent reference run). The default symbolic
//! depth limit is three; it is diagnostic, and exhaustion returns an error.
//! Families: vacuum 1/2/3, fam1_11/12/111/112, bc4PMRad1, fam_cosmo.
//! PM numerical depth follows the original fixture; all other cases use three.
//! Optional ordering lines are `sector-mask coordinate-priority...` (zero-based).
//! Schedule is `active-first` (default) or `input-order`; it only orders jobs.
//! Optional diagnostic RUSTRED_SPIRED_SYMBOLIC_EXACT_BACKEND=dense-fraction-free
//! selects native dense symbolic lifting (numerical-tail lifting stays sparse).
//! RUSTRED_SPIRED_FRACTION_FREE_MAX_ENTRIES bounds initial dense matrix slots;
//! it does not bound intermediate coefficient memory. Sparse is the default.
//! Outputs are conditional source-port rules
//! and finite search residuals, NOT certified family-closing artifacts.

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rustred::family::IntegralFamily;
use rustred::solver::{
    Integral, IntegralOrder, SearchOptions, SectorConfig, SectorExecutor, SectorScheduling,
    SectorSolution, SectorSolveOptions, SectorStats, prepare_linear_cuts,
};

#[path = "support/spired_exact_backend.rs"]
mod spired_exact_backend;
#[path = "support/spired_families.rs"]
mod spired_families;
#[path = "support/spired_progress.rs"]
mod spired_progress;
#[path = "support/spired_reference.rs"]
mod spired_reference;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

struct Arguments {
    target: String,
    output: PathBuf,
    zero_manifest: Option<PathBuf>,
    nonzero_manifest: Option<PathBuf>,
    reference: Option<PathBuf>,
    oracle_aliases: &'static [spired_reference::CoefficientAlias<'static>],
    symbolic_depth: Option<u32>,
    workers: usize,
    orderings: Option<PathBuf>,
    scheduling: SectorScheduling,
}

/// Only compact summaries survive a sector task in ordinary generation.
/// Full solutions are retained solely for explicitly requested oracle checks.
struct Completed<const N: usize> {
    label: String,
    rules: usize,
    residuals: Vec<Integral<N>>,
    precondition: Duration,
    stats: SectorStats,
    write_time: Duration,
    retained: Option<SectorSolution<N>>,
}

fn mask<const N: usize>(sector: &[bool; N]) -> String {
    sector
        .iter()
        .map(|active| if *active { '1' } else { '0' })
        .collect()
}

fn parse_mask<const N: usize>(input: &str) -> Result<[bool; N]> {
    let bits = input
        .chars()
        .map(|bit| match bit {
            '0' => Ok(false),
            '1' => Ok(true),
            _ => Err(format!("invalid sector bit {bit:?}")),
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    bits.try_into()
        .map_err(|_: Vec<bool>| format!("expected a {N}-bit sector mask").into())
}

fn parse_scheduling(input: Option<&str>) -> Result<SectorScheduling> {
    match input.unwrap_or("active-first") {
        "active-first" => Ok(SectorScheduling::ActiveFirst),
        "input-order" => Ok(SectorScheduling::InputOrder),
        value => {
            Err(format!("invalid schedule {value:?}; expected active-first or input-order").into())
        }
    }
}

fn scheduling_name(scheduling: SectorScheduling) -> &'static str {
    match scheduling {
        SectorScheduling::ActiveFirst => "active-first",
        SectorScheduling::InputOrder => "input-order",
    }
}

fn read_manifest<const N: usize>(path: &Path) -> Result<Vec<[bool; N]>> {
    let text = fs::read_to_string(path)?;
    let bits = text
        .split_whitespace()
        .map(|token| match token {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err(format!(
                "{}: expected a whitespace-separated 0 or 1, got {token:?}",
                path.display()
            )),
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if bits.len() % N != 0 {
        return Err(format!("{}: incomplete {N}-coordinate sector", path.display()).into());
    }
    let mut seen = HashSet::new();
    bits.chunks_exact(N)
        .map(|chunk| {
            let sector: [bool; N] = chunk.try_into().expect("exact-sized chunk");
            if !seen.insert(sector) {
                return Err(
                    format!("{}: duplicate sector {}", path.display(), mask(&sector)).into(),
                );
            }
            Ok(sector)
        })
        .collect()
}

fn writer(path: impl AsRef<Path>) -> std::io::Result<BufWriter<File>> {
    Ok(BufWriter::new(
        OpenOptions::new().write(true).create_new(true).open(path)?,
    ))
}

/// Explicit input data: `sector-mask coordinate-priority...`, one per line.
fn read_orderings<const N: usize>(path: &Path) -> Result<BTreeMap<[bool; N], [usize; N]>> {
    parse_orderings(&fs::read_to_string(path)?, &path.display().to_string())
}

fn parse_orderings<const N: usize>(
    text: &str,
    origin: &str,
) -> Result<BTreeMap<[bool; N], [usize; N]>> {
    let mut orderings = BTreeMap::new();
    for (line, text) in text.lines().enumerate() {
        let text = text.split('#').next().unwrap().trim();
        if text.is_empty() {
            continue;
        }
        let mut fields = text.split_whitespace();
        let sector = parse_mask(fields.next().unwrap())?;
        let permutation: [usize; N] = fields
            .map(str::parse)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_: Vec<usize>| {
                format!(
                    "{}:{}: expected {N} coordinate priorities",
                    origin,
                    line + 1
                )
            })?;
        IntegralOrder::new(sector, [false; N]).with_permutation(permutation)?;
        if orderings.insert(sector, permutation).is_some() {
            return Err(format!(
                "{}:{}: duplicate ordering for {}",
                origin,
                line + 1,
                mask(&sector)
            )
            .into());
        }
    }
    Ok(orderings)
}

#[cfg(test)]
mod input_tests {
    use super::*;

    #[test]
    fn scheduling_parser_preserves_default_and_round_trips_both_policies() {
        assert_eq!(
            parse_scheduling(None).unwrap(),
            SectorScheduling::ActiveFirst
        );
        for policy in [SectorScheduling::ActiveFirst, SectorScheduling::InputOrder] {
            assert_eq!(
                parse_scheduling(Some(scheduling_name(policy))).unwrap(),
                policy
            );
        }
    }

    #[test]
    fn scheduling_parser_rejects_unknown_or_ambiguous_values() {
        for input in ["", "-", "input", "active", "InputOrder", "input-order "] {
            assert!(parse_scheduling(Some(input)).is_err(), "{input:?}");
        }
    }

    #[test]
    fn all_reference_orderings_are_explicit_valid_input() {
        let orderings =
            parse_orderings::<15>(include_str!("support/fam1_112_orderings.txt"), "fixture")
                .unwrap();
        assert_eq!(orderings.len(), 16);
        assert_eq!(
            orderings[&parse_mask("111011010101101").unwrap()],
            [0, 1, 2, 6, 3, 5, 14, 7, 9, 11, 8, 12, 10, 13, 4]
        );
        assert_eq!(
            orderings[&parse_mask("111100101011111").unwrap()],
            [0, 1, 2, 11, 12, 10, 7, 3, 13, 5, 8, 4, 14, 6, 9]
        );
    }

    #[test]
    fn ordering_parser_rejects_invalid_permutations_and_duplicates() {
        for input in [
            "111 0 0 2",
            "111 0 1 3",
            "111 0 1",
            "11x 0 1 2",
            "111 0 1 2\n111 2 1 0",
        ] {
            assert!(parse_orderings::<3>(input, "test").is_err(), "{input}");
        }
        assert_eq!(
            parse_orderings::<3>("# comment\n111 2 0 1 # reverse\n", "test")
                .unwrap()
                .len(),
            1
        );
    }
}

fn write_rules<const N: usize>(path: &Path, solution: &SectorSolution<N>) -> std::io::Result<()> {
    let mut output = writer(path)?;
    writeln!(
        output,
        "# Conditional parametric equations; not a closing artifact."
    )?;
    writeln!(
        output,
        "# Family parameters and preparation conditions are recorded in family.txt."
    )?;
    writeln!(
        output,
        "# Index variables are zero-based. A guard conjunction excludes its zero locus."
    )?;
    for rule in &solution.rules {
        writeln!(output, "target = {}", rule.candidate.target)?;
        if let Some(case) = rule.candidate.case.affine() {
            write!(output, "required = ")?;
            for (position, equation) in case.equations().iter().enumerate() {
                if position != 0 {
                    write!(output, " AND ")?;
                }
                write!(output, "({equation}) = 0")?;
            }
            writeln!(output)?;
        }
        write!(output, "excluded = ")?;
        if rule.exceptions.branches.is_empty() {
            writeln!(output, "false")?;
        } else {
            for (branch_index, branch) in rule.exceptions.branches.iter().enumerate() {
                if branch_index != 0 {
                    write!(output, " OR ")?;
                }
                write!(output, "(")?;
                for (equation_index, equation) in branch.iter().enumerate() {
                    if equation_index != 0 {
                        write!(output, " AND ")?;
                    }
                    write!(output, "({equation}) = 0")?;
                }
                if branch.is_empty() {
                    write!(output, "true")?;
                }
                write!(output, ")")?;
            }
            writeln!(output)?;
        }
        if rule.candidate.rhs.is_empty() {
            writeln!(output, "  0")?;
        }
        for term in &rule.candidate.rhs {
            writeln!(output, "  ({}) * {}", term.coefficient, term.integral)?;
        }
        writeln!(output)?;
    }
    writeln!(
        output,
        "# Fully fixed residuals of the bounded numerical search, not proven independent masters:"
    )?;
    for residual in &solution.finite_residuals {
        writeln!(output, "{residual}")?;
    }
    output.flush()?;
    Ok(())
}

fn run<const N: usize>(
    build_family: impl FnOnce() -> Result<IntegralFamily>,
    removed: [bool; N],
    numerical_depth: u32,
    args: Arguments,
    process_start: Instant,
) -> Result<()> {
    let input_start = Instant::now();
    let symbolic_exact_backend = spired_exact_backend::from_environment()?;
    let zero_sectors = args
        .zero_manifest
        .as_deref()
        .map(read_manifest::<N>)
        .transpose()?
        .unwrap_or_default();
    let sectors = if args.target == "all" {
        read_manifest::<N>(
            args.nonzero_manifest
                .as_deref()
                .ok_or("all requires an explicit nonzero-sector manifest")?,
        )?
    } else {
        vec![parse_mask::<N>(&args.target)?]
    };
    if sectors.is_empty() {
        return Err("the nonzero-sector manifest is empty".into());
    }
    let orderings = args
        .orderings
        .as_deref()
        .map(read_orderings::<N>)
        .transpose()?
        .unwrap_or_default();
    let worker_budget = args.workers.min(sectors.len());
    for sector in &sectors {
        if zero_sectors.contains(sector) {
            return Err(format!("requested sector {} is declared zero", mask(sector)).into());
        }
    }
    if args.output.exists() {
        return Err(format!(
            "output directory already exists: {}; choose a fresh path",
            args.output.display()
        )
        .into());
    }
    fs::create_dir_all(&args.output)?;
    let input_time = input_start.elapsed();
    let preparation_start = Instant::now();
    let family = build_family()?;
    let loops = family.loop_count();
    let prepared = prepare_linear_cuts::<N>(&family, removed, true)?;
    let sources = prepared.sources;
    let preparation = preparation_start.elapsed();
    let mut pre_rules = writer(args.output.join("pre_rules.txt"))?;
    for rule in &prepared.rules {
        writeln!(
            pre_rules,
            "target={} excluded=n{}=1 source_ordinal={}",
            rule.target, rule.axis, rule.source_ordinal
        )?;
        for term in &rule.rhs {
            writeln!(pre_rules, "  ({}) * {}", term.coefficient, term.integral)?;
        }
    }
    pre_rules.flush()?;
    let mut metadata = writer(args.output.join("family.txt"))?;
    writeln!(
        metadata,
        "loops={loops}\nK={N}\nprepared_sources={}",
        sources.rows().len()
    )?;
    writeln!(
        metadata,
        "symbolic_depth={:?}\nnumerical_depth={numerical_depth}\nworkers={worker_budget}\nrequested_workers={}\nschedule={}",
        args.symbolic_depth,
        args.workers,
        scheduling_name(args.scheduling)
    )?;
    writeln!(
        metadata,
        "parameters={:?}\nremoved_cuts={}",
        family.coefficient_context().parameter_names(),
        mask(&removed)
    )?;
    writeln!(
        metadata,
        "symbolic_exact_backend={symbolic_exact_backend:?}"
    )?;
    writeln!(metadata, "coordinates={:?}", family.coordinates())?;
    for (axis, shift) in family.power_shifts().iter().enumerate() {
        writeln!(metadata, "power_shift[{axis}]={shift}")?;
    }
    for (row, entries) in family.external_gram().iter().enumerate() {
        for (column, value) in entries.iter().enumerate() {
            writeln!(metadata, "external_gram[{row},{column}]={value}")?;
        }
    }
    for (axis, denominator) in family.denominators().iter().enumerate() {
        writeln!(
            metadata,
            "D{axis}: constant={} coefficients={:?}",
            denominator.constant(),
            denominator.coefficients()
        )?;
    }
    for sector in &zero_sectors {
        writeln!(metadata, "zero_sector={}", mask(sector))?;
    }
    for sector in &sectors {
        writeln!(metadata, "requested_sector={}", mask(sector))?;
        if let Some(permutation) = orderings.get(sector) {
            writeln!(metadata, "ordering.{}={permutation:?}", mask(sector))?;
        }
    }
    for condition in sources.conditions() {
        writeln!(metadata, "source_nonzero_condition={condition}")?;
    }
    metadata.flush()?;

    let mut stats_file = writer(args.output.join("stats.tsv"))?;
    writeln!(
        stats_file,
        "sector\trules\tresiduals\tprecondition_us\tsolve_us\tsymbolic_cases\tnumeric_cases\tsymbolic_rows\tsymbolic_search_us\texceptions_us\tgeometry_us\tnumeric_search_us\twrite_us"
    )?;
    let mut retained = Vec::new();
    let mut residual_file = writer(args.output.join("residuals.txt"))?;
    writeln!(
        residual_file,
        "# Bounded numerical-search residuals; no independence or family-closure claim."
    )?;
    let mut total_rules = 0;
    let mut total_residuals = 0;
    let mut total_precondition = Duration::ZERO;
    let mut total_solve = Duration::ZERO;
    let progress = std::env::var("RUSTRED_SPIRED_PROGRESS").is_ok_and(|value| value == "1");
    let campaign_start = Instant::now();
    let executor = SectorExecutor::new(worker_budget)?.with_scheduling(args.scheduling);
    let pool_setup = campaign_start.elapsed();
    let config = SectorConfig {
        deltas: removed,
        removed_deltas: removed,
        zero_sectors: zero_sectors.into(),
        symbolic_exact_backend,
        ..Default::default()
    };
    let completed = executor.map_configured_with_observer(
        &sources,
        &sectors,
        |_, sector| SectorConfig {
            permutation: orderings.get(&sector).copied(),
            ..config.clone()
        },
        SectorSolveOptions {
            symbolic: SearchOptions {
                max_depth: args.symbolic_depth,
                ..Default::default()
            },
            numerical_depth,
            max_symbolic_cases: None,
        },
        |_, sector, event| {
            if !progress {
                return;
            }
            let label = mask(&sector);
            spired_progress::write_event(
                &mut std::io::stderr().lock(),
                &label,
                process_start.elapsed(),
                event,
            )
            .expect("progress stderr write failed");
        },
        |completed| -> std::io::Result<Completed<N>> {
            let label = mask(&completed.sector);
            let solution = completed.solution;
            let write_start = Instant::now();
            write_rules(&args.output.join(format!("{label}.rules.txt")), &solution)?;
            let write_time = write_start.elapsed();
            let rules = solution.rules.len();
            let stats = solution.stats;
            let (residuals, retained) = if args.reference.is_some() {
                (solution.finite_residuals.clone(), Some(solution))
            } else {
                (solution.finite_residuals, None)
            };
            Ok(Completed {
                label,
                rules,
                residuals,
                precondition: completed.preconditioning,
                stats,
                write_time,
                retained,
            })
        },
    )?;
    // Canonical aggregate output is independent of worker completion order.
    for (ordinal, completed) in completed.into_iter().enumerate() {
        let Completed {
            label,
            rules,
            residuals,
            precondition,
            stats,
            write_time,
            retained: solution,
        } = completed;
        for residual in &residuals {
            writeln!(residual_file, "{label}\t{residual}")?;
        }
        writeln!(
            stats_file,
            "{label}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            rules,
            residuals.len(),
            precondition.as_micros(),
            stats.elapsed.as_micros(),
            stats.symbolic_cases,
            stats.numerical_cases,
            stats.symbolic_rows,
            stats.symbolic_search.as_micros(),
            stats.exception_extraction.as_micros(),
            stats.geometry.as_micros(),
            stats.numerical_search.as_micros(),
            write_time.as_micros()
        )?;
        stats_file.flush()?;
        total_rules += rules;
        total_residuals += residuals.len();
        total_precondition += precondition;
        total_solve += stats.elapsed;
        println!(
            "sector={label} completed={}/{} rules={} residuals={} precondition_us={} solve_us={}",
            ordinal + 1,
            sectors.len(),
            rules,
            residuals.len(),
            precondition.as_micros(),
            stats.elapsed.as_micros()
        );
        if let Some(solution) = solution {
            retained.push((label, solution));
        }
    }
    residual_file.flush()?;
    stats_file.flush()?;
    let campaign = campaign_start.elapsed();
    // Oracle data is first read only after every sector has independently run.
    // Validation is deliberately outside the timed generation workload.
    let validation_start = Instant::now();
    let mut reference_rules = 0;
    let mut reference_empty_rules = 0;
    let mut reference_covered_rules = 0;
    if let Some(directory) = &args.reference {
        if !prepared.rules.is_empty() {
            let reference = fs::read_to_string(directory.join("preRules.dat"))?;
            spired_reference::compare_pre_rules(&reference, &prepared.rules, &sources)
                .map_err(|error| format!("preliminary cut rules: {error}"))?;
            println!("reference_pre_rule_matches={}", prepared.rules.len());
        }
        for (label, solution) in &retained {
            let sector = parse_mask::<N>(label)?;
            let reference = fs::read_to_string(directory.join(format!("{label}.dat")))?;
            let comparison: spired_reference::SectorComparison =
                spired_reference::compare_sector_with_aliases(
                    &reference,
                    &solution.rules,
                    &sources,
                    &sector,
                    None,
                    args.oracle_aliases,
                )
                .map_err(|error| format!("sector {label}: {error}"))?;
            reference_rules += comparison.matched_rules;
            reference_empty_rules += comparison.integer_empty_rules;
            reference_covered_rules += comparison.covered_reference_rules;
        }
        println!(
            "reference_rule_matches={reference_rules}; reference_integer_empty_rules={reference_empty_rules}; reference_covered_rules={reference_covered_rules}; exact symbolic parameters, required domains, guards, and sector signs; covered reference-only cases have separate exact domain proofs; residuals NOT compared"
        );
    }
    let validation = validation_start.elapsed();
    let mut summary = writer(args.output.join("summary.txt"))?;
    let report = format!(
        "scope=conditional sector rule generation, NOT certified family closure\nloops={loops}\nK={N}\nsectors={}\nrules={total_rules}\nfinite_residuals={total_residuals}\nworkers={}\nschedule={}\ninput_us={}\nsource_preparation_us={}\npool_setup_us={}\nprecondition_sum_us={}\nsector_solve_sum_us={}\ncampaign_including_output_us={}\nreference_rhs_matches={reference_rules}\nreference_validation_us={}\nlogical_process_elapsed_us={}\n",
        sectors.len(),
        executor.workers(),
        scheduling_name(executor.scheduling()),
        input_time.as_micros(),
        preparation.as_micros(),
        pool_setup.as_micros(),
        total_precondition.as_micros(),
        total_solve.as_micros(),
        campaign.as_micros(),
        validation.as_micros(),
        process_start.elapsed().as_micros()
    );
    summary.write_all(report.as_bytes())?;
    writeln!(
        summary,
        "reference_integer_empty_rules={reference_empty_rules}"
    )?;
    writeln!(summary, "reference_covered_rules={reference_covered_rules}")?;
    summary.flush()?;
    print!("{report}");
    println!("output_directory={}", args.output.display());
    Ok(())
}

fn main() -> Result<()> {
    let start = Instant::now();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(4..=10).contains(&args.len()) {
        return Err("usage: spired-solve-sector <family> <sector-mask|all> <new-output-dir> <zero-sectors-file|-> [nonzero-sectors-file|-] [reference-dir|-] [symbolic-depth|unbounded] [workers] [orderings-file|-] [active-first|input-order]".into());
    }
    let optional_path = |position: usize| {
        args.get(position)
            .filter(|value| value.as_str() != "-")
            .map(PathBuf::from)
    };
    let config = Arguments {
        target: args[1].clone(),
        output: PathBuf::from(&args[2]),
        zero_manifest: optional_path(3),
        nonzero_manifest: optional_path(4),
        reference: optional_path(5),
        oracle_aliases: if args[0] == "fam_cosmo" {
            &[spired_reference::CoefficientAlias {
                reference: "dot[p,p]",
                parameter: "s",
            }]
        } else {
            &[]
        },
        symbolic_depth: match args.get(6).map(String::as_str).unwrap_or("3") {
            "unbounded" => None,
            value => Some(value.parse()?),
        },
        workers: args.get(7).map(String::as_str).unwrap_or("1").parse()?,
        orderings: optional_path(8),
        scheduling: parse_scheduling(args.get(9).map(String::as_str))?,
    };
    if config.workers == 0 {
        return Err("workers must be nonzero".into());
    }
    match args[0].as_str() {
        "1" => run::<1>(
            || spired_families::vacuum(&[&[1]]),
            [false; 1],
            3,
            config,
            start,
        ),
        "2" => run::<3>(
            || spired_families::vacuum(&[&[1, 0], &[0, 1], &[1, 1]]),
            [false; 3],
            3,
            config,
            start,
        ),
        "3" => run::<6>(
            || {
                spired_families::vacuum(&[
                    &[1, 0, 0],
                    &[0, 1, 0],
                    &[0, 0, 1],
                    &[1, 1, 0],
                    &[1, 0, 1],
                    &[0, 1, -1],
                ])
            },
            [false; 6],
            3,
            config,
            start,
        ),
        "fam1_11" => run::<9>(
            spired_families::fam1_11,
            [true, true, false, false, false, false, false, false, false],
            2,
            config,
            start,
        ),
        "fam1_12" => run::<9>(
            spired_families::fam1_12,
            std::array::from_fn(|i| i < 2),
            2,
            config,
            start,
        ),
        "fam1_111" => run::<15>(
            spired_families::fam1_111,
            std::array::from_fn(|i| i < 3),
            2,
            config,
            start,
        ),
        "fam1_112" => run::<15>(
            spired_families::fam1_112,
            std::array::from_fn(|i| i < 3),
            3,
            config,
            start,
        ),
        "bc4PMRad1" => run::<7>(spired_families::bc4pm_rad1, [false; 7], 3, config, start),
        "fam_cosmo" => run::<5>(spired_families::fam_cosmo, [false; 5], 3, config, start),
        _ => Err(
            "supported fixtures are vacuum 1/2/3, fam1_11/12/111/112, bc4PMRad1, and fam_cosmo"
                .into(),
        ),
    }
}
