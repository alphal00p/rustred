//! Four-loop validation of the scope 1-4 library program (owner: M3-T18
//! rr-validate4).
//!
//! G4's validation items at four loops (plan M3 section 7; AD section 9, the
//! L = 4 row and "additional 4L acceptance"), on the package named by
//! `RUSTRED_P_LIBRARY_1_4` (M3-T8c's; not in the repository, its 10- and
//! 11-line Custom owners take hours to generate):
//!
//! - `p-validate` at 4L, sharded: `validation_keys` of `rustred p-validate`
//!   (10^4 distinct random keys of the 4L tops, dots <= 3 and rank <= 8 in
//!   total) are split into shards by index; each shard traces its keys with
//!   `descent::trace` (`PUncoveredPolicy::Reject` semantics, no coefficients)
//!   and writes one JSON row; the rows must add up to every key with 0
//!   Uncovered, 0 MissingOwner and 0 descent failures.
//! - Routing invariance: a 4L input and its image under a unimodular
//!   loop-momentum map `k -> A k + b q` (`q` fixed) that carries the input's
//!   lines onto lines of a 4L top (the same top or another) reduce to the same
//!   terms.
//! - Exactness: RustRed's own IBP rows at random seeds on every owner sector of
//!   the program (every owner, so every 4L node class) and on random subsectors
//!   of the 4L tops reduce to zero exactly in `Q(d)`: `PLib(x) = Sum_j e_j
//!   PLib(T_j)` for every row `x = Sum_j e_j T_j`. The rules-study oracles:
//!   R_INT (the 8 integer 4L Custom owners keep one residual, the corner),
//!   the knock-outs T148, T015, T041, T003, T016 keep none; P0 SpIRed in the
//!   owner family of the Forcer-blind crossed owners T147 (cross), T152 (no1),
//!   T153 (no2), compared in the tag basis at two random `d`.
//! - Arithmetic and threads (the one place in M3): Exact, ModularPolynomial
//!   and ModularSampled give the same terms on 4L inputs, and 1 and 16
//!   scheduler threads the same bits, in a NON-ignored small test that says
//!   SKIPPED without the package.
//!
//! The ignored tests run optimised, once, on the final library
//! (notes/rr-validate4.md); environment variables `P_VALIDATE4_*` size them.
//! Their reductions run under ModularPolynomial (vakint's default, exact in
//! `Q(d)`; `P_VALIDATE4_ARITHMETIC=exact` for Exact, which takes minutes on
//! some 11-line seeds), shown equal to Exact by the non-ignored test.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use rustred::algebra::{
    Coefficient, CoefficientContext, ExactAlgebraLimits, IndexedAlgebraLimits,
    IndexedCoefficientContext,
};
use rustred::family::IntegralKey;
use rustred::identity::{ParametricIbpGenerator, ParametricRelation};
use rustred::propagator::census::PSectorFate;
use rustred::propagator::descent::{self, TraceFailureKind, TraceReport};
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::kernel::gamma::FactoredRational;
use rustred::propagator::library::{
    PLibrary, PReduction, PReductionRequest, PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{AffineSlotForm, Presentation, PresentationKind, Slot};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::{OwnerClassId, PresentationId};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionLimits, SampledOptions,
};
use rustred::solver::litered::{DynamicLiteRedSession, LiteRedOptions};
use rustred_app::propagator::assemble::{SplitMix, ValidateOptions, validation_keys};
use rustred_app::propagator::{AssembledLibrary, PLibraryOptions, PLibrarySource, load_p_library};
use serde_json::{Value, json};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::{Q, Rational};

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

/// The directory of the scope 1-4 package.
const PACKAGE: &str = "RUSTRED_P_LIBRARY_1_4";

const OWNERS_TSV: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rustred-core/tests/p_library/golden/owners_1-4.tsv"
);

fn package() -> Option<PathBuf> {
    std::env::var_os(PACKAGE).map(PathBuf::from)
}

/// `P_VALIDATE4_<name>` parsed, or `default`.
fn setting<T: std::str::FromStr>(name: &str, default: T) -> T {
    match std::env::var(format!("P_VALIDATE4_{name}")) {
        Ok(text) => text
            .parse()
            .unwrap_or_else(|_| panic!("P_VALIDATE4_{name}={text:?} does not parse")),
        Err(_) => default,
    }
}

/// The output directory of the evidence rows (`P_VALIDATE4_OUT`), if any.
fn out_dir() -> Option<PathBuf> {
    std::env::var_os("P_VALIDATE4_OUT").map(PathBuf::from)
}

fn write_json(path: &Path, value: &Value) {
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    std::fs::rename(&partial, path).unwrap();
}

/// The scope 1-4 program of the package, loaded once without the descent
/// certificate (its writer and the integrator's batch end check it once).
fn scope_four() -> &'static Arc<PLibrary> {
    static LIBRARY: OnceLock<Arc<PLibrary>> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let directory =
            package().unwrap_or_else(|| panic!("{PACKAGE} names the scope 1-4 package"));
        let start = Instant::now();
        let library = load_p_library(
            PLibrarySource::Directory(directory),
            &PLibraryOptions {
                verify_descent: false,
            },
        )
        .unwrap();
        assert_eq!(library.scope(), 1..=4);
        println!(
            "scope 1-4 package loaded in {:.1} s: {}",
            start.elapsed().as_secs_f64(),
            library.identity()
        );
        library
    })
}

/// The loaded program seen as an assembly, for `p-validate`'s key generator:
/// the tops are presentations `0..` in the order of the built-in family file.
fn as_assembled(library: &Arc<PLibrary>) -> AssembledLibrary {
    let mut families: PFamiliesFile = serde_json::from_str(P_FAMILIES_V1_JSON).unwrap();
    families.families.retain(|entry| entry.loops <= 4);
    for (index, entry) in families.families.iter().enumerate() {
        let top = library.presentation(PresentationId(index as u32));
        assert!(
            matches!(&top.kind, PresentationKind::Top { name } if *name == entry.name),
            "presentation {index} is not the top {}",
            entry.name
        );
    }
    AssembledLibrary {
        library: Arc::clone(library),
        families,
        max_loops: 4,
        dag: Arc::clone(library.topology().expect("the package has its DAG")),
        nodes: Vec::new(),
        custom: BTreeMap::new(),
        descent: None,
        timings: Vec::new(),
    }
}

/// The top presentations of `loops` loops.
fn tops(library: &PLibrary, loops: u8) -> Vec<PresentationId> {
    (library.presentations().iter())
        .filter(|p| p.loops == loops && matches!(p.kind, PresentationKind::Top { .. }))
        .map(|p| p.id)
        .collect()
}

fn label(library: &PLibrary, class: OwnerClassId) -> String {
    library.census().classes[&class].key.label()
}

