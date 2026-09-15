//! Autonomous vacuum generation followed by independent cold diagnostics.
//! Usage: spired-artifact-audit <1|2|3> [workers]
//! No artifact is installed or written by this diagnostic example.
//! Shares the solver example's optional symbolic exact-backend environment
//! settings. Sparse is the default; numerical-tail lifting remains sparse.

use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use rustred::family::IntegralFamily;
use rustred::foundry::artifact::SourcePortAudit;
use rustred::sector::{Mask, zero};
use rustred::solver::{
    CoefficientVariableOrder, SectorConfig, SectorExecutor, SectorSolveOptions, SourceSystem,
    SymbolicExactBackend,
};

#[path = "support/spired_exact_backend.rs"]
mod spired_exact_backend;
#[allow(dead_code)]
#[path = "support/spired_families.rs"]
mod spired_families;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn run<const N: usize>(
    family: IntegralFamily,
    workers: usize,
    symbolic_exact_backend: SymbolicExactBackend,
    coefficient_variable_order: CoefficientVariableOrder,
) -> Result<()> {
    let start = Instant::now();
    println!("# symbolic_exact_backend={symbolic_exact_backend:?}");
    println!("# coefficient_variable_order={coefficient_variable_order:?}");
    let zero_analyzer = zero::Analyzer::try_unrestricted(&family)?;
    let mut zeros = Vec::new();
    let mut sectors = Vec::new();
    for bits in 0..(1_usize << N) {
        let sector = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        match zero_analyzer.analyze(&Mask::try_new(sector)?)? {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
            zero::Decision::Inconclusive(_) => sectors.push(sector),
            zero::Decision::Excluded(_) => {
                return Err("unrestricted census unexpectedly excluded a sector".into());
            }
        }
    }
    sectors.sort_unstable();
    let zeros: Arc<[[bool; N]]> = Arc::from(zeros);
    let sources = SourceSystem::from_family(&family)?;
    let audit = SourcePortAudit::try_new(&family, zeros.clone())?;
    let prepared = start.elapsed();
    let executor = SectorExecutor::new(workers)?;
    let reports = executor.map(
        &sources, &sectors, &SectorConfig { zero_sectors: zeros, symbolic_exact_backend, coefficient_variable_order, ..Default::default() },
        SectorSolveOptions { numerical_depth: 3, ..Default::default() },
        |done| {
            let report = audit.audit_sector(done.sector, None, &done.solution)?;
            eprintln!("audited {:?}: rules={} replayed={} descending={} stored_gaps={} checked_gaps={} extra_guards={}",
                done.sector, report.rules, report.exact_replayed_rules, report.uniformly_descending_rules,
                report.stored_guard_uncovered_boxes, report.checked_rule_uncovered_boxes, report.additional_replay_guard_branches);
            Ok::<_, rustred::foundry::artifact::SourcePortAuditError>(report)
        },
    )?;
    audit.validate_sector_census(&reports)?;
    println!(
        "# Diagnostic only; no ClosedArtifact authority. Common squared mass remains symbolic m."
    );
    println!(
        "sector\trules\treplayed\tdescending\tterminals\textra_guards\tstored_gaps\tchecked_gaps\tchecked_unbounded\taudit_us"
    );
    let mut issues = 0;
    let mut incomplete = false;
    for report in &reports {
        let label: String = report
            .sector
            .iter()
            .map(|active| if *active { '1' } else { '0' })
            .collect();
        println!(
            "{label}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            report.rules,
            report.exact_replayed_rules,
            report.uniformly_descending_rules,
            report.finite_terminals,
            report.additional_replay_guard_branches,
            report.stored_guard_uncovered_boxes,
            report.checked_rule_uncovered_boxes,
            report.checked_rule_unbounded_boxes,
            report.elapsed.as_micros()
        );
        for issue in &report.issues {
            eprintln!("{label}: {issue}");
            issues += 1;
        }
        incomplete |= report.checked_rule_uncovered_boxes != 0;
    }
    println!(
        "# sectors={} zero_sectors={} original_sources={} rules={} terminals={} issues={} prepared_us={} full_us={} workers={}",
        reports.len(),
        audit.proved_zero_sector_count(),
        audit.original_source_count(),
        reports.iter().map(|r| r.rules).sum::<usize>(),
        reports.iter().map(|r| r.finite_terminals).sum::<usize>(),
        issues,
        prepared.as_micros(),
        start.elapsed().as_micros(),
        workers
    );
    if incomplete || issues != 0 {
        return Err(
            "cold diagnostic found unresolved obligations; no artifact was published".into(),
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args.len() > 2 {
        return Err("usage: spired-artifact-audit <1|2|3> [workers]".into());
    }
    let workers = args
        .get(1)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1);
    let symbolic_exact_backend = spired_exact_backend::from_environment()?;
    let coefficient_variable_order = spired_exact_backend::coefficient_order_from_environment()?;
    match args[0].as_str() {
        "1" => run::<1>(
            spired_families::vacuum(&[&[1]])?,
            workers,
            symbolic_exact_backend,
            coefficient_variable_order,
        ),
        "2" => run::<3>(
            spired_families::vacuum(&[&[1, 0], &[0, 1], &[1, 1]])?,
            workers,
            symbolic_exact_backend,
            coefficient_variable_order,
        ),
        "3" => run::<6>(
            spired_families::vacuum(&[
                &[1, 0, 0],
                &[0, 1, 0],
                &[0, 0, 1],
                &[1, 1, 0],
                &[1, 0, 1],
                &[0, 1, -1],
            ])?,
            workers,
            symbolic_exact_backend,
            coefficient_variable_order,
        ),
        _ => Err("expected vacuum loop count 1, 2 or 3".into()),
    }
}
