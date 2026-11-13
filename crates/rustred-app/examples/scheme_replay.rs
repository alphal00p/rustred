//! Scheme-rule replay (M5 A1 schemes): the numeric-index-point verification of a
//! Scheme rule by an independent, verified RustRed p library (the oracle).
//!
//! A Scheme rule is a relation `Sum_s r_s(d, n) I(n + s) = 0` on a domain of `n`.
//! At a numeric point `n0` of the domain every `I(n0 + s)` is rewritten (outside
//! this program, tools/replay_points.py) onto keys of the oracle's owner
//! presentation of the same node, so the relation becomes one linear combination
//! `Sum_k c_k(d) I_O(k)` with coefficients in Q(d). The oracle reduces it onto its
//! masters EXACTLY in Q(d) (modular polynomial lift): the rule holds at `n0` iff
//! the reduction is empty. A mutated copy (one coefficient changed) must give a
//! non-empty reduction, so the check has teeth.
//!
//! usage: scheme_replay <oracle p-library dir> <checks.json> <out.json> [threads]
//!
//! checks.json: `{"presentation": <oracle presentation id>, "checks": [{"name": ..,
//!   "inputs": [[key], [[d exponent, "p/q"], ..] numerator, [[e, "p/q"], ..] denominator], ..]}, ..]}`.
//! out.json: per check: name, inputs, status, nonzero terms, seconds.
//!
//! scheme-certs (rank check): a check may instead carry `"terms": [{"shift": [..], "inputs": [..]}, ..]`,
//! one entry per term `I(n0 + s)` of the rule with unit coefficient; each term is reduced onto the
//! oracle's masters separately and out.json gets, per term, every master coefficient (`"reductions"`,
//! Symbolica expressions in d), for the rank of the term images and the rule's kernel check
//! (SCRATCHPAD/scheme-certs/tools/replay_ext.py).
use std::path::PathBuf;
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::family::IntegralKey;
use rustred::propagator::PresentationId;
use rustred::propagator::library::{PReductionRequest, PUncoveredPolicy};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionLimits,
};
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

/// A decimal integer of any size, by 18-digit chunks.
fn integer(context: &CoefficientContext, text: &str) -> Coefficient {
    let limits = ReductionLimits::default().exact_algebra;
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let chunk = context.integer(1_000_000_000_000_000_000);
    let mut value = context.zero();
    let head = digits.len() % 18;
    let mut pieces = Vec::new();
    if head > 0 {
        pieces.push(&digits[..head]);
    }
    pieces.extend((head..digits.len()).step_by(18).map(|i| &digits[i..i + 18]));
    for piece in pieces {
        value = context.try_mul(&value, &chunk, limits).unwrap();
        let part = context.integer(piece.parse::<i64>().unwrap());
        value = context.try_add(&value, &part, limits).unwrap();
    }
    if negative {
        context.try_neg(&value, limits).unwrap()
    } else {
        value
    }
}

fn rational(context: &CoefficientContext, text: &str) -> Coefficient {
    let limits = ReductionLimits::default().exact_algebra;
    let (n, d) = text.split_once('/').unwrap_or((text, "1"));
    context
        .try_div(&integer(context, n), &integer(context, d), limits)
        .unwrap()
}

