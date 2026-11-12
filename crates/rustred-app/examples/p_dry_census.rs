//! Dry census (route R4a): the keys and edges of the forward pass of the
//! benchmark cells, for a base p library and candidate libraries that differ
//! from it only in some owners' rules, in seconds instead of minutes.
//!
//! The forward pass of a p reduction is the weight-independent trace: the
//! popped keys and the emitted children are those of the full reduction (the
//! modular record already decides exact cancellations with modular probes; the
//! replay never adds a key). This probe stops each request at the end of the
//! forward pass (as `P_CENSUS_FORWARD_ONLY=1` of `p_path_census`) and counts
//! keys (`processed_keys`) and edges (`emitted_terms`) from the driver's
//! progress, which equal `popped` and `edges` of the path census.
//!
//! The cost of a forward pass is mostly the steps. The base library's steps
//! are recorded in an in-memory step cache while its cells run; a candidate
//! library reuses every recorded shard except the Apply shards of the owners
//! whose payload (`owners/<class>.rrbin`) or ordering entry differs from the
//! base, so only the swapped owners' steps are computed again. This is sound
//! when the candidate changes only those owners' rules (same family,
//! presentation, terminals, routes, maps): the probe refuses a candidate whose
//! other package files differ (`census.rrbin`, `routes.rrbin`, `orbits.rrbin`,
//! `families/`, `layer/`, `topology/`), unless `DRY_ALLOW_OTHER=1`.
//!
//! usage: p_dry_census <fixture json> <workers> <out jsonl> <base lib> [<candidate lib> ...]
//!
//! Environment:
//!   DRY_CELLS=NO3d2r8,...   cells to run (default: NO1-3 x d0-2 x r6/r8, largest first)
//!   DRY_CACHE=off           no step cache (every request computes its steps: the reference)
//!   DRY_ARITH=modular_polynomial|modular_sampled|exact (default modular_polynomial)
//!   DRY_REPEAT_BASE=1       run the base cells a second time on the warm cache (hit-rate check)
//!   P_ORBIT_CANON=on|off    as p_path_census (unset: the library default)
//!
//! Each line of <out jsonl>: library, cell, keys, edges, merged, cancelled,
//! peak_frontier, terminal_keys, seconds, cpu-independent counters of the cache
//! (hits, misses), status.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::family::IntegralKey;
use rustred::propagator::PresentationId;
use rustred::propagator::error::PError;
use rustred::propagator::library::{PInspector, PLibrary, PReductionRequest, PUncoveredPolicy};
use rustred::propagator::presentation::{AffineSlotForm, Slot};
use rustred::reduction::{
    ModularCommon, OrderedPhaseBoundary, OrderedPhaseEvent, OrderedSchedulerOptions,
    OrderedStepCache, OrderedStepCacheOptions, OrderedStepStage, OrderedWeightArithmetic,
    PolynomialOptions, ReductionLimits, SampledOptions,
};
use rustred::solver::CandidateOrderedReductionProgress;
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};
use symbolica::prelude::Rational;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

type Input = (PresentationId, IntegralKey, Coefficient);

struct Cell {
    name: String,
    loops: usize,
    tops: Vec<(String, Vec<i64>)>,
    dots: Vec<(usize, usize)>,
}

fn rows(dot: (usize, usize), loops: usize) -> (Vec<i64>, Vec<i64>) {
    let unit = |index: usize| {
        let mut row = vec![0; loops + 1];
        row[index] = 1;
        row
    };
    (unit(dot.0), unit(dot.1))
}

/// Cell `fixture_cell` (e.g. `L4_d2_r8`) restricted to the top `top`.
fn cell(fixture: &serde_json::Value, name: &str, fixture_cell: &str, top: &str) -> Cell {
    let cell = fixture["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["name"] == fixture_cell)
        .unwrap_or_else(|| panic!("cell {fixture_cell}"));
    let loops = cell["loops"].as_u64().unwrap() as usize;
    let tops: Vec<(String, Vec<i64>)> = cell["tops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["family"].as_str().unwrap().to_owned(),
                t["powers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p.as_i64().unwrap())
                    .collect(),
            )
        })
        .filter(|(family, _)| family == top)
        .collect();
    assert!(!tops.is_empty(), "no top {top} in {fixture_cell}");
    let numerator = cell["numerator"].as_str().unwrap().to_owned();
    let mut dots = Vec::new();
    if numerator != "1" {
        for factor in numerator.split('*') {
            let inner = factor.trim_start_matches("dot(").trim_end_matches(')');
            let (a, b) = inner.split_once("),").unwrap();
            let index = |text: &str| -> usize {
                text.trim_start_matches("k(")
                    .trim_end_matches(')')
                    .parse()
                    .unwrap()
            };
            let a = index(a);
            let b = if b.starts_with("p(") {
                loops + 1
            } else {
                index(b)
            };
            dots.push((a - 1, b - 1));
        }
    }
    Cell {
        name: name.to_owned(),
        loops,
        tops,
        dots,
    }
}

