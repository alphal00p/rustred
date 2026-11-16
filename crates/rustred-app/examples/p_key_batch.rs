//! Lane basis-census: exact per-key reductions of many keys with ONE library load and a shared in-memory step cache.
//!
//! usage: p_key_batch <p-library dir> <keys.tsv> <exact|modular_sampled|modular_polynomial> <out.txt> [threads]
//!   keys.tsv: one key per line: `<top family name | pres:<presentation id>>\t<comma separated slot powers>`
//!   (lines starting with '#' are skipped). Every key is reduced ALONE (its own request, onto the masters).
//! out.txt: per key `key <n> <top> <powers>`, the rustred.p-reduction.canonical.v1 text of its reduction (or
//! `error <message>`), and `end <n> <seconds>`.
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rustred::family::IntegralKey;
use rustred::propagator::library::{PReductionRequest, PUncoveredPolicy};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionLimits, SampledOptions,
};
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

#[path = "p_path_census/canonical.rs"]
mod canonical;

fn arithmetic(name: &str, threads: usize) -> OrderedWeightArithmetic {
    let common = ModularCommon {
        threads,
        ..ModularCommon::default()
    };
    match name {
        "exact" => OrderedWeightArithmetic::Exact,
        "modular_polynomial" => {
            OrderedWeightArithmetic::ModularPolynomial(common, PolynomialOptions::default())
        }
        "modular_sampled" => {
            OrderedWeightArithmetic::ModularSampled(common, SampledOptions::default())
        }
        other => panic!("arithmetic {other}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() >= 5,
        "usage: p_key_batch <p-library dir> <keys.tsv> <arith> <out.txt> [threads]"
    );
    let library = load_p_library(
        PLibrarySource::Directory(PathBuf::from(&args[1])),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap();
    let arith = args[3].as_str();
    let threads: usize = args.get(5).map_or(1, |t| t.parse().unwrap());
    let binding = library.step_cache_binding().expect("step cache binding");
    let cache = rustred::reduction::OrderedStepCache::in_memory(
        binding,
        rustred::reduction::OrderedStepCacheOptions {
            populate: true,
            ..Default::default()
        },
    );
    let mut out = std::io::BufWriter::new(std::fs::File::create(&args[4]).unwrap());
    let context = library.coefficient_context();
    for (n, line) in std::fs::read_to_string(&args[2])
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .enumerate()
    {
        let (top, powers) = line.split_once('\t').expect("<top>\\t<powers>");
        let powers: Vec<i64> = powers.split(',').map(|p| p.trim().parse().unwrap()).collect();
        writeln!(out, "key {n} {top} {}", powers.iter().map(i64::to_string).collect::<Vec<_>>().join(",")).unwrap();
        let started = Instant::now();
        let by_id = top
            .strip_prefix("pres:")
            .map(|id| rustred::propagator::PresentationId(id.parse().unwrap()));
        let Some(id) = by_id.or_else(|| library.top_presentation(top)) else {
            writeln!(out, "error unknown top {top}").unwrap();
            writeln!(out, "end {n} 0").unwrap();
            continue;
        };
        let request = PReductionRequest {
            inputs: vec![(id, IntegralKey::try_new(powers).unwrap(), context.one())],
            arithmetic: arithmetic(arith, threads),
            scheduler: OrderedSchedulerOptions::with_threads(threads),
            step_cache: if std::env::var("P_KEY_BATCH_NO_STEP_CACHE").is_ok() { None } else { Some(cache.clone()) },
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        };
        match library.reduce(request) {
            Ok(reduction) => out
                .write_all(canonical::canonical_text(&reduction).as_bytes())
                .unwrap(),
            Err(error) => writeln!(out, "error {}", format!("{error}").replace('\n', " ")).unwrap(),
        }
        writeln!(out, "end {n} {:.3}", started.elapsed().as_secs_f64()).unwrap();
        out.flush().unwrap();
    }
}
