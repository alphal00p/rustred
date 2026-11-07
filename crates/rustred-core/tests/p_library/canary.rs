//! 4L eps regression set and the 5L canary (owner: M2-T10b rr-canary).
//!
//! A report harness on top of the two-sided generation of M2-T7b and the DAG
//! and lanes of M2-T1b and M2-T3; it holds no library code and pins no cost.
//!
//! Parts A and C (generation): one chart per process, so a cap keeps the other
//! charts' numbers. `run::family_info` lists the charts and counts LP,
//! `run::chart` solves one chart, `run::union_guard` puts the hard guard on the
//! union, `run::closure` takes random points through a family's rules. They
//! read `CANARY_*` variables (see `run::required`). `family_info` and `chart`
//! refuse a family whose forced axes are not the expected stars: an uncrossed or
//! partly crossed file gives valid IBPs of the wrong integral. The driver that
//! sets the variables and holds the caps is `$WS/SCRATCHPAD/m2-canary/drive.py`.
//! The LP count of a 10- or 11-line 4L family outlasts the caps of the report
//! rows, so those rows take LP from `rules_study/targets.tsv` (`lp_source`) and
//! solve through [`solve_uncounted`]; part A always counts it.
//!
//! Part B (lanes only): [`lanes_5l_counts`] pins the ForcerTree DAG of the 64
//! 5L tops and `decide` on every node against the cascade census.
//! `run::lanes_5l_report` compares node by node with the census tables and
//! gives the lane of every E1 batch A/B sector.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustred::family::IntegralFamily;
use rustred::input::{Compiler, Limits, LoweringLimits};
use rustred::order::{CompiledOrder, OrderDescriptor};
use rustred::propagator::decide::decide;
use rustred::propagator::lane::OwnerLane;
use rustred::propagator::topology::{
    ChildMode, Dag, DagEdgeKind, GraphKey, Normalised, PTopo, TopologyService, canonical, normalise,
};
use rustred::sector::{Mask, MasterCount, MasterCountOptions, MasterCounter, zero};
use rustred::solver::{SectorConfig, SectorExecutor, SectorSolveOptions, SourceSystem};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::twosided::{
    EpsCharts, check_eps_guard, eps_family_charts, eps_family_source, generate_eps_charts,
    generate_eps_family,
};

#[path = "../../../rustred-topology/tests/support/forcer.rs"]
mod forcer;

// ----- generation: one chart per process, the guard on the union -----

/// The axes of a family file, read from the compiled family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FamilyAxes {
    family: String,
    /// The file the row was generated from: rule counts are routing-specific.
    file: String,
    forced: String,
    support: String,
    /// Lee-Pomeransky master count of the owner support, with shifts.
    lp: u32,
    /// `counted` here, or `given`: the targets table's, not counted in this run.
    lp_source: String,
    charts: Vec<String>,
}

/// One chart of one family: a refused chart has `error`, a capped one has no
/// row at all (the driver writes it from the guard's `completion.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ChartRow {
    family: String,
    file: String,
    chart: String,
    forced: String,
    support: String,
    two_sided: Option<String>,
    lp: u32,
    rules: Option<usize>,
    residuals: Option<Vec<Vec<i64>>>,
    error: Option<String>,
    /// The chart's own solve.
    chart_seconds: Option<f64>,
    /// Wall clock of the call, with compilation, zero sectors and LP.
    total_seconds: f64,
    /// Process peak and process CPU time.
    vm_hwm_kb: Option<u64>,
    cpu_seconds: Option<f64>,
}

/// Verdict on the union of a family's charts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct UnionRow {
    family: String,
    file: String,
    lp: u32,
    charts: usize,
    closed: usize,
    rules: usize,
    residuals: Vec<Vec<i64>>,
    /// `residuals=LP`, `residuals>LP`, or why the union has no verdict.
    outcome: String,
    guard: String,
}

fn parse_mask(mask: &str) -> Result<Vec<bool>, String> {
    mask.chars()
        .map(|c| match c {
            '1' => Ok(true),
            '0' => Ok(false),
            other => Err(format!("mask {mask}: {other:?} is not 0 or 1")),
        })
        .collect()
}