fn multiply(
    polynomial: &BTreeMap<Vec<u32>, Rational>,
    form: &AffineSlotForm,
) -> BTreeMap<Vec<u32>, Rational> {
    let mut product = BTreeMap::<Vec<u32>, Rational>::new();
    let mut add = |monomial: Vec<u32>, value: Rational| {
        let entry = product.entry(monomial).or_insert_with(Rational::zero);
        *entry += &value;
    };
    for (monomial, coefficient) in polynomial {
        if !form.constant.is_zero() {
            add(monomial.clone(), coefficient * &form.constant);
        }
        for (slot, value) in &form.terms {
            let mut shifted = monomial.clone();
            shifted[usize::from(*slot)] += 1;
            add(shifted, coefficient * value);
        }
    }
    product.retain(|_, value| !value.is_zero());
    product
}

fn coefficient(context: &CoefficientContext, value: &Rational) -> Coefficient {
    let n = i64::try_from(value.numerator_ref().clone()).unwrap();
    let d = i64::try_from(value.denominator_ref().clone()).unwrap();
    context
        .try_div(
            &context.integer(n),
            &context.integer(d),
            ReductionLimits::default().exact_algebra,
        )
        .unwrap()
}

/// vakint's lowering of `numerator * top(powers)` (as p_path_census).
fn inputs(library: &PLibrary, cell: &Cell) -> Vec<Input> {
    let mut lowered = BTreeMap::<(PresentationId, Vec<i64>), Rational>::new();
    for (family, powers) in &cell.tops {
        let top = library
            .top_presentation(family)
            .unwrap_or_else(|| panic!("top {family}"));
        let presentation = library.presentation(top);
        let arity = presentation.slots.len();
        let mut product = BTreeMap::from([(vec![0u32; arity], Rational::one())]);
        for &dot in &cell.dots {
            let (a, b) = rows(dot, cell.loops);
            let form = presentation.express_scalar_product(&a, &b).unwrap();
            product = multiply(&product, &form);
        }
        for (monomial, value) in product {
            let key: Vec<i64> = (0..arity)
                .map(|slot| {
                    powers.get(slot).copied().unwrap_or(0).max(0) - i64::from(monomial[slot])
                })
                .collect();
            *lowered.entry((top, key)).or_insert_with(Rational::zero) += &value;
        }
    }
    let context = library.coefficient_context();
    lowered
        .into_iter()
        .filter(|(_, value)| !value.is_zero())
        .map(|((top, key), value)| {
            (
                top,
                IntegralKey::try_new(key).unwrap(),
                coefficient(context, &value),
            )
        })
        .collect()
}