/// `forcer name -> class` of the Custom owners (`owners_1-4.tsv` of M3-T15).
fn custom_owners(library: &PLibrary) -> BTreeMap<String, OwnerClassId> {
    let text = std::fs::read_to_string(OWNERS_TSV).unwrap();
    let mut rows = text.lines().filter(|line| !line.starts_with('#'));
    let header: Vec<&str> = rows.next().unwrap().split('\t').collect();
    let column = |name: &str| header.iter().position(|h| *h == name).unwrap();
    let (forcer, class) = (column("forcer"), column("class"));
    rows.map(|row| {
        let fields: Vec<&str> = row.split('\t').collect();
        let hex = fields[class].split_once('-').unwrap().1;
        let id = OwnerClassId(u64::from_str_radix(hex, 16).unwrap());
        assert_eq!(label(library, id), fields[class], "{}", fields[forcer]);
        (fields[forcer].to_owned(), id)
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Reductions
// ---------------------------------------------------------------------------

fn exact() -> ExactAlgebraLimits {
    ExactAlgebraLimits::default()
}

type Input = (PresentationId, IntegralKey, Coefficient);

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn rational(numerator: i64, denominator: i64) -> Rational {
    Rational::from((Integer::from(numerator), Integer::from(denominator)))
}

fn coefficient_of(context: &CoefficientContext, value: &Rational) -> Coefficient {
    FactoredRational::from_rational(value.clone())
        .to_coefficient(context, exact())
        .unwrap()
}

fn modular_polynomial(threads: usize) -> OrderedWeightArithmetic {
    OrderedWeightArithmetic::ModularPolynomial(
        ModularCommon {
            threads,
            ..Default::default()
        },
        PolynomialOptions::default(),
    )
}

fn modular_sampled(threads: usize) -> OrderedWeightArithmetic {
    OrderedWeightArithmetic::ModularSampled(
        ModularCommon {
            threads,
            ..Default::default()
        },
        SampledOptions::default(),
    )
}

/// `inputs` reduced onto the terminals under `arithmetic` with `threads`
/// scheduler threads.
fn reduce_with(
    library: &PLibrary,
    inputs: Vec<Input>,
    arithmetic: OrderedWeightArithmetic,
    threads: usize,
) -> PReduction {
    let scheduler = if threads > 1 {
        OrderedSchedulerOptions {
            threads,
            window_keys: 8,
            ..Default::default()
        }
    } else {
        OrderedSchedulerOptions::default()
    };
    library
        .reduce(PReductionRequest {
            inputs,
            arithmetic,
            scheduler,
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: false,
            limits: ReductionLimits::default(),
            inspector: None,
        })
        .unwrap()
}

/// `inputs` reduced onto the terminals in [`exactness_arithmetic`].
fn reduce(library: &PLibrary, inputs: Vec<Input>) -> PReduction {
    reduce_with(library, inputs, exactness_arithmetic(), 1)
}

/// The nonzero terms of a reduction, in a canonical order.
fn nonzero_terms(reduction: &PReduction) -> Vec<(String, GammaTag, Coefficient)> {
    let mut terms: Vec<_> = (reduction.terms.iter())
        .filter(|term| !term.coefficient.is_zero())
        .map(|term| {
            (
                format!("{:?}", term.basis),
                term.tag.clone(),
                term.coefficient.clone(),
            )
        })
        .collect();
    terms.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    terms
}

/// A weighted sum of keys of one presentation as reduction inputs.
fn inputs_of(
    library: &PLibrary,
    presentation: PresentationId,
    terms: &[(Vec<i64>, Rational)],
) -> Vec<Input> {
    let context = library.coefficient_context();
    (terms.iter())
        .map(|(powers, weight)| (presentation, key(powers), coefficient_of(context, weight)))
        .collect()
}

fn line_slots(p: &Presentation) -> Vec<usize> {
    (0..p.slots.len())
        .filter(|&s| matches!(p.slots[s], Slot::Line(_)))
        .collect()
}

/// The census support of `powers` (positive line powers plus forced axes),
/// and whether it is a zero sector.
fn sector(library: &PLibrary, p: &Presentation, powers: &[i64]) -> (u64, bool) {
    let raw = (line_slots(p).into_iter())
        .filter(|&s| powers[s] > 0)
        .fold(0_u64, |mask, s| mask | 1 << s);
    let support = raw | p.forced_axes();
    let zero = !matches!(
        library.census().sectors.get(&(p.id, support)),
        Some(PSectorFate::Owner { .. })
    );
    (support, zero)
}

/// A random key of `p` on the lines `kept` (each kept with probability 3/4
/// when `kept` is `None`): `dots` extra units on them and `rank` numerator
/// units on the other slots, each at most `-3`.
fn random_powers(
    p: &Presentation,
    kept: Option<u64>,
    dots: u32,
    rank: u32,
    rng: &mut SplitMix,
) -> Vec<i64> {
    let lines = line_slots(p);
    let mut powers = vec![0_i64; p.slots.len()];
    let mut on = Vec::new();
    for &slot in &lines {
        let keep = match kept {
            Some(mask) => mask >> slot & 1 == 1,
            None => rng.below(4) != 0,
        };
        if keep {
            powers[slot] = 1;
            on.push(slot);
        }
    }
    if !on.is_empty() {
        for _ in 0..dots {
            powers[on[rng.below(on.len() as u64) as usize]] += 1;
        }
    }
    let free: Vec<usize> = (0..powers.len()).filter(|&s| powers[s] == 0).collect();
    let mut left = rank;
    while left > 0 && free.iter().any(|&s| powers[s] > -3) {
        let slot = free[rng.below(free.len() as u64) as usize];
        if powers[slot] > -3 {
            powers[slot] -= 1;
            left -= 1;
        }
    }
    powers
}

fn rank_of(powers: &[i64]) -> i64 {
    powers.iter().filter(|&&x| x < 0).map(|x| -x).sum()
}

// ---------------------------------------------------------------------------
// (1) p-validate 10^4 keys at 4L, sharded
// ---------------------------------------------------------------------------

/// The keys and bounds of the run: `rustred p-validate`'s defaults (10^4
/// keys, dots <= 3, rank <= 8, its seed), at `P_VALIDATE4_LOOPS` (4).
fn validate_run() -> (u8, ValidateOptions) {
    let defaults = ValidateOptions::default();
    let options = ValidateOptions {
        points: setting("POINTS", defaults.points),
        seed: setting("SEED", defaults.seed),
        max_dots: setting("MAX_DOTS", defaults.max_dots),
        max_rank: setting("MAX_RANK", defaults.max_rank),
        limits: defaults.limits,
    };
    (setting("LOOPS", 4_u8), options)
}

/// `P_VALIDATE4_SHARD = "i/n"`.
fn shard() -> (usize, usize) {
    let text = std::env::var("P_VALIDATE4_SHARD").expect("P_VALIDATE4_SHARD=<i>/<n>");
    let (index, count) = text.split_once('/').expect("P_VALIDATE4_SHARD=<i>/<n>");
    let (index, count): (usize, usize) = (index.parse().unwrap(), count.parse().unwrap());
    assert!(index < count, "shard {index} of {count}");
    (index, count)
}

fn shard_file(directory: &Path, index: usize, count: usize) -> PathBuf {
    directory.join(format!("shard-{index:03}-of-{count:03}.json"))
}

fn failures(report: &TraceReport) -> Value {
    json!({
        "uncovered": report.failures_of(TraceFailureKind::Uncovered),
        "missing_owner": report.failures_of(TraceFailureKind::MissingOwner),
        "descent": report.failures_of(TraceFailureKind::Descent),
        "other": report.failures_of(TraceFailureKind::Other),
    })
}

/// One shard of `p-validate` at 4L: the keys of `validation_keys` whose index
/// is `i mod n`, traced; its row goes to `P_VALIDATE4_OUT`.
#[test]
#[ignore = "slow: one shard of p-validate 10^4 at 4L (P_VALIDATE4_SHARD, P_VALIDATE4_OUT); optimised"]
fn p_validate_traces_one_shard_of_the_four_loop_keys() {
    let (index, count) = shard();
    let directory = out_dir().expect("P_VALIDATE4_OUT names the row directory");
    std::fs::create_dir_all(&directory).unwrap();
    let (loops, options) = validate_run();
    let library = scope_four();
    let assembled = as_assembled(library);
    let start = Instant::now();
    let (keys, whole_box) = validation_keys(&assembled, loops, &options);
    let total = keys.len();
    let mine: Vec<(PresentationId, Vec<i64>)> = (keys.into_iter().enumerate())
        .filter(|(position, _)| position % count == index)
        .map(|(_, key)| key)
        .collect();
    let drawn = start.elapsed().as_secs_f64();
    let report = descent::trace(library, &mine, options.limits).unwrap();
    let seconds = start.elapsed().as_secs_f64();
    let applied: BTreeMap<String, usize> = (report.applied.iter())
        .map(|(class, sites)| (label(library, *class), *sites))
        .collect();
    let row = json!({
        "schema": "rustred.p-validate4-shard.v1",
        "identity": library.identity(),
        "loops": loops,
        "points": options.points,
        "seed": options.seed,
        "max_dots": options.max_dots,
        "max_rank": options.max_rank,
        "shard": index,
        "shards": count,
        "total_keys": total,
        "whole_box": whole_box,
        "keys": mine.len(),
        "distinct_inputs": report.distinct_inputs,
        "route_keys": report.route_keys,
        "apply_keys": report.apply_keys,
        "apply_sites": report.apply_sites,
        "zero_keys": report.zero_keys,
        "flipped_route_keys": report.flipped_route_keys,
        "failures": failures(&report),
        "samples": report.samples.iter().map(|s| format!("input {} {:?}: {}", s.input, s.kind, s.detail)).collect::<Vec<_>>(),
        "applied": applied,
        "seconds_keys": drawn,
        "seconds": seconds,
    });
    write_json(&shard_file(&directory, index, count), &row);
    println!(
        "p-validate L{loops} shard {index}/{count}: {} keys of {total}, {} route + {} apply keys, \
         {} owners applied, failures {}; {seconds:.1} s",
        mine.len(),
        report.route_keys,
        report.apply_keys,
        report.applied.len(),
        failures(&report)
    );
    assert!(report.is_clean(), "{:?}", report.samples);
}

/// The shard rows of `P_VALIDATE4_OUT` add up: every shard of one run (same
/// identity, keys and bounds) present once, every key traced, 0 Uncovered,
/// 0 MissingOwner, 0 descent failures (and 0 other); the owners never
/// applied are listed. Writes `summary.json` beside the rows.
#[test]
#[ignore = "evidence: sums the p-validate shard rows of P_VALIDATE4_OUT"]
fn p_validate_shards_add_up_to_every_key_with_no_refusal() {
    let directory = out_dir().expect("P_VALIDATE4_OUT names the row directory");
    let mut rows: Vec<Value> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("shard-") && name.ends_with(".json"))
        })
        .map(|path| serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap())
        .collect();
    assert!(!rows.is_empty(), "no shard row in {}", directory.display());
    rows.sort_by_key(|row| row["shard"].as_u64().unwrap());
    let first = rows[0].clone();
    let count = first["shards"].as_u64().unwrap() as usize;
    let same = [
        "identity",
        "loops",
        "points",
        "seed",
        "max_dots",
        "max_rank",
        "shards",
        "total_keys",
    ];
    for row in &rows {
        for field in same {
            assert_eq!(row[field], first[field], "shard {}: {field}", row["shard"]);
        }
    }
    let shards: Vec<u64> = rows
        .iter()
        .map(|row| row["shard"].as_u64().unwrap())
        .collect();
    let missing: Vec<usize> = (0..count)
        .filter(|s| !shards.contains(&(*s as u64)))
        .collect();
    let sum = |field: &str| -> u64 { rows.iter().map(|row| row[field].as_u64().unwrap()).sum() };
    let failure = |kind: &str| -> u64 {
        rows.iter()
            .map(|row| row["failures"][kind].as_u64().unwrap())
            .sum()
    };
    let mut applied = BTreeSet::new();
    for row in &rows {
        applied.extend(row["applied"].as_object().unwrap().keys().cloned());
    }
    let seconds: Vec<f64> = rows
        .iter()
        .map(|row| row["seconds"].as_f64().unwrap())
        .collect();
    let library = package().map(|_| scope_four());
    let never: Vec<String> = library.map_or_else(Vec::new, |library| {
        (library.owners())
            .map(|owner| label(library, owner.spec.class))
            .filter(|owner| !applied.contains(owner))
            .collect()
    });
    let summary = json!({
        "identity": first["identity"],
        "loops": first["loops"],
        "points": first["points"],
        "max_dots": first["max_dots"],
        "max_rank": first["max_rank"],
        "shards": count,
        "rows": rows.len(),
        "missing_shards": missing,
        "total_keys": first["total_keys"],
        "keys": sum("keys"),
        "route_keys_summed": sum("route_keys"),
        "apply_keys_summed": sum("apply_keys"),
        "flipped_route_keys_summed": sum("flipped_route_keys"),
        "uncovered": failure("uncovered"),
        "missing_owner": failure("missing_owner"),
        "descent": failure("descent"),
        "other": failure("other"),
        "owners_applied": applied.len(),
        "owners_never_applied": never,
        "seconds_max": seconds.iter().copied().fold(0.0, f64::max),
        "seconds_sum": seconds.iter().sum::<f64>(),
    });
    write_json(&directory.join("summary.json"), &summary);
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    assert!(missing.is_empty(), "shards missing: {missing:?}");
    assert_eq!(sum("keys"), first["total_keys"].as_u64().unwrap());
    for kind in ["uncovered", "missing_owner", "descent", "other"] {
        assert_eq!(failure(kind), 0, "{kind}");
    }
}

