//! Lane basis-census: exact per-key reductions of many keys with ONE library load and a shared in-memory step cache.
//!
//! usage: p_key_batch <p-library dir> <keys.tsv> <arithmetic> <out.txt> [threads]
//!   <arithmetic>: the canonical text of `OrderedWeightArithmetic`: `exact`, `modular_sampled[:k=v,...]`,
//!   `modular_polynomial[:k=v,...]` or (helper-vh2) `tadic[:d0=4,order=1,primes=1,...]`; `threads` overrides the
//!   arithmetic's thread count.
//!   keys.tsv: one input per line: `<top family name | pres:<presentation id>>\t<comma separated slot powers>`
//!   (lines starting with '#' are skipped). Every key is reduced ALONE (its own request, onto the masters).
//!   helper-vh2 (weighted requests): optional 3rd column = an integer weight (constant in d, default 1) and optional
//!   4th column = a group label: CONSECUTIVE lines with the same label form ONE request `sum_k w_k I(k)`; lines
//!   without a label stay alone. All inputs of a group must have the same loop count.
//! out.txt: per request `key <n> <top> <powers>` (single unweighted key, as before) or `group <n> <label> <inputs>`
//! followed by one `input <top> <powers> <weight>` line per input; then the rustred.p-reduction.canonical.v1 text
//! of its reduction (exact and modular arithmetics) or the rustred.p-reduction.tadic.v1 text (tadic), or
//! `error <message>`; then `end <n> <seconds>`.
//!
//! rustred.p-reduction.tadic.v1 (helper-vh2), one block per replayed prime:
//!   rustred.p-reduction.tadic.v1
//!   dimension <L>
//!   expansion d0=<d0> order=<order> primes=<count>
//!   prime <p>
//!   tterm <basis> <tag> <valuation> <precision> <c_valuation> ... <c_(precision-1)>
//!   tterm <basis> <tag> O <precision>
//! with <basis>, <tag> as in the canonical text, t = d - d0, `<valuation>` the order of the first coefficient that
//! is nonzero modulo p, the coefficients in 0..p, and `O` when every known coefficient vanishes modulo p (the value
//! is O(t^precision), NOT zero). Every term has precision >= order (proved by the replay's precision plan).
//! With RUSTRED_ORDERED_MODULAR_STATS set, a line `{"stage":"p_key_batch.request","n":<n>}` is appended to the
//! statistics file before each request (helper-vh2 job driver).
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rustred::algebra::Coefficient;
use rustred::family::IntegralKey;
use rustred::propagator::PresentationId;
use rustred::propagator::library::{
    PBasis, PLibrary, PReductionRequest, PTadicReduction, PUncoveredPolicy,
};
use rustred::reduction::{OrderedSchedulerOptions, OrderedWeightArithmetic, ReductionLimits};
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

#[path = "p_path_census/canonical.rs"]
mod canonical;

const TADIC_SCHEMA: &str = "rustred.p-reduction.tadic.v1";

fn arithmetic(name: &str, threads: usize) -> OrderedWeightArithmetic {
    name.parse::<OrderedWeightArithmetic>()
        .unwrap_or_else(|error| panic!("arithmetic {name}: {error}"))
        .with_threads(threads)
}

/// One input line: top, powers, weight, optional group label.
struct Input {
    top: String,
    powers: Vec<i64>,
    weight: i64,
    group: Option<String>,
}

fn parse_line(line: &str) -> Input {
    let mut columns = line.split('\t');
    let top = columns.next().expect("<top>").to_owned();
    let powers: Vec<i64> = columns
        .next()
        .expect("<top>\\t<powers>")
        .split(',')
        .map(|p| p.trim().parse().unwrap())
        .collect();
    let weight = columns
        .next()
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .map_or(1, |w| w.parse().expect("integer weight"));
    let group = columns
        .next()
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .map(str::to_owned);
    Input {
        top,
        powers,
        weight,
        group,
    }
}

/// Consecutive inputs with the same group label (or one unlabelled input).
fn requests(inputs: Vec<Input>) -> Vec<Vec<Input>> {
    let mut out: Vec<Vec<Input>> = vec![];
    for input in inputs {
        let joins = input.group.is_some()
            && out
                .last()
                .and_then(|last| last.last())
                .is_some_and(|last| last.group == input.group);
        if joins {
            out.last_mut().unwrap().push(input);
        } else {
            out.push(vec![input]);
        }
    }
    out
}

