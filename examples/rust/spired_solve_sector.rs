//! End-to-end executable-reference sector benchmark with a symbolic mass.
//!
//! Usage:
//! spired-solve-sector <1|2|3> <sector-mask|all> <new-output-directory>
//!   <zero-sectors-file|-> [nonzero-sectors-file|-] [reference-directory|-]
//!   [symbolic-depth|unbounded]
//!
//! Manifest files contain whitespace-separated 0/1 entries, N per sector,
//! exactly as the original SpIRed examples. `all` requires the nonzero manifest.
//! Set RUSTRED_SPIRED_PROGRESS=1 for per-case progress. The default symbolic
//! depth limit is three; it is diagnostic, and exhaustion returns an error.
//! Numerical cases use depth three. Outputs are conditional source-port rules
//! and finite search residuals, NOT certified family-closing artifacts.

use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::solver::{
    SearchOptions, SectorConfig, SectorEvent, SectorSolution, SectorSolveOptions, SectorSolver,
    SourceSystem,
};

#[path = "support/spired_reference.rs"]
mod spired_reference;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Arguments {
    target: String,
    output: PathBuf,
    zero_manifest: Option<PathBuf>,
    nonzero_manifest: Option<PathBuf>,
    reference: Option<PathBuf>,
    symbolic_depth: Option<u32>,
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

fn writer(path: impl AsRef<Path>) -> Result<BufWriter<File>> {
    Ok(BufWriter::new(
        OpenOptions::new().write(true).create_new(true).open(path)?,
    ))
}

fn write_rules<const N: usize>(path: &Path, solution: &SectorSolution<N>) -> Result<()> {
    let mut output = writer(path)?;
    writeln!(
        output,
        "# Conditional parametric equations; not a closing artifact."
    )?;
    writeln!(
        output,
        "# Generic coefficient parameters d,m; denominators are q^2-m."
    )?;
    writeln!(
        output,
        "# Index variables are zero-based. A guard conjunction excludes its zero locus."
    )?;
    for rule in &solution.rules {
        writeln!(output, "target = {}", rule.candidate.target)?;
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

fn run<const N: usize>(momenta: &[&[i64]], args: Arguments, process_start: Instant) -> Result<()> {
    let input_start = Instant::now();
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
    let loops = momenta[0].len();
    let coefficients = CoefficientContext::try_new(["d", "m"])?;
    let mass = coefficients
        .parameter("m")
        .expect("declared common squared mass");
    let denominators = momenta
        .iter()
        .map(|momentum| {
            let row = (0..loops)
                .flat_map(|i| {
                    (i..loops).map(move |j| momentum[i] * momentum[j] * if i == j { 1 } else { 2 })
                })
                .map(|value| coefficients.integer(value))
                .collect();
            AffineDenominator::new(-mass.clone(), row)
        })
        .collect();
    let family = IntegralFamily::new(
        "spired-vacuum-sector",
        (0..loops).map(|i| format!("k{i}")).collect(),
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![coefficients.zero(); N],
    )?;
    let sources = SourceSystem::<N>::from_family(&family)?;
    let preparation = preparation_start.elapsed();
    let mut metadata = writer(args.output.join("family.txt"))?;
    writeln!(
        metadata,
        "loops={loops}\nK={N}\nparameters=d,m\ndenominator=q^2-m\nordinary_sources={}",
        sources.rows().len()
    )?;
    writeln!(
        metadata,
        "symbolic_depth={:?}\nnumerical_depth=3\nworkers=1",
        args.symbolic_depth
    )?;
    for (axis, momentum) in momenta.iter().enumerate() {
        writeln!(metadata, "q{axis}={momentum:?}")?;
    }
    for sector in &zero_sectors {
        writeln!(metadata, "zero_sector={}", mask(sector))?;
    }
    for sector in &sectors {
        writeln!(metadata, "requested_sector={}", mask(sector))?;
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
    for (ordinal, sector) in sectors.iter().enumerate() {
        let label = mask(sector);
        let start = Instant::now();
        let solver = SectorSolver::new(
            &sources,
            *sector,
            SectorConfig {
                zero_sectors: zero_sectors.clone(),
                ..Default::default()
            },
        )?;
        let precondition = start.elapsed();
        let solution = solver.solve_sector_with_observer(
            SectorSolveOptions {
                symbolic: SearchOptions {
                    max_depth: args.symbolic_depth,
                    ..Default::default()
                },
                numerical_depth: 3,
                max_symbolic_cases: None,
            },
            |event| {
                if !progress {
                    return;
                }
                match event {
                    SectorEvent::CaseStarted { case, pending } => {
                        eprintln!("sector={label} case={} pending={pending}", case.integral())
                    }
                    SectorEvent::RuleFound { rule, pending } => eprintln!(
                        "sector={label} solved={} rhs_terms={} rows={} pending={pending}",
                        rule.candidate.target,
                        rule.candidate.rhs.len(),
                        rule.candidate.stats.rows
                    ),
                    SectorEvent::NumericalStarted { cases } => {
                        eprintln!("sector={label} numerical_cases={}", cases.len())
                    }
                }
            },
        )?;
        let write_start = Instant::now();
        write_rules(&args.output.join(format!("{label}.rules.txt")), &solution)?;
        for residual in &solution.finite_residuals {
            writeln!(residual_file, "{label}\t{residual}")?;
        }
        let write_time = write_start.elapsed();
        let stats = &solution.stats;
        writeln!(
            stats_file,
            "{label}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            solution.rules.len(),
            solution.finite_residuals.len(),
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
        total_rules += solution.rules.len();
        total_residuals += solution.finite_residuals.len();
        total_precondition += precondition;
        total_solve += stats.elapsed;
        println!(
            "sector={label} completed={}/{} rules={} residuals={} precondition_us={} solve_us={}",
            ordinal + 1,
            sectors.len(),
            solution.rules.len(),
            solution.finite_residuals.len(),
            precondition.as_micros(),
            stats.elapsed.as_micros()
        );
        if args.reference.is_some() {
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
    if let Some(directory) = &args.reference {
        for (label, solution) in &retained {
            let sector = parse_mask::<N>(label)?;
            let reference = fs::read_to_string(directory.join(format!("{label}.dat")))?;
            let count = reference.matches("->").count();
            if count != solution.rules.len() {
                return Err(format!("sector {label}: Rust produced {} rules, reference has {count}; generated outputs retained", solution.rules.len()).into());
            }
            let mut cases = HashSet::new();
            for rule in &solution.rules {
                if !cases.insert(*rule.candidate.case.fixed()) {
                    return Err(format!("sector {label}: duplicate Rust coordinate case").into());
                }
                spired_reference::compare_rule(&reference, rule, &sources, &sector, None).map_err(
                    |error| format!("sector {label}, target {}: {error}", rule.candidate.target),
                )?;
                reference_rules += 1;
            }
        }
        println!(
            "reference_rule_matches={reference_rules}; exact symbolic m, coordinate guards, and sector signs; residuals NOT compared"
        );
    }
    let validation = validation_start.elapsed();
    let mut summary = writer(args.output.join("summary.txt"))?;
    let report = format!(
        "scope=conditional sector rule generation, NOT certified family closure\nloops={loops}\nK={N}\nsectors={}\nrules={total_rules}\nfinite_residuals={total_residuals}\nworkers=1\ninput_us={}\nsource_preparation_us={}\nprecondition_sum_us={}\nsector_solve_sum_us={}\ncampaign_including_output_us={}\nreference_rhs_matches={reference_rules}\nreference_validation_us={}\nlogical_process_elapsed_us={}\n",
        sectors.len(),
        input_time.as_micros(),
        preparation.as_micros(),
        total_precondition.as_micros(),
        total_solve.as_micros(),
        campaign.as_micros(),
        validation.as_micros(),
        process_start.elapsed().as_micros()
    );
    summary.write_all(report.as_bytes())?;
    summary.flush()?;
    print!("{report}");
    println!("output_directory={}", args.output.display());
    Ok(())
}

fn main() -> Result<()> {
    let start = Instant::now();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(4..=7).contains(&args.len()) {
        return Err("usage: spired-solve-sector <1|2|3> <sector-mask|all> <new-output-dir> <zero-sectors-file|-> [nonzero-sectors-file|-] [reference-dir|-] [symbolic-depth|unbounded]".into());
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
        symbolic_depth: match args.get(6).map(String::as_str).unwrap_or("3") {
            "unbounded" => None,
            value => Some(value.parse()?),
        },
    };
    match args[0].as_str() {
        "1" => run::<1>(&[&[1]], config, start),
        "2" => run::<3>(&[&[1, 0], &[0, 1], &[1, 1]], config, start),
        "3" => run::<6>(
            &[
                &[1, 0, 0],
                &[0, 1, 0],
                &[0, 0, 1],
                &[1, 1, 0],
                &[1, 0, 1],
                &[0, 1, -1],
            ],
            config,
            start,
        ),
        _ => Err("supported fixture loop counts are 1, 2, and 3".into()),
    }
}