fn format_mask(bits: &[bool]) -> String {
    bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

fn compile(sym: &str) -> Result<(String, IntegralFamily, Vec<i64>), String> {
    let project = Compiler::new(Limits::default())
        .map_err(|e| e.to_string())?
        .compile_compact(sym, None)
        .map_err(|e| e.to_string())?;
    let name = project.name().to_owned();
    let target = project
        .propagators()
        .iter()
        .map(|p| p.target_power())
        .collect();
    let family = project
        .into_lowered(LoweringLimits::default())
        .map_err(|e| e.to_string())?
        .into_family();
    Ok((name, family, target))
}

/// The owner support's LP count with shifts.
fn count_lp(name: &str, family: &IntegralFamily, support: &[bool]) -> Result<u32, String> {
    let mut counter = MasterCounter::try_new_p_shifted(family, MasterCountOptions::default())
        .map_err(|e| e.to_string())?;
    let top = Mask::try_new(support.iter().copied()).map_err(|e| e.to_string())?;
    match counter.count(&top).map_err(|e| e.to_string())? {
        MasterCount::Zero => Ok(0),
        MasterCount::Counted(count) => u32::try_from(count).map_err(|e| e.to_string()),
        other => Err(format!("{name}: no LP verdict: {other:?}")),
    }
}

/// The axes of `sym`, after checking the forced axes against `stars`: the
/// starred lines of the node the row claims to be, in the file's slot order.
/// LP is counted unless given.
fn family_axes(
    file: &str,
    sym: &str,
    stars: &[bool],
    given_lp: Option<u32>,
) -> Result<FamilyAxes, String> {
    let (name, family, target) = compile(sym)?;
    let forced: Vec<bool> = family.power_shifts().iter().map(|s| !s.is_zero()).collect();
    if forced != stars {
        return Err(format!(
            "{name} ({file}): forced axes {} are not the stars {}",
            format_mask(&forced),
            format_mask(stars),
        ));
    }
    let support: Vec<bool> = (0..forced.len())
        .map(|i| target[i] > 0 || forced[i])
        .collect();
    let (lp, lp_source) = match given_lp {
        Some(lp) => (lp, "given"),
        None => (count_lp(&name, &family, &support)?, "counted"),
    };
    Ok(FamilyAxes {
        family: name,
        file: file.to_owned(),
        forced: format_mask(&forced),
        support: format_mask(&support),
        lp,
        lp_source: lp_source.to_owned(),
        charts: eps_family_charts(sym)?
            .iter()
            .map(|c| format_mask(c))
            .collect(),
    })
}

/// What a solved chart reports.
struct SolvedChart {
    two_sided: Vec<bool>,
    rules: usize,
    residuals: Vec<Vec<i64>>,
    seconds: f64,
}

/// One chart, one worker, with the solve of `twosided::generate_eps_charts`
/// (same sources, order program and zero sectors), minus its LP count. Pinned
/// to it by [`the_uncounted_solve_is_the_helper_solve`].
fn solve_uncounted(sym: &str, chart: &[bool]) -> Result<SolvedChart, String> {
    let (name, family, target) = compile(sym)?;
    macro_rules! dispatch {
        ($($n:literal),*) => {
            match family.denominator_count() {
                $($n => solve_chart::<$n>(&name, &family, &target, chart),)*
                other => Err(format!("arity {other} is not instantiated in the tests")),
            }
        };
    }
    dispatch!(5, 9, 14)
}

fn solve_chart<const N: usize>(
    name: &str,
    family: &IntegralFamily,
    target: &[i64],
    chart: &[bool],
) -> Result<SolvedChart, String> {
    let n = family.denominator_count();
    let mut sources =
        SourceSystem::<N>::from_family_with_capacity(family, true).map_err(|e| e.to_string())?;
    let axes = sources
        .set_two_sided_eps_axes(family)
        .map_err(|e| e.to_string())?;
    let two_sided = axes[..n].to_vec();
    let descriptor = if two_sided.contains(&true) {
        OrderDescriptor::spired_uncut_two_sided(two_sided.clone())
    } else {
        OrderDescriptor::spired_uncut(n)
    };
    let program =
        CompiledOrder::compile(descriptor, Default::default()).map_err(|e| e.to_string())?;

    // Zero raw masks inside the support (forced axes are always effective).
    let forced = family.power_shifts();
    let free: Vec<usize> = (0..n)
        .filter(|&i| target[i] > 0 || !forced[i].is_zero())
        .collect();
    let analyzer = zero::Analyzer::try_unrestricted(family).map_err(|e| e.to_string())?;
    let mut zeros: Vec<[bool; N]> = Vec::new();
    for bits in 0u64..1 << free.len() {
        let mut sector = [false; N];
        for (bit, &axis) in free.iter().enumerate() {
            sector[axis] = bits >> bit & 1 == 1;
        }
        let raw = Mask::try_new(sector[..n].iter().copied()).map_err(|e| e.to_string())?;
        if let zero::Decision::ProvedZero(_) = analyzer.analyze(&raw).map_err(|e| e.to_string())? {
            zeros.push(sector);
        }
    }

    let sector: [bool; N] = std::array::from_fn(|i| i < n && chart[i]);
    if zeros.contains(&sector) {
        return Err(format!(
            "{name}: chart {} is a zero sector",
            format_mask(chart)
        ));
    }
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros),
        integral_order: Some(program),
        ..Default::default()
    };
    let executor = SectorExecutor::new(1).map_err(|e| e.to_string())?;
    let mut solved = executor
        .map(
            &sources,
            &[sector],
            &config,
            SectorSolveOptions::default(),
            |done| {
                let solution = &done.solution;
                let powers = |key: &rustred::solver::Integral<N>| -> Vec<i64> {
                    (key.powers()[..n].iter())
                        .map(|p| i64::from(p.value()))
                        .collect()
                };
                Ok::<_, String>(SolvedChart {
                    two_sided: two_sided.clone(),
                    rules: solution.rules.len(),
                    residuals: solution.finite_residuals.iter().map(powers).collect(),
                    seconds: (done.preconditioning + solution.stats.elapsed).as_secs_f64(),
                })
            },
        )
        .map_err(|e| format!("{name} chart {}: {e}", format_mask(&e.sector()[..n])))?;
    solved
        .pop()
        .ok_or_else(|| format!("{name}: no chart solved"))
}