fn powers_text(powers: &[i64]) -> String {
    powers
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn basis_text(basis: &PBasis) -> String {
    match basis {
        PBasis::Unit => "U".to_owned(),
        PBasis::Master(name) => format!("M:{name}"),
        PBasis::Residual(id) => format!("R:{}", id.0),
    }
}

fn tadic_text(reduction: &PTadicReduction) -> String {
    let mut out = format!(
        "{TADIC_SCHEMA}\ndimension {}\nexpansion d0={} order={} primes={}\n",
        reduction.dimension,
        reduction.d0,
        reduction.order,
        reduction.images.len()
    );
    for image in &reduction.images {
        out.push_str(&format!("prime {}\n", image.prime));
        for term in &image.terms {
            let tag = format!("{:?}", term.tag).replace(' ', "");
            let series = &term.series;
            match series.valuation() {
                Some(valuation) => {
                    let coefficients: Vec<String> = (valuation..series.precision())
                        .map(|order| series.coefficient(order).unwrap().to_string())
                        .collect();
                    out.push_str(&format!(
                        "tterm {} {tag} {valuation} {} {}\n",
                        basis_text(&term.basis),
                        series.precision(),
                        coefficients.join(" ")
                    ));
                }
                None => out.push_str(&format!(
                    "tterm {} {tag} O {}\n",
                    basis_text(&term.basis),
                    series.precision()
                )),
            }
        }
    }
    out
}

/// helper-vh2: a marker line in the statistics file (`RUSTRED_ORDERED_MODULAR_STATS`, appended by the engine one
/// line per record) before each request, so that a job driver can attribute the engine's statistics (trace digest,
/// t-adic certificate fields) to the request.
fn stats_marker(line: &str) {
    if let Some(path) = std::env::var_os("RUSTRED_ORDERED_MODULAR_STATS") {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn presentation(library: &PLibrary, top: &str) -> Option<PresentationId> {
    top.strip_prefix("pres:")
        .map(|id| PresentationId(id.parse().unwrap()))
        .or_else(|| library.top_presentation(top))
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
    let threads: usize = args.get(5).map_or(1, |t| t.parse().unwrap());
    let arith = arithmetic(&args[3], threads);
    let tadic = matches!(arith, OrderedWeightArithmetic::Tadic(..));
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
    let inputs: Vec<Input> = std::fs::read_to_string(&args[2])
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(parse_line)
        .collect();
    for (n, request) in requests(inputs).into_iter().enumerate() {
        let single = request.len() == 1 && request[0].group.is_none() && request[0].weight == 1;
        if single {
            let input = &request[0];
            writeln!(out, "key {n} {} {}", input.top, powers_text(&input.powers)).unwrap();
        } else {
            let label = request[0].group.as_deref().unwrap_or("-");
            writeln!(out, "group {n} {label} {}", request.len()).unwrap();
            for input in &request {
                writeln!(
                    out,
                    "input {} {} {}",
                    input.top,
                    powers_text(&input.powers),
                    input.weight
                )
                .unwrap();
            }
        }
        stats_marker(&format!("{{\"stage\":\"p_key_batch.request\",\"n\":{n}}}"));
        let started = Instant::now();
        let mut weighted: Vec<(PresentationId, IntegralKey, Coefficient)> = vec![];
        let mut unknown = None;
        for input in &request {
            match presentation(&library, &input.top) {
                Some(id) => weighted.push((
                    id,
                    IntegralKey::try_new(input.powers.clone()).unwrap(),
                    context.integer(input.weight),
                )),
                None => {
                    unknown = Some(input.top.clone());
                    break;
                }
            }
        }
        if let Some(top) = unknown {
            writeln!(out, "error unknown top {top}").unwrap();
            writeln!(out, "end {n} 0").unwrap();
            continue;
        }
        let request = PReductionRequest {
            inputs: weighted,
            arithmetic: arith,
            scheduler: OrderedSchedulerOptions::with_threads(threads),
            step_cache: if std::env::var("P_KEY_BATCH_NO_STEP_CACHE").is_ok() {
                None
            } else {
                Some(cache.clone())
            },
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        };
        let text = if tadic {
            library.reduce_tadic(request).map(|reduction| tadic_text(&reduction))
        } else {
            library
                .reduce(request)
                .map(|reduction| canonical::canonical_text(&reduction))
        };
        match text {
            Ok(text) => out.write_all(text.as_bytes()).unwrap(),
            Err(error) => writeln!(out, "error {}", format!("{error}").replace('\n', " ")).unwrap(),
        }
        writeln!(out, "end {n} {:.3}", started.elapsed().as_secs_f64()).unwrap();
        out.flush().unwrap();
    }
}
