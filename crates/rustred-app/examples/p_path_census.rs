//! M5 R0 WP0 path census probe: one p reduction of one benchmark cell, lowered
//! exactly as vakint lowers it (`express_scalar_product`), with the opt-in
//! path census of the p engine (`rustred::propagator::path_census`).
//!
//! usage: p_path_census <p-library dir> <fixture json> <cell> <tops|all> <threads> <arith> <out dir>
//!
//! Environment:
//!   P_CENSUS=off            no census (plain closure inspector): the reference run of "census on and
//!                           off give byte-identical values"
//!   P_CENSUS_KEYS=1         keep every stepped Apply key (apply_keys.tsv, presentations.json): orbit counts
//!   P_CENSUS_MAX_KEYS=N     stop once N keys were popped (sizing)
//!   P_CENSUS_FORWARD_ONLY=1 stop at the end of the forward pass (the inspector errs at the last phase
//!                           boundary, before the modular replay): DAG sizing at about half the cost
//!   R0_LINES=a-b            keep only inputs whose support has a..=b lines
//!   R0_NUMERATOR, R0_POWERS override the cell's numerator and top powers
//!
//! Writes into <out dir>: inputs.tsv, phases.jsonl, preduction.txt (rustred.p-reduction.canonical.v1,
//! the T-VAL golden format, `p_path_census/canonical.rs`), summary.json, and with the census on:
//! steps.tsv, order.tsv, owners.tsv, inflow.tsv, inflow_profile.tsv, certificate.json, and when kept
//! apply_keys.tsv and presentations.json.
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::family::IntegralKey;
use rustred::propagator::error::PError;
use rustred::propagator::library::{PInspector, PLibrary, PReductionRequest, PUncoveredPolicy};
use rustred::propagator::path_census::{PPathCensus, PPathCensusOptions};
use rustred::propagator::presentation::{AffineSlotForm, Slot};
use rustred::propagator::{OwnerClassId, PresentationId};
use rustred::reduction::{
    ModularCommon, OrderedPhaseBoundary, OrderedPhaseEvent, OrderedSchedulerOptions,
    OrderedWeightArithmetic, PolynomialOptions, ReductionLimits, SampledOptions,
};
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};
use symbolica::prelude::Rational;

#[path = "p_path_census/canonical.rs"]
mod canonical;

type Input = (PresentationId, IntegralKey, Coefficient);

const TOP_NAMES: [&str; 32] = [
    "P1L", "P2L", "P3L_LA", "P3L_BE", "P3L_NO", "P3L_FA", "P3L_BU", "P3L_O1", "P4L_LA4", "P4L_J1",
    "P4L_J2", "P4L_J4", "P4L_J5", "P4L_J6", "P4L_J11", "P4L_HAHA", "P4L_NO1", "P4L_NO2", "P4L_NO3",
    "P4L_FA4", "P4L_BU4", "P4L_BE4", "P4L_NONO", "P4L_O4", "P4L_LA", "P4L_BE", "P4L_NO", "P4L_FA",
    "P4L_BU", "P4L_O1", "P4L_ZZ", "P4L_X",
];

fn rows(dot: (usize, usize), loops: usize) -> (Vec<i64>, Vec<i64>) {
    let unit = |index: usize| {
        let mut row = vec![0; loops + 1];
        row[index] = 1;
        row
    };
    (unit(dot.0), unit(dot.1))
}

struct Cell {
    loops: usize,
    tops: Vec<(String, Vec<i64>)>,
    dots: Vec<(usize, usize)>,
}