/// Solve one chart. A refusal is a row, not a failure: it is what the run is
/// there to record, with its message uncut.
fn chart_row(axes: &FamilyAxes, sym: &str, chart: &str, workers: usize) -> ChartRow {
    let start = std::time::Instant::now();
    let solved = parse_mask(chart).and_then(|bits| {
        if axes.lp_source == "given" {
            return solve_uncounted(sym, &bits);
        }
        let outcome = generate_eps_charts(sym, &[bits], workers)?;
        Ok(SolvedChart {
            seconds: outcome.charts.iter().map(|c| c.seconds).sum(),
            two_sided: outcome.two_sided,
            rules: outcome.rules,
            residuals: outcome.residual_keys,
        })
    });
    let total_seconds = start.elapsed().as_secs_f64();
    let mut row = ChartRow {
        family: axes.family.clone(),
        file: axes.file.clone(),
        chart: chart.to_owned(),
        forced: axes.forced.clone(),
        support: axes.support.clone(),
        two_sided: None,
        lp: axes.lp,
        rules: None,
        residuals: None,
        error: None,
        chart_seconds: None,
        total_seconds,
        vm_hwm_kb: vm_hwm_kb(),
        cpu_seconds: process_cpu_seconds(),
    };
    match solved {
        Ok(chart) => {
            row.two_sided = Some(format_mask(&chart.two_sided));
            row.rules = Some(chart.rules);
            row.residuals = Some(chart.residuals);
            row.chart_seconds = Some(chart.seconds);
        }
        Err(error) => row.error = Some(error),
    }
    row
}

/// The hard guard on the union of the charts of one family: a completed union
/// with fewer residuals than LP is a gate failure, the `Err` here. A refused or
/// missing chart leaves the union without a verdict, which is a recorded miss.
fn union_row(axes: &FamilyAxes, rows: &[ChartRow]) -> Result<UnionRow, String> {
    if rows.iter().any(|row| row.family != axes.family) {
        return Err(format!("rows of another family than {}", axes.family));
    }
    let closed: Vec<&ChartRow> = rows.iter().filter(|row| row.rules.is_some()).collect();
    let residuals: Vec<Vec<i64>> = closed
        .iter()
        .flat_map(|row| row.residuals.iter().flatten().cloned())
        .collect();
    let mut union = UnionRow {
        family: axes.family.clone(),
        file: axes.file.clone(),
        lp: axes.lp,
        charts: axes.charts.len(),
        closed: closed.len(),
        rules: closed.iter().filter_map(|row| row.rules).sum(),
        residuals,
        outcome: String::new(),
        guard: "not run".to_owned(),
    };
    let solved: BTreeSet<&str> = closed.iter().map(|row| row.chart.as_str()).collect();
    let missing = axes
        .charts
        .iter()
        .filter(|chart| !solved.contains(chart.as_str()))
        .count();
    if missing > 0 {
        union.outcome = format!("no verdict: {missing} of {} charts open", axes.charts.len());
        return Ok(union);
    }
    let two_sided = closed[0].two_sided.as_deref().unwrap_or_default();
    let charts = axes
        .charts
        .iter()
        .map(|chart| parse_mask(chart))
        .collect::<Result<Vec<_>, _>>()?;
    check_eps_guard(
        &parse_mask(&axes.forced)?,
        &parse_mask(two_sided)?,
        &parse_mask(&axes.support)?,
        &charts,
        axes.lp,
        union.residuals.len(),
    )?;
    union.guard = "pass".to_owned();
    union.outcome = if union.residuals.len() == axes.lp as usize {
        "residuals=LP".to_owned()
    } else {
        "residuals>LP".to_owned()
    };
    Ok(union)
}

fn status_field(field: &str) -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with(field))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn vm_hwm_kb() -> Option<u64> {
    status_field("VmHWM:")
}

/// User + system time of this process, all threads; 100 ticks per second.
fn process_cpu_seconds() -> Option<f64> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let mut after_name = stat.rsplit_once(')')?.1.split_whitespace();
    let utime: f64 = after_name.nth(11)?.parse().ok()?;
    let stime: f64 = after_name.next()?.parse().ok()?;
    Some((utime + stime) / 100.0)
}

/// Asserting a forced axis the file does not carry is the trap of the
/// uncrossed family files; either way round the harness refuses before a run.
#[test]
fn a_file_whose_forced_axes_are_not_the_stars_is_refused() {
    let t007c = eps_family_source("T007c");
    let axes = family_axes("T007c", t007c, &parse_mask("10000").unwrap(), None);
    assert_eq!(axes.as_ref().map(|a| a.lp), Ok(1), "{axes:?}");
    let wrong = family_axes("T007c", t007c, &parse_mask("01000").unwrap(), None).unwrap_err();
    assert!(wrong.contains("are not the stars"), "{wrong}");
    let uncrossed = family_axes("T007c", t007c, &parse_mask("00000").unwrap(), None).unwrap_err();
    assert!(uncrossed.contains("are not the stars"), "{uncrossed}");
    let bubu = eps_family_source("bubu");
    let stars = vec![false; 14];
    let axes = family_axes("bubu", bubu, &stars, Some(0)).unwrap();
    assert_eq!((axes.charts.len(), axes.lp_source.as_str()), (1, "given"));
}

/// Chart by chart equals the whole family: same rules, same residual, and the
/// guard holds on the union. A forged union without the residual fails it, and
/// a union with an open chart has no verdict.
#[test]
fn the_union_of_single_chart_processes_passes_the_guard() {
    let sym = eps_family_source("T007c");
    let axes = family_axes("T007c", sym, &parse_mask("10000").unwrap(), None).unwrap();
    let rows: Vec<ChartRow> = axes
        .charts
        .iter()
        .map(|chart| chart_row(&axes, sym, chart, 1))
        .collect();
    let union = union_row(&axes, &rows).unwrap();
    let whole = generate_eps_family(sym, EpsCharts::All).unwrap();
    assert_eq!(union.rules, whole.rules);
    assert_eq!(union.residuals, whole.residual_keys);
    assert_eq!((union.lp, union.outcome.as_str()), (1, "residuals=LP"));

    let mut forged = rows.clone();
    for row in &mut forged {
        row.residuals = Some(Vec::new());
    }
    let refused = union_row(&axes, &forged).unwrap_err();
    assert!(refused.contains("false knock-out"), "{refused}");

    let open = union_row(&axes, &rows[1..]).unwrap();
    assert_eq!(open.outcome, "no verdict: 1 of 2 charts open");
    assert_eq!(open.guard, "not run");
}

