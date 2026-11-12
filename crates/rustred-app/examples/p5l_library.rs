//! p5l-library: the first scope 1-5 p library from imported 5L tables, and its
//! acceptance checks.
//!
//! ```text
//! p5l_library coverage <families.json> <max_loops>
//!     TSV per top of <max_loops> loops: the DAG nodes with a Custom action in its descent.
//! p5l_library assemble <families.json> <max_loops> <provided package dir> <imports.tsv> <out dir> <threads>
//!     assemble with CustomPayloads::Imported (provided payloads by class label from the package,
//!     imported nodes by graph key from p-import-table records), write the package (no basis layer).
//!     imports.tsv: <TID> \t <record .json> \t <payload .rrbin>.  P5L_VERIFY=0 skips the descent certificate.
//! p5l_library load <package dir> <verify 0|1>
//!     load time and peak RSS.
//! p5l_library check <package dir> <top name> <dots> <rank> <ibp seeds> <laporta dots> <laporta rank>
//!     the cell numerator of the benchmark grid (rank r = dot(k1,p) dot(k2,p) dot(k1,k3) ...) times the top at
//!     powers 1 (first line 1 + dots): (A) the library reduction; (B) its top-sector part against a RustRed
//!     Laporta of the top sector modulo subsectors (RustRed's ordinary IBP rows, Z_p, p = 2^61-1, at three
//!     numeric d), mapped onto the library terminals by reducing the Laporta right-hand side integrals through
//!     the library; (C) random combinations of the top family's IBP rows at <ibp seeds> seeds reduce to zero
//!     exactly (modular_polynomial = exact in Q(d) by reconstruction).
//! ```
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits, IndexedAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::identity::{ParametricIbpGenerator, ParametricRelation};
use rustred::propagator::PresentationId;
use rustred::propagator::census::PSectorFate;
use rustred::propagator::families::load_p_families;
use rustred::propagator::library::{
    CustomOwnerMeta, PBasis, PLibrary, PReduction, PReductionRequest, PTerminal, PUncoveredPolicy,
};
use rustred::propagator::present::top_topology;
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot, SlotIsp, SlotLine};
use rustred::propagator::topology::Action;
use rustred::propagator::topology::{ChildMode, Normalised, TopologyService, canonical, normalise};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionLimits,
};
use rustred_app::propagator::assemble::SplitMix;
use rustred_app::propagator::loader::read_custom_payloads;
use rustred_app::propagator::{
    AssembleOptions, CustomOwnerPayload, CustomPayloads, ImportedCustomNode, PLibraryOptions,
    PLibrarySource, assemble_library, load_p_library, write_p_library,
};
use serde_json::{Value, json};

type Input = (PresentationId, IntegralKey, Coefficient);

const P: u64 = (1 << 61) - 1;

fn rss() -> (f64, f64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1048576.0
    };
    (field("VmRSS:"), field("VmHWM:"))
}

fn stamp() -> String {
    let (now, peak) = rss();
    format!("[rss {now:.2} GB, peak {peak:.2} GB]")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("coverage") => coverage(&args[2], args[3].parse().unwrap()),
        Some("assemble") => assemble(&args[2..]),
        Some("load") => {
            load(&args[2], args[3] == "1");
        }
        Some("check") => check(&args[2..]),
        Some("edges") => edges(
            &args[2],
            args[3].parse().unwrap(),
            args.get(4).map(String::as_str),
        ),
        _ => panic!("usage: see the module documentation"),
    }
}

// ---------------------------------------------------------------------------
// coverage
// ---------------------------------------------------------------------------

fn coverage(families: &str, loops: u8) {
    let text = std::fs::read_to_string(families).unwrap();
    let (file, _) = load_p_families(&text).unwrap();
    let entries: Vec<_> = (file.families.iter())
        .filter(|entry| entry.loops <= loops)
        .cloned()
        .collect();
    let roots: Vec<_> = entries.iter().map(top_topology).collect();
    let started = Instant::now();
    let dag = TopologyService::new().reduction_graph(&roots, ChildMode::ForcerTree);
    eprintln!(
        "dag: {} nodes, {} edges, {:.1} s",
        dag.nodes.len(),
        dag.edges.len(),
        started.elapsed().as_secs_f64()
    );
    let index: HashMap<Vec<u8>, u32> = (dag.nodes.iter())
        .map(|node| (node.key.as_bytes().to_vec(), node.index))
        .collect();
    let mut out = std::io::stdout().lock();
    writeln!(out, "top\tloops\ttop_node\ttop_action\tnodes\tcustom_nodes").unwrap();
    for (entry, root) in entries.iter().zip(&roots) {
        if entry.loops != loops {
            continue;
        }
        let Normalised::Topo(topo, _) = normalise(root) else {
            writeln!(out, "{}\t{}\t-\tzero\t0\t", entry.name, entry.loops).unwrap();
            continue;
        };
        let (key, _) = canonical(&topo);
        let start = index[key.as_bytes()];
        let mut seen = BTreeSet::from([start]);
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            for &child in &dag.nodes[node as usize].children {
                if seen.insert(child) {
                    stack.push(child);
                }
            }
        }
        let custom: Vec<String> = (seen.iter())
            .filter(|&&node| matches!(dag.nodes[node as usize].action, Action::Custom))
            .map(|node| node.to_string())
            .collect();
        writeln!(
            out,
            "{}\t{}\t{}\t{:?}\t{}\t{}",
            entry.name,
            entry.loops,
            start,
            dag.nodes[start as usize].action.kind(),
            seen.len(),
            custom.join(",")
        )
        .unwrap();
    }
}

