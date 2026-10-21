//! Reconstruction API diagnostic, NOT a semi-numerical refinement backend.
//! Usage: refinement_reconstruction_probe NATIVE_STATE.rrbin COUNT MAX_PRIMES
//! Recover existing nonconstant terminal coefficients from native finite-field
//! evaluations and compare exactly. The oracle here evaluates an already known
//! coefficient: these times are not end-to-end elimination speedups.
use rustred::reduction::terminal_relations::{TerminalRelationLimits, TerminalRelationSession};
use std::time::Instant;
use symbolica::domains::finite_field::{ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring};
use symbolica::poly::reconstruction::{
    ReconstructionMethod, ReconstructionOptions, reconstruct_rational_function_over_q,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("expected NATIVE_STATE.rrbin COUNT MAX_PRIMES".into());
    }
    let count: usize = args[1].parse()?;
    let max_primes: usize = args[2].parse()?;
    let limits = TerminalRelationLimits {
        max_seeds: usize::MAX,
        max_columns: usize::MAX,
        max_rows: usize::MAX,
        max_nonzeros: usize::MAX,
        ..Default::default()
    };
    let state = TerminalRelationSession::from_native_bytes(
        &std::fs::read(&args[0])?,
        limits,
        Default::default(),
    )?;
    let mut coefficients = Vec::new();
    for terms in state.terminal_rules().values() {
        for value in terms.values() {
            if !value.is_constant() && !coefficients.contains(value) {
                coefficients.push(value.clone());
            }
        }
    }
    coefficients.sort_by_key(|value| {
        std::cmp::Reverse(value.numerator.nterms() + value.denominator.nterms())
    });
    for (index, value) in coefficients.into_iter().take(count).enumerate() {
        let variables = value.numerator.variables().clone();
        let degree = (0..variables.len())
            .map(|i| value.numerator.degree(i).max(value.denominator.degree(i)))
            .max()
            .unwrap_or(0);
        let options = ReconstructionOptions {
            max_degree: degree.saturating_add(2).max(1),
            ..Default::default()
        };
        for method in [
            ReconstructionMethod::Automatic,
            ReconstructionMethod::HuMonagan,
            ReconstructionMethod::CuytLee,
        ] {
            let started = Instant::now();
            let lifted = reconstruct_rational_function_over_q(
                variables.clone(),
                |field: &Zp64, point| {
                    let denominator = value.denominator.evaluate_with_coeff_map(
                        |c| c.to_finite_field(field),
                        point,
                        field,
                    );
                    if field.is_zero(&denominator) {
                        return None;
                    }
                    let numerator = value.numerator.evaluate_with_coeff_map(
                        |c| c.to_finite_field(field),
                        point,
                        field,
                    );
                    Some(field.div(&numerator, &denominator))
                },
                method,
                &options,
                max_primes,
            );
            match lifted {
                Ok((result, stats)) => {
                    let exact = result == value;
                    println!(
                        "{{\"coefficient\":{index},\"method\":\"{method:?}\",\"variables\":{},\"degree\":{degree},\"terms\":{},\"seconds\":{},\"probes\":{},\"primes\":{},\"exact_equal\":{exact}}}",
                        variables.len(),
                        value.numerator.nterms() + value.denominator.nterms(),
                        started.elapsed().as_secs_f64(),
                        stats.probes,
                        stats.primes
                    );
                    if !exact {
                        return Err(
                            "native reconstruction disagrees with exact saved coefficient".into(),
                        );
                    }
                }
                Err(error) => println!(
                    "{{\"coefficient\":{index},\"method\":\"{method:?}\",\"error\":{:?}}}",
                    error.to_string()
                ),
            }
        }
    }
    Ok(())
}