fn poly(context: &CoefficientContext, record: &serde_json::Value) -> Coefficient {
    let limits = ReductionLimits::default().exact_algebra;
    let d = context.parameter("d").unwrap();
    let mut total = context.zero();
    for term in record.as_array().unwrap() {
        let exponent = term[0].as_u64().unwrap();
        let mut value = rational(context, term[1].as_str().unwrap());
        for _ in 0..exponent {
            value = context.try_mul(&value, &d, limits).unwrap();
        }
        total = context.try_add(&total, &value, limits).unwrap();
    }
    total
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() >= 4,
        "usage: scheme_replay <oracle lib> <checks.json> <out.json> [threads]"
    );
    let threads: usize = args.get(4).map_or(1, |t| t.parse().unwrap());
    let t_load = Instant::now();
    let library = load_p_library(
        PLibrarySource::Directory(PathBuf::from(&args[1])),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap();
    eprintln!(
        "[replay] oracle loaded in {:.2} s",
        t_load.elapsed().as_secs_f64()
    );
    let spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
    let presentation =
        PresentationId(u32::try_from(spec["presentation"].as_u64().unwrap()).unwrap());
    let context = library.coefficient_context().clone();
    let limits = ReductionLimits::default().exact_algebra;
    let mut results = Vec::new();
    let reduce = |inputs: Vec<(PresentationId, IntegralKey, Coefficient)>| {
        library.reduce(PReductionRequest {
            inputs,
            arithmetic: OrderedWeightArithmetic::ModularPolynomial(
                ModularCommon {
                    threads,
                    ..ModularCommon::default()
                },
                PolynomialOptions::default(),
            ),
            scheduler: OrderedSchedulerOptions::with_threads(threads),
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        })
    };
    for check in spec["checks"].as_array().unwrap() {
        let name = check["name"].as_str().unwrap().to_owned();
        if let Some(terms) = check.get("terms").and_then(|t| t.as_array()) {
            let started = Instant::now();
            let mut images = Vec::new();
            let mut status = "ok".to_owned();
            for term in terms {
                let mut inputs = Vec::new();
                for input in term["inputs"].as_array().unwrap() {
                    let key: Vec<i64> = (input[0].as_array().unwrap().iter())
                        .map(|v| v.as_i64().unwrap())
                        .collect();
                    let value = context
                        .try_div(
                            &poly(&context, &input[1]),
                            &poly(&context, &input[2]),
                            limits,
                        )
                        .unwrap();
                    inputs.push((presentation, IntegralKey::try_new(key).unwrap(), value));
                }
                let reductions: Vec<serde_json::Value> = if inputs.is_empty() {
                    Vec::new()
                } else {
                    match reduce(inputs) {
                        Ok(reduction) => (reduction.terms.iter())
                            .filter(|t| !t.coefficient.is_zero())
                            .map(|t| {
                                serde_json::json!([
                                    format!("{:?}/{:?}", t.basis, t.tag),
                                    t.coefficient.to_expression().to_string()
                                ])
                            })
                            .collect(),
                        Err(error) => {
                            status = format!("error: {error}");
                            Vec::new()
                        }
                    }
                };
                images.push(serde_json::json!({"shift": term["shift"], "reductions": reductions}));
            }
            let seconds = started.elapsed().as_secs_f64();
            eprintln!(
                "[replay] {name}: {} terms reduced separately, {status}, {seconds:.2} s",
                terms.len()
            );
            results.push(serde_json::json!({"name": name, "status": status, "terms": images, "seconds": seconds}));
            std::fs::write(&args[3], serde_json::to_string_pretty(&results).unwrap()).unwrap();
            continue;
        }
        let mut inputs = Vec::new();
        for input in check["inputs"].as_array().unwrap() {
            let key: Vec<i64> = (input[0].as_array().unwrap().iter())
                .map(|v| v.as_i64().unwrap())
                .collect();
            let num = poly(&context, &input[1]);
            let den = poly(&context, &input[2]);
            let value = context.try_div(&num, &den, limits).unwrap();
            inputs.push((presentation, IntegralKey::try_new(key).unwrap(), value));
        }
        let started = Instant::now();
        let result = library.reduce(PReductionRequest {
            inputs: inputs.clone(),
            arithmetic: OrderedWeightArithmetic::ModularPolynomial(
                ModularCommon {
                    threads,
                    ..ModularCommon::default()
                },
                PolynomialOptions::default(),
            ),
            scheduler: OrderedSchedulerOptions::with_threads(threads),
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        });
        let seconds = started.elapsed().as_secs_f64();
        let (status, nonzero) = match &result {
            Ok(reduction) => (
                "ok".to_owned(),
                reduction
                    .terms
                    .iter()
                    .filter(|t| !t.coefficient.is_zero())
                    .count(),
            ),
            Err(error) => (format!("error: {error}"), usize::MAX),
        };
        eprintln!(
            "[replay] {name}: {} inputs, {status}, {} nonzero terms, {seconds:.2} s",
            inputs.len(),
            if nonzero == usize::MAX {
                -1
            } else {
                nonzero as i64
            }
        );
        results.push(serde_json::json!({
            "name": name, "inputs": inputs.len(), "status": status,
            "nonzero_terms": if nonzero == usize::MAX { serde_json::Value::Null } else { nonzero.into() },
            "seconds": seconds,
        }));
        std::fs::write(&args[3], serde_json::to_string_pretty(&results).unwrap()).unwrap();
    }
}