// ---------------------------------------------------------------------------
// assemble
// ---------------------------------------------------------------------------

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn ints(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect()
}

/// The import presentation of a `p-import-table` record (`presentation_slots`).
fn record_presentation(record: &Value, context: &CoefficientContext) -> Presentation {
    let slots: Vec<Slot> = (record["presentation_slots"].as_array().unwrap().iter())
        .map(|slot| {
            if let Some(line) = slot.get("line") {
                Slot::Line(SlotLine {
                    momentum: ints(line),
                    eps_units: slot["eps_units"].as_u64().unwrap() as u8,
                })
            } else {
                let isp = slot["isp"].as_array().unwrap();
                Slot::Isp(SlotIsp {
                    left: ints(&isp[0]),
                    right: ints(&isp[1]),
                })
            }
        })
        .collect();
    assert_eq!(record["presentation_kind"], "caller");
    assert_eq!(record["external_square"], -1);
    Presentation::from_slots(
        PresentationId(0),
        PresentationKind::Caller,
        record["table_family"].as_str().unwrap(),
        record["loops"].as_u64().unwrap() as u8,
        slots,
        context,
        None,
    )
    .unwrap()
}

fn record_meta(record: &Value) -> CustomOwnerMeta {
    let mut charts: Vec<u64> = (record["charts"].as_array().unwrap().iter())
        .map(|chart| chart["chart"].as_u64().unwrap())
        .collect();
    charts.sort_unstable();
    CustomOwnerMeta {
        lp_count: record["lp_count"].as_u64().unwrap() as u32,
        residuals: record["residuals"].as_u64().unwrap() as u32,
        two_sided_axes: record["two_sided_axes"].as_u64().unwrap(),
        charts,
    }
}

fn assemble(args: &[String]) {
    let started = Instant::now();
    let families_text = std::fs::read_to_string(&args[0]).unwrap();
    let max_loops: u8 = args[1].parse().unwrap();
    let provided_dir = PathBuf::from(&args[2]);
    let imports = std::fs::read_to_string(&args[3]).unwrap();
    let out = PathBuf::from(&args[4]);
    let threads: usize = args[5].parse().unwrap();
    let verify = std::env::var("P5L_VERIFY").map_or(true, |v| v != "0");
    let (families, _) = load_p_families(&families_text).unwrap();
    let provided = read_custom_payloads(&PLibrarySource::Directory(provided_dir)).unwrap();
    eprintln!(
        "provided: {} payloads, {:.1} s {}",
        provided.len(),
        started.elapsed().as_secs_f64(),
        stamp()
    );
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let mut imported = BTreeMap::new();
    let mut bytes_total = 0usize;
    let mut labels = BTreeMap::new();
    for line in imports
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let record: Value =
            serde_json::from_str(&std::fs::read_to_string(fields[1]).unwrap()).unwrap();
        let bytes: Arc<[u8]> = std::fs::read(fields[2]).unwrap().into();
        bytes_total += bytes.len();
        let key = unhex(record["graph_key"].as_str().unwrap());
        labels.insert(key.clone(), fields[0].to_owned());
        imported.insert(
            key,
            ImportedCustomNode {
                presentation: record_presentation(&record, &context),
                payload: CustomOwnerPayload {
                    bytes,
                    meta: record_meta(&record),
                },
            },
        );
    }
    eprintln!(
        "imported: {} nodes, {:.3} GB payload, {:.1} s {}",
        imported.len(),
        bytes_total as f64 / 1e9,
        started.elapsed().as_secs_f64(),
        stamp()
    );
    let t_assemble = Instant::now();
    let assembled = assemble_library(
        &families,
        max_loops,
        CustomPayloads::Imported { provided, imported },
        &AssembleOptions {
            threads,
            verify_descent: verify,
        },
    );
    let assembled = match assembled {
        Ok(assembled) => assembled,
        Err(error) => {
            eprintln!(
                "ASSEMBLY FAILED after {:.1} s: {error}",
                t_assemble.elapsed().as_secs_f64()
            );
            std::process::exit(5);
        }
    };
    let assemble_seconds = t_assemble.elapsed().as_secs_f64();
    let library = &assembled.library;
    let mut lanes: BTreeMap<String, usize> = BTreeMap::new();
    for owner in library.owners() {
        *lanes.entry(format!("{:?}", owner.spec.lane)).or_default() += 1;
    }
    let callers = (library.presentations().iter())
        .filter(|p| matches!(p.kind, PresentationKind::Caller))
        .count();
    eprintln!(
        "assembled in {assemble_seconds:.1} s {}: {} presentations ({callers} imported), {} owners {lanes:?}, \
         {} terminals, census {} sectors / {} classes, timings {:?}, descent {:?}",
        stamp(),
        library.presentations().len(),
        library.owners().len(),
        library.terminals().len(),
        library.census().sectors.len(),
        library.census().classes.len(),
        assembled.timings,
        assembled
            .descent
            .as_ref()
            .map(|d| format!("{d:?}").chars().take(400).collect::<String>()),
    );
    // Which imported nodes became owners (a class owner is its min-GraphKey node).
    let mut used = Vec::new();
    for owner in library.owners() {
        let p = library.presentation(owner.spec.presentation);
        if let (PresentationKind::Caller, Some(node)) = (&p.kind, &p.node) {
            used.push(labels[&node.graph_key.to_vec()].clone());
        }
    }
    used.sort();
    let t_write = Instant::now();
    write_p_library(&assembled, &out).unwrap();
    let write_seconds = t_write.elapsed().as_secs_f64();
    let summary = json!({
        "max_loops": max_loops, "tops": assembled.families.families.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
        "presentations": library.presentations().len(), "imported_presentations": callers,
        "owners": library.owners().len(), "lanes": lanes, "terminals": library.terminals().len(),
        "census_sectors": library.census().sectors.len(), "classes": library.census().classes.len(),
        "dag_nodes": assembled.dag.nodes.len(), "imported_owners": used,
        "assemble_seconds": assemble_seconds, "write_seconds": write_seconds,
        "timings": assembled.timings.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        "verify_descent": verify, "peak_rss_gb": rss().1,
        "process_seconds": started.elapsed().as_secs_f64(),
    });
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}

