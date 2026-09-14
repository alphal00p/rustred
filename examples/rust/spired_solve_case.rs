//! A single coordinate-case probe, not a complete sector/family campaign.
use std::error::Error;
use std::time::Instant;

use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::solver::{CoordinateCase, SearchOptions, SectorConfig, SectorSolver, SourceSystem};

#[path = "support/spired_reference.rs"]
mod spired_reference;

fn run<const N: usize>(
    momenta: &[&[i64]],
    fixed: Option<&str>,
    reference: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    let loops = momenta[0].len();
    let coefficients = CoefficientContext::try_new(["d"])?;
    let denominators = momenta
        .iter()
        .map(|momentum| {
            let row = (0..loops)
                .flat_map(|i| {
                    (i..loops).map(move |j| momentum[i] * momentum[j] * if i == j { 1 } else { 2 })
                })
                .map(|x| coefficients.integer(x))
                .collect();
            AffineDenominator::new(coefficients.integer(-1), row)
        })
        .collect();
    let family = IntegralFamily::new(
        "spired-vacuum-case",
        (0..loops).map(|i| format!("k{i}")).collect(),
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![coefficients.zero(); N],
    )?;
    let sources = SourceSystem::<N>::from_family(&family)?;
    let preparation = start.elapsed();
    let start = Instant::now();
    let solver = SectorSolver::new(&sources, [true; N], SectorConfig::default())?;
    let preconditioning = start.elapsed();
    let fixed = if let Some(input) = fixed {
        let values = input
            .split(',')
            .map(|x| {
                if x == "*" {
                    Ok(None)
                } else {
                    x.parse::<i16>().map(Some)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        <[Option<i16>; N]>::try_from(values)
            .map_err(|_| format!("expected {N} comma-separated coordinates"))?
    } else {
        [None; N]
    };
    let candidate = solver.solve_case(
        CoordinateCase::new(fixed)?,
        SearchOptions {
            max_depth: Some(3),
            ..Default::default()
        },
    )?;
    println!("scope = one coordinate case; NOT sector or family closure");
    println!("K = {N}, ordinary_sources = {}", sources.rows().len());
    println!(
        "preparation_us = {}, preconditioning_us = {}",
        preparation.as_micros(),
        preconditioning.as_micros()
    );
    println!(
        "case_search_us = {}, direct_hit = {}, seeds = {}, rows = {}, exact_trace_rows = {}",
        candidate.stats.elapsed.as_micros(),
        candidate.stats.direct_hit,
        candidate.stats.seeds,
        candidate.stats.rows,
        candidate.stats.exact_trace_rows
    );
    println!("target = {}", candidate.target);
    for term in &candidate.rhs {
        println!("  {} * {}", term.coefficient, term.integral);
    }
    if let Some(path) = reference {
        let reference = std::fs::read_to_string(path)?;
        spired_reference::compare(&reference, &candidate, &sources, Some("m"))?;
        println!("reference_rhs = exact match after m=1; guards NOT compared");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let fixed = args.get(1).map(String::as_str);
    let reference = args.get(2).map(String::as_str);
    match args.first().map(String::as_str).unwrap_or("1") {
        "1" => run::<1>(&[&[1]], fixed, reference),
        "2" => run::<3>(&[&[1, 0], &[0, 1], &[1, 1]], fixed, reference),
        "3" => run::<6>(
            &[
                &[1, 0, 0],
                &[0, 1, 0],
                &[0, 0, 1],
                &[1, 1, 0],
                &[1, 0, 1],
                &[0, 1, -1],
            ],
            fixed, reference,
        ),
        _ => Err("usage: spired-solve-case <loops: 1|2|3> [fixed coordinates, e.g. 1,*,*] [reference .dat]".into()),
    }
}
