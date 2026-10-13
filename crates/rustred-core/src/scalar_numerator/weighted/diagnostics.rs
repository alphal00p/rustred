//! Manual stage accounting for a native captured input; not acceptance evidence.
use super::*;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::scalar_numerator::FamilyScalarNumeratorService;
use std::path::PathBuf;
use symbolica::parser::ParseSettings;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "manual core-only diagnostic; requires an explicitly captured native weighted input"]
fn inspect_captured_weighted_work_stages() -> Result<()> {
    let directory = PathBuf::from(
        std::env::var_os("RUSTRED_WEIGHTED_DIAGNOSTIC_INPUT")
            .ok_or("set RUSTRED_WEIGHTED_DIAGNOSTIC_INPUT")?,
    );
    let evidence = PathBuf::from(
        std::env::var_os("RUSTRED_WEIGHTED_DIAGNOSTIC_EVIDENCE")
            .ok_or("set RUSTRED_WEIGHTED_DIAGNOSTIC_EVIDENCE")?,
    );
    std::fs::create_dir_all(&evidence)?;
    let read = |name| std::fs::read_to_string(directory.join(name));
    let parse =
        |value: &str| Atom::parse(value, "weighted_work_diagnostic", ParseSettings::default());
    let head_name = read("dot-head.txt")?;
    // The native canonical atom spelling includes its symmetry attributes.
    let head = parse(head_name.trim())?
        .get_symbol()
        .ok_or("captured dot head is not a symbol")?;
    let parameters = read("family-parameters.txt")?;
    let context = CoefficientContext::try_new(parameters.lines())?;
    let coefficient = |value: &str| -> Result<Coefficient> {
        Ok(parse(value)?.try_to_rational_polynomial(&Q, &Z, Some(context.variables().clone()))?)
    };
    let rows = read("family-denominators.tsv")?;
    let denominators = rows
        .lines()
        .map(|row| -> Result<_> {
            let mut fields = row.split('\t');
            let constant = coefficient(fields.next().ok_or("empty denominator row")?)?;
            let coefficients = fields.map(coefficient).collect::<Result<Vec<_>>>()?;
            Ok(AffineDenominator::new(constant, coefficients))
        })
        .collect::<Result<Vec<_>>>()?;
    let family = IntegralFamily::new(
        read("family-name.txt")?,
        read("family-loops.txt")?
            .lines()
            .map(str::to_owned)
            .collect(),
        vec![],
        context.clone(),
        coefficient(&read("family-dimension.symbolica")?)?,
        denominators,
        vec![],
        read("family-shifts.symbolica")?
            .lines()
            .map(coefficient)
            .collect::<Result<Vec<_>>>()?,
    )?;
    assert_eq!(family.fingerprint(), read("family-fingerprint.txt")?);
    let labels = read("loop-labels.symbolica")?
        .lines()
        .map(parse)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let target = IntegralKey::try_new(
        read("powers.txt")?
            .split_whitespace()
            .map(str::parse::<i64>)
            .collect::<std::result::Result<Vec<_>, _>>()?,
    )?;
    let numerator = parse(&read("numerator.symbolica")?)?;
    let mass = parse(&read("mass.symbolica")?)?;
    let mut limits = ScalarNumeratorLimits::default();
    if let Ok(limit) = std::env::var("RUSTRED_WEIGHTED_DIAGNOSTIC_MAX_WORK") {
        limits.exact_algebra.max_term_operations = limit.parse()?;
    }
    let service = FamilyScalarNumeratorService::try_new(&family, head, labels, limits)?;
    let mut work = bounds::Work::new(limits);
    work.enable_diagnostic();
    let started = std::time::Instant::now();
    let outcome = service.lower_weighted_in_request(&numerator, &target, &mass, &mut work);
    let trace = work.diagnostic().unwrap();
    let mut report = format!(
        "diagnostic_only=true\nmax_work={}\nelapsed_seconds={}\nlast_phase={}\nlast_key={:?}\n",
        limits.exact_algebra.max_term_operations,
        started.elapsed().as_secs_f64(),
        trace.phase,
        trace.last_key
    );
    for (phase, count) in &trace.counts {
        report.push_str(&format!("phase={phase}\tcharged_work={count}\n"));
    }
    match outcome {
        Ok(result) => report.push_str(&format!(
            "outcome=success\nterms={}\nconditions={}\n",
            result.terms().len(),
            result.nonzero_conditions().len()
        )),
        Err(error) => report.push_str(&format!("outcome=error\nerror={error:?}\n")),
    }
    if let Some(weight) = &trace.last_weight {
        std::fs::write(
            evidence.join("last-weight.symbolica"),
            weight.to_canonical_string(),
        )?;
    }
    std::fs::write(evidence.join("stage-work.txt"), &report)?;
    print!("{report}");
    Ok(())
}