// ---------------------------------------------------------------------------
// load
// ---------------------------------------------------------------------------

fn load(dir: &str, verify: bool) -> (Arc<PLibrary>, f64) {
    let started = Instant::now();
    let library = load_p_library(
        PLibrarySource::Directory(PathBuf::from(dir)),
        &PLibraryOptions {
            verify_descent: verify,
        },
    )
    .unwrap();
    let seconds = started.elapsed().as_secs_f64();
    eprintln!(
        "load {dir} verify {verify}: {seconds:.2} s {} ({} presentations, {} owners, {} terminals)",
        stamp(),
        library.presentations().len(),
        library.owners().len(),
        library.terminals().len()
    );
    println!(
        "{}",
        json!({"load_seconds": seconds, "verify": verify, "load_peak_rss_gb": rss().1,
               "presentations": library.presentations().len(), "owners": library.owners().len()})
    );
    (library, seconds)
}

// ---------------------------------------------------------------------------
// check
// ---------------------------------------------------------------------------

fn exact() -> ExactAlgebraLimits {
    ExactAlgebraLimits::default()
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

fn reduce(library: &PLibrary, inputs: Vec<Input>) -> PReduction {
    library
        .reduce(PReductionRequest {
            inputs,
            arithmetic: modular_polynomial(1),
            scheduler: OrderedSchedulerOptions::default(),
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: false,
            limits: ReductionLimits::default(),
            inspector: None,
        })
        .unwrap()
}

fn mul(a: u64, b: u64) -> u64 {
    ((a as u128 * b as u128) % P as u128) as u64
}

fn add(a: u64, b: u64) -> u64 {
    (a + b) % P
}

fn sub(a: u64, b: u64) -> u64 {
    (a + P - b) % P
}

fn pow(mut a: u64, mut e: u64) -> u64 {
    let mut r = 1;
    while e > 0 {
        if e & 1 == 1 {
            r = mul(r, a);
        }
        a = mul(a, a);
        e >>= 1;
    }
    r
}

fn inv(a: u64) -> u64 {
    assert!(a != 0, "division by zero mod p");
    pow(a, P - 2)
}

fn modp_i64(v: i64) -> u64 {
    if v >= 0 {
        v as u64 % P
    } else {
        sub(0, (-v) as u64 % P)
    }
}

fn modp_decimal(text: &str) -> u64 {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let mut r = 0u64;
    for c in digits.bytes() {
        r = add(mul(r, 10), u64::from(c - b'0'));
    }
    if negative { sub(0, r) } else { r }
}

/// `c(d)` modulo `p` (one variable `d`).
fn eval(c: &Coefficient, d: u64) -> u64 {
    let poly = |monomials: Vec<(Vec<u16>, String)>| {
        let mut sum = 0;
        for (exponents, coefficient) in monomials {
            let e = exponents.first().copied().unwrap_or(0);
            sum = add(sum, mul(modp_decimal(&coefficient), pow(d, u64::from(e))));
        }
        sum
    };
    let numerator: Vec<_> = (&c.numerator)
        .into_iter()
        .map(|m| (m.exponents.to_vec(), m.coefficient.to_string()))
        .collect();
    let denominator: Vec<_> = (&c.denominator)
        .into_iter()
        .map(|m| (m.exponents.to_vec(), m.coefficient.to_string()))
        .collect();
    mul(poly(numerator), inv(poly(denominator)))
}

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn line_slots(p: &Presentation) -> Vec<usize> {
    (0..p.slots.len())
        .filter(|&s| matches!(p.slots[s], Slot::Line(_)))
        .collect()
}

/// `numerator * top(powers)` lowered onto keys of the top presentation (exact rationals as `Coefficient`s).
fn cell_inputs(
    library: &PLibrary,
    top: PresentationId,
    powers: &[i64],
    dots: &[(usize, usize)],
) -> Vec<Input> {
    let p = library.presentation(top);
    let context = library.coefficient_context();
    let loops = usize::from(p.loops);
    let arity = p.slots.len();
    // polynomial over slot exponents with Coefficient values
    let mut product: BTreeMap<Vec<u32>, Coefficient> =
        BTreeMap::from([(vec![0u32; arity], context.one())]);
    for &(a, b) in dots {
        let unit = |i: usize| {
            let mut row = vec![0i64; loops + 1];
            row[i] = 1;
            row
        };
        let form = p.express_scalar_product(&unit(a), &unit(b)).unwrap();
        let rational = |r: &symbolica::domains::rational::Rational| {
            let n = context.integer(i64::try_from(r.numerator_ref().clone()).unwrap());
            let d = context.integer(i64::try_from(r.denominator_ref().clone()).unwrap());
            context.try_div(&n, &d, exact()).unwrap()
        };
        let mut next: BTreeMap<Vec<u32>, Coefficient> = BTreeMap::new();
        let mut put = |monomial: Vec<u32>, value: Coefficient| {
            let entry = next.entry(monomial).or_insert_with(|| context.integer(0));
            *entry = context.try_add(entry, &value, exact()).unwrap();
        };
        for (monomial, coefficient) in &product {
            if !form.constant.is_zero() {
                put(
                    monomial.clone(),
                    context
                        .try_mul(coefficient, &rational(&form.constant), exact())
                        .unwrap(),
                );
            }
            for (slot, value) in &form.terms {
                let mut shifted = monomial.clone();
                shifted[usize::from(*slot)] += 1;
                put(
                    shifted,
                    context
                        .try_mul(coefficient, &rational(value), exact())
                        .unwrap(),
                );
            }
        }
        product = next.into_iter().filter(|(_, c)| !c.is_zero()).collect();
    }
    product
        .into_iter()
        .map(|(monomial, c)| {
            let k: Vec<i64> = (0..arity)
                .map(|slot| {
                    powers.get(slot).copied().unwrap_or(0).max(0) - i64::from(monomial[slot])
                })
                .collect();
            (top, key(&k), c)
        })
        .collect()
}

/// RustRed's ordinary IBP rows of `p`'s family (indexed).
fn ibp_rows(
    p: &Presentation,
) -> (
    rustred::algebra::IndexedCoefficientContext,
    Vec<ParametricRelation>,
) {
    let generator = ParametricIbpGenerator::try_new(&p.family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len()).map(|row| batch.generate(row)).collect();
    let relations = batch.complete(rows).unwrap().into_relations();
    (generator.context().clone(), relations)
}

/// The rows at `seed`: per relation, (powers, coefficient in Q(d)).
fn rows_at(
    rows: &(
        rustred::algebra::IndexedCoefficientContext,
        Vec<ParametricRelation>,
    ),
    seed: &[i64],
) -> Vec<Vec<(Vec<i64>, Coefficient)>> {
    let (indexed, relations) = rows;
    relations
        .iter()
        .map(|relation| {
            let mut terms = Vec::new();
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
                terms.push((powers, value));
            }
            terms
        })
        .collect()
}