fn arithmetic(name: &str) -> OrderedWeightArithmetic {
    let common = ModularCommon {
        threads: 1,
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

/// Stops the request at the end of the forward pass and keeps the progress.
struct Stop {
    last_phase: usize,
    progress: Option<CandidateOrderedReductionProgress>,
}

const STOP: &str = "dry census: stopped before the replay";

impl PInspector for Stop {
    fn inspect(&mut self, event: &OrderedPhaseEvent<'_>) -> Result<(), PError> {
        if event.boundary == OrderedPhaseBoundary::Finished && event.phase == self.last_phase {
            self.progress = Some(event.progress.clone());
            return Err(PError::Unsupported(STOP.to_owned()));
        }
        Ok(())
    }
}

/// Owner classes (hex of `owners/<loops>T<t>-<class>.rrbin`) whose payload or
/// ordering entry differs between two packages, and the other package files
/// that differ.
fn changed_owners(base: &Path, candidate: &Path) -> (BTreeSet<u64>, Vec<String>) {
    let class_of = |name: &str| -> Option<u64> {
        let stem = name.strip_suffix(".rrbin")?;
        u64::from_str_radix(stem.rsplit_once('-')?.1, 16).ok()
    };
    let files = |dir: &Path| -> BTreeMap<String, Vec<u8>> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| e.path().is_file())
                    .map(|e| {
                        (
                            e.file_name().to_string_lossy().into_owned(),
                            std::fs::read(e.path()).unwrap(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut changed = BTreeSet::new();
    let (a, b) = (
        files(&base.join("owners")),
        files(&candidate.join("owners")),
    );
    for name in a.keys().chain(b.keys()) {
        if a.get(name) != b.get(name) {
            changed.insert(class_of(name).unwrap_or_else(|| panic!("owner file {name}")));
        }
    }
    // ordering.json: per-owner entries.
    let ordering = |dir: &Path| -> BTreeMap<String, serde_json::Value> {
        std::fs::read_to_string(dir.join("ordering.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| value["owners"].as_array().cloned())
            .map(|owners| {
                owners
                    .into_iter()
                    .map(|o| (o["class"].as_str().unwrap_or_default().to_owned(), o))
                    .collect()
            })
            .unwrap_or_default()
    };
    let (oa, ob) = (ordering(base), ordering(candidate));
    for name in oa.keys().chain(ob.keys()) {
        if oa.get(name) != ob.get(name) {
            let class = name
                .rsplit_once('-')
                .and_then(|(_, hex)| u64::from_str_radix(hex, 16).ok())
                .unwrap_or_else(|| panic!("ordering class {name}"));
            changed.insert(class);
        }
    }
    let mut other = Vec::new();
    for file in ["census.rrbin", "routes.rrbin", "orbits.rrbin"] {
        if std::fs::read(base.join(file)).ok() != std::fs::read(candidate.join(file)).ok() {
            other.push(file.to_owned());
        }
    }
    for dir in ["families", "layer", "topology"] {
        if files(&base.join(dir)) != files(&candidate.join(dir)) {
            other.push(format!("{dir}/"));
        }
    }
    (changed, other)
}

struct Job {
    name: String,
    fixture_cell: String,
    top: String,
}

fn jobs() -> Vec<Job> {
    let all: Vec<Job> = [3, 1, 2]
        .iter()
        .flat_map(|n| {
            [8, 6].iter().flat_map(move |r| {
                [2, 1, 0].iter().map(move |d| Job {
                    name: format!("NO{n}d{d}r{r}"),
                    fixture_cell: format!("L4_d{d}_r{r}"),
                    top: format!("P4L_NO{n}"),
                })
            })
        })
        .collect();
    match std::env::var("DRY_CELLS") {
        Ok(list) => {
            let wanted: Vec<&str> = list.split(',').collect();
            all.into_iter()
                .filter(|job| wanted.contains(&job.name.as_str()))
                .collect()
        }
        Err(_) => all,
    }
}

fn last_phase(library: &PLibrary) -> usize {
    let max_lines = library
        .presentations()
        .iter()
        .map(|p| {
            p.slots
                .iter()
                .filter(|slot| matches!(slot, Slot::Line(_)))
                .count()
        })
        .max()
        .unwrap_or(0);
    2 * max_lines + 1
}

#[allow(clippy::too_many_arguments)]
fn run_cells(
    label: &str,
    library: &PLibrary,
    fixture: &serde_json::Value,
    jobs: &[Job],
    workers: usize,
    cache: Option<&Arc<OrderedStepCache>>,
    arith: &str,
    out: &Mutex<std::fs::File>,
) -> (u64, u64, f64) {
    let next = AtomicUsize::new(0);
    let totals = Mutex::new((0u64, 0u64));
    let t0 = Instant::now();
    let last = last_phase(library);
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let cell = cell(fixture, &job.name, &job.fixture_cell, &job.top);
                    let inputs = inputs(library, &cell);
                    let mut stop = Stop {
                        last_phase: last,
                        progress: None,
                    };
                    let t = Instant::now();
                    let result = library.reduce(PReductionRequest {
                        inputs,
                        arithmetic: arithmetic(arith),
                        scheduler: OrderedSchedulerOptions::with_threads(1),
                        step_cache: cache.cloned(),
                        uncovered: PUncoveredPolicy::Reject,
                        onto_masters: true,
                        limits: ReductionLimits::default(),
                        inspector: Some(&mut stop),
                    });
                    let seconds = t.elapsed().as_secs_f64();
                    let status = match &result {
                        Err(PError::Unsupported(text)) if text == STOP => "ok".to_owned(),
                        Ok(_) => "completed".to_owned(),
                        Err(error) => format!("error: {error}"),
                    };
                    let line = match &stop.progress {
                        Some(p) => {
                            let mut totals = totals.lock().unwrap();
                            totals.0 += p.processed_keys as u64;
                            totals.1 += p.emitted_terms as u64;
                            serde_json::json!({
                                "library": label, "cell": job.name, "keys": p.processed_keys,
                                "edges": p.emitted_terms, "merged": p.merged_terms,
                                "cancelled": p.cancelled_terms, "peak_frontier": p.peak_frontier_keys,
                                "terminal_keys": p.terminal_keys, "seconds": seconds,
                                "hits": p.step_cache_hits, "misses": p.step_cache_misses,
                                "status": status,
                            })
                        }
                        None => serde_json::json!({
                            "library": label, "cell": job.name, "seconds": seconds, "status": status,
                        }),
                    };
                    eprintln!("[dry] {line}");
                    writeln!(out.lock().unwrap(), "{line}").unwrap();
                }
            });
        }
    });
    let totals = *totals.lock().unwrap();
    (totals.0, totals.1, t0.elapsed().as_secs_f64())
}

fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0)
}