// ---------------------------------------------------------------------------
// (2) Routing invariance under unimodular loop-momentum maps
// ---------------------------------------------------------------------------

/// `k_i -> Sum_j A_ij k_j + b_i q` with `q` fixed: row `i` is `(A_i., b_i)`
/// over `(k_1, .., k_L, q)`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LoopMap(Vec<Vec<i64>>);

impl LoopMap {
    fn is_identity(&self) -> bool {
        (self.0.iter().enumerate())
            .all(|(i, row)| row.iter().enumerate().all(|(j, &v)| v == i64::from(i == j)))
    }

    /// `m(phi(k), q)` for a momentum row `m` over `(k, q)`.
    fn image(&self, momentum: &[i64]) -> Vec<i64> {
        let loops = self.0.len();
        let mut out = vec![0_i64; loops + 1];
        out[loops] = momentum[loops];
        for (i, &c) in momentum[..loops].iter().enumerate() {
            if c != 0 {
                for (o, v) in out.iter_mut().zip(&self.0[i]) {
                    *o += c * v;
                }
            }
        }
        out
    }
}

/// The determinant of a small integer matrix (Bareiss).
fn determinant(matrix: &[Vec<i64>]) -> i64 {
    let n = matrix.len();
    let mut m: Vec<Vec<i128>> = (matrix.iter())
        .map(|row| row.iter().map(|&v| i128::from(v)).collect())
        .collect();
    let (mut sign, mut previous) = (1_i128, 1_i128);
    for k in 0..n {
        if m[k][k] == 0 {
            let Some(swap) = (k + 1..n).find(|&r| m[r][k] != 0) else {
                return 0;
            };
            m.swap(k, swap);
            sign = -sign;
        }
        for i in k + 1..n {
            for j in k + 1..n {
                m[i][j] = (m[i][j] * m[k][k] - m[i][k] * m[k][j]) / previous;
            }
        }
        previous = m[k][k];
    }
    i64::try_from(sign * m[n - 1][n - 1]).unwrap()
}

/// The inverse of an integer matrix of determinant +-1 (adjugate).
fn unimodular_inverse(matrix: &[Vec<i64>]) -> Vec<Vec<i64>> {
    let n = matrix.len();
    let det = determinant(matrix);
    assert_eq!(det.abs(), 1);
    let minor = |row: usize, column: usize| -> Vec<Vec<i64>> {
        (0..n)
            .filter(|&r| r != row)
            .map(|r| {
                (0..n)
                    .filter(|&c| c != column)
                    .map(|c| matrix[r][c])
                    .collect()
            })
            .collect()
    };
    (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    let cofactor = if n == 1 { 1 } else { determinant(&minor(j, i)) };
                    let sign = if (i + j) % 2 == 0 { 1 } else { -1 };
                    sign * cofactor * det
                })
                .collect()
        })
        .collect()
}

/// The momentum rows `(left, right)` of the slots of `p`: `D_s = left . right`.
fn slot_factors(p: &Presentation) -> Vec<(Vec<i64>, Vec<i64>)> {
    (p.slots.iter())
        .map(|slot| match slot {
            Slot::Line(line) => (line.momentum.clone(), line.momentum.clone()),
            Slot::Isp(isp) => (isp.left.clone(), isp.right.clone()),
        })
        .collect()
}

/// A line correspondence: source line slot -> target line slot.
type Sigma = BTreeMap<usize, usize>;

/// Up to `limit` maps `phi` (unimodular, `q` fixed) that carry every line of
/// `lines` (slots of a source with momentum rows `momenta`) onto a distinct
/// line of `target` up to sign, the identity excluded when `exclude_identity`.
fn line_maps(
    momenta: &[(Vec<i64>, Vec<i64>)],
    lines: &[usize],
    target: &Presentation,
    exclude_identity: bool,
    limit: usize,
) -> Vec<(LoopMap, Sigma)> {
    let loops = usize::from(target.loops);
    let k_part = |slot: usize| momenta[slot].0[..loops].to_vec();
    // A basis of the loop momenta among the lines, unimodular.
    let mut basis = None;
    let mut choice: Vec<usize> = (0..loops).collect();
    if lines.len() >= loops {
        loop {
            let rows: Vec<Vec<i64>> = choice.iter().map(|&c| k_part(lines[c])).collect();
            if determinant(&rows).abs() == 1 {
                basis = Some(choice.iter().map(|&c| lines[c]).collect::<Vec<usize>>());
                break;
            }
            // next combination
            let mut i = loops;
            while i > 0 && choice[i - 1] == lines.len() - loops + i - 1 {
                i -= 1;
            }
            if i == 0 {
                break;
            }
            choice[i - 1] += 1;
            for j in i..loops {
                choice[j] = choice[j - 1] + 1;
            }
        }
    }
    let Some(basis) = basis else {
        return Vec::new();
    };
    let c: Vec<Vec<i64>> = basis.iter().map(|&s| k_part(s)).collect();
    let inverse = unimodular_inverse(&c);
    let targets: Vec<(usize, Vec<i64>)> = (target.slots.iter().enumerate())
        .filter_map(|(slot, s)| match s {
            Slot::Line(line) => Some((slot, line.momentum.clone())),
            Slot::Isp(_) => None,
        })
        .collect();
    let mut lookup: HashMap<Vec<i64>, usize> = HashMap::new();
    for (slot, momentum) in &targets {
        lookup.insert(momentum.clone(), *slot);
        lookup.insert(momentum.iter().map(|v| -v).collect(), *slot);
    }
    let mut found = Vec::new();
    let mut tuple: Vec<(usize, i64)> = Vec::new();
    fn search(
        tuple: &mut Vec<(usize, i64)>,
        loops: usize,
        targets: &[(usize, Vec<i64>)],
        visit: &mut dyn FnMut(&[(usize, i64)]) -> bool,
    ) -> bool {
        if tuple.len() == loops {
            return visit(tuple);
        }
        for (index, _) in targets.iter().enumerate() {
            if tuple.iter().any(|&(t, _)| t == index) {
                continue;
            }
            for sign in [1, -1] {
                tuple.push((index, sign));
                let stop = search(tuple, loops, targets, visit);
                tuple.pop();
                if stop {
                    return true;
                }
            }
        }
        false
    }
    let mut visit = |tuple: &[(usize, i64)]| -> bool {
        // rows = C^-1 (T - s e_q): the images of the basis lines are the targets.
        let r: Vec<Vec<i64>> = (basis.iter().zip(tuple))
            .map(|(&slot, &(index, sign))| {
                let mut row: Vec<i64> = targets[index].1.iter().map(|v| sign * v).collect();
                row[loops] -= momenta[slot].0[loops];
                row
            })
            .collect();
        let rows: Vec<Vec<i64>> = (0..loops)
            .map(|i| {
                (0..=loops)
                    .map(|j| (0..loops).map(|m| inverse[i][m] * r[m][j]).sum())
                    .collect()
            })
            .collect();
        let a: Vec<Vec<i64>> = rows.iter().map(|row| row[..loops].to_vec()).collect();
        if determinant(&a).abs() != 1 {
            return false;
        }
        let map = LoopMap(rows);
        if exclude_identity && map.is_identity() {
            return false;
        }
        let mut sigma = Sigma::new();
        let mut used = BTreeSet::new();
        for &slot in lines {
            let image = map.image(&momenta[slot].0);
            let Some(&to) = lookup.get(&image) else {
                return false;
            };
            if !used.insert(to) {
                return false;
            }
            sigma.insert(slot, to);
        }
        found.push((map, sigma));
        found.len() >= limit
    };
    search(&mut tuple, loops, &targets, &mut visit);
    found
}