fn cell(fixture: &serde_json::Value, name: &str, filter: &str) -> Cell {
    let cell = fixture["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["name"] == name)
        .unwrap_or_else(|| panic!("cell {name}"));
    let loops = cell["loops"].as_u64().unwrap() as usize;
    let wanted: Vec<&str> = filter.split(',').collect();
    let tops = cell["tops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|top| {
            (
                top["family"].as_str().unwrap().to_owned(),
                top["powers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p.as_i64().unwrap())
                    .collect(),
            )
        })
        .filter(|(family, _): &(String, Vec<i64>)| {
            filter == "all" || wanted.contains(&family.as_str())
        })
        .collect::<Vec<_>>();
    assert!(!tops.is_empty(), "no top of {name} matches {filter}");
    let numerator = std::env::var("R0_NUMERATOR")
        .unwrap_or_else(|_| cell["numerator"].as_str().unwrap().to_owned());
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
    let tops = match std::env::var("R0_POWERS") {
        Ok(text) => {
            let powers: Vec<i64> = text.split(',').map(|p| p.trim().parse().unwrap()).collect();
            tops.into_iter()
                .map(|(family, _)| (family, powers.clone()))
                .collect()
        }
        Err(_) => tops,
    };
    Cell { loops, tops, dots }
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

/// vakint's lowering of `numerator * top(powers)`, summed over the tops.
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

fn rss() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<u64>().ok())
            .unwrap_or(0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

/// The inspector of this probe: phases.jsonl (time, RSS, progress), the census
/// when it is on, and the forward-only stop.
struct Probe {
    census: Option<PPathCensus>,
    phases: std::fs::File,
    started: Instant,
    last_phase: usize,
    forward_only: bool,
}

impl PInspector for Probe {
    fn inspect(&mut self, event: &OrderedPhaseEvent<'_>) -> Result<(), PError> {
        let now = self.started.elapsed().as_secs_f64();
        let (vmrss, vmhwm) = rss();
        let p = &event.progress;
        let boundary = match event.boundary {
            OrderedPhaseBoundary::Started => "started",
            OrderedPhaseBoundary::Finished => "finished",
        };
        writeln!(
            self.phases,
            "{{\"boundary\":\"{boundary}\",\"phase\":{},\"kind\":\"{:?}\",\"support\":{},\"t\":{now:.3},\"vmrss_kb\":{vmrss},\"vmhwm_kb\":{vmhwm},\"processed\":{},\"frontier\":{},\"peak_frontier\":{},\"terminal_keys\":{},\"emitted\":{},\"merged\":{},\"cancelled\":{},\"hits\":{},\"misses\":{}}}",
            event.phase,
            event.descriptor.kind,
            event.descriptor.support.map_or(-1, |s| s as i64),
            p.processed_keys,
            p.frontier_keys,
            p.peak_frontier_keys,
            p.terminal_keys,
            p.emitted_terms,
            p.merged_terms,
            p.cancelled_terms,
            p.step_cache_hits,
            p.step_cache_misses,
        )
        .unwrap();
        self.phases.flush().unwrap();
        eprintln!(
            "[census] {now:9.2}s phase {:2} {boundary:8} processed {:>10} frontier {:>9} rss {:.2} GB",
            event.phase,
            p.processed_keys,
            p.frontier_keys,
            vmrss as f64 / 1048576.0
        );
        if self.forward_only
            && event.boundary == OrderedPhaseBoundary::Finished
            && event.phase == self.last_phase
        {
            return Err(PError::Unsupported(
                "forward-only census: stopped before the replay".to_owned(),
            ));
        }
        Ok(())
    }

    fn path_census(&mut self) -> Option<&mut PPathCensus> {
        self.census.as_mut()
    }
}

fn tsv(path: PathBuf) -> std::io::BufWriter<std::fs::File> {
    std::io::BufWriter::new(std::fs::File::create(path).unwrap())
}

fn main() {
    let t_main = Instant::now();
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() >= 8,
        "usage: p_path_census <p-library dir> <fixture json> <cell> <tops|all> <threads> <arith> <out dir>"
    );
    let library_dir = PathBuf::from(&args[1]);
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
    let cell_name = args[3].as_str();
    let filter = args[4].as_str();
    let threads: usize = args[5].parse().unwrap();
    let arith = args[6].as_str();
    let out = PathBuf::from(&args[7]);
    let flag = |name: &str| std::env::var(name).is_ok_and(|v| v == "1");
    let census_on = std::env::var("P_CENSUS").map_or(true, |v| v != "off");
    let keep_keys = flag("P_CENSUS_KEYS");
    let forward_only = flag("P_CENSUS_FORWARD_ONLY");
    let max_keys = std::env::var("P_CENSUS_MAX_KEYS")
        .ok()
        .map(|v| v.parse::<u64>().unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let t_load = Instant::now();
    let library = load_p_library(
        PLibrarySource::Directory(library_dir.clone()),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap();
    let load_seconds = t_load.elapsed().as_secs_f64();
    let (_, load_vmhwm) = rss();
    let cell = cell(&fixture, cell_name, filter);
    let mut inputs = inputs(&library, &cell);
    if let Ok(range) = std::env::var("R0_LINES") {
        let (lo, hi) = range
            .split_once('-')
            .unwrap_or((range.as_str(), range.as_str()));
        let (lo, hi): (u32, u32) = (lo.parse().unwrap(), hi.parse().unwrap());
        inputs.retain(|(presentation, key, _)| {
            let p = library.presentation(*presentation);
            let lines = key
                .powers()
                .iter()
                .zip(&p.slots)
                .filter(|(power, slot)| **power > 0 && matches!(slot, Slot::Line(_)))
                .count() as u32;
            (lo..=hi).contains(&lines)
        });
    }
    eprintln!(
        "[census] load {load_seconds:.2} s; {cell_name} tops {:?}; {} inputs; census {census_on}",
        cell.tops,
        inputs.len()
    );

    let presentations = library.presentations();
    let lines_mask: Vec<u64> = presentations
        .iter()
        .map(|p| {
            p.slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| matches!(slot, Slot::Line(_)))
                .fold(0u64, |mask, (slot, _)| mask | 1 << slot)
        })
        .collect();
    let mut names: HashMap<u32, String> = HashMap::new();
    for name in TOP_NAMES {
        if let Some(id) = library.top_presentation(name) {
            names.insert(id.0, name.to_owned());
        }
    }
    {
        let mut census: BTreeMap<(u32, u32, i64, i64), usize> = BTreeMap::new();
        for (presentation, key, _) in &inputs {
            let mask = lines_mask[presentation.0 as usize];
            let powers = key.powers();
            let lines = powers
                .iter()
                .enumerate()
                .filter(|&(s, &p)| p > 0 && mask >> s & 1 == 1)
                .count() as u32;
            let dots: i64 = powers.iter().filter(|&&p| p > 1).map(|&p| p - 1).sum();
            let rank: i64 = powers.iter().filter(|&&p| p < 0).map(|&p| -p).sum();
            *census
                .entry((presentation.0, lines, dots, rank))
                .or_default() += 1;
        }
        let mut file = tsv(out.join("inputs.tsv"));
        writeln!(file, "presentation\tname\tlines\tdots\trank\tinputs").unwrap();
        for ((presentation, lines, dots, rank), count) in census {
            let name = names.get(&presentation).cloned().unwrap_or_default();
            writeln!(
                file,
                "{presentation}\t{name}\t{lines}\t{dots}\t{rank}\t{count}"
            )
            .unwrap();
        }
    }

    let options = PPathCensusOptions {
        keep_apply_keys: keep_keys,
        max_keys,
    };
    let mut probe = Probe {
        census: census_on.then(|| PPathCensus::new(options)),
        phases: std::fs::File::create(out.join("phases.jsonl")).unwrap(),
        started: Instant::now(),
        last_phase: 0,
        forward_only,
    };
    // The last phase index: 2 * max_lines + 1 (PhasePlan), max_lines over the presentations.
    let max_lines = lines_mask
        .iter()
        .map(|m| m.count_ones() as usize)
        .max()
        .unwrap_or(0);
    probe.last_phase = 2 * max_lines + 1;
    let t_reduce = Instant::now();
    let result = library.reduce(PReductionRequest {
        inputs: inputs.clone(),
        arithmetic: arithmetic(arith, threads),
        scheduler: OrderedSchedulerOptions::with_threads(threads),
        step_cache: None,
        uncovered: PUncoveredPolicy::Reject,
        onto_masters: true,
        limits: ReductionLimits::default(),
        inspector: Some(&mut probe),
    });
    let reduce_seconds = t_reduce.elapsed().as_secs_f64();
    let (vmrss, vmhwm) = rss();
    // P_WARM=1 (M5 1L-3L): the same request again in this process, without
    // inspector or census, after the cold one prepared routes and owner
    // programs: the warm time of a long-lived process (vakint).
    let warm_seconds = flag("P_WARM").then(|| {
        let t_warm = Instant::now();
        let warm = library.reduce(PReductionRequest {
            inputs: inputs.clone(),
            arithmetic: arithmetic(arith, threads),
            scheduler: OrderedSchedulerOptions::with_threads(threads),
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        });
        let seconds = t_warm.elapsed().as_secs_f64();
        if let (Ok(cold), Ok(warm)) = (&result, &warm) {
            assert_eq!(
                canonical::canonical_text(cold),
                canonical::canonical_text(warm),
                "the warm request gives another reduction"
            );
        }
        seconds
    });
    let (status, terms, progress) = match &result {
        Ok(reduction) => {
            std::fs::write(
                out.join("preduction.txt"),
                canonical::canonical_text(reduction),
            )
            .unwrap();
            (
                "ok".to_owned(),
                reduction.terms.len(),
                Some(reduction.statistics.progress),
            )
        }
        Err(error) => (format!("stopped: {error}"), 0, None),
    };
    eprintln!(
        "[census] reduce {reduce_seconds:.2} s, {status}, peak RSS {:.2} GB",
        vmhwm as f64 / 1048576.0
    );

    let mut summary = serde_json::json!({
        "cell": cell_name, "tops": filter, "threads": threads, "arithmetic": arith, "inputs": inputs.len(),
        "census": census_on, "keep_apply_keys": keep_keys, "forward_only": forward_only, "max_keys": max_keys,
        "status": status, "terms": terms, "load_seconds": load_seconds, "reduce_seconds": reduce_seconds,
        "vmrss_kb": vmrss, "vmhwm_kb": vmhwm, "schema": canonical::CANONICAL_SCHEMA,
        "load_vmhwm_kb": load_vmhwm, "warm_seconds": warm_seconds,
        "process_seconds": t_main.elapsed().as_secs_f64(),
    });
    if let Some(p) = progress {
        summary["processed"] = p.processed_keys.into();
        summary["peak_frontier"] = p.peak_frontier_keys.into();
        summary["emitted"] = p.emitted_terms.into();
        summary["merged"] = p.merged_terms.into();
        summary["cancelled"] = p.cancelled_terms.into();
        summary["terminal_keys"] = p.terminal_keys.into();
    }
    if let Some(census) = &probe.census {
        let certificate = census.certificate();
        let total = census
            .steps
            .values()
            .fold((0u64, 0u64, 0u64, 0u64), |acc, a| {
                (
                    acc.0 + a.steps,
                    acc.1 + a.edges(),
                    acc.2 + a.nanos,
                    acc.3 + a.ch_zero,
                )
            });
        summary["steps"] = total.0.into();
        summary["edges"] = total.1.into();
        summary["step_seconds"] = (total.2 as f64 / 1e9).into();
        summary["edges_into_zero_sectors"] = total.3.into();
        summary["popped"] = census.popped.into();
        summary["distinct"] = census.distinct().into();
        summary["repeats"] = census.repeats.into();
        summary["owners_used"] = census.owners.len().into();
        summary["order_runs"] = census.order.len().into();
        summary["t_bfs"] = certificate.holds().into();
        let certificate_json = serde_json::json!({
            "holds": certificate.holds(), "popped": certificate.popped, "distinct": certificate.distinct,
            "repeats": certificate.repeats, "phase_regressions": certificate.phase_regressions,
            "split_owner_phases": certificate.split_owner_phases, "apply_phases": certificate.apply_phases,
            "earlier_phase_children": certificate.earlier_phase_children,
            "foreign_same_phase_children": certificate.foreign_same_phase_children,
            "cache_hits": certificate.cache_hits,
            "fold": "off: SeriesParallelStep::with_fold is only called from tests (src never sets it)",
            "check_descent": status == "ok" || forward_only,
        });
        std::fs::write(
            out.join("certificate.json"),
            serde_json::to_string_pretty(&certificate_json).unwrap(),
        )
        .unwrap();
        let lane = |class: u64| {
            census
                .owners
                .get(&class)
                .map_or(String::new(), |o| o.lane.clone())
        };
        let owner_presentation =
            |class: u64| census.owners.get(&class).map_or(0, |o| o.presentation);
        // rule_hits.tsv (M5 artefact v2, spec item 8): per Custom owner, chart,
        // rule position (application order) and phase, the steps that applied it.
        let mut file = tsv(out.join("rule_hits.tsv"));
        writeln!(file, "class\tpresentation\tchart\trule\tphase\thits").unwrap();
        for ((class, chart, rule, phase), hits) in &census.rule_hits {
            writeln!(
                file,
                "{class:016x}\t{}\t{chart:#x}\t{rule}\t{phase}\t{hits}",
                owner_presentation(*class)
            )
            .unwrap();
        }
        drop(file);
        summary["rule_hits"] = census.rule_hits.values().sum::<u64>().into();
        // steps.tsv: the rustred-path columns, then the census additions.
        let mut file = tsv(out.join("steps.tsv"));
        writeln!(file, "kind\tid\tpresentation\tname\tlane\tphase\tlines\tdots\trank\tsteps\tstep_ms\tch_same\tch_apply\tch_route\tterminals\tch_max\tcoeff_terms\tcoeff_terms_max\tempty\tch_zero\tch_same_phase\tch_foreign_same_phase\tch_earlier_phase").unwrap();
        for ((phase, kind, id, lines, dots, rank), a) in &census.steps {
            let (presentation, lane) = if *kind == 0 {
                (*id as u32, "Route".to_owned())
            } else {
                (owner_presentation(*id), lane(*id))
            };
            writeln!(
                file,
                "{}\t{id:016x}\t{presentation}\t{}\t{lane}\t{phase}\t{lines}\t{dots}\t{rank}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                if *kind == 0 { "R" } else { "A" },
                names.get(&presentation).cloned().unwrap_or_default(),
                a.steps, a.nanos as f64 / 1e6, a.ch_same, a.ch_apply, a.ch_route, a.terminals, a.ch_max, a.coeff_terms,
                a.coeff_terms_max, a.empty, a.ch_zero, a.ch_same_phase, a.ch_foreign_same_phase, a.ch_earlier_phase
            )
            .unwrap();
        }
        let mut file = tsv(out.join("order.tsv"));
        writeln!(file, "seq\tphase\tkind\tid\tpresentation\tlane\tcount").unwrap();
        for (seq, (phase, kind, id, count)) in census.order.iter().enumerate() {
            let (presentation, lane) = if *kind == 0 {
                (*id as u32, "Route".to_owned())
            } else {
                (owner_presentation(*id), lane(*id))
            };
            writeln!(
                file,
                "{seq}\t{phase}\t{}\t{id:016x}\t{presentation}\t{lane}\t{count}",
                if *kind == 0 { "R" } else { "A" }
            )
            .unwrap();
        }
        let mut file = tsv(out.join("owners.tsv"));
        writeln!(
            file,
            "class\tlane\tpresentation\tname\tt\tnode_rank\tloops\tlines\tkind\tsupport"
        )
        .unwrap();
        for (class, o) in &census.owners {
            let p = &presentations[o.presentation as usize];
            let support = library
                .census()
                .classes
                .get(&OwnerClassId(*class))
                .map_or(0, |r| r.owner.1);
            writeln!(
                file,
                "{class:016x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{support:#x}",
                o.lane,
                o.presentation,
                names.get(&o.presentation).cloned().unwrap_or_default(),
                o.t,
                o.rank,
                p.loops,
                lines_mask[o.presentation as usize].count_ones(),
                p.kind
            )
            .unwrap();
        }
        let mut file = tsv(out.join("inflow.tsv"));
        writeln!(file, "phase\tkind\tid\tpresentation\tlane\tinflow\tlater").unwrap();
        for (phase, inflow) in &census.inflow {
            let later = census.later.get(phase).copied().unwrap_or(0);
            for ((kind, id), n) in inflow {
                let (presentation, lane) = if *kind == 0 {
                    (*id as u32, "Route".to_owned())
                } else {
                    let o = library.census().classes.get(&OwnerClassId(*id));
                    let lane = library
                        .owner(OwnerClassId(*id))
                        .map_or(String::new(), |o| format!("{:?}", o.spec.lane));
                    (o.map_or(0, |o| o.owner.0.0), lane)
                };
                writeln!(
                    file,
                    "{phase}\t{}\t{id:016x}\t{presentation}\t{lane}\t{n}\t{later}",
                    if *kind == 0 { "R" } else { "A" }
                )
                .unwrap();
            }
        }
        let mut file = tsv(out.join("inflow_profile.tsv"));
        writeln!(file, "phase\tlines\tdots\trank\tinflow").unwrap();
        for (phase, profile) in &census.inflow_profile {
            for ((lines, dots, rank), n) in profile {
                writeln!(file, "{phase}\t{lines}\t{dots}\t{rank}\t{n}").unwrap();
            }
        }
        let mut file = tsv(out.join("census_phases.tsv"));
        writeln!(file, "phase\tboundary\tseconds\tprocessed\tfrontier\tpeak_frontier\temitted\tmerged\tcancelled").unwrap();
        for ph in &census.phases {
            let p = &ph.progress;
            writeln!(
                file,
                "{}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}\t{}",
                ph.phase,
                if ph.started { "started" } else { "finished" },
                ph.seconds,
                p.processed_keys,
                p.frontier_keys,
                p.peak_frontier_keys,
                p.emitted_terms,
                p.merged_terms,
                p.cancelled_terms
            )
            .unwrap();
        }
        if keep_keys {
            let mut file = tsv(out.join("apply_keys.tsv"));
            writeln!(file, "class\tview").unwrap();
            for (class, view) in &census.apply_keys {
                let view: Vec<String> = view.iter().map(i64::to_string).collect();
                writeln!(file, "{class:016x}\t{}", view.join(",")).unwrap();
            }
            // Slots of every owner presentation, for the orbit counts.
            let mut owners = Vec::new();
            for (class, o) in &census.owners {
                let p = &presentations[o.presentation as usize];
                let support = library
                    .census()
                    .classes
                    .get(&OwnerClassId(*class))
                    .map_or(0, |r| r.owner.1);
                let slots: Vec<serde_json::Value> = p
                    .slots
                    .iter()
                    .map(|slot| match slot {
                        Slot::Line(line) => {
                            serde_json::json!({"line": line.momentum, "eps": line.eps_units})
                        }
                        Slot::Isp(isp) => serde_json::json!({"isp": [isp.left, isp.right]}),
                    })
                    .collect();
                owners.push(serde_json::json!({
                    "class": format!("{class:016x}"), "lane": o.lane, "presentation": o.presentation,
                    "loops": p.loops, "support": support, "t": o.t, "slots": slots,
                }));
            }
            std::fs::write(
                out.join("presentations.json"),
                serde_json::to_string(&owners).unwrap(),
            )
            .unwrap();
        }
    }
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("{summary}");
}