/// `p_dry_census overlay BASE OUT CLASS=PAYLOAD [CLASS=PAYLOAD ...]`: a
/// candidate package = BASE with the Custom payloads of the named owners
/// replaced (unchanged files hard-linked). The payload must keep the owner's
/// family, arity, support, two-sided axes, charts and residuals; the owner's
/// ordering entry is recomputed (`compute_ordering`, heuristic `generation`,
/// as Astra's p_overlay_saved_owner 676169ea) unless `ORDER=donor:<package>`
/// copies the donor package's entry for the owner; the identity gets the tag
/// `<base heuristic>.dry-<OUT file name>`.
fn overlay(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use rustred::propagator::custom::inspect_custom_payload;
    use rustred_app::propagator::format::{CustomRecord, LIBRARY_FILE, LibraryManifest};
    use rustred_app::propagator::ordering::{OrderingFile, compute_ordering, ordered_identity};
    let base = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let swaps: Vec<(String, PathBuf)> = args[2..]
        .iter()
        .map(|a| {
            let (class, path) = a.split_once('=').expect("CLASS=PAYLOAD");
            (class.to_owned(), PathBuf::from(path))
        })
        .collect();
    let read_json = |path: &Path| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    };
    let mut manifest: LibraryManifest =
        serde_json::from_slice(&std::fs::read(base.join(LIBRARY_FILE))?)?;
    let ordering_name = manifest
        .files
        .ordering
        .clone()
        .ok_or("BASE has no ordering")?;
    let base_ordering: OrderingFile =
        serde_json::from_slice(&std::fs::read(base.join(&ordering_name))?)?;
    std::fs::create_dir(&output)?;
    // Hard-link every file of BASE (recursively), then replace.
    fn link_tree(from: &Path, to: &Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let (src, dst) = (entry.path(), to.join(entry.file_name()));
            if entry.file_type()?.is_dir() {
                std::fs::create_dir_all(&dst)?;
                link_tree(&src, &dst)?;
            } else if std::fs::hard_link(&src, &dst).is_err() {
                std::fs::copy(&src, &dst)?;
            }
        }
        Ok(())
    }
    link_tree(&base, &output)?;
    for (class, payload_path) in &swaps {
        let index = manifest
            .owners
            .iter()
            .position(|o| &o.class == class)
            .ok_or_else(|| format!("BASE has no owner {class}"))?;
        let record = manifest.owners[index].clone();
        let payload = record.payload.clone().ok_or("owner has no payload")?;
        let meta_old = record.custom.clone().ok_or("owner has no Custom record")?;
        let old = inspect_custom_payload(&std::fs::read(base.join(&payload.file))?)?;
        let bytes = std::fs::read(payload_path)?;
        let new = inspect_custom_payload(&bytes)?;
        let residuals = |s: &rustred::propagator::custom::CustomPayloadSummary| {
            s.charts
                .iter()
                .map(|c| {
                    (
                        c.chart,
                        c.residuals.iter().cloned().collect::<BTreeSet<_>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        if old.family_fingerprint != new.family_fingerprint
            || old.arity != new.arity
            || old.support != new.support
            || old.two_sided_axes != new.two_sided_axes
            || residuals(&old) != residuals(&new)
        {
            return Err(format!("{class}: the payload changes the owner contract").into());
        }
        let meta = new.meta(meta_old.lp_count)?;
        if meta.charts != meta_old.charts || meta.residuals != meta_old.residuals {
            return Err(format!("{class}: the payload changes charts/residuals").into());
        }
        let destination = output.join(&payload.file);
        std::fs::remove_file(&destination)?;
        std::fs::write(&destination, &bytes)?;
        manifest.owners[index].custom = Some(CustomRecord::of(&meta));
        manifest.owners[index].payload.as_mut().unwrap().bytes = bytes.len() as u64;
        eprintln!(
            "[overlay] {class}: {} -> {} rules, ordering {} -> {}",
            old.rules(),
            new.rules(),
            old.ordering,
            new.ordering
        );
    }
    let write = |path: &Path, value: &dyn erased::Json| -> std::io::Result<()> {
        let _ = std::fs::remove_file(path);
        let mut bytes = value.bytes();
        bytes.push(b'\n');
        std::fs::write(path, bytes)
    };
    write(&output.join(LIBRARY_FILE), &manifest)?;
    let generated = compute_ordering(&output, "generation")?;
    let donor = std::env::var("ORDER")
        .ok()
        .and_then(|v| v.strip_prefix("donor:").map(PathBuf::from));
    let donor_ordering = match &donor {
        Some(dir) => Some(read_json(&dir.join("ordering.json"))?),
        None => None,
    };
    let tag = output
        .file_name()
        .ok_or("OUT needs a name")?
        .to_string_lossy()
        .into_owned();
    let mut ordering = base_ordering.clone();
    for (class, _) in &swaps {
        let replacement = match &donor_ordering {
            Some(donor) => serde_json::from_value(
                donor["owners"]
                    .as_array()
                    .and_then(|owners| owners.iter().find(|o| o["class"] == class.as_str()))
                    .ok_or("donor ordering has no entry")?
                    .clone(),
            )?,
            None => generated
                .owners
                .iter()
                .find(|o| &o.class == class)
                .ok_or("ordering omitted the owner")?
                .clone(),
        };
        *ordering
            .owners
            .iter_mut()
            .find(|o| &o.class == class)
            .ok_or("BASE ordering omitted the owner")? = replacement;
    }
    let suffix = format!(
        ",P_ORDERING_VERSION={}:{}",
        rustred::propagator::P_ORDERING_VERSION,
        base_ordering.heuristic
    );
    let base_identity = manifest
        .identity
        .strip_suffix(&suffix)
        .ok_or("BASE identity does not end with its ordering tag")?
        .to_owned();
    ordering.heuristic = format!("{}.dry-{tag}", base_ordering.heuristic);
    manifest.identity = ordered_identity(&base_identity, &ordering.heuristic);
    write(&output.join(&ordering_name), &ordering)?;
    write(&output.join(LIBRARY_FILE), &manifest)?;
    let loaded = load_p_library(
        PLibrarySource::Directory(output.clone()),
        &PLibraryOptions {
            verify_descent: false,
        },
    )?;
    eprintln!(
        "[overlay] {} loads: {}",
        output.display(),
        loaded.identity()
    );
    Ok(())
}

mod erased {
    pub trait Json {
        fn bytes(&self) -> Vec<u8>;
    }
    impl<T: serde::Serialize> Json for T {
        fn bytes(&self) -> Vec<u8> {
            serde_json::to_vec_pretty(self).unwrap()
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("overlay") {
        if let Err(error) = overlay(&args[2..]) {
            eprintln!("overlay: {error}");
            std::process::exit(2);
        }
        return;
    }
    assert!(
        args.len() >= 5,
        "usage: p_dry_census <fixture json> <workers> <out jsonl> <base lib> [<candidate lib> ...]"
    );
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let workers: usize = args[2].parse().unwrap();
    let out = Mutex::new(std::fs::File::create(&args[3]).unwrap());
    let base_dir = PathBuf::from(&args[4]);
    let candidates: Vec<PathBuf> = args[5..].iter().map(PathBuf::from).collect();
    let use_cache = std::env::var("DRY_CACHE").as_deref() != Ok("off");
    let arith = std::env::var("DRY_ARITH").unwrap_or_else(|_| "modular_polynomial".to_owned());
    let allow_other = std::env::var("DRY_ALLOW_OTHER").as_deref() == Ok("1");
    let jobs = jobs();
    let load = |dir: &Path| {
        let library = load_p_library(
            PLibrarySource::Directory(dir.to_path_buf()),
            &PLibraryOptions {
                verify_descent: false,
            },
        )
        .unwrap_or_else(|e| panic!("load {}: {e}", dir.display()));
        match std::env::var("P_ORBIT_CANON").as_deref() {
            Ok("off") => library.set_orbit_canonicalization(false),
            Ok("on") => library.set_orbit_canonicalization(true),
            _ => {}
        }
        library
    };
    let summary = |label: &str, (keys, edges, wall): (u64, u64, f64), extra: serde_json::Value| {
        let line = serde_json::json!({
            "library": label, "summary": true, "keys": keys, "edges": edges, "wall": wall,
            "vmhwm_kb": rss_kb(), "extra": extra,
        });
        eprintln!("[dry] {line}");
        writeln!(out.lock().unwrap(), "{line}").unwrap();
    };
    let t = Instant::now();
    let base = load(&base_dir);
    let load_seconds = t.elapsed().as_secs_f64();
    let base_cache = use_cache.then(|| {
        OrderedStepCache::in_memory(
            base.step_cache_binding().unwrap(),
            OrderedStepCacheOptions::default(),
        )
    });
    let label = base_dir.display().to_string();
    let totals = run_cells(
        &label,
        &base,
        &fixture,
        &jobs,
        workers,
        base_cache.as_ref(),
        &arith,
        &out,
    );
    summary(
        &label,
        totals,
        serde_json::json!({"role": "base", "load_seconds": load_seconds,
            "shards": base_cache.as_ref().map(|c| c.resident_shards().len())}),
    );
    if std::env::var("DRY_REPEAT_BASE").as_deref() == Ok("1") {
        let totals = run_cells(
            &format!("{label}#warm"),
            &base,
            &fixture,
            &jobs,
            workers,
            base_cache.as_ref(),
            &arith,
            &out,
        );
        summary(&label, totals, serde_json::json!({"role": "base-warm"}));
    }
    for dir in &candidates {
        let t = Instant::now();
        let (changed, other) = changed_owners(&base_dir, dir);
        let library = load(dir);
        let load_seconds = t.elapsed().as_secs_f64();
        let label = dir.display().to_string();
        if !other.is_empty() && !allow_other && base_cache.is_some() {
            let line = serde_json::json!({"library": label, "summary": true,
                "status": format!("refused: other package files differ: {other:?}")});
            eprintln!("[dry] {line}");
            writeln!(out.lock().unwrap(), "{line}").unwrap();
            continue;
        }
        // Re-canonicalise replayed Apply children of the swapped owners
        // (their order may differ; DRY_REFRESH=off keeps the base's
        // representatives).
        if std::env::var("DRY_REFRESH").as_deref() != Ok("off") {
            rustred::propagator::engine::set_step_cache_refresh_classes(changed.clone());
        }
        let cache = base_cache.as_ref().map(|cache| {
            cache.fork_in_memory(
                library.step_cache_binding().unwrap(),
                |shard| !(shard.stage == OrderedStepStage::Apply && changed.contains(&shard.class)),
                OrderedStepCacheOptions::default(),
            )
        });
        let totals = run_cells(
            &label,
            &library,
            &fixture,
            &jobs,
            workers,
            cache.as_ref(),
            &arith,
            &out,
        );
        summary(
            &label,
            totals,
            serde_json::json!({"role": "candidate", "load_seconds": load_seconds,
                "changed": changed.iter().map(|c| format!("{c:016x}")).collect::<Vec<_>>(),
                "other": other}),
        );
        rustred::propagator::engine::set_step_cache_refresh_classes(BTreeSet::new());
    }
}