/// `I(powers)` of a source with slot rows `momenta`, written on `target`
/// through `phi`: every denominator goes to its image line (by `sigma`, else
/// the target line its squared momentum maps onto) with its power, every
/// numerator `D_s^(-n)` becomes `(left.phi . right.phi)^(-n)` expressed in the
/// slots of `target` and expanded. A weighted sum of keys of `target`; `None`
/// when a denominator has no image line.
fn transport(
    momenta: &[(Vec<i64>, Vec<i64>)],
    powers: &[i64],
    map: &LoopMap,
    sigma: &Sigma,
    target: &Presentation,
) -> Option<Vec<(Vec<i64>, Rational)>> {
    let width = target.slots.len();
    let mut base = vec![0_i64; width];
    for (slot, &power) in powers.iter().enumerate().filter(|(_, power)| **power > 0) {
        let to = match sigma.get(&slot) {
            Some(&to) => to,
            None => {
                let (left, right) = &momenta[slot];
                let image = map.image(left);
                let negated: Vec<i64> = image.iter().map(|v| -v).collect();
                (left == right).then_some(())?;
                (target.slots.iter().enumerate()).find_map(|(to, s)| match s {
                    Slot::Line(line) if line.momentum == image || line.momentum == negated => {
                        Some(to)
                    }
                    _ => None,
                })?
            }
        };
        if base[to] != 0 {
            return None;
        }
        base[to] = power;
    }
    let mut polynomial: BTreeMap<Vec<i64>, Rational> =
        BTreeMap::from([(vec![0_i64; width], Rational::one())]);
    for (slot, &power) in powers.iter().enumerate() {
        if power >= 0 {
            continue;
        }
        let (left, right) = &momenta[slot];
        let form = target
            .express_scalar_product(&map.image(left), &map.image(right))
            .unwrap();
        for _ in 0..-power {
            polynomial = times(&polynomial, &form);
        }
    }
    Some(over(&base, polynomial))
}

/// A polynomial in the slots (exponents -> coefficient) times `a.b = c + Sum_i
/// r_i D_i`.
fn times(
    polynomial: &BTreeMap<Vec<i64>, Rational>,
    form: &AffineSlotForm,
) -> BTreeMap<Vec<i64>, Rational> {
    let mut next: BTreeMap<Vec<i64>, Rational> = BTreeMap::new();
    for (exponents, weight) in polynomial {
        let mut add = |exponents: Vec<i64>, value: Rational| {
            let entry = next.entry(exponents).or_insert_with(Rational::zero);
            *entry = &*entry + &value;
        };
        if !form.constant.is_zero() {
            add(exponents.clone(), weight * &form.constant);
        }
        for (s, factor) in &form.terms {
            let mut raised = exponents.clone();
            raised[usize::from(*s)] += 1;
            add(raised, weight * factor);
        }
    }
    next.retain(|_, value| !value.is_zero());
    next
}

/// The keys `base - exponents` of a numerator polynomial over denominators `base`.
fn over(base: &[i64], polynomial: BTreeMap<Vec<i64>, Rational>) -> Vec<(Vec<i64>, Rational)> {
    (polynomial.into_iter())
        .map(|(exponents, weight)| {
            let powers = base.iter().zip(&exponents).map(|(b, e)| b - e).collect();
            (powers, weight)
        })
        .collect()
}

/// 100 nonzero 4L inputs (random top, random subsector, dots <= 2, rank <= 3
/// on the other lines and the ISPs, odd ranks included; zero draws redrawn),
/// each against its image under a random non-identity unimodular map onto
/// the same top or another 4L top: the same terms, exactly.
#[test]
#[ignore = "slow: needs the scope 1-4 package (RUSTRED_P_LIBRARY_1_4); optimised"]
fn four_loop_reductions_are_invariant_under_unimodular_loop_momentum_maps() {
    let library = scope_four();
    let wanted: usize = setting("ROUTING_INPUTS", 100);
    let four = tops(library, 4);
    let mut rng = SplitMix::new(setting("ROUTING_SEED", 0x0418_7a11_u64));
    let start = Instant::now();
    let (mut checked, mut zero_draws, mut no_map, mut across, mut odd) = (0, 0, 0, 0, 0);
    let mut targets_used = BTreeSet::new();
    while checked < wanted {
        assert!(zero_draws + no_map < 20 * wanted, "too many redraws");
        let source = library.presentation(four[rng.below(four.len() as u64) as usize]);
        let dots = rng.below(3) as u32;
        let rank = rng.below(4) as u32;
        let powers = random_powers(source, None, dots, rank, &mut rng);
        if sector(library, source, &powers).1 {
            zero_draws += 1;
            continue;
        }
        let lines: Vec<usize> = (0..powers.len()).filter(|&s| powers[s] > 0).collect();
        let momenta = slot_factors(source);
        let mut order = four.clone();
        for i in (1..order.len()).rev() {
            order.swap(i, rng.below(i as u64 + 1) as usize);
        }
        let mut maps = Vec::new();
        for &target in &order {
            let found = line_maps(
                &momenta,
                &lines,
                library.presentation(target),
                target == source.id,
                16,
            );
            if !found.is_empty() {
                maps = found
                    .into_iter()
                    .map(|(map, sigma)| (target, map, sigma))
                    .collect();
                break;
            }
        }
        if maps.is_empty() {
            no_map += 1;
            continue;
        }
        let (target, map, sigma) = maps.swap_remove(rng.below(maps.len() as u64) as usize);
        let ours = reduce(
            library,
            vec![(source.id, key(&powers), library.coefficient_context().one())],
        );
        let ours = nonzero_terms(&ours);
        if ours.is_empty() {
            zero_draws += 1;
            continue;
        }
        let image = transport(
            &momenta,
            &powers,
            &map,
            &sigma,
            library.presentation(target),
        )
        .unwrap();
        let theirs = nonzero_terms(&reduce(library, inputs_of(library, target, &image)));
        assert_eq!(
            ours,
            theirs,
            "{} {powers:?} and its image on {} under {map:?} ({} keys)",
            source.id.0,
            target.0,
            image.len()
        );
        checked += 1;
        across += usize::from(target != source.id);
        odd += usize::from(rank_of(&powers) % 2 == 1);
        targets_used.insert(target);
    }
    println!(
        "routing invariance: {checked} nonzero 4L inputs = their images under unimodular maps \
         ({across} onto another top, {} onto the same; {odd} of odd rank; targets {}/{}); \
         {zero_draws} zero draws, {no_map} without a map redrawn; {:.1} s",
        checked - across,
        targets_used.len(),
        four.len(),
        start.elapsed().as_secs_f64()
    );
    assert!(across > 0 && checked - across > 0 && odd > 0);
}

// ---------------------------------------------------------------------------
// (3) Exactness: RustRed's IBP rows, the rules-study oracles, P0 SpIRed
// ---------------------------------------------------------------------------

/// RustRed's ordinary IBP rows of a presentation's family, with the context
/// that specializes them.
type IbpRows = Arc<(IndexedCoefficientContext, Vec<ParametricRelation>)>;

/// The rows of `p`, generated once per presentation.
fn ibp_rows(p: &Presentation) -> IbpRows {
    static ROWS: OnceLock<Mutex<HashMap<PresentationId, IbpRows>>> = OnceLock::new();
    let cache = ROWS.get_or_init(Default::default);
    if let Some(rows) = cache.lock().unwrap().get(&p.id) {
        return Arc::clone(rows);
    }
    let generator = ParametricIbpGenerator::try_new(&p.family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len()).map(|row| batch.generate(row)).collect();
    let relations = batch.complete(rows).unwrap().into_relations();
    let rows = Arc::new((generator.context().clone(), relations));
    cache.lock().unwrap().insert(p.id, Arc::clone(&rows));
    rows
}