/// Laporta weight: larger = more complex = eliminated first.
fn weight(powers: &[i64], lines: &[usize]) -> (i64, i64, i64, Vec<i64>) {
    let dots: i64 = lines.iter().map(|&s| (powers[s] - 1).max(0)).sum();
    let rank: i64 = powers.iter().map(|&n| (-n).max(0)).sum();
    (dots + rank, rank, dots, powers.to_vec())
}

/// A top-sector Laporta modulo subsectors over Z_p at one numeric d.
struct Laporta {
    ids: HashMap<Vec<i64>, usize>,
    keys: Vec<Vec<i64>>,
    order: Vec<(i64, i64, i64, Vec<i64>)>,
    pivots: HashMap<usize, Vec<(usize, u64)>>,
    lines: Vec<usize>,
}

impl Laporta {
    fn id(&mut self, powers: &[i64]) -> usize {
        if let Some(&id) = self.ids.get(powers) {
            return id;
        }
        let id = self.keys.len();
        self.ids.insert(powers.to_vec(), id);
        self.keys.push(powers.to_vec());
        self.order.push(weight(powers, &self.lines));
        id
    }

    /// Sort a sparse row, most complex first, merging duplicates.
    fn normal(&self, row: BTreeMap<usize, u64>) -> Vec<(usize, u64)> {
        let mut v: Vec<(usize, u64)> = row.into_iter().filter(|(_, c)| *c != 0).collect();
        v.sort_by(|a, b| self.order[b.0].cmp(&self.order[a.0]));
        v
    }