/// A refused chart is a row with the message and no rule count.
#[test]
fn a_refused_chart_is_a_row_not_a_failure() {
    let sym = eps_family_source("T007c");
    let axes = family_axes("T007c", sym, &parse_mask("10000").unwrap(), None).unwrap();
    let row = chart_row(&axes, sym, "1x111", 1);
    assert_eq!(row.rules, None);
    assert!(row.error.as_deref().unwrap().contains("not 0 or 1"));
    let union = union_row(&axes, &[row]).unwrap();
    assert_eq!(union.outcome, "no verdict: 2 of 2 charts open");
}

/// The path without the LP count gives the rows of the helper, chart by chart.
#[test]
fn the_uncounted_solve_is_the_helper_solve() {
    for (name, stars) in [("T007c", "10000"), ("T001c", "10001"), ("T003c", "00011")] {
        let sym = eps_family_source(name);
        let stars = parse_mask(stars).unwrap();
        let counted = family_axes(name, sym, &stars, None).unwrap();
        let given = family_axes(name, sym, &stars, Some(counted.lp)).unwrap();
        assert_eq!(given.charts, counted.charts);
        for chart in &counted.charts {
            let helper = chart_row(&counted, sym, chart, 1);
            let uncounted = chart_row(&given, sym, chart, 1);
            assert!(helper.rules.is_some(), "{name} {chart}: {:?}", helper.error);
            assert_eq!(
                (helper.rules, helper.residuals, helper.two_sided),
                (uncounted.rules, uncounted.residuals, uncounted.two_sided),
                "{name} {chart}",
            );
        }
    }
}

// ----- part B: lanes only -----

/// `c_L5_ftops_forcer.json` `by_L_kind_action`, the cascade census of the
/// ForcerTree DAG of the 64 5L tops (`$WS/SCRATCHPAD/forcer-tricks/census`).
const CENSUS_L5: [(u8, &str, &str, usize); 31] = [
    (0, "int", "trivial", 1),
    (1, "eps", "insertion", 8),
    (1, "int", "insertion", 1),
    (2, "eps", "custom", 14),
    (2, "eps", "insertion", 37),
    (2, "eps", "triangle", 5),
    (2, "int", "insertion", 3),
    (2, "int", "triangle", 1),
    (3, "eps", "carpet", 35),
    (3, "eps", "custom", 42),
    (3, "eps", "insertion", 287),
    (3, "eps", "triangle", 50),
    (3, "int", "carpet", 2),
    (3, "int", "custom", 1),
    (3, "int", "insertion", 20),
    (3, "int", "triangle", 4),
    (4, "eps", "carpet", 86),
    (4, "eps", "custom", 88),
    (4, "eps", "diamond", 10),
    (4, "eps", "insertion", 1252),
    (4, "eps", "triangle", 202),
    (4, "int", "carpet", 14),
    (4, "int", "custom", 9),
    (4, "int", "diamond", 4),
    (4, "int", "insertion", 220),
    (4, "int", "triangle", 33),
    (5, "int", "carpet", 144),
    (5, "int", "custom", 129),
    (5, "int", "diamond", 28),
    (5, "int", "insertion", 3384),
    (5, "int", "triangle", 457),
];

/// The ForcerTree DAG of the `loops`-loop tops, through the service.
fn forcer_tree(service: &TopologyService, loops: u8) -> Dag {
    let tops: Vec<PTopo> = forcer::forcer_tops(loops)
        .into_iter()
        .map(|(_, topo)| topo)
        .collect();
    service.reduction_graph(&tops, ChildMode::ForcerTree)
}

fn kind_name(eps_lines: u8) -> &'static str {
    if eps_lines > 0 { "eps" } else { "int" }
}

/// The lane of every node, by `decide`; `decide` failing on a node is a failure
/// of the canary.
fn lanes(dag: &Dag) -> Vec<OwnerLane> {
    dag.nodes
        .iter()
        .map(|node| {
            decide(node)
                .unwrap_or_else(|e| panic!("decide on node {}: {e:?}", node.index))
                .lane
        })
        .collect()
}

/// (L, int/eps, action) -> nodes.
fn action_table(dag: &Dag) -> BTreeMap<(u8, &'static str, &'static str), usize> {
    let mut table = BTreeMap::new();
    for node in &dag.nodes {
        let key = (
            node.loops,
            kind_name(node.eps_lines),
            node.action.kind().as_str(),
        );
        *table.entry(key).or_default() += 1;
    }
    table
}

