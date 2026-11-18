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
//!
//! helper-vh2 B3 options (after `[threads]`, `key=value`):
//!   `seeds=<s1>,<s2>,...`  record each tadic request ONCE and replay it for every seed (each seed gives the primes
//!                          and erasure-check points a separate run with `seed=<s>` would give); the `prime` lines
//!                          then read `prime <p> seed=<s> vector=<j>`.
//!   `vectors=<m>[:<seed>]` every LABELLED tadic group is recorded once (with random marker weights; the group's own
//!                          weights are ignored) and replayed for m weight vectors drawn uniformly from [1, 2^62),
//!                          reproducibly from (seed, label); explicit vectors: a weight column `w1;w2;...;wm` on every
//!                          line of the group. Output: `markers <m_1> ... <m_B>` after the input lines, then per vector
//!                          `vector <j> <w_1> ... <w_B>` followed by its `prime` blocks.
//!   `release_every=<n>`    after every n-th request call `PLibrary::release_retained` (owner programs, prepared
//!                          routes / orbits, zero-proof cache, the calling thread's caches), drop and recreate the
//!                          in-memory step cache, and `malloc_trim(0)`.
//! With RUSTRED_ORDERED_MODULAR_STATS set, a line `{"stage":"p_key_batch.loaded",...}` (after the library load) and
//! after each request `{"stage":"p_key_batch.memory","n":<n>,"rss_kb":..,"hwm_kb":..,"malloc_in_use":..,
//! "malloc_free":..,"retained":{..},"released":{..}|null,"rss_after_kb":..}` are appended (RSS from /proc/self/status,
//! glibc mallinfo2).
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rustred::algebra::Coefficient;
use rustred::family::IntegralKey;
use rustred::propagator::PresentationId;
use rustred::propagator::library::{
    PBasis, PLibrary, PReductionRequest, PTadicReduction, PTadicVariants, PUncoveredPolicy,
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
    /// helper-vh2 B3: explicit weights of the group's weight vectors (`w1;w2;..`).
    vector: Vec<i64>,
    weight_text: String,
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
    let weight_text = columns
        .next()
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .unwrap_or("1")
        .to_owned();
    let vector: Vec<i64> = if weight_text.contains(';') {
        weight_text
            .split(';')
            .map(|w| w.trim().parse().expect("integer weight"))
            .collect()
    } else {
        vec![]
    };
    // with explicit vectors the request's own weight is a placeholder (the marker recording ignores it)
    let weight = if vector.is_empty() {
        weight_text.parse().expect("integer weight")
    } else {
        1
    };
    let group = columns
        .next()
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .map(str::to_owned);
    Input {
        top,
        powers,
        weight,
        vector,
        weight_text,
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

fn tadic_text(reduction: &PTadicReduction, seeds: &[u64], vectors: Option<&[Vec<i64>]>) -> String {
    let mut out = String::new();
    if let Some(vectors) = vectors {
        let markers: Vec<String> = reduction
            .recording_markers
            .iter()
            .map(u64::to_string)
            .collect();
        out.push_str(&format!("markers {}\n", markers.join(" ")));
        let _ = vectors;
    }
    let primes_per_seed = reduction
        .images
        .iter()
        .filter(|image| image.vector == 0 && image.seed == reduction.images[0].seed)
        .count();
    out.push_str(&format!(
        "{TADIC_SCHEMA}\ndimension {}\nexpansion d0={} order={} primes={}",
        reduction.dimension, reduction.d0, reduction.order, primes_per_seed
    ));
    if !seeds.is_empty() {
        out.push_str(&format!(
            " seeds={}",
            seeds
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    if let Some(vectors) = vectors {
        out.push_str(&format!(" vectors={}", vectors.len()));
    }
    out.push('\n');
    let mut current_vector = usize::MAX;
    for image in &reduction.images {
        if let Some(vectors) = vectors
            && image.vector != current_vector
        {
            current_vector = image.vector;
            let weights: Vec<String> = vectors[image.vector].iter().map(i64::to_string).collect();
            out.push_str(&format!("vector {} {}\n", image.vector, weights.join(" ")));
        }
        if seeds.is_empty() && vectors.is_none() {
            out.push_str(&format!("prime {}\n", image.prime));
        } else {
            out.push_str(&format!(
                "prime {} seed={} vector={}\n",
                image.prime, image.seed, image.vector
            ));
        }
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
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn presentation(library: &PLibrary, top: &str) -> Option<PresentationId> {
    top.strip_prefix("pres:")
        .map(|id| PresentationId(id.parse().unwrap()))
        .or_else(|| library.top_presentation(top))
}

/// helper-vh2 B3: (VmRSS, VmHWM) in kB of this process.
fn rss_kb() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

/// helper-vh2 B3: glibc heap bytes in use and free-but-held (all arenas).
fn malloc_bytes() -> (u64, u64) {
    // SAFETY: mallinfo2 only reads the allocator's statistics.
    let info = unsafe { libc::mallinfo2() };
    (
        info.uordblks as u64 + info.hblkhd as u64,
        info.fordblks as u64,
    )
}

/// Options after `[threads]`.
#[derive(Default)]
struct Options {
    seeds: Vec<u64>,
    vectors: Option<(usize, u64)>,
    release_every: usize,
}

fn options(args: &[String]) -> Options {
    let mut options = Options::default();
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .unwrap_or_else(|| panic!("option {arg}: key=value"));
        match key {
            "seeds" => {
                options.seeds = value
                    .split(',')
                    .map(|s| s.trim().parse().expect("seed"))
                    .collect()
            }
            "vectors" => {
                let (m, seed) = value.split_once(':').unwrap_or((value, "0"));
                options.vectors = Some((
                    m.parse().expect("vectors=m"),
                    seed.parse().expect("vector seed"),
                ));
            }
            "release_every" => options.release_every = value.parse().expect("release_every=n"),
            other => panic!("unknown option {other}"),
        }
    }
    options
}

/// Weight vectors of a group: explicit (`w1;w2;..` columns) or drawn.
fn group_vectors(
    request: &[Input],
    drawn: Option<(usize, u64)>,
    label: &str,
) -> Option<Vec<Vec<i64>>> {
    if request.iter().any(|input| !input.vector.is_empty()) {
        let m = request[0].vector.len();
        assert!(
            request.iter().all(|input| input.vector.len() == m),
            "group {label}: every input needs the same number of explicit weights"
        );
        return Some(
            (0..m)
                .map(|j| request.iter().map(|input| input.vector[j]).collect())
                .collect(),
        );
    }
    let (m, seed) = drawn?;
    // splitmix64 from (seed, FNV-1a of the label): reproducible, documented in the output (`vector` lines)
    let mut state = label.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    }) ^ seed;
    let mut next = move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 2).max(1) as i64
    };
    Some(
        (0..m)
            .map(|_| request.iter().map(|_| next()).collect())
            .collect(),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() >= 5,
        "usage: p_key_batch <p-library dir> <keys.tsv> <arith> <out.txt> [threads] [seeds=..] [vectors=m[:seed]] [release_every=n]"
    );
    let mut library = load_p_library(
        PLibrarySource::Directory(PathBuf::from(&args[1])),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap();
    let threads: usize = args.get(5).map_or(1, |t| t.parse().unwrap());
    let opts = options(args.get(6..).unwrap_or(&[]));
    let arith = arithmetic(&args[3], threads);
    let tadic = matches!(arith, OrderedWeightArithmetic::Tadic(..));
    let new_cache = |library: &PLibrary| {
        let binding = library.step_cache_binding().expect("step cache binding");
        rustred::reduction::OrderedStepCache::in_memory(
            binding,
            rustred::reduction::OrderedStepCacheOptions {
                populate: true,
                ..Default::default()
            },
        )
    };
    let mut cache = new_cache(&library);
    let (rss, hwm) = rss_kb();
    stats_marker(&format!(
        "{{\"stage\":\"p_key_batch.loaded\",\"rss_kb\":{rss},\"hwm_kb\":{hwm},\"retained\":{}}}",
        library.retained_state()
    ));
    // 12d2: resume files of a request stay until its lines are durably written.
    rustred::reduction::set_resume_caller_commit(true);
    let mut out = std::io::BufWriter::new(std::fs::File::create(&args[4]).unwrap());
    let context = library.coefficient_context().clone();
    let inputs: Vec<Input> = std::fs::read_to_string(&args[2])
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(parse_line)
        .collect();
    for (n, request) in requests(inputs).into_iter().enumerate() {
        let single = request.len() == 1
            && request[0].group.is_none()
            && request[0].weight == 1
            && request[0].vector.is_empty();
        let label = request[0].group.as_deref().unwrap_or("-").to_owned();
        if single {
            let input = &request[0];
            writeln!(out, "key {n} {} {}", input.top, powers_text(&input.powers)).unwrap();
        } else {
            writeln!(out, "group {n} {label} {}", request.len()).unwrap();
            for input in &request {
                writeln!(
                    out,
                    "input {} {} {}",
                    input.top,
                    powers_text(&input.powers),
                    input.weight_text
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
        let vectors = if tadic && request[0].group.is_some() {
            group_vectors(&request, opts.vectors, &label)
        } else {
            None
        };
        let variants = PTadicVariants {
            seeds: opts.seeds.clone(),
            weight_vectors: vectors
                .iter()
                .flatten()
                .map(|vector| vector.iter().map(|&w| context.integer(w)).collect())
                .collect(),
        };
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
            library
                .reduce_tadic_variants(request, &variants)
                .map(|reduction| tadic_text(&reduction, &opts.seeds, vectors.as_deref()))
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
        out.get_ref().sync_data().unwrap();
        rustred::reduction::commit_resume();
        // helper-vh2 B3: memory after the request, and the release.
        let (rss, hwm) = rss_kb();
        let (in_use, free) = malloc_bytes();
        let retained = library.retained_state();
        let mut released = String::from("null");
        let mut after = (rss, in_use, free);
        if opts.release_every > 0 && (n + 1) % opts.release_every == 0 {
            released = match std::sync::Arc::get_mut(&mut library) {
                Some(library) => library.release_retained().to_string(),
                None => "\"the library is shared: not released\"".to_owned(),
            };
            cache = new_cache(&library);
            // SAFETY: malloc_trim only returns free heap memory to the system.
            unsafe { libc::malloc_trim(0) };
            let (rss_after, _) = rss_kb();
            let (in_use_after, free_after) = malloc_bytes();
            after = (rss_after, in_use_after, free_after);
        }
        stats_marker(&format!(
            "{{\"stage\":\"p_key_batch.memory\",\"n\":{n},\"rss_kb\":{rss},\"hwm_kb\":{hwm},\
             \"malloc_in_use\":{in_use},\"malloc_free\":{free},\"retained\":{retained},\"released\":{released},\
             \"rss_after_kb\":{},\"malloc_in_use_after\":{},\"malloc_free_after\":{}}}",
            after.0, after.1, after.2
        ));
    }
}