    /// Reduce `row` until its most complex term has no pivot; returns it (possibly empty).
    fn reduce_head(&self, row: Vec<(usize, u64)>) -> Vec<(usize, u64)> {
        let mut row = row;
        loop {
            let Some(&(head, c)) = row.first() else {
                return row;
            };
            let Some(pivot) = self.pivots.get(&head) else {
                return row;
            };
            let mut map: BTreeMap<usize, u64> = row.iter().copied().collect();
            for &(id, value) in pivot {
                let entry = map.entry(id).or_insert(0);
                *entry = sub(*entry, mul(c, value));
            }
            row = self.normal(map);
        }
    }

    fn insert(&mut self, row: Vec<(usize, u64)>) {
        let row = self.reduce_head(row);
        if let Some(&(head, c)) = row.first() {
            let scale = inv(c);
            let normalised = row.iter().map(|&(id, v)| (id, mul(v, scale))).collect();
            self.pivots.insert(head, normalised);
        }
    }

    /// Fully reduce one integral: `target = Sum c_K K` over integrals without a pivot.
    fn express(&self, target: usize) -> BTreeMap<usize, u64> {
        let mut map: BTreeMap<usize, u64> = BTreeMap::from([(target, 1)]);
        loop {
            let next = map
                .iter()
                .filter(|(id, c)| **c != 0 && self.pivots.contains_key(id))
                .max_by(|a, b| self.order[*a.0].cmp(&self.order[*b.0]))
                .map(|(&id, &c)| (id, c));
            let Some((id, c)) = next else { break };
            map.remove(&id);
            for &(other, value) in &self.pivots[&id] {
                if other == id {
                    continue;
                }
                let entry = map.entry(other).or_insert(0);
                *entry = sub(*entry, mul(c, value));
            }
        }
        map.retain(|_, c| *c != 0);
        map
    }
}

/// Every key of the sector `sector` (its lines at power >= 1) of `p` with dots <= `dots` on the sector lines and
/// numerator rank <= `rank` on the other slots.
fn top_box(p: &Presentation, sector: &[usize], dots: i64, rank: i64) -> Vec<Vec<i64>> {
    let arity = p.slots.len();
    let on: Vec<bool> = (0..arity).map(|s| sector.contains(&s)).collect();
    let mut out = Vec::new();
    let mut powers = vec![0i64; arity];
    fn walk(
        slot: usize,
        dots: i64,
        rank: i64,
        on: &[bool],
        powers: &mut Vec<i64>,
        out: &mut Vec<Vec<i64>>,
    ) {
        if slot == powers.len() {
            out.push(powers.clone());
            return;
        }
        if on[slot] {
            for e in 0..=dots {
                powers[slot] = 1 + e;
                walk(slot + 1, dots - e, rank, on, powers, out);
            }
        } else {
            for r in 0..=rank {
                powers[slot] = -r;
                walk(slot + 1, dots, rank - r, on, powers, out);
            }
        }
        powers[slot] = 0;
    }
    walk(0, dots, rank, &on, &mut powers, &mut out);
    out
}

fn in_top(powers: &[i64], lines: &[usize]) -> bool {
    lines.iter().all(|&s| powers[s] >= 1)
}

/// The top-sector part of a reduction at numeric d: terminal id -> value.
fn top_part(
    library: &PLibrary,
    reduction: &PReduction,
    class: rustred::propagator::OwnerClassId,
    d: u64,
) -> BTreeMap<u32, u64> {
    let mut out = BTreeMap::new();
    for term in &reduction.terms {
        if let PBasis::Residual(id) = &term.basis
            && let PTerminal::Residual { owner, .. } = &library.terminals()[id.0 as usize]
            && *owner == class
        {
            let entry = out.entry(id.0).or_insert(0);
            *entry = add(*entry, eval(&term.coefficient, d));
        }
    }
    out.retain(|_, v| *v != 0);
    out
}