/// A random combination of every ordinary IBP row of `p`'s family at `seed`,
/// as reduction inputs (keys of `p`), with the row count.
fn ibp_combination(
    library: &PLibrary,
    p: &Presentation,
    seed: &[i64],
    rng: &mut SplitMix,
) -> (Vec<Input>, usize) {
    let context = library.coefficient_context();
    let rows = ibp_rows(p);
    let (indexed, relations) = &*rows;
    let mut sum: BTreeMap<Vec<i64>, Coefficient> = BTreeMap::new();
    for relation in relations {
        let weight = context.integer(1 + rng.below(97) as i64);
        for (shift, value) in relation.terms() {
            let (value, _) = indexed
                .specialize(value, seed, IndexedAlgebraLimits::default())
                .unwrap();
            if value.is_zero() {
                continue;
            }
            let powers: Vec<i64> = seed
                .iter()
                .zip(shift.values())
                .map(|(a, b)| a + b)
                .collect();
            assert!(
                (p.slots.iter().zip(&powers))
                    .all(|(slot, &n)| n <= 0 || matches!(slot, Slot::Line(_))),
                "a row term with a positive ISP power: {powers:?}"
            );
            let term = context.try_mul(&value, &weight, exact()).unwrap();
            let entry = sum.entry(powers).or_insert_with(|| context.integer(0));
            *entry = context.try_add(entry, &term, exact()).unwrap();
        }
    }
    let inputs: Vec<Input> = (sum.into_iter())
        .filter(|(_, c)| !c.is_zero())
        .map(|(powers, c)| (p.id, key(&powers), c))
        .collect();
    (inputs, relations.len())
}

/// The arithmetic of the exactness reductions (`P_VALIDATE4_ARITHMETIC`):
/// `modular_polynomial` by default (vakint's default; exact in `Q(d)` by
/// reconstruction, equal to Exact by
/// [`exact_equals_both_modular_arithmetics_and_threads_are_bit_identical_at_four_loops`];
/// Exact itself takes minutes on some 11-line 4L seeds), or `exact`.
fn exactness_arithmetic() -> OrderedWeightArithmetic {
    match setting("ARITHMETIC", "modular_polynomial".to_owned()).as_str() {
        "exact" => OrderedWeightArithmetic::Exact,
        "modular_polynomial" => modular_polynomial(1),
        other => panic!("P_VALIDATE4_ARITHMETIC={other}"),
    }
}

/// One exactness seed: its owner (or top), presentation and powers.
struct Seed {
    what: String,
    presentation: PresentationId,
    powers: Vec<i64>,
}

/// What one seed gave.
struct SeedRow {
    what: String,
    rows: usize,
    keys: usize,
    seed_nonzero: bool,
    rank: i64,
    seconds: f64,
}

/// `PLib(x) = Sum_j e_j PLib(T_j)` for RustRed's own IBP rows: at every
/// owner of the program (`P_VALIDATE4_IBP_SEEDS` seeds each, default 2: the
/// owner sector's corner with random dots <= 2 and numerator rank 0..=3 off
/// the support, odd and even) and at 4 random subsectors of each 4L top
/// (negative powers, rank <= 3), a random combination of all `L (L + 1)`
/// ordinary IBP rows reduces to zero exactly in `Q(d)` (`exactness_arithmetic`). Every
/// owner is a seed site, so every owner and every 4L node class is checked;
/// the seeds' keys are traced (Route keys through `q -> -q` routes counted).
#[test]
#[ignore = "slow: needs the scope 1-4 package (RUSTRED_P_LIBRARY_1_4); optimised"]
fn ibp_rows_reduce_to_zero_at_every_owner_and_on_random_four_loop_subsectors() {
    let library = scope_four();
    let per_owner: usize = setting("IBP_SEEDS", 2);
    let stride: usize = setting("IBP_STRIDE", 1);
    let workers: usize = setting("WORKERS", 4);
    let mut rng = SplitMix::new(setting("IBP_SEED", 0x0418_1b9a_u64));
    let mut seeds = Vec::new();
    for (position, owner) in library.owners().enumerate() {
        if position % stride != 0 {
            continue;
        }
        let record = &library.census().classes[&owner.spec.class];
        let (presentation, support) = record.owner;
        let p = library.presentation(presentation);
        for s in 0..per_owner {
            let rank = if s % 2 == 0 {
                2 * rng.below(2)
            } else {
                1 + 2 * rng.below(2)
            };
            let powers =
                random_powers(p, Some(support), rng.below(3) as u32, rank as u32, &mut rng);
            seeds.push(Seed {
                what: format!(
                    "L{} {:?} {}",
                    record.key.loops,
                    owner.spec.lane,
                    record.key.label()
                ),
                presentation,
                powers,
            });
        }
    }
    let owner_seeds = seeds.len();
    for top in tops(library, 4) {
        let p = library.presentation(top);
        let mut drawn = 0;
        while drawn < 4 {
            let powers = random_powers(p, None, rng.below(2) as u32, rng.below(4) as u32, &mut rng);
            if sector(library, p, &powers).1 {
                continue;
            }
            seeds.push(Seed {
                what: format!("top {}", top.0),
                presentation: top,
                powers,
            });
            drawn += 1;
        }
    }
    let start = Instant::now();
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<SeedRow>> = Mutex::new(Vec::new());
    let combination_seed = rng.next_u64();
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(seed) = seeds.get(index) else {
                        break;
                    };
                    let begin = Instant::now();
                    eprintln!("IBP seed {index} {} {:?}: start", seed.what, seed.powers);
                    let p = library.presentation(seed.presentation);
                    let mut local = SplitMix::new(combination_seed ^ index as u64);
                    let (inputs, rows) = ibp_combination(library, p, &seed.powers, &mut local);
                    let keys = inputs.len();
                    let built = begin.elapsed().as_secs_f64();
                    let reduction = reduce(library, inputs);
                    assert!(
                        nonzero_terms(&reduction).is_empty(),
                        "{}: IBP rows at {:?} do not reduce to zero: {:?}",
                        seed.what,
                        seed.powers,
                        nonzero_terms(&reduction)
                    );
                    let one = library.coefficient_context().one();
                    let alone = reduce(library, vec![(seed.presentation, key(&seed.powers), one)]);
                    eprintln!(
                        "IBP seed {index} {} {:?}: {rows} rows, {keys} keys, rows built in {built:.1} s, \
                         {:.1} s",
                        seed.what,
                        seed.powers,
                        begin.elapsed().as_secs_f64()
                    );
                    results.lock().unwrap().push(SeedRow {
                        what: seed.what.clone(),
                        rows,
                        keys,
                        seed_nonzero: !nonzero_terms(&alone).is_empty(),
                        rank: rank_of(&seed.powers),
                        seconds: begin.elapsed().as_secs_f64(),
                    });
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
    assert_eq!(results.len(), seeds.len());
    let keys: Vec<(PresentationId, Vec<i64>)> = seeds
        .iter()
        .map(|s| (s.presentation, s.powers.clone()))
        .collect();
    let trace = descent::trace(library, &keys, ReductionLimits::default()).unwrap();
    assert!(trace.is_clean(), "{:?}", trace.samples);
    let nonzero = results.iter().filter(|r| r.seed_nonzero).count();
    let odd = results
        .iter()
        .filter(|r| r.rank % 2 == 1 && r.seed_nonzero)
        .count();
    let slowest = results
        .iter()
        .max_by(|a, b| a.seconds.total_cmp(&b.seconds))
        .unwrap();
    if let Some(directory) = out_dir() {
        std::fs::create_dir_all(&directory).unwrap();
        let rows: Vec<Value> = (results.iter())
            .map(|r| json!([r.what, r.rows, r.keys, r.seed_nonzero, r.rank, r.seconds]))
            .collect();
        write_json(
            &directory.join("ibp_rows.json"),
            &json!({"identity": library.identity(), "seeds": rows}),
        );
    }
    println!(
        "IBP rows: {} seeds ({owner_seeds} on {} owners, {} on 4L top subsectors), {} rows, {} keys, \
         every combination zero; {nonzero} seeds nonzero ({odd} of odd rank); trace {} route + {} apply \
         keys, {} owners applied, {} route keys through q -> -q routes; slowest {} {:.1} s; {:.1} s wall",
        results.len(),
        library.owners().len().div_ceil(stride),
        results.len() - owner_seeds,
        results.iter().map(|r| r.rows).sum::<usize>(),
        results.iter().map(|r| r.keys).sum::<usize>(),
        trace.route_keys,
        trace.apply_keys,
        trace.applied.len(),
        trace.flipped_route_keys,
        slowest.what,
        slowest.seconds,
        start.elapsed().as_secs_f64()
    );
    assert!(
        nonzero * 2 >= results.len(),
        "most seeds are nonzero integrals"
    );
}