/// `decide` on the 6,571 nodes against the census: the actions are the census's
/// and the lanes are the actions, split and merged as `decide` documents.
#[test]
#[ignore = "slow: 5L ForcerTree DAG from Forcer's 64 tops and decide on its 6,571 nodes"]
fn lanes_5l_counts() {
    let dag = forcer_tree(&TopologyService::new(), 5);
    let lanes = lanes(&dag);
    assert_eq!(dag.nodes.len(), 6571);

    let census: BTreeMap<_, _> = CENSUS_L5
        .iter()
        .map(|&(l, kind, action, count)| ((l, kind, action), count))
        .collect();
    assert_eq!(action_table(&dag), census, "action table vs the census");

    let mut by_lane: BTreeMap<(&str, OwnerLane), usize> = BTreeMap::new();
    for (node, &lane) in dag.nodes.iter().zip(&lanes) {
        let action = node.action.kind().as_str();
        *by_lane.entry((action, lane)).or_default() += 1;
    }
    let count = |action: &str| -> usize {
        by_lane
            .iter()
            .filter(|((a, _), _)| *a == action)
            .map(|(_, &n)| n)
            .sum()
    };
    // Custom 283 in the census; decide puts the 42 diamonds on the Custom lane
    // until M3 (plan decision D11), so the lane count is 325.
    assert_eq!((count("custom"), count("diamond")), (283, 42));
    assert_eq!(by_lane[&("custom", OwnerLane::Custom)], 283);
    assert_eq!(by_lane[&("diamond", OwnerLane::Custom)], 42);
    assert_eq!(by_lane[&("carpet", OwnerLane::Carpet)], count("carpet"));
    assert_eq!(
        by_lane[&("triangle", OwnerLane::Triangle)],
        count("triangle")
    );
    assert_eq!(by_lane[&("trivial", OwnerLane::Zero)], 1);
    // The census's 5,212 insertion nodes, split: 1,214 series-parallel (5L: 547,
    // the census's `sp` nodes), 3,998 Insertion.
    assert_eq!(count("insertion"), 5212);
    assert_eq!(by_lane[&("insertion", OwnerLane::SeriesParallel)], 1214);
    assert_eq!(by_lane[&("insertion", OwnerLane::Insertion)], 3998);
    let sp_5l = (dag.nodes.iter().zip(&lanes))
        .filter(|(node, lane)| node.loops == 5 && **lane == OwnerLane::SeriesParallel)
        .count();
    assert_eq!(sp_5l, 547);
    eprintln!("{by_lane:#?}");
}

/// The topology of a closure-study sector: lines with mask bit 0 contracted.
fn sector_topo(line_edges: &[(u8, u8)], cut: (u8, u8), mask: &str) -> Normalised {
    fn root(parent: &mut [u8], x: u8) -> u8 {
        let mut r = x;
        while parent[usize::from(r)] != r {
            r = parent[usize::from(r)];
        }
        parent[usize::from(x)] = r;
        r
    }
    let vertices = line_edges
        .iter()
        .flat_map(|&(u, v)| [u, v])
        .chain([cut.0, cut.1])
        .max()
        .expect("a line")
        + 1;
    let mut parent: Vec<u8> = (0..vertices).collect();
    let kept: Vec<bool> = mask
        .bytes()
        .take(line_edges.len())
        .map(|b| b == b'1')
        .collect();
    for (&(u, v), &keep) in line_edges.iter().zip(&kept) {
        if !keep {
            let (a, b) = (root(&mut parent, u), root(&mut parent, v));
            if a == b {
                // A cycle of contracted lines leaves a loop with no scale.
                return Normalised::Zero;
            }
            parent[usize::from(a)] = b;
        }
    }
    let lines: Vec<(u8, u8, u8)> = line_edges
        .iter()
        .zip(&kept)
        .filter(|&(_, &keep)| keep)
        .map(|(&(u, v), _)| (root(&mut parent, u), root(&mut parent, v), 0))
        .collect();
    let (s, t) = (root(&mut parent, cut.0), root(&mut parent, cut.1));
    normalise(&PTopo::new(vertices, s, t, lines))
}

fn key_of(normalised: Normalised) -> Option<GraphKey> {
    match normalised {
        Normalised::Zero => None,
        Normalised::Trivial => Some(canonical(&PTopo::trivial()).0),
        Normalised::Topo(topo, _) => Some(canonical(&topo).0),
    }
}