fn check(args: &[String]) {
    let (library, load_seconds) = load(&args[0], false);
    let top_name = &args[1];
    let dots: i64 = args[2].parse().unwrap();
    let rank: usize = args[3].parse().unwrap();
    let ibp_seeds: usize = args[4].parse().unwrap();
    let lap_dots: i64 = args[5].parse().unwrap();
    let lap_rank: i64 = args[6].parse().unwrap();
    let top = library
        .top_presentation(top_name)
        .unwrap_or_else(|| panic!("top {top_name}"));
    let p = Arc::clone(&library.presentations()[top.0 as usize]);
    let all_lines = line_slots(&p);
    let forced = p.forced_axes();
    // The sector of (B) and (C): the top sector, or (P5L_SECTOR=custom) the largest sector of the top whose class
    // is owned in an imported (Caller) presentation, so the check crosses a route into an imported owner.
    let lines: Vec<usize> = match std::env::var("P5L_SECTOR").as_deref() {
        Ok("custom") => {
            let mut best: Option<(u32, u64)> = None;
            for (&(presentation, support), fate) in
                library.census().sectors.range((top, 0)..=(top, u64::MAX))
            {
                debug_assert_eq!(presentation, top);
                if let PSectorFate::Owner { class, .. } = fate {
                    let owner = library.owner(*class).unwrap();
                    let kind = &library.presentation(owner.spec.presentation).kind;
                    let size = (support & !forced).count_ones();
                    if matches!(kind, PresentationKind::Caller)
                        && best.is_none_or(|(b, _)| size > b)
                    {
                        best = Some((size, support));
                    }
                }
            }
            let (_, support) =
                best.expect("no sector of the top is owned by an imported presentation");
            all_lines
                .iter()
                .copied()
                .filter(|&s| support >> s & 1 == 1)
                .collect()
        }
        _ => all_lines.clone(),
    };
    let support = lines.iter().fold(0u64, |m, &s| m | 1 << s) | forced;
    let class = match library.census().sectors.get(&(top, support)) {
        Some(PSectorFate::Owner { class, .. }) => *class,
        other => panic!("sector {support:#x} fate {other:?}"),
    };
    let owner = library.owner(class).unwrap();
    let owner_kind = format!("{:?}", library.presentation(owner.spec.presentation).kind);
    eprintln!(
        "top {top_name}: presentation {}, {} lines; check sector {support:#x} ({} lines), owner lane {:?}, presentation {} {}",
        top.0,
        all_lines.len(),
        lines.len(),
        owner.spec.lane,
        owner.spec.presentation.0,
        owner_kind
    );
    let context = library.coefficient_context().clone();

    // (A) the cell.
    let mut powers = vec![0i64; p.slots.len()];
    for &s in &all_lines {
        powers[s] = 1;
    }
    powers[all_lines[0]] += dots;
    let pool: Vec<(usize, usize)> = {
        let l = usize::from(p.loops);
        vec![(0, l), (1, l), (0, 2), (1, 3), (2, l), (3, 4)]
    };
    let numerator: Vec<(usize, usize)> = match rank {
        0 => vec![],
        2 => pool[..2].to_vec(),
        4 => pool[..3].to_vec(),
        6 => pool[..4].to_vec(),
        8 => pool[..6].to_vec(),
        _ => panic!("rank {rank}"),
    };
    let inputs = cell_inputs(&library, top, &powers, &numerator);
    let t = Instant::now();
    let value = reduce(&library, inputs.clone());
    let reduce_seconds = t.elapsed().as_secs_f64();
    eprintln!(
        "(A) {} inputs reduced in {reduce_seconds:.2} s: {} terms {}",
        inputs.len(),
        value.terms.len(),
        stamp()
    );

    // (B) top sector vs Laporta modulo subsectors at three d.
    let ds = [(37, 7), (-11, 5), (9, 13)];
    let rows = ibp_rows(&p);
    // the same numerator on the check sector: lines outside it at power 0
    let mut sector_powers = vec![0i64; p.slots.len()];
    for &s in &lines {
        sector_powers[s] = 1;
    }
    sector_powers[lines[0]] += dots;
    let sector_inputs = if lines.len() == all_lines.len() {
        inputs.clone()
    } else {
        cell_inputs(&library, top, &sector_powers, &numerator)
    };
    let sector_value = if lines.len() == all_lines.len() {
        value.clone()
    } else {
        reduce(&library, sector_inputs.clone())
    };
    let targets: Vec<(Vec<i64>, Coefficient)> = (sector_inputs.iter())
        .filter(|(_, k, _)| in_top(&k.powers().to_vec(), &lines))
        .map(|(_, k, c)| (k.powers().to_vec(), c.clone()))
        .collect();
    let max_target_rank = targets
        .iter()
        .map(|(k, _)| k.iter().map(|&n| (-n).max(0)).sum::<i64>())
        .max()
        .unwrap_or(0);
    let lap_rank = lap_rank.max(max_target_rank);
    let lap_dots = lap_dots.max(dots);
    let seeds = top_box(&p, &lines, lap_dots, lap_rank);
    let t_rows = Instant::now();
    // rows at every seed in Q(d), once
    let seed_rows: Vec<Vec<Vec<(Vec<i64>, Coefficient)>>> =
        seeds.iter().map(|seed| rows_at(&rows, seed)).collect();
    eprintln!(
        "(B) {} top-sector targets, Laporta box dots <= {lap_dots}, ISP rank <= {lap_rank}: {} seeds, {} rows, built in {:.1} s {}",
        targets.len(),
        seeds.len(),
        seed_rows.iter().map(Vec::len).sum::<usize>(),
        t_rows.elapsed().as_secs_f64(),
        stamp()
    );
    let mut b_ok = true;
    let mut b_report = Vec::new();
    let mut master_cache: HashMap<Vec<i64>, PReduction> = HashMap::new();
    for &(dn, dd) in &ds {
        let d = mul(modp_i64(dn), inv(modp_i64(dd)));
        let t_lap = Instant::now();
        let mut lap = Laporta {
            ids: HashMap::new(),
            keys: Vec::new(),
            order: Vec::new(),
            pivots: HashMap::new(),
            lines: lines.clone(),
        };
        // seeds ordered simplest first (Laporta)
        let mut order: Vec<usize> = (0..seeds.len()).collect();
        order.sort_by(|&a, &b| weight(&seeds[a], &lines).cmp(&weight(&seeds[b], &lines)));
        let mut equations = 0usize;
        for &index in &order {
            for row in &seed_rows[index] {
                let mut map = BTreeMap::new();
                for (powers, value) in row {
                    if !in_top(powers, &lines) {
                        continue;
                    }
                    let id = lap.id(powers);
                    let entry = map.entry(id).or_insert(0);
                    *entry = add(*entry, eval(value, d));
                }
                let row = lap.normal(map);
                if !row.is_empty() {
                    equations += 1;
                    lap.insert(row);
                }
            }
        }
        let lap_seconds = t_lap.elapsed().as_secs_f64();
        // the library side: top part of the whole cell value
        let lib = top_part(&library, &sector_value, class, d);
        // Laporta side: Sum_i a_i(d) Sum_K c_iK top(Library(K))
        let mut combination: BTreeMap<Vec<i64>, u64> = BTreeMap::new();
        for (target, a) in &targets {
            let id = lap.id(target);
            let a = eval(a, d);
            for (k, c) in lap.express(id) {
                let entry = combination.entry(lap.keys[k].clone()).or_insert(0);
                *entry = add(*entry, mul(a, c));
            }
        }
        combination.retain(|_, v| *v != 0);
        let mut via = BTreeMap::<u32, u64>::new();
        for (k, c) in &combination {
            let reduction = master_cache
                .entry(k.clone())
                .or_insert_with(|| reduce(&library, vec![(top, key(k), context.one())]));
            for (id, v) in top_part(&library, reduction, class, d) {
                let entry = via.entry(id).or_insert(0);
                *entry = add(*entry, mul(*c, v));
            }
        }
        via.retain(|_, v| *v != 0);
        let equal = lib == via;
        b_ok &= equal;
        eprintln!(
            "(B) d = {dn}/{dd}: {equations} equations, {} pivots, {} integrals, {lap_seconds:.1} s; Laporta RHS integrals {} {:?}; library top part {lib:?} vs Laporta {via:?}: {}",
            lap.pivots.len(),
            lap.keys.len(),
            combination.len(),
            combination.keys().take(6).collect::<Vec<_>>(),
            if equal { "EQUAL" } else { "MISMATCH" }
        );
        b_report.push(json!({"d": format!("{dn}/{dd}"), "equations": equations, "pivots": lap.pivots.len(),
            "integrals": lap.keys.len(), "laporta_seconds": lap_seconds, "rhs_integrals": combination.len(),
            "library_top_terms": lib.len(), "equal": equal}));
    }

    // (C) IBP rows reduce to zero at random seeds (top sector and subsectors).
    let mut rng = SplitMix::new(0x5_1b5_11b ^ (rank as u64) << 8 ^ dots as u64);
    let mut c_rows = Vec::new();
    let mut c_ok = true;
    for s in 0..ibp_seeds {
        let mut seed = vec![0i64; p.slots.len()];
        for &l in &lines {
            seed[l] = if s % 2 == 1 && rng.below(5) == 0 {
                0
            } else {
                1
            };
        }
        let on: Vec<usize> = lines.iter().copied().filter(|&l| seed[l] == 1).collect();
        if !on.is_empty() {
            for _ in 0..rng.below(dots.max(1) as u64 + 1) {
                seed[on[rng.below(on.len() as u64) as usize]] += 1;
            }
        }
        let free: Vec<usize> = (0..seed.len()).filter(|&i| seed[i] == 0).collect();
        for _ in 0..rng.below(rank as u64 + 1) {
            seed[free[rng.below(free.len() as u64) as usize]] -= 1;
        }
        let t = Instant::now();
        let mut sum: BTreeMap<Vec<i64>, Coefficient> = BTreeMap::new();
        let at = rows_at(&rows, &seed);
        for row in &at {
            let w = context.integer(1 + rng.below(97) as i64);
            for (powers, value) in row {
                let term = context.try_mul(value, &w, exact()).unwrap();
                let entry = sum
                    .entry(powers.clone())
                    .or_insert_with(|| context.integer(0));
                *entry = context.try_add(entry, &term, exact()).unwrap();
            }
        }
        let combination: Vec<Input> = sum
            .into_iter()
            .filter(|(_, c)| !c.is_zero())
            .map(|(k, c)| (top, key(&k), c))
            .collect();
        let keys = combination.len();
        let reduction = reduce(&library, combination);
        let nonzero = reduction
            .terms
            .iter()
            .filter(|t| !t.coefficient.is_zero())
            .count();
        c_ok &= nonzero == 0;
        eprintln!(
            "(C) seed {s} {seed:?}: {} rows, {keys} keys, {nonzero} nonzero terms, {:.2} s {}",
            at.len(),
            t.elapsed().as_secs_f64(),
            if nonzero == 0 { "ZERO" } else { "NONZERO" }
        );
        c_rows.push(json!({"seed": seed, "rows": at.len(), "keys": keys, "nonzero_terms": nonzero, "seconds": t.elapsed().as_secs_f64()}));
    }
    let summary = json!({
        "top": top_name, "dots": dots, "rank": rank, "inputs": inputs.len(), "terms": value.terms.len(),
        "load_seconds": load_seconds, "reduce_seconds": reduce_seconds, "owner_lane": format!("{:?}", owner.spec.lane),
        "laporta": b_report, "laporta_equal": b_ok, "ibp_seeds": c_rows, "ibp_zero": c_ok,
        "peak_rss_gb": rss().1,
    });
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    if !(b_ok && c_ok) {
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// edges: present every node (assembly step 3, no template checks) and build every rule-edge map
// ---------------------------------------------------------------------------

fn edges(families: &str, max_loops: u8, imports: Option<&str>) {
    use rustred::propagator::present::{edge_map, present_node};
    use rustred::propagator::topology::DagEdgeKind;
    let text = std::fs::read_to_string(families).unwrap();
    let (file, _) = load_p_families(&text).unwrap();
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let entries: Vec<_> = (file.families.iter())
        .filter(|e| e.loops <= max_loops)
        .cloned()
        .collect();
    let mut presentations: Vec<Arc<Presentation>> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        presentations.push(Arc::new(
            entry
                .presentation(PresentationId(index as u32), &context)
                .unwrap(),
        ));
    }
    let tops = presentations.clone();
    let roots: Vec<_> = entries.iter().map(top_topology).collect();
    let dag = TopologyService::new().reduction_graph(&roots, ChildMode::ForcerTree);
    // imported presentations by graph key (as the assembly hook swaps them in)
    let mut imported: HashMap<Vec<u8>, (String, Presentation)> = HashMap::new();
    if let Some(imports) = imports {
        for line in std::fs::read_to_string(imports)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        {
            let fields: Vec<&str> = line.split('\t').collect();
            let record: Value =
                serde_json::from_str(&std::fs::read_to_string(fields[1]).unwrap()).unwrap();
            imported.insert(
                unhex(record["graph_key"].as_str().unwrap()),
                (fields[0].to_owned(), record_presentation(&record, &context)),
            );
        }
    }
    let mut swapped = 0;
    let mut node_presentation: Vec<Option<Arc<Presentation>>> = Vec::new();
    let mut lanes: BTreeMap<String, usize> = BTreeMap::new();
    for node in &dag.nodes {
        let id = PresentationId(presentations.len() as u32);
        let made = present_node(node, &tops, id, &context).unwrap();
        *lanes
            .entry(format!(
                "L{} {:?}{}",
                node.loops,
                made.decision.lane,
                if made.fallback.is_some() {
                    " (fallback)"
                } else {
                    ""
                }
            ))
            .or_default() += 1;
        let mut presentation = made.presentation;
        if made.decision.lane == rustred::propagator::lane::OwnerLane::Custom
            && let Some((_, import)) = imported.get(node.key.as_bytes())
        {
            let mut adapted = import.clone();
            adapted.id = id;
            adapted.node = Some(rustred::propagator::presentation::NodeRef {
                index: rustred::propagator::NodeIndex(node.index),
                graph_key: Arc::clone(&node.key.0),
            });
            adapted.validate().unwrap();
            presentation = Some(adapted);
            swapped += 1;
        }
        let p = presentation.map(Arc::new);
        if let Some(p) = &p {
            presentations.push(Arc::clone(p));
        }
        node_presentation.push(p);
    }
    eprintln!(
        "dag {} nodes; lanes {lanes:?}; {swapped} imported presentations swapped in",
        dag.nodes.len()
    );
    let (mut ok, mut failed) = (0, 0);
    for edge in &dag.edges {
        if edge.kind != DagEdgeKind::Rule {
            continue;
        }
        let (Some(parent), Some(child)) = (
            &node_presentation[edge.parent as usize],
            &node_presentation[edge.child as usize],
        ) else {
            continue;
        };
        match edge_map(parent, child, edge) {
            Ok(_) => ok += 1,
            Err(error) => {
                failed += 1;
                let pn = &dag.nodes[edge.parent as usize];
                let cn = &dag.nodes[edge.child as usize];
                println!(
                    "FAIL {} -> {}: {error}\n  parent L{} t{} eps{} {:?} kind {:?} graph {:?}\n  child  L{} t{} eps{} {:?} kind {:?} graph {:?}\n  fates {:?}\n  parent lines {:?}\n  child lines {:?}",
                    edge.parent,
                    edge.child,
                    pn.loops,
                    pn.t,
                    pn.eps_lines,
                    pn.action.kind(),
                    parent.kind,
                    pn.topo,
                    cn.loops,
                    cn.t,
                    cn.eps_lines,
                    cn.action.kind(),
                    child.kind,
                    cn.topo,
                    edge.fates,
                    parent
                        .slots
                        .iter()
                        .filter_map(|slot| match slot {
                            Slot::Line(l) => Some((l.momentum.clone(), l.eps_units)),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    child
                        .slots
                        .iter()
                        .filter_map(|slot| match slot {
                            Slot::Line(l) => Some((l.momentum.clone(), l.eps_units)),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }
    }
    println!("rule edges: {ok} ok, {failed} failed");
}