/// The rules-study statements on the owners (rules_study/M2_M3_INPUTS.md
/// sections 3 and 4a): R_INT, one residual on each of the 8 integer 4L
/// master-carrying owners, the corner key (every line of the owner's graph at
/// power 1); the knock-outs T148 bubu (VERIFIED), T015 fastar3 and T041
/// lastar5 (VERIFIED), T003 t1star34 (F1) and T016 bustar5 (R_GT) carry no
/// residual.
#[test]
#[ignore = "slow: needs the scope 1-4 package (RUSTRED_P_LIBRARY_1_4); optimised"]
fn the_rules_study_residual_statements_hold_on_the_owners() {
    let library = scope_four();
    let owners = custom_owners(library);
    let residuals = |name: &str| -> Vec<Vec<i64>> {
        let class = owners[name];
        let owner = library.owner(class).unwrap();
        (owner.spec.terminals.iter())
            .filter_map(|id| match &library.terminals()[id.0 as usize] {
                PTerminal::Residual { owner, key } => {
                    assert_eq!(*owner, class, "{name}");
                    Some(key.powers().to_vec())
                }
                PTerminal::Unit => None,
            })
            .collect()
    };
    for name in ["bebe", "cross", "lala", "nono", "no6", "no1", "no2", "haha"] {
        let record = &library.census().classes[&owners[name]];
        let (presentation, support) = record.owner;
        let p = library.presentation(presentation);
        let corner: Vec<i64> = (0..p.slots.len())
            .map(|slot| i64::from(support >> slot & 1 == 1))
            .collect();
        assert_eq!(residuals(name), [corner], "R_INT: {name}");
    }
    for name in ["bubu", "fastar3", "lastar5", "t1star34", "bustar5"] {
        assert_eq!(residuals(name), Vec::<Vec<i64>>::new(), "knock-out {name}");
    }
    println!("R_INT 8/8 owners: one residual, the corner; knock-outs 5/5: no residual");
}

/// A table key `I(n0, n1-1, 2, ..)` at the point `n`.
fn table_key(text: &str, n: &[i64]) -> Vec<i64> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    (inner.split(','))
        .map(|entry| {
            let entry = entry.trim();
            match entry.strip_prefix('n') {
                None => entry.parse().unwrap(),
                Some(rest) => {
                    let end = rest.find(['+', '-']).unwrap_or(rest.len());
                    let index: usize = rest[..end].parse().unwrap();
                    let offset: i64 = if end == rest.len() {
                        0
                    } else {
                        rest[end..].parse().unwrap()
                    };
                    n[index] + offset
                }
            }
        })
        .collect()
}

/// A table expression (Symbolica's printed form in `n0, n1, ..` and `d`) at
/// the point `n`; `None` at a pole.
fn table_value(context: &CoefficientContext, text: &str, n: &[i64]) -> Option<Coefficient> {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    let mut previous = ' ';
    while let Some(c) = chars.next() {
        if c == 'n'
            && !previous.is_ascii_alphanumeric()
            && chars.peek().is_some_and(char::is_ascii_digit)
        {
            let mut digits = String::new();
            while let Some(&digit) = chars.peek().filter(|c| c.is_ascii_digit()) {
                digits.push(digit);
                chars.next();
            }
            out.push_str(&format!("({})", n[digits.parse::<usize>().unwrap()]));
            previous = ')';
        } else {
            out.push(c);
            previous = c;
        }
    }
    let parse = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustred::propagator::basis::parse_coefficient(context, &out, exact())
    }));
    match parse {
        Ok(Ok(value)) => Some(value),
        _ => None,
    }
}