#[test]
fn a_contracted_line_gives_the_sector_of_the_graph() {
    // The kite; contracting its central line gives two bubbles in a chain, and
    // contracting a cycle of lines leaves no scale.
    let kite = [(0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    let lines = |edges: &[(u8, u8)]| edges.iter().map(|&(u, v)| (u, v, 0)).collect::<Vec<_>>();
    let whole = PTopo::new(4, 0, 1, lines(&kite));
    assert_eq!(
        key_of(sector_topo(&kite, (0, 1), "11111")),
        key_of(normalise(&whole))
    );
    let chain = PTopo::new(3, 0, 1, lines(&[(0, 2), (0, 2), (1, 2), (1, 2)]));
    assert_eq!(
        key_of(sector_topo(&kite, (0, 1), "11110")),
        key_of(normalise(&chain))
    );
    assert_eq!(key_of(sector_topo(&kite, (0, 1), "00000")), None);
}

/// For every node: does a custom node lie below it (itself excluded)?
fn custom_below(dag: &Dag) -> Vec<bool> {
    let mut below = vec![false; dag.nodes.len()];
    // Every edge points to a larger index.
    for index in (0..dag.nodes.len()).rev() {
        let hit = dag.edges_from(index as u32).any(|edge| {
            let child = edge.child as usize;
            below[child] || dag.nodes[child].action.kind().as_str() == "custom"
        });
        below[index] = hit;
    }
    below
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A node of a census table.
struct CensusNode {
    /// `L<loops> int|eps <action>`.
    name: String,
    action: String,
    /// The lane of the first draft (M1): `sp`, `tri`, `gen`.
    m1_lane: String,
}

/// The nodes of a census table by our key.
fn census_nodes(table: &Value) -> BTreeMap<GraphKey, CensusNode> {
    let mut nodes = BTreeMap::new();
    for node in table["node_table"].as_array().expect("node_table") {
        // The trivial node has no graph in the table.
        let topo = match node["edges"].as_array() {
            None => PTopo::trivial(),
            Some(edges) => {
                let lines: Vec<(u8, u8, u8)> = edges
                    .iter()
                    .map(|e| {
                        let v =
                            |i: usize| u8::try_from(e[i].as_u64().expect("vertex")).expect("small");
                        (v(0), v(1), v(2))
                    })
                    .collect();
                let vertex =
                    |name: &str| u8::try_from(node[name].as_u64().expect(name)).expect("small");
                let (s, t) = (vertex("S"), vertex("T"));
                let n = lines
                    .iter()
                    .flat_map(|&(u, v, _)| [u, v])
                    .chain([s, t])
                    .max()
                    .expect("a vertex")
                    + 1;
                PTopo::new(n, s, t, lines)
            }
        };
        let key = key_of(normalise(&topo)).expect("a census node is nonzero");
        let action = node["action"].as_str().expect("action").to_owned();
        let census_node = CensusNode {
            name: format!(
                "L{} {} {action}",
                node["L"],
                kind_name(u8::from(node["neps"].as_u64() != Some(0))),
            ),
            action,
            m1_lane: node["m1_lane"].as_str().unwrap_or("-").to_owned(),
        };
        assert!(
            nodes.insert(key, census_node).is_none(),
            "two census nodes with one key"
        );
    }
    nodes
}

/// Drift of a node set against a census table: nodes on one side only, by
/// (L, int/eps, action), and action mismatches on the common nodes.
fn census_drift(dag: &Dag, table: &Value) -> Value {
    let census = census_nodes(table);
    let mut only_ours: BTreeMap<String, usize> = BTreeMap::new();
    let mut mismatches = Vec::new();
    for node in &dag.nodes {
        let action = node.action.kind().as_str();
        match census.get(&node.key) {
            None => {
                let name = format!("L{} {} {action}", node.loops, kind_name(node.eps_lines));
                *only_ours.entry(name).or_default() += 1;
            }
            Some(theirs) if theirs.action != action => {
                mismatches.push(json!([node.key.to_hex(), action, theirs.action]));
            }
            Some(_) => {}
        }
    }
    let mut only_census: BTreeMap<String, usize> = BTreeMap::new();
    for (key, node) in &census {
        if dag.find(key).is_none() {
            *only_census.entry(node.name.clone()).or_default() += 1;
        }
    }
    json!({
        "ours": dag.nodes.len(),
        "census": census.len(),
        "only_ours": only_ours,
        "only_census": only_census,
        "action_mismatches": mismatches,
    })
}

/// The nonzero integer sector classes of the tops, the eager Demand roots
/// (census mode `eall`): the contraction closure in the ForcerTree DAG.
fn eall_roots(dag: &Dag, tops: &[PTopo]) -> Vec<PTopo> {
    let mut seen = BTreeSet::new();
    let mut work: Vec<u32> = tops
        .iter()
        .map(|top| {
            let key = key_of(normalise(top)).expect("a top is nonzero");
            dag.find(&key).expect("a top is a node").index
        })
        .collect();
    while let Some(index) = work.pop() {
        if seen.insert(index) {
            let children = dag
                .edges_from(index)
                .filter(|edge| matches!(edge.kind, DagEdgeKind::Contract(_)))
                .map(|edge| edge.child);
            work.extend(children);
        }
    }
    seen.into_iter()
        .map(|index| dag.nodes[index as usize].topo.clone())
        .collect()
}

/// The status of an E1 row, as the closure study's own `summarize_e1.py`.
fn e1_status(row: &Value) -> &'static str {
    if row["timeout"].as_bool() == Some(true) {
        "TIMEOUT"
    } else if row["rc"].as_i64() != Some(0) || row["result"].get("error").is_some() {
        "FAIL"
    } else {
        "OK"
    }
}

/// Env-driven entries of the driver. Each refuses to run without its variables.
mod run {
    use super::*;

    /// A required `CANARY_*` variable:
    /// `CANARY_SYM` family file, or `embedded:<name>` for a repo family;
    /// `CANARY_STARS` forced axes expected, a mask in the file's slot order;
    /// `CANARY_CHART` the chart; `CANARY_WORKERS` default 1;
    /// `CANARY_INFO` the info file of the family;
    /// `CANARY_OUT` the file or directory to write;
    /// `CANARY_ROWS` the chart rows of a family, JSON lines;
    /// `CANARY_POINTS` the number of random points of `closure`;
    /// `CANARY_CENSUS_DIR`, `CANARY_E1_DIR`, `CANARY_NP4_DIR` inputs of the
    /// lanes report. Optional: `CANARY_LP_GIVEN` an LP from the targets table,
    /// which the family's rows then use instead of a count.
    pub(super) fn required(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set (run through drive.py)"))
    }

    pub(super) fn family_source() -> (String, String) {
        let file = required("CANARY_SYM");
        let source = match file.strip_prefix("embedded:") {
            Some(name) => eps_family_source(name).to_owned(),
            None => fs::read_to_string(&file).unwrap_or_else(|e| panic!("{file}: {e}")),
        };
        (file, source)
    }

    fn write(path: &str, text: String) {
        fs::write(path, text).unwrap_or_else(|e| panic!("{path}: {e}"));
    }

    #[test]
    #[ignore = "run harness: axes, charts and LP of one family file (drive.py)"]
    fn family_info() {
        let (file, source) = family_source();
        let stars = parse_mask(&required("CANARY_STARS")).unwrap();
        let given = std::env::var("CANARY_LP_GIVEN").ok();
        let given = given.map(|lp| lp.parse().expect("CANARY_LP_GIVEN"));
        let axes = family_axes(&file, &source, &stars, given).unwrap();
        write(
            &required("CANARY_OUT"),
            serde_json::to_string(&axes).unwrap(),
        );
    }

    #[test]
    #[ignore = "run harness: one chart of one family file, one process (drive.py)"]
    fn chart() {
        let (file, source) = family_source();
        let axes: FamilyAxes =
            serde_json::from_value(read_json(required("CANARY_INFO").as_ref())).unwrap();
        let stars = parse_mask(&required("CANARY_STARS")).unwrap();
        let checked = family_axes(&file, &source, &stars, Some(axes.lp)).unwrap();
        assert_eq!(
            (&checked.forced, &checked.support, &checked.charts),
            (&axes.forced, &axes.support, &axes.charts),
            "{file} is not the family of its info file",
        );
        let workers = std::env::var("CANARY_WORKERS").map_or(1, |w| w.parse().expect("workers"));
        let row = chart_row(&axes, &source, &required("CANARY_CHART"), workers);
        write(
            &required("CANARY_OUT"),
            serde_json::to_string(&row).unwrap(),
        );
    }

    #[test]
    #[ignore = "run harness: the hard guard on the union of a family's chart rows (drive.py)"]
    fn union_guard() {
        let axes: FamilyAxes =
            serde_json::from_value(read_json(required("CANARY_INFO").as_ref())).unwrap();
        let rows: Vec<ChartRow> = fs::read_to_string(required("CANARY_ROWS"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let verdict = union_row(&axes, &rows);
        match verdict {
            Ok(union) => write(
                &required("CANARY_OUT"),
                serde_json::to_string(&union).unwrap(),
            ),
            Err(failure) => panic!("GATE FAILURE {}: {failure}", axes.family),
        }
    }

    /// `CANARY_POINTS` random owner keys (eps powers in -2..=3, at most two
    /// dots, rank at most 2 on the ISPs) through every chart's rules at
    /// `d = 37/10`, each step with its strict descent witness. A check of a
    /// generated program on points, count of steps and exits reported; a pole of
    /// a rule coefficient at this `d` is a refusal of the applier, not a verdict.
    #[test]
    #[ignore = "run harness: random points through the rules of a family file (drive.py)"]
    fn closure() {
        let (file, source) = family_source();
        let points: u64 = required("CANARY_POINTS").parse().expect("CANARY_POINTS");
        let outcome = generate_eps_family(&source, EpsCharts::All).unwrap();
        let (mut closed, mut poles, mut steps, mut witnesses) = (0u64, 0u64, 0usize, 0usize);
        let mut with_residual = 0u64;
        let mut exits = BTreeSet::new();
        let mut failures = Vec::new();
        let mut state = 0x5eed_u64;
        let mut next = |low: i64, high: i64| -> i64 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            low + ((state >> 33) % (high - low + 1) as u64) as i64
        };
        let slots = outcome.support.len();
        for _ in 0..points {
            let mut key = vec![0_i64; slots];
            let lines: Vec<usize> = (0..slots)
                .filter(|&i| outcome.support[i] && !outcome.forced[i])
                .collect();
            for &i in &lines {
                key[i] = 1;
            }
            for _ in 0..next(0, 2) {
                key[lines[next(0, lines.len() as i64 - 1) as usize]] += 1;
            }
            let isps: Vec<usize> = (0..slots).filter(|&i| !outcome.support[i]).collect();
            if !isps.is_empty() {
                for _ in 0..next(0, 2) {
                    key[isps[next(0, isps.len() as i64 - 1) as usize]] -= 1;
                }
            }
            for (power, &forced) in key.iter_mut().zip(&outcome.forced) {
                if forced {
                    *power = next(-2, 3);
                }
            }
            match outcome.reduce(&key) {
                Ok(reduction) => {
                    closed += 1;
                    steps += reduction.steps;
                    witnesses += reduction.witnesses;
                    with_residual += u64::from(!reduction.residuals.is_empty());
                    exits.extend(reduction.exits.into_keys());
                }
                Err(error) if error.contains("has a pole") => poles += 1,
                Err(error) => failures.push(format!("{key:?}: {error}")),
            }
        }
        let row = json!({
            "file": file,
            "family": outcome.name,
            "rules": outcome.rules,
            "residuals": outcome.residual_keys,
            "lp": outcome.lp,
            "points": points,
            "closed": closed,
            "with_residual": with_residual,
            "pole_refusals": poles,
            "steps": steps,
            "witnesses": witnesses,
            "distinct_exits": exits.len(),
            "failures": failures,
            "seconds": outcome.seconds,
        });
        write(&required("CANARY_OUT"), row.to_string());
        assert!(failures_empty(&row), "{row}");
    }

    fn failures_empty(row: &Value) -> bool {
        row["failures"].as_array().is_some_and(Vec::is_empty)
    }

    fn e1_rows(path: &Path) -> Vec<Value> {
        fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .lines()
            .filter(|line| line.starts_with('{'))
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn edge(value: &Value) -> (u8, u8) {
        let v = |i: usize| u8::try_from(value[i].as_u64().expect("vertex")).expect("small");
        (v(0), v(1))
    }

    #[test]
    #[ignore = "run harness: 5L lanes against the census tables and the E1 batches (drive.py)"]
    fn lanes_5l_report() {
        let out = PathBuf::from(required("CANARY_OUT"));
        fs::create_dir_all(&out).unwrap();
        let census_dir = PathBuf::from(required("CANARY_CENSUS_DIR"));
        let e1_dir = PathBuf::from(required("CANARY_E1_DIR"));

        let service = TopologyService::new();
        let start = std::time::Instant::now();
        let dag = forcer_tree(&service, 5);
        let dag_seconds = start.elapsed().as_secs_f64();
        let start = std::time::Instant::now();
        let lanes = lanes(&dag);
        let decide_seconds = start.elapsed().as_secs_f64();
        let below = custom_below(&dag);

        // Node table.
        let mut nodes =
            String::from("index\tkey\tL\tt\teps_lines\taction\tsp\tlane\tcustom_below\n");
        for (node, lane) in dag.nodes.iter().zip(&lanes) {
            nodes += &format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{}\n",
                node.index,
                node.key.to_hex(),
                node.loops,
                node.t,
                node.eps_lines,
                node.action.kind().as_str(),
                node.series_parallel,
                lane,
                below[node.index as usize],
            );
        }
        write(out.join("nodes.tsv").to_str().unwrap(), nodes);

        // The DAG and the Demand DAG, node by node, against the census.
        let ftops = read_json(&census_dir.join("c_L5_ftops_forcer.json"));
        let eall = read_json(&census_dir.join("c_L5_eall_forcer.json"));
        let tops: Vec<PTopo> = forcer::forcer_tops(5).into_iter().map(|(_, t)| t).collect();
        let demand = service.reduction_graph(&eall_roots(&dag, &tops), ChildMode::Demand);
        let below_demand = custom_below(&demand);
        let demand_custom = demand
            .nodes
            .iter()
            .filter(|node| node.action.kind().as_str() == "custom")
            .count();

        // Forcer's names of the nodes of its 4L tree.
        let names = forcer::forcer_names_by_key(&forcer::forcer_dag());

        // The sectors of E1, batch A (tops), batch B (lower sectors), E4, and
        // the 4L non-planar refusals of the AD's table.
        let np4_dir = PathBuf::from(required("CANARY_NP4_DIR"));
        let mut families: BTreeMap<String, Value> = BTreeMap::new();
        for path in [
            e1_dir.join("families/index.json"),
            np4_dir.join("index.json"),
        ] {
            for family in read_json(&path).as_array().unwrap() {
                let name = family["family"].as_str().unwrap().to_owned();
                families.insert(name, family.clone());
            }
        }
        let census = census_nodes(&ftops);
        let mut table: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
        let mut rows = String::from(
            "batch\tfamily\tsector\tt\tnode\tL\taction\tname\tlane\tm1_lane\tcustom_below_tree\tcustom_below_demand\tstatus\twall\tmaxrss_kb\trules\tresiduals\n",
        );
        for (batch, path) in [
            ("A", e1_dir.join("runs/e1/resultsA.jsonl")),
            ("B", e1_dir.join("runs/e1/resultsB.jsonl")),
            ("E4", e1_dir.join("runs/e4/results.jsonl")),
            ("NP4", np4_dir.join("results.jsonl")),
        ] {
            for row in e1_rows(&path) {
                let family = row["family"].as_str().unwrap();
                let sector = row["sector"].as_str().unwrap();
                let entry = &families[family];
                let line_edges: Vec<(u8, u8)> = entry["line_edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(edge)
                    .collect();
                let cut = edge(&entry["cut_edge"]);
                let key = key_of(sector_topo(&line_edges, cut, sector))
                    .unwrap_or_else(|| panic!("{family} {sector} is zero"));
                let node = dag
                    .find(&key)
                    .unwrap_or_else(|| panic!("{family} {sector} is not a DAG node"));
                let index = node.index as usize;
                let action = node.action.kind().as_str();
                let status = e1_status(&row);
                let result = &row["result"];
                rows += &format!(
                    "{batch}\t{family}\t{sector}\t{}\t{}\t{}\t{action}\t{}\t{:?}\t{}\t{}\t{}\t{status}\t{}\t{}\t{}\t{}\n",
                    sector[..line_edges.len()].matches('1').count(),
                    node.index,
                    node.loops,
                    names.get(&key).map_or("-", String::as_str),
                    lanes[index],
                    census.get(&key).map_or("-", |node| node.m1_lane.as_str()),
                    below[index],
                    demand
                        .find(&key)
                        .is_some_and(|node| below_demand[node.index as usize]),
                    row["wall"],
                    row["maxrss_kb"],
                    result["rules"],
                    result["finite_residuals"],
                );
                *table
                    .entry(format!("{batch} {action}"))
                    .or_default()
                    .entry(status.to_owned())
                    .or_default() += 1;
            }
        }
        write(out.join("e1.tsv").to_str().unwrap(), rows);

        let summary = json!({
            "dag_seconds": dag_seconds,
            "decide_seconds": decide_seconds,
            "nodes": dag.nodes.len(),
            "demand_nodes": demand.nodes.len(),
            "demand_custom": demand_custom,
            "drift_ftops": census_drift(&dag, &ftops),
            "drift_eall": census_drift(&demand, &eall),
            "e1": table,
        });
        write(
            out.join("lanes_5l.json").to_str().unwrap(),
            serde_json::to_string_pretty(&summary).unwrap(),
        );
    }
}