/// The T148 bubu table of the rules study (`RUSTRED_T148_TABLE`, frozen,
/// table gate items 1-4 PASS; `rules_study/s5-tables/s5-dump/tables/
/// T148.table.jsonl`): every rule's identity `I(target) = Sum_t c_t I(key_t)`
/// at `P_VALIDATE4_T148_POINTS` random points of its domain (default 2; free
/// axes small: lines 1 or 2, numerators 0 or -1; the
/// chart, its fixed values, off its exception branches and off coefficient
/// poles) holds through the library: the table's family (`gram(q,q,1)`) is
/// carried onto bubu's owner presentation (`q^2 = -1`) by a unimodular map of
/// its 9 lines onto the owner's (each term times `(-1)^(sum n)` for the sign of
/// `q^2`, M2_M3_INPUTS A11; an exit key whose extra line is no line of that
/// top goes onto another 4L top that has it), and the reduction of the
/// difference is zero in `Q(d)`.
#[test]
#[ignore = "slow: needs the scope 1-4 package and RUSTRED_T148_TABLE; optimised"]
fn the_frozen_t148_bubu_table_holds_through_the_library() {
    let library = scope_four();
    let context = library.coefficient_context();
    let path =
        std::env::var_os("RUSTRED_T148_TABLE").expect("RUSTRED_T148_TABLE names T148.table.jsonl");
    let points: usize = setting("T148_POINTS", 2);
    let workers: usize = setting("WORKERS", 4);
    let text = std::fs::read_to_string(path).unwrap();
    let records: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let header = &records[0];
    assert_eq!(header["schema"], "table_dump.v1");
    assert_eq!(header["family"], "T148");
    let source = header["family_source"].as_str().unwrap();
    assert!(source.contains("gram(q,q,1)"), "the table's q^2");
    // prop(Dk,(momentum)^2,line): rows over (k1..k4, q).
    let mut momenta = Vec::new();
    let mut lines = Vec::new();
    for prop in source.split("prop(").skip(1) {
        let inside = prop.split_once(",(").unwrap().1;
        let (momentum, rest) = inside.split_once(")^2,").unwrap();
        let mut row = vec![0_i64; 5];
        for (sign, name) in momentum
            .replace('-', "+-")
            .split('+')
            .filter(|t| !t.is_empty())
            .map(|t| t.strip_prefix('-').map_or((1, t), |name| (-1, name)))
        {
            let column = if name == "q" {
                4
            } else {
                name[1..].parse::<usize>().unwrap() - 1
            };
            row[column] += sign;
        }
        if rest.starts_with('1') {
            lines.push(momenta.len());
        }
        momenta.push((row.clone(), row));
    }
    assert_eq!((momenta.len(), lines.len()), (14, 9));
    let bubu = custom_owners(library)["bubu"];
    let (owner, support) = library.census().classes[&bubu].owner;
    // Keys of the table may carry a denominator off bubu's sector (an exit
    // term into a ten-line sector of the top): reduce on the top itself.
    let presentation = match &library.presentation(owner).kind {
        PresentationKind::Induced { top, .. } => *top,
        _ => owner,
    };
    let target = library.presentation(presentation);
    assert_eq!(target.slots, library.presentation(owner).slots);
    // Of the maps onto bubu's lines, the one carrying most numerator props onto
    // a slot of the target (fewest expansions).
    let slot_rows: Vec<Vec<i64>> = slot_factors(target)
        .into_iter()
        .filter(|(left, right)| left == right)
        .map(|(left, _)| left)
        .collect();
    let on_slot = |map: &LoopMap| {
        (0..momenta.len())
            .filter(|slot| !lines.contains(slot))
            .filter(|&slot| {
                let image = map.image(&momenta[slot].0);
                let negated: Vec<i64> = image.iter().map(|v| -v).collect();
                slot_rows.contains(&image) || slot_rows.contains(&negated)
            })
            .count()
    };
    let (map, sigma) = line_maps(&momenta, &lines, target, false, 4096)
        .into_iter()
        .filter(|(_, sigma)| sigma.values().fold(0_u64, |m, &s| m | 1 << s) == support)
        .max_by_key(|(map, _)| on_slot(map))
        .expect("the table's lines map onto bubu's");
    println!(
        "T148 map onto bubu's presentation {}: {} of 5 numerator props onto a slot",
        presentation.0,
        on_slot(&map)
    );
    let rules: Vec<&Value> = records.iter().filter(|r| r["type"] == "rule").collect();
    assert_eq!(rules.len(), 139);
    let chart_guards: Vec<String> =
        (records.iter().find(|r| r["type"] == "chart").unwrap()["guards"])
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g.as_str().unwrap().to_owned())
            .collect();
    let mut rng = SplitMix::new(setting("T148_SEED", 0x0418_7148_u64));
    let mut cases: Vec<(usize, Vec<i64>)> = Vec::new();
    let mut skipped = 0;
    for (index, rule) in rules.iter().enumerate() {
        assert!(rule["equations"].as_array().unwrap().is_empty());
        let mut found = 0;
        for _ in 0..200 {
            if found == points {
                break;
            }
            let n: Vec<i64> = (0..14)
                .map(|axis| match rule["fixed"][axis].as_i64() {
                    Some(value) => value,
                    None if chart_guards[axis] == "n>=1" => 1 + i64::from(rng.below(3) == 0),
                    None => -i64::from(rng.below(3) == 0),
                })
                .collect();
            let excepted = rule["exceptions"].as_array().unwrap().iter().any(|branch| {
                branch.as_array().unwrap().iter().all(|equation| {
                    table_value(context, equation.as_str().unwrap(), &n).is_none_or(|v| v.is_zero())
                })
            });
            let regular = rule["rhs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|term| table_value(context, term[1].as_str().unwrap(), &n).is_some());
            if !excepted && regular && !cases.contains(&(index, n.clone())) {
                cases.push((index, n));
                found += 1;
            }
        }
        skipped += points - found;
    }
    let start = Instant::now();
    let next = AtomicUsize::new(0);
    let nonzero = AtomicUsize::new(0);
    let off_top = AtomicUsize::new(0);
    let one = context.one();
    // Exit keys off bubu's top: a 4L top and map per denominator set, found once.
    let four = tops(library, 4);
    let other_tops: Mutex<HashMap<Vec<usize>, Option<(PresentationId, LoopMap, Sigma)>>> =
        Mutex::new(HashMap::new());
    let elsewhere = |powers: &[i64]| -> Option<(PresentationId, Vec<(Vec<i64>, Rational)>)> {
        let lines: Vec<usize> = (0..powers.len()).filter(|&s| powers[s] > 0).collect();
        let found = other_tops.lock().unwrap().get(&lines).cloned();
        let found = found.unwrap_or_else(|| {
            let found = four.iter().find_map(|&top| {
                let p = library.presentation(top);
                (line_maps(&momenta, &lines, p, false, 1).into_iter().next())
                    .map(|(map, sigma)| (top, map, sigma))
            });
            other_tops.lock().unwrap().insert(lines, found.clone());
            found
        })?;
        let (top, map, sigma) = found;
        transport(&momenta, powers, &map, &sigma, library.presentation(top))
            .map(|image| (top, image))
    };
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let position = next.fetch_add(1, Ordering::Relaxed);
                    let Some((index, n)) = cases.get(position) else {
                        break;
                    };
                    let rule = rules[*index];
                    let mut terms =
                        vec![(table_key(rule["target"].as_str().unwrap(), n), one.clone())];
                    for term in rule["rhs"].as_array().unwrap() {
                        let value = table_value(context, term[1].as_str().unwrap(), n).unwrap();
                        let value = context.try_neg(&value, exact()).unwrap();
                        terms.push((table_key(term[0].as_str().unwrap(), n), value));
                    }
                    // Each integral of the identity is carried over on its own: onto bubu's top
                    // by the map of the table's lines, or, for an exit key whose extra line is
                    // no line of that top, onto any 4L top whose lines take its denominators.
                    let carried: Option<Vec<(PresentationId, Vec<(Vec<i64>, Rational)>)>> = (terms
                        .iter())
                    .map(|(powers, _)| {
                        transport(&momenta, powers, &map, &sigma, target)
                            .map(|image| (presentation, image))
                            .or_else(|| elsewhere(powers))
                    })
                    .collect();
                    let Some(carried) = carried else {
                        off_top.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    let mut sum: BTreeMap<(PresentationId, Vec<i64>), Coefficient> =
                        BTreeMap::new();
                    for ((powers, value), (onto, image)) in terms.iter().zip(carried) {
                        let sign = if powers.iter().sum::<i64>() % 2 == 0 {
                            1
                        } else {
                            -1
                        };
                        for (key, weight) in image {
                            let weight = coefficient_of(context, &(&weight * &rational(sign, 1)));
                            let term = context.try_mul(value, &weight, exact()).unwrap();
                            let entry =
                                (sum.entry((onto, key))).or_insert_with(|| context.integer(0));
                            *entry = context.try_add(entry, &term, exact()).unwrap();
                        }
                    }
                    let inputs: Vec<Input> = (sum.into_iter())
                        .filter(|(_, c)| !c.is_zero())
                        .map(|((onto, powers), c)| (onto, key(&powers), c))
                        .collect();
                    let begin = Instant::now();
                    let size = inputs.len();
                    eprintln!("T148 rule {index} at {n:?}: {size} keys: start");
                    let reduction = reduce(library, inputs);
                    eprintln!(
                        "T148 rule {index} at {n:?}: {size} keys, {:.1} s",
                        begin.elapsed().as_secs_f64()
                    );
                    assert!(
                        nonzero_terms(&reduction).is_empty(),
                        "T148 rule {index} at {n:?}: {:?}",
                        nonzero_terms(&reduction)
                    );
                    let target_key = table_key(rule["target"].as_str().unwrap(), n);
                    let alone: Vec<Input> = transport(&momenta, &target_key, &map, &sigma, target)
                        .unwrap()
                        .into_iter()
                        .map(|(powers, w)| {
                            (presentation, key(&powers), coefficient_of(context, &w))
                        })
                        .collect();
                    if !nonzero_terms(&reduce(library, alone)).is_empty() {
                        nonzero.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    println!(
        "T148 table: {} rule identities ({} rules, {skipped} points not found in a domain, {} with a key \
         on no 4L top's lines) hold through the library; {} nonzero targets; map {map:?}; {:.1} s",
        cases.len() - off_top.load(Ordering::Relaxed),
        rules.len(),
        off_top.load(Ordering::Relaxed),
        nonzero.load(Ordering::Relaxed),
        start.elapsed().as_secs_f64()
    );
    assert!(nonzero.load(Ordering::Relaxed) > 0);
}

/// `count` distinct rational `d` over the prime 101: no integer, no half-integer.
fn dimensions(rng: &mut SplitMix, count: usize) -> Vec<Rational> {
    let mut points = Vec::with_capacity(count);
    while points.len() < count {
        let d = rational(
            101 * (1 + rng.below(19) as i64) + 1 + rng.below(100) as i64,
            101,
        );
        if !points.contains(&d) {
            points.push(d);
        }
    }
    points
}

/// `value` at a rational `d`.
fn at(value: &Coefficient, d: &Rational) -> Rational {
    value.evaluate_with_coeff_map(|c| Rational::from(c.clone()), std::slice::from_ref(d), &Q)
}

/// A reduction onto the terminals in the tag basis at `d`.
fn tag_values(reduction: &PReduction, d: &Rational) -> BTreeMap<(String, GammaTag), Rational> {
    let mut values = BTreeMap::new();
    for term in &reduction.terms {
        let entry = values
            .entry((format!("{:?}", term.basis), term.tag.clone()))
            .or_insert_with(Rational::zero);
        *entry = &*entry + &at(&term.coefficient, d);
    }
    values.retain(|_, value: &mut Rational| !value.is_zero());
    values
}

fn add_scaled(
    sum: &mut BTreeMap<(String, GammaTag), Rational>,
    values: &BTreeMap<(String, GammaTag), Rational>,
    factor: &Rational,
) {
    for (basis, value) in values {
        let entry = sum.entry(basis.clone()).or_insert_with(Rational::zero);
        *entry = &*entry + &(value * factor);
    }
    sum.retain(|_, value| !value.is_zero());
}

/// The scalar products `k_i.k_j` (`i <= j < L`) and `k_i.q` (`j = L`).
fn scalar_products(p: &Presentation) -> Vec<(usize, usize)> {
    let loops = usize::from(p.loops);
    (0..loops)
        .flat_map(|i| (i..=loops).map(move |j| (i, j)))
        .collect()
}

/// `weight` times the denominators `base` times the scalar products
/// `products` (`k_i.k_j`, and `k_i.q` with `q` signed by `q_sign`), lowered
/// onto the slots of `p` and expanded.
fn lowered(
    p: &Presentation,
    base: &[i64],
    products: &[(usize, usize)],
    q_sign: i64,
    weight: Rational,
) -> Vec<(Vec<i64>, Rational)> {
    let loops = usize::from(p.loops);
    let unit = |column: usize| {
        let mut row = vec![0_i64; loops + 1];
        row[column] = if column == loops { q_sign } else { 1 };
        row
    };
    let mut polynomial = BTreeMap::from([(vec![0_i64; p.slots.len()], weight)]);
    for &(i, j) in products {
        let form = p.express_scalar_product(&unit(i), &unit(j)).unwrap();
        polynomial = times(&polynomial, &form);
    }
    over(base, polynomial)
}

/// A top family under the P0 lane: all-rank SpIRed rules on the downset of
/// the owner sector.
struct P0 {
    lane: DynamicLiteRedSession,
    masters: Vec<IntegralKey>,
}

impl P0 {
    fn new(p: &Presentation, support: u64, workers: usize) -> Self {
        let root: Vec<bool> = (0..p.slots.len()).map(|s| support >> s & 1 == 1).collect();
        let options = LiteRedOptions {
            workers,
            ..LiteRedOptions::default()
        };
        let (mut lane, _) =
            DynamicLiteRedSession::generate(p.family.clone(), &root, workers, true, options, None)
                .unwrap();
        lane.compress(&[]).unwrap();
        let masters = lane.masters();
        Self { lane, masters }
    }

    /// Rows of `keys` on the masters at `d`.
    fn rows(&mut self, keys: &[Vec<i64>], d: &Rational) -> Vec<Vec<Rational>> {
        let keys: Vec<IntegralKey> = keys.iter().map(|powers| key(powers)).collect();
        let ids = self.lane.add_targets(&keys).unwrap();
        assert_eq!(self.lane.statistics(&ids).uncovered, 0);
        self.lane
            .reduce_exact_at_point(std::slice::from_ref(d), &ids)
            .unwrap()
            .coefficients
    }
}

/// P0 SpIRed on the Forcer-blind crossed owners (`P_VALIDATE4_P0`, default
/// `cross,no1,no2` = T147, T152, T153; Forcer cannot check them whole-family):
/// in the owner's family, `P_VALIDATE4_P0_INPUTS` random inputs (default 60;
/// plain keys with random subsector masks of the owner sector and negative
/// powers, and inputs times scalar products with `q` of either sign, odd
/// ranks included) satisfy `PLib(x) == Sum_j e_j(d) PLib(T_j)` in the tag
/// basis at two random `d`, where `x = Sum_j e_j T_j` is P0's reduction onto
/// its masters. Count and seconds are printed per owner.
#[test]
#[ignore = "slow: P0 SpIRed in three 4L owner families (RUSTRED_P_LIBRARY_1_4); optimised"]
fn p0_spired_agrees_on_the_forcer_blind_crossed_owners() {
    let library = scope_four();
    let owners = custom_owners(library);
    let names: String = setting("P0", "cross,no1,no2".to_owned());
    let count: usize = setting("P0_INPUTS", 60);
    let workers: usize = setting("WORKERS", 4);
    let context = library.coefficient_context();
    let mut rng = SplitMix::new(setting("P0_SEED", 0x0418_0b0a_u64));
    let points = dimensions(&mut rng, 2);
    for name in names.split(',') {
        let class = owners[name];
        let (presentation, support) = library.census().classes[&class].owner;
        let p = library.presentation(presentation);
        let start = Instant::now();
        let mut p0 = P0::new(p, support, workers);
        let generated = start.elapsed().as_secs_f64();
        println!(
            "P0 {name} ({}): generated in {generated:.1} s, {} masters",
            label(library, class),
            p0.masters.len()
        );
        let mut inputs: Vec<(Vec<(Vec<i64>, Rational)>, i64)> = Vec::new();
        for index in 0..count {
            let kept = (line_slots(p).into_iter())
                .filter(|&s| support >> s & 1 == 1 && rng.below(5) != 0)
                .fold(0_u64, |mask, s| mask | 1 << s);
            let weight = rational(1 + rng.below(9) as i64, 1 + rng.below(4) as i64);
            if index % 2 == 0 {
                let rank = rng.below(5);
                let powers =
                    random_powers(p, Some(kept), rng.below(3) as u32, rank as u32, &mut rng);
                let rank = rank_of(&powers);
                inputs.push((vec![(powers, weight)], rank));
            } else {
                let base = random_powers(p, Some(kept), rng.below(3) as u32, 0, &mut rng);
                let types = scalar_products(p);
                let rank = rng.below(5) as usize;
                let products: Vec<(usize, usize)> = (0..rank)
                    .map(|_| types[rng.below(types.len() as u64) as usize])
                    .collect();
                let q_sign = if rng.below(2) == 0 { 1 } else { -1 };
                inputs.push((lowered(p, &base, &products, q_sign, weight), rank as i64));
            }
        }
        let keys: Vec<Vec<i64>> = (inputs.iter())
            .flat_map(|(terms, _)| terms.iter().map(|(powers, _)| powers.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let index: BTreeMap<&Vec<i64>, usize> =
            keys.iter().enumerate().map(|(i, k)| (k, i)).collect();
        let masters: Vec<PReduction> = (p0.masters.clone().into_iter())
            .map(|master| reduce(library, vec![(presentation, master, context.one())]))
            .collect();
        let reductions: Vec<PReduction> = (inputs.iter())
            .map(|(terms, _)| reduce(library, inputs_of(library, presentation, terms)))
            .collect();
        let (mut nonzero, mut odd) = (0, 0);
        for d in &points {
            let rows = p0.rows(&keys, d);
            let master_values: Vec<_> = masters.iter().map(|m| tag_values(m, d)).collect();
            for (position, (terms, rank)) in inputs.iter().enumerate() {
                let ours = tag_values(&reductions[position], d);
                let mut expected = BTreeMap::new();
                for (powers, weight) in terms {
                    for (master, coefficient) in rows[index[powers]].iter().enumerate() {
                        add_scaled(
                            &mut expected,
                            &master_values[master],
                            &(coefficient * weight),
                        );
                    }
                }
                assert_eq!(ours, expected, "P0 {name}: {terms:?} at d = {d}");
                if !ours.is_empty() {
                    nonzero += 1;
                    odd += usize::from(rank % 2 == 1);
                }
            }
        }
        let trace_keys: Vec<(PresentationId, Vec<i64>)> =
            keys.iter().map(|k| (presentation, k.clone())).collect();
        let trace = descent::trace(library, &trace_keys, ReductionLimits::default()).unwrap();
        assert!(trace.is_clean(), "{:?}", trace.samples);
        println!(
            "P0 {name}: {} inputs ({} keys) x {} d: {nonzero} nonzero comparisons ({odd} of odd rank) \
             equal; {} owners applied, {} route keys through q -> -q routes; {:.1} s (P0 generation \
             {generated:.1} s)",
            inputs.len(),
            keys.len(),
            points.len(),
            trace.applied.len(),
            trace.flipped_route_keys,
            start.elapsed().as_secs_f64()
        );
        assert!(nonzero > 0, "P0 {name}: every comparison zero");
    }
}

// ---------------------------------------------------------------------------
// (4) Arithmetic and threads (non-ignored)
// ---------------------------------------------------------------------------

/// Exact, ModularPolynomial and ModularSampled weights give the same terms and
/// nonzero conditions on small 4L inputs (the corners of the 4L ladder top and
/// of the crossed Custom owner T147, each with one numerator), and 1 and
/// 16 scheduler threads the same bits on the first (also both modular
/// arithmetics with 4 threads under 16 scheduler threads, vakint's shape).
/// The one place of these checks in M3. Says SKIPPED without the package.
#[test]
fn exact_equals_both_modular_arithmetics_and_threads_are_bit_identical_at_four_loops() {
    if package().is_none() {
        eprintln!("SKIPPED: {PACKAGE} is unset (the scope 1-4 package of M3-T8c)");
        return;
    }
    let library = scope_four();
    let context = library.coefficient_context();
    let ladder = library.top_presentation("P4L_LA4").unwrap();
    let p = library.presentation(ladder);
    let mut first = vec![0_i64; p.slots.len()];
    for slot in line_slots(p) {
        first[slot] = 1;
    }
    let isp = (0..p.slots.len())
        .find(|&s| matches!(p.slots[s], Slot::Isp(_)))
        .unwrap();
    first[isp] = -1;
    let cross = custom_owners(library)["cross"];
    let (owner, support) = library.census().classes[&cross].owner;
    let mut second: Vec<i64> = (0..library.presentation(owner).slots.len())
        .map(|slot| i64::from(support >> slot & 1 == 1))
        .collect();
    let numerator = (0..second.len()).find(|&s| second[s] == 0).unwrap();
    second[numerator] = -1;
    let inputs = [(ladder, first), (owner, second)];
    let configurations: [(&str, OrderedWeightArithmetic, usize); 6] = [
        ("Exact", OrderedWeightArithmetic::Exact, 1),
        ("ModularPolynomial", modular_polynomial(1), 1),
        ("ModularSampled", modular_sampled(1), 1),
        ("Exact x 16", OrderedWeightArithmetic::Exact, 16),
        ("ModularPolynomial(4) x 16", modular_polynomial(4), 16),
        ("ModularSampled(4) x 16", modular_sampled(4), 16),
    ];
    for (position, (presentation, powers)) in inputs.iter().enumerate() {
        let used = if position == 0 {
            &configurations[..]
        } else {
            &configurations[..3]
        };
        let mut exact: Option<PReduction> = None;
        for &(name, arithmetic, threads) in used {
            let start = Instant::now();
            let input = vec![(*presentation, key(powers), context.one())];
            let reduction = reduce_with(library, input, arithmetic, threads);
            println!(
                "{} {powers:?} {name}: {} terms, {:.2} s",
                presentation.0,
                reduction.terms.len(),
                start.elapsed().as_secs_f64()
            );
            match &exact {
                None => {
                    assert!(!nonzero_terms(&reduction).is_empty(), "{powers:?} is zero");
                    exact = Some(reduction);
                }
                Some(exact) => {
                    assert_eq!(exact.terms, reduction.terms, "{name}: {powers:?}");
                    assert_eq!(
                        exact.nonzero_conditions, reduction.nonzero_conditions,
                        "{name}: {powers:?}"
                    );
                }
            }
        }
    }
}
